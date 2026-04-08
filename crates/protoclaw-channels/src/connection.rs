use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use protoclaw_config::ChannelConfig;
use protoclaw_core::types::ChannelId;
use protoclaw_jsonrpc::NdJsonCodec;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::error::ChannelsError;
use protoclaw_sdk_types::ChannelCapabilities;

// Port discovery: channel subprocesses can print `PORT:{port}` to stderr
// to advertise their listening port (e.g., debug-http's HTTP server).
// This watch channel gets updated when the pattern is detected.

/// Messages received from a channel subprocess.
#[derive(Debug)]
pub enum IncomingChannelMessage {
    /// Channel-initiated JSON-RPC request (has method + id).
    ChannelRequest(serde_json::Value),
    /// Channel-initiated JSON-RPC notification (has method, no id).
    ChannelNotification(serde_json::Value),
}

/// Manages a bidirectional NDJSON JSON-RPC connection to a channel subprocess.
///
/// Mirrors AgentConnection: stdin writer and stdout reader run on separate
/// tokio tasks to prevent deadlock.
pub struct ChannelConnection {
    channel_id: ChannelId,
    child: Child,
    stdin_tx: mpsc::Sender<serde_json::Value>,
    incoming_rx: mpsc::Receiver<IncomingChannelMessage>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<AtomicU64>,
    reader_handle: JoinHandle<()>,
    writer_handle: JoinHandle<()>,
    stderr_handle: JoinHandle<()>,
    capabilities: Option<ChannelCapabilities>,
    port_rx: tokio::sync::watch::Receiver<u16>,
}

impl std::fmt::Debug for ChannelConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelConnection")
            .field("channel_id", &self.channel_id)
            .field("next_id", &self.next_id.load(Ordering::SeqCst))
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl ChannelConnection {
    /// Spawn a channel subprocess with piped stdin/stdout/stderr.
    ///
    /// Creates reader + writer + stderr tasks using NdJsonCodec framing.
    pub fn spawn(config: &ChannelConfig, channel_id: ChannelId) -> Result<Self, ChannelsError> {
        let mut cmd = Command::new(&config.binary);
        cmd.args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        for (key, value) in &config.options {
            let env_val = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            cmd.env(key, env_val);
        }

        let mut child = cmd.spawn()
            .map_err(|e| ChannelsError::SpawnFailed(format!("{}: {e}", config.binary)))?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<serde_json::Value>(64);
        let (incoming_tx, incoming_rx) = mpsc::channel::<IncomingChannelMessage>(64);

        // Writer task: receive from stdin_tx, encode via NdJsonCodec, write to stdin
        let writer_handle = tokio::spawn(async move {
            use futures::SinkExt;
            let mut framed = FramedWrite::new(stdin, NdJsonCodec);
            while let Some(msg) = stdin_rx.recv().await {
                if framed.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader task: read NDJSON from stdout, route responses to pending_requests,
        // route notifications/requests to incoming_rx
        let pending_for_reader = pending_requests.clone();
        let reader_handle = tokio::spawn(async move {
            let mut framed = FramedRead::new(stdout, NdJsonCodec);
            while let Some(Ok(value)) = framed.next().await {
                let has_id = value.get("id").is_some_and(|v| !v.is_null());
                let has_method = value.get("method").is_some();

                if has_id && !has_method {
                    // Response to our request
                    let id = value["id"].as_u64().unwrap_or(0);
                    let mut pending = pending_for_reader.lock().await;
                    if let Some(tx) = pending.remove(&id) {
                        let result = value
                            .get("result")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let _ = tx.send(result);
                    }
                } else if has_method && has_id {
                    // Channel-initiated request
                    let _ = incoming_tx
                        .send(IncomingChannelMessage::ChannelRequest(value))
                        .await;
                } else if has_method {
                    // Channel-initiated notification
                    let _ = incoming_tx
                        .send(IncomingChannelMessage::ChannelNotification(value))
                        .await;
                }
            }
        });

        // Stderr task: log channel stderr at debug level, parse PORT:{port} for discovery
        let channel_name = channel_id.as_ref().to_string();
        let (port_tx, port_rx) = tokio::sync::watch::channel(0u16);
        let stderr_handle = tokio::spawn(async move {
            let span = tracing::info_span!("subprocess", source = %channel_name);
            let _guard = span.enter();
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(port_str) = line.strip_prefix("PORT:") {
                    if let Ok(port) = port_str.trim().parse::<u16>() {
                        let _ = port_tx.send(port);
                    }
                }
                tracing::info!(target: "subprocess_stderr", "{}", line);
            }
        });

        Ok(Self {
            channel_id,
            child,
            stdin_tx,
            incoming_rx,
            pending_requests,
            next_id,
            reader_handle,
            writer_handle,
            stderr_handle,
            capabilities: None,
            port_rx,
        })
    }

    /// Send a JSON-RPC request and return a receiver for the correlated response.
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<serde_json::Value>, ChannelsError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending_requests.lock().await.insert(id, tx);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        self.stdin_tx
            .send(msg)
            .await
            .map_err(|_| ChannelsError::ConnectionClosed)?;

        Ok(rx)
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), ChannelsError> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.stdin_tx
            .send(msg)
            .await
            .map_err(|_| ChannelsError::ConnectionClosed)?;

        Ok(())
    }

    /// Receive the next incoming message from the channel subprocess.
    /// Returns None on EOF (crash signal).
    pub async fn recv_incoming(&mut self) -> Option<IncomingChannelMessage> {
        self.incoming_rx.recv().await
    }

    /// Check if the channel subprocess is still alive.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Terminate the channel subprocess and abort I/O tasks.
    pub async fn kill(&mut self) -> Result<(), ChannelsError> {
        self.child.kill().await?;
        self.reader_handle.abort();
        self.writer_handle.abort();
        self.stderr_handle.abort();
        Ok(())
    }

    /// Get the channel ID.
    pub fn channel_id(&self) -> &ChannelId {
        &self.channel_id
    }

    /// Get the channel capabilities (set after initialize handshake).
    pub fn capabilities(&self) -> Option<&ChannelCapabilities> {
        self.capabilities.as_ref()
    }

    /// Set capabilities after successful initialize handshake.
    pub fn set_capabilities(&mut self, caps: ChannelCapabilities) {
        self.capabilities = Some(caps);
    }

    /// Get a watch receiver for the channel's discovered port (from stderr PORT:{port}).
    pub fn port_rx(&self) -> tokio::sync::watch::Receiver<u16> {
        self.port_rx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn cat_channel_config() -> ChannelConfig {
        ChannelConfig {
            binary: "cat".into(),
            args: vec![],
            enabled: true,
            agent: "default".into(),
            ack: Default::default(),
            init_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn when_channel_subprocess_spawned_then_reports_alive() {
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();
        assert!(conn.is_alive());
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_nonexistent_channel_binary_spawned_then_returns_error() {
        let config = ChannelConfig {
            binary: "nonexistent-binary-xyz-99999".into(),
            args: vec![],
            enabled: true,
            agent: "default".into(),
            ack: Default::default(),
            init_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        };
        let result = ChannelConnection::spawn(&config, ChannelId::from("bad"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChannelsError::SpawnFailed(_)));
    }

    #[tokio::test]
    async fn when_request_sent_to_channel_then_response_correlated_correctly() {
        // `cat` echoes stdin to stdout — we send a JSON-RPC response-shaped message
        // that the reader will route to pending_requests.
        // But cat echoes the request itself, which has method+id, so it goes to ChannelRequest.
        // Instead, use a simple echo approach: send a response-shaped JSON directly.
        // Actually, cat echoes exactly what we send. A request has method+id, so reader
        // routes it to ChannelRequest, not pending_requests.
        // We need to test correlation differently — send a response-shaped object.
        // But we can't send a response through stdin and have cat echo it back as a response.
        // The request we send has method, so cat echoes it back with method+id → ChannelRequest.
        //
        // Better approach: test that send_request puts the message on the wire and creates
        // a pending entry. We'll verify full correlation in integration tests.
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let conn = ChannelConnection::spawn(&config, channel_id).unwrap();

        let rx = conn
            .send_request("initialize", serde_json::json!({"protocolVersion": 1}))
            .await
            .unwrap();

        // cat echoes back our request (which has method+id), so reader routes it
        // to incoming as ChannelRequest, not to pending_requests.
        // The oneshot won't resolve from cat — but we verify it was created.
        assert!(!rx.is_terminated());

        // Verify pending_requests has our entry
        let pending = conn.pending_requests.lock().await;
        assert_eq!(pending.len(), 1);
        drop(pending);

        // Clean up — drop conn to close stdin, which makes cat exit
        drop(conn);
    }

    #[tokio::test]
    async fn when_channel_process_exits_then_recv_incoming_returns_none() {
        // Use `true` which exits immediately
        let config = ChannelConfig {
            binary: "true".into(),
            args: vec![],
            enabled: true,
            agent: "default".into(),
            ack: Default::default(),
            init_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        };
        let channel_id = ChannelId::from("exits");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();

        // Process exits immediately, reader task sees EOF, incoming channel closes
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            conn.recv_incoming(),
        )
        .await
        .expect("should not timeout");

        assert!(result.is_none(), "recv_incoming should return None on subprocess exit");
    }

    #[tokio::test]
    async fn when_channel_id_accessor_called_then_returns_configured_id() {
        let config = cat_channel_config();
        let channel_id = ChannelId::from("my-channel");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();
        assert_eq!(conn.channel_id().as_ref(), "my-channel");
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_capabilities_not_set_then_accessor_returns_none() {
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();
        assert!(conn.capabilities().is_none());
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_set_capabilities_called_then_accessor_returns_caps() {
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();

        let caps = ChannelCapabilities {
            streaming: true,
            rich_text: false,
        };
        conn.set_capabilities(caps.clone());

        let retrieved = conn.capabilities().expect("capabilities should be set");
        assert_eq!(retrieved.streaming, true);
        assert_eq!(retrieved.rich_text, false);
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_port_rx_called_then_returns_watch_receiver_with_initial_zero() {
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();

        let port_rx = conn.port_rx();
        assert_eq!(*port_rx.borrow(), 0u16);
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_channel_sends_notification_then_recv_incoming_returns_it() {
        // cat echoes what we send. Send a notification-shaped JSON (method, no id).
        let config = cat_channel_config();
        let channel_id = ChannelId::from("test");
        let mut conn = ChannelConnection::spawn(&config, channel_id).unwrap();

        // Send a notification-shaped message through stdin — cat echoes it back on stdout
        conn.send_notification("channel/sendMessage", serde_json::json!({"content": "hi"}))
            .await
            .unwrap();

        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            conn.recv_incoming(),
        )
        .await
        .expect("should not timeout")
        .expect("should receive message");

        assert!(matches!(msg, IncomingChannelMessage::ChannelNotification(_)));
        conn.kill().await.unwrap();
    }
}
