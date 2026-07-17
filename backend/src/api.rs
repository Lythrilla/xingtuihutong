use crate::{
    db,
    error::{AppError, AppResult},
    models::{
        BudgetOption, Certification, ConnectPartner, CreateMatch, CreateSession, Notification,
        Partner, Plan, PortfolioCase, SendMessage, Song, SubmitOnboarding, TargetType,
        UpdateProfile, UpdateRole, User, UserRow, WithdrawalRequest,
    },
    state::AppState,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/session", post(create_session))
        .route("/me/role", put(update_role))
        .route("/onboarding", get(onboarding).put(submit_onboarding))
        .route("/uploads/work", post(upload_work))
        .route("/uploads/avatar", post(upload_avatar))
        .route("/home", get(home))
        .route("/plaza", get(plaza))
        .route("/partners/{id}", get(partner_detail))
        .route("/partners/{id}/unlock", post(unlock_partner_contact))
        .route("/plaza/connect", post(connect_partner))
        .route("/match/bootstrap", get(match_bootstrap))
        .route("/match", post(create_match))
        .nest("/demands", crate::demands::routes())
        .route("/ai/plans", get(ai_plans))
        .route("/ai/plans/{id}/save", post(save_plan))
        .nest("/agent", crate::agent::routes())
        .nest("/analytics", crate::analytics::user_routes())
        .route("/messages", get(messages))
        .route("/messages/read-all", post(mark_all_read))
        .route(
            "/conversations/{id}",
            get(conversation_detail).post(send_message),
        )
        .route("/profile", get(profile).put(update_profile))
        .route("/wallet/withdraw", post(withdraw))
        .route("/membership/plans", get(membership_plans))
        .route("/membership/purchase", post(purchase_membership))
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
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "session_created",
        Some("user"),
        Some(&user.id),
        json!({ "role": &user.role }),
    )
    .await;
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
    if input.role == user.role {
        return Ok(Json(user.into()));
    }
    if input.role != user.role && user.onboarding_status != "draft" {
        return Err(AppError::BadRequest(
            "submitted role cannot be changed".into(),
        ));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE users SET role = ?, verified = 0, onboarding_status = 'draft',
         review_note = '', updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&input.role)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    if input.role != user.role {
        sqlx::query("DELETE FROM onboarding_applications WHERE user_id = ?")
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE partners SET active = 0, updated_at = CURRENT_TIMESTAMP
             WHERE source_user_id = ?",
        )
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE songs SET active = 0, updated_at = CURRENT_TIMESTAMP
             WHERE source_user_id = ?",
        )
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let updated = load_user(&state, &user.id).await?;
    Ok(Json(updated.into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OnboardingApplicationView {
    entity_name: String,
    contact_name: String,
    contact_method: String,
    description: String,
    tags: Vec<String>,
    work_title: String,
    work_url: String,
    work_file_url: String,
    work_file_name: String,
    verification_items: Vec<String>,
    audience_size: String,
    cooperation_budget: String,
    status: String,
    review_note: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OnboardingResponse {
    role: String,
    status: String,
    review_note: String,
    application: Option<OnboardingApplicationView>,
}

async fn onboarding(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<OnboardingResponse>> {
    let user = current_user(&state, &headers).await?;
    let row = sqlx::query(
        "SELECT entity_name, contact_name, contact_method, description, tags,
         work_title, work_url, work_file_url, work_file_name, verification_items,
         audience_size, cooperation_budget, status, review_note
         FROM onboarding_applications WHERE user_id = ?",
    )
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?;
    let application = row.map(|row| OnboardingApplicationView {
        entity_name: row.get("entity_name"),
        contact_name: row.get("contact_name"),
        contact_method: row.get("contact_method"),
        description: row.get("description"),
        tags: serde_json::from_str(&row.get::<String, _>("tags")).unwrap_or_default(),
        work_title: row.get("work_title"),
        work_url: row.get("work_url"),
        work_file_url: row.get("work_file_url"),
        work_file_name: row.get("work_file_name"),
        verification_items: serde_json::from_str(&row.get::<String, _>("verification_items"))
            .unwrap_or_default(),
        audience_size: row.get("audience_size"),
        cooperation_budget: row.get("cooperation_budget"),
        status: row.get("status"),
        review_note: row.get("review_note"),
    });
    Ok(Json(OnboardingResponse {
        role: user.role,
        status: user.onboarding_status,
        review_note: user.review_note,
        application,
    }))
}

async fn submit_onboarding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SubmitOnboarding>,
) -> AppResult<Json<OnboardingResponse>> {
    let user = current_user(&state, &headers).await?;
    let entity_name = input.entity_name.trim();
    let contact_name = input.contact_name.trim();
    let contact_method = input.contact_method.trim();
    let description = input.description.trim();
    let work_title = input.work_title.unwrap_or_default().trim().to_string();
    let work_url = input.work_url.unwrap_or_default().trim().to_string();
    let work_file_url = input.work_file_url.unwrap_or_default().trim().to_string();
    let work_file_name = input.work_file_name.unwrap_or_default().trim().to_string();
    let verification_items = input.verification_items.unwrap_or_default();
    let audience_size = input.audience_size.unwrap_or_default().trim().to_string();
    let cooperation_budget = input
        .cooperation_budget
        .unwrap_or_default()
        .trim()
        .to_string();
    let avatar = input.avatar.unwrap_or_default().trim().to_string();
    if entity_name.is_empty()
        || contact_name.is_empty()
        || contact_method.is_empty()
        || (user.role == "provider" && description.is_empty())
    {
        return Err(AppError::BadRequest(
            "onboarding required fields are missing".into(),
        ));
    }
    if user.role == "client" && work_file_url.is_empty() && work_url.is_empty() {
        return Err(AppError::BadRequest(
            "creator work information is required".into(),
        ));
    }
    if user.role == "client"
        && !["ownership", "publishable", "authentic"]
            .iter()
            .all(|item| verification_items.iter().any(|selected| selected == item))
    {
        return Err(AppError::BadRequest(
            "creator verification checklist is required".into(),
        ));
    }
    let work_title = if user.role == "client" && work_title.is_empty() {
        std::path::Path::new(&work_file_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("上传作品")
            .to_string()
    } else {
        work_title
    };
    let description = if user.role == "client" && description.is_empty() {
        format!("内容创作者，代表作品：{work_title}")
    } else {
        description.to_string()
    };
    let tags: Vec<String> = input
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .take(8)
        .collect();
    if tags.is_empty() {
        return Err(AppError::BadRequest(
            "at least one specialty is required".into(),
        ));
    }
    let tags_json =
        serde_json::to_string(&tags).map_err(|error| AppError::Internal(error.into()))?;
    let verification_items_json = serde_json::to_string(&verification_items)
        .map_err(|error| AppError::Internal(error.into()))?;
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO onboarding_applications
         (user_id, role, entity_name, contact_name, contact_method, description, tags,
          work_title, work_url, work_file_url, work_file_name, verification_items,
          audience_size, cooperation_budget, status, review_note)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', '')
         ON CONFLICT(user_id) DO UPDATE SET
          role = excluded.role,
          entity_name = excluded.entity_name,
          contact_name = excluded.contact_name,
          contact_method = excluded.contact_method,
          description = excluded.description,
          tags = excluded.tags,
          work_title = excluded.work_title,
          work_url = excluded.work_url,
          work_file_url = excluded.work_file_url,
          work_file_name = excluded.work_file_name,
          verification_items = excluded.verification_items,
          audience_size = excluded.audience_size,
          cooperation_budget = excluded.cooperation_budget,
          status = 'pending',
          review_note = '',
          submitted_at = CURRENT_TIMESTAMP,
          reviewed_at = NULL,
          updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&user.id)
    .bind(&user.role)
    .bind(entity_name)
    .bind(contact_name)
    .bind(contact_method)
    .bind(&description)
    .bind(tags_json)
    .bind(work_title)
    .bind(work_url)
    .bind(work_file_url)
    .bind(work_file_name)
    .bind(verification_items_json)
    .bind(audience_size)
    .bind(cooperation_budget)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE users SET display_name = ?, organization = ?, description = ?,
         avatar = COALESCE(NULLIF(?, ''), avatar), verified = 0, onboarding_status = 'pending',
         review_note = '', updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(entity_name)
    .bind(entity_name)
    .bind(description)
    .bind(&avatar)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_tags WHERE user_id = ?")
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    for (index, tag) in tags.iter().enumerate() {
        sqlx::query("INSERT INTO user_tags (user_id, tag, sort_order) VALUES (?, ?, ?)")
            .bind(&user.id)
            .bind(tag)
            .bind(index as i64 + 1)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE partners SET active = 0, updated_at = CURRENT_TIMESTAMP
         WHERE source_user_id = ?",
    )
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE songs SET active = 0, updated_at = CURRENT_TIMESTAMP
         WHERE source_user_id = ?",
    )
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    onboarding(State(state), headers).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkUploadResponse {
    url: String,
    file_name: String,
}

async fn upload_work(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> AppResult<Json<WorkUploadResponse>> {
    let _user = current_user(&state, &headers).await?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("invalid upload payload".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field.file_name().unwrap_or("work").to_string();
        let content_type = field.content_type().unwrap_or("").to_string();
        let extension = upload_extension(&file_name, &content_type)
            .ok_or_else(|| AppError::BadRequest("unsupported work file type".into()))?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::BadRequest("invalid upload payload".into()))?;
        if bytes.is_empty() || bytes.len() > 30 * 1024 * 1024 {
            return Err(AppError::BadRequest("work file size is invalid".into()));
        }
        let stored_name = format!("{}.{}", Uuid::new_v4(), extension);
        tokio::fs::write(format!("data/uploads/{stored_name}"), bytes)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        return Ok(Json(WorkUploadResponse {
            url: format!("/uploads/{stored_name}"),
            file_name,
        }));
    }
    Err(AppError::BadRequest("work file is required".into()))
}

async fn upload_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> AppResult<Json<WorkUploadResponse>> {
    let _user = current_user(&state, &headers).await?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("invalid upload payload".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field.file_name().unwrap_or("avatar").to_string();
        let content_type = field.content_type().unwrap_or("").to_string();
        let extension = match content_type.as_str() {
            "image/jpeg" | "image/jpg" => Some("jpg"),
            "image/png" => Some("png"),
            "image/webp" => Some("webp"),
            _ => match std::path::Path::new(&file_name)
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("jpg") | Some("jpeg") => Some("jpg"),
                Some("png") => Some("png"),
                Some("webp") => Some("webp"),
                _ => None,
            },
        }
        .ok_or_else(|| AppError::BadRequest("unsupported avatar image type".into()))?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::BadRequest("invalid upload payload".into()))?;
        if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
            return Err(AppError::BadRequest("avatar image must be <= 2MB".into()));
        }
        let stored_name = format!("{}.{}", Uuid::new_v4(), extension);
        tokio::fs::write(format!("data/uploads/{stored_name}"), bytes)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        return Ok(Json(WorkUploadResponse {
            url: format!("/uploads/{stored_name}"),
            file_name,
        }));
    }
    Err(AppError::BadRequest("avatar image is required".into()))
}

fn upload_extension(file_name: &str, content_type: &str) -> Option<&'static str> {
    match content_type {
        "audio/mpeg" => Some("mp3"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/mp4" | "audio/x-m4a" => Some("m4a"),
        "video/mp4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        _ => match std::path::Path::new(file_name)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("mp3") => Some("mp3"),
            Some("wav") => Some("wav"),
            Some("m4a") => Some("m4a"),
            Some("mp4") => Some("mp4"),
            Some("mov") => Some("mov"),
            Some("jpg") | Some("jpeg") => Some("jpg"),
            Some("png") => Some("png"),
            _ => None,
        },
    }
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
    verified: bool,
    preferred: bool,
    title: String,
    subtitle: String,
    score: String,
    price: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeResponse {
    header_subtitle: String,
    name: String,
    role: String,
    onboarding_status: String,
    status_title: String,
    status_description: String,
    metrics: Vec<Metric>,
    recommendations: Vec<Recommendation>,
}

async fn home(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<HomeResponse>> {
    let user = current_user(&state, &headers).await?;
    let conversation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversations WHERE user_id = ? OR participant_user_id = ?",
    )
    .bind(&user.id)
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(unread_count), 0)
         FROM conversation_read_states WHERE user_id = ?",
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
    let opposite_role = if user.role == "provider" {
        "client"
    } else {
        "provider"
    };
    let rows = sqlx::query(
        "SELECT id, avatar, avatar_class, name, description, match_score, result_text
         FROM partners WHERE active = 1 AND partner_type = ?
         ORDER BY match_score DESC LIMIT 3",
    )
    .bind(opposite_role)
    .fetch_all(&state.pool)
    .await?;
    let recommendations: Vec<Recommendation> = rows
        .into_iter()
        .map(|row| Recommendation {
            id: row.get("id"),
            avatar: row.get("avatar"),
            avatar_class: row.get("avatar_class"),
            verified: true,
            preferred: true,
            title: row.get("name"),
            subtitle: row.get("description"),
            score: row.get::<i64, _>("match_score").to_string(),
            price: row.get("result_text"),
        })
        .collect();
    let (status_title, status_description) = match user.onboarding_status.as_str() {
        "approved" => (
            "入驻已通过".into(),
            if user.role == "provider" {
                "现在可以发现创作者项目并发起真实合作。".into()
            } else {
                "现在可以发布作品推广需求并匹配已审核推广方。".into()
            },
        ),
        "pending" => (
            "入驻资料审核中".into(),
            "平台正在核验身份与合作资料，审核通过前不会公开展示。".into(),
        ),
        "rejected" => (
            "入驻资料需要补充".into(),
            if user.review_note.is_empty() {
                "请修改资料后重新提交审核。".into()
            } else {
                user.review_note.clone()
            },
        ),
        _ => (
            "完成入驻后再开始合作".into(),
            "先提交真实身份与业务资料，审核通过后进入对应工作台。".into(),
        ),
    };

    Ok(Json(HomeResponse {
        header_subtitle: if user.role == "provider" {
            "推广服务方工作台".into()
        } else {
            "内容创作者工作台".into()
        },
        name: user.organization.clone(),
        role: user.role.clone(),
        onboarding_status: user.onboarding_status.clone(),
        status_title,
        status_description,
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
#[serde(rename_all = "camelCase")]
struct PlazaResponse {
    types: Vec<FilterOption>,
    entries: Vec<Partner>,
    role: String,
    onboarding_status: String,
    can_connect: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PartnerDetailResponse {
    partner: Partner,
    verification_items: Vec<String>,
    contact_preview: String,
    contact_method: String,
    contact_available: bool,
    unlocked: bool,
    reviewed_at: String,
    role: String,
    onboarding_status: String,
    can_connect: bool,
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
    let user = current_user(&state, &headers).await?;
    let opposite_role = if user.role == "provider" {
        "client"
    } else {
        "provider"
    };
    let rows = match query.partner_type.as_deref() {
        Some("latest") => {
            sqlx::query_as::<_, Partner>(
                "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
                 tags AS tags_json, match_score, result_text, active
                 FROM partners WHERE active = 1 AND partner_type = ?
                 ORDER BY created_at DESC",
            )
            .bind(opposite_role)
            .fetch_all(&state.pool)
            .await?
        }
        _ => {
            sqlx::query_as::<_, Partner>(
                "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
                 tags AS tags_json, match_score, result_text, active
                 FROM partners WHERE active = 1 AND partner_type = ?
                 ORDER BY match_score DESC",
            )
            .bind(opposite_role)
            .fetch_all(&state.pool)
            .await?
        }
    };
    Ok(Json(PlazaResponse {
        types: vec![filter("all", "全部"), filter("latest", "最新")],
        entries: rows.into_iter().map(with_partner_tags).collect(),
        role: user.role.clone(),
        onboarding_status: user.onboarding_status.clone(),
        can_connect: user.onboarding_status == "approved",
    }))
}

async fn partner_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<PartnerDetailResponse>> {
    let user = current_user(&state, &headers).await?;
    let opposite_role = if user.role == "provider" {
        "client"
    } else {
        "provider"
    };
    let partner = sqlx::query_as::<_, Partner>(
        "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
         tags AS tags_json, match_score, result_text, active
         FROM partners WHERE id = ? AND active = 1 AND partner_type = ?",
    )
    .bind(&id)
    .bind(opposite_role)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    let application = sqlx::query(
        "SELECT oa.contact_method, COALESCE(oa.reviewed_at, oa.updated_at) AS reviewed_at
         FROM partners p
         JOIN onboarding_applications oa ON oa.user_id = p.source_user_id
         WHERE p.id = ? AND oa.status = 'approved'",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?;
    let (contact_preview, contact_method, reviewed_at, contact_available) = application
        .map(|row| {
            let contact_method: String = row.get("contact_method");
            (
                describe_contact_method(&contact_method),
                contact_method.clone(),
                row.get::<String, _>("reviewed_at"),
                true,
            )
        })
        .unwrap_or_else(|| {
            (
                "该主页暂未配置可解锁联系方式".into(),
                String::new(),
                String::new(),
                false,
            )
        });
    let source_user_id: Option<String> =
        sqlx::query_scalar("SELECT source_user_id FROM partners WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await?;
    let is_owner = source_user_id.as_deref() == Some(&user.id);
    let membership_active = membership_active(&state.pool, &user.id).await?;
    let has_unlock = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM contact_unlocks WHERE user_id = ? AND partner_id = ?",
    )
    .bind(&user.id)
    .bind(&id)
    .fetch_one(&state.pool)
    .await?
        > 0;
    let unlocked = contact_available && (is_owner || membership_active || has_unlock);
    let verification_items = if partner.partner_type == "provider" && contact_available {
        vec![
            "平台入驻已审核".into(),
            "服务资料可追溯".into(),
            "联系方式已留存".into(),
        ]
    } else if partner.partner_type == "client" && contact_available {
        vec![
            "创作者入驻已审核".into(),
            "代表作品已提交".into(),
            "联系方式已留存".into(),
        ]
    } else {
        vec!["平台公开资料已审核".into(), "主页当前处于可展示状态".into()]
    };

    Ok(Json(PartnerDetailResponse {
        partner: with_partner_tags(partner),
        verification_items,
        contact_preview: if unlocked {
            contact_method.clone()
        } else {
            contact_preview
        },
        contact_method: if unlocked { contact_method } else { String::new() },
        contact_available,
        unlocked,
        reviewed_at,
        role: user.role.clone(),
        onboarding_status: user.onboarding_status.clone(),
        can_connect: user.onboarding_status == "approved",
    }))
}

async fn connect_partner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ConnectPartner>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    let connection = establish_partner_connection(&state, &user, &input.partner_id, None).await?;
    Ok(Json(json!({
        "conversationId": connection.conversation_id,
        "partnerName": connection.partner_name
    })))
}

pub(crate) struct PartnerConnection {
    pub conversation_id: String,
    pub partner_name: String,
    pub created: bool,
}

pub(crate) async fn establish_partner_connection(
    state: &AppState,
    user: &UserRow,
    partner_id: &str,
    agent_session_id: Option<&str>,
) -> AppResult<PartnerConnection> {
    require_approved(user)?;
    let partner: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, source_user_id FROM partners
             WHERE id = ? AND active = 1 AND partner_type != ?",
    )
    .bind(partner_id)
    .bind(&user.role)
    .fetch_optional(&state.pool)
    .await?;
    let (partner_name, participant_user_id) =
        partner.ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    let welcome_message = "双方会话已建立，可以在这里继续沟通合作细节。";
    let mut tx = state.pool.begin().await?;
    let existing_id = if let Some(participant_user_id) = participant_user_id.as_deref() {
        sqlx::query_scalar(
            "SELECT id FROM conversations
             WHERE (user_id = ? AND participant_user_id = ?)
                OR (user_id = ? AND participant_user_id = ?)
             ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(&user.id)
        .bind(participant_user_id)
        .bind(participant_user_id)
        .bind(&user.id)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query_scalar("SELECT id FROM conversations WHERE user_id = ? AND partner_id = ?")
            .bind(&user.id)
            .bind(partner_id)
            .fetch_optional(&mut *tx)
            .await?
    };
    let created = existing_id.is_none();
    let conversation_id = existing_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    if created {
        sqlx::query(
            "INSERT INTO conversations
             (id, user_id, partner_id, participant_user_id, last_message, unread_count)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(&conversation_id)
        .bind(&user.id)
        .bind(partner_id)
        .bind(&participant_user_id)
        .bind(welcome_message)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO conversation_messages
             (id, conversation_id, sender, sender_user_id, content)
             VALUES (?, ?, 'system', NULL, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&conversation_id)
        .bind(welcome_message)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO conversation_read_states
             (conversation_id, user_id, unread_count, last_read_at)
             VALUES (?, ?, 0, CURRENT_TIMESTAMP)",
        )
        .bind(&conversation_id)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
        if let Some(participant_user_id) = participant_user_id.as_deref() {
            sqlx::query(
                "INSERT INTO conversation_read_states
                 (conversation_id, user_id, unread_count)
                 VALUES (?, ?, 1)",
            )
            .bind(&conversation_id)
            .bind(participant_user_id)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(session_id) = agent_session_id {
            sqlx::query(
                "INSERT INTO agent_actions
                 (id, session_id, user_id, action_type, title, payload)
                 VALUES (?, ?, ?, 'connect_partner', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(&user.id)
            .bind(format!("联系{partner_name}"))
            .bind(
                json!({
                    "conversationId": conversation_id,
                    "partnerId": partner_id
                })
                .to_string(),
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    let _ = crate::analytics::track_event(
        state,
        Some(&user.id),
        "partner_connected",
        Some("conversation"),
        Some(&conversation_id),
        json!({ "partnerId": partner_id, "created": created }),
    )
    .await;
    Ok(PartnerConnection {
        conversation_id,
        partner_name,
        created,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchBootstrap {
    songs: Vec<Song>,
    targets: Vec<TargetType>,
    budgets: Vec<BudgetOption>,
    available_providers: i64,
}

async fn match_bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<MatchBootstrap>> {
    let user = current_user(&state, &headers).await?;
    require_creator(&user)?;
    require_approved(&user)?;
    let songs = sqlx::query_as::<_, Song>(
        "SELECT id, name, artist, cover_class, active FROM songs
         WHERE active = 1 AND source_user_id = ? ORDER BY created_at",
    )
    .bind(&user.id)
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
    let available_providers = sqlx::query_scalar(
        "SELECT COUNT(*) FROM partners WHERE active = 1 AND partner_type = 'provider'",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(MatchBootstrap {
        songs,
        targets,
        budgets,
        available_providers,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchResponse {
    match_id: String,
    partners: Vec<Partner>,
}

async fn create_match(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateMatch>,
) -> AppResult<Json<MatchResponse>> {
    let user = current_user(&state, &headers).await?;
    require_creator(&user)?;
    require_approved(&user)?;
    if input.target_keys.is_empty() {
        return Err(AppError::BadRequest(
            "at least one target must be selected".into(),
        ));
    }
    let goal = input.goal.trim();
    if goal.chars().count() < 8 || goal.chars().count() > 300 {
        return Err(AppError::BadRequest(
            "goal must contain between 8 and 300 characters".into(),
        ));
    }
    if !["7 天", "14 天", "30 天", "60 天"].contains(&input.cycle.as_str()) {
        return Err(AppError::BadRequest("invalid campaign cycle".into()));
    }
    let song_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM songs WHERE id = ? AND active = 1 AND source_user_id = ?
             )",
    )
    .bind(&input.song_id)
    .bind(&user.id)
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
         (id, user_id, song_id, target_keys, budget_id, goal, cycle, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'open')",
    )
    .bind(&match_id)
    .bind(&user.id)
    .bind(&input.song_id)
    .bind(target_keys)
    .bind(&input.budget_id)
    .bind(goal)
    .bind(&input.cycle)
    .execute(&mut *tx)
    .await?;
    let partners = sqlx::query_as::<_, Partner>(
        "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
         tags AS tags_json, match_score, result_text, active
         FROM partners WHERE active = 1 AND partner_type = 'provider'
         ORDER BY match_score DESC LIMIT 6",
    )
    .fetch_all(&mut *tx)
    .await?;
    if partners.is_empty() {
        return Err(AppError::BadRequest(
            "no approved providers available".into(),
        ));
    }
    let description = format!(
        "需求已进入大厅，同时为你推荐了 {} 位已审核推广方",
        partners.len()
    );
    sqlx::query(
        "INSERT INTO notifications (id, user_id, kind, title, description)
         VALUES (?, ?, 'spark', '推广需求已发布', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(description)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "match_created",
        Some("match_request"),
        Some(&match_id),
        json!({
            "songId": input.song_id,
            "budgetId": input.budget_id,
            "targets": input.target_keys,
            "goal": goal,
            "cycle": input.cycle,
            "partnerCount": partners.len()
        }),
    )
    .await;
    Ok(Json(MatchResponse {
        match_id,
        partners: partners.into_iter().map(with_partner_tags).collect(),
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
    if user.role != "provider" {
        return Err(AppError::BadRequest(
            "creator wallet is not available".into(),
        ));
    }
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
    .bind(&id)
    .execute(&state.pool)
    .await?;
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "plan_saved",
        Some("plan"),
        Some(&id),
        json!({}),
    )
    .await;
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

#[derive(sqlx::FromRow)]
struct ConversationListRow {
    id: String,
    avatar: String,
    avatar_class: String,
    partner_name: String,
    last_message: String,
    unread_count: i64,
    updated_at: String,
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
    let chats = sqlx::query_as::<_, ConversationListRow>(
        "SELECT c.id,
         CASE WHEN c.user_id = ? THEN target.avatar ELSE owner.avatar END AS avatar,
         CASE WHEN c.user_id = ? THEN target.avatar_class ELSE owner.avatar_class END
           AS avatar_class,
         CASE WHEN c.user_id = ? THEN target.name ELSE owner.name END AS partner_name,
         c.last_message,
         COALESCE(rs.unread_count, CASE WHEN c.user_id = ? THEN c.unread_count ELSE 0 END)
           AS unread_count,
         c.updated_at
         FROM conversations c
         JOIN partners target ON target.id = c.partner_id
         LEFT JOIN partners owner ON owner.source_user_id = c.user_id AND owner.active = 1
         LEFT JOIN conversation_read_states rs
           ON rs.conversation_id = c.id AND rs.user_id = ?
         WHERE c.user_id = ? OR c.participant_user_id = ?
         ORDER BY c.updated_at DESC",
    )
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
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
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE notifications SET is_read = 1 WHERE user_id = ?")
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE conversation_read_states SET unread_count = 0, last_read_at = CURRENT_TIMESTAMP
         WHERE user_id = ?",
    )
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE conversations SET unread_count = 0 WHERE user_id = ?")
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
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
        "SELECT EXISTS(
           SELECT 1 FROM conversations
           WHERE id = ? AND (user_id = ? OR participant_user_id = ?)
         )",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await?;
    if !owned {
        return Err(AppError::NotFound("conversation not found".into()));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO conversation_read_states
         (conversation_id, user_id, unread_count, last_read_at)
         VALUES (?, ?, 0, CURRENT_TIMESTAMP)
         ON CONFLICT(conversation_id, user_id) DO UPDATE SET
         unread_count = 0, last_read_at = CURRENT_TIMESTAMP",
    )
    .bind(&id)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE conversations SET unread_count = 0 WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let items = sqlx::query_as::<_, MessageItem>(
        "SELECT id,
         CASE
           WHEN sender = 'system' THEN 'system'
           WHEN sender_user_id = ? THEN 'user'
           WHEN sender_user_id IS NOT NULL THEN 'partner'
           ELSE sender
         END AS sender,
         content, created_at FROM conversation_messages
         WHERE conversation_id = ? ORDER BY created_at",
    )
    .bind(&user.id)
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
    let participants: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT user_id, participant_user_id FROM conversations
         WHERE id = ? AND (user_id = ? OR participant_user_id = ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await?;
    let (owner_user_id, participant_user_id) =
        participants.ok_or_else(|| AppError::NotFound("conversation not found".into()))?;
    let message_id = Uuid::new_v4().to_string();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO conversation_messages
         (id, conversation_id, sender, sender_user_id, content)
         VALUES (?, ?, 'user', ?, ?)",
    )
    .bind(&message_id)
    .bind(&id)
    .bind(&user.id)
    .bind(content)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE conversations SET last_message = ?,
         unread_count = CASE WHEN user_id = ? THEN 0 ELSE unread_count END,
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(content)
    .bind(&user.id)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO conversation_read_states
         (conversation_id, user_id, unread_count, last_read_at)
         VALUES (?, ?, 0, CURRENT_TIMESTAMP)
         ON CONFLICT(conversation_id, user_id) DO UPDATE SET
         unread_count = 0, last_read_at = CURRENT_TIMESTAMP",
    )
    .bind(&id)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    let recipient_user_id = if user.id == owner_user_id {
        participant_user_id
    } else {
        Some(owner_user_id)
    };
    if let Some(recipient_user_id) = recipient_user_id {
        sqlx::query(
            "INSERT INTO conversation_read_states (conversation_id, user_id, unread_count)
             VALUES (?, ?, 1)
             ON CONFLICT(conversation_id, user_id) DO UPDATE SET
             unread_count = unread_count + 1",
        )
        .bind(&id)
        .bind(recipient_user_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let item = sqlx::query_as::<_, MessageItem>(
        "SELECT id, 'user' AS sender, content, created_at
         FROM conversation_messages WHERE id = ?",
    )
    .bind(message_id)
    .fetch_one(&state.pool)
    .await?;
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "message_sent",
        Some("conversation"),
        Some(&id),
        json!({ "messageId": &item.id }),
    )
    .await;
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
    if let Some(avatar) = input.avatar.as_deref().map(str::trim) {
        sqlx::query(
            "UPDATE users SET avatar = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(avatar)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
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
    let _ = crate::analytics::track_event(
        &state,
        Some(&user.id),
        "withdrawal_requested",
        Some("settlement"),
        None,
        json!({ "amount": input.amount }),
    )
    .await;
    Ok(Json(json!({
        "success": true,
        "balance": balance - input.amount
    })))
}

async fn unlock_partner_contact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<ContactUnlockResponse>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    let partner = sqlx::query_as::<_, Partner>(
        "SELECT id, partner_type, avatar, avatar_class, name, identity, description,
         tags AS tags_json, match_score, result_text, active
         FROM partners WHERE id = ? AND active = 1",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("partner not found".into()))?;
    if partner.partner_type == user.role {
        return Err(AppError::BadRequest(
            "cannot unlock same role contact".into(),
        ));
    }
    let source_user_id: Option<String> =
        sqlx::query_scalar("SELECT source_user_id FROM partners WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await?;
    if source_user_id.as_deref() == Some(&user.id) {
        return Err(AppError::BadRequest("cannot unlock own contact".into()));
    }
    let balance = wallet_balance(&state.pool, &user.id).await?;
    if membership_active(&state.pool, &user.id).await? {
        let contact_method = partner_contact_method(&state.pool, &id).await?;
        return Ok(Json(ContactUnlockResponse {
            contact_method,
            unlocked: true,
            balance,
        }));
    }
    let already_unlocked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM contact_unlocks WHERE user_id = ? AND partner_id = ?",
    )
    .bind(&user.id)
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    if already_unlocked > 0 {
        let contact_method = partner_contact_method(&state.pool, &id).await?;
        return Ok(Json(ContactUnlockResponse {
            contact_method,
            unlocked: true,
            balance,
        }));
    }
    let price = app_setting_i64(&state.pool, "contact_unlock_price").await?;
    let mut tx = state.pool.begin().await?;
    if balance < price {
        return Err(AppError::BadRequest("insufficient wallet balance".into()));
    }
    sqlx::query(
        "UPDATE wallets SET balance = balance - ?, updated_at = CURRENT_TIMESTAMP WHERE user_id = ?",
    )
    .bind(price)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    let contact_method = partner_contact_method(&state.pool, &id).await?;
    sqlx::query("INSERT INTO contact_unlocks (user_id, partner_id) VALUES (?, ?)")
        .bind(&user.id)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO settlements (id, user_id, title, amount, status)
         VALUES (?, ?, '联系权益解锁', ?, 'completed')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(-price)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(ContactUnlockResponse {
        contact_method,
        unlocked: true,
        balance: balance - price,
    }))
}

async fn partner_contact_method(pool: &sqlx::SqlitePool, partner_id: &str) -> AppResult<String> {
    let row = sqlx::query(
        "SELECT oa.contact_method
         FROM partners p
         JOIN onboarding_applications oa ON oa.user_id = p.source_user_id
         WHERE p.id = ? AND oa.status = 'approved'",
    )
    .bind(partner_id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => Ok(r.get("contact_method")),
        None => Err(AppError::BadRequest("contact not available".into())),
    }
}

async fn wallet_balance(pool: &sqlx::SqlitePool, user_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?)
}

async fn app_setting_i64(pool: &sqlx::SqlitePool, key: &str) -> AppResult<i64> {
    let value: String = sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::Internal(anyhow::anyhow!("missing app setting: {key}")))?;
    value
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid app setting: {key}")))
}

async fn membership_active(pool: &sqlx::SqlitePool, user_id: &str) -> AppResult<bool> {
    let active_until: Option<String> =
        sqlx::query_scalar("SELECT active_until FROM memberships WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    match active_until {
        Some(value) => {
            let time = NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_else(|_| Utc::now().naive_utc());
            Ok(time >= Utc::now().naive_utc())
        }
        None => Ok(false),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MembershipPlan {
    key: String,
    name: String,
    price: i64,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MembershipPlansResponse {
    plans: Vec<MembershipPlan>,
    active_until: Option<String>,
    balance: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MembershipPurchaseInput {
    plan: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactUnlockResponse {
    contact_method: String,
    unlocked: bool,
    balance: i64,
}

async fn membership_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<MembershipPlansResponse>> {
    let user = current_user(&state, &headers).await?;
    let monthly = app_setting_i64(&state.pool, "membership_monthly_price").await?;
    let quarterly = app_setting_i64(&state.pool, "membership_quarterly_price").await?;
    let active_until: Option<String> =
        sqlx::query_scalar("SELECT active_until FROM memberships WHERE user_id = ?")
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await?;
    let balance = wallet_balance(&state.pool, &user.id).await?;
    Ok(Json(MembershipPlansResponse {
        plans: vec![
            MembershipPlan {
                key: "monthly".into(),
                name: "月度会员".into(),
                price: monthly,
                description: "30 天无限解锁".into(),
            },
            MembershipPlan {
                key: "quarterly".into(),
                name: "季度会员".into(),
                price: quarterly,
                description: "90 天无限解锁".into(),
            },
        ],
        active_until,
        balance,
    }))
}

async fn purchase_membership(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<MembershipPurchaseInput>,
) -> AppResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    require_approved(&user)?;
    let (plan_key, days, price_key) = match input.plan.as_str() {
        "monthly" => ("monthly", 30, "membership_monthly_price"),
        "quarterly" => ("quarterly", 90, "membership_quarterly_price"),
        _ => return Err(AppError::BadRequest("invalid membership plan".into())),
    };
    let price = app_setting_i64(&state.pool, price_key).await?;
    let mut tx = state.pool.begin().await?;
    let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(&user.id)
        .fetch_one(&mut *tx)
        .await?;
    if balance < price {
        return Err(AppError::BadRequest("insufficient wallet balance".into()));
    }
    sqlx::query(
        "UPDATE wallets SET balance = balance - ?, updated_at = CURRENT_TIMESTAMP WHERE user_id = ?",
    )
    .bind(price)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    let active_until = (Utc::now() + Duration::days(days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    sqlx::query(
        "INSERT INTO memberships (user_id, plan, active_until)
         VALUES (?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET
          plan = excluded.plan,
          active_until = MAX(COALESCE(memberships.active_until, '1970-01-01 00:00:00'), excluded.active_until),
          updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&user.id)
    .bind(plan_key)
    .bind(&active_until)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO settlements (id, user_id, title, amount, status)
         VALUES (?, ?, ?, ?, 'completed')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(format!("{} 会员购买", plan_key))
    .bind(-price)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "success": true,
        "activeUntil": active_until,
        "balance": balance - price
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
    let stats = if user.role == "provider" {
        vec![
            Metric {
                value: conversation_count.to_string(),
                label: "创作者会话".into(),
            },
            Metric {
                value: completed_settlements.to_string(),
                label: "完成合作".into(),
            },
            Metric {
                value: if user.verified {
                    "已通过"
                } else {
                    "审核中"
                }
                .into(),
                label: "入驻状态".into(),
            },
        ]
    } else {
        vec![
            Metric {
                value: match_count.to_string(),
                label: "推广需求".into(),
            },
            Metric {
                value: conversation_count.to_string(),
                label: "推广伙伴".into(),
            },
            Metric {
                value: if user.verified {
                    "已通过"
                } else {
                    "审核中"
                }
                .into(),
                label: "创作者认证".into(),
            },
        ]
    };
    let role_label = if user.role == "provider" {
        "推广服务方"
    } else {
        "内容创作者"
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

pub(crate) async fn current_user(state: &AppState, headers: &HeaderMap) -> AppResult<UserRow> {
    let token = bearer_token(headers).ok_or(AppError::Unauthorized)?;
    db::user_from_token(&state.pool, token).await
}

async fn load_user(state: &AppState, id: &str) -> AppResult<UserRow> {
    Ok(sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, organization, role, avatar, description, verified,
         onboarding_status, review_note
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

fn require_approved(user: &UserRow) -> AppResult<()> {
    if user.onboarding_status != "approved" {
        return Err(AppError::BadRequest("onboarding approval required".into()));
    }
    Ok(())
}

fn require_creator(user: &UserRow) -> AppResult<()> {
    if user.role != "client" {
        return Err(AppError::BadRequest("creator role required".into()));
    }
    Ok(())
}

fn describe_contact_method(contact_method: &str) -> String {
    if contact_method.contains('@') {
        "邮箱联系方式已提交平台审核".into()
    } else if contact_method.chars().any(|value| value.is_ascii_digit()) {
        "手机号或微信联系方式已提交平台审核".into()
    } else {
        "联系方式已提交平台审核".into()
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
