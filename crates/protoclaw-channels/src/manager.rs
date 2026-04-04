use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::types::PermissionOption;
use protoclaw_agents::AgentsCommand;
use protoclaw_config::{ChannelConfig, DebounceConfig};
use protoclaw_core::types::ChannelId;
use protoclaw_core::{constants, ChannelEvent, CrashTracker, ExponentialBackoff, Manager, ManagerError, ManagerHandle, SessionKey};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{ChannelConnection, IncomingChannelMessage};
use crate::debounce::{DebounceAction, DebounceBuffer};
use crate::error::ChannelsError;
use crate::types::{ChannelCapabilities, ChannelInitializeResult, ChannelSendMessage, ChannelRespondPermission};

/// Commands sent to the ChannelsManager from other managers.
pub enum ChannelsCommand {
    /// Deliver agent message to a specific channel (by session key).
    DeliverToChannel {
        session_key: String,
        content: serde_json::Value,
    },
    /// Route permission request to originating channel.
    RoutePermission {
        session_key: String,
        request_id: String,
        description: String,
        options: Vec<PermissionOption>,
    },
    Shutdown,
}

/// Routing table entry mapping a session key to its channel and ACP session.
struct RoutingEntry {
    channel_id: ChannelId,
    acp_session_id: String,
    slot_index: usize,
    agent_name: String,
}

/// Per-channel state: connection, config, crash recovery.
struct ChannelSlot {
    name: String,
    config: ChannelConfig,
    connection: Option<ChannelConnection>,
    channel_id: ChannelId,
    cancel_token: CancellationToken,
    backoff: ExponentialBackoff,
    crash_tracker: CrashTracker,
    disabled: bool,
}

/// Manages channel subprocesses with crash isolation and session-keyed routing.
///
/// Each channel runs as a subprocess communicating over JSON-RPC stdio.
/// A crash in one channel does not affect other channels or the sidecar.
/// Inbound messages create/reuse ACP sessions via AgentsManager.
/// Outbound agent updates route back to the originating channel via routing table.
pub struct ChannelsManager {
    channel_configs: HashMap<String, ChannelConfig>,
    default_agent_name: String,
    init_timeout_secs: u64,
    slots: Vec<ChannelSlot>,
    cmd_rx: Option<mpsc::Receiver<ChannelsCommand>>,
    cmd_tx: mpsc::Sender<ChannelsCommand>,
    routing_table: HashMap<SessionKey, RoutingEntry>,
    agents_handle: Option<ManagerHandle<AgentsCommand>>,
    channel_events_rx: Option<mpsc::Receiver<ChannelEvent>>,
    debounce: DebounceBuffer,
    acked_sessions: HashSet<SessionKey>,
}

impl ChannelsManager {
    pub fn new(channel_configs: HashMap<String, ChannelConfig>, debounce_config: DebounceConfig, init_timeout_secs: u64, default_agent_name: String) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(constants::CMD_CHANNEL_CAPACITY);
        Self {
            channel_configs,
            default_agent_name,
            init_timeout_secs,
            slots: Vec::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
            routing_table: HashMap::new(),
            agents_handle: None,
            channel_events_rx: None,
            debounce: DebounceBuffer::new(debounce_config),
            acked_sessions: HashSet::new(),
        }
    }

    pub fn command_sender(&self) -> mpsc::Sender<ChannelsCommand> {
        self.cmd_tx.clone()
    }

    /// Set the agents handle for creating/prompting sessions.
    pub fn with_agents_handle(mut self, handle: ManagerHandle<AgentsCommand>) -> Self {
        self.agents_handle = Some(handle);
        self
    }

    /// Set the channel events receiver for agent updates routed back.
    pub fn with_channel_events_rx(mut self, rx: mpsc::Receiver<ChannelEvent>) -> Self {
        self.channel_events_rx = Some(rx);
        self
    }

    fn agent_name_for_channel(&self, slot_index: usize) -> &str {
        &self.slots[slot_index].config.agent
    }

    /// Get a watch receiver for a named channel's discovered port (from stderr PORT:{port}).
    /// Returns None if the channel doesn't exist or has no active connection.
    pub fn channel_port(&self, name: &str) -> Option<tokio::sync::watch::Receiver<u16>> {
        self.slots
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.connection.as_ref())
            .map(|conn| conn.port_rx())
    }

    /// Spawn and initialize a single channel subprocess.
    fn resolve_init_timeout(&self, channel_config: &ChannelConfig) -> Duration {
        let secs = channel_config.init_timeout_secs.unwrap_or(self.init_timeout_secs);
        Duration::from_secs(secs)
    }

    async fn spawn_and_initialize(
        config: &ChannelConfig,
        channel_id: &ChannelId,
        init_timeout: Duration,
    ) -> Result<(ChannelConnection, ChannelCapabilities), ChannelsError> {
        let mut conn = ChannelConnection::spawn(config, channel_id.clone())?;

        let ack_json = serde_json::json!({
            "reaction": config.ack.reaction,
            "typing": config.ack.typing,
            "reactionEmoji": config.ack.reaction_emoji,
            "reactionLifecycle": config.ack.reaction_lifecycle,
        });
        let params = serde_json::json!({
            "protocolVersion": 1,
            "channelId": channel_id.as_ref(),
            "ack": ack_json,
        });

        let rx = conn.send_request("initialize", params).await?;
        let resp = tokio::time::timeout(init_timeout, rx)
            .await
            .map_err(|_| ChannelsError::Timeout(init_timeout))?
            .map_err(|_| ChannelsError::ConnectionClosed)?;

        let result: ChannelInitializeResult = serde_json::from_value(resp)?;
        let caps = result.capabilities;
        conn.set_capabilities(caps.clone());

        Ok((conn, caps))
    }

    /// Handle a crashed channel: backoff, respawn, re-initialize.
    async fn handle_channel_crash(&mut self, slot_index: usize) {
        let slot = &mut self.slots[slot_index];
        let channel_name = &slot.name;

        slot.crash_tracker.record_crash();

        if slot.crash_tracker.is_crash_loop() {
            tracing::error!(
                channel = %channel_name,
                "channel in crash loop, disabling"
            );
            slot.connection = None;
            slot.disabled = true;
            return;
        }

        let delay = slot.backoff.next_delay();
        tracing::warn!(
            channel = %channel_name,
            delay_ms = delay.as_millis(),
            "channel crashed, respawning after backoff"
        );
        tokio::time::sleep(delay).await;

        match Self::spawn_and_initialize(&slot.config, &slot.channel_id, Duration::from_secs(self.init_timeout_secs)).await {
            Ok((conn, caps)) => {
                tracing::info!(
                    channel = %channel_name,
                    streaming = caps.streaming,
                    rich_text = caps.rich_text,
                    "channel recovered"
                );
                slot.connection = Some(conn);
                slot.backoff.reset();
            }
            Err(e) => {
                tracing::error!(
                    channel = %channel_name,
                    error = %e,
                    "failed to respawn channel"
                );
                slot.connection = None;
            }
        }
    }

    /// Handle a ChannelsCommand.
    async fn handle_command(&mut self, cmd: ChannelsCommand) -> bool {
        match cmd {
            ChannelsCommand::DeliverToChannel {
                session_key,
                content,
            } => {
                let sk: SessionKey = session_key.as_str().into();
                if let Some(entry) = self.routing_table.get(&sk) {
                    let slot = &self.slots[entry.slot_index];
                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "sessionId": entry.acp_session_id,
                            "content": content,
                        });
                        if let Err(e) = conn.send_notification("channel/deliverMessage", params).await {
                            tracing::warn!(
                                channel = %slot.name,
                                error = %e,
                                "failed to deliver message"
                            );
                        }
                    }
                } else {
                    tracing::warn!(session_key = %session_key, "no routing entry for session key");
                }
            }
            ChannelsCommand::RoutePermission {
                session_key,
                request_id,
                description,
                options,
            } => {
                let sk: SessionKey = session_key.as_str().into();
                if let Some(entry) = self.routing_table.get(&sk) {
                    let slot = &self.slots[entry.slot_index];
                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "requestId": request_id,
                            "sessionId": entry.acp_session_id,
                            "description": description,
                            "options": options,
                        });
                        if let Err(e) = conn.send_notification("channel/requestPermission", params).await {
                            tracing::warn!(
                                channel = %slot.name,
                                error = %e,
                                "failed to route permission"
                            );
                        }
                    }
                } else {
                    tracing::warn!(session_key = %session_key, request_id = %request_id, "no routing entry for permission");
                }
            }
            ChannelsCommand::Shutdown => {
                self.shutdown_all().await;
                return true;
            }
        }
        false
    }

    /// Check if a DeliverMessage content payload is a "result" update.
    /// Content is the raw session/update params: `{"sessionId": "...", "type": "result", ...}`.
    fn is_result_content(content: &serde_json::Value) -> bool {
        content.get("type")
            .and_then(|t| t.as_str())
            .map(|t| t == "result")
            .unwrap_or(false)
    }

    /// Handle a ChannelEvent from AgentsManager (outbound: agent → channel).
    async fn handle_channel_event(&mut self, event: ChannelEvent) {
        match event {
            ChannelEvent::DeliverMessage { session_key, content } => {
                let is_result = Self::is_result_content(&content);

                if is_result {
                    self.debounce.mark_session_idle(&session_key);
                    if let Some(queued_msg) = self.debounce.drain_queued(&session_key) {
                        self.send_ack_to_channel(&session_key).await;
                        let agent_name = self.routing_table
                            .get(&session_key)
                            .map(|e| e.agent_name.clone())
                            .unwrap_or_default();
                        self.debounce.mark_session_active(&session_key);
                        self.dispatch_to_agent(&session_key, &queued_msg, &agent_name).await;
                    }
                }

                if let Some(entry) = self.routing_table.get(&session_key) {
                    let slot = &self.slots[entry.slot_index];

                    // Send ack lifecycle on first response for this session
                    if !self.acked_sessions.contains(&session_key) {
                        self.acked_sessions.insert(session_key.clone());
                        if let Some(conn) = &slot.connection {
                            let lifecycle_params = serde_json::json!({
                                "sessionId": entry.acp_session_id,
                                "action": "response_started",
                            });
                            let _ = conn.send_notification("channel/ackLifecycle", lifecycle_params).await;
                        }
                    }

                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "sessionId": entry.acp_session_id,
                            "content": content,
                        });
                        if let Err(e) = conn.send_notification("channel/deliverMessage", params).await {
                            tracing::warn!(
                                channel = %slot.name,
                                session_key = %session_key,
                                error = %e,
                                "failed to deliver agent update"
                            );
                        }
                    }
                } else {
                    tracing::warn!(session_key = %session_key, "no routing entry for agent update");
                }
            }
            ChannelEvent::RoutePermission { session_key, request_id, description, options } => {
                if let Some(entry) = self.routing_table.get(&session_key) {
                    let slot = &self.slots[entry.slot_index];
                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "requestId": request_id,
                            "sessionId": entry.acp_session_id,
                            "description": description,
                            "options": options,
                        });
                        if let Err(e) = conn.send_notification("channel/requestPermission", params).await {
                            tracing::warn!(
                                channel = %slot.name,
                                session_key = %session_key,
                                error = %e,
                                "failed to route permission from agent"
                            );
                        }
                    }
                } else {
                    tracing::warn!(session_key = %session_key, request_id = %request_id, "no routing entry for permission");
                }
            }
            ChannelEvent::AckMessage { session_key, channel_name, peer_id, message_id } => {
                tracing::debug!(session_key = %session_key, channel = %channel_name, peer = %peer_id, message_id = ?message_id, "ack message event received");
            }
        }
    }

    /// Send ack notification to the channel for a session (at dispatch time, not inbound time).
    async fn send_ack_to_channel(&self, session_key: &SessionKey) {
        let entry = match self.routing_table.get(session_key) {
            Some(e) => e,
            None => return,
        };
        let slot = &self.slots[entry.slot_index];
        if let Some(conn) = &slot.connection {
            // Parse peer_id from session key format "channel:kind:peer_id"
            let sk_str = session_key.as_ref();
            let peer_id = sk_str.splitn(3, ':').nth(2).unwrap_or("");
            let ack_params = serde_json::json!({
                "sessionId": entry.acp_session_id,
                "channelName": slot.name,
                "peerId": peer_id,
                "messageId": serde_json::Value::Null,
            });
            if let Err(e) = conn.send_notification("channel/ackMessage", ack_params).await {
                tracing::warn!(
                    channel = %slot.name,
                    error = %e,
                    "failed to send ack notification"
                );
            }
        }
    }

    /// Dispatch a merged/immediate message to the agent as a PromptSession.
    async fn dispatch_to_agent(&mut self, session_key: &SessionKey, message: &str, agent_name: &str) {
        // Reset ack lifecycle tracking so next response triggers lifecycle again
        self.acked_sessions.remove(session_key);

        let agents_handle = match &self.agents_handle {
            Some(h) => h.clone(),
            None => {
                tracing::warn!(session_key = %session_key, "no agents handle, cannot dispatch");
                return;
            }
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(e) = agents_handle.send(AgentsCommand::PromptSession {
            agent_name: agent_name.to_string(),
            session_key: session_key.clone(),
            message: message.to_string(),
            reply: reply_tx,
        }).await {
            tracing::error!(session_key = %session_key, error = %e, "failed to send PromptSession");
            return;
        }

        match reply_rx.await {
            Ok(Ok(())) => {
                tracing::debug!(session_key = %session_key, "prompt sent to agent");
            }
            Ok(Err(e)) => {
                tracing::warn!(session_key = %session_key, error = %e, "PromptSession failed");
            }
            Err(_) => {
                tracing::warn!(session_key = %session_key, "PromptSession reply dropped");
            }
        }
    }

    /// Handle an incoming message from a channel subprocess (inbound: channel → agent).
    async fn handle_channel_message(&mut self, slot_index: usize, msg: IncomingChannelMessage) {
        let channel_name = self.slots[slot_index].name.clone();
        let channel_id = self.slots[slot_index].channel_id.clone();

        let value = match &msg {
            IncomingChannelMessage::ChannelRequest(v) | IncomingChannelMessage::ChannelNotification(v) => v.clone(),
        };

        let method = value["method"].as_str().unwrap_or("");
        let params = value.get("params").cloned().unwrap_or(serde_json::Value::Null);

        match method {
            "channel/sendMessage" => {
                if let Ok(send_msg) = serde_json::from_value::<ChannelSendMessage>(params) {
                    let session_key = SessionKey::new(
                        &send_msg.peer_info.channel_name,
                        &send_msg.peer_info.kind,
                        &send_msg.peer_info.peer_id,
                    );

                    let agents_handle = match &self.agents_handle {
                        Some(h) => h.clone(),
                        None => {
                            tracing::warn!(channel = %channel_name, "no agents handle, cannot route message");
                            return;
                        }
                    };

                    let agent_name = self.agent_name_for_channel(slot_index).to_string();

                    if !self.routing_table.contains_key(&session_key) {
                        let (reply_tx, reply_rx) = oneshot::channel();
                        if let Err(e) = agents_handle.send(AgentsCommand::CreateSession {
                            agent_name: agent_name.clone(),
                            session_key: session_key.clone(),
                            reply: reply_tx,
                        }).await {
                            tracing::error!(channel = %channel_name, error = %e, "failed to send CreateSession");
                            return;
                        }

                         match reply_rx.await {
                            Ok(Ok(acp_session_id)) => {
                                tracing::info!(
                                    channel = %channel_name,
                                    session_key = %session_key,
                                    acp_session_id = %acp_session_id,
                                    "session created"
                                );
                                self.routing_table.insert(session_key.clone(), RoutingEntry {
                                    channel_id: channel_id.clone(),
                                    acp_session_id: acp_session_id.clone(),
                                    slot_index,
                                    agent_name: agent_name.clone(),
                                });

                                if let Some(conn) = &self.slots[slot_index].connection {
                                    let params = serde_json::json!({
                                        "sessionId": acp_session_id,
                                        "peerInfo": serde_json::to_value(&send_msg.peer_info).unwrap_or_default(),
                                    });
                                    if let Err(e) = conn.send_notification("channel/sessionCreated", params).await {
                                        tracing::warn!(
                                            channel = %channel_name,
                                            error = %e,
                                            "failed to notify channel of session creation"
                                        );
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                tracing::error!(channel = %channel_name, error = %e, "CreateSession failed");
                                return;
                            }
                            Err(_) => {
                                tracing::error!(channel = %channel_name, "CreateSession reply dropped");
                                return;
                            }
                        }
                    }

                    match self.debounce.push(&session_key, send_msg.content.clone()) {
                        DebounceAction::Immediate(msg) => {
                            self.send_ack_to_channel(&session_key).await;
                            self.debounce.mark_session_active(&session_key);
                            self.dispatch_to_agent(&session_key, &msg, &agent_name).await;
                        }
                        DebounceAction::Buffered => {
                            tracing::debug!(
                                channel = %channel_name,
                                session_key = %session_key,
                                "message buffered for debounce"
                            );
                        }
                        DebounceAction::Queued => {
                            tracing::debug!(
                                channel = %channel_name,
                                session_key = %session_key,
                                "message queued (agent mid-response)"
                            );
                        }
                    }
                } else {
                    tracing::warn!(channel = %channel_name, "failed to parse channel/sendMessage params");
                }
            }
            "channel/respondPermission" => {
                if let Ok(resp) = serde_json::from_value::<ChannelRespondPermission>(params) {
                    if let Some(agents_handle) = &self.agents_handle {
                        if let Err(e) = agents_handle.send(AgentsCommand::RespondPermission {
                            request_id: resp.request_id.clone(),
                            option_id: resp.option_id,
                        }).await {
                            tracing::warn!(
                                channel = %channel_name,
                                request_id = %resp.request_id,
                                error = %e,
                                "failed to forward permission response"
                            );
                        }
                    }
                } else {
                    tracing::warn!(channel = %channel_name, "failed to parse channel/respondPermission params");
                }
            }
            _ => {
                tracing::debug!(
                    channel = %channel_name,
                    method = %method,
                    "unhandled channel message"
                );
            }
        }
    }

    /// Shutdown all channel subprocesses.
    async fn shutdown_all(&mut self) {
        for slot in &mut self.slots {
            slot.cancel_token.cancel();
            if let Some(mut conn) = slot.connection.take() {
                let _ = conn.kill().await;
            }
        }
    }

    /// Poll all active channel connections for incoming messages.
    /// Returns (slot_index, message) or (slot_index, None) for crash.
    async fn poll_channels(&mut self) -> Option<(usize, Option<IncomingChannelMessage>)> {
        // Build futures for each active connection
        // We need to poll them without holding &mut self across await,
        // so we iterate and find the first ready one.
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.disabled {
                continue;
            }
            if let Some(conn) = &mut slot.connection {
                // Try a non-blocking poll using tokio::time::timeout with zero duration
                match tokio::time::timeout(Duration::from_millis(constants::POLL_TIMEOUT_MS), conn.recv_incoming()).await {
                    Ok(msg) => return Some((i, msg)),
                    Err(_) => continue, // timeout = no message ready
                }
            }
        }
        None
    }
}

impl Manager for ChannelsManager {
    type Command = ChannelsCommand;

    fn name(&self) -> &str {
        "channels"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        let parent_cancel = CancellationToken::new();

        for (name, config) in &self.channel_configs {
            if !config.enabled {
                tracing::info!(channel = %name, "channel disabled, skipping");
                continue;
            }
            let channel_id = ChannelId::from(name.as_str());
            let cancel_token = parent_cancel.child_token();

            let init_timeout = self.resolve_init_timeout(config);
            let backoff = match &config.backoff {
                Some(cfg) => ExponentialBackoff::new(
                    Duration::from_millis(cfg.base_delay_ms),
                    Duration::from_secs(cfg.max_delay_secs),
                ),
                None => ExponentialBackoff::default(),
            };
            let crash_tracker = match &config.crash_tracker {
                Some(cfg) => CrashTracker::new(cfg.max_crashes, Duration::from_secs(cfg.window_secs)),
                None => CrashTracker::default(),
            };

            match Self::spawn_and_initialize(config, &channel_id, init_timeout).await {
                Ok((conn, caps)) => {
                    tracing::info!(
                        channel = %name,
                        streaming = caps.streaming,
                        rich_text = caps.rich_text,
                        "channel initialized"
                    );
                    self.slots.push(ChannelSlot {
                        name: name.clone(),
                        config: config.clone(),
                        connection: Some(conn),
                        channel_id,
                        cancel_token,
                        backoff,
                        crash_tracker,
                        disabled: false,
                    });
                }
                Err(e) => {
                    tracing::error!(
                        channel = %name,
                        error = %e,
                        "failed to initialize channel, continuing without it"
                    );
                    self.slots.push(ChannelSlot {
                        name: name.clone(),
                        config: config.clone(),
                        connection: None,
                        channel_id,
                        cancel_token,
                        backoff,
                        crash_tracker,
                        disabled: false,
                    });
                }
            }
        }

        tracing::info!(
            manager = self.name(),
            active = self.slots.iter().filter(|s| s.connection.is_some()).count(),
            total = self.slots.len(),
            "manager started"
        );
        Ok(())
    }

    async fn run(mut self, cancel: CancellationToken) -> Result<(), ManagerError> {
        let mut cmd_rx = self.cmd_rx.take().expect("cmd_rx must exist");
        let mut channel_events_rx = self.channel_events_rx.take();

        tracing::info!(manager = self.name(), "manager running");

        let mut poll_interval = tokio::time::interval(Duration::from_millis(constants::POLL_INTERVAL_MS));

        loop {
            let channel_event_fut = async {
                match &mut channel_events_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            };

            let debounce_sleep = async {
                match self.debounce.next_deadline() {
                    Some(deadline) => tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await,
                    None => std::future::pending().await,
                }
            };

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(manager = "channels", "shutting down");
                    self.shutdown_all().await;
                    break;
                }
                Some(cmd) = cmd_rx.recv() => {
                    if self.handle_command(cmd).await {
                        break;
                    }
                }
                Some(event) = channel_event_fut => {
                    self.handle_channel_event(event).await;
                }
                _ = debounce_sleep => {
                    let ready = self.debounce.ready_sessions();
                    for session_key in ready {
                        if let Some(merged) = self.debounce.drain(&session_key) {
                            self.send_ack_to_channel(&session_key).await;
                            let agent_name = self.routing_table
                                .get(&session_key)
                                .map(|e| e.agent_name.clone())
                                .unwrap_or_default();
                            self.debounce.mark_session_active(&session_key);
                            self.dispatch_to_agent(&session_key, &merged, &agent_name).await;
                        }
                    }
                }
                _ = poll_interval.tick() => {
                    if let Some((idx, msg)) = self.poll_channels().await {
                        match msg {
                            Some(incoming) => {
                                self.handle_channel_message(idx, incoming).await;
                            }
                            None => {
                                let channel_name = self.slots[idx].name.clone();
                                tracing::warn!(channel = %channel_name, "channel subprocess exited");
                                self.handle_channel_crash(idx).await;
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(manager = "channels", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.slots.iter().any(|s| s.connection.is_some() && !s.disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel_map(entries: Vec<(&str, ChannelConfig)>) -> HashMap<String, ChannelConfig> {
        entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    fn test_channel_config(binary: &str, enabled: bool, agent: &str) -> ChannelConfig {
        ChannelConfig {
            binary: binary.into(),
            args: vec![],
            enabled,
            agent: agent.into(),
            ack: Default::default(),
            init_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
        }
    }

    fn default_init_timeout() -> u64 {
        10
    }

    #[test]
    fn channels_manager_name() {
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        assert_eq!(m.name(), "channels");
    }

    #[tokio::test]
    async fn channels_manager_start_with_no_channels() {
        let mut m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(m.slots.is_empty());
    }

    #[tokio::test]
    async fn channels_manager_health_check_no_channels() {
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn channels_manager_start_with_bad_binary_continues() {
        let configs = make_channel_map(vec![
            ("bad-channel", test_channel_config("nonexistent-binary-xyz-99999", true, "default")),
        ]);
        let mut m = ChannelsManager::new(configs, DebounceConfig::default(), default_init_timeout(), "default".into());
        let result = m.start().await;
        assert!(result.is_ok());
        assert_eq!(m.slots.len(), 1);
        assert!(m.slots[0].connection.is_none());
    }

    #[tokio::test]
    async fn channels_manager_shutdown_command() {
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        let tx = m.command_sender();
        tx.send(ChannelsCommand::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn channels_manager_crash_isolation() {
        let mut slot = ChannelSlot {
            name: "test".into(),
            config: test_channel_config("true", true, "default"),
            connection: None,
            channel_id: ChannelId::from("test"),
            cancel_token: CancellationToken::new(),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
            disabled: false,
        };

        slot.crash_tracker.record_crash();
        slot.crash_tracker.record_crash();
        assert!(!slot.crash_tracker.is_crash_loop());
        assert!(!slot.disabled);
    }

    #[tokio::test]
    async fn channels_manager_crash_loop_disables_channel() {
        let configs = make_channel_map(vec![
            ("crasher", test_channel_config("nonexistent-binary-xyz-99999", true, "default")),
        ]);
        let mut m = ChannelsManager::new(configs, DebounceConfig::default(), default_init_timeout(), "default".into());
        m.start().await.unwrap();
        let slot = &mut m.slots[0];
        for _ in 0..5 {
            slot.crash_tracker.record_crash();
        }

        m.handle_channel_crash(0).await;
        assert!(m.slots[0].disabled);
        assert!(m.slots[0].connection.is_none());
    }

    #[test]
    fn routing_table_insert_and_lookup() {
        let mut table: HashMap<SessionKey, RoutingEntry> = HashMap::new();
        let key = SessionKey::new("debug-http", "local", "dev");
        table.insert(key.clone(), RoutingEntry {
            channel_id: ChannelId::from("debug-http"),
            acp_session_id: "acp-sess-1".into(),
            slot_index: 0,
            agent_name: "default".into(),
        });

        assert!(table.contains_key(&key));
        let entry = table.get(&key).unwrap();
        assert_eq!(entry.acp_session_id, "acp-sess-1");
        assert_eq!(entry.slot_index, 0);
    }

    #[test]
    fn routing_table_different_peers_different_sessions() {
        let mut table: HashMap<SessionKey, RoutingEntry> = HashMap::new();
        let key_alice = SessionKey::new("telegram", "direct", "alice");
        let key_bob = SessionKey::new("telegram", "direct", "bob");

        table.insert(key_alice.clone(), RoutingEntry {
            channel_id: ChannelId::from("telegram"),
            acp_session_id: "sess-alice".into(),
            slot_index: 0,
            agent_name: "default".into(),
        });
        table.insert(key_bob.clone(), RoutingEntry {
            channel_id: ChannelId::from("telegram"),
            acp_session_id: "sess-bob".into(),
            slot_index: 0,
            agent_name: "default".into(),
        });

        assert_eq!(table.len(), 2);
        assert_eq!(table.get(&key_alice).unwrap().acp_session_id, "sess-alice");
        assert_eq!(table.get(&key_bob).unwrap().acp_session_id, "sess-bob");
    }

    #[tokio::test]
    async fn handle_channel_event_deliver_routes_to_correct_slot() {
        let mut m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        let key = SessionKey::new("test", "local", "dev");
        m.routing_table.insert(key.clone(), RoutingEntry {
            channel_id: ChannelId::from("test"),
            acp_session_id: "acp-1".into(),
            slot_index: 0,
            agent_name: "default".into(),
        });

        let event = ChannelEvent::DeliverMessage {
            session_key: key,
            content: serde_json::json!({"text": "hello"}),
        };
        let missing_key = SessionKey::new("nonexistent", "x", "y");
        let event_missing = ChannelEvent::DeliverMessage {
            session_key: missing_key,
            content: serde_json::json!({"text": "hello"}),
        };
        m.handle_channel_event(event_missing).await;
        let _ = event;
    }

    #[tokio::test]
    async fn with_agents_handle_sets_handle() {
        let (tx, _rx) = mpsc::channel::<AgentsCommand>(16);
        let handle = ManagerHandle::new(tx);
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into()).with_agents_handle(handle);
        assert!(m.agents_handle.is_some());
    }

    #[tokio::test]
    async fn with_channel_events_rx_sets_receiver() {
        let (_tx, rx) = mpsc::channel::<ChannelEvent>(16);
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into()).with_channel_events_rx(rx);
        assert!(m.channel_events_rx.is_some());
    }

    #[test]
    fn agent_name_for_channel_uses_config_agent_field() {
        let configs = make_channel_map(vec![
            ("telegram", test_channel_config("telegram-channel", true, "opencode")),
        ]);
        let mut m = ChannelsManager::new(configs, DebounceConfig::default(), default_init_timeout(), "default-agent".to_string());
        m.slots.push(ChannelSlot {
            name: "telegram".into(),
            config: test_channel_config("telegram-channel", true, "opencode"),
            connection: None,
            channel_id: ChannelId::from("telegram"),
            cancel_token: CancellationToken::new(),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
            disabled: false,
        });
        assert_eq!(m.agent_name_for_channel(0), "opencode");
    }

    #[test]
    fn agent_name_for_channel_defaults_to_default_agent() {
        let configs = make_channel_map(vec![
            ("debug-http", test_channel_config("debug-http", true, "default")),
        ]);
        let mut m = ChannelsManager::new(configs, DebounceConfig::default(), default_init_timeout(), "first-enabled".to_string());
        m.slots.push(ChannelSlot {
            name: "debug-http".into(),
            config: test_channel_config("debug-http", true, "default"),
            connection: None,
            channel_id: ChannelId::from("debug-http"),
            cancel_token: CancellationToken::new(),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
            disabled: false,
        });
        assert_eq!(m.agent_name_for_channel(0), "default");
    }

    #[tokio::test]
    async fn disabled_channel_not_spawned() {
        let configs = make_channel_map(vec![
            ("disabled-ch", test_channel_config("nonexistent-binary-xyz-99999", false, "default")),
        ]);
        let mut m = ChannelsManager::new(configs, DebounceConfig::default(), default_init_timeout(), "default".into());
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(m.slots.is_empty(), "disabled channel should not create a slot");
    }

    #[test]
    fn is_result_content_detects_result_type() {
        let result_content = serde_json::json!({
            "sessionId": "abc-123",
            "type": "result",
            "content": "Echo: hello"
        });
        assert!(ChannelsManager::is_result_content(&result_content));
    }

    #[test]
    fn is_result_content_rejects_thought_chunk() {
        let thought = serde_json::json!({
            "sessionId": "abc-123",
            "type": "agent_thought_chunk",
            "content": "thinking..."
        });
        assert!(!ChannelsManager::is_result_content(&thought));
    }

    #[test]
    fn is_result_content_rejects_message_chunk() {
        let chunk = serde_json::json!({
            "sessionId": "abc-123",
            "type": "agent_message_chunk",
            "content": "Echo: "
        });
        assert!(!ChannelsManager::is_result_content(&chunk));
    }

    #[test]
    fn is_result_content_rejects_empty_object() {
        assert!(!ChannelsManager::is_result_content(&serde_json::json!({})));
    }

    #[tokio::test]
    async fn send_ack_to_channel_no_routing_entry_is_noop() {
        let m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        let unknown_key = SessionKey::new("nonexistent", "x", "y");
        m.send_ack_to_channel(&unknown_key).await;
    }

    #[tokio::test]
    async fn send_ack_to_channel_no_connection_is_noop() {
        let mut m = ChannelsManager::new(HashMap::new(), DebounceConfig::default(), default_init_timeout(), "default".into());
        m.slots.push(ChannelSlot {
            name: "telegram".into(),
            config: test_channel_config("telegram-channel", true, "default"),
            connection: None,
            channel_id: ChannelId::from("telegram"),
            cancel_token: CancellationToken::new(),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
            disabled: false,
        });
        let key = SessionKey::new("telegram", "direct", "alice");
        m.routing_table.insert(key.clone(), RoutingEntry {
            channel_id: ChannelId::from("telegram"),
            acp_session_id: "sess-1".into(),
            slot_index: 0,
            agent_name: "default".into(),
        });
        m.send_ack_to_channel(&key).await;
    }
}
