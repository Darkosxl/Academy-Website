//! The read-only student portal: hub, videos, watch tracking, demos, leaderboard.

use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LevelQ {
    level: Option<String>,
}

#[derive(Deserialize)]
pub struct LangQ {
    lang: Option<String>,
}

pub async fn demos(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<LangQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let lang = if q.lang.as_deref() == Some("en") {
        "en"
    } else {
        "tr"
    };
    Ok(Html(html::demos(&user, lang)))
}

/// Ana Sayfa. No content of its own — three doors (videolar / görevler / puan tablosu),
/// each carrying the one number that tells the student where they stand.
pub async fn home(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let videos_total: i64 = sqlx::query_scalar("select count(*) from videos_exposure_academy")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    let videos_done: i64 = sqlx::query_scalar(
        "select count(*) from watch_progress_exposure_academy
         where user_id = $1 and duration > 0 and max_position >= duration * 0.9",
    )
    .bind(user.id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    // "Açık" = bu öğrencinin henüz geçmiş bir gönderimi olmayan görev.
    let open_tasks: i64 = sqlx::query_scalar(
        "select count(*) from tasks_exposure_academy t
         where not exists (select 1 from submissions_exposure_academy s
                           where s.task_id = t.id and s.user_id = $1 and s.status = 'passed')",
    )
    .bind(user.id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let all = leader_rows(&app).await;
    // Points come from the full list so a hidden (intern) account still sees its own
    // total; the rank comes from the visible standings, where it has no place at all.
    let points = all
        .iter()
        .find(|r| r.id == user.id)
        .map(|r| r.points())
        .unwrap_or(0);
    let rows: Vec<LeaderRow> = all.into_iter().filter(|r| !r.hidden).collect();
    let ranks = html::dense_ranks(&rows);
    let rank = rows.iter().position(|r| r.id == user.id).map(|i| ranks[i]);
    Ok(Html(html::home(
        &user,
        videos_done,
        videos_total,
        open_tasks,
        points,
        rank,
    )))
}

pub async fn video_grid(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<LevelQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let level = q
        .level
        .as_deref()
        .filter(|l| html::LEVELS.iter().any(|(k, _)| k == l));
    let videos = sqlx::query_as::<_, VideoWithProgress>(
        "select v.id, v.youtube_id, v.title, v.level,
                coalesce(w.max_position, 0) as max_position, coalesce(w.duration, 0) as duration
         from videos_exposure_academy v
         left join watch_progress_exposure_academy w on w.video_id = v.id and w.user_id = $1
         -- videos are presented as one Beginner-Intermediate tier, so either of those
         -- filters shows the whole PRESEED+SEED set; Advanced (SERIES_A) stays separate.
         where ($2::text is null
             or ($2 in ('PRESEED','SEED') and v.level in ('PRESEED','SEED'))
             or v.level = $2)
         order by v.level, v.position, v.created_at",
    )
    .bind(user.id)
    .bind(level)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    Ok(Html(html::video_grid(&user, &videos, level)))
}

pub async fn watch(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let video = sqlx::query_as::<_, Video>(
        "select id, youtube_id, title, level from videos_exposure_academy where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let playlist = sqlx::query_as::<_, VideoWithProgress>(
        "select v.id, v.youtube_id, v.title, v.level,
                coalesce(w.max_position, 0) as max_position, coalesce(w.duration, 0) as duration
         from videos_exposure_academy v
         left join watch_progress_exposure_academy w on w.video_id = v.id and w.user_id = $1
         where v.level = $2 order by v.position, v.created_at",
    )
    .bind(user.id)
    .bind(&video.level)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let resume_at = playlist
        .iter()
        .find(|v| v.id == video.id)
        .map(|v| {
            if v.duration > 0.0 && v.max_position < v.duration - 10.0 {
                v.max_position as f64
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);
    Ok(Html(html::watch(&user, &video, &playlist, resume_at)))
}

#[derive(Deserialize)]
pub struct ProgressReq {
    video_id: Uuid,
    position: f32,
    duration: f32,
    delta: f32,
}

pub async fn progress(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<ProgressReq>,
) -> Result<StatusCode, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let delta = r.delta.clamp(0.0, 30.0); // heartbeat is 10s; anything bigger is a client lying
    sqlx::query(
        "insert into watch_progress_exposure_academy (user_id, video_id, seconds_watched, max_position, duration, updated_at)
         values ($1,$2,$3,$4,$5, now())
         on conflict (user_id, video_id) do update set
           seconds_watched = watch_progress_exposure_academy.seconds_watched + $3,
           max_position = greatest(watch_progress_exposure_academy.max_position, $4),
           duration = $5, updated_at = now()")
        .bind(user.id).bind(r.video_id).bind(delta).bind(r.position.max(0.0)).bind(r.duration.max(0.0))
        .execute(&app.pool).await.unwrap();
    Ok(StatusCode::NO_CONTENT)
}

pub async fn leaderboard(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // Hidden (intern) accounts never reach a rendered standings list — not even their
    // own, so nothing about them can leak through a shared screen or a screenshot.
    let rows: Vec<LeaderRow> = leader_rows(&app)
        .await
        .into_iter()
        .filter(|r| !r.hidden)
        .collect();
    Ok(Html(html::leaderboard(&user, &rows)))
}

/// The standings, ordered. Shared by /leaderboard and the Ana Sayfa summary card so
/// the two can never disagree about a student's points or place.
///
/// Includes hidden (intern) accounts, flagged as `hidden` — they are scored like anyone
/// else so they can see their own total, and every caller drops them before rendering a
/// list. Ranks are therefore computed over the visible rows only: a hidden account never
/// pushes a student down a place.
///
/// A video counts once it is ≥90% watched — same threshold the grid calls "Tamamlanmış".
/// A project counts once per task, and only when the submission passed, so resubmits
/// of the same task don't stack points.
///
/// A passed project is worth its level default (PTS_PROJECT_L*) unless the admin typed
/// a number into the Puan box, which stores a `points_override` on that submission row.
/// Where a student has several passed submissions for one task, the newest one is the
/// one that counts — so re-scoring means editing (or re-passing) the latest row.
pub async fn leader_rows(app: &App) -> Vec<LeaderRow> {
    sqlx::query_as::<_, LeaderRow>(
        "select u.id, u.display_name, u.nickname, u.hidden_from_leaderboard as hidden,
                coalesce(w.videos, 0) as videos,
                coalesce(p.projects, 0) as projects,
                coalesce(p.project_points, 0) as project_points
         from users_exposure_academy u
         left join (select user_id, count(*) as videos
                    from watch_progress_exposure_academy
                    where duration > 0 and max_position >= duration * 0.9
                    group by user_id) w on w.user_id = u.id
         -- one row per (user, passed task) first so a task counts once — the newest
         -- passed submission wins, and it carries the override the admin typed.
         -- sum() over bigint yields numeric, so cast back for the i64 decode.
         left join (select d.user_id,
                           count(*) as projects,
                           sum(coalesce(d.points_override,
                                        case t.level when 'PRESEED' then $2 when 'SEED' then $3
                                                     when 'SERIES_A' then $4 else 0 end))::bigint as project_points
                    from (select distinct on (user_id, task_id) user_id, task_id, points_override
                          from submissions_exposure_academy where status = 'passed'
                          order by user_id, task_id, created_at desc) d
                    join tasks_exposure_academy t on t.id = d.task_id
                    group by d.user_id) p on p.user_id = u.id
         -- nickname is null until onboarding is done: it is no longer what the board
         -- shows, but it still marks a finished onboarding, so keep gating on it
         where not u.is_admin and u.nickname is not null
         order by coalesce(w.videos,0) * $1 + coalesce(p.project_points,0) desc, u.created_at")
        .bind(PTS_VIDEO)
        .bind(PTS_PROJECT_L1).bind(PTS_PROJECT_L2).bind(PTS_PROJECT_L3)
        .fetch_all(&app.pool).await.unwrap()
}
