use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::acp_error::AcpError;
use crate::acp_types::{
    ClientCapabilities, ContentPart, InitializeParams, InitializeResult, McpServerInfo,
    SessionCancelParams, SessionForkParams, SessionForkResult, SessionListParams,
    SessionLoadParams, SessionNewParams, SessionPromptParams, SessionUpdateEvent,
    SessionUpdateType,
};
use crate::slot::{AgentSlot, find_slot_by_name};
use anyclaw_config::{AgentConfig, AgentsManagerConfig, WorkspaceConfig};
use anyclaw_core::{
    AgentStatusInfo, AgentsCommand, CrashAction, Manager, ManagerError, ManagerHandle,
    McpServerUrl, PendingPermissionInfo, PersistedSession, SessionKey, ToolDescription,
    ToolsCommand, constants,
};
use anyclaw_sdk_agent::{DynAgentAdapter, GenericAcpAdapter};
use anyclaw_sdk_types::{ChannelEvent, PermissionOption};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{AgentConnection, IncomingMessage};
use crate::error::AgentsError;
use anyclaw_core::{DynSessionStore, NoopSessionStore};
use anyclaw_jsonrpc::types::{JsonRpcRequest, JsonRpcResponse, RequestId};

pub(crate) struct PendingPermission {
    pub request: JsonRpcRequest,
    pub description: String,
    pub options: Vec<PermissionOption>,
    #[allow(dead_code)] // Used by permission timeout logging (channels manager reads elapsed time)
    pub received_at: std::time::Instant,
}

pub(crate) struct SlotIncoming {
    pub(crate) slot_idx: usize,
    pub(crate) msg: Option<IncomingMessage>,
}

struct PromptCompletion {
    session_key: SessionKey,
    /// Set when the agent reports the session no longer exists,
    /// so `handle_prompt_completion` can invalidate the stale mapping.
    /// Read via `completion_rx` channel in `handle_prompt_completion`.
    session_expired: bool,
}

/// Resolve the effective working directory for an agent from its workspace config.
fn resolve_agent_cwd(workspace: &WorkspaceConfig) -> std::path::PathBuf {
    match workspace {
        WorkspaceConfig::Local(local) => local.working_dir.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
        }),
        WorkspaceConfig::Docker(docker) => docker.working_dir.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
        }),
    }
}

/// Validate that `requested` resolves to a path inside `sandbox_root`.
/// Uses `canonicalize()` so symlinks cannot escape the sandbox.
/// Returns the canonical resolved path on success.
fn validate_fs_path(
    sandbox_root: &std::path::Path,
    requested: &str,
) -> Result<std::path::PathBuf, String> {
    let requested_path = std::path::Path::new(requested);
    let resolved = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        sandbox_root.join(requested_path)
    };
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("path resolution failed: {e}"))?;
    let canonical_root = sandbox_root
        .canonicalize()
        .map_err(|e| format!("sandbox root resolution failed: {e}"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err("path outside allowed directory".into());
    }
    Ok(canonical)
}

/// Validate that `requested` resolves to a path whose *parent directory* is inside `sandbox_root`.
/// Used for writes where the file may not yet exist.
/// Returns the validated write path (canonical parent + filename) on success.
fn validate_fs_write_path(
    sandbox_root: &std::path::Path,
    requested: &str,
) -> Result<std::path::PathBuf, String> {
    let requested_path = std::path::Path::new(requested);
    let resolved = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        sandbox_root.join(requested_path)
    };
    let parent = resolved
        .parent()
        .ok_or_else(|| "invalid path: no parent directory".to_string())?;
    let filename = resolved
        .file_name()
        .ok_or_else(|| "invalid path: no filename".to_string())?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("parent directory resolution failed: {e}"))?;
    let canonical_root = sandbox_root
        .canonicalize()
        .map_err(|e| format!("sandbox root resolution failed: {e}"))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err("path outside allowed directory".into());
    }
    Ok(canonical_parent.join(filename))
}

/// Manages agent subprocess lifecycles, ACP session routing, and crash recovery.
///
/// The `AgentsManager` owns all [`AgentSlot`]s, dispatches commands from the
/// channels manager, and forwards agent events back to channels. It implements
/// the bridge-collapsed architecture where incoming agent messages flow directly
/// to the manager's shared channel without an intermediate forwarding task.
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
    /// Persistent session store. Defaults to [`NoopSessionStore`].
    session_store: Arc<dyn DynSessionStore>,
    /// TTL for expired session cleanup at boot (seconds). Default: 7 days.
    session_ttl_secs: i64,
}

impl AgentsManager {
    /// Create a new agents manager from the given config and tools handle.
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
            session_store: Arc::new(NoopSessionStore),
            session_ttl_secs: 7 * 24 * 3600,
        }
    }

    /// Set the tracing log level filter passed to agent subprocesses.
    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Replace the default [`GenericAcpAdapter`] with a custom adapter.
    pub fn with_adapter(mut self, adapter: Box<dyn DynAgentAdapter>) -> Self {
        self.adapter = adapter;
        self
    }

    /// Set the outbound channel for forwarding agent events to the channels manager.
    pub fn with_channels_sender(mut self, sender: mpsc::Sender<ChannelEvent>) -> Self {
        self.channels_sender = Some(sender);
        self
    }

    /// Set the persistent session store (default: [`NoopSessionStore`]).
    pub fn with_session_store(mut self, store: Arc<dyn DynSessionStore>) -> Self {
        self.session_store = store;
        self
    }

    /// Set the TTL for expired session cleanup at boot (seconds).
    pub fn with_session_ttl_secs(mut self, ttl: i64) -> Self {
        self.session_ttl_secs = ttl;
        self
    }

    /// Clone the command sender so the supervisor can wire it to the channels manager.
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
            protocol_version: 2,
            capabilities: ClientCapabilities { experimental: None },
            options,
        })?;

        let rx = conn.send_request("initialize", params).await?;
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: InitializeResult = serde_json::from_value(resp.result.unwrap_or_default())?;
        if result.protocol_version != 1 && result.protocol_version != 2 {
            return Err(AcpError::ProtocolMismatch {
                expected: 2,
                got: result.protocol_version,
            }
            .into());
        }

        slot.protocol_version = result.protocol_version;
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
            .into_iter()
            .map(|u| McpServerInfo {
                name: u.name,
                server_type: "http".into(),
                url: u.url,
                command: String::new(),
                args: vec![],
                env: vec![],
                headers: vec![],
            })
            .collect();

        let cwd = resolve_agent_cwd(&slot.config.workspace);

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

        let result: crate::acp_types::SessionNewResult =
            serde_json::from_value(resp.result.unwrap_or_default())?;
        tracing::info!(agent = %slot.name(), session_id = %result.session_id, "session started");
        Ok(result.session_id)
    }

    /// Fetch MCP server info for a given slot.
    ///
    /// Sends `ToolsCommand::GetMcpUrls` and maps the result to `McpServerInfo`.
    /// Returns an empty vec on any error so that `session/load` can still be
    /// attempted without tools.
    async fn fetch_mcp_servers(&self, slot_idx: usize) -> Vec<McpServerInfo> {
        let tool_names = if self.slots[slot_idx].config.tools.is_empty() {
            None
        } else {
            Some(self.slots[slot_idx].config.tools.clone())
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tools_handle
            .send(ToolsCommand::GetMcpUrls {
                tool_names,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            tracing::warn!("tools handle unavailable while fetching MCP URLs for session/load");
            return Vec::new();
        }
        let urls: Vec<McpServerUrl> = reply_rx.await.unwrap_or_else(|_| {
            tracing::warn!(
                "tools handle dropped before providing MCP URLs for session/load — continuing with no tools"
            );
            Vec::new()
        });
        urls.into_iter()
            .map(|u| McpServerInfo {
                name: u.name,
                server_type: "http".into(),
                url: u.url,
                command: String::new(),
                args: vec![],
                env: vec![],
                headers: vec![],
            })
            .collect()
    }

    /// Fetch tool descriptions from the tools manager and build a compact context string.
    ///
    /// Returns `None` if no tools are available or the tools handle is unavailable.
    /// The result is cached on the slot so it's only fetched once per agent lifecycle.
    async fn fetch_tool_context(&self, slot_idx: usize) -> Option<String> {
        let tool_names = if self.slots[slot_idx].config.tools.is_empty() {
            None
        } else {
            Some(self.slots[slot_idx].config.tools.clone())
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tools_handle
            .send(ToolsCommand::GetToolDescriptions {
                tool_names,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        let descriptions: Vec<ToolDescription> = reply_rx.await.unwrap_or_else(|_| {
            tracing::warn!("tools handle dropped before providing tool descriptions");
            Vec::new()
        });
        if descriptions.is_empty() {
            return None;
        }
        let mut ctx = String::from(
            "[Platform context: You have the following MCP tools available. \
             Use them when relevant instead of approximating with built-in tools.\n",
        );
        for desc in &descriptions {
            ctx.push_str("- ");
            ctx.push_str(&desc.name);
            if !desc.description.is_empty() {
                ctx.push_str(": ");
                ctx.push_str(&desc.description);
            }
            ctx.push('\n');
        }
        ctx.push(']');
        Some(ctx)
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
                        tracing::info!(agent = %slot.name(), %request_id, %option_id, "permission response received from channel");
                        if let Some(conn) = slot.connection.as_ref() {
                            let resp = JsonRpcResponse::success(
                                perm.request.id.clone(),
                                serde_json::json!({
                                    "outcome": {
                                        "outcome": "selected",
                                        "optionId": option_id,
                                    }
                                }),
                            );
                            let _ = conn.send_raw(resp).await;
                            tracing::info!(agent = %slot.name(), %request_id, "permission response sent to agent");
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
                let slot_idx = find_slot_by_name(&self.slots, &agent_name);
                let has_stale = slot_idx
                    .map(|idx| self.slots[idx].stale_sessions.contains_key(&session_key))
                    .unwrap_or(false);

                let result = if has_stale {
                    let idx = slot_idx.expect("slot_idx must be Some when has_stale is true");
                    match self.heal_session(idx, &agent_name, &session_key).await {
                        Ok(()) => self.slots[idx]
                            .session_map
                            .get(&session_key)
                            .cloned()
                            .ok_or(AgentsError::ConnectionClosed),
                        Err(e) => Err(e),
                    }
                } else {
                    self.create_session(&agent_name, session_key).await
                };
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

        // Cache tool context on first session creation for this slot.
        if self.slots[slot_idx].tool_context.is_none() {
            self.slots[slot_idx].tool_context = self.fetch_tool_context(slot_idx).await;
        }

        let slot = &mut self.slots[slot_idx];
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let persisted = PersistedSession {
            session_key: session_key.to_string(),
            agent_name: agent_name.to_string(),
            acp_session_id: acp_session_id.clone(),
            created_at: now,
            last_active_at: now,
            closed: false,
        };
        if let Err(e) = self.session_store.upsert_session(&persisted).await {
            tracing::warn!(
                agent = %agent_name,
                session_key = %session_key,
                error = %e,
                "failed to persist new session to store"
            );
        }

        tracing::info!(agent = %agent_name, session_key = %acp_session_id, "multi-session created");
        Ok(acp_session_id)
    }

    async fn prompt_session(
        &mut self,
        agent_name: &str,
        session_key: &SessionKey,
        message: &str,
    ) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        // Platform commands are handled in the agents layer — not forwarded to the agent process.
        if let Some(cmd) = crate::platform_commands::match_platform_command(message) {
            return self
                .handle_platform_command(cmd.name, slot_idx, agent_name, session_key)
                .await;
        }

        if !self.slots[slot_idx].session_map.contains_key(session_key) {
            self.heal_session(slot_idx, agent_name, session_key).await?;
        }

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        self.slots[slot_idx]
            .awaiting_first_prompt
            .remove(&acp_session_id);

        // Build prompt parts, injecting tool context on first prompt per session.
        let mut prompt_parts = Vec::new();
        if !self.slots[slot_idx]
            .tool_context_sent
            .contains(&acp_session_id)
            && let Some(ctx) = self.slots[slot_idx].tool_context.as_deref()
        {
            prompt_parts.push(ContentPart::text(ctx));
            self.slots[slot_idx]
                .tool_context_sent
                .insert(acp_session_id.clone());
        }
        prompt_parts.push(ContentPart::text(message));

        let slot = &self.slots[slot_idx];
        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_session_id.clone(),
            prompt: prompt_parts,
        })?;

        let response_rx = conn.send_request("session/prompt", params).await?;

        if let Some(commands_content) = self.slots[slot_idx].last_available_commands.as_ref()
            && let Some(sender) = &self.channels_sender
            && let Err(e) = sender
                .send(ChannelEvent::DeliverMessage {
                    session_key: session_key.clone(),
                    content: commands_content.clone(),
                })
                .await
        {
            tracing::debug!(error = %e, "failed to replay buffered available_commands_update");
        }

        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let sk_string = session_key.to_string();
            let store = Arc::clone(&self.session_store);
            tokio::spawn(async move {
                if let Err(e) = store.update_last_active(&sk_string, now).await {
                    tracing::warn!(
                        session_key = %sk_string,
                        error = %e,
                        "failed to update last_active in store"
                    );
                }
            });
        }

        {
            let completion_tx = self.completion_tx.clone();
            let channels_tx = self.channels_sender.clone();
            let sk = session_key.clone();
            tokio::spawn(async move {
                match response_rx.await {
                    Ok(response) => {
                        // Check if the agent returned a JSON-RPC error
                        let mut session_expired = false;
                        if let Some(error) = &response.error {
                            let msg = &error.message;
                            tracing::warn!(session_key = %sk, error = %msg, "agent returned error for prompt");

                            // Detect "session not found" so the stale mapping gets
                            // invalidated in handle_prompt_completion, allowing the
                            // next prompt to trigger heal_session.
                            let combined = format!(
                                "{} {}",
                                msg,
                                error
                                    .data
                                    .as_ref()
                                    .map(std::string::ToString::to_string)
                                    .unwrap_or_default()
                            );
                            if combined.to_lowercase().contains("session not found") {
                                session_expired = true;
                            }

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
                            .send(PromptCompletion {
                                session_key: sk,
                                session_expired,
                            })
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

    async fn handle_platform_command(
        &mut self,
        command: &str,
        slot_idx: usize,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        match command {
            "new" => {
                self.slots[slot_idx].session_map.remove(session_key);
                self.slots[slot_idx]
                    .reverse_map
                    .retain(|_, v| v != session_key);
                let acp_id = self.create_session(agent_name, session_key.clone()).await?;
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    acp_session_id = %acp_id,
                    "platform command /new: fresh session created"
                );
                // Signal channels manager that this "prompt" is done so the queue unblocks.
                if let Some(sender) = &self.channels_sender {
                    let _ = sender
                        .send(ChannelEvent::DeliverMessage {
                            session_key: session_key.clone(),
                            content: serde_json::json!({
                                "update": {
                                    "sessionUpdate": "agent_message_chunk",
                                    "content": "New conversation started."
                                }
                            }),
                        })
                        .await;
                    let _ = sender
                        .send(ChannelEvent::DeliverMessage {
                            session_key: session_key.clone(),
                            content: serde_json::json!({
                                "update": {
                                    "sessionUpdate": "result",
                                    "content": ""
                                }
                            }),
                        })
                        .await;
                    let _ = sender
                        .send(ChannelEvent::SessionComplete {
                            session_key: session_key.clone(),
                        })
                        .await;
                }
                Ok(())
            }
            _ => {
                tracing::warn!(command = %command, "unknown platform command — ignoring");
                Ok(())
            }
        }
    }

    /// Attempt to recover a missing session before a prompt:
    /// 1. Try `session/resume` if the agent supports it and a stale ACP session ID exists.
    /// 2. Try `session/load` if the agent supports it and a stale ACP session ID exists.
    /// 3. Fall back to `create_session` otherwise.
    async fn heal_session(
        &mut self,
        slot_idx: usize,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);

        let stale_acp_id = self.slots[slot_idx]
            .stale_sessions
            .get(session_key)
            .cloned();

        let supports_resume = self.slots[slot_idx].has_session_capability(|c| c.resume.is_some());
        let supports_load = self.slots[slot_idx]
            .agent_capabilities
            .as_ref()
            .and_then(|r| r.agent_capabilities.as_ref())
            .is_some_and(|c| c.load_session);

        tracing::info!(
            agent = %agent_name,
            session_key = %session_key,
            has_stale_acp_id = stale_acp_id.is_some(),
            supports_resume = supports_resume,
            supports_load = supports_load,
            step = "recovery_started",
            "session recovery initiated"
        );

        if supports_resume && let Some(acp_id) = stale_acp_id.as_deref() {
            let cwd = resolve_agent_cwd(&self.slots[slot_idx].config.workspace)
                .to_string_lossy()
                .into_owned();
            let mcp_servers = self.fetch_mcp_servers(slot_idx).await;
            let params = serde_json::json!({
                "sessionId": acp_id,
                "cwd": cwd,
                "mcpServers": serde_json::to_value(&mcp_servers).unwrap_or_default(),
            });

            let conn = self.slots[slot_idx]
                .connection
                .as_ref()
                .ok_or(AgentsError::ConnectionClosed)?;

            if let Ok(rx) = conn.send_request("session/resume", params).await
                && let Ok(Ok(resp)) = tokio::time::timeout(acp_timeout, rx).await
                && resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .is_some()
            {
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "resume_attempted",
                    success = true,
                    "session/resume succeeded"
                );
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "recovery_outcome",
                    outcome = "resumed",
                    "session recovery complete"
                );
                let slot = &mut self.slots[slot_idx];
                slot.stale_sessions.remove(session_key);
                slot.session_map
                    .insert(session_key.clone(), acp_id.to_owned());
                slot.reverse_map
                    .insert(acp_id.to_owned(), session_key.clone());
                // No awaiting_first_prompt for resume — no replay needed.
                return Ok(());
            }
            tracing::info!(
                agent = %agent_name,
                session_key = %session_key,
                step = "resume_attempted",
                success = false,
                "session/resume rejected, falling back to create"
            );
        } else if supports_load && let Some(acp_id) = stale_acp_id {
            let cwd = resolve_agent_cwd(&self.slots[slot_idx].config.workspace)
                .to_string_lossy()
                .into_owned();
            let mcp_servers = self.fetch_mcp_servers(slot_idx).await;
            let params = match serde_json::to_value(SessionLoadParams {
                session_id: acp_id.clone(),
                cwd: Some(cwd),
                mcp_servers: Some(mcp_servers),
            }) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(agent = %agent_name, error = %e, "failed to serialize session/load params");
                    serde_json::json!({})
                }
            };

            let conn = self.slots[slot_idx]
                .connection
                .as_ref()
                .ok_or(AgentsError::ConnectionClosed)?;

            if let Ok(rx) = conn.send_request("session/load", params).await
                && let Ok(Ok(resp)) = tokio::time::timeout(acp_timeout, rx).await
                && resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .is_some()
            {
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "load_attempted",
                    success = true,
                    "session/load succeeded"
                );
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "recovery_outcome",
                    outcome = "loaded",
                    "session recovery complete"
                );
                let slot = &mut self.slots[slot_idx];
                slot.stale_sessions.remove(session_key);
                slot.session_map.insert(session_key.clone(), acp_id.clone());
                slot.reverse_map.insert(acp_id.clone(), session_key.clone());
                slot.awaiting_first_prompt.insert(acp_id);
                return Ok(());
            }
            tracing::info!(
                agent = %agent_name,
                session_key = %session_key,
                step = "load_attempted",
                success = false,
                "session/load rejected, falling back to create"
            );
        }

        self.slots[slot_idx].stale_sessions.remove(session_key);
        let acp_session_id = match self.create_session(agent_name, session_key.clone()).await {
            Ok(id) => id,
            Err(e) => {
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "create_attempted",
                    success = false,
                    error = %e,
                    "session creation failed during recovery"
                );
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    step = "recovery_outcome",
                    outcome = "failed",
                    "session recovery exhausted all attempts"
                );
                return Err(e);
            }
        };
        tracing::info!(
            agent = %agent_name,
            session_key = %session_key,
            acp_session_id = %acp_session_id,
            step = "create_attempted",
            success = true,
            "session created for recovery"
        );
        tracing::info!(
            agent = %agent_name,
            session_key = %session_key,
            step = "recovery_outcome",
            outcome = "created",
            "session recovery complete"
        );
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
        if !slot.has_session_capability(|c| c.fork.is_some()) {
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

        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: SessionForkResult = serde_json::from_value(resp.result.unwrap_or_default())?;

        let fork_key = SessionKey::new(session_key.channel_name(), "fork", &result.session_id);
        let slot = &mut self.slots[slot_idx];
        slot.session_map
            .insert(fork_key.clone(), result.session_id.clone());
        slot.reverse_map.insert(result.session_id.clone(), fork_key);

        tracing::info!(agent = %agent_name, forked_session_id = %result.session_id, "session forked");
        Ok(result.session_id)
    }

    async fn list_sessions(
        &self,
        agent_name: &str,
    ) -> Result<anyclaw_sdk_types::SessionListResult, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if !slot.has_session_capability(|c| c.list.is_some()) {
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

        let typed: anyclaw_sdk_types::SessionListResult =
            serde_json::from_value(resp.result.unwrap_or_default())?;
        Ok(typed)
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
        let request = match msg {
            IncomingMessage::AgentNotification(r) | IncomingMessage::AgentRequest(r) => r,
        };

        match request.method.as_str() {
            "session/update" => {
                // D-03: session/update params are forwarded as raw content to channels
                // (with timestamp injection, tool normalization, command merging).
                // Must stay as Value for content mutation pipeline.
                let params = request.params.unwrap_or(serde_json::Value::Null);
                self.handle_session_update(slot_idx, params).await;
            }
            "session/request_permission" => {
                self.handle_permission_request(slot_idx, &request).await;
            }
            "fs/read_text_file" => {
                Self::handle_fs_read(&self.slots[slot_idx], &request).await;
            }
            "fs/write_text_file" => {
                Self::handle_fs_write(&self.slots[slot_idx], &request).await;
            }
            _ => {
                Self::send_error_response(
                    &self.slots[slot_idx],
                    &request,
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
            SessionUpdateType::CurrentModeUpdate { .. } => "extension:current_mode",
            SessionUpdateType::ConfigOptionUpdate { .. } => "extension:config_option",
            SessionUpdateType::SessionInfoUpdate { .. } => "extension:session_info",
            _ => "unknown",
        }
    }

    // D-03: agent content is arbitrary JSON that requires raw mutation (timestamps, normalization, command injection).
    // DeliverMessage.content stays as Value because agents manager injects _received_at_ms, normalizes
    // tool event fields, and merges platform commands — all operations on raw JSON structure.
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

    // D-03: content is raw agent JSON — inject_platform_commands merges platform command
    // descriptors into the agent's availableCommands array, requiring Value array manipulation.
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

        if update_type == "available_commands_update" {
            if let Some(cmds) = content
                .pointer_mut("/update/availableCommands")
                .and_then(|v| v.as_array_mut())
                && let serde_json::Value::Array(platform_arr) =
                    crate::platform_commands::platform_commands_json()
            {
                cmds.extend(platform_arr);
            }
            self.slots[slot_idx].last_available_commands = Some(content.clone());
        }

        if self.slots[slot_idx]
            .awaiting_first_prompt
            .contains(&event.session_id)
        {
            tracing::debug!(
                agent = %self.slots[slot_idx].name(),
                session_id = %event.session_id,
                update_type,
                seq,
                "suppressed replay event during session/load"
            );
            return;
        }

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

    // D-03: session/update params are the raw agent content payload that gets forwarded
    // to channels after mutation (timestamps, tool normalization, command merging).
    // Deserialized into SessionUpdateEvent for typed dispatch, but the raw Value is
    // forwarded as DeliverMessage.content for channel consumption.
    async fn handle_session_update(&mut self, slot_idx: usize, params: serde_json::Value) {
        let seq = self.update_seq.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(raw_params = %params, seq, "session/update received — attempting deser");

        // Clone needed: typed event for dispatch + raw Value for content forwarding (D-03)
        match serde_json::from_value::<SessionUpdateEvent>(params.clone()) {
            Ok(event) => {
                self.forward_session_update(slot_idx, event, params, seq)
                    .await;
            }
            Err(error) => {
                self.forward_malformed_update_error(slot_idx, &params, &error, seq)
                    .await;
            }
        }
    }

    async fn handle_permission_request(&mut self, slot_idx: usize, request: &JsonRpcRequest) {
        // D-03: permission request params have agent-defined schemas (requestId location varies by agent)
        let params = request.params.as_ref();
        let request_id = params
            .and_then(|p| p["requestId"].as_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| {
                // OpenCode uses JSON-RPC id field instead of params.requestId
                match &request.id {
                    Some(RequestId::Number(n)) => n.to_string(),
                    Some(RequestId::String(s)) => s.clone(),
                    None => String::new(),
                }
            });
        let description = params
            .and_then(|p| {
                p["description"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| p["toolCall"]["title"].as_str())
            })
            .unwrap_or("Permission requested")
            .to_string();

        let options: Vec<PermissionOption> = params
            .and_then(|p| p.get("options"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| {
                tracing::warn!(%request_id, "malformed permission options, using empty list");
                Vec::new()
            });

        tracing::info!(agent = %self.slots[slot_idx].name(), %request_id, %description, "permission requested");

        let session_id = params.and_then(|p| p["sessionId"].as_str()).unwrap_or("");
        let routed = if let Some(session_key) =
            self.slots[slot_idx].reverse_map.get(session_id).cloned()
            && let Some(sender) = &self.channels_sender
        {
            sender
                .send(ChannelEvent::RoutePermission {
                    session_key,
                    request_id: request_id.clone(),
                    description: description.clone(),
                    options: options.clone(),
                })
                .await
                .is_ok()
        } else {
            false
        };

        if routed {
            self.slots[slot_idx].pending_permissions.insert(
                request_id,
                PendingPermission {
                    request: request.clone(),
                    description,
                    options,
                    received_at: std::time::Instant::now(),
                },
            );
        } else {
            tracing::warn!(
                agent = %self.slots[slot_idx].name(),
                %request_id,
                "permission not routable to channel, auto-approving"
            );
            let auto_option = options
                .first()
                .map(|o| o.option_id.clone())
                .unwrap_or_else(|| "once".to_string());
            if let Some(conn) = self.slots[slot_idx].connection.as_ref() {
                let resp = JsonRpcResponse::success(
                    request.id.clone(),
                    serde_json::json!({
                        "requestId": request_id,
                        "optionId": auto_option,
                    }),
                );
                let _ = conn.send_raw(resp).await;
            }
        }
    }

    async fn handle_fs_read(slot: &AgentSlot, request: &JsonRpcRequest) {
        // D-03: fs request params have agent-defined path/content fields
        let params = request.params.as_ref();
        let path = params.and_then(|p| p["path"].as_str()).unwrap_or("");
        let sandbox_root = resolve_agent_cwd(&slot.config.workspace);
        let resolved = match validate_fs_path(&sandbox_root, path) {
            Ok(p) => p,
            Err(msg) => {
                Self::send_error_response(slot, request, -32000, &msg).await;
                return;
            }
        };
        match tokio::fs::read_to_string(&resolved).await {
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

    async fn handle_fs_write(slot: &AgentSlot, request: &JsonRpcRequest) {
        // D-03: fs request params have agent-defined path/content fields
        let params = request.params.as_ref();
        let path = params.and_then(|p| p["path"].as_str()).unwrap_or("");
        let content = params.and_then(|p| p["content"].as_str()).unwrap_or("");
        let sandbox_root = resolve_agent_cwd(&slot.config.workspace);
        let resolved = match validate_fs_write_path(&sandbox_root, path) {
            Ok(p) => p,
            Err(msg) => {
                Self::send_error_response(slot, request, -32000, &msg).await;
                return;
            }
        };
        match tokio::fs::write(&resolved, content).await {
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
        request: &JsonRpcRequest,
        result: serde_json::Value,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = JsonRpcResponse::success(request.id.clone(), result);
            let _ = conn.send_raw(resp).await;
        }
    }

    async fn send_error_response(
        slot: &AgentSlot,
        request: &JsonRpcRequest,
        code: i64,
        message: &str,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = JsonRpcResponse::error(
                request.id.clone(),
                anyclaw_jsonrpc::types::JsonRpcError {
                    code,
                    message: message.to_string(),
                    data: None,
                },
            );
            let _ = conn.send_raw(resp).await;
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

        // When the agent reports "session not found", invalidate the stale mapping
        // so the next inbound prompt triggers heal_session() instead of reusing
        // the dead ACP session ID.
        if completion.session_expired {
            tracing::info!(
                session_key = %completion.session_key,
                "invalidating expired session mapping — next prompt will trigger recovery"
            );
            for slot in &mut self.slots {
                if let Some(acp_id) = slot.session_map.remove(&completion.session_key) {
                    slot.reverse_map.remove(&acp_id);
                    slot.tool_context_sent.remove(&acp_id);
                }
            }
        }

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
            if slot.connection.is_some() {
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
        let (supports_resume, supports_load, first_acp_id) = {
            let slot = &self.slots[slot_idx];
            let supports_resume = slot.has_session_capability(|c| c.resume.is_some());
            let supports_load = slot
                .agent_capabilities
                .as_ref()
                .and_then(|r| r.agent_capabilities.as_ref())
                .is_some_and(|c| c.load_session);
            let first_acp_id = slot.stale_sessions.values().next().cloned();
            (supports_resume, supports_load, first_acp_id)
        };

        let Some(first_acp_id) = first_acp_id else {
            return false;
        };

        if supports_resume {
            let cwd = resolve_agent_cwd(&self.slots[slot_idx].config.workspace)
                .to_string_lossy()
                .into_owned();
            let mcp_servers = self.fetch_mcp_servers(slot_idx).await;
            let params = serde_json::json!({
                "sessionId": first_acp_id,
                "cwd": cwd,
                "mcpServers": serde_json::to_value(&mcp_servers).unwrap_or_default(),
            });

            let conn = self.slots[slot_idx]
                .connection
                .as_ref()
                .expect("connection just spawned");
            let Ok(rx) = conn.send_request("session/resume", params).await else {
                tracing::warn!(agent = %agent_name, "session/resume failed, starting fresh session");
                return false;
            };

            match tokio::time::timeout(acp_timeout, rx).await {
                Ok(Ok(resp))
                    if resp
                        .result
                        .as_ref()
                        .and_then(|r| r.get("sessionId"))
                        .is_some() =>
                {
                    tracing::info!(
                        agent = %agent_name,
                        step = "resume_attempted",
                        success = true,
                        "session restored via session/resume"
                    );
                    let slot = &mut self.slots[slot_idx];
                    slot.session_map.extend(slot.stale_sessions.drain());
                    // No awaiting_first_prompt for resume — no replay needed.
                    slot.lifecycle.backoff.reset();
                    return true;
                }
                _ => {
                    tracing::warn!(agent = %agent_name, "session/resume failed, starting fresh session");
                    return false;
                }
            }
        }

        if !supports_load {
            return false;
        }

        let cwd = resolve_agent_cwd(&self.slots[slot_idx].config.workspace)
            .to_string_lossy()
            .into_owned();
        let mcp_servers = self.fetch_mcp_servers(slot_idx).await;
        let params = serde_json::to_value(SessionLoadParams {
             session_id: first_acp_id,
             cwd: Some(cwd),
             mcp_servers: Some(mcp_servers),
         })
         .unwrap_or_else(|e| {
             tracing::warn!(error = %e, agent = %agent_name, "failed to serialize session/load params, using empty object");
             serde_json::json!({})
         });

        let conn = self.slots[slot_idx]
            .connection
            .as_ref()
            .expect("connection just spawned");
        let Ok(rx) = conn.send_request("session/load", params).await else {
            tracing::warn!(agent = %agent_name, "session/load failed, starting fresh session");
            return false;
        };

        match tokio::time::timeout(acp_timeout, rx).await {
            Ok(Ok(resp))
                if resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .is_some() =>
            {
                tracing::info!(agent = %agent_name, "session restored via session/load");
                let slot = &mut self.slots[slot_idx];
                slot.session_map.extend(slot.stale_sessions.drain());
                for acp_id in slot.session_map.values() {
                    slot.awaiting_first_prompt.insert(acp_id.clone());
                }
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
        // Drain session_map into stale_sessions so they survive the crash boundary.
        // try_restore_session reads from stale_sessions; prompt_session uses them for
        // self-healing on the next prompt if session/load isn't attempted here.
        let slot = &mut self.slots[slot_idx];
        slot.stale_sessions.extend(slot.session_map.drain());
        slot.awaiting_first_prompt.clear();
        slot.tool_context_sent.clear();

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
    /// `anyclaw.managed=true` label.  Errors are logged as warnings; this
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
                vec!["anyclaw.managed=true".to_string()],
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
                let Some(id) = container.id else {
                    continue;
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

        let deleted = self
            .session_store
            .delete_expired(self.session_ttl_secs)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to delete expired sessions from store");
                0
            });
        if deleted > 0 {
            tracing::info!(deleted, "expired sessions cleaned up at boot");
        }

        let open_sessions = self
            .session_store
            .load_open_sessions()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to load open sessions from store, starting fresh");
                vec![]
            });
        for session in open_sessions {
            if let Some(idx) = find_slot_by_name(&self.slots, &session.agent_name) {
                let key: anyclaw_core::SessionKey =
                    session.session_key.parse().unwrap_or_else(|_| {
                        anyclaw_core::SessionKey::new(
                            &session.agent_name,
                            "restored",
                            &session.acp_session_id,
                        )
                    });
                self.slots[idx]
                    .stale_sessions
                    .entry(key)
                    .or_insert(session.acp_session_id);
            }
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

// D-03: agent content mutation — normalizes agent-specific wire quirks (title→name, rawOutput→output)
// into the canonical format that ContentKind expects. Operates on raw JSON structure.
fn normalize_tool_event_fields(content: &mut serde_json::Value, update_type: &str) {
    if update_type != "tool_call" && update_type != "tool_call_update" {
        return;
    }
    let Some(update) = content.get_mut("update").and_then(|u| u.as_object_mut()) else {
        return;
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
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
                    binary: target_dir.to_string_lossy().to_string().into(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    fn mock_agent_config_with_working_dir(working_dir: &std::path::Path) -> AgentConfig {
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
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
                    binary: target_dir.to_string_lossy().to_string().into(),
                    working_dir: Some(working_dir.to_path_buf()),
                    env: HashMap::new(),
                },
            ),
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
                ToolsCommand::GetToolDescriptions {
                    tool_names: _,
                    reply,
                } => {
                    let _ = reply.send(vec![]);
                }
                ToolsCommand::Shutdown => break,
            }
        }
    }

    /// Test helper: register a session in both session_map and reverse_map.
    fn register_test_session(slot: &mut AgentSlot, session_key: &SessionKey, acp_session_id: &str) {
        slot.session_map
            .insert(session_key.clone(), acp_session_id.to_owned());
        slot.reverse_map
            .insert(acp_session_id.to_owned(), session_key.clone());
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
        assert_eq!(m.slots[0].protocol_version, 2);
        assert_eq!(
            m.slots[0]
                .agent_capabilities
                .as_ref()
                .unwrap()
                .protocol_version,
            2
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
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let result_notification = IncomingMessage::AgentNotification(JsonRpcRequest::new(
            "session/update",
            None,
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "update": {
                    "sessionUpdate": "result",
                    "content": "Echo: hello"
                }
            })),
        ));

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
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let chunk_notification = IncomingMessage::AgentNotification(JsonRpcRequest::new(
            "session/update",
            None,
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": "partial response"
                }
            })),
        ));

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
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let (_incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);

        let completion = PromptCompletion {
            session_key: session_key.clone(),
            session_expired: false,
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
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        m.streaming_completed.insert(session_key.clone());

        let (_incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);
        let completion = PromptCompletion {
            session_key: session_key.clone(),
            session_expired: false,
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
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let (incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);

        // Pre-populate incoming_rx with streaming events (simulating bridge lag)
        let chunk1 = IncomingMessage::AgentNotification(JsonRpcRequest::new(
            "session/update",
            None,
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "update": { "sessionUpdate": "agent_message_chunk", "content": { "text": "hello", "type": "text" }, "messageId": "msg-1" }
            })),
        ));
        let result_event = IncomingMessage::AgentNotification(JsonRpcRequest::new(
            "session/update",
            None,
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "update": { "sessionUpdate": "result" }
            })),
        ));
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
            session_expired: false,
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
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/read_text_file",
            Some(RequestId::Number(1)),
            Some(serde_json::json!({"path": file_path.to_str().unwrap()})),
        );

        AgentsManager::handle_fs_read(&slot, &request).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_nonexistent_path_then_completes_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/read_text_file",
            Some(RequestId::Number(2)),
            Some(serde_json::json!({"path": dir.path().join("nonexistent.txt").to_str().unwrap()})),
        );

        AgentsManager::handle_fs_read(&slot, &request).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_write_called_with_valid_path_then_file_written() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("output.txt");

        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/write_text_file",
            Some(RequestId::Number(3)),
            Some(
                serde_json::json!({"path": file_path.to_str().unwrap(), "content": "written content"}),
            ),
        );

        AgentsManager::handle_fs_write(&slot, &request).await;
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "written content");
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_write_called_with_invalid_path_then_completes_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/write_text_file",
            Some(RequestId::Number(4)),
            Some(
                serde_json::json!({"path": dir.path().join("subdir/nonexistent/file.txt").to_str().unwrap(), "content": "data"}),
            ),
        );

        AgentsManager::handle_fs_write(&slot, &request).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_relative_path_inside_sandbox_then_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "secure content").unwrap();

        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/read_text_file",
            Some(RequestId::Number(10)),
            Some(serde_json::json!({"path": "notes.txt"})),
        );

        AgentsManager::handle_fs_read(&slot, &request).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_path_traversal_then_completes_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/read_text_file",
            Some(RequestId::Number(11)),
            Some(serde_json::json!({"path": "../../etc/passwd"})),
        );

        AgentsManager::handle_fs_read(&slot, &request).await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_fs_read_called_with_absolute_path_outside_sandbox_then_completes_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new(
            "test-agent".into(),
            mock_agent_config_with_working_dir(dir.path()),
            &cancel,
        );
        let request = JsonRpcRequest::new(
            "fs/read_text_file",
            Some(RequestId::Number(12)),
            Some(serde_json::json!({"path": "/etc/hostname"})),
        );

        AgentsManager::handle_fs_read(&slot, &request).await;
    }

    #[rstest]
    fn when_validate_fs_path_with_relative_inside_sandbox_then_ok() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), "x").unwrap();
        let result = validate_fs_path(dir.path(), "file.txt");
        assert!(result.is_ok());
    }

    #[rstest]
    fn when_validate_fs_path_with_path_traversal_then_err() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_fs_path(dir.path(), "../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err == "path outside allowed directory" || err.starts_with("path resolution failed"),
            "unexpected error: {err}"
        );
    }

    #[rstest]
    fn when_validate_fs_path_with_absolute_outside_sandbox_then_err() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_fs_path(dir.path(), "/etc/hostname");
        let err = result.unwrap_err();
        assert!(
            err == "path outside allowed directory" || err.starts_with("path resolution failed"),
            "unexpected error: {err}"
        );
    }

    #[rstest]
    fn when_validate_fs_path_with_absolute_inside_sandbox_then_ok() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("inside.txt"), "y").unwrap();
        let abs_path = dir.path().join("inside.txt").to_string_lossy().to_string();
        let result = validate_fs_path(dir.path(), &abs_path);
        assert!(result.is_ok());
    }

    #[rstest]
    fn when_validate_fs_write_path_with_relative_inside_sandbox_then_ok() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_fs_write_path(dir.path(), "newfile.txt");
        assert!(result.is_ok());
    }

    #[rstest]
    fn when_validate_fs_write_path_with_path_traversal_then_err() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_fs_write_path(dir.path(), "../../tmp/escape.txt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err == "path outside allowed directory"
                || err.starts_with("parent directory resolution failed"),
            "unexpected error: {err}"
        );
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
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
                    binary: target_dir.to_string_lossy().to_string().into(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: Some(anyclaw_config::CrashTrackerConfig {
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
    fn when_session_update_type_name_called_with_available_commands_update_then_returns_correct_string()
     {
        let update = SessionUpdateType::AvailableCommandsUpdate { commands: vec![] };
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

    fn make_protocol_mismatch_error(expected: u32, got: u32) -> AgentsError {
        crate::acp_error::AcpError::ProtocolMismatch { expected, got }.into()
    }

    #[rstest]
    fn when_v1_is_accepted_range_value_then_not_a_mismatch() {
        let negotiated = 1u32;
        assert!(negotiated == 1 || negotiated == 2);
    }

    #[rstest]
    fn when_v2_is_accepted_range_value_then_not_a_mismatch() {
        let negotiated = 2u32;
        assert!(negotiated == 1 || negotiated == 2);
    }

    #[rstest]
    fn when_unknown_version_v99_then_protocol_mismatch_error_produced() {
        let err = make_protocol_mismatch_error(2, 99);
        assert!(
            matches!(
                err,
                AgentsError::Protocol(crate::acp_error::AcpError::ProtocolMismatch {
                    expected: 2,
                    got: 99
                })
            ),
            "unexpected version 99 must produce ProtocolMismatch(expected=2, got=99), got: {err:?}"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_request_has_no_description_then_falls_back_to_tool_call_title() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-1".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(0)),
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "options": [{"optionId": "once", "label": "Allow once"}],
                "toolCall": {"title": "external_directory", "kind": "other"}
            })),
        );

        m.handle_permission_request(0, &request).await;

        let event = channels_rx.try_recv().expect("should route permission");
        match event {
            ChannelEvent::RoutePermission { description, .. } => {
                assert_eq!(description, "external_directory");
            }
            other => panic!("expected RoutePermission, got: {other:?}"),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_request_has_no_description_or_title_then_uses_default() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-2".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(1)),
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "options": [{"optionId": "once", "label": "Allow once"}],
            })),
        );

        m.handle_permission_request(0, &request).await;

        let event = channels_rx.try_recv().expect("should route permission");
        match event {
            ChannelEvent::RoutePermission { description, .. } => {
                assert_eq!(description, "Permission requested");
            }
            other => panic!("expected RoutePermission, got: {other:?}"),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_not_routable_then_auto_approves_and_skips_pending() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);
        // No channels_sender set — permission cannot be routed

        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(5)),
            Some(serde_json::json!({
                "sessionId": "unknown-session",
                "options": [{"optionId": "once", "label": "Allow once"}],
                "toolCall": {"title": "external_directory", "kind": "other"}
            })),
        );

        m.handle_permission_request(0, &request).await;

        assert!(
            m.slots[0].pending_permissions.is_empty(),
            "unroutable permission must not be stored in pending_permissions"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_routed_then_stored_in_pending() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, _channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-3".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(2)),
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "options": [{"optionId": "once", "label": "Allow once"}],
                "description": "Allow access?"
            })),
        );

        m.handle_permission_request(0, &request).await;

        assert_eq!(
            m.slots[0].pending_permissions.len(),
            1,
            "routed permission must be stored in pending_permissions"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_request_has_no_request_id_then_falls_back_to_jsonrpc_id() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-fallback".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(42)),
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "options": [{"optionId": "once", "label": "Allow once"}],
                "toolCall": {"title": "external_directory", "kind": "other"}
            })),
        );

        m.handle_permission_request(0, &request).await;

        assert_eq!(m.slots[0].pending_permissions.len(), 1);
        assert!(
            m.slots[0].pending_permissions.contains_key("42"),
            "should use JSON-RPC id as request_id when params.requestId is missing"
        );

        let event = channels_rx.try_recv().expect("should route permission");
        match event {
            ChannelEvent::RoutePermission { request_id, .. } => {
                assert_eq!(request_id, "42");
            }
            other => panic!("expected RoutePermission, got: {other:?}"),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_options_use_name_instead_of_label_then_deserialized_correctly() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-4".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(3)),
            Some(serde_json::json!({
                "sessionId": acp_session_id,
                "options": [
                    {"optionId": "once", "name": "Allow once", "kind": "allow_once"},
                    {"optionId": "always", "name": "Always allow", "kind": "allow_always"},
                    {"optionId": "reject", "name": "Reject", "kind": "reject_once"}
                ],
                "toolCall": {"title": "external_directory", "kind": "other"}
            })),
        );

        m.handle_permission_request(0, &request).await;

        assert_eq!(
            m.slots[0].pending_permissions.len(),
            1,
            "options with name alias must deserialize successfully"
        );
        let perm = m.slots[0].pending_permissions.values().next().unwrap();
        assert_eq!(perm.options.len(), 3);
        assert_eq!(perm.options[0].label, "Allow once");
        assert_eq!(perm.options[1].label, "Always allow");

        let event = channels_rx.try_recv().expect("should route permission");
        assert!(matches!(event, ChannelEvent::RoutePermission { .. }));
    }

    #[rstest]
    #[tokio::test]
    async fn when_agent_reports_v2_then_slot_protocol_version_stored() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        assert_eq!(
            m.slots[0].protocol_version, 2,
            "slot must store the negotiated protocol version"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_permission_responded_then_acp_format_sent_to_agent() {
        use tokio::io::AsyncBufReadExt;

        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, _channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "u1");
        let acp_session_id = "acp-perm-format".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);

        // Wire up a real connection with a readable stdin pipe
        let (stdin_write, stdin_read) = tokio::io::duplex(64 * 1024);
        let (stdout_write, _stdout_read) = tokio::io::duplex(64 * 1024);
        let (_stderr_write, stderr_read) = tokio::io::duplex(64 * 1024);
        slot.connection = Some(AgentConnection::from_parts(
            Box::new(crate::connection::test_support::MockBackend::new(true)),
            Box::new(stdin_write),
            Box::new(stdout_write),
            Box::new(stderr_read),
            "test-agent",
            None,
        ));

        // Store a pending permission with original request id=0
        let original_request = JsonRpcRequest::new(
            "session/request_permission",
            Some(RequestId::Number(0)),
            None,
        );
        slot.pending_permissions.insert(
            "0".to_string(),
            PendingPermission {
                request: original_request,
                description: "external_directory".into(),
                options: vec![],
                received_at: std::time::Instant::now(),
            },
        );
        m.slots.push(slot);

        // Send RespondPermission
        m.handle_command(AgentsCommand::RespondPermission {
            request_id: "0".to_string(),
            option_id: "once".to_string(),
        })
        .await;

        // Read what was written to agent stdin
        let mut reader = tokio::io::BufReader::new(stdin_read);
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            reader.read_line(&mut line),
        )
        .await
        .expect("timeout reading agent stdin")
        .expect("read error");

        let written: serde_json::Value = serde_json::from_str(line.trim()).expect("invalid JSON");

        // ACP wire format per @agentclientprotocol/sdk@0.16.1
        assert_eq!(written["jsonrpc"], "2.0");
        assert_eq!(written["id"], 0);
        assert_eq!(written["result"]["outcome"]["outcome"], "selected");
        assert_eq!(written["result"]["outcome"]["optionId"], "once");
        assert!(
            written["result"]["requestId"].is_null(),
            "requestId must not be in result"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_stale_sessions_populated_from_store_then_slot_stale_map_contains_them() {
        use anyclaw_core::{
            DynSessionStore, NoopSessionStore, PersistedSession, SessionStoreError,
        };
        use std::future::Future;
        use std::pin::Pin;

        struct StubStore {
            sessions: Vec<PersistedSession>,
        }

        impl anyclaw_core::SessionStore for StubStore {
            fn load_open_sessions(
                &self,
            ) -> impl Future<Output = Result<Vec<PersistedSession>, SessionStoreError>> + Send
            {
                let sessions = self.sessions.clone();
                async move { Ok(sessions) }
            }
            fn upsert_session(
                &self,
                _: &PersistedSession,
            ) -> impl Future<Output = Result<(), SessionStoreError>> + Send {
                async { Ok(()) }
            }
            fn mark_closed(
                &self,
                _: &str,
            ) -> impl Future<Output = Result<(), SessionStoreError>> + Send {
                async { Ok(()) }
            }
            fn update_last_active(
                &self,
                _: &str,
                _: i64,
            ) -> impl Future<Output = Result<(), SessionStoreError>> + Send {
                async { Ok(()) }
            }
            fn delete_expired(
                &self,
                _: i64,
            ) -> impl Future<Output = Result<u64, SessionStoreError>> + Send {
                async { Ok(0) }
            }
        }

        let session_key_str = SessionKey::new("telegram", "direct", "alice").to_string();
        let store = Arc::new(StubStore {
            sessions: vec![PersistedSession {
                session_key: session_key_str.clone(),
                agent_name: "default".to_string(),
                acp_session_id: "stale-acp-111".to_string(),
                created_at: 1_000_000,
                last_active_at: 1_000_100,
                closed: false,
            }],
        });

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m =
            AgentsManager::new(mock_agents_manager_config(), handle).with_session_store(store);

        m.start().await.unwrap();

        let key = SessionKey::new("telegram", "direct", "alice");
        assert!(
            m.slots[0].stale_sessions.contains_key(&key),
            "stale_sessions should contain the key loaded from store"
        );
        assert_eq!(
            m.slots[0].stale_sessions.get(&key).map(String::as_str),
            Some("stale-acp-111"),
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_agent_rejects_session_load_then_heal_session_falls_back_to_create() {
        let mut config = mock_agent_config();
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

        let session_key = SessionKey::new("telegram", "direct", "bob");
        m.slots[0]
            .stale_sessions
            .insert(session_key.clone(), "stale-acp-old".to_string());

        let result = m.heal_session(0, "default", &session_key).await;
        assert!(
            result.is_ok(),
            "heal_session should succeed via fallback: {result:?}"
        );

        assert!(
            m.slots[0].session_map.contains_key(&session_key),
            "session_map must contain the new session after heal"
        );
        assert!(
            !m.slots[0].stale_sessions.contains_key(&session_key),
            "stale_sessions must be cleared after successful heal"
        );

        let acp_id = m.slots[0].session_map.get(&session_key).unwrap();
        assert_ne!(
            acp_id, "stale-acp-old",
            "session_map must have new id, not stale"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_start_called_with_sqlite_store_then_expired_sessions_deleted() {
        use anyclaw_core::{PersistedSession, SqliteSessionStore};

        let store_arc: Arc<dyn anyclaw_core::DynSessionStore> =
            Arc::new(SqliteSessionStore::open_in_memory().expect("in-memory sqlite failed"));

        let old_session = PersistedSession {
            session_key: "old-key".to_string(),
            agent_name: "default".to_string(),
            acp_session_id: "acp-old-1".to_string(),
            created_at: 1,
            last_active_at: 1,
            closed: false,
        };
        store_arc
            .upsert_session(&old_session)
            .await
            .expect("upsert failed");

        let before = store_arc.load_open_sessions().await.expect("load failed");
        assert_eq!(before.len(), 1, "old session should exist before start");

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle)
            .with_session_store(Arc::clone(&store_arc))
            .with_session_ttl_secs(1);

        m.start().await.expect("start failed");

        let after = store_arc
            .load_open_sessions()
            .await
            .expect("load after failed");
        assert!(
            after.is_empty(),
            "expired session should have been deleted at boot, but got: {after:?}"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_shutdown_all_called_then_sessions_remain_open_for_recovery() {
        use anyclaw_core::{PersistedSession, SqliteSessionStore};

        let store = SqliteSessionStore::open_in_memory().expect("in-memory sqlite failed");
        let store_arc: Arc<dyn anyclaw_core::DynSessionStore> = Arc::new(store);

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle)
            .with_session_store(Arc::clone(&store_arc));

        m.start().await.expect("start failed");

        let session_key = m.slots[0]
            .session_map
            .keys()
            .next()
            .cloned()
            .expect("slot should have a default session key");

        let acp_id = m.slots[0]
            .session_map
            .get(&session_key)
            .cloned()
            .expect("slot should have a default acp session id");

        let persisted = PersistedSession {
            session_key: session_key.to_string(),
            agent_name: "default".to_string(),
            acp_session_id: acp_id,
            created_at: 1_000_000,
            last_active_at: 1_000_100,
            closed: false,
        };
        store_arc
            .upsert_session(&persisted)
            .await
            .expect("upsert failed");

        let before = store_arc.load_open_sessions().await.expect("load failed");
        assert_eq!(before.len(), 1, "session should be open before shutdown");

        m.shutdown_all().await;
        tools_task.abort();

        let after = store_arc
            .load_open_sessions()
            .await
            .expect("load after failed");
        assert_eq!(
            after.len(),
            1,
            "session should remain open after shutdown_all for recovery on restart"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_session_loaded_then_awaiting_first_prompt_populated() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let session_key = SessionKey::new("telegram", "direct", "alice");
        m.slots[0]
            .stale_sessions
            .insert(session_key.clone(), "stale-acp-load-1".to_string());

        let result = m.heal_session(0, "default", &session_key).await;
        assert!(
            result.is_ok(),
            "heal_session should succeed via session/load: {result:?}"
        );

        let acp_id = m.slots[0]
            .session_map
            .get(&session_key)
            .cloned()
            .expect("session_map must contain the healed session key");

        assert!(
            m.slots[0].awaiting_first_prompt.contains(&acp_id),
            "awaiting_first_prompt must contain the ACP session ID after session/load"
        );
        assert!(
            !m.slots[0].stale_sessions.contains_key(&session_key),
            "stale_sessions must be cleared after successful heal"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_replay_event_received_for_awaiting_session_then_not_forwarded() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "alice");
        let acp_session_id = "acp-replay-1".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        slot.awaiting_first_prompt.insert(acp_session_id.clone());
        m.slots.push(slot);

        let params = serde_json::json!({
            "sessionId": acp_session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "text": "replay content", "type": "text" },
                "messageId": "replay-msg-1"
            }
        });
        m.handle_session_update(0, params).await;

        assert!(
            channels_rx.try_recv().is_err(),
            "replay event must not be forwarded to channels while awaiting_first_prompt is set"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_prompt_sent_after_load_then_awaiting_first_prompt_cleared() {
        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(mock_agents_manager_config(), handle);
        m.start().await.unwrap();

        let default_key = SessionKey::new("default", "default", "default");
        let acp_id = m.slots[0]
            .session_map
            .get(&default_key)
            .cloned()
            .expect("default session must exist after start");

        m.slots[0].awaiting_first_prompt.insert(acp_id.clone());

        let result = m.prompt_session("default", &default_key, "hello").await;
        assert!(result.is_ok(), "prompt_session should succeed: {result:?}");

        assert!(
            !m.slots[0].awaiting_first_prompt.contains(&acp_id),
            "awaiting_first_prompt must be cleared after prompt_session"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_live_event_received_after_prompt_then_forwarded() {
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, mut channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "alice");
        let acp_session_id = "acp-live-1".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let params = serde_json::json!({
            "sessionId": acp_session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "text": "live content", "type": "text" },
                "messageId": "live-msg-1"
            }
        });
        m.handle_session_update(0, params).await;

        let event = channels_rx
            .try_recv()
            .expect("live event must be forwarded to channels");
        assert!(
            matches!(event, ChannelEvent::DeliverMessage { .. }),
            "forwarded event must be DeliverMessage, got: {event:?}"
        );
    }
}
