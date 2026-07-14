use crate::{
    admin::require_admin,
    error::{AppError, AppResult},
    models::{AgentSettings, AgentSettingsInput, AgentTool, AgentToolInput},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route("/tools", get(get_tools).put(update_tools))
        .route("/tools/{name}", put(update_tool))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}/messages", get(get_messages))
        .route("/sessions/{id}/tool_calls", get(get_tool_calls))
        .route("/test", post(test_agent))
}

async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<AgentSettings>> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_settings(&state).await?))
}

async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AgentSettingsInput>,
) -> AppResult<Json<AgentSettings>> {
    require_admin(&state, &headers).await?;
    let default_suggestions = serde_json::to_string(&input.default_suggestions)
        .map_err(|e| AppError::Internal(e.into()))?;
    let follow_up_suggestions = serde_json::to_string(&input.follow_up_suggestions)
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE agent_settings SET
         enabled = ?, engine = ?, model = ?, welcome_message = ?, system_prompt = ?, max_tokens = ?,
         temperature = ?, max_tool_calls = ?, max_history = ?, fallback_reply = ?, suggestion_count = ?,
         default_suggestions = ?, follow_up_suggestions = ?,
         updated_at = CURRENT_TIMESTAMP WHERE id = 'default'",
    )
    .bind(input.enabled)
    .bind(input.engine)
    .bind(input.model)
    .bind(input.welcome_message)
    .bind(input.system_prompt)
    .bind(input.max_tokens)
    .bind(input.temperature)
    .bind(input.max_tool_calls)
    .bind(input.max_history)
    .bind(input.fallback_reply)
    .bind(input.suggestion_count)
    .bind(default_suggestions)
    .bind(follow_up_suggestions)
    .execute(&state.pool)
    .await?;
    Ok(Json(load_settings(&state).await?))
}

async fn get_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AgentTool>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_tools(&state).await?))
}

async fn update_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(inputs): Json<Vec<AgentToolInput>>,
) -> AppResult<Json<Vec<AgentTool>>> {
    require_admin(&state, &headers).await?;
    for input in inputs {
        upsert_tool(&state, &input).await?;
    }
    Ok(Json(load_tools(&state).await?))
}

async fn update_tool(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(input): Json<AgentToolInput>,
) -> AppResult<Json<AgentTool>> {
    require_admin(&state, &headers).await?;
    if name != input.name {
        return Err(AppError::BadRequest("tool name mismatch".into()));
    }
    upsert_tool(&state, &input).await?;
    Ok(Json(load_tool(&state, &name).await?.ok_or_else(|| {
        AppError::NotFound("tool not found".into())
    })?))
}

pub async fn load_settings(state: &AppState) -> AppResult<AgentSettings> {
    let row = sqlx::query_as::<_, AgentSettings>(
        "SELECT id, enabled, engine, model, welcome_message, system_prompt, max_tokens, temperature, max_tool_calls, max_history, fallback_reply, suggestion_count, default_suggestions, follow_up_suggestions
         FROM agent_settings WHERE id = 'default'",
    )
    .fetch_optional(&state.pool)
    .await?;
    if let Some(row) = row {
        return Ok(row);
    }
    sqlx::query("INSERT INTO agent_settings (id) VALUES ('default')")
        .execute(&state.pool)
        .await?;
    sqlx::query_as::<_, AgentSettings>(
        "SELECT id, enabled, engine, model, welcome_message, system_prompt, max_tokens, temperature, max_tool_calls, max_history, fallback_reply, suggestion_count, default_suggestions, follow_up_suggestions
         FROM agent_settings WHERE id = 'default'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.into())
}

async fn load_tools(state: &AppState) -> AppResult<Vec<AgentTool>> {
    Ok(sqlx::query_as::<_, AgentTool>(
        "SELECT name, enabled, label, description, mode, keywords, blocked_keywords, keyword_groups, required_tools, sort_order
         FROM agent_tools ORDER BY sort_order, name",
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn load_tool(state: &AppState, name: &str) -> AppResult<Option<AgentTool>> {
    Ok(sqlx::query_as::<_, AgentTool>(
        "SELECT name, enabled, label, description, mode, keywords, blocked_keywords, keyword_groups, required_tools, sort_order
         FROM agent_tools WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(&state.pool)
    .await?)
}

async fn upsert_tool(state: &AppState, input: &AgentToolInput) -> AppResult<()> {
    let keywords =
        serde_json::to_string(&input.keywords).map_err(|e| AppError::Internal(e.into()))?;
    let blocked_keywords =
        serde_json::to_string(&input.blocked_keywords).map_err(|e| AppError::Internal(e.into()))?;
    let keyword_groups =
        serde_json::to_string(&input.keyword_groups).map_err(|e| AppError::Internal(e.into()))?;
    let required_tools =
        serde_json::to_string(&input.required_tools).map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO agent_tools
         (name, enabled, label, description, mode, keywords, blocked_keywords, keyword_groups, required_tools, sort_order, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(name) DO UPDATE SET
         enabled = excluded.enabled, label = excluded.label, description = excluded.description,
         mode = excluded.mode, keywords = excluded.keywords, blocked_keywords = excluded.blocked_keywords,
         keyword_groups = excluded.keyword_groups, required_tools = excluded.required_tools,
         sort_order = excluded.sort_order, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&input.name)
    .bind(input.enabled)
    .bind(&input.label)
    .bind(&input.description)
    .bind(&input.mode)
    .bind(keywords)
    .bind(blocked_keywords)
    .bind(keyword_groups)
    .bind(required_tools)
    .bind(input.sort_order)
    .execute(&state.pool)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SessionListParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionRow {
    pub id: String,
    pub user_id: String,
    pub user_organization: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionDetail {
    pub id: String,
    pub user_id: String,
    pub user_organization: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolCallRow {
    pub id: String,
    pub tool_name: String,
    pub label: String,
    pub status: String,
    pub result: String,
    pub created_at: String,
}

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SessionListParams>,
) -> AppResult<Json<Vec<AgentSessionRow>>> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query_as::<_, AgentSessionRow>(
        "SELECT s.id, s.user_id, u.organization as user_organization, s.title, s.status, s.created_at, s.updated_at
         FROM agent_sessions s
         LEFT JOIN users u ON u.id = s.user_id
         ORDER BY s.updated_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(params.limit.unwrap_or(50))
    .bind(params.offset.unwrap_or(0))
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<AgentSessionDetail>> {
    require_admin(&state, &headers).await?;
    let row = sqlx::query_as::<_, AgentSessionDetail>(
        "SELECT s.id, s.user_id, u.organization as user_organization, s.title, s.status, s.created_at, s.updated_at
         FROM agent_sessions s
         LEFT JOIN users u ON u.id = s.user_id
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

async fn get_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<AgentMessageRow>>> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query_as::<_, AgentMessageRow>(
        "SELECT id, role, content, created_at FROM agent_messages
         WHERE session_id = ? ORDER BY created_at ASC, rowid ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_tool_calls(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<AgentToolCallRow>>> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query_as::<_, AgentToolCallRow>(
        "SELECT t.id, t.tool_name, COALESCE(at.label, t.tool_name) as label, t.status, t.output_json as result, t.created_at
         FROM agent_tool_calls t
         LEFT JOIN agent_tools at ON at.name = t.tool_name
         WHERE t.session_id = ? ORDER BY t.created_at ASC, t.rowid ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAgentInput {
    pub user_id: String,
    pub prompt: String,
}

async fn test_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestAgentInput>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    let user = sqlx::query_as::<_, crate::models::UserRow>("SELECT * FROM users WHERE id = ?")
        .bind(&input.user_id)
        .fetch_one(&state.pool)
        .await?;
    let response = crate::agent::query_for_user(&state, &user, &input.prompt, None).await?;
    let value = serde_json::to_value(response).map_err(|e| AppError::Internal(e.into()))?;
    Ok(Json(value))
}
