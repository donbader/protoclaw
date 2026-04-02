use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use protoclaw_agents::AgentsCommand;
use protoclaw_core::{Manager, ManagerError, ManagerHandle};
use serde::Deserialize;
use tokio::sync::{broadcast, oneshot, watch};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

pub struct DebugHttpChannel {
    port: u16,
    agents_handle: ManagerHandle<AgentsCommand>,
    event_tx: broadcast::Sender<String>,
    port_tx: watch::Sender<u16>,
}

#[derive(Clone)]
struct AppState {
    agents_handle: ManagerHandle<AgentsCommand>,
    event_tx: broadcast::Sender<String>,
}

#[derive(Deserialize)]
struct MessageBody {
    message: String,
}

#[derive(Deserialize)]
struct PermissionResponseBody {
    #[serde(rename = "optionId")]
    option_id: String,
}

impl DebugHttpChannel {
    pub fn new(port: u16, agents_handle: ManagerHandle<AgentsCommand>) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        let (port_tx, _) = watch::channel(0);
        Self {
            port,
            agents_handle,
            event_tx,
            port_tx,
        }
    }

    pub fn with_port_tx(mut self, port_tx: watch::Sender<u16>) -> Self {
        self.port_tx = port_tx;
        self
    }

    pub fn event_sender(&self) -> broadcast::Sender<String> {
        self.event_tx.clone()
    }

    pub fn port_rx(&self) -> watch::Receiver<u16> {
        self.port_tx.subscribe()
    }

    fn build_router(state: AppState) -> Router {
        Router::new()
            .route("/health", get(handle_health))
            .route("/message", post(handle_message))
            .route("/events", get(handle_events))
            .route("/cancel", post(handle_cancel))
            .route("/permissions/pending", get(handle_permissions_pending))
            .route("/permissions/{id}/respond", post(handle_permission_respond))
            .with_state(state)
    }
}

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_message(
    State(state): State<AppState>,
    Json(body): Json<MessageBody>,
) -> impl IntoResponse {
    let (reply_tx, reply_rx) = oneshot::channel();
    let cmd = AgentsCommand::SendPrompt {
        message: body.message,
        reply: reply_tx,
    };

    if let Err(e) = state.agents_handle.send(cmd).await {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        );
    }

    match reply_rx.await {
        Ok(Ok(())) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({"status": "sent"})),
        ),
        Ok(Err(e)) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "reply channel closed"})),
        ),
    }
}

async fn handle_events(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn handle_cancel(State(state): State<AppState>) -> Json<serde_json::Value> {
    let _ = state.agents_handle.send(AgentsCommand::CancelOperation).await;
    Json(serde_json::json!({"status": "cancelled"}))
}

async fn handle_permissions_pending(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let (reply_tx, reply_rx) = oneshot::channel();
    let cmd = AgentsCommand::GetPendingPermissions { reply: reply_tx };

    if state.agents_handle.send(cmd).await.is_err() {
        return Json(serde_json::json!([]));
    }

    match reply_rx.await {
        Ok(perms) => {
            let items: Vec<serde_json::Value> = perms
                .into_iter()
                .map(|p| {
                    serde_json::json!({
                        "requestId": p.request_id,
                        "description": p.description,
                        "options": p.options,
                    })
                })
                .collect();
            Json(serde_json::Value::Array(items))
        }
        Err(_) => Json(serde_json::json!([])),
    }
}

async fn handle_permission_respond(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PermissionResponseBody>,
) -> Json<serde_json::Value> {
    let cmd = AgentsCommand::RespondPermission {
        request_id: id,
        option_id: body.option_id,
    };
    let _ = state.agents_handle.send(cmd).await;
    Json(serde_json::json!({"status": "responded"}))
}

impl Manager for DebugHttpChannel {
    type Command = ();

    fn name(&self) -> &str {
        "channels"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        tracing::info!(manager = self.name(), "manager started");
        Ok(())
    }

    async fn run(self, cancel: CancellationToken) -> Result<(), ManagerError> {
        let state = AppState {
            agents_handle: self.agents_handle.clone(),
            event_tx: self.event_tx.clone(),
        };
        let router = Self::build_router(state);

        let addr = format!("127.0.0.1:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| ManagerError::Internal(format!("bind failed: {e}")))?;

        let bound_addr = listener.local_addr()
            .map_err(|e| ManagerError::Internal(format!("local_addr failed: {e}")))?;
        let _ = self.port_tx.send(bound_addr.port());
        tracing::info!(port = bound_addr.port(), "debug-http listening");

        axum::serve(listener, router)
            .with_graceful_shutdown(async move { cancel.cancelled().await })
            .await
            .map_err(|e| ManagerError::Internal(format!("serve error: {e}")))?;

        tracing::info!(manager = "channels", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_agents::AgentsCommand;

    async fn spawn_test_server() -> (u16, CancellationToken, broadcast::Sender<String>, tokio::sync::mpsc::Receiver<AgentsCommand>) {
        let (tx, rx) = tokio::sync::mpsc::channel::<AgentsCommand>(16);
        let handle = ManagerHandle::new(tx);
        let (event_tx, _) = broadcast::channel(256);

        let state = AppState {
            agents_handle: handle,
            event_tx: event_tx.clone(),
        };
        let router = DebugHttpChannel::build_router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let c = cancel.clone();

        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { c.cancelled().await })
                .await
                .unwrap();
        });

        (port, cancel, event_tx, rx)
    }

    #[tokio::test]
    async fn debug_http_health_endpoint() {
        let (port, cancel, _, _rx) = spawn_test_server().await;

        let resp = reqwest::get(format!("http://127.0.0.1:{port}/health"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");

        cancel.cancel();
    }

    #[tokio::test]
    async fn debug_http_message_endpoint() {
        let (port, cancel, _, mut rx) = spawn_test_server().await;

        let responder = tokio::spawn(async move {
            if let Some(AgentsCommand::SendPrompt { reply, .. }) = rx.recv().await {
                let _ = reply.send(Ok(()));
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": "hello"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "sent");

        responder.await.unwrap();
        cancel.cancel();
    }

    #[tokio::test]
    async fn debug_http_cancel_endpoint() {
        let (port, cancel, _, mut rx) = spawn_test_server().await;

        let responder = tokio::spawn(async move {
            if let Some(AgentsCommand::CancelOperation) = rx.recv().await {}
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/cancel"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "cancelled");

        responder.await.unwrap();
        cancel.cancel();
    }

    #[tokio::test]
    async fn debug_http_permissions_endpoint() {
        let (port, cancel, _, mut rx) = spawn_test_server().await;

        let responder = tokio::spawn(async move {
            if let Some(AgentsCommand::GetPendingPermissions { reply }) = rx.recv().await {
                let _ = reply.send(vec![]);
            }
        });

        let resp = reqwest::get(format!("http://127.0.0.1:{port}/permissions/pending"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body.as_array().unwrap().is_empty());

        responder.await.unwrap();
        cancel.cancel();
    }
}
