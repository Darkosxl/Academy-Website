use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Clone)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    /// Public handle. `None` means onboarding is unfinished — see `require_onboarded`.
    pub nickname: Option<String>,
    pub is_admin: bool,
}

impl User {
    /// What the sidebar shows: the handle they picked, else their real name.
    pub fn label(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.display_name)
    }
}

/// Everything the student can see and edit about themselves on /profile.
#[derive(FromRow, Default)]
pub struct Profile {
    pub email: String,
    pub display_name: String,
    pub nickname: Option<String>,
    pub school: Option<String>,
    pub grade: Option<String>,
}

/// The onboarding form. Lives here so `html::join` can re-render what the student
/// typed after a validation error without main.rs and html.rs disagreeing on shape.
#[derive(Deserialize, Default)]
pub struct JoinForm {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub school: String,
    #[serde(default)]
    pub grade: String,
    #[serde(default)]
    pub github_url: String,
    #[serde(default)]
    pub linkedin_url: String,
}

pub const GRADES: [&str; 4] = ["9'a geçiyor", "10'a geçiyor", "11'e geçiyor", "12'ye geçiyor"];

/// An optional public-profile link (GitHub / LinkedIn). Empty is allowed — the step
/// is skippable. If present, we tolerate a missing scheme (prepend `https://`) and
/// require the expected host so the board can trust it enough to render as a link.
/// `Ok(None)` means "left blank", `Ok(Some(url))` the normalized URL, `Err(())` invalid.
pub fn normalize_profile_url(raw: &str, host: &str) -> Result<Option<String>, ()> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let url = if s.to_lowercase().starts_with("http://") || s.to_lowercase().starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    if url.to_lowercase().contains(host) {
        Ok(Some(url))
    } else {
        Err(())
    }
}

/// The instruction handed to a coding agent to review one submission. Shared by the
/// per-row copy button on /admin and the bulk .txt export, so both emit the same text.
pub fn review_prompt(repo_url: &str, goal: &str) -> String {
    format!(
        "Project: git clone {repo}\nGoals: \"{goal}\"\ncheck if this goal is achieved, \
         for anything you can't test yourself, give a short concise report at the end and tell me",
        repo = repo_url.trim(), goal = goal.trim(),
    )
}

/// Nickname rules, one place. Letters (Turkish included), digits, `_` and `-`; no
/// spaces, so it always fits the leaderboard row.
pub fn validate_nickname(n: &str) -> Result<String, &'static str> {
    let n = n.trim();
    let len = n.chars().count();
    if len < 2 || len > 20 {
        return Err("Nickname 2-20 karakter olmalı.");
    }
    if !n.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err("Nickname yalnızca harf, rakam, _ ve - içerebilir (boşluk yok).");
    }
    Ok(n.to_string())
}

#[derive(FromRow)]
pub struct Video {
    pub id: Uuid,
    pub youtube_id: String,
    pub title: String,
    pub level: String,
}

#[derive(FromRow)]
pub struct VideoWithProgress {
    pub id: Uuid,
    pub youtube_id: String,
    pub title: String,
    #[allow(dead_code)] // selected by the query; videos now show a fixed combined label (VIDEO_LEVEL_LABEL)
    pub level: String,
    pub max_position: f32,
    pub duration: f32,
}

#[derive(FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub level: String,
    pub example_url: Option<String>,
    /// true = site allows iframe embedding (live preview); false/null = show cached screenshot.
    pub example_embeddable: Option<bool>,
}

/// One "Bunu yapmak isterim" flip, joined to the student's public nickname, for the
/// board's teammate list. `is_me` marks the current viewer's own row.
#[derive(FromRow)]
pub struct InterestRow {
    pub task_id: Uuid,
    pub nickname: String,
    pub is_me: bool,
}

#[derive(FromRow)]
pub struct SubmissionView {
    pub id: Uuid,
    pub task_id: Uuid,
    pub repo_url: String,
    pub status: String,
    pub feedback: Option<String>,
    pub demo_video_url: Option<String>,
    /// Null on submissions made before plan.md became required.
    pub plan_md: Option<String>,
    pub display_name: String,
    pub email: String,
    pub task_title: String,
    /// The task's level, so the admin row can show the level default as the
    /// placeholder next to the manual point box.
    pub task_level: String,
    /// Admin-entered points for this submission. `None` = score it with the level
    /// default, which is what almost every row does.
    pub points_override: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// One student's standing. Points: 20 per completed video; passed projects are
/// level-weighted (Beginner/Intermediate/Advanced = 100/400/700) and summed server-side into
/// `project_points`. `projects` is the plain count, kept for the "X proje" label.
#[derive(FromRow)]
pub struct LeaderRow {
    pub id: Uuid,
    /// The board shows both: real name first, nickname in parentheses after it.
    pub display_name: String,
    /// Non-null: the query filters to onboarded students, who by definition have one.
    pub nickname: String,
    pub videos: i64,
    pub projects: i64,
    /// Level-weighted sum of passed projects, computed in `leader_rows`' SQL.
    pub project_points: i64,
    /// Intern/staff account: scored like everyone else, but kept out of the published
    /// standings. `leader_rows` returns these so a hidden student can still be shown
    /// her own points; every student-facing render filters them out first.
    pub hidden: bool,
}

pub const PTS_VIDEO: i64 = 20;
/// Default passed-project points by level, used when a submission has no
/// `points_override`. `leader_rows` binds these into its CASE, so changing a
/// number here changes both the scoring and what the site says it awards.
pub const PTS_PROJECT_L1: i64 = 100; // Beginner / PRESEED
pub const PTS_PROJECT_L2: i64 = 400; // Intermediate / SEED
pub const PTS_PROJECT_L3: i64 = 700; // Advanced / SERIES_A

/// The level default a passed project is worth with no manual override.
pub fn level_points(level: &str) -> i64 {
    match level {
        "PRESEED" => PTS_PROJECT_L1,
        "SEED" => PTS_PROJECT_L2,
        "SERIES_A" => PTS_PROJECT_L3,
        _ => 0,
    }
}

impl LeaderRow {
    pub fn points(&self) -> i64 {
        self.videos * PTS_VIDEO + self.project_points
    }
}

// ---- Haftalık program ----

/// The two student groups that run their own schedule: (url key, what students see).
/// The key is also the primary key of schedule_image_exposure_academy, so it is baked
/// into a CHECK constraint — rename the right-hand side freely, never the left.
pub const SCHEDULE_TRACKS: [(&str, &str); 2] = [("beginner", "Beginner"), ("advanced", "Advanced")];

/// A track we're willing to look up, matched case-insensitively. Anything else falls
/// back to the first, so a hand-typed `?track=` renders a page instead of an error.
pub fn valid_schedule_track(t: Option<&str>) -> &'static str {
    let want = t.unwrap_or_default().trim().to_ascii_lowercase();
    SCHEDULE_TRACKS.iter().find(|(k, _)| *k == want).map(|(k, _)| *k).unwrap_or(SCHEDULE_TRACKS[0].0)
}

pub fn schedule_track_name(t: &str) -> &'static str {
    SCHEDULE_TRACKS.iter().find(|(k, _)| *k == t).map(|(_, v)| *v).unwrap_or("?")
}

/// What's on file for one track — never the bytes, which are only ever streamed
/// straight out of `/schedule/image/{track}`. `uploaded_at` doubles as the image's
/// cache-busting version, so a re-upload is visible immediately despite a long
/// max-age on the image response.
#[derive(FromRow)]
pub struct ScheduleImage {
    pub track: String,
    pub content_type: String,
    pub uploaded_at: DateTime<Utc>,
    pub bytes: i64,
}

impl ScheduleImage {
    /// Cache key for the <img> src: changes exactly when a new image is uploaded.
    pub fn version(&self) -> i64 {
        self.uploaded_at.timestamp()
    }
}

/// One row in the admin "Öğrenciler" list — enough to identify and remove a member.
#[derive(FromRow)]
pub struct MemberRow {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
    pub nickname: Option<String>,
    pub is_admin: bool,
    /// Hidden from the leaderboard and the board's teammate chips (intern accounts).
    pub hidden_from_leaderboard: bool,
}

#[derive(FromRow)]
pub struct StatRow {
    pub display_name: String,
    pub video_title: String,
    pub seconds_watched: f32,
    pub max_position: f32,
    pub duration: f32,
    pub updated_at: DateTime<Utc>,
}
