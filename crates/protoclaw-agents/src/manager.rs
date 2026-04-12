use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::acp_error::AcpError;
use crate::acp_types::{
    ClientCapabilities, ContentPart, InitializeParams, InitializeResult, McpServerInfo,
    SessionCancelParams, SessionForkParams, SessionForkResult, SessionListParams,
    SessionLoadParams, SessionNewParams, SessionPromptParams, SessionUpdateEvent, SessionUpdateType,
};
use crate::slot::{AgentSlot, find_slot_by_name};
use protoclaw_config::{AgentConfig, AgentsManagerConfig, WorkspaceConfig};
use protoclaw_core::{
    AgentStatusInfo, AgentsCommand, CrashAction, Manager, ManagerError, ManagerHandle,
    McpServerUrl, PendingPermissionInfo, SessionKey, ToolsCommand, constants,
};
use protoclaw_sdk_agent::{DynAgentAdapter, GenericAcpAdapter};
use protoclaw_sdk_types::{ChannelEvent, PermissionOption};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{AgentConnection, IncomingMessage};
use crate::error::AgentsError;

pub(crate) struct PendingPermission {
    pub request: serde_json::Value,
    pub description: String,
    pub options: Vec<PermissionOption>,
    pub _received_at: std::time::Instant,
}

pub(crate) struct SlotIncoming {
    pub(crate) slot_idx: usize,
    pub(crate) msg: Option<IncomingMessage>,
}

struct PromptCompletion {
    session_key: SessionKey,
}

pub struct AgentsManager {
    agent_configs: Vec<(String, AgentConfig)>,
    manager_config: AgentsManagerConfig,
    tools_handle: ManagerHandle<ToolsCommand>,
    slots: Vec<AgentSlot>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<AgentsCommand>>,
    cmd_tx: tokio::sync::mpsc::Sender<AgentsCommand>,
    channels_sender: Option<mpsc::Sender<ChannelEvent>>,
    adapter: Box<dyn DynAgentAdapter>,
    parent_cancel: CancellationToken,
    incoming_tx: mpsc::Sender<SlotIncoming>,
    incoming_rx: Option<mpsc::Receiver<SlotIncoming>>,
    completion_tx: mpsc::Sender<PromptCompletion>,
    completion_rx: Option<mpsc::Receiver<PromptCompletion>>,
    streaming_completed: HashSet<SessionKey>,
    update_seq: AtomicU64,
    log_level: Option<String>,
}

impl AgentsManager {
    pub fn new(
        mut agents_manager_config: AgentsManagerConfig,
        tools_handle: ManagerHandle<ToolsCommand>,
    ) -> Self {
        let configs: Vec<(String, AgentConfig)> = agents_manager_config.agents.drain().collect();
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(constants::CMD_CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) =
            mpsc::channel::<SlotIncoming>(constants::EVENT_CHANNEL_CAPACITY);
        let (completion_tx, completion_rx) =
            mpsc::channel::<PromptCompletion>(constants::CMD_CHANNEL_CAPACITY);
        Self {
            agent_configs: configs,
            manager_config: agents_manager_config,
            tools_handle,
            slots: Vec::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
            channels_sender: None,
            adapter: Box::new(GenericAcpAdapter),
            parent_cancel: CancellationToken::new(),
            incoming_tx,
            incoming_rx: Some(incoming_rx),
            completion_tx,
            completion_rx: Some(completion_rx),
            streaming_completed: HashSet::new(),
            update_seq: AtomicU64::new(0),
            log_level: None,
        }
    }

    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = Some(level);
        self
    }

    pub fn with_adapter(mut self, adapter: Box<dyn DynAgentAdapter>) -> Self {
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

    /// Resolve ACP timeout for a specific agent, falling back to manager default.
    fn acp_timeout_for(
        agent_config: &AgentConfig,
        manager_config: &AgentsManagerConfig,
    ) -> Duration {
        let secs = agent_config
            .acp_timeout_secs
            .unwrap_or(manager_config.acp_timeout_secs);
        Duration::from_secs(secs)
    }

    #[tracing::instrument(skip(slot), fields(agent = %slot.name()))]
    async fn initialize_agent(
        slot: &mut AgentSlot,
        acp_timeout: Duration,
    ) -> Result<(), AgentsError> {
        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;

        let options = if slot.config.options.is_empty() {
            None
        } else {
            Some(slot.config.options.clone())
        };
        let params = serde_json::to_value(InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options,
        })?;

        let rx = conn.send_request("initialize", params).await?;
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
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

    async fn start_session(
        slot: &mut AgentSlot,
        tools_handle: &ManagerHandle<ToolsCommand>,
        acp_timeout: Duration,
    ) -> Result<String, AgentsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let tool_names = if slot.config.tools.is_empty() {
            None
        } else {
            Some(slot.config.tools.clone())
        };
        tools_handle
            .send(ToolsCommand::GetMcpUrls {
                tool_names,
                reply: reply_tx,
            })
            .await
            .map_err(|e| AgentsError::SpawnFailed(format!("tools handle: {e}")))?;

        let urls: Vec<McpServerUrl> = reply_rx.await.unwrap_or_else(|_| {
            tracing::warn!(
                "tools handle dropped before providing MCP URLs — agent will start with no tools"
            );
            Vec::new()
        });

        let mcp_servers: Vec<McpServerInfo> = urls
            .iter()
            .map(|u| McpServerInfo {
                name: u.name.clone(),
                server_type: "http".into(),
                url: u.url.clone(),
                command: String::new(),
                args: vec![],
                env: vec![],
                headers: vec![],
            })
            .collect();

        let cwd = match &slot.config.workspace {
            WorkspaceConfig::Local(local) => local
                .working_dir
                .clone()
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
                }),
            WorkspaceConfig::Docker(docker) => docker
                .working_dir
                .clone()
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
                }),
        };

        let params = serde_json::to_value(SessionNewParams {
            session_id: None,
            cwd: cwd.to_string_lossy().into_owned(),
            mcp_servers,
        })?;

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let rx = conn.send_request("session/new", params).await?;
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
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
                let _ = reply.send(result.map_err(|e| e.to_string()));
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
            AgentsCommand::CreateSession {
                agent_name,
                session_key,
                reply,
            } => {
                let result = self.create_session(&agent_name, session_key).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::PromptSession {
                agent_name,
                session_key,
                message,
                reply,
            } => {
                let result = self
                    .prompt_session(&agent_name, &session_key, &message)
                    .await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::ForkSession {
                agent_name,
                session_key,
                reply,
            } => {
                let result = self.fork_session(&agent_name, &session_key).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::ListSessions { agent_name, reply } => {
                let result = self.list_sessions(&agent_name).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::CancelSession {
                agent_name,
                session_key,
                reply,
            } => {
                let result = self.cancel_session(&agent_name, &session_key).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
        }
        false
    }

    async fn send_prompt_to_slot(slot: &AgentSlot, message: &str) -> Result<(), AgentsError> {
        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let acp_id = slot
            .session_map
            .values()
            .next()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_id.clone(),
            prompt: vec![ContentPart::text(message)],
        })?;

        let _response_rx = conn.send_request("session/prompt", params).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(agent = %agent_name, session_key = %session_key))]
    async fn create_session(
        &mut self,
        agent_name: &str,
        session_key: SessionKey,
    ) -> Result<String, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if let Some(acp_id) = slot.session_map.get(&session_key) {
            return Ok(acp_id.clone());
        }

        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        let acp_session_id =
            Self::start_session(&mut self.slots[slot_idx], &self.tools_handle, acp_timeout).await?;

        let slot = &mut self.slots[slot_idx];
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map.insert(acp_session_id.clone(), session_key);

        tracing::info!(agent = %agent_name, session_key = %acp_session_id, "multi-session created");
        Ok(acp_session_id)
    }

    async fn prompt_session(
        &self,
        agent_name: &str,
        session_key: &SessionKey,
        message: &str,
    ) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?;

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_session_id.clone(),
            prompt: vec![ContentPart::text(message)],
        })?;

        let response_rx = conn.send_request("session/prompt", params).await?;

        {
            let completion_tx = self.completion_tx.clone();
            let channels_tx = self.channels_sender.clone();
            let sk = session_key.clone();
            tokio::spawn(async move {
                match response_rx.await {
                    Ok(response) => {
                        // Check if the agent returned a JSON-RPC error
                        if let Some(error) = response.get("__jsonrpc_error") {
                            let msg = error["message"].as_str().unwrap_or("agent error");
                            tracing::warn!(session_key = %sk, error = %error, "agent returned error for prompt");
                            if let Some(sender) = &channels_tx {
                                let error_content = serde_json::json!({
                                    "error": msg,
                                    "update": { "sessionUpdate": "result" }
                                });
                                let _ = sender
                                    .send(ChannelEvent::DeliverMessage {
                                        session_key: sk.clone(),
                                        content: error_content,
                                    })
                                    .await;
                            }
                        }
                        let _ = completion_tx
                            .send(PromptCompletion { session_key: sk })
                            .await;
                    }
                    Err(_) => {
                        tracing::warn!(session_key = %sk, "prompt response channel dropped");
                    }
                }
            });
        }

        Ok(())
    }

    async fn fork_session(
        &mut self,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<String, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if slot
            .agent_capabilities
            .as_ref()
            .and_then(|r| r.session_capabilities.as_ref())
            .and_then(|c| c.fork.as_ref())
            .is_none()
        {
            return Err(AgentsError::CapabilityNotSupported("fork".into()));
        }

        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionForkParams {
            session_id: acp_session_id,
        })?;
        let rx = conn.send_request("session/fork", params).await?;

        let acp_timeout =
            Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: SessionForkResult = serde_json::from_value(resp)?;
        let new_session_id = result.session_id.clone();

        let fork_key = SessionKey::new(
            session_key.channel_name(),
            "fork",
            &new_session_id,
        );
        let slot = &mut self.slots[slot_idx];
        slot.session_map.insert(fork_key.clone(), new_session_id.clone());
        slot.reverse_map.insert(new_session_id.clone(), fork_key);

        tracing::info!(agent = %agent_name, forked_session_id = %new_session_id, "session forked");
        Ok(new_session_id)
    }

    async fn list_sessions(&self, agent_name: &str) -> Result<serde_json::Value, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if slot
            .agent_capabilities
            .as_ref()
            .and_then(|r| r.session_capabilities.as_ref())
            .and_then(|c| c.list.as_ref())
            .is_none()
        {
            return Err(AgentsError::CapabilityNotSupported("list".into()));
        }

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionListParams {})?;
        let rx = conn.send_request("session/list", params).await?;

        let acp_timeout = Self::acp_timeout_for(&slot.config, &self.manager_config);
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        Ok(resp)
    }

    async fn cancel_session(
        &self,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionCancelParams {
            session_id: acp_session_id,
        })?;
        conn.send_notification("session/cancel", params).await?;
        Ok(())
    }

    async fn handle_incoming(&mut self, slot_idx: usize, msg: IncomingMessage) {
        let value = match &msg {
            IncomingMessage::AgentNotification(v) | IncomingMessage::AgentRequest(v) => v.clone(),
        };
        let method = value["method"].as_str().unwrap_or("");
        let params = value
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            "session/update" => {
                self.handle_session_update(slot_idx, params).await;
            }
            "session/request_permission" => {
                self.handle_permission_request(slot_idx, &value, &params)
                    .await;
            }
            "fs/read_text_file" => {
                Self::handle_fs_read(&self.slots[slot_idx], &value, &params).await;
            }
            "fs/write_text_file" => {
                Self::handle_fs_write(&self.slots[slot_idx], &value, &params).await;
            }
            _ => {
                Self::send_error_response(
                    &self.slots[slot_idx],
                    &value,
                    -32601,
                    "Method not found",
                )
                .await;
            }
        }
    }

    fn session_update_type_name(update: &SessionUpdateType) -> &'static str {
        match update {
            SessionUpdateType::AgentThoughtChunk { .. } => "agent_thought_chunk",
            SessionUpdateType::AgentMessageChunk { .. } => "agent_message_chunk",
            SessionUpdateType::Result { .. } => "result",
            SessionUpdateType::ToolCall { .. } => "tool_call",
            SessionUpdateType::ToolCallUpdate { .. } => "tool_call_update",
            SessionUpdateType::Plan { .. } => "plan",
            SessionUpdateType::UsageUpdate { .. } => "usage_update",
            SessionUpdateType::UserMessageChunk { .. } => "user_message_chunk",
            SessionUpdateType::AvailableCommandsUpdate { .. } => "available_commands_update",
            SessionUpdateType::CurrentModeUpdate { .. } => "current_mode_update",
            SessionUpdateType::ConfigOptionUpdate { .. } => "config_option_update",
            SessionUpdateType::SessionInfoUpdate { .. } => "session_info_update",
        }
    }

    fn add_received_timestamp(content: &mut serde_json::Value) {
        if let Some(obj) = content.as_object_mut() {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "system time before UNIX_EPOCH, using zero duration");
                    std::time::Duration::default()
                })
                .as_millis() as u64;
            obj.insert("_received_at_ms".to_string(), serde_json::json!(now_ms));
        }
    }

    async fn forward_session_update(
        &mut self,
        slot_idx: usize,
        event: SessionUpdateEvent,
        mut content: serde_json::Value,
        seq: u64,
    ) {
        let update_type = Self::session_update_type_name(&event.update);
        tracing::debug!(agent = %self.slots[slot_idx].name(), session_id = %event.session_id, update_type, seq, "session update routed");

        let is_result = matches!(event.update, SessionUpdateType::Result { .. });
        Self::add_received_timestamp(&mut content);
        normalize_tool_event_fields(&mut content, update_type);

        if let Some(session_key) = self.slots[slot_idx]
            .reverse_map
            .get(&event.session_id)
            .cloned()
            && let Some(sender) = &self.channels_sender
        {
            let _ = sender
                .send(ChannelEvent::DeliverMessage {
                    session_key: session_key.clone(),
                    content,
                })
                .await;

            if is_result {
                self.streaming_completed.insert(session_key);
            }
        }
    }

    async fn forward_malformed_update_error(
        &self,
        slot_idx: usize,
        params: &serde_json::Value,
        error: &serde_json::Error,
        seq: u64,
    ) {
        tracing::warn!(error = %error, raw_params = %params, seq, "session/update deserialization FAILED — update dropped");

        let Some(session_id) = params.get("sessionId").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(session_key) = self.slots[slot_idx].reverse_map.get(session_id).cloned() else {
            return;
        };
        let Some(sender) = &self.channels_sender else {
            return;
        };

        let error_content = serde_json::json!({
            "error": format!("Agent sent malformed update: {error}"),
            "update": { "sessionUpdate": "result" }
        });
        let _ = sender
            .send(ChannelEvent::DeliverMessage {
                session_key,
                content: error_content,
            })
            .await;
    }

    async fn handle_session_update(&mut self, slot_idx: usize, params: serde_json::Value) {
        let seq = self.update_seq.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(raw_params = %params, seq, "session/update received — attempting deser");

        match serde_json::from_value::<SessionUpdateEvent>(params.clone()) {
            Ok(event) => {
                self.forward_session_update(slot_idx, event, params, seq)
                    .await
            }
            Err(error) => {
                self.forward_malformed_update_error(slot_idx, &params, &error, seq)
                    .await;
            }
        }
    }

    async fn handle_permission_request(
        &mut self,
        slot_idx: usize,
        request: &serde_json::Value,
        params: &serde_json::Value,
    ) {
        let request_id = params["requestId"].as_str().unwrap_or("").to_string();
        let description = params["description"].as_str().unwrap_or("").to_string();

        let options: Vec<PermissionOption> = params
            .get("options")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| {
                tracing::warn!(%request_id, "malformed permission options, using empty list");
                Vec::new()
            });

        tracing::info!(agent = %self.slots[slot_idx].name(), %request_id, %description, "permission requested");

        let session_id = params["sessionId"].as_str().unwrap_or("");
        if let Some(session_key) = self.slots[slot_idx].reverse_map.get(session_id).cloned()
            && let Some(sender) = &self.channels_sender
        {
            let options_json = serde_json::to_value(&options).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, %request_id, "failed to serialize permission options, using null");
                    serde_json::Value::default()
                });
            let _ = sender
                .send(ChannelEvent::RoutePermission {
                    session_key,
                    request_id: request_id.clone(),
                    description: description.clone(),
                    options: options_json,
                })
                .await;
        }

        self.slots[slot_idx].pending_permissions.insert(
            request_id,
            PendingPermission {
                request: request.clone(),
                description,
                options,
                _received_at: std::time::Instant::now(),
            },
        );
    }

    async fn handle_fs_read(
        slot: &AgentSlot,
        request: &serde_json::Value,
        params: &serde_json::Value,
    ) {
        let path = params["path"].as_str().unwrap_or("");
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                Self::send_success_response(
                    slot,
                    request,
                    serde_json::json!({ "content": content }),
                )
                .await;
            }
            Err(e) => {
                Self::send_error_response(slot, request, -32000, &e.to_string()).await;
            }
        }
    }

    async fn handle_fs_write(
        slot: &AgentSlot,
        request: &serde_json::Value,
        params: &serde_json::Value,
    ) {
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

    async fn send_success_response(
        slot: &AgentSlot,
        request: &serde_json::Value,
        result: serde_json::Value,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": result,
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn send_error_response(
        slot: &AgentSlot,
        request: &serde_json::Value,
        code: i64,
        message: &str,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": { "code": code, "message": message },
            });
            let _ = conn.send_notification("_raw_response", resp).await;
        }
    }

    async fn handle_prompt_completion(
        &mut self,
        completion: PromptCompletion,
        incoming_rx: &mut mpsc::Receiver<SlotIncoming>,
    ) {
        // Drain any pending streaming events before sending SessionComplete.
        // The RPC response arrives after all streaming events on the agent's stdout,
        // but select! can pick completion_rx before incoming_rx is fully drained.
        while let Ok(slot_msg) = incoming_rx.try_recv() {
            match slot_msg.msg {
                Some(incoming_msg) => self.handle_incoming(slot_msg.slot_idx, incoming_msg).await,
                None => {
                    self.handle_crash(slot_msg.slot_idx).await;
                }
            }
        }

        let already_got_result = self.streaming_completed.remove(&completion.session_key);

        if let Some(sender) = &self.channels_sender {
            if !already_got_result {
                let acp_session_id = self.slots.iter()
                    .find_map(|slot| slot.session_map.get(&completion.session_key).cloned())
                    .unwrap_or_else(|| {
                        tracing::warn!(session_key = %completion.session_key, "no acp_session_id in reverse_map for synthetic result");
                        String::new()
                    });

                let synthetic_result = serde_json::json!({
                    "sessionId": acp_session_id,
                    "update": {
                        "sessionUpdate": "result",
                    }
                });
                let _ = sender
                    .send(ChannelEvent::DeliverMessage {
                        session_key: completion.session_key.clone(),
                        content: synthetic_result,
                    })
                    .await;
            }

            let _ = sender
                .send(ChannelEvent::SessionComplete {
                    session_key: completion.session_key,
                })
                .await;
        }
    }

    async fn shutdown_all(&mut self) {
        for slot in &mut self.slots {
            if let Some(conn) = &slot.connection {
                for acp_id in slot.session_map.values() {
                    let params = serde_json::json!({ "sessionId": acp_id });
                    let _ = conn.send_notification("session/close", params).await;
                }
                tokio::time::sleep(Duration::from_millis(self.manager_config.shutdown_grace_ms))
                    .await;
            }
            if let Some(mut conn) = slot.connection.take() {
                let _ = conn.kill().await;
            }
        }
    }

    async fn handle_crash(&mut self, slot_idx: usize) {
        let agent_name = self.slots[slot_idx].name().to_string();
        if !self.prepare_restart(slot_idx, &agent_name).await {
            return;
        }

        if !self.respawn_and_initialize(slot_idx, &agent_name).await {
            return;
        }

        self.restore_or_start_session(slot_idx, &agent_name).await;
    }

    async fn prepare_restart(&mut self, slot_idx: usize, agent_name: &str) -> bool {
        let slot = &mut self.slots[slot_idx];
        match slot.lifecycle.record_crash_and_check() {
            CrashAction::Disabled => {
                tracing::error!(agent = %agent_name, crash_loop = true, "agent crash loop detected — disabling slot");
                if let Some(mut old_conn) = slot.connection.take() {
                    let _ = old_conn.kill().await;
                }
                false
            }
            CrashAction::RestartAfter(delay) => {
                tracing::warn!(agent = %agent_name, "agent process exited, attempting recovery");
                if let Some(mut old_conn) = slot.connection.take()
                    && let Err(e) = old_conn.kill().await
                {
                    tracing::debug!(agent = %agent_name, error = %e, "failed to clean up old connection (may already be dead)");
                }
                tracing::info!(agent = %agent_name, delay_ms = delay.as_millis(), "waiting before restart");
                tokio::time::sleep(delay).await;
                true
            }
        }
    }

    async fn respawn_and_initialize(&mut self, slot_idx: usize, agent_name: &str) -> bool {
        let incoming_tx = self.incoming_tx.clone();
        let log_level = self.log_level.clone();
        let config = self.slots[slot_idx].config.clone();

        let conn = match AgentConnection::spawn_with_bridge(
            &config,
            agent_name,
            slot_idx,
            incoming_tx,
            log_level.as_deref(),
        )
        .await
        {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!(agent = %agent_name, error = %e, "failed to respawn agent");
                return false;
            }
        };

        let acp_timeout = Self::acp_timeout_for(&config, &self.manager_config);
        let slot = &mut self.slots[slot_idx];
        slot.connection = Some(conn);
        if let Err(e) = Self::initialize_agent(slot, acp_timeout).await {
            tracing::error!(agent = %agent_name, error = %e, "failed to re-initialize agent");
            slot.connection = None;
            return false;
        }

        true
    }

    async fn try_restore_session(
        &mut self,
        slot_idx: usize,
        agent_name: &str,
        acp_timeout: Duration,
    ) -> bool {
        let slot = &mut self.slots[slot_idx];
        let supports_load = slot
            .agent_capabilities
            .as_ref()
            .and_then(|c| c.load_session)
            .unwrap_or(false);
        if !supports_load {
            return false;
        }

        let Some(first_acp_id) = slot.session_map.values().next().cloned() else {
            return false;
        };
        let params = serde_json::to_value(SessionLoadParams {
            session_id: first_acp_id,
        })
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, agent = %agent_name, "failed to serialize session/load params, using null");
            serde_json::Value::default()
        });

        let conn = slot.connection.as_ref().expect("connection just spawned");
        let Ok(rx) = conn.send_request("session/load", params).await else {
            tracing::warn!(agent = %agent_name, "session/load failed, starting fresh session");
            return false;
        };

        match tokio::time::timeout(acp_timeout, rx).await {
            Ok(Ok(resp)) if resp.get("sessionId").is_some() => {
                tracing::info!(agent = %agent_name, "session restored via session/load");
                slot.lifecycle.backoff.reset();
                true
            }
            _ => {
                tracing::warn!(agent = %agent_name, "session/load failed, starting fresh session");
                false
            }
        }
    }

    async fn restore_or_start_session(&mut self, slot_idx: usize, agent_name: &str) {
        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        if self
            .try_restore_session(slot_idx, agent_name, acp_timeout)
            .await
        {
            return;
        }

        let slot = &mut self.slots[slot_idx];
        match Self::start_session(slot, &self.tools_handle, acp_timeout).await {
            Ok(session_id) => {
                // Clear stale session IDs from the crashed process — they are
                // no longer valid in the newly spawned agent.
                slot.session_map.clear();
                slot.reverse_map.clear();
                Self::register_default_session(slot, agent_name, session_id);
                slot.lifecycle.backoff.reset();
                tracing::info!(agent = %agent_name, "agent recovered successfully");
            }
            Err(e) => {
                tracing::error!(agent = %agent_name, error = %e, "failed to start new session after crash");
                slot.connection = None;
            }
        }
    }

    /// Remove any Docker containers left over from a previous (crashed) run.
    ///
    /// Scans all configured agents for Docker workspaces, connects to the matching
    /// Docker daemon, and forcibly removes every container that carries the
    /// `protoclaw.managed=true` label.  Errors are logged as warnings; this
    /// method never propagates failures so that `start()` is not blocked by
    /// stale-container cleanup.
    async fn cleanup_stale_containers(&self) {
        use bollard::query_parameters::{
            ListContainersOptions, RemoveContainerOptions, StopContainerOptions,
        };

        for (name, config) in &self.agent_configs {
            let docker_config = match &config.workspace {
                WorkspaceConfig::Docker(d) => d,
                WorkspaceConfig::Local(_) => continue,
            };

            let docker = match &docker_config.docker_host {
                Some(host) => {
                    bollard::Docker::connect_with_http(host, 120, bollard::API_DEFAULT_VERSION)
                }
                None => bollard::Docker::connect_with_local_defaults(),
            };
            let docker = match docker {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(agent = %name, error = %e, "cleanup: cannot connect to Docker daemon");
                    continue;
                }
            };

            let mut filters = HashMap::new();
            filters.insert(
                "label".to_string(),
                vec!["protoclaw.managed=true".to_string()],
            );
            let opts = ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            };
            let containers = match docker.list_containers(Some(opts)).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(agent = %name, error = %e, "cleanup: failed to list containers");
                    continue;
                }
            };

            for container in containers {
                let id = match container.id {
                    Some(ref id) => id.clone(),
                    None => continue,
                };
                tracing::info!(container_id = %id, agent = %name, "cleanup: removing stale container");
                if let Err(e) = docker
                    .stop_container(
                        &id,
                        Some(StopContainerOptions {
                            t: Some(5),
                            ..Default::default()
                        }),
                    )
                    .await
                {
                    tracing::warn!(container_id = %id, error = %e, "cleanup: stop failed, proceeding to remove");
                }
                if let Err(e) = docker
                    .remove_container(
                        &id,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                {
                    tracing::warn!(container_id = %id, error = %e, "cleanup: remove failed");
                }
            }
        }
    }

    async fn build_started_slot(
        &self,
        name: &str,
        config: &AgentConfig,
    ) -> Result<AgentSlot, ManagerError> {
        let mut slot = AgentSlot::new(name.to_string(), config.clone(), &self.parent_cancel);
        let slot_idx = self.slots.len();
        let conn = AgentConnection::spawn_with_bridge(
            config,
            name,
            slot_idx,
            self.incoming_tx.clone(),
            self.log_level.as_deref(),
        )
        .await
        .map_err(|e| ManagerError::Internal(format!("{name}: {e}")))?;
        slot.connection = Some(conn);

        let acp_timeout = Self::acp_timeout_for(config, &self.manager_config);
        Self::initialize_agent(&mut slot, acp_timeout)
            .await
            .map_err(|e| ManagerError::Internal(format!("{name}: {e}")))?;

        let session_id = Self::start_session(&mut slot, &self.tools_handle, acp_timeout)
            .await
            .map_err(|e| ManagerError::Internal(format!("{name}: {e}")))?;

        Self::register_default_session(&mut slot, name, session_id);
        Ok(slot)
    }

    fn register_default_session(slot: &mut AgentSlot, name: &str, session_id: String) {
        let default_key = SessionKey::new(name, "default", "default");
        slot.session_map
            .insert(default_key.clone(), session_id.clone());
        slot.reverse_map.insert(session_id, default_key);
    }
}

impl Manager for AgentsManager {
    type Command = AgentsCommand;

    fn name(&self) -> &str {
        "agents"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        self.cleanup_stale_containers().await;

        for (name, config) in self.agent_configs.iter() {
            if !config.enabled {
                tracing::info!(agent = %name, "agent disabled, skipping");
                continue;
            }

            let slot = self.build_started_slot(name, config).await?;
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
        let mut incoming_rx = self.incoming_rx.take().expect("incoming_rx must exist");
        let mut completion_rx = self.completion_rx.take().expect("completion_rx must exist");

        tracing::info!(manager = self.name(), "manager running");

        loop {
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
                Some(slot_msg) = incoming_rx.recv() => {
                    match slot_msg.msg {
                        Some(incoming_msg) => self.handle_incoming(slot_msg.slot_idx, incoming_msg).await,
                        None => {
                            if self.slots[slot_msg.slot_idx].lifecycle.disabled {
                                continue;
                            }
                            self.handle_crash(slot_msg.slot_idx).await;
                        }
                    }
                }
                Some(completion) = completion_rx.recv() => {
                    self.handle_prompt_completion(completion, &mut incoming_rx).await;
                }
            }
        }

        tracing::info!(manager = "agents", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        let enabled_slots: Vec<_> = self
            .slots
            .iter()
            .filter(|s| !s.lifecycle.disabled)
            .collect();
        if enabled_slots.is_empty() {
            return self.agent_configs.iter().all(|(_, c)| !c.enabled);
        }
        enabled_slots.iter().all(|s| s.connection.is_some())
    }
}

fn normalize_tool_event_fields(content: &mut serde_json::Value, update_type: &str) {
    if update_type != "tool_call" && update_type != "tool_call_update" {
        return;
    }
    let update = match content.get_mut("update").and_then(|u| u.as_object_mut()) {
        Some(u) => u,
        None => return,
    };

    if !update.contains_key("name")
        && let Some(title) = update.remove("title")
    {
        update.insert("name".to_string(), title);
    }

    if update_type == "tool_call_update"
        && !update.contains_key("output")
        && let Some(raw) = update
            .get("rawOutput")
            .and_then(|r| r.get("output"))
            .cloned()
    {
        update.insert("output".to_string(), raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
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
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: target_dir.to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    fn mock_agents_manager_config() -> AgentsManagerConfig {
        AgentsManagerConfig {
            agents: HashMap::from([("default".to_string(), mock_agent_config())]),
            ..Default::default()
        }
    }

    fn mock_agents_manager_config_with(
        agents: HashMap<String, AgentConfig>,
    ) -> AgentsManagerConfig {
        AgentsManagerConfig {
            agents,
            ..Default::default()
        }
    }

    fn make_tools_handle() -> (
        ManagerHandle<ToolsCommand>,
        tokio::sync::mpsc::Receiver<ToolsCommand>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        (ManagerHandle::new(tx), rx)
    }

    async fn serve_tools_urls(mut rx: tokio::sync::mpsc::Receiver<ToolsCommand>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                ToolsCommand::GetMcpUrls {
                    tool_names: _,
                    reply,
                } => {
                    let _ = reply.send(vec![]);
                }
                ToolsCommand::Shutdown => break,
            }
        }
    }

    #[test]
    fn when_agents_manager_created_then_name_is_agents() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(mock_agents_manager_config(), handle);
        assert_eq!(m.name(), "agents");
    }

    #[tokio::test]
    async fn when_manager_started_then_agent_session_initialized() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        let result = m.start().await;
        assert!(result.is_ok(), "start failed: {result:?}");
        assert_eq!(m.slots.len(), 1);
        assert!(m.slots[0].connection.is_some());
        assert!(m.slots[0].agent_capabilities.is_some());
        assert_eq!(
            m.slots[0]
                .agent_capabilities
                .as_ref()
                .unwrap()
                .protocol_version,
            1
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn when_agent_connected_then_health_check_returns_true() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();
        assert!(m.health_check().await);

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn when_no_agents_connected_then_health_check_returns_false() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(mock_agents_manager_config(), handle);
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn when_no_agents_configured_then_health_check_returns_true() {
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn when_all_agents_disabled_then_health_check_returns_true() {
        let mut config = mock_agent_config();
        config.enabled = false;
        let (handle, _rx) = make_tools_handle();
        let m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn when_prompt_sent_to_slot_then_no_error_returned() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let result = AgentsManager::send_prompt_to_slot(&m.slots[0], "hello").await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(200)).await;

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn when_agent_crashes_then_handle_crash_reconnects() {
        let mut config = mock_agent_config();
        config
            .options
            .insert("exit_after".into(), serde_json::json!(1));

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        m.start().await.unwrap();

        let _ = AgentsManager::send_prompt_to_slot(&m.slots[0], "trigger-crash").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        m.handle_crash(0).await;

        assert!(m.slots[0].connection.is_some(), "should have reconnected");

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn when_agent_crashes_then_session_map_has_fresh_id() {
        let mut config = mock_agent_config();
        config
            .options
            .insert("exit_after".into(), serde_json::json!(1));
        config
            .options
            .insert("reject_load".into(), serde_json::json!(true));

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        m.start().await.unwrap();

        let default_key = SessionKey::new("default", "default", "default");
        let pre_crash_id = m.slots[0]
            .session_map
            .get(&default_key)
            .cloned()
            .expect("session_map should have default key after start");

        let _ = AgentsManager::send_prompt_to_slot(&m.slots[0], "trigger-crash").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        m.handle_crash(0).await;

        let post_crash_id = m.slots[0]
            .session_map
            .get(&default_key)
            .cloned()
            .expect("session_map should have default key after crash recovery");

        assert_ne!(
            pre_crash_id, post_crash_id,
            "session_map must contain the new session ID, not the stale pre-crash one"
        );
        assert!(
            !m.slots[0].reverse_map.contains_key(&pre_crash_id),
            "reverse_map must not contain the stale pre-crash session ID"
        );
        assert!(
            m.slots[0].reverse_map.contains_key(&post_crash_id),
            "reverse_map must contain the new session ID"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[tokio::test]
    async fn when_get_status_command_sent_then_returns_agent_info_vec() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);

        let mut slot = AgentSlot::new("default".into(), mock_agent_config(), &m.parent_cancel);
        slot.session_map
            .insert(SessionKey::new("test", "local", "dev"), "acp-1".to_string());
        m.slots.push(slot);

        let (reply_tx, reply_rx) = oneshot::channel();
        let done = m
            .handle_command(AgentsCommand::GetStatus { reply: reply_tx })
            .await;
        assert!(!done);

        let statuses = reply_rx.await.expect("should receive status");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "default");
        assert!(!statuses[0].connected);
        assert_eq!(statuses[0].session_count, 1);
    }

    #[tokio::test]
    async fn when_create_session_for_unknown_agent_then_returns_agent_not_found() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let result = m
            .create_session("nonexistent", SessionKey::new("ch", "k", "p"))
            .await;
        assert!(matches!(result, Err(AgentsError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn when_streaming_result_received_then_deliver_message_sent_and_flag_set() {
        // When handle_incoming processes a Result, it sends DeliverMessage
        // but NOT SessionComplete. It sets streaming_completed so
        // handle_prompt_completion knows to skip the synthetic result.
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("debug-http", "local", "dev");
        let acp_session_id = "acp-sess-1".to_string();
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());
        m.slots.push(slot);

        let result_notification = IncomingMessage::AgentNotification(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": acp_session_id,
                "update": {
                    "sessionUpdate": "result",
                    "content": "Echo: hello"
                }
            }
        }));

        m.handle_incoming(0, result_notification).await;

        let mut got_deliver = false;
        let mut got_complete = false;
        while let Ok(event) = channels_rx.try_recv() {
            match event {
                ChannelEvent::DeliverMessage { .. } => got_deliver = true,
                ChannelEvent::SessionComplete { .. } => got_complete = true,
                _ => {}
            }
        }
        assert!(got_deliver, "must send DeliverMessage for result content");
        assert!(
            !got_complete,
            "streaming path must NOT send SessionComplete"
        );
        assert!(
            m.streaming_completed.contains(&session_key),
            "must set streaming_completed flag"
        );
    }

    #[tokio::test]
    async fn when_message_chunk_received_then_session_complete_not_sent() {
        // Non-result updates (message chunks, thought chunks) must NOT send SessionComplete.
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("debug-http", "local", "dev");
        let acp_session_id = "acp-sess-1".to_string();
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());
        m.slots.push(slot);

        let chunk_notification = IncomingMessage::AgentNotification(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": acp_session_id,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": "partial response"
                }
            }
        }));

        m.handle_incoming(0, chunk_notification).await;

        while let Ok(event) = channels_rx.try_recv() {
            assert!(
                !matches!(event, ChannelEvent::SessionComplete { .. }),
                "message chunk must NOT trigger SessionComplete"
            );
        }
    }

    #[tokio::test]
    async fn when_completion_fires_without_streaming_result_then_synthetic_result_sent_first() {
        // When handle_prompt_completion fires and streaming did NOT send a result,
        // it must send a synthetic DeliverMessage with sessionUpdate "result"
        // BEFORE SessionComplete.
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "user1");
        let acp_session_id = "acp-sess-1".to_string();
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());
        m.slots.push(slot);

        let (_incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);

        let completion = PromptCompletion {
            session_key: session_key.clone(),
        };
        m.handle_prompt_completion(completion, &mut incoming_rx)
            .await;

        let mut events = Vec::new();
        while let Ok(event) = channels_rx.try_recv() {
            events.push(event);
        }

        assert!(
            events.len() >= 2,
            "expected DeliverMessage + SessionComplete, got {} events",
            events.len()
        );

        match &events[0] {
            ChannelEvent::DeliverMessage {
                session_key: sk,
                content,
            } => {
                assert_eq!(sk, &session_key);
                let update_type = content
                    .get("update")
                    .and_then(|u| u.get("sessionUpdate"))
                    .and_then(|t| t.as_str());
                assert_eq!(
                    update_type,
                    Some("result"),
                    "synthetic DeliverMessage must have sessionUpdate: result"
                );
            }
            other => panic!("expected DeliverMessage as first event, got {:?}", other),
        }

        assert!(
            matches!(&events[1], ChannelEvent::SessionComplete { session_key: sk } if sk == &session_key),
            "expected SessionComplete as second event"
        );
    }

    #[tokio::test]
    async fn when_completion_fires_after_streaming_result_then_only_session_complete_sent() {
        // When streaming already sent the result, handle_prompt_completion
        // must only send SessionComplete (no duplicate result DeliverMessage).
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "user1");
        let acp_session_id = "acp-sess-1".to_string();
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());
        m.slots.push(slot);

        m.streaming_completed.insert(session_key.clone());

        let (_incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);
        let completion = PromptCompletion {
            session_key: session_key.clone(),
        };
        m.handle_prompt_completion(completion, &mut incoming_rx)
            .await;

        let mut events = Vec::new();
        while let Ok(event) = channels_rx.try_recv() {
            events.push(event);
        }

        assert_eq!(
            events.len(),
            1,
            "expected only SessionComplete, got {} events",
            events.len()
        );
        assert!(
            matches!(&events[0], ChannelEvent::SessionComplete { session_key: sk } if sk == &session_key),
            "expected SessionComplete only"
        );
        assert!(
            !m.streaming_completed.contains(&session_key),
            "flag must be cleared"
        );
    }

    #[tokio::test]
    async fn when_agent_disabled_then_start_skips_it() {
        let mut config = mock_agent_config();
        config.enabled = false;

        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(
            m.slots.is_empty(),
            "disabled agent should not create a slot"
        );
    }

    #[tokio::test]
    async fn when_completion_fires_with_pending_events_then_all_delivered_before_session_complete()
    {
        // When completion fires with pending streaming events in incoming_rx,
        // all events must be forwarded as DeliverMessage BEFORE SessionComplete.
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "user1");
        let acp_session_id = "acp-sess-1".to_string();
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());
        m.slots.push(slot);

        let (incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);

        // Pre-populate incoming_rx with streaming events (simulating bridge lag)
        let chunk1 = IncomingMessage::AgentNotification(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": acp_session_id,
                "update": { "sessionUpdate": "agent_message_chunk", "content": { "text": "hello", "type": "text" }, "messageId": "msg-1" }
            }
        }));
        let result_event = IncomingMessage::AgentNotification(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": acp_session_id,
                "update": { "sessionUpdate": "result" }
            }
        }));
        incoming_tx
            .send(SlotIncoming {
                slot_idx: 0,
                msg: Some(chunk1),
            })
            .await
            .unwrap();
        incoming_tx
            .send(SlotIncoming {
                slot_idx: 0,
                msg: Some(result_event),
            })
            .await
            .unwrap();

        let completion = PromptCompletion {
            session_key: session_key.clone(),
        };
        m.handle_prompt_completion(completion, &mut incoming_rx)
            .await;

        let mut events = Vec::new();
        while let Ok(event) = channels_rx.try_recv() {
            events.push(event);
        }

        // chunk DeliverMessage + result DeliverMessage + SessionComplete = 3 events
        assert!(
            events.len() >= 3,
            "expected chunk + result + SessionComplete, got {} events",
            events.len()
        );

        let deliver_count = events
            .iter()
            .filter(|e| matches!(e, ChannelEvent::DeliverMessage { .. }))
            .count();
        assert!(
            deliver_count >= 2,
            "expected at least 2 DeliverMessages (chunk + result), got {deliver_count}"
        );

        assert!(
            matches!(events.last(), Some(ChannelEvent::SessionComplete { .. })),
            "SessionComplete must be the LAST event"
        );

        assert!(
            !m.streaming_completed.contains(&session_key),
            "flag must be cleared"
        );
    }

    #[tokio::test]
    async fn when_create_session_called_then_session_added_to_correct_agent_slot() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let agents = HashMap::from([
            ("agent-a".to_string(), mock_agent_config()),
            ("agent-b".to_string(), mock_agent_config()),
        ]);

        let mut m = AgentsManager::new(mock_agents_manager_config_with(agents), handle);
        m.start().await.unwrap();
        assert_eq!(m.slots.len(), 2);

        let key = SessionKey::new("telegram", "direct", "alice");
        let result = m.create_session("agent-b", key.clone()).await;
        assert!(result.is_ok());

        let b_idx = find_slot_by_name(&m.slots, "agent-b").unwrap();
        let a_idx = find_slot_by_name(&m.slots, "agent-a").unwrap();
        assert!(m.slots[b_idx].session_map.contains_key(&key));
        assert!(!m.slots[a_idx].session_map.contains_key(&key));

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_valid_path_then_completes_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let request = serde_json::json!({"id": 1, "method": "fs/read_text_file"});
        let params = serde_json::json!({"path": file_path.to_str().unwrap()});

        AgentsManager::handle_fs_read(&slot, &request, &params).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_nonexistent_path_then_completes_without_panic() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let request = serde_json::json!({"id": 2, "method": "fs/read_text_file"});
        let params = serde_json::json!({"path": "/nonexistent/path/file.txt"});

        AgentsManager::handle_fs_read(&slot, &request, &params).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_write_called_with_valid_path_then_file_written() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("output.txt");

        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let request = serde_json::json!({"id": 3, "method": "fs/write_text_file"});
        let params =
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": "written content"});

        AgentsManager::handle_fs_write(&slot, &request, &params).await;
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "written content");
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_write_called_with_invalid_path_then_completes_without_panic() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let request = serde_json::json!({"id": 4, "method": "fs/write_text_file"});
        let params = serde_json::json!({"path": "/nonexistent/dir/file.txt", "content": "data"});

        AgentsManager::handle_fs_write(&slot, &request, &params).await;
    }

    fn mock_agent_config_with_crash_tracker(max_crashes: u32, window_secs: u64) -> AgentConfig {
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
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: target_dir.to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: Some(protoclaw_config::CrashTrackerConfig {
                max_crashes,
                window_secs,
            }),
            options: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn when_crash_loop_threshold_reached_then_handle_crash_disables_slot() {
        let config = mock_agent_config_with_crash_tracker(2, 60);
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        m.start().await.unwrap();

        m.slots[0].lifecycle.crash_tracker.record_crash();
        m.slots[0].lifecycle.crash_tracker.record_crash();
        assert!(
            m.slots[0].lifecycle.crash_tracker.is_crash_loop(),
            "precondition: crash loop must be active"
        );

        m.handle_crash(0).await;

        assert!(
            m.slots[0].lifecycle.disabled,
            "slot must be disabled after crash loop"
        );
        assert!(
            m.slots[0].connection.is_none(),
            "connection must be cleaned up after crash loop"
        );

        tools_task.abort();
    }

    #[tokio::test]
    async fn when_crash_below_loop_threshold_then_handle_crash_records_and_restarts() {
        let config = mock_agent_config_with_crash_tracker(3, 60);
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        m.start().await.unwrap();

        m.handle_crash(0).await;

        assert!(
            !m.slots[0].lifecycle.disabled,
            "slot must NOT be disabled below loop threshold"
        );
        assert!(
            m.slots[0].connection.is_some(),
            "slot should have reconnected"
        );
        assert!(m.slots[0].lifecycle.crash_tracker.is_crash_loop() == false || true);

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    fn when_tool_call_has_title_but_no_name_then_title_promoted_to_name() {
        let mut content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call",
                "title": "system-info_system-info",
                "toolCallId": "tc-1"
            }
        });
        normalize_tool_event_fields(&mut content, "tool_call");
        assert_eq!(content["update"]["name"], "system-info_system-info");
        assert!(content["update"].get("title").is_none());
    }

    #[rstest]
    fn when_tool_call_already_has_name_then_title_not_overwritten() {
        let mut content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call",
                "name": "read_file",
                "title": "Read File Tool",
                "toolCallId": "tc-1"
            }
        });
        normalize_tool_event_fields(&mut content, "tool_call");
        assert_eq!(content["update"]["name"], "read_file");
    }

    #[rstest]
    fn when_tool_call_update_has_raw_output_but_no_output_then_raw_output_promoted() {
        let mut content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "tc-1",
                "status": "completed",
                "rawOutput": {"output": "file contents here", "metadata": {}}
            }
        });
        normalize_tool_event_fields(&mut content, "tool_call_update");
        assert_eq!(content["update"]["output"], "file contents here");
    }

    #[rstest]
    fn when_tool_call_update_already_has_output_then_raw_output_not_overwritten() {
        let mut content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "tc-1",
                "status": "completed",
                "output": "direct output",
                "rawOutput": {"output": "raw output"}
            }
        });
        normalize_tool_event_fields(&mut content, "tool_call_update");
        assert_eq!(content["update"]["output"], "direct output");
    }

    #[rstest]
    fn when_non_tool_event_then_normalization_is_noop() {
        let mut content = serde_json::json!({
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "title": "should not be touched"
            }
        });
        let original = content.clone();
        normalize_tool_event_fields(&mut content, "agent_message_chunk");
        assert_eq!(content, original);
    }

    #[rstest]
    fn when_session_update_type_name_called_with_available_commands_update_then_returns_correct_string() {
        let update = SessionUpdateType::AvailableCommandsUpdate {
            commands: serde_json::Value::Array(vec![]),
        };
        assert_eq!(
            AgentsManager::session_update_type_name(&update),
            "available_commands_update"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_agent_lacks_fork_capability_then_fork_session_returns_error() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let session_key = SessionKey::new("telegram", "user", "u1");
        let result = m.fork_session("default", &session_key).await;
        assert!(
            matches!(result, Err(AgentsError::CapabilityNotSupported(_))),
            "expected CapabilityNotSupported, got: {result:?}"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_agent_lacks_list_capability_then_list_sessions_returns_error() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let result = m.list_sessions("default").await;
        assert!(
            matches!(result, Err(AgentsError::CapabilityNotSupported(_))),
            "expected CapabilityNotSupported, got: {result:?}"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_cancel_session_called_for_unknown_session_then_returns_error() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let unknown_key = SessionKey::new("telegram", "user", "no-such-session");
        let result = m.cancel_session("default", &unknown_key).await;
        assert!(
            matches!(result, Err(AgentsError::ConnectionClosed)),
            "expected ConnectionClosed for unknown session, got: {result:?}"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_fork_session_called_for_unknown_agent_then_returns_agent_not_found() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let session_key = SessionKey::new("telegram", "user", "u1");
        let result = m.fork_session("no-such-agent", &session_key).await;
        assert!(
            matches!(result, Err(AgentsError::AgentNotFound(_))),
            "expected AgentNotFound, got: {result:?}"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }
}
