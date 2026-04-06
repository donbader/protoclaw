use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::codec::{FramedRead, FramedWrite};

use protoclaw_config::{AgentConfig, WorkspaceConfig};
use protoclaw_jsonrpc::NdJsonCodec;

use crate::backend::ProcessBackend;
use crate::error::AgentsError;
use crate::local_backend::LocalBackend;
use crate::manager::SlotIncoming;

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
    backend: Box<dyn ProcessBackend + Sync>,
    stdin_tx: mpsc::Sender<serde_json::Value>,
    incoming_rx: Option<mpsc::Receiver<IncomingMessage>>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<AtomicU64>,
    reader_handle: tokio::task::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    stderr_handle: tokio::task::JoinHandle<()>,
}

impl AgentConnection {
    pub fn spawn(config: &AgentConfig, name: &str) -> Result<Self, AgentsError> {
        Self::spawn_inner(config, name, None)
    }

    pub(crate) fn spawn_with_bridge(
        config: &AgentConfig,
        name: &str,
        slot_idx: usize,
        bridge_tx: mpsc::Sender<SlotIncoming>,
    ) -> Result<Self, AgentsError> {
        Self::spawn_inner(config, name, Some((slot_idx, bridge_tx)))
    }

    fn spawn_inner(
        config: &AgentConfig,
        name: &str,
        bridge: Option<(usize, mpsc::Sender<SlotIncoming>)>,
    ) -> Result<Self, AgentsError> {
        let (binary, env, working_dir) = match &config.workspace {
            WorkspaceConfig::Local(local) => (
                local.binary.clone(),
                local.env.clone(),
                local.working_dir.clone(),
            ),
            WorkspaceConfig::Docker(_) => {
                return Err(AgentsError::SpawnFailed(
                    format!("{name}: Docker workspace not yet supported"),
                ));
            }
        };

        let work_dir = working_dir
            .as_deref()
            .unwrap_or(Path::new("."));

        let child = Command::new(&binary)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .envs(&env)
            .current_dir(work_dir)
            .spawn()
            .map_err(|e| AgentsError::SpawnFailed(format!("{binary}: {e}")))?;

        let mut backend: Box<dyn ProcessBackend + Sync> = Box::new(LocalBackend::new(child));

        let stdin: Box<dyn AsyncWrite + Unpin + Send> =
            backend.take_stdin().expect("stdin was piped");
        let stdout: Box<dyn AsyncRead + Unpin + Send> =
            backend.take_stdout().expect("stdout was piped");
        let stderr: Box<dyn AsyncRead + Unpin + Send> =
            backend.take_stderr().expect("stderr was piped");

        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<serde_json::Value>(64);

        let (local_incoming_tx, incoming_rx) = if bridge.is_none() {
            let (tx, rx) = mpsc::channel::<IncomingMessage>(64);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

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
            while let Some(frame) = framed.next().await {
                let value = match frame {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping malformed line from agent stdout");
                        continue;
                    }
                };

                tracing::debug!(raw = %value, "agent stdout line");

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
                } else if has_method {
                    let msg = if has_id {
                        IncomingMessage::AgentRequest(value)
                    } else {
                        IncomingMessage::AgentNotification(value)
                    };
                    if let Some((slot_idx, ref bridge_tx)) = bridge {
                        if bridge_tx.send(SlotIncoming { slot_idx, msg: Some(msg) }).await.is_err() {
                            break;
                        }
                    } else if let Some(ref local_tx) = local_incoming_tx {
                        if local_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
            if let Some((slot_idx, ref bridge_tx)) = bridge {
                let _ = bridge_tx.send(SlotIncoming { slot_idx, msg: None }).await;
            }
        });

        let agent_name = name.to_string();
        let stderr_handle = tokio::spawn(async move {
            let span = tracing::info_span!("subprocess", source = %agent_name);
            let _guard = span.enter();
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(target: "subprocess_stderr", "{}", line);
            }
        });

        Ok(Self {
            backend,
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

    /// Take ownership of the incoming message receiver.
    /// Used by AgentsManager to merge all agent streams into a single channel.
    /// Panics if called more than once (receiver already taken).
    pub fn take_incoming_rx(&mut self) -> mpsc::Receiver<IncomingMessage> {
        self.incoming_rx.take().expect("incoming_rx already taken")
    }

    pub fn is_alive(&mut self) -> bool {
        self.backend.is_alive()
    }

    pub async fn kill(&mut self) -> Result<(), AgentsError> {
        self.backend.kill().await?;
        self.reader_handle.abort();
        self.writer_handle.abort();
        self.stderr_handle.abort();
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, AgentsError> {
        self.backend.wait().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use protoclaw_config::{LocalWorkspaceConfig, WorkspaceConfig};

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
            workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                binary: target_dir.to_string_lossy().to_string(),
                working_dir: None,
                env: HashMap::new(),
            }),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn spawn_mock_agent() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config, "test-agent").unwrap();
        assert!(conn.is_alive());
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn spawn_nonexistent_binary_returns_error() {
        let config = AgentConfig {
            workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                binary: "nonexistent-binary-xyz-12345".to_string(),
                working_dir: None,
                env: HashMap::new(),
            }),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        };
        let result = AgentConnection::spawn(&config, "test-agent");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AgentsError::SpawnFailed(_)));
    }

    #[tokio::test]
    async fn send_request_and_receive_response() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config, "test-agent").unwrap();

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
        let mut conn = AgentConnection::spawn(&config, "test-agent").unwrap();

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

    use std::future::Future;
    use std::pin::Pin;

    struct MockBackend {
        alive: bool,
    }

    impl MockBackend {
        fn new(alive: bool) -> Self {
            Self { alive }
        }
    }

    impl ProcessBackend for MockBackend {
        fn is_alive(&mut self) -> bool {
            self.alive
        }
        fn take_stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>> {
            None
        }
        fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
            None
        }
        fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
            None
        }
        fn kill(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>> {
            self.alive = false;
            Box::pin(async { Ok(()) })
        }
        fn wait(
            &mut self,
        ) -> Pin<
            Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>,
        > {
            Box::pin(async {
                std::process::Command::new("true")
                    .status()
                    .map_err(AgentsError::Io)
            })
        }
    }

    #[tokio::test]
    async fn mock_backend_is_alive() {
        let mut backend = MockBackend::new(true);
        assert!(backend.is_alive());
        backend.kill().await.unwrap();
        assert!(!backend.is_alive());
    }

    #[tokio::test]
    async fn mock_backend_as_trait_object() {
        let mut backend: Box<dyn ProcessBackend> = Box::new(MockBackend::new(true));
        assert!(backend.is_alive());
    }
}
