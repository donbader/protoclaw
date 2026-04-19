use std::sync::Arc;

use anyclaw_core::HealthSnapshot;
use axum::{Router, extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tokio::sync::RwLock;

type SharedHealth = Arc<RwLock<HealthSnapshot>>;

/// Start the admin HTTP server on the given port, serving `/health` and `/metrics`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_core::{HealthSnapshot, HealthStatus};
    use axum::{body::to_bytes, extract::State, response::IntoResponse};
    use metrics_exporter_prometheus::PrometheusBuilder;
    use rstest::rstest;

    fn given_shared_health(status: HealthStatus) -> SharedHealth {
        let snapshot = HealthSnapshot {
            status,
            agents: Vec::new(),
            channels: Vec::new(),
            mcp_servers: Vec::new(),
        };
        Arc::new(RwLock::new(snapshot))
    }

    fn given_prometheus_handle() -> Arc<PrometheusHandle> {
        let recorder = PrometheusBuilder::new().build_recorder();
        Arc::new(recorder.handle())
    }

    #[rstest]
    #[tokio::test]
    async fn when_healthy_then_health_handler_returns_200_with_json() {
        let health = given_shared_health(HealthStatus::Healthy);
        let handle = given_prometheus_handle();
        let response = health_handler(State((health, handle)))
            .await
            .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "application/json"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "healthy");
    }

    #[rstest]
    #[tokio::test]
    async fn when_degraded_then_health_handler_returns_503_with_json() {
        let health = given_shared_health(HealthStatus::Degraded);
        let handle = given_prometheus_handle();
        let response = health_handler(State((health, handle)))
            .await
            .into_response();

        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "application/json"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "degraded");
    }

    #[rstest]
    #[tokio::test]
    async fn when_metrics_requested_then_returns_200_with_prometheus_content_type() {
        let health = given_shared_health(HealthStatus::Healthy);
        let handle = given_prometheus_handle();
        let response = metrics_handler(State((health, handle)))
            .await
            .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "text/plain; version=0.0.4; charset=utf-8"
        );
    }
}
