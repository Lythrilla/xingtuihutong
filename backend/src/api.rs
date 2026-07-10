use crate::{
    db,
    error::{AppError, AppResult},
    models::{
        BudgetOption, Certification, ConnectPartner, Conversation, CreateMatch, CreateSession,
        Notification, Partner, Plan, PortfolioCase, SendMessage, Song, TargetType, UpdateProfile,
        UpdateRole, User, UserRow, WithdrawalRequest,
    },
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/session", post(create_session))
        .route("/me/role", put(update_role))
        .route("/home", get(home))
        .route("/plaza", get(plaza))
        .route("/plaza/connect", post(connect_partner))
        .route("/match/bootstrap", get(match_bootstrap))
        .route("/match", post(create_match))
        .route("/ai/plans", get(ai_plans))
        .route("/ai/plans/{id}/save", post(save_plan))
        .route("/messages", get(messages))
        .route("/messages/read-all", post(mark_all_read))
        .route(
            "/conversations/{id}",
            get(conversation_detail).post(send_message),
        )
        .route("/profile", get(profile).put(update_profile))
        .route("/wallet/withdraw", post(withdraw))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    token: String,
    user: User,
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSession>,
) -> AppResult<Json<SessionResponse>> {
    if let Some(token) = bearer_token(&headers) {
        if let Ok(user) = db::user_from_token(&state.pool, token).await {
            return Ok(Json(SessionResponse {
                token: token.into(),
                user: user.into(),
            }));
        }
    }
    let role = input.role.as_deref().unwrap_or("provider");
    let (token, user) = db::create_user_session(&state.pool, role).await?;
    Ok(Json(SessionResponse {
        token,
        user: user.into(),
    }))
}

async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UpdateRole>,
) -> AppResult<Json<User>> {
    let user = current_user(&state, &headers).await?;
    if input.role != "provider" && input.role != "client" {
        return Err(AppError::BadRequest(
            "role must be provider or client".into(),
        ));
    }
    sqlx::query("UPDATE users SET role = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&input.role)
        .bind(&user.id)
        .execute(&state.pool)
        .await?;
    let updated = load_user(&state, &user.id).await?;
    Ok(Json(updated.into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    value: String,
    label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Recommendation {
    id: String,
    avatar: String,
    avatar_class: String,
    title: String,
    subtitle: String,
    score: i64,
    price: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeResponse {
    header_subtitle: String,
    metrics: Vec<Metric>,
    recommendations: Vec<Recommendation>,
}

async fn home(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<HomeResponse>> {
    let user = current_user(&state, &headers).await?;
    let conversation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE user_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await?;
    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(unread_count), 0) FROM conversations WHERE user_id = ?",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let exposure_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM match_requests WHERE user_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await?;
    let settled: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM settlements
         WHERE user_id = ? AND status = 'completed' AND amount > 0",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let metrics = if user.role == "provider" {
        vec![
            Metric {
                value: conversation_count.to_string(),
                label: "合作会话".into(),
            },
            Metric {
                value: unread_count.to_string(),
                label: "待处理消息".into(),
            },
            Metric {
                value: format_money(settled),
                label: "累计收益".into(),
            },
        ]
    } else {
        vec![
            Metric {
                value: exposure_count.to_string(),
                label: "推广任务".into(),
            },
            Metric {
                value: conversation_count.to_string(),
                label: "合作伙伴".into(),
            },
            Metric {
                value: unread_count.to_string(),
                label: "待处理消息".into(),
            },
        ]
    };
    let rows = sqlx::query(
        "SELECT id, avatar, avatar_class, name, description, match_score, result_text
         FROM partners WHERE active = 1 ORDER BY match_score DESC LIMIT 3",
    )
    .fetch_all(&state.pool)
    .await?;
    let recommendations = rows
        .into_iter()
        .map(|row| Recommendation {
            id: row.get("id"),
            avatar: row.get("avatar"),
            avatar_class: row.get("avatar_class"),
            title: row.get("name"),
            subtitle: row.get("description"),
            score: row.get("match_score"),
            price: row.get("result_text"),
        })
        .collect();

    Ok(Json(HomeResponse {
        header_subtitle: if user.role == "provider" {
            "服务方 · 今日合作概览".into()
        } else {
            "被服务方 · 今日推广概览".into()
        },
        metrics,
        recommendations,
    }))
}

#[derive(Deserialize)]
struct PlazaQuery {
    #[serde(rename = "type")]
    partner_type: Option<String>,
}

#[derive(Serialize)]
struct PlazaResponse {
    types: Vec<FilterOption>,
    entries: Vec<Partner>,
}

#[derive(Serialize)]
struct FilterOption {
    key: String,
    label: String,
}

async fn plaza(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlazaQuery>,
) -> AppResult<Json<PlazaResponse>> {
    current_user(&state, &headers).await?;
    let rows = match query.partner_type.as_deref() {
        Some("provider" | "client") => {
            sqlx::query_as::<_, Partner>(
                "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
                 tags AS tags_json, match_score, result_text, active
                 FROM partners WHERE active = 1 AND partner_type = ?
                 ORDER BY match_score DESC",
            )
            .bind(query.partner_type)
            .fetch_all(&state.pool)
            .await?
        }
        Some("latest") => {
            sqlx::query_as::<_, Partner>(
                "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
                 tags AS tags_json, match_score, result_text, active
                 FROM partners WHERE active = 1 ORDER BY created_at DESC",
            )
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query_as::<_, Partner>(
                "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
                 tags AS tags_json, match_score, result_text, active
                 FROM partners WHERE active = 1 ORDER BY match_score DESC",
            )
            .fetch_all(&state.pool)
            .await?
        }
    };
    Ok(Json(PlazaResponse {
        types: vec![
            filter("all", "全部"),
            filter("provider", "服务方"),
            filter("client", "被服务方"),
            filter("latest", "最新"),
        ],
        entries: rows.into_iter().map(with_partner_tags).collect(),
    }))
}

async fn connect_partner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ConnectPartner>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    let partner_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM partners WHERE id = ? AND active = 1")
            .bind(&input.partner_id)
            .fetch_optional(&state.pool)
            .await?;
    let partner_name =
        partner_name.ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    let welcome_message = "你好，欢迎联系我！我们可以先聊聊合作需求。";
    let mut tx = state.pool.begin().await?;
    let result = sqlx::query(
        "INSERT INTO conversations
         (id, user_id, partner_id, last_message, unread_count)
         VALUES (?, ?, ?, ?, 1)
         ON CONFLICT(user_id, partner_id) DO NOTHING",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(&input.partner_id)
    .bind(welcome_message)
    .execute(&mut *tx)
    .await?;
    let created = result.rows_affected() == 1;
    let conversation_id: String =
        sqlx::query_scalar("SELECT id FROM conversations WHERE user_id = ? AND partner_id = ?")
            .bind(&user.id)
            .bind(&input.partner_id)
            .fetch_one(&mut *tx)
            .await?;
    if created {
        sqlx::query(
            "INSERT INTO conversation_messages (id, conversation_id, sender, content)
             VALUES (?, ?, 'partner', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&conversation_id)
        .bind(welcome_message)
        .execute(&mut *tx)
        .await?;
        if user.role == "provider" {
            let amount = 150_000_i64;
            let notification_description = format!("合作服务收益 {} 已到账", format_money(amount));
            sqlx::query(
                "INSERT INTO settlements (id, user_id, title, amount, status)
                 VALUES (?, ?, '合作服务收益', ?, 'completed')",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user.id)
            .bind(amount)
            .execute(&mut *tx)
            .await?;
            let wallet_update = sqlx::query(
                "UPDATE wallets SET balance = balance + ?, updated_at = CURRENT_TIMESTAMP
                 WHERE user_id = ?",
            )
            .bind(amount)
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
            if wallet_update.rows_affected() != 1 {
                return Err(AppError::NotFound("wallet not found".into()));
            }
            sqlx::query(
                "INSERT INTO notifications (id, user_id, kind, title, description)
                 VALUES (?, ?, 'wallet', '合作收益到账', ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user.id)
            .bind(notification_description)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(
        json!({ "conversationId": conversation_id, "partnerName": partner_name }),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchBootstrap {
    songs: Vec<Song>,
    targets: Vec<TargetType>,
    budgets: Vec<BudgetOption>,
}

async fn match_bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<MatchBootstrap>> {
    current_user(&state, &headers).await?;
    let songs = sqlx::query_as::<_, Song>(
        "SELECT id, name, artist, cover_class, active FROM songs
         WHERE active = 1 ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;
    let targets = sqlx::query_as::<_, TargetType>(
        "SELECT key, icon_class, title, description FROM target_types ORDER BY sort_order",
    )
    .fetch_all(&state.pool)
    .await?;
    let budgets = sqlx::query_as::<_, BudgetOption>(
        "SELECT id, label FROM budget_options ORDER BY sort_order",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(MatchBootstrap {
        songs,
        targets,
        budgets,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchResponse {
    match_id: String,
    plans: Vec<Plan>,
}

async fn create_match(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateMatch>,
) -> AppResult<Json<MatchResponse>> {
    let user = current_user(&state, &headers).await?;
    if input.target_keys.is_empty() {
        return Err(AppError::BadRequest(
            "at least one target must be selected".into(),
        ));
    }
    let song_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM songs WHERE id = ? AND active = 1)")
            .bind(&input.song_id)
            .fetch_one(&state.pool)
            .await?;
    let budget_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM budget_options WHERE id = ?)")
            .bind(&input.budget_id)
            .fetch_one(&state.pool)
            .await?;
    if !song_exists || !budget_exists {
        return Err(AppError::BadRequest("invalid song or budget".into()));
    }
    let match_id = Uuid::new_v4().to_string();
    let target_keys = serde_json::to_string(&input.target_keys)
        .map_err(|error| AppError::Internal(error.into()))?;
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO match_requests
         (id, user_id, song_id, target_keys, budget_id)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&match_id)
    .bind(&user.id)
    .bind(&input.song_id)
    .bind(target_keys)
    .bind(&input.budget_id)
    .execute(&mut *tx)
    .await?;
    let plans = sqlx::query_as::<_, Plan>(
        "SELECT id, icon_class, color_class, title, plan_type, description,
         tags AS tags_json, budget_amount, score, active
         FROM plans WHERE active = 1 ORDER BY score DESC LIMIT 3",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (rank, plan) in plans.iter().enumerate() {
        sqlx::query(
            "INSERT INTO match_request_plans (match_request_id, plan_id, rank)
             VALUES (?, ?, ?)",
        )
        .bind(&match_id)
        .bind(&plan.id)
        .bind(rank as i64 + 1)
        .execute(&mut *tx)
        .await?;
    }
    let description = format!("匹配已完成，共生成 {} 个可用方案", plans.len());
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', '智能匹配完成', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(description)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(MatchResponse {
        match_id,
        plans: plans.into_iter().map(with_plan_tags).collect(),
    }))
}

#[derive(Deserialize)]
struct PlanQuery {
    refresh: Option<bool>,
}

#[derive(Serialize)]
struct AiPlansResponse {
    insight: Insight,
    tabs: Vec<FilterOption>,
    plans: Vec<Plan>,
}

#[derive(Serialize)]
struct Insight {
    title: String,
    description: String,
}

async fn ai_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlanQuery>,
) -> AppResult<Json<AiPlansResponse>> {
    current_user(&state, &headers).await?;
    let order = if query.refresh.unwrap_or(false) {
        "RANDOM()"
    } else {
        "score DESC"
    };
    let sql = format!(
        "SELECT id, icon_class, color_class, title, plan_type, description,
         tags AS tags_json, budget_amount, score, active
         FROM plans WHERE active = 1 ORDER BY {order} LIMIT 6"
    );
    let plans: Vec<Plan> = sqlx::query_as::<_, Plan>(&sql)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(with_plan_tags)
        .collect();
    Ok(Json(AiPlansResponse {
        insight: Insight {
            title: "推荐概览".into(),
            description: format!("当前共有 {} 个可用推广方案", plans.len()),
        },
        tabs: vec![filter("plans", "推荐方案")],
        plans,
    }))
}

async fn save_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plans WHERE id = ? AND active = 1)")
            .bind(&id)
            .fetch_one(&state.pool)
            .await?;
    if !exists {
        return Err(AppError::NotFound("plan not found".into()));
    }
    sqlx::query(
        "INSERT INTO saved_plans (user_id, plan_id) VALUES (?, ?)
         ON CONFLICT(user_id, plan_id) DO NOTHING",
    )
    .bind(&user.id)
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "saved": true })))
}

#[derive(Serialize)]
struct MessagesResponse {
    notices: Vec<NotificationView>,
    chats: Vec<ConversationView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NotificationView {
    id: String,
    icon: String,
    icon_class: String,
    title: String,
    desc: String,
    time: String,
    is_read: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationView {
    id: String,
    avatar: String,
    avatar_class: String,
    name: String,
    message: String,
    time: String,
    unread: i64,
}

async fn messages(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<MessagesResponse>> {
    let user = current_user(&state, &headers).await?;
    let notices = sqlx::query_as::<_, Notification>(
        "SELECT id, kind, title, description, is_read, created_at
         FROM notifications
         WHERE user_id = ? OR user_id IS NULL
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|item| NotificationView {
        id: item.id,
        icon: item.kind.clone(),
        icon_class: if item.kind == "wallet" {
            "gold"
        } else {
            "mint"
        }
        .into(),
        title: item.title,
        desc: item.description,
        time: relative_time(&item.created_at),
        is_read: item.is_read,
    })
    .collect();
    let chats = sqlx::query_as::<_, Conversation>(
        "SELECT c.id, c.partner_id, p.avatar, p.avatar_class,
         p.name AS partner_name, c.last_message, c.unread_count, c.updated_at
         FROM conversations c JOIN partners p ON p.id = c.partner_id
         WHERE c.user_id = ? ORDER BY c.updated_at DESC",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|item| ConversationView {
        id: item.id,
        avatar: item.avatar,
        avatar_class: item.avatar_class,
        name: item.partner_name,
        message: item.last_message,
        time: relative_time(&item.updated_at),
        unread: item.unread_count,
    })
    .collect();
    Ok(Json(MessagesResponse { notices, chats }))
}

async fn mark_all_read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    sqlx::query("UPDATE notifications SET is_read = 1 WHERE user_id = ?")
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "success": true })))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct MessageItem {
    id: String,
    sender: String,
    content: String,
    created_at: String,
}

async fn conversation_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<MessageItem>>> {
    let user = current_user(&state, &headers).await?;
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ? AND user_id = ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    if !owned {
        return Err(AppError::NotFound("conversation not found".into()));
    }
    sqlx::query("UPDATE conversations SET unread_count = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    let items = sqlx::query_as::<_, MessageItem>(
        "SELECT id, sender, content, created_at FROM conversation_messages
         WHERE conversation_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<SendMessage>,
) -> AppResult<Json<MessageItem>> {
    let user = current_user(&state, &headers).await?;
    let content = input.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("message content is required".into()));
    }
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ? AND user_id = ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    if !owned {
        return Err(AppError::NotFound("conversation not found".into()));
    }
    let message_id = Uuid::new_v4().to_string();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO conversation_messages (id, conversation_id, sender, content)
         VALUES (?, ?, 'user', ?)",
    )
    .bind(&message_id)
    .bind(&id)
    .bind(content)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE conversations SET last_message = ?, unread_count = 0,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(content)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let item = sqlx::query_as::<_, MessageItem>(
        "SELECT id, sender, content, created_at FROM conversation_messages WHERE id = ?",
    )
    .bind(message_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(item))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileResponse {
    user: User,
    role_label: String,
    stats: Vec<Metric>,
    tags: Vec<String>,
    certifications: Vec<Certification>,
    cases: Vec<PortfolioCase>,
    wallet_balance: i64,
}

async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ProfileResponse>> {
    let user = current_user(&state, &headers).await?;
    profile_payload(&state, user).await.map(Json)
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UpdateProfile>,
) -> AppResult<Json<ProfileResponse>> {
    let user = current_user(&state, &headers).await?;
    let organization = input
        .organization
        .as_deref()
        .unwrap_or(&user.organization)
        .trim();
    let description = input
        .description
        .as_deref()
        .unwrap_or(&user.description)
        .trim();
    if organization.is_empty() {
        return Err(AppError::BadRequest("organization is required".into()));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE users SET organization = ?, description = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(organization)
    .bind(description)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    if let Some(display_name) = input.display_name.as_deref().map(str::trim) {
        if !display_name.is_empty() {
            sqlx::query(
                "UPDATE users SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(display_name)
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
        }
    }
    if let Some(tags) = input.tags {
        sqlx::query("DELETE FROM user_tags WHERE user_id = ?")
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
        for (index, tag) in tags.iter().filter(|tag| !tag.trim().is_empty()).enumerate() {
            sqlx::query("INSERT INTO user_tags (user_id, tag, sort_order) VALUES (?, ?, ?)")
                .bind(&user.id)
                .bind(tag.trim())
                .bind(index as i64 + 1)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    let updated = load_user(&state, &user.id).await?;
    profile_payload(&state, updated).await.map(Json)
}

async fn withdraw(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WithdrawalRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    if input.amount <= 0 {
        return Err(AppError::BadRequest("amount must be positive".into()));
    }
    let mut tx = state.pool.begin().await?;
    let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(&user.id)
        .fetch_one(&mut *tx)
        .await?;
    if balance < input.amount {
        return Err(AppError::BadRequest("insufficient wallet balance".into()));
    }
    sqlx::query(
        "UPDATE wallets SET balance = balance - ?, updated_at = CURRENT_TIMESTAMP
         WHERE user_id = ?",
    )
    .bind(input.amount)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO settlements (id, user_id, title, amount, status)
         VALUES (?, ?, '钱包提现', ?, 'pending')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(-input.amount)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "success": true,
        "balance": balance - input.amount
    })))
}

async fn profile_payload(state: &AppState, user: UserRow) -> AppResult<ProfileResponse> {
    let tags = sqlx::query_scalar::<_, String>(
        "SELECT tag FROM user_tags WHERE user_id = ? ORDER BY sort_order",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await?;
    let certifications = sqlx::query_as::<_, Certification>(
        "SELECT id, title, icon_class, color_class, status
         FROM certifications WHERE user_id = ? ORDER BY rowid",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await?;
    let cases = sqlx::query_as::<_, PortfolioCase>(
        "SELECT id, case_type, name, result_text, color_class
         FROM portfolio_cases WHERE user_id = ? ORDER BY sort_order",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await?;
    let wallet_balance = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(&user.id)
        .fetch_one(&state.pool)
        .await?;
    let conversation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE user_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await?;
    let match_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM match_requests WHERE user_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await?;
    let completed_settlements: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM settlements WHERE user_id = ? AND status = 'completed'",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let stats = vec![
        Metric {
            value: conversation_count.to_string(),
            label: "合作伙伴".into(),
        },
        Metric {
            value: match_count.to_string(),
            label: "智能匹配".into(),
        },
        Metric {
            value: completed_settlements.to_string(),
            label: "完成结算".into(),
        },
    ];
    let role_label = if user.role == "provider" {
        "服务方"
    } else {
        "被服务方"
    }
    .into();
    Ok(ProfileResponse {
        user: user.into(),
        role_label,
        stats,
        tags,
        certifications,
        cases,
        wallet_balance,
    })
}

async fn current_user(state: &AppState, headers: &HeaderMap) -> AppResult<UserRow> {
    let token = bearer_token(headers).ok_or(AppError::Unauthorized)?;
    db::user_from_token(&state.pool, token).await
}

async fn load_user(state: &AppState, id: &str) -> AppResult<UserRow> {
    Ok(sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, organization, role, avatar, description, verified
         FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

fn filter(key: &str, label: &str) -> FilterOption {
    FilterOption {
        key: key.into(),
        label: label.into(),
    }
}

fn with_partner_tags(mut partner: Partner) -> Partner {
    partner.tags = serde_json::from_str(&partner.tags_json).unwrap_or_default();
    partner
}

fn with_plan_tags(mut plan: Plan) -> Plan {
    plan.tags = serde_json::from_str(&plan.tags_json).unwrap_or_default();
    plan
}

fn format_money(cents: i64) -> String {
    format!("¥{:.0}", cents as f64 / 100.0)
}

fn relative_time(value: &str) -> String {
    let Ok(time) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") else {
        return value.into();
    };
    let seconds = (Utc::now().naive_utc() - time).num_seconds();
    match seconds {
        ..=59 => "刚刚".into(),
        60..=3599 => format!("{}分钟前", seconds / 60),
        3600..=86_399 => format!("{}小时前", seconds / 3600),
        86_400..=172_799 => "昨天".into(),
        _ => format!("{}天前", seconds / 86_400),
    }
}
