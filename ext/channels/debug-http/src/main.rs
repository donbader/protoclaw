use std::convert::Infallible;
use std::sync::Arc;

use anyclaw_sdk_channel::{
    Channel, ChannelCapabilities, ChannelHarness, ChannelSdkError, ChannelSendMessage,
    PermissionBroker, content_to_string,
};
use anyclaw_sdk_types::{
    ChannelRequestPermission, ContentKind, DeliverMessage, PeerInfo, PermissionOption,
    PermissionResponse, acp::ContentPart,
};
use axum::Router;
use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
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
    options: Vec<PermissionOption>,
}

/// Shared state between Channel impl and HTTP handlers.
struct SharedState {
    /// Outbound sender provided by ChannelHarness in on_ready.
    outbound: Mutex<Option<mpsc::Sender<ChannelSendMessage>>>,
    /// Broadcast agent updates to SSE subscribers.
    event_tx: broadcast::Sender<SsePayload>,
    /// Pending permission requests from the agent.
    pending_permissions: RwLock<Vec<PendingPermission>>,
    permission_broker: Mutex<PermissionBroker>,
    /// Sender for deferred permission responses back to the harness.
    permission_tx: Mutex<Option<mpsc::Sender<PermissionResponse>>>,
    /// Optional API key; when set, all routes except /health require Bearer auth.
    api_key: Option<String>,
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
    api_key: Option<String>,
}

impl Channel for DebugHttpChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: true,
            rich_text: false,
            media: true,
        }
    }

    // D-03: defaults() returns HashMap<String, Value> — option values have channel-defined schemas
    #[allow(clippy::disallowed_types)]
    fn defaults(&self) -> Option<std::collections::HashMap<String, serde_json::Value>> {
        const DEFAULTS_YAML: &str = include_str!("../defaults.yaml");
        let value: serde_json::Value =
            serde_yaml::from_str(DEFAULTS_YAML).expect("embedded defaults.yaml must be valid YAML");
        value
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    async fn on_initialize(
        &mut self,
        params: anyclaw_sdk_types::ChannelInitializeParams,
    ) -> Result<(), ChannelSdkError> {
        if let Some(host) = params.options.get("host").and_then(|v| v.as_str()) {
            self.host = host.to_string();
        }
        if let Some(port) = params.options.get("port").and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        }) {
            self.port = port as u16;
        }
        if let Some(key) = params.options.get("api_key").and_then(|v| v.as_str()) {
            self.api_key = Some(key.to_string());
        }
        Ok(())
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
        permission_tx: mpsc::Sender<PermissionResponse>,
    ) -> Result<(), ChannelSdkError> {
        let (event_tx, _) = broadcast::channel::<SsePayload>(256);
        self.state = Arc::new(SharedState {
            outbound: Mutex::new(Some(outbound)),
            event_tx,
            pending_permissions: RwLock::new(Vec::new()),
            permission_broker: Mutex::new(PermissionBroker::new()),
            permission_tx: Mutex::new(Some(permission_tx)),
            api_key: self.api_key.clone(),
        });

        let router = build_router(self.state.clone());
        let addr = format!("{}:{}", self.host, self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(ChannelSdkError::Io)?;
        let bound_port = listener
            .local_addr()
            .expect("TCP listener must have local address after successful bind")
            .port();

        eprintln!("PORT:{bound_port}");
        tracing::info!(port = bound_port, "debug-http listening");

        tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        let kind = ContentKind::from_content(&msg.content);
        let payload = match kind {
            ContentKind::Thought(thought) => SsePayload {
                event_type: Some("thought".into()),
                data: thought.content,
            },
            ContentKind::UserMessageChunk { text } => SsePayload {
                event_type: Some("user_message_chunk".into()),
                data: text,
            },
            ContentKind::MessageChunk { text } => SsePayload {
                event_type: None,
                data: text,
            },
            ContentKind::Result { text, .. } => SsePayload {
                event_type: None,
                data: text,
            },
            ContentKind::ToolCall {
                name,
                tool_call_id,
                input,
            } => SsePayload {
                event_type: Some("tool_call".into()),
                data: serde_json::json!({
                    "toolCallId": tool_call_id,
                    "name": name,
                    "input": input,
                })
                .to_string(),
            },
            ContentKind::ToolCallUpdate {
                name,
                tool_call_id,
                status,
                output,
                ..
            } => SsePayload {
                event_type: Some("tool_call_update".into()),
                data: serde_json::json!({
                    "toolCallId": tool_call_id,
                    "name": name,
                    "status": status,
                    "output": output,
                })
                .to_string(),
            },
            ContentKind::AvailableCommandsUpdate { commands } => SsePayload {
                event_type: Some("available_commands".into()),
                data: commands.to_string(),
            },
            ContentKind::UsageUpdate => SsePayload {
                event_type: Some("usage".into()),
                data: String::new(),
            },
            ContentKind::Image { url } => SsePayload {
                event_type: Some("image".into()),
                data: serde_json::json!({ "url": url }).to_string(),
            },
            ContentKind::File {
                url,
                filename,
                mime_type,
            } => SsePayload {
                event_type: Some("file".into()),
                data:
                    serde_json::json!({ "url": url, "filename": filename, "mimeType": mime_type })
                        .to_string(),
            },
            ContentKind::Audio { url, mime_type } => SsePayload {
                event_type: Some("audio".into()),
                data: serde_json::json!({ "url": url, "mimeType": mime_type }).to_string(),
            },
            _ => {
                let content_str = content_to_string(&msg.content);
                SsePayload {
                    event_type: None,
                    data: content_str,
                }
            }
        };
        let _ = self.state.event_tx.send(payload);
        Ok(())
    }

    async fn show_permission_prompt(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<(), ChannelSdkError> {
        self.state
            .pending_permissions
            .write()
            .await
            .push(PendingPermission {
                request_id: req.request_id.clone(),
                session_id: req.session_id,
                description: req.description,
                options: req.options,
            });

        self.state
            .permission_broker
            .lock()
            .await
            .register(&req.request_id);

        Ok(())
    }
}

async fn auth_middleware(
    State(state): State<Arc<SharedState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let Some(ref expected_key) = state.api_key else {
        return next.run(request).await;
    };
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let authorized =
        matches!(auth_header, Some(h) if h.starts_with("Bearer ") && &h[7..] == expected_key);
    if authorized {
        next.run(request).await
    } else {
        axum::response::Response::builder()
            .status(axum::http::StatusCode::UNAUTHORIZED)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(r#"{"error":"unauthorized"}"#))
            .expect("building 401 response must not fail")
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

// D-03: ad-hoc HTTP JSON responses — lightweight debug endpoints don't warrant dedicated response structs
#[allow(clippy::disallowed_types)]
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
            content: vec![ContentPart::text(body.message)],
            metadata: None,
            meta: None,
        };
        let _ = tx.send(msg).await;
    }
    (
        axum::http::StatusCode::OK,
        Json(
            serde_json::json!({"status": "queued", "message": "Message received and queued for processing"}),
        ),
    )
}

async fn handle_events(
    State(state): State<Arc<SharedState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(payload) => {
                let mut event = Event::default().data(payload.data);
                if let Some(ref et) = payload.event_type {
                    event = event.event(et);
                }
                Some(Ok(event))
            }
            Err(e) => {
                tracing::warn!(error = %e, "SSE broadcast lagged, event dropped");
                None
            }
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// D-03: ad-hoc HTTP JSON response — debug endpoint status
#[allow(clippy::disallowed_types)]
async fn handle_cancel(State(state): State<Arc<SharedState>>) -> Json<serde_json::Value> {
    let outbound = state.outbound.lock().await;
    if let Some(tx) = outbound.as_ref() {
        let msg = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "local".into(),
                kind: "local".into(),
            },
            content: vec![ContentPart::text("__cancel__")],
            metadata: None,
            meta: None,
        };
        let _ = tx.send(msg).await;
    }
    Json(serde_json::json!({"status": "cancelled"}))
}

async fn handle_permissions_pending(
    State(state): State<Arc<SharedState>>,
) -> Json<Vec<PendingPermission>> {
    let perms = state.pending_permissions.read().await;
    Json(perms.clone())
}

// D-03: ad-hoc HTTP JSON response — debug endpoint status
#[allow(clippy::disallowed_types)]
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
        state
            .permission_broker
            .lock()
            .await
            .resolve(&id, &body.option_id);
    }
    {
        let tx = state.permission_tx.lock().await.clone();
        if let Some(tx) = tx {
            let resp = PermissionResponse {
                request_id: id,
                option_id: body.option_id,
            };
            if let Err(e) = tx.send(resp).await {
                tracing::warn!(error = %e, "failed to send permission response to harness");
            }
        }
    }
    Json(serde_json::json!({"status": "responded"}))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = "127.0.0.1".to_string();
    let port: u16 = 0;

    let (event_tx, _) = broadcast::channel::<SsePayload>(256);

    let state = Arc::new(SharedState {
        outbound: Mutex::new(None),
        event_tx,
        pending_permissions: RwLock::new(Vec::new()),
        permission_broker: Mutex::new(PermissionBroker::new()),
        permission_tx: Mutex::new(None),
        api_key: None,
    });

    let channel = DebugHttpChannel {
        state: state.clone(),
        host,
        port,
        api_key: None,
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
            permission_broker: Mutex::new(PermissionBroker::new()),
            permission_tx: Mutex::new(None),
            api_key: None,
        })
    }

    fn make_shared_state_with_key(key: &str) -> Arc<SharedState> {
        let (event_tx, _) = broadcast::channel::<SsePayload>(256);
        Arc::new(SharedState {
            outbound: Mutex::new(None),
            event_tx,
            pending_permissions: RwLock::new(Vec::new()),
            permission_broker: Mutex::new(PermissionBroker::new()),
            permission_tx: Mutex::new(None),
            api_key: Some(key.to_string()),
        })
    }

    #[test]
    fn debug_http_channel_capabilities() {
        let state = make_shared_state();
        let ch = DebugHttpChannel {
            state,
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let caps = ch.capabilities();
        assert!(caps.streaming, "debug-http must support streaming");
        assert!(!caps.rich_text, "debug-http must not claim rich_text");
    }

    #[tokio::test]
    async fn on_ready_stores_outbound_sender() {
        let state = make_shared_state();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let (tx, _rx) = mpsc::channel(16);
        let (perm_tx, _perm_rx) = mpsc::channel(16);
        ch.on_ready(tx, perm_tx).await.unwrap();
        let outbound = ch.state.outbound.lock().await;
        assert!(
            outbound.is_some(),
            "on_ready must store the outbound sender"
        );
    }

    #[tokio::test]
    async fn deliver_message_broadcasts_to_sse() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!("hello from agent"),
            meta: None,
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(
            received.event_type.is_none(),
            "plain string should have no event type"
        );
        assert_eq!(received.data, "hello from agent");
    }

    #[tokio::test]
    async fn thought_chunk_broadcasts_as_named_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"update": {"sessionUpdate": "agent_thought_chunk", "content": "thinking..."}}),
            meta: None,
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
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"update": {"sessionUpdate": "agent_message_chunk", "content": "hello"}}),
            meta: None,
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(
            received.event_type.is_none(),
            "message chunk should use default event"
        );
    }

    #[tokio::test]
    async fn result_broadcasts_as_default_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({"update": {"sessionUpdate": "result", "content": "done"}}),
            meta: None,
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert!(
            received.event_type.is_none(),
            "result should use default event"
        );
    }

    #[tokio::test]
    async fn show_permission_prompt_registers_and_stores_pending() {
        let state = make_shared_state();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let req = ChannelRequestPermission {
            request_id: "perm-1".into(),
            session_id: "s1".into(),
            description: "Allow?".into(),
            options: vec![anyclaw_sdk_types::PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };

        ch.show_permission_prompt(req).await.unwrap();

        let perms = state.pending_permissions.read().await;
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].request_id, "perm-1");
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

        let msg = rx
            .try_recv()
            .expect("should have received outbound message");
        assert_eq!(
            msg.content,
            vec![anyclaw_sdk_types::acp::ContentPart::text("hello")]
        );
        assert_eq!(msg.peer_info.channel_name, "debug-http");

        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "queued");
    }

    #[tokio::test]
    async fn permission_respond_endpoint_resolves_pending() {
        let state = make_shared_state();
        state
            .pending_permissions
            .write()
            .await
            .push(PendingPermission {
                request_id: "perm-1".into(),
                session_id: "s1".into(),
                description: "Allow?".into(),
                options: vec![PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                }],
            });
        let rx = state.permission_broker.lock().await.register("perm-1");

        let app = build_router(state.clone());
        let req = Request::builder()
            .method("POST")
            .uri("/permissions/perm-1/respond")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"optionId":"allow"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let perm_resp = rx
            .await
            .expect("broker should have resolved the permission");
        assert_eq!(perm_resp.option_id, "allow");

        let pending = state.pending_permissions.read().await;
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn tool_call_broadcasts_as_named_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({
                "update": {
                    "sessionUpdate": "tool_call",
                    "toolCallId": "tc-1",
                    "name": "read_file",
                    "input": {"path": "/tmp/foo.txt"}
                }
            }),
            meta: None,
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert_eq!(received.event_type.as_deref(), Some("tool_call"));
        let data: serde_json::Value =
            serde_json::from_str(&received.data).expect("data should be JSON");
        assert_eq!(data["name"], "read_file");
        assert_eq!(data["toolCallId"], "tc-1");
    }

    #[tokio::test]
    async fn tool_call_update_broadcasts_as_named_event() {
        let state = make_shared_state();
        let mut rx = state.event_tx.subscribe();
        let mut ch = DebugHttpChannel {
            state: state.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: None,
        };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!({
                "update": {
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": "tc-1",
                    "name": "read_file",
                    "status": "completed",
                    "output": "file contents"
                }
            }),
            meta: None,
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.try_recv().expect("should have received broadcast");
        assert_eq!(received.event_type.as_deref(), Some("tool_call_update"));
        let data: serde_json::Value =
            serde_json::from_str(&received.data).expect("data should be JSON");
        assert_eq!(data["name"], "read_file");
        assert_eq!(data["status"], "completed");
        assert_eq!(data["toolCallId"], "tc-1");
    }

    #[tokio::test]
    async fn when_api_key_configured_and_no_auth_header_then_returns_401() {
        let state = make_shared_state_with_key("secret-token");
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/message")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn when_api_key_configured_and_wrong_token_then_returns_401() {
        let state = make_shared_state_with_key("secret-token");
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/message")
            .header("content-type", "application/json")
            .header("authorization", "Bearer wrong-token")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn when_api_key_configured_and_correct_token_then_request_succeeds() {
        let state = make_shared_state_with_key("secret-token");
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/message")
            .header("content-type", "application/json")
            .header("authorization", "Bearer secret-token")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn when_api_key_configured_then_health_endpoint_exempt_from_auth() {
        let state = make_shared_state_with_key("secret-token");
        let app = build_router(state);
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn when_no_api_key_configured_then_requests_succeed_without_auth() {
        let state = make_shared_state();
        let app = build_router(state);
        let req = Request::builder()
            .method("POST")
            .uri("/message")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hello"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
