use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use protoclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelHarness, ChannelSdkError, ChannelSendMessage};
use protoclaw_sdk_types::{
    ChannelRequestPermission, DeliverMessage, PeerInfo, PermissionResponse,
};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};
use tokio_stream::StreamExt;

#[derive(Clone, Debug)]
struct SsePayload {
    event_type: Option<String>,
    data: String,
}

/// Pending permission request stored for HTTP retrieval.
#[derive(Clone, serde::Serialize)]
struct PendingPermission {
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "sessionId")]
    session_id: String,
    description: String,
    options: serde_json::Value,
}

/// Shared state between Channel impl and HTTP handlers.
struct SharedState {
    /// Outbound sender provided by ChannelHarness in on_ready.
    outbound: Mutex<Option<mpsc::Sender<ChannelSendMessage>>>,
    /// Broadcast agent updates to SSE subscribers.
    event_tx: broadcast::Sender<SsePayload>,
    /// Pending permission requests from the agent.
    pending_permissions: RwLock<Vec<PendingPermission>>,
    /// Oneshot senders for permission responses — HTTP handler resolves these.
    permission_resolvers: Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>,
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

struct DebugHttpChannel {
    state: Arc<SharedState>,
    host: String,
    port: u16,
}

#[async_trait]
impl Channel for DebugHttpChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: true,
            rich_text: false,
        }
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
    ) -> Result<(), ChannelSdkError> {
        *self.state.outbound.lock().await = Some(outbound);

        let router = build_router(self.state.clone());
        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| ChannelSdkError::Io(e))?;
        let bound_port = listener.local_addr().unwrap().port();

        eprintln!("PORT:{bound_port}");
        tracing::info!(port = bound_port, "debug-http listening");

        tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        let update_type = msg.content.get("type").and_then(|t| t.as_str());
        let payload = match update_type {
            Some("agent_thought_chunk") => {
                let thought = msg.content.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                SsePayload {
                    event_type: Some("thought".into()),
                    data: thought.to_string(),
                }
            }
            _ => {
                let content_str = match msg.content {
                    serde_json::Value::String(s) => s,
                    other => serde_json::to_string(&other).unwrap_or_default(),
                };
                SsePayload { event_type: None, data: content_str }
            }
        };
        let _ = self.state.event_tx.send(payload);
        Ok(())
    }

    async fn request_permission(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError> {
        let (tx, rx) = oneshot::channel();

        self.state
            .pending_permissions
            .write()
            .await
            .push(PendingPermission {
                request_id: req.request_id.clone(),
                session_id: req.session_id,
                description: req.description,
                options: serde_json::to_value(&req.options).unwrap_or_default(),
            });

        self.state
            .permission_resolvers
            .lock()
            .await
            .insert(req.request_id.clone(), tx);

        rx.await.map_err(|_| {
            ChannelSdkError::Protocol("permission response channel closed".into())
        })
    }
}

fn build_router(state: Arc<SharedState>) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/message", post(handle_message))
        .route("/events", get(handle_events))
        .route("/cancel", post(handle_cancel))
        .route("/permissions/pending", get(handle_permissions_pending))
        .route("/permissions/{id}/respond", post(handle_permission_respond))
        .with_state(state)
}

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_message(
    State(state): State<Arc<SharedState>>,
    Json(body): Json<MessageBody>,
) -> impl IntoResponse {
    let outbound = state.outbound.lock().await;
    if let Some(tx) = outbound.as_ref() {
        let msg = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "local".into(),
                kind: "local".into(),
            },
            content: body.message,
        };
        let _ = tx.send(msg).await;
    }
    (
        axum::http::StatusCode::OK,
        Json(serde_json::json!({"status": "queued", "message": "Message received and queued for processing"})),
    )
}

async fn handle_events(
    State(state): State<Arc<SharedState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(payload) => {
            let mut event = Event::default().data(payload.data);
            if let Some(ref et) = payload.event_type {
                event = event.event(et);
            }
            Some(Ok(event))
        }
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn handle_cancel(State(state): State<Arc<SharedState>>) -> Json<serde_json::Value> {
    let outbound = state.outbound.lock().await;
    if let Some(tx) = outbound.as_ref() {
        let msg = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "local".into(),
                kind: "local".into(),
            },
            content: "__cancel__".into(),
        };
        let _ = tx.send(msg).await;
    }
    Json(serde_json::json!({"status": "cancelled"}))
}

async fn handle_permissions_pending(
    State(state): State<Arc<SharedState>>,
) -> Json<serde_json::Value> {
    let perms = state.pending_permissions.read().await;
    let items: Vec<serde_json::Value> = perms
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(serde_json::json!(null)))
        .collect();
    Json(serde_json::Value::Array(items))
}

async fn handle_permission_respond(
    State(state): State<Arc<SharedState>>,
    Path(id): Path<String>,
    Json(body): Json<PermissionResponseBody>,
) -> Json<serde_json::Value> {
    {
        let mut perms = state.pending_permissions.write().await;
        perms.retain(|p| p.request_id != id);
    }
    {
        let mut resolvers = state.permission_resolvers.lock().await;
        if let Some(tx) = resolvers.remove(&id) {
            let _ = tx.send(PermissionResponse {
                request_id: id.clone(),
                option_id: body.option_id,
            });
        }
    }
    Json(serde_json::json!({"status": "responded"}))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let (event_tx, _) = broadcast::channel::<SsePayload>(256);

    let state = Arc::new(SharedState {
        outbound: Mutex::new(None),
        event_tx,
        pending_permissions: RwLock::new(Vec::new()),
        permission_resolvers: Mutex::new(HashMap::new()),
    });

    let channel = DebugHttpChannel {
        state: state.clone(),
        host,
        port,
    };

    if let Err(e) = ChannelHarness::new(channel).run_stdio().await {
        tracing::error!(%e, "channel harness exited with error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn make_shared_state() -> Arc<SharedState> {
        let (event_tx, _) = broadcast::channel::<SsePayload>(256);
        Arc::new(SharedState {
            outbound: Mutex::new(None),
            event_tx,
            pending_permissions: RwLock::new(Vec::new()),
            permission_resolvers: Mutex::new(HashMap::new()),
        })
    }

    #[test]
    fn debug_http_channel_capabilities() {
        let state = make_shared_state();
        let ch = DebugHttpChannel { state, host: "127.0.0.1".to_string(), port: 0 };
        let caps = ch.capabilities();
        assert!(caps.streaming, "debug-http must support streaming");
        assert!(!caps.rich_text, "debug-http must not claim rich_text");
    }

    #[tokio::test]
    async fn on_ready_stores_outbound_sender() {
        let state = make_shared_state();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let (tx, _rx) = mpsc::channel(16);
        ch.on_ready(tx).await.unwrap();
        let outbound = state.outbound.lock().await;
        assert!(outbound.is_some(), "on_ready must store the outbound sender");
    }

    #[tokio::test]
    async fn deliver_message_broadcasts_to_sse() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!("hello from agent"),
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(received.event_type.is_none(), "plain string should have no event type");
        assert_eq!(received.data, "hello from agent");
    }

    #[tokio::test]
    async fn thought_chunk_broadcasts_as_named_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"type": "agent_thought_chunk", "content": "thinking..."}),
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert_eq!(received.event_type.as_deref(), Some("thought"));
        assert_eq!(received.data, "thinking...");
    }

    #[tokio::test]
    async fn message_chunk_broadcasts_as_default_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"type": "agent_message_chunk", "content": "hello"}),
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(received.event_type.is_none(), "message chunk should use default event");
    }

    #[tokio::test]
    async fn result_broadcasts_as_default_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"type": "result", "content": "done"}),
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(received.event_type.is_none(), "result should use default event");
    }

    #[tokio::test]
    async fn request_permission_resolves_via_oneshot() {
        let state = make_shared_state();
        let mut ch = DebugHttpChannel { state: state.clone(), host: "127.0.0.1".to_string(), port: 0 };
        let req = ChannelRequestPermission {
            request_id: "perm-1".into(),
            session_id: "s1".into(),
            description: "Allow?".into(),
            options: vec![protoclaw_sdk_types::PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };

        // Spawn the request_permission call
        let state2 = state.clone();
        let handle = tokio::spawn(async move { ch.request_permission(req).await });

        // Give it a moment to register the oneshot
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Simulate HTTP handler resolving the permission
        {
            let mut resolvers = state2.permission_resolvers.lock().await;
            if let Some(tx) = resolvers.remove("perm-1") {
                tx.send(PermissionResponse {
                    request_id: "perm-1".into(),
                    option_id: "allow".into(),
                })
                .unwrap();
            }
        }

        let resp = handle.await.unwrap().unwrap();
        assert_eq!(resp.request_id, "perm-1");
        assert_eq!(resp.option_id, "allow");
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let state = make_shared_state();
        let app = build_router(state);
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn message_endpoint_sends_via_outbound() {
        let state = make_shared_state();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/message")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let msg = rx.try_recv().expect("should have received outbound message");
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.peer_info.channel_name, "debug-http");

        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "queued");
    }

    #[tokio::test]
    async fn permission_respond_endpoint_resolves_pending() {
        let state = make_shared_state();
        // Add a pending permission
        state.pending_permissions.write().await.push(PendingPermission {
            request_id: "perm-1".into(),
            session_id: "s1".into(),
            description: "Allow?".into(),
            options: serde_json::json!([{"optionId": "allow", "label": "Allow"}]),
        });
        // Add a resolver
        let (tx, rx) = oneshot::channel();
        state
            .permission_resolvers
            .lock()
            .await
            .insert("perm-1".into(), tx);

        let app = build_router(state.clone());
        let req = Request::builder()
            .method("POST")
            .uri("/permissions/perm-1/respond")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"optionId":"allow"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify the oneshot was resolved
        let perm_resp = rx.await.expect("oneshot should have been resolved");
        assert_eq!(perm_resp.option_id, "allow");

        // Verify pending was removed
        let pending = state.pending_permissions.read().await;
        assert!(pending.is_empty());
    }
}
