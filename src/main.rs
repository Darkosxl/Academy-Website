mod html;
mod model;

use axum::{
    Form, Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use rand::RngCore;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use model::*;

#[derive(Clone)]
struct App {
    pool: PgPool,
    worker_token: String,
    http: reqwest::Client,
    resend_key: String,
    mail_from: String,
    base_url: String,
    /// Optional Microlink API key for screenshot generation; blank = free tier.
    microlink_key: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing (.env)");
    // NOTE: use Supabase's SESSION pooler (port 5432), not transaction pooler (6543) —
    // transaction mode can't do prepared statements, which sqlx relies on.
    let pool = PgPool::connect(&db_url).await.expect("db connect failed");

    // idempotent schema + seed admin
    sqlx::raw_sql(include_str!("../migrations/001_init.sql")).execute(&pool).await.expect("migration failed");
    seed_admin(&pool).await;
    seed_invite_code(&pool).await;
    seed_videos(&pool).await;
    // opportunistic cleanup of stale magic links and sessions, no scheduler needed
    let _ = sqlx::query("delete from magic_links_exposure_academy where expires_at < now() - interval '1 day'")
        .execute(&pool).await;
    let _ = sqlx::query("delete from sessions_exposure_academy where expires_at < now()")
        .execute(&pool).await;

    let app = App {
        pool,
        worker_token: std::env::var("WORKER_TOKEN").unwrap_or_default(),
        http: reqwest::Client::new(),
        resend_key: std::env::var("RESEND_API_KEY").expect("RESEND_API_KEY missing (.env)"),
        mail_from: std::env::var("MAIL_FROM").expect("MAIL_FROM missing (.env)"),
        base_url: std::env::var("APP_BASE_URL").expect("APP_BASE_URL missing (.env)"),
        microlink_key: std::env::var("MICROLINK_API_KEY").unwrap_or_default(),
    };

    let router = Router::new()
        .route("/", get(landing))
        .route("/login", get(login_page).post(login_post))
        .route("/magic/{token}", get(magic_consume))
        .route("/join", get(join_page).post(join_post))
        .route("/join/{code}", get(join_page_code))
        .route("/logout", post(logout))
        .route("/profile", get(profile_page).post(profile_post))
        .route("/app", get(home))
        .route("/schedule", get(schedule))
        .route("/schedule/image/{track}", get(schedule_image))
        .route("/location", get(location))
        // consent forms: a page of scans/photos is far past axum's 2 MB default
        .route("/documents", get(documents))
        .route("/documents/upload", post(documents_upload)
            .layer(DefaultBodyLimit::max(CONSENT_UPLOAD_MAX_MB * 1024 * 1024)))
        .route("/documents/delete", post(documents_delete))
        .route("/documents/file/{id}", get(document_file))
        .route("/videos", get(video_grid))
        .route("/agentic-harness", get(agentic_harness))
        .route("/ai-monopoly", get(ai_monopoly))
        .route("/demos", get(demos))
        .route("/watch/{id}", get(watch))
        .route("/api/progress", post(progress))
        .route("/leaderboard", get(leaderboard))
        .route("/board", get(board))
        .route("/board/profiles", post(board_profiles))
        .route("/board/submit", post(board_submit).layer(DefaultBodyLimit::max(300 * 1024)))
        .route("/board/interest", post(board_interest))
        .route("/admin", get(admin_page))
        .route("/admin/video", post(admin_video))
        .route("/admin/video/level", post(admin_video_level))
        .route("/admin/video/delete", post(admin_video_delete))
        .route("/admin/task", post(admin_task))
        .route("/admin/task/edit", post(admin_task_edit))
        .route("/admin/task/example", post(admin_task_example))
        .route("/admin/task/preview", post(admin_task_preview))
        .route("/admin/task/level", post(admin_task_level))
        .route("/admin/task/move", post(admin_task_move))
        .route("/admin/task/delete", post(admin_task_delete))
        // a screenshot is far past axum's 2 MB default, so this route raises its own limit
        .route("/admin/schedule", post(admin_schedule)
            .layer(DefaultBodyLimit::max(html::SCHEDULE_IMAGE_MAX_MB * 1024 * 1024)))
        .route("/admin/schedule/delete", post(admin_schedule_delete))
        .route("/admin/venue", post(admin_venue))
        .route("/admin/documents.zip", get(admin_documents_zip))
        .route("/admin/documents/lock", post(admin_documents_lock))
        .route("/admin/user", post(admin_user))
        .route("/admin/user/delete", post(admin_user_delete))
        .route("/admin/user/hidden", post(admin_user_hidden))
        .route("/admin/review", post(admin_review))
        .route("/admin/prompts.txt", get(admin_prompts_txt))
        .route("/admin/invite", post(admin_rotate_invite))
        .route("/api/worker/pending", get(worker_pending))
        .route("/api/worker/result", post(worker_result))
        // rolling session refresh — applies to the routes above only; static assets
        // are mounted after the layer so they don't each cost a session write
        .layer(middleware::from_fn_with_state(app.clone(), rolling_session))
        // cached example-URL screenshots: public cacheable assets like /static, mounted
        // after the layer so they don't cost a session write and need no auth
        .route("/preview/{id}", get(task_preview))
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .with_state(app);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:3000".into());
    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

async fn seed_admin(pool: &PgPool) {
    let Ok(email) = std::env::var("ADMIN_EMAIL") else { return };
    let email = email.trim().to_lowercase();
    let exists: Option<(Uuid,)> = sqlx::query_as("select id from users_exposure_academy where email = $1")
        .bind(&email).fetch_optional(pool).await.unwrap();
    match exists {
        None => {
            sqlx::query("insert into users_exposure_academy (email, display_name, is_admin) values ($1,$2,true)")
                .bind(&email).bind(&email).execute(pool).await.unwrap();
            println!("admin '{email}' seeded");
        }
        Some(_) => {
            sqlx::query("update users_exposure_academy set is_admin = true where email = $1")
                .bind(&email).execute(pool).await.unwrap();
        }
    }
}

/// Lesson video youtube-IDs live hex-encoded in videos.dat (committed) so the raw
/// URLs aren't sitting in the repo as plaintext; video_links.md (the readable
/// source) is git-ignored. Line order in the decoded blob IS the playlist order:
/// positions 1..=8 are Beginner (PRESEED), 9..=15 Intermediate (SEED). Insert-once by
/// youtube_id, so title/level/position edits made later in the admin panel survive
/// restarts. Regenerate videos.dat after editing video_links.md:
///   python3 -c "import sys;d={};[d.__setitem__(int(o),u.strip().rsplit('/',1)[-1]) for l in open('video_links.md') if l.strip() for u,o in [l.rsplit(' - ',1)]];open('videos.dat','w').write('\n'.join(d[k] for k in sorted(d)).encode().hex())"
/// YouTube titles for the IDs in videos.dat, same order (fetched via oEmbed).
/// Keep in sync when regenerating videos.dat.
const VIDEO_TITLES: [&str; 15] = [
    "AI Academy! Tanışmaca",
    "AI Academy! Programlama Nedir?",
    "AI Academy! Programlamaya Giriş I",
    "AI Academy! Programlamaya Giriş II",
    "AI Academy! Programlamaya Giriş III",
    "AI Academy! Programlama IV",
    "AI Academy! Programlama V",
    "AI Academy! Yazılım Mühendisliği I",
    "AI Academy! Yazılım Mühendisliği II",
    "AI Academy! Git(hub)!",
    "AI Academy! Web Geliştirme I",
    "AI Academy! Web Geliştirme II",
    "AI Academy! Web Geliştirme III",
    "AI Academy! Yapay Zeka I",
    "AI Academy! Yapay Zeka II",
];

async fn seed_videos(pool: &PgPool) {
    let hex = include_str!("../videos.dat").trim();
    let bytes: Vec<u8> = (0..hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("videos.dat not valid hex"))
        .collect();
    let blob = String::from_utf8(bytes).expect("videos.dat not valid utf-8");
    for (i, yt) in blob.lines().filter(|l| !l.is_empty()).enumerate() {
        let pos = (i + 1) as i32;
        let level = if pos <= 8 { "PRESEED" } else { "SEED" };
        let title = VIDEO_TITLES.get(i).map(|t| t.to_string())
            .unwrap_or_else(|| format!("Ders {pos}"));
        sqlx::query(
            "insert into videos_exposure_academy (youtube_id, title, level, position)
             select $1,$2,$3,$4
             where not exists (select 1 from videos_exposure_academy where youtube_id = $1)")
            .bind(yt).bind(&title).bind(level).bind(pos)
            .execute(pool).await.unwrap();
        // Rows seeded before real titles existed still say "Ders N" — rename those
        // in place. Admin-edited titles don't match the default and are left alone.
        sqlx::query(
            "update videos_exposure_academy set title = $2
             where youtube_id = $1 and title = $3")
            .bind(yt).bind(&title).bind(format!("Ders {pos}"))
            .execute(pool).await.unwrap();
    }
}

async fn seed_invite_code(pool: &PgPool) {
    let Ok(code) = std::env::var("INVITE_CODE") else { return };
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ('invite_code', $1, now())
         on conflict (key) do update set value = $1, updated_at = now()")
        .bind(code.trim()).execute(pool).await.unwrap();
}

fn random_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

async fn send_magic_link_email(app: &App, to: &str, link: &str) {
    // Ensure a display name so clients don't show the bare address as sender.
    let from = if app.mail_from.contains('<') {
        app.mail_from.clone()
    } else {
        format!("Exposure Academy <{}>", app.mail_from)
    };
    // Email-client-safe HTML: table layout, inline styles, no external assets.
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="tr">
<body style="margin:0;padding:0;background-color:#FFFCF6;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#FFFCF6;padding:40px 16px;">
<tr><td align="center">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:440px;">
    <tr><td style="padding:0 4px 20px 4px;">
      <span style="font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:22px;font-weight:800;letter-spacing:-0.5px;color:#0D0D0D;">exposure</span>
      <span style="font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:9px;font-weight:700;letter-spacing:3px;color:#a1a1aa;text-transform:uppercase;">&nbsp;AI ACADEMY</span>
    </td></tr>
    <tr><td style="background-color:#ffffff;border:1px solid #e8e4da;border-radius:16px;padding:36px 32px;">
      <p style="margin:0 0 6px 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:22px;font-weight:800;letter-spacing:-0.5px;color:#0D0D0D;">Oturum aç</p>
      <p style="margin:0 0 26px 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:14px;line-height:1.6;color:#71717a;">Exposure Academy hesabına giriş yapmak için aşağıdaki butona tıkla.</p>
      <table role="presentation" cellpadding="0" cellspacing="0" width="100%"><tr><td align="center">
        <a href="{link}" style="display:block;background-color:#0339A6;color:#ffffff;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:14px;font-weight:700;letter-spacing:1px;text-transform:uppercase;text-decoration:none;padding:14px 24px;border-radius:12px;text-align:center;">Oturum a&ccedil; &rarr;</a>
      </td></tr></table>
      <p style="margin:26px 0 0 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:12px;line-height:1.6;color:#a1a1aa;">Bu bağlantı <strong style="color:#71717a;">15 dakika</strong> geçerlidir ve yalnızca bir kez kullanılabilir.</p>
      <p style="margin:8px 0 0 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:12px;line-height:1.6;color:#a1a1aa;">Buton çalışmıyorsa bu bağlantıyı tarayıcına yapıştır:<br><a href="{link}" style="color:#0339A6;word-break:break-all;">{link}</a></p>
    </td></tr>
    <tr><td style="padding:20px 4px 0 4px;">
      <p style="margin:0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:11px;line-height:1.6;color:#a1a1aa;">Bu e-postayı sen istemediysen görmezden gelebilirsin — hesabında hiçbir işlem yapılmaz.<br>&copy; Exposure Academy</p>
    </td></tr>
  </table>
</td></tr>
</table>
</body>
</html>"##
    );
    let body = serde_json::json!({
        "from": from,
        "to": [to],
        "subject": "Exposure Academy giriş bağlantın",
        "html": html,
    });
    if let Err(e) = app.http.post("https://api.resend.com/emails")
        .bearer_auth(&app.resend_key)
        .json(&body)
        .send().await
    {
        eprintln!("resend send failed: {e}");
    }
}

// ---- session helpers ----

/// Session lifetime. Kept in one place so the DB row's `expires_at`, the cookie's
/// Max-Age and the rolling refresh below can never drift apart.
const SESSION_DAYS: i64 = 30;
const SESSION_MAX_AGE: i64 = SESSION_DAYS * 24 * 60 * 60;
/// Refresh once the session drops below this — one extra write per user per day,
/// not one per request.
const SESSION_REFRESH_BELOW_DAYS: i64 = SESSION_DAYS - 1;

fn session_cookie(token: &str) -> String {
    format!("session={token}; HttpOnly; Secure; Path=/; Max-Age={SESSION_MAX_AGE}; SameSite=Lax")
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?
        .split(';').map(str::trim)
        .find_map(|c| c.strip_prefix("session=").map(String::from))
}

async fn current_user(app: &App, headers: &HeaderMap) -> Option<User> {
    let token = cookie_token(headers)?;
    sqlx::query_as::<_, User>(
        "select u.id, u.display_name, u.nickname, u.is_admin from sessions_exposure_academy s join users_exposure_academy u on u.id = s.user_id where s.token = $1 and s.expires_at > now()")
        .bind(token).fetch_optional(&app.pool).await.ok()?
}

/// insert a 30-day session row and build the matching Set-Cookie + redirect to /app
async fn issue_session(app: &App, uid: Uuid) -> Response {
    let session_token = random_token();
    sqlx::query("insert into sessions_exposure_academy (token, user_id, expires_at) values ($1,$2, now() + make_interval(days => $3))")
        .bind(&session_token).bind(uid).bind(SESSION_DAYS as i32).execute(&app.pool).await.unwrap();
    (
        // cookie Max-Age mirrors the row's expires_at; the DB check is the one that counts
        [(header::SET_COOKIE, session_cookie(&session_token))],
        Redirect::to("/app"),
    ).into_response()
}

/// Rolling window: every request carrying a live session pushes its expiry back out
/// to the full 30 days, so an active user is never logged out mid-use — only 30 days
/// of *inactivity* ends the session.
///
/// Two things that matter here, both learned the hard way in the Next.js version:
/// the DB row and the browser cookie must be extended *together* (extending only the
/// row leaves the cookie to expire out from under a still-valid session), and the
/// refresh must run after the handler so /logout's delete wins — a deleted row
/// matches nothing below, so no Set-Cookie is appended and the logout sticks.
async fn rolling_session(State(app): State<App>, req: Request, next: Next) -> Response {
    let token = cookie_token(req.headers());
    let mut res = next.run(req).await;
    let Some(token) = token else { return res };

    let rolled: Option<(Uuid,)> = sqlx::query_as(
        "update sessions_exposure_academy set expires_at = now() + make_interval(days => $2)
         where token = $1 and expires_at > now() and expires_at < now() + make_interval(days => $3)
         returning user_id")
        .bind(&token)
        .bind(SESSION_DAYS as i32)
        .bind(SESSION_REFRESH_BELOW_DAYS as i32)
        .fetch_optional(&app.pool).await.ok().flatten();

    if rolled.is_some() {
        if let Ok(v) = HeaderValue::from_str(&session_cookie(&token)) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

fn require(user: Option<User>) -> Result<User, Response> {
    user.ok_or_else(|| Redirect::to("/login").into_response())
}

/// Same as `require`, plus: no nickname means onboarding never finished, so send them
/// to /profile to pick one. Used by every student page except /profile itself, which
/// would otherwise redirect to itself forever.
fn require_onboarded(user: Option<User>) -> Result<User, Response> {
    let u = require(user)?;
    // admins never appear on the leaderboard, so a nickname is optional for them —
    // gating them too would just lock you out of the portal after a fresh seed
    if u.nickname.is_none() && !u.is_admin {
        return Err(Redirect::to("/profile").into_response());
    }
    Ok(u)
}

fn require_admin(user: Option<User>) -> Result<User, Response> {
    match user {
        Some(u) if u.is_admin => Ok(u),
        Some(_) => Err(StatusCode::FORBIDDEN.into_response()),
        None => Err(Redirect::to("/login").into_response()),
    }
}

// ---- pages ----

async fn landing(State(app): State<App>, headers: HeaderMap) -> Response {
    // valid session cookie -> straight to the portal, skip the marketing page
    if current_user(&app, &headers).await.is_some() {
        return Redirect::to("/app").into_response();
    }
    Html(html::landing()).into_response()
}

async fn login_page(State(app): State<App>, headers: HeaderMap) -> Response {
    if current_user(&app, &headers).await.is_some() {
        return Redirect::to("/app").into_response();
    }
    Html(html::login(None)).into_response()
}

#[derive(Deserialize)]
struct LoginForm { email: String }

const CHECK_EMAIL_MSG: &str = "Eğer bu e-posta kayıtlıysa, giriş bağlantısı gönderildi.";

async fn login_post(State(app): State<App>, Form(f): Form<LoginForm>) -> Response {
    let email = f.email.trim().to_lowercase();
    let allowed: Option<(Uuid,)> = sqlx::query_as("select id from users_exposure_academy where email = $1")
        .bind(&email).fetch_optional(&app.pool).await.unwrap();
    if allowed.is_some() {
        send_login_link(&app, &email).await;
    }
    // same response whether or not the email is registered — avoids account enumeration
    Html(html::login(Some(CHECK_EMAIL_MSG))).into_response()
}

async fn magic_consume(State(app): State<App>, Path(token): Path<String>) -> Response {
    let row: Option<(String,)> = sqlx::query_as(
        "update magic_links_exposure_academy set used_at = now()
         where token = $1 and used_at is null and expires_at > now()
         returning email")
        .bind(&token).fetch_optional(&app.pool).await.unwrap();
    let Some((email,)) = row else {
        return Html(html::login(Some("Bağlantı geçersiz ya da süresi dolmuş, yeniden deneyin."))).into_response();
    };
    let user_id: Option<(Uuid,)> = sqlx::query_as("select id from users_exposure_academy where email = $1")
        .bind(&email).fetch_optional(&app.pool).await.unwrap();
    let Some((uid,)) = user_id else {
        return Html(html::login(Some("Hesap bulunamadı."))).into_response();
    };
    issue_session(&app, uid).await
}

async fn join_page() -> Html<String> {
    Html(html::join(&JoinForm::default(), false, None))
}

/// The link that goes in the WhatsApp group: /join/<invite code>. The code rides in
/// the path so students only fill in their own details; it is still validated on POST.
async fn join_page_code(Path(code): Path<String>) -> Html<String> {
    let f = JoinForm { code, ..Default::default() };
    Html(html::join(&f, true, None))
}

async fn invite_code(app: &App) -> String {
    sqlx::query_scalar("select value from app_settings_exposure_academy where key = 'invite_code'")
        .fetch_optional(&app.pool).await.unwrap().unwrap_or_default()
}

async fn join_post(State(app): State<App>, Form(f): Form<JoinForm>) -> Response {
    let locked = !f.code.trim().is_empty();
    let fail = |msg: &str| Html(html::join(&f, locked, Some(msg))).into_response();

    if f.code.trim() != invite_code(&app).await {
        return fail("Davet kodu geçersiz.");
    }
    let email = f.email.trim().to_lowercase();
    if !email.contains('@') {
        return fail("Geçerli bir e-posta gir.");
    }
    let name = f.display_name.trim();
    if name.chars().count() < 2 {
        return fail("Ad soyadını yaz.");
    }
    let nickname = match validate_nickname(&f.nickname) {
        Ok(n) => n,
        Err(e) => return fail(e),
    };
    let taken: Option<(Uuid,)> = sqlx::query_as(
        "select id from users_exposure_academy where lower(nickname) = lower($1)")
        .bind(&nickname).fetch_optional(&app.pool).await.unwrap();
    if taken.is_some() {
        return fail("Bu nickname alınmış, başka bir tane seç.");
    }
    let school = f.school.trim();
    if school.chars().count() < 2 {
        return fail("Okulunu yaz.");
    }
    // the browser enforces `required`, but the grade must also be one we offer — a
    // hand-rolled POST could otherwise put anything in the column
    if !GRADES.contains(&f.grade.trim()) {
        return fail("Sınıfını seç.");
    }
    // GitHub/LinkedIn are optional — the student may skip them here and add them in-app
    // later. Only validate when actually provided.
    let github = match normalize_profile_url(&f.github_url, "github.com") {
        Ok(v) => v,
        Err(()) => return fail("GitHub bağlantısı github.com adresinde olmalı (ör. https://github.com/kullanici)."),
    };
    let linkedin = match normalize_profile_url(&f.linkedin_url, "linkedin.com") {
        Ok(v) => v,
        Err(()) => return fail("LinkedIn bağlantısı linkedin.com adresinde olmalı (ör. https://linkedin.com/in/adin)."),
    };

    // `do nothing` on an existing email: a returning student (or one the admin added
    // by hand) just gets a login link, and their existing profile is left alone rather
    // than being overwritten by whoever typed their address.
    sqlx::query(
        "insert into users_exposure_academy (email, display_name, nickname, school, grade, github_url, linkedin_url)
         values ($1,$2,$3,$4,$5,$6,$7)
         on conflict (email) do nothing")
        .bind(&email).bind(name).bind(&nickname).bind(school).bind(f.grade.trim())
        .bind(&github).bind(&linkedin)
        .execute(&app.pool).await.unwrap();

    send_login_link(&app, &email).await;
    Html(html::join_sent(&email)).into_response()
}

/// Mint a magic link for an email that is known to have an account, unless one was
/// already sent in the last minute.
async fn send_login_link(app: &App, email: &str) {
    let recent: Option<(i32,)> = sqlx::query_as(
        "select 1 from magic_links_exposure_academy where email = $1 and used_at is null and created_at > now() - interval '60 seconds'")
        .bind(email).fetch_optional(&app.pool).await.unwrap();
    if recent.is_some() { return }
    let token = random_token();
    sqlx::query("insert into magic_links_exposure_academy (token, email, expires_at) values ($1,$2, now() + interval '15 minutes')")
        .bind(&token).bind(email).execute(&app.pool).await.unwrap();
    let link = format!("{}/magic/{}", app.base_url, token);
    send_magic_link_email(app, email, &link).await;
}

// ---- profile ----

async fn load_profile(app: &App, uid: Uuid) -> Profile {
    sqlx::query_as::<_, Profile>(
        "select email, display_name, nickname, school, grade from users_exposure_academy where id = $1")
        .bind(uid).fetch_one(&app.pool).await.unwrap()
}

async fn profile_page(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let p = load_profile(&app, user.id).await;
    Ok(Html(html::profile(&user, &p, None, None)))
}

#[derive(Deserialize)]
struct ProfileForm {
    display_name: String,
    nickname: String,
    // optional fields: default so a missing one is an empty value, not a 422 with no
    // error banner for the student to read
    #[serde(default)] school: String,
    #[serde(default)] grade: String,
}

async fn profile_post(State(app): State<App>, headers: HeaderMap, Form(f): Form<ProfileForm>) -> Result<Response, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let mut p = load_profile(&app, user.id).await;
    // echo the attempted values back so a rejected edit isn't retyped from scratch
    p.display_name = f.display_name.trim().to_string();
    p.nickname = Some(f.nickname.trim().to_string());
    p.school = Some(f.school.trim().to_string());
    p.grade = Some(f.grade.trim().to_string());
    let err = |p: &Profile, msg: &str| Html(html::profile(&user, p, None, Some(msg))).into_response();

    if p.display_name.chars().count() < 2 {
        return Ok(err(&p, "Ad soyadını yaz."));
    }
    let nickname = match validate_nickname(&f.nickname) {
        Ok(n) => n,
        Err(e) => return Ok(err(&p, e)),
    };
    let taken: Option<(Uuid,)> = sqlx::query_as(
        "select id from users_exposure_academy where lower(nickname) = lower($1) and id <> $2")
        .bind(&nickname).bind(user.id).fetch_optional(&app.pool).await.unwrap();
    if taken.is_some() {
        return Ok(err(&p, "Bu nickname alınmış, başka bir tane seç."));
    }
    let school = f.school.trim();
    if school.chars().count() < 2 {
        return Ok(err(&p, "Okulunu yaz."));
    }
    if !GRADES.contains(&f.grade.trim()) {
        return Ok(err(&p, "Sınıfını seç."));
    }

    sqlx::query(
        "update users_exposure_academy
         set display_name = $2, nickname = $3, school = $4, grade = $5
         where id = $1")
        .bind(user.id).bind(&p.display_name).bind(&nickname).bind(school).bind(f.grade.trim())
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;

    // first save completes onboarding — drop them into the portal instead of sitting on /profile
    if user.nickname.is_none() {
        return Ok(Redirect::to("/app").into_response());
    }
    let user = current_user(&app, &headers).await.unwrap_or(user);
    let p = load_profile(&app, user.id).await;
    Ok(Html(html::profile(&user, &p, Some("Profilin güncellendi."), None)).into_response())
}

async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&headers) {
        let _ = sqlx::query("delete from sessions_exposure_academy where token = $1").bind(t).execute(&app.pool).await;
    }
    (
        [(header::SET_COOKIE, "session=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax".to_string())],
        Redirect::to("/"),
    ).into_response()
}

#[derive(Deserialize)]
struct LevelQ { level: Option<String> }

async fn agentic_harness(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    Ok(Html(html::agentic_harness(&user)))
}

async fn ai_monopoly(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    Ok(Html(html::ai_monopoly(&user)))
}

#[derive(Deserialize)]
struct LangQ { lang: Option<String> }

// ---- haftalık program ----

#[derive(Deserialize)]
struct TrackQ { track: Option<String> }

/// Metadata for a track's uploaded schedule, without the bytes — those only ever leave
/// via `schedule_image`, so the page render never pulls a few MB out of the database.
async fn schedule_meta(app: &App, track: &str) -> Option<ScheduleImage> {
    sqlx::query_as::<_, ScheduleImage>(
        "select track, content_type, uploaded_at, length(image)::bigint as bytes
         from schedule_image_exposure_academy where track = $1")
        .bind(track).fetch_optional(&app.pool).await.ok().flatten()
}

async fn schedule(State(app): State<App>, headers: HeaderMap, Query(q): Query<TrackQ>) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let track = valid_schedule_track(q.track.as_deref());
    let img = schedule_meta(&app, track).await;
    let venues = load_venues(&app).await;
    Ok(Html(html::schedule(&user, track, img.as_ref(), &venues)))
}

// ---- konum / adres ----

/// One `Venue` per entry in VENUE_WEEKS, in order, always the full set. A week with no
/// rows yet comes back with empty strings rather than being absent, so the pages can
/// name it as "not announced" instead of leaving a hole where a week should be.
async fn load_venues(app: &App) -> Vec<Venue> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "select key, value from app_settings_exposure_academy where key like 'venue%'")
        .fetch_all(&app.pool).await.unwrap_or_default();
    let get = |week: u8, field: &str| {
        let k = venue_key(week, field);
        rows.iter().find(|(key, _)| *key == k).map(|(_, v)| v.clone()).unwrap_or_default()
    };
    VENUE_WEEKS.iter().map(|&week| Venue {
        week,
        dates: get(week, "dates"),
        name: get(week, "name"),
        address: get(week, "address"),
        maps_url: get(week, "maps_url"),
        notes: get(week, "notes"),
    }).collect()
}

async fn location(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let venues = load_venues(&app).await;
    Ok(Html(html::location(&user, &venues)))
}

#[derive(Deserialize)]
struct VenueForm {
    week: u8,
    #[serde(default)] dates: String,
    #[serde(default)] name: String,
    #[serde(default)] address: String,
    #[serde(default)] maps_url: String,
    #[serde(default)] notes: String,
}

async fn admin_venue(State(app): State<App>, headers: HeaderMap, Form(f): Form<VenueForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // a week we don't render would write settings rows nothing ever reads again
    if !VENUE_WEEKS.contains(&f.week) {
        return Err((StatusCode::BAD_REQUEST, "Geçersiz hafta.").into_response());
    }
    // Scheme-gate before it ever reaches an href: blank is fine (the button is then
    // simply not rendered), but a javascript:/data: value must never become a link.
    let maps_url = f.maps_url.trim();
    if !maps_url.is_empty() && !valid_http_url(maps_url) {
        return Err((StatusCode::BAD_REQUEST,
            "Haritalar bağlantısı http:// veya https:// ile başlamalı.").into_response());
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
             on conflict (key) do update set value = $2, updated_at = now()")
            .bind(venue_key(f.week, field)).bind(value)
            .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/admin"))
}

/// The uploaded screenshot itself. Members-only (it sits inside the authed routes), so
/// the schedule isn't a public URL the way /preview/{id} is. Cached hard but privately:
/// the src carries `?v=<upload time>`, so a replacement is a different URL and no stale
/// image can survive it.
async fn schedule_image(State(app): State<App>, headers: HeaderMap, Path(track): Path<String>) -> Result<Response, Response> {
    require_onboarded(current_user(&app, &headers).await)?;
    let track = valid_schedule_track(Some(&track));
    let row: Option<(Vec<u8>, String)> = sqlx::query_as(
        "select image, content_type from schedule_image_exposure_academy where track = $1")
        .bind(track).fetch_optional(&app.pool).await.ok().flatten();
    let Some((bytes, ct)) = row else { return Err(StatusCode::NOT_FOUND.into_response()) };
    Ok((
        [(header::CONTENT_TYPE, ct),
         (header::CACHE_CONTROL, "private, max-age=86400".to_string()),
         // the type is one we sniffed on upload; forbid the browser guessing another
         (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string())],
        bytes,
    ).into_response())
}

/// Content type from the file's own magic bytes rather than whatever the browser
/// claimed — these bytes get served back out under that type, so it has to be one we
/// actually recognised. `None` = not an image we accept.
fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) { return Some("image/png") }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some("image/jpeg") }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") { return Some("image/gif") }
    if b.len() >= 12 && b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" { return Some("image/webp") }
    None
}

async fn admin_schedule(State(app): State<App>, headers: HeaderMap, mut mp: Multipart) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let mut track: Option<&'static str> = None;
    let mut image: Option<Vec<u8>> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Form okunamadı."))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "track" => track = field.text().await.ok().map(|t| valid_schedule_track(Some(&t))),
            "image" => {
                let bytes = field.bytes().await.map_err(|_| bad("Görsel okunamadı — dosya çok büyük olabilir."))?;
                if !bytes.is_empty() { image = Some(bytes.to_vec()); }
            }
            _ => {}
        }
    }

    let Some(track) = track else { return Err(bad("Grup seçilmedi.")) };
    let Some(image) = image else { return Err(bad("Bir görsel seç.")) };
    let Some(content_type) = sniff_image(&image) else {
        return Err(bad("Dosya PNG, JPEG, WebP veya GIF olmalı."));
    };

    sqlx::query(
        "insert into schedule_image_exposure_academy (track, image, content_type, uploaded_at)
         values ($1,$2,$3, now())
         on conflict (track) do update set
           image = $2, content_type = $3, uploaded_at = now()")
        .bind(track).bind(&image).bind(content_type)
        .execute(&app.pool).await.map_err(|_| bad("Kaydedilemedi."))?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct ScheduleDeleteForm { track: String }

async fn admin_schedule_delete(State(app): State<App>, headers: HeaderMap, Form(f): Form<ScheduleDeleteForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from schedule_image_exposure_academy where track = $1")
        .bind(valid_schedule_track(Some(&f.track)))
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

// ---- veli onay formları ----

/// Per-file ceiling. A phone photo of a signed page is 2–5 MB and a scanned multi-page
/// PDF rarely passes 10, so this leaves headroom without letting a video through.
const CONSENT_FILE_MAX_MB: usize = 15;
/// Whole-request ceiling — the form takes several pages at once, so it has to hold a
/// handful of files, not one.
const CONSENT_UPLOAD_MAX_MB: usize = 60;
/// Per student per form. Enough for a page-by-page photo set of a long contract,
/// low enough that nobody uses the portal as a file locker.
const CONSENT_MAX_FILES: usize = 12;

/// Which forms are closed for uploads right now, in CONSENT_DOCS order. A form with no
/// settings row falls back to CONSENT_LOCKED_BY_DEFAULT, so a fresh database starts with
/// Paribu closed (its document doesn't exist yet) and the other two open.
async fn consent_locks(app: &App) -> Vec<(&'static str, bool)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r"select key, value from app_settings_exposure_academy where key like 'consent\_lock\_%'")
        .fetch_all(&app.pool).await.unwrap_or_default();
    CONSENT_DOCS.iter().map(|(kind, ..)| {
        let stored = rows.iter()
            .find(|(key, _)| *key == consent_lock_key(kind))
            .map(|(_, v)| v == "1");
        (*kind, stored.unwrap_or_else(|| CONSENT_LOCKED_BY_DEFAULT.contains(kind)))
    }).collect()
}

fn consent_is_locked(locks: &[(&'static str, bool)], kind: &str) -> bool {
    locks.iter().find(|(k, _)| *k == kind).map(|(_, l)| *l).unwrap_or(true)
}

/// One student's uploaded files, metadata only — the bytes stay in the database until
/// someone actually downloads one.
async fn user_consent_docs(app: &App, uid: Uuid) -> Vec<ConsentDoc> {
    sqlx::query_as::<_, ConsentDoc>(
        "select id, user_id, kind, filename, length(file)::bigint as bytes, uploaded_at
         from consent_docs_exposure_academy where user_id = $1 order by kind, uploaded_at")
        .bind(uid).fetch_all(&app.pool).await.unwrap_or_default()
}

async fn documents_page(app: &App, user: &User, error: Option<&str>, notice: Option<&str>) -> Response {
    let docs = user_consent_docs(app, user.id).await;
    let locks = consent_locks(app).await;
    Html(html::documents(user, &docs, &locks, error, notice)).into_response()
}

#[derive(Deserialize)]
struct OkQ { ok: Option<String> }

async fn documents(State(app): State<App>, headers: HeaderMap, Query(q): Query<OkQ>) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // POST-redirect-GET: ?ok=1 is what a finished upload comes back as, so a refresh
    // re-renders the page instead of re-posting the files.
    let notice = q.ok.as_deref().map(|_| "Belgelerin yüklendi. Teşekkürler!");
    Ok(documents_page(&app, &user, None, notice).await)
}

/// Content type + canonical extension from the file's own magic bytes. Word documents
/// are the exception: `.docx` is a ZIP and `.doc` an OLE2 container, neither of which
/// says "Word" in its header, so those two fall back to the name the browser sent.
/// `None` = not a format we accept.
fn sniff_document(b: &[u8], filename: &str) -> Option<(&'static str, &'static str)> {
    if b.starts_with(b"%PDF-") { return Some(("application/pdf", "pdf")) }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some(("image/jpeg", "jpg")) }
    if let Some(ct) = sniff_image(b) {
        return Some(match ct {
            "image/png" => (ct, "png"),
            "image/gif" => (ct, "gif"),
            _ => (ct, "webp"),
        });
    }
    // ISO-BMFF box: iPhone photos are HEIC unless the phone is set to "Most Compatible"
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return match &b[8..12] {
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" => Some(("image/heic", "heic")),
            b"mif1" | b"msf1" | b"miaf" => Some(("image/heif", "heif")),
            _ => None,
        };
    }
    let ext = filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    // OLE2 compound file — the legacy .doc container (also .xls/.ppt, hence the name check)
    if b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) && ext == "doc" {
        return Some(("application/msword", "doc"));
    }
    // OOXML/ODF are ZIP archives; only trust the name to tell us which, and only for
    // the document formats — everything is served back as an attachment, never inline.
    if b.starts_with(b"PK\x03\x04") {
        return match ext.as_str() {
            "docx" => Some(("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "docx")),
            "odt" => Some(("application/vnd.oasis.opendocument.text", "odt")),
            _ => None,
        };
    }
    None
}

async fn documents_upload(State(app): State<App>, headers: HeaderMap, mut mp: Multipart) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let too_big = format!(
        "Dosya okunamadı — tek dosya en fazla {CONSENT_FILE_MAX_MB} MB, hepsi birden en fazla {CONSENT_UPLOAD_MAX_MB} MB olabilir.");

    let mut kind: Option<&'static str> = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut oversize = false;
    // A body over the route's limit surfaces here as a read error; say so in Turkish
    // rather than letting axum's bare 413 reach a student on a phone.
    let mut read_failed = false;
    while let Some(field) = match mp.next_field().await {
        Ok(f) => f,
        Err(_) => { read_failed = true; None }
    } {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "kind" => kind = field.text().await.ok().and_then(|t| valid_consent_kind(&t)),
            "files" => {
                let filename = field.file_name().unwrap_or("belge").to_string();
                let Ok(bytes) = field.bytes().await else { read_failed = true; break };
                if bytes.len() > CONSENT_FILE_MAX_MB * 1024 * 1024 { oversize = true; break }
                if !bytes.is_empty() { files.push((filename, bytes.to_vec())); }
            }
            _ => {}
        }
    }
    if read_failed || oversize {
        return Ok(documents_page(&app, &user, Some(&too_big), None).await);
    }

    let err = |msg: &str| msg.to_string();
    let checked: Result<(&'static str, Vec<(String, Vec<u8>, &'static str)>), String> = async {
        let kind = kind.ok_or_else(|| err("Hangi form olduğu anlaşılamadı, sayfayı yenileyip tekrar dene."))?;
        if consent_is_locked(&consent_locks(&app).await, kind) {
            return Err(format!("{} şu anda yüklemeye kapalı.", consent_title(kind)));
        }
        if files.is_empty() {
            return Err(err("Bir dosya seç."));
        }
        let already: i64 = sqlx::query_scalar(
            "select count(*) from consent_docs_exposure_academy where user_id = $1 and kind = $2")
            .bind(user.id).bind(kind).fetch_one(&app.pool).await.unwrap_or(0);
        if already as usize + files.len() > CONSENT_MAX_FILES {
            return Err(format!(
                "Bir form için en fazla {CONSENT_MAX_FILES} dosya yükleyebilirsin ({already} tane zaten yüklü). Fazlasını sil ya da sayfaları tek PDF'te birleştir."));
        }
        let mut prepared = Vec::with_capacity(files.len());
        for (filename, bytes) in files {
            let Some((content_type, ext)) = sniff_document(&bytes, &filename) else {
                return Err(format!(
                    "\"{}\" desteklenmeyen bir dosya türü. PDF, JPG, PNG, HEIC, WebP ya da Word (DOC/DOCX) yükle.",
                    filename.chars().take(60).collect::<String>()));
            };
            // Keep the student's own name, but make sure it ends in the extension the
            // bytes actually are — phone uploads arrive as "image.jpg" or worse, and the
            // admin has to be able to open the file by double-clicking it.
            let base = safe_filename(&filename);
            let named = if base.to_ascii_lowercase().ends_with(&format!(".{ext}")) { base }
                else { format!("{base}.{ext}") };
            prepared.push((named, bytes, content_type));
        }
        Ok((kind, prepared))
    }.await;

    let (kind, prepared) = match checked {
        Ok(v) => v,
        Err(msg) => return Ok(documents_page(&app, &user, Some(&msg), None).await),
    };

    for (filename, bytes, content_type) in prepared {
        sqlx::query(
            "insert into consent_docs_exposure_academy (user_id, kind, filename, content_type, file)
             values ($1,$2,$3,$4,$5)")
            .bind(user.id).bind(kind).bind(&filename).bind(content_type).bind(&bytes)
            .execute(&app.pool).await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Belge kaydedilemedi, tekrar dene.").into_response())?;
    }
    Ok(Redirect::to("/documents?ok=1").into_response())
}

async fn documents_delete(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdForm>) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // `and user_id` is the authorization: another student's id simply matches nothing.
    // A closed form is also closed for deletions — once it is being collected, the set
    // of files on record stops moving under the admin's feet.
    let locks = consent_locks(&app).await;
    let kind: Option<(String,)> = sqlx::query_as(
        "select kind from consent_docs_exposure_academy where id = $1 and user_id = $2")
        .bind(f.id).bind(user.id).fetch_optional(&app.pool).await.unwrap_or(None);
    if let Some((kind,)) = kind {
        if consent_is_locked(&locks, &kind) {
            return Ok(documents_page(&app, &user, Some("Bu form şu anda kapalı, dosyaları değiştiremezsin."), None).await);
        }
        sqlx::query("delete from consent_docs_exposure_academy where id = $1 and user_id = $2")
            .bind(f.id).bind(user.id).execute(&app.pool).await
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/documents").into_response())
}

/// Percent-encoding for the RFC 5987 `filename*` parameter — Turkish names are the
/// normal case here, and a raw ç in a header is not a legal header value.
fn pct_encode(s: &str) -> String {
    s.bytes().map(|b| {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
            (b as char).to_string()
        } else {
            format!("%{b:02X}")
        }
    }).collect()
}

/// `attachment` both ways: an ASCII-folded name for old clients, the real UTF-8 one for
/// everything else. Always a download, never rendered in the tab — the bytes came from
/// a student, so nothing gets to run in our origin.
fn attachment(filename: &str, content_type: &str, bytes: Vec<u8>) -> Response {
    let name = safe_filename(filename);
    let ascii: String = name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') { c } else { '_' })
        .collect();
    (
        [(header::CONTENT_TYPE, content_type.to_string()),
         (header::CONTENT_DISPOSITION,
          format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", pct_encode(&name))),
         (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
         (header::CACHE_CONTROL, "private, no-store".to_string())],
        bytes,
    ).into_response()
}

/// One uploaded document. The student who uploaded it can fetch it back; an admin can
/// fetch anyone's. Nobody else — a signed consent form has a minor's name, address and
/// a parent's signature on it.
async fn document_file(State(app): State<App>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Response, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let row: Option<(Uuid, String, String, Vec<u8>)> = sqlx::query_as(
        "select user_id, filename, content_type, file from consent_docs_exposure_academy where id = $1")
        .bind(id).fetch_optional(&app.pool).await.ok().flatten();
    let Some((owner, filename, content_type, bytes)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    if owner != user.id && !user.is_admin {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(attachment(&filename, &content_type, bytes))
}

#[derive(Deserialize)]
struct ConsentLockForm { kind: String, locked: bool }

async fn admin_documents_lock(State(app): State<App>, headers: HeaderMap, Form(f): Form<ConsentLockForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let Some(kind) = valid_consent_kind(&f.kind) else {
        return Err((StatusCode::BAD_REQUEST, "Geçersiz form.").into_response());
    };
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ($1,$2, now())
         on conflict (key) do update set value = $2, updated_at = now()")
        .bind(consent_lock_key(kind)).bind(if f.locked { "1" } else { "0" })
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// Every consent form on file, as one ZIP: a folder per form, a folder per student
/// inside it, files numbered in upload order. Stored (uncompressed) because the payload
/// is PDFs and phone photos, which don't get smaller — this is a copy, not a squeeze.
///
/// `_EKSIKLER.txt` at the root lists, per form, who has uploaded and who hasn't, so the
/// chasing list comes out of the same download as the documents.
async fn admin_documents_zip(State(app): State<App>, headers: HeaderMap) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // one bail-out for three unrelated error types (sqlx, zip, io) — a half-written
    // archive is worse than none, so any of them ends the download
    fn oops<E: std::fmt::Display>(e: E) -> Response {
        eprintln!("documents.zip failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }

    let docs: Vec<(String, String, String, String, Vec<u8>)> = sqlx::query_as(
        "select u.display_name, u.email, d.kind, d.filename, d.file
         from consent_docs_exposure_academy d
         join users_exposure_academy u on u.id = d.user_id
         order by d.kind, lower(u.display_name), d.uploaded_at, d.id")
        .fetch_all(&app.pool).await.map_err(oops)?;
    // Everyone expected to hand something in: every non-admin account, same set the
    // admin grid lists. Deliberately not filtered to onboarded students — an account
    // the admin added by hand that never opened its invite link is exactly the kind of
    // row a chase list has to show.
    let students: Vec<(String, String)> = sqlx::query_as(
        "select display_name, email from users_exposure_academy
         where not is_admin order by lower(display_name)")
        .fetch_all(&app.pool).await.map_err(oops)?;

    let summary = format!("Veli onay formları · {}\n{}",
        chrono::Utc::now().format("%d.%m.%Y %H:%M UTC"),
        consent_summary(&docs, &students));
    let bytes = build_documents_zip(&docs, &summary).map_err(oops)?;
    let filename = format!("veli-onay-formlari-{}.zip", chrono::Utc::now().format("%Y-%m-%d"));
    Ok(attachment(&filename, "application/zip", bytes))
}

/// One `(display_name, email, kind, filename, bytes)` row as it comes out of the join.
type DocRow = (String, String, String, String, Vec<u8>);

/// The `_EKSIKLER.txt` body: per form, who has uploaded and who hasn't. Marked `[X]`
/// or `[ ]` so it reads as a checklist for whoever is chasing the missing ones.
fn consent_summary(docs: &[DocRow], students: &[(String, String)]) -> String {
    let mut out = String::new();
    for (kind, title, _) in CONSENT_DOCS {
        let has = |email: &str| docs.iter().any(|(_, e, k, ..)| e == email && k == kind);
        let uploaded = students.iter().filter(|(_, email)| has(email)).count();
        out.push_str(&format!("\n=== {title} ===\nYükleyen: {uploaded}/{}\n", students.len()));
        for (name, email) in students {
            out.push_str(&format!("  [{}] {name} <{email}>\n", if has(email) { "X" } else { " " }));
        }
    }
    out
}

/// A folder per form, a folder per student inside it, files numbered in upload order —
/// so the QNBEYOND folder is exactly what gets handed to QNBEYOND. Stored, not deflated:
/// PDFs and phone photos don't get smaller, so this is a copy rather than a squeeze.
/// Split out of the handler so the layout the admin unzips is covered by a test.
fn build_documents_zip(docs: &[DocRow], summary: &str) -> Result<Vec<u8>, zip::result::ZipError> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    zip.start_file("_EKSIKLER.txt", opts)?;
    zip.write_all(summary.as_bytes())?;

    // number files per (form, student) so a student's pages keep their order and two
    // photos both called "image.jpg" can't collide inside the same folder. The rows
    // arrive grouped by (kind, student), which is what makes the running count work.
    let mut n = 0usize;
    let mut prev: Option<(&str, &str)> = None;
    for (name, email, kind, filename, bytes) in docs {
        let who = (kind.as_str(), email.as_str());
        n = if prev == Some(who) { n + 1 } else { 1 };
        prev = Some(who);
        zip.start_file(format!("{kind}/{student}/{n:02}-{file}",
            student = safe_filename(name), file = safe_filename(filename)), opts)?;
        zip.write_all(bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

async fn demos(State(app): State<App>, headers: HeaderMap, Query(q): Query<LangQ>) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let lang = if q.lang.as_deref() == Some("en") { "en" } else { "tr" };
    Ok(Html(html::demos(&user, lang)))
}

/// Ana Sayfa. No content of its own — three doors (videolar / görevler / puan tablosu),
/// each carrying the one number that tells the student where they stand.
async fn home(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let videos_total: i64 = sqlx::query_scalar("select count(*) from videos_exposure_academy")
        .fetch_one(&app.pool).await.unwrap();
    let videos_done: i64 = sqlx::query_scalar(
        "select count(*) from watch_progress_exposure_academy
         where user_id = $1 and duration > 0 and max_position >= duration * 0.9")
        .bind(user.id).fetch_one(&app.pool).await.unwrap();
    // "Açık" = bu öğrencinin henüz geçmiş bir gönderimi olmayan görev.
    let open_tasks: i64 = sqlx::query_scalar(
        "select count(*) from tasks_exposure_academy t
         where not exists (select 1 from submissions_exposure_academy s
                           where s.task_id = t.id and s.user_id = $1 and s.status = 'passed')")
        .bind(user.id).fetch_one(&app.pool).await.unwrap();
    let all = leader_rows(&app).await;
    // Points come from the full list so a hidden (intern) account still sees its own
    // total; the rank comes from the visible standings, where it has no place at all.
    let points = all.iter().find(|r| r.id == user.id).map(|r| r.points()).unwrap_or(0);
    let rows: Vec<LeaderRow> = all.into_iter().filter(|r| !r.hidden).collect();
    let ranks = html::dense_ranks(&rows);
    let rank = rows.iter().position(|r| r.id == user.id).map(|i| ranks[i]);
    // Consent forms count only while they're open for upload: a form whose document
    // isn't ready yet (locked) is not something the student can be behind on.
    let locks = consent_locks(&app).await;
    let open: Vec<&'static str> = locks.iter().filter(|(_, l)| !l).map(|(k, _)| *k).collect();
    let docs = user_consent_docs(&app, user.id).await;
    let consent_done = open.iter().filter(|k| docs.iter().any(|d| d.kind == **k)).count();
    Ok(Html(html::home(&user, videos_done, videos_total, open_tasks, points, rank, consent_done, open.len())))
}

async fn video_grid(State(app): State<App>, headers: HeaderMap, Query(q): Query<LevelQ>) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let level = q.level.as_deref().filter(|l| html::LEVELS.iter().any(|(k, _)| k == l));
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
         order by v.level, v.position, v.created_at")
        .bind(user.id).bind(level)
        .fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::video_grid(&user, &videos, level)))
}

async fn watch(State(app): State<App>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let video = sqlx::query_as::<_, Video>("select id, youtube_id, title, level from videos_exposure_academy where id = $1")
        .bind(id).fetch_optional(&app.pool).await.unwrap()
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let playlist = sqlx::query_as::<_, VideoWithProgress>(
        "select v.id, v.youtube_id, v.title, v.level,
                coalesce(w.max_position, 0) as max_position, coalesce(w.duration, 0) as duration
         from videos_exposure_academy v
         left join watch_progress_exposure_academy w on w.video_id = v.id and w.user_id = $1
         where v.level = $2 order by v.position, v.created_at")
        .bind(user.id).bind(&video.level)
        .fetch_all(&app.pool).await.unwrap();
    let resume_at = playlist.iter().find(|v| v.id == video.id)
        .map(|v| if v.duration > 0.0 && v.max_position < v.duration - 10.0 { v.max_position as f64 } else { 0.0 })
        .unwrap_or(0.0);
    Ok(Html(html::watch(&user, &video, &playlist, resume_at)))
}

#[derive(Deserialize)]
struct ProgressReq { video_id: Uuid, position: f32, duration: f32, delta: f32 }

async fn progress(State(app): State<App>, headers: HeaderMap, Json(r): Json<ProgressReq>) -> Result<StatusCode, Response> {
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

// ---- leaderboard ----

async fn leaderboard(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // Hidden (intern) accounts never reach a rendered standings list — not even their
    // own, so nothing about them can leak through a shared screen or a screenshot.
    let rows: Vec<LeaderRow> = leader_rows(&app).await.into_iter().filter(|r| !r.hidden).collect();
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
async fn leader_rows(app: &App) -> Vec<LeaderRow> {
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

// ---- board ----

/// True once the student has BOTH public profiles on file. The board is gated on this:
/// they could skip the profiles during onboarding, but not to reach the task board.
async fn has_both_profiles(app: &App, uid: Uuid) -> (bool, Option<String>, Option<String>) {
    let (github, linkedin): (Option<String>, Option<String>) = sqlx::query_as(
        "select github_url, linkedin_url from users_exposure_academy where id = $1")
        .bind(uid).fetch_one(&app.pool).await.unwrap();
    let ok = github.as_deref().is_some_and(|s| !s.trim().is_empty())
        && linkedin.as_deref().is_some_and(|s| !s.trim().is_empty());
    (ok, github, linkedin)
}

async fn board(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // Gate: no board until both GitHub and LinkedIn are set. Skippable at onboarding,
    // enforced here — show the setup requirement instead of the tasks. Admins are exempt,
    // same as the onboarding gate (they don't onboard and need to see the board).
    if !user.is_admin {
        let (ok, github, linkedin) = has_both_profiles(&app, user.id).await;
        if !ok {
            return Ok(Html(html::board_locked(&user, github.as_deref(), linkedin.as_deref(), None)));
        }
    }
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let subs = sqlx::query_as::<_, SubmissionView>(
        "select distinct on (s.task_id) s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url, s.plan_md,
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
         order by ti.created_at")
        .bind(user.id).fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::board(&user, &tasks, &subs, &interests)))
}

#[derive(Deserialize)]
struct ProfilesForm {
    #[serde(default)] github_url: String,
    #[serde(default)] linkedin_url: String,
}

/// Save the GitHub/LinkedIn profiles from the board gate. Both are required here (the
/// board stays locked otherwise); on success we drop the student straight into /board.
async fn board_profiles(State(app): State<App>, headers: HeaderMap, Form(f): Form<ProfilesForm>) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // re-render the gate with an error, echoing back what they typed
    let fail = |msg: &str| Html(html::board_locked(&user, Some(f.github_url.trim()), Some(f.linkedin_url.trim()), Some(msg))).into_response();

    let github = match normalize_profile_url(&f.github_url, "github.com") {
        Ok(Some(u)) => u,
        Ok(None) => return Ok(fail("GitHub profilini ekle.")),
        Err(()) => return Ok(fail("GitHub bağlantısı github.com adresinde olmalı (ör. https://github.com/kullanici).")),
    };
    let linkedin = match normalize_profile_url(&f.linkedin_url, "linkedin.com") {
        Ok(Some(u)) => u,
        Ok(None) => return Ok(fail("LinkedIn profilini ekle.")),
        Err(()) => return Ok(fail("LinkedIn bağlantısı linkedin.com adresinde olmalı (ör. https://linkedin.com/in/adin).")),
    };

    sqlx::query("update users_exposure_academy set github_url = $2, linkedin_url = $3 where id = $1")
        .bind(user.id).bind(&github).bind(&linkedin)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/board").into_response())
}

#[derive(Deserialize)]
struct InterestForm { task_id: Uuid }

// toggle: delete my interest if present, else add it. Two idempotent statements,
// no tx needed — worst case a double-click is a harmless no-op either way.
async fn board_interest(State(app): State<App>, headers: HeaderMap, Form(f): Form<InterestForm>) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let deleted = sqlx::query("delete from task_interest_exposure_academy where task_id = $1 and user_id = $2")
        .bind(f.task_id).bind(user.id).execute(&app.pool).await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    if deleted.rows_affected() == 0 {
        sqlx::query("insert into task_interest_exposure_academy (task_id, user_id) values ($1,$2) on conflict do nothing")
            .bind(f.task_id).bind(user.id).execute(&app.pool).await
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/board"))
}

/// plan.md is stored inline in the DB as text, so it stays small.
const PLAN_MAX_BYTES: usize = 200 * 1024;

async fn board_submit(State(app): State<App>, headers: HeaderMap, mut mp: Multipart) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let mut task_id: Option<Uuid> = None;
    let mut repo_url = String::new();
    let mut plan_md: Option<String> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Form okunamadı."))? {
        // name() borrows the field, text()/bytes() consume it — copy the name out first
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "task_id" => task_id = field.text().await.ok().and_then(|t| t.parse().ok()),
            "repo_url" => repo_url = field.text().await.map_err(|_| bad("Form okunamadı."))?.trim().to_string(),
            "plan" => {
                let bytes = field.bytes().await.map_err(|_| bad("plan.md okunamadı."))?;
                if bytes.len() > PLAN_MAX_BYTES {
                    return Err(bad("plan.md 200 KB'den büyük olamaz."));
                }
                let text = String::from_utf8(bytes.to_vec()).map_err(|_| bad("plan.md UTF-8 metin olmalı."))?;
                if !text.trim().is_empty() {
                    plan_md = Some(text);
                }
            }
            _ => {}
        }
    }

    let Some(task_id) = task_id else { return Err(bad("Görev bulunamadı.")) };
    if !repo_url.starts_with("https://github.com/") {
        return Err(bad("Repo bağlantısı https://github.com/ ile başlamalı."));
    }
    let Some(plan_md) = plan_md else { return Err(bad("plan.md dosyası gerekli.")) };
    sqlx::query("insert into submissions_exposure_academy (task_id, user_id, repo_url, plan_md) values ($1,$2,$3,$4)")
        .bind(task_id).bind(user.id).bind(&repo_url).bind(&plan_md)
        .execute(&app.pool).await.unwrap();
    Ok(Redirect::to("/board"))
}

// ---- admin ----

async fn admin_page(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    let stats = sqlx::query_as::<_, StatRow>(
        "select u.display_name, v.title as video_title, w.seconds_watched, w.max_position, w.duration, w.updated_at
         from watch_progress_exposure_academy w join users_exposure_academy u on u.id = w.user_id join videos_exposure_academy v on v.id = w.video_id
         order by w.updated_at desc limit 200")
        .fetch_all(&app.pool).await.unwrap();
    let subs = sqlx::query_as::<_, SubmissionView>(
        "select s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url, s.plan_md,
                u.display_name, u.email, t.title as task_title, t.level as task_level, s.points_override, s.created_at
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         order by s.created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let videos = sqlx::query_as::<_, Video>("select id, youtube_id, title, level from videos_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let members = sqlx::query_as::<_, MemberRow>(
        "select id, display_name, email, nickname, is_admin, hidden_from_leaderboard
         from users_exposure_academy order by is_admin desc, lower(coalesce(nickname, display_name))")
        .fetch_all(&app.pool).await.unwrap();
    let invite_code = invite_code(&app).await;
    let schedule_images = sqlx::query_as::<_, ScheduleImage>(
        "select track, content_type, uploaded_at, length(image)::bigint as bytes
         from schedule_image_exposure_academy")
        .fetch_all(&app.pool).await.unwrap_or_default();
    // metadata only — the bytes leave through /documents/file/{id} or the ZIP
    let consent_docs = sqlx::query_as::<_, ConsentDoc>(
        "select id, user_id, kind, filename, length(file)::bigint as bytes, uploaded_at
         from consent_docs_exposure_academy order by kind, uploaded_at")
        .fetch_all(&app.pool).await.unwrap_or_default();
    let consent_locks = consent_locks(&app).await;
    Ok(Html(html::admin(&user, &stats, &subs, &videos, &tasks, &members, &invite_code, &app.base_url,
        &schedule_images, &load_venues(&app).await, &consent_docs, &consent_locks)))
}

fn parse_youtube_id(input: &str) -> String {
    // accepts raw ID, youtube.com/watch?v=ID, youtu.be/ID
    let s = input.trim();
    if let Some(i) = s.find("v=") {
        return s[i + 2..].split('&').next().unwrap_or("").to_string();
    }
    if let Some(i) = s.find("youtu.be/") {
        return s[i + 9..].split(['?', '&']).next().unwrap_or("").to_string();
    }
    s.rsplit('/').next().unwrap_or(s).to_string()
}

#[derive(Deserialize)]
struct VideoForm { title: String, youtube: String, level: String }

async fn admin_video(State(app): State<App>, headers: HeaderMap, Form(f): Form<VideoForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("insert into videos_exposure_academy (youtube_id, title, level) values ($1,$2,$3)")
        .bind(parse_youtube_id(&f.youtube)).bind(&f.title).bind(&f.level)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

fn valid_http_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://")
}

/// Reject obvious internal targets before the server-side fetch (SSRF hardening).
/// ponytail: literal host/IP denylist, no DNS resolution — a hostname that resolves
/// to an internal IP still slips through. Proportionate here: the caller is an admin
/// and only ever learns a boolean, never a response body. Upgrade to resolve-then-
/// check-the-IP if this fetch is ever made to return data.
fn is_internal_host(url: &str) -> bool {
    use std::net::IpAddr;
    let Ok(parsed) = reqwest::Url::parse(url) else { return true }; // unparseable → treat as blocked
    let Some(host) = parsed.host_str() else { return true };
    let h = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h == "metadata.google.internal" {
        return true;
    }
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified(),
        Ok(IpAddr::V6(v6)) => {
            v6.is_loopback() || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00  // fc00::/7 unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80  // fe80::/10 link-local
        }
        Err(_) => false, // a real hostname — allowed (see the DNS-rebinding ceiling above)
    }
}

/// GET the URL and decide whether it permits iframe embedding. Conservative:
/// any framing restriction, or a network error/timeout, counts as NOT embeddable
/// (so we fall back to a screenshot, which always renders).
async fn check_embeddable(client: &reqwest::Client, url: &str) -> bool {
    if is_internal_host(url) { return false; }
    let Ok(resp) = client.get(url).timeout(std::time::Duration::from_secs(6)).send().await else { return false };
    let h = resp.headers();
    if let Some(xfo) = h.get("x-frame-options").and_then(|v| v.to_str().ok()) {
        let x = xfo.to_ascii_lowercase();
        if x.contains("deny") || x.contains("sameorigin") { return false; }
    }
    if let Some(csp) = h.get("content-security-policy").and_then(|v| v.to_str().ok()) {
        let c = csp.to_ascii_lowercase();
        // a frame-ancestors directive that isn't a blanket '*' means we're very likely blocked
        if c.contains("frame-ancestors") && !c.contains('*') { return false; }
    }
    true
}

/// Fetch a hero (above-the-fold) screenshot via Microlink, returning (bytes, content_type).
/// `embed=screenshot.url` makes Microlink respond with the image binary directly (one hop).
async fn fetch_screenshot(client: &reqwest::Client, key: &str, url: &str) -> Option<(Vec<u8>, String)> {
    let mut req = client.get("https://api.microlink.io/")
        .query(&[
            ("url", url), ("screenshot", "true"), ("meta", "false"),
            ("embed", "screenshot.url"),
            ("viewport.width", "1280"), ("viewport.height", "800"),
        ])
        .timeout(std::time::Duration::from_secs(25));
    if !key.is_empty() { req = req.header("x-api-key", key); }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() { return None; }
    let ct = resp.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("image/png").to_string();
    if !ct.starts_with("image/") { return None; } // Microlink returns JSON error on failure
    let bytes = resp.bytes().await.ok()?;
    Some((bytes.to_vec(), ct))
}

#[derive(Deserialize)]
struct TaskForm { title: String, description: String, level: String, #[serde(default)] example_url: String }

async fn admin_task(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let example = f.example_url.trim();
    if !example.is_empty() && !valid_http_url(example) {
        return Err((StatusCode::BAD_REQUEST, "Örnek URL http:// veya https:// ile başlamalı.").into_response());
    }
    let embeddable = if example.is_empty() { None } else { Some(check_embeddable(&app.http, example).await) };
    // position = end of this level's order, so new tasks land last (hardest) until reordered
    sqlx::query("insert into tasks_exposure_academy (title, description, level, example_url, example_embeddable, position) values ($1,$2,$3, nullif($4,''), $5, (select coalesce(max(position),0)+1 from tasks_exposure_academy where level=$3))")
        .bind(&f.title).bind(&f.description).bind(&f.level).bind(example).bind(embeddable)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct TaskEditForm { id: Uuid, title: String, description: String }

async fn admin_task_edit(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskEditForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let title = f.title.trim();
    let description = f.description.trim();
    if title.is_empty() || description.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Başlık ve tanım boş olamaz.").into_response());
    }
    sqlx::query("update tasks_exposure_academy set title = $2, description = $3 where id = $1")
        .bind(f.id).bind(title).bind(description)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct TaskExampleForm { id: Uuid, example_url: String }

async fn admin_task_example(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskExampleForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let url = f.example_url.trim();
    if !url.is_empty() && !valid_http_url(url) {
        return Err((StatusCode::BAD_REQUEST, "Örnek URL http:// veya https:// ile başlamalı.").into_response());
    }
    // only update the URL — the live/image preview mode is the admin's manual choice
    // (set via /admin/task/preview) and is preserved across URL edits
    sqlx::query("update tasks_exposure_academy set example_url = nullif($2,'') where id = $1")
        .bind(f.id).bind(url)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct TaskPreviewForm { id: Uuid, mode: String }

/// Admin's manual per-task choice: live iframe preview vs cached screenshot image.
async fn admin_task_preview(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskPreviewForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let live = f.mode == "live";
    sqlx::query("update tasks_exposure_academy set example_embeddable = $2 where id = $1")
        .bind(f.id).bind(live)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

// ---- example-project screenshot preview ----

fn image_response(bytes: Vec<u8>, ct: &str) -> Response {
    (
        [(header::CONTENT_TYPE, ct.to_owned()),
         (header::CACHE_CONTROL, "public, max-age=86400".to_string())],
        bytes,
    ).into_response()
}

/// Fallback shown when there's no cached image yet and generation failed. Short
/// cache so the next view retries. Displays the URL's host, or a generic label.
fn placeholder_svg(url: &str) -> Response {
    let host = url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("");
    let label = if host.is_empty() { "önizleme yok".to_string() } else { html::esc(host) };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="800" viewBox="0 0 1280 800"><rect width="1280" height="800" fill="#18181b"/><text x="640" y="400" fill="#71717a" font-family="sans-serif" font-size="36" text-anchor="middle" dominant-baseline="middle">{label}</text></svg>"##,
    );
    (
        [(header::CONTENT_TYPE, "image/svg+xml".to_string()),
         (header::CACHE_CONTROL, "public, max-age=300".to_string())],
        svg,
    ).into_response()
}

/// Serve the cached hero screenshot for a task's example URL, generating it on
/// first request. Keyed by task id (not raw URL) so only admin-set URLs are ever
/// fetched — no open proxy. Public, no auth (it screenshots public sites).
async fn task_preview(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    let url: Option<String> = sqlx::query_scalar("select example_url from tasks_exposure_academy where id = $1")
        .bind(id).fetch_optional(&app.pool).await.ok().flatten().flatten();
    let Some(url) = url.filter(|u| !u.is_empty()) else { return placeholder_svg("") };

    // cache hit?
    if let Ok(Some((img, ct))) = sqlx::query_as::<_, (Vec<u8>, String)>(
        "select image, content_type from screenshot_cache_exposure_academy where url = $1")
        .bind(&url).fetch_optional(&app.pool).await {
        return image_response(img, &ct);
    }
    // miss -> fetch from Microlink, cache, serve. On failure serve a non-cached placeholder.
    match fetch_screenshot(&app.http, &app.microlink_key, &url).await {
        Some((bytes, ct)) => {
            let _ = sqlx::query("insert into screenshot_cache_exposure_academy (url, image, content_type) values ($1,$2,$3) on conflict (url) do nothing")
                .bind(&url).bind(&bytes).bind(&ct).execute(&app.pool).await;
            image_response(bytes, &ct)
        }
        None => placeholder_svg(&url),
    }
}

#[derive(Deserialize)]
struct IdForm { id: Uuid }

#[derive(Deserialize)]
struct IdLevelForm { id: Uuid, level: String }

async fn admin_task_level(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdLevelForm>) -> Result<Redirect, Response> {
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

#[derive(Deserialize)]
struct TaskMoveForm { id: Uuid, dir: String }

// swap a task's position with its neighbour in the same level (ponytail: adjacent-swap
// assumes unique positions per level, which the backfill + insert-position guarantee).
async fn admin_task_move(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskMoveForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let mut tx = app.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let Some((level, position)) = sqlx::query_as::<_, (String, i32)>(
        "select level, position from tasks_exposure_academy where id = $1")
        .bind(f.id).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    else { return Ok(Redirect::to("/admin")); };
    let neighbor = if f.dir == "up" {
        "select id, position from tasks_exposure_academy where level = $1 and position < $2 order by position desc limit 1"
    } else {
        "select id, position from tasks_exposure_academy where level = $1 and position > $2 order by position asc limit 1"
    };
    if let Some((nid, npos)) = sqlx::query_as::<_, (Uuid, i32)>(neighbor)
        .bind(&level).bind(position).fetch_optional(&mut *tx).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    {
        sqlx::query("update tasks_exposure_academy set position = $2 where id = $1")
            .bind(f.id).bind(npos).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
        sqlx::query("update tasks_exposure_academy set position = $2 where id = $1")
            .bind(nid).bind(position).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/admin"))
}

async fn admin_task_delete(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades to submissions (FK) — points earned from this task go with it
    sqlx::query("delete from tasks_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

async fn admin_video_level(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdLevelForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("update videos_exposure_academy set level = $2 where id = $1")
        .bind(f.id).bind(&f.level)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

async fn admin_video_delete(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades to watch progress (FK) — points earned from this video go with it.
    // NOTE: seed_videos re-inserts any ID still listed in videos.dat on next restart.
    sqlx::query("delete from videos_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct UserForm {
    email: String,
    display_name: String,
    /// Unchecked checkboxes are simply absent from the POST body, hence the Option.
    #[serde(default)]
    hidden: Option<String>,
}

async fn admin_user(State(app): State<App>, headers: HeaderMap, Form(f): Form<UserForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let email = f.email.trim().to_lowercase();
    // Adding the row here with `hidden` pre-set is how an intern account gets created
    // before she ever opens the invite link: join_post's `on conflict (email) do nothing`
    // leaves this row alone, so she is never visible for even one page load.
    sqlx::query(
        "insert into users_exposure_academy (email, display_name, hidden_from_leaderboard)
         values ($1,$2,$3) on conflict (email) do nothing")
        .bind(&email).bind(&f.display_name).bind(f.hidden.is_some())
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct UserHiddenForm { id: Uuid, hidden: bool }

/// Flip a student in or out of the published standings (and the board's teammate chips).
/// Admins are excluded on both sides already, so the flag is only meaningful for students.
async fn admin_user_hidden(State(app): State<App>, headers: HeaderMap, Form(f): Form<UserHiddenForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("update users_exposure_academy set hidden_from_leaderboard = $2 where id = $1")
        .bind(f.id).bind(f.hidden)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

async fn admin_user_delete(State(app): State<App>, headers: HeaderMap, Form(f): Form<IdForm>) -> Result<Redirect, Response> {
    let me = require_admin(current_user(&app, &headers).await)?;
    // guard rails: never let an admin delete themselves or another admin from here.
    // Deleting a student cascades to their sessions, watch progress, and submissions (FK).
    if f.id == me.id {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query("delete from users_exposure_academy where id = $1 and is_admin = false")
        .bind(f.id)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

async fn admin_rotate_invite(State(app): State<App>, headers: HeaderMap) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let new_code = &random_token()[..8];
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ('invite_code', $1, now())
         on conflict (key) do update set value = $1, updated_at = now()")
        .bind(new_code).execute(&app.pool).await.unwrap();
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct ReviewForm {
    id: Uuid,
    status: String,
    feedback: String,
    /// The Puan box. Blank is the normal case and means "score it by level".
    #[serde(default)] points: String,
}

async fn admin_review(State(app): State<App>, headers: HeaderMap, Form(f): Form<ReviewForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // Blank clears any previous override and puts the row back on the level default;
    // anything that isn't a non-negative number is a typo, so reject rather than
    // silently scoring the project at some other value.
    let points: Option<i32> = match f.points.trim() {
        "" => None,
        s => Some(s.parse::<i32>().ok().filter(|p| *p >= 0)
            .ok_or_else(|| StatusCode::BAD_REQUEST.into_response())?),
    };
    sqlx::query("update submissions_exposure_academy set status = $2, feedback = nullif($3,''), points_override = $4 where id = $1")
        .bind(f.id).bind(&f.status).bind(&f.feedback).bind(points)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// One .txt holding a review prompt per submission that hasn't reached a verdict yet,
/// so a whole grading round can be pasted into an agent in one go. Its own narrow
/// projection rather than SubmissionView — that struct has no task description, and
/// widening it would drag the board query along for no reason.
async fn admin_prompts_txt(State(app): State<App>, headers: HeaderMap) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let rows: Vec<(String, String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "select u.display_name, t.title, t.description, s.repo_url, s.created_at
         from submissions_exposure_academy s
         join users_exposure_academy u on u.id = s.user_id
         join tasks_exposure_academy t on t.id = s.task_id
         where s.status in ('pending', 'reviewing')
         order by s.created_at desc")
        .fetch_all(&app.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    let body = if rows.is_empty() {
        "İncelenmeyi bekleyen gönderim yok.\n".to_string()
    } else {
        rows.iter().map(|(name, title, desc, repo, at)| format!(
            "=== {name} — {title} — {date} ===\n{prompt}\n",
            date = at.format("%d.%m.%Y"),
            prompt = review_prompt(repo, if desc.trim().is_empty() { title } else { desc }),
        )).collect::<Vec<_>>().join("\n")
    };

    let filename = format!("prompts-{}.txt", chrono::Utc::now().format("%Y-%m-%d"));
    Ok((
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
         (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
         (header::CACHE_CONTROL, "no-store".to_string())],
        body,
    ).into_response())
}

// ---- worker API (Phase 3 pipeline, see README) ----

/// Constant-time byte equality — no early exit on the first mismatch, so the compare
/// time doesn't leak how many leading bytes were right (would let a co-located
/// attacker recover the token). Length is allowed to short-circuit; it isn't secret.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) { diff |= x ^ y; }
    diff == 0
}

fn check_worker(app: &App, headers: &HeaderMap) -> Result<(), Response> {
    let ok = !app.worker_token.is_empty()
        && headers.get("x-worker-token").and_then(|v| v.to_str().ok())
            .is_some_and(|t| ct_eq(t.as_bytes(), app.worker_token.as_bytes()));
    if ok { Ok(()) } else { Err(StatusCode::UNAUTHORIZED.into_response()) }
}

async fn worker_pending(State(app): State<App>, headers: HeaderMap) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    // claim atomically: pending -> reviewing
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "update submissions_exposure_academy set status = 'reviewing'
         where id in (select id from submissions_exposure_academy where status = 'pending' order by created_at limit 5)
         returning id, repo_url, (select title from tasks_exposure_academy where tasks_exposure_academy.id = submissions_exposure_academy.task_id)")
        .fetch_all(&app.pool).await.unwrap();
    Ok(Json(serde_json::json!(rows.iter().map(|(id, repo, task)| {
        serde_json::json!({"id": id, "repo_url": repo, "task_title": task})
    }).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct WorkerResult { id: Uuid, status: String, feedback: Option<String>, demo_video_url: Option<String> }

async fn worker_result(State(app): State<App>, headers: HeaderMap, Json(r): Json<WorkerResult>) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.status != "passed" && r.status != "failed" {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query("update submissions_exposure_academy set status = $2, feedback = $3, demo_video_url = $4 where id = $1")
        .bind(r.id).bind(&r.status).bind(&r.feedback).bind(&r.demo_video_url)
        .execute(&app.pool).await.unwrap();
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_matches_only_exact() {
        assert!(ct_eq(b"secret-token", b"secret-token"));
        assert!(!ct_eq(b"secret-token", b"secret-toke"));  // length differs
        assert!(!ct_eq(b"secret-token", b"Secret-token")); // one byte differs
        assert!(!ct_eq(b"", b"x"));
    }

    // ---- veli onay formları ----

    /// Every format the form advertises is recognised from its own bytes, and the
    /// extension we store matches what the file actually is (phones send "image.jpg"
    /// for a HEIC often enough that trusting the name would hand the admin a file
    /// their computer refuses to open).
    #[test]
    fn accepted_formats_are_sniffed_not_trusted() {
        let pdf = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3";
        assert_eq!(sniff_document(pdf, "veli onay.pdf"), Some(("application/pdf", "pdf")));
        // a PDF that arrived named .jpg is still a PDF
        assert_eq!(sniff_document(pdf, "photo.jpg"), Some(("application/pdf", "pdf")));
        assert_eq!(sniff_document(&[0xFF, 0xD8, 0xFF, 0xE0], "a.jpg"), Some(("image/jpeg", "jpg")));
        assert_eq!(sniff_document(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "a.png"),
                   Some(("image/png", "png")));
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0";
        assert_eq!(sniff_document(heic, "IMG_0042.jpg"), Some(("image/heic", "heic")));
        let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
        webp.extend_from_slice(b"VP8 ");
        assert_eq!(sniff_document(&webp, "a.webp"), Some(("image/webp", "webp")));
        // Word: the container gives nothing away, so the name decides — but only
        // between the document formats, never "anything in a zip"
        let docx = b"PK\x03\x04\x14\x00\x06\x00";
        assert!(sniff_document(docx, "form.docx").is_some_and(|(_, e)| e == "docx"));
        assert_eq!(sniff_document(docx, "backup.zip"), None);
        assert_eq!(sniff_document(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], "form.doc"),
                   Some(("application/msword", "doc")));
        // and the things a consent form is not
        for (bytes, name) in [(&b"\0asm\x01\0\0\0"[..], "x.wasm"), (b"<svg onload=alert(1)>", "x.svg"),
                              (b"MZ\x90\0", "x.exe"), (b"", "empty.pdf")] {
            assert_eq!(sniff_document(bytes, name), None, "{name}");
        }
    }

    /// Names go into a Content-Disposition header and into ZIP entry paths, so a
    /// student can't send one that escapes either.
    #[test]
    fn filenames_cannot_escape_a_header_or_a_folder() {
        assert_eq!(safe_filename("../../etc/passwd"), "etc_passwd");
        assert_eq!(safe_filename(r#"a"; filename="b.pdf"#), "a_; filename=_b.pdf");
        assert_eq!(safe_filename("veli\r\nonay.pdf"), "veli__onay.pdf");
        assert_eq!(safe_filename("   "), "belge");
        assert_eq!(safe_filename("...."), "belge");
        assert_eq!(safe_filename("İzin Formu.pdf"), "İzin Formu.pdf"); // Turkish survives
        assert_eq!(safe_filename(&"a".repeat(400)).chars().count(), 120);
        // the header itself is ASCII either way: the real name rides in filename*
        assert!(pct_encode("İzin Formu.pdf").is_ascii());
        assert_eq!(pct_encode("a b.pdf"), "a%20b.pdf");
    }

    /// The archive an admin downloads: it opens, `_EKSIKLER.txt` is in it, and every
    /// document is where the panel promises — form folder, student folder, numbered.
    #[test]
    fn documents_zip_round_trips() {
        let rows: Vec<DocRow> = vec![
            ("Ada Çelik".into(), "ada@x.com".into(), "exposure".into(), "sayfa1.pdf".into(), b"PDF-ONE".to_vec()),
            ("Ada Çelik".into(), "ada@x.com".into(), "exposure".into(), "sayfa2.pdf".into(), b"PDF-TWO".to_vec()),
            ("Bora Ay".into(), "bora@x.com".into(), "exposure".into(), "../evil.jpg".into(), b"JPG".to_vec()),
            ("Ada Çelik".into(), "ada@x.com".into(), "qnbeyond".into(), "izin.jpg".into(), b"QNB".to_vec()),
        ];
        let students = vec![("Ada Çelik".to_string(), "ada@x.com".to_string()),
                            ("Bora Ay".to_string(), "bora@x.com".to_string())];
        let summary = consent_summary(&rows, &students);
        assert!(summary.contains("[X] Ada Çelik <ada@x.com>"));
        assert!(summary.contains("[ ] Bora Ay <bora@x.com>"), "bora has no QNBEYOND form");
        assert!(summary.contains("Yükleyen: 2/2") && summary.contains("Yükleyen: 1/2"));

        let bytes = build_documents_zip(&rows, &summary).expect("zip built");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip opens");
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        for want in ["_EKSIKLER.txt",
                     "exposure/Ada Çelik/01-sayfa1.pdf",
                     "exposure/Ada Çelik/02-sayfa2.pdf",  // second page of the same form
                     "exposure/Bora Ay/01-evil.jpg",      // path traversal flattened
                     "qnbeyond/Ada Çelik/01-izin.jpg"] {  // numbering restarts per form
            assert!(names.contains(&want.to_string()), "{want} missing from {names:?}");
        }
        assert_eq!(names.len(), 5);
        let mut f = archive.by_name("exposure/Ada Çelik/02-sayfa2.pdf").expect("entry readable");
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut content).unwrap();
        assert_eq!(content, b"PDF-TWO", "bytes come back out unchanged");
    }

    #[test]
    fn internal_hosts_blocked_public_allowed() {
        for u in ["http://localhost/x", "http://127.0.0.1", "http://169.254.169.254/latest/meta-data",
                  "https://10.0.0.5", "http://192.168.1.1", "http://172.16.0.1", "http://[::1]/",
                  "http://metadata.google.internal", "not a url"] {
            assert!(is_internal_host(u), "{u} should be blocked");
        }
        for u in ["https://example.com", "https://ornek.vercel.app", "http://172.15.0.1", "http://172.32.0.1"] {
            assert!(!is_internal_host(u), "{u} should be allowed");
        }
    }
}
