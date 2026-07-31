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

/// Where the academy meets — per week, because the two weeks run in different places.
/// Kept as rows in app_settings_exposure_academy (`venue{week}_{field}`) rather than
/// its own table: it is a handful of free-text fields, the same shape as the invite
/// code that already lives there, so there is no schema to keep in step.
#[derive(Default)]
pub struct Venue {
    /// 1 or 2. Every heading names it, so a student is never left guessing which
    /// week an address belongs to.
    pub week: u8,
    /// Optional date range shown beside the week number, e.g. "3–7 Ağustos".
    pub dates: String,
    pub name: String,
    pub address: String,
    /// Whatever the admin pasted out of Google Maps. Validated as http(s) on save, so
    /// it is safe to put straight in an href.
    pub maps_url: String,
    /// Anything else students need: floor, door code, transit, parking.
    pub notes: String,
}

/// The weeks that get their own address. Adding a third is this array plus nothing —
/// the settings keys, the admin panel and both pages are all driven off it.
pub const VENUE_WEEKS: [u8; 2] = [1, 2];

/// Settings key for one field of one week's venue.
pub fn venue_key(week: u8, field: &str) -> String {
    format!("venue{week}_{field}")
}

impl Venue {
    /// Nothing filled in yet. The page then says so rather than rendering an empty card.
    pub fn is_empty(&self) -> bool {
        [&self.dates, &self.name, &self.address, &self.maps_url, &self.notes]
            .iter().all(|f| f.trim().is_empty())
    }

    /// What the card is titled: "1. Hafta", or "1. Hafta · 3–7 Ağustos" once dates
    /// are filled in.
    pub fn heading(&self) -> String {
        match self.dates.trim() {
            "" => format!("{}. Hafta", self.week),
            d => format!("{}. Hafta · {}", self.week, d),
        }
    }
}

// ---- Veli onay formları ----

/// The consent forms, in the order students see them: `(key, title, what it is for)`.
/// The key is the `kind` column and is baked into a CHECK constraint, so the title and
/// the note change freely — the left-hand side never does.
pub const CONSENT_DOCS: [(&str, &str, &str); 3] = [
    ("exposure", "Exposure AI Academy Veli İzin ve Katılım Formu",
     "Programa katılım için veli/yasal temsilci onayı."),
    ("qnbeyond", "QNBEYOND Lokasyon/Katılım İzin Formu",
     "1. haftanın yapılacağı QNBEYOND lokasyonu için veli/yasal temsilci onayı."),
    ("paribu", "Paribu Lokasyon/Katılım İzin Formu",
     "Programın 2. haftasında kullanılacak. Form hazır olduğunda paylaşılacak."),
];

/// Forms that start out closed: the document itself isn't ready to hand out yet, so the
/// card is blurred and uploads are refused until an admin opens it from /admin. Stored
/// per form in app_settings, so opening one is a button, not a deploy.
pub const CONSENT_LOCKED_BY_DEFAULT: [&str; 1] = ["paribu"];

/// When the two forms that already exist have to be in. Stated on the student page and
/// in the admin panel from this one place.
pub const CONSENT_DEADLINE: &str = "3 Ağustos Pazartesi";

/// A `kind` we're willing to touch, or `None`. Everything that reaches the database or
/// a filesystem-ish name goes through here first, so a hand-rolled POST can't invent one.
pub fn valid_consent_kind(k: &str) -> Option<&'static str> {
    let want = k.trim().to_ascii_lowercase();
    CONSENT_DOCS.iter().find(|(key, ..)| *key == want).map(|(key, ..)| *key)
}

pub fn consent_title(kind: &str) -> &'static str {
    CONSENT_DOCS.iter().find(|(k, ..)| *k == kind).map(|(_, t, _)| *t).unwrap_or("?")
}

/// Settings key holding whether this form is closed for uploads.
pub fn consent_lock_key(kind: &str) -> String {
    format!("consent_lock_{kind}")
}

/// One uploaded file. Never carries the bytes — those only ever leave through
/// `/documents/file/{id}` or the admin ZIP, so listing a page of documents doesn't
/// drag megabytes out of the database.
#[derive(FromRow)]
pub struct ConsentDoc {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub filename: String,
    pub bytes: i64,
    pub uploaded_at: DateTime<Utc>,
}

impl ConsentDoc {
    /// Human size for the file list: KB up to a megabyte, then MB with one decimal.
    pub fn size_label(&self) -> String {
        if self.bytes < 1024 * 1024 {
            format!("{} KB", (self.bytes / 1024).max(1))
        } else {
            format!("{:.1} MB", self.bytes as f64 / (1024.0 * 1024.0))
        }
    }
}

/// A file name safe to put in a Content-Disposition header or a ZIP entry: no path
/// separators, quotes, control characters or leading dots, and never empty.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name.trim()
        .chars()
        .map(|c| if c.is_control() || matches!(c, '/' | '\\' | '"' | '\'' | ':' | '*' | '?' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim_matches(['.', ' ', '_']).to_string();
    if cleaned.is_empty() { "belge".to_string() } else { cleaned.chars().take(120).collect() }
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
