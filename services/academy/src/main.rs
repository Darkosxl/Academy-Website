// Wiring only: state, startup, and the route table. Every handler lives in the module
// for its section, and each of those owns its student pages, its admin handlers and its
// worker API together.
mod admin;
mod auth;
mod board;
mod harness;
mod html;
mod model;
mod monopoly;
mod portal;

use axum::extract::DefaultBodyLimit;
use axum::{
    Router, middleware,
    routing::{get, post},
};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

use admin::*;
use auth::*;
use board::*;
use harness::*;
use monopoly::*;
use portal::*;

#[derive(Clone)]
pub struct App {
    pub pool: PgPool,
    pub worker_token: String,
    pub http: reqwest::Client,
    pub resend_key: String,
    pub mail_from: String,
    pub base_url: String,
    /// Optional Microlink API key for screenshot generation; blank = free tier.
    pub microlink_key: String,
    /// DeepL key for the Turkish transcripts. Blank just means the toggle shows English.
    pub deepl_key: String,
    /// XChaCha20-Poly1305 key for team Kaggle tokens. None disables official submit.
    pub kaggle_key: Option<[u8; 32]>,
}

fn secret_key(name: &str) -> Option<[u8; 32]> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    let raw = raw.trim();
    assert!(
        raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()),
        "{name} must be exactly 64 hexadecimal characters"
    );
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).unwrap();
    }
    Some(key)
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing (.env)");
    // NOTE: use Supabase's SESSION pooler (port 5432), not transaction pooler (6543) —
    // transaction mode can't do prepared statements, which sqlx relies on.
    let pool = PgPool::connect(&db_url).await.expect("db connect failed");

    // idempotent schema + seed admin
    sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
        .execute(&pool)
        .await
        .expect("migration failed");
    sqlx::raw_sql(include_str!("../migrations/002_harness_v2.sql"))
        .execute(&pool)
        .await
        .expect("harness v2 migration failed");
    sqlx::raw_sql(include_str!("../migrations/003_arc_frames.sql"))
        .execute(&pool)
        .await
        .expect("arc frames migration failed");
    sqlx::raw_sql(include_str!("../migrations/004_worker_protocol.sql"))
        .execute(&pool)
        .await
        .expect("worker protocol migration failed");
    seed_admin(&pool).await;
    seed_invite_code(&pool).await;
    seed_videos(&pool).await;
    // opportunistic cleanup of stale magic links and sessions, no scheduler needed
    let _ = sqlx::query(
        "delete from magic_links_exposure_academy where expires_at < now() - interval '1 day'",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query("delete from sessions_exposure_academy where expires_at < now()")
        .execute(&pool)
        .await;

    let app = App {
        pool,
        worker_token: std::env::var("WORKER_TOKEN").unwrap_or_default(),
        // A User-Agent is not optional: api.github.com answers 403 to any request without
        // one, and reqwest sends none by default — the live-site resolver would silently
        // find nothing for every submission.
        http: reqwest::Client::builder()
            .user_agent("exposure-academy")
            .build()
            .expect("http client"),
        resend_key: std::env::var("RESEND_API_KEY").expect("RESEND_API_KEY missing (.env)"),
        mail_from: std::env::var("MAIL_FROM").expect("MAIL_FROM missing (.env)"),
        base_url: std::env::var("APP_BASE_URL").expect("APP_BASE_URL missing (.env)"),
        microlink_key: std::env::var("MICROLINK_API_KEY").unwrap_or_default(),
        deepl_key: std::env::var("DEEPL_API_KEY").unwrap_or_default(),
        kaggle_key: secret_key("KAGGLE_CREDENTIAL_KEY"),
    };

    // background: keeps submissions' live site URLs up to date. Students deploy after they
    // submit, so this can't be a one-shot at submit time.
    spawn_resolver(app.clone());

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
        .route("/agentic-harness/submit", post(harness_submit))
        .route("/agentic-harness/status", get(harness_status))
        .route("/agentic-harness/arc/live", get(harness_arc_live))
        .route(
            "/agentic-harness/kaggle/credentials",
            post(harness_kaggle_credentials),
        )
        .route(
            "/agentic-harness/kaggle/credentials/delete",
            post(harness_kaggle_credentials_delete),
        )
        .route(
            "/agentic-harness/kaggle/submit",
            post(harness_kaggle_submit),
        )
        .route("/ai-monopoly", get(ai_monopoly))
        .route("/ai-monopoly/submit", post(monopoly_submit))
        .route("/ai-monopoly/live", get(monopoly_live_json))
        .route("/ai-monopoly/practice", post(monopoly_practice))
        .route("/ai-monopoly/match/{id}", get(monopoly_match_page))
        .route("/demos", get(demos))
        .route("/watch/{id}", get(watch))
        .route("/api/progress", post(progress))
        .route("/leaderboard", get(leaderboard))
        .route("/board", get(board))
        .route("/board/profiles", post(board_profiles))
        .route(
            "/board/submit",
            post(board_submit).layer(DefaultBodyLimit::max(300 * 1024)),
        )
        .route("/board/interest", post(board_interest))
        .route("/board/sites/{task_id}", get(board_sites))
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
        .route("/admin/user", post(admin_user))
        .route("/admin/user/delete", post(admin_user_delete))
        .route("/admin/user/hidden", post(admin_user_hidden))
        .route("/admin/review", post(admin_review))
        .route("/admin/prompts.txt", get(admin_prompts_txt))
        .route("/admin/invite", post(admin_rotate_invite))
        .route("/admin/submission/live", post(admin_submission_live))
        .route("/admin/harness/team", post(admin_harness_team))
        .route(
            "/admin/harness/team/delete",
            post(admin_harness_team_delete),
        )
        .route("/admin/harness/member", post(admin_harness_member))
        .route(
            "/admin/harness/member/remove",
            post(admin_harness_member_remove),
        )
        .route("/admin/harness/run/fail", post(admin_harness_run_fail))
        .route("/admin/monopoly/team", post(admin_monopoly_team))
        .route(
            "/admin/monopoly/team/delete",
            post(admin_monopoly_team_delete),
        )
        .route("/admin/monopoly/member", post(admin_monopoly_member))
        .route(
            "/admin/monopoly/member/remove",
            post(admin_monopoly_member_remove),
        )
        .route("/admin/monopoly/start", post(admin_monopoly_start))
        .route("/admin/monopoly/fail", post(admin_monopoly_fail))
        .route("/api/worker/pending", get(worker_pending))
        .route("/api/worker/result", post(worker_result))
        .route("/api/worker/harness/claim", post(worker_harness_claim))
        .route(
            "/api/worker/harness/heartbeat",
            post(worker_harness_heartbeat),
        )
        .route("/api/worker/harness/stage", post(worker_harness_stage))
        .route(
            "/api/worker/harness/progress",
            post(worker_harness_progress),
        )
        .route("/api/worker/harness/result", post(worker_harness_result))
        // 64 frames x a 16-grid animation buffer is ~4 MB of hex, over the 2 MB default
        .route(
            "/api/worker/harness/arc/frames",
            post(worker_harness_arc_frames).layer(DefaultBodyLimit::max(6 * 1024 * 1024)),
        )
        .route(
            "/api/worker/harness/kaggle/claim",
            post(worker_harness_kaggle_claim),
        )
        .route(
            "/api/worker/harness/kaggle/result",
            post(worker_harness_kaggle_result),
        )
        .route("/api/worker/monopoly/pending", get(worker_monopoly_pending))
        .route("/api/worker/monopoly/stage", post(worker_monopoly_stage))
        .route(
            "/api/worker/monopoly/progress",
            post(worker_monopoly_progress),
        )
        .route(
            "/api/worker/monopoly/message",
            post(worker_monopoly_message),
        )
        .route("/api/worker/monopoly/invite", post(worker_monopoly_invite))
        .route(
            "/api/worker/monopoly/verdict",
            post(worker_monopoly_verdict),
        )
        .route("/api/worker/monopoly/note", post(worker_monopoly_note))
        .route("/api/worker/monopoly/result", post(worker_monopoly_result))
        // rolling session refresh — applies to the routes above only; static assets
        // are mounted after the layer so they don't each cost a session write
        .layer(middleware::from_fn_with_state(app.clone(), rolling_session))
        // cached example-URL screenshots: public cacheable assets like /static, mounted
        // after the layer so they don't cost a session write and need no auth
        .route("/preview/{id}", get(task_preview))
        .route("/preview/sub/{id}", get(submission_preview))
        .nest_service(
            "/static",
            tower_http::services::ServeDir::new(concat!(env!("CARGO_MANIFEST_DIR"), "/static")),
        )
        // last, so it covers every route above: one ARC live poll is ~53 KB of 16-symbol
        // hex and gzips ~20x, which is the difference between a watchable grid and not
        .layer(tower_http::compression::CompressionLayer::new())
        .with_state(app);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:3000".into());
    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

async fn seed_admin(pool: &PgPool) {
    let Ok(email) = std::env::var("ADMIN_EMAIL") else {
        return;
    };
    let email = email.trim().to_lowercase();
    let exists: Option<(Uuid,)> =
        sqlx::query_as("select id from users_exposure_academy where email = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await
            .unwrap();
    match exists {
        None => {
            sqlx::query("insert into users_exposure_academy (email, display_name, is_admin) values ($1,$2,true)")
                .bind(&email).bind(&email).execute(pool).await.unwrap();
            println!("admin '{email}' seeded");
        }
        Some(_) => {
            sqlx::query("update users_exposure_academy set is_admin = true where email = $1")
                .bind(&email)
                .execute(pool)
                .await
                .unwrap();
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
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("videos.dat not valid hex"))
        .collect();
    let blob = String::from_utf8(bytes).expect("videos.dat not valid utf-8");
    for (i, yt) in blob.lines().filter(|l| !l.is_empty()).enumerate() {
        let pos = (i + 1) as i32;
        let level = if pos <= 8 { "PRESEED" } else { "SEED" };
        let title = VIDEO_TITLES
            .get(i)
            .map(|t| t.to_string())
            .unwrap_or_else(|| format!("Ders {pos}"));
        sqlx::query(
            "insert into videos_exposure_academy (youtube_id, title, level, position)
             select $1,$2,$3,$4
             where not exists (select 1 from videos_exposure_academy where youtube_id = $1)",
        )
        .bind(yt)
        .bind(&title)
        .bind(level)
        .bind(pos)
        .execute(pool)
        .await
        .unwrap();
        // Rows seeded before real titles existed still say "Ders N" — rename those
        // in place. Admin-edited titles don't match the default and are left alone.
        sqlx::query(
            "update videos_exposure_academy set title = $2
             where youtube_id = $1 and title = $3",
        )
        .bind(yt)
        .bind(&title)
        .bind(format!("Ders {pos}"))
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn seed_invite_code(pool: &PgPool) {
    let Ok(code) = std::env::var("INVITE_CODE") else {
        return;
    };
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ('invite_code', $1, now())
         on conflict (key) do update set value = $1, updated_at = now()")
        .bind(code.trim()).execute(pool).await.unwrap();
}

pub fn random_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- session helpers ----

// ---- pages ----

// ---- profile ----

// ---- Agentic Harness (student side) ----

// ---- leaderboard ----

// ---- board ----

// ---- admin ----

// ---- admin: Agentic Harness teams (interim until team onboarding) ----
