//! The read-only student portal: hub, videos, watch tracking, demos, leaderboard.

use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

// ---- haftalık program ----

#[derive(Deserialize)]
pub struct TrackQ {
    track: Option<String>,
}

/// Metadata for a track's uploaded schedule, without the bytes — those only ever leave
/// via `schedule_image`, so the page render never pulls a few MB out of the database.
async fn schedule_meta(app: &App, track: &str) -> Option<ScheduleImage> {
    sqlx::query_as::<_, ScheduleImage>(
        "select track, content_type, uploaded_at, length(image)::bigint as bytes
         from schedule_image_exposure_academy where track = $1",
    )
    .bind(track)
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten()
}

pub async fn schedule(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<TrackQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let track = valid_schedule_track(q.track.as_deref());
    let img = schedule_meta(&app, track).await;
    let venues = load_venues(&app).await;
    Ok(Html(html::schedule(&user, track, img.as_ref(), &venues)))
}

/// The uploaded screenshot itself. Members-only (it sits inside the authed routes), so
/// the schedule isn't a public URL the way /preview/{id} is. Cached hard but privately:
/// the src carries `?v=<upload time>`, so a replacement is a different URL and no stale
/// image can survive it.
pub async fn schedule_image(
    State(app): State<App>,
    headers: HeaderMap,
    Path(track): Path<String>,
) -> Result<Response, Response> {
    require_onboarded(current_user(&app, &headers).await)?;
    let track = valid_schedule_track(Some(&track));
    let row: Option<(Vec<u8>, String)> = sqlx::query_as(
        "select image, content_type from schedule_image_exposure_academy where track = $1",
    )
    .bind(track)
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten();
    let Some((bytes, ct)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    Ok((
        [
            (header::CONTENT_TYPE, ct),
            (header::CACHE_CONTROL, "private, max-age=86400".to_string()),
            // the type is one we sniffed on upload; forbid the browser guessing another
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    )
        .into_response())
}

// ---- konum / adres ----

/// One `Venue` per entry in VENUE_WEEKS, in order, always the full set. A week with no
/// rows yet comes back with empty strings rather than being absent, so the pages can
/// name it as "not announced" instead of leaving a hole where a week should be.
pub async fn load_venues(app: &App) -> Vec<Venue> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "select key, value from app_settings_exposure_academy where key like 'venue%'",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    let get = |week: u8, field: &str| {
        let k = venue_key(week, field);
        rows.iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    VENUE_WEEKS
        .iter()
        .map(|&week| Venue {
            week,
            dates: get(week, "dates"),
            name: get(week, "name"),
            address: get(week, "address"),
            maps_url: get(week, "maps_url"),
            notes: get(week, "notes"),
        })
        .collect()
}

pub async fn location(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let venues = load_venues(&app).await;
    Ok(Html(html::location(&user, &venues)))
}

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

/// The stats shown on both Ana Sayfa and Online — kept in one place so the two hubs
/// can never disagree about a student's progress.
struct StudentProgress {
    videos_done: i64,
    videos_total: i64,
    open_tasks: i64,
    points: i64,
    rank: Option<i64>,
}

async fn student_progress(app: &App, user: &User) -> StudentProgress {
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
    let all = leader_rows(app).await;
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
    StudentProgress {
        videos_done,
        videos_total,
        open_tasks,
        points,
        rank,
    }
}

/// Ana Sayfa. No content of its own — three doors (videolar / görevler / puan tablosu),
/// each carrying the one number that tells the student where they stand.
pub async fn home(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let p = student_progress(&app, &user).await;
    // Consent forms count only while they're open for upload: a form whose document
    // isn't ready yet (locked) is not something the student can be behind on.
    let locks = crate::consent::consent_locks(&app).await;
    let open: Vec<&'static str> = locks.iter().filter(|(_, l)| !l).map(|(k, _)| *k).collect();
    let docs = crate::consent::user_consent_docs(&app, user.id).await;
    let consent_done = open
        .iter()
        .filter(|k| docs.iter().any(|d| d.kind == **k))
        .count();
    Ok(Html(html::home(
        &user,
        p.videos_done,
        p.videos_total,
        p.open_tasks,
        p.points,
        p.rank,
        consent_done,
        open.len(),
    )))
}

/// Online — videos, tasks, demos and the leaderboard, grouped behind one sidebar entry.
pub async fn online_hub(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let p = student_progress(&app, &user).await;
    Ok(Html(html::online(
        &user,
        p.videos_done,
        p.videos_total,
        p.open_tasks,
        p.points,
        p.rank,
    )))
}

/// Beginner Track — the seven fixed projects, each with the student's saved links (if any).
pub async fn beginner_track_hub(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let projects_done: i64 = sqlx::query_scalar(
        "select count(*) from beginner_submissions_exposure_academy where user_id = $1",
    )
    .bind(user.id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let chatbot_level = crate::chatbot_challenge::current_level(&app, user.id).await;
    Ok(Html(html::beginner_track(
        &user,
        projects_done as usize,
        chatbot_level,
    )))
}

pub async fn beginner_projects_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let subs = sqlx::query_as::<_, BeginnerSubmission>(
        "select project_key, repo_url, vercel_url
         from beginner_submissions_exposure_academy where user_id = $1",
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    Ok(Html(html::beginner_projects(&user, &subs)))
}

#[derive(Deserialize)]
pub struct BeginnerSubmitForm {
    project_key: String,
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    vercel_url: String,
}

/// Save (or replace) a student's GitHub + Vercel links for one Beginner Track project.
/// Upserts the pair, same shape as `board_submit`'s validation but without the plan.md
/// requirement.
///
/// Changing either link sends the row back to the grading queue: status to 'pending',
/// feedback and points cleared. Board submissions get this for free because each one
/// INSERTs a fresh pending row, but these upsert in place — without the reset a student
/// could be awarded 100 points and then swap the repo for anything at all. The reset is
/// conditional on a link actually differing so re-saving the same pair never silently
/// discards a grade.
pub async fn beginner_track_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<BeginnerSubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();
    if !html::BEGINNER_PROJECTS
        .iter()
        .any(|(k, ..)| *k == f.project_key)
    {
        return Err(bad("Proje bulunamadı."));
    }
    let repo_url = f.repo_url.trim().to_string();
    let vercel_url = f.vercel_url.trim().to_string();
    if !repo_url.starts_with("https://github.com/") {
        return Err(bad("Repo bağlantısı https://github.com/ ile başlamalı."));
    }
    if !vercel_url.starts_with("https://") && !vercel_url.starts_with("http://") {
        return Err(bad("Vercel bağlantısı https:// ile başlamalı."));
    }
    sqlx::query(
        // aliased so the do-update can read the row's *current* values — Postgres needs
        // the alias declared on the INSERT target to reference it in ON CONFLICT
        "insert into beginner_submissions_exposure_academy as b
           (user_id, project_key, repo_url, vercel_url, updated_at)
         values ($1,$2,$3,$4, now())
         on conflict (user_id, project_key) do update set
           repo_url = excluded.repo_url, vercel_url = excluded.vercel_url, updated_at = now(),
           status = case when b.repo_url is distinct from excluded.repo_url
                           or b.vercel_url is distinct from excluded.vercel_url
                         then 'pending' else b.status end,
           feedback = case when b.repo_url is distinct from excluded.repo_url
                             or b.vercel_url is distinct from excluded.vercel_url
                           then null else b.feedback end,
           points_override = case when b.repo_url is distinct from excluded.repo_url
                                    or b.vercel_url is distinct from excluded.vercel_url
                                  then null else b.points_override end",
    )
    .bind(user.id)
    .bind(&f.project_key)
    .bind(&repo_url)
    .bind(&vercel_url)
    .execute(&app.pool)
    .await
    .unwrap();
    Ok(Redirect::to("/beginner-track/projects"))
}

// ---- Agent Lab (Beginner Track) ----
//
// A sandbox pair of challenges for driving a browser agent around the portal. Every write
// below targets agent_lab_*_exposure_academy and nothing else: the lab never reads or
// writes users_exposure_academy, beginner_submissions_exposure_academy or
// submissions_exposure_academy, so an agent loose in here cannot alter a real profile,
// a real submission, or anybody's score.

async fn agent_lab_profile_row(app: &App, user_id: Uuid) -> Option<AgentLabProfile> {
    sqlx::query_as::<_, AgentLabProfile>(
        "select full_name, school, grade, interest, agent_goal, updated_at
         from agent_lab_profiles_exposure_academy where user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

async fn agent_lab_submission_row(app: &App, user_id: Uuid) -> Option<AgentLabSubmission> {
    sqlx::query_as::<_, AgentLabSubmission>(
        "select project_key, repo_url, demo_url, correct, updated_at
         from agent_lab_submissions_exposure_academy where user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

/// The lab hub — the two challenges as cards.
pub async fn agent_lab_hub(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    Ok(Html(html::agent_lab(&user)))
}

/// `/beginner/agent-lab` — the shorthand path, kept working so a link written either way
/// lands in the same place. The canonical URL is the one under `/beginner-track/`.
/// Temporary rather than permanent on purpose: a 308 sticks in browser caches long after
/// the route it points at could be moved.
pub async fn agent_lab_alias() -> Redirect {
    Redirect::to(html::AGENT_LAB_PATH)
}

pub async fn agent_lab_profile_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let saved = agent_lab_profile_row(&app, user.id).await;
    Ok(Html(html::agent_lab_profile(&user, saved.as_ref(), None)))
}

#[derive(Deserialize)]
pub struct AgentLabProfileForm {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    school: String,
    #[serde(default)]
    grade: String,
    #[serde(default)]
    interest: String,
    #[serde(default)]
    agent_goal: String,
}

/// Challenge 1's save. Validation is deliberately visible rather than lenient: an agent
/// that leaves a field blank or invents a grade should get a message on the page it can
/// read and act on, which is half of what the challenge is teaching.
pub async fn agent_lab_profile_save(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<AgentLabProfileForm>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let full_name = f.full_name.trim();
    let school = f.school.trim();
    let grade = f.grade.trim();
    let interest = f.interest.trim();
    let agent_goal = f.agent_goal.trim();
    let err = if full_name.is_empty()
        || school.is_empty()
        || interest.is_empty()
        || agent_goal.is_empty()
    {
        Some("Beş alanın da dolu olması gerekiyor.")
    } else if !GRADES.contains(&grade) {
        Some("Sınıf alanından listedeki seçeneklerden birini seç.")
    } else {
        None
    };
    if let Some(msg) = err {
        let saved = agent_lab_profile_row(&app, user.id).await;
        return Ok(Html(html::agent_lab_profile(&user, saved.as_ref(), Some(msg))).into_response());
    }
    sqlx::query(
        "insert into agent_lab_profiles_exposure_academy
           (user_id, full_name, school, grade, interest, agent_goal, updated_at)
         values ($1,$2,$3,$4,$5,$6, now())
         on conflict (user_id) do update set
           full_name = excluded.full_name, school = excluded.school, grade = excluded.grade,
           interest = excluded.interest, agent_goal = excluded.agent_goal, updated_at = now()",
    )
    .bind(user.id)
    .bind(full_name)
    .bind(school)
    .bind(grade)
    .bind(interest)
    .bind(agent_goal)
    .execute(&app.pool)
    .await
    .unwrap();
    Ok(Redirect::to(&format!("{}/student-profile", html::AGENT_LAB_PATH)).into_response())
}

pub async fn agent_lab_submission_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let saved = agent_lab_submission_row(&app, user.id).await;
    Ok(Html(html::agent_lab_submission(
        &user,
        saved.as_ref(),
        None,
    )))
}

#[derive(Deserialize)]
pub struct AgentLabSubmitForm {
    #[serde(default)]
    project_key: String,
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    demo_url: String,
}

/// Challenge 2's save. A pick off the list is stored whether or not it is the right one —
/// the page then tells the student which project their agent chose, which is far more
/// useful for debugging a run than a bare "başarısız". Only a key that isn't on the list
/// at all is rejected outright.
pub async fn agent_lab_submission_save(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<AgentLabSubmitForm>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let project_key = f.project_key.trim();
    let repo_url = f.repo_url.trim();
    let demo_url = f.demo_url.trim();
    let err = if !html::AGENT_LAB_PROJECTS
        .iter()
        .any(|(k, ..)| *k == project_key)
    {
        Some("Listedeki projelerden birini seç.")
    } else if !repo_url.starts_with("https://github.com/") {
        Some("Repo bağlantısı https://github.com/ ile başlamalı.")
    } else if !demo_url.starts_with("https://") {
        Some("Demo bağlantısı https:// ile başlamalı.")
    } else {
        None
    };
    if let Some(msg) = err {
        let saved = agent_lab_submission_row(&app, user.id).await;
        return Ok(
            Html(html::agent_lab_submission(&user, saved.as_ref(), Some(msg))).into_response(),
        );
    }
    sqlx::query(
        "insert into agent_lab_submissions_exposure_academy
           (user_id, project_key, repo_url, demo_url, correct, updated_at)
         values ($1,$2,$3,$4,$5, now())
         on conflict (user_id) do update set
           project_key = excluded.project_key, repo_url = excluded.repo_url,
           demo_url = excluded.demo_url, correct = excluded.correct, updated_at = now()",
    )
    .bind(user.id)
    .bind(project_key)
    .bind(repo_url)
    .bind(demo_url)
    .bind(project_key == html::AGENT_LAB_TARGET)
    .execute(&app.pool)
    .await
    .unwrap();
    Ok(Redirect::to(&format!("{}/project-submission", html::AGENT_LAB_PATH)).into_response())
}

/// Challenge 3's landing page: which of the ten postings this student has submitted, which
/// is also the progress count — a row exists only for a completed application.
pub async fn agent_lab_jobs_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let done = agent_lab_done_jobs(&app, user.id).await;
    Ok(Html(html::agent_lab_jobs(&user, &done)))
}

/// The job keys this student has submitted. Scoped to their own user_id, like every other
/// read in the lab, so one student's sandbox is never visible from another's session.
async fn agent_lab_done_jobs(app: &App, user_id: Uuid) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "select job_key from agent_lab_job_applications_exposure_academy where user_id = $1",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .unwrap()
}

/// One application, with whatever was submitted last time filled back in.
pub async fn agent_lab_job_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(job_key): Path<String>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let job = html::agent_lab_job(&job_key).ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let saved = agent_lab_job_row(&app, user.id, &job_key).await;
    let answers = saved.as_ref().and_then(|r| parse_answers(&r.answers));
    Ok(Html(html::agent_lab_job_form(
        &user,
        job,
        answers.as_ref(),
        saved.as_ref().map(|r| r.updated_at),
        None,
    )))
}

async fn agent_lab_job_row(
    app: &App,
    user_id: Uuid,
    job_key: &str,
) -> Option<AgentLabJobApplication> {
    sqlx::query_as::<_, AgentLabJobApplication>(
        "select answers, updated_at
         from agent_lab_job_applications_exposure_academy
         where user_id = $1 and job_key = $2",
    )
    .bind(user_id)
    .bind(job_key)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

/// Stored answers back into a map. A row that somehow isn't a JSON object renders as an
/// empty form rather than taking the page down — the student can just fill it in again.
fn parse_answers(raw: &str) -> Option<html::Answers> {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(m)) => Some(m),
        _ => None,
    }
}

/// Submit (or re-submit) one application. Upserts on (user_id, job_key), so correcting an
/// application replaces it and the progress count never double-counts a job.
pub async fn agent_lab_job_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(job_key): Path<String>,
    Form(raw): Form<std::collections::HashMap<String, String>>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let job = html::agent_lab_job(&job_key).ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let answers = match html::validate_job_application(job, &raw) {
        Ok(a) => a,
        // Re-render with the message and what they typed still in the fields, so an agent
        // can read the failure and fix it without starting the form over.
        Err(msg) => {
            let attempt = html::job_answers_from_raw(job, &raw);
            let already = agent_lab_job_row(&app, user.id, &job_key)
                .await
                .map(|r| r.updated_at);
            return Ok(Html(html::agent_lab_job_form(
                &user,
                job,
                Some(&attempt),
                already,
                Some(&msg),
            ))
            .into_response());
        }
    };
    let encoded = serde_json::Value::Object(answers).to_string();
    sqlx::query(
        "insert into agent_lab_job_applications_exposure_academy
           (user_id, job_key, answers, updated_at)
         values ($1,$2,$3, now())
         on conflict (user_id, job_key) do update set
           answers = excluded.answers, updated_at = now()",
    )
    .bind(user.id)
    .bind(&job_key)
    .bind(&encoded)
    .execute(&app.pool)
    .await
    .unwrap();
    Ok(Redirect::to(&format!(
        "{}/job-applications/{job_key}",
        html::AGENT_LAB_PATH
    ))
    .into_response())
}

#[derive(Deserialize)]
pub struct AgentLabResetForm {
    #[serde(default)]
    challenge: String,
}

/// Wipe one challenge's sandbox row so the run can be repeated from empty. Scoped to the
/// caller's own user_id, and only ever to the two lab tables.
pub async fn agent_lab_reset(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<AgentLabResetForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(table) = html::agent_lab_reset_table(&f.challenge) else {
        return Err((StatusCode::BAD_REQUEST, "Challenge bulunamadı.".to_string()).into_response());
    };
    // `table` is one of three literals returned by a closed match — never anything the
    // caller typed — so the format! cannot carry a value into the statement.
    sqlx::query(&format!("delete from {table} where user_id = $1"))
        .bind(user.id)
        .execute(&app.pool)
        .await
        .unwrap();
    Ok(Redirect::to(&format!(
        "{}/{}",
        html::AGENT_LAB_PATH,
        f.challenge
    )))
}

/// Advanced Track — Agentic Harness and AI Monopoly, grouped behind one sidebar entry.
pub async fn advanced_track_hub(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    Ok(Html(html::advanced_track(&user)))
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
///
/// Beginner Track projects score by exactly the same rules, graded from the same queue.
/// They have no level, so they take PTS_PROJECT_L1 as their default — Beginner is the
/// only thing they could be. Their PK is (user_id, project_key), so unlike board
/// submissions they cannot pile up and need no `distinct on`. Both sources are summed
/// into the same two output columns: `projects` stays the plain "X proje" count and
/// `project_points` stays the total, so nothing downstream has to know the difference.
pub async fn leader_rows(app: &App) -> Vec<LeaderRow> {
    sqlx::query_as::<_, LeaderRow>(
        "select u.id, u.display_name, u.nickname, u.hidden_from_leaderboard as hidden,
                coalesce(w.videos, 0) as videos,
                coalesce(p.projects, 0) + coalesce(b.bprojects, 0) as projects,
                coalesce(p.project_points, 0) + coalesce(b.bpoints, 0) as project_points
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
         left join (select user_id, count(*) as bprojects,
                           sum(coalesce(points_override, $2))::bigint as bpoints
                    from beginner_submissions_exposure_academy
                    where status = 'passed'
                    group by user_id) b on b.user_id = u.id
         -- nickname is null until onboarding is done: it is no longer what the board
         -- shows, but it still marks a finished onboarding, so keep gating on it
         where not u.is_admin and u.nickname is not null
         -- must stay in step with the two summed columns above, or the standings would
         -- sort by a total they don't print
         order by coalesce(w.videos,0) * $1
                  + coalesce(p.project_points,0) + coalesce(b.bpoints,0) desc, u.created_at")
        .bind(PTS_VIDEO)
        .bind(PTS_PROJECT_L1).bind(PTS_PROJECT_L2).bind(PTS_PROJECT_L3)
        .fetch_all(&app.pool).await.unwrap()
}
