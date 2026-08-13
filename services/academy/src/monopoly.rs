//! Frozen ppo-plus-v2 tournament, submission validation, replay pages, and worker API.

use crate::admin::{IdForm, MemberForm, NameForm};
use crate::harness::{github_repo_url, repo_error_tr};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use serde::Deserialize;
use sqlx::{FromRow, Postgres, Transaction};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    path::Component,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const LEASE_SECONDS: i64 = 90;
const VALIDATION_LEASE_MINUTES: i64 = 20;
const MAX_EVENT_BATCH: usize = 100;
const MAX_ACTIONS: i32 = 50_000;
const MAX_ACTION_INDEX: i32 = 2_957;
const MAX_ROUNDS: i32 = 200;
const PLAY_LIMIT_US: i64 = 600_000_000;
const BUILTIN_BOTS: [&str; 6] = [
    "hoarder",
    "dealmaker",
    "gambler",
    "builder",
    "blocker",
    "railbaron",
];

const GAME_SELECT: &str = r#"
select g.id, g.tournament_id, g.game_no, g.status, g.seed, g.attempt_count,
       coalesce((
         select jsonb_agg(jsonb_build_object(
           'player_id', s.seat,
           'entry_id', s.entry_id,
           'bot_key', s.bot_key,
           'label', s.label,
           'artifact_sha256', e.artifact_sha256,
           'agent_path', e.agent_path,
           'final_cash', s.final_cash,
           'final_net_worth', s.final_net_worth,
           'strikes', s.strikes,
           'decision_count', s.decision_count,
           'decision_total_us', s.decision_total_us,
           'decision_min_us', s.decision_min_us,
           'decision_max_us', s.decision_max_us,
           'disqualified', s.disqualified,
           'replacement_bot', s.replacement_bot
         ) order by s.seat)
         from monopoly_game_seats_exposure_academy s
         left join monopoly_tournament_entries_exposure_academy e on e.id = s.entry_id
         where s.game_id = g.id
       ), '[]'::jsonb) as seats,
       (select s.final_snapshot from monopoly_game_seats_exposure_academy s
        where s.game_id = g.id and s.final_snapshot is not null
        order by s.seat limit 1) as final_snapshot,
       g.round, g.action_count, g.winner_seat::int as winner_seat,
       g.winner_entry_id, g.end_reason, g.duration_us, g.error_log,
       g.created_at, g.started_at, g.finished_at
from monopoly_games_exposure_academy g
"#;

fn db_error(context: &'static str, error: sqlx::Error) -> Response {
    eprintln!("AI Monopoly {context}: {error}");
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

fn bad_request(message: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, message.into()).into_response()
}

fn conflict(message: &'static str) -> Response {
    (StatusCode::CONFLICT, message).into_response()
}

fn short_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

pub async fn monopoly_team_of(app: &App, uid: Uuid) -> Option<MonopolyTeam> {
    sqlx::query_as(
        "select t.id, t.name from monopoly_team_members_exposure_academy tm
         join monopoly_teams_exposure_academy t on t.id = tm.team_id
         where tm.user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&app.pool)
    .await
    .unwrap_or_default()
}

pub async fn monopoly_submission_of(app: &App, team_id: Uuid) -> Option<MonopolySubmission> {
    sqlx::query_as(
        "select id, team_id, generation, repo_url, agent_path, status,
                commit_sha, repo_size_bytes, validation_log
         from monopoly_submissions_exposure_academy where team_id = $1",
    )
    .bind(team_id)
    .fetch_optional(&app.pool)
    .await
    .unwrap_or_default()
}

pub async fn latest_tournament(app: &App) -> Option<MonopolyTournament> {
    sqlx::query_as(
        "select id, status, seed, ruleset_version, schema_version, total_games,
                completed_games, partial_reason, created_at, started_at, completed_at
         from monopoly_tournaments_exposure_academy
         order by (status = 'active') desc, created_at desc limit 1",
    )
    .fetch_optional(&app.pool)
    .await
    .unwrap_or_default()
}

async fn games_for_tournament(app: &App, tournament_id: Uuid) -> Vec<MonopolyGame> {
    sqlx::query_as::<_, MonopolyGame>(&format!(
        "{GAME_SELECT} where g.tournament_id = $1 order by g.game_no"
    ))
    .bind(tournament_id)
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default()
}

async fn game_by_id(app: &App, id: Uuid) -> Result<Option<MonopolyGame>, sqlx::Error> {
    sqlx::query_as::<_, MonopolyGame>(&format!("{GAME_SELECT} where g.id = $1"))
        .bind(id)
        .fetch_optional(&app.pool)
        .await
}

pub async fn current_or_latest_game(app: &App) -> Option<MonopolyGame> {
    sqlx::query_as::<_, MonopolyGame>(&format!(
        "{GAME_SELECT}
         order by (g.status = 'leased') desc, (g.status = 'queued') desc,
                  g.created_at desc, g.game_no limit 1"
    ))
    .fetch_optional(&app.pool)
    .await
    .unwrap_or_default()
}

async fn standings_for_tournament(app: &App, tournament_id: Uuid) -> Vec<MonopolyStanding> {
    sqlx::query_as(
        "select e.id as entry_id, e.team_name,
                count(*) filter (where g.status = 'done' and g.winner_entry_id = e.id)::bigint as wins,
                count(*) filter (where g.status = 'done')::bigint as games,
                coalesce(avg(s.final_net_worth) filter (where g.status = 'done'), 0)::float8
                  as average_net_worth,
                coalesce(sum(s.strikes) filter (where g.status = 'done'), 0)::bigint as strikes,
                coalesce(sum(s.decision_count) filter (where g.status = 'done'), 0)::bigint
                  as decision_count,
                coalesce(sum(s.decision_total_us) filter (where g.status = 'done'), 0)::bigint
                  as decision_total_us,
                min(s.decision_min_us) filter (where g.status = 'done') as decision_min_us,
                max(s.decision_max_us) filter (where g.status = 'done') as decision_max_us
         from monopoly_tournament_entries_exposure_academy e
         left join monopoly_game_seats_exposure_academy s on s.entry_id = e.id
         left join monopoly_games_exposure_academy g on g.id = s.game_id
         where e.tournament_id = $1
         group by e.id, e.team_name
         order by wins desc, average_net_worth desc, strikes asc, lower(e.team_name), e.id",
    )
    .bind(tournament_id)
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default()
}

#[derive(Deserialize)]
pub struct MonopolyQ {
    tab: Option<String>,
}

pub async fn ai_monopoly(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<MonopolyQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let tab = match q.tab.as_deref() {
        Some("live") => "live",
        Some("history") => "history",
        Some("standings") => "standings",
        Some("instructions") => "instructions",
        _ => "main",
    };
    if tab == "instructions" {
        return Ok(Html(html::monopoly_instructions(&user)));
    }

    let tournament = latest_tournament(&app).await;
    let games = match &tournament {
        Some(tournament) => games_for_tournament(&app, tournament.id).await,
        None => Vec::new(),
    };
    let standings = match &tournament {
        Some(tournament) => standings_for_tournament(&app, tournament.id).await,
        None => Vec::new(),
    };
    if tab == "live" {
        return Ok(Html(html::monopoly_live(
            &user,
            tournament.as_ref(),
            &games,
        )));
    }
    if tab == "history" || tab == "standings" {
        return Ok(Html(html::monopoly_history(
            &user,
            tab,
            tournament.as_ref(),
            &games,
            &standings,
        )));
    }

    let team = monopoly_team_of(&app, user.id).await;
    let submission = match &team {
        Some(team) => monopoly_submission_of(&app, team.id).await,
        None => None,
    };
    let members: Vec<TeamMemberRow> = sqlx::query_as(
        "select tm.team_id, tm.user_id, u.display_name,
                (u.nickname is not null and not u.hidden_from_leaderboard) as public
         from monopoly_team_members_exposure_academy tm
         join users_exposure_academy u on u.id = tm.user_id
         order by tm.created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    Ok(Html(html::monopoly_main(
        &user,
        team.as_ref(),
        &members,
        submission.as_ref(),
        tournament.as_ref(),
        games
            .iter()
            .find(|game| matches!(game.status.as_str(), "leased" | "queued"))
            .or_else(|| games.last()),
        &standings,
    )))
}

#[derive(Deserialize)]
pub struct MonopolyLiveQ {
    game_id: Option<Uuid>,
    seq: Option<i32>,
    at: Option<i32>,
    tail: Option<bool>,
}

async fn events_after(
    app: &App,
    game: &MonopolyGame,
    after: i32,
    limit: i64,
) -> Result<Vec<MonopolyEvent>, sqlx::Error> {
    if game.attempt_count == 0 {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "select seq, acted_player, round, phase, action_idx, action_desc,
                decision_us, strike, info, snapshot
         from monopoly_events_exposure_academy
         where game_id = $1 and attempt_no = $2 and seq > $3
         order by seq limit $4",
    )
    .bind(game.id)
    .bind(game.attempt_count)
    .bind(after.max(0))
    .bind(limit)
    .fetch_all(&app.pool)
    .await
}

async fn events_tail(
    app: &App,
    game: &MonopolyGame,
    limit: i64,
) -> Result<Vec<MonopolyEvent>, sqlx::Error> {
    if game.attempt_count == 0 {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "select seq, acted_player, round, phase, action_idx, action_desc,
                decision_us, strike, info, snapshot
         from (
           select seq, acted_player, round, phase, action_idx, action_desc,
                  decision_us, strike, info, snapshot
           from monopoly_events_exposure_academy
           where game_id = $1 and attempt_no = $2
           order by seq desc limit $3
         ) recent order by seq",
    )
    .bind(game.id)
    .bind(game.attempt_count)
    .bind(limit)
    .fetch_all(&app.pool)
    .await
}

async fn event_at(
    app: &App,
    game: &MonopolyGame,
    seq: i32,
) -> Result<Vec<MonopolyEvent>, sqlx::Error> {
    if game.attempt_count == 0 {
        return Ok(Vec::new());
    }
    sqlx::query_as(
        "select seq, acted_player, round, phase, action_idx, action_desc,
                decision_us, strike, info, snapshot
         from monopoly_events_exposure_academy
         where game_id = $1 and attempt_no = $2 and seq = $3",
    )
    .bind(game.id)
    .bind(game.attempt_count)
    .bind(seq)
    .fetch_all(&app.pool)
    .await
}

fn game_json(game: &MonopolyGame, events: &[MonopolyEvent]) -> serde_json::Value {
    serde_json::json!({
        "id": game.id,
        "tournament_id": game.tournament_id,
        "game_no": game.game_no,
        "status": game.status,
        "attempt": game.attempt_count,
        "seed": game.seed,
        "round": game.round,
        "max_rounds": MAX_ROUNDS,
        "action_count": game.action_count,
        "action_ceiling": MAX_ACTIONS,
        "winner_seat": game.winner_seat,
        "winner_entry_id": game.winner_entry_id,
        "end_reason": game.end_reason,
        "duration_us": game.duration_us,
        "error_log": game.error_log,
        "created_at": game.created_at,
        "started_at": game.started_at,
        "finished_at": game.finished_at,
        "seats": game.seats,
        "final_snapshot": game.final_snapshot,
        "events": events,
    })
}

pub async fn monopoly_live_json(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<MonopolyLiveQ>,
) -> Result<Json<serde_json::Value>, Response> {
    require_onboarded(current_user(&app, &headers).await)?;
    if q.at.is_some() && q.tail.unwrap_or(false)
        || q.at.is_some_and(|seq| !(1..=MAX_ACTIONS).contains(&seq))
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let game = match q.game_id {
        Some(id) => game_by_id(&app, id)
            .await
            .map_err(|error| db_error("live game lookup", error))?,
        None => current_or_latest_game(&app).await,
    };
    let Some(game) = game else {
        return Ok(Json(serde_json::json!({"status": null})));
    };
    let events = if let Some(seq) = q.at {
        event_at(&app, &game, seq).await
    } else if q.tail.unwrap_or(false) {
        events_tail(&app, &game, 100).await
    } else {
        events_after(&app, &game, q.seq.unwrap_or(0), 100).await
    }
    .map_err(|error| db_error("live event lookup", error))?;
    Ok(Json(game_json(&game, &events)))
}

pub async fn monopoly_game_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let game = game_by_id(&app, id)
        .await
        .map_err(|error| db_error("replay lookup", error))?
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let logs: Vec<(i16, String, String)> = if user.is_admin {
        sqlx::query_as(
            "select seat, label, runtime_log
             from monopoly_game_seats_exposure_academy
             where game_id = $1 and runtime_log is not null order by seat",
        )
        .bind(id)
        .fetch_all(&app.pool)
        .await
        .map_err(|error| db_error("admin runtime logs", error))?
    } else if let Some(team) = monopoly_team_of(&app, user.id).await {
        sqlx::query_as(
            "select s.seat, s.label, s.runtime_log
             from monopoly_game_seats_exposure_academy s
             join monopoly_tournament_entries_exposure_academy e on e.id = s.entry_id
             where s.game_id = $1 and e.team_id = $2 and s.runtime_log is not null
             order by s.seat",
        )
        .bind(id)
        .bind(team.id)
        .fetch_all(&app.pool)
        .await
        .map_err(|error| db_error("team runtime logs", error))?
    } else {
        Vec::new()
    };
    Ok(Html(html::monopoly_game_page(&user, &game, &logs)))
}

#[derive(Deserialize)]
pub struct MonopolySubmitForm {
    repo_url: String,
    #[serde(default = "default_agent_path")]
    agent_path: String,
}

fn default_agent_path() -> String {
    MONOPOLY_SUBMISSION_ENTRYPOINT.to_string()
}

fn validate_agent_path(raw: &str) -> Result<String, &'static str> {
    let path = raw.trim();
    if path.is_empty() || path.len() > 240 || path.contains('\\') {
        return Err("Ajan yolu geçerli bir göreli dosya yolu olmalı.");
    }
    let parsed = std::path::Path::new(path);
    if parsed.is_absolute()
        || parsed.components().any(|part| {
            !matches!(part, Component::Normal(_))
                || part.as_os_str().to_str().is_none_or(|part| part.is_empty())
        })
    {
        return Err("Ajan yolu geçerli bir göreli dosya yolu olmalı.");
    }
    Ok(path.to_string())
}

pub async fn monopoly_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<MonopolySubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let team = monopoly_team_of(&app, user.id)
        .await
        .ok_or_else(|| bad_request("Bir takıma atanmadın."))?;
    let repo_url =
        github_repo_url(&form.repo_url).map_err(|error| bad_request(repo_error_tr(error)))?;
    let agent_path = validate_agent_path(&form.agent_path).map_err(bad_request)?;
    sqlx::query(
        "insert into monopoly_submissions_exposure_academy
           (team_id, submitted_by, repo_url, agent_path)
         values ($1,$2,$3,$4)
         on conflict (team_id) do update set
           generation = monopoly_submissions_exposure_academy.generation + 1,
           submitted_by = excluded.submitted_by,
           repo_url = excluded.repo_url,
           agent_path = excluded.agent_path,
           status = 'pending', commit_sha = null, repo_size_bytes = null,
           artifact_sha256 = null, dependency_lock = null, validation_log = null,
           validation_lease_id = null, validation_worker_id = null,
           validation_lease_expires_at = null, approved_at = null, updated_at = now()",
    )
    .bind(team.id)
    .bind(user.id)
    .bind(repo_url)
    .bind(agent_path)
    .execute(&app.pool)
    .await
    .map_err(|error| db_error("submission save", error))?;
    Ok(Redirect::to("/ai-monopoly"))
}

// ---- team and tournament administration ---------------------------------

pub async fn admin_monopoly_team(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<NameForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let name = form.name.trim();
    if !short_text(name, 100) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query(
        "insert into monopoly_teams_exposure_academy (name) values ($1) on conflict do nothing",
    )
    .bind(name)
    .execute(&app.pool)
    .await
    .map_err(|error| db_error("team create", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

pub async fn admin_monopoly_team_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from monopoly_teams_exposure_academy where id = $1")
        .bind(form.id)
        .execute(&app.pool)
        .await
        .map_err(|_| conflict("Dondurulmuş turnuvada yer alan takım silinemez."))?;
    Ok(Redirect::to("/admin/monopoly"))
}

pub async fn admin_monopoly_member(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<MemberForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("member transaction", error))?;
    sqlx::query(
        "insert into monopoly_team_members_exposure_academy (user_id, team_id) values ($1,$2)
         on conflict (user_id) do update set team_id = excluded.team_id",
    )
    .bind(form.user_id)
    .bind(form.team_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("member assign", error))?;
    sqlx::query("update users_exposure_academy set level = 'SERIES_A' where id = $1")
        .bind(form.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("member level", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("member commit", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

pub async fn admin_monopoly_member_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("member removal transaction", error))?;
    sqlx::query("delete from monopoly_team_members_exposure_academy where user_id = $1")
        .bind(form.id)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("member remove", error))?;
    sqlx::query(
        "update users_exposure_academy set level = 'PRESEED' where id = $1
         and not exists (select 1 from harness_team_members_exposure_academy where user_id = $1)",
    )
    .bind(form.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("member demotion", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("member removal commit", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

/// Disable the mutable submission. Frozen entries deliberately remain untouched.
pub async fn admin_monopoly_submission_reject(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query(
        "update monopoly_submissions_exposure_academy set
           status = 'disabled', commit_sha = null, repo_size_bytes = null,
           artifact_sha256 = null, dependency_lock = null,
           validation_lease_id = null, validation_worker_id = null,
           validation_lease_expires_at = null, approved_at = null, updated_at = now()
         where id = $1",
    )
    .bind(form.id)
    .execute(&app.pool)
    .await
    .map_err(|error| db_error("submission disable", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

#[derive(FromRow)]
struct FreezeRow {
    submission_id: Uuid,
    team_id: Uuid,
    generation: i32,
    team_name: String,
    repo_url: String,
    agent_path: String,
    commit_sha: String,
    artifact_sha256: String,
    dependency_lock: String,
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn pair_cost(group: &[usize], pairs: &HashMap<(usize, usize), u16>) -> u64 {
    let mut cost = 0;
    for left in 0..group.len() {
        for right in left + 1..group.len() {
            let pair = if group[left] < group[right] {
                (group[left], group[right])
            } else {
                (group[right], group[left])
            };
            let old = u64::from(*pairs.get(&pair).unwrap_or(&0));
            cost += 2 * old + 1;
        }
    }
    cost
}

fn record_pairs(group: &[usize], pairs: &mut HashMap<(usize, usize), u16>) {
    for left in 0..group.len() {
        for right in left + 1..group.len() {
            let pair = if group[left] < group[right] {
                (group[left], group[right])
            } else {
                (group[right], group[left])
            };
            *pairs.entry(pair).or_default() += 1;
        }
    }
}

/// Six shuffled appearance rounds are flattened into four-seat games. Crossing a round
/// boundary completes the previous partial table first; the next permutation is chosen so
/// that no team can meet itself. Candidate scoring minimizes the incremental repeated-pair
/// cost while the flattening proves the appearance and bot-fill properties directly.
fn schedule_groups(team_count: usize, seed: i64) -> Vec<Vec<usize>> {
    let mut groups = Vec::with_capacity((team_count * 6).div_ceil(4));
    let mut remainder = Vec::new();
    let mut pair_counts = HashMap::new();
    for round in 0..6u64 {
        let mut rng = StdRng::seed_from_u64(splitmix64(seed as u64 ^ round));
        let needed = if remainder.is_empty() {
            0
        } else {
            4 - remainder.len()
        };
        let mut best: Option<(u64, Vec<usize>)> = None;
        for candidate_no in 0..256 {
            let mut allowed: Vec<usize> = (0..team_count)
                .filter(|team| !remainder.contains(team))
                .collect();
            let mut delayed: Vec<usize> = (0..team_count)
                .filter(|team| remainder.contains(team))
                .collect();
            if candidate_no != 0 {
                allowed.shuffle(&mut rng);
                delayed.shuffle(&mut rng);
            } else {
                let offset = (splitmix64(seed as u64 ^ round) as usize) % team_count;
                let allowed_len = allowed.len().max(1);
                allowed.rotate_left(offset % allowed_len);
            }
            let mut candidate = allowed;
            candidate.extend(delayed);
            if candidate
                .iter()
                .take(needed)
                .any(|team| remainder.contains(team))
            {
                continue;
            }
            let mut flattened = remainder.clone();
            flattened.extend(candidate.iter().copied());
            let cost = flattened
                .chunks(4)
                .filter(|group| group.len() == 4)
                .map(|group| pair_cost(group, &pair_counts))
                .sum();
            if best
                .as_ref()
                .is_none_or(|(best_cost, best_order)| (cost, &candidate) < (*best_cost, best_order))
            {
                best = Some((cost, candidate));
            }
        }
        let (_, order) = best.expect("four or more teams always have a boundary-safe order");
        remainder.extend(order);
        while remainder.len() >= 4 {
            let group: Vec<usize> = remainder.drain(..4).collect();
            record_pairs(&group, &mut pair_counts);
            groups.push(group);
        }
    }
    if !remainder.is_empty() {
        record_pairs(&remainder, &mut pair_counts);
        groups.push(remainder);
    }
    groups
}

fn seat_candidates(group: &[usize]) -> Vec<Vec<(usize, usize)>> {
    fn build(
        group: &[usize],
        index: usize,
        used: u8,
        current: &mut Vec<(usize, usize)>,
        output: &mut Vec<Vec<(usize, usize)>>,
    ) {
        if index == group.len() {
            output.push(current.clone());
            return;
        }
        for seat in 0..4 {
            if used & (1 << seat) != 0 {
                continue;
            }
            current.push((group[index], seat));
            build(group, index + 1, used | (1 << seat), current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    build(group, 0, 0, &mut Vec::new(), &mut output);
    output
}

fn assign_balanced_seats(
    groups: &[Vec<usize>],
    team_count: usize,
    seed: i64,
) -> Option<Vec<[Option<usize>; 4]>> {
    fn visit(
        game_index: usize,
        groups: &[Vec<usize>],
        counts: &mut [[u8; 4]],
        remaining: &mut [u8],
        output: &mut [[Option<usize>; 4]],
        seed: i64,
    ) -> bool {
        if game_index == groups.len() {
            return counts.iter().all(|row| {
                row.iter().copied().min() == Some(1) && row.iter().copied().max() == Some(2)
            });
        }
        let mut candidates = seat_candidates(&groups[game_index]);
        candidates.sort_by_key(|candidate| {
            let load: u32 = candidate
                .iter()
                .map(|(team, seat)| u32::from(counts[*team][*seat]) * 16 + *seat as u32)
                .sum();
            let tie = candidate.iter().fold(
                splitmix64(seed as u64 ^ game_index as u64),
                |hash, (team, seat)| splitmix64(hash ^ ((*team as u64) << 8) ^ *seat as u64),
            );
            (load, tie)
        });
        for candidate in candidates {
            let valid = candidate.iter().all(|(team, seat)| {
                if counts[*team][*seat] >= 2 || remaining[*team] == 0 {
                    return false;
                }
                let mut projected = counts[*team];
                projected[*seat] += 1;
                let after = remaining[*team] - 1;
                let seats_still_zero = projected.iter().filter(|count| **count == 0).count() as u8;
                let capacity: u8 = projected.iter().map(|count| 2 - *count).sum();
                seats_still_zero <= after && after <= capacity
            });
            if !valid {
                continue;
            }
            for (team, seat) in &candidate {
                counts[*team][*seat] += 1;
                remaining[*team] -= 1;
                output[game_index][*seat] = Some(*team);
            }
            if visit(game_index + 1, groups, counts, remaining, output, seed) {
                return true;
            }
            for (team, seat) in &candidate {
                counts[*team][*seat] -= 1;
                remaining[*team] += 1;
                output[game_index][*seat] = None;
            }
        }
        false
    }

    let mut counts = vec![[0u8; 4]; team_count];
    let mut remaining = vec![6u8; team_count];
    let mut output = vec![[None; 4]; groups.len()];
    visit(0, groups, &mut counts, &mut remaining, &mut output, seed).then_some(output)
}

fn generate_schedule(team_count: usize, seed: i64) -> Result<Vec<[Option<usize>; 4]>, String> {
    if team_count < 4 {
        return Err("Turnuva için en az dört takım gerekli.".into());
    }
    for retry in 0..32u64 {
        let retry_seed = splitmix64(seed as u64 ^ retry) as i64;
        let groups = schedule_groups(team_count, retry_seed);
        if let Some(schedule) = assign_balanced_seats(&groups, team_count, retry_seed) {
            return Ok(schedule);
        }
    }
    Err("Dengeli fikstür üretilemedi.".into())
}

pub async fn admin_monopoly_start(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    let admin = require_admin(current_user(&app, &headers).await)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("tournament transaction", error))?;
    let active: bool = sqlx::query_scalar(
        "select exists(select 1 from monopoly_tournaments_exposure_academy where status = 'active')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| db_error("active tournament check", error))?;
    if active {
        return Err(conflict("Zaten aktif bir turnuva var."));
    }
    let rows: Vec<FreezeRow> = sqlx::query_as(
        "select s.id as submission_id, s.team_id, s.generation, t.name as team_name,
                s.repo_url, s.agent_path, s.commit_sha, s.artifact_sha256, s.dependency_lock
         from monopoly_submissions_exposure_academy s
         join monopoly_teams_exposure_academy t on t.id = s.team_id
         where s.status = 'approved'
         order by lower(t.name), t.id
         for share of s",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|error| db_error("approved submission freeze", error))?;
    if rows.len() < 4 {
        return Err(bad_request("Turnuva için en az dört onaylı takım gerekli."));
    }
    let seed = rand::thread_rng().r#gen::<i64>();
    let schedule = generate_schedule(rows.len(), seed).map_err(bad_request)?;
    let tournament_id = Uuid::new_v4();
    sqlx::query(
        "insert into monopoly_tournaments_exposure_academy
           (id, seed, total_games, created_by) values ($1,$2,$3,$4)",
    )
    .bind(tournament_id)
    .bind(seed)
    .bind(schedule.len() as i32)
    .bind(admin.id)
    .execute(&mut *tx)
    .await
    .map_err(|_| conflict("Turnuva aynı anda yalnızca bir kez başlatılabilir."))?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        let entry_id = Uuid::new_v4();
        sqlx::query(
            "insert into monopoly_tournament_entries_exposure_academy
               (id, tournament_id, team_id, submission_id, submission_generation,
                team_name, repo_url, commit_sha, agent_path, artifact_sha256, dependency_lock)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(entry_id)
        .bind(tournament_id)
        .bind(row.team_id)
        .bind(row.submission_id)
        .bind(row.generation)
        .bind(&row.team_name)
        .bind(&row.repo_url)
        .bind(&row.commit_sha)
        .bind(&row.agent_path)
        .bind(&row.artifact_sha256)
        .bind(&row.dependency_lock)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("entry freeze", error))?;
        entries.push(entry_id);
    }

    for (index, seats) in schedule.iter().enumerate() {
        let game_id = Uuid::new_v4();
        let game_seed = splitmix64(seed as u64 ^ (index as u64 + 1)) as i64;
        sqlx::query(
            "insert into monopoly_games_exposure_academy
               (id, tournament_id, game_no, seed) values ($1,$2,$3,$4)",
        )
        .bind(game_id)
        .bind(tournament_id)
        .bind(index as i32 + 1)
        .bind(game_seed)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("game insert", error))?;
        for (seat, team_index) in seats.iter().enumerate() {
            match team_index {
                Some(team_index) => {
                    sqlx::query(
                        "insert into monopoly_game_seats_exposure_academy
                           (game_id, seat, entry_id, label) values ($1,$2,$3,$4)",
                    )
                    .bind(game_id)
                    .bind(seat as i16)
                    .bind(entries[*team_index])
                    .bind(&rows[*team_index].team_name)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| db_error("submitted seat insert", error))?;
                }
                None => {
                    let bot = BUILTIN_BOTS[(index + seat) % BUILTIN_BOTS.len()];
                    sqlx::query(
                        "insert into monopoly_game_seats_exposure_academy
                           (game_id, seat, bot_key, label) values ($1,$2,$3,$4)",
                    )
                    .bind(game_id)
                    .bind(seat as i16)
                    .bind(bot)
                    .bind(format!("Bot · {bot}"))
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| db_error("bot seat insert", error))?;
                }
            }
        }
    }
    sqlx::query(
        "update monopoly_artifacts_exposure_academy set last_used_at = now()
         where sha256 = any($1)",
    )
    .bind(
        rows.iter()
            .map(|row| row.artifact_sha256.clone())
            .collect::<Vec<_>>(),
    )
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("artifact use update", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("tournament commit", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

pub async fn admin_monopoly_cancel(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("cancel transaction", error))?;
    sqlx::query(
        "update monopoly_tournaments_exposure_academy
         set status = 'cancelled', completed_at = now(), partial_reason = 'Yönetici durdurdu.'
         where id = $1 and status = 'active'",
    )
    .bind(form.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("tournament cancel", error))?;
    sqlx::query(
        "update monopoly_games_exposure_academy set status = 'cancelled',
           lease_id = null, lease_worker_id = null, lease_expires_at = null,
           finished_at = now(), updated_at = now()
         where tournament_id = $1 and status in ('queued','leased')",
    )
    .bind(form.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("game cancel", error))?;
    sqlx::query(
        "update monopoly_game_attempts_exposure_academy a set status = 'cancelled', finished_at = now()
         from monopoly_games_exposure_academy g
         where a.game_id = g.id and g.tournament_id = $1 and a.status = 'leased'",
    )
    .bind(form.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("attempt cancel", error))?;
    sqlx::query(
        "update monopoly_workers_exposure_academy w set status = 'ready', current_game_id = null,
           updated_at = now()
         from monopoly_games_exposure_academy g
         where w.current_game_id = g.id and g.tournament_id = $1",
    )
    .bind(form.id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("cancel worker release", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("cancel commit", error))?;
    Ok(Redirect::to("/admin/monopoly"))
}

// ---- worker resources and validation -------------------------------------

#[derive(Deserialize)]
pub struct WorkerResourceReq {
    worker_id: String,
    kind: String,
    session_name: Option<String>,
    status: String,
    ram_bytes: Option<i64>,
    effective_vcpus: Option<f64>,
    machine_shape: Option<String>,
    preflight_reason: Option<String>,
    current_game_id: Option<Uuid>,
}

pub async fn worker_rl_monopoly_resource(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<WorkerResourceReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !short_text(&request.worker_id, 120)
        || !matches!(
            request.kind.as_str(),
            "colab" | "host" | "validation" | "other"
        )
        || !matches!(
            request.status.as_str(),
            "preflight" | "ready" | "busy" | "stopping" | "stopped" | "rejected" | "error"
        )
        || request.ram_bytes.is_some_and(|value| value < 0)
        || request
            .effective_vcpus
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        || request
            .session_name
            .as_deref()
            .is_some_and(|value| !short_text(value, 120))
        || request
            .machine_shape
            .as_deref()
            .is_some_and(|value| value.chars().count() > 200)
        || request
            .preflight_reason
            .as_deref()
            .is_some_and(|value| value.chars().count() > 1000)
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query(
        "insert into monopoly_workers_exposure_academy
           (worker_id, kind, session_name, status, ram_bytes, effective_vcpus,
            machine_shape, preflight_reason, current_game_id)
         values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         on conflict (worker_id) do update set
           kind = excluded.kind, session_name = excluded.session_name, status = excluded.status,
           ram_bytes = coalesce(excluded.ram_bytes, monopoly_workers_exposure_academy.ram_bytes),
           effective_vcpus = coalesce(excluded.effective_vcpus, monopoly_workers_exposure_academy.effective_vcpus),
           machine_shape = coalesce(excluded.machine_shape, monopoly_workers_exposure_academy.machine_shape),
           preflight_reason = excluded.preflight_reason,
           current_game_id = excluded.current_game_id, last_seen_at = now(), updated_at = now()",
    )
    .bind(request.worker_id.trim())
    .bind(request.kind)
    .bind(request.session_name)
    .bind(request.status)
    .bind(request.ram_bytes)
    .bind(request.effective_vcpus)
    .bind(request.machine_shape)
    .bind(request.preflight_reason)
    .bind(request.current_game_id)
    .execute(&app.pool)
    .await
    .map_err(|error| db_error("resource heartbeat", error))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct WorkerIdentityReq {
    worker_id: String,
}

pub async fn worker_rl_monopoly_validation_claim(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<WorkerIdentityReq>,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    if !short_text(&request.worker_id, 120) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("validation claim transaction", error))?;
    sqlx::query(
        "update monopoly_submissions_exposure_academy set status = 'pending',
           validation_lease_id = null, validation_worker_id = null,
           validation_lease_expires_at = null, updated_at = now()
         where status = 'validating' and validation_lease_expires_at < now()",
    )
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("stale validation recovery", error))?;
    let lease_id = Uuid::new_v4();
    let claimed: Option<(Uuid, i32, String, String)> = sqlx::query_as(
        "with candidate as (
           select id from monopoly_submissions_exposure_academy
           where status = 'pending' order by updated_at
           for update skip locked limit 1
         )
         update monopoly_submissions_exposure_academy s set
           status = 'validating', validation_lease_id = $1, validation_worker_id = $2,
           validation_lease_expires_at = now() + make_interval(mins => $3), updated_at = now()
         from candidate where s.id = candidate.id
         returning s.id, s.generation, s.repo_url, s.agent_path",
    )
    .bind(lease_id)
    .bind(request.worker_id.trim())
    .bind(VALIDATION_LEASE_MINUTES as i32)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| db_error("validation claim", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("validation claim commit", error))?;
    Ok(Json(match claimed {
        Some((id, generation, repo_url, agent_path)) => serde_json::json!({
            "id": id,
            "generation": generation,
            "lease_id": lease_id,
            "repo_url": repo_url,
            "agent_path": agent_path,
            "repo_max_bytes": MONOPOLY_SUBMISSION_REPO_MAX_BYTES,
        }),
        None => serde_json::json!({"id": null}),
    }))
}

#[derive(Deserialize)]
pub struct ValidationHeartbeatReq {
    worker_id: String,
    id: Uuid,
    generation: i32,
    lease_id: Uuid,
}

pub async fn worker_rl_monopoly_validation_heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<ValidationHeartbeatReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let updated = sqlx::query(
        "update monopoly_submissions_exposure_academy set
           validation_lease_expires_at = now() + make_interval(mins => $5), updated_at = now()
         where id = $1 and generation = $2 and status = 'validating'
           and validation_lease_id = $3 and validation_worker_id = $4
           and validation_lease_expires_at >= now()",
    )
    .bind(request.id)
    .bind(request.generation)
    .bind(request.lease_id)
    .bind(request.worker_id)
    .bind(VALIDATION_LEASE_MINUTES as i32)
    .execute(&app.pool)
    .await
    .map_err(|error| db_error("validation heartbeat", error))?;
    if updated.rows_affected() == 0 {
        return Err(conflict("Doğrulama lease'i artık geçerli değil."));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ValidationResultReq {
    worker_id: String,
    id: Uuid,
    generation: i32,
    lease_id: Uuid,
    status: String,
    commit_sha: Option<String>,
    repo_size_bytes: Option<i64>,
    artifact_sha256: Option<String>,
    artifact_size_bytes: Option<i64>,
    dependency_lock: Option<String>,
    #[serde(default)]
    validation_log: Option<String>,
}

fn valid_sha(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub async fn worker_rl_monopoly_validation_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<ValidationResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(request.status.as_str(), "approved" | "failed") {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let log = request
        .validation_log
        .as_deref()
        .map(|value| value.chars().take(12_000).collect::<String>());
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("validation result transaction", error))?;
    if request.status == "approved" {
        let commit = request.commit_sha.as_deref().unwrap_or_default();
        let artifact = request.artifact_sha256.as_deref().unwrap_or_default();
        let repo_size = request.repo_size_bytes.unwrap_or(-1);
        let artifact_size = request.artifact_size_bytes.unwrap_or(-1);
        let lock = request.dependency_lock.as_deref().unwrap_or_default();
        if !valid_sha(commit, 40)
            || !valid_sha(artifact, 64)
            || !(0..=MONOPOLY_SUBMISSION_REPO_MAX_BYTES).contains(&repo_size)
            || !(1..=2 * 1024 * 1024 * 1024_i64).contains(&artifact_size)
            || lock.len() > 100_000
        {
            return Err(StatusCode::BAD_REQUEST.into_response());
        }
        let path = app.monopoly_artifact_dir.join(format!("{artifact}.tar.gz"));
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|_| bad_request("Artifact sunucu diskinde bulunamadı."))?;
        if metadata.len() != artifact_size as u64 {
            return Err(bad_request(
                "Artifact boyutu doğrulama sonucuyla eşleşmiyor.",
            ));
        }
        sqlx::query(
            "insert into monopoly_artifacts_exposure_academy (sha256, size_bytes)
             values ($1,$2) on conflict (sha256) do update set last_used_at = now()",
        )
        .bind(artifact)
        .bind(artifact_size)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("artifact record", error))?;
        let updated = sqlx::query(
            "update monopoly_submissions_exposure_academy set
               status = 'approved', commit_sha = $5, repo_size_bytes = $6,
               artifact_sha256 = $7, dependency_lock = $8, validation_log = $9,
               validation_lease_id = null, validation_worker_id = null,
               validation_lease_expires_at = null, approved_at = now(), updated_at = now()
             where id = $1 and generation = $2 and status = 'validating'
               and validation_lease_id = $3 and validation_worker_id = $4
               and validation_lease_expires_at >= now()",
        )
        .bind(request.id)
        .bind(request.generation)
        .bind(request.lease_id)
        .bind(&request.worker_id)
        .bind(commit)
        .bind(repo_size)
        .bind(artifact)
        .bind(lock)
        .bind(log)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("approved validation result", error))?;
        if updated.rows_affected() == 0 {
            return Err(conflict("Doğrulama sonucu eski bir gönderime ait."));
        }
    } else {
        let updated = sqlx::query(
            "update monopoly_submissions_exposure_academy set
               status = 'failed', commit_sha = null, repo_size_bytes = null,
               artifact_sha256 = null, dependency_lock = null, validation_log = $5,
               validation_lease_id = null, validation_worker_id = null,
               validation_lease_expires_at = null, approved_at = null, updated_at = now()
             where id = $1 and generation = $2 and status = 'validating'
               and validation_lease_id = $3 and validation_worker_id = $4
               and validation_lease_expires_at >= now()",
        )
        .bind(request.id)
        .bind(request.generation)
        .bind(request.lease_id)
        .bind(&request.worker_id)
        .bind(log)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("failed validation result", error))?;
        if updated.rows_affected() == 0 {
            return Err(conflict("Doğrulama sonucu eski bir gönderime ait."));
        }
    }
    tx.commit()
        .await
        .map_err(|error| db_error("validation result commit", error))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_rl_monopoly_artifact(
    State(app): State<App>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<Response, Response> {
    check_worker(&app, &headers)?;
    if !valid_sha(&sha256, 64) {
        return Err(StatusCode::NOT_FOUND.into_response());
    }
    let expected_size: i64 = sqlx::query_scalar(
        "update monopoly_artifacts_exposure_academy set last_used_at = now()
         where sha256 = $1 returning size_bytes",
    )
    .bind(&sha256)
    .fetch_optional(&app.pool)
    .await
    .map_err(|error| db_error("artifact access", error))?
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let path = app.monopoly_artifact_dir.join(format!("{sha256}.tar.gz"));
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND.into_response())?;
    let size = file
        .metadata()
        .await
        .map_err(|error| db_error("artifact metadata", error.into()))?
        .len();
    if size != expected_size as u64 {
        return Err(StatusCode::SERVICE_UNAVAILABLE.into_response());
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(header::CONTENT_LENGTH, size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{sha256}.tar.gz\""),
        )
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ---- leased games --------------------------------------------------------

async fn refresh_tournament(
    tx: &mut Transaction<'_, Postgres>,
    tournament_id: Uuid,
) -> Result<(), sqlx::Error> {
    let (done, failed, outstanding): (i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where status = 'done'),
                count(*) filter (where status = 'failed'),
                count(*) filter (where status in ('queued','leased'))
         from monopoly_games_exposure_academy where tournament_id = $1",
    )
    .bind(tournament_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "update monopoly_tournaments_exposure_academy set
           completed_games = $2,
           status = case when $4 = 0 then
             case when $3 > 0 or partial_reason is not null then 'partial' else 'completed' end
             else status end,
           completed_at = case when $4 = 0 then now() else completed_at end
         where id = $1 and status = 'active'",
    )
    .bind(tournament_id)
    .bind(done as i32)
    .bind(failed)
    .bind(outstanding)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn recover_expired_leases(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    let expired: Vec<(Uuid, Uuid, i32, Uuid)> = sqlx::query_as(
        "select id, tournament_id, attempt_count, lease_id
         from monopoly_games_exposure_academy
         where status = 'leased' and lease_expires_at < now()
         for update skip locked",
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut tournaments = Vec::new();
    for (game_id, tournament_id, attempt, lease_id) in expired {
        sqlx::query(
            "update monopoly_game_attempts_exposure_academy
             set status = 'expired', finished_at = now(), error_log = 'Lease heartbeat expired.'
             where lease_id = $1 and status = 'leased'",
        )
        .bind(lease_id)
        .execute(&mut **tx)
        .await?;
        let exhausted = attempt >= 3;
        sqlx::query(
            "update monopoly_games_exposure_academy set
               status = case when $2 then 'failed' else 'queued' end,
               lease_id = null, lease_worker_id = null, lease_expires_at = null,
               round = case when $2 then round else 0 end,
               action_count = case when $2 then action_count else 0 end,
               last_event_seq = case when $2 then last_event_seq else 0 end,
               end_reason = case when $2 then 'infra_exhausted' else null end,
               error_log = case when $2 then 'Üç altyapı denemesi de lease süresini aştı.' else null end,
               finished_at = case when $2 then now() else null end, updated_at = now()
             where id = $1",
        )
        .bind(game_id)
        .bind(exhausted)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "update monopoly_workers_exposure_academy set status = 'error', current_game_id = null,
             preflight_reason = 'Game lease expired.', updated_at = now()
             where current_game_id = $1",
        )
        .bind(game_id)
        .execute(&mut **tx)
        .await?;
        if exhausted {
            sqlx::query(
                "update monopoly_tournaments_exposure_academy
                 set partial_reason = coalesce(partial_reason, 'Bir oyun üç altyapı denemesini tüketti.')
                 where id = $1",
            )
            .bind(tournament_id)
            .execute(&mut **tx)
            .await?;
        }
        tournaments.push(tournament_id);
    }
    tournaments.sort_unstable();
    tournaments.dedup();
    for tournament_id in tournaments {
        refresh_tournament(tx, tournament_id).await?;
    }
    Ok(())
}

fn worker_qualifies(row: &(String, String, Option<i64>, Option<f64>)) -> bool {
    matches!(row.0.as_str(), "colab" | "host")
        && row.1 == "ready"
        && row.2.is_some_and(|ram| ram >= 32 * 1024 * 1024 * 1024_i64)
        && row.3.is_some_and(|cpus| cpus >= 8.0)
}

pub async fn worker_rl_monopoly_claim(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<WorkerIdentityReq>,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    if !short_text(&request.worker_id, 120) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("claim transaction", error))?;
    recover_expired_leases(&mut tx)
        .await
        .map_err(|error| db_error("lease recovery", error))?;
    let resource: Option<(String, String, Option<i64>, Option<f64>)> = sqlx::query_as(
        "select kind, status, ram_bytes, effective_vcpus
         from monopoly_workers_exposure_academy where worker_id = $1 for update",
    )
    .bind(request.worker_id.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| db_error("worker qualification", error))?;
    if resource
        .as_ref()
        .is_none_or(|resource| !worker_qualifies(resource))
    {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            "Worker en az 32 GiB RAM ve 8 etkin vCPU bildirmeli.",
        )
            .into_response());
    }
    sqlx::query(
        "update monopoly_workers_exposure_academy
         set last_seen_at = now(), updated_at = now() where worker_id = $1",
    )
    .bind(request.worker_id.trim())
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("worker idle heartbeat", error))?;
    let candidate: Option<(Uuid, Uuid, i32, i64, i32)> = sqlx::query_as(
        "select g.id, g.tournament_id, g.game_no, g.seed, g.attempt_count
         from monopoly_games_exposure_academy g
         join monopoly_tournaments_exposure_academy t on t.id = g.tournament_id
         where g.status = 'queued' and g.attempt_count < 3 and t.status = 'active'
         order by g.created_at, g.game_no for update of g skip locked limit 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| db_error("game candidate", error))?;
    let Some((game_id, tournament_id, game_no, seed, old_attempt)) = candidate else {
        tx.commit()
            .await
            .map_err(|error| db_error("empty claim commit", error))?;
        return Ok(Json(serde_json::json!({"game": null})));
    };
    let lease_id = Uuid::new_v4();
    let attempt = old_attempt + 1;
    sqlx::query(
        "update monopoly_games_exposure_academy set
           status = 'leased', attempt_count = $2, lease_id = $3, lease_worker_id = $4,
           lease_expires_at = now() + make_interval(secs => $5),
           started_at = coalesce(started_at, now()), finished_at = null,
           error_log = null, end_reason = null, updated_at = now()
         where id = $1",
    )
    .bind(game_id)
    .bind(attempt)
    .bind(lease_id)
    .bind(request.worker_id.trim())
    .bind(LEASE_SECONDS as f64)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("game claim", error))?;
    sqlx::query(
        "insert into monopoly_game_attempts_exposure_academy
           (game_id, attempt_no, lease_id, worker_id) values ($1,$2,$3,$4)",
    )
    .bind(game_id)
    .bind(attempt)
    .bind(lease_id)
    .bind(request.worker_id.trim())
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("attempt insert", error))?;
    let seats: serde_json::Value = sqlx::query_scalar(
        "select jsonb_agg(jsonb_build_object(
           'player_id', s.seat, 'entry_id', s.entry_id, 'bot_key', s.bot_key,
           'label', s.label, 'artifact_sha256', e.artifact_sha256,
           'agent_path', e.agent_path) order by s.seat)
         from monopoly_game_seats_exposure_academy s
         left join monopoly_tournament_entries_exposure_academy e on e.id = s.entry_id
         where s.game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| db_error("claimed seats", error))?;
    sqlx::query(
        "update monopoly_workers_exposure_academy set status = 'busy', current_game_id = $2,
           last_seen_at = now(), updated_at = now() where worker_id = $1",
    )
    .bind(request.worker_id.trim())
    .bind(game_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("worker busy", error))?;
    let artifacts: Vec<String> = sqlx::query_scalar(
        "select distinct e.artifact_sha256
         from monopoly_game_seats_exposure_academy s
         join monopoly_tournament_entries_exposure_academy e on e.id = s.entry_id
         where s.game_id = $1",
    )
    .bind(game_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|error| db_error("claimed artifacts", error))?;
    if !artifacts.is_empty() {
        sqlx::query(
            "update monopoly_artifacts_exposure_academy set last_used_at = now()
             where sha256 = any($1)",
        )
        .bind(&artifacts)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("claimed artifact use", error))?;
    }
    tx.commit()
        .await
        .map_err(|error| db_error("claim commit", error))?;
    Ok(Json(serde_json::json!({
        "game": {
            "id": game_id,
            "tournament_id": tournament_id,
            "game_no": game_no,
            "attempt_no": attempt,
            "lease_id": lease_id,
            "seed": seed,
            "ruleset_version": MONOPOLY_RULESET_VERSION,
            "schema_version": MONOPOLY_SCHEMA_VERSION,
            "max_rounds": MAX_ROUNDS,
            "action_ceiling": MAX_ACTIONS,
            "play_limit_us": PLAY_LIMIT_US,
            "seats": seats,
        }
    })))
}

#[derive(Deserialize)]
pub struct GameLeaseReq {
    worker_id: String,
    game_id: Uuid,
    attempt_no: i32,
    lease_id: Uuid,
}

async fn renew_lease(
    tx: &mut Transaction<'_, Postgres>,
    request: &GameLeaseReq,
) -> Result<bool, sqlx::Error> {
    let updated = sqlx::query(
        "update monopoly_games_exposure_academy set
           lease_expires_at = now() + make_interval(secs => $5), updated_at = now()
         where id = $1 and status = 'leased' and attempt_count = $2
           and lease_id = $3 and lease_worker_id = $4 and lease_expires_at >= now()",
    )
    .bind(request.game_id)
    .bind(request.attempt_no)
    .bind(request.lease_id)
    .bind(&request.worker_id)
    .bind(LEASE_SECONDS as f64)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query(
        "update monopoly_game_attempts_exposure_academy set last_heartbeat_at = now()
         where lease_id = $1 and status = 'leased'",
    )
    .bind(request.lease_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "update monopoly_workers_exposure_academy set last_seen_at = now(), updated_at = now()
         where worker_id = $1",
    )
    .bind(&request.worker_id)
    .execute(&mut **tx)
    .await?;
    Ok(true)
}

pub async fn worker_rl_monopoly_heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<GameLeaseReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("heartbeat transaction", error))?;
    if !renew_lease(&mut tx, &request)
        .await
        .map_err(|error| db_error("heartbeat", error))?
    {
        return Err(conflict("Oyun lease'i artık geçerli değil."));
    }
    tx.commit()
        .await
        .map_err(|error| db_error("heartbeat commit", error))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct WorkerEvent {
    seq: i32,
    acted_player: i16,
    round: i32,
    phase: String,
    action_idx: i32,
    action_desc: String,
    decision_us: Option<i64>,
    #[serde(default)]
    strike: bool,
    #[serde(default = "empty_json_object")]
    info: serde_json::Value,
    snapshot: serde_json::Value,
}

fn empty_json_object() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Deserialize)]
pub struct EventBatchReq {
    worker_id: String,
    game_id: Uuid,
    attempt_no: i32,
    lease_id: Uuid,
    events: Vec<WorkerEvent>,
}

pub async fn worker_rl_monopoly_events(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<EventBatchReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if request.events.is_empty()
        || request.events.len() > MAX_EVENT_BATCH
        || request
            .events
            .windows(2)
            .any(|pair| pair[0].seq.checked_add(1) != Some(pair[1].seq))
        || request.events.iter().any(|event| {
            !(1..=MAX_ACTIONS).contains(&event.seq)
                || !(0..=3).contains(&event.acted_player)
                || !(0..=MAX_ROUNDS).contains(&event.round)
                || !(0..=MAX_ACTION_INDEX).contains(&event.action_idx)
                || !short_text(&event.phase, 60)
                || !short_text(&event.action_desc, 500)
                || event
                    .decision_us
                    .is_some_and(|value| !(0..=2_000_000).contains(&value))
                || event.strike && event.decision_us.is_none()
                || !event.info.is_object()
                || !event.snapshot.is_object()
        })
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let lease = GameLeaseReq {
        worker_id: request.worker_id,
        game_id: request.game_id,
        attempt_no: request.attempt_no,
        lease_id: request.lease_id,
    };
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("event transaction", error))?;
    if !renew_lease(&mut tx, &lease)
        .await
        .map_err(|error| db_error("event lease", error))?
    {
        return Err(conflict("Olay paketi eski bir oyun denemesine ait."));
    }
    let last_stored: i32 = sqlx::query_scalar(
        "select last_event_seq from monopoly_games_exposure_academy where id = $1",
    )
    .bind(request.game_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| db_error("event cursor read", error))?;
    if request.events[0].seq > last_stored.saturating_add(1) {
        return Err(conflict("Olay paketinde önceki hamleler eksik."));
    }
    for event in &request.events {
        sqlx::query(
            "insert into monopoly_events_exposure_academy
               (game_id, attempt_no, seq, acted_player, round, phase, action_idx,
                action_desc, decision_us, strike, info, snapshot)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::jsonb,$12::jsonb)
             on conflict (game_id, attempt_no, seq) do nothing",
        )
        .bind(request.game_id)
        .bind(request.attempt_no)
        .bind(event.seq)
        .bind(event.acted_player)
        .bind(event.round)
        .bind(&event.phase)
        .bind(event.action_idx)
        .bind(&event.action_desc)
        .bind(event.decision_us)
        .bind(event.strike)
        .bind(event.info.to_string())
        .bind(event.snapshot.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("event insert", error))?;
    }
    let last = request.events.last().expect("non-empty event batch");
    sqlx::query(
        "update monopoly_games_exposure_academy set
           last_event_seq = greatest(last_event_seq, $2),
           action_count = greatest(action_count, $2), round = greatest(round, $3), updated_at = now()
         where id = $1",
    )
    .bind(request.game_id)
    .bind(last.seq)
    .bind(last.round)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("event cursor", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("event commit", error))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SeatResultReq {
    seat: i16,
    final_cash: f64,
    final_net_worth: f64,
    strikes: i32,
    decision_count: i32,
    decision_total_us: i64,
    decision_min_us: Option<i64>,
    decision_max_us: Option<i64>,
    #[serde(default)]
    disqualified: bool,
    replacement_bot: Option<String>,
    #[serde(default)]
    runtime_log: Option<String>,
    final_snapshot: serde_json::Value,
}

#[derive(Deserialize)]
pub struct GameResultReq {
    worker_id: String,
    game_id: Uuid,
    attempt_no: i32,
    lease_id: Uuid,
    outcome: String,
    end_reason: Option<String>,
    winner_seat: Option<i16>,
    winner_entry_id: Option<Uuid>,
    duration_us: Option<i64>,
    round: Option<i32>,
    action_count: Option<i32>,
    #[serde(default)]
    seats: Vec<SeatResultReq>,
    #[serde(default)]
    error_log: Option<String>,
}

fn valid_seat_result(result: &SeatResultReq) -> bool {
    (0..=3).contains(&result.seat)
        && result.final_cash.is_finite()
        && result.final_net_worth.is_finite()
        && (0..=3).contains(&result.strikes)
        && result.decision_count >= 0
        && result.strikes <= result.decision_count
        && result.decision_total_us >= 0
        && result.decision_min_us.is_none_or(|value| value >= 0)
        && result
            .decision_max_us
            .is_none_or(|value| (0..=2_000_000).contains(&value))
        && match (
            result.decision_count,
            result.decision_min_us,
            result.decision_max_us,
        ) {
            (0, None, None) => result.decision_total_us == 0,
            (count, Some(min), Some(max)) if count > 0 => {
                min <= max
                    && result.decision_total_us >= min * i64::from(count)
                    && result.decision_total_us <= max * i64::from(count)
            }
            _ => false,
        }
        && result.replacement_bot.as_deref().is_none_or(|value| {
            short_text(value, 80) && BUILTIN_BOTS.get(result.seat as usize).copied() == Some(value)
        })
        && result.disqualified == result.replacement_bot.is_some()
        && result.final_snapshot.is_object()
}

fn final_snapshot_matches(results: &[SeatResultReq]) -> bool {
    let Some(snapshot) = results.first().map(|seat| &seat.final_snapshot) else {
        return false;
    };
    if results.iter().any(|seat| seat.final_snapshot != *snapshot) {
        return false;
    }
    let Some(players) = snapshot
        .get("players")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    players.len() == 4
        && results.iter().all(|result| {
            players.iter().any(|player| {
                player.get("player_id").and_then(serde_json::Value::as_i64)
                    == Some(i64::from(result.seat))
                    && player.get("cash").and_then(serde_json::Value::as_f64)
                        == Some(result.final_cash)
                    && player.get("net_worth").and_then(serde_json::Value::as_f64)
                        == Some(result.final_net_worth)
            })
        })
}

fn timing_disqualification(
    identities: &[(i16, Option<Uuid>)],
    results: &[SeatResultReq],
) -> Option<i16> {
    identities
        .iter()
        .filter_map(|(seat, entry)| {
            entry.as_ref()?;
            let result = results.iter().find(|result| result.seat == *seat)?;
            (result.strikes < 3 && result.decision_count > 0).then_some(result)
        })
        .max_by(|left, right| {
            let left_weighted = left.decision_total_us * i64::from(right.decision_count);
            let right_weighted = right.decision_total_us * i64::from(left.decision_count);
            left_weighted
                .cmp(&right_weighted)
                .then_with(|| right.seat.cmp(&left.seat))
        })
        .map(|result| result.seat)
}

pub async fn worker_rl_monopoly_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<GameResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(request.outcome.as_str(), "completed" | "infra_failed") {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|error| db_error("result transaction", error))?;
    let game: Option<(Uuid, i32, i32, i32)> = sqlx::query_as(
        "select tournament_id, attempt_count, last_event_seq, round
         from monopoly_games_exposure_academy
         where id = $1 and status = 'leased' and attempt_count = $2
           and lease_id = $3 and lease_worker_id = $4
           and lease_expires_at >= now() for update",
    )
    .bind(request.game_id)
    .bind(request.attempt_no)
    .bind(request.lease_id)
    .bind(&request.worker_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| db_error("result lease", error))?;
    let Some((tournament_id, attempt, stored_action_count, stored_round)) = game else {
        return Err(conflict("Sonuç eski bir oyun denemesine ait."));
    };
    let error_log = request
        .error_log
        .as_deref()
        .map(|value| value.chars().take(4000).collect::<String>());
    if request.outcome == "infra_failed" {
        let exhausted = attempt >= 3;
        sqlx::query(
            "update monopoly_game_attempts_exposure_academy set
               status = 'infra_failed', finished_at = now(), error_log = $2
             where lease_id = $1 and status = 'leased'",
        )
        .bind(request.lease_id)
        .bind(&error_log)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("failed attempt", error))?;
        sqlx::query(
            "update monopoly_games_exposure_academy set
               status = case when $2 then 'failed' else 'queued' end,
               lease_id = null, lease_worker_id = null, lease_expires_at = null,
               round = case when $2 then round else 0 end,
               action_count = case when $2 then action_count else 0 end,
               last_event_seq = case when $2 then last_event_seq else 0 end,
               end_reason = case when $2 then 'infra_exhausted' else null end,
               error_log = case when $2 then $3 else null end,
               finished_at = case when $2 then now() else null end, updated_at = now()
             where id = $1",
        )
        .bind(request.game_id)
        .bind(exhausted)
        .bind(&error_log)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("failed game", error))?;
        if exhausted {
            sqlx::query(
                "update monopoly_tournaments_exposure_academy
                 set partial_reason = coalesce(partial_reason, 'Bir oyun üç altyapı denemesini tüketti.')
                 where id = $1",
            )
            .bind(tournament_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| db_error("partial tournament", error))?;
        }
    } else {
        let final_done = request
            .seats
            .first()
            .and_then(|seat| seat.final_snapshot.get("done"))
            .and_then(serde_json::Value::as_bool);
        if request.seats.len() != 4
            || request.seats.iter().any(|seat| !valid_seat_result(seat))
            || !final_snapshot_matches(&request.seats)
            || {
                let mut seats: Vec<i16> = request.seats.iter().map(|seat| seat.seat).collect();
                seats.sort_unstable();
                seats != [0, 1, 2, 3]
            }
            || !matches!(
                request.end_reason.as_deref(),
                Some("normal" | "ten_minute" | "action_ceiling")
            )
            || request.duration_us.is_none_or(|value| value < 0)
            || request
                .round
                .is_none_or(|value| !(0..=MAX_ROUNDS).contains(&value))
            || request
                .action_count
                .is_none_or(|value| !(0..=MAX_ACTIONS).contains(&value))
            || request
                .seats
                .iter()
                .any(|seat| seat.decision_count > request.action_count.unwrap_or_default())
            || request.action_count != Some(stored_action_count)
            || request.round != Some(stored_round)
            || request.end_reason.as_deref() == Some("action_ceiling")
                && request.action_count != Some(MAX_ACTIONS)
            || request.end_reason.as_deref() == Some("ten_minute")
                && (request
                    .duration_us
                    .is_none_or(|value| value < PLAY_LIMIT_US)
                    || request.action_count == Some(MAX_ACTIONS))
            || request.end_reason.as_deref() == Some("normal")
                && (request
                    .duration_us
                    .is_none_or(|value| value >= PLAY_LIMIT_US)
                    || request.action_count == Some(MAX_ACTIONS))
            || final_done.is_none()
            || request.end_reason.as_deref() == Some("normal") && final_done != Some(true)
        {
            return Err(StatusCode::BAD_REQUEST.into_response());
        }
        let identities: Vec<(i16, Option<Uuid>)> = sqlx::query_as(
            "select seat, entry_id from monopoly_game_seats_exposure_academy
             where game_id = $1 order by seat",
        )
        .bind(request.game_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| db_error("result seat identities", error))?;
        let event_summaries: Vec<(i16, i64, i64, Option<i64>, Option<i64>, i64)> = sqlx::query_as(
            "select acted_player, count(decision_us)::bigint,
                        coalesce(sum(decision_us), 0)::bigint,
                        min(decision_us), max(decision_us),
                        count(*) filter (where strike)::bigint
                 from monopoly_events_exposure_academy
                 where game_id = $1 and attempt_no = $2
                 group by acted_player",
        )
        .bind(request.game_id)
        .bind(attempt)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| db_error("result event summaries", error))?;
        let timing_seat = (request.end_reason.as_deref() == Some("ten_minute"))
            .then(|| timing_disqualification(&identities, &request.seats))
            .flatten();
        if identities.len() != 4
            || identities.iter().any(|(seat, entry)| {
                let Some(result) = request.seats.iter().find(|result| result.seat == *seat) else {
                    return true;
                };
                let summary = event_summaries.iter().find(|summary| summary.0 == *seat);
                let event_metrics_match = match summary {
                    Some((_, count, total, min, max, strikes)) => {
                        i64::from(result.decision_count) == *count
                            && result.decision_total_us == *total
                            && result.decision_min_us == *min
                            && result.decision_max_us == *max
                            && i64::from(result.strikes) == *strikes
                    }
                    None => {
                        result.decision_count == 0
                            && result.decision_total_us == 0
                            && result.decision_min_us.is_none()
                            && result.decision_max_us.is_none()
                            && result.strikes == 0
                    }
                };
                if !event_metrics_match {
                    return true;
                }
                match entry {
                    Some(_) => {
                        result.disqualified != (result.strikes == 3 || timing_seat == Some(*seat))
                    }
                    None => {
                        result.strikes != 0
                            || result.decision_count != 0
                            || result.disqualified
                            || result.replacement_bot.is_some()
                    }
                }
            })
        {
            return Err(StatusCode::BAD_REQUEST.into_response());
        }
        let expected_winner = identities
            .iter()
            .filter_map(|(seat, entry)| {
                let entry = entry.as_ref()?;
                let result = request
                    .seats
                    .iter()
                    .find(|result| result.seat == *seat && !result.disqualified)?;
                Some((*seat, *entry, result.final_net_worth))
            })
            .max_by(|left, right| {
                left.2
                    .total_cmp(&right.2)
                    .then_with(|| right.0.cmp(&left.0))
            })
            .map(|(seat, entry, _)| (seat, entry));
        let winner_valid = expected_winner == request.winner_seat.zip(request.winner_entry_id)
            && request.winner_seat.is_some() == request.winner_entry_id.is_some();
        if !winner_valid {
            return Err(StatusCode::BAD_REQUEST.into_response());
        }
        for seat in &request.seats {
            let runtime_log = seat
                .runtime_log
                .as_deref()
                .map(|value| value.chars().take(4000).collect::<String>());
            sqlx::query(
                "update monopoly_game_seats_exposure_academy set
                   final_cash = $3, final_net_worth = $4, strikes = $5,
                   decision_count = $6, decision_total_us = $7,
                   decision_min_us = $8, decision_max_us = $9,
                   disqualified = $10, replacement_bot = $11,
                   runtime_log = $12, final_snapshot = $13::jsonb
                 where game_id = $1 and seat = $2",
            )
            .bind(request.game_id)
            .bind(seat.seat)
            .bind(seat.final_cash)
            .bind(seat.final_net_worth)
            .bind(seat.strikes)
            .bind(seat.decision_count)
            .bind(seat.decision_total_us)
            .bind(seat.decision_min_us)
            .bind(seat.decision_max_us)
            .bind(seat.disqualified)
            .bind(&seat.replacement_bot)
            .bind(runtime_log)
            .bind(seat.final_snapshot.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|error| db_error("seat result", error))?;
        }
        sqlx::query(
            "update monopoly_game_attempts_exposure_academy set
               status = 'completed', finished_at = now()
             where lease_id = $1 and status = 'leased'",
        )
        .bind(request.lease_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("completed attempt", error))?;
        sqlx::query(
            "update monopoly_games_exposure_academy set
               status = 'done', lease_id = null, lease_worker_id = null, lease_expires_at = null,
               winner_seat = $2, winner_entry_id = $3, end_reason = $4,
               duration_us = $5, round = $6, action_count = $7,
               error_log = null, finished_at = now(), updated_at = now()
             where id = $1",
        )
        .bind(request.game_id)
        .bind(request.winner_seat)
        .bind(request.winner_entry_id)
        .bind(request.end_reason)
        .bind(request.duration_us)
        .bind(request.round)
        .bind(request.action_count)
        .execute(&mut *tx)
        .await
        .map_err(|error| db_error("completed game", error))?;
    }
    sqlx::query(
        "update monopoly_workers_exposure_academy set status = 'ready', current_game_id = null,
           last_seen_at = now(), updated_at = now() where worker_id = $1",
    )
    .bind(&request.worker_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| db_error("worker ready", error))?;
    refresh_tournament(&mut tx, tournament_id)
        .await
        .map_err(|error| db_error("tournament result refresh", error))?;
    tx.commit()
        .await
        .map_err(|error| db_error("result commit", error))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_rl_monopoly_demand(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    let (validation, games): (bool, bool) = sqlx::query_as(
        "select exists(select 1 from monopoly_submissions_exposure_academy
                       where status in ('pending','validating')),
                exists(select 1 from monopoly_games_exposure_academy
                       where status in ('queued','leased'))",
    )
    .fetch_one(&app.pool)
    .await
    .map_err(|error| db_error("demand", error))?;
    Ok(Json(serde_json::json!({
        "validation": validation,
        "games": games,
        "demand": validation || games,
    })))
}

/// Retention is a background task, never work performed by an HTTP request. Full step
/// replays expire after 30 days; final seat snapshots and summaries remain. Artifact
/// metadata remains as frozen provenance, while the archive file is removed once neither
/// the current approved submission nor an active tournament can use it.
async fn run_monopoly_retention_once(app: &App) {
    if let Err(error) = sqlx::query(
        "delete from monopoly_events_exposure_academy
         where created_at < now() - interval '30 days'",
    )
    .execute(&app.pool)
    .await
    {
        eprintln!("AI Monopoly replay retention failed: {error}");
    }
    let candidates: Vec<String> = sqlx::query_scalar(
        "select a.sha256 from monopoly_artifacts_exposure_academy a
         where a.last_used_at < now() - interval '30 days'
           and not exists (
             select 1 from monopoly_submissions_exposure_academy s
             where s.artifact_sha256 = a.sha256 and s.status = 'approved'
           )
           and not exists (
             select 1 from monopoly_tournament_entries_exposure_academy e
             join monopoly_tournaments_exposure_academy t on t.id = e.tournament_id
             where e.artifact_sha256 = a.sha256 and t.status = 'active'
           )",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    for sha256 in candidates {
        if valid_sha(&sha256, 64) {
            let path = app.monopoly_artifact_dir.join(format!("{sha256}.tar.gz"));
            if let Err(error) = tokio::fs::remove_file(path).await
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("AI Monopoly artifact retention failed for {sha256}: {error}");
            }
        }
    }
    let known: HashSet<String> =
        sqlx::query_scalar("select sha256 from monopoly_artifacts_exposure_academy")
            .fetch_all(&app.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
    if let Ok(mut directory) = tokio::fs::read_dir(&app.monopoly_artifact_dir).await {
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(30 * 24 * 60 * 60));
        while let Ok(Some(entry)) = directory.next_entry().await {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let sha256 = name.strip_suffix(".tar.gz");
            let orphan_sha =
                sha256.is_some_and(|sha256| valid_sha(sha256, 64) && !known.contains(sha256));
            let abandoned_validation =
                name.starts_with(".validation-") && name.ends_with(".tar.gz");
            if !orphan_sha && !abandoned_validation {
                continue;
            }
            let old_enough = match (entry.metadata().await, cutoff) {
                (Ok(metadata), Some(cutoff)) => {
                    metadata.modified().is_ok_and(|modified| modified < cutoff)
                }
                _ => false,
            };
            if old_enough
                && let Err(error) = tokio::fs::remove_file(entry.path()).await
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("AI Monopoly orphan artifact cleanup failed for {name}: {error}");
            }
        }
    }
}

pub fn spawn_monopoly_retention(app: App) {
    tokio::spawn(async move {
        loop {
            run_monopoly_retention_once(&app).await;
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

// ---- authenticated export ------------------------------------------------

pub async fn monopoly_export(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let tournament: MonopolyTournament = sqlx::query_as(
        "select id, status, seed, ruleset_version, schema_version, total_games,
                completed_games, partial_reason, created_at, started_at, completed_at
         from monopoly_tournaments_exposure_academy where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .map_err(|error| db_error("export tournament", error))?
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let entries: Vec<serde_json::Value> = sqlx::query_scalar(
        "select jsonb_build_object(
           'id', e.id, 'team_id', e.team_id, 'team_name', e.team_name,
           'submission_generation', e.submission_generation, 'repo_url', e.repo_url,
           'commit_sha', e.commit_sha, 'agent_path', e.agent_path,
           'artifact_sha256', e.artifact_sha256, 'dependency_lock', e.dependency_lock)
         from monopoly_tournament_entries_exposure_academy e
         where e.tournament_id = $1 order by lower(e.team_name), e.id",
    )
    .bind(id)
    .fetch_all(&app.pool)
    .await
    .map_err(|error| db_error("export entries", error))?;
    let artifacts: Vec<serde_json::Value> = sqlx::query_scalar(
        "select jsonb_build_object(
           'sha256', a.sha256, 'size_bytes', a.size_bytes,
           'created_at', a.created_at, 'last_used_at', a.last_used_at)
         from monopoly_artifacts_exposure_academy a
         where a.sha256 in (
           select e.artifact_sha256 from monopoly_tournament_entries_exposure_academy e
           where e.tournament_id = $1
         ) order by a.sha256",
    )
    .bind(id)
    .fetch_all(&app.pool)
    .await
    .map_err(|error| db_error("export artifacts", error))?;
    let games: Vec<MonopolyGame> = sqlx::query_as(&format!(
        "{GAME_SELECT} where g.tournament_id = $1 order by g.game_no"
    ))
    .bind(id)
    .fetch_all(&app.pool)
    .await
    .map_err(|error| db_error("export games", error))?;
    let workers: Vec<MonopolyWorker> = sqlx::query_as(
        "select worker_id, kind, session_name, status, ram_bytes, effective_vcpus,
                machine_shape, preflight_reason, current_game_id, last_seen_at
         from monopoly_workers_exposure_academy order by worker_id",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(|error| db_error("export workers", error))?;
    let mut prefix = serde_json::json!({
        "tournament": tournament,
        "entries": entries,
        "artifacts": artifacts,
        "standings": standings_for_tournament(&app, id).await,
        "workers": workers,
    })
    .to_string();
    prefix.pop();
    prefix.push_str(",\"games\":[");
    let pool = app.pool.clone();
    let is_admin = user.is_admin;
    let viewer_team_id = if is_admin {
        None
    } else {
        monopoly_team_of(&app, user.id).await.map(|team| team.id)
    };
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(2);
    tokio::spawn(async move {
        if sender.send(Ok(Bytes::from(prefix))).await.is_err() {
            return;
        }
        let mut first = true;
        let mut interrupted = false;
        for game in games {
            let attempts: Vec<serde_json::Value> = match sqlx::query_scalar(
                "select jsonb_build_object(
                   'attempt_no', a.attempt_no, 'lease_id', a.lease_id,
                   'worker_id', a.worker_id, 'status', a.status,
                   'claimed_at', a.claimed_at, 'last_heartbeat_at', a.last_heartbeat_at,
                   'finished_at', a.finished_at, 'error_log', a.error_log,
                   'events', coalesce((
                     select jsonb_agg(jsonb_build_object(
                       'seq', x.seq, 'acted_player', x.acted_player, 'round', x.round,
                       'phase', x.phase, 'action_idx', x.action_idx,
                       'action_desc', x.action_desc, 'decision_us', x.decision_us,
                       'strike', x.strike, 'info', x.info, 'snapshot', x.snapshot,
                       'created_at', x.created_at) order by x.seq)
                     from monopoly_events_exposure_academy x
                     where x.game_id = a.game_id and x.attempt_no = a.attempt_no
                   ), '[]'::jsonb))
                 from monopoly_game_attempts_exposure_academy a
                 where a.game_id = $1 order by a.attempt_no",
            )
            .bind(game.id)
            .fetch_all(&pool)
            .await
            {
                Ok(attempts) => attempts,
                Err(error) => {
                    eprintln!("AI Monopoly streaming export attempts: {error}");
                    interrupted = true;
                    break;
                }
            };
            let mut value = game_json(&game, &[]);
            value["attempts"] = serde_json::Value::Array(attempts);
            // Logs are the only result detail not visible tournament-wide. A team sees
            // its own tail; admins see all.
            let logs = if is_admin {
                Some(
                    sqlx::query_scalar(
                        "select jsonb_build_object('seat', s.seat, 'runtime_log', s.runtime_log)
                         from monopoly_game_seats_exposure_academy s
                         where s.game_id = $1 and s.runtime_log is not null order by s.seat",
                    )
                    .bind(game.id)
                    .fetch_all(&pool)
                    .await,
                )
            } else if let Some(team_id) = viewer_team_id {
                Some(
                    sqlx::query_scalar(
                        "select jsonb_build_object('seat', s.seat, 'runtime_log', s.runtime_log)
                         from monopoly_game_seats_exposure_academy s
                         join monopoly_tournament_entries_exposure_academy e on e.id = s.entry_id
                         where s.game_id = $1 and e.team_id = $2 and s.runtime_log is not null",
                    )
                    .bind(game.id)
                    .bind(team_id)
                    .fetch_all(&pool)
                    .await,
                )
            } else {
                None
            };
            if let Some(logs) = logs {
                match logs {
                    Ok(logs) => value["runtime_logs"] = serde_json::Value::Array(logs),
                    Err(error) => {
                        eprintln!("AI Monopoly streaming export logs: {error}");
                        interrupted = true;
                        break;
                    }
                }
            }
            let separator = if first { "" } else { "," };
            first = false;
            if sender
                .send(Ok(Bytes::from(format!("{separator}{value}"))))
                .await
                .is_err()
            {
                return;
            }
        }
        let suffix = if interrupted {
            "],\"export_error\":\"database read interrupted\"}"
        } else {
            "]}"
        };
        let _ = sender.send(Ok(Bytes::from_static(suffix.as_bytes()))).await;
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"ai-monopoly-tournament.json\"",
        )
        .body(Body::from_stream(ReceiverStream::new(receiver)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    fn worker_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-worker-token", "test-worker-token".parse().unwrap());
        headers
    }

    fn test_app(pool: PgPool) -> App {
        App {
            pool,
            worker_token: "test-worker-token".into(),
            http: reqwest::Client::new(),
            resend_key: String::new(),
            mail_from: String::new(),
            base_url: "http://127.0.0.1".into(),
            microlink_key: String::new(),
            deepl_key: String::new(),
            monopoly_artifact_dir: std::env::temp_dir(),
            kaggle_key: None,
            deepinfra_api_key: String::new(),
        }
    }

    async fn insert_test_game(pool: &PgPool) -> (Uuid, Uuid) {
        let tournament_id = Uuid::new_v4();
        let game_id = Uuid::new_v4();
        sqlx::query(
            "insert into monopoly_tournaments_exposure_academy
               (id, seed, total_games) values ($1, 17, 1)",
        )
        .bind(tournament_id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into monopoly_games_exposure_academy
               (id, tournament_id, game_no, seed) values ($1,$2,1,29)",
        )
        .bind(game_id)
        .bind(tournament_id)
        .execute(pool)
        .await
        .unwrap();
        for seat in 0_i16..4 {
            sqlx::query(
                "insert into monopoly_game_seats_exposure_academy
                   (game_id, seat, bot_key, label) values ($1,$2,'hoarder',$3)",
            )
            .bind(game_id)
            .bind(seat)
            .bind(format!("Bot {seat}"))
            .execute(pool)
            .await
            .unwrap();
        }
        (tournament_id, game_id)
    }

    fn lease_from_claim(worker_id: &str, job: &serde_json::Value) -> GameLeaseReq {
        GameLeaseReq {
            worker_id: worker_id.into(),
            game_id: job["id"].as_str().unwrap().parse().unwrap(),
            attempt_no: job["attempt_no"].as_i64().unwrap() as i32,
            lease_id: job["lease_id"].as_str().unwrap().parse().unwrap(),
        }
    }

    fn infra_result(worker_id: &str, job: &serde_json::Value) -> GameResultReq {
        let lease = lease_from_claim(worker_id, job);
        GameResultReq {
            worker_id: lease.worker_id,
            game_id: lease.game_id,
            attempt_no: lease.attempt_no,
            lease_id: lease.lease_id,
            outcome: "infra_failed".into(),
            end_reason: None,
            winner_seat: None,
            winner_entry_id: None,
            duration_us: None,
            round: None,
            action_count: None,
            seats: vec![],
            error_log: Some("test infrastructure failure".into()),
        }
    }

    fn completed_result(
        worker_id: &str,
        job: &serde_json::Value,
        winner_seat: i16,
        winner_entry_id: Uuid,
    ) -> GameResultReq {
        let lease = lease_from_claim(worker_id, job);
        GameResultReq {
            worker_id: lease.worker_id,
            game_id: lease.game_id,
            attempt_no: lease.attempt_no,
            lease_id: lease.lease_id,
            outcome: "completed".into(),
            end_reason: Some("normal".into()),
            winner_seat: Some(winner_seat),
            winner_entry_id: Some(winner_entry_id),
            duration_us: Some(2_000_000),
            round: Some(3),
            action_count: Some(2),
            seats: (0_i16..4)
                .map(|seat| {
                    let submitted = seat < 2;
                    SeatResultReq {
                        seat,
                        final_cash: f64::from(100 + seat * 10),
                        final_net_worth: [100.0, 200.0, 9_999.0, 300.0][seat as usize],
                        strikes: 0,
                        decision_count: i32::from(submitted),
                        decision_total_us: if submitted { 1_000 } else { 0 },
                        decision_min_us: submitted.then_some(1_000),
                        decision_max_us: submitted.then_some(1_000),
                        disqualified: false,
                        replacement_bot: None,
                        runtime_log: submitted.then(|| format!("private seat {seat}")),
                        final_snapshot: serde_json::json!({
                            "done": true,
                            "round": 3,
                            "players": [
                                {"player_id": 0, "cash": 100.0, "net_worth": 100.0},
                                {"player_id": 1, "cash": 110.0, "net_worth": 200.0},
                                {"player_id": 2, "cash": 120.0, "net_worth": 9999.0},
                                {"player_id": 3, "cash": 130.0, "net_worth": 300.0}
                            ]
                        }),
                    }
                })
                .collect(),
            error_log: None,
        }
    }

    fn assert_schedule(team_count: usize, seed: i64) {
        let schedule = generate_schedule(team_count, seed).unwrap();
        assert_eq!(schedule.len(), (team_count * 6).div_ceil(4));
        let mut appearances = vec![0usize; team_count];
        let mut seats = vec![[0usize; 4]; team_count];
        for game in &schedule {
            let teams: Vec<usize> = game.iter().flatten().copied().collect();
            let mut unique = teams.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(teams.len(), unique.len(), "team repeated in one game");
            for (seat, team) in game.iter().enumerate() {
                if let Some(team) = team {
                    appearances[*team] += 1;
                    seats[*team][seat] += 1;
                }
            }
        }
        assert!(appearances.iter().all(|count| *count == 6));
        assert_eq!(
            schedule
                .iter()
                .flatten()
                .filter(|seat| seat.is_none())
                .count(),
            schedule.len() * 4 - team_count * 6
        );
        for counts in seats {
            assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
        }
        assert_eq!(schedule, generate_schedule(team_count, seed).unwrap());
    }

    #[test]
    fn scheduler_properties_across_team_counts() {
        for team_count in 4..=32 {
            for seed in [0, 1, 42, -17, i64::MAX] {
                assert_schedule(team_count, seed);
            }
        }
    }

    #[test]
    fn scheduler_limits_repeated_opponents() {
        for team_count in 5..=24 {
            let schedule = generate_schedule(team_count, 91).unwrap();
            let mut pairs = HashMap::new();
            for game in schedule {
                let teams: Vec<_> = game.into_iter().flatten().collect();
                record_pairs(&teams, &mut pairs);
            }
            let max = pairs.values().copied().max().unwrap_or_default();
            let theoretical_average = 18.0 / (team_count - 1) as f64;
            assert!(f64::from(max) <= theoretical_average.ceil() + 2.0);
        }
    }

    #[test]
    fn agent_paths_are_relative_and_normalized() {
        assert_eq!(validate_agent_path("agent.py").unwrap(), "agent.py");
        assert_eq!(
            validate_agent_path("src/team_agent.py").unwrap(),
            "src/team_agent.py"
        );
        for unsafe_path in [
            "",
            "/agent.py",
            "../agent.py",
            "src/../agent.py",
            "src\\agent.py",
        ] {
            assert!(
                validate_agent_path(unsafe_path).is_err(),
                "accepted {unsafe_path}"
            );
        }
    }

    /// Run against a disposable PostgreSQL database:
    /// `MONOPOLY_TEST_DATABASE_URL=postgres://... cargo test -p academy monopoly::tests::lease_protocol_database -- --ignored`
    #[tokio::test]
    #[ignore = "requires a disposable PostgreSQL database"]
    async fn lease_protocol_database() {
        let database_url = std::env::var("MONOPOLY_TEST_DATABASE_URL")
            .expect("MONOPOLY_TEST_DATABASE_URL must point to a disposable database");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(6)
            .connect(&database_url)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/018_rl_monopoly.sql"))
            .execute(&pool)
            .await
            .unwrap();
        for worker in ["lease-worker-a", "lease-worker-b"] {
            sqlx::query(
                "insert into monopoly_workers_exposure_academy
                   (worker_id, kind, status, ram_bytes, effective_vcpus)
                 values ($1,'colab','ready',$2,8)
                 on conflict (worker_id) do update set status = 'ready',
                   ram_bytes = excluded.ram_bytes, effective_vcpus = excluded.effective_vcpus,
                   current_game_id = null",
            )
            .bind(worker)
            .bind(32_i64 * 1024 * 1024 * 1024)
            .execute(&pool)
            .await
            .unwrap();
        }
        let app = test_app(pool.clone());
        let unauthenticated_artifact =
            worker_rl_monopoly_artifact(State(app.clone()), HeaderMap::new(), Path("a".repeat(64)))
                .await
                .unwrap_err();
        assert_eq!(unauthenticated_artifact.status(), StatusCode::UNAUTHORIZED);
        let missing_artifact =
            worker_rl_monopoly_artifact(State(app.clone()), worker_headers(), Path("a".repeat(64)))
                .await
                .unwrap_err();
        assert_eq!(missing_artifact.status(), StatusCode::NOT_FOUND);

        let validation_team = Uuid::new_v4();
        sqlx::query("insert into monopoly_teams_exposure_academy (id, name) values ($1,$2)")
            .bind(validation_team)
            .bind(format!("Validation {validation_team}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "insert into monopoly_submissions_exposure_academy
               (team_id, repo_url, agent_path) values ($1,'https://github.com/test/agent','agent.py')",
        )
        .bind(validation_team)
        .execute(&pool)
        .await
        .unwrap();
        let Json(validation) = worker_rl_monopoly_validation_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: "validation-worker".into(),
            }),
        )
        .await
        .unwrap();
        let validation_id: Uuid = validation["id"].as_str().unwrap().parse().unwrap();
        let validation_lease: Uuid = validation["lease_id"].as_str().unwrap().parse().unwrap();
        sqlx::query(
            "update monopoly_submissions_exposure_academy set generation = generation + 1,
               status = 'pending', validation_lease_id = null, validation_worker_id = null,
               validation_lease_expires_at = null where id = $1",
        )
        .bind(validation_id)
        .execute(&pool)
        .await
        .unwrap();
        let stale_validation = worker_rl_monopoly_validation_result(
            State(app.clone()),
            worker_headers(),
            Json(ValidationResultReq {
                worker_id: "validation-worker".into(),
                id: validation_id,
                generation: 1,
                lease_id: validation_lease,
                status: "failed".into(),
                commit_sha: None,
                repo_size_bytes: None,
                artifact_sha256: None,
                artifact_size_bytes: None,
                dependency_lock: None,
                validation_log: Some("stale".into()),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(stale_validation.status(), StatusCode::CONFLICT);

        let Json(expiring_validation) = worker_rl_monopoly_validation_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: "validation-worker".into(),
            }),
        )
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_submissions_exposure_academy
             set validation_lease_expires_at = now() - interval '1 second' where id = $1",
        )
        .bind(validation_id)
        .execute(&pool)
        .await
        .unwrap();
        let expired_validation = worker_rl_monopoly_validation_heartbeat(
            State(app.clone()),
            worker_headers(),
            Json(ValidationHeartbeatReq {
                worker_id: "validation-worker".into(),
                id: validation_id,
                generation: expiring_validation["generation"].as_i64().unwrap() as i32,
                lease_id: expiring_validation["lease_id"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(expired_validation.status(), StatusCode::CONFLICT);

        let (tournament_id, game_id) = insert_test_game(&pool).await;

        let claim_a = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: "lease-worker-a".into(),
            }),
        );
        let claim_b = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: "lease-worker-b".into(),
            }),
        );
        let (result_a, result_b) = tokio::join!(claim_a, claim_b);
        let Json(result_a) = result_a.unwrap();
        let Json(result_b) = result_b.unwrap();
        let (worker, job) = match (result_a["game"].as_object(), result_b["game"].as_object()) {
            (Some(_), None) => ("lease-worker-a", result_a["game"].clone()),
            (None, Some(_)) => ("lease-worker-b", result_b["game"].clone()),
            state => panic!("exactly one concurrent claim must win: {state:?}"),
        };
        assert_eq!(job["id"], game_id.to_string());
        assert_eq!(job["attempt_no"], 1);

        let stale_lease = GameLeaseReq {
            lease_id: Uuid::new_v4(),
            ..lease_from_claim(worker, &job)
        };
        let stale_heartbeat =
            worker_rl_monopoly_heartbeat(State(app.clone()), worker_headers(), Json(stale_lease))
                .await
                .unwrap_err();
        assert_eq!(stale_heartbeat.status(), StatusCode::CONFLICT);

        let event_lease = lease_from_claim(worker, &job);
        assert_eq!(
            worker_rl_monopoly_events(
                State(app.clone()),
                worker_headers(),
                Json(EventBatchReq {
                    worker_id: event_lease.worker_id,
                    game_id: event_lease.game_id,
                    attempt_no: event_lease.attempt_no,
                    lease_id: event_lease.lease_id,
                    events: vec![WorkerEvent {
                        seq: 1,
                        acted_player: 0,
                        round: 1,
                        phase: "test".into(),
                        action_idx: 1,
                        action_desc: "test action".into(),
                        decision_us: Some(500),
                        strike: false,
                        info: serde_json::json!({}),
                        snapshot: serde_json::json!({}),
                    }],
                }),
            )
            .await
            .unwrap(),
            StatusCode::NO_CONTENT
        );
        worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(infra_result(worker, &job)),
        )
        .await
        .unwrap();

        let retry_worker = if worker == "lease-worker-a" {
            "lease-worker-b"
        } else {
            "lease-worker-a"
        };
        let Json(retry) = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: retry_worker.into(),
            }),
        )
        .await
        .unwrap();
        let retry = retry["game"].clone();
        assert_eq!(retry["id"], game_id.to_string());
        assert_eq!(retry["attempt_no"], 2);

        let stale_result = worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(infra_result(worker, &job)),
        )
        .await
        .unwrap_err();
        assert_eq!(stale_result.status(), StatusCode::CONFLICT);
        let stale_event_lease = lease_from_claim(worker, &job);
        let stale_events = worker_rl_monopoly_events(
            State(app.clone()),
            worker_headers(),
            Json(EventBatchReq {
                worker_id: stale_event_lease.worker_id,
                game_id: stale_event_lease.game_id,
                attempt_no: stale_event_lease.attempt_no,
                lease_id: stale_event_lease.lease_id,
                events: vec![WorkerEvent {
                    seq: 2,
                    acted_player: 0,
                    round: 1,
                    phase: "stale".into(),
                    action_idx: 1,
                    action_desc: "stale action".into(),
                    decision_us: None,
                    strike: false,
                    info: serde_json::json!({}),
                    snapshot: serde_json::json!({}),
                }],
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(stale_events.status(), StatusCode::CONFLICT);

        worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(infra_result(retry_worker, &retry)),
        )
        .await
        .unwrap();
        let Json(third) = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: worker.into(),
            }),
        )
        .await
        .unwrap();
        let third = third["game"].clone();
        assert_eq!(third["attempt_no"], 3);
        worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(infra_result(worker, &third)),
        )
        .await
        .unwrap();
        let (game_status, tournament_status): (String, String) = sqlx::query_as(
            "select g.status, t.status from monopoly_games_exposure_academy g
             join monopoly_tournaments_exposure_academy t on t.id = g.tournament_id
             where g.id = $1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(game_status, "failed");
        assert_eq!(tournament_status, "partial");

        let (cancelled_tournament, cancelled_game) = insert_test_game(&pool).await;
        let Json(cancelled_claim) = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: retry_worker.into(),
            }),
        )
        .await
        .unwrap();
        let cancelled_claim = cancelled_claim["game"].clone();
        sqlx::query(
            "update monopoly_games_exposure_academy
             set lease_expires_at = now() - interval '1 second' where id = $1",
        )
        .bind(cancelled_game)
        .execute(&pool)
        .await
        .unwrap();
        let expired_heartbeat = worker_rl_monopoly_heartbeat(
            State(app.clone()),
            worker_headers(),
            Json(lease_from_claim(retry_worker, &cancelled_claim)),
        )
        .await
        .unwrap_err();
        assert_eq!(expired_heartbeat.status(), StatusCode::CONFLICT);
        sqlx::query(
            "update monopoly_games_exposure_academy
             set lease_expires_at = now() + interval '90 seconds' where id = $1",
        )
        .bind(cancelled_game)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_game_attempts_exposure_academy set status = 'cancelled', finished_at = now()
             where game_id = $1 and status = 'leased'",
        )
        .bind(cancelled_game)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_games_exposure_academy set status = 'cancelled', lease_id = null,
               lease_worker_id = null, lease_expires_at = null where id = $1",
        )
        .bind(cancelled_game)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_tournaments_exposure_academy set status = 'cancelled', completed_at = now()
             where id = $1",
        )
        .bind(cancelled_tournament)
        .execute(&pool)
        .await
        .unwrap();
        let cancelled_heartbeat = worker_rl_monopoly_heartbeat(
            State(app.clone()),
            worker_headers(),
            Json(lease_from_claim(retry_worker, &cancelled_claim)),
        )
        .await
        .unwrap_err();
        assert_eq!(cancelled_heartbeat.status(), StatusCode::CONFLICT);

        sqlx::query(
            "update monopoly_workers_exposure_academy set status = 'ready', current_game_id = null
             where worker_id in ('lease-worker-a','lease-worker-b')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let artifact_sha = "b".repeat(64);
        sqlx::query(
            "insert into monopoly_artifacts_exposure_academy (sha256, size_bytes)
             values ($1,1)",
        )
        .bind(&artifact_sha)
        .execute(&pool)
        .await
        .unwrap();
        let completed_tournament = Uuid::new_v4();
        sqlx::query(
            "insert into monopoly_tournaments_exposure_academy
               (id, seed, total_games) values ($1,31,1)",
        )
        .bind(completed_tournament)
        .execute(&pool)
        .await
        .unwrap();
        let completed_game = Uuid::new_v4();
        sqlx::query(
            "insert into monopoly_games_exposure_academy
               (id, tournament_id, game_no, seed) values ($1,$2,1,37)",
        )
        .bind(completed_game)
        .bind(completed_tournament)
        .execute(&pool)
        .await
        .unwrap();
        let mut result_teams = Vec::new();
        let mut result_entries = Vec::new();
        for index in 0..2 {
            let team_id = Uuid::new_v4();
            let entry_id = Uuid::new_v4();
            sqlx::query("insert into monopoly_teams_exposure_academy (id, name) values ($1,$2)")
                .bind(team_id)
                .bind(format!("Result Team {index} {team_id}"))
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query(
                "insert into monopoly_tournament_entries_exposure_academy
                   (id, tournament_id, team_id, submission_generation, team_name, repo_url,
                    commit_sha, agent_path, artifact_sha256, dependency_lock)
                 values ($1,$2,$3,1,$4,'https://github.com/test/agent',$5,'agent.py',$6,'')",
            )
            .bind(entry_id)
            .bind(completed_tournament)
            .bind(team_id)
            .bind(format!("Result Team {index}"))
            .bind("c".repeat(40))
            .bind(&artifact_sha)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "insert into monopoly_game_seats_exposure_academy
                   (game_id, seat, entry_id, label) values ($1,$2,$3,$4)",
            )
            .bind(completed_game)
            .bind(index as i16)
            .bind(entry_id)
            .bind(format!("Result Team {index}"))
            .execute(&pool)
            .await
            .unwrap();
            result_teams.push(team_id);
            result_entries.push(entry_id);
        }
        for seat in 2_i16..4 {
            sqlx::query(
                "insert into monopoly_game_seats_exposure_academy
                   (game_id, seat, bot_key, label) values ($1,$2,'hoarder',$3)",
            )
            .bind(completed_game)
            .bind(seat)
            .bind(format!("Bot {seat}"))
            .execute(&pool)
            .await
            .unwrap();
        }
        let Json(completed_claim) = worker_rl_monopoly_claim(
            State(app.clone()),
            worker_headers(),
            Json(WorkerIdentityReq {
                worker_id: "lease-worker-a".into(),
            }),
        )
        .await
        .unwrap();
        let completed_claim = completed_claim["game"].clone();
        let completed_lease = lease_from_claim("lease-worker-a", &completed_claim);
        worker_rl_monopoly_events(
            State(app.clone()),
            worker_headers(),
            Json(EventBatchReq {
                worker_id: completed_lease.worker_id,
                game_id: completed_lease.game_id,
                attempt_no: completed_lease.attempt_no,
                lease_id: completed_lease.lease_id,
                events: (0_i16..2)
                    .map(|seat| WorkerEvent {
                        seq: i32::from(seat) + 1,
                        acted_player: seat,
                        round: 3,
                        phase: "test".into(),
                        action_idx: 1,
                        action_desc: "test action".into(),
                        decision_us: Some(1_000),
                        strike: false,
                        info: serde_json::json!({}),
                        snapshot: serde_json::json!({}),
                    })
                    .collect(),
            }),
        )
        .await
        .unwrap();
        let wrong_winner = worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(completed_result(
                "lease-worker-a",
                &completed_claim,
                0,
                result_entries[0],
            )),
        )
        .await
        .unwrap_err();
        assert_eq!(wrong_winner.status(), StatusCode::BAD_REQUEST);

        let mut tampered_metrics =
            completed_result("lease-worker-a", &completed_claim, 1, result_entries[1]);
        tampered_metrics.seats[0].decision_total_us = 999;
        let tampered_metrics =
            worker_rl_monopoly_result(State(app.clone()), worker_headers(), Json(tampered_metrics))
                .await
                .unwrap_err();
        assert_eq!(tampered_metrics.status(), StatusCode::BAD_REQUEST);

        let mut arbitrary_disqualification =
            completed_result("lease-worker-a", &completed_claim, 1, result_entries[1]);
        arbitrary_disqualification.seats[0].disqualified = true;
        arbitrary_disqualification.seats[0].replacement_bot = Some("hoarder".into());
        let arbitrary_disqualification = worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(arbitrary_disqualification),
        )
        .await
        .unwrap_err();
        assert_eq!(arbitrary_disqualification.status(), StatusCode::BAD_REQUEST);

        let mut wrong_timing_disqualification =
            completed_result("lease-worker-a", &completed_claim, 0, result_entries[0]);
        wrong_timing_disqualification.end_reason = Some("ten_minute".into());
        wrong_timing_disqualification.duration_us = Some(PLAY_LIMIT_US);
        wrong_timing_disqualification.seats[1].disqualified = true;
        wrong_timing_disqualification.seats[1].replacement_bot = Some("dealmaker".into());
        for seat in &mut wrong_timing_disqualification.seats {
            seat.final_snapshot["done"] = serde_json::json!(false);
        }
        let wrong_timing_disqualification = worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(wrong_timing_disqualification),
        )
        .await
        .unwrap_err();
        assert_eq!(
            wrong_timing_disqualification.status(),
            StatusCode::BAD_REQUEST
        );

        let mut correct_timing_disqualification =
            completed_result("lease-worker-a", &completed_claim, 1, result_entries[1]);
        correct_timing_disqualification.end_reason = Some("ten_minute".into());
        correct_timing_disqualification.duration_us = Some(PLAY_LIMIT_US);
        correct_timing_disqualification.seats[0].disqualified = true;
        correct_timing_disqualification.seats[0].replacement_bot = Some("hoarder".into());
        // The decision that crosses ten minutes may also finish round 200. The
        // timing rule still takes precedence, and the truthful snapshot is done.
        for seat in &mut correct_timing_disqualification.seats {
            seat.final_snapshot["done"] = serde_json::json!(true);
        }
        worker_rl_monopoly_result(
            State(app.clone()),
            worker_headers(),
            Json(correct_timing_disqualification),
        )
        .await
        .unwrap();
        let standings = standings_for_tournament(&app, completed_tournament).await;
        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].entry_id, result_entries[1]);
        assert_eq!(standings[0].wins, 1);
        assert_eq!(standings[0].average_net_worth, 200.0);
        assert_eq!(standings[0].decision_count, 1);
        assert_eq!(standings[0].decision_min_us, Some(1_000));

        // Wins dominate wealth. With wins tied, average final worth dominates;
        // with both tied, fewer strikes wins the ordering.
        let tie_team = Uuid::new_v4();
        let tie_entry = Uuid::new_v4();
        sqlx::query("insert into monopoly_teams_exposure_academy (id, name) values ($1,$2)")
            .bind(tie_team)
            .bind(format!("Tie Team {tie_team}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "insert into monopoly_tournament_entries_exposure_academy
               (id, tournament_id, team_id, submission_generation, team_name, repo_url,
                commit_sha, agent_path, artifact_sha256, dependency_lock)
             values ($1,$2,$3,1,'Tie Team','https://github.com/test/tie',$4,'agent.py',$5,'')",
        )
        .bind(tie_entry)
        .bind(completed_tournament)
        .bind(tie_team)
        .bind("d".repeat(40))
        .bind(&artifact_sha)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_game_seats_exposure_academy
             set entry_id = $2, bot_key = null where game_id = $1 and seat = 2",
        )
        .bind(completed_game)
        .bind(tie_entry)
        .execute(&pool)
        .await
        .unwrap();
        result_teams.push(tie_team);
        result_entries.push(tie_entry);

        let standings = standings_for_tournament(&app, completed_tournament).await;
        assert_eq!(standings[0].entry_id, result_entries[1]);
        assert_eq!(standings[1].entry_id, tie_entry);
        assert_eq!(standings[2].entry_id, result_entries[0]);

        sqlx::query(
            "update monopoly_game_seats_exposure_academy set
               final_net_worth = 9999,
               strikes = case seat when 0 then 0 when 2 then 2 else strikes end,
               decision_count = case when seat = 2 then 2 else decision_count end,
               decision_total_us = case when seat = 2 then 2000 else decision_total_us end,
               decision_min_us = case when seat = 2 then 1000 else decision_min_us end,
               decision_max_us = case when seat = 2 then 1000 else decision_max_us end
             where game_id = $1 and seat in (0,2)",
        )
        .bind(completed_game)
        .execute(&pool)
        .await
        .unwrap();
        let standings = standings_for_tournament(&app, completed_tournament).await;
        assert_eq!(standings[0].entry_id, result_entries[1]);
        assert_eq!(standings[1].entry_id, result_entries[0]);
        assert_eq!(standings[2].entry_id, tie_entry);

        let retention_dir =
            std::env::temp_dir().join(format!("academy-monopoly-retention-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&retention_dir).await.unwrap();
        let stale_artifact_path = retention_dir.join(format!("{artifact_sha}.tar.gz"));
        tokio::fs::write(&stale_artifact_path, b"stale")
            .await
            .unwrap();
        sqlx::query(
            "update monopoly_artifacts_exposure_academy
             set last_used_at = now() - interval '31 days' where sha256 = $1",
        )
        .bind(&artifact_sha)
        .execute(&pool)
        .await
        .unwrap();

        let protected_sha = "e".repeat(64);
        let protected_artifact_path = retention_dir.join(format!("{protected_sha}.tar.gz"));
        tokio::fs::write(&protected_artifact_path, b"protected")
            .await
            .unwrap();
        sqlx::query(
            "insert into monopoly_artifacts_exposure_academy
               (sha256, size_bytes, last_used_at)
             values ($1,9,now() - interval '31 days')",
        )
        .bind(&protected_sha)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_submissions_exposure_academy set
               status = 'approved', commit_sha = $2, repo_size_bytes = 1,
               artifact_sha256 = $3, dependency_lock = '', approved_at = now(),
               validation_lease_id = null, validation_worker_id = null,
               validation_lease_expires_at = null
             where team_id = $1",
        )
        .bind(validation_team)
        .bind("e".repeat(40))
        .bind(&protected_sha)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "update monopoly_events_exposure_academy
             set created_at = now() - interval '31 days'
             where game_id = $1 and attempt_no = 1 and seq = 1",
        )
        .bind(completed_game)
        .execute(&pool)
        .await
        .unwrap();
        let retention_app = App {
            monopoly_artifact_dir: retention_dir.clone(),
            ..app.clone()
        };
        run_monopoly_retention_once(&retention_app).await;
        assert!(!stale_artifact_path.exists());
        assert!(protected_artifact_path.exists());
        let retained_events: i64 = sqlx::query_scalar(
            "select count(*) from monopoly_events_exposure_academy where game_id = $1",
        )
        .bind(completed_game)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(retained_events, 1);
        let retained_snapshots: i64 = sqlx::query_scalar(
            "select count(*) from monopoly_game_seats_exposure_academy
             where game_id = $1 and final_snapshot is not null",
        )
        .bind(completed_game)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(retained_snapshots, 4);
        tokio::fs::remove_file(&protected_artifact_path)
            .await
            .unwrap();
        tokio::fs::remove_dir(&retention_dir).await.unwrap();

        sqlx::query("delete from monopoly_tournaments_exposure_academy where id = any($1)")
            .bind([tournament_id, cancelled_tournament, completed_tournament])
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "delete from monopoly_workers_exposure_academy
             where worker_id in ('lease-worker-a','lease-worker-b')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("delete from monopoly_teams_exposure_academy where id = $1")
            .bind(validation_team)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("delete from monopoly_teams_exposure_academy where id = any($1)")
            .bind(&result_teams)
            .execute(&pool)
            .await
            .unwrap();
    }
}
