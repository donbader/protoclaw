use std::time::Duration;

use crate::acp_error::AcpError;
use crate::acp_types::{
    ClientCapabilities, InitializeParams, InitializeResult, McpServerInfo, PromptMessage,
    SessionCancelParams, SessionLoadParams, SessionNewParams, SessionPromptParams,
    SessionUpdateEvent, SessionUpdateType,
};
use crate::slot::{find_slot_by_name, AgentSlot};
use protoclaw_config::AgentConfig;
use protoclaw_core::{ChannelEvent, Manager, ManagerError, ManagerHandle, SessionKey};
use protoclaw_sdk_agent::{AgentAdapter, GenericAcpAdapter};
use protoclaw_sdk_types::PermissionOption;
use protoclaw_tools::{McpServerUrl, ToolsCommand};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{AgentConnection, IncomingMessage};
use crate::error::AgentsError;

#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    pub name: String,
    pub connected: bool,
    pub session_count: usize,
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
    GetStatus {
        reply: oneshot::Sender<Vec<AgentStatusInfo>>,
    },
    CreateSession {
        agent_name: String,
        session_key: SessionKey,
        reply: oneshot::Sender<Result<String, AgentsError>>,
    },
    PromptSession {
        agent_name: String,
        session_key: SessionKey,
        message: String,
        reply: oneshot::Sender<Result<(), AgentsError>>,
    },
}

pub struct AgentsManager {
    agent_configs: Vec<AgentConfig>,
    tools_handle: ManagerHandle<ToolsCommand>,
    slots: Vec<AgentSlot>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<AgentsCommand>>,
    cmd_tx: tokio::sync::mpsc::Sender<AgentsCommand>,
    channels_sender: Option<mpsc::Sender<ChannelEvent>>,
    adapter: Box<dyn AgentAdapter>,
    parent_cancel: CancellationToken,
}

impl AgentsManager {
    pub fn new(agent_configs: Vec<AgentConfig>, tools_handle: ManagerHandle<ToolsCommand>) -> Self {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
        Self {
            agent_configs,
            tools_handle,
            slots: Vec::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
            channels_sender: None,
            adapter: Box::new(GenericAcpAdapter),
            parent_cancel: CancellationToken::new(),
        }
    }

    pub fn with_adapter(mut self, adapter: Box<dyn AgentAdapter>) -> Self {
        self.adapter = adapter;
        self
    }

    pub fn with_channels_sender(mut self, sender: mpsc::Sender<ChannelEvent>) -> Self {
        self.channels_sender = Some(sender);
        self
    }

    pub fn command_sender(&self) -> tokio::sync::mpsc::Sender<AgentsCommand> {
        self.cmd_tx.clone()
    }

    async fn initialize_agent(slot: &mut AgentSlot) -> Result<(), AgentsError> {
        let conn = slot.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;

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

        slot.agent_capabilities = Some(result);
        Ok(())
    }

    async fn start_session(slot: &mut AgentSlot, tools_handle: &ManagerHandle<ToolsCommand>) -> Result<String, AgentsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        tools_handle
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

        let conn = slot.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;
        let rx = conn.send_request("session/new", params).await?;
        let resp = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| AgentsError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: crate::acp_types::SessionNewResult = serde_json::from_value(resp)?;
        tracing::info!(agent = %slot.name(), session_id = %result.session_id, "session started");
        Ok(result.session_id)
    }

    async fn handle_command(&mut self, cmd: AgentsCommand) -> bool {
        match cmd {
            AgentsCommand::SendPrompt { message, reply } => {
                let result = if let Some(slot) = self.slots.first() {
                    Self::send_prompt_to_slot(slot, &message).await
                } else {
                    Err(AgentsError::ConnectionClosed)
                };
                let _ = reply.send(result);
            }
            AgentsCommand::CancelOperation => {
                for slot in &self.slots {
                    if let Some(conn) = &slot.connection {
                        for acp_id in slot.session_map.values() {
                            let params = serde_json::to_value(SessionCancelParams {
                                session_id: acp_id.clone(),
                            })
                            .ok();
                            if let Some(p) = params {
                                let _ = conn.send_notification("session/cancel", p).await;
                            }
                        }
                    }
                }
            }
            AgentsCommand::RespondPermission {
                request_id,
                option_id,
            } => {
                for slot in &mut self.slots {
                    if let Some(perm) = slot.pending_permissions.remove(&request_id) {
                        if let Some(conn) = slot.connection.as_ref() {
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
                        break;
                    }
                }
            }
            AgentsCommand::GetPendingPermissions { reply } => {
                let mut infos = Vec::new();
                for slot in &self.slots {
                    for (id, p) in &slot.pending_permissions {
                        infos.push(PendingPermissionInfo {
                            request_id: id.clone(),
                            description: p.description.clone(),
                            options: p.options.clone(),
                        });
                    }
                }
                let _ = reply.send(infos);
            }
            AgentsCommand::Shutdown => {
                self.shutdown_all().await;
                return true;
            }
            AgentsCommand::GetStatus { reply } => {
                let statuses: Vec<AgentStatusInfo> = self
                    .slots
                    .iter()
                    .map(|slot| AgentStatusInfo {
                        name: slot.name().to_string(),
                        connected: slot.connection.is_some(),
                        session_count: slot.session_map.len(),
                    })
                    .collect();
                let _ = reply.send(statuses);
            }
            AgentsCommand::CreateSession { agent_name, session_key, reply } => {
                let result = self.create_session(&agent_name, session_key).await;
                let _ = reply.send(result);
            }
            AgentsCommand::PromptSession { agent_name, session_key, message, reply } => {
                let result = self.prompt_session(&agent_name, &session_key, &message).await;
                let _ = reply.send(result);
            }
        }
        false
    }

    async fn send_prompt_to_slot(slot: &AgentSlot, message: &str) -> Result<(), AgentsError> {
        let conn = slot.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;
        let acp_id = slot.session_map.values().next()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_id.clone(),
            message: PromptMessage {
                role: "user".into(),
                content: message.into(),
            },
        })?;

        let _response_rx = conn.send_request("session/prompt", params).await?;
        Ok(())
    }

    async fn create_session(&mut self, agent_name: &str, session_key: SessionKey) -> Result<String, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if let Some(acp_id) = slot.session_map.get(&session_key) {
            return Ok(acp_id.clone());
        }

        let acp_session_id = Self::start_session(&mut self.slots[slot_idx], &self.tools_handle).await?;

        let slot = &mut self.slots[slot_idx];
        slot.session_map.insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map.insert(acp_session_id.clone(), session_key);

        tracing::info!(agent = %agent_name, session_key = %acp_session_id, "multi-session created");
        Ok(acp_session_id)
    }

    async fn prompt_session(&self, agent_name: &str, session_key: &SessionKey, message: &str) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot.session_map.get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?;

        let conn = slot.connection.as_ref().ok_or(AgentsError::ConnectionClosed)?;

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

    async fn handle_incoming(&mut self, slot_idx: usize, msg: IncomingMessage) {
        let value = match &msg {
            IncomingMessage::AgentNotification(v) | IncomingMessage::AgentRequest(v) => v.clone(),
        };

        let method = value["method"].as_str().unwrap_or("");
        let params = value.get("params").cloned().unwrap_or(serde_json::Value::Null);

        match method {
            "session/update" => {
                if let Ok(event) = serde_json::from_value::<SessionUpdateEvent>(params.clone()) {
                    tracing::debug!(agent = %self.slots[slot_idx].name(), session_id = %event.session_id, update = ?event.update, "session update");

                    if matches!(&event.update, SessionUpdateType::AgentThoughtChunk { .. }) {
                        tracing::debug!(session_id = %event.session_id, "agent thought chunk received, routing to channel");
                    }

                    if let Some(session_key) = self.slots[slot_idx].reverse_map.get(&event.session_id).cloned() {
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
                self.handle_permission_request(slot_idx, &value, &params).await;
            }
            "fs/read_text_file" => {
                Self::handle_fs_read(&self.slots[slot_idx], &value, &params).await;
            }
            "fs/write_text_file" => {
                Self::handle_fs_write(&self.slots[slot_idx], &value, &params).await;
            }
            _ => {
                Self::send_error_response(&self.slots[slot_idx], &value, -32601, "Method not found").await;
            }
        }
    }

    async fn handle_permission_request(&mut self, slot_idx: usize, request: &serde_json::Value, params: &serde_json::Value) {
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

        tracing::info!(agent = %self.slots[slot_idx].name(), %request_id, %description, "permission requested");

        let session_id = params["sessionId"].as_str().unwrap_or("");
        if let Some(session_key) = self.slots[slot_idx].reverse_map.get(session_id).cloned() {
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

        self.slots[slot_idx].pending_permissions.insert(
            request_id,
            PendingPermission {
                request: request.clone(),
                description,
                options,
                received_at: std::time::Instant::now(),
            },
        );
    }

    async fn handle_fs_read(slot: &AgentSlot, request: &serde_json::Value, params: &serde_json::Value) {
        let path = params["path"].as_str().unwrap_or("");
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                Self::send_success_response(slot, request, serde_json::json!({ "content": content })).await;
            }
            Err(e) => {
                Self::send_error_response(slot, request, -32000, &e.to_string()).await;
            }
        }
    }

    async fn handle_fs_write(slot: &AgentSlot, request: &serde_json::Value, params: &serde_json::Value) {
        let path = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        match tokio::fs::write(path, content).await {
            Ok(()) => {
                Self::send_success_response(slot, request, serde_json::json!({})).await;
            }
            Err(e) => {
                Self::send_error_response(slot, request, -32000, &e.to_string()).await;
            }
        }
    }

    async fn send_success_response(slot: &AgentSlot, request: &serde_json::Value, result: serde_json::Value) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": result,
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn send_error_response(slot: &AgentSlot, request: &serde_json::Value, code: i64, message: &str) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": { "code": code, "message": message },
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn shutdown_all(&mut self) {
        for slot in &mut self.slots {
            if let Some(conn) = &slot.connection {
                for acp_id in slot.session_map.values() {
                    let params = serde_json::json!({ "sessionId": acp_id });
                    let _ = conn.send_notification("session/close", params).await;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if let Some(mut conn) = slot.connection.take() {
                let _ = conn.kill().await;
            }
        }
    }

    async fn handle_crash(&mut self, slot_idx: usize) {
        let slot = &mut self.slots[slot_idx];
        let agent_name = slot.name().to_string();
        tracing::warn!(agent = %agent_name, "agent process exited, attempting recovery");
        slot.connection = None;

        let delay = slot.backoff.next_delay();
        tracing::info!(agent = %agent_name, delay_ms = delay.as_millis(), "waiting before restart");
        tokio::time::sleep(delay).await;

        match AgentConnection::spawn(&slot.config) {
            Ok(conn) => {
                slot.connection = Some(conn);
            }
            Err(e) => {
                tracing::error!(agent = %agent_name, error = %e, "failed to respawn agent");
                return;
            }
        }

        if let Err(e) = Self::initialize_agent(slot).await {
            tracing::error!(agent = %agent_name, error = %e, "failed to re-initialize agent");
            slot.connection = None;
            return;
        }

        let supports_load = slot
            .agent_capabilities
            .as_ref()
            .and_then(|c| c.load_session)
            .unwrap_or(false);

        if supports_load {
            if let Some(first_acp_id) = slot.session_map.values().next().cloned() {
                let params = serde_json::to_value(SessionLoadParams {
                    session_id: first_acp_id,
                })
                .unwrap_or_default();

                let conn = slot.connection.as_ref().expect("connection just spawned");
                if let Ok(rx) = conn.send_request("session/load", params).await {
                    match tokio::time::timeout(Duration::from_secs(30), rx).await {
                        Ok(Ok(resp)) => {
                            if resp.get("sessionId").is_some() {
                                tracing::info!(agent = %agent_name, "session restored via session/load");
                                slot.backoff.reset();
                                return;
                            }
                        }
                        _ => {
                            tracing::warn!(agent = %agent_name, "session/load failed, starting fresh session");
                        }
                    }
                }
            }
        }

        match Self::start_session(slot, &self.tools_handle).await {
            Ok(_session_id) => {
                slot.backoff.reset();
                tracing::info!(agent = %agent_name, "agent recovered successfully");
            }
            Err(e) => {
                tracing::error!(agent = %agent_name, error = %e, "failed to start new session after crash");
                slot.connection = None;
            }
        }
    }
}

impl Manager for AgentsManager {
    type Command = AgentsCommand;

    fn name(&self) -> &str {
        "agents"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        for config in &self.agent_configs {
            if !config.enabled {
                tracing::info!(agent = %config.name, "agent disabled, skipping");
                continue;
            }

            let mut slot = AgentSlot::new(config.clone(), &self.parent_cancel);

            let conn = AgentConnection::spawn(&config)
                .map_err(|e| ManagerError::Internal(format!("{}: {e}", config.name)))?;
            slot.connection = Some(conn);

            Self::initialize_agent(&mut slot)
                .await
                .map_err(|e| ManagerError::Internal(format!("{}: {e}", config.name)))?;

            let session_id = Self::start_session(&mut slot, &self.tools_handle)
                .await
                .map_err(|e| ManagerError::Internal(format!("{}: {e}", config.name)))?;

            let default_key = SessionKey::new(&config.name, "default", "default");
            slot.session_map.insert(default_key.clone(), session_id.clone());
            slot.reverse_map.insert(session_id, default_key);

            self.slots.push(slot);
        }

        tracing::info!(
            manager = self.name(),
            active = self.slots.len(),
            total = self.agent_configs.len(),
            "manager started"
        );
        Ok(())
    }

    async fn run(mut self, cancel: CancellationToken) -> Result<(), ManagerError> {
        let mut cmd_rx = self.cmd_rx.take().expect("cmd_rx must exist");

        tracing::info!(manager = self.name(), "manager running");

        loop {
            let incoming = async {
                for (i, slot) in self.slots.iter_mut().enumerate() {
                    if slot.disabled {
                        continue;
                    }
                    if let Some(conn) = slot.connection.as_mut() {
                        match tokio::time::timeout(Duration::from_millis(1), conn.recv_incoming()).await {
                            Ok(msg) => return Some((i, msg)),
                            Err(_) => continue,
                        }
                    }
                }
                std::future::pending().await
            };

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(manager = "agents", "shutting down");
                    self.shutdown_all().await;
                    break;
                }
                Some(cmd) = cmd_rx.recv() => {
                    if self.handle_command(cmd).await {
                        break;
                    }
                }
                result = incoming => {
                    if let Some((idx, msg)) = result {
                        match msg {
                            Some(incoming_msg) => self.handle_incoming(idx, incoming_msg).await,
                            None => {
                                self.handle_crash(idx).await;
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(manager = "agents", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        let enabled_slots: Vec<_> = self.slots.iter().filter(|s| !s.disabled).collect();
        if enabled_slots.is_empty() {
            return self.agent_configs.iter().all(|c| !c.enabled);
        }
        enabled_slots.iter().all(|s| s.connection.is_some())
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
            name: "default".to_string(),
            binary: target_dir.to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            env: HashMap::new(),
            working_dir: None,
            tools: vec![],
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
        let m = AgentsManager::new(vec![mock_agent_config()], handle);
        assert_eq!(m.name(), "agents");
    }

    #[tokio::test]
    async fn agents_manager_start_initializes_session() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(vec![mock_agent_config()], handle);
        let result = m.start().await;
        assert!(result.is_ok(), "start failed: {result:?}");
        assert_eq!(m.slots.len(), 1);
        assert!(m.slots[0].connection.is_some());
        assert!(m.slots[0].agent_capabilities.is_some());
        assert_eq!(m.slots[0].agent_capabilities.as_ref().unwrap().protocol_version, 1);

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_health_check_alive() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(vec![mock_agent_config()], handle);
        m.start().await.unwrap();
        assert!(m.health_check().await);

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_health_check_dead() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(vec![mock_agent_config()], handle);
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn agents_manager_health_check_no_agents_configured() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(vec![], handle);
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn agents_manager_health_check_all_disabled() {
        let mut config = mock_agent_config();
        config.enabled = false;
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(vec![config], handle);
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn agents_manager_send_prompt_receives_echo() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(vec![mock_agent_config()], handle);
        m.start().await.unwrap();

        let result = AgentsManager::send_prompt_to_slot(&m.slots[0], "hello").await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(200)).await;

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn agents_manager_crash_recovery() {
        let mut config = mock_agent_config();
        config.env.insert("MOCK_AGENT_EXIT_AFTER".into(), "1".into());

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(vec![config], handle);
        m.start().await.unwrap();

        let _ = AgentsManager::send_prompt_to_slot(&m.slots[0], "trigger-crash").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        m.handle_crash(0).await;

        assert!(m.slots[0].connection.is_some(), "should have reconnected");

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn get_status_returns_vec_of_agent_info() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(vec![mock_agent_config()], handle);

        let mut slot = AgentSlot::new(mock_agent_config(), &m.parent_cancel);
        slot.session_map.insert(
            SessionKey::new("test", "local", "dev"),
            "acp-1".to_string(),
        );
        m.slots.push(slot);

        let (reply_tx, reply_rx) = oneshot::channel();
        let done = m.handle_command(AgentsCommand::GetStatus { reply: reply_tx }).await;
        assert!(!done);

        let statuses = reply_rx.await.expect("should receive status");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "default");
        assert!(!statuses[0].connected);
        assert_eq!(statuses[0].session_count, 1);
    }

    #[tokio::test]
    async fn create_session_with_unknown_agent_returns_error() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(vec![], handle);

        let result = m.create_session("nonexistent", SessionKey::new("ch", "k", "p")).await;
        assert!(matches!(result, Err(AgentsError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn start_skips_disabled_agents() {
        let mut config = mock_agent_config();
        config.enabled = false;

        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(vec![config], handle);
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(m.slots.is_empty(), "disabled agent should not create a slot");
    }

    #[tokio::test]
    async fn create_session_routes_to_correct_slot() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut config_a = mock_agent_config();
        config_a.name = "agent-a".to_string();
        let mut config_b = mock_agent_config();
        config_b.name = "agent-b".to_string();

        let mut m = AgentsManager::new(vec![config_a, config_b], handle);
        m.start().await.unwrap();
        assert_eq!(m.slots.len(), 2);

        let key = SessionKey::new("telegram", "direct", "alice");
        let result = m.create_session("agent-b", key.clone()).await;
        assert!(result.is_ok());

        assert!(m.slots[1].session_map.contains_key(&key));
        assert!(!m.slots[0].session_map.contains_key(&key));

        m.shutdown_all().await;
        tools_task.abort();
    }
}
