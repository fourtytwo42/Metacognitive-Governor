//! Versioned local HTTP surface shared by bundled clients.

use std::{net::SocketAddr, sync::Arc};

use axum::{Json, Router, extract::State, routing::get};
use mcg_runtime::{AppRuntime, RuntimeStatus};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct ApiState {
    runtime: Arc<AppRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

pub fn router(runtime: Arc<AppRuntime>) -> Router {
    Router::new()
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/system/status", get(status))
        .layer(TraceLayer::new_for_http())
        .with_state(ApiState { runtime })
}

/// Binds the local API listener and serves requests until shutdown.
///
/// # Errors
///
/// Returns an I/O error when the listener cannot bind or the server fails.
pub async fn serve(address: SocketAddr, runtime: Arc<AppRuntime>) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(address).await?;
    axum::serve(listener, router(runtime)).await
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "mcg-daemon",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status(State(state): State<ApiState>) -> Json<RuntimeStatus> {
    Json(state.runtime.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_reports_service_identity() {
        let Json(response) = health().await;

        assert_eq!(response.status, "ok");
        assert_eq!(response.service, "mcg-daemon");
    }
}
