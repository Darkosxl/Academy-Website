//! Sessions, magic-link login, onboarding, profile — plus the shared request
//! guards every handler starts with, and the worker-API shared-secret check.

use crate::html;
use crate::model::*;
use crate::{App, random_token};
use axum::{
    Form,
    extract::Request,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

pub async fn send_magic_link_email(app: &App, to: &str, link: &str) {
    // Ensure a display name so clients don't show the bare address as sender.
    let from = if app.mail_from.contains('<') {
        app.mail_from.clone()
    } else {
        format!("Exposure Academy <{}>", app.mail_from)
    };
    // Email-client-safe HTML: table layout, inline styles, no external assets.
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="tr">
<body style="margin:0;padding:0;background-color:#FFFCF6;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#FFFCF6;padding:40px 16px;">
<tr><td align="center">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:440px;">
    <tr><td style="padding:0 4px 20px 4px;">
      <span style="font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:22px;font-weight:800;letter-spacing:-0.5px;color:#0D0D0D;">exposure</span>
      <span style="font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:9px;font-weight:700;letter-spacing:3px;color:#a1a1aa;text-transform:uppercase;">&nbsp;AI ACADEMY</span>
    </td></tr>
    <tr><td style="background-color:#ffffff;border:1px solid #e8e4da;border-radius:16px;padding:36px 32px;">
      <p style="margin:0 0 6px 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:22px;font-weight:800;letter-spacing:-0.5px;color:#0D0D0D;">Oturum aç</p>
      <p style="margin:0 0 26px 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:14px;line-height:1.6;color:#71717a;">Exposure Academy hesabına giriş yapmak için aşağıdaki butona tıkla.</p>
      <table role="presentation" cellpadding="0" cellspacing="0" width="100%"><tr><td align="center">
        <a href="{link}" style="display:block;background-color:#0339A6;color:#ffffff;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:14px;font-weight:700;letter-spacing:1px;text-transform:uppercase;text-decoration:none;padding:14px 24px;border-radius:12px;text-align:center;">Oturum a&ccedil; &rarr;</a>
      </td></tr></table>
      <p style="margin:26px 0 0 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:12px;line-height:1.6;color:#a1a1aa;">Bu bağlantı <strong style="color:#71717a;">15 dakika</strong> geçerlidir ve yalnızca bir kez kullanılabilir.</p>
      <p style="margin:8px 0 0 0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:12px;line-height:1.6;color:#a1a1aa;">Buton çalışmıyorsa bu bağlantıyı tarayıcına yapıştır:<br><a href="{link}" style="color:#0339A6;word-break:break-all;">{link}</a></p>
    </td></tr>
    <tr><td style="padding:20px 4px 0 4px;">
      <p style="margin:0;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;font-size:11px;line-height:1.6;color:#a1a1aa;">Bu e-postayı sen istemediysen görmezden gelebilirsin — hesabında hiçbir işlem yapılmaz.<br>&copy; Exposure Academy</p>
    </td></tr>
  </table>
</td></tr>
</table>
</body>
</html>"##
    );
    let body = serde_json::json!({
        "from": from,
        "to": [to],
        "subject": "Exposure Academy giriş bağlantın",
        "html": html,
    });
    if let Err(e) = app
        .http
        .post("https://api.resend.com/emails")
        .bearer_auth(&app.resend_key)
        .json(&body)
        .send()
        .await
    {
        eprintln!("resend send failed: {e}");
    }
}

/// Session lifetime. Kept in one place so the DB row's `expires_at`, the cookie's
/// Max-Age and the rolling refresh below can never drift apart.
pub const SESSION_DAYS: i64 = 30;

pub const SESSION_MAX_AGE: i64 = SESSION_DAYS * 24 * 60 * 60;

/// Refresh once the session drops below this — one extra write per user per day,
/// not one per request.
pub const SESSION_REFRESH_BELOW_DAYS: i64 = SESSION_DAYS - 1;

pub fn session_cookie(token: &str) -> String {
    format!("session={token}; HttpOnly; Secure; Path=/; Max-Age={SESSION_MAX_AGE}; SameSite=Lax")
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|c| c.strip_prefix("session=").map(String::from))
}

pub async fn current_user(app: &App, headers: &HeaderMap) -> Option<User> {
    let token = cookie_token(headers)?;
    sqlx::query_as::<_, User>(
        "select u.id, u.display_name, u.nickname, u.is_admin from sessions_exposure_academy s join users_exposure_academy u on u.id = s.user_id where s.token = $1 and s.expires_at > now()")
        .bind(token).fetch_optional(&app.pool).await.ok()?
}

/// insert a 30-day session row and build the matching Set-Cookie + redirect to /app
pub async fn issue_session(app: &App, uid: Uuid) -> Response {
    let session_token = random_token();
    sqlx::query("insert into sessions_exposure_academy (token, user_id, expires_at) values ($1,$2, now() + make_interval(days => $3))")
        .bind(&session_token).bind(uid).bind(SESSION_DAYS as i32).execute(&app.pool).await.unwrap();
    (
        // cookie Max-Age mirrors the row's expires_at; the DB check is the one that counts
        [(header::SET_COOKIE, session_cookie(&session_token))],
        Redirect::to("/app"),
    )
        .into_response()
}

/// Rolling window: every request carrying a live session pushes its expiry back out
/// to the full 30 days, so an active user is never logged out mid-use — only 30 days
/// of *inactivity* ends the session.
///
/// Two things that matter here, both learned the hard way in the Next.js version:
/// the DB row and the browser cookie must be extended *together* (extending only the
/// row leaves the cookie to expire out from under a still-valid session), and the
/// refresh must run after the handler so /logout's delete wins — a deleted row
/// matches nothing below, so no Set-Cookie is appended and the logout sticks.
pub async fn rolling_session(State(app): State<App>, req: Request, next: Next) -> Response {
    let token = cookie_token(req.headers());
    let mut res = next.run(req).await;
    let Some(token) = token else { return res };

    let rolled: Option<(Uuid,)> = sqlx::query_as(
        "update sessions_exposure_academy set expires_at = now() + make_interval(days => $2)
         where token = $1 and expires_at > now() and expires_at < now() + make_interval(days => $3)
         returning user_id",
    )
    .bind(&token)
    .bind(SESSION_DAYS as i32)
    .bind(SESSION_REFRESH_BELOW_DAYS as i32)
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten();

    if rolled.is_some() {
        if let Ok(v) = HeaderValue::from_str(&session_cookie(&token)) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

pub fn require(user: Option<User>) -> Result<User, Response> {
    user.ok_or_else(|| Redirect::to("/login").into_response())
}

/// Same as `require`, plus: no nickname means onboarding never finished, so send them
/// to /profile to pick one. Used by every student page except /profile itself, which
/// would otherwise redirect to itself forever.
pub fn require_onboarded(user: Option<User>) -> Result<User, Response> {
    let u = require(user)?;
    // admins never appear on the leaderboard, so a nickname is optional for them —
    // gating them too would just lock you out of the portal after a fresh seed
    if u.nickname.is_none() && !u.is_admin {
        return Err(Redirect::to("/profile").into_response());
    }
    Ok(u)
}

pub fn require_admin(user: Option<User>) -> Result<User, Response> {
    match user {
        Some(u) if u.is_admin => Ok(u),
        Some(_) => Err(StatusCode::FORBIDDEN.into_response()),
        None => Err(Redirect::to("/login").into_response()),
    }
}

pub async fn landing(State(app): State<App>, headers: HeaderMap) -> Response {
    // valid session cookie -> straight to the portal, skip the marketing page
    if current_user(&app, &headers).await.is_some() {
        return Redirect::to("/app").into_response();
    }
    Html(html::landing()).into_response()
}

pub async fn login_page(State(app): State<App>, headers: HeaderMap) -> Response {
    if current_user(&app, &headers).await.is_some() {
        return Redirect::to("/app").into_response();
    }
    Html(html::login(None)).into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
}

pub const CHECK_EMAIL_MSG: &str = "Eğer bu e-posta kayıtlıysa, giriş bağlantısı gönderildi.";

pub async fn login_post(State(app): State<App>, Form(f): Form<LoginForm>) -> Response {
    let email = f.email.trim().to_lowercase();
    let allowed: Option<(Uuid,)> =
        sqlx::query_as("select id from users_exposure_academy where email = $1")
            .bind(&email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    if allowed.is_some() {
        send_login_link(&app, &email).await;
    }
    // same response whether or not the email is registered — avoids account enumeration
    Html(html::login(Some(CHECK_EMAIL_MSG))).into_response()
}

pub async fn magic_consume(State(app): State<App>, Path(token): Path<String>) -> Response {
    let row: Option<(String,)> = sqlx::query_as(
        "update magic_links_exposure_academy set used_at = now()
         where token = $1 and used_at is null and expires_at > now()
         returning email",
    )
    .bind(&token)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((email,)) = row else {
        return Html(html::login(Some(
            "Bağlantı geçersiz ya da süresi dolmuş, yeniden deneyin.",
        )))
        .into_response();
    };
    let user_id: Option<(Uuid,)> =
        sqlx::query_as("select id from users_exposure_academy where email = $1")
            .bind(&email)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    let Some((uid,)) = user_id else {
        return Html(html::login(Some("Hesap bulunamadı."))).into_response();
    };
    issue_session(&app, uid).await
}

pub async fn join_page() -> Html<String> {
    Html(html::join(&JoinForm::default(), false, None))
}

/// The link that goes in the WhatsApp group: /join/<invite code>. The code rides in
/// the path so students only fill in their own details; it is still validated on POST.
pub async fn join_page_code(Path(code): Path<String>) -> Html<String> {
    let f = JoinForm {
        code,
        ..Default::default()
    };
    Html(html::join(&f, true, None))
}

pub async fn invite_code(app: &App) -> String {
    sqlx::query_scalar("select value from app_settings_exposure_academy where key = 'invite_code'")
        .fetch_optional(&app.pool)
        .await
        .unwrap()
        .unwrap_or_default()
}

pub async fn join_post(State(app): State<App>, Form(f): Form<JoinForm>) -> Response {
    let locked = !f.code.trim().is_empty();
    let fail = |msg: &str| Html(html::join(&f, locked, Some(msg))).into_response();

    if f.code.trim() != invite_code(&app).await {
        return fail("Davet kodu geçersiz.");
    }
    let email = f.email.trim().to_lowercase();
    if !email.contains('@') {
        return fail("Geçerli bir e-posta gir.");
    }
    let name = f.display_name.trim();
    if name.chars().count() < 2 {
        return fail("Ad soyadını yaz.");
    }
    let nickname = match validate_nickname(&f.nickname) {
        Ok(n) => n,
        Err(e) => return fail(e),
    };
    let taken: Option<(Uuid,)> =
        sqlx::query_as("select id from users_exposure_academy where lower(nickname) = lower($1)")
            .bind(&nickname)
            .fetch_optional(&app.pool)
            .await
            .unwrap();
    if taken.is_some() {
        return fail("Bu nickname alınmış, başka bir tane seç.");
    }
    let school = f.school.trim();
    if school.chars().count() < 2 {
        return fail("Okulunu yaz.");
    }
    // the browser enforces `required`, but the grade must also be one we offer — a
    // hand-rolled POST could otherwise put anything in the column
    if !GRADES.contains(&f.grade.trim()) {
        return fail("Sınıfını seç.");
    }
    // GitHub/LinkedIn are optional — the student may skip them here and add them in-app
    // later. Only validate when actually provided.
    let github = match normalize_profile_url(&f.github_url, "github.com") {
        Ok(v) => v,
        Err(()) => {
            return fail(
                "GitHub bağlantısı github.com adresinde olmalı (ör. https://github.com/kullanici).",
            );
        }
    };
    let linkedin = match normalize_profile_url(&f.linkedin_url, "linkedin.com") {
        Ok(v) => v,
        Err(()) => {
            return fail(
                "LinkedIn bağlantısı linkedin.com adresinde olmalı (ör. https://linkedin.com/in/adin).",
            );
        }
    };

    // `do nothing` on an existing email: a returning student (or one the admin added
    // by hand) just gets a login link, and their existing profile is left alone rather
    // than being overwritten by whoever typed their address.
    sqlx::query(
        "insert into users_exposure_academy (email, display_name, nickname, school, grade, github_url, linkedin_url)
         values ($1,$2,$3,$4,$5,$6,$7)
         on conflict (email) do nothing")
        .bind(&email).bind(name).bind(&nickname).bind(school).bind(f.grade.trim())
        .bind(&github).bind(&linkedin)
        .execute(&app.pool).await.unwrap();

    send_login_link(&app, &email).await;
    Html(html::join_sent(&email)).into_response()
}

/// Mint a magic link for an email that is known to have an account, unless one was
/// already sent in the last minute.
pub async fn send_login_link(app: &App, email: &str) {
    let recent: Option<(i32,)> = sqlx::query_as(
        "select 1 from magic_links_exposure_academy where email = $1 and used_at is null and created_at > now() - interval '60 seconds'")
        .bind(email).fetch_optional(&app.pool).await.unwrap();
    if recent.is_some() {
        return;
    }
    let token = random_token();
    sqlx::query("insert into magic_links_exposure_academy (token, email, expires_at) values ($1,$2, now() + interval '15 minutes')")
        .bind(&token).bind(email).execute(&app.pool).await.unwrap();
    let link = format!("{}/magic/{}", app.base_url, token);
    send_magic_link_email(app, email, &link).await;
}

pub async fn load_profile(app: &App, uid: Uuid) -> Profile {
    sqlx::query_as::<_, Profile>(
        "select email, display_name, nickname, school, grade from users_exposure_academy where id = $1")
        .bind(uid).fetch_one(&app.pool).await.unwrap()
}

pub async fn profile_page(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Html<String>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let p = load_profile(&app, user.id).await;
    Ok(Html(html::profile(&user, &p, None, None)))
}

pub async fn profile_post(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<ProfileForm>,
) -> Result<Response, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let mut p = load_profile(&app, user.id).await;
    // echo the attempted values back so a rejected edit isn't retyped from scratch
    p.display_name = f.display_name.trim().to_string();
    p.nickname = Some(f.nickname.trim().to_string());
    p.school = Some(f.school.trim().to_string());
    p.grade = Some(f.grade.trim().to_string());
    let err =
        |p: &Profile, msg: &str| Html(html::profile(&user, p, None, Some(msg))).into_response();

    if p.display_name.chars().count() < 2 {
        return Ok(err(&p, "Ad soyadını yaz."));
    }
    let nickname = match validate_nickname(&f.nickname) {
        Ok(n) => n,
        Err(e) => return Ok(err(&p, e)),
    };
    let taken: Option<(Uuid,)> = sqlx::query_as(
        "select id from users_exposure_academy where lower(nickname) = lower($1) and id <> $2",
    )
    .bind(&nickname)
    .bind(user.id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    if taken.is_some() {
        return Ok(err(&p, "Bu nickname alınmış, başka bir tane seç."));
    }
    let school = f.school.trim();
    if school.chars().count() < 2 {
        return Ok(err(&p, "Okulunu yaz."));
    }
    if !GRADES.contains(&f.grade.trim()) {
        return Ok(err(&p, "Sınıfını seç."));
    }

    sqlx::query(
        "update users_exposure_academy
         set display_name = $2, nickname = $3, school = $4, grade = $5
         where id = $1",
    )
    .bind(user.id)
    .bind(&p.display_name)
    .bind(&nickname)
    .bind(school)
    .bind(f.grade.trim())
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;

    // first save completes onboarding — drop them into the portal instead of sitting on /profile
    if user.nickname.is_none() {
        return Ok(Redirect::to("/app").into_response());
    }
    let user = current_user(&app, &headers).await.unwrap_or(user);
    let p = load_profile(&app, user.id).await;
    Ok(Html(html::profile(
        &user,
        &p,
        Some("Profilin güncellendi."),
        None,
    ))
    .into_response())
}

pub async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&headers) {
        let _ = sqlx::query("delete from sessions_exposure_academy where token = $1")
            .bind(t)
            .execute(&app.pool)
            .await;
    }
    (
        [(
            header::SET_COOKIE,
            "session=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax".to_string(),
        )],
        Redirect::to("/"),
    )
        .into_response()
}

/// Constant-time byte equality — no early exit on the first mismatch, so the compare
/// time doesn't leak how many leading bytes were right (would let a co-located
/// attacker recover the token). Length is allowed to short-circuit; it isn't secret.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn check_worker(app: &App, headers: &HeaderMap) -> Result<(), Response> {
    let ok = !app.worker_token.is_empty()
        && headers
            .get("x-worker-token")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| ct_eq(t.as_bytes(), app.worker_token.as_bytes()));
    if ok {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED.into_response())
    }
}

#[derive(Deserialize)]
pub struct ProfileForm {
    display_name: String,
    nickname: String,
    // optional fields: default so a missing one is an empty value, not a 422 with no
    // error banner for the student to read
    #[serde(default)]
    school: String,
    #[serde(default)]
    grade: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shared-secret comparison must not leak length or content by timing.

    #[test]
    fn ct_eq_matches_only_exact() {
        assert!(ct_eq(b"secret-token", b"secret-token"));
        assert!(!ct_eq(b"secret-token", b"secret-toke")); // length differs
        assert!(!ct_eq(b"secret-token", b"Secret-token")); // one byte differs
        assert!(!ct_eq(b"", b"x"));
    }
}
