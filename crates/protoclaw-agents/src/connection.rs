use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::codec::{FramedRead, FramedWrite};

use protoclaw_config::AgentConfig;
use protoclaw_jsonrpc::NdJsonCodec;

use crate::error::AgentsError;

/// Messages received from the agent process (requests or notifications it initiates).
#[derive(Debug)]
pub enum IncomingMessage {
    /// Agent-initiated JSON-RPC request (has method + id).
    AgentRequest(serde_json::Value),
    /// Agent-initiated JSON-RPC notification (has method, no id).
    AgentNotification(serde_json::Value),
}

/// Manages a bidirectional NDJSON JSON-RPC connection to an agent subprocess.
///
/// Stdin writer and stdout reader run on separate tokio tasks to prevent deadlock.
impl std::fmt::Debug for AgentConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConnection")
            .field("next_id", &self.next_id.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

pub struct AgentConnection {
    child: Child,
    stdin_tx: mpsc::Sender<serde_json::Value>,
    incoming_rx: mpsc::Receiver<IncomingMessage>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<AtomicU64>,
    reader_handle: tokio::task::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    stderr_handle: tokio::task::JoinHandle<()>,
}

impl AgentConnection {
    pub fn spawn(config: &AgentConfig) -> Result<Self, AgentsError> {
        let working_dir = config
            .working_dir
            .as_deref()
            .unwrap_or(Path::new("."));

        let mut child = Command::new(&config.binary)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .envs(&config.env)
            .current_dir(working_dir)
            .spawn()
            .map_err(|e| AgentsError::SpawnFailed(format!("{}: {e}", config.binary)))?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<serde_json::Value>(64);
        let (incoming_tx, incoming_rx) = mpsc::channel::<IncomingMessage>(64);

        let writer_handle = tokio::spawn(async move {
            use futures::SinkExt;
            let mut framed = FramedWrite::new(stdin, NdJsonCodec);
            while let Some(msg) = stdin_rx.recv().await {
                if framed.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let pending_for_reader = pending_requests.clone();
        let reader_handle = tokio::spawn(async move {
            let mut framed = FramedRead::new(stdout, NdJsonCodec);
            while let Some(Ok(value)) = framed.next().await {
                let has_id = value.get("id").is_some_and(|v| !v.is_null());
                let has_method = value.get("method").is_some();

                if has_id && !has_method {
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
                    let _ = incoming_tx.send(IncomingMessage::AgentRequest(value)).await;
                } else if has_method {
                    let _ = incoming_tx.send(IncomingMessage::AgentNotification(value)).await;
                }
            }
        });

        let stderr_handle = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "agent_stderr", "{}", line);
            }
        });

        Ok(Self {
            child,
            stdin_tx,
            incoming_rx,
            pending_requests,
            next_id,
            reader_handle,
            writer_handle,
            stderr_handle,
        })
    }

    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<serde_json::Value>, AgentsError> {
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
            .map_err(|_| AgentsError::ConnectionClosed)?;

        Ok(rx)
    }

    pub async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), AgentsError> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.stdin_tx
            .send(msg)
            .await
            .map_err(|_| AgentsError::ConnectionClosed)?;

        Ok(())
    }

    pub async fn recv_incoming(&mut self) -> Option<IncomingMessage> {
        self.incoming_rx.recv().await
    }

    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    pub async fn kill(&mut self) -> Result<(), AgentsError> {
        self.child.kill().await?;
        self.reader_handle.abort();
        self.writer_handle.abort();
        self.stderr_handle.abort();
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, AgentsError> {
        self.child.wait().await.map_err(AgentsError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mock_agent_config() -> AgentConfig {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let target_dir = std::path::Path::new(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("mock-agent");

        AgentConfig {
            binary: target_dir.to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            env: HashMap::new(),
            working_dir: None,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
        }
    }

    #[tokio::test]
    async fn spawn_mock_agent() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config).unwrap();
        assert!(conn.is_alive());
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn spawn_nonexistent_binary_returns_error() {
        let config = AgentConfig {
            binary: "nonexistent-binary-xyz-12345".to_string(),
            args: vec![],
            enabled: true,
            env: HashMap::new(),
            working_dir: None,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
        };
        let result = AgentConnection::spawn(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AgentsError::SpawnFailed(_)));
    }

    #[tokio::test]
    async fn send_request_and_receive_response() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config).unwrap();

        let params = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": {}
        });
        let rx = conn.send_request("initialize", params).await.unwrap();
        let resp = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("timeout waiting for response")
            .expect("oneshot cancelled");

        assert_eq!(resp["protocolVersion"], 1);
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn send_notification_does_not_create_pending_request() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config).unwrap();

        let params = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": {}
        });
        let rx = conn.send_request("initialize", params).await.unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;

        conn.send_notification("some/event", serde_json::json!({}))
            .await
            .unwrap();

        let pending = conn.pending_requests.lock().await;
        assert!(pending.is_empty(), "notification should not create pending request");

        drop(pending);
        conn.kill().await.unwrap();
    }
}
