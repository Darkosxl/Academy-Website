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
    /// The deployed site, if we've resolved one. Null is the normal state right after
    /// submitting — the admin rescan and the manual override both fill it in later.
    pub live_url: Option<String>,
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

// ---- Agentic Harness ----

pub const HARNESS_VERSION: &str = "harness-2026-sprint-v1";

#[derive(FromRow, Clone)]
pub struct HarnessTeam {
    pub id: Uuid,
    pub name: String,
}

/// One submission = one versioned run with three independently terminal benchmarks.
#[derive(FromRow)]
pub struct HarnessRun {
    pub id: Uuid,
    pub repo_url: String,
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

/// Interim admin-side team management, bundled so `html::admin`'s signature grows
/// by one parameter. Goes away when real team onboarding lands.
pub struct HarnessAdmin {
    pub teams: Vec<HarnessTeam>,
    pub members: Vec<TeamMemberRow>,
    pub active_runs: Vec<HarnessActiveRun>,
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
