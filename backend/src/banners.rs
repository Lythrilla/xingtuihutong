use crate::{
    admin::require_admin,
    error::{AppError, AppResult},
    models::{Banner, BannerInput},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, put},
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_admin).post(create))
        .route("/{id}", put(update).delete(delete_banner))
}

pub fn public_routes() -> Router<AppState> {
    Router::new().route("/", get(list_public))
}

async fn list_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Banner>>> {
    require_admin(&state, &headers).await?;
    let banners = sqlx::query_as::<_, Banner>(
        "SELECT id, image_url, link_url, title, subtitle, sort_order, active, start_at, end_at
         FROM banners ORDER BY sort_order, created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(banners))
}

async fn list_public(State(state): State<AppState>) -> AppResult<Json<Vec<Banner>>> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let banners = sqlx::query_as::<_, Banner>(
        "SELECT id, image_url, link_url, title, subtitle, sort_order, active, start_at, end_at
         FROM banners
         WHERE active = 1
           AND (start_at IS NULL OR start_at <= ?)
           AND (end_at IS NULL OR end_at >= ?)
         ORDER BY sort_order, created_at DESC",
    )
    .bind(&now)
    .bind(&now)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(banners))
}

async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BannerInput>,
) -> AppResult<Json<Banner>> {
    require_admin(&state, &headers).await?;
    let id = Uuid::new_v4().to_string();
    let banner = build_banner(id, input)?;
    sqlx::query(
        "INSERT INTO banners
         (id, image_url, link_url, title, subtitle, sort_order, active, start_at, end_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&banner.id)
    .bind(&banner.image_url)
    .bind(&banner.link_url)
    .bind(&banner.title)
    .bind(&banner.subtitle)
    .bind(banner.sort_order)
    .bind(banner.active)
    .bind(&banner.start_at)
    .bind(&banner.end_at)
    .execute(&state.pool)
    .await?;
    Ok(Json(banner))
}

async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<BannerInput>,
) -> AppResult<Json<Banner>> {
    require_admin(&state, &headers).await?;
    let banner = build_banner(id.clone(), input)?;
    let result = sqlx::query(
        "UPDATE banners SET
         image_url = ?, link_url = ?, title = ?, subtitle = ?, sort_order = ?, active = ?,
         start_at = ?, end_at = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&banner.image_url)
    .bind(&banner.link_url)
    .bind(&banner.title)
    .bind(&banner.subtitle)
    .bind(banner.sort_order)
    .bind(banner.active)
    .bind(&banner.start_at)
    .bind(&banner.end_at)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("banner not found".into()));
    }
    Ok(Json(banner))
}

async fn delete_banner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&state, &headers).await?;
    let result = sqlx::query("DELETE FROM banners WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("banner not found".into()));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

fn build_banner(id: String, input: BannerInput) -> AppResult<Banner> {
    if input.image_url.trim().is_empty() {
        return Err(AppError::BadRequest("banner image url is required".into()));
    }
    Ok(Banner {
        id,
        image_url: input.image_url.trim().to_string(),
        link_url: input.link_url.trim().to_string(),
        title: input.title.trim().to_string(),
        subtitle: input.subtitle.trim().to_string(),
        sort_order: input.sort_order.max(0),
        active: input.active,
        start_at: input.start_at.filter(|s| !s.is_empty()),
        end_at: input.end_at.filter(|s| !s.is_empty()),
    })
}
