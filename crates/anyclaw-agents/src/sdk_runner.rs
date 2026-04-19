use std::path::PathBuf;

use agent_client_protocol::Error as AcpSdkError;
use agent_client_protocol::Result as AcpResult;
use agent_client_protocol::{
    Agent, CancelNotification, Client, ClientCapabilities, ClientSideConnection, ContentBlock,
    ForkSessionRequest, ForkSessionResponse, InitializeRequest, InitializeResponse,
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse, ProtocolVersion,
    ReadTextFileRequest, ReadTextFileResponse, RequestPermissionRequest, RequestPermissionResponse,
    ResumeSessionRequest, ResumeSessionResponse, SessionId, SessionNotification,
    WriteTextFileRequest, WriteTextFileResponse,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::backend::ProcessBackend;
use crate::connection::IncomingMessage;
use crate::error::AgentsError;
use crate::manager::SlotIncoming;

// D-03: options and mcp_servers are deployment-defined schemas that cannot be typed at this layer
#[allow(clippy::disallowed_types)]
pub(crate) enum AgentRunnerCommand {
    Initialize {
        protocol_version: u32,
        capabilities: ClientCapabilities,
        options: Option<std::collections::HashMap<String, serde_json::Value>>,
        reply: oneshot::Sender<Result<InitializeResponse, AgentsError>>,
    },
    NewSession {
        cwd: String,
        mcp_servers: Vec<serde_json::Value>,
        reply: oneshot::Sender<Result<NewSessionResponse, AgentsError>>,
    },
    Prompt {
        session_id: String,
        content: Vec<ContentBlock>,
        reply: oneshot::Sender<Result<PromptResponse, AgentsError>>,
    },
    Cancel {
        session_id: String,
    },
    LoadSession {
        session_id: String,
        cwd: String,
        mcp_servers: Vec<serde_json::Value>,
        reply: oneshot::Sender<Result<LoadSessionResponse, AgentsError>>,
    },
    ResumeSession {
        session_id: String,
        cwd: String,
        mcp_servers: Vec<serde_json::Value>,
        reply: oneshot::Sender<Result<ResumeSessionResponse, AgentsError>>,
    },
    ForkSession {
        session_id: String,
        cwd: String,
        reply: oneshot::Sender<Result<ForkSessionResponse, AgentsError>>,
    },
    ListSessions {
        reply: oneshot::Sender<Result<ListSessionsResponse, AgentsError>>,
    },
    Keepalive,
    Kill,
}

pub(crate) enum AgentRunnerEvent {
    SessionNotification(SessionNotification),
    PermissionRequest {
        args: RequestPermissionRequest,
        reply: oneshot::Sender<AcpResult<RequestPermissionResponse>>,
    },
    FsRead {
        args: ReadTextFileRequest,
        reply: oneshot::Sender<AcpResult<ReadTextFileResponse>>,
    },
    FsWrite {
        args: WriteTextFileRequest,
        reply: oneshot::Sender<AcpResult<WriteTextFileResponse>>,
    },
    ConnectionClosed,
}

struct AnyclawClientHandler {
    event_tx: mpsc::Sender<AgentRunnerEvent>,
}

#[async_trait::async_trait(?Send)]
impl Client for AnyclawClientHandler {
    async fn session_notification(&self, args: SessionNotification) -> AcpResult<()> {
        self.event_tx
            .send(AgentRunnerEvent::SessionNotification(args))
            .await
            .map_err(|_| AcpSdkError::internal_error().data("event channel closed"))
    }

    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> AcpResult<RequestPermissionResponse> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.event_tx
            .send(AgentRunnerEvent::PermissionRequest {
                args,
                reply: reply_tx,
            })
            .await
            .map_err(|_| AcpSdkError::internal_error().data("event channel closed"))?;
        reply_rx
            .await
            .unwrap_or_else(|_| Err(AcpSdkError::internal_error().data("permission reply dropped")))
    }

    async fn read_text_file(&self, args: ReadTextFileRequest) -> AcpResult<ReadTextFileResponse> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.event_tx
            .send(AgentRunnerEvent::FsRead {
                args,
                reply: reply_tx,
            })
            .await
            .map_err(|_| AcpSdkError::internal_error().data("event channel closed"))?;
        reply_rx
            .await
            .unwrap_or_else(|_| Err(AcpSdkError::internal_error().data("fs read reply dropped")))
    }

    async fn write_text_file(
        &self,
        args: WriteTextFileRequest,
    ) -> AcpResult<WriteTextFileResponse> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.event_tx
            .send(AgentRunnerEvent::FsWrite {
                args,
                reply: reply_tx,
            })
            .await
            .map_err(|_| AcpSdkError::internal_error().data("event channel closed"))?;
        reply_rx
            .await
            .unwrap_or_else(|_| Err(AcpSdkError::internal_error().data("fs write reply dropped")))
    }
}

async fn agent_runner(
    mut cmd_rx: mpsc::Receiver<AgentRunnerCommand>,
    event_tx: mpsc::Sender<AgentRunnerEvent>,
    stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    stdout: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
) {
    let handler = AnyclawClientHandler {
        event_tx: event_tx.clone(),
    };
    let (conn, io_task) =
        ClientSideConnection::new(handler, stdin.compat_write(), stdout.compat(), |fut| {
            tokio::task::spawn_local(fut);
        });

    let conn = std::rc::Rc::new(conn);

    let command_loop = async {
        while let Some(cmd) = cmd_rx.recv().await {
            let conn = conn.clone();
            tokio::task::spawn_local(async move {
                handle_runner_command(cmd, &conn).await;
            });
        }
    };

    tokio::select! {
        result = io_task => {
            if let Err(e) = result {
                tracing::warn!(error = %e, "agent SDK io_task ended with error");
            }
        }
        _ = command_loop => {
            tracing::debug!("agent runner command channel closed");
        }
    }

    let _ = event_tx.send(AgentRunnerEvent::ConnectionClosed).await;
}

#[allow(clippy::disallowed_types)]
async fn handle_runner_command(cmd: AgentRunnerCommand, conn: &ClientSideConnection) {
    match cmd {
        AgentRunnerCommand::Initialize {
            protocol_version,
            capabilities,
            options,
            reply,
        } => {
            let mut request =
                InitializeRequest::new(ProtocolVersion::from(protocol_version as u16));
            request.client_capabilities = capabilities;
            if let Some(opts) = options {
                let mut meta = serde_json::Map::new();
                meta.insert(
                    "options".into(),
                    serde_json::Value::Object(opts.into_iter().collect()),
                );
                request.meta = Some(meta);
            }
            let result = conn
                .initialize(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::NewSession {
            cwd,
            mcp_servers,
            reply,
        } => {
            let mut request = NewSessionRequest::new(PathBuf::from(&cwd));
            if !mcp_servers.is_empty() {
                request.mcp_servers = mcp_servers
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect();
            }
            let result = conn
                .new_session(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::Prompt {
            session_id,
            content,
            reply,
        } => {
            let request = PromptRequest::new(SessionId::new(session_id), content);
            let result = conn
                .prompt(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::Cancel { session_id } => {
            let notification = CancelNotification::new(SessionId::new(session_id));
            let _ = conn.cancel(notification).await;
        }
        AgentRunnerCommand::LoadSession {
            session_id,
            cwd,
            mcp_servers,
            reply,
        } => {
            let mut request =
                LoadSessionRequest::new(SessionId::new(session_id), PathBuf::from(&cwd));
            if !mcp_servers.is_empty() {
                request.mcp_servers = mcp_servers
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect();
            }
            let result = conn
                .load_session(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::ResumeSession {
            session_id,
            cwd,
            mcp_servers,
            reply,
        } => {
            let mut request =
                ResumeSessionRequest::new(SessionId::new(session_id), PathBuf::from(&cwd));
            if !mcp_servers.is_empty() {
                request.mcp_servers = mcp_servers
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect();
            }
            let result = conn
                .resume_session(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::ForkSession {
            session_id,
            cwd,
            reply,
        } => {
            let request = ForkSessionRequest::new(SessionId::new(session_id), PathBuf::from(&cwd));
            let result = conn
                .fork_session(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::ListSessions { reply } => {
            let request = ListSessionsRequest::default();
            let result = conn
                .list_sessions(request)
                .await
                .map_err(|e| sdk_err_to_agents_err(&e));
            let _ = reply.send(result);
        }
        AgentRunnerCommand::Keepalive => {
            // TODO: The SDK's ClientSideConnection doesn't expose a raw notification
            // method, so we can't send a keepalive JSON-RPC notification to prevent
            // Docker attach connection idle timeouts. The old AgentConnection used
            // send_notification("keepalive", {}) for this. Once the SDK adds a ping
            // or raw-write API, wire it here.
            tracing::trace!("keepalive received (no-op — SDK lacks raw notification API)");
        }
        AgentRunnerCommand::Kill => {}
    }
}

fn sdk_err_to_agents_err(e: &AcpSdkError) -> AgentsError {
    AgentsError::Protocol(crate::acp_error::AcpError::Transport(e.to_string()))
}

pub(crate) struct AgentRunnerHandle {
    pub(crate) cmd_tx: mpsc::Sender<AgentRunnerCommand>,
    pub(crate) event_rx: Option<mpsc::Receiver<AgentRunnerEvent>>,
    pub(crate) backend: Box<dyn ProcessBackend>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl AgentRunnerHandle {
    pub(crate) async fn kill(&mut self) -> Result<(), AgentsError> {
        let _ = self.cmd_tx.send(AgentRunnerCommand::Kill).await;
        self.backend.kill().await?;
        Ok(())
    }

    #[allow(dead_code)] // Used by session_recovery crash detection
    pub(crate) fn is_alive(&mut self) -> bool {
        self.backend.is_alive()
    }

    #[allow(dead_code)] // Used by session_recovery crash detection
    pub(crate) async fn wait(&mut self) -> Result<std::process::ExitStatus, AgentsError> {
        self.backend.wait().await
    }

    pub(crate) fn take_event_rx(&mut self) -> mpsc::Receiver<AgentRunnerEvent> {
        self.event_rx.take().expect("event_rx already taken")
    }
}

impl Drop for AgentRunnerHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) async fn spawn_agent_runner(
    config: &anyclaw_config::AgentConfig,
    name: &str,
    log_level: Option<&str>,
) -> Result<AgentRunnerHandle, AgentsError> {
    let (backend, stdin, stdout, stderr) =
        crate::connection::build_backend(config, name, log_level).await?;

    crate::connection::spawn_stderr_task(name, stderr);

    let (cmd_tx, cmd_rx) = mpsc::channel::<AgentRunnerCommand>(32);
    let (event_tx, event_rx) = mpsc::channel::<AgentRunnerEvent>(256);

    let agent_name = name.to_string();
    let thread_handle = std::thread::Builder::new()
        .name(format!("agent-{agent_name}"))
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build agent tokio runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, agent_runner(cmd_rx, event_tx, stdin, stdout));
        })
        .map_err(|e| AgentsError::SpawnFailed(format!("thread spawn: {e}")))?;

    Ok(AgentRunnerHandle {
        cmd_tx,
        event_rx: Some(event_rx),
        backend,
        thread_handle: Some(thread_handle),
    })
}

pub(crate) fn spawn_event_forwarder(
    slot_idx: usize,
    mut event_rx: mpsc::Receiver<AgentRunnerEvent>,
    incoming_tx: mpsc::Sender<SlotIncoming>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let is_closed = matches!(event, AgentRunnerEvent::ConnectionClosed);

            let msg = match event {
                AgentRunnerEvent::SessionNotification(notif) => {
                    Some(IncomingMessage::SdkSessionNotification(notif))
                }
                AgentRunnerEvent::PermissionRequest { args, reply } => {
                    Some(IncomingMessage::SdkPermissionRequest { args, reply })
                }
                AgentRunnerEvent::FsRead { args, reply } => {
                    Some(IncomingMessage::SdkFsRead { args, reply })
                }
                AgentRunnerEvent::FsWrite { args, reply } => {
                    Some(IncomingMessage::SdkFsWrite { args, reply })
                }
                AgentRunnerEvent::ConnectionClosed => None,
            };

            let slot_msg = SlotIncoming { slot_idx, msg };
            if incoming_tx.send(slot_msg).await.is_err() || is_closed {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_agent_runner_command_kill_constructed_then_no_panic() {
        let _cmd = AgentRunnerCommand::Kill;
    }

    #[rstest]
    fn when_agent_runner_event_connection_closed_constructed_then_no_panic() {
        let _event = AgentRunnerEvent::ConnectionClosed;
    }
}
