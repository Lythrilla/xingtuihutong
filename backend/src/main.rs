mod admin;
mod admin_agent;
mod agent;
mod analytics;
mod api;
mod config;
mod db;
mod error;
mod models;
mod state;

use anyhow::Result;
use axum::{
    http::{header, HeaderValue, Method},
    response::Redirect,
    routing::get,
    Json, Router,
};
use config::Config;
use serde_json::json;
use state::AppState;
use std::path::Path;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xingtuihutong_backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    if !Path::new("data").exists() {
        std::fs::create_dir_all("data")?;
    }
    if !Path::new("data/uploads").exists() {
        std::fs::create_dir_all("data/uploads")?;
    }
    let pool = db::connect(&config.database_url).await?;
    let state = AppState {
        pool,
        config: config.clone(),
    };
    let cors = cors_layer(&config)?;
    let app = Router::new()
        .route("/", get(|| async { Redirect::permanent("/admin/") }))
        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
        .nest("/api", api::routes())
        .nest("/api/admin", admin::routes())
        .nest_service(
            "/admin",
            ServeDir::new("admin").append_index_html_on_directories(true),
        )
        .nest_service("/uploads", ServeDir::new("data/uploads"))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    tracing::info!(address = %config.bind_address, "server started");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn cors_layer(config: &Config) -> Result<CorsLayer> {
    let origin = if config.allowed_origin == "*" {
        AllowOrigin::any()
    } else {
        AllowOrigin::exact(HeaderValue::from_str(&config.allowed_origin)?)
    };
    Ok(CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_credentials(config.allowed_origin != "*"))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
