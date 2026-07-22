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
        .route("/videos", get(video_grid))
        .route("/agentic-harness", get(agentic_harness))
        .route("/ai-monopoly", get(ai_monopoly))
        .route("/demos", get(demos))
        .route("/watch/{id}", get(watch))
        .route("/api/progress", post(progress))
        .route("/leaderboard", get(leaderboard))
        .route("/board", get(board))
        .route("/board/submit", post(board_submit).layer(DefaultBodyLimit::max(300 * 1024)))
        .route("/admin", get(admin_page))
        .route("/admin/video", post(admin_video))
        .route("/admin/video/level", post(admin_video_level))
        .route("/admin/video/delete", post(admin_video_delete))
        .route("/admin/task", post(admin_task))
        .route("/admin/task/edit", post(admin_task_edit))
        .route("/admin/task/example", post(admin_task_example))
        .route("/admin/task/level", post(admin_task_level))
        .route("/admin/task/delete", post(admin_task_delete))
        .route("/admin/user", post(admin_user))
        .route("/admin/review", post(admin_review))
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
/// positions 1..=8 are Seviye 1 (PRESEED), 9..=15 Seviye 2 (SEED). Insert-once by
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

    // `do nothing` on an existing email: a returning student (or one the admin added
    // by hand) just gets a login link, and their existing profile is left alone rather
    // than being overwritten by whoever typed their address.
    sqlx::query(
        "insert into users_exposure_academy (email, display_name, nickname, school, grade)
         values ($1,$2,$3,$4,$5)
         on conflict (email) do nothing")
        .bind(&email).bind(name).bind(&nickname).bind(school).bind(f.grade.trim())
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
    let rows = leader_rows(&app).await;
    let ranks = html::dense_ranks(&rows);
    let me = rows.iter().position(|r| r.id == user.id);
    let points = me.map(|i| rows[i].points()).unwrap_or(0);
    let rank = me.map(|i| ranks[i]);
    Ok(Html(html::home(&user, videos_done, videos_total, open_tasks, points, rank)))
}

async fn video_grid(State(app): State<App>, headers: HeaderMap, Query(q): Query<LevelQ>) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let level = q.level.as_deref().filter(|l| html::LEVELS.iter().any(|(k, _)| k == l));
    let videos = sqlx::query_as::<_, VideoWithProgress>(
        "select v.id, v.youtube_id, v.title, v.level,
                coalesce(w.max_position, 0) as max_position, coalesce(w.duration, 0) as duration
         from videos_exposure_academy v
         left join watch_progress_exposure_academy w on w.video_id = v.id and w.user_id = $1
         where ($2::text is null or v.level = $2)
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
    let rows = leader_rows(&app).await;
    Ok(Html(html::leaderboard(&user, &rows)))
}

/// The standings, ordered. Shared by /leaderboard and the Ana Sayfa summary card so
/// the two can never disagree about a student's points or place.
///
/// A video counts once it is ≥90% watched — same threshold the grid calls "Tamamlanmış".
/// A project counts once per task, and only when the submission passed, so resubmits
/// of the same task don't stack points.
async fn leader_rows(app: &App) -> Vec<LeaderRow> {
    sqlx::query_as::<_, LeaderRow>(
        "select u.id, u.nickname,
                coalesce(w.videos, 0) as videos, coalesce(p.projects, 0) as projects
         from users_exposure_academy u
         left join (select user_id, count(*) as videos
                    from watch_progress_exposure_academy
                    where duration > 0 and max_position >= duration * 0.9
                    group by user_id) w on w.user_id = u.id
         left join (select user_id, count(distinct task_id) as projects
                    from submissions_exposure_academy where status = 'passed'
                    group by user_id) p on p.user_id = u.id
         -- nickname is null until onboarding is done: a student appears on the board
         -- only once they have picked the handle the board is going to show
         where not u.is_admin and u.nickname is not null
         order by coalesce(w.videos,0) * $1 + coalesce(p.projects,0) * $2 desc, u.created_at")
        .bind(PTS_VIDEO).bind(PTS_PROJECT)
        .fetch_all(&app.pool).await.unwrap()
}

// ---- board ----

async fn board(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let subs = sqlx::query_as::<_, SubmissionView>(
        "select distinct on (s.task_id) s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url, s.plan_md,
                u.display_name, u.email, t.title as task_title, s.created_at
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         where s.user_id = $1 order by s.task_id, s.created_at desc")
        .bind(user.id).fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::board(&user, &tasks, &subs)))
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
                u.display_name, u.email, t.title as task_title, s.created_at
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         order by s.created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let videos = sqlx::query_as::<_, Video>("select id, youtube_id, title, level from videos_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level, example_url, example_embeddable from tasks_exposure_academy order by created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let invite_code = invite_code(&app).await;
    Ok(Html(html::admin(&user, &stats, &subs, &videos, &tasks, &invite_code, &app.base_url)))
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

/// GET the URL and decide whether it permits iframe embedding. Conservative:
/// any framing restriction, or a network error/timeout, counts as NOT embeddable
/// (so we fall back to a screenshot, which always renders).
async fn check_embeddable(client: &reqwest::Client, url: &str) -> bool {
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
    sqlx::query("insert into tasks_exposure_academy (title, description, level, example_url, example_embeddable) values ($1,$2,$3, nullif($4,''), $5)")
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
    let embeddable = if url.is_empty() { None } else { Some(check_embeddable(&app.http, url).await) };
    // saving an empty field removes the example
    sqlx::query("update tasks_exposure_academy set example_url = nullif($2,''), example_embeddable = $3 where id = $1")
        .bind(f.id).bind(url).bind(embeddable)
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
    // level is checked by the DB constraint; an invalid value just 400s
    sqlx::query("update tasks_exposure_academy set level = $2 where id = $1")
        .bind(f.id).bind(&f.level)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
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
struct UserForm { email: String, display_name: String }

async fn admin_user(State(app): State<App>, headers: HeaderMap, Form(f): Form<UserForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let email = f.email.trim().to_lowercase();
    sqlx::query("insert into users_exposure_academy (email, display_name) values ($1,$2) on conflict (email) do nothing")
        .bind(&email).bind(&f.display_name)
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
struct ReviewForm { id: Uuid, status: String, feedback: String }

async fn admin_review(State(app): State<App>, headers: HeaderMap, Form(f): Form<ReviewForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("update submissions_exposure_academy set status = $2, feedback = nullif($3,'') where id = $1")
        .bind(f.id).bind(&f.status).bind(&f.feedback)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

// ---- worker API (Phase 3 pipeline, see README) ----

fn check_worker(app: &App, headers: &HeaderMap) -> Result<(), Response> {
    let ok = !app.worker_token.is_empty()
        && headers.get("x-worker-token").and_then(|v| v.to_str().ok()) == Some(app.worker_token.as_str());
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
