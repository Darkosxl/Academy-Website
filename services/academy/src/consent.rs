//! Veli onay formları — the signed parental-consent forms students upload, and the
//! admin side that collects them. Ported from the pre-workspace tree, where all of this
//! lived in a single main.rs.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::admin::{IdForm, valid_http_url};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};

/// Per-file ceiling. A phone photo of a signed page is 2–5 MB and a scanned multi-page
/// PDF rarely passes 10, so this leaves headroom without letting a video through.
pub const CONSENT_FILE_MAX_MB: usize = 15;
/// Whole-request ceiling — the form takes several pages at once, so it has to hold a
/// handful of files, not one.
pub const CONSENT_UPLOAD_MAX_MB: usize = 60;
/// Per student per form. Enough for a page-by-page photo set of a long contract,
/// low enough that nobody uses the portal as a file locker.
const CONSENT_MAX_FILES: usize = 12;

/// Which forms are closed for uploads right now, in CONSENT_DOCS order. A form with no
/// settings row falls back to CONSENT_LOCKED_BY_DEFAULT, so a fresh database starts with
/// Paribu closed (its document doesn't exist yet) and the other two open.
pub async fn consent_locks(app: &App) -> Vec<(&'static str, bool)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r"select key, value from app_settings_exposure_academy where key like 'consent\_lock\_%'",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    CONSENT_DOCS
        .iter()
        .map(|(kind, ..)| {
            let stored = rows
                .iter()
                .find(|(key, _)| *key == consent_lock_key(kind))
                .map(|(_, v)| v == "1");
            (
                *kind,
                stored.unwrap_or_else(|| CONSENT_LOCKED_BY_DEFAULT.contains(kind)),
            )
        })
        .collect()
}

/// Where each blank form can be downloaded, in CONSENT_DOCS order: the admin's override
/// if there is one, else the default baked into CONSENT_DOCS. Empty = no link yet, which
/// is Paribu's state until its document exists.
pub async fn consent_urls(app: &App) -> Vec<(&'static str, String)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r"select key, value from app_settings_exposure_academy where key like 'consent\_url\_%'",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default();
    CONSENT_DOCS
        .iter()
        .map(|(kind, _, _, default_url)| {
            let stored = rows
                .iter()
                .find(|(key, _)| *key == consent_url_key(kind))
                .map(|(_, v)| v.trim().to_string());
            (*kind, stored.unwrap_or_else(|| default_url.to_string()))
        })
        .collect()
}

fn consent_is_locked(locks: &[(&'static str, bool)], kind: &str) -> bool {
    locks
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, l)| *l)
        .unwrap_or(true)
}

/// One student's uploaded files, metadata only — the bytes stay in the database until
/// someone actually downloads one.
pub async fn user_consent_docs(app: &App, uid: Uuid) -> Vec<ConsentDoc> {
    sqlx::query_as::<_, ConsentDoc>(
        "select id, user_id, kind, filename, length(file)::bigint as bytes, uploaded_at
         from consent_docs_exposure_academy where user_id = $1 order by kind, uploaded_at",
    )
    .bind(uid)
    .fetch_all(&app.pool)
    .await
    .unwrap_or_default()
}

async fn documents_page(
    app: &App,
    user: &User,
    error: Option<&str>,
    notice: Option<&str>,
) -> Response {
    let docs = user_consent_docs(app, user.id).await;
    let locks = consent_locks(app).await;
    let urls = consent_urls(app).await;
    Html(html::documents(user, &docs, &locks, &urls, error, notice)).into_response()
}

#[derive(Deserialize)]
pub struct OkQ {
    ok: Option<String>,
}

pub async fn documents(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<OkQ>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // POST-redirect-GET: ?ok=1 is what a finished upload comes back as, so a refresh
    // re-renders the page instead of re-posting the files.
    let notice = q.ok.as_deref().map(|_| "Belgelerin yüklendi. Teşekkürler!");
    Ok(documents_page(&app, &user, None, notice).await)
}

/// Content type from the file's own magic bytes rather than whatever the browser
/// claimed — these bytes get served back out under that type, so it has to be one we
/// actually recognised. `None` = not an image we accept.
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if b.len() >= 12 && b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// Content type + canonical extension from the file's own magic bytes. Word documents
/// are the exception: `.docx` is a ZIP and `.doc` an OLE2 container, neither of which
/// says "Word" in its header, so those two fall back to the name the browser sent.
/// `None` = not a format we accept.
fn sniff_document(b: &[u8], filename: &str) -> Option<(&'static str, &'static str)> {
    if b.starts_with(b"%PDF-") {
        return Some(("application/pdf", "pdf"));
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("image/jpeg", "jpg"));
    }
    if let Some(ct) = sniff_image(b) {
        return Some(match ct {
            "image/png" => (ct, "png"),
            "image/gif" => (ct, "gif"),
            _ => (ct, "webp"),
        });
    }
    // ISO-BMFF box: iPhone photos are HEIC unless the phone is set to "Most Compatible"
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return match &b[8..12] {
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" => {
                Some(("image/heic", "heic"))
            }
            b"mif1" | b"msf1" | b"miaf" => Some(("image/heif", "heif")),
            _ => None,
        };
    }
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    // OLE2 compound file — the legacy .doc container (also .xls/.ppt, hence the name check)
    if b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) && ext == "doc" {
        return Some(("application/msword", "doc"));
    }
    // OOXML/ODF are ZIP archives; only trust the name to tell us which, and only for
    // the document formats — everything is served back as an attachment, never inline.
    if b.starts_with(b"PK\x03\x04") {
        return match ext.as_str() {
            "docx" => Some((
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "docx",
            )),
            "odt" => Some(("application/vnd.oasis.opendocument.text", "odt")),
            _ => None,
        };
    }
    None
}

pub async fn documents_upload(
    State(app): State<App>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let too_big = format!(
        "Dosya okunamadı — tek dosya en fazla {CONSENT_FILE_MAX_MB} MB, hepsi birden en fazla {CONSENT_UPLOAD_MAX_MB} MB olabilir."
    );

    let mut kind: Option<&'static str> = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut oversize = false;
    // A body over the route's limit surfaces here as a read error; say so in Turkish
    // rather than letting axum's bare 413 reach a student on a phone.
    let mut read_failed = false;
    while let Some(field) = match mp.next_field().await {
        Ok(f) => f,
        Err(_) => {
            read_failed = true;
            None
        }
    } {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "kind" => kind = field.text().await.ok().and_then(|t| valid_consent_kind(&t)),
            "files" => {
                let filename = field.file_name().unwrap_or("belge").to_string();
                let Ok(bytes) = field.bytes().await else {
                    read_failed = true;
                    break;
                };
                if bytes.len() > CONSENT_FILE_MAX_MB * 1024 * 1024 {
                    oversize = true;
                    break;
                }
                if !bytes.is_empty() {
                    files.push((filename, bytes.to_vec()));
                }
            }
            _ => {}
        }
    }
    if read_failed || oversize {
        return Ok(documents_page(&app, &user, Some(&too_big), None).await);
    }

    let err = |msg: &str| msg.to_string();
    let checked: Result<(&'static str, Vec<(String, Vec<u8>, &'static str)>), String> = async {
        let kind = kind.ok_or_else(|| {
            err("Hangi form olduğu anlaşılamadı, sayfayı yenileyip tekrar dene.")
        })?;
        if consent_is_locked(&consent_locks(&app).await, kind) {
            return Err(format!("{} şu anda yüklemeye kapalı.", consent_title(kind)));
        }
        if files.is_empty() {
            return Err(err("Bir dosya seç."));
        }
        let already: i64 = sqlx::query_scalar(
            "select count(*) from consent_docs_exposure_academy where user_id = $1 and kind = $2",
        )
        .bind(user.id)
        .bind(kind)
        .fetch_one(&app.pool)
        .await
        .unwrap_or(0);
        if already as usize + files.len() > CONSENT_MAX_FILES {
            return Err(format!(
                "Bir form için en fazla {CONSENT_MAX_FILES} dosya yükleyebilirsin ({already} tane zaten yüklü). Fazlasını sil ya da sayfaları tek PDF'te birleştir."
            ));
        }
        let mut prepared = Vec::with_capacity(files.len());
        for (filename, bytes) in files {
            let Some((content_type, ext)) = sniff_document(&bytes, &filename) else {
                return Err(format!(
                    "\"{}\" desteklenmeyen bir dosya türü. PDF, JPG, PNG, HEIC, WebP ya da Word (DOC/DOCX) yükle.",
                    filename.chars().take(60).collect::<String>()
                ));
            };
            // Keep the student's own name, but make sure it ends in the extension the
            // bytes actually are — phone uploads arrive as "image.jpg" or worse, and the
            // admin has to be able to open the file by double-clicking it.
            let base = safe_filename(&filename);
            let named = if base.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
                base
            } else {
                format!("{base}.{ext}")
            };
            prepared.push((named, bytes, content_type));
        }
        Ok((kind, prepared))
    }
    .await;

    let (kind, prepared) = match checked {
        Ok(v) => v,
        Err(msg) => return Ok(documents_page(&app, &user, Some(&msg), None).await),
    };

    for (filename, bytes, content_type) in prepared {
        sqlx::query(
            "insert into consent_docs_exposure_academy (user_id, kind, filename, content_type, file)
             values ($1,$2,$3,$4,$5)",
        )
        .bind(user.id)
        .bind(kind)
        .bind(&filename)
        .bind(content_type)
        .bind(&bytes)
        .execute(&app.pool)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Belge kaydedilemedi, tekrar dene.",
            )
                .into_response()
        })?;
    }
    Ok(Redirect::to("/documents?ok=1").into_response())
}

pub async fn documents_delete(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Form(f): axum::extract::Form<IdForm>,
) -> Result<Response, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // `and user_id` is the authorization: another student's id simply matches nothing.
    // A closed form is also closed for deletions — once it is being collected, the set
    // of files on record stops moving under the admin's feet.
    let locks = consent_locks(&app).await;
    let kind: Option<(String,)> = sqlx::query_as(
        "select kind from consent_docs_exposure_academy where id = $1 and user_id = $2",
    )
    .bind(f.id)
    .bind(user.id)
    .fetch_optional(&app.pool)
    .await
    .unwrap_or(None);
    if let Some((kind,)) = kind {
        if consent_is_locked(&locks, &kind) {
            return Ok(documents_page(
                &app,
                &user,
                Some("Bu form şu anda kapalı, dosyaları değiştiremezsin."),
                None,
            )
            .await);
        }
        sqlx::query("delete from consent_docs_exposure_academy where id = $1 and user_id = $2")
            .bind(f.id)
            .bind(user.id)
            .execute(&app.pool)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    }
    Ok(Redirect::to("/documents").into_response())
}

/// Percent-encoding for the RFC 5987 `filename*` parameter — Turkish names are the
/// normal case here, and a raw ç in a header is not a legal header value.
fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `attachment` both ways: an ASCII-folded name for old clients, the real UTF-8 one for
/// everything else. Always a download, never rendered in the tab — the bytes came from
/// a student, so nothing gets to run in our origin.
fn attachment(filename: &str, content_type: &str, bytes: Vec<u8>) -> Response {
    let name = safe_filename(filename);
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
                    pct_encode(&name)
                ),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        bytes,
    )
        .into_response()
}

/// One uploaded document. The student who uploaded it can fetch it back; an admin can
/// fetch anyone's. Nobody else — a signed consent form has a minor's name, address and
/// a parent's signature on it.
pub async fn document_file(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let row: Option<(Uuid, String, String, Vec<u8>)> = sqlx::query_as(
        "select user_id, filename, content_type, file from consent_docs_exposure_academy where id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .ok()
    .flatten();
    let Some((owner, filename, content_type, bytes)) = row else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    if owner != user.id && !user.is_admin {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(attachment(&filename, &content_type, bytes))
}

#[derive(Deserialize)]
pub struct ConsentLockForm {
    kind: String,
    locked: bool,
}

pub async fn admin_documents_lock(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Form(f): axum::extract::Form<ConsentLockForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let Some(kind) = valid_consent_kind(&f.kind) else {
        return Err((StatusCode::BAD_REQUEST, "Geçersiz form.").into_response());
    };
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ($1,$2, now())
         on conflict (key) do update set value = $2, updated_at = now()",
    )
    .bind(consent_lock_key(kind))
    .bind(if f.locked { "1" } else { "0" })
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

#[derive(Deserialize)]
pub struct ConsentUrlForm {
    kind: String,
    url: String,
}

/// Point a form's "Formu indir" button somewhere — a Drive share link, anything else
/// public, or a `/static/...` path for a document that ships with the site. Blank takes
/// the button off the card.
pub async fn admin_documents_link(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Form(f): axum::extract::Form<ConsentUrlForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let Some(kind) = valid_consent_kind(&f.kind) else {
        return Err((StatusCode::BAD_REQUEST, "Geçersiz form.").into_response());
    };
    // scheme-gate before it ever reaches an href: a javascript:/data: value must never
    // become a link on a page students click through. A same-origin path is allowed too —
    // that is how the Paribu forms are pointed at the PDFs in static/, and it is how one
    // gets restored if somebody clears the field.
    let url = f.url.trim();
    if !url.is_empty() && !valid_http_url(url) && !same_origin_path(url) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Bağlantı http:// veya https:// ile ya da /static/ ile başlamalı.",
        )
            .into_response());
    }
    sqlx::query(
        "insert into app_settings_exposure_academy (key, value, updated_at) values ($1,$2, now())
         on conflict (key) do update set value = $2, updated_at = now()",
    )
    .bind(consent_url_key(kind))
    .bind(url)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// Every consent form on file, as one ZIP: a folder per form, a folder per student
/// inside it, files numbered in upload order. Stored (uncompressed) because the payload
/// is PDFs and phone photos, which don't get smaller — this is a copy, not a squeeze.
///
/// `_EKSIKLER.txt` at the root lists, per form, who has uploaded and who hasn't, so the
/// chasing list comes out of the same download as the documents.
pub async fn admin_documents_zip(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // one bail-out for three unrelated error types (sqlx, zip, io) — a half-written
    // archive is worse than none, so any of them ends the download
    fn oops<E: std::fmt::Display>(e: E) -> Response {
        eprintln!("documents.zip failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }

    let docs: Vec<DocRow> = sqlx::query_as(
        "select u.display_name, u.email, d.kind, d.filename, d.file
         from consent_docs_exposure_academy d
         join users_exposure_academy u on u.id = d.user_id
         order by d.kind, lower(u.display_name), d.uploaded_at, d.id",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(oops)?;
    // Everyone expected to hand something in: every non-admin account, same set the
    // admin grid lists. Deliberately not filtered to onboarded students — an account
    // the admin added by hand that never opened its invite link is exactly the kind of
    // row a chase list has to show.
    let students: Vec<(String, String)> = sqlx::query_as(
        "select display_name, email from users_exposure_academy
         where not is_admin order by lower(display_name)",
    )
    .fetch_all(&app.pool)
    .await
    .map_err(oops)?;

    let summary = format!(
        "Veli onay formları · {}\n{}",
        chrono::Utc::now().format("%d.%m.%Y %H:%M UTC"),
        consent_summary(&docs, &students)
    );
    let bytes = build_documents_zip(&docs, &summary).map_err(oops)?;
    let filename = format!(
        "veli-onay-formlari-{}.zip",
        chrono::Utc::now().format("%Y-%m-%d")
    );
    Ok(attachment(&filename, "application/zip", bytes))
}

/// One `(display_name, email, kind, filename, bytes)` row as it comes out of the join.
type DocRow = (String, String, String, String, Vec<u8>);

/// The `_EKSIKLER.txt` body: per form, who has uploaded and who hasn't. Marked `[X]`
/// or `[ ]` so it reads as a checklist for whoever is chasing the missing ones.
fn consent_summary(docs: &[DocRow], students: &[(String, String)]) -> String {
    let mut out = String::new();
    for (kind, title, ..) in CONSENT_DOCS {
        let has = |email: &str| docs.iter().any(|(_, e, k, ..)| e == email && k == kind);
        let uploaded = students.iter().filter(|(_, email)| has(email)).count();
        out.push_str(&format!(
            "\n=== {title} ===\nYükleyen: {uploaded}/{}\n",
            students.len()
        ));
        for (name, email) in students {
            out.push_str(&format!(
                "  [{}] {name} <{email}>\n",
                if has(email) { "X" } else { " " }
            ));
        }
    }
    out
}

/// A folder per form, a folder per student inside it, files numbered in upload order —
/// so the QNBEYOND folder is exactly what gets handed to QNBEYOND. Stored, not deflated:
/// PDFs and phone photos don't get smaller, so this is a copy rather than a squeeze.
/// Split out of the handler so the layout the admin unzips is covered by a test.
fn build_documents_zip(docs: &[DocRow], summary: &str) -> Result<Vec<u8>, zip::result::ZipError> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("_EKSIKLER.txt", opts)?;
    zip.write_all(summary.as_bytes())?;

    // number files per (form, student) so a student's pages keep their order and two
    // photos both called "image.jpg" can't collide inside the same folder. The rows
    // arrive grouped by (kind, student), which is what makes the running count work.
    let mut n = 0usize;
    let mut prev: Option<(&str, &str)> = None;
    for (name, email, kind, filename, bytes) in docs {
        let who = (kind.as_str(), email.as_str());
        n = if prev == Some(who) { n + 1 } else { 1 };
        prev = Some(who);
        zip.start_file(
            format!(
                "{kind}/{student}/{n:02}-{file}",
                student = safe_filename(name),
                file = safe_filename(filename)
            ),
            opts,
        )?;
        zip.write_all(bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names go into a Content-Disposition header and into ZIP entry paths, so a
    /// student can't send one that escapes either.
    #[test]
    fn filenames_cannot_escape_a_header_or_a_folder() {
        assert_eq!(safe_filename("../../etc/passwd"), "etc_passwd");
        assert_eq!(
            safe_filename(r#"a"; filename="b.pdf"#),
            "a_; filename=_b.pdf"
        );
        assert_eq!(safe_filename("veli\r\nonay.pdf"), "veli__onay.pdf");
        assert_eq!(safe_filename("   "), "belge");
        assert_eq!(safe_filename("...."), "belge");
        assert_eq!(safe_filename("İzin Formu.pdf"), "İzin Formu.pdf"); // Turkish survives
        assert_eq!(safe_filename(&"a".repeat(400)).chars().count(), 120);
        // the header itself is ASCII either way: the real name rides in filename*
        assert!(pct_encode("İzin Formu.pdf").is_ascii());
        assert_eq!(pct_encode("a b.pdf"), "a%20b.pdf");
    }

    /// The bytes decide the type, not the browser — and the formats a consent form
    /// is definitely not are refused outright.
    #[test]
    fn documents_are_sniffed_not_trusted() {
        assert_eq!(
            sniff_document(b"%PDF-1.7", "x.png"),
            Some(("application/pdf", "pdf"))
        );
        assert_eq!(
            sniff_document(&[0xFF, 0xD8, 0xFF, 0xE0], "x.pdf"),
            Some(("image/jpeg", "jpg"))
        );
        for (bytes, name) in [
            (&b"\0asm\x01\0\0\0"[..], "x.wasm"),
            (b"<svg onload=alert(1)>", "x.svg"),
            (b"MZ\x90\0", "x.exe"),
            (b"", "empty.pdf"),
        ] {
            assert_eq!(sniff_document(bytes, name), None, "{name}");
        }
    }

    /// The archive an admin downloads: it opens, `_EKSIKLER.txt` is in it, and every
    /// document is where the panel promises — form folder, student folder, numbered.
    #[test]
    fn documents_zip_round_trips() {
        let rows: Vec<DocRow> = vec![
            (
                "Ada Çelik".into(),
                "ada@x.com".into(),
                "exposure".into(),
                "sayfa1.pdf".into(),
                b"PDF-ONE".to_vec(),
            ),
            (
                "Ada Çelik".into(),
                "ada@x.com".into(),
                "exposure".into(),
                "sayfa2.pdf".into(),
                b"PDF-TWO".to_vec(),
            ),
            (
                "Bora Ay".into(),
                "bora@x.com".into(),
                "exposure".into(),
                "../evil.jpg".into(),
                b"JPG".to_vec(),
            ),
            (
                "Ada Çelik".into(),
                "ada@x.com".into(),
                "qnbeyond".into(),
                "izin.jpg".into(),
                b"QNB".to_vec(),
            ),
        ];
        let students = vec![
            ("Ada Çelik".to_string(), "ada@x.com".to_string()),
            ("Bora Ay".to_string(), "bora@x.com".to_string()),
        ];
        let summary = consent_summary(&rows, &students);
        assert!(summary.contains("[X] Ada Çelik <ada@x.com>"));
        assert!(
            summary.contains("[ ] Bora Ay <bora@x.com>"),
            "bora has no QNBEYOND form"
        );
        assert!(summary.contains("Yükleyen: 2/2") && summary.contains("Yükleyen: 1/2"));

        let bytes = build_documents_zip(&rows, &summary).expect("zip built");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip opens");
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        for want in [
            "_EKSIKLER.txt",
            "exposure/Ada Çelik/01-sayfa1.pdf",
            "exposure/Ada Çelik/02-sayfa2.pdf", // second page of the same form
            "exposure/Bora Ay/01-evil.jpg",     // path traversal flattened
            "qnbeyond/Ada Çelik/01-izin.jpg",
        ] {
            // numbering restarts per form
            assert!(
                names.contains(&want.to_string()),
                "{want} missing from {names:?}"
            );
        }
        assert_eq!(names.len(), 5);
        let mut f = archive
            .by_name("exposure/Ada Çelik/02-sayfa2.pdf")
            .expect("entry readable");
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut content).unwrap();
        assert_eq!(content, b"PDF-TWO", "bytes come back out unchanged");
    }
}
