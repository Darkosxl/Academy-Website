mod html;
mod model;

use axum::{
    Form, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand::RngCore;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use model::*;

#[derive(Clone)]
struct App {
    pool: PgPool,
    worker_token: String,
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

    let app = App {
        pool,
        worker_token: std::env::var("WORKER_TOKEN").unwrap_or_default(),
    };

    let router = Router::new()
        .route("/", get(landing))
        .route("/login", get(login_page).post(login_post))
        .route("/logout", post(logout))
        .route("/app", get(video_grid))
        .route("/watch/{id}", get(watch))
        .route("/api/progress", post(progress))
        .route("/board", get(board))
        .route("/board/submit", post(board_submit))
        .route("/admin", get(admin_page))
        .route("/admin/video", post(admin_video))
        .route("/admin/task", post(admin_task))
        .route("/admin/user", post(admin_user))
        .route("/admin/review", post(admin_review))
        .route("/api/worker/pending", get(worker_pending))
        .route("/api/worker/result", post(worker_result))
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .with_state(app);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:3000".into());
    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

async fn seed_admin(pool: &PgPool) {
    let (Ok(u), Ok(p)) = (std::env::var("ADMIN_USERNAME"), std::env::var("ADMIN_PASSWORD")) else { return };
    let exists: Option<(Uuid,)> = sqlx::query_as("select id from users_exposure_academy where username = $1")
        .bind(&u).fetch_optional(pool).await.unwrap();
    if exists.is_none() {
        sqlx::query("insert into users_exposure_academy (username, password_hash, display_name, is_admin) values ($1,$2,$3,true)")
            .bind(&u).bind(hash_pw(&p)).bind(&u).execute(pool).await.unwrap();
        println!("admin '{u}' created");
    }
}

fn hash_pw(pw: &str) -> String {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default().hash_password(pw.as_bytes(), &salt).unwrap().to_string()
}

fn verify_pw(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok()).unwrap_or(false)
}

// ---- session helpers ----

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?
        .split(';').map(str::trim)
        .find_map(|c| c.strip_prefix("session=").map(String::from))
}

async fn current_user(app: &App, headers: &HeaderMap) -> Option<User> {
    let token = cookie_token(headers)?;
    sqlx::query_as::<_, User>(
        "select u.id, u.display_name, u.is_admin from sessions_exposure_academy s join users_exposure_academy u on u.id = s.user_id where s.token = $1")
        .bind(token).fetch_optional(&app.pool).await.ok()?
}

fn require(user: Option<User>) -> Result<User, Response> {
    user.ok_or_else(|| Redirect::to("/login").into_response())
}

fn require_admin(user: Option<User>) -> Result<User, Response> {
    match user {
        Some(u) if u.is_admin => Ok(u),
        Some(_) => Err(StatusCode::FORBIDDEN.into_response()),
        None => Err(Redirect::to("/login").into_response()),
    }
}

// ---- pages ----

async fn landing() -> Html<String> {
    Html(html::landing())
}

async fn login_page() -> Html<String> {
    Html(html::login(false))
}

#[derive(Deserialize)]
struct LoginForm { username: String, password: String }

async fn login_post(State(app): State<App>, Form(f): Form<LoginForm>) -> Response {
    let row: Option<(Uuid, String)> = sqlx::query_as("select id, password_hash from users_exposure_academy where username = $1")
        .bind(&f.username).fetch_optional(&app.pool).await.unwrap();
    let Some((uid, hash)) = row else { return Html(html::login(true)).into_response() };
    if !verify_pw(&f.password, &hash) {
        return Html(html::login(true)).into_response();
    }
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let token: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    sqlx::query("insert into sessions_exposure_academy (token, user_id) values ($1,$2)")
        .bind(&token).bind(uid).execute(&app.pool).await.unwrap();
    (
        [(header::SET_COOKIE, format!("session={token}; HttpOnly; Path=/; Max-Age=31536000; SameSite=Lax"))],
        Redirect::to("/app"),
    ).into_response()
}

async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&headers) {
        let _ = sqlx::query("delete from sessions_exposure_academy where token = $1").bind(t).execute(&app.pool).await;
    }
    (
        [(header::SET_COOKIE, "session=; Path=/; Max-Age=0".to_string())],
        Redirect::to("/"),
    ).into_response()
}

#[derive(Deserialize)]
struct LevelQ { level: Option<String> }

async fn video_grid(State(app): State<App>, headers: HeaderMap, Query(q): Query<LevelQ>) -> Result<Html<String>, Response> {
    let user = require(current_user(&app, &headers).await)?;
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
    let user = require(current_user(&app, &headers).await)?;
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

// ---- board ----

async fn board(State(app): State<App>, headers: HeaderMap) -> Result<Html<String>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level from tasks_exposure_academy order by created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let subs = sqlx::query_as::<_, SubmissionView>(
        "select distinct on (s.task_id) s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url,
                u.display_name, t.title as task_title
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         where s.user_id = $1 order by s.task_id, s.created_at desc")
        .bind(user.id).fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::board(&user, &tasks, &subs)))
}

#[derive(Deserialize)]
struct SubmitForm { task_id: Uuid, repo_url: String }

async fn board_submit(State(app): State<App>, headers: HeaderMap, Form(f): Form<SubmitForm>) -> Result<Redirect, Response> {
    let user = require(current_user(&app, &headers).await)?;
    if !f.repo_url.starts_with("https://github.com/") {
        return Err((StatusCode::BAD_REQUEST, "GitHub deposu bağlantısı gerekli").into_response());
    }
    sqlx::query("insert into submissions_exposure_academy (task_id, user_id, repo_url) values ($1,$2,$3)")
        .bind(f.task_id).bind(user.id).bind(&f.repo_url)
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
        "select s.id, s.task_id, s.repo_url, s.status, s.feedback, s.demo_video_url,
                u.display_name, t.title as task_title
         from submissions_exposure_academy s join users_exposure_academy u on u.id = s.user_id join tasks_exposure_academy t on t.id = s.task_id
         order by s.created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    let videos = sqlx::query_as::<_, Video>("select id, youtube_id, title, level from videos_exposure_academy order by level, position")
        .fetch_all(&app.pool).await.unwrap();
    let tasks = sqlx::query_as::<_, Task>("select id, title, description, level from tasks_exposure_academy order by created_at desc")
        .fetch_all(&app.pool).await.unwrap();
    Ok(Html(html::admin(&user, &stats, &subs, &videos, &tasks)))
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

#[derive(Deserialize)]
struct TaskForm { title: String, description: String, level: String }

async fn admin_task(State(app): State<App>, headers: HeaderMap, Form(f): Form<TaskForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("insert into tasks_exposure_academy (title, description, level) values ($1,$2,$3)")
        .bind(&f.title).bind(&f.description).bind(&f.level)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
struct UserForm { username: String, display_name: String, password: String }

async fn admin_user(State(app): State<App>, headers: HeaderMap, Form(f): Form<UserForm>) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("insert into users_exposure_academy (username, password_hash, display_name) values ($1,$2,$3)")
        .bind(&f.username).bind(hash_pw(&f.password)).bind(&f.display_name)
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
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
