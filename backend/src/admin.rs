use crate::{
    error::{AppError, AppResult},
    models::{
        AdminLogin, BudgetOptionInput, Partner, PartnerInput, Plan, PlanInput, ReviewOnboarding,
        Song, SongInput, TargetTypeInput,
    },
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
use sqlx::{FromRow, Row};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/overview", get(overview))
        .route("/users", get(users))
        .route("/users/{id}", put(update_user))
        .route("/users/{id}/review", put(review_user))
        .route("/users/{id}/notify", post(notify_user))
        .route("/matches", get(matches))
        .route("/matches/{id}", put(update_match))
        .route("/partners", get(partners).post(create_partner))
        .route("/partners/{id}", put(update_partner).delete(delete_partner))
        .route("/songs", get(songs).post(create_song))
        .route("/songs/{id}", put(update_song).delete(delete_song))
        .route("/plans", get(plans).post(create_plan))
        .route("/plans/{id}", put(update_plan).delete(delete_plan))
        .route("/conversations", get(conversations))
        .route("/settlements", get(settlements))
        .route("/settlements/{id}", put(update_settlement))
        .route("/target-types", get(target_types).post(create_target_type))
        .route("/target-types/{key}", put(update_target_type).delete(delete_target_type))
        .route("/budget-options", get(budget_options).post(create_budget_option))
        .route("/budget-options/{id}", put(update_budget_option).delete(delete_budget_option))
        .route("/export/{type}", get(export_csv))
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
    pending_onboarding: i64,
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
    let pending_onboarding = count(
        &state,
        "SELECT COUNT(*) FROM users WHERE onboarding_status = 'pending'",
    )
    .await?;
    let pending_settlements = count(
        &state,
        "SELECT COUNT(*) FROM settlements WHERE status = 'pending'",
    )
    .await?;
    let recent_users = sqlx::query_as::<_, AdminUser>(&admin_users_query(true))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(Overview {
        users: users_count,
        active_partners,
        active_songs,
        active_plans,
        conversations,
        pending_onboarding,
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
    onboarding_status: String,
    contact_name: String,
    contact_method: String,
    application_description: String,
    tags_json: String,
    work_title: String,
    work_url: String,
    work_file_url: String,
    work_file_name: String,
    verification_items_json: String,
    audience_size: String,
    cooperation_budget: String,
    review_note: String,
    submitted_at: String,
    reviewed_at: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAdminUser {
    organization: String,
    contact_name: String,
    contact_method: String,
    application_description: String,
    tags: Vec<String>,
    work_title: String,
    audience_size: String,
    cooperation_budget: String,
}

#[derive(Deserialize)]
struct AdminNotificationInput {
    title: String,
    description: String,
}

async fn users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminUser>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminUser>(&admin_users_query(false))
            .fetch_all(&state.pool)
            .await?,
    ))
}

fn admin_users_query(limited: bool) -> String {
    format!(
        "SELECT u.id, u.organization, u.role, u.avatar, u.verified,
         u.onboarding_status,
         COALESCE(a.contact_name, '') AS contact_name,
         COALESCE(a.contact_method, '') AS contact_method,
         COALESCE(a.description, '') AS application_description,
         COALESCE(a.tags, '[]') AS tags_json,
         COALESCE(a.work_title, '') AS work_title,
         COALESCE(a.work_url, '') AS work_url,
         COALESCE(a.work_file_url, '') AS work_file_url,
         COALESCE(a.work_file_name, '') AS work_file_name,
         COALESCE(a.verification_items, '[]') AS verification_items_json,
         COALESCE(a.audience_size, '') AS audience_size,
         COALESCE(a.cooperation_budget, '') AS cooperation_budget,
         u.review_note,
         COALESCE(a.submitted_at, u.created_at) AS submitted_at,
         a.reviewed_at,
         u.created_at
         FROM users u
         LEFT JOIN onboarding_applications a ON a.user_id = u.id
         ORDER BY CASE WHEN u.onboarding_status = 'pending' THEN 0 ELSE 1 END,
         submitted_at DESC{}",
        if limited { " LIMIT 6" } else { "" }
    )
}

async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateAdminUser>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    let organization = input.organization.trim();
    let contact_name = input.contact_name.trim();
    let contact_method = input.contact_method.trim();
    let description = input.application_description.trim();
    if organization.is_empty() || contact_name.is_empty() || contact_method.is_empty() {
        return Err(AppError::BadRequest(
            "user required fields are missing".into(),
        ));
    }
    let tags: Vec<String> = input
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .take(8)
        .collect();
    let tags_json =
        serde_json::to_string(&tags).map_err(|error| AppError::Internal(error.into()))?;
    let mut tx = state.pool.begin().await?;
    let result = sqlx::query(
        "UPDATE onboarding_applications SET entity_name = ?, contact_name = ?,
         contact_method = ?, description = ?, tags = ?, work_title = ?, audience_size = ?,
         cooperation_budget = ?, updated_at = CURRENT_TIMESTAMP WHERE user_id = ?",
    )
    .bind(organization)
    .bind(contact_name)
    .bind(contact_method)
    .bind(description)
    .bind(&tags_json)
    .bind(input.work_title.trim())
    .bind(input.audience_size.trim())
    .bind(input.cooperation_budget.trim())
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "onboarding application not found".into(),
        ));
    }
    sqlx::query(
        "UPDATE users SET display_name = ?, organization = ?, description = ?,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(organization)
    .bind(organization)
    .bind(description)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_tags WHERE user_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    for (index, tag) in tags.iter().enumerate() {
        sqlx::query("INSERT INTO user_tags (user_id, tag, sort_order) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(tag)
            .bind(index as i64 + 1)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE partners SET name = ?, description = ?, tags = ?,
         updated_at = CURRENT_TIMESTAMP WHERE source_user_id = ?",
    )
    .bind(organization)
    .bind(description)
    .bind(&tags_json)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE songs SET name = ?, artist = ?, updated_at = CURRENT_TIMESTAMP
         WHERE source_user_id = ?",
    )
    .bind(input.work_title.trim())
    .bind(organization)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "success": true })))
}

async fn notify_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<AdminNotificationInput>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    let title = input.title.trim();
    let description = input.description.trim();
    if title.is_empty() || description.is_empty() {
        return Err(AppError::BadRequest(
            "notification content is required".into(),
        ));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = ?)")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound("user not found".into()));
    }
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'shield', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(id)
    .bind(title)
    .bind(description)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "success": true })))
}

async fn review_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<ReviewOnboarding>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    if input.status != "approved" && input.status != "rejected" {
        return Err(AppError::BadRequest("invalid review status".into()));
    }
    let application = sqlx::query(
        "SELECT role, entity_name, description, tags, work_title, audience_size,
         cooperation_budget FROM onboarding_applications WHERE user_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("onboarding application not found".into()))?;
    let role: String = application.get("role");
    let entity_name: String = application.get("entity_name");
    let description: String = application.get("description");
    let tags: String = application.get("tags");
    let work_title: String = application.get("work_title");
    let audience_size: String = application.get("audience_size");
    let cooperation_budget: String = application.get("cooperation_budget");
    let review_note = input.review_note.unwrap_or_else(|| {
        if input.status == "rejected" {
            "资料未通过，请补充真实身份、作品或服务案例后重新提交。".into()
        } else {
            String::new()
        }
    });
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE users SET verified = ?, onboarding_status = ?, review_note = ?,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(input.status == "approved")
    .bind(&input.status)
    .bind(&review_note)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE onboarding_applications SET status = ?, review_note = ?,
         reviewed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE user_id = ?",
    )
    .bind(&input.status)
    .bind(&review_note)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    if input.status == "approved" {
        let avatar = entity_name.chars().next().unwrap_or('星').to_string();
        let identity = if role == "provider" {
            "已审核推广服务方"
        } else {
            "已审核内容创作者"
        };
        let result_text = if role == "provider" {
            if cooperation_budget.is_empty() {
                "可承接推广合作".to_string()
            } else {
                format!("合作预算 {cooperation_budget}")
            }
        } else if audience_size.is_empty() {
            "寻找作品推广合作".to_string()
        } else {
            format!("受众规模 {audience_size}")
        };
        sqlx::query(
            "INSERT INTO partners
             (id, partner_type, avatar, avatar_class, name, identity, description,
              tags, match_score, result_text, active, source_user_id)
             VALUES (?, ?, ?, 'violet', ?, ?, ?, ?, 88, ?, 1, ?)
             ON CONFLICT(source_user_id) DO UPDATE SET
              partner_type = excluded.partner_type,
              avatar = excluded.avatar,
              name = excluded.name,
              identity = excluded.identity,
              description = excluded.description,
              tags = excluded.tags,
              result_text = excluded.result_text,
              active = 1,
              updated_at = CURRENT_TIMESTAMP",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&role)
        .bind(avatar)
        .bind(&entity_name)
        .bind(identity)
        .bind(&description)
        .bind(&tags)
        .bind(result_text)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
        if role == "client" {
            sqlx::query(
                "INSERT INTO songs
                 (id, name, artist, cover_class, active, source_user_id)
                 VALUES (?, ?, ?, 'violet', 1, ?)
                 ON CONFLICT(source_user_id) DO UPDATE SET
                  name = excluded.name,
                  artist = excluded.artist,
                  active = 1,
                  updated_at = CURRENT_TIMESTAMP",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(work_title)
            .bind(&entity_name)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        }
    } else {
        sqlx::query(
            "UPDATE partners SET active = 0, updated_at = CURRENT_TIMESTAMP
             WHERE source_user_id = ?",
        )
        .bind(&id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE songs SET active = 0, updated_at = CURRENT_TIMESTAMP
             WHERE source_user_id = ?",
        )
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    }
    let notification_title = if input.status == "approved" {
        "入驻审核通过"
    } else {
        "入驻资料需补充"
    };
    let notification_description = if input.status == "approved" {
        "你的主页已公开，可以开始真实合作。"
    } else {
        review_note.as_str()
    };
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'shield', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(notification_title)
    .bind(notification_description)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "success": true, "status": input.status })))
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminMatch {
    id: String,
    user_id: String,
    user_name: String,
    song_name: String,
    target_keys_json: String,
    budget_label: String,
    goal: String,
    cycle: String,
    status: String,
    proposal_count: i64,
    accepted_provider_name: Option<String>,
    accepted_amount: Option<i64>,
    created_at: String,
}

#[derive(Deserialize)]
struct UpdateMatchStatus {
    status: String,
}

async fn matches(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminMatch>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminMatch>(
            "SELECT m.id, m.user_id, u.organization AS user_name, s.name AS song_name,
             m.target_keys AS target_keys_json, b.label AS budget_label, m.goal, m.cycle,
             m.status, COUNT(dp.id) AS proposal_count,
             MAX(CASE WHEN dp.status = 'accepted' THEN pu.organization END)
               AS accepted_provider_name,
             MAX(CASE WHEN dp.status = 'accepted' THEN dp.amount END) AS accepted_amount,
             m.created_at
             FROM match_requests m
             JOIN users u ON u.id = m.user_id
             JOIN songs s ON s.id = m.song_id
             JOIN budget_options b ON b.id = m.budget_id
             LEFT JOIN demand_proposals dp
               ON dp.match_request_id = m.id AND dp.status != 'withdrawn'
             LEFT JOIN users pu ON pu.id = dp.provider_user_id
             GROUP BY m.id
             ORDER BY m.created_at DESC",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

async fn update_match(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateMatchStatus>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    if !["open", "completed", "following", "closed"].contains(&input.status.as_str()) {
        return Err(AppError::BadRequest("invalid match status".into()));
    }
    let user_id: Option<String> =
        sqlx::query_scalar("SELECT user_id FROM match_requests WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await?;
    let user_id = user_id.ok_or_else(|| AppError::NotFound("match request not found".into()))?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE match_requests SET status = ? WHERE id = ?")
        .bind(&input.status)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    if input.status == "closed" {
        sqlx::query(
            "UPDATE demand_proposals SET status = 'rejected', updated_at = CURRENT_TIMESTAMP
             WHERE match_request_id = ? AND status = 'pending'",
        )
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    }
    let (title, description) = match input.status.as_str() {
        "open" => ("推广需求已重新开放", "运营团队已重新开放你的推广需求。"),
        "following" => ("推广需求进入跟进", "运营团队已开始跟进你的推广需求。"),
        "closed" => ("推广需求已关闭", "该推广需求已由运营团队关闭。"),
        _ => (
            "推广需求已完成",
            "该推广需求已完成匹配，可继续联系推荐推广方。",
        ),
    };
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(title)
    .bind(description)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "success": true, "status": input.status })))
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

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminTargetType {
    key: String,
    icon_class: String,
    title: String,
    description: String,
    sort_order: i64,
}

async fn target_types(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminTargetType>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminTargetType>(
            "SELECT key, icon_class, title, description, sort_order
             FROM target_types ORDER BY sort_order, key",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

async fn create_target_type(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TargetTypeInput>,
) -> AppResult<Json<AdminTargetType>> {
    require_admin(&state, &headers).await?;
    validate_target_type(&input)?;
    sqlx::query(
        "INSERT INTO target_types (key, icon_class, title, description, sort_order)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&input.key)
    .bind(&input.icon_class)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.sort_order)
    .execute(&state.pool)
    .await?;
    Ok(Json(load_target_type(&state, &input.key).await?))
}

async fn update_target_type(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(input): Json<TargetTypeInput>,
) -> AppResult<Json<AdminTargetType>> {
    require_admin(&state, &headers).await?;
    validate_target_type(&input)?;
    if key != input.key {
        return Err(AppError::BadRequest("target type key cannot be changed".into()));
    }
    let result = sqlx::query(
        "UPDATE target_types SET icon_class = ?, title = ?, description = ?, sort_order = ?
         WHERE key = ?",
    )
    .bind(&input.icon_class)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.sort_order)
    .bind(&key)
    .execute(&state.pool)
    .await?;
    ensure_changed(result.rows_affected(), "target type")?;
    Ok(Json(load_target_type(&state, &key).await?))
}

async fn delete_target_type(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&state, &headers).await?;
    let in_use: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM match_requests WHERE target_keys LIKE ?)",
    )
    .bind(format!("%\"{}\"%", key))
    .fetch_one(&state.pool)
    .await?;
    if in_use {
        return Err(AppError::BadRequest(
            "target type is referenced by match requests".into(),
        ));
    }
    let result = sqlx::query("DELETE FROM target_types WHERE key = ?")
        .bind(&key)
        .execute(&state.pool)
        .await?;
    ensure_changed(result.rows_affected(), "target type")?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_target_type(state: &AppState, key: &str) -> AppResult<AdminTargetType> {
    sqlx::query_as::<_, AdminTargetType>(
        "SELECT key, icon_class, title, description, sort_order
         FROM target_types WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("target type not found".into()))
}

fn validate_target_type(input: &TargetTypeInput) -> AppResult<()> {
    validate_required(&input.key, "key")?;
    validate_required(&input.icon_class, "icon_class")?;
    validate_required(&input.title, "title")?;
    validate_required(&input.description, "description")?;
    Ok(())
}

#[derive(Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct AdminBudgetOption {
    id: String,
    label: String,
    min_amount: Option<i64>,
    max_amount: Option<i64>,
    sort_order: i64,
}

async fn budget_options(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdminBudgetOption>>> {
    require_admin(&state, &headers).await?;
    Ok(Json(
        sqlx::query_as::<_, AdminBudgetOption>(
            "SELECT id, label, min_amount, max_amount, sort_order
             FROM budget_options ORDER BY sort_order, id",
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}

async fn create_budget_option(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BudgetOptionInput>,
) -> AppResult<Json<AdminBudgetOption>> {
    require_admin(&state, &headers).await?;
    validate_budget_option(&input)?;
    sqlx::query(
        "INSERT INTO budget_options (id, label, min_amount, max_amount, sort_order)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&input.id)
    .bind(&input.label)
    .bind(input.min_amount)
    .bind(input.max_amount)
    .bind(input.sort_order)
    .execute(&state.pool)
    .await?;
    Ok(Json(load_budget_option(&state, &input.id).await?))
}

async fn update_budget_option(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<BudgetOptionInput>,
) -> AppResult<Json<AdminBudgetOption>> {
    require_admin(&state, &headers).await?;
    validate_budget_option(&input)?;
    if id != input.id {
        return Err(AppError::BadRequest("budget option id cannot be changed".into()));
    }
    let result = sqlx::query(
        "UPDATE budget_options SET label = ?, min_amount = ?, max_amount = ?, sort_order = ?
         WHERE id = ?",
    )
    .bind(&input.label)
    .bind(input.min_amount)
    .bind(input.max_amount)
    .bind(input.sort_order)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    ensure_changed(result.rows_affected(), "budget option")?;
    Ok(Json(load_budget_option(&state, &id).await?))
}

async fn delete_budget_option(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&state, &headers).await?;
    let in_use: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM match_requests WHERE budget_id = ?)")
            .bind(&id)
            .fetch_one(&state.pool)
            .await?;
    if in_use {
        return Err(AppError::BadRequest(
            "budget option is referenced by match requests".into(),
        ));
    }
    let result = sqlx::query("DELETE FROM budget_options WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    ensure_changed(result.rows_affected(), "budget option")?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_budget_option(state: &AppState, id: &str) -> AppResult<AdminBudgetOption> {
    sqlx::query_as::<_, AdminBudgetOption>(
        "SELECT id, label, min_amount, max_amount, sort_order
         FROM budget_options WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("budget option not found".into()))
}

fn validate_budget_option(input: &BudgetOptionInput) -> AppResult<()> {
    validate_required(&input.id, "id")?;
    validate_required(&input.label, "label")?;
    if let (Some(min), Some(max)) = (input.min_amount, input.max_amount) {
        if min > max {
            return Err(AppError::BadRequest("min amount cannot exceed max amount".into()));
        }
    }
    Ok(())
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

async fn export_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(export_type): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&state, &headers).await?;
    let (filename, content) = match export_type.as_str() {
        "users" => build_users_csv(&state).await?,
        "partners" => build_partners_csv(&state).await?,
        "songs" => build_songs_csv(&state).await?,
        "plans" => build_plans_csv(&state).await?,
        "matches" => build_matches_csv(&state).await?,
        "settlements" => build_settlements_csv(&state).await?,
        "demands" => build_demands_csv(&state).await?,
        _ => return Err(AppError::BadRequest("unknown export type".into())),
    };
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"{filename}\"");
    response_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|error| AppError::Internal(error.into()))?,
    );
    Ok((StatusCode::OK, response_headers, content))
}

fn csv_line(fields: &[&str]) -> String {
    fields.iter().map(|field| csv_escape(field)).collect::<Vec<_>>().join(",") + "\n"
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

async fn build_users_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "display_name", "organization", "role", "verified", "onboarding_status", "created_at"]);
    let rows = sqlx::query(
        "SELECT id, display_name, organization, role, verified, onboarding_status, created_at
         FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("display_name").as_str(),
            row.get::<String, _>("organization").as_str(),
            row.get::<String, _>("role").as_str(),
            if row.get::<i64, _>("verified") == 1 { "是" } else { "否" },
            row.get::<String, _>("onboarding_status").as_str(),
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("users.csv".into(), output))
}

async fn build_partners_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "partner_type", "name", "identity", "description", "tags", "match_score", "result_text", "active", "created_at"]);
    let rows = sqlx::query(
        "SELECT id, partner_type, name, identity, description, tags, match_score, result_text, active, created_at
         FROM partners ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("partner_type").as_str(),
            row.get::<String, _>("name").as_str(),
            row.get::<String, _>("identity").as_str(),
            row.get::<String, _>("description").as_str(),
            row.get::<String, _>("tags").as_str(),
            row.get::<i64, _>("match_score").to_string().as_str(),
            row.get::<String, _>("result_text").as_str(),
            if row.get::<i64, _>("active") == 1 { "是" } else { "否" },
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("partners.csv".into(), output))
}

async fn build_songs_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "name", "artist", "cover_class", "active", "created_at"]);
    let rows = sqlx::query(
        "SELECT id, name, artist, cover_class, active, created_at FROM songs ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("name").as_str(),
            row.get::<String, _>("artist").as_str(),
            row.get::<String, _>("cover_class").as_str(),
            if row.get::<i64, _>("active") == 1 { "是" } else { "否" },
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("songs.csv".into(), output))
}

async fn build_plans_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "title", "plan_type", "description", "tags", "budget_amount", "score", "active", "created_at"]);
    let rows = sqlx::query(
        "SELECT id, title, plan_type, description, tags, budget_amount, score, active, created_at FROM plans ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("title").as_str(),
            row.get::<String, _>("plan_type").as_str(),
            row.get::<String, _>("description").as_str(),
            row.get::<String, _>("tags").as_str(),
            row.get::<i64, _>("budget_amount").to_string().as_str(),
            row.get::<i64, _>("score").to_string().as_str(),
            if row.get::<i64, _>("active") == 1 { "是" } else { "否" },
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("plans.csv".into(), output))
}

async fn build_matches_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "user_name", "song_name", "budget_label", "goal", "cycle", "status", "target_keys", "created_at"]);
    let rows = sqlx::query(
        "SELECT m.id, u.organization AS user_name, s.name AS song_name, b.label AS budget_label,
         m.goal, m.cycle, m.status, m.target_keys, m.created_at
         FROM match_requests m
         JOIN users u ON u.id = m.user_id
         JOIN songs s ON s.id = m.song_id
         JOIN budget_options b ON b.id = m.budget_id
         ORDER BY m.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("user_name").as_str(),
            row.get::<String, _>("song_name").as_str(),
            row.get::<String, _>("budget_label").as_str(),
            row.get::<String, _>("goal").as_str(),
            row.get::<String, _>("cycle").as_str(),
            row.get::<String, _>("status").as_str(),
            row.get::<String, _>("target_keys").as_str(),
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("matches.csv".into(), output))
}

async fn build_settlements_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "user_name", "title", "amount", "status", "created_at"]);
    let rows = sqlx::query(
        "SELECT s.id, u.organization AS user_name, s.title, s.amount, s.status, s.created_at
         FROM settlements s JOIN users u ON u.id = s.user_id
         ORDER BY s.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("user_name").as_str(),
            row.get::<String, _>("title").as_str(),
            row.get::<i64, _>("amount").to_string().as_str(),
            row.get::<String, _>("status").as_str(),
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("settlements.csv".into(), output))
}

async fn build_demands_csv(state: &AppState) -> AppResult<(String, String)> {
    let mut output = csv_line(&["id", "creator_name", "song_name", "budget_label", "goal", "cycle", "status", "target_keys", "proposal_count", "created_at"]);
    let rows = sqlx::query(
        "SELECT m.id, u.organization AS creator_name, s.name AS song_name, b.label AS budget_label,
         m.goal, m.cycle, m.status, m.target_keys,
         (SELECT COUNT(*) FROM demand_proposals dp WHERE dp.match_request_id = m.id AND dp.status != 'withdrawn') AS proposal_count,
         m.created_at
         FROM match_requests m
         JOIN users u ON u.id = m.user_id
         JOIN songs s ON s.id = m.song_id
         JOIN budget_options b ON b.id = m.budget_id
         ORDER BY m.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    for row in rows {
        output.push_str(&csv_line(&[
            row.get::<String, _>("id").as_str(),
            row.get::<String, _>("creator_name").as_str(),
            row.get::<String, _>("song_name").as_str(),
            row.get::<String, _>("budget_label").as_str(),
            row.get::<String, _>("goal").as_str(),
            row.get::<String, _>("cycle").as_str(),
            row.get::<String, _>("status").as_str(),
            row.get::<String, _>("target_keys").as_str(),
            row.get::<i64, _>("proposal_count").to_string().as_str(),
            row.get::<String, _>("created_at").as_str(),
        ]));
    }
    Ok(("demands.csv".into(), output))
}
