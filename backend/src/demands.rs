use crate::{
    api::{current_user, establish_partner_connection},
    error::{AppError, AppResult},
    models::UserRow,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_demands))
        .route(
            "/{id}/proposals",
            post(submit_proposal).delete(withdraw_proposal),
        )
        .route("/{id}/close", post(close_demand))
        .route("/proposals/{id}/accept", post(accept_proposal))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemandBoardResponse {
    role: String,
    demands: Vec<DemandView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemandView {
    id: String,
    creator_name: String,
    creator_avatar: String,
    song_name: String,
    target_keys: Vec<String>,
    target_labels: Vec<String>,
    budget_label: String,
    goal: String,
    cycle: String,
    status: String,
    proposal_count: i64,
    proposals: Vec<ProposalView>,
    created_at: String,
}

#[derive(FromRow)]
struct DemandRow {
    id: String,
    creator_user_id: String,
    creator_name: String,
    creator_avatar: String,
    song_name: String,
    target_keys_json: String,
    budget_label: String,
    goal: String,
    cycle: String,
    status: String,
    proposal_count: i64,
    created_at: String,
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct ProposalView {
    id: String,
    provider_user_id: String,
    provider_name: String,
    provider_avatar: String,
    amount: i64,
    cycle: String,
    deliverables: String,
    message: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposalInput {
    amount: i64,
    cycle: String,
    deliverables: String,
    message: Option<String>,
}

async fn list_demands(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<DemandBoardResponse>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    let rows = if user.role == "client" {
        sqlx::query_as::<_, DemandRow>(&demand_query("WHERE m.user_id = ?", "m.created_at DESC"))
            .bind(&user.id)
            .fetch_all(&state.pool)
            .await?
    } else {
        require_provider(&user)?;
        sqlx::query_as::<_, DemandRow>(&demand_query(
            "WHERE m.user_id != ? AND (
               m.status = 'open'
               OR EXISTS (
                 SELECT 1 FROM demand_proposals own
                 WHERE own.match_request_id = m.id AND own.provider_user_id = ?
               )
             )",
            "CASE WHEN m.status = 'open' THEN 0 ELSE 1 END, m.created_at DESC",
        ))
        .bind(&user.id)
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await?
    };
    let target_labels = load_target_labels(&state).await?;
    let mut demands = Vec::with_capacity(rows.len());
    for row in rows {
        let target_keys: Vec<String> =
            serde_json::from_str(&row.target_keys_json).unwrap_or_default();
        let labels = target_keys
            .iter()
            .map(|key| {
                target_labels
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| key.clone())
            })
            .collect();
        let proposals = load_proposals(&state, &row.id, &row.creator_user_id, &user).await?;
        demands.push(DemandView {
            id: row.id,
            creator_name: row.creator_name,
            creator_avatar: row.creator_avatar,
            song_name: row.song_name,
            target_keys,
            target_labels: labels,
            budget_label: row.budget_label,
            goal: row.goal,
            cycle: row.cycle,
            status: row.status,
            proposal_count: row.proposal_count,
            proposals,
            created_at: row.created_at,
        });
    }
    Ok(Json(DemandBoardResponse {
        role: user.role,
        demands,
    }))
}

fn demand_query(filter: &str, order: &str) -> String {
    format!(
        "SELECT m.id, m.user_id AS creator_user_id, u.organization AS creator_name,
         u.avatar AS creator_avatar, s.name AS song_name, m.target_keys AS target_keys_json,
         b.label AS budget_label, m.goal, m.cycle, m.status,
         (SELECT COUNT(*) FROM demand_proposals dp
          WHERE dp.match_request_id = m.id AND dp.status != 'withdrawn') AS proposal_count,
         m.created_at
         FROM match_requests m
         JOIN users u ON u.id = m.user_id
         JOIN songs s ON s.id = m.song_id
         JOIN budget_options b ON b.id = m.budget_id
         {filter}
         ORDER BY {order}"
    )
}

async fn load_target_labels(state: &AppState) -> AppResult<HashMap<String, String>> {
    Ok(
        sqlx::query_as::<_, (String, String)>("SELECT key, title FROM target_types")
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .collect(),
    )
}

async fn load_proposals(
    state: &AppState,
    demand_id: &str,
    creator_user_id: &str,
    user: &UserRow,
) -> AppResult<Vec<ProposalView>> {
    let filter = if user.id == creator_user_id {
        "dp.match_request_id = ?"
    } else {
        "dp.match_request_id = ? AND dp.provider_user_id = ?"
    };
    let query = format!(
        "SELECT dp.id, dp.provider_user_id, u.organization AS provider_name,
         u.avatar AS provider_avatar, dp.amount, dp.cycle, dp.deliverables, dp.message,
         dp.status, dp.created_at, dp.updated_at
         FROM demand_proposals dp
         JOIN users u ON u.id = dp.provider_user_id
         WHERE {filter}
         ORDER BY CASE dp.status WHEN 'accepted' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,
         dp.amount, dp.created_at"
    );
    let mut query = sqlx::query_as::<_, ProposalView>(&query).bind(demand_id);
    if user.id != creator_user_id {
        query = query.bind(&user.id);
    }
    Ok(query.fetch_all(&state.pool).await?)
}

async fn submit_proposal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(demand_id): Path<String>,
    Json(input): Json<ProposalInput>,
) -> AppResult<Json<ProposalView>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    require_provider(&user)?;
    validate_proposal(&input)?;
    let creator_user_id: Option<String> =
        sqlx::query_scalar("SELECT user_id FROM match_requests WHERE id = ? AND status = 'open'")
            .bind(&demand_id)
            .fetch_optional(&state.pool)
            .await?;
    let creator_user_id =
        creator_user_id.ok_or_else(|| AppError::NotFound("open demand not found".into()))?;
    if creator_user_id == user.id {
        return Err(AppError::BadRequest(
            "providers cannot quote their own demand".into(),
        ));
    }
    let has_public_profile: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM partners
           WHERE source_user_id = ? AND partner_type = 'provider' AND active = 1
         )",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    if !has_public_profile {
        return Err(AppError::BadRequest(
            "approved provider profile required".into(),
        ));
    }
    let existing_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM demand_proposals
         WHERE match_request_id = ? AND provider_user_id = ?",
    )
    .bind(&demand_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?;
    if existing_status.as_deref() == Some("accepted") {
        return Err(AppError::BadRequest(
            "accepted proposal cannot be changed".into(),
        ));
    }
    let proposal_id = Uuid::new_v4().to_string();
    let message = input.message.as_deref().unwrap_or("").trim();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO demand_proposals
         (id, match_request_id, provider_user_id, amount, cycle, deliverables, message)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(match_request_id, provider_user_id) DO UPDATE SET
         amount = excluded.amount, cycle = excluded.cycle,
         deliverables = excluded.deliverables, message = excluded.message,
         status = 'pending', updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&proposal_id)
    .bind(&demand_id)
    .bind(&user.id)
    .bind(input.amount)
    .bind(input.cycle.trim())
    .bind(input.deliverables.trim())
    .bind(message)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', '收到新的推广报价', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&creator_user_id)
    .bind(format!(
        "{}已提交推广报价，可前往需求中心查看。",
        user.organization
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let proposal = load_provider_proposal(&state, &demand_id, &user.id).await?;
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "proposal_submitted",
        Some("match_request"),
        Some(&demand_id),
        json!({ "proposalId": proposal.id, "amount": proposal.amount }),
    )
    .await;
    Ok(Json(proposal))
}

async fn withdraw_proposal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(demand_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    require_provider(&user)?;
    let result = sqlx::query(
        "UPDATE demand_proposals SET status = 'withdrawn', updated_at = CURRENT_TIMESTAMP
         WHERE match_request_id = ? AND provider_user_id = ? AND status = 'pending'",
    )
    .bind(&demand_id)
    .bind(&user.id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest("pending proposal not found".into()));
    }
    Ok(Json(json!({ "success": true })))
}

async fn accept_proposal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(proposal_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    require_creator(&user)?;
    let acceptance = sqlx::query_as::<_, AcceptanceRow>(
        "SELECT dp.id, dp.match_request_id, dp.provider_user_id, dp.status AS proposal_status,
         m.status AS demand_status, p.id AS provider_partner_id, p.name AS provider_name
         FROM demand_proposals dp
         JOIN match_requests m ON m.id = dp.match_request_id
         JOIN partners p ON p.source_user_id = dp.provider_user_id
         WHERE dp.id = ? AND m.user_id = ? AND p.active = 1",
    )
    .bind(&proposal_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("proposal not found".into()))?;
    let already_accepted =
        acceptance.proposal_status == "accepted" && acceptance.demand_status == "following";
    if !already_accepted
        && (acceptance.proposal_status != "pending" || acceptance.demand_status != "open")
    {
        return Err(AppError::BadRequest(
            "proposal is no longer available".into(),
        ));
    }
    if !already_accepted {
        let mut tx = state.pool.begin().await?;
        let accepted = sqlx::query(
            "UPDATE demand_proposals SET status = 'accepted', updated_at = CURRENT_TIMESTAMP
             WHERE id = ? AND status = 'pending'
             AND EXISTS (
               SELECT 1 FROM match_requests m
               WHERE m.id = demand_proposals.match_request_id
                 AND m.user_id = ? AND m.status = 'open'
             )",
        )
        .bind(&proposal_id)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
        if accepted.rows_affected() == 0 {
            return Err(AppError::BadRequest(
                "proposal is no longer available".into(),
            ));
        }
        sqlx::query(
            "UPDATE demand_proposals SET status = 'rejected', updated_at = CURRENT_TIMESTAMP
             WHERE match_request_id = ? AND id != ? AND status = 'pending'",
        )
        .bind(&acceptance.match_request_id)
        .bind(&proposal_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE match_requests SET status = 'following'
             WHERE id = ? AND user_id = ? AND status = 'open'",
        )
        .bind(&acceptance.match_request_id)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO notifications (id, user_id, kind, title, description)
             VALUES (?, ?, 'spark', '推广报价已被接受', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&acceptance.provider_user_id)
        .bind("创作者已接受你的报价，双方现在可以在站内会话中继续沟通。")
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }
    let connection =
        establish_partner_connection(&state, &user, &acceptance.provider_partner_id, None).await?;
    sqlx::query(
        "UPDATE conversations SET context_type = 'match_request', context_id = ?
         WHERE id = ?",
    )
    .bind(&acceptance.match_request_id)
    .bind(&connection.conversation_id)
    .execute(&state.pool)
    .await?;
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "proposal_accepted",
        Some("demand_proposal"),
        Some(&proposal_id),
        json!({
            "matchRequestId": acceptance.match_request_id,
            "providerUserId": acceptance.provider_user_id,
            "conversationId": connection.conversation_id
        }),
    )
    .await;
    Ok(Json(json!({
        "success": true,
        "providerName": acceptance.provider_name,
        "conversationId": connection.conversation_id
    })))
}

async fn close_demand(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(demand_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    require_creator(&user)?;
    let provider_user_ids = sqlx::query_scalar::<_, String>(
        "SELECT provider_user_id FROM demand_proposals
         WHERE match_request_id = ? AND status = 'pending'",
    )
    .bind(&demand_id)
    .fetch_all(&state.pool)
    .await?;
    let mut tx = state.pool.begin().await?;
    let result = sqlx::query(
        "UPDATE match_requests SET status = 'closed'
         WHERE id = ? AND user_id = ? AND status = 'open'",
    )
    .bind(&demand_id)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest("open demand not found".into()));
    }
    sqlx::query(
        "UPDATE demand_proposals SET status = 'rejected', updated_at = CURRENT_TIMESTAMP
         WHERE match_request_id = ? AND status = 'pending'",
    )
    .bind(&demand_id)
    .execute(&mut *tx)
    .await?;
    for provider_user_id in provider_user_ids {
        sqlx::query(
            "INSERT INTO notifications (id, user_id, kind, title, description)
             VALUES (?, ?, 'spark', '推广需求已关闭', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(provider_user_id)
        .bind("创作者已关闭该推广需求，你提交的报价不再参与选择。")
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "success": true })))
}

#[derive(FromRow)]
struct AcceptanceRow {
    match_request_id: String,
    provider_user_id: String,
    proposal_status: String,
    demand_status: String,
    provider_partner_id: String,
    provider_name: String,
}

async fn load_provider_proposal(
    state: &AppState,
    demand_id: &str,
    provider_user_id: &str,
) -> AppResult<ProposalView> {
    Ok(sqlx::query_as::<_, ProposalView>(
        "SELECT dp.id, dp.provider_user_id, u.organization AS provider_name,
         u.avatar AS provider_avatar, dp.amount, dp.cycle, dp.deliverables, dp.message,
         dp.status, dp.created_at, dp.updated_at
         FROM demand_proposals dp
         JOIN users u ON u.id = dp.provider_user_id
         WHERE dp.match_request_id = ? AND dp.provider_user_id = ?",
    )
    .bind(demand_id)
    .bind(provider_user_id)
    .fetch_one(&state.pool)
    .await?)
}

fn validate_proposal(input: &ProposalInput) -> AppResult<()> {
    if input.amount < 100 || input.amount > 100_000_000 {
        return Err(AppError::BadRequest("proposal amount is invalid".into()));
    }
    let cycle = input.cycle.trim();
    if cycle.chars().count() < 2 || cycle.chars().count() > 40 {
        return Err(AppError::BadRequest("proposal cycle is invalid".into()));
    }
    let deliverables = input.deliverables.trim();
    if deliverables.chars().count() < 4 || deliverables.chars().count() > 500 {
        return Err(AppError::BadRequest(
            "proposal deliverables are invalid".into(),
        ));
    }
    if input
        .message
        .as_deref()
        .is_some_and(|message| message.chars().count() > 500)
    {
        return Err(AppError::BadRequest("proposal message is too long".into()));
    }
    Ok(())
}

fn require_approved(user: &UserRow) -> AppResult<()> {
    if user.onboarding_status != "approved" {
        return Err(AppError::BadRequest("onboarding approval required".into()));
    }
    Ok(())
}

fn require_provider(user: &UserRow) -> AppResult<()> {
    if user.role != "provider" {
        return Err(AppError::BadRequest("provider role required".into()));
    }
    Ok(())
}

fn require_creator(user: &UserRow) -> AppResult<()> {
    if user.role != "client" {
        return Err(AppError::BadRequest("creator role required".into()));
    }
    Ok(())
}
