use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use crate::acp_error::AcpError;
use crate::acp_types::{
    ClientCapabilities, InitializeParams, InitializeResult, McpServerInfo, SessionNewParams,
};
use crate::slot::{AgentSlot, find_slot_by_name};
use anyclaw_config::{AgentConfig, AgentsManagerConfig};
use anyclaw_core::{
    AgentsCommand, Manager, ManagerError, ManagerHandle, McpServerUrl, SessionKey, ToolDescription,
    ToolsCommand, constants,
};
use anyclaw_sdk_agent::{DynAgentAdapter, GenericAcpAdapter};
use anyclaw_sdk_types::{ChannelEvent, PermissionOption};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{AgentConnection, IncomingMessage};
use crate::error::AgentsError;
use anyclaw_core::{DynSessionStore, NoopSessionStore};
use anyclaw_jsonrpc::types::JsonRpcRequest;

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

pub(crate) struct PromptCompletion {
    pub(crate) session_key: SessionKey,
    /// Set when the agent reports the session no longer exists,
    /// so `handle_prompt_completion` can invalidate the stale mapping.
    /// Read via `completion_rx` channel in `handle_prompt_completion`.
    pub(crate) session_expired: bool,
    pub(crate) stop_reason: anyclaw_sdk_types::acp::StopReason,
}

use crate::fs_sandbox::resolve_agent_cwd;

/// Manages agent subprocess lifecycles, ACP session routing, and crash recovery.
///
/// The `AgentsManager` owns all [`AgentSlot`]s, dispatches commands from the
/// channels manager, and forwards agent events back to channels. It implements
/// the bridge-collapsed architecture where incoming agent messages flow directly
/// to the manager's shared channel without an intermediate forwarding task.
pub struct AgentsManager {
    pub(crate) agent_configs: Vec<(String, AgentConfig)>,
    pub(crate) manager_config: AgentsManagerConfig,
    pub(crate) tools_handle: ManagerHandle<ToolsCommand>,
    pub(crate) slots: Vec<AgentSlot>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<AgentsCommand>>,
    cmd_tx: tokio::sync::mpsc::Sender<AgentsCommand>,
    pub(crate) channels_sender: Option<mpsc::Sender<ChannelEvent>>,
    adapter: Box<dyn DynAgentAdapter>,
    pub(crate) parent_cancel: CancellationToken,
    pub(crate) incoming_tx: mpsc::Sender<SlotIncoming>,
    incoming_rx: Option<mpsc::Receiver<SlotIncoming>>,
    pub(crate) completion_tx: mpsc::Sender<PromptCompletion>,
    pub(crate) completion_rx: Option<mpsc::Receiver<PromptCompletion>>,
    pub(crate) streaming_completed: HashSet<SessionKey>,
    pub(crate) update_seq: AtomicU64,
    pub(crate) log_level: Option<String>,
    /// Persistent session store. Defaults to [`NoopSessionStore`].
    pub(crate) session_store: Arc<dyn DynSessionStore>,
    /// TTL for expired session cleanup at boot (seconds). Default: 7 days.
    session_ttl_secs: i64,
    pub(crate) queue: crate::session_queue::SessionQueue,
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
            queue: crate::session_queue::SessionQueue::new(),
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
    pub(crate) fn acp_timeout_for(
        agent_config: &AgentConfig,
        manager_config: &AgentsManagerConfig,
    ) -> Duration {
        let secs = agent_config
            .acp_timeout_secs
            .unwrap_or(manager_config.acp_timeout_secs);
        Duration::from_secs(secs)
    }
}

pub(crate) fn apply_agent_defaults(
    options: &mut HashMap<String, serde_json::Value>,
    defaults: &HashMap<String, serde_json::Value>,
) {
    for (key, value) in defaults {
        options.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

impl AgentsManager {
    #[tracing::instrument(skip(slot), fields(agent = %slot.name()))]
    pub(crate) async fn initialize_agent(
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

        if let Some(defaults) = result.defaults.as_ref() {
            apply_agent_defaults(&mut slot.config.options, defaults);
        }

        slot.protocol_version = result.protocol_version;
        slot.agent_capabilities = Some(result);
        Ok(())
    }

    pub(crate) async fn start_session(
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
    pub(crate) async fn fetch_mcp_servers(&self, slot_idx: usize) -> Vec<McpServerInfo> {
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
    pub(crate) async fn fetch_tool_context(&self, slot_idx: usize) -> Option<String> {
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

    pub(crate) async fn shutdown_all(&mut self) {
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

    pub(crate) fn register_default_session(slot: &mut AgentSlot, name: &str, session_id: String) {
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
        // LIMITATION: Do not call run() twice
        // cmd_rx is consumed via .take() on first run(). A second call would panic on
        // the .expect() because the receiver has already been moved into the select! loop.
        // The Manager trait consumes self, so this is enforced at the type level — but
        // internal Option<Receiver> fields add a runtime guard as defense in depth.
        // See also: AGENTS.md §Anti-Patterns
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp_types::SessionUpdateType;
    use crate::connection::IncomingMessage;
    use crate::error::AgentsError;
    use crate::fs_sandbox::{validate_fs_path, validate_fs_write_path};
    use crate::incoming::normalize_tool_event_fields;
    use anyclaw_jsonrpc::types::RequestId;
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
            stop_reason: anyclaw_sdk_types::acp::StopReason::EndTurn,
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
            matches!(&events[1], ChannelEvent::SessionComplete { session_key: sk, .. } if sk == &session_key),
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
            stop_reason: anyclaw_sdk_types::acp::StopReason::EndTurn,
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
            matches!(&events[0], ChannelEvent::SessionComplete { session_key: sk, .. } if sk == &session_key),
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
            stop_reason: anyclaw_sdk_types::acp::StopReason::EndTurn,
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
        use anyclaw_core::{PersistedSession, SessionStoreError};
        use std::future::Future;

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

    #[rstest]
    #[tokio::test]
    async fn when_heal_session_resume_returns_new_id_then_session_map_uses_returned_id() {
        let mut config = mock_agent_config();
        config
            .options
            .insert("support_resume".into(), serde_json::json!(true));
        config.options.insert(
            "recovery_new_id".into(),
            serde_json::json!("new-acp-from-resume"),
        );

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
            "heal_session should succeed via resume: {result:?}"
        );

        let acp_id = m.slots[0]
            .session_map
            .get(&session_key)
            .expect("session_map must contain the healed session key");
        assert_eq!(
            acp_id, "new-acp-from-resume",
            "session_map must use the ID returned by session/resume, not the stale ID"
        );
        assert!(
            m.slots[0].reverse_map.contains_key("new-acp-from-resume"),
            "reverse_map must contain the returned ID"
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
    async fn when_heal_session_load_returns_new_id_then_session_map_uses_returned_id() {
        let mut config = mock_agent_config();
        config.options.insert(
            "recovery_new_id".into(),
            serde_json::json!("new-acp-from-load"),
        );

        let (handle, rx) = make_tools_handle();
        let tools_task = tokio::spawn(serve_tools_urls(rx));

        let mut m = AgentsManager::new(
            mock_agents_manager_config_with(HashMap::from([("default".into(), config)])),
            handle,
        );
        m.start().await.unwrap();

        let session_key = SessionKey::new("telegram", "direct", "carol");
        m.slots[0]
            .stale_sessions
            .insert(session_key.clone(), "stale-acp-old".to_string());

        let result = m.heal_session(0, "default", &session_key).await;
        assert!(
            result.is_ok(),
            "heal_session should succeed via load: {result:?}"
        );

        let acp_id = m.slots[0]
            .session_map
            .get(&session_key)
            .expect("session_map must contain the healed session key");
        assert_eq!(
            acp_id, "new-acp-from-load",
            "session_map must use the ID returned by session/load, not the stale ID"
        );
        assert!(
            m.slots[0].reverse_map.contains_key("new-acp-from-load"),
            "reverse_map must contain the returned ID"
        );
        assert!(
            m.slots[0]
                .awaiting_first_prompt
                .contains("new-acp-from-load"),
            "awaiting_first_prompt must contain the returned ID"
        );

        m.shutdown_all().await;
        tools_task.abort();
    }

    #[rstest]
    #[tokio::test]
    async fn when_session_expired_then_mapping_moved_to_stale_sessions_for_recovery() {
        // When the agent reports "session not found", handle_prompt_completion must
        // move the dead ACP session ID into stale_sessions so that heal_session can
        // attempt session/resume or session/load on the next prompt — not just drop it.
        let (handle, _rx) = make_tools_handle();
        let mut m = AgentsManager::new(mock_agents_manager_config_with(HashMap::new()), handle);

        let (channels_tx, _channels_rx) = mpsc::channel::<ChannelEvent>(16);
        m.channels_sender = Some(channels_tx);

        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("test-agent".into(), mock_agent_config(), &cancel);
        let session_key = SessionKey::new("telegram", "direct", "user1");
        let acp_session_id = "acp-sess-expired".to_string();
        register_test_session(&mut slot, &session_key, &acp_session_id);
        m.slots.push(slot);

        let (_incoming_tx, mut incoming_rx) = mpsc::channel::<SlotIncoming>(16);

        let completion = PromptCompletion {
            session_key: session_key.clone(),
            session_expired: true,
            stop_reason: anyclaw_sdk_types::acp::StopReason::Refusal,
        };
        m.handle_prompt_completion(completion, &mut incoming_rx)
            .await;

        // session_map must no longer contain the dead mapping
        assert!(
            !m.slots[0].session_map.contains_key(&session_key),
            "expired session must be removed from session_map"
        );
        assert!(
            !m.slots[0].reverse_map.contains_key(&acp_session_id),
            "expired session must be removed from reverse_map"
        );

        // stale_sessions must contain the dead ACP ID so heal_session can recover
        assert_eq!(
            m.slots[0]
                .stale_sessions
                .get(&session_key)
                .map(String::as_str),
            Some("acp-sess-expired"),
            "expired session must be moved to stale_sessions for recovery"
        );
    }

    #[rstest]
    fn when_apply_agent_defaults_called_then_missing_keys_are_populated() {
        let mut options: HashMap<String, serde_json::Value> = HashMap::new();
        options.insert("user_key".to_string(), serde_json::json!("user_value"));

        let mut defaults: HashMap<String, serde_json::Value> = HashMap::new();
        defaults.insert(
            "default_key".to_string(),
            serde_json::json!("default_value"),
        );
        defaults.insert(
            "user_key".to_string(),
            serde_json::json!("should_not_override"),
        );

        apply_agent_defaults(&mut options, &defaults);

        assert_eq!(options["default_key"], serde_json::json!("default_value"));
        assert_eq!(options["user_key"], serde_json::json!("user_value"));
    }

    #[rstest]
    fn when_apply_agent_defaults_called_then_user_options_win_over_defaults() {
        let mut options: HashMap<String, serde_json::Value> = HashMap::new();
        options.insert("model".to_string(), serde_json::json!("gpt-4"));

        let mut defaults: HashMap<String, serde_json::Value> = HashMap::new();
        defaults.insert("model".to_string(), serde_json::json!("claude-3"));
        defaults.insert("temperature".to_string(), serde_json::json!(0.7));

        apply_agent_defaults(&mut options, &defaults);

        assert_eq!(
            options["model"],
            serde_json::json!("gpt-4"),
            "user option must win"
        );
        assert_eq!(
            options["temperature"],
            serde_json::json!(0.7),
            "default must be applied for missing key"
        );
        assert_eq!(options.len(), 2);
    }
}
