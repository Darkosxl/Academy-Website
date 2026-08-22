// Exposure Academy Verified — standalone site (platform.exposureai.org). Admins post
// cases on behalf of a real submitter (client/mentor, own scoped login); Academy students
// submit work (links + files, multiple times) and ask public per-case questions the
// submitter (or an admin, as fallback) answers.
//
// Deliberately one file: this product is a fraction of Academy's surface area (no
// gamification, no leaderboard, no harness/monopoly), so splitting it into
// main/model/html/admin/auth modules the way services/academy does would be structure
// for structure's sake. ponytail: string templates, no template engine — same call as
// Academy's html.rs, at 1/20th the size.
//
// Shares Academy's Postgres project (same DATABASE_URL) so student eligibility is a plain
// `select ... from users_exposure_academy` — no sync job, no second roster to keep in
// sync. Everything this service owns lives in its own verified_* tables (see
// migrations/001_init.sql); nothing here ever writes to an *_exposure_academy table.

use axum::{
    Form, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::Deserialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

// ---------------------------------------------------------------------------------
// App state, startup
// ---------------------------------------------------------------------------------

#[derive(Clone)]
struct App {
    pool: PgPool,
    http: reqwest::Client,
    resend_key: String,
    mail_from: String,
    base_url: String,
    supabase_url: String,
    supabase_service_key: String,
    storage_bucket: String,
    max_file_bytes: u64,
    max_submission_bytes: u64,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing (.env)");
    // Same Supabase session pooler academy uses — sqlx needs prepared statements, which
    // the transaction pooler (6543) can't do.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("db connect failed");

    sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
        .execute(&pool)
        .await
        .expect("migration failed");
    sqlx::raw_sql(include_str!("../migrations/002_avatars_and_documents.sql"))
        .execute(&pool)
        .await
        .expect("avatars/documents migration failed");

    seed_admin(&pool).await;

    let _ = sqlx::query(
        "delete from verified_magic_links where expires_at < now() - interval '1 day'",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query("delete from verified_sessions where expires_at < now()")
        .execute(&pool)
        .await;

    let max_file_bytes: u64 = std::env::var("VERIFIED_MAX_FILE_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100 * 1024 * 1024);
    let max_submission_bytes: u64 = std::env::var("VERIFIED_MAX_SUBMISSION_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500 * 1024 * 1024);

    let app = App {
        pool,
        http: reqwest::Client::builder()
            .user_agent("exposure-academy-verified")
            .build()
            .expect("http client"),
        resend_key: std::env::var("RESEND_API_KEY").expect("RESEND_API_KEY missing (.env)"),
        mail_from: std::env::var("MAIL_FROM").expect("MAIL_FROM missing (.env)"),
        base_url: std::env::var("APP_BASE_URL").expect("APP_BASE_URL missing (.env)"),
        supabase_url: std::env::var("SUPABASE_URL").expect("SUPABASE_URL missing (.env)"),
        supabase_service_key: std::env::var("SUPABASE_SECRET_KEY")
            .expect("SUPABASE_SECRET_KEY missing (.env)"),
        storage_bucket: std::env::var("VERIFIED_STORAGE_BUCKET")
            .unwrap_or_else(|_| "verified-submissions".to_string()),
        max_file_bytes,
        max_submission_bytes,
    };

    let router = Router::new()
        .route("/", get(landing))
        .route("/login", get(login_page).post(login_post))
        .route("/magic/{token}", get(magic_consume))
        .route("/logout", post(logout))
        .route("/cases", get(cases_list))
        .route("/cases/{id}", get(case_detail))
        .route(
            "/cases/{id}/submit",
            post(case_submit).layer(DefaultBodyLimit::max(
                (max_submission_bytes + 2 * 1024 * 1024) as usize,
            )),
        )
        .route("/cases/{id}/ask", post(case_ask))
        .route(
            "/cases/{id}/documents",
            post(case_document_upload).layer(DefaultBodyLimit::max(
                (max_file_bytes + 2 * 1024 * 1024) as usize,
            )),
        )
        .route("/submitter", get(submitter_dashboard))
        .route("/submitter/cases/{id}", get(submitter_case_view))
        .route("/questions/{id}/answer", post(question_answer))
        .route("/admin", get(admin_dashboard))
        .route("/admin/cases/new", get(admin_case_new))
        .route("/admin/cases", post(admin_case_create))
        .route("/admin/cases/{id}", get(admin_case_manage))
        .route(
            "/admin/cases/{id}/edit",
            get(admin_case_edit_page).post(admin_case_edit_post),
        )
        .route("/admin/cases/{id}/hide", post(admin_case_hide_toggle))
        .route(
            "/admin/cases/{id}/submitter-photo",
            post(admin_submitter_photo).layer(DefaultBodyLimit::max(
                (max_file_bytes + 2 * 1024 * 1024) as usize,
            )),
        )
        .route(
            "/admin/admins",
            get(admin_admins_page).post(admin_admins_post),
        )
        .route("/files/{id}", get(file_download))
        .route("/case-files/{id}", get(case_file_download))
        .layer(middleware::from_fn_with_state(app.clone(), rolling_session))
        .route("/avatar/{id}", get(avatar_get))
        .route("/style.css", get(style_css))
        .with_state(app);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:3100".into());
    println!("verified listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

async fn seed_admin(pool: &PgPool) {
    let Ok(email) = std::env::var("ADMIN_EMAIL") else {
        return;
    };
    let email = email.trim().to_lowercase();
    let existing: Option<(String,)> =
        sqlx::query_as("select role from verified_users where email = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await
            .unwrap();
    match existing {
        None => {
            sqlx::query(
                "insert into verified_users (email, display_name, role) values ($1,$2,'admin')",
            )
            .bind(&email)
            .bind(&email)
            .execute(pool)
            .await
            .unwrap();
            println!("verified admin '{email}' seeded");
        }
        Some((role,)) if role != "admin" => {
            sqlx::query("update verified_users set role = 'admin' where email = $1")
                .bind(&email)
                .execute(pool)
                .await
                .unwrap();
        }
        Some(_) => {}
    }
}

fn random_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------------

#[derive(sqlx::FromRow, Clone)]
struct VUser {
    id: Uuid,
    email: String,
    display_name: String,
    role: String,
    linkedin_url: Option<String>,
    has_avatar: bool,
}

impl VUser {
    fn home(&self) -> &'static str {
        role_home(&self.role)
    }
}

fn role_home(role: &str) -> &'static str {
    match role {
        "admin" => "/admin",
        "submitter" => "/submitter",
        _ => "/cases",
    }
}

#[derive(sqlx::FromRow, Clone)]
struct VCase {
    id: Uuid,
    title: String,
    description: String,
    submitter_id: Uuid,
    hidden: bool,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct VLink {
    url: String,
}

#[derive(sqlx::FromRow)]
struct VFile {
    id: Uuid,
    filename: String,
    size_bytes: i64,
}

#[derive(sqlx::FromRow)]
struct VCaseDoc {
    id: Uuid,
    filename: String,
    size_bytes: i64,
}

#[derive(sqlx::FromRow)]
struct SubmissionRow {
    id: Uuid,
    student_id: Uuid,
    student_name: String,
    student_linkedin: Option<String>,
    student_has_avatar: bool,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct QuestionRow {
    id: Uuid,
    student_name: String,
    body: String,
    created_at: DateTime<Utc>,
    answer_body: Option<String>,
    answered_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------------
// Auth: sessions, magic links, request guards
// ---------------------------------------------------------------------------------

const SESSION_DAYS: i64 = 30;
const SESSION_MAX_AGE: i64 = SESSION_DAYS * 24 * 60 * 60;
const SESSION_REFRESH_BELOW_DAYS: i64 = SESSION_DAYS - 1;

fn session_cookie(token: &str) -> String {
    format!("vsession={token}; HttpOnly; Secure; Path=/; Max-Age={SESSION_MAX_AGE}; SameSite=Lax")
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|c| c.strip_prefix("vsession=").map(String::from))
}

async fn current_user(app: &App, headers: &HeaderMap) -> Option<VUser> {
    let token = cookie_token(headers)?;
    sqlx::query_as::<_, VUser>(
        "select u.id, u.email, u.display_name, u.role, u.linkedin_url, (u.avatar is not null) as has_avatar
         from verified_sessions s join verified_users u on u.id = s.user_id
         where s.token = $1 and s.expires_at > now()",
    )
    .bind(token)
    .fetch_optional(&app.pool)
    .await
    .ok()?
}

async fn issue_session(app: &App, uid: Uuid, role: &str) -> Response {
    let token = random_token();
    sqlx::query("insert into verified_sessions (token, user_id, expires_at) values ($1,$2, now() + make_interval(days => $3))")
        .bind(&token).bind(uid).bind(SESSION_DAYS as i32).execute(&app.pool).await.unwrap();
    (
        [(header::SET_COOKIE, session_cookie(&token))],
        Redirect::to(role_home(role)),
    )
        .into_response()
}

// Same rolling-window reasoning as academy/src/auth.rs: extend the DB row and the
// cookie together, after the handler runs, so /logout's delete always wins.
async fn rolling_session(
    State(app): State<App>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let token = cookie_token(req.headers());
    let mut res = next.run(req).await;
    let Some(token) = token else { return res };

    let rolled: Option<(Uuid,)> = sqlx::query_as(
        "update verified_sessions set expires_at = now() + make_interval(days => $2)
         where token = $1 and expires_at > now() and expires_at < now() + make_interval(days => $3)
         returning user_id",
    )
    .bind(&token)
    .bind(SESSION_DAYS as i32)
    .bind(SESSION_REFRESH_BELOW_DAYS as i32)
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten();

    if rolled.is_some() {
        if let Ok(v) = HeaderValue::from_str(&session_cookie(&token)) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

fn require(user: Option<VUser>) -> Result<VUser, Response> {
    user.ok_or_else(|| Redirect::to("/login").into_response())
}

fn require_role(user: Option<VUser>, role: &str) -> Result<VUser, Response> {
    match user {
        Some(u) if u.role == role => Ok(u),
        Some(_) => Err(StatusCode::FORBIDDEN.into_response()),
        None => Err(Redirect::to("/login").into_response()),
    }
}

async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&headers) {
        let _ = sqlx::query("delete from verified_sessions where token = $1")
            .bind(t)
            .execute(&app.pool)
            .await;
    }
    (
        [(
            header::SET_COOKIE,
            "vsession=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax".to_string(),
        )],
        Redirect::to("/"),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------
// Email (Resend — same account/mechanism as academy/src/auth.rs)
// ---------------------------------------------------------------------------------

async fn send_email(app: &App, to: &str, subject: &str, html: String) {
    let from = if app.mail_from.contains('<') {
        app.mail_from.clone()
    } else {
        format!("Exposure Academy Verified <{}>", app.mail_from)
    };
    let body = serde_json::json!({ "from": from, "to": [to], "subject": subject, "html": html });
    if let Err(e) = app
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&app.resend_key)
        .json(&body)
        .send()
        .await
    {
        eprintln!("resend send failed: {e}");
    }
}

async fn send_login_link(app: &App, email: &str) {
    let recent: Option<(i32,)> = sqlx::query_as(
        "select 1 from verified_magic_links where email = $1 and used_at is null and created_at > now() - interval '60 seconds'")
        .bind(email).fetch_optional(&app.pool).await.unwrap();
    if recent.is_some() {
        return;
    }
    let token = random_token();
    sqlx::query("insert into verified_magic_links (token, email, expires_at) values ($1,$2, now() + interval '15 minutes')")
        .bind(&token).bind(email).execute(&app.pool).await.unwrap();
    let link = format!("{}/magic/{}", app.base_url, token);
    let html = format!(
        r#"<p>Sign in to Exposure Academy Verified:</p><p><a href="{link}">{link}</a></p><p>This link expires in 15 minutes and works once.</p>"#
    );
    send_email(app, email, "Your Verified sign-in link", html).await;
}

// Emails the case's submitter AND every admin — the submitter is who's expected to
// answer, admins are cc'd so nothing sits unanswered if the submitter is slow.
async fn notify_submitter_of_question(app: &App, submitter_email: &str, case: &VCase) {
    let submitter_url = format!("{}/submitter/cases/{}", app.base_url, case.id);
    let html = format!(
        r#"<p>A student asked a question on <strong>{}</strong>.</p><p><a href="{submitter_url}">{submitter_url}</a></p>"#,
        esc(&case.title)
    );
    send_email(
        app,
        submitter_email,
        "New question on your case — Exposure Academy Verified",
        html,
    )
    .await;

    let admin_url = format!("{}/admin/cases/{}", app.base_url, case.id);
    let admin_html = format!(
        r#"<p>A student asked a question on <strong>{}</strong>.</p><p><a href="{admin_url}">{admin_url}</a></p>"#,
        esc(&case.title)
    );
    let admins: Vec<(String,)> = sqlx::query_as("select email from verified_users where role = 'admin'")
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default();
    for (email,) in admins {
        send_email(
            app,
            &email,
            "New question on a case — Exposure Academy Verified",
            admin_html.clone(),
        )
        .await;
    }
}

// Same "submitter + every admin" fan-out as new-question notifications, fired after a
// student submits work on a case.
async fn notify_new_submission(app: &App, submitter_email: &str, case: &VCase) {
    let submitter_url = format!("{}/submitter/cases/{}", app.base_url, case.id);
    let html = format!(
        r#"<p>A student submitted work on <strong>{}</strong>.</p><p><a href="{submitter_url}">{submitter_url}</a></p>"#,
        esc(&case.title)
    );
    send_email(
        app,
        submitter_email,
        "New submission on your case — Exposure Academy Verified",
        html,
    )
    .await;

    let admin_url = format!("{}/admin/cases/{}", app.base_url, case.id);
    let admin_html = format!(
        r#"<p>A student submitted work on <strong>{}</strong>.</p><p><a href="{admin_url}">{admin_url}</a></p>"#,
        esc(&case.title)
    );
    let admins: Vec<(String,)> = sqlx::query_as("select email from verified_users where role = 'admin'")
        .fetch_all(&app.pool)
        .await
        .unwrap_or_default();
    for (email,) in admins {
        send_email(
            app,
            &email,
            "New submission on a case — Exposure Academy Verified",
            admin_html.clone(),
        )
        .await;
    }
}

// ---------------------------------------------------------------------------------
// Object storage (Supabase Storage REST API, via reqwest — no bytea-in-Postgres: see
// migrations/001_init.sql's comment on verified_submission_files for why)
// ---------------------------------------------------------------------------------

async fn storage_put(app: &App, key: &str, bytes: Vec<u8>, content_type: &str) -> Result<(), String> {
    let url = format!(
        "{}/storage/v1/object/{}/{}",
        app.supabase_url.trim_end_matches('/'),
        app.storage_bucket,
        key
    );
    let res = app
        .http
        .post(&url)
        .header("apikey", &app.supabase_service_key)
        .bearer_auth(&app.supabase_service_key)
        .header(header::CONTENT_TYPE, content_type)
        .body(bytes)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("storage upload failed: {}", res.status()));
    }
    Ok(())
}

async fn storage_get(app: &App, key: &str) -> Result<Vec<u8>, String> {
    let url = format!(
        "{}/storage/v1/object/{}/{}",
        app.supabase_url.trim_end_matches('/'),
        app.storage_bucket,
        key
    );
    let res = app
        .http
        .get(&url)
        .header("apikey", &app.supabase_service_key)
        .bearer_auth(&app.supabase_service_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("storage download failed: {}", res.status()));
    }
    res.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------
// Small pure helpers (unit-tested at the bottom)
// ---------------------------------------------------------------------------------

const MAX_LINKS: usize = 5;
const MAX_FILES: usize = 10;

fn valid_url(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        Some(s.to_string())
    } else {
        None
    }
}

fn safe_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.chars().take(180).collect()
    }
}

/// Checked once caps are known, before any network call — the multipart handler stops
/// reading further fields as soon as a cap is hit, so this only re-validates counts.
fn check_caps(link_count: usize, file_count: usize) -> Result<(), &'static str> {
    if link_count > MAX_LINKS {
        return Err("En fazla 5 link ekleyebilirsin.");
    }
    if file_count > MAX_FILES {
        return Err("En fazla 10 dosya ekleyebilirsin.");
    }
    Ok(())
}

fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn attachment(filename: &str, content_type: &str, bytes: Vec<u8>) -> Response {
    let name = safe_filename(filename);
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
                    pct_encode(&name)
                ),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        bytes,
    )
        .into_response()
}

// ---------------------------------------------------------------------------------
// Auth routes
// ---------------------------------------------------------------------------------

async fn landing(State(app): State<App>, headers: HeaderMap) -> Response {
    match current_user(&app, &headers).await {
        Some(u) => Redirect::to(u.home()).into_response(),
        None => Redirect::to("/login").into_response(),
    }
}

async fn login_page(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(u) = current_user(&app, &headers).await {
        return Redirect::to(u.home()).into_response();
    }
    Html(page("Sign in", None, login_body(None))).into_response()
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
}

const CHECK_EMAIL_MSG: &str = "If that email is registered, a sign-in link is on its way.";

async fn login_post(State(app): State<App>, Form(f): Form<LoginForm>) -> Response {
    let email = f.email.trim().to_lowercase();
    let known: Option<(String,)> =
        sqlx::query_as("select role from verified_users where email = $1")
            .bind(&email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    if known.is_some() {
        send_login_link(&app, &email).await;
    } else {
        // Not seen on Verified yet — eligible only if they're already an Academy student.
        let student: Option<(String, Option<String>)> = sqlx::query_as(
            "select coalesce(nickname, display_name), linkedin_url from users_exposure_academy where email = $1",
        )
        .bind(&email)
        .fetch_optional(&app.pool)
        .await
        .unwrap();
        if let Some((name, linkedin)) = student {
            sqlx::query(
                "insert into verified_users (email, display_name, role, linkedin_url) values ($1,$2,'student',$3) on conflict (email) do nothing",
            )
            .bind(&email)
            .bind(&name)
            .bind(&linkedin)
            .execute(&app.pool)
            .await
            .unwrap();
            send_login_link(&app, &email).await;
        }
    }
    // Same response whether or not the email is eligible — avoids account enumeration.
    Html(page("Sign in", None, login_body(Some(CHECK_EMAIL_MSG)))).into_response()
}

async fn magic_consume(State(app): State<App>, Path(token): Path<String>) -> Response {
    let row: Option<(String,)> = sqlx::query_as(
        "update verified_magic_links set used_at = now()
         where token = $1 and used_at is null and expires_at > now()
         returning email",
    )
    .bind(&token)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((email,)) = row else {
        return Html(page(
            "Sign in",
            None,
            login_body(Some("That link is invalid or expired, try again.")),
        ))
        .into_response();
    };
    let user: Option<(Uuid, String)> =
        sqlx::query_as("select id, role from verified_users where email = $1")
            .bind(&email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    let Some((uid, role)) = user else {
        return Html(page("Sign in", None, login_body(Some("Account not found.")))).into_response();
    };
    issue_session(&app, uid, &role).await
}

// ---------------------------------------------------------------------------------
// Student routes
// ---------------------------------------------------------------------------------

async fn cases_list(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "student")?;
    let cases: Vec<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at
         from verified_cases where hidden = false order by created_at desc",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut rows = String::new();
    for c in &cases {
        let submitted: (i64,) = sqlx::query_as(
            "select count(*) from verified_submissions where case_id = $1 and student_id = $2",
        )
        .bind(c.id)
        .bind(user.id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
        rows.push_str(&format!(
            r#"<a class="case-card" href="/cases/{id}">
                 <h3>{title}</h3>
                 <p class="muted">{snippet}</p>
                 {badge}
               </a>"#,
            id = c.id,
            title = esc(&c.title),
            snippet = esc(&snippet(&c.description, 160)),
            badge = if submitted.0 > 0 {
                r#"<span class="badge">You've submitted</span>"#
            } else {
                ""
            },
        ));
    }
    if cases.is_empty() {
        rows.push_str(r#"<p class="muted">No open cases yet — check back soon.</p>"#);
    }
    Ok(Html(page(
        "Cases",
        Some(&user),
        format!(r#"<h1>Open cases</h1><div class="grid">{rows}</div>"#),
    )))
}

fn snippet(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

async fn case_detail(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "student")?;
    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at
         from verified_cases where id = $1 and hidden = false",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(case) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    let submitter: VUser = sqlx::query_as(
        "select id, email, display_name, role, linkedin_url, (avatar is not null) as has_avatar from verified_users where id = $1",
    )
    .bind(case.submitter_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let questions: Vec<QuestionRow> = sqlx::query_as(
        "select q.id, u.display_name as student_name, q.body, q.created_at, q.answer_body, q.answered_at
         from verified_questions q join verified_users u on u.id = q.student_id
         where q.case_id = $1 order by q.created_at",
    )
    .bind(case.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let my_submissions: Vec<SubmissionRow> = sqlx::query_as(
        "select s.id, s.student_id, u.display_name as student_name, u.linkedin_url as student_linkedin,
                (u.avatar is not null) as student_has_avatar, s.note, s.created_at
         from verified_submissions s join verified_users u on u.id = s.student_id
         where s.case_id = $1 and s.student_id = $2 order by s.created_at desc",
    )
    .bind(case.id)
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut my_submissions_html = String::new();
    for s in &my_submissions {
        my_submissions_html.push_str(&submission_card(&app, s, false).await);
    }

    let documents: Vec<VCaseDoc> = sqlx::query_as(
        "select id, filename, size_bytes from verified_case_documents where case_id = $1 order by position",
    )
    .bind(case.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let documents_html = case_documents_html(case.id, &documents, false);

    Ok(Html(page(
        &case.title,
        Some(&user),
        case_detail_body(&case, &submitter, &questions, &my_submissions_html, &documents_html, false),
    )))
}

fn case_detail_body(
    case: &VCase,
    submitter: &VUser,
    questions: &[QuestionRow],
    my_submissions_html: &str,
    documents_html: &str,
    manage_toolbar: bool,
) -> String {
    let submitter_chip = avatar_chip(
        submitter.id,
        &submitter.display_name,
        submitter.linkedin_url.as_deref(),
        submitter.has_avatar,
        false,
    );

    let mut qa = String::new();
    for q in questions {
        let answer = match &q.answer_body {
            Some(a) => format!(
                r#"<div class="answer"><strong>Answer</strong> <span class="muted">({when})</span>: {body}</div>"#,
                when = q
                    .answered_at
                    .map(|t| t.format("%d %b %Y %H:%M").to_string())
                    .unwrap_or_default(),
                body = nl2br(a)
            ),
            None => format!(
                r#"<div class="answer pending">Not answered yet.</div>
                   {}"#,
                if manage_toolbar {
                    format!(
                        r#"<form method="post" action="/questions/{}/answer" class="inline-form">
                             <textarea name="body" placeholder="Write an answer…" required></textarea>
                             <button type="submit">Answer</button>
                           </form>"#,
                        q.id
                    )
                } else {
                    String::new()
                }
            ),
        };
        qa.push_str(&format!(
            r#"<div class="qa-item">
                 <p><strong>{name}</strong> asked <span class="muted">({when})</span>: {body}</p>
                 {answer}
               </div>"#,
            name = esc(&q.student_name),
            when = q.created_at.format("%d %b %Y %H:%M"),
            body = nl2br(&q.body),
        ));
    }
    if questions.is_empty() {
        qa.push_str(r#"<p class="muted">No questions yet.</p>"#);
    }

    let ask_form = if manage_toolbar {
        String::new()
    } else {
        format!(
            r#"<form method="post" action="/cases/{}/ask" class="stack">
                 <textarea name="body" placeholder="Ask the case owner a question…" required></textarea>
                 <button type="submit">Ask</button>
               </form>"#,
            case.id
        )
    };

    let submit_form = if manage_toolbar {
        String::new()
    } else {
        submission_form(case.id)
    };

    let history_heading = if manage_toolbar {
        "All submissions"
    } else {
        "Your submissions"
    };

    format!(
        r#"<h1>{title}</h1>
           {submitter_chip}
           <div class="prose">{desc}</div>
           {documents_html}

           <section>
             <h2>Ask a question</h2>
             {qa}
             {ask_form}
           </section>

           <section>
             <h2>{history_heading}</h2>
             {my_submissions_html}
             {submit_form}
           </section>"#,
        title = esc(&case.title),
        desc = nl2br(&case.description),
    )
}

// Case-level reference documents: every viewer can see/download them, only the
// submitter or an admin (i.e. whoever gets the manage toolbar) can upload more.
fn case_documents_html(case_id: Uuid, docs: &[VCaseDoc], can_upload: bool) -> String {
    let list: String = docs
        .iter()
        .map(|d| {
            format!(
                r#"<li><a href="/case-files/{id}">{name}</a> ({kb} KB)</li>"#,
                id = d.id,
                name = esc(&d.filename),
                kb = d.size_bytes / 1024,
            )
        })
        .collect();
    let list_html = if docs.is_empty() && !can_upload {
        String::new()
    } else {
        format!(
            r#"<section>
                 <h2>Documents</h2>
                 <ul>{list}</ul>
                 {upload}
               </section>"#,
            list = list,
            upload = if can_upload {
                format!(
                    r#"<form method="post" action="/cases/{case_id}/documents" enctype="multipart/form-data" class="inline-form">
                         <input type="file" name="file" required>
                         <button type="submit">Upload document</button>
                       </form>"#
                )
            } else {
                String::new()
            },
        )
    };
    list_html
}

fn submission_form(case_id: Uuid) -> String {
    format!(
        r#"<form method="post" action="/cases/{case_id}/submit" enctype="multipart/form-data" class="stack">
             <label>Note (optional)<textarea name="note"></textarea></label>
             <div id="link-fields"><label>Link<input type="url" name="link" placeholder="https://…"></label></div>
             <button type="button" onclick="addField('link-fields','link','url','Link',{max_links})">+ Add another link</button>
             <div id="file-fields"><label>File<input type="file" name="file"></label></div>
             <button type="button" onclick="addField('file-fields','file','file','File',{max_files})">+ Add another file</button>
             <button type="submit">Submit</button>
           </form>
           <script>
           function addField(containerId, name, type, label, max) {{
             var c = document.getElementById(containerId);
             if (c.children.length >= max) return;
             var wrap = document.createElement('label');
             wrap.textContent = label;
             var input = document.createElement('input');
             input.type = type; input.name = name;
             wrap.appendChild(input);
             c.appendChild(wrap);
           }}
           </script>"#,
        max_links = MAX_LINKS,
        max_files = MAX_FILES,
    )
}

async fn submission_card(app: &App, s: &SubmissionRow, show_student: bool) -> String {
    let links: Vec<VLink> = sqlx::query_as(
        "select url from verified_submission_links where submission_id = $1 order by position",
    )
    .bind(s.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let files: Vec<VFile> = sqlx::query_as(
        "select id, filename, size_bytes from verified_submission_files where submission_id = $1 order by position",
    )
    .bind(s.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let links_html: String = links
        .iter()
        .map(|l| format!(r#"<li><a href="{u}" target="_blank" rel="noopener">{u}</a></li>"#, u = esc(&l.url)))
        .collect();
    let files_html: String = files
        .iter()
        .map(|f| {
            format!(
                r#"<li><a href="/files/{id}">{name}</a> ({kb} KB)</li>"#,
                id = f.id,
                name = esc(&f.filename),
                kb = f.size_bytes / 1024,
            )
        })
        .collect();
    let note_html = s
        .note
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .map(|n| format!("<p>{}</p>", nl2br(n)))
        .unwrap_or_default();
    let who = if show_student {
        avatar_chip(s.student_id, &s.student_name, s.student_linkedin.as_deref(), s.student_has_avatar, true)
    } else {
        String::new()
    };

    format!(
        r#"<div class="submission-card">
             {who}
             <div class="submission-body">
               <p class="muted">{when}</p>
               {note}
               <ul>{links}{files}</ul>
             </div>
           </div>"#,
        who = who,
        when = s.created_at.format("%d %b %Y %H:%M"),
        note = note_html,
        links = links_html,
        files = files_html,
    )
}

#[derive(Deserialize)]
struct AskForm {
    body: String,
}

async fn case_ask(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Form(f): Form<AskForm>,
) -> Result<Redirect, Response> {
    let user = require_role(current_user(&app, &headers).await, "student")?;
    let body = f.body.trim();
    if body.is_empty() {
        return Ok(Redirect::to(&format!("/cases/{id}")));
    }
    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at from verified_cases where id = $1 and hidden = false",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(case) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    sqlx::query("insert into verified_questions (case_id, student_id, body) values ($1,$2,$3)")
        .bind(case.id)
        .bind(user.id)
        .bind(body)
        .execute(&app.pool)
        .await
        .unwrap();
    let submitter_email: (String,) =
        sqlx::query_as("select email from verified_users where id = $1")
            .bind(case.submitter_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    notify_submitter_of_question(&app, &submitter_email.0, &case).await;
    Ok(Redirect::to(&format!("/cases/{id}")))
}

async fn case_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    mut mp: Multipart,
) -> Result<Response, Response> {
    let user = require_role(current_user(&app, &headers).await, "student")?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at from verified_cases where id = $1 and hidden = false",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(case) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };

    let mut note: Option<String> = None;
    let mut links: Vec<String> = Vec::new();
    let mut files: Vec<(String, String, Vec<u8>)> = Vec::new();
    let mut total_bytes: u64 = 0;

    while let Some(field) = mp.next_field().await.map_err(|_| bad("Could not read form."))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "note" => {
                let t = field.text().await.unwrap_or_default();
                if !t.trim().is_empty() {
                    note = Some(t.trim().to_string());
                }
            }
            "link" => {
                let t = field.text().await.unwrap_or_default();
                if let Some(u) = valid_url(&t) {
                    if links.len() >= MAX_LINKS {
                        return Err(bad("At most 5 links."));
                    }
                    links.push(u);
                }
            }
            "file" => {
                let filename = field.file_name().unwrap_or("file").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = field.bytes().await.map_err(|_| bad("Could not read a file."))?;
                if bytes.is_empty() {
                    continue;
                }
                if files.len() >= MAX_FILES {
                    return Err(bad("At most 10 files."));
                }
                if bytes.len() as u64 > app.max_file_bytes {
                    return Err(bad(&format!(
                        "A single file can't exceed {} MB.",
                        app.max_file_bytes / 1024 / 1024
                    )));
                }
                total_bytes += bytes.len() as u64;
                if total_bytes > app.max_submission_bytes {
                    return Err(bad(&format!(
                        "Total upload can't exceed {} MB.",
                        app.max_submission_bytes / 1024 / 1024
                    )));
                }
                files.push((safe_filename(&filename), content_type, bytes.to_vec()));
            }
            _ => {}
        }
    }

    if let Err(msg) = check_caps(links.len(), files.len()) {
        return Err(bad(msg));
    }
    if links.is_empty() && files.is_empty() {
        return Err(bad("Add at least one link or file."));
    }

    let submission_id: (Uuid,) = sqlx::query_as(
        "insert into verified_submissions (case_id, student_id, note) values ($1,$2,$3) returning id",
    )
    .bind(case.id)
    .bind(user.id)
    .bind(&note)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let submission_id = submission_id.0;

    for (i, url) in links.iter().enumerate() {
        sqlx::query(
            "insert into verified_submission_links (submission_id, url, position) values ($1,$2,$3)",
        )
        .bind(submission_id)
        .bind(url)
        .bind(i as i32)
        .execute(&app.pool)
        .await
        .unwrap();
    }

    // Uploaded one at a time, not in a transaction with the DB rows: a storage failure
    // partway through leaves a submission with fewer files attached rather than none at
    // all, which is the safer half-failure to have. Not expected to happen in practice.
    for (i, (filename, content_type, bytes)) in files.into_iter().enumerate() {
        let file_id = Uuid::new_v4();
        let key = format!("{submission_id}/{file_id}-{filename}");
        let size = bytes.len() as i64;
        if let Err(e) = storage_put(&app, &key, bytes, &content_type).await {
            eprintln!("storage upload failed for {key}: {e}");
            continue;
        }
        sqlx::query(
            "insert into verified_submission_files (id, submission_id, filename, content_type, storage_key, size_bytes, position)
             values ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(file_id)
        .bind(submission_id)
        .bind(&filename)
        .bind(&content_type)
        .bind(&key)
        .bind(size)
        .bind(i as i32)
        .execute(&app.pool)
        .await
        .unwrap();
    }

    let submitter_email: (String,) =
        sqlx::query_as("select email from verified_users where id = $1")
            .bind(case.submitter_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    notify_new_submission(&app, &submitter_email.0, &case).await;

    Ok(Redirect::to(&format!("/cases/{id}")).into_response())
}

// ---------------------------------------------------------------------------------
// Submitter routes
// ---------------------------------------------------------------------------------

async fn submitter_dashboard(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "submitter")?;
    let cases: Vec<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at
         from verified_cases where submitter_id = $1 order by created_at desc",
    )
    .bind(user.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut rows = String::new();
    for c in &cases {
        let unanswered: (i64,) = sqlx::query_as(
            "select count(*) from verified_questions where case_id = $1 and answer_body is null",
        )
        .bind(c.id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
        rows.push_str(&format!(
            r#"<a class="case-card" href="/submitter/cases/{id}">
                 <h3>{title}</h3>
                 {badge}
               </a>"#,
            id = c.id,
            title = esc(&c.title),
            badge = if unanswered.0 > 0 {
                format!(r#"<span class="badge warn">{} unanswered question(s)</span>"#, unanswered.0)
            } else {
                String::new()
            },
        ));
    }
    if cases.is_empty() {
        rows.push_str(r#"<p class="muted">No cases assigned to you yet.</p>"#);
    }
    Ok(Html(page(
        "Your cases",
        Some(&user),
        format!(r#"<h1>Your cases</h1><div class="grid">{rows}</div>"#),
    )))
}

async fn submitter_case_view(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "submitter")?;
    let case = load_case_owned_by(&app, id, user.id).await?;
    Ok(Html(page(
        &case.title,
        Some(&user),
        render_case_manage(&app, &case, &user, false).await,
    )))
}

async fn load_case_owned_by(app: &App, id: Uuid, submitter_id: Uuid) -> Result<VCase, Response> {
    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at from verified_cases where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    match case {
        Some(c) if c.submitter_id == submitter_id => Ok(c),
        Some(_) => Err(StatusCode::FORBIDDEN.into_response()),
        None => Err(StatusCode::NOT_FOUND.into_response()),
    }
}

/// Shared render for the submitter's own case view and the admin's manage view — same
/// content (every submission across every student, plus the answerable Q&A thread), the
/// admin gets an extra edit/hide toolbar up top.
async fn render_case_manage(app: &App, case: &VCase, _viewer: &VUser, is_admin: bool) -> String {
    let submitter: VUser = sqlx::query_as(
        "select id, email, display_name, role, linkedin_url, (avatar is not null) as has_avatar from verified_users where id = $1",
    )
    .bind(case.submitter_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let submissions: Vec<SubmissionRow> = sqlx::query_as(
        "select s.id, s.student_id, u.display_name as student_name, u.linkedin_url as student_linkedin,
                (u.avatar is not null) as student_has_avatar, s.note, s.created_at
         from verified_submissions s join verified_users u on u.id = s.student_id
         where s.case_id = $1 order by s.created_at desc",
    )
    .bind(case.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut submissions_html = String::new();
    for s in &submissions {
        submissions_html.push_str(&submission_card(app, s, true).await);
    }
    if submissions.is_empty() {
        submissions_html.push_str(r#"<p class="muted">No submissions yet.</p>"#);
    }

    let questions: Vec<QuestionRow> = sqlx::query_as(
        "select q.id, u.display_name as student_name, q.body, q.created_at, q.answer_body, q.answered_at
         from verified_questions q join verified_users u on u.id = q.student_id
         where q.case_id = $1 order by q.created_at",
    )
    .bind(case.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let toolbar = if is_admin {
        format!(
            r#"<div class="toolbar">
                 <a href="/admin/cases/{id}/edit">Edit</a>
                 <form method="post" action="/admin/cases/{id}/hide" class="inline-form">
                   <button type="submit">{hide_label}</button>
                 </form>
               </div>"#,
            id = case.id,
            hide_label = if case.hidden { "Unhide" } else { "Hide" },
        )
    } else {
        String::new()
    };

    let documents: Vec<VCaseDoc> = sqlx::query_as(
        "select id, filename, size_bytes from verified_case_documents where case_id = $1 order by position",
    )
    .bind(case.id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let documents_html = case_documents_html(case.id, &documents, true);

    let body = case_detail_body(case, &submitter, &questions, &submissions_html, &documents_html, true);
    format!("{toolbar}{body}")
}

#[derive(Deserialize)]
struct AnswerForm {
    body: String,
}

async fn question_answer(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Form(f): Form<AnswerForm>,
) -> Result<Redirect, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let body = f.body.trim();
    if body.is_empty() {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "select q.case_id, c.submitter_id from verified_questions q
         join verified_cases c on c.id = q.case_id where q.id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((case_id, submitter_id)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    let authorized = user.role == "admin" || user.id == submitter_id;
    if !authorized {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    sqlx::query(
        "update verified_questions set answer_body = $2, answered_by = $3, answered_at = now() where id = $1",
    )
    .bind(id)
    .bind(body)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .unwrap();
    let dest = match user.role.as_str() {
        "admin" => format!("/admin/cases/{case_id}"),
        _ => format!("/submitter/cases/{case_id}"),
    };
    Ok(Redirect::to(&dest))
}

// ---------------------------------------------------------------------------------
// Admin routes
// ---------------------------------------------------------------------------------

async fn admin_dashboard(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    let rows: Vec<(Uuid, String, bool, String)> = sqlx::query_as(
        "select c.id, c.title, c.hidden, u.display_name
         from verified_cases c join verified_users u on u.id = c.submitter_id
         order by c.created_at desc",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut list = String::new();
    for (id, title, hidden, submitter_name) in &rows {
        list.push_str(&format!(
            r#"<a class="case-card" href="/admin/cases/{id}">
                 <h3>{title}{hidden_tag}</h3>
                 <p class="muted">Submitter: {submitter}</p>
               </a>"#,
            id = id,
            title = esc(title),
            hidden_tag = if *hidden { " (hidden)" } else { "" },
            submitter = esc(submitter_name),
        ));
    }
    if rows.is_empty() {
        list.push_str(r#"<p class="muted">No cases yet.</p>"#);
    }
    Ok(Html(page(
        "Admin",
        Some(&user),
        format!(
            r#"<h1>Cases</h1>
               <div class="toolbar"><a href="/admin/cases/new">+ New case</a> <a href="/admin/admins">Manage admins</a></div>
               <div class="grid">{list}</div>"#
        ),
    )))
}

async fn admin_case_new(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    Ok(Html(page("New case", Some(&user), case_form(None))))
}

fn case_form(error: Option<&str>) -> String {
    let err = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    format!(
        r#"<h1>New case</h1>
           {err}
           <form method="post" action="/admin/cases" class="stack">
             <label>Title<input type="text" name="title" required></label>
             <label>Description<textarea name="description" required></textarea></label>
             <h2>Submitter (the client/mentor this case is posted for)</h2>
             <label>Name<input type="text" name="submitter_name" required></label>
             <label>Email<input type="email" name="submitter_email" required></label>
             <label>LinkedIn URL (optional)<input type="url" name="submitter_linkedin" placeholder="https://linkedin.com/in/…"></label>
             <button type="submit">Create case</button>
           </form>"#
    )
}

#[derive(Deserialize)]
struct CaseForm {
    title: String,
    description: String,
    submitter_name: String,
    submitter_email: String,
    #[serde(default)]
    submitter_linkedin: String,
}

async fn admin_case_create(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<CaseForm>,
) -> Result<Response, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    let title = f.title.trim();
    let description = f.description.trim();
    let submitter_email = f.submitter_email.trim().to_lowercase();
    let submitter_name = f.submitter_name.trim();
    if title.is_empty() || description.is_empty() || submitter_name.is_empty() || !submitter_email.contains('@') {
        return Ok(Html(page("New case", Some(&user), case_form(Some("Fill in every field with a valid email.")))).into_response());
    }
    let linkedin = if f.submitter_linkedin.trim().is_empty() {
        None
    } else {
        Some(f.submitter_linkedin.trim().to_string())
    };

    let existing: Option<(Uuid, String)> =
        sqlx::query_as("select id, role from verified_users where email = $1")
            .bind(&submitter_email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    let (submitter_id, is_new) = match existing {
        Some((id, role)) if role == "submitter" => {
            sqlx::query("update verified_users set display_name = $2, linkedin_url = coalesce($3, linkedin_url) where id = $1")
                .bind(id).bind(submitter_name).bind(&linkedin)
                .execute(&app.pool).await.unwrap();
            (id, false)
        }
        Some((_, role)) => {
            let msg = format!("{submitter_email} is already registered as {role} — use a different email.");
            return Ok(Html(page("New case", Some(&user), case_form(Some(&msg)))).into_response());
        }
        None => {
            let id: (Uuid,) = sqlx::query_as(
                "insert into verified_users (email, display_name, role, linkedin_url) values ($1,$2,'submitter',$3) returning id",
            )
            .bind(&submitter_email)
            .bind(submitter_name)
            .bind(&linkedin)
            .fetch_one(&app.pool)
            .await
            .unwrap();
            (id.0, true)
        }
    };

    let case_id: (Uuid,) = sqlx::query_as(
        "insert into verified_cases (title, description, submitter_id, created_by) values ($1,$2,$3,$4) returning id",
    )
    .bind(title)
    .bind(description)
    .bind(submitter_id)
    .bind(user.id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    if is_new {
        send_login_link(&app, &submitter_email).await;
    }
    Ok(Redirect::to(&format!("/admin/cases/{}", case_id.0)).into_response())
}

async fn admin_case_manage(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at from verified_cases where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(case) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    Ok(Html(page(
        &case.title,
        Some(&user),
        render_case_manage(&app, &case, &user, true).await,
    )))
}

async fn admin_case_edit_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    let case: Option<VCase> = sqlx::query_as(
        "select id, title, description, submitter_id, hidden, created_at from verified_cases where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(case) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    Ok(Html(page(
        "Edit case",
        Some(&user),
        format!(
            r#"<h1>Edit case</h1>
               <form method="post" action="/admin/cases/{id}/edit" class="stack">
                 <label>Title<input type="text" name="title" value="{title}" required></label>
                 <label>Description<textarea name="description" required>{desc}</textarea></label>
                 <button type="submit">Save</button>
               </form>
               <p class="muted">To change the submitter's identity, hide this case and create a new one.</p>

               <h2>Submitter photo</h2>
               <form method="post" action="/admin/cases/{id}/submitter-photo" enctype="multipart/form-data" class="inline-form">
                 <input type="file" name="photo" accept="image/*" required>
                 <button type="submit">Upload photo</button>
               </form>"#,
            id = case.id,
            title = esc(&case.title),
            desc = esc(&case.description),
        ),
    )))
}

#[derive(Deserialize)]
struct EditCaseForm {
    title: String,
    description: String,
}

async fn admin_case_edit_post(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Form(f): Form<EditCaseForm>,
) -> Result<Redirect, Response> {
    let _user = require_role(current_user(&app, &headers).await, "admin")?;
    let title = f.title.trim();
    let description = f.description.trim();
    if title.is_empty() || description.is_empty() {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query("update verified_cases set title = $2, description = $3 where id = $1")
        .bind(id)
        .bind(title)
        .bind(description)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to(&format!("/admin/cases/{id}")))
}

async fn admin_case_hide_toggle(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Redirect, Response> {
    let _user = require_role(current_user(&app, &headers).await, "admin")?;
    sqlx::query("update verified_cases set hidden = not hidden where id = $1")
        .bind(id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to(&format!("/admin/cases/{id}")))
}

async fn admin_admins_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require_role(current_user(&app, &headers).await, "admin")?;
    let admins: Vec<(String, String)> =
        sqlx::query_as("select email, display_name from verified_users where role = 'admin' order by created_at")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    let rows: String = admins
        .iter()
        .map(|(email, name)| format!("<li>{} &lt;{}&gt;</li>", esc(name), esc(email)))
        .collect();
    Ok(Html(page(
        "Admins",
        Some(&user),
        format!(
            r#"<h1>Admins</h1>
               <ul>{rows}</ul>
               <form method="post" action="/admin/admins" class="stack">
                 <label>Email<input type="email" name="email" required></label>
                 <button type="submit">Grant admin</button>
               </form>"#
        ),
    )))
}

#[derive(Deserialize)]
struct AddAdminForm {
    email: String,
}

async fn admin_admins_post(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<AddAdminForm>,
) -> Result<Response, Response> {
    let _user = require_role(current_user(&app, &headers).await, "admin")?;
    let email = f.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("select id, role from verified_users where email = $1")
            .bind(&email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    match existing {
        None => {
            sqlx::query("insert into verified_users (email, display_name, role) values ($1,$2,'admin')")
                .bind(&email)
                .bind(&email)
                .execute(&app.pool)
                .await
                .unwrap();
            send_login_link(&app, &email).await;
        }
        Some((_, role)) if role == "admin" => {}
        Some((_, role)) => {
            eprintln!("refused to promote {email} (already {role}) to admin without explicit DB change");
        }
    }
    Ok(Redirect::to("/admin/admins").into_response())
}

// ---------------------------------------------------------------------------------
// File download
// ---------------------------------------------------------------------------------

async fn file_download(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let row: Option<(Uuid, Uuid, Uuid, String, String, String)> = sqlx::query_as(
        "select f.submission_id, s.case_id, s.student_id, f.filename, f.content_type, f.storage_key
         from verified_submission_files f
         join verified_submissions s on s.id = f.submission_id
         where f.id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((_submission_id, case_id, student_id, filename, content_type, storage_key)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    let authorized = user.role == "admin" || user.id == student_id || {
        let submitter: Option<(Uuid,)> =
            sqlx::query_as("select submitter_id from verified_cases where id = $1")
                .bind(case_id)
                .fetch_optional(&app.pool)
                .await
                .unwrap();
        submitter.is_some_and(|(sid,)| sid == user.id)
    };
    if !authorized {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    let bytes = storage_get(&app, &storage_key)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY.into_response())?;
    Ok(attachment(&filename, &content_type, bytes))
}

// Public, no auth: this is what an <img src="/avatar/{id}"> in a page points at.
async fn avatar_get(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Response, StatusCode> {
    let row: Option<(Option<Vec<u8>>, Option<String>)> =
        sqlx::query_as("select avatar, avatar_content_type from verified_users where id = $1")
            .bind(id)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    let Some((Some(bytes), Some(content_type))) = row else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=3600".to_string()),
        ],
        bytes,
    )
        .into_response())
}

async fn case_file_download(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, Response> {
    // Every signed-in role may view case documents (students included) — only upload is
    // restricted, enforced separately in case_document_upload.
    require(current_user(&app, &headers).await)?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "select filename, content_type, storage_key from verified_case_documents where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((filename, content_type, storage_key)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    let bytes = storage_get(&app, &storage_key)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY.into_response())?;
    Ok(attachment(&filename, &content_type, bytes))
}

async fn case_document_upload(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    mut mp: Multipart,
) -> Result<Redirect, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let submitter_id: Option<(Uuid,)> =
        sqlx::query_as("select submitter_id from verified_cases where id = $1")
            .bind(id)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    let Some((submitter_id,)) = submitter_id else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    if !(user.role == "admin" || user.id == submitter_id) {
        return Err(StatusCode::FORBIDDEN.into_response());
    }

    let mut upload: Option<(String, String, Vec<u8>)> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Could not read form."))? {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("file").to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = field.bytes().await.map_err(|_| bad("Could not read the file."))?;
            if !bytes.is_empty() {
                if bytes.len() as u64 > app.max_file_bytes {
                    return Err(bad(&format!(
                        "File can't exceed {} MB.",
                        app.max_file_bytes / 1024 / 1024
                    )));
                }
                upload = Some((safe_filename(&filename), content_type, bytes.to_vec()));
            }
        }
    }
    let Some((filename, content_type, bytes)) = upload else {
        return Err(bad("Choose a file."));
    };

    let doc_id = Uuid::new_v4();
    let key = format!("case-docs/{id}/{doc_id}-{filename}");
    let size = bytes.len() as i64;
    storage_put(&app, &key, bytes, &content_type)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY.into_response())?;
    let position: (i64,) = sqlx::query_as(
        "select coalesce(max(position), -1) + 1 from verified_case_documents where case_id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into verified_case_documents (id, case_id, filename, content_type, storage_key, size_bytes, uploaded_by, position)
         values ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(doc_id)
    .bind(id)
    .bind(&filename)
    .bind(&content_type)
    .bind(&key)
    .bind(size)
    .bind(user.id)
    .bind(position.0 as i32)
    .execute(&app.pool)
    .await
    .unwrap();

    let dest = match user.role.as_str() {
        "admin" => format!("/admin/cases/{id}"),
        _ => format!("/submitter/cases/{id}"),
    };
    Ok(Redirect::to(&dest))
}

// Admin-only: sets or replaces a submitter's avatar. Not exposed to the submitter
// themselves — the user asked for admin to control this, same as who creates the case.
async fn admin_submitter_photo(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    mut mp: Multipart,
) -> Result<Redirect, Response> {
    require_role(current_user(&app, &headers).await, "admin")?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();

    let case: Option<(Uuid,)> = sqlx::query_as("select submitter_id from verified_cases where id = $1")
        .bind(id)
        .fetch_optional(&app.pool)
        .await
        .unwrap();
    let Some((submitter_id,)) = case else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };

    let mut photo: Option<(String, Vec<u8>)> = None;
    while let Some(field) = mp.next_field().await.map_err(|_| bad("Could not read form."))? {
        if field.name() == Some("photo") {
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
            let bytes = field.bytes().await.map_err(|_| bad("Could not read the photo."))?;
            if !bytes.is_empty() {
                if bytes.len() as u64 > app.max_file_bytes {
                    return Err(bad(&format!(
                        "Photo can't exceed {} MB.",
                        app.max_file_bytes / 1024 / 1024
                    )));
                }
                photo = Some((content_type, bytes.to_vec()));
            }
        }
    }
    let Some((content_type, bytes)) = photo else {
        return Err(bad("Choose a photo."));
    };

    sqlx::query("update verified_users set avatar = $2, avatar_content_type = $3 where id = $1")
        .bind(submitter_id)
        .bind(&bytes)
        .bind(&content_type)
        .execute(&app.pool)
        .await
        .unwrap();

    Ok(Redirect::to(&format!("/admin/cases/{id}/edit")))
}

// ---------------------------------------------------------------------------------
// HTML shell + small render helpers
// ---------------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn nl2br(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

// Circular avatar (real image if uploaded, else an initials fallback) paired with a
// name that links to LinkedIn when we have a URL for them. `vertical` stacks avatar
// above name (submission cards); horizontal puts them side by side (case header).
fn avatar_chip(user_id: Uuid, name: &str, linkedin: Option<&str>, has_avatar: bool, vertical: bool) -> String {
    let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let img = if has_avatar {
        format!(r#"<img class="avatar" src="/avatar/{id}" alt="">"#, id = user_id)
    } else {
        format!(r#"<span class="avatar avatar-fallback">{initial}</span>"#, initial = esc(&initial))
    };
    let name_html = match linkedin.filter(|u| !u.trim().is_empty()) {
        Some(u) => format!(r#"<a href="{url}" target="_blank" rel="noopener">{name}</a>"#, url = esc(u), name = esc(name)),
        None => format!("<span>{}</span>", esc(name)),
    };
    let class = if vertical { "person-chip vertical" } else { "person-chip horizontal" };
    format!(r#"<span class="{class}">{img}<span class="person-name">{name_html}</span></span>"#)
}

fn login_body(message: Option<&str>) -> String {
    let msg = message
        .map(|m| format!(r#"<p class="notice">{}</p>"#, esc(m)))
        .unwrap_or_default();
    format!(
        r#"<h1>Exposure Academy Verified</h1>
           {msg}
           <form method="post" action="/login" class="stack">
             <label>Email<input type="email" name="email" required autofocus></label>
             <button type="submit">Send sign-in link</button>
           </form>
           <p class="muted">Students: use the email your Academy account is registered with.</p>"#
    )
}

fn nav(user: Option<&VUser>) -> String {
    match user {
        None => String::new(),
        Some(u) => {
            let links = match u.role.as_str() {
                "admin" => r#"<a href="/admin">Cases</a><a href="/admin/admins">Admins</a>"#,
                "submitter" => r#"<a href="/submitter">Your cases</a>"#,
                _ => r#"<a href="/cases">Cases</a>"#,
            };
            format!(
                r#"<nav>
                     <a class="brand" href="{home}">Verified</a>
                     {links}
                     <span class="spacer"></span>
                     <span class="muted">{name}</span>
                     <form method="post" action="/logout" class="inline-form"><button type="submit">Log out</button></form>
                   </nav>"#,
                home = u.home(),
                name = esc(&u.display_name),
            )
        }
    }
}

fn page(title: &str, user: Option<&VUser>, body: String) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Exposure Academy Verified</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
{nav}
<main>{body}</main>
</body>
</html>"#,
        title = esc(title),
        nav = nav(user),
    )
}

async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        CSS,
    )
}

const CSS: &str = r#"
:root { color-scheme: light; --bg:#FFFCF6; --card:#ffffff; --border:#e8e4da; --ink:#0D0D0D;
  --muted:#71717a; --accent:#0339A6; --warn:#b45309; }
* { box-sizing: border-box; }
body { margin:0; font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif; background:var(--bg); color:var(--ink); }
main { max-width: 720px; margin: 0 auto; padding: 24px 16px 64px; }
nav { display:flex; align-items:center; gap:16px; padding:14px 20px; border-bottom:1px solid var(--border); background:var(--card); }
nav a { color: var(--ink); text-decoration:none; font-weight:600; font-size:14px; }
nav .brand { font-weight:800; color: var(--accent); }
nav .spacer { flex: 1; }
h1 { font-size: 26px; margin: 8px 0 16px; }
h2 { font-size: 18px; margin: 28px 0 12px; }
.muted { color: var(--muted); font-size: 13px; }
.error { color: #b91c1c; font-weight: 600; }
.notice { color: var(--accent); font-weight: 600; }
.prose { white-space: pre-wrap; line-height: 1.6; }
.stack { display:flex; flex-direction:column; gap:12px; margin: 16px 0; }
.stack label { display:flex; flex-direction:column; gap:4px; font-size:13px; font-weight:600; }
input, textarea, button { font: inherit; padding: 10px 12px; border:1px solid var(--border); border-radius:10px; }
textarea { min-height: 80px; resize: vertical; }
button { background: var(--accent); color: white; border: none; font-weight:700; cursor:pointer; }
button:hover { opacity: 0.9; }
.inline-form { display:inline; }
.inline-form button { background: transparent; color: var(--ink); border:1px solid var(--border); font-weight:600; padding:6px 10px; }
.grid { display:grid; gap:12px; margin-top:16px; }
.case-card { display:block; background: var(--card); border:1px solid var(--border); border-radius:14px; padding:16px 18px; text-decoration:none; color:var(--ink); }
.case-card h3 { margin: 0 0 6px; }
.badge { display:inline-block; margin-top:6px; font-size:12px; font-weight:700; color: var(--accent); }
.badge.warn { color: var(--warn); }
.toolbar { display:flex; gap:16px; margin: 12px 0 20px; }
.toolbar a { color: var(--accent); font-weight:700; text-decoration:none; font-size:14px; }
.avatar { width:40px; height:40px; border-radius:50%; object-fit:cover; flex:none; display:inline-flex; align-items:center; justify-content:center; }
.avatar-fallback { background: var(--accent); color:#fff; font-weight:700; font-size:15px; }
.person-chip { display:inline-flex; gap:10px; align-items:center; }
.person-chip.horizontal { margin: 4px 0 16px; }
.person-chip.vertical { flex-direction:column; text-align:center; width:64px; }
.person-chip.vertical .avatar { width:48px; height:48px; }
.person-chip .person-name { font-weight:600; font-size:14px; }
.person-chip .person-name a { color: var(--ink); text-decoration:none; }
.person-chip .person-name a:hover { text-decoration:underline; }
.submission-card { display:flex; gap:14px; border:1px solid var(--border); border-radius:12px; padding:12px 14px; margin: 10px 0; }
.submission-card .submission-body { flex:1; min-width:0; }
.submission-card ul { margin:8px 0 0; padding-left:18px; }
.qa-item { border-top:1px solid var(--border); padding: 12px 0; }
.qa-item:first-child { border-top: none; }
.answer { margin-top:6px; }
.answer.pending { color: var(--muted); font-style: italic; }
"#;

// ---------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_reject_over_limit() {
        assert!(check_caps(5, 10).is_ok());
        assert!(check_caps(6, 0).is_err());
        assert!(check_caps(0, 11).is_err());
    }

    #[test]
    fn valid_url_requires_scheme() {
        assert_eq!(valid_url("https://x.com"), Some("https://x.com".to_string()));
        assert_eq!(valid_url("  http://x.com  "), Some("http://x.com".to_string()));
        assert_eq!(valid_url("x.com"), None);
        assert_eq!(valid_url("   "), None);
    }

    #[test]
    fn safe_filename_strips_path_and_controls() {
        assert_eq!(safe_filename("../../etc/passwd"), "passwd");
        assert_eq!(safe_filename("C:\\Users\\me\\report.pdf"), "report.pdf");
        assert_eq!(safe_filename(""), "file");
    }

    #[test]
    fn role_home_routes_by_role() {
        assert_eq!(role_home("admin"), "/admin");
        assert_eq!(role_home("submitter"), "/submitter");
        assert_eq!(role_home("student"), "/cases");
        assert_eq!(role_home("whatever"), "/cases");
    }
}
