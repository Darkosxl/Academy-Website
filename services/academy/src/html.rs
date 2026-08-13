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
const P_BRIEFCASE: &str = "M20.25 14.15v4.073a2.25 2.25 0 0 1-1.976 2.233c-3.037.383-6.126.383-9.163 0a2.25 2.25 0 0 1-1.976-2.233V14.15M20.25 14.15c.313-.446.5-.99.5-1.575v-3.5a2.25 2.25 0 0 0-2.25-2.25h-12a2.25 2.25 0 0 0-2.25 2.25v3.5c0 .585.187 1.129.5 1.575M20.25 14.15a2.25 2.25 0 0 1-1.184.65 48.02 48.02 0 0 1-13.632 0 2.25 2.25 0 0 1-1.184-.65M15.75 6.825V5.25A2.25 2.25 0 0 0 13.5 3h-3a2.25 2.25 0 0 0-2.25 2.25v1.575";
const P_BEAKER: &str = "M9.75 3.104v5.714a2.25 2.25 0 0 1-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 0 1 4.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0 1 12 15a9.065 9.065 0 0 0-6.23-.693L5 14.5m14.8.8 1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A48.309 48.309 0 0 1 12 21c-2.773 0-5.491-.235-8.135-.687-1.718-.293-2.3-2.379-1.067-3.61L5 14.5";
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
                    r#"<div class="sb-head">Yönetim</div>{}{}{}{}{}{}"#,
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
                    ),
                    nav_link(
                        "/admin/monopoly",
                        active,
                        "monopoly-admin",
                        &ico(P_MONOPOLY),
                        "AI Monopoly (Admin)"
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
<link rel="stylesheet" href="/static/style.css?v=40">
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

fn monopoly_game_status_tr(status: &str) -> (&'static str, &'static str) {
    match status {
        "queued" => ("Sırada", "st-pending"),
        "leased" => ("Oynanıyor", "st-reviewing"),
        "done" => ("Tamamlandı", "st-passed"),
        "cancelled" => ("Durduruldu", "st-failed"),
        _ => ("Altyapı hatası", "st-failed"),
    }
}

/// AI Monopoly submission status (model.rs monopoly_submissions_exposure_academy.status).
fn monopoly_submission_status_tr(status: &str) -> (&'static str, &'static str) {
    match status {
        "pending" => ("Doğrulama sırasında", "st-pending"),
        "validating" => ("Repo hazırlanıyor", "st-reviewing"),
        "approved" => ("Onaylandı", "st-passed"),
        "disabled" => ("Devre dışı", "st-failed"),
        "rejected" => ("Reddedildi", "st-failed"),
        _ => ("Doğrulama başarısız", "st-failed"),
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

const MONOPOLY_TOURNAMENT_TABS: [(&str, &str, &str); 5] = [
    ("main", "/ai-monopoly", "Gönderim"),
    ("live", "/ai-monopoly?tab=live", "Canlı"),
    ("standings", "/ai-monopoly?tab=standings", "Puan durumu"),
    ("history", "/ai-monopoly?tab=history", "Maçlar"),
    (
        "instructions",
        "/ai-monopoly?tab=instructions",
        "Instructions",
    ),
];

fn tournament_shell(user: &User, tab: &str, sub: &str, inner: &str) -> String {
    let chips: String = MONOPOLY_TOURNAMENT_TABS
        .iter()
        .map(|(key, href, label)| {
            format!(
                r#"<a class="chip {}" href="{}">{}</a>"#,
                if tab == *key { "active" } else { "" },
                href,
                label
            )
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

fn tournament_seats(game: &MonopolyGame) -> Vec<MonopolySeat> {
    serde_json::from_value(game.seats.clone()).unwrap_or_default()
}

fn tournament_seat_list(game: &MonopolyGame) -> String {
    tournament_seats(game)
        .iter()
        .map(|seat| {
            let winner = if game.winner_seat == Some(seat.player_id) {
                " winner"
            } else {
                ""
            };
            let kind = if seat.entry_id.is_some() { "" } else { " · bot" };
            format!(
                r#"<li class="monopoly-seat{winner}"><span class="seat-token">{number}</span><span>{label}<small>{kind}</small></span></li>"#,
                number = seat.player_id + 1,
                label = esc(&seat.label),
            )
        })
        .collect()
}

fn duration_text(duration_us: Option<i64>) -> String {
    duration_us
        .map(|value| format!("{:.2} sn", value as f64 / 1_000_000.0))
        .unwrap_or_else(|| "—".into())
}

fn timing_text(total: i64, count: i64, min: Option<i64>, max: Option<i64>) -> String {
    if count == 0 {
        return "karar yok".into();
    }
    format!(
        "ort {:.1} ms · en hızlı {:.1} · en yavaş {:.1}",
        total as f64 / count as f64 / 1000.0,
        min.unwrap_or_default() as f64 / 1000.0,
        max.unwrap_or_default() as f64 / 1000.0,
    )
}

fn tournament_summary(tournament: &MonopolyTournament) -> String {
    let (label, class) = match tournament.status.as_str() {
        "active" => ("Devam ediyor", "st-reviewing"),
        "completed" => ("Tamamlandı", "st-passed"),
        "partial" => ("Kısmi tamamlandı", "st-failed"),
        _ => ("Durduruldu", "st-failed"),
    };
    let reason = tournament
        .partial_reason
        .as_deref()
        .map(|reason| format!(r#"<p class="fieldnote">{}</p>"#, esc(reason)))
        .unwrap_or_default();
    format!(
        r##"<div class="tournament-strip">
  <span class="substatus {class}">{label}</span>
  <b>{done}/{total} maç</b>
  <span>{ruleset}</span>
  <a href="/ai-monopoly/tournament/{id}/export.json">JSON indir ↓</a>
</div>{reason}"##,
        done = tournament.completed_games,
        total = tournament.total_games,
        ruleset = esc(&tournament.ruleset_version),
        id = tournament.id,
    )
}

pub fn monopoly_main(
    user: &User,
    team: Option<&MonopolyTeam>,
    members: &[TeamMemberRow],
    submission: Option<&MonopolySubmission>,
    tournament: Option<&MonopolyTournament>,
    game: Option<&MonopolyGame>,
    standings: &[MonopolyStanding],
) -> String {
    let submission_panel = match team {
        None => r##"<section class="panel monopoly-submit">
  <h2>Önce bir takım</h2>
  <p class="muted">Takım ataması için eğitmenine yaz. Gönderim yalnızca takım üyelerine açıktır.</p>
</section>"##
            .to_string(),
        Some(team) => {
            let roster: String = members
                .iter()
                .filter(|member| member.team_id == team.id)
                .map(|member| format!(r#"<span class="chip">{}</span>"#, esc(&member.display_name)))
                .collect();
            let current = submission
                .map(|submission| {
                    let (status, class) = monopoly_submission_status_tr(&submission.status);
                    let commit = submission
                        .commit_sha
                        .as_deref()
                        .map(|sha| format!(r#"<code title="Sabit commit">{}</code>"#, esc(&sha[..12])))
                        .unwrap_or_default();
                    let size = submission
                        .repo_size_bytes
                        .map(|bytes| format!("{:.1} MiB", bytes as f64 / 1_048_576.0))
                        .unwrap_or_else(|| "boyut bekleniyor".into());
                    let log = submission
                        .validation_log
                        .as_deref()
                        .filter(|log| !log.trim().is_empty())
                        .map(|log| format!(
                            r#"<details class="build-log"><summary>Doğrulama günlüğü</summary><pre>{}</pre></details>"#,
                            esc(log)
                        ))
                        .unwrap_or_default();
                    format!(
                        r##"<div class="submission-current">
  <div><span class="substatus {class}">{status}</span><span class="item-meta">nesil {generation} · {size}</span></div>
  <a href="{repo}" target="_blank" rel="noopener">{repo_label}</a>
  <span class="item-meta"><code>{path}</code> {commit}</span>{log}
</div>"##,
                        generation = submission.generation,
                        repo = esc(&submission.repo_url),
                        repo_label = esc(submission.repo_url.trim_start_matches("https://github.com/")),
                        path = esc(&submission.agent_path),
                    )
                })
                .unwrap_or_else(|| r#"<p class="fieldnote">Henüz ajan göndermediniz.</p>"#.into());
            let repo = submission
                .map(|value| value.repo_url.as_str())
                .unwrap_or("");
            let path = submission
                .map(|value| value.agent_path.as_str())
                .unwrap_or(MONOPOLY_SUBMISSION_ENTRYPOINT);
            format!(
                r##"<section class="panel monopoly-submit">
  <p class="eyebrow">TAKIM</p><h2>{team}</h2><div class="chips">{roster}</div>
  {current}
  <form method="post" action="/ai-monopoly/submit" class="subform">
    <label>Public GitHub repo<input name="repo_url" type="url" value="{repo}" placeholder="https://github.com/kullanici/ajan" required></label>
    <label>Ajan dosyası<input name="agent_path" value="{path}" required></label>
    <button class="btn-dark">Doğrulamaya gönder</button>
  </form>
  <p class="fieldnote">Varsayılan dalın commit'i sabitlenir. Git LFS desteklenir; çözülmüş checkout sınırı 250 MiB.</p>
  <a class="textlink" href="/ai-monopoly?tab=instructions">Sözleşmeyi ve kuralları gör →</a>
</section>"##,
                team = esc(&team.name),
                repo = esc(repo),
                path = esc(path),
            )
        }
    };

    let tournament_panel = match (tournament, game) {
        (None, _) => r##"<section class="panel monopoly-next-game">
  <p class="eyebrow">TURNUVA</p><h2>Henüz başlamadı</h2>
  <p class="muted">En az dört ajan doğrulandığında eğitmen fikstürü dondurur.</p>
</section>"##
            .to_string(),
        (Some(tournament), game) => {
            let next = game
                .map(|game| {
                    let (status, class) = monopoly_game_status_tr(&game.status);
                    format!(
                        r##"<a class="match-peek" href="/ai-monopoly/game/{id}">
  <span><small>Maç {number}</small><b>Tur {round}/{max}</b></span>
  <span class="substatus {class}">{status}</span>
</a><ol class="monopoly-seats">{seats}</ol>"##,
                        id = game.id,
                        number = game.game_no,
                        round = game.round,
                        max = 200,
                        seats = tournament_seat_list(game),
                    )
                })
                .unwrap_or_default();
            let leader = standings
                .first()
                .map(|row| {
                    format!(
                        r#"<p class="fieldnote">Lider: <b>{}</b> · {} galibiyet</p>"#,
                        esc(&row.team_name),
                        row.wins
                    )
                })
                .unwrap_or_default();
            format!(
                r##"<section class="panel monopoly-next-game"><p class="eyebrow">TURNUVA</p>
{summary}{next}{leader}
<a class="btn-outline" href="/ai-monopoly?tab=live">Aktif maçları aç →</a></section>"##,
                summary = tournament_summary(tournament),
            )
        }
    };
    tournament_shell(
        user,
        "main",
        "Ajanını gönder; sabitlenen turnuvayı, maçları ve karar sürelerini izle.",
        &format!(r#"<div class="monopoly-grid">{submission_panel}{tournament_panel}</div>"#),
    )
}

pub fn monopoly_live(
    user: &User,
    tournament: Option<&MonopolyTournament>,
    games: &[MonopolyGame],
) -> String {
    let Some(tournament) = tournament else {
        return tournament_shell(
            user,
            "live",
            "Aktif masalar burada birlikte görünür.",
            r#"<div class="panel arena-idle"><h2>Turnuva bekleniyor</h2><p class="muted">Fikstür dondurulduğunda maçlar burada açılır.</p></div>"#,
        );
    };
    let active: Vec<&MonopolyGame> = games
        .iter()
        .filter(|game| matches!(game.status.as_str(), "leased" | "queued"))
        .collect();
    let rows: String = games
        .iter()
        .map(|game| {
            let (status, class) = monopoly_game_status_tr(&game.status);
            let names = tournament_seats(game)
                .iter()
                .map(|seat| esc(&seat.label))
                .collect::<Vec<_>>()
                .join(" · ");
            format!(
                r##"<a class="monopoly-history-row" href="/ai-monopoly/game/{id}">
  <b>Maç {number}</b><span class="history-table"><span>{names}</span></span>
  <span class="history-round">Tur {round}/200</span><span class="substatus {class}">{status}</span>
</a>"##,
                id = game.id,
                number = game.game_no,
                round = game.round,
            )
        })
        .collect();
    let arena = active
        .iter()
        .find(|game| game.status == "leased")
        .or_else(|| active.first())
        .map(|game| format!(
            r#"<div id="monopoly-arena" class="monopoly-arena" data-poll="true" data-game-id="{}"><div class="arena-idle">Maç hazırlanıyor…</div></div>"#,
            game.id
        ))
        .unwrap_or_else(|| r#"<div class="panel arena-idle"><h2>Bütün maçlar bitti</h2><a class="btn-outline" href="/ai-monopoly?tab=standings">Son tabloyu aç →</a></div>"#.into());
    tournament_shell(
        user,
        "live",
        "Bir masayı aç; tam tahta ve batched hamle akışı iki saniyede bir yenilenir.",
        &format!(
            r##"{summary}<div class="active-match-layout"><div>{arena}</div><aside class="active-match-list"><h2>Fikstür</h2>{rows}</aside></div>
<script src="/static/monopoly.js?v=5" defer></script>"##,
            summary = tournament_summary(tournament),
        ),
    )
}

pub fn monopoly_history(
    user: &User,
    tab: &str,
    tournament: Option<&MonopolyTournament>,
    games: &[MonopolyGame],
    standings: &[MonopolyStanding],
) -> String {
    let Some(tournament) = tournament else {
        return tournament_shell(
            user,
            tab,
            "Turnuva sonuçları.",
            r#"<div class="panel"><p class="muted">Henüz turnuva yok.</p></div>"#,
        );
    };
    if tab == "standings" {
        let rows: String = standings
            .iter()
            .enumerate()
            .map(|(index, row)| {
                format!(
                    r##"<tr><td>{rank}</td><td><b>{team}</b><small>{games}/6 maç</small></td>
<td>{wins}</td><td>{worth:.0}</td><td>{strikes}</td><td>{timing}</td></tr>"##,
                    rank = index + 1,
                    team = esc(&row.team_name),
                    games = row.games,
                    wins = row.wins,
                    worth = row.average_net_worth,
                    strikes = row.strikes,
                    timing = timing_text(
                        row.decision_total_us,
                        row.decision_count,
                        row.decision_min_us,
                        row.decision_max_us
                    ),
                )
            })
            .collect();
        return tournament_shell(
            user,
            tab,
            "Galibiyet, ortalama final serveti ve daha az strike sırasıyla belirler.",
            &format!(
                r##"{summary}<div class="tablewrap standings-table"><table>
<thead><tr><th>#</th><th>Takım</th><th>G</th><th>Ort. servet</th><th>Strike</th><th>Karar süresi</th></tr></thead>
<tbody>{rows}</tbody></table></div>"##,
                summary = tournament_summary(tournament)
            ),
        );
    }
    let rows: String = games
        .iter()
        .rev()
        .map(|game| {
            let (status, class) = monopoly_game_status_tr(&game.status);
            let seats = tournament_seats(game);
            let winner = game
                .winner_seat
                .and_then(|winner| seats.iter().find(|seat| seat.player_id == winner))
                .map(|seat| format!("Kazanan: {}", esc(&seat.label)))
                .unwrap_or_else(|| "Kazanan yok".into());
            let names = seats
                .iter()
                .map(|seat| esc(&seat.label))
                .collect::<Vec<_>>()
                .join(" · ");
            format!(
                r##"<a class="monopoly-history-row" href="/ai-monopoly/game/{id}">
  <span class="history-date">Maç {number}</span>
  <span class="history-table"><b>{winner}</b><span>{names}</span></span>
  <span class="history-round">{duration}</span><span class="substatus {class}">{status}</span>
</a>"##,
                id = game.id,
                number = game.game_no,
                duration = duration_text(game.duration_us),
            )
        })
        .collect();
    tournament_shell(
        user,
        tab,
        "Her maçın final serveti, strike'ları, süresi ve tam hamle tekrarı.",
        &format!(
            r#"{}<div class="monopoly-history">{rows}</div>"#,
            tournament_summary(tournament)
        ),
    )
}

pub fn monopoly_game_page(
    user: &User,
    game: &MonopolyGame,
    runtime_logs: &[(i16, String, String)],
) -> String {
    let runtime_logs: String = runtime_logs
        .iter()
        .map(|(seat, label, log)| {
            format!(
                r#"<details class="build-log"><summary>Koltuk {seat} · {label} çalışma günlüğü</summary><pre>{log}</pre></details>"#,
                seat = seat + 1,
                label = esc(label),
                log = esc(log),
            )
        })
        .collect();
    let runtime_panel = if runtime_logs.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="panel monopoly-runtime-logs"><h2>Özel çalışma günlükleri</h2><p class="muted">Yalnızca eğitmenler ve bu koltuğun takım üyeleri görebilir.</p>{runtime_logs}</section>"#
        )
    };
    tournament_shell(
        user,
        "history",
        &format!(
            "Maç {} · tam tahta tekrarı ve karar ölçümleri.",
            game.game_no
        ),
        &format!(
            r##"<p><a class="textlink" href="/ai-monopoly?tab=history">← Maçlara dön</a></p>
<div id="monopoly-arena" class="monopoly-arena" data-poll="true" data-replay="true" data-game-id="{id}"><div class="arena-idle">Tekrar yükleniyor…</div></div>
{runtime_panel}
<script src="/static/monopoly.js?v=5" defer></script>"##,
            id = game.id,
        ),
    )
}

pub fn monopoly_instructions(user: &User) -> String {
    tournament_shell(
        user,
        "instructions",
        "Gönderim sözleşmesi, doğrulama ve turnuva kuralları.",
        r##"<div class="rulewrap monopoly-rules">
<section class="panel"><p class="eyebrow">GÖNDERİM</p><h2>Public GitHub repo + ajan yolu</h2>
<p>Varsayılan dalın commit'i doğrulama sırasında bir kez sabitlenir. <code>agent.py</code>
varsayılandır; repo içindeki başka bir göreli Python dosyasını seçebilirsin. Git LFS dâhil
çözülmüş checkout en fazla 250 MiB olabilir.</p>
<pre><code>def choose_action(state, player_id, allowed_actions) -&gt; int:
    return allowed_actions[0]</code></pre></section>
<section class="panel"><h2>Salt okunur karar durumu</h2><ul class="harness-rules">
<li><code>ruleset_version</code>: <code>ppo-plus-v2</code>; <code>schema_version</code>: sözleşme sürümü.</li>
<li><code>vector</code>: aktöre göre düzenlenmiş tam 300 float.</li>
<li><code>board</code>: okunabilir tahta görüntüsü; canlı engine nesnesi verilmez.</li>
<li><code>actions</code>: legal aksiyon açıklamaları; <code>decision_seed</code>: deterministik karar tohumu.</li>
</ul></section>
<section class="panel"><h2>Bağımlılıklar ve izolasyon</h2>
<p><code>requirements.txt</code> en fazla 32 wheel-only PyPI girdisi alır. Doğrulama bunları
çözer ve lock dosyasını artifact'e koyar; maç sırasında internetten paket indirilmez.
Server ajanları Docker'da, Colab ajanları 2 GiB sınırlandırılmış ayrı venv süreçlerinde çalışır.</p></section>
<section class="panel"><h2>Altı maç, sert iki saniye</h2><p>Her takım tam altı kez oynar.
Tek legal aksiyon engine tarafından otomatik uygulanır. Gerçek karar iki saniyeyi aşar, çöker
veya illegal değer döndürürse strike ve deterministik fallback gelir; üçüncü strike o maçta
ajanı sabit botla değiştirir.</p></section>
<section class="panel"><h2>Maç sonu</h2><p>Canonical oyun 200 tur veya 50.000 aksiyonda biter.
Engine-play süresi on dakikaya ulaşırsa ortalama kararı en yavaş uygun takım diskalifiye edilir.
Botlar kazanamaz; kazanan her zaman uygun gönderimler arasındaki en yüksek final servetidir.
Puan durumu galibiyet, ortalama final serveti ve daha az strike ile sıralanır.</p></section>
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

/// The weeks the track runs over, in the order they are shown. Every project carries one
/// of these numbers; a week nobody is assigned to renders nothing at all.
pub const BEGINNER_WEEKS: [(u8, &str); 2] = [(1, "1. Hafta"), (2, "2. Hafta")];

/// One project: key, title, one-line summary, its handouts as (button label, pdf filename
/// in static/beginner-projects/), whether the project deploys anywhere, which
/// `BEGINNER_WEEKS` week it belongs to, and an optional badge shown next to the title on
/// the card — `None` for a project that needs no flag on it.
pub type BeginnerProject = (
    &'static str,
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
    bool,
    u8,
    Option<&'static str>,
);

// ponytail: hardcoded list, same pattern as DEMOS — these are fixed, code-and-deploy
// content, not something an admin edits day to day. Add a row here (and the PDFs) for a
// new project. Handouts are a list rather than a brief plus an optional extra because a
// project can ship a reference sheet next to its brief, and a group project hands each
// student a brief of their own — the card renders them in the order written here. An
// open-ended project hands out nothing at all, so the list is allowed to be empty. The
// flag is false for a project that runs locally and has no live site to hand in — those
// ask for the repo only.
pub const BEGINNER_PROJECTS: [BeginnerProject; 11] = [
    (
        "kisisel-web-sitesi",
        "Proje 1 — Kişisel Web Sitesi",
        "İlgi alanlarını ve ürettiklerini anlatan, yayında olan kişisel bir web sitesi kur.",
        &[("Brifi indir ⬇", "01-kisisel-web-sitesi.pdf")],
        true,
        1,
        None,
    ),
    (
        "kisisel-web-sitesi-chatbotu",
        "Proje 2 — Kişisel Web Sitesi Chatbotu",
        "Web siteni, profile.md dosyasından seni tanıtan bir chatbot ile genişlet.",
        &[("Brifi indir ⬇", "02-kisisel-web-sitesi-chatbotu.pdf")],
        true,
        1,
        None,
    ),
    (
        "ai-bouquet-maker",
        "Proje 3 — AI Bouquet Maker",
        "Annen için kişiselleştirilmiş yapay zekâ çiçek buketleri oluşturan bir uygulama geliştir.",
        &[("Brifi indir ⬇", "03-ai-bouquet-maker.pdf")],
        true,
        1,
        None,
    ),
    (
        "renovate-your-room",
        "Proje 4 — Renovate Your Room",
        "Oda fotoğrafını yükleyip yapay zekâ ile farklı dekorasyon stillerinde yeniden tasarla.",
        &[("Brifi indir ⬇", "04-renovate-your-room.pdf")],
        true,
        1,
        None,
    ),
    (
        "character-voice-studio",
        "Proje 5 — Character Voice Studio",
        "Kendi karakterini oluştur, görsel ve sesle hayata geçirip konuştur.",
        &[("Brifi indir ⬇", "05-character-voice-studio.pdf")],
        true,
        1,
        None,
    ),
    (
        "ai-calorie-tracker",
        "Proje 6 — AI Calorie Tracker",
        "Yemek fotoğrafını yapay zekâ ile analiz edip kalori ve besin değerlerini takip eden bir uygulama geliştir.",
        &[("Brifi indir ⬇", "06-ai-calorie-tracker.pdf")],
        true,
        1,
        None,
    ),
    (
        "smart-receipt",
        "Proje 7 — Smart Receipt",
        "Fiş fotoğraflarını yapay zekâ ile okuyup harcamaları Google Sheets'e otomatik aktaran bir uygulama geliştir.",
        &[
            ("Brifi indir ⬇", "07-smart-receipt.pdf"),
            (
                "Apps Script cheat sheet ⬇",
                "07-google-apps-script-cheat-sheet.pdf",
            ),
        ],
        true,
        1,
        None,
    ),
    // The keys below are deliberately not numbered: these two swapped places once already,
    // and a key is what a saved submission points at — renaming one orphans every row.
    (
        "campus-lost-and-found",
        "Proje 8 — Campus Lost & Found",
        "İki kişilik bir takımla, kampüs için ilan verme ve claim gönderme taraflarını tek uygulamada birleştiren bir Lost & Found platformu geliştir.",
        &[
            (
                "Student 1 brifi ⬇",
                "08-campus-lost-and-found-student-1.pdf",
            ),
            (
                "Student 2 brifi ⬇",
                "08-campus-lost-and-found-student-2.pdf",
            ),
            (
                "Group project cheat sheet ⬇",
                "08-group-project-cheat-sheet.pdf",
            ),
        ],
        true,
        2,
        None,
    ),
    (
        "browser-agent",
        "Proje 9 — Browser Agent",
        "Browser Use ve Gemma 4 31B ile Agent Lab challenge'larını kendi başına tamamlayan bir browser agent geliştir.",
        &[
            ("Brifi indir ⬇", "09-browser-agent.pdf"),
            (
                "Browser Agent cheat sheet ⬇",
                "09-browser-agent-cheat-sheet.pdf",
            ),
        ],
        false,
        2,
        None,
    ),
    (
        "habit-tracker-mobile-app",
        "Proje 10 — Habit Tracker Mobile App",
        "React Native + Expo ile sınırsız goal, deadline hatırlatmaları ve günlük streak takibi olan gerçek bir mobil uygulama geliştir.",
        &[
            ("Brifi indir ⬇", "10-habit-tracker-mobile-app.pdf"),
            ("Expo cheat sheet ⬇", "10-expo-mobile-app-cheat-sheet.pdf"),
            (
                "Kurulum rehberi ⬇",
                "10-habit-tracker-mobile-app-install-guide.pdf",
            ),
        ],
        false,
        2,
        None,
    ),
    // The last one hands out nothing: the point is that the group picks the problem, so a
    // brief would be the one thing that gets in the way. It is also the project that goes
    // on stage, hence the badge — the link saved here is the one demoed on Demo Day.
    (
        "kendi-projen-1",
        "Proje 11 — Kendi Projeniz",
        "Brif yok, cheat sheet yok. 3 kişilik gruplar kurun ve hayatınızdaki gerçek bir problemi seçin — ailenizin işletmesinin sitesi çok eski, babanız işinde bir otomasyona ihtiyaç duyuyor, e-postalarınızı okumaya üşeniyorsunuz — sonra öğrendiklerinizle çözün. Landing page, web uygulaması, otomasyon, mobil uygulama, browser agent: formatı siz seçin.\n\nDemo Day'de sahnede gösterilecek proje bu: buraya kaydettiğiniz bağlantı sunumda kullanılacak, o yüzden grubun her üyesi aynı repo ve canlı bağlantıyı kaydetsin.",
        &[],
        true,
        2,
        Some("Demo Day"),
    ),
];

/// Whether this project hands in a live site next to its repo. An unknown key answers
/// `true`, the stricter side — callers validate the key against the list first anyway.
pub fn project_wants_live_url(key: &str) -> bool {
    BEGINNER_PROJECTS
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|(.., wants, _week, _badge)| *wants)
        .unwrap_or(true)
}

/// Beginner Track — the fixed projects above, each with a downloadable brief and a
/// save-your-links form. Self-reported, no grading: the form always shows, pre-filled
/// with whatever was last saved, and resaving just overwrites it.
/// The track's own hub: three subsets side by side, same pattern advanced_track()
/// uses for Agentic Harness / AI Monopoly — Chatbot Challenge, Agent Lab and the
/// weekly projects are peers, not one floating card above a flat project list.
pub fn beginner_track(user: &User, projects_done: usize, chatbot_level: i16) -> String {
    let content = format!(
        r##"<h1 class="pagetitle">Beginner Track</h1>
<p class="muted">Başlangıç seviyesindeki üç bölüm.</p>
<div class="hubgrid">
  <a class="hubcard" href="/beginner-track/projects">
    <span class="hubico">{ico_projects}</span>
    <h2>Haftalık Projeler</h2>
    <p>{total} proje. Brifi olanın brifini indir, projeni yap, sonra GitHub ve Vercel bağlantılarını kaydet. Kaydedilen: {projects_done}/{total}.</p>
    <span class="hubgo">Projelere git →</span>
  </a>
  <a class="hubcard" href="/chatbot-challenge">
    <span class="hubico">{ico_chat}</span>
    <h2>Chatbot Challenge</h2>
    <p>Bir chatbotu kandırıp gizli anahtarını söylettirmeye çalış — {CHATBOT_LEVEL_COUNT} seviye, her biri bir öncekinden daha zor. {chat_status}</p>
    <span class="hubgo">Oyuna git →</span>
  </a>
  <a class="hubcard" href="{AGENT_LAB_PATH}">
    <span class="hubico">{ico_lab}</span>
    <h2 lang="en">Agent Lab</h2>
    <p>Browser agent'ını Exposure Student Portal üzerinde test et. Form doldur, doğru projeyi bul ve görev akışlarını otomatikleştir.</p>
    <span class="hubgo">Agent Lab'e git →</span>
  </a>
</div>"##,
        ico_projects = ico(P_DOC),
        ico_chat = ico(P_CHAT),
        ico_lab = ico(P_BEAKER),
        // Counted off the list, not typed in — adding a project should mean adding a row.
        total = BEGINNER_PROJECTS.len(),
        chat_status = if chatbot_level > CHATBOT_LEVEL_COUNT {
            format!("{CHATBOT_LEVEL_COUNT}/{CHATBOT_LEVEL_COUNT} — tamamlandı 🏆")
        } else {
            format!("Şu an seviye {chatbot_level}/{CHATBOT_LEVEL_COUNT}.")
        },
    );
    layout("Beginner Track", Some(user), "beginner-track", &content)
}

/// The weekly-projects subset: cheat sheet plus one card per project, split out of
/// beginner_track() so that page can stay a clean two-card hub. Cards are grouped under
/// the week they are handed out in — the track runs over two weeks, and a flat run of
/// eight cards hides which ones are this week's.
pub fn beginner_projects(user: &User, subs: &[BeginnerSubmission]) -> String {
    let card = |(key, title, summary, handouts, wants_live, _week, badge): &BeginnerProject| {
            // A project's own handouts sit on its card, not up with the track-wide cheat
            // sheet — they are only useful once you're on this project. A group project
            // puts a brief per student here, which is why this is a list and not one
            // download with an extra hanging off it. An open-ended project hands out
            // nothing, and then the actions row is dropped rather than left empty.
            let handout_links = if handouts.is_empty() {
                String::new()
            } else {
                let links: String = handouts
                    .iter()
                    .map(|(label, file)| format!(
                        r#"<a class="btn-outline small" href="/static/beginner-projects/{file}" target="_blank" rel="noopener">{label}</a>"#,
                        label = esc(label),
                    ))
                    .collect();
            format!(r#"<div class="cardactions">{links}</div>"#)
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
        // A locally-run project has nothing to deploy, so its card asks for the repo
        // and stops there — no field the student can only fill by making one up. The
        // input is dropped entirely rather than left optional; `vercel_url` defaults
        // to empty server-side, which is what those rows store.
        let live_input = if *wants_live {
            format!(
                r#"<input name="vercel_url" type="url" placeholder="https://...vercel.app" value="{vercel_val}" required>"#,
                vercel_val = esc(&vercel_val),
            )
        } else {
            String::new()
        };
        format!(
            r##"<div class="taskcard">
  <div class="taskhead"><h3>{title}</h3>{badge}</div>
  <p class="desc">{summary}</p>
  {handout_links}
  {saved_note}
  <form method="post" action="/beginner-track/submit" class="subform">
    <input type="hidden" name="project_key" value="{key}">
    <input name="repo_url" type="url" placeholder="https://github.com/..." value="{repo_val}" required>
    {live_input}
    <button class="btn-dark">Kaydet →</button>
  </form>
</div>"##,
                title = esc(title),
                summary = esc(summary),
                repo_val = esc(&repo_val),
                // Same .badge the board cards use, so a flagged project reads as flagged
                // in the one place students already look for it — next to the title.
                badge = badge
                    .map(|b| format!(r#"<span class="badge">{}</span>"#, esc(b)))
                    .unwrap_or_default(),
            )
        };
    // One heading + grid per week, in BEGINNER_WEEKS order. A week with no projects on it
    // yet renders nothing rather than a heading over an empty grid.
    let sections: String = BEGINNER_WEEKS
        .iter()
        .filter_map(|(week, label)| {
            let cards: String = BEGINNER_PROJECTS
                .iter()
                .filter(|(.., w, _badge)| w == week)
                .map(card)
                .collect();
            if cards.is_empty() {
                return None;
            }
            Some(format!(
                r##"<h2 class="weekhead">{label}</h2>
<div class="tasks">{cards}</div>"##,
                label = esc(label),
            ))
        })
        .collect();
    let total = BEGINNER_PROJECTS.len();
    let content = format!(
        r##"<h1 class="pagetitle">Haftalık Projeler</h1>
<p class="muted">Başlangıç seviyesindeki {total} proje. Brifi olanın brifini indir, projeni yap, sonra GitHub ve Vercel bağlantılarını kaydet. Son proje brifsiz ve 3 kişilik gruplarla: problemi de çözümü de siz seçiyorsunuz, sonuç Demo Day'de sahnede.</p>
<div class="taskcard">
  <div class="taskhead"><h3>Vibe Coding Cheat Sheet</h3></div>
  <p class="desc">Tüm beginner track projelerinde işine yarayacak hızlı referans rehberi.</p>
  <div class="cardactions">
    <a class="btn-outline small" href="/static/beginner-projects/vibe-coding-cheat-sheet.pdf" target="_blank" rel="noopener">Cheat sheet indir ⬇</a>
  </div>
</div>
{sections}"##
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
  f.addEventListener('submit', function(ev){{
    ev.preventDefault();
    var ta = f.querySelector('textarea'), btn = document.getElementById('chsend');
    var text = ta.value;
    if (!text.trim()) return;
    var u = document.createElement('div');
    u.className = 'bub r';
    u.innerHTML = '<div class="say"></div>';
    u.querySelector('.say').textContent = text;
    chat.appendChild(u);
    var b = document.createElement('div');
    b.className = 'bub l typing';
    b.innerHTML = '<div class="say"></div>';
    chat.appendChild(b);
    var say = b.querySelector('.say');
    chat.scrollTop = chat.scrollHeight;
    ta.value = '';
    ta.readOnly = true;
    btn.disabled = true;

    function showError(){{
      b.remove();
      var n = document.createElement('p');
      n.className = 'notice';
      n.textContent = 'Bot şu an cevap veremedi, tekrar dene.';
      chat.after(n);
    }}

    fetch(f.action, {{ method: 'POST', body: new URLSearchParams({{ message: text }}) }})
      .then(function(resp){{
        var reader = resp.body.getReader();
        var decoder = new TextDecoder();
        var buf = '';
        function pump(){{
          return reader.read().then(function(res){{
            if (res.done) return;
            buf += decoder.decode(res.value, {{ stream: true }});
            var parts = buf.split('\n\n');
            buf = parts.pop();
            parts.forEach(function(block){{
              var evType = 'message', dataLines = [], sawField = false;
              block.split('\n').forEach(function(line){{
                if (line.indexOf('event:') === 0) {{ evType = line.slice(6).trim(); sawField = true; }}
                else if (line.indexOf('data:') === 0) {{
                  var v = line.slice(5);
                  if (v.charAt(0) === ' ') v = v.slice(1);
                  dataLines.push(v);
                  sawField = true;
                }}
              }});
              if (!sawField) return; // pure keep-alive comment, not a real event
              var data = dataLines.join('\n');
              if (evType === 'message') {{
                b.classList.remove('typing');
                say.textContent += data;
                chat.scrollTop = chat.scrollHeight;
              }} else if (evType === 'error') {{
                showError();
              }} else if (evType === 'done') {{
                var info = {{}};
                try {{ info = JSON.parse(data); }} catch (e) {{}}
                if (info.completed) location.reload();
              }}
            }});
            return pump();
          }});
        }}
        return pump();
      }})
      .catch(showError)
      .finally(function(){{
        ta.readOnly = false;
        btn.disabled = false;
      }});
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
    layout(
        "Chatbot Challenge",
        Some(user),
        "chatbot-challenge",
        &content,
    )
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
    let board = if user.is_admin {
        format!(r##"<div class="lb">{list}</div>"##)
    } else {
        format!(
            r##"<div class="doc-locked">
  <div class="doc-blur" aria-hidden="true"><div class="lb">{list}</div></div>
  <div class="doc-lockmsg">{lock}<b>Oyun daha bitmedi!</b>
    <span>Kazananlar zamanı gelince yayınlanacak.</span></div>
</div>"##,
            lock = ico(P_LOCK),
        )
    };
    let content = format!(
        r##"<h1 class="pagetitle">Chatbot Challenge — Sıralama</h1>
<p class="muted">Kim daha çok seviye kırdı? İlk {CHATBOT_LEVEL_COUNT}/{CHATBOT_LEVEL_COUNT}'a ulaşan kazanır.</p>
{board}"##
    );
    layout(
        "Chatbot Challenge Sıralaması",
        Some(user),
        "chatbot-challenge",
        &content,
    )
}

// ---- Agent Lab (Beginner Track) ----

/// Canonical Agent Lab URL. It is a Beginner Track sub-page, not a sidebar section of its
/// own, so it lives under `/beginner-track/` and the sidebar keeps highlighting Beginner
/// Track while a student is inside the lab.
pub const AGENT_LAB_PATH: &str = "/beginner-track/agent-lab";

/// The lab's challenges: (path segment, badge, title, one-line summary, difficulty). Fixed
/// content, same hardcoded-list pattern as `BEGINNER_PROJECTS` and `DEMOS`.
///
/// An empty difficulty prints the badge alone, which is what challenges 1 and 2 have always
/// shown — the pill only grows a "· <level>" suffix for a challenge that declares one.
pub const AGENT_LAB_CHALLENGES: [(&str, &str, &str, &str, &str); 3] = [
    (
        "student-profile",
        "Challenge 1",
        "Student Profile Agent",
        "Ajanın sandbox öğrenci profilini açsın, beş alanı da doldursun ve kaydetsin. \
         Formu ajan bulmalı — sen tıklamayacaksın.",
        "",
    ),
    (
        "project-submission",
        "Challenge 2",
        "Project Submission Agent",
        "Ajanın brifi okusun, listedeki beş sandbox projeden tarife uyanı seçsin ve \
         repo + demo bağlantılarıyla göndersin.",
        "",
    ),
    (
        "job-applications",
        "Challenge 3",
        "Job Application Agent",
        "Aynı bilgileri 10 farklı iş başvurusuna tek tek yazmak yerine browser agent'ın \
         senin yerine formları anlayıp doldursun.",
        "Intermediate",
    ),
];

/// The five sandbox projects challenge 2 shows. The titles deliberately echo the real
/// Beginner Track catalogue, because picking the right one out of projects a student
/// actually recognises is the reading task worth practising — a set of invented names
/// would make it a vocabulary puzzle instead.
///
/// The echo is in the labels only. Keys carry a `lab-` prefix that no `BEGINNER_PROJECTS`
/// key shares (asserted in `agent_lab_target_is_one_of_its_projects`), no brief here is
/// downloadable, and submissions land in `agent_lab_submissions_exposure_academy` — so a
/// row written from this page can never be read as a student handing in real work.
///
/// Each summary states what its project *does*, in wording the brief does not reuse: the
/// agent has to match behaviour to description rather than string-match the two.
///
/// The order is display order, and the answer sits mid-list on purpose — first or last it
/// would be reachable by guessing rather than by reading. Reordering is safe: everything
/// downstream keys off `AGENT_LAB_TARGET`, never a position.
pub const AGENT_LAB_PROJECTS: [(&str, &str, &str); 5] = [
    (
        "lab-renovate-your-room",
        "Renovate Your Room",
        "Yüklenen oda fotoğraflarını farklı stillerde yeniden tasarlayan uygulama.",
    ),
    (
        "lab-character-voice-studio",
        "Character Voice Studio",
        "Karakter oluşturup onlara ses üreten stüdyo.",
    ),
    (
        "lab-personal-website",
        "Personal Website",
        "Öğrencinin çevrimiçi kimliğini ve portfolyosunu tek sayfada toplayan site.",
    ),
    (
        "lab-track-your-calories",
        "Track Your Calories",
        "Yemek fotoğraflarını analiz edip besin değerlerini tahmin eden uygulama.",
    ),
    (
        "lab-ai-bouquet-maker",
        "AI Bouquet Maker",
        "Yapay zekâ ile çiçek buketi görselleri üreten uygulama.",
    ),
];

/// Which of `AGENT_LAB_PROJECTS` the challenge-2 brief describes. The brief below names the
/// behaviour, never the key or the title, so finding it is a reading task for the agent.
pub const AGENT_LAB_TARGET: &str = "lab-personal-website";

/// What the agent has to satisfy in challenge 2, in the student's own words. Kept next to
/// `AGENT_LAB_TARGET` so the two can't drift apart.
///
/// It describes the target obliquely on purpose — never "Personal Website" or "Kişisel Web
/// Sitesi" — so the answer has to be reasoned out of the description rather than matched
/// against it. `agent_lab_brief_does_not_name_its_answer` guards that.
const AGENT_LAB_BRIEF: &str = "Öğrencinin kendisini, ilgi alanlarını, projelerini ve sosyal \
     medya bağlantılarını tanıttığı; kendine özel bir tasarım veya etkileşim eklediği ve \
     Vercel üzerinden yayınladığı web projesini bul.";

/// The lab hub: the challenges as peer hubcards, the same shape `beginner_track()` itself
/// uses one level up — the lab is a subset with subsets, not a page of task cards.
pub fn agent_lab(user: &User) -> String {
    let cards: String = AGENT_LAB_CHALLENGES
        .iter()
        .map(|(slug, badge, title, summary, difficulty)| {
            format!(
                r##"<a class="hubcard" href="{AGENT_LAB_PATH}/{slug}">
    <span class="hubico">{icon}</span>
    <h2 lang="en">{title}</h2>
    <p>{summary}</p>
    <span class="hubstat" lang="en">{pill}</span>
    <span class="hubgo">Challenge'ı aç →</span>
  </a>"##,
                icon = ico(match *slug {
                    "student-profile" => P_TEAMS,
                    "job-applications" => P_BRIEFCASE,
                    _ => P_BOARD,
                }),
                title = esc(title),
                pill = if difficulty.is_empty() {
                    esc(badge)
                } else {
                    format!("{} · {}", esc(badge), esc(difficulty))
                },
                summary = esc(summary),
            )
        })
        .collect();
    let content = format!(
        r##"<p class="fieldnote"><a href="/beginner-track">← Beginner Track</a></p>
<h1 class="pagetitle" lang="en">Agent Lab</h1>
<p class="muted">Browser agent'ını Exposure Student Portal üzerinde test et. Form doldur, doğru projeyi bul ve görev akışlarını otomatikleştir.</p>
<div class="taskcard">
  <div class="taskhead"><h3>Bu bir test alanı</h3></div>
  <p class="desc">Buradaki üç challenge gerçek portalın kopyası olan sandbox sayfalarda çalışır. Ajanın ne yazarsa yazsın gerçek profiline, gerçek proje gönderimlerine ya da puanına dokunmaz; her challenge'ı istediğin kadar sıfırlayıp baştan çalıştırabilirsin.</p>
</div>
<div class="hubgrid">
  {cards}
</div>"##
    );
    layout("Agent Lab", Some(user), "beginner-track", &content)
}

/// Back link + heading shared by both challenge pages, so they can't drift apart.
fn agent_lab_head(badge: &str, title: &str, lead: &str) -> String {
    format!(
        r##"<p class="fieldnote"><a href="{AGENT_LAB_PATH}">← Agent Lab</a></p>
<h1 class="pagetitle" lang="en">{title}</h1>
<p class="muted">{badge} · {lead}</p>"##,
        title = esc(title),
        badge = esc(badge),
        lead = esc(lead),
    )
}

/// Challenge 1 — the sandbox profile form. Deliberately the same field vocabulary as
/// /profile so driving it teaches the real thing, but it posts to its own table.
pub fn agent_lab_profile(
    user: &User,
    saved: Option<&AgentLabProfile>,
    error: Option<&str>,
) -> String {
    let banner = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    let grade_now = saved.map(|s| s.grade.as_str()).unwrap_or("");
    let grade_opts: String = std::iter::once(String::from(r#"<option value="">Seç…</option>"#))
        .chain(GRADES.iter().map(|g| {
            let sel = if grade_now == *g { " selected" } else { "" };
            format!(r#"<option value="{g}"{sel}>{g}</option>"#)
        }))
        .collect();
    let status = match saved {
        Some(s) => format!(
            r##"<span class="substatus st-passed">Kaydedildi ✓</span>
<p class="fieldnote">Son kayıt: {when}</p>"##,
            when = s.updated_at.format("%d.%m.%Y %H:%M"),
        ),
        None => r#"<span class="substatus st-pending">Henüz doldurulmadı</span>"#.into(),
    };
    let content = format!(
        r##"{head}
<div class="tasks">
  <div class="taskcard">
    <div class="taskhead"><h3>Görev</h3></div>
    <p class="desc">Ajanına şunu yaptır: bu sayfayı aç, aşağıdaki beş alanı da doldur ve <b>Kaydet</b> düğmesine bas. Kayıt başarılıysa form bir sonraki açılışta dolu gelir ve yukarıda «Kaydedildi ✓» görünür.</p>
    <p class="desc">Başarı ölçütü: beş alan da dolu ve kaydedilmiş olacak. Boş bırakılan bir alan kaydı reddeder — ajanın hata mesajını okuyup düzeltebilmeli.</p>
    <form method="post" action="{AGENT_LAB_PATH}/reset">
      <input type="hidden" name="challenge" value="student-profile">
      <button class="btn-outline small">Testi sıfırla</button>
    </form>
  </div>
  <div class="taskcard">
    <div class="taskhead"><h3>Sandbox öğrenci profili</h3></div>
    {status}
    {banner}
    <form method="post" action="{AGENT_LAB_PATH}/student-profile">
      <label>Ad soyad<input name="full_name" value="{full_name}" placeholder="ör. Deniz Yılmaz" required></label>
      <label>Okul<input name="school" value="{school}" placeholder="ör. Test Anadolu Lisesi" required></label>
      <label>Sınıf<select name="grade" required>{grade_opts}</select></label>
      <label>İlgi alanı<input name="interest" value="{interest}" placeholder="ör. robotik" required></label>
      <label>Ajanın hedefi<input name="agent_goal" value="{agent_goal}" placeholder="ör. formu tek seferde doldurmak" required></label>
      <p class="fieldnote">Bu form sandbox verisine yazar. Gerçek profilin <a href="/profile">Profilim</a> sayfasında ve buradan etkilenmez.</p>
      <button class="btn-dark">Kaydet</button>
    </form>
  </div>
</div>"##,
        head = agent_lab_head(
            "Challenge 1",
            "Student Profile Agent",
            "Sandbox öğrenci profilini ajanına doldurt."
        ),
        full_name = esc(saved.map(|s| s.full_name.as_str()).unwrap_or("")),
        school = esc(saved.map(|s| s.school.as_str()).unwrap_or("")),
        interest = esc(saved.map(|s| s.interest.as_str()).unwrap_or("")),
        agent_goal = esc(saved.map(|s| s.agent_goal.as_str()).unwrap_or("")),
    );
    layout(
        "Student Profile Agent",
        Some(user),
        "beginner-track",
        &content,
    )
}

/// Challenge 2 — the sandbox submission form. The five projects are listed in full so the
/// agent has something to read and choose from; only one of them matches the brief.
pub fn agent_lab_submission(
    user: &User,
    saved: Option<&AgentLabSubmission>,
    error: Option<&str>,
) -> String {
    let banner = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    let picked = saved.map(|s| s.project_key.as_str()).unwrap_or("");
    let options: String = std::iter::once(String::from(r#"<option value="">Proje seç…</option>"#))
        .chain(AGENT_LAB_PROJECTS.iter().map(|(key, title, _)| {
            format!(
                r#"<option value="{key}"{sel}>{title}</option>"#,
                title = esc(title),
                sel = if picked == *key { " selected" } else { "" },
            )
        }))
        .collect();
    // `.desc` is white-space:pre-wrap, so the list is newline-separated text rather than a
    // <ul> — the reset zeroes list padding, which would clip the markers.
    let list: Vec<String> = AGENT_LAB_PROJECTS
        .iter()
        .map(|(_, title, summary)| {
            format!(
                "• <b>{title}</b> — {summary}",
                title = esc(title),
                summary = esc(summary),
            )
        })
        .collect();
    let list = list.join("\n");
    // Wrong picks are saved too, so a student can see what the agent actually chose
    // instead of only "başarısız".
    let status = match saved {
        Some(s) if s.correct => format!(
            r##"<span class="substatus st-passed">Doğru proje ✓</span>
<p class="fieldnote">Son gönderim: {when}</p>"##,
            when = s.updated_at.format("%d.%m.%Y %H:%M"),
        ),
        Some(s) => format!(
            r##"<span class="substatus st-failed">Yanlış proje</span>
<p class="fieldnote">Ajanın <b>{title}</b> gönderdi ({when}). Brifi tekrar okut ve yeniden dene.</p>"##,
            title = esc(agent_lab_project_title(&s.project_key)),
            when = s.updated_at.format("%d.%m.%Y %H:%M"),
        ),
        None => r#"<span class="substatus st-pending">Henüz gönderilmedi</span>"#.into(),
    };
    let content = format!(
        r##"{head}
<div class="tasks">
  <div class="taskcard">
    <div class="taskhead"><h3>Brif</h3></div>
    <p class="desc">{brief}</p>
    <p class="desc">{list}</p>
    <p class="desc">Başarı ölçütü: doğru proje seçilmiş, repo bağlantısı <b>https://github.com/</b> ile başlıyor ve demo bağlantısı <b>https://</b> ile başlıyor olacak.</p>
    <form method="post" action="{AGENT_LAB_PATH}/reset">
      <input type="hidden" name="challenge" value="project-submission">
      <button class="btn-outline small">Testi sıfırla</button>
    </form>
  </div>
  <div class="taskcard">
    <div class="taskhead"><h3>Sandbox proje gönderimi</h3></div>
    {status}
    {banner}
    <form method="post" action="{AGENT_LAB_PATH}/project-submission">
      <label>Proje<select name="project_key" required>{options}</select></label>
      <label>Repo bağlantısı<input name="repo_url" type="url" value="{repo}" placeholder="https://github.com/..." required></label>
      <label>Demo bağlantısı<input name="demo_url" type="url" value="{demo}" placeholder="https://..." required></label>
      <p class="fieldnote">Bu gönderim sandbox verisine yazar; puanlanmaz ve <a href="/beginner-track">Beginner Track</a> gönderimlerine karışmaz.</p>
      <button class="btn-dark">Gönder →</button>
    </form>
  </div>
</div>"##,
        head = agent_lab_head(
            "Challenge 2",
            "Project Submission Agent",
            "Ajanına doğru projeyi buldurup gönderimi tamamlat."
        ),
        brief = esc(AGENT_LAB_BRIEF),
        repo = esc(saved.map(|s| s.repo_url.as_str()).unwrap_or("")),
        demo = esc(saved.map(|s| s.demo_url.as_str()).unwrap_or("")),
    );
    layout(
        "Project Submission Agent",
        Some(user),
        "beginner-track",
        &content,
    )
}

/// Display title for a lab project key. Falls back to the key itself so a row written by
/// an older list still renders instead of taking the page down.
fn agent_lab_project_title(key: &str) -> &str {
    AGENT_LAB_PROJECTS
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, title, _)| *title)
        .unwrap_or(key)
}

// ---- Agent Lab challenge 3: Job Application Agent ----
//
// Ten sandbox internship forms. The point of the challenge is that they are NOT the same
// form ten times: an agent that hard-codes "first input is the name, second is the school"
// fails on the second posting, because Orbit opens with School and Cortex opens with Email.
//
// The variation lives in the data below — field order, label wording, input kind, which
// fields are optional — so one renderer and one validator serve all ten while the DOM they
// produce genuinely differs. Nothing here hints at which profile value belongs in which
// field: matching "Current School" / "Which school do you attend?" to the same profile line
// is the reading work the challenge exists to exercise.

/// One input on a job form. `name` is the form field name (and, prefixed with the job key,
/// the DOM id); `label` is what the student and their agent actually read.
pub struct Field {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    /// Optional fields exist so an agent meets information it cannot find in profile.md.
    /// Leaving those blank is the correct answer — never an error.
    pub required: bool,
}

/// What kind of control a `Field` renders as, and for the three choice kinds, the options
/// it accepts. Submitted values are checked against these lists server-side, so a POST
/// cannot smuggle in an option the form never offered.
pub enum FieldKind {
    Text,
    Email,
    Url,
    Textarea,
    Select(&'static [&'static str]),
    Radio(&'static [&'static str]),
    Checkbox(&'static [&'static str]),
}

pub struct JobPosting {
    pub key: &'static str,
    pub company: &'static str,
    pub role: &'static str,
    pub blurb: &'static str,
    pub fields: &'static [Field],
}

const GRADE_OPTS: &[&str] = &["9", "10", "11", "12"];
const GRAD_YEAR_OPTS: &[&str] = &["2026", "2027", "2028", "2029"];

/// Shorthand for a required field of one of the plain kinds.
const fn req(name: &'static str, label: &'static str, kind: FieldKind) -> Field {
    Field {
        name,
        label,
        kind,
        required: true,
    }
}

/// Shorthand for an optional field. Used for the two questions a standard profile.md has
/// no answer to, which is the point: the agent must leave them alone rather than invent.
const fn opt(name: &'static str, label: &'static str, kind: FieldKind) -> Field {
    Field {
        name,
        label,
        kind,
        required: false,
    }
}

/// The ten sandbox postings. Fake companies throughout — no real employer, no outside
/// request, nothing leaves the portal.
pub const AGENT_LAB_JOBS: [JobPosting; 10] = [
    JobPosting {
        key: "nova-labs",
        company: "Nova Labs",
        role: "AI Intern",
        blurb: "Küçük bir yapay zekâ ekibinde model denemelerine katıl.",
        fields: &[
            req("full_name", "Full Name", FieldKind::Text),
            req("email", "Email Address", FieldKind::Email),
            req("school", "Current School", FieldKind::Text),
            req("grade", "Grade", FieldKind::Select(GRADE_OPTS)),
            req("github", "GitHub Profile", FieldKind::Url),
            req(
                "why_ai",
                "Why are you interested in AI?",
                FieldKind::Textarea,
            ),
        ],
    },
    // School first, name second: the simplest possible break for a positional script.
    JobPosting {
        key: "orbit",
        company: "Orbit",
        role: "Product Intern",
        blurb: "Ürün ekibiyle birlikte kullanıcı geri bildirimlerini özelliklere çevir.",
        fields: &[
            req("school", "School", FieldKind::Text),
            req("full_name", "Name", FieldKind::Text),
            req("linkedin", "LinkedIn", FieldKind::Url),
            req(
                "interests",
                "Areas of Interest",
                FieldKind::Checkbox(&["Product", "Design", "Data", "Engineering", "Growth"]),
            ),
            req(
                "why",
                "Tell us why you would like to join our product team",
                FieldKind::Textarea,
            ),
            req("grade", "Current Grade", FieldKind::Radio(GRADE_OPTS)),
        ],
    },
    // Asks graduation year where the others ask grade — same profile fact, different shape.
    JobPosting {
        key: "cortex-research",
        company: "Cortex Research",
        role: "Research Assistant",
        blurb: "Araştırma ekibine literatür taraması ve deney kurulumunda destek ol.",
        fields: &[
            req("email", "Email", FieldKind::Email),
            req("full_name", "Student Name", FieldKind::Text),
            req(
                "graduation_year",
                "Graduation Year",
                FieldKind::Select(GRAD_YEAR_OPTS),
            ),
            req(
                "interest",
                "Main Academic / Technical Interest",
                FieldKind::Text,
            ),
            req("favorite_project", "Favorite Project", FieldKind::Text),
            req(
                "learn",
                "What would you like to learn during this internship?",
                FieldKind::Textarea,
            ),
        ],
    },
    JobPosting {
        key: "linearworks",
        company: "LinearWorks",
        role: "Growth Intern",
        blurb: "Büyüme ekibiyle deneyler kur, sonuçları ölç.",
        fields: &[
            req("full_name", "Full Name", FieldKind::Text),
            req("school", "School", FieldKind::Text),
            req("linkedin", "LinkedIn URL", FieldKind::Url),
            req(
                "topics",
                "What topics are you interested in?",
                FieldKind::Text,
            ),
            req("bio", "Short Bio", FieldKind::Textarea),
            req(
                "fit",
                "Why do you think you would be a good fit?",
                FieldKind::Textarea,
            ),
            opt("phone", "Phone Number (Optional)", FieldKind::Text),
        ],
    },
    JobPosting {
        key: "bytelabs",
        company: "ByteLabs",
        role: "Software Intern",
        blurb: "Üretimdeki bir web uygulamasında küçük özellikler geliştir.",
        fields: &[
            req("full_name", "Name", FieldKind::Text),
            req("email", "Email", FieldKind::Email),
            req("github", "GitHub", FieldKind::Url),
            req("grade", "Grade", FieldKind::Select(GRADE_OPTS)),
            req("favorite_project", "Favorite Project", FieldKind::Text),
            req("built", "Describe something you built", FieldKind::Textarea),
        ],
    },
    // "Portfolio / GitHub URL" — the agent has to work out that the GitHub link fits.
    JobPosting {
        key: "canvas-studio",
        company: "Canvas Studio",
        role: "Design Intern",
        blurb: "Tasarım ekibiyle arayüz ve marka çalışmalarına katıl.",
        fields: &[
            req("full_name", "Name", FieldKind::Text),
            req("school", "School", FieldKind::Text),
            req("portfolio", "Portfolio / GitHub URL", FieldKind::Url),
            req("interests", "Interests", FieldKind::Text),
            req(
                "proud",
                "Tell us about a project you are proud of",
                FieldKind::Textarea,
            ),
            req("learn", "What do you want to learn?", FieldKind::Textarea),
        ],
    },
    JobPosting {
        key: "flow-systems",
        company: "Flow Systems",
        role: "Operations Intern",
        blurb: "Operasyon ekibiyle süreçleri düzenle ve otomatikleştir.",
        fields: &[
            req("full_name", "Full Name", FieldKind::Text),
            req("email", "Email", FieldKind::Email),
            req("school", "School", FieldKind::Text),
            req(
                "grade",
                "Current Year / Grade",
                FieldKind::Radio(GRADE_OPTS),
            ),
            req("bio", "Short Bio", FieldKind::Textarea),
            req(
                "why",
                "Why are you interested in operations and startups?",
                FieldKind::Textarea,
            ),
        ],
    },
    JobPosting {
        key: "atlas-data",
        company: "Atlas Data",
        role: "Data Intern",
        blurb: "Veri ekibiyle veri setlerini temizle ve görselleştir.",
        fields: &[
            req("full_name", "Name", FieldKind::Text),
            req("email", "Email", FieldKind::Email),
            req("github", "GitHub URL", FieldKind::Url),
            req(
                "graduation_year",
                "Graduation Year",
                FieldKind::Select(GRAD_YEAR_OPTS),
            ),
            req(
                "interests",
                "Technical Interests",
                FieldKind::Checkbox(&[
                    "Data analysis",
                    "Machine learning",
                    "Visualization",
                    "Databases",
                    "Statistics",
                ]),
            ),
            req("favorite_project", "Favorite Project", FieldKind::Text),
            req("learn", "Learning Goals", FieldKind::Textarea),
        ],
    },
    JobPosting {
        key: "forge-robotics",
        company: "Forge Robotics",
        role: "Robotics Intern",
        blurb: "Robotik ekibiyle mekanik ve yazılım tarafında birlikte çalış.",
        fields: &[
            req("full_name", "Student Name", FieldKind::Text),
            req("school", "School", FieldKind::Text),
            req("grade", "Grade", FieldKind::Select(GRADE_OPTS)),
            req("linkedin", "LinkedIn", FieldKind::Url),
            req(
                "areas",
                "Areas you would like to work on",
                FieldKind::Checkbox(&[
                    "Mechanical design",
                    "Embedded software",
                    "Computer vision",
                    "Control systems",
                    "Testing",
                ]),
            ),
            req(
                "built",
                "Tell us about something you have built",
                FieldKind::Textarea,
            ),
        ],
    },
    // Deliberately the longest form, and the second one carrying an unanswerable optional.
    JobPosting {
        key: "pioneer-ventures",
        company: "Pioneer Ventures",
        role: "Startup Intern",
        blurb: "Erken aşama girişimlerle çalışan bir ekipte her işe biraz dokun.",
        fields: &[
            req("full_name", "Full Name", FieldKind::Text),
            req("email", "Email", FieldKind::Email),
            req("school", "School", FieldKind::Text),
            req("profile_url", "GitHub or LinkedIn", FieldKind::Url),
            req("bio", "Short Bio", FieldKind::Textarea),
            req("interests", "What are you interested in?", FieldKind::Text),
            req(
                "why",
                "Why do you want to work with startups?",
                FieldKind::Textarea,
            ),
            req(
                "learn",
                "What would you like to learn?",
                FieldKind::Textarea,
            ),
            opt(
                "expected_salary",
                "Expected Salary (Optional)",
                FieldKind::Text,
            ),
        ],
    },
];

/// The posting behind a key, or `None` for a URL naming a job that doesn't exist.
pub fn agent_lab_job(key: &str) -> Option<&'static JobPosting> {
    AGENT_LAB_JOBS.iter().find(|j| j.key == key)
}

/// Checkbox groups post one key per option (`interests__0`, `interests__1`, …) so a plain
/// `HashMap<String, String>` form decode keeps them all — a single repeated key would
/// collapse to one value and silently drop the rest of the student's selection.
pub fn checkbox_key(field: &str, index: usize) -> String {
    format!("{field}__{index}")
}

/// Which sandbox table "Testi sıfırla" clears, per challenge. Returning a fixed `&'static
/// str` from a closed match — never anything the caller typed — is what lets the reset
/// handler interpolate the name into its DELETE safely. `None` means "no such challenge".
pub fn agent_lab_reset_table(challenge: &str) -> Option<&'static str> {
    match challenge {
        "student-profile" => Some("agent_lab_profiles_exposure_academy"),
        "project-submission" => Some("agent_lab_submissions_exposure_academy"),
        "job-applications" => Some("agent_lab_job_applications_exposure_academy"),
        _ => None,
    }
}

/// Good enough for a sandbox form, and deliberately no stricter than the browser's own
/// `type="email"`: something before the @, something after it, and a dot inside the domain.
fn looks_like_email(s: &str) -> bool {
    match s.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && domain.contains('.')
                && !domain.contains('@')
        }
        None => false,
    }
}

/// The submitted values in stored shape, with no validation at all. Used to re-fill a form
/// after a rejected submit: the raw POST keys checkbox groups as `name__0`, `name__1`, so
/// handing the raw map straight to the renderer would silently lose every ticked box.
pub fn job_answers_from_raw(
    job: &JobPosting,
    raw: &std::collections::HashMap<String, String>,
) -> Answers {
    let mut out = Answers::new();
    for f in job.fields {
        match &f.kind {
            FieldKind::Checkbox(options) => {
                let picked: Vec<serde_json::Value> = options
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| raw.contains_key(&checkbox_key(f.name, *i)))
                    .map(|(_, o)| (*o).into())
                    .collect();
                if !picked.is_empty() {
                    out.insert(f.name.into(), picked.into());
                }
            }
            _ => {
                let v = raw.get(f.name).map(|s| s.trim()).unwrap_or("");
                if !v.is_empty() {
                    out.insert(f.name.into(), v.into());
                }
            }
        }
    }
    out
}

/// Check a submitted application against its posting and return what to store.
///
/// Runs server-side because the HTML `required` / `type` attributes are a courtesy to
/// browsers, not a control: an agent POSTing directly never sees them. Choice values are
/// checked against the posting's own option lists, so a crafted POST cannot store an option
/// the form never offered.
///
/// Optional fields left blank are correct, not errors — that is the whole point of the two
/// questions a profile.md cannot answer. They are simply omitted from the stored object.
pub fn validate_job_application(
    job: &JobPosting,
    raw: &std::collections::HashMap<String, String>,
) -> Result<Answers, String> {
    let mut out = Answers::new();
    for f in job.fields {
        let missing = || format!("{} alanı zorunlu.", f.label);
        match &f.kind {
            FieldKind::Text | FieldKind::Email | FieldKind::Url | FieldKind::Textarea => {
                let v = raw.get(f.name).map(|s| s.trim()).unwrap_or("");
                if v.is_empty() {
                    if f.required {
                        return Err(missing());
                    }
                    continue;
                }
                if matches!(f.kind, FieldKind::Email) && !looks_like_email(v) {
                    return Err(format!("{} geçerli bir e-posta olmalı.", f.label));
                }
                if matches!(f.kind, FieldKind::Url) && !v.starts_with("https://") {
                    return Err(format!("{} https:// ile başlamalı.", f.label));
                }
                out.insert(f.name.into(), v.into());
            }
            FieldKind::Select(options) | FieldKind::Radio(options) => {
                let v = raw.get(f.name).map(|s| s.trim()).unwrap_or("");
                if v.is_empty() {
                    if f.required {
                        return Err(missing());
                    }
                    continue;
                }
                if !options.contains(&v) {
                    return Err(format!(
                        "{} için listedeki seçeneklerden birini seç.",
                        f.label
                    ));
                }
                out.insert(f.name.into(), v.into());
            }
            FieldKind::Checkbox(options) => {
                let mut picked: Vec<serde_json::Value> = Vec::new();
                for (i, o) in options.iter().enumerate() {
                    let Some(v) = raw.get(&checkbox_key(f.name, i)) else {
                        continue;
                    };
                    // the box for option i may only ever carry option i's own text
                    if v.trim() != *o {
                        return Err(format!("{} için geçersiz seçim.", f.label));
                    }
                    picked.push((*o).into());
                }
                if picked.is_empty() {
                    if f.required {
                        return Err(missing());
                    }
                    continue;
                }
                out.insert(f.name.into(), picked.into());
            }
        }
    }
    Ok(out)
}

/// Saved answers, decoded. A string per plain field, a list per checkbox group.
pub type Answers = serde_json::Map<String, serde_json::Value>;

/// One saved answer as display text: the string itself, or a comma-joined list.
fn answer_text(v: Option<&serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Whether a checkbox option was among the saved selections.
fn answer_has(v: Option<&serde_json::Value>, option: &str) -> bool {
    match v {
        Some(serde_json::Value::Array(items)) => items.iter().any(|i| i.as_str() == Some(option)),
        _ => false,
    }
}

/// Render one field: a real `<label for>` bound to a real control, every time. Choice
/// groups get a `<fieldset>`/`<legend>` so the question reads as one unit to a screen
/// reader and to an agent walking the accessibility tree.
fn agent_lab_job_field(job_key: &str, f: &Field, answers: Option<&Answers>) -> String {
    let id = format!("{job_key}-{name}", name = f.name);
    let saved = answers.and_then(|a| a.get(f.name));
    let value = esc(&answer_text(saved));
    // Optionality is carried by the label text itself — every optional field's label ends
    // in "(Optional)", asserted in `optional_fields_say_so_in_their_label`. A second marker
    // appended here would just read "Expected Salary (Optional) — optional".
    let attr = if f.required { " required" } else { "" };
    match &f.kind {
        FieldKind::Text | FieldKind::Email | FieldKind::Url => {
            let ty = match f.kind {
                FieldKind::Email => "email",
                FieldKind::Url => "url",
                _ => "text",
            };
            format!(
                r##"<div class="jobfield">
  <label for="{id}">{label}</label>
  <input type="{ty}" id="{id}" name="{name}" value="{value}"{attr}>
</div>"##,
                label = esc(f.label),
                name = f.name,
            )
        }
        FieldKind::Textarea => format!(
            r##"<div class="jobfield">
  <label for="{id}">{label}</label>
  <textarea id="{id}" name="{name}" rows="4"{attr}>{value}</textarea>
</div>"##,
            label = esc(f.label),
            name = f.name,
        ),
        FieldKind::Select(options) => {
            let opts: String = std::iter::once(String::from(r#"<option value="">Seç…</option>"#))
                .chain(options.iter().map(|o| {
                    format!(
                        r#"<option value="{o}"{sel}>{o}</option>"#,
                        o = esc(o),
                        sel = if answer_text(saved) == *o {
                            " selected"
                        } else {
                            ""
                        },
                    )
                }))
                .collect();
            format!(
                r##"<div class="jobfield">
  <label for="{id}">{label}</label>
  <select id="{id}" name="{name}"{attr}>{opts}</select>
</div>"##,
                label = esc(f.label),
                name = f.name,
            )
        }
        FieldKind::Radio(options) => {
            let items: String = options
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    format!(
                        r##"<label class="checkline" for="{id}-{i}"><input type="radio" id="{id}-{i}" name="{name}" value="{o}"{sel}{attr}> {o}</label>"##,
                        o = esc(o),
                        name = f.name,
                        sel = if answer_text(saved) == *o { " checked" } else { "" },
                    )
                })
                .collect();
            format!(
                r##"<fieldset class="jobfield">
  <legend>{label}</legend>
  {items}
</fieldset>"##,
                label = esc(f.label),
            )
        }
        FieldKind::Checkbox(options) => {
            let items: String = options
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    format!(
                        r##"<label class="checkline" for="{id}-{i}"><input type="checkbox" id="{id}-{i}" name="{key}" value="{o}"{sel}> {o}</label>"##,
                        o = esc(o),
                        key = checkbox_key(f.name, i),
                        sel = if answer_has(saved, o) { " checked" } else { "" },
                    )
                })
                .collect();
            // no `required` attribute on the boxes themselves — HTML would demand every one
            // of them be ticked; "at least one" is enforced server-side instead
            format!(
                r##"<fieldset class="jobfield">
  <legend>{label}</legend>
  {items}
</fieldset>"##,
                label = esc(f.label),
            )
        }
    }
}

/// Challenge 3's landing page: the instruction block, progress, and the ten postings.
pub fn agent_lab_jobs(user: &User, done: &[String]) -> String {
    let total = AGENT_LAB_JOBS.len();
    let count = done.len();
    let cards: String = AGENT_LAB_JOBS
        .iter()
        .map(|job| {
            let completed = done.iter().any(|k| k == job.key);
            let (pill, cls, cta) = if completed {
                ("Completed ✓", "st-passed", "Continue")
            } else {
                ("Not Started", "st-pending", "Apply")
            };
            format!(
                r##"<div class="taskcard">
  <div class="taskhead"><h3>{company}</h3></div>
  <p class="desc"><b>{role}</b>
{blurb}</p>
  <p class="substatus {cls}">{pill}</p>
  <div class="cardactions">
    <a class="btn-outline small" href="{AGENT_LAB_PATH}/job-applications/{key}">{cta} →</a>
  </div>
</div>"##,
                company = esc(job.company),
                role = esc(job.role),
                blurb = esc(job.blurb),
                key = job.key,
            )
        })
        .collect();
    // The finished state is its own element rather than a styled variant of the counter, so
    // an agent can assert on "did I finish" without parsing "10 / 10" out of a sentence.
    let complete = if count == total {
        format!(
            r##"<p class="substatus st-passed" id="challenge-complete">Challenge Complete ✓</p>
  <p class="desc">{total} / {total} Applications Submitted</p>"##
        )
    } else {
        String::new()
    };
    let content = format!(
        r##"{head}
<div class="taskcard">
  <div class="taskhead"><h3>Instructions</h3></div>
  <p class="desc"><b>Goal:</b> Complete all {total} sandbox job applications using the student's profile information.</p>
  <p class="desc"><b>Rules:</b></p>
  <ul class="harness-rules">
    <li>Do not invent information.</li>
    <li>If information is not present in the profile and the field is optional, leave it blank.</li>
    <li>Read each form carefully.</li>
    <li>Verify the values before submitting.</li>
    <li>Complete all {total} applications.</li>
  </ul>
</div>
<div class="taskcard">
  <div class="taskhead"><h3>Applications Completed</h3></div>
  <p class="desc" id="applications-progress">{count} / {total}</p>
  {complete}
  <form method="post" action="{AGENT_LAB_PATH}/reset">
    <input type="hidden" name="challenge" value="job-applications">
    <button class="btn-outline small">Testi sıfırla</button>
  </form>
</div>
<div class="tasks">{cards}</div>"##,
        head = agent_lab_head(
            "Challenge 3",
            "Job Application Agent",
            "10 farklı internship başvuru formu seni bekliyor. Sorular birbirine benziyor ama her şirket aynı şeyi farklı şekilde soruyor. Browser agent'ın profile.md içindeki bilgileri kullanarak başvuruları tamamlasın."
        ),
    );
    layout(
        "Job Application Agent",
        Some(user),
        "beginner-track",
        &content,
    )
}

/// One posting's application form.
pub fn agent_lab_job_form(
    user: &User,
    job: &JobPosting,
    answers: Option<&Answers>,
    submitted: Option<chrono::DateTime<chrono::Utc>>,
    error: Option<&str>,
) -> String {
    let banner = error
        .map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
        .unwrap_or_default();
    let status = match submitted {
        Some(when) => format!(
            r##"<p class="substatus st-passed" id="application-status">Application Submitted ✓</p>
  <p class="fieldnote">Son gönderim: {when}</p>"##,
            when = when.format("%d.%m.%Y %H:%M"),
        ),
        None => {
            r##"<p class="substatus st-pending" id="application-status">Not Started</p>"##.into()
        }
    };
    let fields: String = job
        .fields
        .iter()
        .map(|f| agent_lab_job_field(job.key, f, answers))
        .collect();
    let content = format!(
        r##"<p class="fieldnote"><a href="{AGENT_LAB_PATH}/job-applications">← Job Application Agent</a></p>
<h1 class="pagetitle">{company}</h1>
<p class="muted">{role} · sandbox başvuru formu</p>
<div class="taskcard">
  {status}
  {banner}
  <form method="post" action="{AGENT_LAB_PATH}/job-applications/{key}" lang="en">
    {fields}
    <p class="fieldnote" lang="tr">Bu form Agent Lab sandbox verisine yazar. Gerçek bir başvuru gönderilmez, dışarıya hiçbir istek çıkmaz.</p>
    <button class="btn-dark">Submit Application →</button>
  </form>
</div>"##,
        company = esc(job.company),
        role = esc(job.role),
        key = job.key,
    );
    layout(
        &format!("{} — {}", job.company, job.role),
        Some(user),
        "beginner-track",
        &content,
    )
}

/// Admin view, step 1: the projects as a list, each carrying how many students have
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
        .unwrap_or((key, key, "", &[], true, 1, None));
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

fn admin_monopoly_panel(members: &[MemberRow], monopoly: &MonopolyAdmin) -> String {
    let student_opts: String = members
        .iter()
        .filter(|member| !member.is_admin)
        .map(|member| {
            format!(
                r#"<option value="{}">{}</option>"#,
                member.id,
                esc(&member.display_name)
            )
        })
        .collect();
    let team_opts: String = monopoly
        .teams
        .iter()
        .map(|team| {
            format!(
                r#"<option value="{}">{}</option>"#,
                team.id,
                esc(&team.name)
            )
        })
        .collect();
    let team_rows: String = if monopoly.teams.is_empty() {
        "<p class='muted'>Henüz takım yok</p>".into()
    } else {
        monopoly
            .teams
            .iter()
            .map(|team| {
                let member_buttons: String = monopoly
                    .members
                    .iter()
                    .filter(|member| member.team_id == team.id)
                    .map(|member| {
                        format!(
                            r#"<form method="post" action="/admin/monopoly/member/remove" class="inline">
      <input type="hidden" name="id" value="{uid}">
      <button class="btn-outline small" title="Takımdan çıkar">{name} ✕</button>
    </form>"#,
                            uid = member.user_id,
                            name = esc(&member.display_name)
                        )
                    })
                    .collect();
                let submission = monopoly
                    .submissions
                    .iter()
                    .find(|submission| submission.team_id == team.id);
                let (submission_line, submission_actions) = match submission {
                    Some(submission) => {
                        let (status, class) = monopoly_submission_status_tr(&submission.status);
                        let log = submission
                            .validation_log
                            .as_deref()
                            .filter(|log| !log.trim().is_empty())
                            .map(|log| {
                                format!(
                                    r#"<details class="build-log"><summary>Doğrulama günlüğü</summary><pre>{}</pre></details>"#,
                                    esc(log)
                                )
                            })
                            .unwrap_or_default();
                        let sha = submission
                            .commit_sha
                            .as_deref()
                            .map(|sha| esc(&sha[..sha.len().min(8)]))
                            .unwrap_or_else(|| "bekliyor".into());
                        let line = format!(
                            r##"<span class="item-meta"><a href="{repo}" target="_blank" rel="noopener">{repo_label}</a>
                        · <code>{path}</code> · <code>{sha}</code> · nesil {generation}
                        · <span class="substatus {class}">{status}</span></span>{log}"##,
                            repo = esc(&submission.repo_url),
                            repo_label = esc(
                                submission.repo_url.trim_start_matches("https://github.com/")
                            ),
                            path = esc(&submission.agent_path),
                            generation = submission.generation,
                        );
                        let disable = if submission.status != "disabled" {
                            format!(
                                r#"<form method="post" action="/admin/monopoly/submission/reject" class="inline" onsubmit="return confirm('Güncel gönderim devre dışı bırakılacak. Dondurulmuş turnuva girdileri değişmez. Emin misin?')">
      <input type="hidden" name="id" value="{}"><button class="btn-outline small">Devre dışı bırak</button></form>"#,
                                submission.id
                            )
                        } else {
                            String::new()
                        };
                        (line, disable)
                    }
                    None => (
                        r#"<span class="item-meta">gönderim yok</span>"#.to_string(),
                        String::new(),
                    ),
                };
                format!(
                    r##"<div class="itemrow">
  <div class="item-title"><span>{name}</span>{submission_line}</div>
  <div class="item-controls">{member_buttons}{submission_actions}
    <form method="post" action="/admin/monopoly/team/delete" class="inline" onsubmit="return confirm('Bu takımın üyelikleri ve güncel gönderimi silinecek. Geçmiş oyun kaydı koltuk adıyla kalır. Emin misin?')">
      <input type="hidden" name="id" value="{id}">
      <button class="btn-dark small">Sil</button>
    </form>
  </div>
</div>"##,
                    name = esc(&team.name),
                    id = team.id
                )
            })
            .collect()
    };
    let approved = monopoly
        .submissions
        .iter()
        .filter(|submission| submission.status == "approved")
        .count();
    let worker_rows: String = if monopoly.workers.is_empty() {
        r#"<p class="fieldnote">Controller henüz kaynak durumu bildirmedi.</p>"#.into()
    } else {
        monopoly
            .workers
            .iter()
            .map(|worker| {
                let ready = matches!(worker.status.as_str(), "ready" | "busy");
                let ram = worker
                    .ram_bytes
                    .map(|bytes| format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0))
                    .unwrap_or_else(|| "RAM ?".into());
                let cpu = worker
                    .effective_vcpus
                    .map(|value| format!("{value:.1} vCPU"))
                    .unwrap_or_else(|| "CPU ?".into());
                let reason = worker
                    .preflight_reason
                    .as_deref()
                    .map(|value| format!(" · {}", esc(value)))
                    .unwrap_or_default();
                format!(
                    r##"<div class="itemrow">
  <div class="item-title"><span>{name}</span><span class="item-meta">{kind} · {ram} · {cpu}{reason}</span></div>
  <span class="substatus {class}">{status}</span>
</div>"##,
                    name = esc(worker.session_name.as_deref().unwrap_or(&worker.worker_id)),
                    kind = esc(&worker.kind),
                    class = if ready { "st-passed" } else { "st-failed" },
                    status = esc(&worker.status),
                )
            })
            .collect()
    };
    let tournament_block = match &monopoly.tournament {
        Some(tournament) if tournament.status == "active" => {
            format!(
                r##"{summary}
<div class="item-controls"><a class="btn-outline small" href="/ai-monopoly?tab=live">Canlı izle</a>
<form method="post" action="/admin/monopoly/cancel" class="inline" onsubmit="return confirm('Kuyruktaki ve çalışan maçlar iptal edilecek. Emin misin?')">
<input type="hidden" name="id" value="{id}"><button class="btn-dark small">Turnuvayı durdur</button></form></div>"##,
                summary = tournament_summary(tournament),
                id = tournament.id,
            )
        }
        latest => {
            let previous = latest.as_ref().map(tournament_summary).unwrap_or_default();
            format!(
                r##"{previous}<p class="fieldnote">{approved} doğrulanmış ajan. Başlatma anında repo, commit, ajan yolu, artifact ve dependency lock dondurulur.</p>
<form method="post" action="/admin/monopoly/start" onsubmit="return confirm('Her takım altı maç oynayacak; fikstür ve gönderimler dondurulacak. Emin misin?')">
<button class="btn-dark" {disabled}>Turnuvayı başlat</button></form>"##,
                disabled = if approved < 4 { "disabled" } else { "" }
            )
        }
    };
    format!(
        r##"<section class="panel" id="monopoly-admin-panel">
  <h2 lang="en">AI Monopoly — <span lang="tr">Takımlar ve gönderimler</span></h2>
  <p class="fieldnote">Gönderimler controller doğrulamasını geçince otomatik onaylanır. Buradan durumu ve doğrulama günlüğünü izleyebilir, güncel gönderimi devre dışı bırakabilirsin.</p>
  <form method="post" action="/admin/monopoly/team">
    <label>Takım adı<input name="name" required></label>
    <button class="btn-dark">Takım oluştur</button>
  </form>
  <form method="post" action="/admin/monopoly/member">
    <label>Öğrenci<select name="user_id">{student_opts}</select></label>
    <label>Takım<select name="team_id">{team_opts}</select></label>
    <button class="btn-dark">Takıma ata</button>
  </form>
  <div class="minilist">{team_rows}</div>
  <p class="muted">Turnuva</p>
  <p class="fieldnote">Başlatınca her takımın o anki gönderimi dondurulur; sonradan yapılan değişiklikler bu turnuvayı ve geçmişini etkilemez.</p>
  {tournament_block}
  <p class="muted">Fleet kaynakları</p>
  <div class="minilist">{worker_rows}</div>
</section>"##
    )
}

pub fn admin_monopoly_page(user: &User, members: &[MemberRow], monopoly: &MonopolyAdmin) -> String {
    let panel = admin_monopoly_panel(members, monopoly);
    layout(
        "AI Monopoly (Admin)",
        Some(user),
        "monopoly-admin",
        &format!(
            r##"<div id="admin-root">
<h1 class="pagetitle" lang="en">AI Monopoly — Admin</h1>
<p class="muted">Takımları, ajan doğrulamalarını, turnuvayı ve worker kaynaklarını yönet.</p>
<div class="admingrid stack">{panel}</div>
</div>"##
        ),
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
    let monopoly_panel = admin_monopoly_panel(members, monopoly);
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

{monopoly_panel}
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
        assert!(
            html.contains(r#"href="/static/beginner-projects/09-browser-agent.pdf""#)
                && html.contains(
                    r#"href="/static/beginner-projects/09-browser-agent-cheat-sheet.pdf""#
                ),
            "proje 9 should link both its brief and its browser agent cheat sheet"
        );
        assert!(
            html.contains(r#"href="/static/beginner-projects/10-habit-tracker-mobile-app.pdf""#)
                && html.contains(
                    r#"href="/static/beginner-projects/10-expo-mobile-app-cheat-sheet.pdf""#
                ),
            "proje 10 should link both its brief and its Expo cheat sheet"
        );
        // Proje 8 is built by two students against one repo, so its card carries a brief
        // per student next to the shared git cheat sheet — all three, not a "the" brief.
        for file in [
            "08-campus-lost-and-found-student-1.pdf",
            "08-campus-lost-and-found-student-2.pdf",
            "08-group-project-cheat-sheet.pdf",
        ] {
            assert!(
                html.contains(&format!(r#"href="/static/beginner-projects/{file}""#)),
                "proje 8 should link {file}"
            );
        }
        // The ampersand in "Lost & Found" reaches the page escaped exactly once.
        assert!(html.contains("Campus Lost &amp; Found") && !html.contains("&amp;amp;"));
    }

    /// The projects are handed out over two weeks, so the page is two labelled groups and
    /// not one flat run of cards: projeler 1-7 under 1. Hafta, proje 8 under 2. Hafta.
    #[test]
    fn projects_are_grouped_by_week() {
        let html = beginner_projects(&student(), &[]);
        let (w1, w2) = (
            html.find("1. Hafta").expect("1. Hafta heading"),
            html.find("2. Hafta").expect("2. Hafta heading"),
        );
        assert!(w1 < w2, "weeks render in order");
        // Every project card falls on the correct side of the second heading.
        for (key, title, _, _, _, week, _) in BEGINNER_PROJECTS {
            let at = html
                .find(&format!(r#"value="{key}""#))
                .unwrap_or_else(|| panic!("no card for {key}"));
            match week {
                1 => assert!(at > w1 && at < w2, "{title} belongs under 1. Hafta"),
                _ => assert!(at > w2, "{title} belongs under 2. Hafta"),
            }
        }
        // Each week gets its own grid, so the two groups can't run together visually.
        assert_eq!(html.matches(r#"<div class="tasks">"#).count(), 2);
    }

    /// A week nobody is on renders nothing — no heading hanging over an empty grid.
    #[test]
    fn an_empty_week_renders_no_heading() {
        let used: Vec<u8> = BEGINNER_PROJECTS.iter().map(|(.., w, _)| *w).collect();
        let html = beginner_projects(&student(), &[]);
        for (week, label) in BEGINNER_WEEKS {
            assert_eq!(
                html.contains(label),
                used.contains(&week),
                "{label} should render only when a project is on it"
            );
        }
    }

    /// Browser Agent runs locally and deploys nowhere, so its card asks for the repo and
    /// nothing else — a required live-site field there can only be satisfied by inventing
    /// a URL. The projects that do deploy must keep both fields.
    #[test]
    fn a_project_with_no_deploy_asks_for_the_repo_only() {
        let html = beginner_projects(&student(), &[]);
        let card = |key: &str| {
            let start = html
                .find(&format!(r#"value="{key}""#))
                .unwrap_or_else(|| panic!("no card for {key}"));
            let end = html[start..].find("</form>").unwrap() + start;
            html[start..end].to_string()
        };
        assert!(
            !card("browser-agent").contains(r#"name="vercel_url""#),
            "browser agent has no live site to hand in"
        );
        assert!(
            card("smart-receipt").contains(r#"name="vercel_url""#),
            "a deployed project still asks for its live link"
        );
        assert!(!project_wants_live_url("browser-agent"));
        assert!(project_wants_live_url("smart-receipt"));
        // Unknown keys take the stricter answer rather than silently skipping validation.
        assert!(project_wants_live_url("no-such-project"));
    }

    /// Every handout named in the list has to exist under static/beginner-projects/ — a
    /// typo'd filename is a 404 the student hits, not a compile error, so pin it here.
    #[test]
    fn beginner_project_handouts_exist_on_disk() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static/beginner-projects/");
        for (key, _, _, handouts, ..) in BEGINNER_PROJECTS {
            for (_, file) in handouts {
                let path = format!("{dir}{file}");
                assert!(
                    std::path::Path::new(&path).is_file(),
                    "{key}: missing handout {file}"
                );
            }
        }
    }

    /// The capstone hands out nothing on purpose — the group picks the problem. That card is
    /// the summary and the save form, with no empty actions row left behind, and it still
    /// asks for both links like any other deployed project. Its Demo Day badge rides in the
    /// taskhead; every other card has no badge at all.
    #[test]
    fn the_capstone_has_no_actions_row_but_wears_its_badge() {
        let html = beginner_projects(&student(), &[]);
        let card = |key: &str| {
            let start = html
                .find(&format!(r#"value="{key}""#))
                .unwrap_or_else(|| panic!("no card for {key}"));
            // Back up to the card's own opening tag — the form sits at the card's end.
            let open = html[..start].rfind(r#"<div class="taskcard">"#).unwrap();
            let end = html[start..].find("</form>").unwrap() + start;
            html[open..end].to_string()
        };
        let capstone = card("kendi-projen-1");
        assert!(
            !capstone.contains("cardactions"),
            "the capstone has nothing to hand out"
        );
        assert!(
            capstone.contains(r#"name="repo_url""#) && capstone.contains(r#"name="vercel_url""#),
            "the capstone still hands in a repo and a live link"
        );
        assert!(project_wants_live_url("kendi-projen-1"));
        assert!(
            capstone.contains(r#"<span class="badge">Demo Day</span>"#),
            "the capstone is the one that goes on stage"
        );
        // The group size and the Demo Day promise are the whole brief, so they have to be
        // on the card — there is no PDF to fall back on.
        assert!(capstone.contains("3 kişilik") && capstone.contains("Demo Day'de sahnede"));
        // A project that does have handouts keeps its actions row and wears no badge.
        let receipt = card("smart-receipt");
        assert!(receipt.contains("cardactions") && !receipt.contains("badge"));
        assert_eq!(
            html.matches(r#"<span class="badge">"#).count(),
            BEGINNER_PROJECTS.iter().filter(|(.., b)| b.is_some()).count(),
            "a badge renders for exactly the projects that carry one"
        );
    }

    /// The hub is three peer hubcards (projects, chatbot, agent lab), same shape as
    /// advanced_track()'s hubcard layout — not a floating card above a flat list.
    #[test]
    fn beginner_track_hub_has_three_subsets() {
        let user = student();
        let html = beginner_track(&user, 3, 2);
        assert!(html.contains(r#"href="/beginner-track/projects""#));
        assert!(html.contains(r#"href="/chatbot-challenge""#));
        assert!(html.contains(&format!(r#"href="{AGENT_LAB_PATH}""#)));
        assert_eq!(
            html.matches("hubcard").count(),
            3,
            "exactly three peer subsets"
        );
        assert!(html.contains(&format!("3/{}", BEGINNER_PROJECTS.len())));
        assert!(html.contains("seviye 2/7"));
    }

    /// Agent Lab is reachable from the track hub and nowhere else — no sidebar item of its
    /// own. Both halves matter: losing the card strands the lab, and a sidebar entry is
    /// what the section was explicitly not to become.
    #[test]
    fn agent_lab_is_reachable_from_beginner_track_only() {
        let user = student();
        // the sidebar ships on every page, so any page renders the whole nav
        let track = beginner_track(&user, 0, 1);
        assert!(
            !track.contains(r#"<span>Agent Lab</span>"#),
            "Agent Lab must not appear as a sidebar item"
        );
        // …and inside the lab the sidebar still highlights Beginner Track
        let hub = agent_lab(&user);
        assert!(
            hub.contains(r#"<a href="/beginner-track" class="active">"#),
            "the lab is a Beginner Track sub-page, so that nav entry stays active"
        );
        for (slug, _, title, _, _) in AGENT_LAB_CHALLENGES {
            assert!(
                hub.contains(&format!(r#"href="{AGENT_LAB_PATH}/{slug}""#)) && hub.contains(title),
                "the lab hub should list {title}"
            );
        }
    }

    /// The lab's project list is the whole of challenge 2's search space, and the brief's
    /// target has to be findable in it. A rename on one side and not the other would leave
    /// a challenge nobody can pass.
    #[test]
    fn agent_lab_target_is_one_of_its_projects() {
        assert!(
            AGENT_LAB_PROJECTS
                .iter()
                .any(|(k, ..)| *k == AGENT_LAB_TARGET),
            "the brief's target must be on the list the student picks from"
        );
        // Sharing a key with a real Beginner Track project is how lab data would start
        // looking like coursework; the two vocabularies stay disjoint.
        for (lab_key, ..) in AGENT_LAB_PROJECTS {
            assert!(
                !BEGINNER_PROJECTS.iter().any(|(k, ..)| *k == lab_key),
                "lab project {lab_key} collides with a real Beginner Track project key"
            );
        }
    }

    /// The whole point of challenge 2 is that the agent reasons from a description to a
    /// project. A brief that spells out its answer's name — in either language — turns
    /// that into a string match and the challenge stops testing anything.
    #[test]
    fn agent_lab_brief_does_not_name_its_answer() {
        let brief = AGENT_LAB_BRIEF.to_lowercase();
        let answer = agent_lab_project_title(AGENT_LAB_TARGET).to_lowercase();
        assert!(
            !brief.contains(&answer),
            "the brief gives away {answer:?} verbatim"
        );
        // the Turkish name is what a student would reach for, and it is nowhere in the
        // project list for the loop above to have caught
        assert!(
            !brief.contains("kişisel web sitesi"),
            "the brief gives away the answer in Turkish"
        );
        // and the page must still show the answer as a choosable option
        let html = agent_lab_submission(&student(), None, None);
        assert!(html.contains(&format!(r#"<option value="{AGENT_LAB_TARGET}""#)));
        assert_eq!(
            html.matches(r#"<option value="lab-"#).count(),
            AGENT_LAB_PROJECTS.len(),
            "all five sandbox projects should be selectable"
        );
    }

    /// A wrong pick is kept and named back to the student — "yanlış proje" with no clue
    /// which one the agent chose is useless for debugging a run.
    #[test]
    fn agent_lab_submission_reports_the_wrong_pick_by_name() {
        let wrong = AGENT_LAB_PROJECTS
            .iter()
            .find(|(k, ..)| *k != AGENT_LAB_TARGET)
            .unwrap();
        let sub = AgentLabSubmission {
            project_key: wrong.0.into(),
            repo_url: "https://github.com/a/b".into(),
            demo_url: "https://a.vercel.app".into(),
            correct: false,
            updated_at: chrono::DateTime::<chrono::Utc>::MIN_UTC,
        };
        let html = agent_lab_submission(&student(), Some(&sub), None);
        assert!(html.contains("st-failed") && html.contains(wrong.1));
        assert!(
            !html.contains("st-passed"),
            "a wrong pick must not also read as passed"
        );
    }

    // ---- Agent Lab challenge 3: Job Application Agent ----

    /// A filled-in application for one posting: every required field answered with
    /// something that passes its kind's rules, optional fields left alone.
    fn job_answers(job: &JobPosting) -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        for f in job.fields {
            if !f.required {
                continue;
            }
            match &f.kind {
                FieldKind::Email => m.insert(f.name.into(), "deniz@example.com".into()),
                FieldKind::Url => m.insert(f.name.into(), "https://github.com/deniz".into()),
                FieldKind::Text | FieldKind::Textarea => m.insert(f.name.into(), "cevap".into()),
                FieldKind::Select(o) | FieldKind::Radio(o) => m.insert(f.name.into(), o[0].into()),
                FieldKind::Checkbox(o) => m.insert(checkbox_key(f.name, 0), o[0].into()),
            };
        }
        m
    }

    #[test]
    fn ten_jobs_render_with_status_and_progress() {
        let none: Vec<String> = vec![];
        let html = agent_lab_jobs(&student(), &none);
        for job in &AGENT_LAB_JOBS {
            assert!(html.contains(job.company), "{} missing", job.company);
            assert!(html.contains(&format!(
                r#"href="{AGENT_LAB_PATH}/job-applications/{}""#,
                job.key
            )));
        }
        assert_eq!(AGENT_LAB_JOBS.len(), 10);
        assert!(html.contains("0 / 10"));
        assert_eq!(html.matches("Not Started").count(), 10);
        assert!(
            !html.contains("challenge-complete"),
            "nothing submitted yet — the finished state must not show"
        );

        // partial progress: three done, seven to go
        let done: Vec<String> = AGENT_LAB_JOBS[..3].iter().map(|j| j.key.into()).collect();
        let part = agent_lab_jobs(&student(), &done);
        assert!(part.contains("3 / 10"));
        assert_eq!(part.matches("Completed ✓").count(), 3);
        assert_eq!(part.matches("Not Started").count(), 7);
        assert!(!part.contains("challenge-complete"));

        // and the completion state, which an agent asserts on to know it is finished
        let all: Vec<String> = AGENT_LAB_JOBS.iter().map(|j| j.key.into()).collect();
        let full = agent_lab_jobs(&student(), &all);
        assert!(full.contains(r#"id="challenge-complete""#));
        assert!(full.contains("Challenge Complete ✓"));
        assert!(full.contains("10 / 10 Applications Submitted"));
    }

    /// The challenge only works if the ten forms genuinely differ. A positional script
    /// ("first input is the name") must break, and every input kind must appear somewhere,
    /// or whole categories of the exercise go untested.
    #[test]
    fn the_ten_forms_are_not_the_same_form() {
        let firsts: std::collections::HashSet<&str> =
            AGENT_LAB_JOBS.iter().map(|j| j.fields[0].name).collect();
        assert!(
            firsts.len() > 1,
            "every form opens with the same field — a positional script would pass"
        );
        // the same profile fact asked under different wording
        let school_labels: std::collections::HashSet<&str> = AGENT_LAB_JOBS
            .iter()
            .flat_map(|j| j.fields)
            .filter(|f| f.name == "school")
            .map(|f| f.label)
            .collect();
        assert!(school_labels.len() > 1, "school is always worded the same");

        let (mut sel, mut radio, mut check, mut area, mut email, mut url, mut optional) =
            (false, false, false, false, false, false, 0);
        for f in AGENT_LAB_JOBS.iter().flat_map(|j| j.fields) {
            match f.kind {
                FieldKind::Select(_) => sel = true,
                FieldKind::Radio(_) => radio = true,
                FieldKind::Checkbox(_) => check = true,
                FieldKind::Textarea => area = true,
                FieldKind::Email => email = true,
                FieldKind::Url => url = true,
                FieldKind::Text => {}
            }
            if !f.required {
                optional += 1;
            }
        }
        assert!(
            sel && radio && check && area && email && url,
            "a kind is unused"
        );
        assert!(optional >= 2, "at least two unanswerable optional fields");

        // field names must be unique within a form, or answers would overwrite each other
        for job in &AGENT_LAB_JOBS {
            let names: std::collections::HashSet<&str> =
                job.fields.iter().map(|f| f.name).collect();
            assert_eq!(names.len(), job.fields.len(), "{} repeats a name", job.key);
        }
    }

    /// Real labels bound to real controls — this is what a browser agent walks. Also the
    /// negative: nothing on the page may hint at which profile value belongs where.
    #[test]
    fn job_forms_are_semantic_and_leak_no_answers() {
        for job in &AGENT_LAB_JOBS {
            let html = agent_lab_job_form(&student(), job, None, None, None);
            for f in job.fields {
                let id = format!("{}-{}", job.key, f.name);
                match f.kind {
                    // choice groups are a fieldset/legend, with a label per option
                    FieldKind::Radio(_) | FieldKind::Checkbox(_) => {
                        assert!(
                            html.contains(&format!(r#"for="{id}-0""#)),
                            "{id} option label"
                        );
                        assert!(html.contains("<legend>"), "{id} needs a legend");
                    }
                    _ => assert!(
                        html.contains(&format!(r#"<label for="{id}">"#)),
                        "{id} has no bound label"
                    ),
                }
                assert!(html.contains(f.label), "{} label text missing", f.name);
            }
            assert!(html.contains("<button class=\"btn-dark\">"), "real button");
            for leak in [
                "data-answer",
                "data-correct",
                "data-expected",
                "type=\"hidden\"",
            ] {
                assert!(!html.contains(leak), "{} leaks via {leak}", job.key);
            }
            // placeholders must not stand in for labels
            assert!(
                !html.contains("placeholder="),
                "{} uses placeholders",
                job.key
            );
        }
    }

    /// The page declares `<html lang="tr">`, and `text-transform:uppercase` is
    /// language-sensitive: under Turkish casing rules a lowercase `i` uppercases to `İ`, so
    /// an English label styled with the portal's uppercase eyebrow renders "LİNKEDIN" and
    /// "JOİN OUR PRODUCT TEAM". Marking the English subtrees `lang="en"` is what keeps the
    /// dot off. Every string that lands in an uppercased class needs it.
    #[test]
    fn english_labels_are_not_uppercased_with_turkish_rules() {
        // .portal label and fieldset legend are both uppercase; the form owns them all.
        // Anchored on the action, because the sidebar ships a <form method="post"> of its
        // own (logout) that would otherwise be the first match on every page.
        for job in &AGENT_LAB_JOBS {
            let html = agent_lab_job_form(&student(), job, None, None, None);
            assert!(
                html.contains(&format!(
                    r#"<form method="post" action="{AGENT_LAB_PATH}/job-applications/{}" lang="en">"#,
                    job.key
                )),
                "{}: the form's English labels need lang=\"en\"",
                job.key
            );
            // the Turkish sentence inside that English form says so for itself
            assert!(
                html.contains(r#"class="fieldnote" lang="tr""#),
                "{}",
                job.key
            );
        }
        // .hubstat is uppercase too, and "Intermediate" is full of dotted i's
        let hub = agent_lab(&student());
        assert!(
            hub.contains(r#"<span class="hubstat" lang="en">Challenge 3 · Intermediate</span>"#)
        );
    }

    /// `.fieldnote` carries `margin:-10px`, which tucks it under the field above — it is a
    /// note *about a field*, and challenges 1 and 2 both place it before their submit
    /// button. After a button it rides 10px up over it, which is what shipped and looked
    /// broken. Keep it inside the form, ahead of the button.
    #[test]
    fn the_sandbox_note_sits_above_the_submit_button() {
        let html = agent_lab_job_form(
            &student(),
            agent_lab_job("orbit").unwrap(),
            None,
            None,
            None,
        );
        let note = html.find("Bu form Agent Lab sandbox").unwrap();
        let button = html.find("Submit Application").unwrap();
        assert!(note < button, "the note must not follow the button");
        assert!(
            html[note..button].contains("</p>"),
            "note and button must be separate blocks"
        );
    }

    /// Optionality is communicated by the label alone now, so a label that forgets to say
    /// so leaves an agent no way to know the field may be skipped.
    #[test]
    fn optional_fields_say_so_in_their_label() {
        let mut found = 0;
        for f in AGENT_LAB_JOBS.iter().flat_map(|j| j.fields) {
            if f.required {
                continue;
            }
            found += 1;
            assert!(
                f.label.contains("(Optional)"),
                "optional field {:?} does not say so in its label",
                f.label
            );
        }
        assert!(found >= 2);
    }

    #[test]
    fn job_validation_accepts_a_complete_application() {
        for job in &AGENT_LAB_JOBS {
            let answers = validate_job_application(job, &job_answers(job))
                .unwrap_or_else(|e| panic!("{} rejected a valid application: {e}", job.key));
            // optional fields were left blank, so they are simply absent
            let required = job.fields.iter().filter(|f| f.required).count();
            assert_eq!(answers.len(), required, "{}", job.key);
        }
    }

    /// Blank optionals are the correct answer, not an error — an agent that cannot find
    /// "Expected Salary" in profile.md must be able to submit without inventing one.
    #[test]
    fn job_validation_allows_blank_optional_fields() {
        let with_optional: Vec<&JobPosting> = AGENT_LAB_JOBS
            .iter()
            .filter(|j| j.fields.iter().any(|f| !f.required))
            .collect();
        assert!(with_optional.len() >= 2);
        for job in with_optional {
            // absent entirely
            assert!(validate_job_application(job, &job_answers(job)).is_ok());
            // present but empty, which is what a browser actually posts
            let mut m = job_answers(job);
            for f in job.fields.iter().filter(|f| !f.required) {
                m.insert(f.name.into(), "   ".into());
                let ok = validate_job_application(job, &m).unwrap();
                assert!(
                    !ok.contains_key(f.name),
                    "blank optional should not be stored"
                );
            }
        }
    }

    #[test]
    fn job_validation_rejects_bad_input() {
        let nova = agent_lab_job("nova-labs").unwrap();
        // a missing required field
        let mut m = job_answers(nova);
        m.remove("full_name");
        assert!(validate_job_application(nova, &m).is_err());
        // blank is the same as missing
        let mut m = job_answers(nova);
        m.insert("full_name".into(), "  ".into());
        assert!(validate_job_application(nova, &m).is_err());
        // malformed email
        for bad in ["deniz", "deniz@", "@example.com", "deniz@example"] {
            let mut m = job_answers(nova);
            m.insert("email".into(), bad.into());
            assert!(
                validate_job_application(nova, &m).is_err(),
                "{bad} accepted"
            );
        }
        // a URL that isn't https
        for bad in ["github.com/deniz", "http://github.com/deniz"] {
            let mut m = job_answers(nova);
            m.insert("github".into(), bad.into());
            assert!(
                validate_job_application(nova, &m).is_err(),
                "{bad} accepted"
            );
        }
        // a select value the form never offered
        let mut m = job_answers(nova);
        m.insert("grade".into(), "13".into());
        assert!(validate_job_application(nova, &m).is_err());

        // a radio value the form never offered
        let orbit = agent_lab_job("orbit").unwrap();
        let mut m = job_answers(orbit);
        m.insert("grade".into(), "üniversite".into());
        assert!(validate_job_application(orbit, &m).is_err());
        // a checkbox slot carrying someone else's value
        let mut m = job_answers(orbit);
        m.insert(checkbox_key("interests", 1), "Product".into());
        assert!(validate_job_application(orbit, &m).is_err());
        // every box unticked, where the group is required
        let mut m = job_answers(orbit);
        m.remove(&checkbox_key("interests", 0));
        assert!(validate_job_application(orbit, &m).is_err());

        // an unknown job key is a 404 at the handler, not a stored row
        assert!(agent_lab_job("goldman-sachs").is_none());
    }

    /// Multi-select answers have to survive the trip out to JSON and back, or re-opening a
    /// completed application would quietly drop the ticked boxes.
    #[test]
    fn checkbox_answers_round_trip_into_the_form() {
        let orbit = agent_lab_job("orbit").unwrap();
        let mut m = job_answers(orbit);
        m.insert(checkbox_key("interests", 2), "Data".into());
        let answers = validate_job_application(orbit, &m).unwrap();
        let encoded = serde_json::Value::Object(answers).to_string();
        let decoded: Answers = serde_json::from_str(&encoded).unwrap();
        let html = agent_lab_job_form(
            &student(),
            orbit,
            Some(&decoded),
            Some(chrono::Utc::now()),
            None,
        );
        // both ticked boxes come back checked, the untouched ones do not
        assert_eq!(html.matches("checked").count(), 3, "2 checkboxes + 1 radio");
        assert!(html.contains(r#"value="Product" checked"#));
        assert!(html.contains(r#"value="Data" checked"#));
        assert!(html.contains(r#"id="application-status""#));
        assert!(html.contains("Application Submitted ✓"));
    }

    /// Reset is routed by challenge slug. Every challenge must map to its own table, and
    /// nothing else may map at all — this is also what keeps the DELETE's interpolated
    /// table name out of the caller's hands.
    #[test]
    fn reset_maps_each_challenge_to_its_own_table() {
        let mut seen = std::collections::HashSet::new();
        for (slug, ..) in AGENT_LAB_CHALLENGES {
            let table =
                agent_lab_reset_table(slug).unwrap_or_else(|| panic!("{slug} has no reset table"));
            assert!(seen.insert(table), "{table} is reset by two challenges");
        }
        assert_eq!(
            agent_lab_reset_table("job-applications"),
            Some("agent_lab_job_applications_exposure_academy")
        );
        // a challenge's reset must not reach another challenge's data
        assert_ne!(
            agent_lab_reset_table("job-applications"),
            agent_lab_reset_table("student-profile")
        );
        assert_ne!(
            agent_lab_reset_table("job-applications"),
            agent_lab_reset_table("project-submission")
        );
        for junk in [
            "",
            "users",
            "profile",
            "beginner-track",
            "'; drop table x --",
        ] {
            assert!(agent_lab_reset_table(junk).is_none(), "{junk} resolved");
        }
    }

    /// The lab's three tables are its own. If a job key ever collided with a real Beginner
    /// Track project key, sandbox rows would start looking like coursework.
    #[test]
    fn job_keys_stay_out_of_the_real_project_vocabulary() {
        for job in &AGENT_LAB_JOBS {
            assert!(
                !BEGINNER_PROJECTS.iter().any(|(k, ..)| *k == job.key),
                "{} collides with a real Beginner Track project key",
                job.key
            );
            assert!(!AGENT_LAB_PROJECTS.iter().any(|(k, ..)| *k == job.key));
        }
        let keys: std::collections::HashSet<&str> = AGENT_LAB_JOBS.iter().map(|j| j.key).collect();
        assert_eq!(keys.len(), AGENT_LAB_JOBS.len(), "duplicate job key");
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

    #[test]
    fn monopoly_admin_has_its_own_management_page() {
        let monopoly = MonopolyAdmin {
            teams: Vec::new(),
            members: Vec::new(),
            submissions: Vec::new(),
            tournament: None,
            workers: Vec::new(),
        };
        let page = admin_monopoly_page(&admin_user(), &[], &monopoly);
        assert!(page.contains(r#"href="/admin/monopoly" class="active""#));
        assert!(page.contains("AI Monopoly (Admin)"));
        assert!(page.contains(r#"action="/admin/monopoly/team""#));
        assert!(page.contains("otomatik onaylanır"));
    }

    #[test]
    fn monopoly_instructions_define_one_repo_contract() {
        let html = monopoly_instructions(&student());
        assert!(html.contains("Public GitHub repo"));
        assert!(html.contains("agent.py") && html.contains("choose_action"));
        assert!(html.contains("250 MiB") && html.contains("iki saniye"));
        assert!(!html.contains("org/model") && !html.contains("64 GB"));
    }

    #[test]
    fn monopoly_replay_page_does_not_embed_untrusted_seat_json() {
        let game = MonopolyGame {
            id: Uuid::nil(),
            tournament_id: Uuid::nil(),
            game_no: 1,
            status: "done".into(),
            seed: 42,
            attempt_count: 1,
            seats: serde_json::json!([{
                "player_id": 0,
                "entry_id": null,
                "bot_key": "hoarder",
                "label": "</script><img src=x onerror=alert(1)>"
            }]),
            final_snapshot: None,
            round: 1,
            action_count: 10,
            winner_seat: Some(0),
            winner_entry_id: None,
            end_reason: Some("normal".into()),
            duration_us: Some(1_000_000),
            error_log: None,
            created_at: chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap(),
            started_at: None,
            finished_at: None,
        };
        let html = monopoly_game_page(
            &student(),
            &game,
            &[(
                0,
                "<img src=x onerror=alert(1)>".into(),
                "</pre><script>alert(2)</script>".into(),
            )],
        );
        assert!(!html.contains("</script><img"));
        assert!(!html.contains("<script>alert(2)</script>"));
        assert!(html.contains("&lt;script&gt;alert(2)&lt;/script&gt;"));
        assert!(html.contains(r#"data-game-id="00000000-0000-0000-0000-000000000000""#));
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

    /// AI Monopoly is no longer a placeholder: a rostered student reaches the real repo
    /// form while the Advanced Track parent stays active in the sidebar.
    #[test]
    fn monopoly_is_open_to_a_rostered_student() {
        let team = MonopolyTeam {
            id: Uuid::nil(),
            name: "Test Takımı".into(),
        };
        let page = monopoly_main(&viewer(), Some(&team), &[], None, None, None, &[]);
        assert!(page.contains(r#"action="/ai-monopoly/submit""#));
        assert!(page.contains("Public GitHub repo"));
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

    /// Same idea as `render_harness_pages`, for the Beginner Track / Agent Lab screens —
    /// they are all database-free too, so the card design and the mobile stack are
    /// checkable in a browser without standing up a server.
    /// `AGENT_LAB_RENDER_DIR=/tmp/a cargo test -p academy -- --ignored render_agent_lab`
    #[test]
    #[ignore]
    fn render_agent_lab_pages() {
        let Ok(dir) = std::env::var("AGENT_LAB_RENDER_DIR") else {
            panic!("set AGENT_LAB_RENDER_DIR to the output directory");
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let user = viewer();
        let profile = AgentLabProfile {
            full_name: "Deniz Yılmaz".into(),
            school: "Test Anadolu Lisesi".into(),
            grade: GRADES[1].into(),
            interest: "robotik".into(),
            agent_goal: "formu tek seferde doldurmak".into(),
            updated_at: chrono::Utc::now(),
        };
        let passed = AgentLabSubmission {
            project_key: AGENT_LAB_TARGET.into(),
            repo_url: "https://github.com/deniz/lab".into(),
            demo_url: "https://lab-deniz.vercel.app".into(),
            correct: true,
            updated_at: chrono::Utc::now(),
        };
        // any project that isn't the answer — pinned to the list so renaming a project
        // can't leave this dumping an unrenderable key
        let wrong = AgentLabSubmission {
            project_key: AGENT_LAB_PROJECTS
                .iter()
                .find(|(k, ..)| *k != AGENT_LAB_TARGET)
                .unwrap()
                .0
                .into(),
            repo_url: passed.repo_url.clone(),
            demo_url: passed.demo_url.clone(),
            correct: false,
            updated_at: passed.updated_at,
        };
        let pages: Vec<(&str, String)> = vec![
            ("beginner-track", beginner_track(&user, 3, 2)),
            ("beginner-projects", beginner_projects(&user, &[])),
            ("agent-lab", agent_lab(&user)),
            ("challenge1-empty", agent_lab_profile(&user, None, None)),
            (
                "challenge1-saved",
                agent_lab_profile(&user, Some(&profile), None),
            ),
            (
                "challenge1-error",
                agent_lab_profile(&user, None, Some("Beş alanın da dolu olması gerekiyor.")),
            ),
            ("challenge2-empty", agent_lab_submission(&user, None, None)),
            (
                "challenge2-wrong",
                agent_lab_submission(&user, Some(&wrong), None),
            ),
            (
                "challenge2-passed",
                agent_lab_submission(&user, Some(&passed), None),
            ),
            ("challenge3-jobs-empty", agent_lab_jobs(&user, &[])),
            (
                "challenge3-jobs-partial",
                agent_lab_jobs(
                    &user,
                    &AGENT_LAB_JOBS[..3]
                        .iter()
                        .map(|j| j.key.to_string())
                        .collect::<Vec<_>>(),
                ),
            ),
            (
                "challenge3-jobs-complete",
                agent_lab_jobs(
                    &user,
                    &AGENT_LAB_JOBS
                        .iter()
                        .map(|j| j.key.to_string())
                        .collect::<Vec<_>>(),
                ),
            ),
            (
                "challenge3-form-orbit",
                agent_lab_job_form(&user, agent_lab_job("orbit").unwrap(), None, None, None),
            ),
            (
                "challenge3-form-pioneer",
                agent_lab_job_form(
                    &user,
                    agent_lab_job("pioneer-ventures").unwrap(),
                    None,
                    None,
                    Some("Email geçerli bir e-posta olmalı."),
                ),
            ),
        ];
        for (name, html) in pages {
            std::fs::write(dir.join(format!("{name}.html")), html).unwrap();
        }
        eprintln!("wrote agent lab pages to {}", dir.display());
    }
}
