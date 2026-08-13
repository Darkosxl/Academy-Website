//! Yönetici paneli: videos, tasks, students, invite code — and the example-project
//! screenshot cache behind /preview/{id}.
//!
//! Grading moved to Görev Puanlama (grading.rs); `valid_http_url` and `probe_embeddable`
//! are still shared with it.

use crate::html;
use crate::model::*;
use crate::monopoly::latest_tournament;
use crate::{App, auth::*, random_token};
use axum::{
    Form,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

pub async fn admin_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    let stats = sqlx::query_as::<_, StatRow>(
        "select u.display_name, v.title as video_title, w.seconds_watched, w.max_position, w.duration, w.updated_at
         from watch_progress_exposure_academy w join users_exposure_academy u on u.id = w.user_id join videos_exposure_academy v on v.id = w.video_id
         order by w.updated_at desc limit 200")
        .fetch_all(&app.pool).await.unwrap();
    let videos = sqlx::query_as::<_, Video>(
        "select id, youtube_id, title, level from videos_exposure_academy order by level, position",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let members = sqlx::query_as::<_, MemberRow>(
        "select id, display_name, email, nickname, is_admin, hidden_from_leaderboard
         from users_exposure_academy order by is_admin desc, lower(coalesce(nickname, display_name))")
        .fetch_all(&app.pool).await.unwrap();
    let invite_code = invite_code(&app).await;
    // Harness team membership lives on /admin/takimlar now (teams.rs). What's left here
    // is the stuck-run escape hatch, which is run management, not formation.
    let harness = HarnessAdmin {
        active_runs: sqlx::query_as(
            "select r.id, t.name as team_name, r.stage, r.created_at
             from harness_runs_exposure_academy r
             join harness_teams_exposure_academy t on t.id = r.team_id
             where r.stage not in ('done','partial','failed','infra_failed','cancelled')
             order by r.created_at",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap(),
        // `unwrap_or_default`, unlike every `unwrap` above it: this panel is a debugging aid,
        // and it must not take the whole admin page down on an instance where migration 012
        // hasn't landed yet.
        rejected: sqlx::query_as(
            "select j.raw_input, j.reason, u.display_name, t.name as team_name, j.created_at
             from harness_rejected_submissions_exposure_academy j
             left join users_exposure_academy u on u.id = j.user_id
             left join harness_teams_exposure_academy t on t.id = j.team_id
             order by j.created_at desc limit 50",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default(),
    };
    // AI Monopoly keeps mutable submissions beside one immutable active/latest tournament.
    let monopoly = MonopolyAdmin {
        teams: sqlx::query_as(
            "select id, name from monopoly_teams_exposure_academy order by lower(name)",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap(),
        members: sqlx::query_as(
            "select tm.team_id, tm.user_id, u.display_name,
                    (u.nickname is not null and not u.hidden_from_leaderboard) as public
             from monopoly_team_members_exposure_academy tm
             join users_exposure_academy u on u.id = tm.user_id
             order by lower(u.display_name)",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap(),
        submissions: sqlx::query_as(
            "select id, team_id, generation, repo_url, agent_path, status,
                    commit_sha, repo_size_bytes, validation_log
             from monopoly_submissions_exposure_academy order by updated_at desc",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap(),
        tournament: latest_tournament(&app).await,
        workers: sqlx::query_as(
            "select worker_id, kind, session_name, status, ram_bytes, effective_vcpus,
                    machine_shape, preflight_reason, current_game_id, last_seen_at
             from monopoly_workers_exposure_academy order by worker_id",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default(),
    };
    let schedule_images = sqlx::query_as::<_, ScheduleImage>(
        "select track, content_type, uploaded_at, length(image)::bigint as bytes
         from schedule_image_exposure_academy",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    // metadata only — the admin grid links to each file, it doesn't embed any
    let consent_docs = sqlx::query_as::<_, ConsentDoc>(
        "select id, user_id, kind, filename, length(file)::bigint as bytes, uploaded_at
         from consent_docs_exposure_academy order by kind, uploaded_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    let consent_locks = crate::consent::consent_locks(&app).await;
    let consent_urls = crate::consent::consent_urls(&app).await;
    Ok(Html(html::admin(
        &user,
        &stats,
        &videos,
        &tasks,
        &members,
        &invite_code,
        &app.base_url,
        &harness,
        &monopoly,
        &schedule_images,
        &crate::portal::load_venues(&app).await,
        &consent_docs,
        &consent_locks,
        &consent_urls,
    )))
}

// ---- Beginner Track gönderimleri ----

#[derive(Deserialize)]
pub struct BeginnerAdminQ {
    /// A `BEGINNER_PROJECTS` key. Absent = the project list, present = that project's
    /// student table. One route, two views, so the back link is just the same URL.
    proje: Option<String>,
}

/// Roster names with no matching account, so the page can say so instead of just
/// rendering a short table. `have` is the set of display names actually in the database.
fn roster_gaps(have: &[String]) -> Vec<&'static str> {
    BEGINNER_ROSTER
        .iter()
        .filter(|n| {
            let key = roster_key(n);
            !have.iter().any(|h| roster_key(h) == key)
        })
        .copied()
        .collect()
}

/// Admin-only read of the Beginner Track submissions. No editing: students own their
/// own links, this page only has to make them clickable and show who is missing.
///
/// Scoped to `BEGINNER_ROSTER`. Filtering happens in Rust rather than in SQL because the
/// match is on the normalized name (`roster_key`), and because the same pass produces the
/// list of roster names that matched nothing — which the page shows as a warning.
pub async fn admin_beginner_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<BeginnerAdminQ>,
) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    // An unknown ?proje= falls back to the list rather than 404ing — a stale bookmark
    // from a renamed project key should still land somewhere useful.
    let project = q
        .proje
        .as_deref()
        .filter(|k| html::BEGINNER_PROJECTS.iter().any(|(pk, ..)| pk == k));
    let Some(key) = project else {
        // Both queries come back unfiltered and get narrowed to the roster below. The
        // whole academy is a few dozen rows, so this is cheaper than it looks and keeps
        // the roster match in one place.
        let enrolled: Vec<String> = sqlx::query_scalar(
            "select display_name from users_exposure_academy where not is_admin",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default();
        let subs: Vec<(String, String)> = sqlx::query_as(
            "select u.display_name, s.project_key
             from beginner_submissions_exposure_academy s
             join users_exposure_academy u on u.id = s.user_id
             where not u.is_admin",
        )
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default();
        let counts: Vec<BeginnerProjectCount> = html::BEGINNER_PROJECTS
            .iter()
            .map(|(pk, ..)| BeginnerProjectCount {
                project_key: (*pk).to_string(),
                submitted: subs
                    .iter()
                    .filter(|(name, k)| k == pk && in_beginner_roster(name))
                    .count() as i64,
            })
            .collect();
        let on_track = enrolled.iter().filter(|n| in_beginner_roster(n)).count() as i64;
        return Ok(Html(html::admin_beginner_list(
            &user,
            &counts,
            on_track,
            &roster_gaps(&enrolled),
        )));
    };
    // Left join from the students, not from the submissions: a student who hasn't handed
    // this one in is a row with empty cells, which is the thing the admin is looking for.
    // Real names — this is the admin side, the nickname is only a public-facing handle.
    let all = sqlx::query_as::<_, BeginnerStudentRow>(
        "select u.display_name, s.repo_url, s.vercel_url, s.updated_at
         from users_exposure_academy u
         left join beginner_submissions_exposure_academy s
           on s.user_id = u.id and s.project_key = $1
         where not u.is_admin
         order by s.updated_at is null, lower(u.display_name)",
    )
    .bind(key)
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    let names: Vec<String> = all.iter().map(|r| r.display_name.clone()).collect();
    let rows: Vec<BeginnerStudentRow> = all
        .into_iter()
        .filter(|r| in_beginner_roster(&r.display_name))
        .collect();
    Ok(Html(html::admin_beginner_project(
        &user,
        key,
        &rows,
        &roster_gaps(&names),
    )))
}

// ---- haftalık program ----

pub async fn admin_schedule(
    State(app): State<App>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let mut track: Option<&'static str> = None;
    let mut image: Option<Vec<u8>> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Form okunamadı."))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "track" => {
                track = field
                    .text()
                    .await
                    .ok()
                    .map(|t| valid_schedule_track(Some(&t)))
            }
            "image" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| bad("Görsel okunamadı — dosya çok büyük olabilir."))?;
                if !bytes.is_empty() {
                    image = Some(bytes.to_vec());
                }
            }
            _ => {}
        }
    }

    let Some(track) = track else {
        return Err(bad("Grup seçilmedi."));
    };
    let Some(image) = image else {
        return Err(bad("Bir görsel seç."));
    };
    let Some(content_type) = crate::consent::sniff_image(&image) else {
        return Err(bad("Dosya PNG, JPEG, WebP veya GIF olmalı."));
    };

    sqlx::query(
        "insert into schedule_image_exposure_academy (track, image, content_type, uploaded_at)
         values ($1,$2,$3, now())
         on conflict (track) do update set
           image = $2, content_type = $3, uploaded_at = now()",
    )
    .bind(track)
    .bind(&image)
    .bind(content_type)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Kaydedilemedi."))?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct ScheduleDeleteForm {
    track: String,
}

pub async fn admin_schedule_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<ScheduleDeleteForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from schedule_image_exposure_academy where track = $1")
        .bind(valid_schedule_track(Some(&f.track)))
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

// ---- konum / adres ----

#[derive(Deserialize)]
pub struct VenueForm {
    week: u8,
    #[serde(default)]
    dates: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    maps_url: String,
    #[serde(default)]
    notes: String,
}

pub async fn admin_venue(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<VenueForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // a week we don't render would write settings rows nothing ever reads again
    if !VENUE_WEEKS.contains(&f.week) {
        return Err((StatusCode::BAD_REQUEST, "Geçersiz hafta.").into_response());
    }
    // Scheme-gate before it ever reaches an href: blank is fine (the button is then
    // simply not rendered), but a javascript:/data: value must never become a link.
    let maps_url = f.maps_url.trim();
    if !maps_url.is_empty() && !valid_http_url(maps_url) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Haritalar bağlantısı http:// veya https:// ile başlamalı.",
        )
            .into_response());
    }
    for (field, value) in [
        ("dates", f.dates.trim()),
        ("name", f.name.trim()),
        ("address", f.address.trim()),
        ("maps_url", maps_url),
        ("notes", f.notes.trim()),
    ] {
        sqlx::query(
            "insert into app_settings_exposure_academy (key, value, updated_at) values ($1,$2, now())
             on conflict (key) do update set value = $2, updated_at = now()",
        )
        .bind(venue_key(f.week, field))
        .bind(value)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/admin"))
}

/// Shared by both sections' team-create forms.
#[derive(Deserialize)]
pub struct NameForm {
    pub name: String,
}

/// Shared by both sections' member-assign forms.
#[derive(Deserialize)]
pub struct MemberForm {
    pub user_id: Uuid,
    pub team_id: Uuid,
}

pub fn parse_youtube_id(input: &str) -> String {
    // accepts raw ID, youtube.com/watch?v=ID, youtu.be/ID
    let s = input.trim();
    if let Some(i) = s.find("v=") {
        return s[i + 2..].split('&').next().unwrap_or("").to_string();
    }
    if let Some(i) = s.find("youtu.be/") {
        return s[i + 9..]
            .split(['?', '&'])
            .next()
            .unwrap_or("")
            .to_string();
    }
    s.rsplit('/').next().unwrap_or(s).to_string()
}

#[derive(Deserialize)]
pub struct VideoForm {
    title: String,
    youtube: String,
    level: String,
}

pub async fn admin_video(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<VideoForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("insert into videos_exposure_academy (youtube_id, title, level) values ($1,$2,$3)")
        .bind(parse_youtube_id(&f.youtube))
        .bind(&f.title)
        .bind(&f.level)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub fn valid_http_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://")
}

/// Reject obvious internal targets before the server-side fetch (SSRF hardening).
/// ponytail: literal host/IP denylist, no DNS resolution — a hostname that resolves
/// to an internal IP still slips through. Proportionate here: the caller is an admin
/// and only ever learns a boolean, never a response body. Upgrade to resolve-then-
/// check-the-IP if this fetch is ever made to return data.
pub fn is_internal_host(url: &str) -> bool {
    use std::net::IpAddr;
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return true;
    }; // unparseable → treat as blocked
    let Some(host) = parsed.host_str() else {
        return true;
    };
    let h = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h == "metadata.google.internal" {
        return true;
    }
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        Ok(IpAddr::V6(v6)) => {
            v6.is_loopback() || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00  // fc00::/7 unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
        Err(_) => false, // a real hostname — allowed (see the DNS-rebinding ceiling above)
    }
}

/// GET the URL and decide whether it permits iframe embedding.
///
/// `None` = we never got a usable response (blocked host, network error, timeout, non-2xx),
/// so the URL isn't worth storing at all. `Some(false)` = it answered but sends a framing
/// restriction, so it needs a screenshot instead of an iframe.
pub async fn probe_embeddable(client: &reqwest::Client, url: &str) -> Option<bool> {
    if is_internal_host(url) {
        return None;
    }
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(6))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let h = resp.headers();
    if let Some(xfo) = h.get("x-frame-options").and_then(|v| v.to_str().ok()) {
        let x = xfo.to_ascii_lowercase();
        if x.contains("deny") || x.contains("sameorigin") {
            return Some(false);
        }
    }
    if let Some(csp) = h
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
    {
        let c = csp.to_ascii_lowercase();
        // a frame-ancestors directive that isn't a blanket '*' means we're very likely blocked
        if c.contains("frame-ancestors") && !c.contains('*') {
            return Some(false);
        }
    }
    Some(true)
}

/// Conservative boolean form for the task example URLs: any framing restriction, or a
/// network error/timeout, counts as NOT embeddable (so we fall back to a screenshot,
/// which always renders something).
pub async fn check_embeddable(client: &reqwest::Client, url: &str) -> bool {
    probe_embeddable(client, url).await.unwrap_or(false)
}

/// Hosts we'll accept as an auto-resolved student site. Everything students actually use
/// is a free static/preview host, and restricting to them is what keeps `/preview/sub/{id}`
/// from becoming an open screenshot proxy for arbitrary pages on our own domain — the
/// live URL is student-controlled, unlike the admin-typed example URLs the preview cache
/// was built for. A custom domain isn't rejected forever: the admin override sets it by hand.
pub fn is_deploy_host(url: &str) -> bool {
    const HOSTS: [&str; 10] = [
        "vercel.app",
        "netlify.app",
        "github.io",
        "pages.dev",
        "web.app",
        "firebaseapp.com",
        "surge.sh",
        "onrender.com",
        "streamlit.app",
        "glitch.me",
    ];
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let h = host.to_ascii_lowercase();
    // match the registrable suffix only — ".vercel.app" so "vercel.app.evil.com" can't pass
    HOSTS
        .iter()
        .any(|s| h == *s || h.ends_with(&format!(".{s}")))
}

/// (owner, repo) out of a github.com URL. Accepts what students paste: trailing slash,
/// `.git`, a deeper `/tree/main` path. Strict about characters, because the result is
/// interpolated straight into an api.github.com URL.
pub fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let s = url.trim().trim_end_matches('/');
    let s = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
        .or_else(|| s.strip_prefix("https://www.github.com/"))?;
    let s = s.split(['?', '#']).next()?;
    let mut parts = s.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p != "."
            && p != ".."
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    (ok(owner) && ok(repo)).then(|| (owner.to_string(), repo.to_string()))
}

/// Outcome of one lookup. `Undeployed` and `Unavailable` are both "no URL", but they must
/// not be treated alike, and the line between them is *whose* fault it is:
///   - `Undeployed` — GitHub answered and there's no usable site (none listed, or the one
///     listed won't serve). A real answer about that row: stamp it and back off.
///   - `Unavailable` — we never got to ask GitHub (rate limit, network). Says nothing about
///     the row, so don't stamp it, and abort the pass since every other row will fail too.
/// Collapsing them either way is a bug: treat `Unavailable` as `Undeployed` and a rate-limited
/// pass writes off live repos as having no site; treat a dead site as `Unavailable` and that
/// one row aborts every pass forever, starving everything behind it in the queue.
pub enum LiveLookup {
    Found(String, bool),
    Undeployed,
    Unavailable,
}

/// Where a submission's live site is, and whether it can be iframed.
///
/// Priority: what the student pasted, else the repo's GitHub `homepage` (Vercel and Netlify
/// both write the deploy URL there on connect), else its GitHub Pages URL. Every candidate is
/// scheme-checked, SSRF-screened, host-allowlisted and probed before it's returned, so a
/// stored `live_url` is always one that actually answered.
pub async fn resolve_live_url(http: &reqwest::Client, repo_url: &str, pasted: &str) -> LiveLookup {
    let usable = |u: &str| valid_http_url(u) && !is_internal_host(u) && is_deploy_host(u);

    // the student's own answer wins — they know where they deployed it
    let pasted = pasted.trim();
    let mut candidate = usable(pasted).then(|| pasted.to_string());

    if candidate.is_none() {
        // a URL we can't even parse a repo out of is a real answer: it'll never resolve
        let Some((owner, repo)) = parse_github_repo(repo_url) else {
            return LiveLookup::Undeployed;
        };
        let resp = http
            .get(format!("https://api.github.com/repos/{owner}/{repo}"))
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await;
        let meta: serde_json::Value = match resp {
            // 404 is a real answer (repo gone/private); 403/429 is the rate limit and
            // every other status is a transport problem — neither tells us anything
            Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => return LiveLookup::Undeployed,
            Ok(r) if !r.status().is_success() => return LiveLookup::Unavailable,
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => return LiveLookup::Unavailable,
            },
            Err(_) => return LiveLookup::Unavailable,
        };
        candidate = meta["homepage"]
            .as_str()
            .map(str::trim)
            .filter(|h| usable(h))
            .map(str::to_string)
            .or_else(|| {
                meta["has_pages"].as_bool().unwrap_or(false).then(|| {
                    // Pages lowercases the host; the path segment keeps the repo's case
                    format!("https://{}.github.io/{repo}/", owner.to_ascii_lowercase())
                })
            });
    }

    let Some(url) = candidate else {
        return LiveLookup::Undeployed;
    };
    match probe_embeddable(http, &url).await {
        Some(embeddable) => LiveLookup::Found(url, embeddable),
        // A named site that won't serve (down, redirect loop, auth wall) is a real answer
        // about the site, so it backs off like any other miss. It is emphatically NOT
        // `Unavailable`: that aborts the whole pass, and one permanently broken site would
        // then block every row behind it forever — which is exactly what happened with a
        // student's Vercel app stuck in an /api/auth redirect loop.
        None => LiveLookup::Undeployed,
    }
}

/// Fetch a hero (above-the-fold) screenshot via Microlink, returning (bytes, content_type).
/// `embed=screenshot.url` makes Microlink respond with the image binary directly (one hop).
pub async fn fetch_screenshot(
    client: &reqwest::Client,
    key: &str,
    url: &str,
) -> Option<(Vec<u8>, String)> {
    let mut req = client
        .get("https://api.microlink.io/")
        .query(&[
            ("url", url),
            ("screenshot", "true"),
            ("meta", "false"),
            ("embed", "screenshot.url"),
            ("viewport.width", "1280"),
            ("viewport.height", "800"),
        ])
        .timeout(std::time::Duration::from_secs(25));
    if !key.is_empty() {
        req = req.header("x-api-key", key);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    if !ct.starts_with("image/") {
        return None;
    } // Microlink returns JSON error on failure
    let bytes = resp.bytes().await.ok()?;
    Some((bytes.to_vec(), ct))
}

#[derive(Deserialize)]
pub struct TaskForm {
    title: String,
    description: String,
    level: String,
    #[serde(default)]
    example_url: String,
}

pub async fn admin_task(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TaskForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let example = f.example_url.trim();
    if !example.is_empty() && !valid_http_url(example) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Örnek URL http:// veya https:// ile başlamalı.",
        )
            .into_response());
    }
    let embeddable = if example.is_empty() {
        None
    } else {
        Some(check_embeddable(&app.http, example).await)
    };
    // position = end of this level's order, so new tasks land last (hardest) until reordered
    sqlx::query("insert into tasks_exposure_academy (title, description, level, example_url, example_embeddable, position) values ($1,$2,$3, nullif($4,''), $5, (select coalesce(max(position),0)+1 from tasks_exposure_academy where level=$3))")
        .bind(&f.title).bind(&f.description).bind(&f.level).bind(example).bind(embeddable)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct TaskEditForm {
    id: Uuid,
    title: String,
    description: String,
}

pub async fn admin_task_edit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TaskEditForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let title = f.title.trim();
    let description = f.description.trim();
    if title.is_empty() || description.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Başlık ve tanım boş olamaz.").into_response());
    }
    sqlx::query("update tasks_exposure_academy set title = $2, description = $3 where id = $1")
        .bind(f.id)
        .bind(title)
        .bind(description)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct TaskExampleForm {
    id: Uuid,
    example_url: String,
}

pub async fn admin_task_example(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TaskExampleForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let url = f.example_url.trim();
    if !url.is_empty() && !valid_http_url(url) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Örnek URL http:// veya https:// ile başlamalı.",
        )
            .into_response());
    }
    // only update the URL — the live/image preview mode is the admin's manual choice
    // (set via /admin/task/preview) and is preserved across URL edits
    sqlx::query("update tasks_exposure_academy set example_url = nullif($2,'') where id = $1")
        .bind(f.id)
        .bind(url)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// Admin's manual per-task choice: live iframe preview vs cached screenshot image.
pub async fn admin_task_preview(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TaskPreviewForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let live = f.mode == "live";
    sqlx::query("update tasks_exposure_academy set example_embeddable = $2 where id = $1")
        .bind(f.id)
        .bind(live)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub fn image_response(bytes: Vec<u8>, ct: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, ct.to_owned()),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        bytes,
    )
        .into_response()
}

/// Fallback shown when there's no cached image yet and generation failed. Short
/// cache so the next view retries. Displays the URL's host, or a generic label.
pub fn placeholder_svg(url: &str) -> Response {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("");
    let label = if host.is_empty() {
        "önizleme yok".to_string()
    } else {
        html::esc(host)
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="800" viewBox="0 0 1280 800"><rect width="1280" height="800" fill="#18181b"/><text x="640" y="400" fill="#71717a" font-family="sans-serif" font-size="36" text-anchor="middle" dominant-baseline="middle">{label}</text></svg>"##,
    );
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml".to_string()),
            (header::CACHE_CONTROL, "public, max-age=300".to_string()),
        ],
        svg,
    )
        .into_response()
}

/// How long a cached screenshot stays fresh. Task example URLs never change, but student
/// sites are redeployed constantly — without an expiry the first shot of a half-built site
/// would be frozen on the gallery forever.
const SCREENSHOT_TTL: &str = "7 days";

/// Serve the cached hero screenshot for `url`, generating it on first request. Keyed by
/// URL, so two rows pointing at the same site share one image. Shared by both preview
/// routes — the only difference between them is which column the URL comes out of.
async fn cached_preview(app: &App, url: Option<String>) -> Response {
    let Some(url) = url.filter(|u| !u.is_empty()) else {
        return placeholder_svg("");
    };

    // cache hit?
    if let Ok(Some((img, ct))) = sqlx::query_as::<_, (Vec<u8>, String)>(&format!(
        "select image, content_type from screenshot_cache_exposure_academy
                  where url = $1 and fetched_at > now() - interval '{SCREENSHOT_TTL}'"
    ))
    .bind(&url)
    .fetch_optional(&app.pool)
    .await
    {
        return image_response(img, &ct);
    }
    // miss or stale -> fetch from Microlink, cache, serve. On failure serve a non-cached placeholder.
    match fetch_screenshot(&app.http, &app.microlink_key, &url).await {
        Some((bytes, ct)) => {
            let _ = sqlx::query(
                "insert into screenshot_cache_exposure_academy (url, image, content_type) values ($1,$2,$3)
                 on conflict (url) do update set image = $2, content_type = $3, fetched_at = now()")
                .bind(&url).bind(&bytes).bind(&ct).execute(&app.pool).await;
            image_response(bytes, &ct)
        }
        None => placeholder_svg(&url),
    }
}

/// A task's example project. Keyed by task id (not raw URL) so only admin-set URLs are
/// ever fetched — no open proxy. Public, no auth (it screenshots public sites).
pub async fn task_preview(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    cached_preview(
        &app,
        sqlx::query_scalar("select example_url from tasks_exposure_academy where id = $1")
            .bind(id)
            .fetch_optional(&app.pool)
            .await
            .ok()
            .flatten()
            .flatten(),
    )
    .await
}

/// A student's deployed site, for the gallery cards whose site blocks iframing. Same
/// id-keyed contract as `task_preview`, one table over; `live_url` is additionally
/// constrained to `is_deploy_host` at write time, so this stays closed to arbitrary pages.
pub async fn submission_preview(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    cached_preview(
        &app,
        sqlx::query_scalar("select live_url from submissions_exposure_academy where id = $1")
            .bind(id)
            .fetch_optional(&app.pool)
            .await
            .ok()
            .flatten()
            .flatten(),
    )
    .await
}

/// How often the resolver wakes up, and how many rows it'll take per pass. 10 rows/10 min
/// with the 6-hour retry floor below keeps the worst case well inside GitHub's 60/hour
/// unauthenticated budget even when every row is a repo that will never resolve.
const RESOLVE_TICK: std::time::Duration = std::time::Duration::from_secs(600);
const RESOLVE_BATCH: i64 = 10;
/// Don't re-ask about the same unresolved row more often than this.
const RESOLVE_RETRY_AFTER: &str = "6 hours";

/// One pass of the background resolver: fill in the live URL for submissions that don't
/// have one yet. Students routinely submit the repo days before they deploy, so a single
/// pass at submit time isn't enough — this is what eventually catches them, and what
/// backfilled every row that predates the column.
///
/// Sequential on purpose: each row costs a GitHub API call plus a GET of the site, and
/// `live_checked_at` is stamped whether or not it resolved, so a repo that never deploys
/// backs off instead of being retried every tick.
pub async fn resolve_pending(app: &App) {
    // one row per (student, task): a duplicate resubmission of the same repo would
    // otherwise burn a second API call for the same answer
    let Ok(rows) = sqlx::query_as::<_, (Uuid, String)>(
        &format!("select distinct on (user_id, task_id) id, repo_url
                  from submissions_exposure_academy
                  where live_url is null
                    and (live_checked_at is null or live_checked_at < now() - interval '{RESOLVE_RETRY_AFTER}')
                  order by user_id, task_id, created_at desc
                  limit {RESOLVE_BATCH}"))
        .fetch_all(&app.pool).await else { return };
    for (id, repo_url) in rows {
        let (url, embeddable) = match resolve_live_url(&app.http, &repo_url, "").await {
            LiveLookup::Found(u, e) => (Some(u), Some(e)),
            // a real "nothing there" — stamp it so this row backs off
            LiveLookup::Undeployed => (None, None),
            // couldn't ask. Leave live_checked_at alone so the row stays due, and stop the
            // pass: if GitHub is rate-limiting us, the rest of the batch would burn the same
            // way and mark every one of them as undeployed.
            LiveLookup::Unavailable => return,
        };
        let _ = sqlx::query(
            "update submissions_exposure_academy
             set live_url = coalesce($2, live_url), live_embeddable = coalesce($3, live_embeddable),
                 live_checked_at = now()
             where id = $1 and live_url is null",
        )
        .bind(id)
        .bind(&url)
        .bind(embeddable)
        .execute(&app.pool)
        .await;
    }
}

/// Fire-and-forget loop started at boot. No admin button, no cron: a student deploying
/// three days after they submitted is picked up on the next tick either way.
pub fn spawn_resolver(app: App) {
    tokio::spawn(async move {
        loop {
            resolve_pending(&app).await;
            tokio::time::sleep(RESOLVE_TICK).await;
        }
    });
}

#[derive(Deserialize)]
pub struct IdForm {
    pub id: Uuid,
}

pub async fn admin_task_level(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdLevelForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // level is checked by the DB constraint; an invalid value just 400s. Moving to a
    // new level appends the task at the end of that level's order.
    sqlx::query(
        "update tasks_exposure_academy set level = $2,
           position = (select coalesce(max(position),0)+1 from tasks_exposure_academy where level = $2)
         where id = $1")
        .bind(f.id).bind(&f.level)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

// swap a task's position with its neighbour in the same level (ponytail: adjacent-swap
// assumes unique positions per level, which the backfill + insert-position guarantee).
pub async fn admin_task_move(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TaskMoveForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let Some((level, position)) = sqlx::query_as::<_, (String, i32)>(
        "select level, position from tasks_exposure_academy where id = $1",
    )
    .bind(f.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    else {
        return Ok(Redirect::to("/admin"));
    };
    let neighbor = if f.dir == "up" {
        "select id, position from tasks_exposure_academy where level = $1 and position < $2 order by position desc limit 1"
    } else {
        "select id, position from tasks_exposure_academy where level = $1 and position > $2 order by position asc limit 1"
    };
    if let Some((nid, npos)) = sqlx::query_as::<_, (Uuid, i32)>(neighbor)
        .bind(&level)
        .bind(position)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    {
        sqlx::query("update tasks_exposure_academy set position = $2 where id = $1")
            .bind(f.id)
            .bind(npos)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
        sqlx::query("update tasks_exposure_academy set position = $2 where id = $1")
            .bind(nid)
            .bind(position)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_task_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades to submissions (FK) — points earned from this task go with it
    sqlx::query("delete from tasks_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_video_level(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdLevelForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("update videos_exposure_academy set level = $2 where id = $1")
        .bind(f.id)
        .bind(&f.level)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_video_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades to watch progress (FK) — points earned from this video go with it.
    // NOTE: seed_videos re-inserts any ID still listed in videos.dat on next restart.
    sqlx::query("delete from videos_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct UserForm {
    email: String,
    display_name: String,
    /// Unchecked checkboxes are simply absent from the POST body, hence the Option.
    #[serde(default)]
    hidden: Option<String>,
}

pub async fn admin_user(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<UserForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let email = f.email.trim().to_lowercase();
    // Adding the row here with `hidden` pre-set is how an intern account gets created
    // before she ever opens the invite link: join_post's `on conflict (email) do nothing`
    // leaves this row alone, so she is never visible for even one page load.
    sqlx::query(
        "insert into users_exposure_academy (email, display_name, hidden_from_leaderboard)
         values ($1,$2,$3) on conflict (email) do nothing",
    )
    .bind(&email)
    .bind(&f.display_name)
    .bind(f.hidden.is_some())
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// Flip a student in or out of the published standings (and the board's teammate chips).
/// Admins are excluded on both sides already, so the flag is only meaningful for students.
pub async fn admin_user_hidden(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<UserHiddenForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("update users_exposure_academy set hidden_from_leaderboard = $2 where id = $1")
        .bind(f.id)
        .bind(f.hidden)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_user_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    let me = require_admin(current_user(&app, &headers).await)?;
    // guard rails: never let an admin delete themselves or another admin from here.
    // Deleting a student cascades to their sessions, watch progress, and submissions (FK).
    if f.id == me.id {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query("delete from users_exposure_academy where id = $1 and is_admin = false")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_rotate_invite(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let new_code = &random_token()[..8];
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ('invite_code', $1, now())
         on conflict (key) do update set value = $1, updated_at = now()")
        .bind(new_code).execute(&app.pool).await.unwrap();
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct TaskPreviewForm {
    id: Uuid,
    mode: String,
}

// ---- example-project screenshot preview ----

#[derive(Deserialize)]
pub struct IdLevelForm {
    id: Uuid,
    level: String,
}

#[derive(Deserialize)]
pub struct TaskMoveForm {
    id: Uuid,
    dir: String,
}

#[derive(Deserialize)]
pub struct UserHiddenForm {
    id: Uuid,
    hidden: bool,
}

// ---- worker API (Phase 3 pipeline, see README) ----

// ---- worker API: Agentic Harness runs ----
//
// Same shape as the board pipeline above: the runner polls `pending` to claim a run,
// reports each stage transition as it goes (so the student's stepper moves), and posts
// the final scores or the failure. Transitions are forward-only and guarded on the
// expected current stage — a stale or duplicate report gets a 409 and must be dropped.

// ---- worker API: AI Monopoly ----
//
// Same discipline as the harness API above: the runner claims work, reports forward-only
// transitions, and a stale or duplicate report gets a 409 it must drop. The one addition
// is that this API owns the money — the runner reports what the judge decided, and the
// server alone turns that into ledger changes.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_hosts_blocked_public_allowed() {
        for u in [
            "http://localhost/x",
            "http://127.0.0.1",
            "http://169.254.169.254/latest/meta-data",
            "https://10.0.0.5",
            "http://192.168.1.1",
            "http://172.16.0.1",
            "http://[::1]/",
            "http://metadata.google.internal",
            "not a url",
        ] {
            assert!(is_internal_host(u), "{u} should be blocked");
        }
        for u in [
            "https://example.com",
            "https://ornek.vercel.app",
            "http://172.15.0.1",
            "http://172.32.0.1",
        ] {
            assert!(!is_internal_host(u), "{u} should be allowed");
        }
    }

    // The (owner, repo) pair is interpolated into an api.github.com URL, so a slash or a
    // ".." escaping the parser would address a different endpoint entirely.
    #[test]
    fn github_repo_parsed_or_rejected() {
        for (url, want) in [
            ("https://github.com/EnzeCbe/enzeceb", ("EnzeCbe", "enzeceb")),
            ("https://github.com/ecer19/website/", ("ecer19", "website")),
            ("https://github.com/a/b.git", ("a", "b")),
            (
                "https://github.com/alinebidal10-afk/portfolio/tree/main",
                ("alinebidal10-afk", "portfolio"),
            ),
            ("https://www.github.com/a/b?tab=readme", ("a", "b")),
        ] {
            let got = parse_github_repo(url).unwrap_or_else(|| panic!("{url} should parse"));
            assert_eq!((got.0.as_str(), got.1.as_str()), want, "{url}");
        }
        for u in [
            "https://github.com/",
            "https://github.com/onlyowner",
            "https://gitlab.com/a/b",
            "https://github.com/a/../../rate_limit",
            "https://github.com/a/b c",
            "not a url",
        ] {
            assert!(parse_github_repo(u).is_none(), "{u} should be rejected");
        }
    }

    // The allowlist is what keeps /preview/sub/{id} from screenshotting arbitrary pages;
    // suffix matching has to be on a dot boundary or an attacker registers vercel.app.evil.com.
    #[test]
    fn deploy_hosts_allowed_others_rejected() {
        for u in [
            "https://enzeceb.vercel.app",
            "https://alinebidal10-afk.github.io/portfolio/",
            "https://x.netlify.app",
            "https://y.pages.dev",
            "https://vercel.app",
        ] {
            assert!(is_deploy_host(u), "{u} should be allowed");
        }
        for u in [
            "https://vercel.app.evil.com",
            "https://notvercel.app",
            "https://evil.com",
            "https://github.com/a/b",
            "not a url",
        ] {
            assert!(!is_deploy_host(u), "{u} should be rejected");
        }
    }

    /// Hits the real GitHub API and the real student sites, so it's #[ignore]d — `cargo test`
    /// stays offline. Run it after touching the resolver:
    ///   cargo test resolves_real_student_sites -- --ignored --nocapture
    /// The User-Agent is the part worth guarding: api.github.com answers 403 without one.
    #[tokio::test]
    #[ignore]
    async fn resolves_real_student_sites() {
        let http = reqwest::Client::builder()
            .user_agent("exposure-academy")
            .build()
            .unwrap();
        // three repos whose Vercel URL is in `homepage`, and one Pages-only repo
        for repo in [
            "https://github.com/emirkaanozdemr/personal-website",
            "https://github.com/EnzeCbe/enzeceb",
            "https://github.com/ecer19/website",
            "https://github.com/alinebidal10-afk/portfolio",
        ] {
            match resolve_live_url(&http, repo, "").await {
                LiveLookup::Found(url, _) => {
                    println!("{repo} -> {url}");
                    assert!(
                        is_deploy_host(&url),
                        "{repo} resolved off-allowlist to {url}"
                    );
                }
                LiveLookup::Undeployed => panic!("{repo} should resolve"),
                LiveLookup::Unavailable => panic!("GitHub unreachable/rate-limited — rerun later"),
            }
        }
        // a repo that doesn't exist is a real answer (404), not a retryable failure
        assert!(matches!(
            resolve_live_url(&http, "https://github.com/ecer19/no-such-repo-xyz", "").await,
            LiveLookup::Undeployed
        ));
    }
}
