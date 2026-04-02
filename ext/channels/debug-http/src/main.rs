use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_stream::StreamExt;

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

/// Shared state between stdio reader task and axum HTTP handlers.
#[derive(Clone)]
struct AppState {
    /// Send JSON-RPC messages to stdout (channel → protoclaw).
    stdout_tx: Arc<Mutex<tokio::io::Stdout>>,
    /// Broadcast agent updates to SSE subscribers.
    event_tx: broadcast::Sender<String>,
    /// Pending permission requests from the agent.
    pending_permissions: Arc<RwLock<Vec<PendingPermission>>>,
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let (event_tx, _) = broadcast::channel::<String>(256);
    let stdout_tx = Arc::new(Mutex::new(tokio::io::stdout()));

    let state = AppState {
        stdout_tx: stdout_tx.clone(),
        event_tx: event_tx.clone(),
        pending_permissions: Arc::new(RwLock::new(Vec::new())),
    };

    // Handle initialize handshake from protoclaw on stdin
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Wait for initialize request
    if let Ok(Some(line)) = lines.next_line().await {
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            let method = msg["method"].as_str().unwrap_or("");
            if method == "initialize" {
                let id = msg.get("id").cloned();
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": 1,
                        "capabilities": {
                            "streaming": true,
                            "richText": false
                        }
                    }
                });
                write_stdout(&stdout_tx, &resp).await;
                tracing::info!("initialize handshake complete");
            }
        }
    }

    // Start HTTP server
    let router = build_router(state.clone());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    let bound_port = listener.local_addr().unwrap().port();

    // Port discovery: print to stderr for ChannelsManager
    eprintln!("PORT:{bound_port}");
    tracing::info!(port = bound_port, "debug-http listening");

    // Spawn HTTP server
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    // Spawn stdin reader task — routes deliver_message to SSE, request_permission to pending store
    let reader_state = state.clone();
    let stdin_handle = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let msg: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let method = msg["method"].as_str().unwrap_or("");
            let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);

            match method {
                "channel/deliverMessage" => {
                    let content = params.get("content").cloned().unwrap_or(serde_json::Value::Null);
                    let content_str = match content {
                        serde_json::Value::String(s) => s,
                        other => serde_json::to_string(&other).unwrap_or_default(),
                    };
                    let _ = reader_state.event_tx.send(content_str);
                }
                "channel/requestPermission" => {
                    let perm = PendingPermission {
                        request_id: params["requestId"].as_str().unwrap_or("").to_string(),
                        session_id: params["sessionId"].as_str().unwrap_or("").to_string(),
                        description: params["description"].as_str().unwrap_or("").to_string(),
                        options: params.get("options").cloned().unwrap_or(serde_json::json!([])),
                    };
                    reader_state.pending_permissions.write().await.push(perm);
                }
                _ => {
                    tracing::debug!(method = %method, "unhandled stdin message");
                }
            }
        }
        tracing::info!("stdin closed, shutting down");
    });

    // Wait for stdin EOF (protoclaw killed us) or HTTP server exit
    tokio::select! {
        _ = stdin_handle => {
            tracing::info!("stdin reader exited");
        }
        _ = http_handle => {
            tracing::info!("http server exited");
        }
    }
}

async fn write_stdout(stdout_tx: &Arc<Mutex<tokio::io::Stdout>>, msg: &serde_json::Value) {
    let mut line = serde_json::to_string(msg).expect("failed to serialize");
    line.push('\n');
    let mut stdout = stdout_tx.lock().await;
    stdout.write_all(line.as_bytes()).await.expect("failed to write stdout");
    stdout.flush().await.expect("failed to flush stdout");
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

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_message(
    State(state): State<AppState>,
    Json(body): Json<MessageBody>,
) -> impl IntoResponse {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "channel/sendMessage",
        "params": {
            "peerInfo": {
                "channelName": "debug-http",
                "peerId": "local",
                "kind": "local"
            },
            "content": body.message
        }
    });
    write_stdout(&state.stdout_tx, &msg).await;
    (
        axum::http::StatusCode::OK,
        Json(serde_json::json!({"status": "sent"})),
    )
}

async fn handle_events(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn handle_cancel(State(state): State<AppState>) -> Json<serde_json::Value> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "channel/cancelOperation",
        "params": {}
    });
    write_stdout(&state.stdout_tx, &msg).await;
    Json(serde_json::json!({"status": "cancelled"}))
}

async fn handle_permissions_pending(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let perms = state.pending_permissions.read().await;
    let items: Vec<serde_json::Value> = perms
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(serde_json::json!(null)))
        .collect();
    Json(serde_json::Value::Array(items))
}

async fn handle_permission_respond(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PermissionResponseBody>,
) -> Json<serde_json::Value> {
    // Remove from pending
    {
        let mut perms = state.pending_permissions.write().await;
        perms.retain(|p| p.request_id != id);
    }

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "channel/respondPermission",
        "params": {
            "requestId": id,
            "optionId": body.option_id
        }
    });
    write_stdout(&state.stdout_tx, &msg).await;
    Json(serde_json::json!({"status": "responded"}))
}
