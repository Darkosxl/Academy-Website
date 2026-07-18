// Server-rendered pages. Turkish strings sourced from Google Translate API.
// ponytail: string templates, no template engine — 8 pages, full control.

use crate::model::*;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub const LEVELS: [(&str, &str); 3] = [("PRESEED", "PRESEED"), ("SEED", "SEED"), ("SERIES_A", "SERIES A")];

pub fn level_name(l: &str) -> &'static str {
    LEVELS.iter().find(|(k, _)| *k == l).map(|(_, v)| *v).unwrap_or("?")
}

// Heroicons v2 (outline, 24x24, 1.5 stroke) — sized/colored via CSS (currentColor).
fn ico(path: &str) -> String {
    format!(r##"<svg class="ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="{path}"/></svg>"##)
}
const P_HOME: &str = "m2.25 12 8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25";
const P_BOARD: &str = "M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 0 0 2.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 0 0-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25Z";
const P_HARNESS: &str = "M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 0 0 2.25-2.25V6.75a2.25 2.25 0 0 0-2.25-2.25H6.75A2.25 2.25 0 0 0 4.5 6.75v10.5a2.25 2.25 0 0 0 2.25 2.25Zm.75-12h9v9h-9v-9Z";
const P_MONOPOLY: &str = "M14.25 6.087c0-.355.186-.676.401-.959.221-.29.349-.634.349-1.003 0-1.036-1.007-1.875-2.25-1.875s-2.25.84-2.25 1.875c0 .369.128.713.349 1.003.215.283.401.604.401.959v0a.64.64 0 0 1-.657.643 48.39 48.39 0 0 1-4.163-.3c.186 1.613.293 3.25.315 4.907a.656.656 0 0 1-.658.663v0c-.355 0-.676-.186-.959-.401a1.647 1.647 0 0 0-1.003-.349c-1.036 0-1.875 1.007-1.875 2.25s.84 2.25 1.875 2.25c.369 0 .713-.128 1.003-.349.283-.215.604-.401.959-.401v0c.31 0 .555.26.532.57a48.039 48.039 0 0 1-.642 5.056c1.518.19 3.058.309 4.616.354a.64.64 0 0 0 .657-.643v0c0-.355-.186-.676-.401-.959a1.647 1.647 0 0 1-.349-1.003c0-1.035 1.008-1.875 2.25-1.875 1.243 0 2.25.84 2.25 1.875 0 .369-.128.713-.349 1.003-.215.283-.4.604-.4.959v0c0 .333.277.599.61.58a48.1 48.1 0 0 0 5.427-.63 48.05 48.05 0 0 0 .582-4.717.532.532 0 0 0-.533-.57v0c-.355 0-.676.186-.959.401-.29.221-.634.349-1.003.349-1.035 0-1.875-1.007-1.875-2.25s.84-2.25 1.875-2.25c.37 0 .713.128 1.003.349.283.215.604.401.96.401v0a.656.656 0 0 0 .658-.663 48.422 48.422 0 0 0-.37-5.36c-1.676.24-3.37.404-5.082.484a.638.638 0 0 1-.667-.643v0Z";
const P_ADMIN: &str = "M11.42 15.17 17.25 21A2.652 2.652 0 0 0 21 17.25l-5.877-5.877M11.42 15.17l2.496-3.03c.317-.384.74-.626 1.208-.766M11.42 15.17l-4.655 5.653a2.548 2.548 0 1 1-3.586-3.586l6.837-5.63m5.108-.233c.55-.164 1.163-.188 1.743-.14a4.5 4.5 0 0 0 4.486-6.336l-3.276 3.277a3.004 3.004 0 0 1-2.25-2.25l3.276-3.276a4.5 4.5 0 0 0-6.336 4.486c.091 1.076-.071 2.264-.904 2.95l-.102.085m-1.745 1.437L5.909 7.5H4.5L2.25 3.75l1.5-1.5L7.5 4.5v1.409l4.26 4.26m-1.745 1.437 1.745-1.437m6.615 8.206L15.75 15.75M4.867 19.125h.008v.008h-.008v-.008Z";
const P_LOGOUT: &str = "M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15M12 9l-3 3m0 0 3 3m-3-3h12.75";

fn nav_link(href: &str, page: &str, key: &str, icon: &str, label: &str) -> String {
    let active = if page == key { "active" } else { "" };
    format!(r#"<a href="{href}" class="{active}">{icon}<span>{label}</span></a>"#)
}

fn layout(title: &str, user: Option<&User>, active: &str, content: &str) -> String {
    let shell = match user {
        Some(u) => {
            let admin_block = if u.is_admin {
                format!(
                    r#"<div class="sb-head">Yönetim</div>{}"#,
                    nav_link("/admin", active, "admin", &ico(P_ADMIN), "Yönetici paneli")
                )
            } else {
                String::new()
            };
            format!(
                r##"<aside class="sidebar">
  <div class="sb-brand">
    <a href="/app"><img class="sb-logo" src="/static/exposure-logo.svg" alt="Exposure"></a>
    <span class="portal-pill">AI Academy</span>
  </div>
  <nav class="sb-nav">
    {home}
    {board}
    <div class="sb-head">Haftalar</div>
    {harness}
    {monopoly}
    {admin_block}
  </nav>
  <div class="sb-footer">
    <div class="sb-user">
      <span class="avatar-fb">{initial}</span>
      <span class="sb-name">{name}</span>
      <form method="post" action="/logout"><button class="sb-logout" title="Oturumu kapat">{logout_ico}</button></form>
    </div>
  </div>
</aside>
<main class="portal-main"><div class="portal-inner">
{content}
</div></main>"##,
                home = nav_link("/app", active, "home", &ico(P_HOME), "Ana Sayfa"),
                board = nav_link("/board", active, "board", &ico(P_BOARD), "Görev Panosu"),
                harness = nav_link("/agentic-harness", active, "agentic-harness", &ico(P_HARNESS), "Agentic Harness (1. Hafta)"),
                monopoly = nav_link("/ai-monopoly", active, "ai-monopoly", &ico(P_MONOPOLY), "AI Monopoly (2. Hafta)"),
                admin_block = admin_block,
                initial = esc(&u.display_name.chars().next().unwrap_or('?').to_string()),
                name = esc(&u.display_name),
                logout_ico = ico(P_LOGOUT),
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
<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Geist:wght@100..900&display=swap" rel="stylesheet">
<link rel="stylesheet" href="/static/style.css?v=3">
</head>
<body class="{body_class}">
{shell}
</body>
</html>"##,
        title = esc(title),
    )
}

pub fn landing() -> String {
    layout("Akademi", None, "", r##"
<section class="hero">
  <div class="pill"><span class="dot"></span> Video Dersleri</div>
  <h1>Yapay Zekayı<br><em>Projelerle Öğren!</em></h1>
  <p class="sub">PRESEED · SEED · SERIES A</p>
  <a class="btn-dark big" href="/login">Oturum aç →</a>
</section>"##)
}

pub fn login(msg: Option<&str>) -> String {
    let notice = msg.map(|m| format!(r#"<p class="notice">{}</p>"#, esc(m))).unwrap_or_default();
    layout("Oturum aç", None, "", &format!(r##"
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
</div>"##))
}

pub fn join(error: Option<&str>) -> String {
    let err = error.map(|e| format!(r#"<p class="error">{}</p>"#, esc(e))).unwrap_or_default();
    layout("Katıl", None, "", &format!(r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>Davet koduyla katıl</h1>
    {err}
    <form method="post" action="/join">
      <label>E-posta<input name="email" type="email" required autofocus></label>
      <label>Davet kodu<input name="code" required></label>
      <button class="btn-dark big">Katıl →</button>
    </form>
  </div>
</div>"##))
}

pub fn join_success() -> String {
    layout("Katıl", None, "", r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>Hesabın hazır</h1>
    <p class="auth-sub">Giriş yapmak için <a href="/login">oturum aç sayfasından</a> e-postana bir giriş bağlantısı iste.</p>
  </div>
</div>"##)
}

pub fn agentic_harness(user: &User) -> String {
    layout("Agentic Harness", Some(user), "agentic-harness", r##"
<h1>Agentic Harness — 1. Hafta</h1>
<p class="muted">Yakında burada.</p>"##)
}

pub fn ai_monopoly(user: &User) -> String {
    layout("AI Monopoly", Some(user), "ai-monopoly", r##"
<h1>AI Monopoly — 2. Hafta</h1>
<p class="muted">Yakında burada.</p>"##)
}

pub fn video_grid(user: &User, videos: &[VideoWithProgress], level: Option<&str>) -> String {
    let chips: String = std::iter::once((None::<&str>, "Hepsi"))
        .chain(LEVELS.iter().map(|(k, v)| (Some(*k), *v)))
        .map(|(k, label)| {
            let href = k.map(|k| format!("/app?level={k}")).unwrap_or_else(|| "/app".into());
            let active = if level == k { "active" } else { "" };
            format!(r#"<a class="chip {active}" href="{href}">{label}</a>"#)
        })
        .collect();
    let cards: String = if videos.is_empty() {
        "<p class='muted'>Henüz video yok</p>".into()
    } else {
        videos.iter().map(|v| {
            let pct = if v.duration > 0.0 { (v.max_position / v.duration * 100.0).min(100.0) } else { 0.0 };
            let done = pct >= 90.0;
            let meta = if done { "Tamamlanmış".into() }
                else if pct > 0.0 { format!("%{:.0} izlendi", pct) }
                else { "Henüz başlamadı".into() };
            format!(
                r##"<a class="vcard {done_class}" href="/watch/{id}">
  <div class="thumb"><img src="https://i.ytimg.com/vi/{yt}/hqdefault.jpg" alt="">
    <div class="progress"><i style="width:{pct:.0}%"></i></div>
  </div>
  <h3>{title}</h3>
  <p class="meta">{level} · {meta}</p>
</a>"##,
                done_class = if done { "done" } else { "" },
                id = v.id, yt = esc(&v.youtube_id), title = esc(&v.title), level = level_name(&v.level),
            )
        }).collect()
    };
    layout("Ana Sayfa", Some(user), level.unwrap_or("home"), &format!(
        r##"<div class="chips">{chips}</div><div class="grid">{cards}</div>"##))
}

pub fn watch(user: &User, video: &Video, playlist: &[VideoWithProgress], resume_at: f64) -> String {
    let list: String = playlist.iter().map(|v| {
        let pct = if v.duration > 0.0 { (v.max_position / v.duration * 100.0).min(100.0) } else { 0.0 };
        let cur = if v.id == video.id { "current" } else { "" };
        format!(
            r##"<a class="plitem {cur}" href="/watch/{id}">
  <div class="plthumb"><img src="https://i.ytimg.com/vi/{yt}/mqdefault.jpg" alt="">
    <div class="progress"><i style="width:{pct:.0}%"></i></div>
  </div>
  <span>{title}</span>
</a>"##,
            id = v.id, yt = esc(&v.youtube_id), title = esc(&v.title),
        )
    }).collect();
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
        title = esc(&video.title), level = level_name(&video.level), id = video.id, yt = esc(&video.youtube_id),
    );
    layout(&video.title, Some(user), &video.level, &content)
}

pub fn board(user: &User, tasks: &[Task], subs: &[SubmissionView]) -> String {
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
            let sub_html = match my_sub {
                Some(s) => {
                    let (label, class) = status_tr(&s.status);
                    let fb = s.feedback.as_deref().filter(|f| !f.is_empty())
                        .map(|f| format!(r#"<p class="feedback"><b>Geri bildirim:</b> {}</p>"#, esc(f)))
                        .unwrap_or_default();
                    let demo = s.demo_video_url.as_deref().filter(|d| !d.is_empty())
                        .map(|d| format!(r#"<p><a class="btn-outline" href="{}" target="_blank">Tanıtım videosu →</a></p>"#, esc(d)))
                        .unwrap_or_default();
                    format!(r#"<div class="substatus {class}">{label}</div>{fb}{demo}"#)
                }
                None => String::new(),
            };
            format!(
                r##"<div class="taskcard">
  <div class="taskhead"><h3>{title}</h3><span class="badge">{level}</span></div>
  <p class="desc">{desc}</p>
  {sub_html}
  <form method="post" action="/board/submit" class="subform">
    <input type="hidden" name="task_id" value="{id}">
    <input name="repo_url" type="url" placeholder="GitHub deposu bağlantısı" required>
    <button class="btn-dark">Gönder →</button>
  </form>
</div>"##,
                title = esc(&t.title), level = level_name(&t.level), desc = esc(&t.description), id = t.id,
            )
        }).collect()
    };
    layout("Görev Panosu", Some(user), "board", &format!(
        r##"<h1 class="pagetitle">Görev Panosu</h1><p class="muted">Projenizi gönderin — GitHub deposu bağlantısı yeterli.</p><div class="tasks">{task_cards}</div>"##))
}

pub fn admin(user: &User, stats: &[StatRow], subs: &[SubmissionView], videos: &[Video], tasks: &[Task], invite_code: &str) -> String {
    let level_opts: String = LEVELS.iter().map(|(k, v)| format!(r#"<option value="{k}">{v}</option>"#)).collect();
    let stat_rows: String = stats.iter().map(|s| {
        let pct = if s.duration > 0.0 { (s.max_position / s.duration * 100.0).min(100.0) } else { 0.0 };
        format!(
            "<tr><td>{}</td><td>{}</td><td>%{:.0}</td><td>{:.0} dk</td><td>{}</td></tr>",
            esc(&s.display_name), esc(&s.video_title), pct, s.seconds_watched / 60.0,
            s.updated_at.format("%d.%m.%Y %H:%M"),
        )
    }).collect();
    let sub_rows: String = subs.iter().map(|s| {
        format!(
            r##"<tr><td>{student}</td><td>{task}</td><td><a href="{url}" target="_blank">repo</a></td><td>{status}</td>
<td><form method="post" action="/admin/review" class="inline">
  <input type="hidden" name="id" value="{id}">
  <select name="status">
    <option value="pending">İnceleme bekleniyor</option>
    <option value="reviewing">İnceleniyor</option>
    <option value="passed">Geçti</option>
    <option value="failed">Başarısız</option>
  </select>
  <input name="feedback" placeholder="Geri bildirim" value="{fb}">
  <button class="btn-dark small">Kaydet</button>
</form></td></tr>"##,
            student = esc(&s.display_name), task = esc(&s.task_title), url = esc(&s.repo_url),
            status = esc(&s.status), id = s.id, fb = esc(s.feedback.as_deref().unwrap_or("")),
        )
    }).collect();
    let video_rows: String = videos.iter().map(|v|
        format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", esc(&v.title), level_name(&v.level), esc(&v.youtube_id))
    ).collect();
    let task_rows: String = tasks.iter().map(|t|
        format!("<tr><td>{}</td><td>{}</td></tr>", esc(&t.title), level_name(&t.level))
    ).collect();
    layout("Yönetici paneli", Some(user), "admin", &format!(
        r##"<h1 class="pagetitle">Yönetici paneli</h1>

<div class="admingrid">
<section class="panel">
  <h2>Video ekle</h2>
  <form method="post" action="/admin/video">
    <label>Başlık<input name="title" required></label>
    <label>YouTube ID / bağlantı<input name="youtube" placeholder="dQw4w9WgXcQ" required></label>
    <label>Seviye<select name="level">{level_opts}</select></label>
    <button class="btn-dark">Kaydet</button>
  </form>
  <table class="mini"><tr><th>Başlık</th><th>Seviye</th><th>YouTube</th></tr>{video_rows}</table>
</section>

<section class="panel">
  <h2>Görev ekle</h2>
  <form method="post" action="/admin/task">
    <label>Başlık<input name="title" required></label>
    <label>Tanım<textarea name="description" rows="3" required></textarea></label>
    <label>Seviye<select name="level">{level_opts}</select></label>
    <button class="btn-dark">Kaydet</button>
  </form>
  <table class="mini"><tr><th>Başlık</th><th>Seviye</th></tr>{task_rows}</table>
</section>

<section class="panel">
  <h2>Öğrenci ekle</h2>
  <form method="post" action="/admin/user">
    <label>E-posta<input name="email" type="email" required></label>
    <label>İsim<input name="display_name" required></label>
    <button class="btn-dark">Kaydet</button>
  </form>
</section>

<section class="panel">
  <h2>Davet kodu</h2>
  <p class="muted">Öğrenciler bu kodu <a href="/join">/join</a> sayfasında e-postalarıyla girerek kendi hesaplarını açar.</p>
  <input value="{invite_code}" readonly>
  <form method="post" action="/admin/invite">
    <button class="btn-dark">Kodu yenile</button>
  </form>
</section>
</div>

<section class="panel wide">
  <h2>İzleme istatistikleri</h2>
  <table><tr><th>Öğrenci</th><th>Video</th><th>İlerleme</th><th>Toplam süre</th><th>Son izleme</th></tr>{stat_rows}</table>
</section>

<section class="panel wide">
  <h2>Gönderimler</h2>
  <table><tr><th>Öğrenci</th><th>Görev</th><th>Repo</th><th>Durum</th><th></th></tr>{sub_rows}</table>
</section>"##))
}
