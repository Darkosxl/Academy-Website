//! Agentic Harness: the student page, the interim team admin, and the worker API
//! the runner drives a submission through.

use crate::admin::{IdForm, MemberForm, NameForm};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use benchmark_protocol::{
    ArcFrame as HarnessArcFrame, ArcFramesRequest as HarnessArcFramesReq, DEFAULT_BEDROCK_MODEL,
    HarnessCapacity, HarnessClaim, HarnessLeaseRequest as HarnessLeaseReq,
    HarnessProgressRequest as HarnessProgressReq, HarnessResultRequest as HarnessResultReq,
    HarnessStageRequest as HarnessStageReq, KaggleClaim,
    KaggleResultRequest as HarnessKaggleResultReq, bedrock_model_supports_images,
    builtin_harness_uri, is_bedrock_model, is_builtin_harness,
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct HarnessQ {
    tab: Option<String>,
    bench: Option<String>,
    /// `?tab=live&run=` — a finished run to replay. Absent means "watch the latest one".
    run: Option<Uuid>,
}

/// The team the student belongs to, if any. Teams are admin-assigned for now.
pub async fn harness_team_of(app: &App, uid: Uuid) -> Option<HarnessTeam> {
    sqlx::query_as(
        "select t.id, t.name from harness_team_members_exposure_academy tm
         join harness_teams_exposure_academy t on t.id = tm.team_id
         where tm.user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

pub async fn agentic_harness(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // unknown values fall back to defaults, same as /demos?lang=
    let tab = match q.tab.as_deref() {
        Some("history") => "history",
        Some("instructions") => "instructions",
        Some("live") => "live",
        _ => "main",
    };
    if tab == "instructions" {
        return Ok(Html(html::agentic_harness_instructions(&user)));
    }
    if tab == "live" {
        // Team-scoped exactly like harness_arc_live below: a ?run= belonging to another
        // team matches no row, so the page renders empty instead of their boards.
        let run: Option<HarnessRun> = sqlx::query_as(
            "select r.id, r.repo_url, r.model_id, r.commit_sha, r.stage, r.benchmark_version,
                    r.benchmark_state, r.bedrock_profile, r.deadline_at, r.score_arc,
                    r.score_frontier, r.ram_1session_mb, r.ram_10session_mb,
                    r.error_log, r.created_at
             from harness_runs_exposure_academy r
             join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
             where tm.user_id = $1 and ($2::uuid is null or r.id = $2)
             order by r.created_at desc limit 1",
        )
        .bind(user.id)
        .bind(q.run)
        .fetch_optional(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        return Ok(Html(html::agentic_harness_live(
            &user,
            run.as_ref(),
            q.run.is_some(),
        )));
    }
    let team = harness_team_of(&app, user.id).await;
    if tab == "history" {
        let runs: Vec<HarnessRun> = match &team {
            Some(t) => sqlx::query_as(
                "select id, repo_url, model_id, commit_sha, stage, benchmark_version,
                        benchmark_state, bedrock_profile, deadline_at, score_arc,
                        score_frontier, ram_1session_mb, ram_10session_mb,
                        error_log, created_at
                 from harness_runs_exposure_academy where team_id = $1 order by created_at desc",
            )
            .bind(t.id)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
            None => Vec::new(),
        };
        let credential_username: Option<String> = match &team {
            Some(t) => sqlx::query_scalar(
                "select username from harness_kaggle_credentials_exposure_academy where team_id = $1")
                .bind(t.id).fetch_optional(&app.pool).await.unwrap(),
            None => None,
        };
        let official: Vec<HarnessKaggleSubmission> = match &team {
            Some(t) => sqlx::query_as(
                "select j.run_id, j.status, j.kernel_slug, j.kernel_version,
                        j.submission_ref, j.public_score, j.private_score,
                        j.status_message, j.updated_at
                 from harness_kaggle_submissions_exposure_academy j
                 join harness_runs_exposure_academy r on r.id = j.run_id
                 where r.team_id = $1 order by j.created_at desc",
            )
            .bind(t.id)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
            None => Vec::new(),
        };
        return Ok(Html(html::agentic_harness_history(
            &user,
            team.as_ref(),
            &runs,
            app.kaggle_key.is_some(),
            credential_username.as_deref(),
            &official,
        )));
    }
    let bench = match q.bench.as_deref() {
        Some("frontier") => "frontier",
        Some("ram") => "ram",
        _ => "arc",
    };
    // Kid names shown next to each team, real names per the leaderboard convention.
    // One query for all teams; html filters per row in memory, same as board() does
    // with interests. `public` = onboarded and not hidden — the leaderboard shows only
    // those, but your own team panel shows the full roster (admins/interns included,
    // it's your roster, not the published standings).
    let members: Vec<TeamMemberRow> = sqlx::query_as(
        "select tm.team_id, tm.user_id, u.display_name,
                (u.nickname is not null and not u.hidden_from_leaderboard) as public
         from harness_team_members_exposure_academy tm
         join users_exposure_academy u on u.id = tm.user_id
         order by tm.created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let active_run: Option<HarnessRun> = match &team {
        Some(t) => sqlx::query_as(
            "select id, repo_url, model_id, commit_sha, stage, benchmark_version,
                    benchmark_state, bedrock_profile, deadline_at, score_arc,
                    score_frontier, ram_1session_mb, ram_10session_mb,
                    error_log, created_at
             from harness_runs_exposure_academy
             where team_id = $1 and stage not in ('done','partial','failed','infra_failed')",
        )
        .bind(t.id)
        .fetch_optional(&app.pool)
        .await
        .unwrap(),
        None => None,
    };
    // Best score per team over its done runs. ARC/Frontier: higher wins. RAM: ranked
    // by the lowest 10-session PSS, and the 1-session column comes from that same run
    // (distinct on picks it), not from whichever run happened to have the lowest 1s.
    let (rows, ram_rows): (Vec<HarnessLeaderRow>, Vec<HarnessRamRow>) = if bench == "ram" {
        (
            Vec::new(),
            sqlx::query_as(
                "select id, name, ram_1session_mb, ram_10session_mb from (
               select distinct on (t.id) t.id, t.name, r.ram_1session_mb, r.ram_10session_mb
               from harness_teams_exposure_academy t
               join harness_runs_exposure_academy r
                 on r.team_id = t.id and r.benchmark_version = $1
                    and r.ram_10session_mb is not null
               order by t.id, r.ram_10session_mb asc, r.created_at desc
             ) best order by ram_10session_mb asc, lower(name)",
            )
            .bind(HARNESS_VERSION)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
        )
    } else {
        let sql = if bench == "frontier" {
            "select t.id, t.name, max(r.score_frontier) as best
             from harness_teams_exposure_academy t
             join harness_runs_exposure_academy r
               on r.team_id = t.id and r.benchmark_version = $1
                  and r.score_frontier is not null
             group by t.id, t.name order by best desc, lower(t.name)"
        } else {
            "select t.id, t.name, max(r.score_arc) as best
             from harness_teams_exposure_academy t
             join harness_runs_exposure_academy r
               on r.team_id = t.id and r.benchmark_version = $1
                  and r.score_arc is not null
             group by t.id, t.name order by best desc, lower(t.name)"
        };
        (
            sqlx::query_as(sql)
                .bind(HARNESS_VERSION)
                .fetch_all(&app.pool)
                .await
                .unwrap(),
            Vec::new(),
        )
    };
    Ok(Html(html::agentic_harness_main(
        &user,
        bench,
        team.as_ref(),
        &members,
        active_run.as_ref(),
        &rows,
        &ram_rows,
    )))
}

#[derive(Deserialize)]
pub struct HarnessSubmitForm {
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    model_id: String,
    #[serde(default)]
    builtin_harness: String,
}

fn github_repo_url(raw: &str) -> Option<String> {
    if raw.len() > 300 {
        return None;
    }
    let url = reqwest::Url::parse(raw.trim()).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let parts: Vec<_> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 2 {
        return None;
    }
    let repo = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !valid(parts[0]) || !valid(repo) || repo == "." || repo == ".." {
        return None;
    }
    Some(format!("https://github.com/{}/{repo}", parts[0]))
}

fn submitted_model_id(raw: &str, requires_images: bool) -> Option<&str> {
    let model_id = match raw.trim() {
        "" => Some(DEFAULT_BEDROCK_MODEL),
        model_id if is_bedrock_model(model_id) => Some(model_id),
        _ => None,
    }?;
    (!requires_images || bedrock_model_supports_images(model_id)).then_some(model_id)
}

fn submission_source(is_admin: bool, repo_url: &str, builtin_harness: &str) -> Option<String> {
    let repo_url = repo_url.trim();
    let builtin_harness = builtin_harness.trim();
    match (repo_url.is_empty(), builtin_harness.is_empty()) {
        (false, true) => github_repo_url(repo_url),
        (true, false) if is_admin => builtin_harness_uri(builtin_harness).map(String::from),
        _ => None,
    }
}

pub async fn harness_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessSubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();
    let Some(team) = harness_team_of(&app, user.id).await else {
        // the form isn't rendered for team-less students; this catches hand-rolled POSTs
        return Err(bad("Bir takımda değilsin — eğitmenine yaz."));
    };
    let Some(repo_url) = submission_source(user.is_admin, &f.repo_url, &f.builtin_harness) else {
        return Err(bad(if user.is_admin {
            "GitHub bağlantısı gir veya bir hazır harness seç."
        } else {
            "Repo bağlantısı https://github.com/ ile başlamalı."
        }));
    };
    let Some(model_id) = submitted_model_id(&f.model_id, is_builtin_harness(&repo_url)) else {
        return Err(bad(
            "Bu agent için görüntü destekleyen geçerli bir model seç.",
        ));
    };
    let in_flight = "Takımının devam eden bir çalıştırması var — bitmesini bekleyin.";
    let active: Option<Uuid> = sqlx::query_scalar(
        "select id from harness_runs_exposure_academy where team_id = $1
         and stage not in ('done','partial','failed','infra_failed')",
    )
    .bind(team.id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    if active.is_some() {
        return Err(bad(in_flight));
    }
    // The one-active-run partial unique index backstops the pre-check above, so a
    // double-click race lands here as a constraint error, not a second run — map it
    // to the same friendly message instead of unwrap-panicking into a 500.
    sqlx::query(
        "insert into harness_runs_exposure_academy
           (team_id, submitted_by, repo_url, model_id, benchmark_version)
         values ($1,$2,$3,$4,$5)",
    )
    .bind(team.id)
    .bind(user.id)
    .bind(&repo_url)
    .bind(model_id)
    .bind(HARNESS_VERSION)
    .execute(&app.pool)
    .await
    .map_err(|_| bad(in_flight))?;
    Ok(Redirect::to("/agentic-harness"))
}

/// Tiny JSON the stepper polls. The server resolves "my team's latest run" from the
/// session — no run id in the URL, so nothing cross-team is addressable.
pub async fn harness_status(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let row: Option<(
        String,
        Option<String>,
        Value,
        Option<DateTime<Utc>>,
        String,
        Option<String>,
    )> = sqlx::query_as(
        "select r.stage, r.commit_sha, r.benchmark_state, r.deadline_at,
                r.benchmark_version, r.bedrock_profile
         from harness_runs_exposure_academy r
         join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
         where tm.user_id = $1 order by r.created_at desc limit 1",
    )
    .bind(user.id)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(match row {
        Some((stage, sha, benchmarks, deadline, version, profile)) => serde_json::json!({
            "stage": stage,
            "commit_sha": sha,
            "benchmarks": benchmarks,
            "deadline_at": deadline,
            "benchmark_version": version,
            "bedrock_profile": profile,
        }),
        None => serde_json::json!({"stage": null}),
    }))
}

#[derive(Deserialize)]
pub struct ArcLiveQ {
    run: Option<Uuid>,
    game: Option<String>,
    seq: Option<i32>,
}

/// One board per game, plus the frames the focused game has produced since the seq the
/// viewer last saw. Same rule as `harness_status`: the run is always reached through the
/// caller's team membership, so a run id in the URL is not cross-team addressable — it
/// either belongs to the caller's team or resolves to nothing.
pub async fn harness_arc_live(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<ArcLiveQ>,
) -> Result<Json<Value>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let run: Option<(Uuid, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "select r.id, r.stage, r.deadline_at from harness_runs_exposure_academy r
         join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
         where tm.user_id = $1 and ($2::uuid is null or r.id = $2)
         order by r.created_at desc limit 1",
    )
    .bind(user.id)
    .bind(q.run)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let Some((run_id, stage, deadline_at)) = run else {
        return Ok(Json(serde_json::json!({
            "run": null, "stage": null, "deadline_at": null, "games": [], "focus": null,
        })));
    };
    let boards: Vec<ArcBoardRow> = sqlx::query_as(
        // right(grids, 4096) is the last grid: every grid is exactly 4096 chars and they are
        // newline-joined, so the tail is the resulting board. Selecting the whole animation
        // buffer here and discarding it in Rust cost up to 64KB per game per poll.
        "select distinct on (game) game, seq, state, levels_completed, baseline,
                right(grids, 4096) as grids
         from harness_arc_frames_exposure_academy
         where run_id = $1 order by game, seq desc",
    )
    .bind(run_id)
    .fetch_all(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    // seq 0 is a real frame (the initial observation), so "seen nothing yet" is -1.
    let since = q.seq.unwrap_or(-1);
    let focus = match q.game.as_deref().filter(|g| g.len() == 4) {
        None => Value::Null,
        Some(game) => {
            let mut frames: Vec<ArcFocusRow> = sqlx::query_as(
                "select seq, grids, action, action_x, action_y, state, levels_completed
                 from harness_arc_frames_exposure_academy
                 where run_id = $1 and game = $2 and seq > $3 order by seq limit 201",
            )
            .bind(run_id)
            .bind(game)
            .bind(since)
            .fetch_all(&app.pool)
            .await
            .map_err(worker_db_unavailable)?;
            let has_more = frames.len() > 200;
            frames.truncate(200);
            let next_seq = frames.last().map(|frame| frame.seq).unwrap_or(since);
            serde_json::json!({
                "game": game,
                "next_seq": next_seq,
                "has_more": has_more,
                "frames": frames.iter().map(|f| serde_json::json!({
                    "seq": f.seq,
                    // the whole animation buffer: the focused board plays it back
                    "grids": f.grids.split('\n').collect::<Vec<_>>(),
                    "action": f.action, "action_x": f.action_x, "action_y": f.action_y,
                    "state": f.state, "levels_completed": f.levels_completed,
                })).collect::<Vec<_>>(),
            })
        }
    };
    Ok(Json(serde_json::json!({
        "run": run_id,
        "stage": stage,
        "deadline_at": deadline_at,
        // `distinct on` already picked each game's highest seq, so seq and total are the
        // same number here — the viewer reads one as its cursor and one as the count.
        "games": boards.iter().map(|b| serde_json::json!({
            "game": b.game, "seq": b.seq, "total": b.seq, "state": b.state,
            "levels_completed": b.levels_completed, "baseline": b.baseline,
            // the resulting board only, not the buffer that led to it
            "grid": b.grids.rsplit('\n').next().unwrap_or(""),
        })).collect::<Vec<_>>(),
        "focus": focus,
    })))
}

#[derive(sqlx::FromRow)]
struct ArcBoardRow {
    game: String,
    seq: i32,
    state: String,
    levels_completed: i32,
    baseline: Option<Vec<i32>>,
    grids: String,
}

#[derive(sqlx::FromRow)]
struct ArcFocusRow {
    seq: i32,
    grids: String,
    action: Option<String>,
    action_x: Option<i32>,
    action_y: Option<i32>,
    state: String,
    levels_completed: i32,
}

fn encrypt_kaggle_token(
    app: &App,
    team_id: Uuid,
    token: &str,
) -> Result<(Vec<u8>, Vec<u8>), Response> {
    let key = app
        .kaggle_key
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: token.as_bytes(),
                aad: team_id.as_bytes(),
            },
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok((nonce.to_vec(), ciphertext))
}

fn decrypt_kaggle_token(
    app: &App,
    team_id: Uuid,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<String, Response> {
    let key = app
        .kaggle_key
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    if nonce.len() != 24 {
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: team_id.as_bytes(),
            },
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    String::from_utf8(plaintext).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[derive(Deserialize)]
pub struct HarnessKaggleCredentialForm {
    username: String,
    token: String,
}

pub async fn harness_kaggle_credentials(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessKaggleCredentialForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let username = f.username.trim();
    let token = f.token.trim();
    if !(2..=64).contains(&username.len())
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        || !(20..=500).contains(&token.len())
        || token.chars().any(char::is_whitespace)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Geçersiz Kaggle kullanıcı adı veya token.",
        )
            .into_response());
    }
    let (nonce, ciphertext) = encrypt_kaggle_token(&app, team.id, token)?;
    sqlx::query(
        "insert into harness_kaggle_credentials_exposure_academy
           (team_id, username, token_nonce, token_ciphertext, updated_by, updated_at)
         values ($1,$2,$3,$4,$5,now())
         on conflict (team_id) do update set username = excluded.username,
           token_nonce = excluded.token_nonce, token_ciphertext = excluded.token_ciphertext,
           updated_by = excluded.updated_by, updated_at = now()",
    )
    .bind(team.id)
    .bind(username)
    .bind(nonce)
    .bind(ciphertext)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

pub async fn harness_kaggle_credentials_delete(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let active: bool = sqlx::query_scalar(
        "select exists(
           select 1 from harness_kaggle_submissions_exposure_academy j
           join harness_runs_exposure_academy r on r.id = j.run_id
           where r.team_id = $1 and j.status in ('queued','kernel_running','submitted'))",
    )
    .bind(team.id)
    .fetch_one(&app.pool)
    .await
    .unwrap_or(true);
    if active {
        return Err((
            StatusCode::CONFLICT,
            "Devam eden resmi gönderim varken Kaggle tokenı silinemez.",
        )
            .into_response());
    }
    sqlx::query("delete from harness_kaggle_credentials_exposure_academy where team_id = $1")
        .bind(team.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

#[derive(Deserialize)]
pub struct HarnessKaggleSubmitForm {
    run_id: Uuid,
}

pub async fn harness_kaggle_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessKaggleSubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let eligible: bool = sqlx::query_scalar(
        "select exists(
           select 1 from harness_runs_exposure_academy r
           join harness_kaggle_credentials_exposure_academy c on c.team_id = r.team_id
           where r.id = $1 and r.team_id = $2 and r.commit_sha is not null
             and r.score_arc is not null and r.benchmark_version = $3
             and r.repo_url like 'https://github.com/%'
         )",
    )
    .bind(f.run_id)
    .bind(team.id)
    .bind(HARNESS_VERSION)
    .fetch_one(&app.pool)
    .await
    .unwrap_or(false);
    if !eligible {
        return Err((
            StatusCode::BAD_REQUEST,
            "Bu çalıştırma resmi gönderime hazır değil.",
        )
            .into_response());
    }
    sqlx::query(
        "insert into harness_kaggle_submissions_exposure_academy
           (run_id, requested_by) values ($1,$2)
         on conflict (run_id) do update set
           requested_by = excluded.requested_by, status = 'queued',
           kernel_slug = null, kernel_version = null, submission_ref = null,
           public_score = null, private_score = null, status_message = null,
           lease_token = null, lease_expires_at = null, next_poll_at = null,
           last_result_lease_token = null, claim_attempts = 0, updated_at = now()
         where harness_kaggle_submissions_exposure_academy.status = 'failed'",
    )
    .bind(f.run_id)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

pub async fn admin_harness_team(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<NameForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let name = f.name.trim().to_string();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    // unique on lower(name); a duplicate is a silent no-op, the list makes it obvious
    sqlx::query(
        "insert into harness_teams_exposure_academy (name) values ($1) on conflict do nothing",
    )
    .bind(&name)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_harness_team_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades members AND runs — the confirm() on the button says so
    sqlx::query("delete from harness_teams_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_harness_member(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<MemberForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // user_id is the PK: assigning an already-assigned student moves them. Past runs
    // stay with the old team — runs are team-scoped, the score was the team's.
    sqlx::query(
        "insert into harness_team_members_exposure_academy (user_id, team_id) values ($1,$2)
         on conflict (user_id) do update set team_id = excluded.team_id",
    )
    .bind(f.user_id)
    .bind(f.team_id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_harness_member_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from harness_team_members_exposure_academy where user_id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// The stuck-run escape hatch: a worker that died after claiming leaves the run
/// non-terminal, which blocks the team's resubmits (one-active-run index). Failing it
/// here unblocks them; a late worker report against it then gets a 409 and is dropped.
pub async fn admin_harness_run_fail(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'failed', error_log = 'Yönetici tarafından durduruldu.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where id = $1 and stage not in ('done','partial','failed','infra_failed')",
    )
    .bind(f.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

fn worker_db_unavailable(error: sqlx::Error) -> Response {
    eprintln!("worker database operation failed: {error}");
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

/// One slot equals one controller/executor node. Include immediately claimable expired
/// leases and due Kaggle polls so the fleet follows actual claim demand, while excluding
/// jobs that are merely waiting for Kaggle's next poll time.
pub async fn worker_harness_capacity(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<HarnessCapacity>, Response> {
    check_worker(&app, &headers)?;
    let (queued, active, oldest_queued_seconds): (i64, i64, i64) = sqlx::query_as(
        "with claimable(created_at) as (
           select created_at from harness_runs_exposure_academy
           where stage = 'queued'
              or (stage in ('preparing','running') and lease_expires_at < now()
                  and deadline_at > now() and claim_attempts < 3)
           union all
           select created_at from harness_kaggle_submissions_exposure_academy
           where status = 'queued'
              or (status = 'kernel_running' and lease_expires_at < now()
                  and claim_attempts < 3)
              or (status = 'submitted' and coalesce(next_poll_at, now()) <= now()
                  and (lease_expires_at is null or lease_expires_at < now()))
         ), active_slots as (
           select id from harness_runs_exposure_academy
           where stage in ('preparing','running') and lease_expires_at >= now()
             and deadline_at > now()
           union all
           select id from harness_kaggle_submissions_exposure_academy
           where status in ('kernel_running','submitted') and lease_expires_at >= now()
         )
         select (select count(*) from claimable)::bigint,
                (select count(*) from active_slots)::bigint,
                greatest(0, coalesce(extract(epoch from now() -
                    (select min(created_at) from claimable))::bigint, 0))",
    )
    .fetch_one(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(HarnessCapacity {
        queued: queued.max(0) as u64,
        active: active.max(0) as u64,
        oldest_queued_seconds: oldest_queued_seconds.max(0) as u64,
    }))
}

pub async fn worker_harness_claim(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<Option<HarnessClaim>>, Response> {
    check_worker(&app, &headers)?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'infra_failed', error_log = 'Worker missed the deadline result grace period.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where stage in ('preparing','running')
           and deadline_at < now() - interval '30 seconds'",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'infra_failed', error_log = 'Worker lease expired three times.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where stage in ('preparing','running') and lease_expires_at < now()
           and deadline_at > now() and claim_attempts >= 3",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;

    let lease = Uuid::new_v4();
    let row: Option<(Uuid, String, DateTime<Utc>, String)> = sqlx::query_as(
        "with candidate as (
           select id from harness_runs_exposure_academy
           where stage = 'queued'
              or (stage in ('preparing','running') and lease_expires_at < now()
                  and deadline_at > now() and claim_attempts < 3)
           order by created_at
           for update skip locked
           limit 1
         )
         update harness_runs_exposure_academy r
         set stage = 'preparing', lease_token = $1,
             lease_expires_at = least(coalesce(r.deadline_at, now() + interval '600 seconds')
                                      + interval '30 seconds',
                                      now() + interval '90 seconds'),
             worker_heartbeat_at = now(),
             deadline_at = coalesce(r.deadline_at, now() + interval '600 seconds'),
             claim_attempts = claim_attempts + 1, updated_at = now()
         from candidate where r.id = candidate.id
         returning r.id, r.repo_url, r.deadline_at, r.model_id",
    )
    .bind(lease)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(row.map(|(id, repo_url, deadline_at, model_id)| {
        HarnessClaim {
            id,
            repo_url,
            model_id,
            lease_token: lease,
            deadline_at,
            benchmark_version: HARNESS_VERSION.into(),
        }
    })))
}

pub async fn worker_harness_heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessLeaseReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set lease_expires_at = least(deadline_at + interval '30 seconds',
                                      now() + interval '90 seconds'),
             worker_heartbeat_at = now(), updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage in ('preparing','running')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_harness_stage(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessStageReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.status != "running"
        || r.bedrock_profile.trim().is_empty()
        || r.bedrock_profile.len() > 120
        || r.bedrock_profile_fingerprint.len() != 64
        || !r
            .bedrock_profile_fingerprint
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let sha = r.commit_sha.trim();
    if !(7..=40).contains(&sha.len()) || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'running', commit_sha = $3, bedrock_profile = $4,
             bedrock_profile_fingerprint = lower($5), updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage = 'preparing'",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(sha.to_lowercase())
    .bind(r.bedrock_profile.trim())
    .bind(&r.bedrock_profile_fingerprint)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Repeatable mid-stage progress report — no stage transition, just the live blob
/// the student's stepper renders ("task 12/70 · score 5.7"). Capped and only
/// accepted while the run is still in flight; a stale report gets a 409.
pub async fn worker_harness_progress(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessProgressReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let encoded = r.state.to_string();
    if !matches!(r.benchmark.as_str(), "arc" | "frontier" | "ram")
        || !r.state.is_object()
        || encoded.len() > 8000
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set benchmark_state = jsonb_set(benchmark_state, array[$3], $4::jsonb, true),
             updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage = 'running'",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.benchmark)
    .bind(encoded)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Worker-supplied and headed for both SQL and the student's screen, so the shape is
/// checked instead of trusted. One grid is 4096 hex cells (the engine's camera is always
/// 64x64) and an animation buffer is at most 16 of them, newline separated.
/// Append-only frames for the live board viewer. Frames are cosmetic and best effort, but
/// they still carry the active lease: a zombie controller must not append to a reclaimed run.
pub async fn worker_harness_arc_frames(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessArcFramesReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.frames.len() > 64 || !r.frames.iter().all(HarnessArcFrame::is_valid) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let encoded = serde_json::to_string(&r.frames).unwrap();
    // One round trip, and deliberately not unnest: a batch carries a per-row int[]
    // (baseline) and Postgres flattens multidimensional arrays, which would smear every
    // row's baseline into one. jsonb_to_recordset keeps each row's array its own.
    let res = sqlx::query(
        "insert into harness_arc_frames_exposure_academy
           (run_id, game, seq, grids, state, levels_completed, baseline,
            action, action_x, action_y)
         select $1, f.game, f.seq, f.grids, f.state, f.levels_completed, f.baseline,
                f.action, f.action_x, f.action_y
         from jsonb_to_recordset($2::jsonb) as f(
           game text, seq int, grids text, state text, levels_completed int,
           baseline int[], action text, action_x int, action_y int)
         where exists (select 1 from harness_runs_exposure_academy r
                       where r.id = $1 and r.lease_token = $3
                         and r.lease_expires_at > now() and r.deadline_at > now()
                         and r.stage = 'running')
         on conflict do nothing",
    )
    .bind(r.run_id)
    .bind(encoded)
    .bind(r.lease_token)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    // `on conflict do nothing` also reports zero rows for a duplicate re-post, which is
    // a success — only an unknown or already-finished run earns the 409 that tells a
    // zombie controller to stop. If that check itself fails, say "keep going": frames
    // are not retried by the controller, but a database outage is still distinguished from
    // a stale lease so operators can see it in metrics instead of misdiagnosing a reclaim.
    if res.rows_affected() == 0 {
        let live: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_runs_exposure_academy where id = $1
                           and lease_token = $2 and lease_expires_at > now()
                           and deadline_at > now() and stage = 'running')",
        )
        .bind(r.run_id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if !live {
            return Err(StatusCode::CONFLICT.into_response());
        }
    }
    Ok(StatusCode::OK)
}

pub async fn worker_harness_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(
        r.status.as_str(),
        "done" | "partial" | "failed" | "infra_failed"
    ) || !r.benchmark_state.is_object()
        || r.benchmark_state.to_string().len() > 30000
        || r.error_log.as_ref().is_some_and(|s| s.len() > 8000)
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let score_ok = |v: Option<f32>, lower: f32, upper: f32| {
        v.is_none_or(|n| n.is_finite() && (lower..=upper).contains(&n))
    };
    if !score_ok(r.score_arc, 0.0, 100.0)
        || !score_ok(r.score_frontier, 0.0, 100.0)
        || !score_ok(r.ram_1session_mb, 0.01, 1_000_000.0)
        || !score_ok(r.ram_10session_mb, 0.01, 1_000_000.0)
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if r.status == "done"
        && (r.score_arc.is_none()
            || r.score_frontier.is_none()
            || r.ram_1session_mb.is_none()
            || r.ram_10session_mb.is_none())
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if r.status == "partial"
        && r.score_arc.is_none()
        && r.score_frontier.is_none()
        && r.ram_1session_mb.is_none()
        && r.ram_10session_mb.is_none()
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let encoded = r.benchmark_state.to_string();
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set stage = $3, benchmark_state = $4::jsonb, score_arc = $5,
             score_frontier = $6, ram_1session_mb = $7, ram_10session_mb = $8,
             error_log = $9, progress = null, lease_token = null,
             lease_expires_at = null, result_lease_token = $2, updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() - interval '30 seconds'
           and stage in ('preparing','running')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.status)
    .bind(encoded)
    .bind(r.score_arc)
    .bind(r.score_frontier)
    .bind(r.ram_1session_mb)
    .bind(r.ram_10session_mb)
    .bind(&r.error_log)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_runs_exposure_academy
                           where id = $1 and result_lease_token = $2
                             and stage in ('done','partial','failed','infra_failed'))",
        )
        .bind(r.id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if duplicate {
            return Ok(StatusCode::NO_CONTENT);
        }
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_harness_kaggle_claim(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<Option<KaggleClaim>>, Response> {
    check_worker(&app, &headers)?;
    if app.kaggle_key.is_none() {
        return Ok(Json(None));
    }
    sqlx::query(
        "update harness_kaggle_submissions_exposure_academy
         set status = 'failed', status_message = 'Worker lease expired three times.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where status = 'kernel_running' and lease_expires_at < now() and claim_attempts >= 3",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let lease = Uuid::new_v4();
    let row: Option<HarnessKaggleClaimRow> = sqlx::query_as(
        "with candidate as (
           select j.id from harness_kaggle_submissions_exposure_academy j
           where j.status = 'queued'
              or (j.status = 'kernel_running' and j.lease_expires_at < now()
                  and j.claim_attempts < 3)
              or (j.status = 'submitted' and coalesce(j.next_poll_at, now()) <= now()
                  and (j.lease_expires_at is null or j.lease_expires_at < now()))
           order by j.created_at for update of j skip locked limit 1
         ), updated as (
           update harness_kaggle_submissions_exposure_academy j
           set status = case when j.status = 'submitted' then 'submitted'
                             else 'kernel_running' end,
               lease_token = $1,
               lease_expires_at = now() + interval '5 minutes',
               claim_attempts = case when j.status = 'submitted' then j.claim_attempts
                                     else j.claim_attempts + 1 end,
               updated_at = now()
           from candidate where j.id = candidate.id
           returning j.id, j.run_id, j.status, j.kernel_slug,
                     j.kernel_version, j.submission_ref
         )
         select u.id, r.id as run_id, r.team_id, r.repo_url, r.commit_sha,
                c.username, r.benchmark_version, c.token_nonce, c.token_ciphertext,
                u.status, u.kernel_slug, u.kernel_version, u.submission_ref
         from updated u join harness_runs_exposure_academy r on r.id = u.run_id
         join harness_kaggle_credentials_exposure_academy c on c.team_id = r.team_id",
    )
    .bind(lease)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let Some(row) = row else {
        return Ok(Json(None));
    };
    let token = decrypt_kaggle_token(&app, row.team_id, &row.token_nonce, &row.token_ciphertext)?;
    Ok(Json(Some(KaggleClaim {
        id: row.id,
        run_id: row.run_id,
        phase: if row.status == "submitted" {
            "poll"
        } else {
            "submit"
        }
        .into(),
        repo_url: row.repo_url,
        commit_sha: row.commit_sha,
        username: row.username,
        token,
        lease_token: lease,
        benchmark_version: row.benchmark_version,
        competition: "arc-prize-2026-arc-agi-3".into(),
        kernel_slug: row.kernel_slug,
        kernel_version: row.kernel_version,
        submission_ref: row.submission_ref,
    })))
}

#[derive(sqlx::FromRow)]
struct HarnessKaggleClaimRow {
    id: Uuid,
    run_id: Uuid,
    team_id: Uuid,
    repo_url: String,
    commit_sha: String,
    username: String,
    benchmark_version: String,
    token_nonce: Vec<u8>,
    token_ciphertext: Vec<u8>,
    status: String,
    kernel_slug: Option<String>,
    kernel_version: Option<i32>,
    submission_ref: Option<String>,
}

pub async fn worker_harness_kaggle_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessKaggleResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(r.status.as_str(), "submitted" | "scored" | "failed")
        || r.status_message.as_ref().is_some_and(|s| s.len() > 2000)
        || r.kernel_slug.as_ref().is_some_and(|s| s.len() > 200)
        || r.submission_ref.as_ref().is_some_and(|s| s.len() > 300)
        || [r.public_score, r.private_score]
            .into_iter()
            .flatten()
            .any(|score| !score.is_finite() || !(0.0..=100.0).contains(&score))
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if matches!(r.status.as_str(), "submitted" | "scored")
        && (r.kernel_slug.as_deref().is_none_or(str::is_empty)
            || r.kernel_version.is_none_or(|version| version < 1)
            || r.submission_ref.as_deref().is_none_or(str::is_empty))
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_kaggle_submissions_exposure_academy
         set status = $3, kernel_slug = coalesce($4, kernel_slug),
             kernel_version = coalesce($5, kernel_version),
             submission_ref = coalesce($6, submission_ref),
             public_score = $7, private_score = $8,
             status_message = $9, lease_token = null, lease_expires_at = null,
             last_result_lease_token = $2,
             next_poll_at = case when $3 = 'submitted'
                                 then now() + interval '30 seconds' else null end,
             updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and status in ('kernel_running','submitted')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.status)
    .bind(&r.kernel_slug)
    .bind(r.kernel_version)
    .bind(&r.submission_ref)
    .bind(r.public_score)
    .bind(r.private_score)
    .bind(&r.status_message)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_kaggle_submissions_exposure_academy
                           where id = $1 and last_result_lease_token = $2)",
        )
        .bind(r.id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if duplicate {
            return Ok(StatusCode::NO_CONTENT);
        }
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitted_model_is_allowlisted_with_legacy_default() {
        assert_eq!(submitted_model_id("", false), Some(DEFAULT_BEDROCK_MODEL));
        assert_eq!(
            submitted_model_id("  xai.grok-4.3  ", false),
            Some(DEFAULT_BEDROCK_MODEL)
        );
        assert_eq!(submitted_model_id("attacker.model", false), None);
        assert_eq!(
            submitted_model_id("google.gemma-4-31b", true),
            Some("google.gemma-4-31b")
        );
        assert_eq!(submitted_model_id("openai.gpt-oss-120b", true), None);
    }

    #[test]
    fn builtin_harnesses_are_admin_only_and_require_a_blank_repo() {
        assert_eq!(
            submission_source(true, "", "forge").as_deref(),
            Some("builtin://forge")
        );
        assert_eq!(submission_source(false, "", "forge"), None);
        assert_eq!(
            submission_source(true, "https://github.com/example/agent", "").as_deref(),
            Some("https://github.com/example/agent")
        );
        assert_eq!(
            submission_source(true, "https://github.com/example/agent", "forge"),
            None
        );
        assert_eq!(submission_source(true, "", "unknown"), None);
    }
}
