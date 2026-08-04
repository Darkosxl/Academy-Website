use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

pub use benchmark_protocol::BENCHMARK_VERSION as HARNESS_VERSION;

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

pub const GRADES: [&str; 4] = [
    "9'a geçiyor",
    "10'a geçiyor",
    "11'e geçiyor",
    "12'ye geçiyor",
];

/// An optional public-profile link (GitHub / LinkedIn). Empty is allowed — the step
/// is skippable. If present, we tolerate a missing scheme (prepend `https://`) and
/// require the expected host so the board can trust it enough to render as a link.
/// `Ok(None)` means "left blank", `Ok(Some(url))` the normalized URL, `Err(())` invalid.
pub fn normalize_profile_url(raw: &str, host: &str) -> Result<Option<String>, ()> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let url = if s.to_lowercase().starts_with("http://") || s.to_lowercase().starts_with("https://")
    {
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
        repo = repo_url.trim(),
        goal = goal.trim(),
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
    if !n
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
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
    #[allow(dead_code)]
    // selected by the query; videos now show a fixed combined label (VIDEO_LEVEL_LABEL)
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

/// A student's saved links for one Beginner Track project (`BEGINNER_PROJECTS` in html.rs).
/// Self-reported, ungraded — an upsert per (user, project_key), so resubmitting replaces
/// rather than piles up.
#[derive(FromRow)]
pub struct BeginnerSubmission {
    pub project_key: String,
    pub repo_url: String,
    pub vercel_url: String,
}

/// One student's line on the admin's per-project submission table. Left-joined, so a
/// student who hasn't handed in that project is still a row with `None` links — the
/// point of the page is seeing who is missing as much as who isn't.
#[derive(FromRow)]
pub struct BeginnerStudentRow {
    pub display_name: String,
    pub repo_url: Option<String>,
    pub vercel_url: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// How many students have handed in one project — the count on the project list.
#[derive(FromRow)]
pub struct BeginnerProjectCount {
    pub project_key: String,
    pub submitted: i64,
}

/// One student's deployed site for a task — a card in the /board/sites/{task_id} gallery.
/// Its own narrow projection rather than a widened SubmissionView: the gallery needs the
/// public nickname and nothing about review status, and SubmissionView carries neither.
#[derive(FromRow)]
pub struct SiteCard {
    /// The submission id — /preview/sub/{id} keys the cached screenshot off it.
    pub id: Uuid,
    /// Non-null: the query filters to students who have one, same rule as the leaderboard.
    pub nickname: String,
    pub repo_url: String,
    /// Non-null: the query filters on `live_url is not null`.
    pub live_url: String,
    /// true = site allows iframe embedding (live preview); false/null = cached screenshot.
    pub live_embeddable: Option<bool>,
}

/// What a student sees about their own latest submission to a task, on /board.
///
/// This used to be the admin grading row as well, and carried the joined student name,
/// e-mail, task title, level and points for that second reader. Grading moved to Görev
/// Puanlama and reads `GradeRow` instead, so those columns are gone — a student's own
/// board never needed them, and they were being joined and shipped on every board load.
#[derive(FromRow)]
pub struct SubmissionView {
    pub task_id: Uuid,
    pub status: String,
    pub feedback: Option<String>,
    pub demo_video_url: Option<String>,
    /// Null on submissions made before plan.md became required.
    pub plan_md: Option<String>,
}

/// One gradeable row on Görev Puanlama. Görev Panosu submissions and Beginner Track
/// projects are two different tables with two different keys, but they are the same job
/// to an admin, so `grading.rs` flattens both into this shape with a `union all` and the
/// page renders one queue.
///
/// Beginner Track rows fill the gaps rather than widening the struct with options: they
/// have no task uuid, no plan.md, and no level of their own — they are Beginner by
/// definition, so `task_level` is hardcoded 'PRESEED' and picks up the 100-point default
/// through the same `level_points` path as every board row.
#[derive(FromRow)]
pub struct GradeRow {
    /// "board" | "beginner" — which table the row came from, and therefore which
    /// primary key `key` holds and which UPDATE the review handler runs.
    pub kind: String,
    /// Opaque row identity for the review form: the submission uuid for board rows,
    /// `{user_id}:{project_key}` for beginner ones, which have a composite PK.
    pub key: String,
    pub display_name: String,
    pub email: String,
    /// Board rows only — the review prompt needs the task's Tanım, which is looked up
    /// from the `tasks` slice in memory rather than widening this query.
    pub task_id: Option<Uuid>,
    /// Board rows carry the task title; beginner rows carry the `project_key`, which the
    /// renderer maps through `BEGINNER_PROJECTS` — SQL has no business knowing those titles.
    pub task_title: String,
    pub task_level: String,
    pub repo_url: String,
    /// Board: the resolved/overridden live site. Beginner: the student's `vercel_url`,
    /// which is student-owned and therefore read-only here.
    pub live_url: Option<String>,
    /// Always null on beginner rows — that flow has no plan.md requirement.
    pub plan_md: Option<String>,
    pub status: String,
    pub feedback: Option<String>,
    /// `None` = score it with the level default, which is what almost every row does.
    pub points_override: Option<i32>,
    pub created_at: DateTime<Utc>,
    /// How many attempts exist at this (öğrenci, görev); >1 draws the "N. deneme" marker.
    /// Always 1 for beginner rows — their PK is (user_id, project_key), so a resubmit
    /// replaces rather than piles up.
    pub attempts: i64,
}

/// (database value, what the admin sees) for a submission's review state. Same four
/// values for board and Beginner Track rows — both tables share the CHECK constraint.
pub const GRADE_STATUSES: [(&str, &str); 4] = [
    ("pending", "İnceleme bekleniyor"),
    ("reviewing", "İnceleniyor"),
    ("passed", "Geçti"),
    ("failed", "Başarısız"),
];

pub fn is_grade_status(s: &str) -> bool {
    GRADE_STATUSES.iter().any(|(k, _)| *k == s)
}

/// The Durum chips. "Bekleyenler" folds pending+reviewing together because the split
/// between them is a worker-pipeline detail, not something an admin grades differently.
pub const DURUM_FILTERS: [(&str, &str); 4] = [
    ("bekleyen", "Bekleyenler"),
    ("hepsi", "Hepsi"),
    ("passed", "Geçti"),
    ("failed", "Başarısız"),
];

/// Opening the page with no `?durum=` lands on the queue, not the archive — the whole
/// point of the split is that "what is waiting on me" is one glance, not a scroll.
pub const DURUM_DEFAULT: &str = "bekleyen";

pub fn durum_matches(durum: &str, status: &str) -> bool {
    match durum {
        "bekleyen" => status == "pending" || status == "reviewing",
        "hepsi" => true,
        exact => status == exact,
    }
}

/// Validated filter state for Görev Puanlama. Built once per request from the query
/// string (or from a form's hidden fields), then used both to run the query and to build
/// every link on the page, so a chip can never disagree with what is on screen.
#[derive(Clone)]
pub struct Filters {
    /// One of `DURUM_FILTERS`. Never empty — absent parses to `DURUM_DEFAULT`.
    pub durum: String,
    pub seviye: Option<String>,
    pub ogrenci: Option<Uuid>,
    /// true = show every attempt; false = newest attempt per (öğrenci, görev) only.
    pub tum: bool,
}

impl Filters {
    /// `?a=b&c=d`, or "" when nothing but the default is set. Only ever emits values this
    /// struct has already validated, which is what keeps it safe to put in a `Location`.
    pub fn query_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.durum != DURUM_DEFAULT {
            parts.push(format!("durum={}", self.durum));
        }
        if let Some(s) = &self.seviye {
            parts.push(format!("seviye={s}"));
        }
        if let Some(u) = self.ogrenci {
            parts.push(format!("ogrenci={u}"));
        }
        if self.tum {
            parts.push("tum=1".into());
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }

    pub fn url(&self) -> String {
        format!("/admin/puanlama{}", self.query_string())
    }

    /// The same page with one filter swapped — how every chip builds its href, so
    /// changing Seviye keeps your Durum and Öğrenci rather than resetting them.
    pub fn with_durum(&self, durum: &str) -> Filters {
        Filters {
            durum: durum.to_string(),
            ..self.clone()
        }
    }

    pub fn with_seviye(&self, seviye: Option<&str>) -> Filters {
        Filters {
            seviye: seviye.map(str::to_string),
            ..self.clone()
        }
    }

    /// True when any filter is narrowing the view, i.e. when "Filtreleri temizle" would
    /// actually do something. The default queue view is not "filtered" for this purpose.
    pub fn is_narrowed(&self) -> bool {
        self.durum != DURUM_DEFAULT || self.seviye.is_some() || self.ogrenci.is_some() || self.tum
    }
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

// ---- Haftalık program ----

/// The two student groups that run their own schedule: (url key, what students see).
/// The key is also the primary key of schedule_image_exposure_academy, so it is baked
/// into a CHECK constraint — rename the right-hand side freely, never the left.
pub const SCHEDULE_TRACKS: [(&str, &str); 2] = [("beginner", "Beginner"), ("advanced", "Advanced")];

/// A track we're willing to look up, matched case-insensitively. Anything else falls
/// back to the first, so a hand-typed `?track=` renders a page instead of an error.
pub fn valid_schedule_track(t: Option<&str>) -> &'static str {
    let want = t.unwrap_or_default().trim().to_ascii_lowercase();
    SCHEDULE_TRACKS
        .iter()
        .find(|(k, _)| *k == want)
        .map(|(k, _)| *k)
        .unwrap_or(SCHEDULE_TRACKS[0].0)
}

pub fn schedule_track_name(t: &str) -> &'static str {
    SCHEDULE_TRACKS
        .iter()
        .find(|(k, _)| *k == t)
        .map(|(_, v)| *v)
        .unwrap_or("?")
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

// ---- Konum / adres ----

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
        [
            &self.dates,
            &self.name,
            &self.address,
            &self.maps_url,
            &self.notes,
        ]
        .iter()
        .all(|f| f.trim().is_empty())
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

/// The consent forms, in the order students see them:
/// `(key, title, what it is for, where the blank form lives)`.
/// The key is the `kind` column and is baked into a CHECK constraint, so everything to
/// its right changes freely — the key itself never does.
///
/// The URL here is only the default. `/admin` stores an override in
/// app_settings_exposure_academy (`consent_url_<kind>`), which is how Paribu's link gets
/// added when the document exists — a form field, not a deploy. Empty = no download
/// button on the card.
pub const CONSENT_DOCS: [(&str, &str, &str, &str); 3] = [
    (
        "exposure",
        "Exposure AI Academy Veli İzin ve Katılım Formu",
        "Programa katılım için veli/yasal temsilci onayı.",
        "https://drive.google.com/file/d/1gkxQLuguXfVFmjjjjcBF0Y39JTKeFXr6/view?usp=sharing",
    ),
    (
        "qnbeyond",
        "QNBEYOND Lokasyon/Katılım İzin Formu",
        "1. haftanın yapılacağı QNBEYOND lokasyonu için veli/yasal temsilci onayı.",
        "https://drive.google.com/file/d/10YgFIm28qjhTy3stEXS5BukS_tmn-9_a/view?usp=sharing",
    ),
    (
        "paribu",
        "Paribu Lokasyon/Katılım İzin Formu",
        "Programın 2. haftasında kullanılacak. Form hazır olduğunda paylaşılacak.",
        "",
    ),
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
    CONSENT_DOCS
        .iter()
        .find(|(key, ..)| *key == want)
        .map(|(key, ..)| *key)
}

pub fn consent_title(kind: &str) -> &'static str {
    CONSENT_DOCS
        .iter()
        .find(|(k, ..)| *k == kind)
        .map(|(_, t, ..)| *t)
        .unwrap_or("?")
}

/// Settings key holding whether this form is closed for uploads.
pub fn consent_lock_key(kind: &str) -> String {
    format!("consent_lock_{kind}")
}

/// Settings key holding where the blank form can be downloaded.
pub fn consent_url_key(kind: &str) -> String {
    format!("consent_url_{kind}")
}

/// A Google Drive share link turned into one that downloads the file instead of opening
/// the preview page. Anything that isn't a recognisable Drive file link is returned as-is,
/// so pasting a plain PDF URL keeps working.
///
/// Both shapes Drive hands out are accepted: `/file/d/<id>/view` and `open?id=<id>`.
pub fn direct_download_url(url: &str) -> String {
    let u = url.trim();
    if !u.contains("drive.google.com") {
        return u.to_string();
    }
    let id = u
        .split_once("/file/d/")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        .or_else(|| {
            u.split_once("id=")
                .map(|(_, rest)| rest.split('&').next().unwrap_or(""))
        })
        .unwrap_or("");
    // an id we can't find means a Drive URL shaped some way we don't know — leave it be
    // rather than building a link that 404s
    if id.is_empty() {
        u.to_string()
    } else {
        format!("https://drive.google.com/uc?export=download&id={id}")
    }
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
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(
                    c,
                    '/' | '\\' | '"' | '\'' | ':' | '*' | '?' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['.', ' ', '_']).to_string();
    if cleaned.is_empty() {
        "belge".to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}

// ---- Agentic Harness ----

#[derive(FromRow, Clone)]
pub struct HarnessTeam {
    pub id: Uuid,
    pub name: String,
}

/// Long enough for a real team name, short enough that it can't blow out the
/// leaderboard column or the podium card. Enforced server-side and as `maxlength`.
pub const HARNESS_TEAM_NAME_MAX: usize = 40;

/// New submissions are one benchmark plus RAM; legacy bundled rows keep all three results.
#[derive(FromRow)]
pub struct HarnessRun {
    pub id: Uuid,
    pub repo_url: String,
    pub model_id: String,
    pub provider: String,
    pub benchmark_kind: String,
    pub commit_sha: Option<String>,
    pub stage: String,
    pub benchmark_version: String,
    pub benchmark_state: serde_json::Value,
    pub bedrock_profile: Option<String>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub score_arc: Option<f32>,
    pub score_frontier: Option<f32>,
    pub ram_1session_mb: Option<f32>,
    pub ram_10session_mb: Option<f32>,
    pub error_log: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One team's best score on ARC-AGI-3 or Frontier-bench (higher = better).
#[derive(FromRow)]
pub struct HarnessLeaderRow {
    pub id: Uuid,
    pub name: String,
    pub best: f32,
}

/// One team's best RAM-bench result: ranked by 10-session PSS (lower = better),
/// the 1-session value comes from the same run that achieved that minimum.
#[derive(FromRow)]
pub struct HarnessRamRow {
    pub id: Uuid,
    pub name: String,
    pub ram_1session_mb: f32,
    pub ram_10session_mb: f32,
}

/// (team, member) pair for the leaderboard's kid-names line and the admin list.
/// Shared by both team-based sections — the harness and AI Monopoly.
#[derive(FromRow)]
pub struct TeamMemberRow {
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    /// Onboarded and not hidden: eligible for the public leaderboard line. The
    /// own-team panel ignores this and shows the whole roster.
    pub public: bool,
}

#[derive(FromRow)]
pub struct HarnessActiveRun {
    pub id: Uuid,
    pub team_name: String,
    pub stage: String,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub struct HarnessKaggleSubmission {
    pub run_id: Uuid,
    pub status: String,
    pub kernel_slug: Option<String>,
    pub kernel_version: Option<i32>,
    pub submission_ref: Option<String>,
    pub public_score: Option<f32>,
    pub private_score: Option<f32>,
    pub status_message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// One rejected harness submission, for the /admin panel. `display_name` and `team_name` are
/// `Option` because both FKs are `on delete set null` — a rejection outlives the account that
/// made it, which is exactly when it's still worth reading.
#[derive(FromRow)]
pub struct HarnessRejectedRow {
    pub raw_input: String,
    pub reason: String,
    pub display_name: Option<String>,
    pub team_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Interim admin-side run management. Team membership moved to /admin/takimlar
/// (`teams.rs`); what stays here is the stuck-run escape hatch, plus the rejected-submission
/// log that answers "but I *did* send a GitHub link".
pub struct HarnessAdmin {
    pub active_runs: Vec<HarnessActiveRun>,
    pub rejected: Vec<HarnessRejectedRow>,
}

/// One draggable card on the Takım formasyonu board. `team_id` is `None` for a kid
/// who is on no team — the state the board exists to make visible, and the one the
/// admin dropdown could never show.
#[derive(FromRow)]
pub struct FormationKid {
    pub id: Uuid,
    pub display_name: String,
    pub team_id: Option<Uuid>,
}

// ---- AI Monopoly ----

/// What every merchant starts with, in ₺. Net worth = cash + goods, and that is the
/// leaderboard. Tuning the game after a dry run should mean editing this line.
pub const MONOPOLY_START_CASH: i32 = 1000;
/// The seller's cost of goods as a percent of the sale price; the seller keeps the rest.
/// This is what stops a sale from being free money and makes dumping stock a losing move.
pub const MONOPOLY_COST_RATE_PCT: i32 = 40;
/// Turns per side in one conversation. Either side may stop sooner by saying `[END]`.
pub const MONOPOLY_MAX_TURNS: i32 = 10;
/// Submission cap on summed safetensors bytes. VRAM is the real constraint and bytes
/// measure it directly, with no architecture-specific parameter arithmetic to get wrong.
/// 64 GiB ≈ 31B at bf16 — one 80 GB GPU per model.
pub const MONOPOLY_SIZE_CAP_BYTES: i64 = 64 * 1024 * 1024 * 1024;

/// Tournament status ladder. Single source for the worker API's expected-predecessor
/// guard (main.rs) and the stepper renderer (html.rs); `done`/`failed` are terminal.
pub const MONOPOLY_STATUSES: [&str; 7] = [
    "queued", "booting", "loading", "running", "judging", "done", "failed",
];

/// Both ledgers after one judged sale. Pure, so the money math has exactly one
/// implementation and one test — the worker never computes any of this.
pub struct Sale {
    /// The price actually charged, i.e. post-clamp. This is what gets stored, so the
    /// transaction row can never claim more changed hands than the buyer had.
    pub price: i32,
    /// The value actually booked, post-clamp — stored for the same reason as `price`.
    pub value: i32,
    pub buyer_cash: i32,
    pub buyer_goods: i32,
    pub seller_cash: i32,
}

/// How many times the price an item may be judged to be worth. A great negotiation is
/// paying 100 for something worth 300; beyond that the judge is not valuing goods, it is
/// being talked into a number. Without this bound a model that gets "worth 999999 to you"
/// past the judge wins outright, because net worth counts goods.
pub const MONOPOLY_VALUE_CAP_MULT: i32 = 3;

/// Apply a sale: the buyer pays what they can, books the item at the judge's read of
/// what it is worth to them, and the seller keeps the price less the cost of goods.
///
/// Both clamps here are the defence against a prompt-injected judge reaching the ledger:
/// the price can't exceed what the buyer actually holds, and the value can't exceed a
/// fixed multiple of the price actually paid. A buyer who paid nothing gains nothing.
pub fn apply_sale(
    buyer_cash: i32,
    buyer_goods: i32,
    seller_cash: i32,
    price: i32,
    value_to_buyer: i32,
) -> Sale {
    let price = price.clamp(0, buyer_cash.max(0));
    let value = value_to_buyer.clamp(0, price.saturating_mul(MONOPOLY_VALUE_CAP_MULT));
    Sale {
        price,
        value,
        buyer_cash: buyer_cash - price,
        buyer_goods: buyer_goods + value,
        seller_cash: seller_cash + price * (100 - MONOPOLY_COST_RATE_PCT) / 100,
    }
}

/// Circle-method round-robin over `n` players, `rounds` rounds deep, as index pairs.
///
/// A full cycle is `n-1` rounds (`n` when odd, where one player sits out each round);
/// asking for more than that repeats from the start, which is what makes the rule "n
/// rounds for n players" well-defined. With 4 teams that means rounds 1-3 are a complete
/// round-robin and round 4 replays round 1.
pub fn round_robin(n: usize, rounds: usize) -> Vec<Vec<(usize, usize)>> {
    if n < 2 {
        return vec![Vec::new(); rounds];
    }
    // pad to even with a ghost; any pair containing it is a bye and gets dropped
    let m = if n % 2 == 0 { n } else { n + 1 };
    let mut seats: Vec<usize> = (0..m).collect();
    let mut out = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        out.push(
            (0..m / 2)
                .map(|i| (seats[i], seats[m - 1 - i]))
                .filter(|(a, b)| *a < n && *b < n)
                .collect(),
        );
        // rotate everything but the first seat
        let last = seats.pop().unwrap();
        seats.insert(1, last);
    }
    out
}

#[derive(FromRow, Clone)]
pub struct MonopolyTeam {
    pub id: Uuid,
    pub name: String,
}

/// A team's current entry: which weights answer for them, and the merchant those
/// weights play. Replaced wholesale on resubmit, which is allowed until the tournament
/// starts — at which point it is snapshotted into a `MonopolyPlayer` and stops mattering.
#[derive(FromRow, Clone)]
pub struct MonopolyEntry {
    pub id: Uuid,
    pub team_id: Uuid,
    pub hf_repo: String,
    pub hf_revision: Option<String>,
    pub size_bytes: Option<i64>,
    pub char_name: String,
    pub product_name: String,
    pub product_desc: String,
    pub list_price: i32,
    pub persona: String,
    pub updated_at: DateTime<Utc>,
}

/// A frozen profile plus its ledger. Matches, messages and transactions all reference
/// players rather than entries, which is what keeps a finished tournament truthful after
/// a team resubmits — and makes a practice decoy an ordinary row instead of a special case.
#[derive(FromRow, Clone)]
pub struct MonopolyPlayer {
    pub id: Uuid,
    pub team_id: Uuid,
    pub char_name: String,
    pub product_name: String,
    pub product_desc: String,
    pub list_price: i32,
    pub cash: i32,
    pub goods: i32,
}

impl MonopolyPlayer {
    /// The leaderboard figure. Cash alone loses: it never grows on its own.
    pub fn net_worth(&self) -> i32 {
        self.cash + self.goods
    }
}

/// A player as the runner needs it: which weights to serve, the profile that becomes the
/// system prompt, and the ledger it reports its own balance from.
#[derive(FromRow)]
pub struct MonopolyPlayerFull {
    pub id: Uuid,
    pub hf_repo: String,
    pub hf_revision: Option<String>,
    pub char_name: String,
    pub product_name: String,
    pub product_desc: String,
    pub list_price: i32,
    pub persona: String,
    pub cash: i32,
    pub goods: i32,
}

/// One row of the standings — a player joined to the team that owns it.
#[derive(FromRow)]
pub struct MonopolyStandingRow {
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_name: String,
    pub char_name: String,
    pub product_name: String,
    pub cash: i32,
    pub goods: i32,
}

impl MonopolyStandingRow {
    pub fn net_worth(&self) -> i32 {
        self.cash + self.goods
    }
}

#[derive(FromRow)]
pub struct MonopolyTournament {
    pub id: Uuid,
    pub status: String,
    pub round: i32,
    pub rounds_total: i32,
    pub progress: Option<String>,
    pub error_log: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One conversation, with both sides' character names resolved for the listing.
#[derive(FromRow)]
pub struct MonopolyMatchRow {
    pub id: Uuid,
    pub round: i32,
    pub kind: String,
    pub status: String,
    pub a_player: Uuid,
    pub a_name: String,
    pub b_player: Uuid,
    pub b_name: String,
    pub created_at: DateTime<Utc>,
}

/// One completed turn. `speaker` is "a" or "b", matching the match's two sides.
#[derive(FromRow)]
pub struct MonopolyMessage {
    pub seq: i32,
    pub speaker: String,
    pub content: String,
}

/// A judged sale as shown in the verdict panel and the history tab.
#[derive(FromRow)]
pub struct MonopolyTxRow {
    pub buyer_name: String,
    pub seller_name: String,
    pub item: String,
    pub price: i32,
    pub value_to_buyer: i32,
    pub reasoning: Option<String>,
}

impl MonopolyTxRow {
    /// What the buyer gained or lost on the deal. Negative means they were talked into
    /// overpaying, which is exactly the thing the game is trying to teach.
    pub fn surplus(&self) -> i32 {
        self.value_to_buyer - self.price
    }
}

/// What one model wrote about the other after they spoke. Withheld from students until
/// the tournament is `done`, then shown in history as the post-game reveal.
#[derive(FromRow)]
pub struct MonopolyNoteRow {
    pub author_name: String,
    pub about_name: String,
    pub round: i32,
    pub note: String,
}

/// Admin-side Monopoly management, bundled so `html::admin`'s signature grows by one
/// parameter rather than six. Mirrors `HarnessAdmin`.
pub struct MonopolyAdmin {
    pub teams: Vec<MonopolyTeam>,
    pub members: Vec<TeamMemberRow>,
    pub entries: Vec<MonopolyEntry>,
    pub tournament: Option<MonopolyTournament>,
}

/// Decoy merchants worn by practice opponents. A practice match runs against another
/// team's real model, but that model is briefed as one of these instead of with its own
/// profile — so practising can neither leak what a team really sells nor let anyone
/// rehearse against the merchant they'll actually face. The chosen decoy is written into
/// the practice player row, so re-opening a match always shows the same merchant.
/// (char_name, product_name, product_desc, list_price, persona)
pub const MONOPOLY_DECOYS: [(&str, &str, &str, i32, &str); 12] = [
    (
        "Marisol Vega",
        "Sunrise Coffee Beans",
        "Small-batch beans, roasted the same morning they ship.",
        120,
        "Warm and chatty, opens with small talk, but never drops below the price that keeps the farm running.",
    ),
    (
        "Otto Brandt",
        "Clockwork Umbrella",
        "An umbrella that opens itself the moment it senses rain.",
        340,
        "Precise and a little pompous. Quotes engineering details nobody asked for. Flattery works on him.",
    ),
    (
        "Priya Raman",
        "Silent Bicycle Chain",
        "A bicycle chain that makes no sound at all, at any speed.",
        260,
        "Blunt and fast. Names a price once, repeats it, and lets silence do the haggling.",
    ),
    (
        "Jonah Fisk",
        "Deep Harbour Salt",
        "Sea salt harvested once a year, on the winter tide.",
        90,
        "Weathered and superstitious. Talks about the sea. Sells cheaper to anyone who seems honest.",
    ),
    (
        "Elif Kaya",
        "Pocket Greenhouse",
        "A glass box that keeps one plant alive anywhere, in any weather.",
        410,
        "Enthusiastic to a fault. Oversells the product, then panics and discounts it.",
    ),
    (
        "Sam Okoro",
        "Everlast Notebook",
        "Paper that erases completely and can be written on forever.",
        150,
        "Dry and practical. Asks what you need it for before quoting anything.",
    ),
    (
        "Nadia Sorokin",
        "Night Map Lantern",
        "A lantern that projects tonight's constellations onto the ceiling.",
        300,
        "Soft-spoken and poetic about the night sky. Hates being rushed; walks away if pushed.",
    ),
    (
        "Theo Lambert",
        "Second Breakfast Jam",
        "Jam made only from fruit picked after midnight.",
        75,
        "Cheerful and generous with samples. Bundles things together rather than cutting the price.",
    ),
    (
        "Rosa Delgado",
        "Featherweight Toolkit",
        "A complete toolkit that weighs less than a phone.",
        520,
        "Impatient. Assumes you already know the value. Respects a buyer who counters hard.",
    ),
    (
        "Kwame Asante",
        "Memory Kettle",
        "A kettle that remembers exactly how each person likes their tea.",
        280,
        "Grandfatherly, tells long stories, and quietly raises the price while telling them.",
    ),
    (
        "Ingrid Halvorsen",
        "Driftwood Speaker",
        "A speaker carved from wood pulled out of the fjord.",
        640,
        "Reserved, answers in short sentences, and only warms up once a real offer is on the table.",
    ),
    (
        "Yusuf Demir",
        "Endless Pencil",
        "A pencil that sharpens itself and never gets any shorter.",
        60,
        "Playful and mischievous. Makes absurd opening offers on purpose to see how you react.",
    ),
];

#[derive(FromRow)]
pub struct StatRow {
    pub display_name: String,
    pub video_title: String,
    pub seconds_watched: f32,
    pub max_position: f32,
    pub duration: f32,
    pub updated_at: DateTime<Utc>,
}
