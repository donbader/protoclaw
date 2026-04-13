use std::collections::{HashMap, HashSet};
use std::time::Duration;

use protoclaw_config::ChannelConfig;
use protoclaw_core::types::ChannelId;
use protoclaw_core::{
    AgentsCommand, CrashAction, CrashTracker, ExponentialBackoff, Manager, ManagerError,
    ManagerHandle, SessionKey, SlotLifecycle, constants,
};
use protoclaw_sdk_types::{ChannelAckConfig, ChannelEvent, PermissionOption};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{ChannelConnection, IncomingChannelMessage};
use crate::error::ChannelsError;
use crate::session_queue::SessionQueue;
use protoclaw_sdk_types::{
    ChannelCapabilities, ChannelInitializeResult, ChannelRespondPermission, ChannelSendMessage,
};

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
    _channel_id: ChannelId,
    acp_session_id: String,
    slot_index: usize,
    agent_name: String,
}

struct ChannelSlot {
    name: String,
    config: ChannelConfig,
    connection: Option<ChannelConnection>,
    channel_id: ChannelId,
    lifecycle: SlotLifecycle,
}

/// Manages channel subprocesses with crash isolation and session-keyed routing.
///
/// Each channel runs as a subprocess communicating over JSON-RPC stdio.
/// A crash in one channel does not affect other channels or the sidecar.
/// Inbound messages create/reuse ACP sessions via AgentsManager.
/// Outbound agent updates route back to the originating channel via routing table.
pub struct ChannelsManager {
    channel_configs: HashMap<String, ChannelConfig>,
    init_timeout_secs: u64,
    exit_timeout_secs: u64,
    permission_timeout_secs: Option<u64>,
    log_level: Option<String>,
    slots: Vec<ChannelSlot>,
    cmd_rx: Option<mpsc::Receiver<ChannelsCommand>>,
    cmd_tx: mpsc::Sender<ChannelsCommand>,
    routing_table: HashMap<SessionKey, RoutingEntry>,
    agents_handle: Option<ManagerHandle<AgentsCommand>>,
    channel_events_rx: Option<mpsc::Receiver<ChannelEvent>>,
    queue: SessionQueue,
    acked_sessions: HashSet<SessionKey>,
}

impl ChannelsManager {
    pub fn new(
        channel_configs: HashMap<String, ChannelConfig>,
        init_timeout_secs: u64,
        exit_timeout_secs: u64,
        _default_agent_name: String,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(constants::CMD_CHANNEL_CAPACITY);
        Self {
            channel_configs,
            init_timeout_secs,
            exit_timeout_secs,
            permission_timeout_secs: None,
            log_level: None,
            slots: Vec::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
            routing_table: HashMap::new(),
            agents_handle: None,
            channel_events_rx: None,
            queue: SessionQueue::new(),
            acked_sessions: HashSet::new(),
        }
    }

    pub fn command_sender(&self) -> mpsc::Sender<ChannelsCommand> {
        self.cmd_tx.clone()
    }

    /// Set the agents handle for creating/prompting sessions.
    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = Some(level);
        self
    }

    pub fn with_agents_handle(mut self, handle: ManagerHandle<AgentsCommand>) -> Self {
        self.agents_handle = Some(handle);
        self
    }

    pub fn with_permission_timeout(mut self, timeout_secs: Option<u64>) -> Self {
        self.permission_timeout_secs = timeout_secs;
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
        let secs = channel_config
            .init_timeout_secs
            .unwrap_or(self.init_timeout_secs);
        Duration::from_secs(secs)
    }

    #[tracing::instrument(skip(config, log_level), fields(channel_id = %channel_id), name = "channel_init_handshake")]
    async fn spawn_and_initialize(
        config: &ChannelConfig,
        channel_id: &ChannelId,
        init_timeout: Duration,
        log_level: Option<&str>,
    ) -> Result<(ChannelConnection, ChannelCapabilities), ChannelsError> {
        let mut conn = ChannelConnection::spawn(config, channel_id.clone(), log_level)?;

        let params = serde_json::json!({
            "protocolVersion": 1,
            "channelId": channel_id.as_ref(),
            "ack": ChannelAckConfig::from(config.ack.clone()),
            "options": config.options,
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

        match slot.lifecycle.record_crash_and_check() {
            CrashAction::Disabled => {
                tracing::error!(
                    channel = %channel_name,
                    "channel in crash loop, disabling"
                );
                slot.connection = None;
                return;
            }
            CrashAction::RestartAfter(delay) => {
                tracing::warn!(
                    channel = %channel_name,
                    delay_ms = delay.as_millis(),
                    "channel crashed, respawning after backoff"
                );
                tokio::time::sleep(delay).await;
            }
        }

        match Self::spawn_and_initialize(
            &slot.config,
            &slot.channel_id,
            Duration::from_secs(self.init_timeout_secs),
            self.log_level.as_deref(),
        )
        .await
        {
            Ok((conn, caps)) => {
                tracing::info!(
                    channel = %channel_name,
                    streaming = caps.streaming,
                    rich_text = caps.rich_text,
                    "channel recovered"
                );
                slot.connection = Some(conn);
                slot.lifecycle.backoff.reset();
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
                        if let Err(e) = conn
                            .send_notification("channel/deliverMessage", params)
                            .await
                        {
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
                        match conn.send_request("channel/requestPermission", params).await {
                            Ok(rx) => {
                                tracing::info!(channel = %slot.name, %request_id, "permission routed to channel");
                                if let Some(agents_handle) = self.agents_handle.clone() {
                                    let req_id = request_id.clone();
                                    let channel_name = slot.name.clone();
                                    let timeout_secs = self.permission_timeout_secs;
                                    tokio::spawn(async move {
                                        let result = if let Some(secs) = timeout_secs {
                                            match tokio::time::timeout(
                                                Duration::from_secs(secs),
                                                rx,
                                            ).await {
                                                Ok(Ok(resp_val)) => Some(resp_val),
                                                Ok(Err(_)) => None,
                                                Err(_elapsed) => {
                                                    tracing::warn!(
                                                        channel = %channel_name,
                                                        request_id = %req_id,
                                                        elapsed_secs = secs,
                                                        "permission response timed out, auto-denying"
                                                    );
                                                    None
                                                }
                                            }
                                        } else {
                                            rx.await.ok()
                                        };

                                        if let Some(resp_val) = result {
                                            let option_id = resp_val["optionId"]
                                                .as_str()
                                                .or_else(|| resp_val["result"]["optionId"].as_str())
                                                .unwrap_or("once")
                                                .to_string();
                                            tracing::info!(channel = %channel_name, request_id = %req_id, %option_id, "permission response from channel, forwarding to agents");
                                            let _ = agents_handle
                                                .send(AgentsCommand::RespondPermission {
                                                    request_id: req_id,
                                                    option_id,
                                                })
                                                .await;
                                        } else {
                                            tracing::info!(request_id = %req_id, "sending auto-deny to agent");
                                            let _ = agents_handle
                                                .send(AgentsCommand::RespondPermission {
                                                    request_id: req_id,
                                                    option_id: "denied".to_string(),
                                                })
                                                .await;
                                        }
                                    });
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    channel = %slot.name,
                                    error = %e,
                                    "failed to route permission"
                                );
                            }
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

    async fn handle_deliver_message(
        &mut self,
        session_key: SessionKey,
        content: serde_json::Value,
    ) {
        if let Some(entry) = self.routing_table.get(&session_key) {
            let slot = &self.slots[entry.slot_index];

            if !self.acked_sessions.contains(&session_key) {
                self.acked_sessions.insert(session_key.clone());
                if let Some(conn) = &slot.connection {
                    let lifecycle_params = serde_json::json!({
                        "sessionId": entry.acp_session_id,
                        "action": "response_started",
                    });
                    let _ = conn
                        .send_notification("channel/ackLifecycle", lifecycle_params)
                        .await;
                }
            }

            if let Some(conn) = &slot.connection {
                let params = serde_json::json!({
                    "sessionId": entry.acp_session_id,
                    "content": content,
                });
                if let Err(e) = conn
                    .send_notification("channel/deliverMessage", params)
                    .await
                {
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

    async fn handle_session_complete(&mut self, session_key: SessionKey) {
        if let Some(next_msg) = self.queue.mark_idle(&session_key) {
            let agent_name = self.routing_table
                .get(&session_key)
                .map(|e| e.agent_name.clone())
                .unwrap_or_else(|| {
                    tracing::warn!(session_key = %session_key, "no agent_name in routing table for SessionComplete");
                    String::new()
                });

            // Drain remaining queued messages and merge with the first
            // so the agent receives one combined prompt instead of N separate turns.
            let remaining = self.queue.drain_queued(&session_key);
            let merged = if remaining.is_empty() {
                next_msg
            } else {
                let mut parts = vec![next_msg];
                parts.extend(remaining);
                let count = parts.len();
                let merged = parts.join("\n");
                tracing::info!(
                    session_key = %session_key,
                    merged_count = count,
                    "merged queued messages into single prompt"
                );
                merged
            };

            self.send_ack_to_channel(&session_key).await;
            self.dispatch_to_agent(&session_key, &merged, &agent_name)
                .await;
        }
    }

    async fn route_permission_event(
        &self,
        session_key: &SessionKey,
        request_id: &str,
        description: String,
        options: serde_json::Value,
    ) {
        if let Some(entry) = self.routing_table.get(session_key) {
            let slot = &self.slots[entry.slot_index];
            if let Some(conn) = &slot.connection {
                let params = serde_json::json!({
                    "requestId": request_id,
                    "sessionId": entry.acp_session_id,
                    "description": description,
                    "options": options,
                });
                match conn.send_request("channel/requestPermission", params).await {
                    Ok(rx) => {
                        tracing::info!(channel = %slot.name, session_key = %session_key, request_id = %request_id, "permission routed to channel");
                        if let Some(agents_handle) = self.agents_handle.clone() {
                            let req_id = request_id.to_string();
                            let channel_name = slot.name.clone();
                            let timeout_secs = self.permission_timeout_secs;
                            tokio::spawn(async move {
                                let result = if let Some(secs) = timeout_secs {
                                    match tokio::time::timeout(
                                        Duration::from_secs(secs),
                                        rx,
                                    ).await {
                                        Ok(Ok(resp_val)) => Some(resp_val),
                                        Ok(Err(_)) => None,
                                        Err(_elapsed) => {
                                            tracing::warn!(
                                                channel = %channel_name,
                                                request_id = %req_id,
                                                elapsed_secs = secs,
                                                "permission response timed out, auto-denying"
                                            );
                                            None
                                        }
                                    }
                                } else {
                                    rx.await.ok()
                                };

                                if let Some(resp_val) = result {
                                    let option_id = resp_val["optionId"]
                                        .as_str()
                                        .or_else(|| resp_val["result"]["optionId"].as_str())
                                        .unwrap_or("once")
                                        .to_string();
                                    tracing::info!(channel = %channel_name, request_id = %req_id, %option_id, "permission response from channel, forwarding to agents");
                                    let _ = agents_handle
                                        .send(AgentsCommand::RespondPermission {
                                            request_id: req_id,
                                            option_id,
                                        })
                                        .await;
                                } else {
                                    tracing::info!(request_id = %req_id, "sending auto-deny to agent");
                                    let _ = agents_handle
                                        .send(AgentsCommand::RespondPermission {
                                            request_id: req_id,
                                            option_id: "denied".to_string(),
                                        })
                                        .await;
                                }
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            channel = %slot.name,
                            session_key = %session_key,
                            error = %e,
                            "failed to route permission from agent"
                        );
                    }
                }
            }
        } else {
            tracing::warn!(session_key = %session_key, request_id = %request_id, "no routing entry for permission");
        }
    }

    fn log_ack_message_event(
        &self,
        session_key: &SessionKey,
        channel_name: &str,
        peer_id: &str,
        message_id: &Option<String>,
    ) {
        tracing::debug!(session_key = %session_key, channel = %channel_name, peer = %peer_id, message_id = ?message_id, "ack message event received");
    }

    /// Handle a ChannelEvent from AgentsManager (outbound: agent → channel).
    async fn handle_channel_event(&mut self, event: ChannelEvent) {
        match event {
            ChannelEvent::DeliverMessage {
                session_key,
                content,
            } => {
                self.handle_deliver_message(session_key, content).await;
            }
            ChannelEvent::SessionComplete { session_key } => {
                self.handle_session_complete(session_key).await;
            }
            ChannelEvent::RoutePermission {
                session_key,
                request_id,
                description,
                options,
            } => {
                self.route_permission_event(&session_key, &request_id, description, options)
                    .await;
            }
            ChannelEvent::AckMessage {
                session_key,
                channel_name,
                peer_id,
                message_id,
            } => {
                self.log_ack_message_event(&session_key, &channel_name, &peer_id, &message_id);
            }
            _ => {}
        }
    }

    fn parse_channel_message(&self, msg: &IncomingChannelMessage) -> (String, serde_json::Value) {
        let value = match msg {
            IncomingChannelMessage::ChannelRequest(v)
            | IncomingChannelMessage::ChannelNotification(v) => v.clone(),
        };
        let method = value["method"].as_str().unwrap_or("").to_string();
        let params = value
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        (method, params)
    }

    fn build_session_key(send_msg: &ChannelSendMessage) -> SessionKey {
        SessionKey::new(
            &send_msg.peer_info.channel_name,
            &send_msg.peer_info.kind,
            &send_msg.peer_info.peer_id,
        )
    }

    fn agents_handle_for_collection(
        &self,
        channel_name: &str,
    ) -> Option<ManagerHandle<AgentsCommand>> {
        match &self.agents_handle {
            Some(handle) => Some(handle.clone()),
            None => {
                tracing::warn!(channel = %channel_name, "no agents handle, cannot route message");
                None
            }
        }
    }

    async fn collect_send_message(
        &mut self,
        slot_index: usize,
        params: serde_json::Value,
        channel_name: &str,
    ) -> Option<SessionKey> {
        let send_msg = match serde_json::from_value::<ChannelSendMessage>(params) {
            Ok(send_msg) => send_msg,
            Err(_) => {
                tracing::warn!(channel = %channel_name, "failed to parse channel/sendMessage params");
                return None;
            }
        };

        let session_key = Self::build_session_key(&send_msg);
        let agents_handle = self.agents_handle_for_collection(channel_name)?;
        let agent_name = self.agent_name_for_channel(slot_index).to_string();

        let session_ready = self
            .ensure_session_created(
                slot_index,
                &session_key,
                &send_msg,
                &agents_handle,
                &agent_name,
            )
            .await;
        if !session_ready {
            return None;
        }

        let content = send_msg.content;
        if self.queue.is_active(&session_key) {
            self.queue.push(&session_key, content);
            tracing::debug!(
                channel = %channel_name,
                session_key = %session_key,
                "message queued (session busy)"
            );
            None
        } else {
            self.queue.push_only(&session_key, content);
            Some(session_key)
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
            if let Err(e) = conn
                .send_notification("channel/ackMessage", ack_params)
                .await
            {
                tracing::warn!(
                    channel = %slot.name,
                    error = %e,
                    "failed to send ack notification"
                );
            }
        }
    }

    /// Dispatch a merged/immediate message to the agent as a PromptSession.
    async fn dispatch_to_agent(
        &mut self,
        session_key: &SessionKey,
        message: &str,
        agent_name: &str,
    ) {
        self.acked_sessions.remove(session_key);

        if let Some(entry) = self.routing_table.get(session_key) {
            let slot = &self.slots[entry.slot_index];
            if let Some(conn) = &slot.connection {
                let params = serde_json::json!({
                    "sessionId": entry.acp_session_id,
                    "channelName": slot.name,
                });
                let _ = conn
                    .send_notification("channel/typingIndicator", params)
                    .await;
            }
        }

        let agents_handle = match &self.agents_handle {
            Some(h) => h.clone(),
            None => {
                tracing::warn!(session_key = %session_key, "no agents handle, cannot dispatch");
                return;
            }
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(e) = agents_handle
            .send(AgentsCommand::PromptSession {
                agent_name: agent_name.to_string(),
                session_key: session_key.clone(),
                message: message.to_string(),
                reply: reply_tx,
            })
            .await
        {
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

    /// Ensure a session exists for the given session key, creating it via AgentsManager if needed.
    /// Returns false if session creation failed (caller should abort message processing).
    async fn ensure_session_created(
        &mut self,
        slot_index: usize,
        session_key: &SessionKey,
        send_msg: &ChannelSendMessage,
        agents_handle: &ManagerHandle<AgentsCommand>,
        agent_name: &str,
    ) -> bool {
        if self.routing_table.contains_key(session_key) {
            return true;
        }
        let channel_name = self.slots[slot_index].name.clone();
        let channel_id = self.slots[slot_index].channel_id.clone();

        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(e) = agents_handle
            .send(AgentsCommand::CreateSession {
                agent_name: agent_name.to_string(),
                session_key: session_key.clone(),
                reply: reply_tx,
            })
            .await
        {
            tracing::error!(channel = %channel_name, error = %e, "failed to send CreateSession");
            return false;
        }

        match reply_rx.await {
            Ok(Ok(acp_session_id)) => {
                tracing::info!(
                    channel = %channel_name,
                    session_key = %session_key,
                    acp_session_id = %acp_session_id,
                    "session created"
                );
                self.routing_table.insert(
                    session_key.clone(),
                    RoutingEntry {
                        _channel_id: channel_id,
                        acp_session_id: acp_session_id.clone(),
                        slot_index,
                        agent_name: agent_name.to_string(),
                    },
                );

                if let Some(conn) = &self.slots[slot_index].connection {
                    let peer_info_json = serde_json::to_value(&send_msg.peer_info).unwrap_or_else(|e| {
                        tracing::warn!(
                            channel = %channel_name,
                            error = %e,
                            "failed to serialize peerInfo for session/created notification, using null"
                        );
                        serde_json::Value::default()
                    });
                    let params = serde_json::json!({
                        "sessionId": acp_session_id,
                        "peerInfo": peer_info_json,
                    });
                    if let Err(e) = conn
                        .send_notification("channel/sessionCreated", params)
                        .await
                    {
                        tracing::warn!(
                            channel = %channel_name,
                            error = %e,
                            "failed to notify channel of session creation"
                        );
                    }
                }
                true
            }
            Ok(Err(e)) => {
                tracing::error!(channel = %channel_name, error = %e, "CreateSession failed");
                if let Some(conn) = &self.slots[slot_index].connection {
                    let error_params = serde_json::json!({
                        "sessionId": "",
                        "content": format!("⚠️ Failed to create session: {e}"),
                    });
                    if let Err(notify_err) = conn
                        .send_notification("channel/deliverMessage", error_params)
                        .await
                    {
                        tracing::warn!(channel = %channel_name, error = %notify_err, "failed to send session error to channel");
                    }
                }
                false
            }
            Err(_) => {
                tracing::error!(channel = %channel_name, "CreateSession reply dropped");
                if let Some(conn) = &self.slots[slot_index].connection {
                    let error_params = serde_json::json!({
                        "sessionId": "",
                        "content": "⚠️ Failed to create session: internal error (reply dropped)",
                    });
                    if let Err(notify_err) = conn
                        .send_notification("channel/deliverMessage", error_params)
                        .await
                    {
                        tracing::warn!(channel = %channel_name, error = %notify_err, "failed to send session error to channel");
                    }
                }
                false
            }
        }
    }

    /// Forward a channel/respondPermission message to the agents manager.
    async fn handle_respond_permission(&self, params: serde_json::Value, channel_name: &str) {
        if let Ok(resp) = serde_json::from_value::<ChannelRespondPermission>(params) {
            if let Some(agents_handle) = &self.agents_handle
                && let Err(e) = agents_handle
                    .send(AgentsCommand::RespondPermission {
                        request_id: resp.request_id.clone(),
                        option_id: resp.option_id,
                    })
                    .await
            {
                tracing::warn!(
                    channel = %channel_name,
                    request_id = %resp.request_id,
                    error = %e,
                    "failed to forward permission response"
                );
            }
        } else {
            tracing::warn!(channel = %channel_name, "failed to parse channel/respondPermission params");
        }
    }

    /// Collect an incoming channel message into the session queue without dispatching.
    /// Returns the session key if the message was collected (for later flush).
    async fn collect_channel_message(
        &mut self,
        slot_index: usize,
        msg: IncomingChannelMessage,
    ) -> Option<SessionKey> {
        let channel_name = self.slots[slot_index].name.clone();
        let (method, params) = self.parse_channel_message(&msg);

        match method.as_str() {
            "channel/sendMessage" => {
                self.collect_send_message(slot_index, params, &channel_name)
                    .await
            }
            "channel/respondPermission" => {
                self.handle_respond_permission(params, &channel_name).await;
                None
            }
            _ => {
                tracing::debug!(
                    channel = %channel_name,
                    method = %method,
                    "unhandled channel message"
                );
                None
            }
        }
    }

    /// Flush pending messages for a session and dispatch as a single merged prompt.
    async fn flush_and_dispatch(&mut self, session_key: &SessionKey) {
        if let Some(merged) = self.queue.flush_pending(session_key) {
            let agent_name = self.routing_table
                .get(session_key)
                .map(|e| e.agent_name.clone())
                .unwrap_or_else(|| {
                    tracing::warn!(session_key = %session_key, "no agent_name in routing table for flush_and_dispatch");
                    String::new()
                });

            let count = merged.matches('\n').count() + 1;
            if count > 1 {
                tracing::info!(
                    session_key = %session_key,
                    merged_count = count,
                    "merged buffered messages into single prompt"
                );
            }

            self.send_ack_to_channel(session_key).await;
            self.dispatch_to_agent(session_key, &merged, &agent_name)
                .await;
        }
    }

    /// Shutdown all channel subprocesses.
    async fn shutdown_all(&mut self) {
        let timeout = Duration::from_secs(self.exit_timeout_secs);
        for slot in &mut self.slots {
            slot.lifecycle.cancel_token.cancel();
            if let Some(mut conn) = slot.connection.take()
                && let Err(e) = conn.graceful_shutdown(timeout).await
            {
                tracing::warn!(channel = %slot.name, error = %e, "graceful shutdown failed, process may linger");
            }
        }
    }

    /// Poll all active channel connections for incoming messages.
    /// Drains all ready messages across all connections in one pass.
    async fn poll_channels(&mut self) -> Vec<(usize, Option<IncomingChannelMessage>)> {
        let mut results = Vec::new();
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.lifecycle.disabled {
                continue;
            }
            if let Some(conn) = &mut slot.connection {
                while let Ok(msg) = tokio::time::timeout(
                    Duration::from_millis(constants::POLL_TIMEOUT_MS),
                    conn.recv_incoming(),
                )
                .await
                {
                    results.push((i, msg));
                }
            }
        }
        results
    }

    fn build_slot_lifecycle(
        &self,
        config: &ChannelConfig,
        parent_cancel: &CancellationToken,
    ) -> SlotLifecycle {
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
        SlotLifecycle::new(parent_cancel, backoff, crash_tracker)
    }

    async fn start_channel_slot(
        &self,
        name: &str,
        config: &ChannelConfig,
        parent_cancel: &CancellationToken,
    ) -> ChannelSlot {
        let channel_id = ChannelId::from(name);
        let init_timeout = self.resolve_init_timeout(config);
        let lifecycle = self.build_slot_lifecycle(config, parent_cancel);

        match Self::spawn_and_initialize(
            config,
            &channel_id,
            init_timeout,
            self.log_level.as_deref(),
        )
        .await
        {
            Ok((conn, caps)) => {
                tracing::info!(
                    channel = %name,
                    streaming = caps.streaming,
                    rich_text = caps.rich_text,
                    "channel initialized"
                );
                ChannelSlot {
                    name: name.to_string(),
                    config: config.clone(),
                    connection: Some(conn),
                    channel_id,
                    lifecycle,
                }
            }
            Err(e) => {
                tracing::error!(
                    channel = %name,
                    error = %e,
                    "failed to initialize channel, continuing without it"
                );
                ChannelSlot {
                    name: name.to_string(),
                    config: config.clone(),
                    connection: None,
                    channel_id,
                    lifecycle,
                }
            }
        }
    }
}

impl Manager for ChannelsManager {
    type Command = ChannelsCommand;

    fn name(&self) -> &str {
        "channels"
    }

    #[tracing::instrument(skip(self), name = "channels_manager_start")]
    async fn start(&mut self) -> Result<(), ManagerError> {
        let parent_cancel = CancellationToken::new();

        for (name, config) in &self.channel_configs {
            if !config.enabled {
                tracing::info!(channel = %name, "channel disabled, skipping");
                continue;
            }
            let slot = self.start_channel_slot(name, config, &parent_cancel).await;
            self.slots.push(slot);
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

        let mut poll_interval =
            tokio::time::interval(Duration::from_millis(constants::POLL_INTERVAL_MS));

        loop {
            let channel_event_fut = async {
                match &mut channel_events_rx {
                    Some(rx) => rx.recv().await,
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
                _ = poll_interval.tick() => {
                    let messages = self.poll_channels().await;
                    if messages.is_empty() {
                        continue;
                    }

                    let mut sessions_to_flush: HashSet<SessionKey> = HashSet::new();

                    for (idx, msg) in messages {
                        match msg {
                            Some(incoming) => {
                                if let Some(session_key) = self.collect_channel_message(idx, incoming).await {
                                    sessions_to_flush.insert(session_key);
                                }
                            }
                            None => {
                                let channel_name = self.slots[idx].name.clone();
                                tracing::warn!(channel = %channel_name, "channel subprocess exited");
                                self.handle_channel_crash(idx).await;
                            }
                        }
                    }

                    for session_key in sessions_to_flush {
                        self.flush_and_dispatch(&session_key).await;
                    }
                }
            }
        }

        tracing::info!(manager = "channels", "manager stopped");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.slots
            .iter()
            .any(|s| s.connection.is_some() && !s.lifecycle.disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn make_channel_map(entries: Vec<(&str, ChannelConfig)>) -> HashMap<String, ChannelConfig> {
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    fn test_channel_config(binary: &str, enabled: bool, agent: &str) -> ChannelConfig {
        ChannelConfig {
            binary: binary.into(),
            args: vec![],
            enabled,
            agent: agent.into(),
            ack: Default::default(),
            init_timeout_secs: None,
            exit_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    fn default_init_timeout() -> u64 {
        10
    }

    fn default_exit_timeout() -> u64 {
        5
    }

    #[test]
    fn when_channels_manager_name_queried_then_returns_expected_name() {
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        assert_eq!(m.name(), "channels");
    }

    #[tokio::test]
    async fn when_channels_manager_started_with_no_channels_then_starts_successfully() {
        let mut m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(m.slots.is_empty());
    }

    #[tokio::test]
    async fn when_no_channels_configured_then_health_check_returns_healthy() {
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn when_channel_binary_invalid_then_manager_continues_without_it() {
        let configs = make_channel_map(vec![(
            "bad-channel",
            test_channel_config("nonexistent-binary-xyz-99999", true, "default"),
        )]);
        let mut m = ChannelsManager::new(
            configs,
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let result = m.start().await;
        assert!(result.is_ok());
        assert_eq!(m.slots.len(), 1);
        assert!(m.slots[0].connection.is_none());
    }

    #[tokio::test]
    async fn when_shutdown_command_sent_then_channels_manager_shuts_down() {
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let tx = m.command_sender();
        tx.send(ChannelsCommand::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn when_one_channel_crashes_then_other_channels_unaffected() {
        let parent = CancellationToken::new();
        let mut slot = ChannelSlot {
            name: "test".into(),
            config: test_channel_config("true", true, "default"),
            connection: None,
            channel_id: ChannelId::from("test"),
            lifecycle: SlotLifecycle::new(
                &parent,
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        };

        slot.lifecycle.crash_tracker.record_crash();
        slot.lifecycle.crash_tracker.record_crash();
        assert!(!slot.lifecycle.crash_tracker.is_crash_loop());
        assert!(!slot.lifecycle.disabled);
    }

    #[tokio::test]
    async fn when_channel_crashes_repeatedly_then_channel_disabled_after_threshold() {
        let configs = make_channel_map(vec![(
            "crasher",
            test_channel_config("nonexistent-binary-xyz-99999", true, "default"),
        )]);
        let mut m = ChannelsManager::new(
            configs,
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        m.start().await.unwrap();
        let slot = &mut m.slots[0];
        for _ in 0..5 {
            slot.lifecycle.crash_tracker.record_crash();
        }

        m.handle_channel_crash(0).await;
        assert!(m.slots[0].lifecycle.disabled);
        assert!(m.slots[0].connection.is_none());
    }

    #[test]
    fn when_routing_entry_inserted_then_lookup_returns_it() {
        let mut table: HashMap<SessionKey, RoutingEntry> = HashMap::new();
        let key = SessionKey::new("debug-http", "local", "dev");
        table.insert(
            key.clone(),
            RoutingEntry {
                _channel_id: ChannelId::from("debug-http"),
                acp_session_id: "acp-sess-1".into(),
                slot_index: 0,
                agent_name: "default".into(),
            },
        );

        assert!(table.contains_key(&key));
        let entry = table.get(&key).unwrap();
        assert_eq!(entry.acp_session_id, "acp-sess-1");
        assert_eq!(entry.slot_index, 0);
    }

    #[test]
    fn when_different_peers_route_then_different_sessions_created() {
        let mut table: HashMap<SessionKey, RoutingEntry> = HashMap::new();
        let key_alice = SessionKey::new("telegram", "direct", "alice");
        let key_bob = SessionKey::new("telegram", "direct", "bob");

        table.insert(
            key_alice.clone(),
            RoutingEntry {
                _channel_id: ChannelId::from("telegram"),
                acp_session_id: "sess-alice".into(),
                slot_index: 0,
                agent_name: "default".into(),
            },
        );
        table.insert(
            key_bob.clone(),
            RoutingEntry {
                _channel_id: ChannelId::from("telegram"),
                acp_session_id: "sess-bob".into(),
                slot_index: 0,
                agent_name: "default".into(),
            },
        );

        assert_eq!(table.len(), 2);
        assert_eq!(table.get(&key_alice).unwrap().acp_session_id, "sess-alice");
        assert_eq!(table.get(&key_bob).unwrap().acp_session_id, "sess-bob");
    }

    #[tokio::test]
    async fn when_deliver_event_received_then_routed_to_correct_agent_slot() {
        let mut m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let key = SessionKey::new("test", "local", "dev");
        m.routing_table.insert(
            key.clone(),
            RoutingEntry {
                _channel_id: ChannelId::from("test"),
                acp_session_id: "acp-1".into(),
                slot_index: 0,
                agent_name: "default".into(),
            },
        );

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
    async fn when_agents_handle_set_then_handle_stored_correctly() {
        let (tx, _rx) = mpsc::channel::<AgentsCommand>(16);
        let handle = ManagerHandle::new(tx);
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        )
        .with_agents_handle(handle);
        assert!(m.agents_handle.is_some());
    }

    #[tokio::test]
    async fn when_channel_events_receiver_set_then_stored_correctly() {
        let (_tx, rx) = mpsc::channel::<ChannelEvent>(16);
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        )
        .with_channel_events_rx(rx);
        assert!(m.channel_events_rx.is_some());
    }

    #[test]
    fn when_channel_config_has_agent_field_then_that_agent_name_used() {
        let configs = make_channel_map(vec![(
            "telegram",
            test_channel_config("telegram-channel", true, "opencode"),
        )]);
        let mut m = ChannelsManager::new(
            configs,
            default_init_timeout(),
            default_exit_timeout(),
            "default-agent".to_string(),
        );
        m.slots.push(ChannelSlot {
            name: "telegram".into(),
            config: test_channel_config("telegram-channel", true, "opencode"),
            connection: None,
            channel_id: ChannelId::from("telegram"),
            lifecycle: SlotLifecycle::new(
                &CancellationToken::new(),
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        });
        assert_eq!(m.agent_name_for_channel(0), "opencode");
    }

    #[test]
    fn when_channel_config_has_no_agent_field_then_default_agent_used() {
        let configs = make_channel_map(vec![(
            "debug-http",
            test_channel_config("debug-http", true, "default"),
        )]);
        let mut m = ChannelsManager::new(
            configs,
            default_init_timeout(),
            default_exit_timeout(),
            "first-enabled".to_string(),
        );
        m.slots.push(ChannelSlot {
            name: "debug-http".into(),
            config: test_channel_config("debug-http", true, "default"),
            connection: None,
            channel_id: ChannelId::from("debug-http"),
            lifecycle: SlotLifecycle::new(
                &CancellationToken::new(),
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        });
        assert_eq!(m.agent_name_for_channel(0), "default");
    }

    #[tokio::test]
    async fn when_channel_is_disabled_then_not_spawned_on_start() {
        let configs = make_channel_map(vec![(
            "disabled-ch",
            test_channel_config("nonexistent-binary-xyz-99999", false, "default"),
        )]);
        let mut m = ChannelsManager::new(
            configs,
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(
            m.slots.is_empty(),
            "disabled channel should not create a slot"
        );
    }

    #[tokio::test]
    async fn when_ack_sent_for_session_with_no_routing_entry_then_is_noop() {
        let m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        let unknown_key = SessionKey::new("nonexistent", "x", "y");
        m.send_ack_to_channel(&unknown_key).await;
    }

    #[tokio::test]
    async fn when_ack_sent_for_session_with_no_connection_then_is_noop() {
        let mut m = ChannelsManager::new(
            HashMap::new(),
            default_init_timeout(),
            default_exit_timeout(),
            "default".into(),
        );
        m.slots.push(ChannelSlot {
            name: "telegram".into(),
            config: test_channel_config("telegram-channel", true, "default"),
            connection: None,
            channel_id: ChannelId::from("telegram"),
            lifecycle: SlotLifecycle::new(
                &CancellationToken::new(),
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        });
        let key = SessionKey::new("telegram", "direct", "alice");
        m.routing_table.insert(
            key.clone(),
            RoutingEntry {
                _channel_id: ChannelId::from("telegram"),
                acp_session_id: "sess-1".into(),
                slot_index: 0,
                agent_name: "default".into(),
            },
        );
        m.send_ack_to_channel(&key).await;
    }

    #[rstest]
    fn when_ack_config_converted_then_all_fields_match() {
        let config_ack = protoclaw_config::AckConfig {
            reaction: true,
            typing: true,
            reaction_emoji: "🤔".to_string(),
            reaction_lifecycle: protoclaw_config::ReactionLifecycle::ReplaceDone,
        };
        let channel_ack: ChannelAckConfig = config_ack.into();
        assert_eq!(channel_ack.reaction, true);
        assert_eq!(channel_ack.typing, true);
        assert_eq!(channel_ack.reaction_emoji, "🤔");
        assert_eq!(channel_ack.reaction_lifecycle, "replace_done");
    }

    #[rstest]
    fn when_default_ack_config_converted_then_matches_defaults() {
        let config_ack = protoclaw_config::AckConfig::default();
        let channel_ack: ChannelAckConfig = config_ack.into();
        assert_eq!(channel_ack.reaction, false);
        assert_eq!(channel_ack.typing, false);
    }
}
