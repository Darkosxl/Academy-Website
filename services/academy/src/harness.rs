//! Agentic Harness: the student page, the interim team admin, and the worker API
//! the runner drives a submission through.

use crate::admin::IdForm;
use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use benchmark_protocol::{
    ARC_GAMES, ArcFrame as HarnessArcFrame, ArcFramesRequest as HarnessArcFramesReq, BenchmarkKind,
    DEFAULT_BEDROCK_MODEL, DEFAULT_CEREBRAS_MODEL, DEFAULT_DEEPINFRA_MODEL, HarnessCapacity,
    HarnessClaim, HarnessLeaseRequest as HarnessLeaseReq,
    HarnessProgressRequest as HarnessProgressReq, HarnessResultRequest as HarnessResultReq,
    HarnessStageRequest as HarnessStageReq, KaggleClaim,
    KaggleResultRequest as HarnessKaggleResultReq, ModelProvider, RUN_DEADLINE_SECONDS,
    builtin_harness_uri, is_builtin_harness,
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct HarnessQ {
    tab: Option<String>,
    bench: Option<String>,
    /// `?tab=live&run=` — a finished run to replay. Absent means "watch the latest one".
    run: Option<Uuid>,
    /// What just happened, so the page can say it inline instead of a handler
    /// dead-ending on a plain-text 400: `busy` (a teammate is already running),
    /// `named` / `name-taken` / `name-long` (team rename).
    msg: Option<String>,
}

/// The team the student belongs to, if any. Teams are admin-assigned for now.
pub async fn harness_team_of(app: &App, uid: Uuid) -> Option<HarnessTeam> {
    sqlx::query_as(
        "select t.id, t.name from harness_team_members_exposure_academy tm
         join harness_teams_exposure_academy t on t.id = tm.team_id
         where tm.user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

pub async fn agentic_harness(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessQ>,
) -> Result<Html<String>, Response> {
    agentic_harness_page(app, headers, q, None).await
}

pub async fn agentic_harness_arc(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessQ>,
) -> Result<Html<String>, Response> {
    agentic_harness_page(app, headers, q, Some(BenchmarkKind::Arc)).await
}

pub async fn agentic_harness_frontier(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessQ>,
) -> Result<Html<String>, Response> {
    agentic_harness_page(app, headers, q, Some(BenchmarkKind::Frontier)).await
}

async fn agentic_harness_page(
    app: App,
    headers: HeaderMap,
    q: HarnessQ,
    page_kind: Option<BenchmarkKind>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // unknown values fall back to defaults, same as /demos?lang=
    let tab = match q.tab.as_deref() {
        Some("history") => "history",
        Some("instructions") => "instructions",
        Some("live") => "live",
        _ => "main",
    };
    if tab == "instructions" {
        return Ok(Html(html::agentic_harness_instructions(&user)));
    }
    if tab == "live" {
        // Team-scoped exactly like harness_arc_live below: a ?run= belonging to another
        // team matches no row, so the page renders empty instead of their boards.
        let run: Option<HarnessRun> = sqlx::query_as(
            "select r.id, r.repo_url, r.model_id, r.provider, r.benchmark_kind,
                    r.commit_sha, r.stage, r.benchmark_version,
                    r.benchmark_state, r.bedrock_profile, r.deadline_at, r.score_arc,
                    r.score_frontier, r.ram_1session_mb, r.ram_10session_mb,
                    r.error_log, r.created_at
             from harness_runs_exposure_academy r
             join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
             where tm.user_id = $1 and ($2::uuid is null or r.id = $2)
               and ($2::uuid is not null or r.benchmark_kind in ('arc', 'bundled'))
             order by r.created_at desc limit 1",
        )
        .bind(user.id)
        .bind(q.run)
        .fetch_optional(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        return Ok(Html(html::agentic_harness_live(
            &user,
            run.as_ref(),
            q.run.is_some(),
        )));
    }
    let team = harness_team_of(&app, user.id).await;
    if tab == "history" {
        let runs: Vec<HarnessRun> = match &team {
            Some(t) => sqlx::query_as(
                "select id, repo_url, model_id, provider, benchmark_kind,
                        commit_sha, stage, benchmark_version,
                        benchmark_state, bedrock_profile, deadline_at, score_arc,
                        score_frontier, ram_1session_mb, ram_10session_mb,
                        error_log, created_at
                 from harness_runs_exposure_academy where team_id = $1 order by created_at desc",
            )
            .bind(t.id)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
            None => Vec::new(),
        };
        let credential_username: Option<String> = match &team {
            Some(t) => sqlx::query_scalar(
                "select username from harness_kaggle_credentials_exposure_academy where team_id = $1")
                .bind(t.id).fetch_optional(&app.pool).await.unwrap(),
            None => None,
        };
        let official: Vec<HarnessKaggleSubmission> = match &team {
            Some(t) => sqlx::query_as(
                "select j.run_id, j.status, j.kernel_slug, j.kernel_version,
                        j.submission_ref, j.public_score, j.private_score,
                        j.status_message, j.updated_at
                 from harness_kaggle_submissions_exposure_academy j
                 join harness_runs_exposure_academy r on r.id = j.run_id
                 where r.team_id = $1 order by j.created_at desc",
            )
            .bind(t.id)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
            None => Vec::new(),
        };
        return Ok(Html(html::agentic_harness_history(
            &user,
            team.as_ref(),
            &runs,
            app.kaggle_key.is_some(),
            credential_username.as_deref(),
            &official,
        )));
    }
    let bench = match page_kind {
        Some(BenchmarkKind::Frontier) => "frontier",
        Some(BenchmarkKind::Arc) => "arc",
        _ => match q.bench.as_deref() {
            Some("frontier") => "frontier",
            Some("ram") => "ram",
            _ => "arc",
        },
    };
    // Kid names shown next to each team, real names per the leaderboard convention.
    // One query for all teams; html filters per row in memory, same as board() does
    // with interests. `public` = onboarded and not hidden — the leaderboard shows only
    // those, but your own team panel shows the full roster (admins/interns included,
    // it's your roster, not the published standings).
    let members: Vec<TeamMemberRow> = sqlx::query_as(
        "select tm.team_id, tm.user_id, u.display_name,
                (u.nickname is not null and not u.hidden_from_leaderboard) as public
         from harness_team_members_exposure_academy tm
         join users_exposure_academy u on u.id = tm.user_id
         order by tm.created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let active_run: Option<HarnessRun> = match &team {
        Some(t) => sqlx::query_as(
            "select id, repo_url, model_id, provider, benchmark_kind,
                    commit_sha, stage, benchmark_version,
                    benchmark_state, bedrock_profile, deadline_at, score_arc,
                    score_frontier, ram_1session_mb, ram_10session_mb,
                    error_log, created_at
             from harness_runs_exposure_academy
             where team_id = $1
               and ($2::text = 'ram' or benchmark_kind in ($2, 'bundled'))
               and stage not in ('done','partial','failed','infra_failed','cancelled')
             order by created_at desc limit 1",
        )
        .bind(t.id)
        .bind(bench)
        .fetch_optional(&app.pool)
        .await
        .unwrap(),
        None => None,
    };
    // Who pressed submit. Fetched separately rather than widened into HarnessRun, which
    // would force the column into all four query_as sites for one label. `submitted_by`
    // is `on delete set null`, so a deleted account leaves this None.
    let submitter: Option<String> = match &active_run {
        Some(run) => sqlx::query_scalar(
            "select coalesce(u.nickname, u.display_name)
             from harness_runs_exposure_academy r
             join users_exposure_academy u on u.id = r.submitted_by
             where r.id = $1",
        )
        .bind(run.id)
        .fetch_optional(&app.pool)
        .await
        .unwrap()
        .flatten(),
        None => None,
    };
    // Best score per team over its done runs. ARC/Frontier: higher wins. RAM: ranked
    // by the lowest 10-session PSS, and the 1-session column comes from that same run
    // (distinct on picks it), not from whichever run happened to have the lowest 1s.
    let (rows, ram_rows): (Vec<HarnessLeaderRow>, Vec<HarnessRamRow>) = if bench == "ram" {
        (
            Vec::new(),
            sqlx::query_as(
                "select id, name, ram_1session_mb, ram_10session_mb from (
               select distinct on (t.id) t.id, t.name, r.ram_1session_mb, r.ram_10session_mb
               from harness_teams_exposure_academy t
               join harness_runs_exposure_academy r
                 on r.team_id = t.id and r.benchmark_version = $1
                    and r.ram_10session_mb is not null
                    and r.stage <> 'cancelled'
               order by t.id, r.ram_10session_mb asc, r.created_at desc
             ) best order by ram_10session_mb asc, lower(name)",
            )
            .bind(HARNESS_VERSION)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
        )
    } else {
        let sql = if bench == "frontier" {
            "select t.id, t.name, max(r.score_frontier) as best
             from harness_teams_exposure_academy t
             join harness_runs_exposure_academy r
               on r.team_id = t.id and r.benchmark_version = $1
                  and r.score_frontier is not null
                  and r.stage <> 'cancelled'
             group by t.id, t.name order by best desc, lower(t.name)"
        } else {
            "select t.id, t.name, max(r.score_arc) as best
             from harness_teams_exposure_academy t
             join harness_runs_exposure_academy r
               on r.team_id = t.id and r.benchmark_version = $1
                  and r.score_arc is not null
                  and r.stage <> 'cancelled'
             group by t.id, t.name order by best desc, lower(t.name)"
        };
        (
            sqlx::query_as(sql)
                .bind(HARNESS_VERSION)
                .fetch_all(&app.pool)
                .await
                .unwrap(),
            Vec::new(),
        )
    };
    Ok(Html(html::agentic_harness_main(
        &user,
        bench,
        team.as_ref(),
        &members,
        active_run.as_ref(),
        submitter.as_deref(),
        q.msg.as_deref(),
        &rows,
        &ram_rows,
    )))
}

/// Admin-only page under "Yönetici paneli": the enhanced agent/provider/model submit form,
/// scoped to the admin's own team exactly like `agentic_harness_page` was before that form
/// moved out of the shared page. RAM isn't independently submittable, so `bench` is arc/frontier only.
pub async fn admin_harness_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessQ>,
) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    let bench = match q.bench.as_deref() {
        Some("frontier") => "frontier",
        _ => "arc",
    };
    let team = harness_team_of(&app, user.id).await;
    let members: Vec<TeamMemberRow> = sqlx::query_as(
        "select tm.team_id, tm.user_id, u.display_name,
                (u.nickname is not null and not u.hidden_from_leaderboard) as public
         from harness_team_members_exposure_academy tm
         join users_exposure_academy u on u.id = tm.user_id
         order by tm.created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let active_run: Option<HarnessRun> = match &team {
        Some(t) => sqlx::query_as(
            "select id, repo_url, model_id, provider, benchmark_kind,
                    commit_sha, stage, benchmark_version,
                    benchmark_state, bedrock_profile, deadline_at, score_arc,
                    score_frontier, ram_1session_mb, ram_10session_mb,
                    error_log, created_at
             from harness_runs_exposure_academy
             where team_id = $1
               and benchmark_kind in ($2, 'bundled')
               and stage not in ('done','partial','failed','infra_failed','cancelled')
             order by created_at desc limit 1",
        )
        .bind(t.id)
        .bind(bench)
        .fetch_optional(&app.pool)
        .await
        .unwrap(),
        None => None,
    };
    Ok(Html(html::admin_harness_page(
        &user,
        bench,
        team.as_ref(),
        &members,
        active_run.as_ref(),
    )))
}

#[derive(Deserialize)]
pub struct HarnessSubmitForm {
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    model_id: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    benchmark_kind: String,
    #[serde(default)]
    builtin_harness: String,
}

/// Why a pasted repo link isn't usable. One variant per thing the student can *fix*, not one
/// per branch in the parser: this used to be a bare `Option`, so every failure below printed
/// "Repo bağlantısı https://github.com/ ile başlamalı" — a sentence that was false for almost
/// every link it was shown for, since the address-bar copy of a repo folder does start with
/// exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoUrlError {
    Empty,
    TooLong,
    /// Not parseable even after the fix-ups in `github_repo_url` — prose, or a typo'd scheme.
    NotAUrl,
    /// Parses, but the host isn't github.com. Covers `github.com.evil.com`.
    NotGithub,
    GistLink,
    RawFileLink,
    /// `user:pass@` or an explicit port. (The scp form `git@github.com:` is rewritten, not
    /// rejected — it reaches this check with no username left.)
    Credentials,
    /// `https://github.com/` — no owner at all.
    NoRepo,
    /// `https://github.com/ali` — a profile, not a repo.
    OwnerOnly,
    /// The owner slot is one of GitHub's own namespaces: /orgs/…, /settings/…, /apps/….
    ReservedOwner,
    /// Turkish letters (or any non-ASCII) in owner/repo. GitHub URLs never contain these.
    NonAscii,
    /// ASCII but outside `[A-Za-z0-9._-]` — a space, a quote, a stray character.
    BadChars,
    SegmentTooLong,
}

/// Owner-position paths GitHub keeps for itself. Without this, truncating a deep path to its
/// first two segments would turn https://github.com/orgs/exposure/repositories into the
/// nonexistent repo "orgs/exposure" — a clone failure days later instead of a sentence now.
/// GitHub reserves these, so no real account can shadow one; if it ever releases one, the fix
/// is deleting a string here and the symptom until then is a clear message, not a silent drop.
const GITHUB_RESERVED_OWNERS: [&str; 24] = [
    "orgs",
    "users",
    "settings",
    "sponsors",
    "topics",
    "search",
    "apps",
    "marketplace",
    "collections",
    "codespaces",
    "notifications",
    "explore",
    "features",
    "pricing",
    "about",
    "login",
    "join",
    "new",
    "organizations",
    "account",
    "dashboard",
    "trending",
    "site",
    "contact",
];

/// Generous: the old 300 cap rejected a `/blob/main/deep/path#L12-L40` before we got the
/// chance to truncate it down to the repo it names. The per-segment caps below are what
/// actually bound the stored value.
const REPO_URL_MAX: usize = 2000;

/// A canonical `https://github.com/{owner}/{repo}` out of whatever the student pasted, or the
/// specific reason it can't be one.
///
/// Forgiving on the way in, unchanged on the way out. Every fix-up here is a lossless rewrite
/// of a string that names exactly one repo — a subpath, a query string, the scp form, a
/// missing scheme — and the result still passes the same host/segment/charset gate the strict
/// version used. Nothing new can reach the worker: `executor.rs::valid_repo_url` and
/// `runner.py::valid_repo_url` re-check the stored value and both are strict, so the output
/// shape is a contract (see `normalized_urls_satisfy_the_worker_contract`).
fn github_repo_url(raw: &str) -> Result<String, RepoUrlError> {
    use RepoUrlError::*;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(Empty);
    }
    if raw.len() > REPO_URL_MAX {
        return Err(TooLong);
    }
    // The scp form and a bare `github.com/...` both name the host literally, so rewriting is
    // safe and the alternative ("that's not a URL") teaches the student nothing.
    let rewritten;
    let candidate = if let Some(rest) = raw.strip_prefix("git@github.com:") {
        rewritten = format!("https://github.com/{rest}");
        rewritten.as_str()
    } else if !raw.contains("://") {
        let lower = raw.to_ascii_lowercase();
        if !lower.starts_with("github.com/") && !lower.starts_with("www.github.com/") {
            return Err(NotAUrl);
        }
        rewritten = format!("https://{raw}");
        rewritten.as_str()
    } else {
        raw
    };
    let url = reqwest::Url::parse(candidate).map_err(|_| NotAUrl)?;

    let host_raw = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host = host_raw.strip_prefix("www.").unwrap_or(host_raw.as_str());
    // The equality arm is the security-critical line here: `github.com.evil.com` and
    // `notgithub.com` must both fall through to NotGithub.
    match host {
        "github.com" => {}
        "gist.github.com" => return Err(GistLink),
        "raw.githubusercontent.com" | "objects.githubusercontent.com" => return Err(RawFileLink),
        _ => return Err(NotGithub),
    }

    // `ssh://git@github.com/o/r` carries its username in the URL rather than a prefix, so it
    // has to be recognised before the credentials check would reject it. We still clone over
    // https — the scheme is normalized away below, never honoured.
    let ssh_git = url.scheme() == "ssh" && url.username() == "git" && url.password().is_none();
    if !ssh_git && !matches!(url.scheme(), "https" | "http") {
        return Err(NotAUrl);
    }
    if (!ssh_git && !url.username().is_empty()) || url.password().is_some() || url.port().is_some()
    {
        return Err(Credentials);
    }

    // Query and fragment are ignored rather than rejected: no GitHub query parameter changes
    // *which* repo a URL names, and neither survives into the stored value anyway. This alone
    // is what fixes the `?tab=readme-ov-file` that GitHub's own copy button appends.
    let segments: Vec<&str> = url
        .path_segments()
        .map(|parts| parts.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let Some(owner) = segments.first().copied() else {
        return Err(NoRepo);
    };
    if GITHUB_RESERVED_OWNERS.contains(&owner.to_ascii_lowercase().as_str()) {
        return Err(ReservedOwner);
    }
    if segments.len() < 2 {
        return Err(OwnerOnly);
    }
    // Everything past owner/repo — tree/main/src, blob/…, pull/3, issues, actions/runs/… — is
    // *inside* the repo those two segments name, so it truncates rather than rejects. Blanket
    // truncation over a /tree|/blob allowlist, which silently rots when GitHub adds a route;
    // the reserved-owner check above covers the only case where segment 1 isn't a repo name.
    let repo = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
    if repo.is_empty() {
        return Err(NoRepo);
    }

    // Never percent-decode. `Url` encodes `ödev` to `%C3%B6dev` and a space to `%20`, which
    // both fail the allowlist identically — checking the *raw* string is the only way to tell
    // "Turkish letters" from "a stray space", and those need very different advice.
    let raw_path = raw.split(['?', '#']).next().unwrap_or(raw);
    let bad_charset = |part: &str| {
        !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if bad_charset(owner) || bad_charset(repo) {
        return Err(if raw_path.is_ascii() { BadChars } else { NonAscii });
    }
    // GitHub's own limits, and what bounds the value we store and render.
    if owner.len() > 39 || repo.len() > 100 {
        return Err(SegmentTooLong);
    }
    // Case is preserved, not lowercased: GitHub is case-preserving, `parse_github_repo` in
    // admin.rs preserves it, and the api.github.com live-site resolver relies on it.
    Ok(format!("https://github.com/{owner}/{repo}"))
}

/// The student-facing sentence for each reason. Every one names the thing to change and shows
/// the target shape, because the reader is a teenager looking at a plain 400 page.
fn repo_error_tr(err: RepoUrlError) -> &'static str {
    use RepoUrlError::*;
    match err {
        Empty => "Repo bağlantısı boş. GitHub'da repo sayfanı aç, adres çubuğundaki bağlantıyı kopyalayıp buraya yapıştır.",
        TooLong => "Bağlantı çok uzun. Yalnızca repo sayfasının adresini yapıştır: https://github.com/kullanici/repo",
        NotAUrl => "Bu bir bağlantıya benzemiyor. Repo sayfasının adresini olduğu gibi yapıştır: https://github.com/kullanici/repo",
        NotGithub => "Bu bağlantı github.com adresinde değil. Ajanını GitHub'a yükle ve repo sayfasının adresini yapıştır: https://github.com/kullanici/repo",
        GistLink => "Bu bir Gist bağlantısı. Ajanın normal bir GitHub repo'su olmalı — repo'nun ana sayfasının adresini yapıştır.",
        RawFileLink => "Bu tek bir dosyanın (raw) bağlantısı. Repo'nun ana sayfasına dön ve oradaki adresi yapıştır: https://github.com/kullanici/repo",
        Credentials => "Bağlantıda kullanıcı adı, şifre veya port var. Sade repo adresini yapıştır: https://github.com/kullanici/repo",
        NoRepo => "Bağlantıda repo adı yok. https://github.com/kullanici/repo biçiminde olmalı.",
        OwnerOnly => "Bu senin GitHub profilinin bağlantısı, repo'nun değil. Profilinde ajanın repo'sunu aç, sonra o sayfanın adresini yapıştır: https://github.com/kullanici/repo",
        ReservedOwner => "Bu bir repo sayfası değil, GitHub'ın kendi sayfalarından biri. Repo'nun ana sayfasını aç ve oradaki adresi yapıştır.",
        NonAscii => "Repo veya kullanıcı adında Türkçe karakter (ö, ç, ş, ğ, ı, ü) görünüyor. GitHub adreslerinde bu harfler bulunmaz — repo sayfasını aç ve adres çubuğundaki adresi olduğu gibi kopyala.",
        BadChars => "Repo veya kullanıcı adında geçersiz karakter var (boşluk gibi). Yalnızca harf, rakam, '-', '_' ve '.' olabilir — bağlantıyı elle yazmak yerine adres çubuğundan kopyala.",
        SegmentTooLong => "Kullanıcı veya repo adı çok uzun. Repo sayfasının adresini yapıştır: https://github.com/kullanici/repo",
    }
}

fn submitted_provider(is_admin: bool, raw: &str) -> Option<ModelProvider> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Some(ModelProvider::Cerebras);
    }
    let provider: ModelProvider = raw.parse().ok()?;
    // Students pick between the two hosted pools; Bedrock stays admin-only.
    (is_admin || provider != ModelProvider::Bedrock).then_some(provider)
}

fn submitted_model_id(provider: ModelProvider, raw: &str, requires_images: bool) -> Option<&str> {
    let model_id = match raw.trim() {
        "" => Some(match provider {
            ModelProvider::Bedrock => DEFAULT_BEDROCK_MODEL,
            ModelProvider::Cerebras => DEFAULT_CEREBRAS_MODEL,
            ModelProvider::DeepInfra => DEFAULT_DEEPINFRA_MODEL,
        }),
        model_id if provider.supports_model(model_id) => Some(model_id),
        _ => None,
    }?;
    (!requires_images || provider.supports_images(model_id)).then_some(model_id)
}

/// Which of the two mutually exclusive sources the form carried, or why neither worked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceError {
    Repo(RepoUrlError),
    /// Both fields filled — we won't guess which one was meant.
    Both,
    /// Neither field filled.
    Neither,
    /// A builtin picked by a non-admin.
    BuiltinForbidden,
    /// Unknown builtin slug — a hand-rolled POST, or a stale form.
    BuiltinUnknown,
}

fn submission_source(
    is_admin: bool,
    repo_url: &str,
    builtin_harness: &str,
) -> Result<String, SourceError> {
    let repo_url = repo_url.trim();
    let builtin_harness = builtin_harness.trim();
    match (repo_url.is_empty(), builtin_harness.is_empty()) {
        (false, true) => github_repo_url(repo_url).map_err(SourceError::Repo),
        (true, false) if is_admin => builtin_harness_uri(builtin_harness)
            .map(String::from)
            .ok_or(SourceError::BuiltinUnknown),
        (true, false) => Err(SourceError::BuiltinForbidden),
        (true, true) => Err(SourceError::Neither),
        (false, false) => Err(SourceError::Both),
    }
}

/// The admin/student split survives the refactor: an admin who submitted nothing has a
/// different next action (pick a builtin) than a student staring at an empty box.
fn source_error_tr(is_admin: bool, err: SourceError) -> &'static str {
    const PICK_ONE: &str = "GitHub bağlantısı gir veya bir hazır harness seç.";
    match err {
        SourceError::Repo(RepoUrlError::Empty) | SourceError::Neither if is_admin => PICK_ONE,
        SourceError::Repo(inner) => repo_error_tr(inner),
        SourceError::Neither => repo_error_tr(RepoUrlError::Empty),
        SourceError::Both => {
            "Ya bir GitHub bağlantısı gir ya da hazır bir harness seç — ikisini birden değil."
        }
        SourceError::BuiltinForbidden => {
            "Hazır harness seçme iznin yok — kendi repo bağlantını gir."
        }
        SourceError::BuiltinUnknown => "Bilinmeyen hazır harness.",
    }
}

/// Stable, greppable slug per reason for the `reason` column. Deliberately a plain string
/// rather than a check-constrained enum, which a new variant would break on an old database.
fn source_error_slug(err: SourceError) -> &'static str {
    use RepoUrlError::*;
    match err {
        SourceError::Repo(Empty) => "empty",
        SourceError::Repo(TooLong) => "too_long",
        SourceError::Repo(NotAUrl) => "not_a_url",
        SourceError::Repo(NotGithub) => "not_github",
        SourceError::Repo(GistLink) => "gist_link",
        SourceError::Repo(RawFileLink) => "raw_file_link",
        SourceError::Repo(Credentials) => "credentials",
        SourceError::Repo(NoRepo) => "no_repo",
        SourceError::Repo(OwnerOnly) => "owner_only",
        SourceError::Repo(ReservedOwner) => "reserved_owner",
        SourceError::Repo(NonAscii) => "non_ascii",
        SourceError::Repo(BadChars) => "bad_chars",
        SourceError::Repo(SegmentTooLong) => "segment_too_long",
        SourceError::Both => "both_sources",
        SourceError::Neither => "no_source",
        SourceError::BuiltinForbidden => "builtin_forbidden",
        SourceError::BuiltinUnknown => "builtin_unknown",
    }
}

/// Rejections are diagnostic, not authoritative: a failed insert must never turn a helpful
/// 400 into a 500, so this swallows its error after a stderr line. Truncation is by chars and
/// not bytes — the strings that land here are exactly the ones with Turkish letters in them.
const HARNESS_RAW_INPUT_MAX: usize = 500;

async fn record_rejected_submission(
    app: &App,
    user_id: Uuid,
    team_id: Uuid,
    raw_input: &str,
    err: SourceError,
    benchmark_kind: &str,
) {
    let raw: String = raw_input
        .trim()
        .chars()
        .take(HARNESS_RAW_INPUT_MAX)
        .collect();
    let done = sqlx::query(
        "insert into harness_rejected_submissions_exposure_academy
           (user_id, team_id, raw_input, reason, benchmark_kind)
         values ($1,$2,$3,$4,$5)",
    )
    .bind(user_id)
    .bind(team_id)
    .bind(&raw)
    .bind(source_error_slug(err))
    .bind(benchmark_kind.trim())
    .execute(&app.pool)
    .await;
    if let Err(error) = done {
        eprintln!("rejected-submission audit insert failed: {error}");
    }
}

pub async fn harness_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessSubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();
    let Some(team) = harness_team_of(&app, user.id).await else {
        // the form isn't rendered for team-less students; this catches hand-rolled POSTs
        return Err(bad(
            "Takımın olmadığı için gönderim yapamazsın — eğitmenine yaz.",
        ));
    };
    let Some(benchmark_kind) = f
        .benchmark_kind
        .trim()
        .parse::<BenchmarkKind>()
        .ok()
        .filter(|kind| matches!(kind, BenchmarkKind::Arc | BenchmarkKind::Frontier))
    else {
        return Err(bad("ARC veya Frontier çalıştırması seç."));
    };
    let Some(provider) = submitted_provider(user.is_admin, &f.provider) else {
        return Err(bad("Bu sağlayıcıyı kullanma iznin yok."));
    };
    let repo_url = match submission_source(user.is_admin, &f.repo_url, &f.builtin_harness) {
        Ok(url) => url,
        Err(err) => {
            // The whole point of the audit table: the next time a student insists he sent a
            // GitHub link, /admin can show exactly what he sent. The stderr line deliberately
            // omits the raw input — that's the one field here with a privacy cost, and the
            // table already holds it behind an admin session.
            eprintln!("harness submit rejected: user={} reason={err:?}", user.id);
            record_rejected_submission(
                &app,
                user.id,
                team.id,
                &f.repo_url,
                err,
                &f.benchmark_kind,
            )
            .await;
            return Err(bad(source_error_tr(user.is_admin, err)));
        }
    };
    let requires_images = benchmark_kind == BenchmarkKind::Arc && is_builtin_harness(&repo_url);
    let Some(model_id) = submitted_model_id(provider, &f.model_id, requires_images) else {
        return Err(bad(
            "Bu agent için görüntü destekleyen geçerli bir model seç.",
        ));
    };
    // A teammate already running is a normal race, not an error: send them back to the
    // page, which explains who started it and offers a watch button. `busy=1` is the
    // only difference from the success redirect below.
    let busy = || {
        Redirect::to(match benchmark_kind {
            BenchmarkKind::Arc => "/agentic-harness/arc?msg=busy",
            BenchmarkKind::Frontier => "/agentic-harness/frontier?msg=busy",
            BenchmarkKind::Bundled => unreachable!(),
        })
    };
    let active: Option<Uuid> = sqlx::query_scalar(
        "select id from harness_runs_exposure_academy where team_id = $1
         and benchmark_kind in ($2, 'bundled')
         and stage not in ('done','partial','failed','infra_failed','cancelled')",
    )
    .bind(team.id)
    .bind(benchmark_kind.as_str())
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if active.is_some() {
        return Ok(busy());
    }
    // The one-active-kind partial unique index backstops the pre-check above, so a
    // double-click race lands here as a constraint error, not a second run — send it
    // down the same path instead of unwrap-panicking into a 500.
    let inserted = sqlx::query(
        "insert into harness_runs_exposure_academy
           (team_id, submitted_by, repo_url, model_id, provider, benchmark_kind,
            benchmark_version)
         values ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(team.id)
    .bind(user.id)
    .bind(&repo_url)
    .bind(model_id)
    .bind(provider.as_str())
    .bind(benchmark_kind.as_str())
    .bind(HARNESS_VERSION)
    .execute(&app.pool)
    .await;
    if inserted.is_err() {
        return Ok(busy());
    }
    Ok(Redirect::to(match benchmark_kind {
        BenchmarkKind::Arc => "/agentic-harness/arc",
        BenchmarkKind::Frontier => "/agentic-harness/frontier",
        BenchmarkKind::Bundled => unreachable!(),
    }))
}

#[derive(Deserialize)]
pub struct TeamNameForm {
    name: String,
}

/// Teams name themselves. Any member can, same rule as submitting — and there is no
/// budget on it: the name is only a label, runs and scores are keyed on team id, so
/// the worst case is an admin renaming them back from /admin/takimlar.
///
/// No team id in the form on purpose: the session decides which team gets renamed, so
/// a hand-rolled POST can't touch anyone else's.
pub async fn harness_team_rename(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<TeamNameForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Takımın olmadığı için isim değiştiremezsin.".to_string(),
        )
            .into_response());
    };
    let name = f.name.trim();
    // chars(), not len(): "Yapay Zekâ Kâşifleri" is fewer letters than bytes
    if name.is_empty() || name.chars().count() > HARNESS_TEAM_NAME_MAX {
        return Ok(Redirect::to("/agentic-harness?msg=name-long"));
    }
    if name == team.name {
        return Ok(Redirect::to("/agentic-harness"));
    }
    // unique on lower(name) — a clash is the only way this fails
    let done = sqlx::query("update harness_teams_exposure_academy set name = $2 where id = $1")
        .bind(team.id)
        .bind(name)
        .execute(&app.pool)
        .await;
    Ok(Redirect::to(if done.is_err() {
        "/agentic-harness?msg=name-taken"
    } else {
        "/agentic-harness?msg=named"
    }))
}

/// Team-scoped cancellation. Clearing the lease makes the controller's next frame,
/// progress, or heartbeat receive 409; it then sends Cancel to the restricted executor.
pub async fn harness_stop(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let result = sqlx::query(
        "update harness_runs_exposure_academy r
         set stage = 'cancelled', error_log = 'Takım üyesi tarafından durduruldu.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where r.id = $1
           and r.stage not in ('done','partial','failed','infra_failed','cancelled')
           and exists (
             select 1 from harness_team_members_exposure_academy tm
             where tm.team_id = r.team_id and tm.user_id = $2
           )",
    )
    .bind(f.id)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(Redirect::to("/agentic-harness"))
}

/// Tiny JSON the stepper polls. The server resolves "my team's latest run" from the
/// session — no run id in the URL, so nothing cross-team is addressable.
#[derive(Deserialize)]
pub struct HarnessStatusQ {
    kind: Option<String>,
}

pub async fn harness_status(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessStatusQ>,
) -> Result<Json<serde_json::Value>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let kind = match q.kind.as_deref() {
        None => None,
        Some(value) => Some(
            value
                .parse::<BenchmarkKind>()
                .ok()
                .ok_or_else(|| StatusCode::BAD_REQUEST.into_response())?,
        ),
    };
    let row: Option<(
        Uuid,
        String,
        Option<String>,
        Value,
        Option<DateTime<Utc>>,
        String,
        Option<String>,
        String,
        String,
    )> = sqlx::query_as(
        "select r.id, r.stage, r.commit_sha, r.benchmark_state, r.deadline_at,
                r.benchmark_version, r.bedrock_profile, r.provider, r.benchmark_kind
         from harness_runs_exposure_academy r
         join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
         where tm.user_id = $1
           and ($2::text is null or r.benchmark_kind in ($2, 'bundled'))
         order by r.created_at desc limit 1",
    )
    .bind(user.id)
    .bind(kind.map(BenchmarkKind::as_str))
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(match row {
        Some((run, stage, sha, benchmarks, deadline, version, profile, provider, kind)) => {
            serde_json::json!({
                "run": run,
                "stage": stage,
                "commit_sha": sha,
                "benchmarks": benchmarks,
                "deadline_at": deadline,
                "benchmark_version": version,
                "bedrock_profile": profile,
                "provider": provider,
                "benchmark_kind": kind,
            })
        }
        None => serde_json::json!({"run": null, "stage": null}),
    }))
}

#[derive(Deserialize)]
pub struct ArcLiveQ {
    run: Option<Uuid>,
    game: Option<String>,
    seq: Option<i32>,
}

/// One board per game, plus the frames the focused game has produced since the seq the
/// viewer last saw. Same rule as `harness_status`: the run is always reached through the
/// caller's team membership, so a run id in the URL is not cross-team addressable — it
/// either belongs to the caller's team or resolves to nothing.
pub async fn harness_arc_live(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<ArcLiveQ>,
) -> Result<Json<Value>, Response> {
    let user = require(current_user(&app, &headers).await)?;
    let run: Option<(Uuid, String, Option<DateTime<Utc>>, String)> = sqlx::query_as(
        "select r.id, r.stage, r.deadline_at, r.benchmark_version
         from harness_runs_exposure_academy r
         join harness_team_members_exposure_academy tm on tm.team_id = r.team_id
         where tm.user_id = $1 and ($2::uuid is null or r.id = $2)
           and ($2::uuid is not null or r.benchmark_kind in ('arc', 'bundled'))
         order by r.created_at desc limit 1",
    )
    .bind(user.id)
    .bind(q.run)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let Some((run_id, stage, deadline_at, benchmark_version)) = run else {
        return Ok(Json(serde_json::json!({
            "run": null, "stage": null, "deadline_at": null, "games": [], "focus": null,
        })));
    };
    let boards: Vec<ArcBoardRow> = sqlx::query_as(
        // right(grids, 4096) is the last grid: every grid is exactly 4096 chars and they are
        // newline-joined, so the tail is the resulting board. Selecting the whole animation
        // buffer here and discarding it in Rust cost up to 64KB per game per poll.
        "select distinct on (game) game, seq, state, levels_completed, baseline,
                right(grids, 4096) as grids
         from harness_arc_frames_exposure_academy
         where run_id = $1 order by game, seq desc",
    )
    .bind(run_id)
    .fetch_all(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    // seq 0 is a real frame (the initial observation), so "seen nothing yet" is -1.
    let since = q.seq.unwrap_or(-1);
    let focus = match q.game.as_deref().filter(|g| g.len() == 4) {
        None => Value::Null,
        Some(game) => {
            let mut frames: Vec<ArcFocusRow> = sqlx::query_as(
                "select seq, grids, action, action_x, action_y, state, levels_completed
                 from harness_arc_frames_exposure_academy
                 where run_id = $1 and game = $2 and seq > $3 order by seq limit 201",
            )
            .bind(run_id)
            .bind(game)
            .bind(since)
            .fetch_all(&app.pool)
            .await
            .map_err(worker_db_unavailable)?;
            let has_more = frames.len() > 200;
            frames.truncate(200);
            let next_seq = frames.last().map(|frame| frame.seq).unwrap_or(since);
            serde_json::json!({
                "game": game,
                "next_seq": next_seq,
                "has_more": has_more,
                "frames": frames.iter().map(|f| serde_json::json!({
                    "seq": f.seq,
                    // the whole animation buffer: the focused board plays it back
                    "grids": f.grids.split('\n').collect::<Vec<_>>(),
                    "action": f.action, "action_x": f.action_x, "action_y": f.action_y,
                    "state": f.state, "levels_completed": f.levels_completed,
                })).collect::<Vec<_>>(),
            })
        }
    };
    let games: Vec<Value> = if benchmark_version == HARNESS_VERSION {
        ARC_GAMES
            .iter()
            .map(
                |game| match boards.iter().find(|board| board.game == *game) {
                    Some(board) => serde_json::json!({
                        "game": board.game, "seq": board.seq, "total": board.seq,
                        "state": board.state, "levels_completed": board.levels_completed,
                        "baseline": board.baseline,
                        "grid": board.grids.rsplit('\n').next().unwrap_or(""),
                    }),
                    None => serde_json::json!({
                        "game": game, "seq": -1, "total": 0, "state": "NOT_PLAYED",
                        "levels_completed": 0, "baseline": null, "grid": "",
                    }),
                },
            )
            .collect()
    } else {
        boards
            .iter()
            .map(|board| {
                serde_json::json!({
                    "game": board.game, "seq": board.seq, "total": board.seq,
                    "state": board.state, "levels_completed": board.levels_completed,
                    "baseline": board.baseline,
                    "grid": board.grids.rsplit('\n').next().unwrap_or(""),
                })
            })
            .collect()
    };
    Ok(Json(serde_json::json!({
        "run": run_id,
        "stage": stage,
        "deadline_at": deadline_at,
        "games": games,
        "focus": focus,
    })))
}

#[derive(sqlx::FromRow)]
struct ArcBoardRow {
    game: String,
    seq: i32,
    state: String,
    levels_completed: i32,
    baseline: Option<Vec<i32>>,
    grids: String,
}

#[derive(sqlx::FromRow)]
struct ArcFocusRow {
    seq: i32,
    grids: String,
    action: Option<String>,
    action_x: Option<i32>,
    action_y: Option<i32>,
    state: String,
    levels_completed: i32,
}

fn encrypt_kaggle_token(
    app: &App,
    team_id: Uuid,
    token: &str,
) -> Result<(Vec<u8>, Vec<u8>), Response> {
    let key = app
        .kaggle_key
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: token.as_bytes(),
                aad: team_id.as_bytes(),
            },
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok((nonce.to_vec(), ciphertext))
}

fn decrypt_kaggle_token(
    app: &App,
    team_id: Uuid,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<String, Response> {
    let key = app
        .kaggle_key
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    if nonce.len() != 24 {
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: team_id.as_bytes(),
            },
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    String::from_utf8(plaintext).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[derive(Deserialize)]
pub struct HarnessKaggleCredentialForm {
    username: String,
    token: String,
}

pub async fn harness_kaggle_credentials(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessKaggleCredentialForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let username = f.username.trim();
    let token = f.token.trim();
    if !(2..=64).contains(&username.len())
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        || !(20..=500).contains(&token.len())
        || token.chars().any(char::is_whitespace)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Geçersiz Kaggle kullanıcı adı veya token.",
        )
            .into_response());
    }
    let (nonce, ciphertext) = encrypt_kaggle_token(&app, team.id, token)?;
    sqlx::query(
        "insert into harness_kaggle_credentials_exposure_academy
           (team_id, username, token_nonce, token_ciphertext, updated_by, updated_at)
         values ($1,$2,$3,$4,$5,now())
         on conflict (team_id) do update set username = excluded.username,
           token_nonce = excluded.token_nonce, token_ciphertext = excluded.token_ciphertext,
           updated_by = excluded.updated_by, updated_at = now()",
    )
    .bind(team.id)
    .bind(username)
    .bind(nonce)
    .bind(ciphertext)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

pub async fn harness_kaggle_credentials_delete(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let active: bool = sqlx::query_scalar(
        "select exists(
           select 1 from harness_kaggle_submissions_exposure_academy j
           join harness_runs_exposure_academy r on r.id = j.run_id
           where r.team_id = $1 and j.status in ('queued','kernel_running','submitted'))",
    )
    .bind(team.id)
    .fetch_one(&app.pool)
    .await
    .unwrap_or(true);
    if active {
        return Err((
            StatusCode::CONFLICT,
            "Devam eden resmi gönderim varken Kaggle tokenı silinemez.",
        )
            .into_response());
    }
    sqlx::query("delete from harness_kaggle_credentials_exposure_academy where team_id = $1")
        .bind(team.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

#[derive(Deserialize)]
pub struct HarnessKaggleSubmitForm {
    run_id: Uuid,
}

pub async fn harness_kaggle_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<HarnessKaggleSubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let Some(team) = harness_team_of(&app, user.id).await else {
        return Err(StatusCode::FORBIDDEN.into_response());
    };
    let eligible: bool = sqlx::query_scalar(
        "select exists(
           select 1 from harness_runs_exposure_academy r
           join harness_kaggle_credentials_exposure_academy c on c.team_id = r.team_id
           where r.id = $1 and r.team_id = $2 and r.commit_sha is not null
             and r.score_arc is not null and r.benchmark_version = $3
             and r.repo_url like 'https://github.com/%'
         )",
    )
    .bind(f.run_id)
    .bind(team.id)
    .bind(HARNESS_VERSION)
    .fetch_one(&app.pool)
    .await
    .unwrap_or(false);
    if !eligible {
        return Err((
            StatusCode::BAD_REQUEST,
            "Bu çalıştırma resmi gönderime hazır değil.",
        )
            .into_response());
    }
    sqlx::query(
        "insert into harness_kaggle_submissions_exposure_academy
           (run_id, requested_by) values ($1,$2)
         on conflict (run_id) do update set
           requested_by = excluded.requested_by, status = 'queued',
           kernel_slug = null, kernel_version = null, submission_ref = null,
           public_score = null, private_score = null, status_message = null,
           lease_token = null, lease_expires_at = null, next_poll_at = null,
           last_result_lease_token = null, claim_attempts = 0, updated_at = now()
         where harness_kaggle_submissions_exposure_academy.status = 'failed'",
    )
    .bind(f.run_id)
    .bind(user.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(Redirect::to("/agentic-harness?tab=history"))
}

// Team create/rename/delete and member assign/remove moved to `teams.rs`
// (/admin/takimlar — the Takım formasyonu board). What stays here is run management.

/// The stuck-run escape hatch: a worker that died after claiming leaves the run
/// non-terminal, which blocks the team's resubmits (one-active-run index). Failing it
/// here unblocks them; a late worker report against it then gets a 409 and is dropped.
pub async fn admin_harness_run_fail(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'failed', error_log = 'Yönetici tarafından durduruldu.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where id = $1 and stage not in ('done','partial','failed','infra_failed','cancelled')",
    )
    .bind(f.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

fn worker_db_unavailable(error: sqlx::Error) -> Response {
    eprintln!("worker database operation failed: {error}");
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

/// One slot equals one controller/executor node. Include immediately claimable expired
/// leases and due Kaggle polls so the fleet follows actual claim demand, while excluding
/// jobs that are merely waiting for Kaggle's next poll time.
pub async fn worker_harness_capacity(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<HarnessCapacity>, Response> {
    check_worker(&app, &headers)?;
    let (queued, active, oldest_queued_seconds): (i64, i64, i64) = sqlx::query_as(
        "with claimable(created_at) as (
           select created_at from harness_runs_exposure_academy
           where benchmark_version = $1 and (
             stage = 'queued'
             or (stage in ('preparing','running') and lease_expires_at < now()
                 and deadline_at > now() and claim_attempts < 3)
           )
           union all
           select created_at from harness_kaggle_submissions_exposure_academy
           where status = 'queued'
              or (status = 'kernel_running' and lease_expires_at < now()
                  and claim_attempts < 3)
              or (status = 'submitted' and coalesce(next_poll_at, now()) <= now()
                  and (lease_expires_at is null or lease_expires_at < now()))
         ), active_slots as (
           select id from harness_runs_exposure_academy
           where benchmark_version = $1 and stage in ('preparing','running')
             and lease_expires_at >= now() and deadline_at > now()
           union all
           select id from harness_kaggle_submissions_exposure_academy
           where status in ('kernel_running','submitted') and lease_expires_at >= now()
         )
         select (select count(*) from claimable)::bigint,
                (select count(*) from active_slots)::bigint,
                greatest(0, coalesce(extract(epoch from now() -
                    (select min(created_at) from claimable))::bigint, 0))",
    )
    .bind(HARNESS_VERSION)
    .fetch_one(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(HarnessCapacity {
        queued: queued.max(0) as u64,
        active: active.max(0) as u64,
        oldest_queued_seconds: oldest_queued_seconds.max(0) as u64,
    }))
}

#[derive(Deserialize)]
pub struct HarnessClaimQ {
    kind: Option<String>,
}

pub async fn worker_harness_claim(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<HarnessClaimQ>,
) -> Result<Json<Option<HarnessClaim>>, Response> {
    check_worker(&app, &headers)?;
    let benchmark_kind = q
        .kind
        .as_deref()
        .unwrap_or("bundled")
        .parse::<BenchmarkKind>()
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'infra_failed', error_log = 'Worker missed the deadline result grace period.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where stage in ('preparing','running')
           and deadline_at < now() - interval '30 seconds'",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'infra_failed', error_log = 'Worker lease expired three times.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where stage in ('preparing','running') and lease_expires_at < now()
           and deadline_at > now() and claim_attempts >= 3",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;

    let lease = Uuid::new_v4();
    let row: Option<(Uuid, String, DateTime<Utc>, String, String, String)> = sqlx::query_as(
        "with candidate as (
           select id from harness_runs_exposure_academy
           where benchmark_version = $3 and benchmark_kind = $4 and (
             stage = 'queued'
             or (stage in ('preparing','running') and lease_expires_at < now()
                 and deadline_at > now() and claim_attempts < 3)
           )
           order by created_at
           for update skip locked
           limit 1
         )
         update harness_runs_exposure_academy r
         set stage = 'preparing', lease_token = $1,
             lease_expires_at = least(coalesce(r.deadline_at, now() + $2::bigint * interval '1 second')
                                      + interval '30 seconds',
                                      now() + interval '90 seconds'),
             worker_heartbeat_at = now(),
             deadline_at = coalesce(r.deadline_at, now() + $2::bigint * interval '1 second'),
             claim_attempts = claim_attempts + 1, updated_at = now()
         from candidate where r.id = candidate.id
         returning r.id, r.repo_url, r.deadline_at, r.model_id,
                   r.provider, r.benchmark_kind",
    )
    .bind(lease)
    .bind(RUN_DEADLINE_SECONDS)
    .bind(HARNESS_VERSION)
    .bind(benchmark_kind.as_str())
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    Ok(Json(row.map(
        |(id, repo_url, deadline_at, model_id, provider, benchmark_kind)| {
            HarnessClaim {
                id,
                repo_url,
                provider: provider.parse().expect("database provider constraint"),
                benchmark_kind: benchmark_kind
                    .parse()
                    .expect("database benchmark kind constraint"),
                model_id,
                lease_token: lease,
                deadline_at,
                benchmark_version: HARNESS_VERSION.into(),
            }
        },
    )))
}

pub async fn worker_harness_heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessLeaseReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set lease_expires_at = least(deadline_at + interval '30 seconds',
                                      now() + interval '90 seconds'),
             worker_heartbeat_at = now(), updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage in ('preparing','running')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_harness_stage(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessStageReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.status != "running"
        || r.bedrock_profile.trim().is_empty()
        || r.bedrock_profile.len() > 120
        || r.bedrock_profile_fingerprint.len() != 64
        || !r
            .bedrock_profile_fingerprint
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let sha = r.commit_sha.trim();
    if !(7..=40).contains(&sha.len()) || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set stage = 'running', commit_sha = $3, bedrock_profile = $4,
             bedrock_profile_fingerprint = lower($5), updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage = 'preparing'",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(sha.to_lowercase())
    .bind(r.bedrock_profile.trim())
    .bind(&r.bedrock_profile_fingerprint)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Repeatable mid-stage progress report — no stage transition, just the live blob
/// the student's stepper renders ("task 12/70 · score 5.7"). Capped and only
/// accepted while the run is still in flight; a stale report gets a 409.
pub async fn worker_harness_progress(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessProgressReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let encoded = r.state.to_string();
    if !matches!(r.benchmark.as_str(), "arc" | "frontier" | "ram")
        || !r.state.is_object()
        || encoded.len() > 8000
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set benchmark_state = jsonb_set(benchmark_state, array[$3], $4::jsonb, true),
             updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() and stage = 'running'",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.benchmark)
    .bind(encoded)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Worker-supplied and headed for both SQL and the student's screen, so the shape is
/// checked instead of trusted. One grid is 4096 hex cells (the engine's camera is always
/// 64x64) and an animation buffer is at most 16 of them, newline separated.
/// Append-only frames for the live board viewer. Frames are cosmetic and best effort, but
/// they still carry the active lease: a zombie controller must not append to a reclaimed run.
pub async fn worker_harness_arc_frames(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessArcFramesReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.frames.len() > 64 || !r.frames.iter().all(HarnessArcFrame::is_valid) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let encoded = serde_json::to_string(&r.frames).unwrap();
    // One round trip, and deliberately not unnest: a batch carries a per-row int[]
    // (baseline) and Postgres flattens multidimensional arrays, which would smear every
    // row's baseline into one. jsonb_to_recordset keeps each row's array its own.
    let res = sqlx::query(
        "insert into harness_arc_frames_exposure_academy
           (run_id, game, seq, grids, state, levels_completed, baseline,
            action, action_x, action_y)
         select $1, f.game, f.seq, f.grids, f.state, f.levels_completed, f.baseline,
                f.action, f.action_x, f.action_y
         from jsonb_to_recordset($2::jsonb) as f(
           game text, seq int, grids text, state text, levels_completed int,
           baseline int[], action text, action_x int, action_y int)
         where exists (select 1 from harness_runs_exposure_academy r
                       where r.id = $1 and r.lease_token = $3
                         and r.lease_expires_at > now() and r.deadline_at > now()
                         and r.stage = 'running')
         on conflict do nothing",
    )
    .bind(r.run_id)
    .bind(encoded)
    .bind(r.lease_token)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    // `on conflict do nothing` also reports zero rows for a duplicate re-post, which is
    // a success — only an unknown or already-finished run earns the 409 that tells a
    // zombie controller to stop. If that check itself fails, say "keep going": frames
    // are not retried by the controller, but a database outage is still distinguished from
    // a stale lease so operators can see it in metrics instead of misdiagnosing a reclaim.
    if res.rows_affected() == 0 {
        let live: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_runs_exposure_academy where id = $1
                           and lease_token = $2 and lease_expires_at > now()
                           and deadline_at > now() and stage = 'running')",
        )
        .bind(r.run_id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if !live {
            return Err(StatusCode::CONFLICT.into_response());
        }
    }
    Ok(StatusCode::OK)
}

pub async fn worker_harness_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(
        r.status.as_str(),
        "done" | "partial" | "failed" | "infra_failed"
    ) || !r.benchmark_state.is_object()
        || r.benchmark_state.to_string().len() > 30000
        || r.error_log.as_ref().is_some_and(|s| s.len() > 8000)
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let score_ok = |v: Option<f32>, lower: f32, upper: f32| {
        v.is_none_or(|n| n.is_finite() && (lower..=upper).contains(&n))
    };
    if !score_ok(r.score_arc, 0.0, 100.0)
        || !score_ok(r.score_frontier, 0.0, 100.0)
        || !score_ok(r.ram_1session_mb, 0.01, 1_000_000.0)
        || !score_ok(r.ram_10session_mb, 0.01, 1_000_000.0)
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if r.status == "done"
        && (r.score_arc.is_none()
            || r.score_frontier.is_none()
            || r.ram_1session_mb.is_none()
            || r.ram_10session_mb.is_none())
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if r.status == "partial"
        && r.score_arc.is_none()
        && r.score_frontier.is_none()
        && r.ram_1session_mb.is_none()
        && r.ram_10session_mb.is_none()
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let encoded = r.benchmark_state.to_string();
    let res = sqlx::query(
        "update harness_runs_exposure_academy
         set stage = $3, benchmark_state = $4::jsonb, score_arc = $5,
             score_frontier = $6, ram_1session_mb = $7, ram_10session_mb = $8,
             error_log = $9, progress = null, lease_token = null,
             lease_expires_at = null, result_lease_token = $2, updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and deadline_at > now() - interval '30 seconds'
           and stage in ('preparing','running')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.status)
    .bind(encoded)
    .bind(r.score_arc)
    .bind(r.score_frontier)
    .bind(r.ram_1session_mb)
    .bind(r.ram_10session_mb)
    .bind(&r.error_log)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_runs_exposure_academy
                           where id = $1 and result_lease_token = $2
                             and stage in ('done','partial','failed','infra_failed'))",
        )
        .bind(r.id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if duplicate {
            return Ok(StatusCode::NO_CONTENT);
        }
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn worker_harness_kaggle_claim(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<Option<KaggleClaim>>, Response> {
    check_worker(&app, &headers)?;
    if app.kaggle_key.is_none() {
        return Ok(Json(None));
    }
    sqlx::query(
        "update harness_kaggle_submissions_exposure_academy
         set status = 'failed', status_message = 'Worker lease expired three times.',
             lease_token = null, lease_expires_at = null, updated_at = now()
         where status = 'kernel_running' and lease_expires_at < now() and claim_attempts >= 3",
    )
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let lease = Uuid::new_v4();
    let row: Option<HarnessKaggleClaimRow> = sqlx::query_as(
        "with candidate as (
           select j.id from harness_kaggle_submissions_exposure_academy j
           where j.status = 'queued'
              or (j.status = 'kernel_running' and j.lease_expires_at < now()
                  and j.claim_attempts < 3)
              or (j.status = 'submitted' and coalesce(j.next_poll_at, now()) <= now()
                  and (j.lease_expires_at is null or j.lease_expires_at < now()))
           order by j.created_at for update of j skip locked limit 1
         ), updated as (
           update harness_kaggle_submissions_exposure_academy j
           set status = case when j.status = 'submitted' then 'submitted'
                             else 'kernel_running' end,
               lease_token = $1,
               lease_expires_at = now() + interval '5 minutes',
               claim_attempts = case when j.status = 'submitted' then j.claim_attempts
                                     else j.claim_attempts + 1 end,
               updated_at = now()
           from candidate where j.id = candidate.id
           returning j.id, j.run_id, j.status, j.kernel_slug,
                     j.kernel_version, j.submission_ref
         )
         select u.id, r.id as run_id, r.team_id, r.repo_url, r.commit_sha,
                c.username, r.benchmark_version, c.token_nonce, c.token_ciphertext,
                u.status, u.kernel_slug, u.kernel_version, u.submission_ref
         from updated u join harness_runs_exposure_academy r on r.id = u.run_id
         join harness_kaggle_credentials_exposure_academy c on c.team_id = r.team_id",
    )
    .bind(lease)
    .fetch_optional(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    let Some(row) = row else {
        return Ok(Json(None));
    };
    let token = decrypt_kaggle_token(&app, row.team_id, &row.token_nonce, &row.token_ciphertext)?;
    Ok(Json(Some(KaggleClaim {
        id: row.id,
        run_id: row.run_id,
        phase: if row.status == "submitted" {
            "poll"
        } else {
            "submit"
        }
        .into(),
        repo_url: row.repo_url,
        commit_sha: row.commit_sha,
        username: row.username,
        token,
        lease_token: lease,
        benchmark_version: row.benchmark_version,
        competition: "arc-prize-2026-arc-agi-3".into(),
        kernel_slug: row.kernel_slug,
        kernel_version: row.kernel_version,
        submission_ref: row.submission_ref,
    })))
}

#[derive(sqlx::FromRow)]
struct HarnessKaggleClaimRow {
    id: Uuid,
    run_id: Uuid,
    team_id: Uuid,
    repo_url: String,
    commit_sha: String,
    username: String,
    benchmark_version: String,
    token_nonce: Vec<u8>,
    token_ciphertext: Vec<u8>,
    status: String,
    kernel_slug: Option<String>,
    kernel_version: Option<i32>,
    submission_ref: Option<String>,
}

pub async fn worker_harness_kaggle_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<HarnessKaggleResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if !matches!(r.status.as_str(), "submitted" | "scored" | "failed")
        || r.status_message.as_ref().is_some_and(|s| s.len() > 2000)
        || r.kernel_slug.as_ref().is_some_and(|s| s.len() > 200)
        || r.submission_ref.as_ref().is_some_and(|s| s.len() > 300)
        || [r.public_score, r.private_score]
            .into_iter()
            .flatten()
            .any(|score| !score.is_finite() || !(0.0..=100.0).contains(&score))
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    if matches!(r.status.as_str(), "submitted" | "scored")
        && (r.kernel_slug.as_deref().is_none_or(str::is_empty)
            || r.kernel_version.is_none_or(|version| version < 1)
            || r.submission_ref.as_deref().is_none_or(str::is_empty))
    {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update harness_kaggle_submissions_exposure_academy
         set status = $3, kernel_slug = coalesce($4, kernel_slug),
             kernel_version = coalesce($5, kernel_version),
             submission_ref = coalesce($6, submission_ref),
             public_score = $7, private_score = $8,
             status_message = $9, lease_token = null, lease_expires_at = null,
             last_result_lease_token = $2,
             next_poll_at = case when $3 = 'submitted'
                                 then now() + interval '30 seconds' else null end,
             updated_at = now()
         where id = $1 and lease_token = $2 and lease_expires_at > now()
           and status in ('kernel_running','submitted')",
    )
    .bind(r.id)
    .bind(r.lease_token)
    .bind(&r.status)
    .bind(&r.kernel_slug)
    .bind(r.kernel_version)
    .bind(&r.submission_ref)
    .bind(r.public_score)
    .bind(r.private_score)
    .bind(&r.status_message)
    .execute(&app.pool)
    .await
    .map_err(worker_db_unavailable)?;
    if res.rows_affected() == 0 {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from harness_kaggle_submissions_exposure_academy
                           where id = $1 and last_result_lease_token = $2)",
        )
        .bind(r.id)
        .bind(r.lease_token)
        .fetch_one(&app.pool)
        .await
        .map_err(worker_db_unavailable)?;
        if duplicate {
            return Ok(StatusCode::NO_CONTENT);
        }
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitted_model_is_scoped_to_its_provider() {
        assert_eq!(
            submitted_model_id(ModelProvider::Cerebras, "", false),
            Some(DEFAULT_CEREBRAS_MODEL)
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Bedrock, "", false),
            Some(DEFAULT_BEDROCK_MODEL)
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Bedrock, "  xai.grok-4.3  ", false),
            Some(DEFAULT_BEDROCK_MODEL)
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Cerebras, "xai.grok-4.3", false),
            None
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Bedrock, "google.gemma-4-31b", true),
            Some("google.gemma-4-31b")
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Cerebras, "zai-glm-4.7", true),
            None
        );
        assert_eq!(
            submitted_model_id(ModelProvider::DeepInfra, "", false),
            Some(DEFAULT_DEEPINFRA_MODEL)
        );
        assert_eq!(
            submitted_model_id(ModelProvider::DeepInfra, "Qwen/Qwen3.6-27B", true),
            Some("Qwen/Qwen3.6-27B")
        );
        assert_eq!(
            submitted_model_id(ModelProvider::DeepInfra, "xai.grok-4.3", false),
            None
        );
        assert_eq!(
            submitted_model_id(ModelProvider::Bedrock, "Qwen/Qwen3.6-27B", false),
            None
        );
    }

    #[test]
    fn bedrock_provider_is_admin_only() {
        assert_eq!(submitted_provider(false, ""), Some(ModelProvider::Cerebras));
        assert_eq!(
            submitted_provider(false, "cerebras"),
            Some(ModelProvider::Cerebras)
        );
        assert_eq!(submitted_provider(false, "bedrock"), None);
        assert_eq!(
            submitted_provider(true, "bedrock"),
            Some(ModelProvider::Bedrock)
        );
        assert_eq!(submitted_provider(true, "unknown"), None);
        assert_eq!(submitted_provider(false, "unknown"), None);
        // DeepInfra is on the student dropdown, so it passes without admin.
        assert_eq!(
            submitted_provider(false, "deepinfra"),
            Some(ModelProvider::DeepInfra)
        );
        assert_eq!(
            submitted_provider(true, "deepinfra"),
            Some(ModelProvider::DeepInfra)
        );
    }

    #[test]
    fn builtin_harnesses_are_admin_only_and_require_a_blank_repo() {
        assert_eq!(
            submission_source(true, "", "forge").as_deref(),
            Ok("builtin://forge")
        );
        assert_eq!(
            submission_source(true, "https://github.com/example/agent", "").as_deref(),
            Ok("https://github.com/example/agent")
        );
        // The three the old `Option` return couldn't tell apart.
        assert_eq!(
            submission_source(false, "", "forge"),
            Err(SourceError::BuiltinForbidden)
        );
        assert_eq!(
            submission_source(true, "https://github.com/example/agent", "forge"),
            Err(SourceError::Both)
        );
        assert_eq!(
            submission_source(true, "", "unknown"),
            Err(SourceError::BuiltinUnknown)
        );
        assert_eq!(submission_source(false, "", ""), Err(SourceError::Neither));
    }

    /// Everything a student plausibly pastes has to land on the same repo. Each of these was
    /// a 400 before, and the message it got said the link didn't start with
    /// `https://github.com/` — which almost all of them do.
    #[test]
    fn github_urls_students_actually_paste_are_normalized() {
        for raw in [
            "https://github.com/ali/proje",
            "  https://github.com/ali/proje  ",
            "https://github.com/ali/proje/",
            "https://github.com/ali/proje.git",
            // the address-bar copy, while browsing a folder
            "https://github.com/ali/proje/tree/main",
            "https://github.com/ali/proje/tree/main/src/agent",
            "https://github.com/ali/proje/blob/main/agent.py",
            "https://github.com/ali/proje/blob/main/agent.py#L12-L40",
            "https://github.com/ali/proje/pull/3",
            "https://github.com/ali/proje/issues",
            "https://github.com/ali/proje/actions/runs/12345",
            // what GitHub's own copy button produces
            "https://github.com/ali/proje?tab=readme-ov-file",
            "https://github.com/ali/proje#readme",
            "http://github.com/ali/proje",
            "https://www.github.com/ali/proje",
            "github.com/ali/proje",
            "www.github.com/ali/proje",
            "git@github.com:ali/proje.git",
            "ssh://git@github.com/ali/proje.git",
        ] {
            assert_eq!(
                github_repo_url(raw).as_deref(),
                Ok("https://github.com/ali/proje"),
                "{raw}"
            );
        }
        // Case and the three legal punctuation marks survive untouched.
        assert_eq!(
            github_repo_url("https://github.com/Ali-Veli_1/pro.je-v2/tree/main").as_deref(),
            Ok("https://github.com/Ali-Veli_1/pro.je-v2")
        );
    }

    #[test]
    fn bad_github_urls_say_exactly_what_is_wrong() {
        use RepoUrlError::*;
        for (raw, want) in [
            ("", Empty),
            ("   ", Empty),
            ("https://github.com/ali", OwnerOnly),
            ("https://github.com/ali/", OwnerOnly),
            ("https://github.com/", NoRepo),
            ("https://github.com", NoRepo),
            ("https://gitlab.com/ali/proje", NotGithub),
            ("https://huggingface.co/ali/model", NotGithub),
            // both of these end in something that *looks* like github.com
            ("https://github.com.evil.com/ali/proje", NotGithub),
            ("https://notgithub.com/ali/proje", NotGithub),
            ("https://gist.github.com/ali/abc123", GistLink),
            (
                "https://raw.githubusercontent.com/ali/proje/main/a.py",
                RawFileLink,
            ),
            ("https://user:pw@github.com/ali/proje", Credentials),
            ("https://github.com:8080/ali/proje", Credentials),
            // the suspected cause of the report this change came from
            ("https://github.com/ali/ödev-çalışması", NonAscii),
            ("https://github.com/öğrenci/proje", NonAscii),
            // ASCII but illegal — must NOT be reported as a Turkish-character problem
            ("https://github.com/ali/pro je", BadChars),
            ("https://github.com/orgs/exposure/repositories", ReservedOwner),
            ("https://github.com/settings/profile", ReservedOwner),
            ("https://github.com/apps/copilot", ReservedOwner),
            ("ali/proje", NotAUrl),
            ("repo linkim bu galiba", NotAUrl),
        ] {
            assert_eq!(github_repo_url(raw), Err(want), "{raw}");
        }
        assert_eq!(
            github_repo_url(&"https://github.com/ali/".repeat(200)),
            Err(TooLong)
        );
        assert_eq!(
            github_repo_url(&format!("https://github.com/{}/x", "a".repeat(60))),
            Err(SegmentTooLong)
        );
    }

    /// The output shape is a contract with the worker, which re-validates it strictly in
    /// `benchmark-node/src/bin/executor.rs::valid_repo_url` and
    /// `benchmark-node/adapters/runner.py::valid_repo_url`. Asserted here independently of the
    /// parser, so loosening the input side can never quietly loosen what we hand downstream.
    #[test]
    fn normalized_urls_satisfy_the_worker_contract() {
        for raw in [
            "https://github.com/ali/proje/blob/main/agent.py#L12-L40",
            "https://github.com/ali/proje?tab=readme-ov-file",
            "git@github.com:ali/proje.git",
            "ssh://git@github.com/ali/proje.git",
            "www.github.com/Ali-Veli_1/pro.je-v2",
            "http://github.com/ali/proje/actions/runs/12345",
        ] {
            let url = github_repo_url(raw).expect(raw);
            let rest = url
                .strip_prefix("https://github.com/")
                .unwrap_or_else(|| panic!("{raw} -> {url}"));
            let parts: Vec<&str> = rest.split('/').collect();
            assert_eq!(parts.len(), 2, "{raw} -> {url}");
            for part in &parts {
                assert!(!part.is_empty(), "{raw} -> {url}");
                assert!(*part != "." && *part != "..", "{raw} -> {url}");
                assert!(
                    part.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
                    "{raw} -> {url}"
                );
            }
            assert!(parts[0].len() <= 39 && parts[1].len() <= 100, "{raw}");
        }
    }

    /// Every reason has to carry its own advice — collapsing two of them back into one shared
    /// sentence is precisely the bug this change fixes.
    #[test]
    fn every_rejection_reason_has_its_own_turkish_message() {
        use RepoUrlError::*;
        const ALL: [RepoUrlError; 13] = [
            Empty,
            TooLong,
            NotAUrl,
            NotGithub,
            GistLink,
            RawFileLink,
            Credentials,
            NoRepo,
            OwnerOnly,
            ReservedOwner,
            NonAscii,
            BadChars,
            SegmentTooLong,
        ];
        let mut seen: Vec<&str> = Vec::new();
        for err in ALL {
            let msg = repo_error_tr(err);
            assert!(!msg.is_empty(), "{err:?}");
            // the retired one-size-fits-all sentence
            assert_ne!(msg, "Repo bağlantısı https://github.com/ ile başlamalı.");
            assert!(!seen.contains(&msg), "{err:?} reuses a message");
            seen.push(msg);
        }
        // The admin/student split at the call site survives.
        assert_eq!(
            source_error_tr(true, SourceError::Neither),
            "GitHub bağlantısı gir veya bir hazır harness seç."
        );
        assert_eq!(
            source_error_tr(false, SourceError::Neither),
            repo_error_tr(Empty)
        );
        assert_eq!(
            source_error_tr(false, SourceError::Repo(NonAscii)),
            repo_error_tr(NonAscii)
        );
    }

    #[test]
    fn every_rejection_reason_has_a_stable_slug() {
        use RepoUrlError::*;
        const ALL: [SourceError; 17] = [
            SourceError::Repo(Empty),
            SourceError::Repo(TooLong),
            SourceError::Repo(NotAUrl),
            SourceError::Repo(NotGithub),
            SourceError::Repo(GistLink),
            SourceError::Repo(RawFileLink),
            SourceError::Repo(Credentials),
            SourceError::Repo(NoRepo),
            SourceError::Repo(OwnerOnly),
            SourceError::Repo(ReservedOwner),
            SourceError::Repo(NonAscii),
            SourceError::Repo(BadChars),
            SourceError::Repo(SegmentTooLong),
            SourceError::Both,
            SourceError::Neither,
            SourceError::BuiltinForbidden,
            SourceError::BuiltinUnknown,
        ];
        let mut seen: Vec<&str> = Vec::new();
        for err in ALL {
            let slug = source_error_slug(err);
            assert!(!slug.is_empty(), "{err:?}");
            assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{slug} is not a greppable slug"
            );
            assert!(!seen.contains(&slug), "{err:?} reuses slug {slug}");
            seen.push(slug);
        }
    }
}
