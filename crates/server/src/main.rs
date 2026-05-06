mod config;
mod logging;
mod makemkv;
mod routes;
mod state;

use anyhow::Result;
use axum::{Json, Router, response::Html, routing::{get, post}};
use clap::Parser;
use config::Config;
use state::AppState;
use tokio::net::TcpListener;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse().validate()?;
    let _log_guard = logging::init(config.log_dir.as_deref());

    info!(
        bind = %config.bind,
        makemkv = %config.makemkv.display(),
        output_dir = %config.output_dir.display(),
        log_dir = ?config.log_dir.as_ref().map(|p| p.display().to_string()),
        "starting server"
    );

    let bind = config.bind.clone();
    let state = AppState::new(config);

    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/status", get(routes::get_status))
        .route("/api/status/stream", get(routes::get_status_stream))
        .route("/api/disc", get(routes::get_disc))
        .route("/api/disc/scan", post(routes::post_scan))
        .route("/api/rip", post(routes::post_rip))
        .route("/api/backup", post(routes::post_backup))
        .route("/api/cancel", post(routes::post_cancel))
        .with_state(state.clone())
        .layer(
            TraceLayer::new_for_http()
                .on_request(())
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let listener = TcpListener::bind(&bind).await?;
    info!(addr = %listener.local_addr()?, "listening");

    let shutdown_state = state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_state.shutdown.notify_waiters();
        })
        .await?;

    info!("server stopped");
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("static/index.html"))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received");
}
