//! Takım formasyonu — the admin board where students are dragged onto Agentic Harness
//! teams. Split out of `/admin` because that page renders ~866 KB and the team controls
//! there were two `<select>`s over a flat list of every student, with no view of who was
//! already assigned and no view of who was on no team at all.
//!
//! Harness only. AI Monopoly keeps its own roster tables and its own admin panel.

use axum::Form;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::admin::{IdForm, MemberForm, NameForm};
use crate::html;
use crate::model::*;
use crate::{App, auth::*};

#[derive(Deserialize)]
pub struct RenameForm {
    pub id: Uuid,
    pub name: String,
}

#[derive(Deserialize)]
pub struct TeamsQ {
    /// `?saved=1` after a no-JS form post, so the fallback path gets the same
    /// confirmation the fetch path shows inline.
    saved: Option<String>,
}

pub async fn teams_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<TeamsQ>,
) -> Result<Html<String>, Response> {
    let user = require_admin(current_user(&app, &headers).await)?;
    let teams: Vec<HarnessTeam> =
        sqlx::query_as("select id, name from harness_teams_exposure_academy order by lower(name)")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    // Left join, not the inner join /admin used: a kid on no team is exactly what the
    // board has to show. Real names, not nicknames — you assign the person in front of
    // you, and this matches the teammate chips on the harness page.
    let kids: Vec<FormationKid> = sqlx::query_as(
        "select u.id, u.display_name, tm.team_id
         from users_exposure_academy u
         left join harness_team_members_exposure_academy tm on tm.user_id = u.id
         where not u.is_admin
         order by lower(u.display_name)",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    // Moving a kid out of a team mid-run is allowed — the run belongs to the team, not
    // to whoever pressed submit. The badge is so it isn't a surprise.
    let busy: Vec<Uuid> = sqlx::query_scalar(
        "select distinct team_id from harness_runs_exposure_academy
         where stage not in ('done','partial','failed','infra_failed','cancelled')",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    Ok(Html(html::teams_page(
        &user,
        &teams,
        &kids,
        &busy,
        q.saved.is_some(),
    )))
}

/// Every mutation lands back on the board. `teams.js` follows the redirect and reloads;
/// with JS off the browser does the same thing by itself.
fn back() -> Redirect {
    Redirect::to("/admin/takimlar?saved=1")
}

fn bad(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, msg.to_string()).into_response()
}

pub async fn team_create(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<NameForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let name = f.name.trim();
    if name.is_empty() {
        return Err(bad("Takım adı boş olamaz."));
    }
    // unique on lower(name) — say so rather than silently doing nothing
    let done = sqlx::query(
        "insert into harness_teams_exposure_academy (name) values ($1) on conflict do nothing",
    )
    .bind(name)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Takım oluşturulamadı."))?;
    if done.rows_affected() == 0 {
        return Err(bad("Bu isimde bir takım zaten var."));
    }
    Ok(back())
}

pub async fn team_rename(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<RenameForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    let name = f.name.trim();
    if name.is_empty() {
        return Err(bad("Takım adı boş olamaz."));
    }
    sqlx::query("update harness_teams_exposure_academy set name = $2 where id = $1")
        .bind(f.id)
        .bind(name)
        .execute(&app.pool)
        .await
        .map_err(|_| bad("Bu isimde bir takım zaten var."))?;
    Ok(back())
}

pub async fn team_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // cascades members AND runs — the confirm() on the button says so
    sqlx::query("delete from harness_teams_exposure_academy where id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| bad("Takım silinemedi."))?;
    Ok(back())
}

pub async fn member_assign(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<MemberForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    // user_id is the PK, so a drop onto another column is a move, not a second row.
    // Past runs stay with the old team — runs are team-scoped, the score was the team's.
    sqlx::query(
        "insert into harness_team_members_exposure_academy (user_id, team_id) values ($1,$2)
         on conflict (user_id) do update set team_id = excluded.team_id",
    )
    .bind(f.user_id)
    .bind(f.team_id)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Öğrenci atanamadı."))?;
    sqlx::query("update users_exposure_academy set level = 'SERIES_A' where id = $1")
        .bind(f.user_id)
        .execute(&app.pool)
        .await
        .map_err(|_| bad("Öğrenci atanamadı."))?;
    Ok(back())
}

/// Drop onto the "Takımsız" column. `id` is the **user** id, matching `IdForm`'s reuse
/// in the panel this replaced.
pub async fn member_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(f): Form<IdForm>,
) -> Result<Redirect, Response> {
    require_admin(current_user(&app, &headers).await)?;
    sqlx::query("delete from harness_team_members_exposure_academy where user_id = $1")
        .bind(f.id)
        .execute(&app.pool)
        .await
        .map_err(|_| bad("Öğrenci takımdan çıkarılamadı."))?;
    // Only demote if they're not also on a monopoly team — either one keeps them advanced.
    sqlx::query(
        "update users_exposure_academy set level = 'PRESEED' where id = $1
         and not exists (select 1 from monopoly_team_members_exposure_academy where user_id = $1)",
    )
    .bind(f.id)
    .execute(&app.pool)
    .await
    .map_err(|_| bad("Öğrenci takımdan çıkarılamadı."))?;
    Ok(back())
}
