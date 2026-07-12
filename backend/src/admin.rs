use crate::{
    error::{AppError, AppResult},
    models::{AdminLogin, Partner, PartnerInput, Plan, PlanInput, Song, SongInput},
    state::AppState,
};
use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/overview", get(overview))
        .route("/users", get(users))
        .route("/partners", get(partners).post(create_partner))
        .route("/partners/{id}", put(update_partner).delete(delete_partner))
        .route("/songs", get(songs).post(create_song))
        .route("/songs/{id}", put(update_song).delete(delete_song))
        .route("/plans", get(plans).post(create_plan))
        .route("/plans/{id}", put(update_plan).delete(delete_plan))
        .route("/conversations", get(conversations))
        .route("/settlements", get(settlements))
        .route("/settlements/{id}", put(update_settlement))
        .nest("/analytics", crate::analytics::admin_routes())
        .nest("/agent", crate::admin_agent::routes())
}

async fn login(
    State(state): State<AppState>,
    Json(input): Json<AdminLogin>,
) -> AppResult<impl IntoResponse> {
    if input.username != state.config.admin_username
        || !verify_password(&input.password, &state.config.admin_password)
    {
        return Err(AppError::Unauthorized);
    }
    let token = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::hours(12))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    sqlx::query("INSERT INTO admin_sessions (token, expires_at) VALUES (?, ?)")
        .bind(&token)
        .bind(expires_at)
        .execute(&state.pool)
        .await?;
    let cookie = format!("admin_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200");
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie)
            .map_err(|error| AppError::Internal(anyhow::Error::from(error)))?,
    );
    Ok((headers, Json(json!({ "success": true }))))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<impl IntoResponse> {
    if let Some(token) = admin_cookie(&headers) {
        sqlx::query("DELETE FROM admin_sessions WHERE token = ?")
            .bind(token)
            .execute(&state.pool)
            .await?;
    }
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_static("admin_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"),
    );
    Ok((response_headers, Json(json!({ "success": true }))))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    users: i64,
    active_partners: i64,
    active_songs: i64,
    active_plans: i64,
    conversations: i64,
    pending_settlements: i64,
    recent_users: Vec<AdminUser>,
}

async fn overview(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Overview>> {
    require_admin(&state, &headers).await?;
    let users_count = count(&state, "SELECT COUNT(*) FROM users").await?;
    let active_partners = count(&state, "SELECT COUNT(*) FROM partners WHERE active = 1").await?;
    let active_songs = count(&state, "SELECT COUNT(*) FROM songs WHERE active = 1").await?;
    let active_plans = count(&state, "SELECT COUNT(*) FROM plans WHERE active = 1").await?;
    let conversations = count(&state, "SELECT COUNT(*) FROM conversations").await?;
    let pending_settlements = count(
        &state,
        "SELECT COUNT(*) FROM settlements WHERE status = 'pending'",
    )
    .await?;
    let recent_users = sqlx::query_as::<_, AdminUser>(
        "SELECT id, organization, role, avatar, verified, created_at
         FROM users ORDER BY created_at DESC LIMIT 6",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Overview {
        users: users_count,
        active_partners,
        active_songs,
        active_plans,
        conversations,
        pending_settlements,
        recent_users,
    }))
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminUser {
    id: String,
    organization: String,
    role: String,
    avatar: String,
    verified: bool,
    created_at: String,
}

async fn users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminUser>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminUser>(
            "SELECT id, organization, role, avatar, verified, created_at
             FROM users ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

async fn partners(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Partner>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_partners(&state).await?))
}

async fn create_partner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PartnerInput>,
) -> AppResult<Json<Partner>> {
    require_admin(&state, &headers).await?;
    validate_partner(&input)?;
    let id = Uuid::new_v4().to_string();
    let tags =
        serde_json::to_string(&input.tags).map_err(|error| AppError::Internal(error.into()))?;
    sqlx::query(
        "INSERT INTO partners
         (id, partner_type, avatar, avatar_class, name, identity, description,
          tags, match_score, result_text, active)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(input.partner_type)
    .bind(input.avatar)
    .bind(input.avatar_class)
    .bind(input.name)
    .bind(input.identity)
    .bind(input.description)
    .bind(tags)
    .bind(input.match_score)
    .bind(input.result_text)
    .bind(input.active)
    .execute(&state.pool)
    .await?;
    Ok(Json(load_partner(&state, &id).await?))
}

async fn update_partner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PartnerInput>,
) -> AppResult<Json<Partner>> {
    require_admin(&state, &headers).await?;
    validate_partner(&input)?;
    let tags =
        serde_json::to_string(&input.tags).map_err(|error| AppError::Internal(error.into()))?;
    let result = sqlx::query(
        "UPDATE partners SET partner_type = ?, avatar = ?, avatar_class = ?,
         name = ?, identity = ?, description = ?, tags = ?, match_score = ?,
         result_text = ?, active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(input.partner_type)
    .bind(input.avatar)
    .bind(input.avatar_class)
    .bind(input.name)
    .bind(input.identity)
    .bind(input.description)
    .bind(tags)
    .bind(input.match_score)
    .bind(input.result_text)
    .bind(input.active)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    ensure_changed(result.rows_affected(), "partner")?;
    Ok(Json(load_partner(&state, &id).await?))
}

async fn delete_partner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&state, &headers).await?;
    let result =
        sqlx::query("UPDATE partners SET active = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await?;
    ensure_changed(result.rows_affected(), "partner")?;
    Ok(StatusCode::NO_CONTENT)
}

async fn songs(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<Song>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_songs(&state).await?))
}

async fn create_song(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SongInput>,
) -> AppResult<Json<Song>> {
    require_admin(&state, &headers).await?;
    validate_required(&input.name, "name")?;
    validate_required(&input.artist, "artist")?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO songs (id, name, artist, cover_class, active) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(input.name)
        .bind(input.artist)
        .bind(input.cover_class)
        .bind(input.active)
        .execute(&state.pool)
        .await?;
    Ok(Json(load_song(&state, &id).await?))
}

async fn update_song(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<SongInput>,
) -> AppResult<Json<Song>> {
    require_admin(&state, &headers).await?;
    validate_required(&input.name, "name")?;
    validate_required(&input.artist, "artist")?;
    let result = sqlx::query(
        "UPDATE songs SET name = ?, artist = ?, cover_class = ?, active = ?,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(input.name)
    .bind(input.artist)
    .bind(input.cover_class)
    .bind(input.active)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    ensure_changed(result.rows_affected(), "song")?;
    Ok(Json(load_song(&state, &id).await?))
}

async fn delete_song(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&state, &headers).await?;
    let result =
        sqlx::query("UPDATE songs SET active = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await?;
    ensure_changed(result.rows_affected(), "song")?;
    Ok(StatusCode::NO_CONTENT)
}

async fn plans(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<Plan>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_plans(&state).await?))
}

async fn create_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PlanInput>,
) -> AppResult<Json<Plan>> {
    require_admin(&state, &headers).await?;
    validate_plan(&input)?;
    let id = Uuid::new_v4().to_string();
    let tags =
        serde_json::to_string(&input.tags).map_err(|error| AppError::Internal(error.into()))?;
    sqlx::query(
        "INSERT INTO plans
         (id, icon_class, color_class, title, plan_type, description, tags,
          budget_amount, score, active)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(input.icon_class)
    .bind(input.color_class)
    .bind(input.title)
    .bind(input.plan_type)
    .bind(input.description)
    .bind(tags)
    .bind(input.budget_amount)
    .bind(input.score)
    .bind(input.active)
    .execute(&state.pool)
    .await?;
    Ok(Json(load_plan(&state, &id).await?))
}

async fn update_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PlanInput>,
) -> AppResult<Json<Plan>> {
    require_admin(&state, &headers).await?;
    validate_plan(&input)?;
    let tags =
        serde_json::to_string(&input.tags).map_err(|error| AppError::Internal(error.into()))?;
    let result = sqlx::query(
        "UPDATE plans SET icon_class = ?, color_class = ?, title = ?,
         plan_type = ?, description = ?, tags = ?, budget_amount = ?, score = ?,
         active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(input.icon_class)
    .bind(input.color_class)
    .bind(input.title)
    .bind(input.plan_type)
    .bind(input.description)
    .bind(tags)
    .bind(input.budget_amount)
    .bind(input.score)
    .bind(input.active)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    ensure_changed(result.rows_affected(), "plan")?;
    Ok(Json(load_plan(&state, &id).await?))
}

async fn delete_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&state, &headers).await?;
    let result =
        sqlx::query("UPDATE plans SET active = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await?;
    ensure_changed(result.rows_affected(), "plan")?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminConversation {
    id: String,
    user_name: String,
    partner_name: String,
    last_message: String,
    unread_count: i64,
    updated_at: String,
}

async fn conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminConversation>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminConversation>(
            "SELECT c.id, u.organization AS user_name, p.name AS partner_name,
             c.last_message, c.unread_count, c.updated_at
             FROM conversations c
             JOIN users u ON u.id = c.user_id
             JOIN partners p ON p.id = c.partner_id
             ORDER BY c.updated_at DESC",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminSettlement {
    id: String,
    user_name: String,
    title: String,
    amount: i64,
    status: String,
    created_at: String,
}

async fn settlements(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminSettlement>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminSettlement>(
            "SELECT s.id, u.organization AS user_name, s.title, s.amount, s.status, s.created_at
             FROM settlements s JOIN users u ON u.id = s.user_id
             ORDER BY s.created_at DESC",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

#[derive(Debug, Deserialize)]
struct SettlementUpdate {
    status: String,
}

async fn update_settlement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<SettlementUpdate>,
) -> AppResult<Json<AdminSettlement>> {
    require_admin(&state, &headers).await?;
    if input.status != "completed" && input.status != "rejected" {
        return Err(AppError::BadRequest(
            "status must be completed or rejected".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let (user_id, amount, current_status): (String, i64, String) =
        sqlx::query_as("SELECT user_id, amount, status FROM settlements WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("settlement not found".into()))?;
    if current_status != "pending" {
        return Err(AppError::BadRequest(
            "settlement is already processed".into(),
        ));
    }
    if amount >= 0 {
        return Err(AppError::BadRequest(
            "only withdrawal settlements can be processed".into(),
        ));
    }

    let result =
        sqlx::query("UPDATE settlements SET status = ? WHERE id = ? AND status = 'pending'")
            .bind(&input.status)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    ensure_changed(result.rows_affected(), "settlement")?;
    if input.status == "rejected" {
        let wallet_update = sqlx::query(
            "UPDATE wallets SET balance = balance - ?, updated_at = CURRENT_TIMESTAMP
             WHERE user_id = ?",
        )
        .bind(amount)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if wallet_update.rows_affected() != 1 {
            return Err(AppError::NotFound("wallet not found".into()));
        }
    }
    tx.commit().await?;
    Ok(Json(load_settlement(&state, &id).await?))
}

async fn load_settlement(state: &AppState, id: &str) -> AppResult<AdminSettlement> {
    Ok(sqlx::query_as::<_, AdminSettlement>(
        "SELECT s.id, u.organization AS user_name, s.title, s.amount, s.status, s.created_at
         FROM settlements s JOIN users u ON u.id = s.user_id
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?)
}

pub(crate) async fn require_admin(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let token = admin_cookie(headers).ok_or(AppError::Unauthorized)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM admin_sessions
           WHERE token = ? AND expires_at > CURRENT_TIMESTAMP
         )",
    )
    .bind(token)
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn admin_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(str::trim)
                .find_map(|cookie| cookie.strip_prefix("admin_session="))
        })
}

fn verify_password(input: &str, configured: &str) -> bool {
    if configured.starts_with("$argon2") {
        return PasswordHash::new(configured).ok().is_some_and(|hash| {
            Argon2::default()
                .verify_password(input.as_bytes(), &hash)
                .is_ok()
        });
    }
    input == configured
}

async fn count(state: &AppState, query: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar(query).fetch_one(&state.pool).await?)
}

fn validate_partner(input: &PartnerInput) -> AppResult<()> {
    if input.partner_type != "provider" && input.partner_type != "client" {
        return Err(AppError::BadRequest("invalid partner type".into()));
    }
    validate_required(&input.name, "name")?;
    validate_required(&input.description, "description")?;
    if !(0..=100).contains(&input.match_score) {
        return Err(AppError::BadRequest(
            "match score must be between 0 and 100".into(),
        ));
    }
    Ok(())
}

fn validate_plan(input: &PlanInput) -> AppResult<()> {
    validate_required(&input.title, "title")?;
    validate_required(&input.description, "description")?;
    if input.budget_amount < 0 || !(0..=100).contains(&input.score) {
        return Err(AppError::BadRequest("invalid budget or score".into()));
    }
    Ok(())
}

fn validate_required(value: &str, field: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{field} is required")));
    }
    Ok(())
}

fn ensure_changed(rows: u64, entity: &str) -> AppResult<()> {
    if rows == 0 {
        return Err(AppError::NotFound(format!("{entity} not found")));
    }
    Ok(())
}

async fn load_partners(state: &AppState) -> AppResult<Vec<Partner>> {
    let rows = sqlx::query_as::<_, Partner>(
        "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
         tags AS tags_json, match_score, result_text, active
         FROM partners ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(with_partner_tags).collect())
}

async fn load_partner(state: &AppState, id: &str) -> AppResult<Partner> {
    let row = sqlx::query_as::<_, Partner>(
        "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
         tags AS tags_json, match_score, result_text, active
         FROM partners WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    Ok(with_partner_tags(row))
}

async fn load_songs(state: &AppState) -> AppResult<Vec<Song>> {
    Ok(sqlx::query_as::<_, Song>(
        "SELECT id, name, artist, cover_class, active FROM songs ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn load_song(state: &AppState, id: &str) -> AppResult<Song> {
    sqlx::query_as::<_, Song>(
        "SELECT id, name, artist, cover_class, active FROM songs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("song not found".into()))
}

async fn load_plans(state: &AppState) -> AppResult<Vec<Plan>> {
    let rows = sqlx::query_as::<_, Plan>(
        "SELECT id, icon_class, color_class, title, plan_type, description,
         tags AS tags_json, budget_amount, score, active
         FROM plans ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(with_plan_tags).collect())
}

async fn load_plan(state: &AppState, id: &str) -> AppResult<Plan> {
    let row = sqlx::query_as::<_, Plan>(
        "SELECT id, icon_class, color_class, title, plan_type, description,
         tags AS tags_json, budget_amount, score, active
         FROM plans WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("plan not found".into()))?;
    Ok(with_plan_tags(row))
}

fn with_partner_tags(mut partner: Partner) -> Partner {
    partner.tags = serde_json::from_str(&partner.tags_json).unwrap_or_default();
    partner
}

fn with_plan_tags(mut plan: Plan) -> Plan {
    plan.tags = serde_json::from_str(&plan.tags_json).unwrap_or_default();
    plan
}
