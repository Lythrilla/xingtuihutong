use crate::{
    error::{AppError, AppResult},
    models::UserRow,
};
use chrono::{Duration, Utc};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use uuid::Uuid;

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn user_from_token(pool: &SqlitePool, token: &str) -> AppResult<UserRow> {
    let user = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.display_name, u.organization, u.role, u.avatar, u.description, u.verified,
         u.onboarding_status, u.review_note
         FROM users u
         JOIN user_sessions s ON s.user_id = u.id
         WHERE s.token = ? AND s.expires_at > CURRENT_TIMESTAMP",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(user)
}

pub async fn create_user_session(pool: &SqlitePool, role: &str) -> AppResult<(String, UserRow)> {
    if role != "provider" && role != "client" {
        return Err(AppError::BadRequest(
            "role must be provider or client".into(),
        ));
    }

    let user_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::days(30))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO users
         (id, display_name, organization, role, avatar, description, verified, onboarding_status)
         VALUES (?, ?, ?, ?, ?, ?, 0, 'draft')",
    )
    .bind(&user_id)
    .bind("新用户")
    .bind("新用户")
    .bind(role)
    .bind("星")
    .bind("")
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO user_sessions (token, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&token)
        .bind(&user_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO wallets (user_id, balance) VALUES (?, ?)")
        .bind(&user_id)
        .bind(0_i64)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, organization, role, avatar, description, verified,
         onboarding_status, review_note
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok((token, user))
}
