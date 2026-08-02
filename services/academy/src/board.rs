//! Görev panosu — task cards, project submissions, and the (server-side only)
//! review worker pipeline that predates the harness.

use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

/// True once the student has BOTH public profiles on file. The board is gated on this:
/// they could skip the profiles during onboarding, but not to reach the task board.
pub async fn has_both_profiles(app: &App, uid: Uuid) -> (bool, Option<String>, Option<String>) {
    let (github, linkedin): (Option<String>, Option<String>) =
        sqlx::query_as("select github_url, linkedin_url from users_exposure_academy where id = $1")
            .bind(uid)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let ok = github.as_deref().is_some_and(|s| !s.trim().is_empty())
        && linkedin.as_deref().is_some_and(|s| !s.trim().is_empty());
    (ok, github, linkedin)
}

pub async fn board(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // Gate: no board until both GitHub and LinkedIn are set. Skippable at onboarding,
    // enforced here — show the setup requirement instead of the tasks. Admins are exempt,
    // same as the onboarding gate (they don't onboard and need to see the board).
    if !user.is_admin {
        let (ok, github, linkedin) = has_both_profiles(&app, user.id).await;
        if !ok {
            return Ok(Html(html::board_locked(
                &user,
                github.as_deref(),
                linkedin.as_deref(),
                None,
            )));
        }
    }
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let subs = sqlx::query_as::<_, SubmissionView>(
        "select distinct on (s.task_id) s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url, s.plan_md, s.live_url,
                u.display_name, u.email, t.title as task_title, t.level as task_level, s.points_override, s.created_at
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         where s.user_id = $1 order by s.task_id, s.created_at desc")
        .bind(user.id).fetch_all(&app.pool).await.unwrap();
    // Include my own interest rows even when I have no nickname (admins don't
    // onboard, so nickname is null) — otherwise `mine`/`started` never flips for
    // them. Others still need a nickname to appear as a teammate chip. coalesce
    // keeps nickname a non-null String; blank ones are filtered out at render.
    // Hidden (intern) accounts are dropped for the same reason as on the leaderboard,
    // but `or u.id = $1` keeps their own row so their "Göreve başladım" state survives.
    let interests = sqlx::query_as::<_, InterestRow>(
        "select ti.task_id, coalesce(u.nickname, '') as nickname, (u.id = $1) as is_me
         from task_interest_exposure_academy ti
         join users_exposure_academy u on u.id = ti.user_id
         where (u.nickname is not null and not u.hidden_from_leaderboard) or u.id = $1
         order by ti.created_at",
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    // Which tasks have deployed sites worth linking to, and how many. The filter is kept
    // identical to board_sites' — a mismatch here is a button that opens an empty gallery.
    let site_counts = sqlx::query_as::<_, (Uuid, i64)>(
        "select s.task_id, count(distinct s.user_id)
         from submissions_exposure_academy s
         join users_exposure_academy u on u.id = s.user_id
         where s.live_url is not null and u.nickname is not null and not u.hidden_from_leaderboard
         group by s.task_id",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    Ok(Html(html::board(
        &user,
        &tasks,
        &subs,
        &interests,
        &site_counts,
    )))
}

/// Every student's deployed site for one task, side by side. Generic per task rather than
/// hardcoded to the personal-website project: any task that collects deployed sites grows
/// the button on its card, and nothing here knows which task that is.
pub async fn board_sites(
    State(app): State<App>,
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // same gate as the board itself, so this isn't a way around the profile requirement
    if !user.is_admin {
        let (ok, github, linkedin) = has_both_profiles(&app, user.id).await;
        if !ok {
            return Ok(Html(html::board_locked(
                &user,
                github.as_deref(),
                linkedin.as_deref(),
                None,
            )));
        }
    }
    let task = sqlx::query_as::<_, Task>(
        "select id, title, description, level, example_url, example_embeddable
         from tasks_exposure_academy where id = $1",
    )
    .bind(task_id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    // One card per student: distinct on (user_id) picks their latest deployed submission,
    // mirroring the board's distinct on (task_id). The wrapper select only exists to sort
    // by nickname — distinct on forces its own order by first. Visibility rule matches the
    // leaderboard and the teammate chips: onboarded, and not a hidden (intern) account.
    let cards = sqlx::query_as::<_, SiteCard>(
        "select * from (
           select distinct on (s.user_id) s.id, u.nickname, s.repo_url, s.live_url, s.live_embeddable
           from submissions_exposure_academy s
           join users_exposure_academy u on u.id = s.user_id
           where s.task_id = $1 and s.live_url is not null
             and u.nickname is not null and not u.hidden_from_leaderboard
           order by s.user_id, s.created_at desc
         ) x order by lower(x.nickname)")
        .bind(task_id).fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::board_sites(&user, &task, &cards)))
}

#[derive(Deserialize)]
pub struct ProfilesForm {
    #[serde(default)]
    github_url: String,
    #[serde(default)]
    linkedin_url: String,
}

/// Save the GitHub/LinkedIn profiles from the board gate. Both are required here (the
/// board stays locked otherwise); on success we drop the student straight into /board.
pub async fn board_profiles(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<ProfilesForm>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // re-render the gate with an error, echoing back what they typed
    let fail = |msg: &str| {
        Html(html::board_locked(
            &user,
            Some(f.github_url.trim()),
            Some(f.linkedin_url.trim()),
            Some(msg),
        ))
        .into_response()
    };

    let github = match normalize_profile_url(&f.github_url, "github.com") {
        Ok(Some(u)) => u,
        Ok(None) => return Ok(fail("GitHub profilini ekle.")),
        Err(()) => {
            return Ok(fail(
                "GitHub bağlantısı github.com adresinde olmalı (ör. https://github.com/kullanici).",
            ));
        }
    };
    let linkedin = match normalize_profile_url(&f.linkedin_url, "linkedin.com") {
        Ok(Some(u)) => u,
        Ok(None) => return Ok(fail("LinkedIn profilini ekle.")),
        Err(()) => {
            return Ok(fail(
                "LinkedIn bağlantısı linkedin.com adresinde olmalı (ör. https://linkedin.com/in/adin).",
            ));
        }
    };

    sqlx::query(
        "update users_exposure_academy set github_url = $2, linkedin_url = $3 where id = $1",
    )
    .bind(user.id)
    .bind(&github)
    .bind(&linkedin)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/board").into_response())
}

#[derive(Deserialize)]
pub struct InterestForm {
    task_id: Uuid,
}

// toggle: delete my interest if present, else add it. Two idempotent statements,
// no tx needed — worst case a double-click is a harmless no-op either way.
pub async fn board_interest(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<InterestForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let deleted = sqlx::query(
        "delete from task_interest_exposure_academy where task_id = $1 and user_id = $2",
    )
    .bind(f.task_id)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    if deleted.rows_affected() == 0 {
        sqlx::query("insert into task_interest_exposure_academy (task_id, user_id) values ($1,$2) on conflict do nothing")
            .bind(f.task_id).bind(user.id).execute(&app.pool).await
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/board"))
}

/// plan.md is stored inline in the DB as text, so it stays small.
pub const PLAN_MAX_BYTES: usize = 200 * 1024;

pub async fn board_submit(
    State(app): State<App>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let mut task_id: Option<Uuid> = None;
    let mut repo_url = String::new();
    let mut live_url = String::new();
    let mut plan_md: Option<String> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Form okunamadı."))? {
        // name() borrows the field, text()/bytes() consume it — copy the name out first
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "task_id" => task_id = field.text().await.ok().and_then(|t| t.parse().ok()),
            "repo_url" => {
                repo_url = field
                    .text()
                    .await
                    .map_err(|_| bad("Form okunamadı."))?
                    .trim()
                    .to_string()
            }
            "live_url" => {
                live_url = field
                    .text()
                    .await
                    .map_err(|_| bad("Form okunamadı."))?
                    .trim()
                    .to_string()
            }
            "plan" => {
                let bytes = field.bytes().await.map_err(|_| bad("plan.md okunamadı."))?;
                if bytes.len() > PLAN_MAX_BYTES {
                    return Err(bad("plan.md 200 KB'den büyük olamaz."));
                }
                let text = String::from_utf8(bytes.to_vec())
                    .map_err(|_| bad("plan.md UTF-8 metin olmalı."))?;
                if !text.trim().is_empty() {
                    plan_md = Some(text);
                }
            }
            _ => {}
        }
    }

    let Some(task_id) = task_id else {
        return Err(bad("Görev bulunamadı."));
    };
    if !repo_url.starts_with("https://github.com/") {
        return Err(bad("Repo bağlantısı https://github.com/ ile başlamalı."));
    }
    if !live_url.is_empty() && !live_url.starts_with("https://") && !live_url.starts_with("http://")
    {
        return Err(bad("Canlı site bağlantısı https:// ile başlamalı."));
    }
    let Some(plan_md) = plan_md else {
        return Err(bad("plan.md dosyası gerekli."));
    };
    // Resolve the deployed site now if there is one. Usually there isn't yet — students
    // submit the repo before they deploy — and that's fine: both columns stay null and the
    // admin rescan picks it up later. Never fails the submission.
    // Leaving live_checked_at null either way: if this didn't resolve, the background
    // resolver should retry within a tick rather than wait out the backoff.
    let (live, embeddable) =
        match crate::admin::resolve_live_url(&app.http, &repo_url, &live_url).await {
            crate::admin::LiveLookup::Found(u, e) => (Some(u), Some(e)),
            _ => (None, None),
        };
    sqlx::query("insert into submissions_exposure_academy (task_id, user_id, repo_url, plan_md, live_url, live_embeddable) values ($1,$2,$3,$4,$5,$6)")
        .bind(task_id).bind(user.id).bind(&repo_url).bind(&plan_md).bind(&live).bind(embeddable)
        .execute(&app.pool).await.unwrap();
    Ok(Redirect::to("/board"))
}

pub async fn worker_pending(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    // claim atomically: pending -> reviewing
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "update submissions_exposure_academy set status = 'reviewing'
         where id in (select id from submissions_exposure_academy where status = 'pending' order by created_at limit 5)
         returning id, repo_url, (select title from tasks_exposure_academy where tasks_exposure_academy.id = submissions_exposure_academy.task_id)")
        .fetch_all(&app.pool).await.unwrap();
    Ok(Json(serde_json::json!(
        rows.iter()
            .map(|(id, repo, task)| {
                serde_json::json!({"id": id, "repo_url": repo, "task_title": task})
            })
            .collect::<Vec<_>>()
    )))
}

#[derive(Deserialize)]
pub struct WorkerResult {
    id: Uuid,
    status: String,
    feedback: Option<String>,
    demo_video_url: Option<String>,
}

pub async fn worker_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<WorkerResult>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.status != "passed" && r.status != "failed" {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query("update submissions_exposure_academy set status = $2, feedback = $3, demo_video_url = $4 where id = $1")
        .bind(r.id).bind(&r.status).bind(&r.feedback).bind(&r.demo_video_url)
        .execute(&app.pool).await.unwrap();
    Ok(StatusCode::NO_CONTENT)
}
