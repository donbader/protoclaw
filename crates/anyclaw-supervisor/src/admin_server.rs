use std::sync::Arc;

use axum::{Router, extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use anyclaw_core::HealthSnapshot;
use tokio::sync::RwLock;

type SharedHealth = Arc<RwLock<HealthSnapshot>>;

pub async fn start(port: u16, health: SharedHealth) {
    let handle = match PrometheusBuilder::new().install_recorder() {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(
                "prometheus recorder already installed, /metrics will be unavailable: {e}"
            );
            return;
        }
    };

    let state = (health, Arc::new(handle));
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(
                port,
                "admin server failed to bind, health/metrics endpoints unavailable: {e}"
            );
            return;
        }
    };

    tracing::info!(port, "admin server listening");

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
}

async fn health_handler(
    State((health, _)): State<(SharedHealth, Arc<PrometheusHandle>)>,
) -> impl IntoResponse {
    let snapshot = health.read().await.clone();
    let http_status = match snapshot.status {
        anyclaw_core::HealthStatus::Healthy => axum::http::StatusCode::OK,
        anyclaw_core::HealthStatus::Degraded => axum::http::StatusCode::SERVICE_UNAVAILABLE,
    };
    let body = serde_json::to_string(&snapshot).expect("HealthSnapshot serialization cannot fail");
    (
        http_status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
}

async fn metrics_handler(
    State((_, handle)): State<(SharedHealth, Arc<PrometheusHandle>)>,
) -> impl IntoResponse {
    let body = handle.render();
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
