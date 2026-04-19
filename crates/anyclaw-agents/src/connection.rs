use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use futures::StreamExt;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{FramedRead, FramedWrite};

use anyclaw_config::{AgentConfig, WorkspaceConfig};
use anyclaw_jsonrpc::NdJsonCodec;
use anyclaw_jsonrpc::types::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId};

use crate::backend::ProcessBackend;
use crate::docker_backend::DockerBackend;
use crate::error::AgentsError;
use crate::local_backend::LocalBackend;
use crate::manager::SlotIncoming;

/// Messages received from the agent process (requests or notifications it initiates).
#[derive(Debug)]
pub enum IncomingMessage {
    /// Agent-initiated JSON-RPC request (has method + id).
    AgentRequest(JsonRpcRequest),
    /// Agent-initiated JSON-RPC notification (has method, no id).
    AgentNotification(JsonRpcRequest),
}

type StdioTriple = (
    Box<dyn ProcessBackend>,
    Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    Box<dyn tokio::io::AsyncRead + Unpin + Send>,
);

async fn build_backend(
    config: &AgentConfig,
    name: &str,
    log_level: Option<&str>,
) -> Result<StdioTriple, AgentsError> {
    match &config.workspace {
        WorkspaceConfig::Local(local) => {
            let work_dir = local.working_dir.as_deref().unwrap_or(Path::new("."));

            let (cmd_name, cmd_args) = local.binary.command_and_args();
            // LIMITATION: Subprocess binary paths not validated at spawn time
            // Agent binary paths from config are passed directly to Command::new() without
            // path sanitization or allowlisting. Config is trusted (loaded from anyclaw.yaml),
            // but a compromised config file could spawn arbitrary binaries. Current mitigation:
            // config is file-based (not user-input-driven) and kill_on_drop(true) limits orphans.
            // See also: CONCERNS.md §Security Concerns
            let mut cmd = Command::new(cmd_name);
            cmd.args(cmd_args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .envs(&local.env)
                .current_dir(work_dir);
            if let Some(level) = log_level {
                cmd.env("RUST_LOG", level);
            }
            let child = cmd
                .spawn()
                .map_err(|e| AgentsError::SpawnFailed(format!("{}: {e}", cmd_name)))?;

            let mut backend: Box<dyn ProcessBackend> = Box::new(LocalBackend::new(child));
            let stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send> =
                backend.take_stdin().expect("stdin was piped");
            let stdout: Box<dyn tokio::io::AsyncRead + Unpin + Send> =
                backend.take_stdout().expect("stdout was piped");
            let stderr: Box<dyn tokio::io::AsyncRead + Unpin + Send> =
                backend.take_stderr().expect("stderr was piped");
            Ok((backend, stdin, stdout, stderr))
        }
        WorkspaceConfig::Docker(docker_config) => {
            let mut backend: Box<dyn ProcessBackend> =
                Box::new(DockerBackend::spawn(docker_config, name).await?);
            let stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send> =
                backend.take_stdin().expect("stdin was attached");
            let stdout: Box<dyn tokio::io::AsyncRead + Unpin + Send> =
                backend.take_stdout().expect("stdout was attached");
            let stderr: Box<dyn tokio::io::AsyncRead + Unpin + Send> =
                backend.take_stderr().expect("stderr was attached");
            Ok((backend, stdin, stdout, stderr))
        }
    }
}

fn spawn_writer_task(
    stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    mut stdin_rx: mpsc::Receiver<JsonRpcMessage>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use futures::SinkExt;
        let mut framed = FramedWrite::new(stdin, NdJsonCodec);
        while let Some(msg) = stdin_rx.recv().await {
            let method = match &msg {
                JsonRpcMessage::Request(r) => r.method.as_str(),
                JsonRpcMessage::Response(_) => "_response",
            };
            tracing::debug!(%method, "writing to agent stdin");
            if framed.send(msg).await.is_err() {
                break;
            }
        }
    })
}

fn spawn_stderr_task(
    name: &str,
    stderr: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
) -> tokio::task::JoinHandle<()> {
    let agent_name = name.to_string();
    tokio::spawn(async move {
        let span = tracing::info_span!("subprocess", source = %agent_name);
        let _guard = span.enter();
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::info!(target: "subprocess_stderr", "{}", line);
        }
    })
}

fn init_incoming_channels(
    bridge: &Option<(usize, mpsc::Sender<SlotIncoming>)>,
) -> (
    Option<mpsc::Sender<IncomingMessage>>,
    Option<mpsc::Receiver<IncomingMessage>>,
) {
    if bridge.is_none() {
        let (tx, rx) = mpsc::channel::<IncomingMessage>(64);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    }
}

async fn handle_pending_response(
    pending_requests: &Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>>,
    response: JsonRpcResponse,
) {
    let id = match &response.id {
        Some(RequestId::Number(n)) => *n as u64,
        _ => 0,
    };
    if let Some((_, tx)) = pending_requests.remove(&id) {
        let _ = tx.send(response);
    }
}

async fn route_incoming_message(
    bridge: &Option<(usize, mpsc::Sender<SlotIncoming>)>,
    local_incoming_tx: &Option<mpsc::Sender<IncomingMessage>>,
    msg: IncomingMessage,
) -> bool {
    if let Some((slot_idx, bridge_tx)) = bridge {
        bridge_tx
            .send(SlotIncoming {
                slot_idx: *slot_idx,
                msg: Some(msg),
            })
            .await
            .is_ok()
    } else if let Some(local_tx) = local_incoming_tx {
        local_tx.send(msg).await.is_ok()
    } else {
        true
    }
}

fn spawn_reader_task(
    stdout: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    pending_requests: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>>,
    bridge: Option<(usize, mpsc::Sender<SlotIncoming>)>,
    local_incoming_tx: Option<mpsc::Sender<IncomingMessage>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut framed = FramedRead::new(stdout, NdJsonCodec);
        while let Some(frame) = framed.next().await {
            let typed_msg = match frame {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed line from agent stdout");
                    continue;
                }
            };

            match typed_msg {
                JsonRpcMessage::Response(response) => {
                    tracing::debug!(id = ?response.id, "agent response received");
                    handle_pending_response(&pending_requests, response).await;
                }
                JsonRpcMessage::Request(request) => {
                    tracing::debug!(method = %request.method, "agent request/notification received");
                    let msg = if request.id.is_some() {
                        IncomingMessage::AgentRequest(request)
                    } else {
                        IncomingMessage::AgentNotification(request)
                    };

                    if !route_incoming_message(&bridge, &local_incoming_tx, msg).await {
                        break;
                    }
                }
            }
        }

        if let Some((slot_idx, bridge_tx)) = bridge {
            let _ = bridge_tx
                .send(SlotIncoming {
                    slot_idx,
                    msg: None,
                })
                .await;
        }
    })
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

/// Manages a single agent subprocess's stdio connection and JSON-RPC framing.
///
/// Spawns reader/writer/stderr tasks that communicate via typed `JsonRpcMessage`.
/// In bridge mode (`spawn_with_bridge`), incoming messages flow directly to the
/// manager's shared channel — no intermediate forwarding task.
pub struct AgentConnection {
    backend: Box<dyn ProcessBackend>,
    stdin_tx: mpsc::Sender<JsonRpcMessage>,
    incoming_rx: Option<mpsc::Receiver<IncomingMessage>>,
    pending_requests: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>>,
    next_id: Arc<AtomicU64>,
    reader_handle: tokio::task::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    stderr_handle: tokio::task::JoinHandle<()>,
}

impl AgentConnection {
    /// Spawn an agent subprocess in standalone mode (own internal incoming channel).
    pub async fn spawn(config: &AgentConfig, name: &str) -> Result<Self, AgentsError> {
        Self::spawn_inner(config, name, None, None).await
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        backend: Box<dyn ProcessBackend>,
        stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
        stdout: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        stderr: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        name: &str,
        bridge: Option<(usize, mpsc::Sender<SlotIncoming>)>,
    ) -> Self {
        let pending_requests: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>> =
            Arc::new(DashMap::new());
        let next_id = Arc::new(AtomicU64::new(1));
        let (stdin_tx, stdin_rx) = mpsc::channel::<JsonRpcMessage>(64);
        let (local_incoming_tx, incoming_rx) = init_incoming_channels(&bridge);
        let writer_handle = spawn_writer_task(stdin, stdin_rx);
        let reader_handle = spawn_reader_task(
            stdout,
            Arc::clone(&pending_requests),
            bridge,
            local_incoming_tx,
        );
        let stderr_handle = spawn_stderr_task(name, stderr);
        Self {
            backend,
            stdin_tx,
            incoming_rx,
            pending_requests,
            next_id,
            reader_handle,
            writer_handle,
            stderr_handle,
        }
    }

    pub(crate) async fn spawn_with_bridge(
        config: &AgentConfig,
        name: &str,
        slot_idx: usize,
        bridge_tx: mpsc::Sender<SlotIncoming>,
        log_level: Option<&str>,
    ) -> Result<Self, AgentsError> {
        Self::spawn_inner(config, name, Some((slot_idx, bridge_tx)), log_level).await
    }

    async fn spawn_inner(
        config: &AgentConfig,
        name: &str,
        bridge: Option<(usize, mpsc::Sender<SlotIncoming>)>,
        log_level: Option<&str>,
    ) -> Result<Self, AgentsError> {
        let (backend, stdin, stdout, stderr) = build_backend(config, name, log_level).await?;

        let pending_requests: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>> =
            Arc::new(DashMap::new());
        let next_id = Arc::new(AtomicU64::new(1));

        let (stdin_tx, stdin_rx) = mpsc::channel::<JsonRpcMessage>(64);
        let (local_incoming_tx, incoming_rx) = init_incoming_channels(&bridge);

        let writer_handle = spawn_writer_task(stdin, stdin_rx);
        let reader_handle = spawn_reader_task(
            stdout,
            Arc::clone(&pending_requests),
            bridge,
            local_incoming_tx,
        );

        let stderr_handle = spawn_stderr_task(name, stderr);

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

    /// Send a JSON-RPC request and return a oneshot receiver for the response.
    // D-03: params schema varies per JSON-RPC method — cannot be typed at this layer
    #[allow(clippy::disallowed_types)]
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<JsonRpcResponse>, AgentsError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending_requests.insert(id, tx);

        let request = JsonRpcRequest::new(method, Some(RequestId::Number(id as i64)), Some(params));

        self.stdin_tx
            .send(JsonRpcMessage::Request(request))
            .await
            .map_err(|_| AgentsError::ConnectionClosed)?;

        Ok(rx)
    }

    /// Send a JSON-RPC notification (no response expected).
    // D-03: params schema varies per JSON-RPC method — cannot be typed at this layer
    #[allow(clippy::disallowed_types)]
    pub async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), AgentsError> {
        let request = JsonRpcRequest::new(method, None, Some(params));

        self.stdin_tx
            .send(JsonRpcMessage::Request(request))
            .await
            .map_err(|_| AgentsError::ConnectionClosed)?;

        Ok(())
    }

    /// Write a pre-built JSON-RPC response directly to the agent's stdin.
    /// Used for permission responses which are replies to agent-initiated requests.
    pub async fn send_raw(&self, msg: JsonRpcResponse) -> Result<(), AgentsError> {
        tracing::debug!(id = ?msg.id, "send_raw to agent stdin");
        self.stdin_tx
            .send(JsonRpcMessage::Response(msg))
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

    /// Check whether the agent subprocess is still running.
    pub fn is_alive(&mut self) -> bool {
        self.backend.is_alive()
    }

    /// Kill the agent subprocess and abort all I/O tasks.
    pub async fn kill(&mut self) -> Result<(), AgentsError> {
        self.backend.kill().await?;
        self.reader_handle.abort();
        self.writer_handle.abort();
        self.stderr_handle.abort();
        Ok(())
    }

    /// Wait for the agent subprocess to exit and return its exit status.
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, AgentsError> {
        self.backend.wait().await
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    pub(crate) struct MockBackend {
        alive: bool,
    }

    impl MockBackend {
        pub(crate) fn new(alive: bool) -> Self {
            Self { alive }
        }
    }

    impl ProcessBackend for MockBackend {
        fn is_alive(&mut self) -> bool {
            self.alive
        }
        fn take_stdin(&mut self) -> Option<Box<dyn tokio::io::AsyncWrite + Unpin + Send>> {
            None
        }
        fn take_stdout(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
            None
        }
        fn take_stderr(&mut self) -> Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> {
            None
        }
        fn kill(&mut self) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>> {
            self.alive = false;
            Box::pin(async { Ok(()) })
        }
        fn wait(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>
        {
            Box::pin(async {
                std::process::Command::new("true")
                    .status()
                    .map_err(AgentsError::Io)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::MockBackend;
    use super::*;
    use anyclaw_config::types::StringOrArray;
    use anyclaw_config::{LocalWorkspaceConfig, WorkspaceConfig};
    use rstest::rstest;

    use std::collections::HashMap;

    fn mock_agent_config() -> AgentConfig {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let target_dir = std::path::Path::new(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ext")
            .join("target")
            .join("debug")
            .join("mock-agent");

        AgentConfig {
            workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                binary: StringOrArray::from(target_dir.to_string_lossy().as_ref()),
                working_dir: None,
                env: HashMap::new(),
            }),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn when_mock_agent_spawned_then_process_starts_successfully() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config, "test-agent").await.unwrap();
        assert!(conn.is_alive());
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_nonexistent_binary_spawned_then_returns_error() {
        let config = AgentConfig {
            workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                binary: StringOrArray::from("nonexistent-binary-xyz-12345"),
                working_dir: None,
                env: HashMap::new(),
            }),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        };
        let result = AgentConnection::spawn(&config, "test-agent").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AgentsError::SpawnFailed(_)));
    }

    #[tokio::test]
    async fn when_request_sent_to_agent_then_response_received() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config, "test-agent").await.unwrap();

        let params = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": {}
        });
        let rx = conn.send_request("initialize", params).await.unwrap();
        let resp = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("timeout waiting for response")
            .expect("oneshot cancelled");

        // Response is now typed JsonRpcResponse — extract result for assertion
        let result = resp.result.expect("expected result in response");
        assert_eq!(result["protocolVersion"], 2);
        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_notification_sent_then_no_pending_request_created() {
        let config = mock_agent_config();
        let mut conn = AgentConnection::spawn(&config, "test-agent").await.unwrap();

        let params = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": {}
        });
        let rx = conn.send_request("initialize", params).await.unwrap();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;

        conn.send_notification("some/event", serde_json::json!({}))
            .await
            .unwrap();

        assert!(
            conn.pending_requests.is_empty(),
            "notification should not create pending request"
        );

        conn.kill().await.unwrap();
    }

    #[tokio::test]
    async fn when_mock_backend_started_then_reports_alive() {
        let mut backend = MockBackend::new(true);
        assert!(backend.is_alive());
        backend.kill().await.unwrap();
        assert!(!backend.is_alive());
    }

    #[tokio::test]
    async fn when_mock_backend_used_as_trait_object_then_works_correctly() {
        let mut backend: Box<dyn ProcessBackend> = Box::new(MockBackend::new(true));
        assert!(backend.is_alive());
    }

    #[rstest]
    #[tokio::test]
    async fn when_send_raw_called_then_exact_json_written_to_stdin() {
        use tokio::io::AsyncBufReadExt;

        let (stdin_write, stdin_read) = tokio::io::duplex(64 * 1024);
        let (stdout_write, _stdout_read) = tokio::io::duplex(64 * 1024);
        let (_stderr_write, stderr_read) = tokio::io::duplex(64 * 1024);

        let conn = AgentConnection::from_parts(
            Box::new(MockBackend::new(true)),
            Box::new(stdin_write),
            Box::new(stdout_write),
            Box::new(stderr_read),
            "test-agent",
            None,
        );

        // ACP permission response format per @agentclientprotocol/sdk@0.16.1:
        // result.outcome.outcome = "selected" | "cancelled"
        // result.outcome.optionId = string (when selected)
        let permission_response = JsonRpcResponse::success(
            Some(RequestId::Number(0)),
            serde_json::json!({
                "outcome": {
                    "outcome": "selected",
                    "optionId": "once",
                }
            }),
        );

        conn.send_raw(permission_response).await.unwrap();

        let mut reader = tokio::io::BufReader::new(stdin_read);
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            reader.read_line(&mut line),
        )
        .await
        .expect("timeout reading stdin")
        .expect("read error");

        let written: serde_json::Value = serde_json::from_str(line.trim()).expect("invalid JSON");
        assert_eq!(written["jsonrpc"], "2.0");
        assert_eq!(written["id"], 0);
        assert_eq!(written["result"]["outcome"]["outcome"], "selected");
        assert_eq!(written["result"]["outcome"]["optionId"], "once");
        // Must NOT have requestId at result level
        assert!(
            written["result"]["requestId"].is_null(),
            "requestId should not be in result"
        );
    }
}
