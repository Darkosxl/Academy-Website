// Server-rendered pages. Turkish strings sourced from Google Translate API.
// ponytail: string templates, no template engine — 8 pages, full control.

use crate::model::*;
use benchmark_protocol::{
    BEDROCK_MODEL_IDS, BUILTIN_HARNESSES, CEREBRAS_MODEL_IDS, DEEPINFRA_MODEL_IDS,
    DEFAULT_BEDROCK_MODEL, DEFAULT_CEREBRAS_MODEL, DEFAULT_DEEPINFRA_MODEL, ModelProvider,
    builtin_harness_label,
};
use uuid::Uuid;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// (database value, what students see). The keys stay as they are — they're baked
/// into a CHECK constraint and into every existing video/task row — so renaming a
/// level is a change to the right-hand side only, never a migration.
pub const LEVELS: [(&str, &str); 3] = [
    ("PRESEED", "Beginner"),
    ("SEED", "Intermediate"),
    ("SERIES_A", "Advanced"),
];

pub fn level_name(l: &str) -> &'static str {
    LEVELS
        .iter()
        .find(|(k, _)| *k == l)
        .map(|(_, v)| *v)
        .unwrap_or("?")
}

/// Lesson videos are all presented as one combined tier, regardless of the level
/// stored on the row (they stay PRESEED in the DB). ponytail: display-only label,
/// no data migration — change the string here if the framing changes.
const VIDEO_LEVEL_LABEL: &str = "Beginner-Intermediate";

/// Badge color modifier per level, so Beginner/Intermediate/Advanced read as distinct (blue → purple → orange,
/// mirroring the brand hero gradient). Beginner falls through to the base blue `.badge`.
fn level_badge_class(l: &str) -> &'static str {
    match l {
        "SEED" => "badge-l2",
        "SERIES_A" => "badge-l3",
        _ => "",
    }
}

/// `<option>` list for a level `<select>`; `current` gets the `selected` attribute
/// (pass "" for a fresh form — no match, browser defaults to the first).
fn level_options(current: &str) -> String {
    LEVELS
        .iter()
        .map(|(k, v)| {
            format!(
                r#"<option value="{k}"{sel}>{v}</option>"#,
                sel = if *k == current { " selected" } else { "" },
            )
        })
        .collect()
}

/// How many `rows` a `<textarea>` needs to show `text` with no internal scrollbar,
/// given it wraps at roughly `wrap_at` characters per line. Floors at 3.
fn textarea_rows(text: &str, wrap_at: usize) -> usize {
    text.lines()
        .map(|line| (line.chars().count().max(1) + wrap_at - 1) / wrap_at)
        .sum::<usize>()
        .max(3)
}

// Heroicons v2 (outline, 24x24, 1.5 stroke) — sized/colored via CSS (currentColor).
fn ico(path: &str) -> String {
    format!(
        r##"<svg class="ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="{path}"/></svg>"##
    )
}
const P_HOME: &str = "m2.25 12 8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25";
const P_BOARD: &str = "M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 0 0 2.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 0 0-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25Z";
const P_HARNESS: &str = "M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 0 0 2.25-2.25V6.75a2.25 2.25 0 0 0-2.25-2.25H6.75A2.25 2.25 0 0 0 4.5 6.75v10.5a2.25 2.25 0 0 0 2.25 2.25Zm.75-12h9v9h-9v-9Z";
const P_MONOPOLY: &str = "M14.25 6.087c0-.355.186-.676.401-.959.221-.29.349-.634.349-1.003 0-1.036-1.007-1.875-2.25-1.875s-2.25.84-2.25 1.875c0 .369.128.713.349 1.003.215.283.401.604.401.959v0a.64.64 0 0 1-.657.643 48.39 48.39 0 0 1-4.163-.3c.186 1.613.293 3.25.315 4.907a.656.656 0 0 1-.658.663v0c-.355 0-.676-.186-.959-.401a1.647 1.647 0 0 0-1.003-.349c-1.036 0-1.875 1.007-1.875 2.25s.84 2.25 1.875 2.25c.369 0 .713-.128 1.003-.349.283-.215.604-.401.959-.401v0c.31 0 .555.26.532.57a48.039 48.039 0 0 1-.642 5.056c1.518.19 3.058.309 4.616.354a.64.64 0 0 0 .657-.643v0c0-.355-.186-.676-.401-.959a1.647 1.647 0 0 1-.349-1.003c0-1.035 1.008-1.875 2.25-1.875 1.243 0 2.25.84 2.25 1.875 0 .369-.128.713-.349 1.003-.215.283-.4.604-.4.959v0c0 .333.277.599.61.58a48.1 48.1 0 0 0 5.427-.63 48.05 48.05 0 0 0 .582-4.717.532.532 0 0 0-.533-.57v0c-.355 0-.676.186-.959.401-.29.221-.634.349-1.003.349-1.035 0-1.875-1.007-1.875-2.25s.84-2.25 1.875-2.25c.37 0 .713.128 1.003.349.283.215.604.401.96.401v0a.656.656 0 0 0 .658-.663 48.422 48.422 0 0 0-.37-5.36c-1.676.24-3.37.404-5.082.484a.638.638 0 0 1-.667-.643v0Z";
const P_ADMIN: &str = "M11.42 15.17 17.25 21A2.652 2.652 0 0 0 21 17.25l-5.877-5.877M11.42 15.17l2.496-3.03c.317-.384.74-.626 1.208-.766M11.42 15.17l-4.655 5.653a2.548 2.548 0 1 1-3.586-3.586l6.837-5.63m5.108-.233c.55-.164 1.163-.188 1.743-.14a4.5 4.5 0 0 0 4.486-6.336l-3.276 3.277a3.004 3.004 0 0 1-2.25-2.25l3.276-3.276a4.5 4.5 0 0 0-6.336 4.486c.091 1.076-.071 2.264-.904 2.95l-.102.085m-1.745 1.437L5.909 7.5H4.5L2.25 3.75l1.5-1.5L7.5 4.5v1.409l4.26 4.26m-1.745 1.437 1.745-1.437m6.615 8.206L15.75 15.75M4.867 19.125h.008v.008h-.008v-.008Z";
const P_TEAMS: &str = "M18 18.72a9.094 9.094 0 0 0 3.741-.479 3 3 0 0 0-4.682-2.72m.94 3.198.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0 1 12 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 0 1 6 18.719m12 0a5.971 5.971 0 0 0-.941-3.197m0 0A5.995 5.995 0 0 0 12 12.75a5.995 5.995 0 0 0-5.058 2.772m0 0a3 3 0 0 0-4.681 2.72 8.986 8.986 0 0 0 3.74.477m.94-3.197a5.971 5.971 0 0 0-.94 3.197M15 6.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm6 3a2.25 2.25 0 1 1-4.5 0 2.25 2.25 0 0 1 4.5 0Zm-13.5 0a2.25 2.25 0 1 1-4.5 0 2.25 2.25 0 0 1 4.5 0Z";
const P_LOGOUT: &str = "M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15M12 9l-3 3m0 0 3 3m-3-3h12.75";
const P_DEMO: &str = "m3.75 13.5 10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75Z";
const P_TROPHY: &str = "M16.5 18.75h-9m9 0a3 3 0 0 1 3 3h-15a3 3 0 0 1 3-3m9 0v-3.375c0-.621-.503-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.007 0H9.497m5.007 0a7.454 7.454 0 0 1-.982-3.172M9.497 14.25a7.454 7.454 0 0 0 .981-3.172M5.25 4.236c-.982.143-1.954.317-2.916.52A6.003 6.003 0 0 0 7.73 9.728M5.25 4.236V4.5c0 2.108.966 3.99 2.48 5.228M5.25 4.236V2.721C7.456 2.41 9.71 2.25 12 2.25c2.291 0 4.545.16 6.75.47v1.516M7.73 9.728a6.726 6.726 0 0 0 2.748 1.35m8.272-6.842V4.5c0 2.108-.966 3.99-2.48 5.228m2.48-5.492a46.32 46.32 0 0 1 2.916.52 6.003 6.003 0 0 1-5.395 4.972m0 0a6.726 6.726 0 0 1-2.749 1.35m0 0a6.772 6.772 0 0 1-3.044 0";
const P_PLAY: &str = "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z M15.91 11.672a.375.375 0 0 1 0 .656l-5.603 3.113a.375.375 0 0 1-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112Z";
const P_MENU: &str = "M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5";
const P_UPLOAD: &str = "M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5m-13.5-9L12 3m0 0 4.5 4.5M12 3v13.5";
const P_CLOSE: &str = "M6 18 18 6M6 6l12 12";
const P_LOCK: &str = "M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z";
const P_CAL: &str = "M6.75 3v2.25M17.25 3v2.25M3 18.75V7.5a2.25 2.25 0 0 1 2.25-2.25h13.5A2.25 2.25 0 0 1 21 7.5v11.25m-18 0A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75m-18 0v-7.5A2.25 2.25 0 0 1 5.25 9h13.5A2.25 2.25 0 0 1 21 11.25v7.5";
const P_PIN: &str = "M15 10.5a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z M19.5 10.5c0 7.142-7.5 11.25-7.5 11.25S4.5 17.642 4.5 10.5a7.5 7.5 0 1 1 15 0Z";
const P_DOC: &str = "M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z";
const P_DOWNLOAD: &str = "M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3";
const P_TRASH: &str = "m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0";
const P_GLOBE: &str = "M12 21a9.004 9.004 0 0 0 8.716-6.747M12 21a9.004 9.004 0 0 1-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 0 1 7.843 4.582M12 3a8.997 8.997 0 0 0-7.843 4.582m15.686 0A11.953 11.953 0 0 1 12 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0 1 21 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0 1 12 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 0 1 3 12c0-1.605.42-3.113 1.157-4.418";
const P_FLAG: &str = "M3 3v1.5M3 21v-6m0 0 2.77-.693a9 9 0 0 1 6.208.682l.108.054a9 9 0 0 0 6.086.71l3.114-.732a48.524 48.524 0 0 1-.005-10.499l-3.11.732a9 9 0 0 1-6.085-.711l-.108-.054a9 9 0 0 0-6.208-.682L3 4.5M3 15V4.5";
const P_ROCKET: &str = "M15.59 14.37a6 6 0 0 1-5.84 7.38v-4.8m5.84-2.58a14.98 14.98 0 0 0 6.16-12.12A14.98 14.98 0 0 0 9.631 8.41m5.96 5.96a14.926 14.926 0 0 1-5.841 2.58m-.119-8.54a6 6 0 0 0-7.381 5.84h4.8m2.581-5.84a14.927 14.927 0 0 0-2.58 5.84m2.699 2.7c-.103.021-.207.041-.311.06a15.09 15.09 0 0 1-2.448-2.448 14.9 14.9 0 0 1 .06-.312m-2.24 2.39a4.493 4.493 0 0 0-1.757 4.306 4.493 4.493 0 0 0 4.306-1.758M16.5 9a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Z";
const P_GRADE: &str = "M11.35 3.836c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m8.9-4.414c.376.023.75.05 1.124.08 1.131.094 1.976 1.057 1.976 2.192V16.5A2.25 2.25 0 0 1 18 18.75h-2.25m-7.5-10.5H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25ZM6.75 15.75l1.5 1.5 3-3.75";
const P_CHAT: &str = "M20.25 8.511c.884.284 1.5 1.128 1.5 2.097v4.286c0 1.136-.847 2.1-1.98 2.193-.34.027-.68.052-1.02.072v3.091l-3-3c-1.354 0-2.694-.055-4.02-.163a2.115 2.115 0 0 1-.825-.242m9.345-8.334a2.126 2.126 0 0 0-.476-.095 48.64 48.64 0 0 0-8.048 0c-1.131.094-1.976 1.057-1.976 2.192v4.286c0 .837.46 1.58 1.155 1.951m9.345-8.334V6.637c0-1.621-1.152-3.026-2.76-3.235A48.455 48.455 0 0 0 11.25 3c-2.115 0-4.198.137-6.24.402-1.608.209-2.76 1.614-2.76 3.235v6.226c0 1.621 1.152 3.026 2.76 3.235.577.075 1.157.14 1.74.194V21l4.155-4.155";

fn nav_link(href: &str, page: &str, key: &str, icon: &str, label: &str) -> String {
    nav_link_group(href, page, &[key], icon, label)
}

/// Like `nav_link`, but highlights for any of several pages — used by sidebar sections
/// (Online, Advanced Track) that fan out into sub-pages with no sidebar entry of their own.
fn nav_link_group(href: &str, page: &str, keys: &[&str], icon: &str, label: &str) -> String {
    let active = if keys.contains(&page) { "active" } else { "" };
    format!(r#"<a href="{href}" class="{active}">{icon}<span>{label}</span></a>"#)
}

fn layout(title: &str, user: Option<&User>, active: &str, content: &str) -> String {
    let shell = match user {
        Some(u) => {
            let admin_block = if u.is_admin {
                format!(
                    r#"<div class="sb-head">Yönetim</div>{}{}{}{}{}"#,
                    nav_link("/admin", active, "admin", &ico(P_ADMIN), "Yönetici paneli"),
                    nav_link(
                        "/admin/beginner-track",
                        active,
                        "beginner-admin",
                        &ico(P_FLAG),
                        "Beginner Track Gönderimleri"
                    ),
                    nav_link(
                        "/admin/puanlama",
                        active,
                        "puanlama",
                        &ico(P_GRADE),
                        "Görev Puanlama"
                    ),
                    nav_link(
                        "/admin/takimlar",
                        active,
                        "teams",
                        &ico(P_TEAMS),
                        "Takım formasyonu"
                    ),
                    nav_link(
                        "/admin/harness",
                        active,
                        "harness-admin",
                        &ico(P_HARNESS),
                        "Agentic Harness (Admin)"
                    )
                )
            } else {
                String::new()
            };
            format!(
                r##"<input type="checkbox" id="navtoggle" class="navtoggle" hidden>
<header class="mobilebar">
  <label for="navtoggle" class="hamburger" aria-label="Menü">{menu_ico}{close_ico}</label>
  <a class="mb-brand" href="/app"><img class="mb-logo" src="/static/exposure-logo.svg" alt="Exposure"></a>
</header>
<label for="navtoggle" class="nav-scrim" aria-hidden="true"></label>
<aside class="sidebar">
  <div class="sb-brand">
    <a href="/app"><img class="sb-logo" src="/static/exposure-logo.svg" alt="Exposure"></a>
    <span class="portal-pill">AI Academy</span>
  </div>
  <nav class="sb-nav">
    {home}
    {beginner_track}
    {advanced_track}
    {schedule}
    {location}
    {documents}
    {online}
    {admin_block}
  </nav>
  <div class="sb-footer">
    <div class="sb-user">
      <a class="sb-me {profile_active}" href="/profile" title="Profilim">
        <span class="avatar-fb">{initial}</span>
        <span class="sb-name">{name}</span>
      </a>
      <form method="post" action="/logout"><button class="sb-logout" title="Oturumu kapat">{logout_ico}</button></form>
    </div>
  </div>
</aside>
<main class="portal-main"><div class="portal-inner">
{content}
</div></main>"##,
                home = nav_link("/app", active, "home", &ico(P_HOME), "Ana Sayfa"),
                schedule = nav_link(
                    "/schedule",
                    active,
                    "schedule",
                    &ico(P_CAL),
                    "Haftalık Program"
                ),
                location = nav_link("/location", active, "location", &ico(P_PIN), "Konum"),
                documents = nav_link(
                    "/documents",
                    active,
                    "documents",
                    &ico(P_DOC),
                    "Veli Onay Formları"
                ),
                online = nav_link_group(
                    "/online",
                    active,
                    &["online", "videos", "board", "leaderboard", "demos"],
                    &ico(P_GLOBE),
                    "Online"
                ),
                beginner_track = nav_link_group(
                    "/beginner-track",
                    active,
                    &["beginner-track", "chatbot-challenge"],
                    &ico(P_FLAG),
                    "Beginner Track"
                ),
                advanced_track = nav_link_group(
                    "/advanced-track",
                    active,
                    &["advanced-track", "agentic-harness", "ai-monopoly"],
                    &ico(P_ROCKET),
                    "Advanced Track"
                ),
                admin_block = admin_block,
                profile_active = if active == "profile" { "active" } else { "" },
                initial = esc(&u
                    .label()
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string()),
                name = esc(u.label()),
                logout_ico = ico(P_LOGOUT),
                // both icons ship every time; CSS shows one or the other off #navtoggle
                menu_ico = ico(P_MENU).replace(r#"class="ico""#, r#"class="ico i-menu""#),
                close_ico = ico(P_CLOSE).replace(r#"class="ico""#, r#"class="ico i-close""#),
            )
        }
        None => format!(
            r##"<header class="topbar">
  <a class="logo" href="/"><img class="topbar-logo" src="/static/exposure-logo-black.svg" alt="Exposure"><span class="logo-tag">AI Academy</span></a>
  <a class="btn-dark" href="/login">Oturum aç</a>
</header>
<main class="public-main">
{content}
</main>"##
        ),
    };
    let body_class = if user.is_some() { "portal" } else { "" };
    format!(
        r##"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Exposure Academy</title>
<link rel="icon" href="/static/favicon.svg" type="image/svg+xml">
<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Geist:wght@100..900&display=swap" rel="stylesheet">
<link rel="stylesheet" href="/static/style.css?v=38">
<script>if('scrollRestoration'in history)history.scrollRestoration='manual';</script>
</head>
<body class="{body_class}">
{shell}
</body>
</html>"##,
        title = esc(title),
    )
}

pub fn landing() -> String {
    layout(
        "Akademi",
        None,
        "",
        r##"
<section class="hero">
  <div class="pill"><span class="dot"></span> Teorik Dersler</div>
  <h1>Yapay Zekayı<br><em>Projelerle Öğren!</em></h1>
  <p class="sub">Türkiye'nin En Seçkin Yapay Zeka Akademisi</p>
  <a class="btn-dark big" href="/login">Oturum aç →</a>
</section>"##,
    )
}

pub fn login(msg: Option<&str>) -> String {
    let notice = msg
        .map(|m| format!(r#"<p class="notice">{}</p>"#, esc(m)))
        .unwrap_or_default();
    layout(
        "Oturum aç",
        None,
        "",
        &format!(
            r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>Oturum aç</h1>
    <p class="auth-sub">Sana bir giriş bağlantısı gönderelim.</p>
    {notice}
    <form method="post" action="/login">
      <label>E-posta<input name="email" type="email" required autofocus></label>
      <button class="btn-dark big">Giriş bağlantısı gönder →</button>
    </form>
    <p class="muted">Hesabın yok mu? <a href="/join">Davet koduyla katıl</a></p>
  </div>
</div>"##
        ),
    )
}

/// Onboarding. `code_locked` = the invite code arrived in the URL (/join/<code>), so it
/// rides along as a hidden field instead of being something the student has to type.
pub fn join(f: &JoinForm, code_locked: bool, error: Option<&str>) -> String {
    let err = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    let code_field = if code_locked {
        format!(
            r#"<input type="hidden" name="code" value="{}">"#,
            esc(&f.code)
        )
    } else {
        format!(
            r#"<label>Davet kodu<input name="code" value="{}" required></label>"#,
            esc(&f.code)
        )
    };
    let grade_opts: String = std::iter::once(String::from(r#"<option value="">Seç…</option>"#))
        .chain(GRADES.iter().map(|g| {
            let sel = if f.grade == *g { " selected" } else { "" };
            format!(r#"<option value="{g}"{sel}>{g}</option>"#)
        }))
        .collect();
    layout(
        "Oluştur",
        None,
        "",
        &format!(
            r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>Profilini oluştur</h1>
    {err}
    <form method="post" action="/join">
      {code_field}
      <label>Ad soyad<input name="display_name" value="{name}" required autofocus></label>
      <label>E-posta<input name="email" type="email" value="{email}" required>
        <span class="fieldnote">Giriş bağlantıların bu adrese gelecek — doğru yazdığından emin ol.</span>
      </label>
      <label><span lang="en">Nickname</span><input name="nickname" value="{nick}" placeholder="ör. onur_maker" maxlength="20" required>
        <span class="fieldnote">Puan tablosunda <b>ad soyadın ve nickname'in birlikte</b> görünür
        (ör. Onur Çelik (onur_maker)). Harf, rakam, _ ve - kullanabilirsin.</span>
      </label>
      <label>Okul<input name="school" value="{school}" required></label>
      <label>Sınıf<select name="grade" required>{grade_opts}</select></label>

      <div class="profiles-block">
        <p class="profiles-head">Profillerin</p>
        <p class="fieldnote">GitHub ve LinkedIn, yaptığın işi dünyaya gösterdiğin yerdir.
        Hesabın varsa aşağıya linkleri gir, yoksa hemen aç!: <a href="https://github.com/signup" target="_blank" rel="noopener">GitHub</a> ·
        <a href="https://www.linkedin.com/signup" target="_blank" rel="noopener">LinkedIn</a>.</p>
        <label>GitHub<input name="github_url" type="text" inputmode="url" value="{github}" placeholder="https://github.com/kullanici"></label>
        <label>LinkedIn<input name="linkedin_url" type="text" inputmode="url" value="{linkedin}" placeholder="https://linkedin.com/in/adin"></label>
      </div>

      <button type="submit" class="btn-dark big">Oluştur →</button>
    </form>
  </div>

  <div class="modal-overlay" id="skipModal" hidden>
    <div class="modal-card" role="dialog" aria-modal="true" aria-labelledby="skipTitle">
      <h2 id="skipTitle">Profillerini eklemedin</h2>
      <p>GitHub ve LinkedIn profillerini şimdilik atlayabilirsin. Ama uygulamanın içinde
      bu profilleri oluşturman gerekecek — <b>Görev Panosu</b> ancak ikisini de eklediğinde açılır.</p>
      <div class="modal-actions">
        <button type="button" class="btn-ghost" id="skipBack">Geri dön ve ekle</button>
        <button type="button" class="btn-dark" id="skipGo">Şimdilik atla →</button>
      </div>
    </div>
  </div>

  <script>
  (function() {{
    var form = document.querySelector('form[action="/join"]');
    var modal = document.getElementById('skipModal');
    var gh = form.querySelector('[name="github_url"]');
    var li = form.querySelector('[name="linkedin_url"]');
    var warned = false;
    form.addEventListener('submit', function(e) {{
      var incomplete = !gh.value.trim() || !li.value.trim();
      if (incomplete && !warned) {{
        e.preventDefault();
        modal.hidden = false;
      }}
    }});
    document.getElementById('skipGo').addEventListener('click', function() {{
      warned = true;
      modal.hidden = true;
      form.submit();
    }});
    document.getElementById('skipBack').addEventListener('click', function() {{
      modal.hidden = true;
      gh.focus();
    }});
  }})();
  </script>
</div>"##,
            name = esc(&f.display_name),
            email = esc(&f.email),
            nick = esc(&f.nickname),
            school = esc(&f.school),
            github = esc(&f.github_url),
            linkedin = esc(&f.linkedin_url),
        ),
    )
}

/// Post-onboarding: the account exists but nothing is signed in yet — the magic link
/// in their inbox is what proves the address is theirs.
pub fn join_sent(email: &str) -> String {
    layout(
        "E-postanı kontrol et",
        None,
        "",
        &format!(
            r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>E-postanı kontrol et</h1>
    <p class="auth-sub"><b>{email}</b> adresine bir giriş bağlantısı gönderdik.
    Bağlantıya tıkladığında hesabın açılacak.</p>
    <p class="notice">Bağlantı 15 dakika geçerli. Gelen kutunda yoksa spam klasörüne bak.</p>
    <p class="muted">Yanlış adres mi yazdın? <a href="/join">Formu tekrar doldur</a></p>
  </div>
</div>"##,
            email = esc(email)
        ),
    )
}

pub fn profile(user: &User, p: &Profile, msg: Option<&str>, error: Option<&str>) -> String {
    let first_time = user.nickname.is_none();
    let banner = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .or_else(|| msg.map(|m| format!(r#"<p class="notice">{}</p>"#, esc(m))))
        .unwrap_or_default();
    let intro = if first_time {
        r#"<p class="muted">Devam etmeden önce profilini tamamla. Nickname'ini seçtiğinde derslere geçebilirsin.</p>"#
    } else {
        r#"<p class="muted">Bilgilerini dilediğin zaman güncelleyebilirsin.</p>"#
    };
    let grade_now = p.grade.as_deref().unwrap_or("");
    let grade_opts: String = std::iter::once(String::from(r#"<option value="">Seç…</option>"#))
        .chain(GRADES.iter().map(|g| {
            let sel = if grade_now == *g { " selected" } else { "" };
            format!(r#"<option value="{g}"{sel}>{g}</option>"#)
        }))
        .collect();
    let content = format!(
        r##"<h1 class="pagetitle">Profilim</h1>
{intro}
<div class="profilewrap">
<section class="panel">
  <h2>Bilgilerim</h2>
  {banner}
  <form method="post" action="/profile">
    <label>Ad soyad<input name="display_name" value="{name}" required></label>
    <label><span lang="en">Nickname</span><input name="nickname" value="{nick}" placeholder="ör. onur_maker" maxlength="20" required></label>
    <p class="fieldnote">Puan tablosunda ad soyadın ve nickname'in birlikte görünür; görev panosunda takım arkadaşlarına nickname'in gösterilir.</p>
    <label>Okul<input name="school" value="{school}" required></label>
    <label>Sınıf<select name="grade" required>{grade_opts}</select></label>
    <label>E-posta<input value="{email}" disabled></label>
    <p class="fieldnote">E-postan giriş kimliğin — değiştirmek için eğitmenine yaz.</p>
    <button class="btn-dark">{save_label}</button>
  </form>
</section>
</div>"##,
        name = esc(&p.display_name),
        nick = esc(p.nickname.as_deref().unwrap_or("")),
        school = esc(p.school.as_deref().unwrap_or("")),
        email = esc(&p.email),
        save_label = if first_time {
            "Kaydet ve başla →"
        } else {
            "Kaydet"
        },
    );
    layout("Profilim", Some(user), "profile", &content)
}

// ---- Agentic Harness ----

/// (query key, label) for the bench switcher chips. Benchmark names stay English.
const HARNESS_BENCHES: [(&str, &str); 3] = [
    ("arc", "ARC-AGI-3"),
    ("frontier", "Terminal Sprint"),
    ("ram", "RAM-bench"),
];

/// (tab key, href, label). "Instructions" stays English per the spec.
const HARNESS_TABS: [(&str, &str, &str); 4] = [
    ("main", "/agentic-harness", "Gönderim ve Sıralama"),
    ("live", "/agentic-harness?tab=live", "Canlı"),
    ("history", "/agentic-harness?tab=history", "Geçmiş"),
    (
        "instructions",
        "/agentic-harness?tab=instructions",
        "Instructions",
    ),
];

/// Turkish label + status-pill class per stage — the board's pill classes, reused.
fn harness_stage_tr(stage: &str) -> (&'static str, &'static str) {
    match stage {
        "queued" => ("Sırada", "st-pending"),
        "preparing" => ("Repo hazırlanıyor", "st-reviewing"),
        "running" => ("Benchmark'lar çalışıyor", "st-reviewing"),
        "done" => ("Tamamlandı", "st-passed"),
        "partial" => ("Kısmen tamamlandı", "st-pending"),
        "infra_failed" => ("Altyapı hatası", "st-failed"),
        "cancelled" => ("Durduruldu", "st-failed"),
        _ => ("Başarısız", "st-failed"),
    }
}

/// Turkish label per `source_error_slug` (harness.rs), for the rejected-submission log. The
/// catch-all arm matters: rows outlive the variant that wrote them, and a retired slug should
/// render as "bilinmeyen" rather than take the admin page down.
fn harness_reject_reason_tr(reason: &str) -> &'static str {
    match reason {
        "empty" => "Boş bırakılmış",
        "too_long" => "Bağlantı çok uzun",
        "not_a_url" => "Bağlantı değil",
        "not_github" => "github.com değil",
        "gist_link" => "Gist bağlantısı",
        "raw_file_link" => "Raw dosya bağlantısı",
        "credentials" => "Kullanıcı adı/şifre/port var",
        "no_repo" => "Repo adı yok",
        "owner_only" => "Profil bağlantısı, repo değil",
        "reserved_owner" => "GitHub'ın kendi sayfası",
        "non_ascii" => "Türkçe karakter var",
        "bad_chars" => "Geçersiz karakter",
        "segment_too_long" => "İsim çok uzun",
        "both_sources" => "Hem repo hem hazır harness",
        "no_source" => "Ne repo ne hazır harness",
        "builtin_forbidden" => "Hazır harness izni yok",
        "builtin_unknown" => "Bilinmeyen hazır harness",
        _ => "bilinmeyen",
    }
}

fn harness_stop_form(run_id: uuid::Uuid) -> String {
    format!(
        r##"<form method="post" action="/agentic-harness/stop" class="inline"
      onsubmit="return confirm('Bu çalıştırma durdurulsun mu?')">
  <input type="hidden" name="id" value="{run_id}">
  <button class="btn-outline small" type="submit">Durdur</button>
</form>"##
    )
}

fn harness_benchmark_status(status: &str) -> (&'static str, &'static str) {
    match status {
        "running" => ("Çalışıyor", "st-reviewing"),
        "done" => ("Tamamlandı", "st-passed"),
        "failed" => ("Başarısız", "st-failed"),
        "infra_failed" => ("Altyapı hatası", "st-failed"),
        _ => ("Bekliyor", "st-pending"),
    }
}

fn harness_kaggle_status(status: &str) -> (&'static str, &'static str) {
    match status {
        "queued" => ("Resmi gönderim sırada", "st-pending"),
        "kernel_running" => ("Notebook gönderiliyor", "st-reviewing"),
        "submitted" => ("Kaggle puanlıyor", "st-reviewing"),
        "scored" => ("Resmi skor hazır", "st-passed"),
        _ => ("Resmi gönderim başarısız", "st-failed"),
    }
}

/// Same shape for the tournament's status ladder (model.rs MONOPOLY_STATUSES).
fn monopoly_status_tr(status: &str) -> (&'static str, &'static str) {
    match status {
        "queued" => ("Sırada", "st-pending"),
        "booting" => ("Sunucu açılıyor", "st-reviewing"),
        "loading" => ("Modeller yükleniyor", "st-reviewing"),
        "running" => ("Oynanıyor", "st-reviewing"),
        "judging" => ("Hakem değerlendiriyor", "st-reviewing"),
        "done" => ("Tamamlandı", "st-passed"),
        _ => ("Başarısız", "st-failed"),
    }
}

/// dense_ranks over any row type: the key is the score formatted at display
/// precision, so float ties rank exactly as students see them on the board.
fn dense_ranks_by<T>(rows: &[T], key: impl Fn(&T) -> String) -> Vec<i64> {
    let mut ranks = Vec::with_capacity(rows.len());
    let mut place = 0i64;
    let mut prev: Option<String> = None;
    for r in rows {
        let k = key(r);
        if prev.as_deref() != Some(k.as_str()) {
            place += 1;
            prev = Some(k);
        }
        ranks.push(place);
    }
    ranks
}

/// Page title + tab chips shared by the three harness tabs.
fn harness_shell(user: &User, tab: &str, sub: &str, inner: &str) -> String {
    let chips: String = HARNESS_TABS
        .iter()
        .map(|(k, href, label)| {
            let active = if tab == *k { "active" } else { "" };
            format!(r#"<a class="chip {active}" href="{href}">{label}</a>"#)
        })
        .collect();
    layout(
        "Agentic Harness",
        Some(user),
        "agentic-harness",
        // .arcade is the skin's only hook. It goes on the content div, never on <body>:
        // the sidebar emits .avatar-fb (html.rs:134), which harness.css also restyles.
        // The <link> rides in the content instead of layout()'s <head> so layout() keeps
        // byte-identical output for every other page — <link rel=stylesheet> is body-ok.
        &format!(
            r##"<div class="arcade">
<link rel="stylesheet" href="/static/harness.css?v=6">
<h1 class="pagetitle" lang="en">Agentic Harness</h1>
<p class="muted">{sub}</p>
<div class="chips">{chips}</div>
{inner}
</div>"##
        ),
    )
}

/// Does the top of this board get a podium? Splits on rank, never on index:
/// `dense_ranks_by` keys on the display-rounded score, so ties are routine and
/// `rows[..3]` would put one tied team on the podium and its equal in the list
/// with the same number beside it. Below three teams a "podium" reads as a bug,
/// and above six podium rows (pathological ties) an all-podium board is worse
/// than none — both fall back to the plain 1..n list.
fn harness_has_podium(ranks: &[i64]) -> bool {
    ranks.len() >= 3 && ranks.iter().filter(|r| **r <= 3).count() <= 6
}

/// One podium card. `mine` is `is-mine`, not the shared `mine`, because a student
/// in the top three has no list row left and has to be findable here.
fn harness_pod_card(
    rank: i64,
    mine: bool,
    name: &str,
    kids: &str,
    score: &str,
    unit: &str,
) -> String {
    let you = if mine {
        r#"<div class="pod-you">senin takımın</div>"#
    } else {
        ""
    };
    format!(
        r##"<div class="pod p{place}{is_mine}">
  <span class="pod-rank">{rank}</span>
  <div class="who">{name}</div>
  <div class="kids">{kids}</div>
  <div class="pts">{score}<small>{unit}</small></div>{you}
</div>"##,
        place = rank.min(3),
        is_mine = if mine { " is-mine" } else { "" },
    )
}

/// Podium + the list under it. A full podium with nothing left over gets a line
/// rather than an empty container.
fn harness_board(pods: &str, list: &str) -> String {
    let podium = if pods.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="podium">{pods}</div>"#)
    };
    if list.is_empty() {
        format!(r#"{podium}<p class="lbnote">Şimdilik sıralamada bu kadar takım var.</p>"#)
    } else {
        format!(r#"{podium}<div class="lb">{list}</div>"#)
    }
}

fn harness_benchmark_card(run: &HarnessRun, key: &str, title: &str, rule: &str) -> String {
    let state = run.benchmark_state.get(key).and_then(|v| v.as_object());
    let status = state
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let (label, class) = harness_benchmark_status(status);
    let mut summary = Vec::new();
    if let (Some(done), Some(total)) = (
        state.and_then(|v| v.get("done")).and_then(|v| v.as_u64()),
        state.and_then(|v| v.get("total")).and_then(|v| v.as_u64()),
    ) {
        summary.push(format!("{done}/{total}"));
    }
    if let Some(score) = state.and_then(|v| v.get("score")).and_then(|v| v.as_f64()) {
        summary.push(format!("Skor {score:.1}"));
    }
    if key == "ram" {
        if let Some(value) = state
            .and_then(|v| v.get("one_session_mb"))
            .and_then(|v| v.as_f64())
        {
            summary.push(format!("1 oturum {value:.1} MB"));
        }
        if let Some(value) = state
            .and_then(|v| v.get("ten_session_mb"))
            .and_then(|v| v.as_f64())
        {
            summary.push(format!("10 oturum {value:.1} MB"));
        }
    }
    if let Some(rate) = state.and_then(|v| v.get("rate")).and_then(|v| v.as_u64()) {
        summary.push(format!("{rate} tur / 30 sn"));
    }
    let summary = if summary.is_empty() {
        "Henüz sonuç yok".into()
    } else {
        summary.join(" · ")
    };
    format!(
        r##"<article class="harness-benchmark" data-benchmark="{key}" data-status="{status}">
  <div class="harness-benchmark-head"><h3 lang="en">{title}</h3>
    <span class="substatus benchmark-status {class}">{label}</span></div>
  <p class="benchmark-rule">{rule}</p>
  <p class="benchmark-progress">{summary}</p>
</article>"##,
        status = esc(status),
        summary = esc(&summary)
    )
}

/// The in-flight run's three independent benchmark states. harness.js polls the
/// team-scoped status endpoint and reloads once the run reaches any terminal state.
///
/// `submitter` is who pressed the button and `busy` means this student just tried to
/// submit while that run was live. One team can only have one run per benchmark going,
/// so the second person needs to be told who beat them to it and given a way to watch —
/// this used to be a plain-text 400 with no way back.
fn harness_stepper(run: &HarnessRun, submitter: Option<&str>, busy: bool) -> String {
    let (stage_label, stage_class) = harness_stage_tr(&run.stage);
    let sha = run
        .commit_sha
        .as_deref()
        .map(|s| esc(&s.chars().take(7).collect::<String>()))
        .unwrap_or_else(|| "—".into());
    let deadline = run
        .deadline_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_default();
    let profile = run
        .bedrock_profile
        .as_deref()
        .map(esc)
        .unwrap_or_else(|| esc(&run.model_id));
    let arc = harness_benchmark_card(run, "arc", "ARC-AGI-3", "25 public oyun · aynı anda 5 oyun");
    let terminal = harness_benchmark_card(
        run,
        "frontier",
        "Terminal Sprint",
        "5 görev · görev başına 120 sn",
    );
    let ram = harness_benchmark_card(
        run,
        "ram",
        "RAM-bench",
        "1 ve 10 oturum · 10 sn · cgroup PSS",
    );
    let cards = match run.benchmark_kind.as_str() {
        "arc" => format!("{arc}{ram}"),
        "frontier" => format!("{terminal}{ram}"),
        _ => format!("{arc}{terminal}{ram}"),
    };
    // Shown whenever a run is live, not only after a blocked submit: a teammate who just
    // opens the page wants the same button. The live tab resolves the team's latest run
    // per request, so no run id has to be threaded through the link.
    let watch = r#"<a class="btn-outline small harness-watch" href="/agentic-harness?tab=live">Canlı izle →</a>"#;
    let warning = if busy {
        let who = match submitter {
            Some(name) => format!("{} zaten bir çalıştırma başlattı.", esc(name)),
            None => "Takımından biri zaten bir çalıştırma başlattı.".into(),
        };
        format!(
            r##"<div class="harness-busy">
    <p><span aria-hidden="true">⚠</span> {who} Aynı anda tek koşu yapılabilir.</p>
    <a class="btn-dark small" href="/agentic-harness?tab=live">Canlı izle →</a>
  </div>"##
        )
    } else {
        String::new()
    };
    format!(
        r##"<div class="harness-live" id="harness-live" data-active="true" data-run="{run_id}" data-kind="{kind}" data-deadline="{deadline}">
  {warning}
  <div class="harness-live-head">
    <span id="harness-run-status" class="substatus {stage_class}">{stage_label}</span>
    <div class="harness-live-actions">
      <span class="harness-countdown" id="harness-countdown"></span>
      {watch}
      {stop}
    </div>
  </div>
  <p class="fieldnote harness-repo">{repo} · <code id="harness-commit">{sha}</code></p>
  <p class="harness-run-meta"><span lang="en">{version}</span> · {provider}: <span id="harness-profile">{profile}</span>{by}</p>
  <div class="harness-benchmark-grid">{cards}</div>
</div>"##,
        deadline = esc(&deadline),
        run_id = run.id,
        kind = esc(&run.benchmark_kind),
        provider = esc(&run.provider),
        repo = harness_source_label(&run.repo_url),
        version = esc(&run.benchmark_version),
        stop = harness_stop_form(run.id),
        by = submitter
            .map(|name| format!(" · gönderen: {}", esc(name)))
            .unwrap_or_default(),
    )
}

/// Main tab: submit panel on the left, the switchable leaderboards on the right.
/// `rows` carries ARC/Terminal standings, `ram_rows` the RAM ones — whichever
/// matches `bench` is populated, the other is empty.
fn provider_model_options(provider: ModelProvider, select_default: bool) -> String {
    let (models, default) = match provider {
        ModelProvider::Cerebras => (CEREBRAS_MODEL_IDS, DEFAULT_CEREBRAS_MODEL),
        ModelProvider::Bedrock => (BEDROCK_MODEL_IDS, DEFAULT_BEDROCK_MODEL),
        ModelProvider::DeepInfra => (DEEPINFRA_MODEL_IDS, DEFAULT_DEEPINFRA_MODEL),
    };
    models
        .iter()
        .map(|model| {
            let selected = if select_default && *model == default {
                " selected"
            } else {
                ""
            };
            let image = if provider.supports_images(model) {
                r#" data-image="true""#
            } else {
                ""
            };
            // Inactive providers start disabled so a submit before touching the
            // provider select can't send a model the server will reject; the
            // picker's onchange re-enables the group it switches to.
            let disabled = if select_default { "" } else { " disabled" };
            format!(
                r#"<option value="{model}" data-provider="{provider}"{selected}{disabled}{image}>{model}{note}</option>"#,
                provider = provider.as_str(),
                model = esc(model),
                note = model_note(model)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Warnings that belong on the option itself — no JS, visible while picking.
fn model_note(model_id: &str) -> &'static str {
    match model_id {
        "zai-org/GLM-5.2" => " · Kaggle RTX 6000 üzerinde çalışmayabilir",
        _ => "",
    }
}

/// Bedrock is the admin-only pool; students choose between the two hosted ones.
/// Order matters: the first entry is the pre-selected provider.
const STUDENT_PROVIDERS: &[ModelProvider] = &[ModelProvider::Cerebras, ModelProvider::DeepInfra];
const ADMIN_PROVIDERS: &[ModelProvider] = &[
    ModelProvider::Cerebras,
    ModelProvider::Bedrock,
    ModelProvider::DeepInfra,
];

/// Provider picker plus the inline filter that greys out models from the other
/// providers. First provider in the list is the selected one.
fn provider_picker(providers: &[ModelProvider]) -> String {
    let options: String = providers
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            let label = match provider {
                ModelProvider::Bedrock => "Bedrock",
                ModelProvider::Cerebras => "Cerebras",
                ModelProvider::DeepInfra => "DeepInfra",
            };
            let selected = if index == 0 { " selected" } else { "" };
            format!(
                r#"<option value="{}"{selected}>{label}</option>"#,
                provider.as_str()
            )
        })
        .collect();
    format!(
        r#"<label>provider:
      <select name="provider" onchange="const m=this.form.elements.model_id;for(const o of m.options)o.disabled=o.dataset.provider!==this.value;const first=[...m.options].find(o=>!o.disabled);if(m.selectedOptions[0]?.disabled&amp;&amp;first)m.value=first.value">
        {options}
      </select>
    </label>"#
    )
}

/// Options for every provider in the list, defaulting to the first provider's model.
fn provider_models(providers: &[ModelProvider]) -> String {
    providers
        .iter()
        .enumerate()
        .map(|(index, provider)| provider_model_options(*provider, index == 0))
        .collect()
}

fn builtin_harness_options(bench: &str) -> String {
    let options = BUILTIN_HARNESSES
        .iter()
        .filter(|(id, _, _)| bench == "frontier" || *id != "terminus-2")
        .map(|(id, _, label)| format!(r#"<option value="{}">{}</option>"#, esc(id), esc(label)))
        .collect::<Vec<_>>()
        .join("");
    format!(r#"<option value="">GitHub URL</option>{options}"#)
}

fn harness_source_label(source: &str) -> String {
    esc(builtin_harness_label(source).unwrap_or(source))
}

pub fn agentic_harness_main(
    user: &User,
    bench: &str,
    team: Option<&HarnessTeam>,
    members: &[TeamMemberRow],
    active_run: Option<&HarnessRun>,
    // who started active_run, and what just happened (?msg= — see HarnessQ)
    submitter: Option<&str>,
    msg: Option<&str>,
    rows: &[HarnessLeaderRow],
    ram_rows: &[HarnessRamRow],
) -> String {
    // kid names next to the team name, real names per the leaderboard convention;
    // only public members (onboarded, not hidden) reach the published line
    let kid_names = |team_id: uuid::Uuid| -> String {
        let names: Vec<String> = members
            .iter()
            .filter(|m| m.team_id == team_id && m.public)
            .map(|m| esc(&m.display_name))
            .collect();
        if names.is_empty() {
            String::new()
        } else {
            format!("({})", names.join(", "))
        }
    };
    let left = match team {
        None => format!(
            r##"<section class="harness-left">
  <div class="gate-lock">{lock}</div>
  <h2>Takımın henüz yok</h2>
  <p class="fieldnote">Takımın olmadığı için gönderim yapamazsın. Eğitmenine yaz — takımlar
  şimdilik eğitmen tarafından atanıyor.</p>
</section>"##,
            lock = ico(P_LOCK)
        ),
        Some(t) => {
            let member_chips: String = members
                .iter()
                .filter(|m| m.team_id == t.id)
                .map(|m| format!(r#"<span class="chip">{}</span>"#, esc(&m.display_name)))
                .collect();
            // the stepper replaces the form while a run is in flight — this is the
            // visible half of the double-submit guard (the DB index is the other)
            let action = match active_run {
                Some(run) => harness_stepper(run, submitter, msg == Some("busy")),
                None => format!(
                    r##"<form method="post" action="/agentic-harness/submit" class="subform">
    <input type="hidden" name="benchmark_kind" value="{bench}">
    <input name="repo_url" type="text" inputmode="url" spellcheck="false"
      placeholder="https://github.com/kullanici/repo" required>
    {provider_picker}
    <label>model:
      <select name="model_id" required>{model_options}</select>
    </label>
    <button class="btn-dark">Ajanı Gönder →</button>
  </form>
  <p class="fieldnote">Repo'nun ana sayfasının adresini yapıştır; klasör veya dosya bağlantısı da olur, biz kısaltırız.
  Herhangi bir takım üyesi gönderebilir. Her benchmark için aynı anda tek çalıştırma.
  Kurallar için <a href="/agentic-harness?tab=instructions" lang="en">Instructions</a> sekmesine bak.</p>"##,
                    provider_picker = provider_picker(STUDENT_PROVIDERS),
                    model_options = provider_models(STUDENT_PROVIDERS),
                ),
            };
            // The name IS the input — no edit button, no modal. A one-field form submits
            // on Enter with no JS at all; harness.js adds save-on-blur on top. Kids
            // rename as often as they like, so the only feedback needed is a clash.
            let note = match msg {
                Some("named") => r#"<p class="teamname-note ok">Takım adı güncellendi ✓</p>"#,
                Some("name-taken") => {
                    r#"<p class="teamname-note bad">Bu isim başka bir takımda kullanılıyor.</p>"#
                }
                Some("name-long") => {
                    r#"<p class="teamname-note bad">Takım adı boş olamaz, en fazla 40 karakter.</p>"#
                }
                _ => "",
            };
            format!(
                r##"<section class="harness-left">
  <form method="post" action="/agentic-harness/team/name" class="teamname">
    <input name="name" value="{name}" maxlength="{max}" required spellcheck="false"
           aria-label="Takım adı" title="Takım adını değiştirmek için buraya yaz">
  </form>
  {note}
  <div class="chips interest-names">{member_chips}</div>
  {action}
</section>"##,
                name = esc(&t.name),
                max = HARNESS_TEAM_NAME_MAX
            )
        }
    };
    let bench_chips: String = HARNESS_BENCHES
        .iter()
        .map(|(k, label)| {
            let active = if bench == *k { "active" } else { "" };
            let href = match *k {
                "arc" => "/agentic-harness/arc",
                "frontier" => "/agentic-harness/frontier",
                _ => "/agentic-harness?bench=ram",
            };
            format!(r#"<a class="chip {active}" href="{href}" lang="en">{label}</a>"#)
        })
        .collect();
    let my_team_id = team.map(|t| t.id);
    let empty_note =
        "<p class='muted'>Henüz tamamlanmış çalıştırma yok — ilk gönderen takım siz olun.</p>";
    let board_rows: String = if bench == "ram" {
        if ram_rows.is_empty() {
            empty_note.into()
        } else {
            let ranks = dense_ranks_by(ram_rows, |r| format!("{:.1}", r.ram_10session_mb));
            let podium = harness_has_podium(&ranks);
            let pods: String = ram_rows
                .iter()
                .zip(&ranks)
                .filter(|(_, rank)| podium && **rank <= 3)
                .map(|(r, rank)| {
                    harness_pod_card(
                        *rank,
                        my_team_id == Some(r.id),
                        &esc(&r.name),
                        &kid_names(r.id),
                        &format!("{:.1}", r.ram_10session_mb),
                        "MB",
                    )
                })
                .collect();
            let list: String = ram_rows
                .iter()
                .zip(&ranks)
                .filter(|(_, rank)| !podium || **rank > 3)
                .map(|(r, rank)| {
                    format!(
                        r##"<div class="lbrow {mine} {medal}">
  <span class="lbrank">{rank}</span>
  <span class="avatar-fb">{initial}</span>
  <span class="lbname">{name} <small class="nick">{kids}</small></span>
  <span class="lbmeta">1 oturum: {r1:.1} MB</span>
  <span class="lbpts">{r10:.1}<small>MB</small></span>
</div>"##,
                        mine = if my_team_id == Some(r.id) { "mine" } else { "" },
                        medal = match rank {
                            1 => "m1",
                            2 => "m2",
                            3 => "m3",
                            _ => "",
                        },
                        initial = esc(&r
                            .name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string()),
                        name = esc(&r.name),
                        kids = kid_names(r.id),
                        r1 = r.ram_1session_mb,
                        r10 = r.ram_10session_mb,
                    )
                })
                .collect();
            harness_board(&pods, &list)
        }
    } else if rows.is_empty() {
        empty_note.into()
    } else {
        let ranks = dense_ranks_by(rows, |r| format!("{:.1}", r.best));
        let podium = harness_has_podium(&ranks);
        let pods: String = rows
            .iter()
            .zip(&ranks)
            .filter(|(_, rank)| podium && **rank <= 3)
            .map(|(r, rank)| {
                harness_pod_card(
                    *rank,
                    my_team_id == Some(r.id),
                    &esc(&r.name),
                    &kid_names(r.id),
                    &format!("{:.1}", r.best),
                    "p",
                )
            })
            .collect();
        let list: String = rows
            .iter()
            .zip(&ranks)
            .filter(|(_, rank)| !podium || **rank > 3)
            .map(|(r, rank)| {
                format!(
                    r##"<div class="lbrow {mine} {medal}">
  <span class="lbrank">{rank}</span>
  <span class="avatar-fb">{initial}</span>
  <span class="lbname">{name} <small class="nick">{kids}</small></span>
  <span class="lbpts">{best:.1}<small>p</small></span>
</div>"##,
                    mine = if my_team_id == Some(r.id) { "mine" } else { "" },
                    medal = match rank {
                        1 => "m1",
                        2 => "m2",
                        3 => "m3",
                        _ => "",
                    },
                    initial = esc(&r
                        .name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()),
                    name = esc(&r.name),
                    kids = kid_names(r.id),
                    best = r.best,
                )
            })
            .collect();
        harness_board(&pods, &list)
    };
    // While a run is in flight the main tab gets one large board that actually plays.
    // #arc-live is the hidden root arc.js binds to — it carries #arc-focus so every
    // existing focus/playback path works untouched, and arc.js mirrors each frame into
    // the visible preview canvas below. No #arc-boards here: the 25-tile wall stays on
    // the Canlı tab. data-poll slows the heartbeat to 5s, because each poll returns all
    // 25 grids (~100KB) and the preview only needs the focused one.
    let preview = match active_run {
        Some(run) if matches!(run.benchmark_kind.as_str(), "arc" | "bundled") => format!(
            r##"<div id="arc-live" class="arc-live" data-active="true" data-run="" data-current-run="{run_id}" data-replay="false" data-poll="5000" hidden>
  <div class="arc-focus" id="arc-focus" hidden></div>
</div>
<a class="panel ah-preview" href="/agentic-harness?tab=live">
  <div class="ah-preview-head"><h2>Canlı</h2>
    <span class="substatus st-reviewing" data-arc-count="label">Tahtalar bekleniyor…</span></div>
  <div class="ah-prev">
    <div class="ah-prev-board">
      <canvas id="arc-preview" width="64" height="64" role="img" aria-label="Canlı oyun tahtası"></canvas>
      <span class="ah-prev-live">CANLI</span>
    </div>
    <div class="ah-prev-side">
      <h3 id="arc-preview-game" lang="en">—</h3>
      <p class="arc-focus-meta" id="arc-preview-meta">Ajanın ilk hamlesi bekleniyor…</p>
      <div class="ah-counts">
        <span><b data-arc-count="playing">0</b> oynuyor</span>
        <span><b data-arc-count="done">0</b> bitti</span>
        <span><b data-arc-count="total">25</b> toplam</span>
      </div>
      <span class="ah-cta">Canlı izle →</span>
    </div>
  </div>
</a>
<script src="/static/arc.js?v=7" defer></script>"##,
            run_id = run.id
        ),
        _ => String::new(),
    };
    let inner = format!(
        r##"<div class="harnesswrap">
{left}
<div class="harness-right">
  <div class="chips">{bench_chips}</div>
  {board_rows}
  <p class="lbnote">Her takımın bu harness sürümündeki en iyi puanı gösterilir. RAM-bench'te
  düşük olan daha iyidir. Kısmi çalıştırmalardaki tamamlanmış puanlar korunur.</p>
</div>
</div>
{preview}
<script src="/static/harness.js?v=6" defer></script>"##
    );
    harness_shell(
        user,
        "main",
        "25 ARC oyunu beşerli çalışır; koşuyu istediğiniz zaman durdurabilirsiniz.",
        &inner,
    )
}

/// The admin-only submit form: agent/provider/model pickers plus an optional repo URL,
/// moved off the student-facing page so `agentic_harness_main` renders the same form for
/// everyone. Still posts to the shared `/agentic-harness/submit`, which already checks the
/// real session's `user.is_admin` — this page is just the only place that shows the extra
/// fields now.
fn admin_harness_form(bench: &str) -> String {
    let builtin_picker = format!(
        r#"<label>agent:
      <select name="builtin_harness">{}</select>
    </label>"#,
        builtin_harness_options(bench)
    );
    let provider_picker = provider_picker(ADMIN_PROVIDERS);
    let model_options = provider_models(ADMIN_PROVIDERS);
    format!(
        r##"<form method="post" action="/agentic-harness/submit" class="subform">
    <input type="hidden" name="benchmark_kind" value="{bench}">
    <input name="repo_url" type="text" inputmode="url" spellcheck="false"
      placeholder="https://github.com/kullanici/repo">
    {builtin_picker}
    {provider_picker}
    <label>model:
      <select name="model_id" required>{model_options}</select>
    </label>
    <button class="btn-dark">Ajanı Gönder →</button>
  </form>
  <p class="fieldnote">Takımın adına gönderiyorsun. Her benchmark için aynı anda tek çalıştırma.</p>"##,
        model_options = model_options,
    )
}

/// Admin-only page living under "Yönetici paneli" in the sidebar: the team's roster plus
/// `admin_harness_form`. Leaderboard, Canlı/Geçmiş/Instructions stay on `/agentic-harness`
/// only — they already render the same for admins and students, so this page just links
/// back to them instead of duplicating them.
pub fn admin_harness_page(
    user: &User,
    bench: &str,
    team: Option<&HarnessTeam>,
    members: &[TeamMemberRow],
    active_run: Option<&HarnessRun>,
) -> String {
    let content = match team {
        None => format!(
            r##"<div class="arcade">
<link rel="stylesheet" href="/static/harness.css?v=6">
<h1 class="pagetitle" lang="en">Agentic Harness — Admin</h1>
<section class="harness-left">
  <div class="gate-lock">{lock}</div>
  <h2>Takımın yok</h2>
  <p class="fieldnote">Bir takıma atanmadan buradan gönderim yapılamaz.</p>
</section>
</div>"##,
            lock = ico(P_LOCK)
        ),
        Some(t) => {
            let member_chips: String = members
                .iter()
                .filter(|m| m.team_id == t.id)
                .map(|m| format!(r#"<span class="chip">{}</span>"#, esc(&m.display_name)))
                .collect();
            let action = match active_run {
                Some(run) => harness_stepper(run, None, false),
                None => admin_harness_form(bench),
            };
            let bench_chips: String = [("arc", "ARC-AGI-3"), ("frontier", "Terminal Sprint")]
                .iter()
                .map(|(k, label)| {
                    let active = if bench == *k { "active" } else { "" };
                    format!(
                        r#"<a class="chip {active}" href="/admin/harness?bench={k}" lang="en">{label}</a>"#
                    )
                })
                .collect();
            format!(
                r##"<div class="arcade">
<link rel="stylesheet" href="/static/harness.css?v=6">
<h1 class="pagetitle" lang="en">Agentic Harness — Admin</h1>
<p class="muted">Takım adına doğrudan çalıştırma gönder.</p>
<section class="harness-left">
  <h2>{name}</h2>
  <div class="chips interest-names">{member_chips}</div>
  <div class="chips">{bench_chips}</div>
  {action}
</section>
<p class="fieldnote"><a href="/agentic-harness">← Agentic Harness</a> sayfasında sıralamayı, canlı izlemeyi ve geçmişi görebilirsin.</p>
</div>"##,
                name = esc(&t.name)
            )
        }
    };
    layout(
        "Agentic Harness (Admin)",
        Some(user),
        "harness-admin",
        &content,
    )
}

/// Live tab: the 25 ARC-AGI-3 boards. Everything inside `#arc-live` is drawn by arc.js
/// from the poll payload — the server renders only the frame and the idle state, so there
/// is exactly one implementation of a board and it lives in the JS.
/// `replay` pages a finished run's frames instead of following the live one.
pub fn agentic_harness_live(user: &User, run: Option<&HarnessRun>, replay: bool) -> String {
    // data-run is set only for a replay. Live mode leaves it empty on purpose: the endpoint
    // resolves the team's latest run per request, so a student sitting on this tab sees the
    // boards appear the moment a run starts, with no reload.
    let run_attr = if replay {
        run.map(|r| r.id.to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    let current_run = run.map(|r| r.id.to_string()).unwrap_or_default();
    let meta = match run {
        Some(r) => {
            let (label, class) = harness_stage_tr(&r.stage);
            let sha = r
                .commit_sha
                .as_deref()
                .map(|s| esc(&s.chars().take(7).collect::<String>()))
                .unwrap_or_else(|| "—".into());
            let back = if replay {
                r#"<a class="chip" href="/agentic-harness?tab=live">Canlıya dön</a>"#
            } else {
                ""
            };
            let stop = if !replay
                && !matches!(
                    r.stage.as_str(),
                    "done" | "partial" | "failed" | "infra_failed" | "cancelled"
                ) {
                harness_stop_form(r.id)
            } else {
                String::new()
            };
            format!(
                r##"<div class="arc-run"><span class="substatus {class}">{label}</span>
  <span class="fieldnote">{repo} · <code>{sha}</code> · {model} · {date}</span>{back}{stop}</div>"##,
                repo = harness_source_label(&r.repo_url),
                model = esc(&r.model_id),
                date = r.created_at.format("%d.%m.%Y %H:%M"),
            )
        }
        None => String::new(),
    };
    // Idle is a real state, not an empty page: say what will show up here and when.
    let idle = match (run, replay) {
        // A ?run= that resolved to nothing is deleted or another team's. Say so and stay
        // inactive: falling through to live would play a *different* run under the
        // requested run's chrome, and the student would never know.
        (None, true) => {
            r##"<h2>Çalıştırma bulunamadı</h2>
  <p class="muted">Bu tekrar silinmiş olabilir veya takımınıza ait değil.</p>
  <a class="btn-outline" href="/agentic-harness?tab=live">Canlıya dön</a>"##
        }
        (None, false) => {
            r##"<h2>Henüz çalıştırma yok</h2>
  <p class="muted">Takımın bir ajan gönderdiğinde 25 ARC-AGI-3 oyunu burada canlı akar.</p>
  <a class="btn-outline" href="/agentic-harness">Gönderim sekmesine git</a>"##
        }
        (Some(_), true) => {
            r##"<h2>Tekrar hazırlanıyor…</h2>
  <p class="muted">Bu çalıştırmanın kareleri getiriliyor.</p>"##
        }
        (Some(_), false) => {
            r##"<h2>Kareler bekleniyor…</h2>
  <p class="muted">Ajanın ilk hamlesi geldiği anda tahtalar burada belirir.</p>"##
        }
    };
    // The one case the viewer must not run: a replay whose run did not resolve.
    let active = !(replay && run.is_none());
    let inner = format!(
        r##"<div id="arc-live" class="arc-live" data-active="{active}" data-run="{run_attr}" data-current-run="{current_run}" data-replay="{replay}">
  {meta}
  <div class="arc-idle" id="arc-idle">{idle}</div>
  <div class="arc-focus" id="arc-focus" hidden></div>
  <div class="arc-grid" id="arc-boards"></div>
</div>
<link href="https://fonts.googleapis.com/css2?family=Silkscreen:wght@400;700&display=swap" rel="stylesheet">
<script src="/static/arc.js?v=7" defer></script>"##
    );
    harness_shell(
        user,
        "live",
        if replay {
            "Biten bir çalıştırmanın tekrarı — bir tahtaya tıkla, kareleri ileri geri sar."
        } else {
            "25 ARC-AGI-3 oyunu canlı — beşi oynarken sıradakiler otomatik başlar."
        },
        &inner,
    )
}

/// History tab: every run of the viewer's team — which commit went in, what it scored.
pub fn agentic_harness_history(
    user: &User,
    team: Option<&HarnessTeam>,
    runs: &[HarnessRun],
    official_enabled: bool,
    credential_username: Option<&str>,
    official: &[HarnessKaggleSubmission],
) -> String {
    let fmt = |v: Option<f32>| v.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into());
    let credential_panel = if team.is_none() {
        String::new()
    } else if !official_enabled {
        r##"<section class="panel harness-official"><h2>Official Kaggle</h2>
  <p class="muted">Resmi gönderim bu sunucuda yapılandırılmamış. Yerel skorlar etkilenmez.</p>
</section>"##
            .to_string()
    } else {
        let current = credential_username.map(|username| format!(
            r##"<p class="fieldnote">Kayıtlı hesap: <b>{}</b>. Token hiçbir zaman tekrar gösterilmez.</p>
<form method="post" action="/agentic-harness/kaggle/credentials/delete" class="inline"
      onsubmit="return confirm('Kayıtlı Kaggle tokenı silinsin mi?')">
  <button class="btn-outline" type="submit">Tokenı sil</button>
</form>"##, esc(username))).unwrap_or_else(||
            "<p class='fieldnote'>Henüz Kaggle hesabı kaydedilmedi.</p>".into());
        format!(
            r##"<section class="panel harness-official"><h2>Official Kaggle</h2>
  <p>ARC-AGI-3 public yarışmasına gönderim yerel harness'ten ayrıdır ve yalnızca aşağıdaki
  düğmeye bastığınızda başlar. Resmi skor yerel sıralamayı değiştirmez.</p>
  {current}
  <form method="post" action="/agentic-harness/kaggle/credentials" class="subform kaggle-credentials">
    <input name="username" autocomplete="username" placeholder="Kaggle kullanıcı adı" required>
    <input name="token" type="password" autocomplete="new-password" placeholder="Kaggle API token" required>
    <button class="btn-dark">Tokenı kaydet / değiştir</button>
  </form>
</section>"##
        )
    };
    let history = if team.is_none() {
        "<p class='muted'>Henüz bir takımda değilsin — eğitmenine yaz.</p>".to_string()
    } else if runs.is_empty() {
        "<p class='muted'>Henüz gönderim yok.</p>".to_string()
    } else {
        let table_rows: String = runs.iter().map(|r| {
            let (label, class) = harness_stage_tr(&r.stage);
            // commit_sha is worker-supplied: only plain hex ever reaches an href
            let commit = match r.commit_sha.as_deref() {
                Some(sha)
                    if builtin_harness_label(&r.repo_url).is_some()
                        && sha.len() >= 7
                        && sha.chars().all(|c| c.is_ascii_hexdigit()) =>
                {
                    format!("{} · <code>{}</code>", harness_source_label(&r.repo_url), esc(&sha[..7]))
                }
                Some(sha) if sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit()) => {
                    let base = r.repo_url.trim_end_matches('/').trim_end_matches(".git");
                    format!(r#"<a href="{base}/commit/{sha}" target="_blank" rel="noopener"><code>{short}</code></a>"#,
                        base = esc(base), sha = esc(sha), short = esc(&sha[..7]))
                }
                _ => "—".into(),
            };
            let commit = format!(
                "{commit}<small>{} · {} · {}</small>",
                esc(&r.benchmark_kind),
                esc(&r.provider),
                esc(&r.model_id)
            );
            let log = r.error_log.as_deref().filter(|l| !l.trim().is_empty())
                .map(|l| format!(r#"<details class="plan-details"><summary>Günlük</summary><pre class="plan-pre">{}</pre></details>"#, esc(l)))
                .unwrap_or_else(|| "—".into());
            let official_cell = match official.iter().find(|submission| submission.run_id == r.id) {
                Some(submission) => {
                    let (official_label, official_class) = harness_kaggle_status(&submission.status);
                    let score = match (submission.public_score, submission.private_score) {
                        (None, None) => String::new(),
                        (public, private) => format!("<small>public {} · private {}</small>", fmt(public), fmt(private)),
                    };
                    let kernel = submission.kernel_slug.as_deref()
                        .filter(|slug| slug.split('/').count() == 2 && slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_')))
                        .map(|slug| format!(r#"<a href="https://www.kaggle.com/code/{slug}" target="_blank" rel="noopener">notebook v{}</a>"#,
                            submission.kernel_version.unwrap_or(0)))
                        .unwrap_or_default();
                    let message = submission.status_message.as_deref().filter(|v| !v.is_empty())
                        .map(|v| format!("<small>{}</small>", esc(v))).unwrap_or_default();
                    let reference = submission.submission_ref.as_deref().filter(|v| !v.is_empty())
                        .map(|v| format!("<small><code>{}</code></small>", esc(v))).unwrap_or_default();
                    let retry = if submission.status == "failed" && credential_username.is_some() {
                        format!(r#"<form method="post" action="/agentic-harness/kaggle/submit">
  <input type="hidden" name="run_id" value="{}"><button class="btn-outline">Tekrar gönder</button></form>"#, r.id)
                    } else { String::new() };
                    format!(r#"<div class="official-state"><span class="substatus {official_class}">{official_label}</span>{score}{kernel}{message}{reference}<small>{}</small>{retry}</div>"#,
                        submission.updated_at.format("%d.%m.%Y %H:%M"))
                }
                None if official_enabled && credential_username.is_some()
                    && builtin_harness_label(&r.repo_url).is_none()
                    && r.benchmark_version == HARNESS_VERSION
                    && r.score_arc.is_some() && r.commit_sha.is_some() => format!(
                    r#"<form method="post" action="/agentic-harness/kaggle/submit" onsubmit="return confirm('Bu commit ARC-AGI-3 public yarışmasına gönderilsin mi?')">
  <input type="hidden" name="run_id" value="{}"><button class="btn-outline">Resmi gönder</button></form>"#, r.id),
                _ => "—".into(),
            };
            // an ARC score means the run got far enough to have stored frames worth replaying
            let replay = match r.score_arc {
                Some(_) => format!(
                    r#"<a class="btn-outline small" href="/agentic-harness?tab=live&amp;run={}">İzle</a>"#,
                    r.id
                ),
                None => "—".into(),
            };
            format!(
                "<tr><td>{date}</td><td>{commit}</td><td><span class=\"substatus {class}\">{label}</span></td>\
                 <td>{arc}</td><td>{terminal}</td><td>{r1}</td><td>{r10}</td><td>{official_cell}</td><td>{replay}</td><td>{log}</td></tr>",
                date = r.created_at.format("%d.%m.%Y %H:%M"),
                arc = fmt(r.score_arc), terminal = fmt(r.score_frontier),
                r1 = fmt(r.ram_1session_mb), r10 = fmt(r.ram_10session_mb),
            )
        }).collect();
        format!(
            r##"<section class="panel wide">
  <table><tr><th>Tarih</th><th>Commit</th><th>Durum</th><th lang="en">ARC-AGI-3</th><th lang="en">Terminal Sprint</th><th>RAM 1 oturum (MB)</th><th>RAM 10 oturum (MB)</th><th>Official Kaggle</th><th>Tekrar</th><th></th></tr>{table_rows}</table>
</section>"##
        )
    };
    let inner = format!("{credential_panel}{history}");
    harness_shell(
        user,
        "history",
        "Hangi commit hangi puanı aldı — yerel ve açıkça başlatılan resmi gönderimler burada listelenir.",
        &inner,
    )
}

/// Instructions tab — the submission rules. Turkish prose via Google Translate API
/// (house convention); benchmark names and code literals stay English.
pub fn agentic_harness_instructions(user: &User) -> String {
    let inner = r##"<div class="admingrid stack">
<section class="panel">
  <h2>Nasıl çalışır</h2>
  <p>Takımınız bir AI ajanı oluşturur ve onu bir kez gönderir. Aynı gönderim üç skor tablosunda da
  puanlanır: <b lang="en">ARC-AGI-3</b>, <b lang="en">Terminal-Bench</b> ve <b lang="en">RAM-bench</b>.</p>
  <p>Repo herkese açık olmalı ve bağlantı <code>https://github.com/</code> ile başlamalıdır.
  Herhangi bir takım üyesi tüm takım adına gönderebilir. Aynı anda yalnızca bir çalıştırma devam edebilir.
  En iyi puanınız her skor tablosunda dikkate alınır; RAM-bench için en düşük değer sayılır.</p>
</section>
<section class="panel">
  <h2>Repo yapısı</h2>
  <p>Deponuzda iki ajan bulunmalıdır: biri <span lang="en">ARC-AGI-3</span> için, diğeri
  <span lang="en">Terminal-Bench</span> için. Depoyu klonlayıp tam otomatik olarak çalıştırıyoruz;
  yapıya uymayan bir gönderim puanlanmadan başarısız olur.</p>
  <pre class="plan-pre" lang="en">takim-repo/
├── agent/
│   ├── my_agent.py       # ARC-AGI-3: class MyAgent(Agent)
│   ├── harbor_agent.py   # Terminal-Bench: class HarborAgent(BaseAgent)
│   └── ...               # geri kalanı size kalmış
├── main.py               # RAM-bench oturum giriş noktası
└── requirements.txt      # bağımlılıklar</pre>
  <p><code>main.py</code> ajanınızın bağımsız RAM oturumudur. Standart prompt'u stdin'den okur,
  stdout'a boş olmayan bir yanıt yazar ve 10 saniyelik sert sınır içinde başarıyla çıkar.</p>
</section>
<section class="panel">
  <h2 lang="en">RAM-bench</h2>
  <p><span lang="en">RAM-bench</span> kendi başına bir benchmark'tır ve diğer ikisiyle hiçbir
  ilgisi yoktur. <code>main.py</code>'yi önce 1 oturum, ardından 10 eşzamanlı oturum olarak
  aynı sabit prompt ile çalıştırıyoruz. Her senaryo en fazla 10 saniye sürer. Tek konteyner
  cgroup'undaki tüm alt süreçlerin toplam belleğini (<span lang="en">PSS</span>) 20 ms'de bir
  örnekliyoruz; zirve değer puanınızdır. Her oturum tam bir LLM isteği yapmalıdır ve
  <code>main.py</code>'nin yaptığı her şey ölçülür. Daha düşük olması daha iyidir.</p>
</section>
<section class="panel">
  <h2 lang="en">agent/my_agent.py — ARC-AGI-3</h2>
  <p>Bu dosya <span lang="en">ARC-AGI-3</span> oyun motorunun içinde çalışır. Sınıfınızın adı
  <code>MyAgent</code> olmalıdır, her adımda bir eylem seçer ve ne zaman biteceğine karar verir.
  <code>requirements.txt</code> dosyasındaki bağımlılıklar bu benchmark çalıştırılmadan önce yüklenir.</p>
  <p>En kolay yol, döngüyü kendiniz yazmak yerine framework'ün kendi ajanını devralmaktır.
  Başlangıç seti; sohbet geçmişini, <code lang="en">RESET</code> başlatmasını ve eylem
  araçlarını hazır yöneten bir <span lang="en">LLM</span> ajanı içerir — siz yalnızca prompt'u
  ve stratejiyi sağlarsınız.</p>
  <pre class="plan-pre" lang="en">from agents.templates.llm_agents import LLM

class MyAgent(LLM):
    MODEL = os.environ["HARNESS_LLM_MODEL"]
    MODEL_REQUIRES_TOOLS = True
    DO_OBSERVATION = False          # True = her eylem öncesi ek bir düşünme çağrısı

    def build_user_prompt(self, latest_frame) -> str:
        return "..."   # asıl kaldıraç burada</pre>
  <p>Sıfırdan yazmak isterseniz <code>agents.agent.Agent</code>'ı devralıp
  <code>choose_action(frames, latest_frame)</code> ve <code>is_done(...)</code> metotlarını
  kendiniz yazın.</p>
  <p>Yerel puan, sabitlenmiş <code>arc-agi==0.9.9</code> public veri setindeki 25 oyunun
  ortalamasıdır. Aynı anda en fazla beş oyun çalışır; biri bitince sıradaki başlar. Oyun
  <code>WIN</code>, <code>GAME_OVER</code>, ajanınızın <code>is_done</code> kararı veya
  <code>MAX_ACTIONS</code> sınırıyla biter. Koşu için dokuz saatlik güvenlik sınırı vardır ve
  istediğiniz zaman <b>Durdur</b> düğmesini kullanabilirsiniz. Her tahtayı
  <a href="/agentic-harness?tab=live">Canlı</a> sekmesinden izleyebilirsiniz.</p>
  <p>Uç nokta size <code lang="en">OPENAI_BASE_URL</code> ve <code lang="en">OPENAI_API_KEY</code>
  ile verilir; <span lang="en">OpenAI SDK</span> bunları kendi kendine okur.
  <span lang="en">LLM</span> şablonunu devralırsanız <code>requirements.txt</code> dosyanıza
  <code lang="en">openai</code> ekleyin.</p>
  <p><b>Token maliyeti:</b> varsayılan çerçeve kodlaması her satırı bir
  <span lang="en">Python</span> listesi olarak yazdırır ve bu, çağrı başına yaklaşık 52 bin
  token'a mal olur. <code>pretty_print_3d</code>'yi hücre başına bir karakter yazacak şekilde
  geçersiz kılmak, hiçbir bilgi kaybı olmadan bunu kabaca beş kat azaltır.</p>
  <p><b>Eşzamanlılık:</b> Beş ARC oyunu model geçidini sürekli kullanır. Yavaş bir model
  çalıştırmayı iptal etmez; oyunlar bitene, ajan durana veya siz koşuyu durdurana kadar devam eder.</p>
</section>
<section class="panel">
  <h2 lang="en">agent/harbor_agent.py — Terminal Sprint</h2>
  <p>Bu dosya <span lang="en">Terminal Sprint (Harbor)</span> ortamının içinde çalışır. Her görevi,
  yalıtılmış bir konteyner içinde kabuk komutları çalıştırarak çözer.</p>
  <p>En kolay yol, <span lang="en">Harbor</span> içinde hazır gelen referans terminal ajanı
  <span lang="en">Terminus 2</span>'yi devralmaktır: konteynerde bir <span lang="en">tmux</span>
  oturumu, araç çağrısı protokolü ve bağlam özetleme — hepsi ayarlanmış durumda.
  Siz yalnızca ayarları değiştirirsiniz.</p>
  <pre class="plan-pre" lang="en">from harbor.agents.terminus_2 import Terminus2

class HarborAgent(Terminus2):
    def __init__(self, *args, **kwargs):
        kwargs.setdefault("max_turns", 40)
        kwargs.setdefault("temperature", 0.0)
        super().__init__(*args, **kwargs)

    @staticmethod
    def name() -> str: return "takim-ajani"
    def version(self) -> str: return "1.0"</pre>
  <p>Sıfırdan yazmak isterseniz <code lang="en">harbor.agents.base.BaseAgent</code>'ı devralıp
  <code>name()</code>, <code>version()</code>, <code>setup()</code> ve <code>run()</code>
  metotlarını yazın; görevi <code>environment.exec("komut")</code> ile çözersiniz.</p>
  <p>Önemli: bu dosya <span lang="en">Harbor</span>'ın kendi Python'unda çalışır, yani
  <code>requirements.txt</code> onun için <b>yüklenmez</b> — yalnızca
  <span lang="en">Harbor</span>'ın kendi paketlerini ve standart kütüphaneyi içe aktarabilirsiniz.
  Model, çalıştırıcı tarafından verilir; deponuzda hiçbir anahtar durmaz. Kendi
  <span lang="en">HTTP</span> çağrınızı <code>urllib</code> ile yazarsanız bir
  <code lang="en">User-Agent</code> başlığı ekleyin — varsayılan başlık 403 ile reddedilir.</p>
  <p>Sprint tam Terminal-Bench değildir: sürümlenmiş beş görev aynı anda başlar. Her ajanın
  sert süresi 120 saniye, doğrulayıcının süresi 60 saniyedir. İki dakika bilinçli bir sprint
  bütçesidir; derin görevler zaman aşımına uğrayabilir ve bu puanın parçasıdır. Konteyner
  çalıştırması 15 dakikalık toplam bütçeye ulaştığında durdurulur ve ardından temizlenir.</p>
</section>
<section class="panel">
  <h2>LLM erişimi</h2>
  <p>Tüm LLM çağrıları AWS Bedrock application inference profile üzerinden, harness'ın yerel
  OpenAI uyumlu geçidiyle gider. AWS kimlik bilgileri konteynere verilmez; geçici yerel anahtar
  yalnızca bu geçide erişir. API anahtarlarını asla deponuza koymayın.</p>
  <table><tr><th>Değişken</th><th>Açıklama</th></tr>
    <tr><td><code>HARNESS_LLM_BASE</code></td><td><span lang="en">OpenAI</span> uyumlu API adresi</td></tr>
    <tr><td><code>HARNESS_LLM_KEY</code></td><td>API anahtarı</td></tr>
    <tr><td><code>HARNESS_LLM_MODEL</code></td><td>Model adı</td></tr>
    <tr><td><code lang="en">OPENAI_BASE_URL</code> / <code lang="en">OPENAI_API_KEY</code></td>
      <td>Aynı adres ve anahtar, <span lang="en">OpenAI SDK</span>'nın okuduğu isimlerle —
      <code>agent/my_agent.py</code> için</td></tr>
  </table>
  <p><code>agent/harbor_agent.py</code> bu değişkenleri kullanmaz: modeli
  <span lang="en">Harbor</span>'ın <code>-m</code> parametresiyle çalıştırıcı verir.</p>
</section>
<section class="panel">
  <h2>Kurallar</h2>
  <ul class="harness-rules">
    <li>Tüm bağımlılıklar <code>requirements.txt</code> dosyasında listelenmelidir.</li>
    <li>Ajanın başsız (<span lang="en">headless</span>) çalışması gerekir: GUI penceresi ve
    etkileşimli bilgi istemi yok.</li>
    <li><span lang="en">Terminal Sprint</span>, sürümlenmiş 5 görevi paralel ve görev başına
    tek denemeyle çalıştırır. Puan, doğrulayıcı ödüllerinin 0–100 ortalamasıdır.</li>
    <li>RAM, ARC ve Terminal bağımsız sonuç verir. Biri başarısız olsa bile biten benchmark'ın
    puanı korunur. Tüm yerel çalıştırma hazırlık dahil en fazla 9 saattir ve elle durdurulabilir.</li>
    <li><code>requirements.txt</code> doğrudan URL, git, yerel dosya veya pip seçeneği içeremez;
    repo 100 MiB, tek dosya 10 MiB ile sınırlıdır.</li>
  </ul>
</section>
<section class="panel">
  <h2>Official Kaggle</h2>
  <p>Geçmiş sekmesinden takımınızın Kaggle kullanıcı adı ve API tokenını şifreli olarak
  kaydedebilirsiniz. Yalnızca açıkça “Resmi gönder” düğmesine bastığınızda, yerelde puanlanan
  tam commit official ARC-AGI-3 public yarışma notebook'una paketlenir. Kaggle puanlaması
  asenkron izlenir ve yerel sıralamayı değiştirmez. Token hiçbir zaman ajan konteynerine girmez.</p>
</section>
<section class="panel">
  <h2>Örnek repo ve bağlantılar</h2>
  <p>Aşağıdaki örnek depo her aşamayı geçer ve her iki ajan sözleşmesini de gösterir — ondan başlayın.</p>
  <ul class="harness-rules" lang="en">
    <li><a href="https://github.com/Darkosxl/harness-mockup-agent" target="_blank" rel="noopener">harness-mockup-agent — reference example</a></li>
    <li><a href="https://docs.arcprize.org/arc-prize-2026" target="_blank" rel="noopener">ARC Prize 2026 SDK docs</a></li>
    <li><a href="https://arcprize.org/arc-agi/3" target="_blank" rel="noopener">ARC-AGI-3 — what it tests</a></li>
    <li><a href="https://www.tbench.ai/" target="_blank" rel="noopener">Terminal-Bench — official site</a></li>
    <li><a href="https://harborframework.com/" target="_blank" rel="noopener">Harbor — BaseAgent docs</a></li>
  </ul>
</section>
</div>"##;
    harness_shell(
        user,
        "instructions",
        "Gönderim kuralları — repo yapısı ve değerlendirme süreci.",
        inner,
    )
}

// ---- AI Monopoly ----

/// (tab key, href, label). "Instructions" stays English, matching the harness.
const MONOPOLY_TABS: [(&str, &str, &str); 4] = [
    ("main", "/ai-monopoly", "Gönderim ve Sıralama"),
    ("live", "/ai-monopoly?tab=live", "Canlı"),
    ("history", "/ai-monopoly?tab=history", "Geçmiş"),
    (
        "instructions",
        "/ai-monopoly?tab=instructions",
        "Instructions",
    ),
];

fn monopoly_shell(user: &User, tab: &str, sub: &str, inner: &str) -> String {
    let chips: String = MONOPOLY_TABS
        .iter()
        .map(|(k, href, label)| {
            let active = if tab == *k { "active" } else { "" };
            format!(r#"<a class="chip {active}" href="{href}">{label}</a>"#)
        })
        .collect();
    layout(
        "AI Monopoly",
        Some(user),
        "ai-monopoly",
        &format!(
            r##"<h1 class="pagetitle" lang="en">AI Monopoly</h1>
<p class="muted">{sub}</p>
<div class="chips">{chips}</div>
{inner}"##
        ),
    )
}

/// What students see at /ai-monopoly until the section opens. The nav link stays in the
/// sidebar on purpose — "coming soon" only reads as a promise if you can find it.
pub fn monopoly_coming_soon(user: &User) -> String {
    layout(
        "AI Monopoly",
        Some(user),
        "ai-monopoly",
        r##"<h1 class="pagetitle" lang="en">COMING SOON!</h1>
<p class="muted">AI Monopoly yakında burada.</p>"##,
    )
}

/// ₺ with Turkish thousands separators (1.234 ₺). Used everywhere money is shown so the
/// arena, the standings and the history tab can't drift apart on formatting.
fn money(v: i32) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    format!("{}{} ₺", if v < 0 { "-" } else { "" }, out)
}

/// The standings list, shared by the main tab and the arena's sidebar. `my_team` gets
/// the `.mine` highlight, same convention as the points leaderboard.
fn monopoly_standings_rows(standings: &[MonopolyStandingRow], my_team: Option<Uuid>) -> String {
    if standings.is_empty() {
        return "<p class='muted'>Turnuva başlayınca sıralama burada görünecek.</p>".into();
    }
    let ranks = dense_ranks_by(standings, |r| r.net_worth().to_string());
    standings
        .iter()
        .zip(ranks)
        .map(|(r, rank)| {
            let medal = match rank {
                1 => "m1",
                2 => "m2",
                3 => "m3",
                _ => "",
            };
            let mine = if Some(r.team_id) == my_team {
                "mine"
            } else {
                ""
            };
            format!(
                r##"<div class="lbrow {medal} {mine}">
  <span class="lbrank">{rank}</span>
  <span class="lbname">{char}<span class="lbmeta">{team} · {product}</span></span>
  <span class="lbpts">{net}<span class="lbmeta">{cash} nakit · {goods} mal</span></span>
</div>"##,
                char = esc(&r.char_name),
                team = esc(&r.team_name),
                product = esc(&r.product_name),
                net = money(r.net_worth()),
                cash = money(r.cash),
                goods = money(r.goods)
            )
        })
        .collect()
}

pub fn monopoly_main(
    user: &User,
    team: Option<&MonopolyTeam>,
    members: &[TeamMemberRow],
    entry: Option<&MonopolyEntry>,
    tournament: Option<&MonopolyTournament>,
    standings: &[MonopolyStandingRow],
    practice: &[MonopolyMatchRow],
) -> String {
    let running = tournament.is_some_and(|t| t.status != "done" && t.status != "failed");
    let left = match team {
        None => r##"<div class="panel">
  <h2>Takımın yok</h2>
  <p class="muted">Bu bölüm takım hâlinde oynanır. Eğitmenine yaz, seni bir takıma eklesin.</p>
</div>"##
            .to_string(),
        Some(t) => {
            let roster: String = members
                .iter()
                .filter(|m| m.team_id == t.id)
                .map(|m| format!(r#"<span class="chip">{}</span>"#, esc(&m.display_name)))
                .collect();
            // Prefill from the current entry so "change one field" doesn't mean retyping
            // the whole merchant.
            let v = |s: Option<&str>| esc(s.unwrap_or(""));
            let (repo, char_name, product_name, product_desc, persona) = (
                v(entry.map(|e| e.hf_repo.as_str())),
                v(entry.map(|e| e.char_name.as_str())),
                v(entry.map(|e| e.product_name.as_str())),
                v(entry.map(|e| e.product_desc.as_str())),
                v(entry.map(|e| e.persona.as_str())),
            );
            let price = entry.map(|e| e.list_price).unwrap_or(100);
            let current = match entry {
                Some(e) => format!(
                    r##"<p class="fieldnote">Şu anki gönderim: <b>{char}</b> — {product} · {price}
                    · <span lang="en">{repo}</span>{size} · {when} tarihinde güncellendi.</p>"##,
                    char = esc(&e.char_name),
                    product = esc(&e.product_name),
                    price = money(e.list_price),
                    repo = esc(&e.hf_repo),
                    size = match e.size_bytes {
                        Some(b) => format!(" · {:.1} GiB", b as f64 / (1024.0 * 1024.0 * 1024.0)),
                        None => String::new(),
                    },
                    when = e.updated_at.format("%d.%m.%Y %H:%M")
                ),
                None => r##"<p class="fieldnote">Henüz gönderim yok. Modelini
                    <span lang="en">Hugging Face</span>'e yükle, kimliğini buraya yapıştır.</p>"##
                    .to_string(),
            };
            let form = if running {
                r##"<p class="muted">Turnuva sürerken gönderim değiştirilemez.</p>"##.to_string()
            } else {
                format!(
                    r##"<form method="post" action="/ai-monopoly/submit">
    <label>Model (<span lang="en">Hugging Face</span>)<input name="hf_repo" placeholder="org/model" value="{repo}" required></label>
    <p class="fieldnote">Depo herkese açık olmalı, ağırlıklar <span lang="en">bf16 safetensors</span>
    — nicemleme (<span lang="en">quantization</span>, <span lang="en">GGUF</span>) kabul edilmiyor.</p>
    <label>Karakter adı<input name="char_name" maxlength="40" value="{char_name}" required></label>
    <label>Ürün adı<input name="product_name" maxlength="60" value="{product_name}" required></label>
    <label>Ürün açıklaması<textarea name="product_desc" rows="3" maxlength="300" required>{product_desc}</textarea></label>
    <label>Fiyat (₺)<input name="list_price" type="number" min="1" max="100000" value="{price}" required></label>
    <label>Karakter tanımı<textarea name="persona" rows="5" maxlength="1500" required>{persona}</textarea></label>
    <p class="fieldnote">Karakter tanımı modelin sistem promptuna girer. Konuşmalar
    <span lang="en">İngilizce</span> geçer — bu alanları da <span lang="en">İngilizce</span> yaz.</p>
    <button class="btn-dark">Gönder</button>
  </form>"##
                )
            };
            // Practice needs a submission to practise with, and is closed while the
            // tournament runs — the GPUs are busy and the entry is frozen anyway.
            let practice_panel = if entry.is_none() {
                String::new()
            } else {
                let rows: String = practice.iter().map(|p| {
                    let (label, class) = match p.status.as_str() {
                        "done" => ("Bitti", "st-passed"),
                        "failed" => ("Başarısız", "st-failed"),
                        "queued" => ("Sırada", "st-reviewing"),
                        _ => ("Sürüyor", "st-reviewing"),
                    };
                    let body = format!(
                        r##"<span class="lbname">{a} ↔ {b}<span class="lbmeta">{date}</span></span>
  <span class="lbpts"><span class="substatus {class}">{label}</span></span>"##,
                        a = esc(&p.a_name), b = esc(&p.b_name),
                        date = p.created_at.format("%d.%m %H:%M"));
                    // only a finished conversation has anything to open
                    if p.status == "done" {
                        format!(r#"<a class="lbrow" href="/ai-monopoly/match/{}">{body}</a>"#, p.id)
                    } else {
                        format!(r#"<div class="lbrow">{body}</div>"#)
                    }
                }).collect();
                let button = if running {
                    r##"<p class="fieldnote">Turnuva sürerken antrenman yapılamaz.</p>"##
                        .to_string()
                } else {
                    r##"<form method="post" action="/ai-monopoly/practice">
      <button class="btn-outline">Antrenman maçı başlat</button>
    </form>"##
                        .to_string()
                };
                format!(
                    r##"<div class="panel">
  <h2>Antrenman</h2>
  <p class="fieldnote">Başka bir takımın modeline karşı deneme konuşması. Rakip, gerçek
  kimliği yerine uydurma bir tüccar olarak çıkar — sonuçlar sıralamayı etkilemez.</p>
  {button}
  <div class="lb practicelist">{rows}</div>
</div>"##
                )
            };
            format!(
                r##"<div class="panel">
  <h2>{team}</h2>
  <div class="chips">{roster}</div>
  {current}
  {form}
</div>
{practice_panel}"##,
                team = esc(&t.name)
            )
        }
    };
    let status_line = match tournament {
        Some(t) if t.status != "done" && t.status != "failed" => {
            let (label, _) = monopoly_status_tr(&t.status);
            format!(
                "Turnuva sürüyor — tur {}/{} · {label}",
                t.round, t.rounds_total
            )
        }
        Some(t) if t.status == "done" => "Turnuva bitti — kazanan en üstte.".to_string(),
        _ => "Turnuva henüz başlamadı.".to_string(),
    };
    let inner = format!(
        r##"<div class="harnesswrap">
<div class="harness-left">{left}</div>
<div class="harness-right">
  <p class="muted">{status_line}</p>
  <div class="lb">{rows}</div>
  <p class="lbnote">Sıralama servet = nakit + mal. Mal, hakemin o ürüne biçtiği değerdir.</p>
</div>
</div>"##,
        rows = monopoly_standings_rows(standings, team.map(|t| t.id))
    );
    monopoly_shell(
        user,
        "main",
        "Modelini gönder, karakterini yaz, pazarlığı izle.",
        &inner,
    )
}

/// The arena. Everything inside `#arena` is (re)built by monopoly.js from the poll
/// payload — the server renders only the frame and the idle state, so there is exactly
/// one implementation of a match view and it lives in the JS.
pub fn monopoly_live(
    user: &User,
    tournament: Option<&MonopolyTournament>,
    standings: &[MonopolyStandingRow],
) -> String {
    let running = tournament.is_some_and(|t| t.status != "done" && t.status != "failed");
    // Idle is a real state, not an empty page: say where the game is and where to look.
    let idle = match tournament {
        None => r##"<div class="arena-idle">
  <h2>Turnuva henüz başlamadı</h2>
  <p class="muted">Takımlar modellerini gönderiyor. Başladığında konuşmalar burada canlı akacak.</p>
  <a class="btn-outline" href="/ai-monopoly?tab=instructions">Kuralları oku</a>
</div>"##
            .to_string(),
        Some(t) if t.status == "done" => {
            let winner = standings
                .first()
                .map(|w| {
                    format!(
                        "<p class=\"arena-winner\">🏆 {} — {}</p>",
                        esc(&w.char_name),
                        money(w.net_worth())
                    )
                })
                .unwrap_or_default();
            format!(
                r##"<div class="arena-idle">
  <h2>Turnuva bitti</h2>
  {winner}
  <a class="btn-outline" href="/ai-monopoly?tab=history">Konuşmaları oku</a>
</div>"##
            )
        }
        Some(t) => format!(
            r##"<div class="arena-idle">
  <h2>{label}</h2>
  <p class="muted">{progress}</p>
</div>"##,
            label = monopoly_status_tr(&t.status).0,
            progress = esc(t.progress.as_deref().unwrap_or("Birazdan başlıyor…"))
        ),
    };
    let inner = format!(
        r##"<div class="arenawrap">
  <div id="arena" class="arena" data-live="{live}">{idle}</div>
  <aside class="arena-side">
    <p class="muted">Sıralama</p>
    <div class="lb" id="arena-standings">{rows}</div>
  </aside>
</div>
<script src="/static/monopoly.js?v=2" defer></script>"##,
        live = running,
        rows = monopoly_standings_rows(standings, None)
    );
    monopoly_shell(
        user,
        "live",
        "İki model karşı karşıya — konuşma bitince hakem parayı böler.",
        &inner,
    )
}

pub fn monopoly_history(
    user: &User,
    tournament: Option<&MonopolyTournament>,
    matches: &[MonopolyMatchRow],
) -> String {
    let done = tournament.is_some_and(|t| t.status == "done");
    let rows: String = if matches.is_empty() {
        "<p class='muted'>Henüz tamamlanmış konuşma yok.</p>".into()
    } else {
        // the list is already ordered by round descending; a heading each time the round
        // changes is enough grouping, and needs no second pass over the rows
        let mut round = -1;
        matches
            .iter()
            .map(|m| {
                let head = if m.round != round {
                    round = m.round;
                    format!(r#"<p class="roundhead">Tur {round}</p>"#)
                } else {
                    String::new()
                };
                let kind = match m.kind.as_str() {
                    "mandatory" => "Eşleşme",
                    "chosen" => "Davet",
                    _ => "Deneme",
                };
                let (label, class) = match m.status.as_str() {
                    "done" => ("Bitti", "st-passed"),
                    "failed" => ("Başarısız", "st-failed"),
                    _ => ("Sürüyor", "st-reviewing"),
                };
                format!(
                    r##"{head}<a class="lbrow" href="/ai-monopoly/match/{id}">
  <span class="lbname">{a} ↔ {b}<span class="lbmeta">{kind} · {date}</span></span>
  <span class="lbpts"><span class="substatus {class}">{label}</span></span>
</a>"##,
                    head = head,
                    id = m.id,
                    a = esc(&m.a_name),
                    b = esc(&m.b_name),
                    kind = kind,
                    date = m.created_at.format("%d.%m %H:%M"),
                    class = class,
                    label = label
                )
            })
            .collect()
    };
    let note = if done {
        "Turnuva bittiği için modellerin birbiri hakkında tuttuğu notlar da açık."
    } else {
        "Modellerin birbiri hakkında tuttuğu notlar turnuva bitince açılacak."
    };
    let inner = format!(
        r##"<div class="lb">{rows}</div>
<p class="lbnote">{note}</p>"##
    );
    monopoly_shell(
        user,
        "history",
        "Bütün konuşmalar, hakem kararları ve para akışı.",
        &inner,
    )
}

/// One conversation, replayed. Both languages ship in the same markup and the toggle
/// flips a class on the wrapper — no second request, and the English original is always
/// one click away from the translation.
pub fn monopoly_match(
    user: &User,
    m: &MonopolyMatchRow,
    msgs: &[MonopolyMessage],
    msgs_tr: &[String],
    txs: &[MonopolyTxRow],
    txs_tr: &[String],
    notes: &[MonopolyNoteRow],
    notes_tr: &[String],
    reveal: bool,
) -> String {
    let translated = !msgs_tr.is_empty();
    /// Both languages in the markup; CSS shows one. Empty translation falls back to the
    /// English, which is what an untranslatable or un-keyed match ends up rendering.
    fn pair(en: &str, tr: Option<&String>) -> String {
        match tr.filter(|t| !t.is_empty()) {
            Some(t) => format!(
                r#"<span class="en">{}</span><span class="tr">{}</span>"#,
                esc(en),
                esc(t)
            ),
            None => esc(en),
        }
    }
    let bubbles: String = msgs
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let (side, who) = if x.speaker == "a" {
                ("l", &m.a_name)
            } else {
                ("r", &m.b_name)
            };
            let turkish = match msgs_tr.get(i).filter(|t| !t.is_empty()) {
                Some(t) => format!(r#"<div class="say tr">{}</div>"#, esc(t)),
                None => String::new(),
            };
            format!(
                r##"<div class="bub {side}">
  <div class="who">{who}</div>
  <div class="say en">{en}</div>
  {turkish}
</div>"##,
                who = esc(who),
                en = esc(&x.content)
            )
        })
        .collect();

    let verdict = if txs.is_empty() {
        "<p class='muted'>Hakem: satış yok — anlaşma çıkmadı.</p>".to_string()
    } else {
        let rows: String = txs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let s = t.surplus();
                format!(
                    r##"<div class="vrow">
  <span class="vflow">{seller} → {buyer}</span>
  <span class="vitem">{item}</span>
  <span class="vprice">{price}</span>
  <span class="vsurp {cls}">{sign}{surp}</span>
</div>{why}"##,
                    seller = esc(&t.seller_name),
                    buyer = esc(&t.buyer_name),
                    item = esc(&t.item),
                    price = money(t.price),
                    cls = if s >= 0 { "ok" } else { "bad" },
                    sign = if s >= 0 { "+" } else { "" },
                    surp = money(s),
                    why = match &t.reasoning {
                        Some(r) if !r.is_empty() =>
                            format!(r#"<p class="vwhy">{}</p>"#, pair(r, txs_tr.get(i))),
                        _ => String::new(),
                    }
                )
            })
            .collect();
        format!(r##"<h3>Hakem kararı</h3>{rows}"##)
    };

    let notes_block = if !reveal {
        r##"<p class="lbnote">Modellerin birbiri hakkında tuttuğu notlar turnuva bitince açılacak.</p>"##.to_string()
    } else if notes.is_empty() {
        r##"<p class="lbnote">Bu konuşmadan not çıkmadı.</p>"##.to_string()
    } else {
        let rows: String = notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                format!(
                    r##"<div class="noterow">
  <span class="notewho">{author} → {about}</span>
  <p class="notetext">{note}</p>
</div>"##,
                    author = esc(&n.author_name),
                    about = esc(&n.about_name),
                    note = pair(&n.note, notes_tr.get(i))
                )
            })
            .collect();
        format!(r##"<div class="panel"><h2>Modellerin notları</h2>{rows}</div>"##)
    };

    let kind = match m.kind.as_str() {
        "mandatory" => "Eşleşme",
        "chosen" => "Davet",
        _ => "Deneme",
    };
    let inner = format!(
        r##"<p class="muted"><a href="/ai-monopoly?tab=history">← Geçmiş</a></p>
<div class="matchhead">
  <h2>{a} ↔ {b}</h2>
  <p class="muted">Tur {round} · {kind} · {date}</p>
  <div class="langtoggle">
    <button class="chip active" data-lang="tr">Türkçe</button>
    <button class="chip" data-lang="en" lang="en">English</button>
  </div>
</div>
<div class="matchbody show-tr{no_tr}" id="replay">
  <div class="arena-chat replay">{bubbles}</div>
  <div class="arena-verdict">{verdict}</div>
  {notes_block}
</div>
<script src="/static/monopoly.js?v=2" defer></script>"##,
        a = esc(&m.a_name),
        b = esc(&m.b_name),
        round = m.round,
        kind = kind,
        date = m.created_at.format("%d.%m.%Y %H:%M"),
        // with no translation the toggle would swap the transcript for nothing
        no_tr = if translated { "" } else { " untranslated" }
    );
    monopoly_shell(
        user,
        "history",
        "Konuşmanın tamamı, hakem kararı ve notlar.",
        &inner,
    )
}

/// Turkish prose, English technical terms in `lang="en"` spans, per the house convention.
/// The paragraphs were written in English and run through DeepL rather than composed in
/// Turkish here — same rule the transcripts follow.
pub fn monopoly_instructions(user: &User) -> String {
    monopoly_shell(
        user,
        "instructions",
        "Gönderim kuralları ve oyunun işleyişi.",
        r##"<div class="rulewrap">
<section class="panel">
  <h2>Nasıl işliyor</h2>
  <p>Ekibiniz küçük bir dil modelini ince ayarlıyor, yayınlıyor ve ona bir tüccar karakteri
  kazandırıyor. Turnuvada, modeliniz diğer ekiplerin modelleriyle masaya oturup pazarlık
  yapıyor. Bir hakem modeli her bir konuşmayı inceliyor ve gerçekte neyin ne kadara
  satıldığına karar veriyor. En zengin olan kazanır.</p>
</section>

<section class="panel">
  <h2>Modelini gönder</h2>
  <p>Modelinizi <span lang="en">Hugging Face</span>'te herkese açık bir depo olarak
  yayınlayın, ardından adını <span lang="en">org/model</span> biçiminde buraya yapıştırın.
  Sayfanın tam adresini de yapıştırabilirsiniz; adresi biz kendimiz kısaltacağız.</p>
  <p>Gönderim yaptığınız anda deponun o anki tam <span lang="en">commit</span> durumunu
  kaydediyoruz. Daha sonra yeni ağırlıklar gönderirseniz bile, turnuva yine de sizin
  gönderdiğiniz sürümü kullanır. Bu nedenle, modelinizde her değişiklik yaptığınızda
  yeniden gönderin.</p>
  <ul class="harness-rules">
    <li>Ağırlıklar <span lang="en">safetensors</span> formatında ve
    <span lang="en">bf16</span> olarak olmalıdır. Kuantize edilmiş modeller ve
    <span lang="en">GGUF</span> dosyaları kabul edilmez; çünkü kuantize edilmiş bir model
    zayıflatılmış bir modeldir ve bu yarışmada önemli olan sıkıştırma değil, eğitim
    sürecinizdir.</li>
    <li>Deponun toplam boyutu en fazla 64 GB olmalıdır. Bu, <span lang="en">bf16</span>'da
    yaklaşık 31 milyar parametreli bir modele karşılık gelir. Daha büyük depolar
    reddedilir.</li>
    <li><span lang="en">Tokenizer</span> yapılandırmasında bir
    <span lang="en">chat template</span> bulunmalıdır. Bu şablon olmadan, bir sohbeti
    modeliniz için bir komut satırına dönüştürmenin tanımlanmış bir yolu yoktur. Çoğu ince
    ayar aracı bunu otomatik olarak ekler; eğer kullandığınız araç bunu yapmadıysa,
    göndermeden önce ekleyin.</li>
  </ul>
</section>

<section class="panel">
  <h2>Tüccarını yaz</h2>
  <p>Tüccarınızı kendiniz yazarsınız: bir karakter adı, bir ürün, o ürünün açıklaması,
  istenen fiyat ve bir karakter tanımı. Karakter tanımı, maç sırasında modelinizin
  <span lang="en">system prompt</span>'u haline gelir; bu nedenle bunu bize yönelik bir
  açıklama olarak değil, modelinize yönelik bir talimat olarak yazın.</p>
  <p>Bu alanların tümünü <span lang="en">İngilizce</span> olarak doldurun. Sohbetler
  <span lang="en">İngilizce</span> olarak gerçekleştirilir ve tamamlanan her sohbet daha
  sonra Türkçeye çevrilir; böylece geçmiş sekmesinden her iki versiyonu da
  okuyabilirsiniz.</p>
</section>

<section class="panel">
  <h2>Para nasıl işliyor</h2>
  <p>Her esnaf, 1.000 ₺ nakit parayla başlar ve elinde mal yoktur.</p>
  <p>Bir satış gerçekleştiğinde, üç şey aynı anda gerçekleşir. Alıcı, kararlaştırılan
  bedeli elindeki nakit paradan öder ve asla sahip olduğu miktardan fazlasını ödeyemez.
  Alıcı, hakemin o alıcı için gerçek değerinin ne olduğunu düşündüğü tutarda kayıt altına
  alınmış ürünü alır. Satıcı ise bedelden, sabit yüzde 40'lık mal maliyetinin
  düşülmesiyle kalan tutarı alır.</p>
  <p>Puanınız, net servetinizdir: nakit paranız artı malınızın değeri. Bundan üç sonuç
  çıkar ve işin özü de budur.</p>
  <ul class="harness-rules">
    <li>Diğer herkes alım satım yaparken hareketsiz kalan nakit hiçbir işe yaramaz; bu
    yüzden biriktirmek kayba yol açar.</li>
    <li>Bir şeyi satın almak, ancak o şeyin sizin için sahip olduğu değerden daha az bir
    bedel ödediğinizde anlamlıdır; bu nedenle, kötü bir anlaşmaya ikna edilmek
    cezalandırılır.</li>
    <li>Maliyetinizin yüzde 40'ının altında satış yapmak zarara yol açar; bu nedenle nakit
    elde etmek için stoklarınızı ucuza elden çıkaramazsınız.</li>
  </ul>
</section>

<section class="panel">
  <h2>Bir tur nasıl geçiyor</h2>
  <p>Her turda, modelinizin fikstür listesindeki bir rakiple planlanmış bir görüşmesi ve
  kendi seçtiği bir rakiple bir görüşmesi olur. Seçilen görüşme, ancak diğer model de
  bunu kabul ederse gerçekleşir.</p>
  <p>Bir sohbette taraflar sırayla konuşur; her bir tarafın en fazla on tur konuşma hakkı
  vardır. Her iki taraf da mesajın sonuna <span lang="en">[END]</span> yazarak sohbeti
  erken sonlandırabilir. Modeliniz kendi karakterini, kendi bakiyesini, o ana kadar geçen
  konuşmayı ve önceki turlarda bu rakip hakkında tuttuğu notları görebilir. Karşı tarafın
  karakterini, başkalarının bakiyelerini veya başkalarının notlarını asla göremez.</p>
  <p>Her sohbetin ardından modeliniz rakibi hakkında özel bir not yazar ve bu not, ikisi
  bir sonraki karşılaşmalarında modele geri verilir. Turnuva sona erdiğinde tüm notlar
  herkese açık hale gelir.</p>
</section>

<section class="panel">
  <h2>Süre sınırları</h2>
  <p>Tek bir yanıt en fazla 120 saniye ve 400 <span lang="en">token</span> sürebilir. Bir
  sohbetin tamamı en fazla 10 dakika, bir deneme maçı ise en fazla 15 dakika sürebilir.
  Zamanında yanıt vermeyen model, o sohbeti kaybeder.</p>
</section>

<section class="panel">
  <h2>Antrenman</h2>
  <p>Turnuva başlamadan önce, diğer takımların gönderdiği modellerle istediğiniz kadar
  antrenman yapabilirsiniz. Antrenmanlarda rakip, gerçek karakteri yerine uydurma bir
  tüccar karakteri kullanır; bu nedenle antrenmanlar, kimin ne sattığını anlamanıza
  yardımcı olmaz.</p>
</section>
</div>"##,
    )
}

// ponytail: hardcoded list — demos are files in static/demos/, add a row here when adding a file
const DEMOS: [(&str, &str, &str); 7] = [
    (
        "ai-timeline/index.html",
        "Makineler Nasıl Öğrenmeyi Öğrendi",
        "Yapay zekânın zaman çizelgesi — 4 bölümlük interaktif seri",
    ),
    (
        "html-css-js-demo.html",
        "HTML + CSS + JS",
        "Koddan çıktıya: web sayfası nasıl oluşur",
    ),
    (
        "backend-frontend-demo.html",
        "Ön Uç ve Arka Uç",
        "İstemci ile sunucu arasındaki iş bölümü",
    ),
    (
        "database-demo.html",
        "Veritabanı Nedir?",
        "Veritabanı nedir, veriler nasıl saklanır",
    ),
    (
        "authentication-demo.html",
        "Kimlik Doğrulama",
        "Kimlik doğrulama nasıl çalışır",
    ),
    (
        "ui-ux-demo.html",
        "UI ve UX",
        "Arayüz ile deneyim arasındaki fark",
    ),
    (
        "package-manager-demo.html",
        "Paket Yöneticisi Nedir?",
        "Paket yöneticileri ne işe yarar",
    ),
];

pub fn demos(user: &User, lang: &str) -> String {
    // lang is validated to "tr" | "en" by the handler; files live in static/demos/{lang}/
    let cards: String = DEMOS.iter().map(|(file, title, desc)| format!(
        r##"<a class="panel demo-card" href="/static/demos/{lang}/{file}" target="_blank" rel="noopener">
  <h3>{title}</h3>
  <p class="meta">{desc}</p>
</a>"##)).collect();
    let chips: String = [("tr", "Türkçe"), ("en", "English")]
        .iter()
        .map(|(k, label)| {
            let active = if lang == *k { "active" } else { "" };
            format!(r#"<a class="chip {active}" href="/demos?lang={k}">{label}</a>"#)
        })
        .collect();
    let content = format!(
        r##"<h1 class="pagetitle">İnteraktif Demolar</h1>
<p class="muted">Derslerde kullanılan interaktif anlatımlar.</p>
<div class="chips">{chips}</div>
<div class="admingrid">{cards}</div>"##
    );
    layout("İnteraktif Demolar", Some(user), "demos", &content)
}

/// Ana Sayfa — portalın giriş kapısı. İçerik yok, yalnızca dört büyük hedef:
/// Online, Beginner Track, Advanced Track ve Veli Onay Formları.
pub fn home(
    user: &User,
    videos_done: i64,
    videos_total: i64,
    open_tasks: i64,
    points: i64,
    rank: Option<i64>,
    consent_done: usize,
    consent_open: usize,
) -> String {
    let rank_line = match rank {
        Some(r) => format!("{r}. sıradasın"),
        None => "Henüz sıralamada değilsin".into(),
    };
    let consent_alert = if consent_open > consent_done {
        format!(
            r##"<a class="alertbar" href="/documents">{doc}
  <div><b>Veli onay formların eksik ({consent_done}/{consent_open})</b>
  <span>İmzalı formları {deadline} gününden önce yükle.</span></div>
  <span class="alertgo">Yükle →</span>
</a>"##,
            doc = ico(P_DOC),
            deadline = CONSENT_DEADLINE
        )
    } else {
        String::new()
    };
    let content = format!(
        r##"<h1 class="pagetitle">Merhaba {name} 👋</h1>
<p class="muted">Nereden devam etmek istersin?</p>
{consent_alert}
<div class="hubgrid">
  <a class="hubcard" href="/online">
    <span class="hubico">{ico_online}</span>
    <h2>Online</h2>
    <p>Videolar, görev panosu, demolar ve puan tablosu — hepsi burada.</p>
    <span class="hubstat">{videos_done}/{videos_total} video · {open_tasks} açık görev · {points} puan · {rank_line}</span>
    <span class="hubgo">Online'a git →</span>
  </a>
  <a class="hubcard" href="/beginner-track">
    <span class="hubico">{ico_beginner}</span>
    <h2>Beginner Track</h2>
    <p>Başlangıç seviyesindeki öğrenciler için içerikler.</p>
    <span class="hubstat">Yakında</span>
    <span class="hubgo">Beginner Track'a git →</span>
  </a>
  <a class="hubcard" href="/advanced-track">
    <span class="hubico">{ico_advanced}</span>
    <h2>Advanced Track</h2>
    <p>Agentic Harness ve AI Monopoly — ileri seviye yarışmalı bölümler.</p>
    <span class="hubgo">Advanced Track'a git →</span>
  </a>
  <a class="hubcard" href="/documents">
    <span class="hubico">{ico_doc}</span>
    <h2>Veli Onay Formları</h2>
    <p>Veli/yasal temsilcinin imzaladığı formları yükle.</p>
    <span class="hubstat">{consent_done}/{consent_open} form yüklendi</span>
    <span class="hubgo">Formlara git →</span>
  </a>
</div>"##,
        name = esc(u_first_name(user)),
        ico_online = ico(P_GLOBE),
        ico_beginner = ico(P_FLAG),
        ico_advanced = ico(P_ROCKET),
        ico_doc = ico(P_DOC),
    );
    layout("Ana Sayfa", Some(user), "home", &content)
}

/// Online — the four day-to-day student surfaces (videos, tasks, demos, standings)
/// behind one sidebar entry, presented as its own hub the same way Ana Sayfa is.
pub fn online(
    user: &User,
    videos_done: i64,
    videos_total: i64,
    open_tasks: i64,
    points: i64,
    rank: Option<i64>,
) -> String {
    let rank_line = match rank {
        Some(r) => format!("{r}. sıradasın"),
        None => "Henüz sıralamada değilsin".into(),
    };
    let content = format!(
        r##"<h1 class="pagetitle">Online</h1>
<p class="muted">Videolar, görevler, demolar ve puan tablosu — hepsi burada.</p>
<div class="hubgrid">
  <a class="hubcard" href="/videos">
    <span class="hubico">{ico_video}</span>
    <h2>Videolar</h2>
    <p>Ders videolarını izle, kaldığın yerden devam et.</p>
    <span class="hubstat">{videos_done}/{videos_total} video tamamlandı</span>
    <span class="hubgo">Videolara git →</span>
  </a>
  <a class="hubcard" href="/board">
    <span class="hubico">{ico_board}</span>
    <h2>Görev Panosu</h2>
    <p>Projeni yap, GitHub bağlantısını gönder, geri bildirim al.</p>
    <span class="hubstat">{open_tasks} açık görev</span>
    <span class="hubgo">Görev panosuna git →</span>
  </a>
  <a class="hubcard" href="/demos">
    <span class="hubico">{ico_demo}</span>
    <h2>İnteraktif Demolar</h2>
    <p>Derslerde kullanılan interaktif anlatımlar.</p>
    <span class="hubstat">{demo_count} demo</span>
    <span class="hubgo">Demolara git →</span>
  </a>
  <a class="hubcard" href="/leaderboard">
    <span class="hubico">{ico_trophy}</span>
    <h2>Puan Tablosu</h2>
    <p>Her görev ve videodan puan kazanın! Video {PTS_VIDEO}; proje Beginner {PTS_PROJECT_L1}, Intermediate {PTS_PROJECT_L2}, Advanced {PTS_PROJECT_L3}.</p>
    <span class="hubstat">{points} puan · {rank_line}</span>
    <span class="hubgo">Sıralamayı gör →</span>
  </a>
</div>"##,
        ico_video = ico(P_PLAY),
        ico_board = ico(P_BOARD),
        ico_demo = ico(P_DEMO),
        ico_trophy = ico(P_TROPHY),
        demo_count = DEMOS.len(),
    );
    layout("Online", Some(user), "online", &content)
}

/// Advanced Track — the two competitive surfaces, grouped behind one sidebar entry and
/// presented the same hub-card way as Ana Sayfa / Online.
pub fn advanced_track(user: &User) -> String {
    let content = format!(
        r##"<h1 class="pagetitle">Advanced Track</h1>
<p class="muted">İleri seviye yarışmalı bölümler.</p>
<div class="hubgrid">
  <a class="hubcard" href="/agentic-harness">
    <span class="hubico">{ico_harness}</span>
    <h2 lang="en">Agentic Harness</h2>
    <p>Ajanını kur, karşılaştırma setlerinde çalıştır, sıralamada yerini gör.</p>
    <span class="hubgo">Agentic Harness'a git →</span>
  </a>
  <a class="hubcard" href="/ai-monopoly">
    <span class="hubico">{ico_monopoly}</span>
    <h2 lang="en">AI Monopoly</h2>
    <p>Modelini Monopoly masasına oturt, rakiplerine karşı oynat.</p>
    <span class="hubgo">AI Monopoly'ye git →</span>
  </a>
</div>"##,
        ico_harness = ico(P_HARNESS),
        ico_monopoly = ico(P_MONOPOLY),
    );
    layout("Advanced Track", Some(user), "advanced-track", &content)
}

// (key, title, one-line summary, pdf filename in static/beginner-projects/, optional
// extra handout as (button label, pdf filename)). ponytail: hardcoded list, same pattern
// as DEMOS — these are fixed, code-and-deploy content, not something an admin edits day
// to day. Add a row here (and the PDF) for a new project. The extra slot is for a brief
// that ships with its own reference sheet; `None` when the brief stands alone.
pub const BEGINNER_PROJECTS: [(&str, &str, &str, &str, Option<(&str, &str)>); 7] = [
    (
        "kisisel-web-sitesi",
        "Proje 1 — Kişisel Web Sitesi",
        "İlgi alanlarını ve ürettiklerini anlatan, yayında olan kişisel bir web sitesi kur.",
        "01-kisisel-web-sitesi.pdf",
        None,
    ),
    (
        "kisisel-web-sitesi-chatbotu",
        "Proje 2 — Kişisel Web Sitesi Chatbotu",
        "Web siteni, profile.md dosyasından seni tanıtan bir chatbot ile genişlet.",
        "02-kisisel-web-sitesi-chatbotu.pdf",
        None,
    ),
    (
        "ai-bouquet-maker",
        "Proje 3 — AI Bouquet Maker",
        "Annen için kişiselleştirilmiş yapay zekâ çiçek buketleri oluşturan bir uygulama geliştir.",
        "03-ai-bouquet-maker.pdf",
        None,
    ),
    (
        "renovate-your-room",
        "Proje 4 — Renovate Your Room",
        "Oda fotoğrafını yükleyip yapay zekâ ile farklı dekorasyon stillerinde yeniden tasarla.",
        "04-renovate-your-room.pdf",
        None,
    ),
    (
        "character-voice-studio",
        "Proje 5 — Character Voice Studio",
        "Kendi karakterini oluştur, görsel ve sesle hayata geçirip konuştur.",
        "05-character-voice-studio.pdf",
        None,
    ),
    (
        "ai-calorie-tracker",
        "Proje 6 — AI Calorie Tracker",
        "Yemek fotoğrafını yapay zekâ ile analiz edip kalori ve besin değerlerini takip eden bir uygulama geliştir.",
        "06-ai-calorie-tracker.pdf",
        None,
    ),
    (
        "smart-receipt",
        "Proje 7 — Smart Receipt",
        "Fiş fotoğraflarını yapay zekâ ile okuyup harcamaları Google Sheets'e otomatik aktaran bir uygulama geliştir.",
        "07-smart-receipt.pdf",
        Some((
            "Apps Script cheat sheet ⬇",
            "07-google-apps-script-cheat-sheet.pdf",
        )),
    ),
];

/// Beginner Track — the seven fixed projects above, each with a downloadable brief and a
/// save-your-links form. Self-reported, no grading: the form always shows, pre-filled
/// with whatever was last saved, and resaving just overwrites it.
/// The track's own hub: two subsets side by side, same pattern advanced_track()
/// uses for Agentic Harness / AI Monopoly — Chatbot Challenge and the weekly
/// projects are peers, not one floating card above a flat project list.
pub fn beginner_track(user: &User, projects_done: usize, chatbot_level: i16) -> String {
    let content = format!(
        r##"<h1 class="pagetitle">Beginner Track</h1>
<p class="muted">Başlangıç seviyesindeki iki bölüm.</p>
<div class="hubgrid">
  <a class="hubcard" href="/beginner-track/projects">
    <span class="hubico">{ico_projects}</span>
    <h2>Haftalık Projeler</h2>
    <p>7 proje. Her biri için brifi indir, projeni yap, sonra GitHub ve Vercel bağlantılarını kaydet. Kaydedilen: {projects_done}/7.</p>
    <span class="hubgo">Projelere git →</span>
  </a>
  <a class="hubcard" href="/chatbot-challenge">
    <span class="hubico">{ico_chat}</span>
    <h2>Chatbot Challenge</h2>
    <p>Bir chatbotu kandırıp gizli anahtarını söylettirmeye çalış — {CHATBOT_LEVEL_COUNT} seviye, her biri bir öncekinden daha zor. {chat_status}</p>
    <span class="hubgo">Oyuna git →</span>
  </a>
</div>"##,
        ico_projects = ico(P_DOC),
        ico_chat = ico(P_CHAT),
        chat_status = if chatbot_level > CHATBOT_LEVEL_COUNT {
            format!("{CHATBOT_LEVEL_COUNT}/{CHATBOT_LEVEL_COUNT} — tamamlandı 🏆")
        } else {
            format!("Şu an seviye {chatbot_level}/{CHATBOT_LEVEL_COUNT}.")
        },
    );
    layout("Beginner Track", Some(user), "beginner-track", &content)
}

/// The weekly-projects subset: cheat sheet plus the 7 project cards, split out of
/// beginner_track() so that page can stay a clean two-card hub.
pub fn beginner_projects(user: &User, subs: &[BeginnerSubmission]) -> String {
    let cards: String = BEGINNER_PROJECTS
        .iter()
        .map(|(key, title, summary, pdf, extra)| {
            // A project's own reference sheet sits next to its brief, not up with the
            // track-wide cheat sheet — it is only useful once you're on this project.
            let extra_link = match extra {
                Some((label, file)) => format!(
                    r#"<a class="btn-outline small" href="/static/beginner-projects/{file}" target="_blank" rel="noopener">{label}</a>"#,
                    label = esc(label),
                ),
                None => String::new(),
            };
            let saved = subs.iter().find(|s| s.project_key == *key);
            let (repo_val, vercel_val) = saved
                .map(|s| (s.repo_url.clone(), s.vercel_url.clone()))
                .unwrap_or_default();
            let saved_note = if saved.is_some() {
                r#"<p class="fieldnote">Kaydedildi ✓</p>"#
            } else {
                ""
            };
            format!(
                r##"<div class="taskcard">
  <div class="taskhead"><h3>{title}</h3></div>
  <p class="desc">{summary}</p>
  <div class="cardactions">
    <a class="btn-outline small" href="/static/beginner-projects/{pdf}" target="_blank" rel="noopener">Brifi indir ⬇</a>
    {extra_link}
  </div>
  {saved_note}
  <form method="post" action="/beginner-track/submit" class="subform">
    <input type="hidden" name="project_key" value="{key}">
    <input name="repo_url" type="url" placeholder="https://github.com/..." value="{repo_val}" required>
    <input name="vercel_url" type="url" placeholder="https://...vercel.app" value="{vercel_val}" required>
    <button class="btn-dark">Kaydet →</button>
  </form>
</div>"##,
                title = esc(title),
                summary = esc(summary),
                repo_val = esc(&repo_val),
                vercel_val = esc(&vercel_val),
            )
        })
        .collect();
    let content = format!(
        r##"<h1 class="pagetitle">Haftalık Projeler</h1>
<p class="muted">Başlangıç seviyesindeki 7 proje. Her biri için brifi indir, projeni yap, sonra GitHub ve Vercel bağlantılarını kaydet.</p>
<div class="taskcard">
  <div class="taskhead"><h3>Vibe Coding Cheat Sheet</h3></div>
  <p class="desc">Tüm beginner track projelerinde işine yarayacak hızlı referans rehberi.</p>
  <div class="cardactions">
    <a class="btn-outline small" href="/static/beginner-projects/vibe-coding-cheat-sheet.pdf" target="_blank" rel="noopener">Cheat sheet indir ⬇</a>
  </div>
</div>
<div class="tasks">{cards}</div>"##
    );
    layout("Haftalık Projeler", Some(user), "beginner-track", &content)
}

/// Chat page for the student's current level. Level indicator is top-center per
/// spec. Student messages render as .bub.r, the bot's as .bub.l, same convention
/// monopoly_match() uses for its two-sided transcript.
pub fn chatbot_challenge(
    user: &User,
    level: i16,
    level_label: &str,
    msgs: &[ChatbotMessage],
    msg: Option<&str>,
) -> String {
    let notice = match msg {
        Some("bedrock-error") => {
            r#"<p class="notice">Bot şu an cevap veremedi, tekrar dene.</p>"#.to_string()
        }
        _ => String::new(),
    };
    let bubbles: String = msgs
        .iter()
        .map(|m| {
            let side = if m.role == "user" { "r" } else { "l" };
            format!(
                r##"<div class="bub {side}"><div class="say">{content}</div></div>"##,
                content = esc(&m.content),
            )
        })
        .collect();
    let title = format!("Seviye {level} — {level_label}");
    let content = format!(
        r##"<div class="chtopbar">
  <form method="post" action="/chatbot-challenge/reset" class="reset-form">
    <button type="submit" class="ch-reset" title="Bu seviyeyi sıfırla" aria-label="Bu seviyeyi sıfırla" onclick="return confirm('Bu seviyenin konuşmasını sıfırlamak istiyor musun?')">+</button>
  </form>
  <span class="ch-level">Level {level}</span>
</div>
{notice}
<div class="chatpanel">
  <div class="arena-chat" id="chchat">{bubbles}</div>
  <form method="post" action="/chatbot-challenge/send" class="composer" id="chform">
    <textarea name="message" placeholder="Mesajını yaz..." required></textarea>
    <button class="ch-send" id="chsend" aria-label="Gönder">→</button>
  </form>
</div>
<p class="muted" style="text-align:center;"><a href="/chatbot-challenge/leaderboard">Sıralamayı gör →</a></p>
<script>
(function(){{
  var f = document.getElementById('chform');
  var chat = document.getElementById('chchat');
  f.addEventListener('submit', function(){{
    var ta = f.querySelector('textarea'), btn = document.getElementById('chsend');
    if (!ta.value.trim()) return;
    var u = document.createElement('div');
    u.className = 'bub r';
    u.innerHTML = '<div class="say"></div>';
    u.querySelector('.say').textContent = ta.value;
    chat.appendChild(u);
    var b = document.createElement('div');
    b.className = 'bub l typing';
    b.innerHTML = '<div class="say"></div>';
    chat.appendChild(b);
    chat.scrollTop = chat.scrollHeight;
    ta.readOnly = true;
    btn.disabled = true;
  }});
}})();
</script>"##,
    );
    layout(&title, Some(user), "chatbot-challenge", &content)
}

pub fn chatbot_challenge_done(user: &User) -> String {
    let content = format!(
        r##"<section class="panel" style="text-align:center;">
  <h2>🎉 {CHATBOT_LEVEL_COUNT}/{CHATBOT_LEVEL_COUNT} tamamladın!</h2>
  <p class="muted">Tüm seviyeleri geçtin. Sıralamada yerini gör.</p>
  <a class="btn-dark" href="/chatbot-challenge/leaderboard">Sıralamayı gör →</a>
</section>"##
    );
    layout("Chatbot Challenge", Some(user), "chatbot-challenge", &content)
}

pub fn chatbot_challenge_leaderboard(user: &User, rows: &[ChatbotLeaderRow]) -> String {
    let ranks = dense_ranks_by(rows, |r| r.levels_done.to_string());
    let list: String = if rows.is_empty() {
        "<p class='muted'>Henüz kimse seviye tamamlamadı — ilk sen ol.</p>".into()
    } else {
        rows.iter()
            .zip(&ranks)
            .map(|(r, rank)| {
                let crown = if r.finished() { " 🏆" } else { "" };
                format!(
                    r##"<div class="lbrow {mine} {medal}">
  <span class="lbrank">{rank}</span>
  <span class="avatar-fb">{initial}</span>
  <span class="lbname">{name} <small class="nick">({nick})</small>{crown}</span>
  <span class="lbmeta">{levels}/{count} seviye</span>
  <span class="lbpts">{levels}<small>/{count}</small></span>
</div>"##,
                    mine = if r.id == user.id { "mine" } else { "" },
                    medal = match rank {
                        1 => "m1",
                        2 => "m2",
                        3 => "m3",
                        _ => "",
                    },
                    initial = esc(&r
                        .display_name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()),
                    name = esc(&r.display_name),
                    nick = esc(&r.nickname),
                    levels = r.levels_done,
                    count = CHATBOT_LEVEL_COUNT,
                    crown = crown,
                )
            })
            .collect()
    };
    let content = format!(
        r##"<h1 class="pagetitle">Chatbot Challenge — Sıralama</h1>
<p class="muted">Kim daha çok seviye kırdı? İlk {CHATBOT_LEVEL_COUNT}/{CHATBOT_LEVEL_COUNT}'a ulaşan kazanır.</p>
<div class="lb">{list}</div>"##
    );
    layout(
        "Chatbot Challenge Sıralaması",
        Some(user),
        "chatbot-challenge",
        &content,
    )
}

/// Admin view, step 1: the seven projects as a list, each carrying how many students have
/// handed it in. Clicking one opens `admin_beginner_project`.
pub fn admin_beginner_list(
    user: &User,
    counts: &[BeginnerProjectCount],
    student_total: i64,
    gaps: &[&str],
) -> String {
    let cards: String = BEGINNER_PROJECTS
        .iter()
        .map(|(key, title, summary, ..)| {
            let submitted = counts
                .iter()
                .find(|c| c.project_key == *key)
                .map(|c| c.submitted)
                .unwrap_or(0);
            // Percent drives the bar width only. Guard the divide: an empty academy
            // (no students yet) would otherwise be 0/0.
            let pct = if student_total > 0 {
                (submitted * 100 / student_total).clamp(0, 100)
            } else {
                0
            };
            // Zero submissions gets the muted badge — a bright blue "0" reads as a score
            // rather than as "nobody has handed this in".
            let badge = if submitted > 0 { "badge" } else { "badge badge-zero" };
            format!(
                r##"<a class="taskcard ba-card" href="/admin/beginner-track?proje={key}">
  <div class="taskhead"><h3>{title}</h3><span class="{badge}">{submitted}/{student_total}</span></div>
  <p class="desc">{summary}</p>
  <div class="ba-bar"><span style="width:{pct}%"></span></div>
  <span class="ba-go">Gönderimleri gör →</span>
</a>"##,
                title = esc(title),
                summary = esc(summary),
            )
        })
        .collect();
    layout(
        "Beginner Track Gönderimleri",
        Some(user),
        "beginner-admin",
        &format!(
            r##"<h1 class="pagetitle">Beginner Track Gönderimleri</h1>
<p class="muted">Bir projeye tıkla, o projeyi gönderen öğrencilerin GitHub ve Vercel bağlantılarını gör.</p>
{warn}
<div class="tasks">{cards}</div>"##,
            warn = roster_gap_note(gaps),
        ),
    )
}

/// Names on `BEGINNER_ROSTER` that match no account. Rendered on both admin views so a
/// misspelled roster line (or a student who never signed up) is visible instead of just
/// being an absent row. Empty list renders nothing.
fn roster_gap_note(gaps: &[&str]) -> String {
    if gaps.is_empty() {
        return String::new();
    }
    format!(
        r##"<p class="ba-warn">⚠ Listede olup portalda hesabı bulunmayan {n} isim: {names}. Kayıt olmamış olabilirler ya da adları portalda farklı yazılmış olabilir.</p>"##,
        n = gaps.len(),
        names = esc(&gaps.join(", ")),
    )
}

/// Admin view, step 2: one project, every student, their two links. Students with no
/// submission sort last and show an em dash — the page is also the "who is behind" list.
pub fn admin_beginner_project(
    user: &User,
    key: &str,
    rows: &[BeginnerStudentRow],
    gaps: &[&str],
) -> String {
    let (_, title, summary, ..) = BEGINNER_PROJECTS
        .iter()
        .find(|(k, ..)| *k == key)
        .copied()
        .unwrap_or((key, key, "", "", None));
    let submitted = rows.iter().filter(|r| r.repo_url.is_some()).count();
    // Long URLs would push the table past the panel, so each cell shows a short label and
    // carries the full URL in the title attribute. href is the raw (escaped) student URL:
    // beginner_track_submit already required an https://github.com/ prefix on the repo and
    // an http(s) scheme on the live one, so neither can be a javascript: payload here.
    let link_cell = |url: &Option<String>, label: &str| match url {
        Some(u) if !u.is_empty() => format!(
            r#"<a class="ba-link" href="{href}" target="_blank" rel="noopener" title="{href}">{label} ↗</a>"#,
            href = esc(u),
        ),
        _ => r#"<span class="ba-missing">—</span>"#.to_string(),
    };
    let body: String = rows
        .iter()
        .map(|r| {
            format!(
                r##"<tr class="{row_class}">
  <td>{name}</td>
  <td>{repo}</td>
  <td>{vercel}</td>
  <td class="ba-when">{when}</td>
</tr>"##,
                row_class = if r.repo_url.is_some() { "" } else { "ba-empty" },
                name = esc(&r.display_name),
                repo = link_cell(&r.repo_url, "GitHub"),
                vercel = link_cell(&r.vercel_url, "Vercel"),
                when = r
                    .updated_at
                    .map(|t| t.format("%d.%m.%Y %H:%M").to_string())
                    .unwrap_or_else(|| "—".into()),
            )
        })
        .collect();
    layout(
        "Beginner Track Gönderimleri",
        Some(user),
        "beginner-admin",
        &format!(
            r##"<p class="fieldnote"><a href="/admin/beginner-track">← Tüm projeler</a></p>
<h1 class="pagetitle">{title}</h1>
<p class="muted">{summary}</p>
{warn}
<div class="panel wide">
  <div class="panel-head"><h2>Öğrenciler</h2><span class="item-meta">{submitted}/{total} gönderdi</span></div>
  <table>
    <thead><tr><th>Öğrenci</th><th>GitHub</th><th>Vercel</th><th>Güncelleme</th></tr></thead>
    <tbody>{body}</tbody>
  </table>
</div>"##,
            title = esc(title),
            summary = esc(summary),
            total = rows.len(),
            warn = roster_gap_note(gaps),
        ),
    )
}

/// Kartta tam ad yerine yalnızca ilk isim — selamlama kısa kalsın.
fn u_first_name(user: &User) -> &str {
    user.label().split_whitespace().next().unwrap_or("")
}

/// Haftalık Program. The schedule itself is kept in a spreadsheet outside the portal;
/// what students see is the image of it the admin uploads on /admin, one per track.
/// The portal stores and shows that image and nothing else — it never parses it, so
/// the layout of the schedule is entirely whatever the screenshot looks like.
///
/// `img` is the metadata for the selected track, `None` when nothing is uploaded yet.
/// `venues` ride along at the bottom: "where is this happening" is the other half of
/// the question the schedule answers, and they are the same cards as /location.
pub fn schedule(user: &User, track: &str, img: Option<&ScheduleImage>, venues: &[Venue]) -> String {
    let chips: String = SCHEDULE_TRACKS
        .iter()
        .map(|(k, label)| {
            format!(
                r#"<a class="chip {active}" href="/schedule?track={k}" lang="en">{label}</a>"#,
                active = if *k == track { "active" } else { "" },
            )
        })
        .collect();

    let body = match img {
        // Click-through to the raw image: a screenshot of a full week is wider than the
        // column it renders in, and this is the "zoom in" students will reach for.
        Some(i) => format!(
            r##"<a class="sheet-shot" href="/schedule/image/{track}?v={v}" target="_blank" rel="noopener"
   title="Tam boyutta aç">
  <img src="/schedule/image/{track}?v={v}" alt="{label} haftalık program">
</a>
<p class="fieldnote sheet-note">Büyütmek için görsele tıkla · Son güncelleme {when}</p>"##,
            v = i.version(),
            label = esc(schedule_track_name(track)),
            when = i.uploaded_at.format("%d.%m.%Y %H:%M"),
        ),
        None => format!(
            r#"<p class="sheet-empty">{label} grubunun programı henüz yüklenmedi.</p>"#,
            label = esc(schedule_track_name(track)),
        ),
    };

    // silent when no address is on file at all — an empty-state box here would just be
    // noise next to the schedule, unlike on /location where it is the whole page
    let venue_block = match venue_cards(venues).as_str() {
        "" => String::new(),
        cards => format!(
            r#"<h2 class="venue-head">Konum</h2>
<p class="fieldnote venue-sub">İki hafta iki ayrı yerde.</p>
<div class="venue-wrap">{cards}</div>"#
        ),
    };

    let content = format!(
        r##"<h1 class="pagetitle">Haftalık Program</h1>
<p class="muted">Bu haftanın akışı.</p>
<div class="chips">{chips}</div>
{body}
{venue_block}"##
    );
    layout("Haftalık Program", Some(user), "schedule", &content)
}

/// The address card for one week. One renderer, shown on its own page (/location) and
/// again under the schedule, so the two can never disagree about where the academy is.
/// Returns "" when nothing is filled in, letting each caller decide what to show
/// in its place.
///
/// The week is always in the card's own eyebrow — the two weeks are in different
/// places, so an address that doesn't say which week it is for is worse than useless.
fn venue_card(v: &Venue) -> String {
    if v.is_empty() {
        return String::new();
    }
    let week = format!(r#"<span class="venue-week">{}</span>"#, esc(&v.heading()));
    let name = if v.name.trim().is_empty() {
        String::new()
    } else {
        format!("<h3>{}</h3>", esc(v.name.trim()))
    };
    let address = if v.address.trim().is_empty() {
        String::new()
    } else {
        format!(r#"<p class="venue-address">{}</p>"#, esc(v.address.trim()))
    };
    // maps_url is validated http(s) on save (admin_venue), so it can go in an href
    let maps = if v.maps_url.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<a class="btn-dark small venue-maps" href="{}" target="_blank" rel="noopener">Google Haritalar'da aç →</a>"#,
            esc(v.maps_url.trim())
        )
    };
    let notes = if v.notes.trim().is_empty() {
        String::new()
    } else {
        format!(r#"<p class="venue-notes">{}</p>"#, esc(v.notes.trim()))
    };
    format!(
        r##"<section class="panel venue-card">
  <span class="venue-pin">{pin}</span>
  {week}
  {name}
  {address}
  {notes}
  {maps}
</section>"##,
        pin = ico(P_PIN),
    )
}

/// Every week that has an address, in order. "" when none of them do.
fn venue_cards(venues: &[Venue]) -> String {
    venues.iter().map(venue_card).collect()
}

/// Konum. One card per week, because the two weeks are in different buildings.
/// A week with nothing filled in says so by name rather than silently vanishing —
/// "2. Hafta" missing entirely would read as "there is no second week".
pub fn location(user: &User, venues: &[Venue]) -> String {
    let body: String = venues
        .iter()
        .map(|v| {
            if v.is_empty() {
                format!(
                    r#"<div class="venue-pending"><span class="venue-week">{}</span>
<p>Adres henüz açıklanmadı.</p></div>"#,
                    esc(&v.heading()),
                )
            } else {
                venue_card(v)
            }
        })
        .collect();
    let content = format!(
        r##"<h1 class="pagetitle">Konum</h1>
<p class="muted">İki hafta iki ayrı yerde geçiyor — hangi hafta nerede, aşağıda.</p>
<div class="venue-wrap">{body}</div>"##
    );
    layout("Konum", Some(user), "location", &content)
}

// ---- Veli onay formları ----

/// What the upload control says it takes. The server re-checks the bytes themselves
/// (`sniff_document`), so this is a filter for the file picker, not a security boundary.
const CONSENT_ACCEPT: &str =
    ".pdf,.jpg,.jpeg,.png,.heic,.heif,.webp,.gif,.doc,.docx,.odt,application/pdf,image/*";
/// Human list of the same thing, for the line under the dropzone.
const CONSENT_FORMATS: &str = "PDF, JPG, PNG, HEIC, WebP veya Word (DOC/DOCX)";

/// One student's file list for one form: what they've uploaded, each downloadable back
/// (so they can check they sent the right page) and removable while the form is open.
fn consent_file_list(docs: &[ConsentDoc], kind: &str, locked: bool) -> String {
    let files: Vec<&ConsentDoc> = docs.iter().filter(|d| d.kind == kind).collect();
    if files.is_empty() {
        return r#"<p class="fieldnote doc-none">Henüz bir dosya yüklemedin.</p>"#.to_string();
    }
    let rows: String = files
        .iter()
        .map(|d| {
            format!(
                r##"<li class="doc-file">
  <a href="/documents/file/{id}">{name}</a>
  <span class="doc-meta">{size} · {when}</span>
  {remove}
</li>"##,
                id = d.id,
                name = esc(&d.filename),
                size = d.size_label(),
                when = d.uploaded_at.format("%d.%m.%Y %H:%M"),
                remove = if locked {
                    String::new()
                } else {
                    format!(
                        r##"<form method="post" action="/documents/delete" class="inline"
    onsubmit="return confirm('{name} silinsin mi?')">
  <input type="hidden" name="id" value="{id}">
  <button class="doc-del" title="Sil" aria-label="Sil">{trash}</button>
</form>"##,
                        id = d.id,
                        name = esc(&d.filename),
                        trash = ico(P_TRASH)
                    )
                },
            )
        })
        .collect();
    format!(r#"<ul class="doc-files">{rows}</ul>"#)
}

/// Veli Onay Formları. One card per form in CONSENT_DOCS. A form whose document isn't
/// ready to hand out yet (Paribu, at the time of writing) is rendered blurred behind a
/// "yakında" overlay instead of being hidden: students should know it is coming, and
/// know they don't have to do anything about it yet. The admin opens it from /admin,
/// and the same lock is enforced server-side on upload.
pub fn documents(
    user: &User,
    docs: &[ConsentDoc],
    locks: &[(&str, bool)],
    urls: &[(&str, String)],
    error: Option<&str>,
    notice: Option<&str>,
) -> String {
    let banner = error
        .map(|e| format!(r#"<p class="error portal-error">{}</p>"#, esc(e)))
        .or_else(|| notice.map(|m| format!(r#"<p class="notice portal-notice">{}</p>"#, esc(m))))
        .unwrap_or_default();

    let cards: String = CONSENT_DOCS
        .iter()
        .map(|(kind, title, note, _)| {
            let locked = locks
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, l)| *l)
                .unwrap_or(false);
            let count = docs.iter().filter(|d| d.kind == *kind).count();
            let (badge, badge_cls) = if locked {
                ("Yakında", "doc-st-soon")
            } else if count > 0 {
                ("Yüklendi", "doc-st-done")
            } else {
                ("Bekleniyor", "doc-st-wait")
            };

            // The blank form to print and sign. Two ways in, because one of them fails on
            // somebody's phone every time: the button downloads the file directly, the link
            // beside it opens the document in the tab (Drive's preview for the Drive-hosted
            // forms, the browser's PDF viewer for the ones we serve ourselves). Both are
            // validated on save — http(s), or a same-origin /static path.
            let url = urls
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, u)| u.trim())
                .unwrap_or("");
            let get_form = if url.is_empty() || locked {
                String::new()
            } else {
                format!(
                    r##"<div class="doc-get">
    <a class="btn-outline doc-getbtn" href="{download}"{dl} target="_blank" rel="noopener">{down} Formu indir</a>
    <a class="doc-getalt" href="{view}" target="_blank" rel="noopener">tarayıcıda aç ↗</a>
  </div>"##,
                    download = esc(&direct_download_url(url)),
                    view = esc(url),
                    down = ico(P_DOWNLOAD),
                    // `download` only counts same-origin — the browser ignores it cross-origin,
                    // which is why the Drive links go through direct_download_url instead.
                    dl = if same_origin_path(url) { " download" } else { "" },
                )
            };

            // Locked: everything below the heading is inert and blurred, with the overlay
            // explaining why. No <input> is rendered at all — there is nothing to click.
            let body = if locked {
                format!(
                    r##"<div class="doc-locked">
    <div class="doc-blur" aria-hidden="true">
      <p class="fieldnote doc-none">Henüz bir dosya yüklemedin.</p>
      <div class="dropzone doc-fake">{up}<b>Dosyalarını sürükle veya seç</b><span>{formats}</span></div>
      <div class="doc-fake-btn">Yükle →</div>
    </div>
    <div class="doc-lockmsg">{lock}<b>Bu form henüz hazır değil</b>
      <span>Hazır olduğunda burada açılacak ve WhatsApp grubundan haber verilecek.</span></div>
  </div>"##,
                    up = ico(P_UPLOAD),
                    lock = ico(P_LOCK),
                    formats = CONSENT_FORMATS
                )
            } else {
                format!(
                    r##"{files}
  <form method="post" action="/documents/upload" enctype="multipart/form-data" class="doc-form">
    <input type="hidden" name="kind" value="{kind}">
    <label class="dropzone">
      <input name="files" type="file" accept="{accept}" multiple required
        onchange="var z=this.closest('.dropzone');z.classList.toggle('has-file',this.files.length>0);z.querySelector('b').textContent=this.files.length?(this.files.length===1?this.files[0].name:this.files.length+' dosya seçildi'):'Dosyalarını sürükle veya seç'"
        ondragenter="this.closest('.dropzone').classList.add('drag')"
        ondragleave="this.closest('.dropzone').classList.remove('drag')"
        ondrop="this.closest('.dropzone').classList.remove('drag')">
      {up}
      <b>Dosyalarını sürükle veya seç</b>
      <span>{formats} · birden fazla sayfa seçebilirsin</span>
    </label>
    <button class="btn-dark">{verb}</button>
  </form>"##,
                    files = consent_file_list(docs, kind, locked),
                    accept = CONSENT_ACCEPT,
                    up = ico(P_UPLOAD),
                    formats = CONSENT_FORMATS,
                    verb = if count > 0 {
                        "Dosya ekle →"
                    } else {
                        "Yükle →"
                    }
                )
            };

            format!(
                r##"<section class="panel doccard {done}">
  <div class="dochead">
    <span class="docico">{doc}</span>
    <h3>{title}</h3>
    <span class="badge {badge_cls}">{badge}</span>
  </div>
  <p class="desc">{note}</p>
  {get_form}
  {body}
</section>"##,
                done = if count > 0 && !locked { "has-docs" } else { "" },
                doc = ico(P_DOC),
                title = esc(title),
                note = esc(note),
            )
        })
        .collect();

    let content = format!(
        r##"<h1 class="pagetitle">Veli Onay Formları ve Sözleşmeler</h1>
<p class="muted">Yaşınız 18'den küçük olduğu için programa katılım bazı formların
veli/yasal temsilciniz tarafından onaylanmasını gerektiriyor. Aşağıdaki belgeleri indirin,
her birinin üzerinde yazan tarafa — <b>katılımcı</b> belgelerini kendiniz, <b>veli/vasi</b>
belgelerini veli/yasal temsilcinize — imzalatıp buraya yükleyin.</p>
{banner}
<div class="doc-deadline">{cal}<div><b>Son tarih: {deadline}</b>
<span>Exposure AI Academy ve QNBEYOND formlarının bu tarihten önce yüklenmiş olması gerekiyor.
Paribu belgelerinin dördü de programın 2. haftası başlamadan önce yüklenmiş olmalı.</span></div></div>
<p class="fieldnote doc-howto">Belgeleri imzaladıktan sonra tarayarak ya da <b>tüm sayfaları net
görünecek şekilde</b> fotoğraflayarak yükleyebilirsiniz. İmzanın, tarihin ve tüm sayfaların
okunaklı olduğundan emin olun. Bir formun sayfalarını tek tek yükleyebilirsiniz — hepsi bir
arada saklanır. Belgelerinizi yalnızca siz ve akademi ekibi görebilir.</p>
<div class="doccards">{cards}</div>"##,
        cal = ico(P_CAL),
        deadline = CONSENT_DEADLINE,
    );
    layout("Veli Onay Formları", Some(user), "documents", &content)
}

/// The admin side of the consent forms: a download-everything button, a per-form
/// open/close switch, and a student × form grid of what has actually arrived.
/// Every file is a direct download link, and the whole set is one ZIP.
fn admin_consent_panel(
    members: &[MemberRow],
    docs: &[ConsentDoc],
    locks: &[(&str, bool)],
    urls: &[(&str, String)],
) -> String {
    // students only: admins hand in nothing, and an admin row in the grid is noise
    let students: Vec<&MemberRow> = members.iter().filter(|m| !m.is_admin).collect();

    let switches: String = CONSENT_DOCS
        .iter()
        .map(|(kind, title, _, _)| {
            let locked = locks
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, l)| *l)
                .unwrap_or(false);
            let url = urls
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, u)| u.as_str())
                .unwrap_or("");
            let have = students
                .iter()
                .filter(|m| docs.iter().any(|d| d.user_id == m.id && d.kind == *kind))
                .count();
            format!(
                r##"<div class="consent-switch">
  <div class="consent-switch-id">
    <b>{title}</b>
    <span class="item-meta">{have}/{total} öğrenci yükledi · {state}</span>
  </div>
  <form method="post" action="/admin/documents/link" class="inline consent-urlform">
    <input type="hidden" name="kind" value="{kind}">
    <input name="url" type="text" value="{url}" placeholder="Boş formun bağlantısı — https://… ya da /static/…">
    <button class="btn-dark small">Kaydet</button>
  </form>
  <form method="post" action="/admin/documents/lock" class="inline">
    <input type="hidden" name="kind" value="{kind}">
    <input type="hidden" name="locked" value="{next}">
    <button class="btn-outline small">{action}</button>
  </form>
</div>"##,
                title = esc(title),
                total = students.len(),
                url = esc(url),
                state = if locked {
                    "kapalı (öğrencilere bulanık görünüyor)"
                } else {
                    "açık"
                },
                next = if locked { "false" } else { "true" },
                action = if locked {
                    "Yüklemeye aç"
                } else {
                    "Yüklemeyi kapat"
                },
            )
        })
        .collect();

    let head: String = CONSENT_DOCS
        .iter()
        .map(|(_, title, _, _)| format!("<th>{}</th>", esc(title)))
        .collect();
    let rows: String = if students.is_empty() {
        // name + e-mail + one column per form
        format!(
            r#"<tr><td colspan="{}" class="muted">Henüz öğrenci yok</td></tr>"#,
            CONSENT_DOCS.len() + 2
        )
    } else {
        students
            .iter()
            .map(|m| {
                let cells: String = CONSENT_DOCS
                    .iter()
                    .map(|(kind, ..)| {
                        let files: Vec<&ConsentDoc> = docs
                            .iter()
                            .filter(|d| d.user_id == m.id && d.kind == *kind)
                            .collect();
                        if files.is_empty() {
                            return r#"<td class="consent-missing">—</td>"#.to_string();
                        }
                        let links: String = files
                            .iter()
                            .enumerate()
                            .map(|(i, d)| {
                                format!(
                                    r#"<a href="/documents/file/{id}" title="{name} · {size}">{n}</a>"#,
                                    id = d.id,
                                    name = esc(&d.filename),
                                    size = d.size_label(),
                                    n = i + 1,
                                )
                            })
                            .collect();
                        format!(
                            r#"<td class="consent-have"><span class="consent-files">{links}</span>
<span class="item-meta">{n} dosya · {when}</span></td>"#,
                            n = files.len(),
                            when = files
                                .iter()
                                .map(|d| d.uploaded_at)
                                .max()
                                .map(|t| t.format("%d.%m.%Y").to_string())
                                .unwrap_or_default()
                        )
                    })
                    .collect();
                format!(
                    "<tr><td>{name}</td><td>{email}</td>{cells}</tr>",
                    name = esc(&m.display_name),
                    email = esc(&m.email)
                )
            })
            .collect()
    };

    format!(
        r##"<section class="panel wide">
  <div class="panel-head">
    <h2>Veli onay formları</h2>
    <a class="btn-dark small" href="/admin/documents.zip">⬇ Tüm belgeler (.zip)</a>
  </div>
  <p class="muted">Öğrenciler bunları <b>Veli Onay Formları</b> sayfasından yükler. Son tarih:
  <b>{deadline}</b>. ZIP her form için ayrı klasör, içinde öğrenci başına bir klasör açar ve
  kökündeki <b>_EKSIKLER.txt</b> kimin yüklemediğini listeler. Kapattığın form öğrencilere
  bulanık görünür ve yükleme kabul etmez.</p>
  <div class="consent-switches">{switches}</div>
  <table class="consent-table"><tr><th>Öğrenci</th><th>E-posta</th>{head}</tr>{rows}</table>
</section>"##,
        deadline = CONSENT_DEADLINE,
    )
}

/// Upload ceiling for a schedule screenshot, in MB. Stated on the form and enforced
/// by the route's body limit in main.rs, which reads this same number.
pub const SCHEDULE_IMAGE_MAX_MB: usize = 8;

/// The Haftalık Program panel: one upload slot per track. Uploading replaces whatever
/// that track had — there is no history, because the only thing students should ever
/// see is the current week.
fn admin_schedule_panel(images: &[ScheduleImage]) -> String {
    let slots: String = SCHEDULE_TRACKS
        .iter()
        .map(|(key, label)| {
            let current = images.iter().find(|i| i.track == *key);
            let state = match current {
                Some(i) => format!(
                    r##"<a class="sched-thumb" href="/schedule/image/{key}?v={v}" target="_blank" rel="noopener">
    <img src="/schedule/image/{key}?v={v}" alt=""></a>
  <p class="fieldnote">{when} · {kb} KB · {ct}</p>
  <form method="post" action="/admin/schedule/delete" onsubmit="return confirm('{label} programı kaldırılsın mı?')">
    <input type="hidden" name="track" value="{key}">
    <button class="btn-outline">Kaldır</button>
  </form>"##,
                    v = i.version(),
                    when = i.uploaded_at.format("%d.%m.%Y %H:%M"),
                    kb = i.bytes / 1024,
                    ct = esc(&i.content_type),
                ),
                None => r#"<p class="fieldnote">Henüz yüklenmedi.</p>"#.to_string(),
            };
            format!(
                r##"<div class="sched-slot">
  <h3>{label}</h3>
  {state}
  <form method="post" action="/admin/schedule" enctype="multipart/form-data">
    <input type="hidden" name="track" value="{key}">
    <label class="dropzone">
      <input name="image" type="file" accept="image/png,image/jpeg,image/webp,image/gif" required
        onchange="var z=this.closest('.dropzone');z.classList.toggle('has-file',this.files.length>0);z.querySelector('b').textContent=this.files.length?this.files[0].name:'Ekran görüntüsünü sürükle veya seç'">
      {up}
      <b>Ekran görüntüsünü sürükle veya seç</b>
      <span>PNG, JPEG, WebP veya GIF · en fazla {max} MB</span>
    </label>
    <button class="btn-dark">{verb}</button>
  </form>
</div>"##,
                up = ico(P_UPLOAD),
                max = SCHEDULE_IMAGE_MAX_MB,
                verb = if current.is_some() {
                    "Değiştir"
                } else {
                    "Yükle"
                },
            )
        })
        .collect();

    format!(
        r##"<section class="panel wide">
  <h2>Haftalık program</h2>
  <p class="muted">Programın ekran görüntüsünü yükle — öğrenciler <b>Haftalık Program</b>
  sayfasında bunu görür. Yeni bir görsel yüklemek eskisinin yerine geçer.</p>
  <div class="sched-slots">{slots}</div>
</section>"##
    )
}

/// The Konum panel — one form per week, saved independently, since the two weeks are
/// in different places and are usually confirmed at different times. Every field is
/// optional and free text; filling in only a Maps link, or only a note, is a
/// legitimate way to use it. Blanking a week's fields takes its card off both pages.
fn admin_venue_panel(venues: &[Venue]) -> String {
    let slots: String = venues
        .iter()
        .map(|v| {
            format!(
                r##"<div class="venue-slot">
  <h3>{week}. Hafta</h3>
  <form method="post" action="/admin/venue">
    <input type="hidden" name="week" value="{week}">
    <label>Tarihler<input name="dates" value="{dates}" placeholder="3–7 Ağustos"></label>
    <label>Mekan adı<input name="name" value="{name}" placeholder="Kolektif House Levent"></label>
    <label>Adres<textarea name="address" rows="3" placeholder="Esentepe Mah. ... Şişli/İstanbul">{address}</textarea></label>
    <label>Google Haritalar bağlantısı<input name="maps_url" type="url" value="{maps_url}" placeholder="https://maps.app.goo.gl/...">
      <span class="fieldnote">Haritalar'da mekânı aç → Paylaş → Bağlantıyı kopyala.</span></label>
    <label>Diğer detaylar<textarea name="notes" rows="4" placeholder="Kat, kapı kodu, ulaşım, otopark…">{notes}</textarea></label>
    <button class="btn-dark">{week}. haftayı kaydet</button>
  </form>
</div>"##,
                week = v.week,
                dates = esc(&v.dates),
                name = esc(&v.name),
                address = esc(&v.address),
                maps_url = esc(&v.maps_url),
                notes = esc(&v.notes),
            )
        })
        .collect();

    format!(
        r##"<section class="panel wide">
  <h2>Konum / adres</h2>
  <p class="muted">İki hafta iki ayrı yerde geçtiği için her hafta ayrı ayrı girilir.
  Öğrenciler bunları <b>Konum</b> sayfasında ve <b>Haftalık Program</b>'ın altında görür.
  Boş bıraktığın alanlar gösterilmez.</p>
  <div class="venue-slots">{slots}</div>
</section>"##
    )
}

pub fn video_grid(user: &User, videos: &[VideoWithProgress], level: Option<&str>) -> String {
    let chips: String = std::iter::once((None::<&str>, "Hepsi"))
        .chain(LEVELS.iter().map(|(k, v)| (Some(*k), *v)))
        .map(|(k, label)| {
            let href = k
                .map(|k| format!("/videos?level={k}"))
                .unwrap_or_else(|| "/videos".into());
            let active = if level == k { "active" } else { "" };
            format!(r#"<a class="chip {active}" href="{href}">{label}</a>"#)
        })
        .collect();
    let cards: String = if videos.is_empty() {
        "<p class='muted'>Henüz video yok</p>".into()
    } else {
        videos
            .iter()
            .map(|v| {
                let pct = if v.duration > 0.0 {
                    (v.max_position / v.duration * 100.0).min(100.0)
                } else {
                    0.0
                };
                let done = pct >= 90.0;
                let meta = if done {
                    "Tamamlanmış".into()
                } else if pct > 0.0 {
                    format!("%{:.0} izlendi", pct)
                } else {
                    "Henüz başlamadı".into()
                };
                format!(
                    r##"<a class="vcard {done_class}" href="/watch/{id}">
  <div class="thumb"><img src="https://i.ytimg.com/vi/{yt}/hqdefault.jpg" alt="">
    <div class="progress"><i style="width:{pct:.0}%"></i></div>
  </div>
  <h3>{title}</h3>
  <p class="meta">{level} · {meta}</p>
</a>"##,
                    done_class = if done { "done" } else { "" },
                    id = v.id,
                    yt = esc(&v.youtube_id),
                    title = esc(&v.title),
                    level = VIDEO_LEVEL_LABEL,
                )
            })
            .collect()
    };
    // Advanced filtresinde video yok; kılavuz mesajı grid yerine büyük ve ortada.
    let body = if level == Some("SERIES_A") {
        r#"<div class="advanced-note"><p>Advanced seviye arkadaşlar büyük olasılıkla video içeriklerine hakimler. Sizler doğrudan görev projeleri yapmaya başlayabilirsiniz!</p><a class="btn-start" href="/board">Görev Panosu →</a></div>"#.to_string()
    } else {
        format!(r#"<div class="grid">{cards}</div>"#)
    };
    // seviye filtresi açıkken de nav'da Videolar seçili kalsın
    layout(
        "Videolar",
        Some(user),
        "videos",
        &format!(
            r##"<h1 class="pagetitle">Videolar</h1>
<p class="muted">Ders videoları. Bir videoyu %90'ına kadar izlediğinde tamamlanmış sayılır.</p>
<div class="chips">{chips}</div>{body}"##
        ),
    )
}

pub fn watch(user: &User, video: &Video, playlist: &[VideoWithProgress], resume_at: f64) -> String {
    let list: String = playlist
        .iter()
        .map(|v| {
            let pct = if v.duration > 0.0 {
                (v.max_position / v.duration * 100.0).min(100.0)
            } else {
                0.0
            };
            let cur = if v.id == video.id { "current" } else { "" };
            format!(
                r##"<a class="plitem {cur}" href="/watch/{id}">
  <div class="plthumb"><img src="https://i.ytimg.com/vi/{yt}/mqdefault.jpg" alt="">
    <div class="progress"><i style="width:{pct:.0}%"></i></div>
  </div>
  <span>{title}</span>
</a>"##,
                id = v.id,
                yt = esc(&v.youtube_id),
                title = esc(&v.title),
            )
        })
        .collect();
    let content = format!(
        r##"<div class="watchwrap">
  <div class="playercol">
    <div class="playerbox"><div id="player"></div></div>
    <h1 class="vtitle">{title}</h1>
    <p class="meta">{level}</p>
  </div>
  <div class="playlist"><p class="head">{level} · Tüm dersler</p>{list}</div>
</div>
<script>
const VIDEO_ID = "{id}", YT_ID = "{yt}", RESUME_AT = {resume_at};
</script>
<script src="/static/tracker.js"></script>
<script src="https://www.youtube.com/iframe_api"></script>"##,
        title = esc(&video.title),
        level = VIDEO_LEVEL_LABEL,
        id = video.id,
        yt = esc(&video.youtube_id),
    );
    layout(&video.title, Some(user), &video.level, &content)
}

/// Dense ranking over an already-sorted standings list: equal points share a place.
pub fn dense_ranks(rows: &[LeaderRow]) -> Vec<i64> {
    let mut ranks: Vec<i64> = Vec::with_capacity(rows.len());
    let mut place = 0i64;
    let mut prev: Option<i64> = None;
    for r in rows {
        if prev != Some(r.points()) {
            place += 1;
            prev = Some(r.points());
        }
        ranks.push(place);
    }
    ranks
}

pub fn leaderboard(user: &User, rows: &[LeaderRow]) -> String {
    let ranks = dense_ranks(rows);

    let me = rows.iter().position(|r| r.id == user.id);
    let my_card = match me {
        Some(i) => {
            let r = &rows[i];
            let name = r.display_name.clone();
            format!(
                r##"<section class="panel mecard">
  <div class="me-rank">#{rank}</div>
  <span class="avatar-fb big">{initial}</span>
  <div class="me-id"><h3>{name} <small class="nick">({nick})</small></h3><p class="meta">Senin sıran</p></div>
  <div class="me-stats">
    <div><b>{videos}</b><span>video · {vpts}p</span></div>
    <div><b>{projects}</b><span>proje · {ppts}p</span></div>
    <div class="me-total"><b>{total}</b><span>toplam puan</span></div>
  </div>
</section>"##,
                rank = ranks[i],
                initial = esc(&name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string()),
                name = esc(&name),
                nick = esc(&r.nickname),
                videos = r.videos,
                vpts = r.videos * PTS_VIDEO,
                projects = r.projects,
                ppts = r.project_points,
                total = r.points(),
            )
        }
        None => String::new(),
    };

    let list: String = if rows.is_empty() {
        "<p class='muted'>Henüz kimse puan toplamadı — ilk sen ol.</p>".into()
    } else {
        rows.iter()
            .zip(&ranks)
            .map(|(r, rank)| {
                let name = r.display_name.clone();
                format!(
                    r##"<div class="lbrow {mine} {medal}">
  <span class="lbrank">{rank}</span>
  <span class="avatar-fb">{initial}</span>
  <span class="lbname">{name} <small class="nick">({nick})</small></span>
  <span class="lbmeta">{videos} video · {projects} proje</span>
  <span class="lbpts">{total}<small>p</small></span>
</div>"##,
                    mine = if r.id == user.id { "mine" } else { "" },
                    medal = match rank {
                        1 => "m1",
                        2 => "m2",
                        3 => "m3",
                        _ => "",
                    },
                    initial = esc(&name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()),
                    name = esc(&name),
                    nick = esc(&r.nickname),
                    videos = r.videos,
                    projects = r.projects,
                    total = r.points(),
                )
            })
            .collect()
    };

    layout(
        "Puan Tablosu",
        Some(user),
        "leaderboard",
        &format!(
            r##"<h1 class="pagetitle">Puan Tablosu</h1>
<p class="muted">Her görev ve videodan puan kazanın! Video <b>{PTS_VIDEO}</b>; proje Beginner <b>{PTS_PROJECT_L1}</b>,
Intermediate <b>{PTS_PROJECT_L2}</b>, Advanced <b>{PTS_PROJECT_L3}</b>.
</p>
{my_card}
<div class="lb">{list}</div>
<p class="lbnote">Bir video, %90'ını izlediğinde tamamlanmış sayılır. Proje puanı, gönderimin durumu
<b>Geçti</b> olduğunda eklenir — aynı görev birden fazla kez puan getirmez.</p>"##
        ),
    )
}

/// The scaled thumbnail shared by the task cards and the site gallery: a live iframe when
/// the site allows framing, otherwise the cached screenshot at `img_src`. The preview
/// itself is the link — the iframe/img is pointer-events:none in CSS, so the click falls
/// through to the <a> and the site opens in a new tab.
///
/// Callers must scheme-gate `url` first: esc() alone doesn't stop a `javascript:` href.
fn preview_link(url: &str, embeddable: bool, img_src: &str, alt: &str) -> String {
    let inner = if embeddable {
        // sandbox without allow-same-origin would break most sites' own scripts; these are
        // cross-origin frames, so the framed page gets its own origin either way and never ours
        format!(
            r##"<iframe src="{url}" loading="lazy" sandbox="allow-scripts allow-same-origin" tabindex="-1" title="{alt}"></iframe>"##,
            url = esc(url)
        )
    } else {
        format!(r##"<img src="{img_src}" loading="lazy" alt="{alt}">"##)
    };
    format!(
        r##"<a class="example-preview" href="{url}" target="_blank" rel="noopener" title="{alt}">{inner}</a>"##,
        url = esc(url),
    )
}

pub fn board(
    user: &User,
    tasks: &[Task],
    subs: &[SubmissionView],
    interests: &[InterestRow],
    site_counts: &[(Uuid, i64)],
) -> String {
    let status_tr = |s: &str| match s {
        "pending" => ("İnceleme bekleniyor", "st-pending"),
        "reviewing" => ("İnceleniyor", "st-reviewing"),
        "passed" => ("Geçti", "st-passed"),
        _ => ("Başarısız", "st-failed"),
    };
    let task_cards: String = if tasks.is_empty() {
        "<p class='muted'>Henüz görev yok</p>".into()
    } else {
        tasks.iter().map(|t| {
            let my_sub = subs.iter().find(|s| s.task_id == t.id);
            // preview of the example project; interaction goes through the wrapping link
            // (the iframe/img is pointer-events:none in CSS). Sites that allow iframe
            // embedding get a live preview; the rest get a cached hero screenshot served
            // from /preview/{id} (many sites send X-Frame-Options and can't be embedded).
            let example = t.example_url.as_deref().filter(|u| !u.is_empty()).map(|u| {
                preview_link(u, t.example_embeddable == Some(true),
                    &format!("/preview/{}", t.id), "Örnek projeyi yeni sekmede aç")
            }).unwrap_or_default();
            // Only tasks that actually collected deployed sites get the gallery button —
            // nothing here names the personal-website task, it's just where the sites are.
            let sites = site_counts.iter().find(|(tid, _)| *tid == t.id).map(|(_, n)| format!(
                r##"<div class="cardactions"><a class="btn-outline small" href="/board/sites/{id}">Arkadaşlarının siteleri ({n}) →</a></div>"##,
                id = t.id)).unwrap_or_default();
            let sub_html = match my_sub {
                Some(s) => {
                    let (label, class) = status_tr(&s.status);
                    let fb = s.feedback.as_deref().filter(|f| !f.is_empty())
                        .map(|f| format!(r#"<p class="feedback"><b>Geri bildirim:</b> {}</p>"#, esc(f)))
                        .unwrap_or_default();
                    // scheme-gate before it lands in an href: only http(s), so a
                    // javascript:/data: value (worker-token write) can't become a clickable script
                    let demo = s.demo_video_url.as_deref()
                        .filter(|d| d.starts_with("https://") || d.starts_with("http://"))
                        .map(|d| format!(r#"<p><a class="btn-outline" href="{}" target="_blank">Tanıtım videosu →</a></p>"#, esc(d)))
                        .unwrap_or_default();
                    let plan_ok = if s.plan_md.as_deref().is_some_and(|p| !p.trim().is_empty()) {
                        r#"<p class="fieldnote">plan.md yüklendi ✓</p>"#
                    } else { "" };
                    format!(r#"<div class="substatus {class}">{label}</div>{fb}{demo}{plan_ok}"#)
                }
                None => String::new(),
            };
            // "Göreve Başla!" gate: before starting, the card shows only the start
            // button (posts to /board/interest, one-way). Once started, the teammates
            // list and the project upload form are revealed. my_sub covers any student
            // who submitted before this flow existed.
            let card_interests: Vec<&InterestRow> = interests.iter().filter(|i| i.task_id == t.id).collect();
            let mine = card_interests.iter().any(|i| i.is_me);
            let started = mine || my_sub.is_some();
            let action_area = if started {
                let chips: String = card_interests.iter()
                    .filter(|i| !i.nickname.is_empty())
                    .map(|i| format!(r#"<span class="chip">{}</span>"#, esc(&i.nickname)))
                    .collect();
                format!(
                    r##"<div class="chips interest-names">{chips}</div>
  <p class="fieldnote">Birlikte yapmak için birbirinize ulaşın 🤝</p>
  <form method="post" action="/board/submit" enctype="multipart/form-data" class="subform">
    <input type="hidden" name="task_id" value="{id}">
    <input name="repo_url" type="url" placeholder="https://github.com/..." required>
    <input name="live_url" type="url" placeholder="Canlı site adresi (varsa) — https://...">
    <label class="dropzone">
      <input name="plan" type="file" accept=".md,.markdown,text/markdown" required
        onchange="var z=this.closest('.dropzone');z.classList.toggle('has-file',this.files.length>0);z.querySelector('b').textContent=this.files.length?this.files[0].name:'plan.md dosyanızı sürükleyin veya seçin'"
        ondragenter="this.closest('.dropzone').classList.add('drag')"
        ondragleave="this.closest('.dropzone').classList.remove('drag')"
        ondrop="this.closest('.dropzone').classList.remove('drag')">
      {up}
      <b>plan.md dosyanızı sürükleyin veya seçin</b>
      <span>Mimari planınız (.md)</span>
    </label>
    <button class="btn-dark">Projenizi Yükle →</button>
  </form>"##,
                    id = t.id, up = ico(P_UPLOAD))
            } else {
                format!(
                    r##"<div class="cardactions">
    <form method="post" action="/board/interest" class="inline">
      <input type="hidden" name="task_id" value="{id}">
      <button class="btn-start" title="Göreve başla, birlikte çalışacak arkadaşlarını gör ve projeni yükle">Göreve Başla!</button>
    </form>
  </div>"##,
                    id = t.id)
            };
            format!(
                r##"<div class="taskcard">
  <div class="taskhead"><h3>{title}</h3><span class="badge {badge_cls}" lang="en">{level}</span></div>
  <p class="desc">{desc}</p>
  {example}
  {sites}
  {sub_html}
  {action_area}
</div>"##,
                title = esc(&t.title), level = level_name(&t.level), badge_cls = level_badge_class(&t.level),
                desc = esc(&t.description),
            )
        }).collect()
    };
    layout(
        "Görev Panosu",
        Some(user),
        "board",
        &format!(
            r##"<div id="board-root"><h1 class="pagetitle">Görev Panosu</h1><p class="muted">Projenizi yükleyin.</p><div class="tasks">{task_cards}</div></div>
<script src="/static/board.js?v=1" defer></script>"##
        ),
    )
}

/// Every student's deployed site for one task, as live previews. Same card grid and the
/// same preview thumbnail as the board, so the two pages read as one thing.
pub fn board_sites(user: &User, task: &Task, cards: &[SiteCard]) -> String {
    let grid: String = if cards.is_empty() {
        "<p class='muted'>Bu görevde henüz yayınlanmış site yok.</p>".into()
    } else {
        cards
            .iter()
            .map(|c| {
                format!(
                    r##"<div class="taskcard">
  {preview}
  <div class="taskhead"><h3>{nick}</h3></div>
  <div class="cardactions">
    <a class="btn-dark small" href="{live}" target="_blank" rel="noopener">Siteyi aç ↗</a>
    <a class="btn-outline small" href="{repo}" target="_blank" rel="noopener">Repo ↗</a>
  </div>
</div>"##,
                    // live_url is written only through resolve_live_url / the admin override, both
                    // of which scheme-check it, so it's safe in an href here
                    preview = preview_link(
                        &c.live_url,
                        c.live_embeddable == Some(true),
                        &format!("/preview/sub/{}", c.id),
                        &format!("{} sitesini yeni sekmede aç", esc(&c.nickname))
                    ),
                    nick = esc(&c.nickname),
                    live = esc(&c.live_url),
                    repo = esc(&c.repo_url),
                )
            })
            .collect()
    };
    layout(
        &format!("{} — Siteler", task.title),
        Some(user),
        "board",
        &format!(
            r##"<p class="backlink"><a href="/board">← Görev Panosu</a></p>
<div class="taskhead"><h1 class="pagetitle">{title}</h1><span class="badge {badge}" lang="en">{level}</span></div>
<p class="muted">Arkadaşlarının yayına aldığı siteler. Önizlemeye tıklayınca site yeni sekmede açılır.</p>
<div class="tasks">{grid}</div>"##,
            title = esc(&task.title),
            level = level_name(&task.level),
            badge = level_badge_class(&task.level),
        ),
    )
}

/// The board gate. Shown instead of the tasks when the student is missing either public
/// profile. Unlike onboarding there is no skip — both fields are required to continue.
/// `github`/`linkedin` pre-fill whatever they already have (e.g. added one, not the other).
pub fn board_locked(
    user: &User,
    github: Option<&str>,
    linkedin: Option<&str>,
    error: Option<&str>,
) -> String {
    let err = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    let content = format!(
        r##"<h1 class="pagetitle">Görev Panosu kilitli</h1>
<p class="muted">Panoya erişmeden önce GitHub ve LinkedIn profillerini eklemen gerekiyor.</p>
<div class="profilewrap">
<section class="panel gate-panel">
  <div class="gate-lock">{lock}</div>
  <h2>Profillerini ekle</h2>
  <p class="fieldnote">GitHub ve LinkedIn, yaptığın işi dünyaya gösterdiğin yerdir.
  Hesabın varsa aşağıya linkleri gir, yoksa hemen aç!
  <a href="https://github.com/signup" target="_blank" rel="noopener">GitHub</a> ·
  <a href="https://www.linkedin.com/signup" target="_blank" rel="noopener">LinkedIn</a>.</p>
  {err}
  <form method="post" action="/board/profiles">
    <label>GitHub<input name="github_url" type="text" inputmode="url" value="{github}" placeholder="https://github.com/kullanici" required></label>
    <label>LinkedIn<input name="linkedin_url" type="text" inputmode="url" value="{linkedin}" placeholder="https://linkedin.com/in/adin" required></label>
    <button class="btn-dark">Kaydet ve panoyu aç →</button>
  </form>
</section>
</div>"##,
        lock = ico(P_LOCK),
        github = esc(github.unwrap_or("")),
        linkedin = esc(linkedin.unwrap_or("")),
    );
    layout("Görev Panosu", Some(user), "board", &content)
}

/// What the Görev column shows. Board rows carry a real task title; beginner rows carry
/// the `project_key`, because titles are a `BEGINNER_PROJECTS` fact and the union query
/// has no business hardcoding them.
pub fn grade_row_title(r: &GradeRow) -> String {
    if r.kind == "beginner" {
        BEGINNER_PROJECTS
            .iter()
            .find(|(k, ..)| *k == r.task_title)
            .map(|(_, title, ..)| (*title).to_string())
            .unwrap_or_else(|| r.task_title.clone())
    } else {
        r.task_title.clone()
    }
}

/// The "Goals:" line of the review prompt: the task's Tanım for board rows, the fixed
/// one-line summary for Beginner Track ones. Falls back to the title when a task has no
/// description, so the prompt is never handed an empty goal.
pub fn grade_row_goal(r: &GradeRow, tasks: &[Task]) -> String {
    if r.kind == "beginner" {
        return BEGINNER_PROJECTS
            .iter()
            .find(|(k, ..)| *k == r.task_title)
            .map(|(_, _, summary, ..)| (*summary).to_string())
            .unwrap_or_else(|| grade_row_title(r));
    }
    r.task_id
        .and_then(|id| tasks.iter().find(|t| t.id == id))
        .map(|t| t.description.trim())
        .filter(|d| !d.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| r.task_title.clone())
}

/// Görev Puanlama — the grading queue (grading.rs).
///
/// Layout follows the standard admin-table filter pattern: a horizontal filter bar above
/// the table, segmented chips for the two low-cardinality cuts (Durum, Seviye), a select
/// for the high-cardinality one (Öğrenci), counts on the chips that reflect the *other*
/// active filters, and a result count plus a clear-all after the last control. The chips
/// are the same `.chip` component /videos and /admin/harness already use.
///
/// Every control saves by itself (static/grading.js). The `<noscript>` buttons are the
/// whole no-JS story: with JS on they are not in the DOM, so there is nothing to click
/// and nothing to forget to click.
pub fn grading(
    user: &User,
    rows: &[GradeRow],
    tasks: &[Task],
    members: &[MemberRow],
    f: &Filters,
) -> String {
    // Hidden filter state, repeated into every row form so the no-JS POST → redirect
    // lands back on the view you were grading from instead of resetting to the queue.
    let mut filter_fields = String::new();
    if f.durum != DURUM_DEFAULT {
        filter_fields.push_str(&format!(
            r#"<input type="hidden" name="durum" value="{}">"#,
            esc(&f.durum)
        ));
    }
    if let Some(s) = &f.seviye {
        filter_fields.push_str(&format!(
            r#"<input type="hidden" name="seviye" value="{}">"#,
            esc(s)
        ));
    }
    if let Some(u) = f.ogrenci {
        filter_fields.push_str(&format!(
            r#"<input type="hidden" name="ogrenci" value="{u}">"#
        ));
    }
    if f.tum {
        filter_fields.push_str(r#"<input type="hidden" name="tum" value="1">"#);
    }

    let durum_chips: String = DURUM_FILTERS
        .iter()
        .map(|(key, label)| {
            let n = rows.iter().filter(|r| durum_matches(key, &r.status)).count();
            format!(
                r#"<a class="chip {active}" href="{href}">{label}<span class="chip-count">{n}</span></a>"#,
                active = if f.durum == *key { "active" } else { "" },
                href = esc(&f.with_durum(key).url()),
            )
        })
        .collect();

    let seviye_chips: String = std::iter::once((None, "Hepsi"))
        .chain(LEVELS.iter().map(|(k, v)| (Some(*k), *v)))
        .map(|(key, label)| {
            format!(
                r#"<a class="chip {active}" href="{href}">{label}</a>"#,
                active = if f.seviye.as_deref() == key {
                    "active"
                } else {
                    ""
                },
                href = esc(&f.with_seviye(key).url()),
            )
        })
        .collect();

    let student_opts: String =
        std::iter::once(r#"<option value="">Tüm öğrenciler</option>"#.to_string())
            .chain(members.iter().filter(|m| !m.is_admin).map(|m| {
                format!(
                    r#"<option value="{id}"{sel}>{name}</option>"#,
                    id = m.id,
                    sel = if f.ogrenci == Some(m.id) {
                        " selected"
                    } else {
                        ""
                    },
                    name = esc(&m.display_name),
                )
            }))
            .collect();

    let shown: Vec<&GradeRow> = rows
        .iter()
        .filter(|r| durum_matches(&f.durum, &r.status))
        .collect();

    let sub_rows: String = shown
        .iter()
        .map(|r| grading_row(r, tasks, &filter_fields))
        .collect();
    let empty_row = if shown.is_empty() {
        r#"<tr><td colspan="10" class="muted empty-row">Bu filtreye uyan gönderim yok.</td></tr>"#
    } else {
        ""
    };

    let clear = if f.is_narrowed() {
        r#"<a class="filter-clear" href="/admin/puanlama?durum=hepsi">Filtreleri temizle</a>"#
    } else {
        ""
    };

    let content = format!(
        r##"<div id="grading-root" data-durum="{durum}">
<h1 class="pagetitle">Görev Puanlama</h1>

<section class="panel wide">
  <div class="panel-head">
    <h2>Gönderimler</h2>
    <a class="btn-dark small" href="/admin/puanlama/prompts.txt{qs}">⬇ Prompts .txt</a>
  </div>

  <div class="filterbar">
    <span class="filter-label">Durum</span>
    <div class="chips">{durum_chips}</div>
  </div>
  <div class="filterbar">
    <span class="filter-label">Seviye</span>
    <div class="chips">{seviye_chips}</div>
  </div>
  <form class="filterbar" method="get" action="/admin/puanlama">
    {filter_fields_get}
    <span class="filter-label">Öğrenci</span>
    <select name="ogrenci" onchange="this.form.submit()">{student_opts}</select>
    <label class="checkline"><input type="checkbox" name="tum" value="1"{tum_checked}
      onchange="this.form.submit()"> Tüm denemeler</label>
    <noscript><button class="btn-dark small">Uygula</button></noscript>
    <span class="filter-count">{total} gönderim · {count} gösteriliyor</span>
    {clear}
  </form>

  <p class="muted">Değişiklikler otomatik kaydedilir — ayrı bir kaydetme adımı yok.
  Puan kutusu boşsa görevin seviye varsayılanı geçerlidir (Beginner {PTS_PROJECT_L1},
  Intermediate {PTS_PROJECT_L2}, Advanced {PTS_PROJECT_L3}). Puan yalnızca durum "Geçti" ise sayılır.</p>
  <p class="muted">Varsayılan olarak her öğrencinin her görevdeki <b>en son</b> denemesi listelenir;
  eskiler için "Tüm denemeler"i aç. Canlı site adresleri arka planda otomatik bulunur (repo'nun
  <code>homepage</code> alanı, yoksa GitHub Pages adresi). Beginner Track satırlarında site adresi
  öğrencinin kendi girdiği Vercel bağlantısıdır, buradan değiştirilmez.</p>

  <table>
    <tr><th>Öğrenci</th><th>Görev</th><th>Repo</th><th>Site</th><th>Plan</th><th>Gönderim</th>
        <th>Durum</th><th>Puan</th><th>Geri bildirim</th><th></th></tr>
    {sub_rows}{empty_row}
  </table>
</section>
</div>
<script src="/static/grading.js?v=1" defer></script>"##,
        qs = esc(&f.query_string()),
        // grading.js reads this to decide whether a row it just saved still belongs in
        // the current queue — see .row-left-queue
        durum = esc(&f.durum),
        // the GET form re-submits durum/seviye as hidden inputs so changing Öğrenci
        // doesn't silently drop the chips above it
        filter_fields_get = {
            let mut s = String::new();
            if f.durum != DURUM_DEFAULT {
                s.push_str(&format!(
                    r#"<input type="hidden" name="durum" value="{}">"#,
                    esc(&f.durum)
                ));
            }
            if let Some(v) = &f.seviye {
                s.push_str(&format!(
                    r#"<input type="hidden" name="seviye" value="{}">"#,
                    esc(v)
                ));
            }
            s
        },
        tum_checked = if f.tum { " checked" } else { "" },
        total = rows.len(),
        count = shown.len(),
    );
    layout("Görev Puanlama", Some(user), "puanlama", &content)
}

/// One queue row. The three graded fields (Durum, Puan, Geri bildirim) each sit in their
/// own column but post as one form via the `form=` attribute — a `<form>` can't span table
/// cells, and cramming them into a single cell is what made the old table hard to read.
fn grading_row(r: &GradeRow, tasks: &[Task], filter_fields: &str) -> String {
    let form_id = format!("rev-{}", r.key);
    let status_opts: String = GRADE_STATUSES
        .iter()
        .map(|(k, v)| {
            format!(
                r#"<option value="{k}"{sel}>{v}</option>"#,
                sel = if *k == r.status { " selected" } else { "" },
            )
        })
        .collect();
    let plan = r
        .plan_md
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            format!(
                r#"<details class="plan-details"><summary>Plan</summary><pre class="plan-pre">{}</pre></details>"#,
                esc(p)
            )
        })
        .unwrap_or_else(|| "—".into());

    // Beginner Track rows are Beginner by definition, so the badge is honest, but they
    // still need to be distinguishable from a board görev at a glance.
    let beginner_tag = if r.kind == "beginner" {
        r#" <span class="tag-beginner" lang="en">Beginner Track</span>"#
    } else {
        ""
    };
    let attempt_tag = if r.attempts > 1 {
        format!(
            r#" <span class="tag-attempt" title="Bu öğrencinin bu görevdeki {n}. denemesi">⟳ {n}. deneme</span>"#,
            n = r.attempts
        )
    } else {
        String::new()
    };

    // The site cell is an editable override for board rows and a plain link for beginner
    // ones, whose vercel_url belongs to the student.
    let site = if r.kind == "beginner" {
        r.live_url
            .as_deref()
            .filter(|u| u.starts_with("http"))
            .map(|u| {
                format!(
                    r#"<a href="{url}" target="_blank" rel="noopener">site ↗</a>"#,
                    url = esc(u)
                )
            })
            .unwrap_or_else(|| "—".into())
    } else {
        format!(
            r##"<form method="post" action="/admin/puanlama/live" class="inline" data-live-form>
  <input type="hidden" name="key" value="{key}">{filter_fields}
  <input name="live_url" type="url" value="{live}" placeholder="https://... (boş = yok)"
    title="Canlı site adresi — boş bırakıp kaydetmek adresi siler">
  <noscript><button class="btn-dark small">Kaydet</button></noscript>
</form>{live_open}"##,
            key = esc(&r.key),
            filter_fields = filter_fields,
            live = esc(r.live_url.as_deref().unwrap_or("")),
            live_open = r
                .live_url
                .as_deref()
                .filter(|u| u.starts_with("http"))
                .map(|u| format!(
                    r#" <a href="{}" target="_blank" rel="noopener">↗</a>"#,
                    esc(u)
                ))
                .unwrap_or_default(),
        )
    };

    format!(
        r##"<tr data-kind="{kind}" data-key="{key}" data-status="{status}">
<td><div class="cell-name">{student}</div><div class="cell-sub">{email}</div></td>
<td><div class="cell-name">{task}</div><div class="cell-sub"><span class="badge {lvl_class}">{lvl}</span>{beginner_tag}{attempt_tag}</div></td>
<td><a href="{url}" target="_blank" rel="noopener">repo</a><button type="button" class="btn-copy" data-prompt="{prompt}">⧉ Prompt</button></td>
<td>{site}</td>
<td>{plan}</td>
<td class="nowrap">{date}</td>
<td><select form="{form_id}" name="status">{status_opts}</select></td>
<td><input class="pts-input" form="{form_id}" type="number" min="0" step="1" name="points" value="{pts}"
     placeholder="{pts_default}" title="Boş bırakırsan {lvl} varsayılanı olan {pts_default} puan verilir"></td>
<td><input class="fb-input" form="{form_id}" name="feedback" placeholder="Geri bildirim" value="{fb}"></td>
<td><form method="post" action="/admin/puanlama/review" class="inline" id="{form_id}">
  <input type="hidden" name="kind" value="{kind}">
  <input type="hidden" name="key" value="{key}">{filter_fields}
  <span class="rowsave" aria-live="polite"></span>
  <noscript><button class="btn-dark small">Kaydet</button></noscript>
</form></td></tr>"##,
        kind = esc(&r.kind),
        key = esc(&r.key),
        status = esc(&r.status),
        student = esc(&r.display_name),
        email = esc(&r.email),
        task = esc(&grade_row_title(r)),
        lvl = level_name(&r.task_level),
        lvl_class = level_badge_class(&r.task_level),
        url = esc(&r.repo_url),
        prompt = esc(&review_prompt(&r.repo_url, &grade_row_goal(r, tasks))),
        date = r.created_at.format("%d.%m.%Y %H:%M"),
        pts = r.points_override.map(|p| p.to_string()).unwrap_or_default(),
        pts_default = level_points(&r.task_level),
        fb = esc(r.feedback.as_deref().unwrap_or("")),
        filter_fields = filter_fields,
    )
}

pub fn admin(
    user: &User,
    stats: &[StatRow],
    videos: &[Video],
    tasks: &[Task],
    members: &[MemberRow],
    invite_code: &str,
    base_url: &str,
    harness: &HarnessAdmin,
    monopoly: &MonopolyAdmin,
    schedule_images: &[ScheduleImage],
    venues: &[Venue],
    consent_docs: &[ConsentDoc],
    consent_locks: &[(&str, bool)],
    consent_urls: &[(&str, String)],
) -> String {
    let consent_panel = admin_consent_panel(members, consent_docs, consent_locks, consent_urls);
    let schedule_panel = admin_schedule_panel(schedule_images);
    let venue_panel = admin_venue_panel(venues);
    let invite_link = format!("{}/join/{}", base_url.trim_end_matches('/'), invite_code);
    let level_opts = level_options("");
    let stat_rows: String = stats
        .iter()
        .map(|s| {
            let pct = if s.duration > 0.0 {
                (s.max_position / s.duration * 100.0).min(100.0)
            } else {
                0.0
            };
            format!(
                "<tr><td>{}</td><td>{}</td><td>%{:.0}</td><td>{:.0} dk</td><td>{}</td></tr>",
                esc(&s.display_name),
                esc(&s.video_title),
                pct,
                s.seconds_watched / 60.0,
                s.updated_at.format("%d.%m.%Y %H:%M"),
            )
        })
        .collect();
    let video_rows: String = videos.iter().map(|v| format!(
        r##"<div class="itemrow">
  <div class="item-title"><span>{title}</span><span class="item-meta">{yt}</span></div>
  <div class="item-controls">
    <form method="post" action="/admin/video/level" class="inline">
      <input type="hidden" name="id" value="{id}">
      <select name="level">{opts}</select>
      <button class="btn-dark small">Kaydet</button>
    </form>
    <form method="post" action="/admin/video/delete" class="inline" onsubmit="return confirm('Bu videoyu silersen öğrencilerin izleme ilerlemesi ve bu videodan kazanılan puanlar da silinir. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>
  </div>
</div>"##,
        title = esc(&v.title), id = v.id, opts = level_options(&v.level), yt = esc(&v.youtube_id),
    )).collect();
    let task_rows: String = tasks.iter().map(|t| format!(
        r##"<div class="itemrow">
  <div class="item-title"><span>{title}</span></div>
  <div class="item-controls">
    <form method="post" action="/admin/task/move" class="inline"><input type="hidden" name="id" value="{id}"><button name="dir" value="up" class="btn-dark small" title="Yukarı">▲</button></form>
    <form method="post" action="/admin/task/move" class="inline"><input type="hidden" name="id" value="{id}"><button name="dir" value="down" class="btn-dark small" title="Aşağı">▼</button></form>
    <form method="post" action="/admin/task/level" class="inline">
      <input type="hidden" name="id" value="{id}">
      <select name="level">{opts}</select>
      <button class="btn-dark small">Kaydet</button>
    </form>
    <form method="post" action="/admin/task/delete" class="inline" onsubmit="return confirm('Bu görevi silersen tüm gönderimler ve bu görevden kazanılan puanlar da silinir. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>
    <form method="post" action="/admin/task/example" class="inline urlform">
      <input type="hidden" name="id" value="{id}">
      <input name="example_url" type="url" placeholder="Örnek proje URL — https://…" value="{example}">
      <button class="btn-dark small">Kaydet</button>
    </form>
    <form method="post" action="/admin/task/preview" class="inline" title="Canlı önizleme yalnızca iframe gömülmesine izin veren siteler için çalışır; izin vermeyen siteler boş görünür — o durumda Görsel seç.">
      <input type="hidden" name="id" value="{id}">
      <select name="mode">
        <option value="image"{image_sel}>Görsel önizleme</option>
        <option value="live"{live_sel}>Canlı önizleme</option>
      </select>
      <button class="btn-dark small">Kaydet</button>
    </form>
  </div>
  <form method="post" action="/admin/task/edit" class="editform edit-details">
    <input type="hidden" name="id" value="{id}">
    <label>Başlık<input name="title" value="{title}" required></label>
    <label>Tanım<textarea name="description" rows="{desc_rows}" required>{desc}</textarea></label>
    <button class="btn-dark small">Kaydet</button>
  </form>
</div>"##,
        title = esc(&t.title), id = t.id, opts = level_options(&t.level),
        example = esc(t.example_url.as_deref().unwrap_or("")),
        image_sel = if t.example_embeddable == Some(true) { "" } else { " selected" },
        live_sel = if t.example_embeddable == Some(true) { " selected" } else { "" },
        desc = esc(&t.description), desc_rows = textarea_rows(&t.description, 48),
    )).collect();
    let member_rows: String = if members.is_empty() {
        "<p class='muted'>Henüz öğrenci yok</p>".into()
    } else {
        members.iter().map(|m| {
            let name = m.nickname.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(&m.display_name);
            // admin accounts and the current user can't be removed from here — avoids
            // locking yourself out or nuking a fellow admin by accident
            let action = if m.is_admin {
                r#"<span class="item-meta">Yönetici</span>"#.to_string()
            } else {
                // "Gizle" keeps the account out of the puan tablosu and the görev
                // panosu teammate chips — for stajyer/ekip accounts that follow the
                // program without competing with the students.
                format!(
                    r#"<form method="post" action="/admin/user/hidden" class="inline">
      <input type="hidden" name="id" value="{id}">
      <input type="hidden" name="hidden" value="{next}">
      <button class="btn-outline small">{toggle}</button>
    </form>
    <form method="post" action="/admin/user/delete" class="inline" onsubmit="return confirm('{name} adlı öğrenciyi ve tüm ilerlemesini/gönderimlerini kalıcı olarak silmek istediğine emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>"#,
                    name = esc(name), id = m.id,
                    next = if m.hidden_from_leaderboard { "false" } else { "true" },
                    toggle = if m.hidden_from_leaderboard { "Puan tablosunda göster" } else { "Puan tablosunda gizle" },
                )
            };
            let badge = if m.hidden_from_leaderboard && !m.is_admin {
                r#"<span class="item-meta">· puan tablosunda gizli</span>"#
            } else { "" };
            format!(
                r##"<div class="itemrow">
  <div class="item-title"><span>{name}</span><span class="item-meta">{email}</span>{badge}</div>
  <div class="item-controls">{action}</div>
</div>"##,
                name = esc(name), email = esc(&m.email),
            )
        }).collect()
    };
    // Shared by the Monopoly assign form below; the harness one moved to /admin/takimlar.
    let harness_student_opts: String = members
        .iter()
        .filter(|m| !m.is_admin)
        .map(|m| {
            format!(
                r#"<option value="{}">{}</option>"#,
                m.id,
                esc(&m.display_name)
            )
        })
        .collect();
    // "Başarısız say" is the stuck-run escape hatch: a worker that died mid-run
    // otherwise blocks the team's resubmits forever (one-active-run index).
    let harness_run_rows: String = if harness.active_runs.is_empty() {
        "<p class='muted'>Aktif çalıştırma yok</p>".into()
    } else {
        harness.active_runs.iter().map(|r| {
            let (label, class) = harness_stage_tr(&r.stage);
            format!(
                r##"<div class="itemrow">
  <div class="item-title"><span>{team}</span><span class="item-meta">{date}</span><span class="substatus {class}">{label}</span></div>
  <div class="item-controls">
    <form method="post" action="/admin/harness/run/fail" class="inline" onsubmit="return confirm('Bu çalıştırma başarısız olarak işaretlenecek ve takım yeniden gönderebilecek. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Başarısız say</button>
    </form>
  </div>
</div>"##,
                team = esc(&r.team_name), date = r.created_at.format("%d.%m.%Y %H:%M"), id = r.id)
        }).collect()
    };
    // The rejected-submission log. `raw_input` is unvalidated student text — the only place in
    // this feature where it reaches HTML — so it goes through esc() like everything else here.
    let harness_rejected_rows: String = if harness.rejected.is_empty() {
        "<p class='muted'>Reddedilen gönderim yok</p>".into()
    } else {
        harness
            .rejected
            .iter()
            .map(|r| {
                format!(
                    r##"<div class="itemrow">
  <div class="item-title"><span>{who}</span><span class="item-meta">{team} · {date}</span><span class="substatus st-failed">{reason}</span></div>
  <div class="item-meta"><code>{raw}</code></div>
</div>"##,
                    who = esc(r.display_name.as_deref().unwrap_or("(silinmiş öğrenci)")),
                    team = esc(r.team_name.as_deref().unwrap_or("takımsız")),
                    date = r.created_at.format("%d.%m.%Y %H:%M"),
                    reason = harness_reject_reason_tr(&r.reason),
                    raw = esc(&r.raw_input),
                )
            })
            .collect()
    };
    // AI Monopoly mirrors the harness block above: same interim team management, plus
    // the start button and the running tournament's stop hatch.
    let monopoly_team_opts: String = monopoly
        .teams
        .iter()
        .map(|t| format!(r#"<option value="{}">{}</option>"#, t.id, esc(&t.name)))
        .collect();
    let monopoly_team_rows: String = if monopoly.teams.is_empty() {
        "<p class='muted'>Henüz takım yok</p>".into()
    } else {
        monopoly.teams.iter().map(|t| {
            let kid_buttons: String = monopoly.members.iter().filter(|m| m.team_id == t.id).map(|m| format!(
                r#"<form method="post" action="/admin/monopoly/member/remove" class="inline">
      <input type="hidden" name="id" value="{uid}">
      <button class="btn-outline small" title="Takımdan çıkar">{name} ✕</button>
    </form>"#,
                uid = m.user_id, name = esc(&m.display_name))).collect();
            // the entry, if they've submitted one — this is what the start button freezes
            let entry = monopoly.entries.iter().find(|e| e.team_id == t.id);
            let entry_line = match entry {
                Some(e) => format!(
                    r#"<span class="item-meta">{char} · {product} · {price}₺ · <span lang="en">{repo}</span></span>"#,
                    char = esc(&e.char_name), product = esc(&e.product_name),
                    price = e.list_price, repo = esc(&e.hf_repo)),
                None => r#"<span class="item-meta">gönderim yok</span>"#.to_string(),
            };
            format!(
                r##"<div class="itemrow">
  <div class="item-title"><span>{name}</span>{entry_line}</div>
  <div class="item-controls">{kid_buttons}
    <form method="post" action="/admin/monopoly/team/delete" class="inline" onsubmit="return confirm('Bu takımı silersen gönderimi, konuşmaları ve turnuva geçmişi de silinir. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>
  </div>
</div>"##,
                name = esc(&t.name), id = t.id)
        }).collect()
    };
    // Three states: no tournament yet (start button), one running (progress + stop),
    // one finished (result + start a new one).
    let monopoly_tournament_block = match &monopoly.tournament {
        Some(t) if t.status != "done" && t.status != "failed" => {
            let (label, class) = monopoly_status_tr(&t.status);
            format!(
                r##"<div class="itemrow">
  <div class="item-title"><span>Tur {round}/{total}</span><span class="substatus {class}">{label}</span>
    <span class="item-meta">{progress}</span></div>
  <div class="item-controls">
    <a class="btn-outline small" href="/ai-monopoly?tab=live">Canlı izle</a>
    <form method="post" action="/admin/monopoly/fail" class="inline" onsubmit="return confirm('Turnuva başarısız olarak işaretlenecek ve yeni bir turnuva başlatılabilecek. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Durdur</button>
    </form>
  </div>
</div>"##,
                round = t.round,
                total = t.rounds_total,
                class = class,
                label = label,
                progress = esc(t.progress.as_deref().unwrap_or("")),
                id = t.id
            )
        }
        other => {
            let last = match other {
                Some(t) => {
                    let (label, class) = monopoly_status_tr(&t.status);
                    format!(
                        r#"<p class="fieldnote">Son turnuva: <span class="substatus {class}">{label}</span> · {date}{err}</p>"#,
                        class = class,
                        label = label,
                        date = t.created_at.format("%d.%m.%Y %H:%M"),
                        err = match &t.error_log {
                            Some(e) if !e.is_empty() => format!(" · {}", esc(e)),
                            _ => String::new(),
                        }
                    )
                }
                None => String::new(),
            };
            format!(
                r##"{last}
  <form method="post" action="/admin/monopoly/start" onsubmit="return confirm('Turnuva başlayacak ve gönderimler bu haliyle dondurulacak. Emin misin?')">
    <button class="btn-dark"{disabled}>Turnuvayı başlat ({n} takım hazır)</button>
  </form>"##,
                last = last,
                n = monopoly.entries.len(),
                // needs two entries to have a game at all; the handler enforces it too
                disabled = if monopoly.entries.len() < 2 {
                    " disabled"
                } else {
                    ""
                }
            )
        }
    };
    layout(
        "Yönetici paneli",
        Some(user),
        "admin",
        &format!(
            r##"<div id="admin-root">
<h1 class="pagetitle">Yönetici paneli</h1>

{consent_panel}

{schedule_panel}

{venue_panel}

<div class="admingrid stack">
<section class="panel">
  <h2>Video ekle</h2>
  <form method="post" action="/admin/video">
    <label>Başlık<input name="title" required></label>
    <label>YouTube ID / bağlantı<input name="youtube" placeholder="dQw4w9WgXcQ" required></label>
    <label>Seviye<select name="level">{level_opts}</select></label>
    <button class="btn-dark">Kaydet</button>
  </form>
  <div class="minilist">{video_rows}</div>
</section>

<section class="panel">
  <h2>Görev ekle</h2>
  <form method="post" action="/admin/task">
    <label>Başlık<input name="title" required></label>
    <label>Tanım<textarea name="description" rows="3" required></textarea></label>
    <label>Örnek proje URL (opsiyonel)<input name="example_url" type="url" placeholder="https://ornek-proje.vercel.app"></label>
    <label>Seviye<select name="level">{level_opts}</select></label>
    <button class="btn-dark">Kaydet</button>
  </form>
  <div class="minilist">{task_rows}</div>
</section>

<section class="panel">
  <h2>Öğrenci ekle</h2>
  <form method="post" action="/admin/user">
    <label>E-posta<input name="email" type="email" required></label>
    <label>İsim<input name="display_name" required></label>
    <label class="checkline"><input type="checkbox" name="hidden" value="true"> Puan tablosunda gizle</label>
    <p class="fieldnote">Stajyer / ekip hesabı için: portalı öğrenciler gibi kullanır, videoları izler ve
    projeleri yapar — ama puan tablosunda ve görev panosundaki takım arkadaşı listesinde görünmez.
    Davet bağlantısıyla kaydolmadan önce burada eklersen hiçbir an görünmez.</p>
    <button class="btn-dark">Kaydet</button>
  </form>
  <div class="minilist">{member_rows}</div>
</section>

<section class="panel">
  <h2>Davet bağlantısı</h2>
  <p class="muted">WhatsApp grubuna bu bağlantıyı at — kod bağlantının içinde, öğrenciler
  yalnızca kendi bilgilerini doldurur.</p>
  <input value="{invite_link}" readonly onclick="this.select()">
  <p class="fieldnote">Kod: <b>{invite_code}</b> · Kodu yenilersen eski bağlantı çalışmaz.</p>
  <form method="post" action="/admin/invite">
    <button class="btn-dark">Kodu yenile</button>
  </form>
</section>

<section class="panel">
  <h2 lang="en">Agentic Harness — <span lang="tr">Çalıştırmalar</span></h2>
  <p class="fieldnote">Takımları kurmak ve öğrencileri sürükleyip atamak için
  <a href="/admin/takimlar">Takım formasyonu</a> sayfasına git. Burada yalnızca
  takılı kalmış çalıştırmalar kurtarılır.</p>
  <p class="muted">Aktif çalıştırmalar</p>
  <div class="minilist">{harness_run_rows}</div>
  <p class="muted">Reddedilen gönderimler (son 50)</p>
  <p class="fieldnote">Öğrencinin yapıştırdığı bağlantı ve neden kabul edilmediği.
  30 günden eski kayıtlar silinir. Burada kayıt yoksa bağlantı kabul edilmiş demektir —
  sorun klonlamada (özel repo gibi), çalıştırmanın hata kaydına bak.</p>
  <div class="minilist">{harness_rejected_rows}</div>
</section>

<section class="panel">
  <h2 lang="en">AI Monopoly — <span lang="tr">Takımlar</span></h2>
  <p class="fieldnote">Bu bölümün takımları <span lang="en">Agentic Harness</span> takımlarından
  ayrıdır. Bir öğrenci aynı anda tek Monopoly takımında olabilir.</p>
  <form method="post" action="/admin/monopoly/team">
    <label>Takım adı<input name="name" required></label>
    <button class="btn-dark">Takım oluştur</button>
  </form>
  <form method="post" action="/admin/monopoly/member">
    <label>Öğrenci<select name="user_id">{harness_student_opts}</select></label>
    <label>Takım<select name="team_id">{monopoly_team_opts}</select></label>
    <button class="btn-dark">Takıma ata</button>
  </form>
  <div class="minilist">{monopoly_team_rows}</div>
  <p class="muted">Turnuva</p>
  <p class="fieldnote">Başlatınca her takımın o anki gönderimi dondurulur; sonradan yapılan
  değişiklikler bu turnuvayı ve geçmişini etkilemez.</p>
  {monopoly_tournament_block}
</section>
</div>

<section class="panel wide">
  <h2>İzleme istatistikleri</h2>
  <table><tr><th>Öğrenci</th><th>Video</th><th>İlerleme</th><th>Toplam süre</th><th>Son izleme</th></tr>{stat_rows}</table>
</section>

</div>
<script src="/static/admin.js?v=6" defer></script>"##
        ),
    )
}

/// One column of the formation board. `team` is `None` for the Takımsız bucket, whose
/// drop target posts to member/remove instead of member.
fn formation_column(
    team: Option<&HarnessTeam>,
    kids: &[&FormationKid],
    busy: bool,
    student_opts: &str,
) -> String {
    // A <button> rather than a <div>: focusable and tab-ordered for free, which is the
    // only keyboard path onto a card. The <select> fallback below is the real one.
    let cards: String = kids
        .iter()
        .map(|k| {
            format!(
                r#"<button type="button" class="kid" draggable="true" data-user="{id}">{name}</button>"#,
                id = k.id,
                name = esc(&k.display_name)
            )
        })
        .collect();
    let empty = if kids.is_empty() {
        r#"<p class="fempty">Buraya sürükle</p>"#
    } else {
        ""
    };
    let (id_attr, head) = match team {
        None => (
            String::new(),
            format!(
                r##"<div class="fhead">
    <h2>Takımsız</h2>
    <span class="item-meta">{n} öğrenci · gönderim yapamaz</span>
  </div>"##,
                n = kids.len()
            ),
        ),
        Some(t) => {
            // Moving a kid out mid-run is allowed on purpose — the run belongs to the
            // team. The badge is so it isn't a surprise.
            let badge = if busy {
                r#"<span class="substatus st-reviewing">çalışıyor</span>"#
            } else {
                ""
            };
            (
                format!(r#" data-team="{}""#, t.id),
                format!(
                    r##"<div class="fhead">
    <form method="post" action="/admin/takimlar/team/rename" class="inline frename">
      <input type="hidden" name="id" value="{id}">
      <input name="name" value="{name}" required aria-label="Takım adı">
      <button class="btn-dark small">Kaydet</button>
    </form>
    {badge}
    <form method="post" action="/admin/takimlar/team/delete" class="inline" onsubmit="return confirm('Bu takımı silersen tüm çalıştırmaları ve skorları da silinir. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>
  </div>
  <span class="item-meta">{n} öğrenci</span>"##,
                    id = t.id,
                    name = esc(&t.name),
                    n = kids.len()
                ),
            )
        }
    };
    // Same-column assign form per team, so the no-JS and touch paths never need the
    // team dropdown the board replaced.
    let assign = match team {
        None => String::new(),
        Some(t) => format!(
            r#"<form method="post" action="/admin/takimlar/member" class="inline fassign">
    <input type="hidden" name="team_id" value="{id}">
    <select name="user_id" aria-label="Öğrenci">{student_opts}</select>
    <button class="btn-dark small">Ata</button>
  </form>"#,
            id = t.id
        ),
    };
    format!(
        r##"<section class="fcol"{id_attr}>
  {head}
  <div class="fkids">{cards}{empty}</div>
  {assign}
</section>"##
    )
}

/// Takım formasyonu: the admin drag-and-drop board for Agentic Harness teams.
/// Harness only — AI Monopoly keeps its own rosters and its own panel on /admin.
pub fn teams_page(
    user: &User,
    teams: &[HarnessTeam],
    kids: &[FormationKid],
    busy: &[Uuid],
    saved: bool,
) -> String {
    let student_opts: String = kids
        .iter()
        .map(|k| {
            format!(
                r#"<option value="{}">{}</option>"#,
                k.id,
                esc(&k.display_name)
            )
        })
        .collect();
    let unassigned: Vec<&FormationKid> = kids.iter().filter(|k| k.team_id.is_none()).collect();
    let columns: String = std::iter::once(formation_column(None, &unassigned, false, ""))
        .chain(teams.iter().map(|t| {
            let mine: Vec<&FormationKid> =
                kids.iter().filter(|k| k.team_id == Some(t.id)).collect();
            formation_column(Some(t), &mine, busy.contains(&t.id), &student_opts)
        }))
        .collect();
    let flash = if saved {
        r#"<p class="fsaved">Kaydedildi ✓</p>"#
    } else {
        ""
    };
    layout(
        "Takım formasyonu",
        Some(user),
        "teams",
        // .formation is the only hook this page's stylesheet matches, and the <link> rides
        // in the content so layout() stays byte-identical for every other page — same
        // arrangement harness.css uses.
        &format!(
            r##"<div class="formation" id="formation-root">
<link rel="stylesheet" href="/static/teams.css?v=1">
<h1 class="pagetitle">Takım formasyonu</h1>
<p class="muted">Öğrencileri sürükleyip takımlara bırak. Bir öğrenci aynı anda tek takımda
olabilir; sürüklemek takımını değiştirir. Takımı olmayan öğrenci gönderim yapamaz.</p>
<p class="fieldnote">Bir takımın çalıştırması sürerken öğrenci taşımak serbest — çalıştırma
takımın, gönderen kişinin değil, bu yüzden durmaz.</p>
{flash}
<form method="post" action="/admin/takimlar/team" class="inline fcreate">
  <input name="name" placeholder="Takım adı" required aria-label="Takım adı">
  <button class="btn-dark">Takım oluştur</button>
</form>
<div class="fboard">{columns}</div>
<script src="/static/teams.js?v=1" defer></script>
</div>"##
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(nick: &str, live: &str, embeddable: Option<bool>) -> SiteCard {
        SiteCard {
            id: Uuid::nil(),
            nickname: nick.into(),
            repo_url: "https://github.com/a/b".into(),
            live_url: live.into(),
            live_embeddable: embeddable,
        }
    }

    /// The gallery's one real branch: an embeddable site is framed live, anything else
    /// falls back to the cached screenshot. Getting this backwards renders a blank box.
    #[test]
    fn gallery_frames_embeddable_and_screenshots_the_rest() {
        let user = User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        };
        let task = Task {
            id: Uuid::nil(),
            title: "Kişisel Website".into(),
            description: "d".into(),
            level: "PRESEED".into(),
            example_url: None,
            example_embeddable: None,
        };
        let html = board_sites(
            &user,
            &task,
            &[
                card("canli", "https://canli.vercel.app", Some(true)),
                card("bloke", "https://bloke.vercel.app", Some(false)),
            ],
        );
        assert!(
            html.contains(r#"<iframe src="https://canli.vercel.app""#),
            "embeddable site should be framed"
        );
        assert!(
            !html.contains("https://bloke.vercel.app\" loading"),
            "blocked site must not be framed"
        );
        assert!(
            html.contains(&format!("/preview/sub/{}", Uuid::nil())),
            "blocked site should fall back to the screenshot"
        );
        // both are still reachable as links, and the nicknames are shown
        assert!(html.contains(r#"href="https://bloke.vercel.app""#));
        assert!(html.contains("<h3>canli</h3>") && html.contains("<h3>bloke</h3>"));
    }

    #[test]
    fn beginner_projects_links_cheat_sheet() {
        let user = User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        };
        let html = beginner_projects(&user, &[]);
        assert!(
            html.contains(r#"href="/static/beginner-projects/vibe-coding-cheat-sheet.pdf""#),
            "beginner track should link the vibe coding cheat sheet"
        );
        // A project's own handout hangs off its card, alongside the brief.
        assert!(
            html.contains(r#"href="/static/beginner-projects/07-smart-receipt.pdf""#)
                && html.contains(
                    r#"href="/static/beginner-projects/07-google-apps-script-cheat-sheet.pdf""#
                ),
            "proje 7 should link both its brief and its apps script cheat sheet"
        );
    }

    /// The hub is two peer hubcards (projects, chatbot), same shape as
    /// advanced_track()'s two-hubcard layout — not a floating card above a flat list.
    #[test]
    fn beginner_track_hub_has_two_subsets() {
        let user = User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        };
        let html = beginner_track(&user, 3, 2);
        assert!(html.contains(r#"href="/beginner-track/projects""#));
        assert!(html.contains(r#"href="/chatbot-challenge""#));
        assert_eq!(html.matches("hubcard").count(), 2, "exactly two peer subsets");
        assert!(html.contains("3/7"));
        assert!(html.contains("seviye 2/7"));
    }

    #[test]
    fn gallery_empty_state() {
        let user = User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        };
        let task = Task {
            id: Uuid::nil(),
            title: "T".into(),
            description: "d".into(),
            level: "PRESEED".into(),
            example_url: None,
            example_embeddable: None,
        };
        assert!(board_sites(&user, &task, &[]).contains("henüz yayınlanmış site yok"));
    }

    #[test]
    fn harness_submission_lists_cerebras_and_deepinfra_for_students() {
        let user = User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        };
        let team = HarnessTeam {
            id: Uuid::nil(),
            name: "Test".into(),
        };
        let html = agentic_harness_main(&user, "arc", Some(&team), &[], None, None, None, &[], &[]);
        assert!(html.contains("<label>model:"));
        assert!(html.contains(r#"<select name="model_id" required>"#));
        assert!(html.contains(r#"name="benchmark_kind" value="arc""#));
        // type=text, not type=url: the browser's own validation fired before the POST and
        // showed a native, untranslatable bubble for links the server now accepts.
        assert!(html.contains(r#"name="repo_url" type="text" inputmode="url""#));
        assert!(!html.contains(r#"name="builtin_harness""#));
        assert!(html.contains(r#"<select name="provider""#));
        // Two provider options plus every Cerebras and DeepInfra model, nothing from Bedrock.
        assert_eq!(
            html.matches("<option ").count(),
            2 + CEREBRAS_MODEL_IDS.len() + DEEPINFRA_MODEL_IDS.len()
        );
        assert!(html.contains(r#"<option value="cerebras" selected>Cerebras</option>"#));
        assert!(html.contains(r#"<option value="deepinfra">DeepInfra</option>"#));
        assert!(!html.contains(r#">Bedrock</option>"#));
        assert!(!html.contains(r#"data-provider="bedrock""#));
        assert!(html.contains(&format!(
            r#"<option value="{DEFAULT_CEREBRAS_MODEL}" data-provider="cerebras" selected data-image="true">"#
        )));
        // DeepInfra's models come in disabled until the provider select switches to them.
        assert!(html.contains(&format!(
            r#"<option value="{DEFAULT_DEEPINFRA_MODEL}" data-provider="deepinfra" disabled data-image="true">"#
        )));
        assert!(html.contains(
            r#"<option value="zai-org/GLM-5.2" data-provider="deepinfra" disabled>zai-org/GLM-5.2 · Kaggle RTX 6000 üzerinde çalışmayabilir</option>"#
        ));
    }

    #[test]
    fn harness_builtin_picker_is_admin_only() {
        let admin = User {
            id: Uuid::nil(),
            display_name: "Admin".into(),
            nickname: None,
            is_admin: true,
            level: "PRESEED".into(),
        };
        let team = HarnessTeam {
            id: Uuid::nil(),
            name: "Test".into(),
        };
        // The pickers moved off the student page onto /admin/harness, so assert them
        // where they now live — and assert the student page no longer carries them.
        let html = admin_harness_page(&admin, "arc", Some(&team), &[], None);
        assert!(
            !agentic_harness_main(&admin, "arc", Some(&team), &[], None, None, None, &[], &[])
                .contains("<label>agent:")
        );
        assert!(html.contains("<label>agent:"));
        assert!(html.contains(r#"<select name="provider""#));
        assert!(html.contains(r#"<option value="cerebras" selected>Cerebras</option>"#));
        assert!(html.contains(r#"<option value="bedrock">Bedrock</option>"#));
        assert!(html.contains(r#"<option value="deepinfra">DeepInfra</option>"#));
        assert!(html.contains(
            r#"<option value="Qwen/Qwen3.6-27B" data-provider="deepinfra" disabled data-image="true">"#
        ));
        assert!(html.contains(r#"<select name="builtin_harness""#));
        assert!(html.contains(r#"<option value="forge">Forge</option>"#));
        assert!(html.contains(r#"<option value="reki">Reki</option>"#));
        assert!(html.contains(
            r#"value="google.gemma-4-31b" data-provider="bedrock" disabled data-image="true""#
        ));
        assert!(!html.contains(
            r#"value="openai.gpt-oss-120b" data-provider="bedrock" disabled data-image="true""#
        ));
        assert!(!html.contains(r#"placeholder="https://github.com/..." required"#));
    }

    fn admin_user() -> User {
        User {
            id: Uuid::nil(),
            display_name: "Admin".into(),
            nickname: None,
            is_admin: true,
            level: "PRESEED".into(),
        }
    }

    fn team_at(n: u128, name: &str) -> HarnessTeam {
        HarnessTeam {
            id: Uuid::from_u128(n),
            name: name.into(),
        }
    }

    fn kid(n: u128, name: &str, team: Option<u128>) -> FormationKid {
        FormationKid {
            id: Uuid::from_u128(n),
            display_name: name.into(),
            team_id: team.map(Uuid::from_u128),
        }
    }

    #[test]
    fn formation_board_puts_every_kid_in_exactly_one_column() {
        let teams = [team_at(1, "Alfa"), team_at(2, "Beta")];
        let kids = [
            kid(10, "Ada", Some(1)),
            kid(11, "Bora", Some(1)),
            kid(12, "Cem", Some(2)),
            kid(13, "Deniz", None),
        ];
        let html = teams_page(&admin_user(), &teams, &kids, &[], false);
        // one card per kid, never two — a drop is a move, the DB PK guarantees it
        for name in ["Ada", "Bora", "Cem", "Deniz"] {
            assert_eq!(
                html.matches(&format!(
                    r#"draggable="true" data-user="{}""#,
                    kids.iter().find(|k| k.display_name == name).unwrap().id
                ))
                .count(),
                1,
                "{name} should appear on exactly one card"
            );
        }
        // three columns: Takımsız + two teams
        assert_eq!(html.matches(r#"<section class="fcol""#).count(), 3);
        assert!(html.contains("Takımsız"));
        assert!(html.contains("gönderim yapamaz"));
        // the unassigned bucket has no data-team, so its drop posts to member/remove
        assert!(html.contains(r#"<section class="fcol">"#));
        assert!(html.contains(r#"action="/admin/takimlar/team/rename""#));
    }

    #[test]
    fn formation_board_survives_an_empty_database() {
        let html = teams_page(&admin_user(), &[], &[], &[], false);
        assert_eq!(html.matches(r#"<section class="fcol""#).count(), 1);
        assert!(html.contains("Buraya sürükle"));
        // create still reachable with nothing on the board
        assert!(html.contains(r#"action="/admin/takimlar/team""#));
    }

    #[test]
    fn formation_board_flags_a_team_with_a_live_run() {
        let teams = [team_at(1, "Alfa"), team_at(2, "Beta")];
        let busy = [Uuid::from_u128(1)];
        let html = teams_page(
            &admin_user(),
            &teams,
            &[kid(10, "Ada", Some(1))],
            &busy,
            true,
        );
        assert_eq!(html.matches("çalışıyor").count(), 1);
        assert!(html.contains("Kaydedildi ✓"));
    }

    fn running_run() -> HarnessRun {
        HarnessRun {
            id: Uuid::from_u128(7),
            repo_url: "https://github.com/kid/agent".into(),
            model_id: "m".into(),
            provider: "cerebras".into(),
            benchmark_kind: "arc".into(),
            commit_sha: None,
            stage: "running".into(),
            benchmark_version: "v3".into(),
            benchmark_state: serde_json::Value::Null,
            bedrock_profile: None,
            deadline_at: None,
            score_arc: None,
            score_frontier: None,
            ram_1session_mb: None,
            ram_10session_mb: None,
            error_log: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn a_live_run_always_offers_a_watch_link() {
        let html = harness_stepper(&running_run(), Some("Ada"), false);
        assert!(html.contains(r#"href="/agentic-harness?tab=live""#));
        assert!(html.contains("Canlı izle"));
        assert!(html.contains("gönderen: Ada"));
        // no warning unless this student actually collided with the run
        assert!(!html.contains("harness-busy"));
    }

    #[test]
    fn a_blocked_submit_names_the_teammate_and_links_to_the_live_tab() {
        let html = harness_stepper(&running_run(), Some("Ada"), true);
        assert!(html.contains("harness-busy"));
        assert!(html.contains("Ada zaten bir çalıştırma başlattı."));
        assert!(html.contains(r#"href="/agentic-harness?tab=live""#));
        // submitted_by is `on delete set null`, so the name can be gone
        let anon = harness_stepper(&running_run(), None, true);
        assert!(anon.contains("Takımından biri zaten bir çalıştırma başlattı."));
    }

    #[test]
    fn the_team_name_is_the_input_with_no_button_to_press() {
        let user = User {
            id: Uuid::nil(),
            display_name: "Kid".into(),
            nickname: None,
            is_admin: false,
            level: "PRESEED".into(),
        };
        let team = team_at(1, "Neural Ninjas");
        let html = agentic_harness_main(&user, "arc", Some(&team), &[], None, None, None, &[], &[]);
        assert!(html.contains(r#"action="/agentic-harness/team/name""#));
        assert!(html.contains(r#"value="Neural Ninjas""#));
        assert!(html.contains(&format!(r#"maxlength="{HARNESS_TEAM_NAME_MAX}""#)));
        // exactly one field and no submit button is what makes Enter submit it
        let form = &html[html.find(r#"class="teamname""#).unwrap()..];
        let form = &form[..form.find("</form>").unwrap()];
        assert_eq!(form.matches("<input").count(), 1);
        assert!(!form.contains("<button"));
        // nothing to say until something happened
        assert!(!html.contains("teamname-note"));
    }

    #[test]
    fn a_name_clash_is_explained_next_to_the_field() {
        let user = User {
            id: Uuid::nil(),
            display_name: "Kid".into(),
            nickname: None,
            is_admin: false,
            level: "PRESEED".into(),
        };
        let team = team_at(1, "Neural Ninjas");
        let taken = agentic_harness_main(
            &user,
            "arc",
            Some(&team),
            &[],
            None,
            None,
            Some("name-taken"),
            &[],
            &[],
        );
        assert!(taken.contains("başka bir takımda kullanılıyor"));
        let ok = agentic_harness_main(
            &user,
            "arc",
            Some(&team),
            &[],
            None,
            None,
            Some("named"),
            &[],
            &[],
        );
        assert!(ok.contains("Takım adı güncellendi"));
        // a rename note must never be mistaken for the busy warning and vice versa
        assert!(!ok.contains("harness-busy"));
    }

    #[test]
    fn a_team_less_student_is_told_submission_is_blocked() {
        let user = User {
            id: Uuid::nil(),
            display_name: "Kid".into(),
            nickname: None,
            is_admin: false,
            level: "PRESEED".into(),
        };
        let html = agentic_harness_main(&user, "arc", None, &[], None, None, None, &[], &[]);
        assert!(html.contains("gönderim yapamazsın"));
        assert!(!html.contains(r#"action="/agentic-harness/submit""#));
    }

    #[test]
    fn harness_stop_form_targets_the_active_run() {
        let run_id = Uuid::parse_str("018f0f65-9abc-7def-8123-456789abcdef").unwrap();
        let html = harness_stop_form(run_id);
        assert!(html.contains(r#"action="/agentic-harness/stop""#));
        assert!(html.contains(&format!(r#"name="id" value="{run_id}""#)));
        assert!(html.contains("Durdur"));
    }

    fn leader(n: u128, name: &str, best: f32) -> HarnessLeaderRow {
        HarnessLeaderRow {
            id: Uuid::from_u128(n),
            name: name.into(),
            best,
        }
    }

    fn viewer() -> User {
        User {
            id: Uuid::nil(),
            display_name: "A".into(),
            nickname: Some("a".into()),
            is_admin: false,
            level: "PRESEED".into(),
        }
    }

    /// The same viewer under the name the ported page tests were written against.
    fn student() -> User {
        viewer()
    }

    // ---- haftalık program ----

    fn image(track: &str) -> ScheduleImage {
        ScheduleImage {
            track: track.into(),
            content_type: "image/png".into(),
            uploaded_at: chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap(),
            bytes: 512_000,
        }
    }

    /// An unknown, blank or differently-cased ?track= still renders a page.
    #[test]
    fn track_is_resolved_leniently() {
        assert_eq!(valid_schedule_track(Some("advanced")), "advanced");
        assert_eq!(valid_schedule_track(Some("Advanced")), "advanced");
        assert_eq!(valid_schedule_track(Some(" BEGINNER ")), "beginner");
        for bad in [None, Some(""), Some("PRESEED"), Some("../../etc/passwd")] {
            assert_eq!(valid_schedule_track(bad), "beginner", "{bad:?}");
        }
    }

    /// The image URL carries the upload time, so replacing a screenshot busts the
    /// cached one instead of leaving students on last week's.
    #[test]
    fn uploaded_image_is_shown_and_versioned() {
        let img = image("advanced");
        let html = schedule(&student(), "advanced", Some(&img), &[]);
        assert!(html.contains(&format!(
            r#"src="/schedule/image/advanced?v={}""#,
            img.version()
        )));
        assert!(html.contains(r#"<a class="chip active" href="/schedule?track=advanced""#));
        assert!(!html.contains("sheet-empty"));
    }

    /// Nothing uploaded yet is a normal state, not an error or a broken <img>.
    #[test]
    fn missing_image_says_so() {
        let html = schedule(&student(), "beginner", None, &[]);
        assert!(html.contains("henüz yüklenmedi"));
        // the shell has its own <img> (the logo), so check for the image route itself
        assert!(
            !html.contains("/schedule/image/"),
            "no <img> pointing at a 404"
        );
    }

    /// Every track gets an upload slot; the one already on file also gets a Kaldır.
    #[test]
    fn admin_panel_has_a_slot_per_track() {
        let panel = admin_schedule_panel(&[image("beginner")]);
        for (key, _) in SCHEDULE_TRACKS {
            assert!(
                panel.contains(&format!(
                    r#"<input type="hidden" name="track" value="{key}">"#
                )),
                "{key}"
            );
        }
        assert!(
            panel.contains("/admin/schedule/delete"),
            "beginner is on file, so it can be removed"
        );
        assert!(panel.contains(r#"enctype="multipart/form-data""#));
    }

    // ---- konum / adres ----

    fn venue(week: u8) -> Venue {
        Venue {
            week,
            dates: "3–7 Ağustos".into(),
            name: "Kolektif House Levent".into(),
            address: "Esentepe Mah.\nŞişli/İstanbul".into(),
            maps_url: "https://maps.app.goo.gl/abc".into(),
            notes: "3. kat · kapı kodu 1234".into(),
        }
    }

    fn blank(week: u8) -> Venue {
        Venue {
            week,
            ..Venue::default()
        }
    }

    /// The same cards render on their own page and under the schedule, so students
    /// can't be shown two different addresses.
    #[test]
    fn venue_cards_are_shared_by_both_pages() {
        let venues = [venue(1), venue(2)];
        let page = location(&student(), &venues);
        let sched = schedule(&student(), "beginner", None, &venues);
        for html in [&page, &sched] {
            assert!(html.contains("Kolektif House Levent"));
            assert!(html.contains("3. kat · kapı kodu 1234"));
            assert!(html.contains(r#"href="https://maps.app.goo.gl/abc""#));
        }
        assert!(sched.contains("venue-head"), "schedule labels the section");
    }

    /// Every card names its week — the two weeks are in different buildings, so an
    /// address that doesn't say which week it belongs to is worse than none.
    #[test]
    fn every_card_names_its_week() {
        let html = location(&student(), &[venue(1), venue(2)]);
        assert!(html.contains("1. Hafta · 3–7 Ağustos"));
        assert!(html.contains("2. Hafta · 3–7 Ağustos"));
        // dates are optional; the week number is not
        assert_eq!(blank(2).heading(), "2. Hafta");
        assert_eq!(
            Venue {
                week: 1,
                dates: " 3–7 Ağustos ".into(),
                ..Venue::default()
            }
            .heading(),
            "1. Hafta · 3–7 Ağustos"
        );
    }

    /// A week with no address yet is still listed by name on /location, so a missing
    /// second week reads as "not announced" rather than "there is no second week".
    /// Beside the schedule it stays silent instead of adding an empty box.
    #[test]
    fn a_week_without_an_address_is_named_not_dropped() {
        let html = location(&student(), &[venue(1), blank(2)]);
        assert!(html.contains("venue-pending"));
        assert!(html.contains("2. Hafta"));
        assert!(html.contains("Adres henüz açıklanmadı"));

        let none = schedule(&student(), "beginner", None, &[blank(1), blank(2)]);
        assert!(
            !none.contains("venue-card"),
            "no empty card beside the schedule"
        );
        assert!(!none.contains("venue-head"));
    }

    /// A blank field is left out rather than rendered as an empty element.
    #[test]
    fn empty_venue_fields_are_omitted() {
        let partial = Venue {
            week: 1,
            maps_url: "https://maps.app.goo.gl/x".into(),
            ..Venue::default()
        };
        let html = location(&student(), &[partial]);
        assert!(html.contains("maps.app.goo.gl/x"));
        assert!(!html.contains("<h3>"), "no empty name heading");
        assert!(!html.contains("venue-notes"), "no empty notes block");
        assert!(blank(1).is_empty());
    }

    /// Whatever the admin typed is escaped on the way into the card and the href.
    #[test]
    fn venue_text_is_escaped() {
        let v = Venue {
            week: 1,
            name: r#"<script>alert(1)</script>"#.into(),
            ..Venue::default()
        };
        let html = location(&student(), &[v]);
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    /// Both weeks get their own form, each posting its own week number.
    #[test]
    fn admin_panel_has_a_form_per_week() {
        let panel = admin_venue_panel(&[blank(1), blank(2)]);
        for week in VENUE_WEEKS {
            assert!(
                panel.contains(&format!(
                    r#"<input type="hidden" name="week" value="{week}">"#
                )),
                "{week}"
            );
        }
        assert_eq!(panel.matches("/admin/venue").count(), VENUE_WEEKS.len());
    }

    // ---- veli onay formları ----

    fn doc(kind: &str, name: &str) -> ConsentDoc {
        ConsentDoc {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            kind: kind.into(),
            filename: name.into(),
            bytes: 2_400_000,
            uploaded_at: chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap(),
        }
    }

    /// The state a fresh database is in — every form open, now that all six have a
    /// document behind them.
    fn default_locks() -> Vec<(&'static str, bool)> {
        CONSENT_DOCS
            .iter()
            .map(|(k, ..)| (*k, CONSENT_LOCKED_BY_DEFAULT.contains(k)))
            .collect()
    }

    /// The blank-form links as they come out of CONSENT_DOCS.
    fn test_urls() -> Vec<(&'static str, String)> {
        CONSENT_DOCS
            .iter()
            .map(|(k, _, _, u)| (*k, u.to_string()))
            .collect()
    }

    fn member(name: &str, id: uuid::Uuid) -> MemberRow {
        MemberRow {
            id,
            display_name: name.into(),
            email: format!("{name}@ornek.com"),
            nickname: Some(name.into()),
            is_admin: false,
            hidden_from_leaderboard: false,
        }
    }

    /// Open forms get a real file input; a locked one gets none at all — the blurred
    /// card is decoration, so there is nothing for a student to click or a script to
    /// find and post to.
    #[test]
    fn locked_form_is_blurred_and_has_no_input() {
        // one form closed by hand from /admin: nothing ships closed any more
        let locks: Vec<(&str, bool)> = CONSENT_DOCS
            .iter()
            .map(|(k, ..)| (*k, *k == "paribu_veli_riza"))
            .collect();
        let html = documents(&student(), &[], &locks, &test_urls(), None, None);
        assert!(html.contains("doc-blur") && html.contains("doc-lockmsg"));
        assert!(html.contains("Bu form henüz hazır değil"));
        // every form but the closed one gets an upload control
        let open = CONSENT_DOCS.len() - 1;
        assert_eq!(html.matches(r#"action="/documents/upload""#).count(), open);
        assert_eq!(html.matches(r#"name="files""#).count(), open);
        assert_eq!(html.matches(r#"value="paribu_veli_riza""#).count(), 0);
        // and it is still named, so a student knows it is coming
        assert!(html.contains("Paribu · Veli/Vasi Açık Rıza Metni"));
        assert!(html.contains("Yakında"));
    }

    /// A fresh database opens every form — the Paribu placeholder that used to ship
    /// blurred has been replaced by four documents that actually exist.
    #[test]
    fn nothing_ships_locked_any_more() {
        let html = documents(&student(), &[], &default_locks(), &test_urls(), None, None);
        assert!(!html.contains("doc-lockmsg"));
        assert_eq!(
            html.matches(r#"action="/documents/upload""#).count(),
            CONSENT_DOCS.len()
        );
    }

    /// The four Paribu documents are each their own card, each pointing at the PDF that
    /// ships in static/ — so a student can tell which two they sign themselves and which
    /// two go to a parent, and the admin grid tracks the four separately.
    #[test]
    fn the_four_paribu_documents_are_separate_forms() {
        let html = documents(&student(), &[], &default_locks(), &test_urls(), None, None);
        for (kind, file) in [
            ("paribu_katilimci_aydinlatma", "katilimci-aydinlatma-metni"),
            ("paribu_katilimci_riza", "katilimci-acik-riza-metni"),
            ("paribu_veli_aydinlatma", "veli-vasi-aydinlatma-metni"),
            ("paribu_veli_riza", "veli-vasi-acik-riza-metni"),
        ] {
            assert!(
                html.contains(&format!(r#"name="kind" value="{kind}""#)),
                "{kind} has no upload bucket"
            );
            assert!(
                html.contains(&format!(
                    r#"href="/static/consent/paribu-{file}.pdf" download"#
                )),
                "{kind} does not offer its PDF as a download"
            );
        }
        // the placeholder is gone, both as a card and as a kind a POST could name
        assert!(!html.contains("Paribu Lokasyon/Katılım İzin Formu"));
        assert!(!html.contains(r#"value="paribu""#));
        assert!(valid_consent_kind("paribu").is_none());
        // a same-origin path is a path, not a protocol-relative jump off the origin
        assert!(same_origin_path("/static/consent/x.pdf"));
        assert!(!same_origin_path("//evil.example/x.pdf"));
        assert!(!same_origin_path("https://evil.example/x.pdf"));
    }

    /// Every form in CONSENT_DOCS has a card, uploads are multipart, and the deadline
    /// is stated once, from the constant.
    #[test]
    fn every_form_gets_a_card() {
        let locks: Vec<(&str, bool)> = CONSENT_DOCS.iter().map(|(k, ..)| (*k, false)).collect();
        let html = documents(&student(), &[], &locks, &test_urls(), None, None);
        for (kind, title, ..) in CONSENT_DOCS {
            assert!(
                html.contains(&format!(
                    r#"<input type="hidden" name="kind" value="{kind}">"#
                )),
                "{kind}"
            );
            assert!(html.contains(&esc(title)), "{kind}");
        }
        assert!(html.contains(r#"enctype="multipart/form-data""#));
        assert!(
            html.contains("multiple"),
            "several pages can be picked at once"
        );
        assert!(html.contains(CONSENT_DEADLINE));
    }

    /// Uploaded files are listed back, downloadable, and removable while the form is
    /// open — a student has to be able to see what actually arrived.
    #[test]
    fn uploaded_files_are_listed_with_a_download_and_a_delete() {
        let d = doc("exposure", "veli-onay-1.pdf");
        let id = d.id;
        let html = documents(&student(), &[d], &default_locks(), &test_urls(), None, None);
        assert!(html.contains(&format!(r#"href="/documents/file/{id}""#)));
        assert!(html.contains("veli-onay-1.pdf"));
        assert!(html.contains(r#"action="/documents/delete""#));
        assert!(html.contains("2.3 MB"));
        assert!(html.contains("doc-st-done"), "the card reads as done");
        // the still-empty QNBEYOND card says so rather than showing an empty list
        assert!(html.contains("Henüz bir dosya yüklemedin"));
    }

    /// A closed form is closed both ways: no upload control and no delete button, so a
    /// collected set of files can't move once the admin closes it.
    #[test]
    fn a_closed_form_cannot_be_edited() {
        let d = doc("qnbeyond", "izin.jpg");
        // everything closed but exposure — including qnbeyond, which is the one with a file
        let locks: Vec<(&str, bool)> = CONSENT_DOCS
            .iter()
            .map(|(k, ..)| (*k, *k != "exposure"))
            .collect();
        let html = documents(&student(), &[d], &locks, &test_urls(), None, None);
        assert_eq!(html.matches(r#"action="/documents/delete""#).count(), 0);
        assert_eq!(html.matches(r#"action="/documents/upload""#).count(), 1);
    }

    /// The file name a student picked is escaped on the way into the list and into the
    /// confirm() string — it is the one piece of this page they control.
    #[test]
    fn student_filenames_are_escaped() {
        let d = doc("exposure", r#"<img src=x onerror=alert(1)>.pdf"#);
        let html = documents(
            &student(),
            &[d],
            &[("exposure", false)],
            &test_urls(),
            None,
            None,
        );
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;.pdf"));
    }

    /// The admin grid says who handed in what, links every file, and offers the whole
    /// set as one download.
    #[test]
    fn admin_grid_shows_who_uploaded_what() {
        let (a, b) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let mut d = doc("exposure", "ada.pdf");
        d.user_id = a;
        let did = d.id;
        let members = [member("ada", a), member("bora", b)];
        // one form closed, so the panel has to show both sides of the switch
        let locks: Vec<(&str, bool)> = CONSENT_DOCS
            .iter()
            .map(|(k, ..)| (*k, *k == "paribu_veli_riza"))
            .collect();
        let panel = admin_consent_panel(&members, &[d], &locks, &test_urls());
        assert!(panel.contains("/admin/documents.zip"));
        assert!(panel.contains(&format!(r#"href="/documents/file/{did}""#)));
        assert!(
            panel.contains("1/2 öğrenci yükledi"),
            "one of two students is in"
        );
        assert!(panel.contains("consent-missing"), "bora's cell is empty");
        // every form gets an open/close switch, and the closed one says so
        assert_eq!(
            panel.matches(r#"action="/admin/documents/lock""#).count(),
            CONSENT_DOCS.len()
        );
        assert!(panel.contains("Yüklemeye aç") && panel.contains("Yüklemeyi kapat"));
    }

    /// Each open form offers the blank document two ways — a direct download and the
    /// Drive preview — and a form with no link yet simply has no button.
    #[test]
    fn the_blank_form_is_downloadable_from_the_card() {
        let html = documents(&student(), &[], &default_locks(), &test_urls(), None, None);
        assert_eq!(
            html.matches("doc-getbtn").count(),
            CONSENT_DOCS.len(),
            "every form has a document behind it now"
        );
        // the /view share link becomes a link that actually downloads
        assert!(html.contains(
            "https://drive.google.com/uc?export=download&amp;id=1gkxQLuguXfVFmjjjjcBF0Y39JTKeFXr6"
        ));
        assert!(html.contains(
            "https://drive.google.com/uc?export=download&amp;id=10YgFIm28qjhTy3stEXS5BukS_tmn-9_a"
        ));
        // …with the original preview URL still reachable beside it
        assert!(html.contains("/file/d/1gkxQLuguXfVFmjjjjcBF0Y39JTKeFXr6/view"));
        assert!(html.contains("Formu indir"));

        // an open form with no link yet shows no button rather than a dead one
        let no_link: Vec<(&str, String)> = CONSENT_DOCS
            .iter()
            .map(|(k, ..)| (*k, String::new()))
            .collect();
        let bare = documents(&student(), &[], &default_locks(), &no_link, None, None);
        assert!(!bare.contains("doc-getbtn"));
    }

    /// Drive links of either shape become downloads; anything else is left alone.
    #[test]
    fn drive_links_become_downloads() {
        assert_eq!(
            direct_download_url("https://drive.google.com/file/d/ABC123/view?usp=sharing"),
            "https://drive.google.com/uc?export=download&id=ABC123"
        );
        assert_eq!(
            direct_download_url("https://drive.google.com/open?id=ABC123"),
            "https://drive.google.com/uc?export=download&id=ABC123"
        );
        // not Drive, or Drive in a shape we don't recognise: untouched
        assert_eq!(
            direct_download_url("https://example.com/form.pdf"),
            "https://example.com/form.pdf"
        );
        assert_eq!(
            direct_download_url("https://drive.google.com/drive/folders/XYZ"),
            "https://drive.google.com/drive/folders/XYZ"
        );
    }

    /// The admin can retarget any form's link, including Paribu's, from the panel.
    #[test]
    fn admin_can_set_each_form_link() {
        let panel = admin_consent_panel(
            &[member("ada", uuid::Uuid::new_v4())],
            &[],
            &default_locks(),
            &test_urls(),
        );
        assert_eq!(
            panel.matches(r#"action="/admin/documents/link""#).count(),
            CONSENT_DOCS.len()
        );
        assert!(
            panel.contains("1gkxQLuguXfVFmjjjjcBF0Y39JTKeFXr6"),
            "current link is pre-filled"
        );
    }

    /// Admins hand nothing in, so they are not rows in the collection grid.
    #[test]
    fn admins_are_not_chased_for_forms() {
        let mut boss = member("onur", uuid::Uuid::new_v4());
        boss.is_admin = true;
        let panel = admin_consent_panel(
            &[boss, member("ada", uuid::Uuid::new_v4())],
            &[],
            &default_locks(),
            &test_urls(),
        );
        assert!(!panel.contains("onur@ornek.com"));
        assert!(panel.contains("0/1 öğrenci yükledi"));
    }

    /// Ana Sayfa leads with the missing forms while any open one is outstanding, and
    /// stops nagging once they are all in.
    #[test]
    fn home_nags_only_while_a_form_is_missing() {
        let missing = home(&student(), 0, 10, 3, 0, None, 1, 2);
        assert!(
            missing.contains("alertbar") && missing.contains("Veli onay formların eksik (1/2)")
        );
        assert!(missing.contains(CONSENT_DEADLINE));
        let done = home(&student(), 0, 10, 3, 0, None, 2, 2);
        assert!(!done.contains("alertbar"));
        assert!(
            done.contains("2/2 form yüklendi"),
            "the card still shows the state"
        );
    }

    /// The placeholder must not leak the section it is standing in for: no tab strip, no
    /// submit form. It keeps the Advanced Track sidebar entry active so students can still
    /// find their way to it.
    #[test]
    fn monopoly_placeholder_shows_nothing_but_the_promise() {
        let page = monopoly_coming_soon(&viewer());
        assert!(page.contains("COMING SOON!"));
        assert!(!page.contains(r#"class="chips""#), "tab strip leaked");
        assert!(!page.contains("subform"), "submit form leaked");
        assert!(
            page.contains(r#"href="/advanced-track" class="active""#),
            "sidebar link dropped"
        );
    }

    /// The top three are drawn once, on the podium, and the list under it starts at 4.
    /// Showing a team in both places is the bug this replaced.
    #[test]
    fn harness_podium_takes_the_top_three_and_the_list_starts_at_four() {
        let rows = [
            leader(1, "Bir", 90.0),
            leader(2, "Iki", 80.0),
            leader(3, "Uc", 70.0),
            leader(4, "Dort", 60.0),
        ];
        let html = agentic_harness_main(&viewer(), "arc", None, &[], None, None, None, &rows, &[]);
        assert_eq!(html.matches(r#"<div class="pod p"#).count(), 3);
        assert_eq!(html.matches(r#"<div class="lbrow "#).count(), 1);
        assert!(html.contains(r#"<span class="lbrank">4</span>"#));
        for name in ["Bir", "Iki", "Uc", "Dort"] {
            assert_eq!(html.matches(name).count(), 1, "{name} rendered twice");
        }
    }

    /// dense_ranks_by keys on the display-rounded score, so a tie puts four rows at
    /// rank <= 3. Splitting on index instead of rank would podium two of the tied
    /// teams and leave the third in the list with the same number beside it.
    #[test]
    fn harness_podium_grows_with_a_tie_and_never_repeats_a_rank() {
        let rows = [
            leader(1, "Bir", 90.0),
            leader(2, "Iki", 80.0),
            leader(3, "Uc", 70.04),
            leader(4, "Dort", 70.0), // both render 70.0 -> both rank 3
            leader(5, "Bes", 60.0),
        ];
        let html = agentic_harness_main(&viewer(), "arc", None, &[], None, None, None, &rows, &[]);
        assert_eq!(html.matches(r#"<div class="pod p"#).count(), 4);
        assert!(html.contains(r#"<span class="lbrank">4</span>"#));
        assert!(!html.contains(r#"<span class="lbrank">3</span>"#));
    }

    /// Under three teams a "podium" is one lonely card, so the plain list stands in.
    #[test]
    fn harness_podium_is_skipped_for_a_short_board() {
        let rows = [leader(1, "Bir", 90.0), leader(2, "Iki", 80.0)];
        let html = agentic_harness_main(&viewer(), "arc", None, &[], None, None, None, &rows, &[]);
        assert!(!html.contains(r#"class="pod p"#));
        assert!(html.contains(r#"<span class="lbrank">1</span>"#));
    }

    /// Dumps every harness screen to $HARNESS_RENDER_DIR for a browser pass — these
    /// pages need no database, so a layout regression is checkable without a server.
    /// `HARNESS_RENDER_DIR=/tmp/h cargo test -p academy -- --ignored render_harness`
    #[test]
    #[ignore]
    fn render_harness_pages() {
        let Ok(dir) = std::env::var("HARNESS_RENDER_DIR") else {
            panic!("set HARNESS_RENDER_DIR to the output directory");
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let user = viewer();
        let team = HarnessTeam {
            id: Uuid::from_u128(9),
            name: "Test Takımı".into(),
        };
        let members = [
            TeamMemberRow {
                team_id: team.id,
                user_id: Uuid::from_u128(1),
                display_name: "Berke Arslan".into(),
                public: true,
            },
            TeamMemberRow {
                team_id: team.id,
                user_id: Uuid::from_u128(2),
                display_name: "Onur Çelik".into(),
                public: true,
            },
        ];
        let run = HarnessRun {
            id: Uuid::from_u128(7),
            repo_url: "forge".into(),
            model_id: "google.gemma-4-31b".into(),
            provider: "bedrock".into(),
            benchmark_kind: "bundled".into(),
            commit_sha: Some("be0f06c1d2e3f4a5".into()),
            stage: "running".into(),
            benchmark_version: "v2".into(),
            benchmark_state: serde_json::json!({
                "arc": {"status": "running", "done": 8, "total": 25, "score": 32.0},
                "frontier": {"status": "pending"},
                "ram": {"status": "done", "one_session_mb": 118.4, "ten_session_mb": 942.1},
            }),
            bedrock_profile: Some("eu-central-1".into()),
            deadline_at: Some(chrono::Utc::now() + chrono::Duration::minutes(7)),
            score_arc: Some(32.0),
            score_frontier: None,
            ram_1session_mb: Some(118.4),
            ram_10session_mb: Some(942.1),
            error_log: None,
            created_at: chrono::Utc::now(),
        };
        let rows = [
            leader(1, "Kuantum Kediler", 88.6),
            leader(2, "Devre Kırıcılar", 81.2),
            leader(9, "Test Takımı", 74.5),
            leader(4, "Piksel Avcıları", 68.0),
            leader(5, "Sonsuz Döngü", 61.3),
            leader(6, "Yarım Elma", 47.9),
        ];
        let pages: Vec<(&str, String)> = vec![
            (
                "main-idle",
                agentic_harness_main(
                    &user,
                    "arc",
                    Some(&team),
                    &members,
                    None,
                    None,
                    None,
                    &rows,
                    &[],
                ),
            ),
            (
                "main-running",
                agentic_harness_main(
                    &user,
                    "arc",
                    Some(&team),
                    &members,
                    Some(&run),
                    Some("Ada"),
                    Some("busy"),
                    &rows,
                    &[],
                ),
            ),
            (
                "main-empty",
                agentic_harness_main(&user, "arc", None, &[], None, None, None, &[], &[]),
            ),
            (
                "live-running",
                agentic_harness_live(&user, Some(&run), false),
            ),
            ("live-idle", agentic_harness_live(&user, None, false)),
            (
                "history",
                agentic_harness_history(&user, Some(&team), &[run], true, Some("test-takimi"), &[]),
            ),
            ("instructions", agentic_harness_instructions(&user)),
        ];
        for (name, html) in pages {
            std::fs::write(dir.join(format!("{name}.html")), html).unwrap();
        }
        eprintln!("wrote harness pages to {}", dir.display());
    }
}
