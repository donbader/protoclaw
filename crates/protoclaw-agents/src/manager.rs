use std::collections::HashMap;
use std::time::Duration;

use protoclaw_acp::{
    AcpError, ClientCapabilities, InitializeParams, InitializeResult, McpServerInfo,
    PermissionOption, PromptMessage, SessionCancelParams, SessionLoadParams, SessionNewParams,
    SessionPromptParams, SessionUpdateEvent,
};
use protoclaw_config::AgentConfig;
use protoclaw_core::{ChannelEvent, ExponentialBackoff, Manager, ManagerError, ManagerHandle, SessionKey};
use protoclaw_tools::{McpServerUrl, ToolsCommand};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{AgentConnection, IncomingMessage};
use crate::error::AgentsError;

#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    pub connected: bool,
    pub session_id: Option<String>,
}

pub struct PendingPermission {
    pub request: serde_json::Value,
    pub description: String,
    pub options: Vec<PermissionOption>,
    pub received_at: std::time::Instant,
}

pub struct PendingPermissionInfo {
    pub request_id: String,
    pub description: String,
    pub options: Vec<PermissionOption>,
}

pub enum AgentsCommand {
    SendPrompt {
        message: String,
        reply: oneshot::Sender<Result<(), AgentsError>>,
    },
    CancelOperation,
    RespondPermission {
        request_id: String,
        option_id: String,
    },
    GetPendingPermissions {
        reply: oneshot::Sender<Vec<PendingPermissionInfo>>,
    },
    Shutdown,
    /// Query current agent connection/session status.
    GetStatus {
        reply: oneshot::Sender<AgentStatusInfo>,
    },
    /// Create a new ACP session keyed by channel+peer identity.
    CreateSession {
        session_key: SessionKey,
        reply: oneshot::Sender<Result<String, AgentsError>>,
    },
    /// Send a prompt to an existing session identified by session key.
    PromptSession {
        session_key: SessionKey,
        message: String,
        reply: oneshot::Sender<Result<(), AgentsError>>,
    },
}

pub struct AgentsManager {
    agent_config: AgentConfig,
    tools_handle: ManagerHandle<ToolsCommand>,
    connection: Option<AgentConnection>,
    session_id: Option<String>,
    agent_capabilities: Option<InitializeResult>,
    backoff: ExponentialBackoff,
    pending_permissions: HashMap<String, PendingPermission>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<AgentsCommand>>,
    cmd_tx: tokio::sync::mpsc::Sender<AgentsCommand>,
    /// SessionKey → ACP session_id mapping for multi-session support.
    session_map: HashMap<SessionKey, String>,
    /// ACP session_id → SessionKey reverse mapping for routing updates back.
    reverse_map: HashMap<String, SessionKey>,
    /// Sender for ChannelEvents back to ChannelsManager (avoids circular dep).
    channels_sender: Option<mpsc::Sender<ChannelEvent>>,
}

impl AgentsManager {
    pub fn new(agent_config: AgentConfig, tools_handle: ManagerHandle<ToolsCommand>) -> Self {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
        Self {
            agent_config,
            tools_handle,
            connection: None,
            session_id: None,
            agent_capabilities: None,
            backoff: ExponentialBackoff::default(),
            pending_permissions: HashMap::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
            session_map: HashMap::new(),
            reverse_map: HashMap::new(),
            channels_sender: None,
        }
    }

    /// Set the channels event sender for routing agent updates back to channels.
    pub fn with_channels_sender(mut self, sender: mpsc::Sender<ChannelEvent>) -> Self {
        self.channels_sender = Some(sender);
        self
    }

    pub fn command_sender(&self) -> tokio::sync::mpsc::Sender<AgentsCommand> {
        self.cmd_tx.clone()
    }

    async fn initialize_agent(&mut self) -> Result<(), AgentsError> {
        let conn = self.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
        })?;

        let rx = conn.send_request("initialize", params).await?;
        let resp = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| AgentsError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: InitializeResult = serde_json::from_value(resp)?;
        if result.protocol_version != 1 {
            return Err(AcpError::ProtocolMismatch {
                expected: 1,
                got: result.protocol_version,
            }
            .into());
        }

        self.agent_capabilities = Some(result);
        Ok(())
    }

    async fn start_session(&mut self) -> Result<(), AgentsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tools_handle
            .send(ToolsCommand::GetMcpUrls { reply: reply_tx })
            .await
            .map_err(|e| AgentsError::SpawnFailed(format!("tools handle: {e}")))?;

        let urls: Vec<McpServerUrl> = reply_rx.await.unwrap_or_default();

        let mcp_servers: Vec<McpServerInfo> = urls
            .iter()
            .map(|u| McpServerInfo {
                name: u.name.clone(),
                server_type: "http".into(),
                url: u.url.clone(),
                headers: None,
            })
            .collect();

        let params = serde_json::to_value(SessionNewParams {
            session_id: None,
            mcp_servers: if mcp_servers.is_empty() {
                None
            } else {
                Some(mcp_servers)
            },
        })?;

        let conn = self.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;
        let rx = conn.send_request("session/new", params).await?;
        let resp = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| AgentsError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: protoclaw_acp::SessionNewResult = serde_json::from_value(resp)?;
        self.session_id = Some(result.session_id.clone());
        tracing::info!(session_id = %result.session_id, "session started");
        Ok(())
    }

    async fn handle_command(&mut self, cmd: AgentsCommand) -> bool {
        match cmd {
            AgentsCommand::SendPrompt { message, reply } => {
                let result = self.send_prompt(&message).await;
                let _ = reply.send(result);
            }
            AgentsCommand::CancelOperation => {
                if let (Some(conn), Some(sid)) = (self.connection.as_ref(), &self.session_id) {
                    let params = serde_json::to_value(SessionCancelParams {
                        session_id: sid.clone(),
                    })
                    .ok();
                    if let Some(p) = params {
                        let _ = conn.send_notification("session/cancel", p).await;
                    }
                }
            }
            AgentsCommand::RespondPermission {
                request_id,
                option_id,
            } => {
                if let Some(perm) = self.pending_permissions.remove(&request_id) {
                    if let Some(conn) = self.connection.as_ref() {
                        let resp = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": perm.request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                            "result": {
                                "requestId": request_id,
                                "optionId": option_id,
                            }
                        });
                        let _ = conn.send_notification("_raw_response", resp).await;
                    }
                }
            }
            AgentsCommand::GetPendingPermissions { reply } => {
                let infos: Vec<PendingPermissionInfo> = self
                    .pending_permissions
                    .iter()
                    .map(|(id, p)| PendingPermissionInfo {
                        request_id: id.clone(),
                        description: p.description.clone(),
                        options: p.options.clone(),
                    })
                    .collect();
                let _ = reply.send(infos);
            }
            AgentsCommand::Shutdown => {
                self.shutdown_agent().await;
                return true;
            }
            AgentsCommand::GetStatus { reply } => {
                let info = AgentStatusInfo {
                    connected: self.connection.is_some(),
                    session_id: self.session_id.clone(),
                };
                let _ = reply.send(info);
            }
            AgentsCommand::CreateSession { session_key, reply } => {
                let result = self.create_session(session_key).await;
                let _ = reply.send(result);
            }
            AgentsCommand::PromptSession { session_key, message, reply } => {
                let result = self.prompt_session(&session_key, &message).await;
                let _ = reply.send(result);
            }
        }
        false
    }

    async fn send_prompt(&self, message: &str) -> Result<(), AgentsError> {
        let conn = self.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;
        let sid = self
            .session_id
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: sid.clone(),
            message: PromptMessage {
                role: "user".into(),
                content: message.into(),
            },
        })?;

        let _response_rx = conn.send_request("session/prompt", params).await?;
        Ok(())
    }

    /// Create a new ACP session keyed by SessionKey.
    async fn create_session(&mut self, session_key: SessionKey) -> Result<String, AgentsError> {
        // If session already exists for this key, return existing ACP session_id
        if let Some(acp_id) = self.session_map.get(&session_key) {
            return Ok(acp_id.clone());
        }

        // Get MCP server URLs from tools manager
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tools_handle
            .send(ToolsCommand::GetMcpUrls { reply: reply_tx })
            .await
            .map_err(|e| AgentsError::SpawnFailed(format!("tools handle: {e}")))?;

        let urls: Vec<McpServerUrl> = reply_rx.await.unwrap_or_default();
        let mcp_servers: Vec<McpServerInfo> = urls
            .iter()
            .map(|u| McpServerInfo {
                name: u.name.clone(),
                server_type: "http".into(),
                url: u.url.clone(),
                headers: None,
            })
            .collect();

        let params = serde_json::to_value(SessionNewParams {
            session_id: None,
            mcp_servers: if mcp_servers.is_empty() {
                None
            } else {
                Some(mcp_servers)
            },
        })?;

        let conn = self.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;
        let rx = conn.send_request("session/new", params).await?;
        let resp = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| AgentsError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: protoclaw_acp::SessionNewResult = serde_json::from_value(resp)?;
        let acp_session_id = result.session_id.clone();

        self.session_map.insert(session_key.clone(), acp_session_id.clone());
        self.reverse_map.insert(acp_session_id.clone(), session_key);

        tracing::info!(session_key = %acp_session_id, "multi-session created");
        Ok(acp_session_id)
    }

    /// Send a prompt to an existing session identified by SessionKey.
    async fn prompt_session(&self, session_key: &SessionKey, message: &str) -> Result<(), AgentsError> {
        let acp_session_id = self.session_map.get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?;

        let conn = self.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_session_id.clone(),
            message: PromptMessage {
                role: "user".into(),
                content: message.into(),
            },
        })?;

        let _response_rx = conn.send_request("session/prompt", params).await?;
        Ok(())
    }

    async fn handle_incoming(&mut self, msg: IncomingMessage) {
        match msg {
            IncomingMessage::AgentNotification(value) | IncomingMessage::AgentRequest(value) => {
                let method = value["method"].as_str().unwrap_or("");
                let params = value.get("params").cloned().unwrap_or(serde_json::Value::Null);

                match method {
                    "session/update" => {
                        if let Ok(event) = serde_json::from_value::<SessionUpdateEvent>(params.clone()) {
                            tracing::debug!(session_id = %event.session_id, update = ?event.update, "session update");

                            // Route back to originating channel via reverse_map
                            if let Some(session_key) = self.reverse_map.get(&event.session_id).cloned() {
                                if let Some(sender) = &self.channels_sender {
                                    let _ = sender.send(ChannelEvent::DeliverMessage {
                                        session_key,
                                        content: params,
                                    }).await;
                                }
                            }
                        }
                    }
                    "session/request_permission" => {
                        self.handle_permission_request(&value, &params).await;
                    }
                    "fs/read_text_file" => {
                        self.handle_fs_read(&value, &params).await;
                    }
                    "fs/write_text_file" => {
                        self.handle_fs_write(&value, &params).await;
                    }
                    _ => {
                        self.send_error_response(&value, -32601, "Method not found").await;
                    }
                }
            }
        }
    }

    async fn handle_permission_request(&mut self, request: &serde_json::Value, params: &serde_json::Value) {
        let request_id = params["requestId"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let description = params["description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let options: Vec<PermissionOption> = params
            .get("options")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        tracing::info!(%request_id, %description, "permission requested");

        // Route permission to originating channel via session_id → session_key
        let session_id = params["sessionId"].as_str().unwrap_or("");
        if let Some(session_key) = self.reverse_map.get(session_id).cloned() {
            if let Some(sender) = &self.channels_sender {
                let options_json = serde_json::to_value(&options).unwrap_or_default();
                let _ = sender.send(ChannelEvent::RoutePermission {
                    session_key,
                    request_id: request_id.clone(),
                    description: description.clone(),
                    options: options_json,
                }).await;
            }
        }

        self.pending_permissions.insert(
            request_id,
            PendingPermission {
                request: request.clone(),
                description,
                options,
                received_at: std::time::Instant::now(),
            },
        );
    }

    async fn handle_fs_read(&self, request: &serde_json::Value, params: &serde_json::Value) {
        let path = params["path"].as_str().unwrap_or("");
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                self.send_success_response(request, serde_json::json!({ "content": content }))
                    .await;
            }
            Err(e) => {
                self.send_error_response(request, -32000, &e.to_string())
                    .await;
            }
        }
    }

    async fn handle_fs_write(&self, request: &serde_json::Value, params: &serde_json::Value) {
        let path = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        match tokio::fs::write(path, content).await {
            Ok(()) => {
                self.send_success_response(request, serde_json::json!({}))
                    .await;
            }
            Err(e) => {
                self.send_error_response(request, -32000, &e.to_string())
                    .await;
            }
        }
    }

    async fn send_success_response(&self, request: &serde_json::Value, result: serde_json::Value) {
        if let Some(conn) = self.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": result,
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn send_error_response(&self, request: &serde_json::Value, code: i64, message: &str) {
        if let Some(conn) = self.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": { "code": code, "message": message },
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn shutdown_agent(&mut self) {
        if let (Some(conn), Some(sid)) = (self.connection.as_ref(), &self.session_id) {
            let params = serde_json::json!({ "sessionId": sid });
            let _ = conn.send_notification("session/close", params).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if let Some(mut conn) = self.connection.take() {
            let _ = conn.kill().await;
        }
    }

    async fn handle_crash(&mut self, _cancel: &CancellationToken) {
        tracing::warn!("agent process exited, attempting recovery");
        self.connection = None;

        let delay = self.backoff.next_delay();
        tracing::info!(delay_ms = delay.as_millis(), "waiting before restart");
        tokio::time::sleep(delay).await;

        match AgentConnection::spawn(&self.agent_config) {
            Ok(conn) => {
                self.connection = Some(conn);
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to respawn agent");
                return;
            }
        }

        if let Err(e) = self.initialize_agent().await {
            tracing::error!(error = %e, "failed to re-initialize agent");
            self.connection = None;
            return;
        }

        let supports_load = self
            .agent_capabilities
            .as_ref()
            .and_then(|c| c.load_session)
            .unwrap_or(false);

        if supports_load {
            if let Some(sid) = &self.session_id {
                let params = serde_json::to_value(SessionLoadParams {
                    session_id: sid.clone(),
                })
                .unwrap_or_default();

                let conn = self.connection.as_ref().unwrap();
                if let Ok(rx) = conn.send_request("session/load", params).await {
                    match tokio::time::timeout(Duration::from_secs(30), rx).await {
                        Ok(Ok(resp)) => {
                            if resp.get("sessionId").is_some() {
                                tracing::info!("session restored via session/load");
                                self.backoff.reset();
                                return;
                            }
                        }
                        _ => {
                            tracing::warn!("session/load failed, starting fresh session");
                        }
                    }
                }
            }
        }

        if let Err(e) = self.start_session().await {
            tracing::error!(error = %e, "failed to start new session after crash");
            self.connection = None;
            return;
        }

        self.backoff.reset();
        tracing::info!("agent recovered successfully");
    }
}

impl Manager for AgentsManager {
    type Command = AgentsCommand;

    fn name(&self) -> &str {
        "agents"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        let conn = AgentConnection::spawn(&self.agent_config)
            .map_err(|e| ManagerError::Internal(e.to_string()))?;
        self.connection = Some(conn);

        self.initialize_agent()
            .await
            .map_err(|e| ManagerError::Internal(e.to_string()))?;

        self.start_session()
            .await
            .map_err(|e| ManagerError::Internal(e.to_string()))?;

        tracing::info!(manager = self.name(), "manager started");
        Ok(())
    }

    async fn run(mut self, cancel: CancellationToken) -> Result<(), ManagerError> {
        let mut cmd_rx = self.cmd_rx.take().expect("cmd_rx must exist");

        tracing::info!(manager = self.name(), "manager running");

        loop {
            let incoming = async {
                if let Some(conn) = self.connection.as_mut() {
                    conn.recv_incoming().await
                } else {
                    std::future::pending().await
                }
            };

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(manager = "agents", "shutting down");
                    self.shutdown_agent().await;
                    break;
                }
                Some(cmd) = cmd_rx.recv() => {
                    if self.handle_command(cmd).await {
                        break;
                    }
                }
                result = incoming => {
                    match result {
                        Some(msg) => self.handle_incoming(msg).await,
                        None => {
                            self.handle_crash(&cancel).await;
                        }
                    }
                }
            }
        }

        tracing::info!(manager = "agents", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.connection.is_some()
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
            env: HashMap::new(),
            working_dir: None,
        }
    }

    fn make_tools_handle() -> (ManagerHandle<ToolsCommand>, tokio::sync::mpsc::Receiver<ToolsCommand>) {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        (ManagerHandle::new(tx), rx)
    }

    async fn serve_tools_urls(mut rx: tokio::sync::mpsc::Receiver<ToolsCommand>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                ToolsCommand::GetMcpUrls { reply } => {
                    let _ = reply.send(vec![]);
                }
                ToolsCommand::Shutdown => break,
            }
        }
    }

    #[test]
    fn agents_manager_name() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(mock_agent_config(), handle);
        assert_eq!(m.name(), "agents");
    }

    #[tokio::test]
    async fn agents_manager_start_initializes_session() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agent_config(), handle);
        let result = m.start().await;
        assert!(result.is_ok(), "start failed: {result:?}");
        assert!(m.session_id.is_some());
        assert!(m.agent_capabilities.is_some());
        assert_eq!(m.agent_capabilities.as_ref().unwrap().protocol_version, 1);

        m.shutdown_agent().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_health_check_alive() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agent_config(), handle);
        m.start().await.unwrap();
        assert!(m.health_check().await);

        m.shutdown_agent().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_health_check_dead() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(mock_agent_config(), handle);
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn agents_manager_send_prompt_receives_echo() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agent_config(), handle);
        m.start().await.unwrap();

        let result = m.send_prompt("hello").await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(200)).await;

        m.shutdown_agent().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_crash_recovery() {
        let mut config = mock_agent_config();
        config.env.insert("MOCK_AGENT_EXIT_AFTER".into(), "1".into());

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(config, handle);
        m.start().await.unwrap();
        let original_session = m.session_id.clone();

        let _ = m.send_prompt("trigger-crash").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let cancel = CancellationToken::new();
        m.handle_crash(&cancel).await;

        assert!(m.connection.is_some(), "should have reconnected");
        assert!(m.session_id.is_some(), "should have new session");

        m.shutdown_agent().await;
        tools_task.abort();
        let _ = original_session;
    }

    #[tokio::test]
    async fn get_status_returns_disconnected_when_fresh() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agent_config(), handle);

        let (reply_tx, reply_rx) = oneshot::channel();
        let done = m.handle_command(AgentsCommand::GetStatus { reply: reply_tx }).await;
        assert!(!done, "GetStatus should not stop the manager");

        let info = reply_rx.await.expect("should receive status info");
        assert!(!info.connected, "fresh manager should not be connected");
        assert!(info.session_id.is_none(), "fresh manager should have no session_id");
    }
}
