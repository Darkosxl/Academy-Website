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

fn layout(title: &str, user: Option<&User>, active: &str, content: &str) -> String {
    let sidebar = match user {
        None => String::new(),
        Some(u) => {
            let admin_link = if u.is_admin {
                r##"<a href="/admin" class="ADMIN_ACTIVE">🛠 Yönetici paneli</a>"##.replace("ADMIN_ACTIVE", if active == "admin" { "active" } else { "" })
            } else {
                String::new()
            };
            format!(
                r##"<aside class="sidebar">
  <a href="/app" class="{home}">🏠 Ana Sayfa</a>
  <a href="/board" class="{board}">🗂 Görev Panosu</a>
  <hr>
  <p class="head">Seviyeler</p>
  <a href="/app?level=PRESEED" class="{pre}">PRESEED</a>
  <a href="/app?level=SEED" class="{seed}">SEED</a>
  <a href="/app?level=SERIES_A" class="{sa}">SERIES A</a>
  <hr>
  {admin_link}
  <form method="post" action="/logout"><button class="linklike">🚪 Oturumu kapat</button></form>
</aside>"##,
                home = if active == "home" { "active" } else { "" },
                board = if active == "board" { "active" } else { "" },
                pre = if active == "PRESEED" { "active" } else { "" },
                seed = if active == "SEED" { "active" } else { "" },
                sa = if active == "SERIES_A" { "active" } else { "" },
            )
        }
    };
    let avatar = user
        .map(|u| format!(r#"<div class="avatar" title="{}">{}</div>"#, esc(&u.display_name), esc(&u.display_name.chars().next().unwrap_or('?').to_string())))
        .unwrap_or_else(|| r##"<a class="btn-dark" href="/login">Oturum aç</a>"##.into());
    format!(
        r##"<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Exposure Academy</title>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;700;800;900&display=swap" rel="stylesheet">
<link rel="stylesheet" href="/static/style.css">
</head>
<body>
<header class="topbar">
  <a class="logo" href="/">exposure<span>AI ACADEMY</span></a>
  {avatar}
</header>
<div class="layout">
{sidebar}
<main class="content">
{content}
</main>
</div>
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

pub fn login(error: bool) -> String {
    let err = if error { r#"<p class="error">Yanlış kullanıcı adı veya şifre</p>"# } else { "" };
    layout("Oturum aç", None, "", &format!(r##"
<div class="loginbox">
  <h1>Oturum aç</h1>
  {err}
  <form method="post" action="/login">
    <label>Kullanıcı adı<input name="username" required autofocus></label>
    <label>Şifre<input name="password" type="password" required></label>
    <button class="btn-dark big">Oturum aç →</button>
  </form>
</div>"##))
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

pub fn admin(user: &User, stats: &[StatRow], subs: &[SubmissionView], videos: &[Video], tasks: &[Task]) -> String {
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
    <label>Kullanıcı adı<input name="username" required></label>
    <label>İsim<input name="display_name" required></label>
    <label>Şifre<input name="password" required></label>
    <button class="btn-dark">Kaydet</button>
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
