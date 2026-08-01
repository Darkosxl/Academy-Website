//! AI Monopoly: submission and HF validation, the arena and history pages, the
//! interim team admin, and the worker API that owns the ledger.

use crate::admin::{IdForm, MemberForm, NameForm};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

pub async fn monopoly_team_of(app: &App, uid: Uuid) -> Option<MonopolyTeam> {
    sqlx::query_as(
        "select t.id, t.name from monopoly_team_members_exposure_academy tm
         join monopoly_teams_exposure_academy t on t.id = tm.team_id
         where tm.user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

pub const GIB: i64 = 1024 * 1024 * 1024;

/// Accepts what a student is likely to paste: the bare `org/model`, the full model URL,
/// or a URL copied out of the file browser with a `/tree/<rev>` on the end. Returns the
/// canonical id, or None. This also has to be strict about the characters it lets
/// through, because the result is interpolated straight into an API URL.
pub fn normalize_hf_repo(raw: &str) -> Option<String> {
    let s = raw.trim().trim_end_matches('/');
    let s = s
        .strip_prefix("https://huggingface.co/")
        .or_else(|| s.strip_prefix("http://huggingface.co/"))
        .or_else(|| s.strip_prefix("huggingface.co/"))
        .unwrap_or(s);
    let s = s.split("/tree/").next().unwrap_or(s);
    let mut parts = s.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 96
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    (ok(owner) && ok(name)).then(|| format!("{owner}/{name}"))
}

/// What the tournament needs to pin and serve a submitted model.
pub struct HfModel {
    revision: String,
    size_bytes: i64,
}

/// Check a Hugging Face repo against the rules before storing an entry, so a bad model
/// is a form error the student can fix rather than a dead player discovered mid-run.
/// Everything here is one unauthenticated GET against the public API plus two small
/// config files; no weights are downloaded.
pub async fn hf_validate(http: &reqwest::Client, repo: &str) -> Result<HfModel, String> {
    let unreachable = "Hugging Face'e ulaşılamadı, biraz sonra tekrar dene.";
    let meta: serde_json::Value = http
        .get(format!(
            "https://huggingface.co/api/models/{repo}?blobs=true"
        ))
        .send()
        .await
        .map_err(|_| unreachable.to_string())?
        // HF answers 401 for a private repo AND for one that doesn't exist, on purpose —
        // so one message has to cover both cases without guessing which it was.
        .error_for_status()
        .map_err(|_| {
            "Bu depo bulunamadı veya herkese açık değil. Model kartını public yap.".to_string()
        })?
        .json()
        .await
        .map_err(|_| unreachable.to_string())?;

    let siblings = meta["siblings"].as_array().cloned().unwrap_or_default();
    let fname = |s: &serde_json::Value| s["rfilename"].as_str().unwrap_or("").to_string();
    if siblings.iter().any(|s| fname(s).ends_with(".gguf")) {
        return Err(
            "Depoda GGUF dosyası var. Nicemleme yasak — modeli bf16 safetensors olarak yayınla."
                .into(),
        );
    }
    let size_bytes: i64 = siblings
        .iter()
        .filter(|s| fname(s).ends_with(".safetensors"))
        .map(|s| s["size"].as_i64().unwrap_or(0))
        .sum();
    if size_bytes == 0 {
        return Err("Depoda .safetensors ağırlık dosyası yok.".into());
    }
    if size_bytes > MONOPOLY_SIZE_CAP_BYTES {
        return Err(format!(
            "Model çok büyük: {:.1} GiB. Sınır {} GiB.",
            size_bytes as f64 / GIB as f64,
            MONOPOLY_SIZE_CAP_BYTES / GIB
        ));
    }

    // Pin the commit now: the tournament serves this exact revision, so a repo edited
    // mid-run can't change what a frozen player answers with.
    let revision = meta["sha"].as_str().unwrap_or("main").to_string();
    let fetch_json = |name: &'static str| {
        let url = format!("https://huggingface.co/{repo}/resolve/{revision}/{name}");
        let http = http.clone();
        async move {
            http.get(url)
                .send()
                .await
                .ok()?
                .json::<serde_json::Value>()
                .await
                .ok()
        }
    };
    let Some(config) = fetch_json("config.json").await else {
        return Err("config.json okunamadı — bu bir transformers modeli değil.".into());
    };
    if !config["quantization_config"].is_null() {
        return Err(
            "Model nicemlenmiş (quantization_config). Yalnızca bf16 tam ağırlık kabul ediliyor."
                .into(),
        );
    }
    // Without a chat template there is no defined way to turn a conversation into a
    // prompt, and the failure would otherwise land halfway through the tournament.
    match fetch_json("tokenizer_config.json").await {
        Some(t) if !t["chat_template"].is_null() => {}
        _ => {
            return Err(
                "tokenizer_config.json içinde chat_template yok — sohbet formatı tanımsız.".into(),
            );
        }
    }
    Ok(HfModel {
        revision,
        size_bytes,
    })
}

#[derive(Deserialize)]
pub struct MonopolyQ {
    tab: Option<String>,
}

pub async fn ai_monopoly(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<MonopolyQ>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    // unknown values fall back to the default tab, same as /demos?lang=
    let tab = match q.tab.as_deref() {
        Some("live") => "live",
        Some("history") => "history",
        Some("instructions") => "instructions",
        _ => "main",
    };
    if tab == "instructions" {
        return Ok(Html(html::monopoly_instructions(&user)));
    }
    let team = monopoly_team_of(&app, user.id).await;
    let tournament = latest_tournament(&app).await;
    let standings = match &tournament {
        Some(t) => monopoly_standings(&app, t.id).await,
        None => Vec::new(),
    };
    if tab == "live" {
        return Ok(Html(html::monopoly_live(
            &user,
            tournament.as_ref(),
            &standings,
        )));
    }
    if tab == "history" {
        // Practice matches are excluded: they're the team's own rehearsals against a
        // decoy, not part of the record everyone reads.
        let matches: Vec<MonopolyMatchRow> = match &tournament {
            Some(t) => sqlx::query_as(
                "select m.id, m.round, m.kind, m.status, m.a_player, a.char_name as a_name,
                        m.b_player, b.char_name as b_name, m.created_at
                 from monopoly_matches_exposure_academy m
                 join monopoly_players_exposure_academy a on a.id = m.a_player
                 join monopoly_players_exposure_academy b on b.id = m.b_player
                 where m.tournament_id = $1 and m.status in ('done','failed')
                 order by m.round desc, m.created_at desc",
            )
            .bind(t.id)
            .fetch_all(&app.pool)
            .await
            .unwrap(),
            None => Vec::new(),
        };
        return Ok(Html(html::monopoly_history(
            &user,
            tournament.as_ref(),
            &matches,
        )));
    }
    let entry: Option<MonopolyEntry> = match &team {
        Some(t) => sqlx::query_as(
            "select id, team_id, hf_repo, hf_revision, size_bytes, char_name, product_name,
                    product_desc, list_price, persona, updated_at
             from monopoly_entries_exposure_academy where team_id = $1",
        )
        .bind(t.id)
        .fetch_optional(&app.pool)
        .await
        .unwrap(),
        None => None,
    };
    let members: Vec<TeamMemberRow> = sqlx::query_as(
        "select tm.team_id, tm.user_id, u.display_name,
                (u.nickname is not null and not u.hidden_from_leaderboard) as public
         from monopoly_team_members_exposure_academy tm
         join users_exposure_academy u on u.id = tm.user_id
         order by tm.created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let practice: Vec<MonopolyMatchRow> = match &team {
        Some(t) => sqlx::query_as(
            "select m.id, m.round, m.kind, m.status, m.a_player, a.char_name as a_name,
                    m.b_player, b.char_name as b_name, m.created_at
             from monopoly_matches_exposure_academy m
             join monopoly_players_exposure_academy a on a.id = m.a_player
             join monopoly_players_exposure_academy b on b.id = m.b_player
             where m.requested_by_team = $1 order by m.created_at desc limit 5",
        )
        .bind(t.id)
        .fetch_all(&app.pool)
        .await
        .unwrap(),
        None => Vec::new(),
    };
    Ok(Html(html::monopoly_main(
        &user,
        team.as_ref(),
        &members,
        entry.as_ref(),
        tournament.as_ref(),
        &standings,
        &practice,
    )))
}

pub async fn latest_tournament(app: &App) -> Option<MonopolyTournament> {
    sqlx::query_as(
        "select id, status, round, rounds_total, progress, error_log, created_at
         from monopoly_tournaments_exposure_academy order by created_at desc limit 1",
    )
    .fetch_optional(&app.pool)
    .await
    .unwrap()
}

/// Standings for one tournament, richest first. Net worth is cash + goods, computed in
/// SQL for the ordering and in `MonopolyStandingRow::net_worth` for display — same sum.
pub async fn monopoly_standings(app: &App, tid: Uuid) -> Vec<MonopolyStandingRow> {
    sqlx::query_as(
        "select p.id, p.team_id, t.name as team_name, p.char_name, p.product_name, p.cash, p.goods
         from monopoly_players_exposure_academy p
         join monopoly_teams_exposure_academy t on t.id = p.team_id
         where p.tournament_id = $1
         order by (p.cash + p.goods) desc, lower(p.char_name)",
    )
    .bind(tid)
    .fetch_all(&app.pool)
    .await
    .unwrap()
}

#[derive(Deserialize)]
pub struct MonopolyLiveQ {
    /// Which conversation to show. Absent = whatever is happening now.
    #[serde(rename = "match")]
    match_id: Option<Uuid>,
    /// Which conversation the client currently has drawn. `seq` only applies if the
    /// focus is still that match — otherwise the arena switched conversations and the
    /// new one's early turns would be filtered out by a stale high-water mark.
    have: Option<Uuid>,
    /// Highest turn the client already has; only later turns come back.
    seq: Option<i32>,
}

/// The arena's poll. One JSON per tick carrying the tournament header, the current
/// round's matches, the focused conversation's new turns, and the standings — the client
/// diffs it against what it already drew. Deliberately one endpoint: the alternative is
/// four polls racing each other into an inconsistent frame.
pub async fn monopoly_live_json(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<MonopolyLiveQ>,
) -> Result<Json<serde_json::Value>, Response> {
    require_onboarded(current_user(&app, &headers).await)?;
    let Some(t) = latest_tournament(&app).await else {
        return Ok(Json(serde_json::json!({"status": null})));
    };
    let matches: Vec<MonopolyMatchRow> = sqlx::query_as(
        "select m.id, m.round, m.kind, m.status, m.a_player, a.char_name as a_name,
                m.b_player, b.char_name as b_name, m.created_at
         from monopoly_matches_exposure_academy m
         join monopoly_players_exposure_academy a on a.id = m.a_player
         join monopoly_players_exposure_academy b on b.id = m.b_player
         where m.tournament_id = $1 and m.round = $2
         order by (m.kind <> 'mandatory'), m.created_at",
    )
    .bind(t.id)
    .bind(t.round)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    // Focus: what the student asked for, else the conversation actually in flight, else
    // the last one that finished — so the arena is never blank mid-round.
    let focus = q
        .match_id
        .and_then(|id| matches.iter().find(|m| m.id == id))
        .or_else(|| matches.iter().find(|m| m.status == "running"))
        .or_else(|| matches.iter().rev().find(|m| m.status == "done"))
        .or_else(|| matches.first());

    let detail = match focus {
        None => serde_json::Value::Null,
        Some(m) => {
            let since = if q.have == Some(m.id) {
                q.seq.unwrap_or(0)
            } else {
                0
            };
            let msgs: Vec<MonopolyMessage> = sqlx::query_as(
                "select seq, speaker, content from monopoly_messages_exposure_academy
                 where match_id = $1 and seq > $2 order by seq",
            )
            .bind(m.id)
            .bind(since)
            .fetch_all(&app.pool)
            .await
            .unwrap();
            // Only a finished conversation has a verdict; asking earlier is a wasted query.
            let txs: Vec<MonopolyTxRow> = if m.status == "done" {
                sqlx::query_as(
                    "select b.char_name as buyer_name, s.char_name as seller_name,
                            x.item, x.price, x.value_to_buyer, x.reasoning
                     from monopoly_transactions_exposure_academy x
                     join monopoly_players_exposure_academy b on b.id = x.buyer_player
                     join monopoly_players_exposure_academy s on s.id = x.seller_player
                     where x.match_id = $1 order by x.created_at",
                )
                .bind(m.id)
                .fetch_all(&app.pool)
                .await
                .unwrap()
            } else {
                Vec::new()
            };
            // Only ids for the two sides: cash, goods and product come from `standings`
            // below, which the client already has to render — no second lookup.
            serde_json::json!({
                "id": m.id, "round": m.round, "kind": m.kind, "status": m.status,
                "a_id": m.a_player, "b_id": m.b_player,
                "a_name": m.a_name, "b_name": m.b_name,
                "messages": msgs.iter().map(|x| serde_json::json!({
                    "seq": x.seq, "speaker": x.speaker, "content": x.content })).collect::<Vec<_>>(),
                "transactions": txs.iter().map(|x| serde_json::json!({
                    "buyer": x.buyer_name, "seller": x.seller_name, "item": x.item,
                    "price": x.price, "value": x.value_to_buyer, "surplus": x.surplus(),
                    "reasoning": x.reasoning })).collect::<Vec<_>>(),
            })
        }
    };

    // Declined invitations leave no match row, so without this the arena would show only
    // the conversations that happened and never the ones someone refused to have.
    let invites: Vec<(String, String, bool)> = sqlx::query_as(
        "select f.char_name, t2.char_name, i.accepted
         from monopoly_invites_exposure_academy i
         join monopoly_players_exposure_academy f on f.id = i.from_player
         join monopoly_players_exposure_academy t2 on t2.id = i.to_player
         where i.tournament_id = $1 and i.round = $2
         order by i.created_at",
    )
    .bind(t.id)
    .bind(t.round)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let standings = monopoly_standings(&app, t.id).await;
    Ok(Json(serde_json::json!({
        "invites": invites.iter().map(|(f, to, ok)| serde_json::json!({
            "from": f, "to": to, "accepted": ok })).collect::<Vec<_>>(),
        "status": t.status,
        "round": t.round,
        "rounds_total": t.rounds_total,
        "progress": t.progress,
        "matches": matches.iter().map(|m| serde_json::json!({
            "id": m.id, "kind": m.kind, "status": m.status,
            "a_name": m.a_name, "b_name": m.b_name })).collect::<Vec<_>>(),
        "match": detail,
        "standings": standings.iter().map(|s| serde_json::json!({
            "id": s.id, "char_name": s.char_name, "team_name": s.team_name,
            "product_name": s.product_name, "cash": s.cash, "goods": s.goods,
            "net": s.net_worth() })).collect::<Vec<_>>(),
    })))
}

/// Queue one practice conversation: this team's own merchant against another team's
/// model wearing a decoy. Both sides get throwaway player rows with `tournament_id` null,
/// so nothing here can touch the tournament ledger or the standings.
///
/// Only one may be in flight per team, and that is enforced by a partial unique index on
/// `requested_by_team` rather than by a check here — two rapid clicks race a pre-check but
/// cannot race the index.
pub async fn monopoly_practice(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();
    let Some(team) = monopoly_team_of(&app, user.id).await else {
        return Err(bad("Bir takımda değilsin — eğitmenine yaz."));
    };
    if let Some(t) = latest_tournament(&app).await {
        if t.status != "done" && t.status != "failed" {
            return Err(bad("Turnuva sürerken antrenman yapılamaz."));
        }
    }
    let mine: Option<MonopolyEntry> = sqlx::query_as(
        "select id, team_id, hf_repo, hf_revision, size_bytes, char_name, product_name,
                product_desc, list_price, persona, updated_at
         from monopoly_entries_exposure_academy where team_id = $1",
    )
    .bind(team.id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(mine) = mine else {
        return Err(bad("Önce kendi modelini gönder."));
    };
    // Any other team's model will do — practising against your own teaches you nothing.
    let opp: Option<MonopolyEntry> = sqlx::query_as(
        "select id, team_id, hf_repo, hf_revision, size_bytes, char_name, product_name,
                product_desc, list_price, persona, updated_at
         from monopoly_entries_exposure_academy where team_id <> $1 order by random() limit 1",
    )
    .bind(team.id)
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some(opp) = opp else {
        return Err(bad("Karşına çıkacak başka bir model henüz yok."));
    };
    let (dn, dp, dd, dl, dpe) =
        MONOPOLY_DECOYS[rand::random::<u32>() as usize % MONOPOLY_DECOYS.len()];

    let mut tx = app.pool.begin().await.unwrap();
    let insert =
        |team_id: Uuid, e: &MonopolyEntry, cn: &str, pn: &str, pd: &str, lp: i32, pe: &str| {
            sqlx::query_scalar::<_, Uuid>(
                "insert into monopoly_players_exposure_academy
               (team_id, entry_id, hf_repo, hf_revision, char_name, product_name,
                product_desc, list_price, persona, cash)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) returning id",
            )
            .bind(team_id)
            .bind(e.id)
            .bind(e.hf_repo.clone())
            .bind(e.hf_revision.clone())
            .bind(cn.to_string())
            .bind(pn.to_string())
            .bind(pd.to_string())
            .bind(lp)
            .bind(pe.to_string())
            .bind(MONOPOLY_START_CASH)
        };
    let a: Uuid = insert(
        team.id,
        &mine,
        &mine.char_name,
        &mine.product_name,
        &mine.product_desc,
        mine.list_price,
        &mine.persona,
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let b: Uuid = insert(opp.team_id, &opp, dn, dp, dd, dl, dpe)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let created = sqlx::query(
        "insert into monopoly_matches_exposure_academy
           (round, kind, a_player, b_player, requested_by_team) values (0,'practice',$1,$2,$3)",
    )
    .bind(a)
    .bind(b)
    .bind(team.id)
    .execute(&mut *tx)
    .await;
    if created.is_err() {
        // the index rejected it: this team already has one waiting or running
        return Err(bad("Zaten süren bir antrenman maçın var."));
    }
    tx.commit().await.unwrap();
    Ok(Redirect::to("/ai-monopoly"))
}

/// Turkish translations of one conversation's turns, in order. Each message goes over as
/// its own array element so DeepL never sees the speaker labels and can't translate a
/// merchant's name; the result is zipped back onto the messages at render time.
///
/// Called lazily, on the first time somebody opens a finished conversation, and cached in
/// `transcript_tr` — a tournament produces far more conversations than anyone reads, and
/// translating on completion would spend the quota on all of them.
async fn translate_tr(app: &App, texts: &[String]) -> Option<Vec<String>> {
    if app.deepl_key.is_empty() || texts.is_empty() {
        return None;
    }
    // free keys carry a `:fx` suffix and live on a different host — DeepL's own rule
    let host = if app.deepl_key.ends_with(":fx") {
        "api-free"
    } else {
        "api"
    };
    let resp = app
        .http
        .post(format!("https://{host}.deepl.com/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {}", app.deepl_key))
        .json(&serde_json::json!({
            "text": texts, "source_lang": "EN", "target_lang": "TR" }))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let out: Vec<String> = body
        .get("translations")?
        .as_array()?
        .iter()
        .map(|t| {
            t.get("text")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    // a partial answer would silently pair turns with the wrong translation
    if out.len() == texts.len() {
        Some(out)
    } else {
        None
    }
}

/// One conversation in full: both transcripts, the judge's ruling, and — once the
/// tournament is over — what each model privately wrote about the other.
pub async fn monopoly_match_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let m: MonopolyMatchRow = sqlx::query_as(
        "select m.id, m.round, m.kind, m.status, m.a_player, a.char_name as a_name,
                m.b_player, b.char_name as b_name, m.created_at
         from monopoly_matches_exposure_academy m
         join monopoly_players_exposure_academy a on a.id = m.a_player
         join monopoly_players_exposure_academy b on b.id = m.b_player
         where m.id = $1",
    )
    .bind(id)
    .fetch_optional(&app.pool)
    .await
    .unwrap()
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;

    // A practice transcript shows the requesting team's real merchant — the opponent is
    // decoyed, they are not. Only that team (and an admin) may read it.
    let owner: Option<Uuid> = sqlx::query_scalar(
        "select requested_by_team from monopoly_matches_exposure_academy where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    if let Some(owner) = owner {
        let mine = monopoly_team_of(&app, user.id).await.map(|t| t.id);
        if mine != Some(owner) && !user.is_admin {
            return Err(StatusCode::NOT_FOUND.into_response());
        }
    }

    let msgs: Vec<MonopolyMessage> = sqlx::query_as(
        "select seq, speaker, content from monopoly_messages_exposure_academy
         where match_id = $1 order by seq",
    )
    .bind(id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let txs: Vec<MonopolyTxRow> = sqlx::query_as(
        "select b.char_name as buyer_name, s.char_name as seller_name,
                x.item, x.price, x.value_to_buyer, x.reasoning
         from monopoly_transactions_exposure_academy x
         join monopoly_players_exposure_academy b on b.id = x.buyer_player
         join monopoly_players_exposure_academy s on s.id = x.seller_player
         where x.match_id = $1 order by x.created_at",
    )
    .bind(id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    // The notes are the post-game reveal: withheld while anyone can still act on them.
    let reveal: bool = sqlx::query_scalar(
        "select coalesce((select t.status = 'done' from monopoly_tournaments_exposure_academy t
                          join monopoly_matches_exposure_academy m on m.tournament_id = t.id
                          where m.id = $1), false)",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let notes: Vec<MonopolyNoteRow> = if reveal {
        sqlx::query_as(
            "select au.char_name as author_name, ab.char_name as about_name, m.round, n.note
             from monopoly_notes_exposure_academy n
             join monopoly_matches_exposure_academy m on m.id = n.match_id
             join monopoly_players_exposure_academy au on au.id = n.author_player
             join monopoly_players_exposure_academy ab
               on ab.id = case when n.author_player = m.a_player then m.b_player else m.a_player end
             where n.match_id = $1 order by au.char_name",
        )
        .bind(id)
        .fetch_all(&app.pool)
        .await
        .unwrap()
    } else {
        Vec::new()
    };

    // Everything a student reads in prose goes through DeepL: the turns, the judge's
    // reasoning and the models' notes. All three ride one call and are cached together;
    // the notes only join the batch once they're revealed, so a cache written earlier is
    // short and gets redone then.
    let want: Vec<String> = msgs
        .iter()
        .map(|x| x.content.clone())
        .chain(txs.iter().map(|t| t.reasoning.clone().unwrap_or_default()))
        .chain(notes.iter().map(|n| n.note.clone()))
        .collect();
    let cached: Option<String> = sqlx::query_scalar(
        "select transcript_tr from monopoly_matches_exposure_academy where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let mut tr: Option<Vec<String>> = cached
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .filter(|v| v.len() == want.len());
    if tr.is_none() && m.status == "done" && !want.is_empty() {
        if let Some(out) = translate_tr(&app, &want).await {
            let _ = sqlx::query(
                "update monopoly_matches_exposure_academy set transcript_tr = $2 where id = $1",
            )
            .bind(id)
            .bind(serde_json::to_string(&out).unwrap())
            .execute(&app.pool)
            .await;
            tr = Some(out);
        }
    }
    // split back into the three groups the page renders
    let (mtr, xtr, ntr) = match &tr {
        Some(v) => (
            v[..msgs.len()].to_vec(),
            v[msgs.len()..msgs.len() + txs.len()].to_vec(),
            v[msgs.len() + txs.len()..].to_vec(),
        ),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };

    Ok(Html(html::monopoly_match(
        &user, &m, &msgs, &mtr, &txs, &xtr, &notes, &ntr, reveal,
    )))
}

#[derive(Deserialize)]
pub struct MonopolySubmitForm {
    hf_repo: String,
    char_name: String,
    product_name: String,
    product_desc: String,
    list_price: i32,
    persona: String,
}

pub async fn monopoly_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<MonopolySubmitForm>,
) -> Result<Redirect, Response> {
    let user = require_onboarded(current_user(&app, &headers).await)?;
    let bad = |msg: String| (StatusCode::BAD_REQUEST, msg).into_response();
    let Some(team) = monopoly_team_of(&app, user.id).await else {
        // the form isn't rendered for team-less students; this catches hand-rolled POSTs
        return Err(bad("Bir takımda değilsin — eğitmenine yaz.".into()));
    };
    // Entries are frozen into players when the tournament starts, so a late edit could
    // not affect the run — but seeing your old merchant in the arena after "changing" it
    // is confusing, so the section is simply closed while one is playing.
    if let Some(t) = latest_tournament(&app).await {
        if t.status != "done" && t.status != "failed" {
            return Err(bad("Turnuva sürerken gönderim değiştirilemez.".into()));
        }
    }
    // Length caps, in one place. These are what the arena can actually lay out, and the
    // persona is a system prompt — it should be a character brief, not a whole strategy.
    let field = |v: &str, max: usize, what: &str| -> Result<String, Response> {
        let v = v.trim();
        if v.is_empty() {
            return Err(bad(format!("{what} boş olamaz.")));
        }
        if v.chars().count() > max {
            return Err(bad(format!("{what} en fazla {max} karakter olabilir.")));
        }
        Ok(v.to_string())
    };
    let char_name = field(&f.char_name, 40, "Karakter adı")?;
    let product_name = field(&f.product_name, 60, "Ürün adı")?;
    let product_desc = field(&f.product_desc, 300, "Ürün açıklaması")?;
    let persona = field(&f.persona, 1500, "Karakter tanımı")?;
    if !(1..=100_000).contains(&f.list_price) {
        return Err(bad("Fiyat 1 ile 100000 ₺ arasında olmalı.".into()));
    }
    let Some(repo) = normalize_hf_repo(&f.hf_repo) else {
        return Err(bad("Model kimliği org/model biçiminde olmalı.".into()));
    };
    let model = hf_validate(&app.http, &repo).await.map_err(bad)?;

    // One entry per team, replaced wholesale — resubmitting is how a team changes
    // its model or its merchant, and the unique index on team_id is what makes that
    // an update rather than a second entry.
    sqlx::query(
        "insert into monopoly_entries_exposure_academy
           (team_id, submitted_by, hf_repo, hf_revision, size_bytes, char_name,
            product_name, product_desc, list_price, persona)
         values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         on conflict (team_id) do update set
           submitted_by = excluded.submitted_by, hf_repo = excluded.hf_repo,
           hf_revision = excluded.hf_revision, size_bytes = excluded.size_bytes,
           char_name = excluded.char_name, product_name = excluded.product_name,
           product_desc = excluded.product_desc, list_price = excluded.list_price,
           persona = excluded.persona, updated_at = now()",
    )
    .bind(team.id)
    .bind(user.id)
    .bind(&repo)
    .bind(&model.revision)
    .bind(model.size_bytes)
    .bind(&char_name)
    .bind(&product_name)
    .bind(&product_desc)
    .bind(f.list_price)
    .bind(&persona)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Gönderim kaydedilemedi.".into()))?;
    Ok(Redirect::to("/ai-monopoly"))
}

pub async fn admin_monopoly_team(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<NameForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let name = f.name.trim().to_string();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query(
        "insert into monopoly_teams_exposure_academy (name) values ($1) on conflict do nothing",
    )
    .bind(&name)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_monopoly_team_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades members, the entry, and any players — which takes their matches,
    // transcripts and transactions with them. The confirm() on the button says so.
    sqlx::query("delete from monopoly_teams_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_monopoly_member(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<MemberForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // user_id is the PK, so assigning an already-assigned student moves them. A running
    // tournament is unaffected: it plays frozen player rows, not the live roster.
    sqlx::query(
        "insert into monopoly_team_members_exposure_academy (user_id, team_id) values ($1,$2)
         on conflict (user_id) do update set team_id = excluded.team_id",
    )
    .bind(f.user_id)
    .bind(f.team_id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

pub async fn admin_monopoly_member_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from monopoly_team_members_exposure_academy where user_id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Redirect::to("/admin"))
}

/// Start the tournament: snapshot every current entry into a player row and queue the
/// run. The snapshot is the point — after this a team may keep editing its entry, and
/// none of it reaches the tournament being played or the history it leaves behind.
pub async fn admin_monopoly_start(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let bad = |msg: &str| (StatusCode::BAD_REQUEST, msg.to_string()).into_response();
    let entries: Vec<MonopolyEntry> = sqlx::query_as(
        "select id, team_id, hf_repo, hf_revision, size_bytes, char_name, product_name,
                product_desc, list_price, persona, updated_at
         from monopoly_entries_exposure_academy order by created_at",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    if entries.len() < 2 {
        return Err(bad("Turnuva için en az iki gönderim gerekiyor."));
    }
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|_| bad("Veritabanı hatası."))?;
    // n rounds for n models, per the game rules.
    let tid: Uuid = sqlx::query_scalar(
        "insert into monopoly_tournaments_exposure_academy (rounds_total) values ($1) returning id",
    )
    .bind(entries.len() as i32)
    .fetch_one(&mut *tx)
    .await
    // the singleton partial index is what rejects a second active tournament, so an
    // admin double-click arrives here as a constraint error rather than a second run
    .map_err(|_| bad("Zaten devam eden bir turnuva var."))?;
    let mut player_ids: Vec<Uuid> = Vec::with_capacity(entries.len());
    for e in &entries {
        let pid: Uuid = sqlx::query_scalar(
            "insert into monopoly_players_exposure_academy
               (tournament_id, team_id, entry_id, hf_repo, hf_revision, char_name,
                product_name, product_desc, list_price, persona, cash)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) returning id",
        )
        .bind(tid)
        .bind(e.team_id)
        .bind(e.id)
        .bind(&e.hf_repo)
        .bind(&e.hf_revision)
        .bind(&e.char_name)
        .bind(&e.product_name)
        .bind(&e.product_desc)
        .bind(e.list_price)
        .bind(&e.persona)
        .bind(MONOPOLY_START_CASH)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| bad("Oyuncular oluşturulamadı."))?;
        player_ids.push(pid);
    }
    // The whole mandatory schedule goes in now rather than round by round: it's fully
    // determined by the player count, and having the rows up front is what lets the
    // arena show the round strip with matches that haven't been played yet.
    for (r, pairs) in round_robin(player_ids.len(), entries.len())
        .iter()
        .enumerate()
    {
        for (a, b) in pairs {
            sqlx::query(
                "insert into monopoly_matches_exposure_academy
                   (tournament_id, round, kind, a_player, b_player) values ($1,$2,'mandatory',$3,$4)")
                .bind(tid).bind(r as i32 + 1).bind(player_ids[*a]).bind(player_ids[*b])
                .execute(&mut *tx).await.map_err(|_| bad("Eşleşmeler oluşturulamadı."))?;
        }
    }
    tx.commit().await.map_err(|_| bad("Veritabanı hatası."))?;
    Ok(Redirect::to("/admin"))
}

/// Stuck-tournament escape hatch, the same role `admin_harness_run_fail` plays: a worker
/// that died mid-run leaves the tournament non-terminal, which the singleton index reads
/// as "still active" and which would block ever starting another. A late worker report
/// against it then gets a 409 and is dropped.
pub async fn admin_monopoly_fail(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query(
        "update monopoly_tournaments_exposure_academy
         set status = 'failed', error_log = 'Yönetici tarafından durduruldu.', updated_at = now()
         where id = $1 and status not in ('done','failed')",
    )
    .bind(f.id)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    // its in-flight matches go with it, so the live view stops showing a conversation
    // that nothing is driving any more
    sqlx::query(
        "update monopoly_matches_exposure_academy set status = 'failed', updated_at = now()
         where tournament_id = $1 and status not in ('done','failed')",
    )
    .bind(f.id)
    .execute(&app.pool)
    .await
    .ok();
    Ok(Redirect::to("/admin"))
}

/// Everything the runner needs to serve one player and write its system prompt.
pub fn monopoly_player_json(p: &MonopolyPlayerFull) -> serde_json::Value {
    serde_json::json!({
        "id": p.id, "hf_repo": p.hf_repo, "hf_revision": p.hf_revision,
        "char_name": p.char_name, "product_name": p.product_name,
        "product_desc": p.product_desc, "list_price": p.list_price,
        "persona": p.persona, "cash": p.cash, "goods": p.goods,
    })
}

pub async fn worker_monopoly_pending(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    // A queued tournament outranks practice: claim it atomically, queued -> booting.
    let claimed: Option<(Uuid, i32)> = sqlx::query_as(
        "update monopoly_tournaments_exposure_academy set status = 'booting', updated_at = now()
         where id in (select id from monopoly_tournaments_exposure_academy
                      where status = 'queued' order by created_at limit 1)
         returning id, rounds_total",
    )
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    // An already-claimed tournament still needs serving after a runner restart, so fall
    // back to whichever one is simply non-terminal.
    let live: Option<(Uuid, i32)> = match claimed {
        Some(t) => Some(t),
        None => sqlx::query_as(
            "select id, rounds_total from monopoly_tournaments_exposure_academy
             where status not in ('done','failed') order by created_at limit 1",
        )
        .fetch_optional(&app.pool)
        .await
        .unwrap(),
    };
    if let Some((tid, rounds_total)) = live {
        let players: Vec<MonopolyPlayerFull> = sqlx::query_as(
            "select id, hf_repo, hf_revision, char_name, product_name, product_desc,
                    list_price, persona, cash, goods
             from monopoly_players_exposure_academy where tournament_id = $1 order by created_at",
        )
        .bind(tid)
        .fetch_all(&app.pool)
        .await
        .unwrap();
        let matches: Vec<(Uuid, i32, String, Uuid, Uuid)> = sqlx::query_as(
            "select id, round, kind, a_player, b_player from monopoly_matches_exposure_academy
             where tournament_id = $1 and status not in ('done','failed') order by round, created_at")
            .bind(tid).fetch_all(&app.pool).await.unwrap();
        return Ok(Json(serde_json::json!({
            "kind": "tournament",
            "id": tid,
            "rounds_total": rounds_total,
            "players": players.iter().map(monopoly_player_json).collect::<Vec<_>>(),
            "matches": matches.iter().map(|(id, round, kind, a, b)| serde_json::json!({
                "id": id, "round": round, "kind": kind, "a_player": a, "b_player": b,
            })).collect::<Vec<_>>(),
        })));
    }
    // Otherwise the next queued practice match, claimed the same way.
    let practice: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "update monopoly_matches_exposure_academy set status = 'running', updated_at = now()
         where id in (select id from monopoly_matches_exposure_academy
                      where tournament_id is null and status = 'queued'
                      order by created_at limit 1)
         returning id, a_player, b_player",
    )
    .fetch_optional(&app.pool)
    .await
    .unwrap();
    let Some((mid, a, b)) = practice else {
        return Ok(Json(serde_json::json!({"kind": "none"})));
    };
    let sides: Vec<MonopolyPlayerFull> = sqlx::query_as(
        "select id, hf_repo, hf_revision, char_name, product_name, product_desc,
                list_price, persona, cash, goods
         from monopoly_players_exposure_academy where id = any($1)",
    )
    .bind(vec![a, b])
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let side = |want: Uuid| {
        sides
            .iter()
            .find(|p| p.id == want)
            .map(monopoly_player_json)
    };
    Ok(Json(serde_json::json!({
        "kind": "practice", "match_id": mid, "a": side(a), "b": side(b),
    })))
}

#[derive(Deserialize)]
pub struct MonopolyStageReq {
    id: Uuid,
    status: String,
}

pub async fn worker_monopoly_stage(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyStageReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    // `booting` is set by the claim in `pending`, `done`/`failed` go through `result`;
    // what's left is the middle of the ladder, each guarded on its exact predecessor.
    let Some(idx) = MONOPOLY_STATUSES
        .iter()
        .position(|s| *s == r.status)
        .filter(|i| (2..=4).contains(i))
    else {
        return Err(StatusCode::BAD_REQUEST.into_response());
    };
    let res = sqlx::query(
        "update monopoly_tournaments_exposure_academy set status = $2, updated_at = now()
         where id = $1 and status = $3",
    )
    .bind(r.id)
    .bind(&r.status)
    .bind(MONOPOLY_STATUSES[idx - 1])
    .execute(&app.pool)
    .await
    .unwrap();
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MonopolyProgressReq {
    id: Uuid,
    round: Option<i32>,
    progress: String,
}

/// Repeatable mid-run report: the round number and the live blob the arena shows. Kept
/// separate from `stage` precisely because it repeats — `running` spans every round.
pub async fn worker_monopoly_progress(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyProgressReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.progress.len() > 2000 {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    let res = sqlx::query(
        "update monopoly_tournaments_exposure_academy
         set progress = $2, round = coalesce($3, round), updated_at = now()
         where id = $1 and status not in ('done','failed')",
    )
    .bind(r.id)
    .bind(&r.progress)
    .bind(r.round)
    .execute(&app.pool)
    .await
    .unwrap();
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MonopolyMessageReq {
    match_id: Uuid,
    seq: i32,
    speaker: String,
    content: String,
}

/// One completed turn. This is the live feed — the arena polls the rows this writes.
/// Idempotent on (match_id, seq) so a runner retrying a timed-out POST can't double-post.
pub async fn worker_monopoly_message(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyMessageReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if (r.speaker != "a" && r.speaker != "b") || r.content.len() > 8000 {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    // first message flips the match to running, so the arena knows where to look
    let res = sqlx::query(
        "update monopoly_matches_exposure_academy set status = 'running', updated_at = now()
         where id = $1 and status not in ('done','failed')",
    )
    .bind(r.match_id)
    .execute(&app.pool)
    .await
    .unwrap();
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    sqlx::query(
        "insert into monopoly_messages_exposure_academy (match_id, seq, speaker, content)
         values ($1,$2,$3,$4) on conflict (match_id, seq) do nothing",
    )
    .bind(r.match_id)
    .bind(r.seq)
    .bind(&r.speaker)
    .bind(&r.content)
    .execute(&app.pool)
    .await
    .unwrap();
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MonopolyInviteReq {
    tournament_id: Uuid,
    round: i32,
    from_player: Uuid,
    to_player: Uuid,
    accepted: bool,
}

/// The self-chosen extra conversation: who asked whom, and whether they agreed. On an
/// acceptance the server creates the match and hands back its id, so the runner never
/// invents match rows of its own.
pub async fn worker_monopoly_invite(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyInviteReq>,
) -> Result<Json<serde_json::Value>, Response> {
    check_worker(&app, &headers)?;
    if r.from_player == r.to_player {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    sqlx::query(
        "insert into monopoly_invites_exposure_academy
           (tournament_id, round, from_player, to_player, accepted) values ($1,$2,$3,$4,$5)
         on conflict (tournament_id, round, from_player) do update
           set to_player = excluded.to_player, accepted = excluded.accepted",
    )
    .bind(r.tournament_id)
    .bind(r.round)
    .bind(r.from_player)
    .bind(r.to_player)
    .bind(r.accepted)
    .execute(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    if !r.accepted {
        return Ok(Json(serde_json::json!({"match_id": null})));
    }
    // the inviter speaks first — they're the one who wanted the conversation
    let mid: Uuid = sqlx::query_scalar(
        "insert into monopoly_matches_exposure_academy
           (tournament_id, round, kind, a_player, b_player) values ($1,$2,'chosen',$3,$4)
         returning id",
    )
    .bind(r.tournament_id)
    .bind(r.round)
    .bind(r.from_player)
    .bind(r.to_player)
    .fetch_one(&app.pool)
    .await
    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(Json(serde_json::json!({"match_id": mid})))
}

#[derive(Deserialize)]
pub struct MonopolyTxReq {
    buyer: Uuid,
    seller: Uuid,
    item: String,
    price: i32,
    value_to_buyer: i32,
    reasoning: Option<String>,
}

#[derive(Deserialize)]
pub struct MonopolyVerdictReq {
    match_id: Uuid,
    #[serde(default)]
    transactions: Vec<MonopolyTxReq>,
}

/// The judge's ruling, turned into money. The runner reports what was decided; every
/// arithmetic step happens here, once, inside one transaction:
///
/// - claiming the match with `status not in ('done','failed')` makes the whole handler
///   idempotent, so a runner retrying a timed-out POST gets a 409 instead of paying
///   every transaction a second time;
/// - a sale is only accepted between the two sides of *this* conversation, so a buggy
///   or tampered-with runner can't move money between unrelated players;
/// - `apply_sale` clamps the price to what the buyer actually holds, which is what keeps
///   a judge talked into a silly number from minting money.
pub async fn worker_monopoly_verdict(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyVerdictReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    let mut tx = app
        .pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let claimed: Option<(Uuid, Uuid)> = sqlx::query_as(
        "update monopoly_matches_exposure_academy set status = 'done', updated_at = now()
         where id = $1 and status not in ('done','failed')
         returning a_player, b_player",
    )
    .bind(r.match_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap();
    let Some((a, b)) = claimed else {
        return Err(StatusCode::CONFLICT.into_response());
    };
    for t in &r.transactions {
        if !((t.buyer == a && t.seller == b) || (t.buyer == b && t.seller == a)) {
            continue;
        }
        // lock both ledgers for the life of the transaction
        let Some((buyer_cash, buyer_goods)): Option<(i32, i32)> = sqlx::query_as(
            "select cash, goods from monopoly_players_exposure_academy where id = $1 for update",
        )
        .bind(t.buyer)
        .fetch_optional(&mut *tx)
        .await
        .unwrap() else {
            continue;
        };
        let Some((seller_cash,)): Option<(i32,)> = sqlx::query_as(
            "select cash from monopoly_players_exposure_academy where id = $1 for update",
        )
        .bind(t.seller)
        .fetch_optional(&mut *tx)
        .await
        .unwrap() else {
            continue;
        };
        let sale = apply_sale(
            buyer_cash,
            buyer_goods,
            seller_cash,
            t.price,
            t.value_to_buyer,
        );
        sqlx::query(
            "update monopoly_players_exposure_academy set cash = $2, goods = $3 where id = $1",
        )
        .bind(t.buyer)
        .bind(sale.buyer_cash)
        .bind(sale.buyer_goods)
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query("update monopoly_players_exposure_academy set cash = $2 where id = $1")
            .bind(t.seller)
            .bind(sale.seller_cash)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(
            "insert into monopoly_transactions_exposure_academy
               (match_id, buyer_player, seller_player, item, price, value_to_buyer, reasoning)
             values ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(r.match_id)
        .bind(t.buyer)
        .bind(t.seller)
        .bind(t.item.chars().take(120).collect::<String>())
        // both figures post-clamp, so the history tab can never show a number that
        // didn't actually move
        .bind(sale.price)
        .bind(sale.value)
        .bind(
            t.reasoning
                .as_deref()
                .map(|s| s.chars().take(600).collect::<String>()),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MonopolyNoteReq {
    match_id: Uuid,
    author_player: Uuid,
    note: String,
}

pub async fn worker_monopoly_note(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyNoteReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    sqlx::query(
        "insert into monopoly_notes_exposure_academy (match_id, author_player, note)
         values ($1,$2,$3) on conflict (match_id, author_player) do update set note = excluded.note")
        .bind(r.match_id).bind(r.author_player)
        .bind(r.note.chars().take(1200).collect::<String>())
        .execute(&app.pool).await.map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MonopolyResultReq {
    id: Uuid,
    status: String,
    error_log: Option<String>,
    /// Set when a single match failed rather than the whole tournament.
    match_id: Option<Uuid>,
}

pub async fn worker_monopoly_result(
    State(app): State<App>,
    headers: HeaderMap,
    Json(r): Json<MonopolyResultReq>,
) -> Result<StatusCode, Response> {
    check_worker(&app, &headers)?;
    if r.status != "done" && r.status != "failed" {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }
    // A single failed match doesn't end the tournament — the rest still plays out.
    if let Some(mid) = r.match_id {
        let res = sqlx::query(
            "update monopoly_matches_exposure_academy
             set status = $2, error_log = $3, updated_at = now()
             where id = $1 and status not in ('done','failed')",
        )
        .bind(mid)
        .bind(&r.status)
        .bind(&r.error_log)
        .execute(&app.pool)
        .await
        .unwrap();
        return if res.rows_affected() == 0 {
            Err(StatusCode::CONFLICT.into_response())
        } else {
            Ok(StatusCode::NO_CONTENT)
        };
    }
    let res = sqlx::query(
        "update monopoly_tournaments_exposure_academy
         set status = $2, error_log = $3, progress = null, updated_at = now()
         where id = $1 and status not in ('done','failed')",
    )
    .bind(r.id)
    .bind(&r.status)
    .bind(&r.error_log)
    .execute(&app.pool)
    .await
    .unwrap();
    if res.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT.into_response());
    }
    // anything still queued when the tournament ends never ran
    sqlx::query(
        "update monopoly_matches_exposure_academy set status = 'failed', updated_at = now()
         where tournament_id = $1 and status not in ('done','failed')",
    )
    .bind(r.id)
    .execute(&app.pool)
    .await
    .ok();
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The money math and the schedule: a silent bug here corrupts a whole tournament.

    // ---- AI Monopoly ----

    #[test]
    fn sale_moves_money_and_books_goods() {
        // 1000 cash each; a 200₺ sale the judge values at 300 to the buyer.
        let s = apply_sale(1000, 0, 1000, 200, 300);
        assert_eq!(s.price, 200);
        assert_eq!(s.buyer_cash, 800);
        assert_eq!(s.buyer_goods, 300); // booked at what it's worth, not what it cost
        assert_eq!(s.seller_cash, 1000 + 120); // 200 less the 40% cost of goods
        // the buyer came out ahead by exactly the surplus
        assert_eq!(s.buyer_cash + s.buyer_goods - 1000, 100);
    }

    #[test]
    fn overpaying_loses_net_worth() {
        // talked into paying 500 for something worth 100
        let s = apply_sale(1000, 0, 0, 500, 100);
        assert_eq!(s.buyer_cash + s.buyer_goods, 600);
        assert!(
            s.buyer_cash + s.buyer_goods < 1000,
            "a bad deal must cost the buyer"
        );
    }

    #[test]
    fn price_is_clamped_to_what_the_buyer_holds() {
        // the judge talked into a silly number cannot mint money
        let s = apply_sale(50, 0, 0, 999_999, 999_999);
        assert_eq!(s.price, 50, "clamped to available cash");
        assert_eq!(s.buyer_cash, 0, "never goes into debt");
        assert_eq!(
            s.seller_cash, 30,
            "seller is paid on the clamped price, not the asked one"
        );
        // a broke buyer still can't be charged
        let broke = apply_sale(0, 0, 0, 400, 400);
        assert_eq!(broke.price, 0);
        assert_eq!(broke.seller_cash, 0);
    }

    #[test]
    fn negative_judge_values_are_floored() {
        // a judge returning nonsense must not subtract from the buyer's holdings
        let s = apply_sale(1000, 500, 0, 100, -900);
        assert_eq!(s.buyer_goods, 500, "goods never go down on a purchase");
        assert!(s.price >= 0);
    }

    #[test]
    fn judged_value_cannot_run_away_from_the_price() {
        // The injection that actually wins the game: net worth counts goods, so a judge
        // talked into "this is worth 999999 to you" would hand over the tournament even
        // though the price clamp kept the cash honest.
        let s = apply_sale(1000, 0, 0, 200, 999_999);
        assert_eq!(s.value, 600, "capped at 3x the price actually paid");
        assert_eq!(s.buyer_goods, 600);
        assert!(
            s.buyer_cash + s.buyer_goods <= 1000 + 200 * 2,
            "one trade can't multiply net worth without bound"
        );
        // a buyer who paid nothing gains nothing, however the item is described
        let free = apply_sale(0, 0, 0, 500, 999_999);
        assert_eq!(free.value, 0);
        assert_eq!(free.buyer_goods, 0);
        // an honest good deal still goes through untouched
        let fair = apply_sale(1000, 0, 0, 200, 300);
        assert_eq!(fair.value, 300);
    }

    #[test]
    fn selling_below_cost_is_a_loss() {
        // the 40% cost of goods is what stops dumping stock from being free cash
        let s = apply_sale(1000, 0, 1000, 100, 100);
        assert_eq!(s.seller_cash - 1000, 60);
        // seller nets less than the sticker price, always
        assert!(s.seller_cash - 1000 < 100);
    }

    #[test]
    fn round_robin_covers_every_pair_then_repeats() {
        // 4 players, 4 rounds: rounds 1-3 are a complete round-robin, round 4 repeats round 1
        let sched = round_robin(4, 4);
        assert_eq!(sched.len(), 4);
        for r in &sched {
            assert_eq!(r.len(), 2, "4 players = 2 matches per round, nobody idle");
        }
        let norm = |p: &Vec<(usize, usize)>| {
            let mut v: Vec<(usize, usize)> =
                p.iter().map(|(a, b)| (*a.min(b), *a.max(b))).collect();
            v.sort();
            v
        };
        let first_three: std::collections::HashSet<(usize, usize)> =
            sched[..3].iter().flat_map(norm).collect();
        assert_eq!(
            first_three.len(),
            6,
            "all 6 pairs of 4 players met exactly once"
        );
        assert_eq!(norm(&sched[3]), norm(&sched[0]), "round 4 replays round 1");
    }

    #[test]
    fn round_robin_gives_odd_counts_a_bye() {
        // 5 players: one sits out each round, and a full cycle is 5 rounds
        let sched = round_robin(5, 5);
        for r in &sched {
            assert_eq!(r.len(), 2, "5 players = 2 matches + 1 bye");
            let mut seen: Vec<usize> = r.iter().flat_map(|(a, b)| [*a, *b]).collect();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), 4, "nobody plays twice in one round");
            assert!(
                seen.iter().all(|i| *i < 5),
                "the ghost player never appears"
            );
        }
        let pairs: std::collections::HashSet<(usize, usize)> = sched
            .iter()
            .flat_map(|r| r.iter().map(|(a, b)| (*a.min(b), *a.max(b))))
            .collect();
        assert_eq!(
            pairs.len(),
            10,
            "all 10 pairs of 5 players met exactly once"
        );
        // and it degrades rather than panicking
        assert!(round_robin(1, 3).iter().all(|r| r.is_empty()));
    }

    #[test]
    fn hf_repo_accepts_what_students_actually_paste() {
        for (input, want) in [
            ("Qwen/Qwen3-0.6B", "Qwen/Qwen3-0.6B"),
            ("  Qwen/Qwen3-0.6B  ", "Qwen/Qwen3-0.6B"),
            ("https://huggingface.co/Qwen/Qwen3-0.6B", "Qwen/Qwen3-0.6B"),
            ("huggingface.co/Qwen/Qwen3-0.6B/", "Qwen/Qwen3-0.6B"),
            (
                "https://huggingface.co/org/my.model-v2/tree/main",
                "org/my.model-v2",
            ),
        ] {
            assert_eq!(
                normalize_hf_repo(input).as_deref(),
                Some(want),
                "input: {input}"
            );
        }
        // anything that could escape the API path it gets interpolated into
        for bad in [
            "",
            "no-slash",
            "a/b/c",
            "../../etc/passwd",
            "org/model?x=1",
            "org/model#f",
            "org name/model",
            "org/mo del",
            "/model",
            "org/",
        ] {
            assert_eq!(normalize_hf_repo(bad), None, "input: {bad}");
        }
    }
}
