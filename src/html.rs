// Server-rendered pages. Turkish strings sourced from Google Translate API.
// ponytail: string templates, no template engine — 8 pages, full control.

use crate::model::*;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// (database value, what students see). The keys stay as they are — they're baked
/// into a CHECK constraint and into every existing video/task row — so renaming a
/// level is a change to the right-hand side only, never a migration.
pub const LEVELS: [(&str, &str); 3] =
    [("PRESEED", "Seviye 1"), ("SEED", "Seviye 2"), ("SERIES_A", "Seviye 3")];

pub fn level_name(l: &str) -> &'static str {
    LEVELS.iter().find(|(k, _)| *k == l).map(|(_, v)| *v).unwrap_or("?")
}

/// `<option>` list for a level `<select>`; `current` gets the `selected` attribute
/// (pass "" for a fresh form — no match, browser defaults to the first).
fn level_options(current: &str) -> String {
    LEVELS.iter().map(|(k, v)| format!(
        r#"<option value="{k}"{sel}>{v}</option>"#,
        sel = if *k == current { " selected" } else { "" },
    )).collect()
}

// Heroicons v2 (outline, 24x24, 1.5 stroke) — sized/colored via CSS (currentColor).
fn ico(path: &str) -> String {
    format!(r##"<svg class="ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="{path}"/></svg>"##)
}
const P_HOME: &str = "m2.25 12 8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25";
const P_BOARD: &str = "M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 0 0 2.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 0 0-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25Z";
#[allow(dead_code)] // Haftalar navı gizli — geri geldiğinde tekrar kullanılacak
const P_HARNESS: &str = "M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 0 0 2.25-2.25V6.75a2.25 2.25 0 0 0-2.25-2.25H6.75A2.25 2.25 0 0 0 4.5 6.75v10.5a2.25 2.25 0 0 0 2.25 2.25Zm.75-12h9v9h-9v-9Z";
#[allow(dead_code)] // Haftalar navı gizli — geri geldiğinde tekrar kullanılacak
const P_MONOPOLY: &str = "M14.25 6.087c0-.355.186-.676.401-.959.221-.29.349-.634.349-1.003 0-1.036-1.007-1.875-2.25-1.875s-2.25.84-2.25 1.875c0 .369.128.713.349 1.003.215.283.401.604.401.959v0a.64.64 0 0 1-.657.643 48.39 48.39 0 0 1-4.163-.3c.186 1.613.293 3.25.315 4.907a.656.656 0 0 1-.658.663v0c-.355 0-.676-.186-.959-.401a1.647 1.647 0 0 0-1.003-.349c-1.036 0-1.875 1.007-1.875 2.25s.84 2.25 1.875 2.25c.369 0 .713-.128 1.003-.349.283-.215.604-.401.959-.401v0c.31 0 .555.26.532.57a48.039 48.039 0 0 1-.642 5.056c1.518.19 3.058.309 4.616.354a.64.64 0 0 0 .657-.643v0c0-.355-.186-.676-.401-.959a1.647 1.647 0 0 1-.349-1.003c0-1.035 1.008-1.875 2.25-1.875 1.243 0 2.25.84 2.25 1.875 0 .369-.128.713-.349 1.003-.215.283-.4.604-.4.959v0c0 .333.277.599.61.58a48.1 48.1 0 0 0 5.427-.63 48.05 48.05 0 0 0 .582-4.717.532.532 0 0 0-.533-.57v0c-.355 0-.676.186-.959.401-.29.221-.634.349-1.003.349-1.035 0-1.875-1.007-1.875-2.25s.84-2.25 1.875-2.25c.37 0 .713.128 1.003.349.283.215.604.401.96.401v0a.656.656 0 0 0 .658-.663 48.422 48.422 0 0 0-.37-5.36c-1.676.24-3.37.404-5.082.484a.638.638 0 0 1-.667-.643v0Z";
const P_ADMIN: &str = "M11.42 15.17 17.25 21A2.652 2.652 0 0 0 21 17.25l-5.877-5.877M11.42 15.17l2.496-3.03c.317-.384.74-.626 1.208-.766M11.42 15.17l-4.655 5.653a2.548 2.548 0 1 1-3.586-3.586l6.837-5.63m5.108-.233c.55-.164 1.163-.188 1.743-.14a4.5 4.5 0 0 0 4.486-6.336l-3.276 3.277a3.004 3.004 0 0 1-2.25-2.25l3.276-3.276a4.5 4.5 0 0 0-6.336 4.486c.091 1.076-.071 2.264-.904 2.95l-.102.085m-1.745 1.437L5.909 7.5H4.5L2.25 3.75l1.5-1.5L7.5 4.5v1.409l4.26 4.26m-1.745 1.437 1.745-1.437m6.615 8.206L15.75 15.75M4.867 19.125h.008v.008h-.008v-.008Z";
const P_LOGOUT: &str = "M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15M12 9l-3 3m0 0 3 3m-3-3h12.75";
const P_DEMO: &str = "m3.75 13.5 10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75Z";
const P_TROPHY: &str = "M16.5 18.75h-9m9 0a3 3 0 0 1 3 3h-15a3 3 0 0 1 3-3m9 0v-3.375c0-.621-.503-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.007 0H9.497m5.007 0a7.454 7.454 0 0 1-.982-3.172M9.497 14.25a7.454 7.454 0 0 0 .981-3.172M5.25 4.236c-.982.143-1.954.317-2.916.52A6.003 6.003 0 0 0 7.73 9.728M5.25 4.236V4.5c0 2.108.966 3.99 2.48 5.228M5.25 4.236V2.721C7.456 2.41 9.71 2.25 12 2.25c2.291 0 4.545.16 6.75.47v1.516M7.73 9.728a6.726 6.726 0 0 0 2.748 1.35m8.272-6.842V4.5c0 2.108-.966 3.99-2.48 5.228m2.48-5.492a46.32 46.32 0 0 1 2.916.52 6.003 6.003 0 0 1-5.395 4.972m0 0a6.726 6.726 0 0 1-2.749 1.35m0 0a6.772 6.772 0 0 1-3.044 0";
const P_PLAY: &str = "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z M15.91 11.672a.375.375 0 0 1 0 .656l-5.603 3.113a.375.375 0 0 1-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112Z";
const P_MENU: &str = "M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5";
const P_UPLOAD: &str = "M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5m-13.5-9L12 3m0 0 4.5 4.5M12 3v13.5";
const P_CLOSE: &str = "M6 18 18 6M6 6l12 12";

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
    {videos}
    {board}
    {leaderboard}
    {demos}
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
                board = nav_link("/board", active, "board", &ico(P_BOARD), "Görev Panosu"),
                leaderboard = nav_link("/leaderboard", active, "leaderboard", &ico(P_TROPHY), "Puan Tablosu"),
                // Haftalar (Agentic Harness / AI Monopoly) geçici olarak gizli — rotalar duruyor,
                // geri getirmek için bu iki satırı {harness}/{monopoly} olarak nav'a ekle.
                videos = nav_link("/videos", active, "videos", &ico(P_PLAY), "Videolar"),
                demos = nav_link("/demos", active, "demos", &ico(P_DEMO), "İnteraktif Demolar"),
                admin_block = admin_block,
                profile_active = if active == "profile" { "active" } else { "" },
                initial = esc(&u.label().chars().next().unwrap_or('?').to_uppercase().to_string()),
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
<link rel="stylesheet" href="/static/style.css?v=14">
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
  <p class="sub">Seviye 1 · Seviye 2 · Seviye 3</p>
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

/// Onboarding. `code_locked` = the invite code arrived in the URL (/join/<code>), so it
/// rides along as a hidden field instead of being something the student has to type.
pub fn join(f: &JoinForm, code_locked: bool, error: Option<&str>) -> String {
    let err = error.map(|e| format!(r#"<p class="error">{}</p>"#, esc(e))).unwrap_or_default();
    let code_field = if code_locked {
        format!(r#"<input type="hidden" name="code" value="{}">"#, esc(&f.code))
    } else {
        format!(r#"<label>Davet kodu<input name="code" value="{}" required></label>"#, esc(&f.code))
    };
    let grade_opts: String = std::iter::once(String::from(r#"<option value="">Seç…</option>"#))
        .chain(GRADES.iter().map(|g| {
            let sel = if f.grade == *g { " selected" } else { "" };
            format!(r#"<option value="{g}"{sel}>{g}</option>"#)
        }))
        .collect();
    layout("Oluştur", None, "", &format!(r##"
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
        <span class="fieldnote"><b>Puan tablosunda yalnızca bu nickname görünür</b> — gerçek adın hiçbir zaman
        diğer öğrencilere gösterilmez. Harf, rakam, _ ve - kullanabilirsin.</span>
      </label>
      <label>Okul<input name="school" value="{school}" required></label>
      <label>Sınıf<select name="grade" required>{grade_opts}</select></label>
      <button class="btn-dark big">Oluştur →</button>
    </form>
  </div>
</div>"##,
        name = esc(&f.display_name), email = esc(&f.email), nick = esc(&f.nickname), school = esc(&f.school),
    ))
}

/// Post-onboarding: the account exists but nothing is signed in yet — the magic link
/// in their inbox is what proves the address is theirs.
pub fn join_sent(email: &str) -> String {
    layout("E-postanı kontrol et", None, "", &format!(r##"
<div class="auth-wrap">
  <div class="auth-dots"></div><div class="auth-glow"></div>
  <div class="loginbox">
    <h1>E-postanı kontrol et</h1>
    <p class="auth-sub"><b>{email}</b> adresine bir giriş bağlantısı gönderdik.
    Bağlantıya tıkladığında hesabın açılacak.</p>
    <p class="notice">Bağlantı 15 dakika geçerli. Gelen kutunda yoksa spam klasörüne bak.</p>
    <p class="muted">Yanlış adres mi yazdın? <a href="/join">Formu tekrar doldur</a></p>
  </div>
</div>"##, email = esc(email)))
}

pub fn profile(user: &User, p: &Profile, msg: Option<&str>, error: Option<&str>) -> String {
    let first_time = user.nickname.is_none();
    let banner = error.map(|e| format!(r#"<p class="error">{}</p>"#, esc(e)))
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
    let content = format!(r##"<h1 class="pagetitle">Profilim</h1>
{intro}
<div class="profilewrap">
<section class="panel">
  <h2>Bilgilerim</h2>
  {banner}
  <form method="post" action="/profile">
    <label>Ad soyad<input name="display_name" value="{name}" required></label>
    <label><span lang="en">Nickname</span><input name="nickname" value="{nick}" placeholder="ör. onur_maker" maxlength="20" required></label>
    <p class="fieldnote">Puan tablosunda yalnızca nickname'in görünür; gerçek adını diğer öğrenciler görmez.</p>
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
        save_label = if first_time { "Kaydet ve başla →" } else { "Kaydet" },
    );
    layout("Profilim", Some(user), "profile", &content)
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

// ponytail: hardcoded list — demos are files in static/demos/, add a row here when adding a file
const DEMOS: [(&str, &str, &str); 7] = [
    ("ai-timeline/index.html", "Makineler Nasıl Öğrenmeyi Öğrendi", "Yapay zekânın zaman çizelgesi — 4 bölümlük interaktif seri"),
    ("html-css-js-demo.html", "HTML + CSS + JS", "Koddan çıktıya: web sayfası nasıl oluşur"),
    ("backend-frontend-demo.html", "Ön Uç ve Arka Uç", "İstemci ile sunucu arasındaki iş bölümü"),
    ("database-demo.html", "Veritabanı Nedir?", "Veritabanı nedir, veriler nasıl saklanır"),
    ("authentication-demo.html", "Kimlik Doğrulama", "Kimlik doğrulama nasıl çalışır"),
    ("ui-ux-demo.html", "UI ve UX", "Arayüz ile deneyim arasındaki fark"),
    ("package-manager-demo.html", "Paket Yöneticisi Nedir?", "Paket yöneticileri ne işe yarar"),
];

pub fn demos(user: &User, lang: &str) -> String {
    // lang is validated to "tr" | "en" by the handler; files live in static/demos/{lang}/
    let cards: String = DEMOS.iter().map(|(file, title, desc)| format!(
        r##"<a class="panel demo-card" href="/static/demos/{lang}/{file}" target="_blank" rel="noopener">
  <h3>{title}</h3>
  <p class="meta">{desc}</p>
</a>"##)).collect();
    let chips: String = [("tr", "Türkçe"), ("en", "English")].iter().map(|(k, label)| {
        let active = if lang == *k { "active" } else { "" };
        format!(r#"<a class="chip {active}" href="/demos?lang={k}">{label}</a>"#)
    }).collect();
    let content = format!(
        r##"<h1 class="pagetitle">İnteraktif Demolar</h1>
<p class="muted">Derslerde kullanılan interaktif anlatımlar.</p>
<div class="chips">{chips}</div>
<div class="admingrid">{cards}</div>"##);
    layout("İnteraktif Demolar", Some(user), "demos", &content)
}

/// Ana Sayfa — portalın giriş kapısı. İçerik yok, yalnızca üç büyük hedef:
/// solda videolar, sağda görevler, altta puan tablosu.
pub fn home(user: &User, videos_done: i64, videos_total: i64, open_tasks: i64, points: i64, rank: Option<i64>) -> String {
    let rank_line = match rank {
        Some(r) => format!("{r}. sıradasın"),
        None => "Henüz sıralamada değilsin".into(),
    };
    let content = format!(
        r##"<h1 class="pagetitle">Merhaba {name} 👋</h1>
<p class="muted">Nereden devam etmek istersin?</p>
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
    <h2>Görevler</h2>
    <p>Projeni yap, GitHub bağlantısını gönder, geri bildirim al.</p>
    <span class="hubstat">{open_tasks} açık görev</span>
    <span class="hubgo">Görev panosuna git →</span>
  </a>
  <a class="hubcard" href="/leaderboard">
    <span class="hubico">{ico_trophy}</span>
    <h2>Puan Tablosu</h2>
    <p>Her tamamlanan video {PTS_VIDEO} puan, kabul edilen her proje {PTS_PROJECT} puan.</p>
    <span class="hubstat">{points} puan · {rank_line}</span>
    <span class="hubgo">Sıralamayı gör →</span>
  </a>
  <a class="hubcard" href="/demos">
    <span class="hubico">{ico_demo}</span>
    <h2>İnteraktif Demolar</h2>
    <p>Derslerde kullanılan interaktif anlatımlar.</p>
    <span class="hubstat">{demo_count} demo</span>
    <span class="hubgo">Demolara git →</span>
  </a>
</div>"##,
        name = esc(u_first_name(user)),
        ico_video = ico(P_PLAY),
        ico_board = ico(P_BOARD),
        ico_trophy = ico(P_TROPHY),
        ico_demo = ico(P_DEMO),
        demo_count = DEMOS.len(),
    );
    layout("Ana Sayfa", Some(user), "home", &content)
}

/// Kartta tam ad yerine yalnızca ilk isim — selamlama kısa kalsın.
fn u_first_name(user: &User) -> &str {
    user.label().split_whitespace().next().unwrap_or("")
}

pub fn video_grid(user: &User, videos: &[VideoWithProgress], level: Option<&str>) -> String {
    let chips: String = std::iter::once((None::<&str>, "Hepsi"))
        .chain(LEVELS.iter().map(|(k, v)| (Some(*k), *v)))
        .map(|(k, label)| {
            let href = k.map(|k| format!("/videos?level={k}")).unwrap_or_else(|| "/videos".into());
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
    // seviye filtresi açıkken de nav'da Videolar seçili kalsın
    layout("Videolar", Some(user), "videos", &format!(
        r##"<h1 class="pagetitle">Videolar</h1>
<p class="muted">Ders videoları. Bir videoyu %90'ına kadar izlediğinde tamamlanmış sayılır.</p>
<div class="chips">{chips}</div><div class="grid">{cards}</div>"##))
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

/// Dense ranking over an already-sorted standings list: equal points share a place.
pub fn dense_ranks(rows: &[LeaderRow]) -> Vec<i64> {
    let mut ranks: Vec<i64> = Vec::with_capacity(rows.len());
    let mut place = 0i64;
    let mut prev: Option<i64> = None;
    for r in rows {
        if prev != Some(r.points()) { place += 1; prev = Some(r.points()); }
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
            let name = r.nickname.clone();
            format!(
                r##"<section class="panel mecard">
  <div class="me-rank">#{rank}</div>
  <span class="avatar-fb big">{initial}</span>
  <div class="me-id"><h3>{name}</h3><p class="meta">Senin sıran</p></div>
  <div class="me-stats">
    <div><b>{videos}</b><span>video · {vpts}p</span></div>
    <div><b>{projects}</b><span>proje · {ppts}p</span></div>
    <div class="me-total"><b>{total}</b><span>toplam puan</span></div>
  </div>
</section>"##,
                rank = ranks[i],
                initial = esc(&name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                name = esc(&name),
                videos = r.videos, vpts = r.videos * PTS_VIDEO,
                projects = r.projects, ppts = r.projects * PTS_PROJECT,
                total = r.points(),
            )
        }
        None => String::new(),
    };

    let list: String = if rows.is_empty() {
        "<p class='muted'>Henüz kimse puan toplamadı — ilk sen ol.</p>".into()
    } else {
        rows.iter().zip(&ranks).map(|(r, rank)| {
            let name = r.nickname.clone();
            format!(
                r##"<div class="lbrow {mine} {medal}">
  <span class="lbrank">{rank}</span>
  <span class="avatar-fb">{initial}</span>
  <span class="lbname">{name}</span>
  <span class="lbmeta">{videos} video · {projects} proje</span>
  <span class="lbpts">{total}<small>p</small></span>
</div>"##,
                mine = if r.id == user.id { "mine" } else { "" },
                medal = match rank { 1 => "m1", 2 => "m2", 3 => "m3", _ => "" },
                initial = esc(&name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                name = esc(&name),
                videos = r.videos, projects = r.projects, total = r.points(),
            )
        }).collect()
    };

    layout("Puan Tablosu", Some(user), "leaderboard", &format!(
        r##"<h1 class="pagetitle">Puan Tablosu</h1>
<p class="muted">Tamamlanan her video <b>{PTS_VIDEO} puan</b>, kabul edilen her proje <b>{PTS_PROJECT} puan</b>.
</p>
{my_card}
<div class="lb">{list}</div>
<p class="lbnote">Bir video, %90'ını izlediğinde tamamlanmış sayılır. Proje puanı, gönderimin durumu
<b>Geçti</b> olduğunda eklenir — aynı görev birden fazla kez puan getirmez.</p>"##))
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
            // live preview of the example project; interaction goes through the link
            // below it (the iframe itself is pointer-events:none in CSS)
            let example = t.example_url.as_deref().filter(|u| !u.is_empty()).map(|u| format!(
                r##"<a class="example-preview" href="{url}" target="_blank" rel="noopener" title="Örnek projeyi yeni sekmede aç"><iframe src="{url}" loading="lazy" sandbox="allow-scripts allow-same-origin" tabindex="-1" title="Örnek proje önizlemesi"></iframe></a>"##,
                url = esc(u),
            )).unwrap_or_default();
            let sub_html = match my_sub {
                Some(s) => {
                    let (label, class) = status_tr(&s.status);
                    let fb = s.feedback.as_deref().filter(|f| !f.is_empty())
                        .map(|f| format!(r#"<p class="feedback"><b>Geri bildirim:</b> {}</p>"#, esc(f)))
                        .unwrap_or_default();
                    let demo = s.demo_video_url.as_deref().filter(|d| !d.is_empty())
                        .map(|d| format!(r#"<p><a class="btn-outline" href="{}" target="_blank">Tanıtım videosu →</a></p>"#, esc(d)))
                        .unwrap_or_default();
                    let plan_ok = if s.plan_md.as_deref().is_some_and(|p| !p.trim().is_empty()) {
                        r#"<p class="fieldnote">plan.md yüklendi ✓</p>"#
                    } else { "" };
                    format!(r#"<div class="substatus {class}">{label}</div>{fb}{demo}{plan_ok}"#)
                }
                None => String::new(),
            };
            format!(
                r##"<div class="taskcard">
  <div class="taskhead"><h3>{title}</h3><span class="badge">{level}</span></div>
  <p class="desc">{desc}</p>
  {example}
  {sub_html}
  <form method="post" action="/board/submit" enctype="multipart/form-data" class="subform">
    <input type="hidden" name="task_id" value="{id}">
    <input name="repo_url" type="url" placeholder="https://github.com/..." required>
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
    <button class="btn-dark">Gönder →</button>
  </form>
</div>"##,
                title = esc(&t.title), level = level_name(&t.level), desc = esc(&t.description), id = t.id,
                up = ico(P_UPLOAD),
            )
        }).collect()
    };
    layout("Görev Panosu", Some(user), "board", &format!(
        r##"<h1 class="pagetitle">Görev Panosu</h1><p class="muted">Projenizi yükleyin.</p><div class="tasks">{task_cards}</div>"##))
}

pub fn admin(user: &User, stats: &[StatRow], subs: &[SubmissionView], videos: &[Video], tasks: &[Task], invite_code: &str, base_url: &str) -> String {
    let invite_link = format!("{}/join/{}", base_url.trim_end_matches('/'), invite_code);
    let level_opts = level_options("");
    let stat_rows: String = stats.iter().map(|s| {
        let pct = if s.duration > 0.0 { (s.max_position / s.duration * 100.0).min(100.0) } else { 0.0 };
        format!(
            "<tr><td>{}</td><td>{}</td><td>%{:.0}</td><td>{:.0} dk</td><td>{}</td></tr>",
            esc(&s.display_name), esc(&s.video_title), pct, s.seconds_watched / 60.0,
            s.updated_at.format("%d.%m.%Y %H:%M"),
        )
    }).collect();
    let sub_rows: String = subs.iter().map(|s| {
        // preselect the submission's actual status so saving without touching the
        // dropdown doesn't silently reset it to "pending"
        let status_opts: String = [
            ("pending", "İnceleme bekleniyor"), ("reviewing", "İnceleniyor"),
            ("passed", "Geçti"), ("failed", "Başarısız"),
        ].iter().map(|(k, v)| format!(
            r#"<option value="{k}"{sel}>{v}</option>"#,
            sel = if *k == s.status { " selected" } else { "" },
        )).collect();
        let plan = s.plan_md.as_deref().filter(|p| !p.trim().is_empty())
            .map(|p| format!(r#"<details class="plan-details"><summary>Plan</summary><pre class="plan-pre">{}</pre></details>"#, esc(p)))
            .unwrap_or_else(|| "—".into());
        format!(
            r##"<tr><td>{student}</td><td>{email}</td><td>{task}</td><td><a href="{url}" target="_blank">repo</a></td><td>{plan}</td><td>{date}</td>
<td><form method="post" action="/admin/review" class="inline">
  <input type="hidden" name="id" value="{id}">
  <select name="status">{status_opts}</select>
  <input name="feedback" placeholder="Geri bildirim" value="{fb}">
  <button class="btn-dark small">Kaydet</button>
</form></td></tr>"##,
            student = esc(&s.display_name), email = esc(&s.email), task = esc(&s.task_title),
            url = esc(&s.repo_url), date = s.created_at.format("%d.%m.%Y %H:%M"),
            id = s.id, fb = esc(s.feedback.as_deref().unwrap_or("")),
        )
    }).collect();
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
  </div>
</div>"##,
        title = esc(&t.title), id = t.id, opts = level_options(&t.level),
        example = esc(t.example_url.as_deref().unwrap_or("")),
    )).collect();
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
    <button class="btn-dark">Kaydet</button>
  </form>
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
</div>

<section class="panel wide">
  <h2>İzleme istatistikleri</h2>
  <table><tr><th>Öğrenci</th><th>Video</th><th>İlerleme</th><th>Toplam süre</th><th>Son izleme</th></tr>{stat_rows}</table>
</section>

<section class="panel wide">
  <h2>Gönderimler</h2>
  <table><tr><th>Öğrenci</th><th>E-posta</th><th>Görev</th><th>Repo</th><th>Plan</th><th>Gönderim</th><th></th></tr>{sub_rows}</table>
</section>"##))
}
