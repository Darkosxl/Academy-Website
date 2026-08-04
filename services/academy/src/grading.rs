//! Görev Puanlama — the grading queue. Split out of `/admin` for the same reason
//! Takım formasyonu was (see teams.rs): the Gönderimler table there was an unpaginated,
//! unfilterable dump of every submission ever made, on a page that already rendered ten
//! other panels. Grading is queue work — "what is waiting on me", "only Advanced", "only
//! this student" — and none of those questions could be asked.
//!
//! Two sources, one queue. Görev Panosu submissions and Beginner Track projects live in
//! different tables with different keys, but they are the same job to an admin, so they
//! are flattened into `GradeRow` by a `union all` and reviewed by the same handler.
//!
//! Everything autosaves (static/grading.js). The mutation handlers answer `204` to a
//! background save and a redirect back to the *same filtered view* to a plain form post,
//! so the page works identically with JS off.

use axum::Form;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::admin::{probe_embeddable, valid_http_url};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};

/// Raw filter state, before validation. `ogrenci` is a String rather than a `Uuid` on
/// purpose: the Öğrenci select's "Hepsi" option submits `ogrenci=`, and an empty value
/// fails to deserialize into `Option<Uuid>` — it would 400 the whole page.
///
/// The mutation forms repeat these four fields inline rather than `#[serde(flatten)]`-ing
/// this struct: `serde_urlencoded` (what axum's `Form`/`Query` are built on) cannot
/// deserialize a flattened struct at all, and it fails at runtime, not at compile time.
#[derive(Deserialize, Default)]
pub struct GradingQ {
    durum: Option<String>,
    seviye: Option<String>,
    ogrenci: Option<String>,
    tum: Option<String>,
}

impl Filters {
    fn parse(q: &GradingQ) -> Self {
        Filters {
            // Absent means first visit, and the queue is the useful default. Only an
            // explicit `durum=hepsi` opens the whole archive.
            durum: q
                .durum
                .as_deref()
                .filter(|d| DURUM_FILTERS.iter().any(|(k, _)| k == d))
                .unwrap_or(DURUM_DEFAULT)
                .to_string(),
            // Garbage in a hand-edited URL is ignored rather than an error — same as
            // what /videos does with `?level=`.
            seviye: q
                .seviye
                .as_deref()
                .filter(|l| html::LEVELS.iter().any(|(k, _)| k == l))
                .map(str::to_string),
            ogrenci: q.ogrenci.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
            tum: q.tum.as_deref() == Some("1"),
        }
    }
}

pub async fn grading_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<GradingQ>,
) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    let f = Filters::parse(&q);
    let rows = fetch_rows(&app, &f).await;
    // The task's Tanım drives the review prompt but isn't worth widening the union for —
    // matched in memory instead, the way /admin already did it.
    let tasks = sqlx::query_as::<_, Task>(
        "select id, title, description, level, example_url, example_embeddable
         from tasks_exposure_academy order by level, position",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let members = sqlx::query_as::<_, MemberRow>(
        "select id, display_name, email, nickname, is_admin, hidden_from_leaderboard
         from users_exposure_academy order by is_admin desc, lower(coalesce(nickname, display_name))")
        .fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::grading(&user, &rows, &tasks, &members, &f)))
}

/// Every row matching everything *except* `durum`. The status filter is applied by the
/// caller in Rust so the four chip counts can be taken off this same list: a count has to
/// reflect the other active filters ("3 Bekleyen **among Advanced**"), and doing it here
/// means one query instead of five. The old /admin fetched every submission unfiltered on
/// every load, so this is strictly less work than what it replaces.
async fn fetch_rows(app: &App, f: &Filters) -> Vec<GradeRow> {
    sqlx::query_as::<_, GradeRow>(
        "with q as (
           select 'board'::text as kind,
                  s.id::text as key,
                  s.user_id,
                  s.task_id,
                  t.title as task_title,
                  t.level as task_level,
                  s.repo_url,
                  s.live_url,
                  s.plan_md,
                  s.status,
                  s.feedback,
                  s.points_override,
                  s.created_at,
                  row_number() over (partition by s.user_id, s.task_id
                                     order by s.created_at desc, s.id) as attempt,
                  count(*) over (partition by s.user_id, s.task_id) as attempts
           from submissions_exposure_academy s
           join tasks_exposure_academy t on t.id = s.task_id
           union all
           -- Beginner Track has no level and cannot pile up (its PK is (user, project)),
           -- so it is Beginner by definition and always attempt 1 of 1. project_key rides
           -- in task_title and the renderer maps it — SQL has no business knowing titles.
           select 'beginner'::text,
                  b.user_id::text || ':' || b.project_key,
                  b.user_id,
                  null::uuid,
                  b.project_key,
                  'PRESEED'::text,
                  b.repo_url,
                  b.vercel_url,
                  null::text,
                  b.status,
                  b.feedback,
                  b.points_override,
                  b.updated_at,
                  1::bigint,
                  1::bigint
           from beginner_submissions_exposure_academy b
         )
         -- user_id and attempt do the filtering above but are never read in Rust, so
         -- they stay out of the projection
         select q.kind, q.key, u.display_name, u.email, q.task_id, q.task_title,
                q.task_level, q.repo_url, q.live_url, q.plan_md, q.status, q.feedback,
                q.points_override, q.created_at, q.attempts
         from q join users_exposure_academy u on u.id = q.user_id
         where ($1::text is null or q.task_level = $1)
           and ($2::uuid is null or q.user_id = $2)
           and ($3::bool or q.attempt = 1)
         order by q.created_at desc",
    )
    .bind(f.seviye.as_deref())
    .bind(f.ogrenci)
    .bind(f.tum)
    .fetch_all(&app.pool)
    .await
    .unwrap()
}

/// A save landed. Background saves (grading.js, including its unload beacon) get an empty
/// 204 — the page has already applied the change locally and must not be navigated or
/// re-rendered mid-typing. A plain form post with JS off gets the redirect, back to the
/// same filtered view so the filters survive the round trip.
fn saved(headers: &HeaderMap, f: &Filters) -> Response {
    if headers.contains_key("x-autosave") {
        StatusCode::NO_CONTENT.into_response()
    } else {
        Redirect::to(&f.url()).into_response()
    }
}

fn bad(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, msg.to_string()).into_response()
}

/// The filter state is round-tripped through hidden fields rather than by echoing a raw
/// query string, and is re-validated here before it is ever put in a `Location` header.
#[derive(Deserialize)]
pub struct ReviewForm {
    kind: String,
    key: String,
    status: String,
    #[serde(default)]
    feedback: String,
    /// The Puan box. Blank is the normal case and means "score it by level".
    #[serde(default)]
    points: String,
    #[serde(default)]
    durum: Option<String>,
    #[serde(default)]
    seviye: Option<String>,
    #[serde(default)]
    ogrenci: Option<String>,
    #[serde(default)]
    tum: Option<String>,
}

impl ReviewForm {
    fn filters(&self) -> Filters {
        Filters::parse(&GradingQ {
            durum: self.durum.clone(),
            seviye: self.seviye.clone(),
            ogrenci: self.ogrenci.clone(),
            tum: self.tum.clone(),
        })
    }
}

pub async fn grading_review(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<ReviewForm>,
) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let f = form.filters();
    // /admin used to leave this to the DB CHECK, which surfaced a typo'd status as a bare
    // 400 with no explanation. Autosave makes that worse — the row would just show a red
    // indicator — so say what was wrong.
    if !is_grade_status(&form.status) {
        return Err(bad("Geçersiz durum."));
    }
    // Blank clears any previous override and puts the row back on the level default;
    // anything that isn't a non-negative number is a typo, so reject rather than
    // silently scoring the project at some other value.
    let points: Option<i32> = match form.points.trim() {
        "" => None,
        s => Some(
            s.parse::<i32>()
                .ok()
                .filter(|p| *p >= 0)
                .ok_or_else(|| bad("Puan 0 veya daha büyük bir tam sayı olmalı."))?,
        ),
    };
    let feedback = form.feedback.trim();

    let done = match form.kind.as_str() {
        "board" => {
            let id = Uuid::parse_str(&form.key).map_err(|_| bad("Geçersiz gönderim."))?;
            sqlx::query(
                "update submissions_exposure_academy
                 set status = $2, feedback = nullif($3,''), points_override = $4
                 where id = $1",
            )
            .bind(id)
            .bind(&form.status)
            .bind(feedback)
            .bind(points)
            .execute(&app.pool)
            .await
        }
        "beginner" => {
            let (user_id, project_key) = split_beginner_key(&form.key)?;
            sqlx::query(
                "update beginner_submissions_exposure_academy
                 set status = $3, feedback = nullif($4,''), points_override = $5
                 where user_id = $1 and project_key = $2",
            )
            .bind(user_id)
            .bind(project_key)
            .bind(&form.status)
            .bind(feedback)
            .bind(points)
            .execute(&app.pool)
            .await
        }
        _ => return Err(bad("Geçersiz gönderim türü.")),
    };
    done.map_err(|_| bad("Kaydedilemedi."))?;
    Ok(saved(&headers, &f))
}

#[derive(Deserialize)]
pub struct LiveForm {
    key: String,
    live_url: String,
    #[serde(default)]
    durum: Option<String>,
    #[serde(default)]
    seviye: Option<String>,
    #[serde(default)]
    ogrenci: Option<String>,
    #[serde(default)]
    tum: Option<String>,
}

/// The admin's manual override, and the escape hatch for a student on a custom domain that
/// `is_deploy_host` won't auto-accept. Blank clears it. Deliberately skips the host
/// allowlist — an admin typing a URL is the same trust level as the task example URLs.
///
/// Board rows only: a Beginner Track row's site is the student's own `vercel_url`, which
/// the admin reads but does not own, so those cells render as a plain link.
pub async fn grading_live(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<LiveForm>,
) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let f = Filters::parse(&GradingQ {
        durum: form.durum.clone(),
        seviye: form.seviye.clone(),
        ogrenci: form.ogrenci.clone(),
        tum: form.tum.clone(),
    });
    let id = Uuid::parse_str(&form.key).map_err(|_| bad("Geçersiz gönderim."))?;
    let url = form.live_url.trim();
    if !url.is_empty() && !valid_http_url(url) {
        return Err(bad("Canlı site URL'i http:// veya https:// ile başlamalı."));
    }
    // re-probe so the iframe/screenshot choice stays honest for whatever was just typed
    let embeddable = if url.is_empty() {
        None
    } else {
        probe_embeddable(&app.http, url).await
    };
    sqlx::query(
        "update submissions_exposure_academy
         set live_url = nullif($2,''), live_embeddable = $3 where id = $1",
    )
    .bind(id)
    .bind(url)
    .bind(embeddable)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Kaydedilemedi."))?;
    Ok(saved(&headers, &f))
}

/// One .txt holding a review prompt per row currently on screen, so a whole grading round
/// can be pasted into an agent in one go. It follows the page's filters rather than
/// hardcoding "pending + reviewing" the way /admin's version did: whatever the queue is
/// showing is what you were about to grade.
pub async fn grading_prompts_txt(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<GradingQ>,
) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let f = Filters::parse(&q);
    let rows = fetch_rows(&app, &f).await;
    let tasks = sqlx::query_as::<_, Task>(
        "select id, title, description, level, example_url, example_embeddable
         from tasks_exposure_academy order by level, position",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let shown: Vec<&GradeRow> = rows
        .iter()
        .filter(|r| durum_matches(&f.durum, &r.status))
        .collect();
    let body = if shown.is_empty() {
        "Bu filtrede gönderim yok.\n".to_string()
    } else {
        shown
            .iter()
            .map(|r| {
                format!(
                    "=== {name} — {title} — {date} ===\n{prompt}\n",
                    name = r.display_name,
                    title = html::grade_row_title(r),
                    date = r.created_at.format("%d.%m.%Y"),
                    prompt = review_prompt(&r.repo_url, &html::grade_row_goal(r, &tasks)),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let filename = format!("prompts-{}.txt", chrono::Utc::now().format("%Y-%m-%d"));
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        body,
    )
        .into_response())
}

/// `{user_id}:{project_key}` — beginner rows have a composite PK, so the review form
/// carries both halves in the one opaque `key` field board rows use for their uuid.
fn split_beginner_key(key: &str) -> Result<(Uuid, &str), Response> {
    let (id, project_key) = key
        .split_once(':')
        .ok_or_else(|| bad("Geçersiz gönderim."))?;
    let id = Uuid::parse_str(id).map_err(|_| bad("Geçersiz gönderim."))?;
    if !html::BEGINNER_PROJECTS
        .iter()
        .any(|(k, ..)| *k == project_key)
    {
        return Err(bad("Proje bulunamadı."));
    }
    Ok((id, project_key))
}
