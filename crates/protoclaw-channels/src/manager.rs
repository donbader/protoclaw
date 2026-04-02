use std::time::Duration;

use protoclaw_acp::PermissionOption;
use protoclaw_config::ChannelConfig;
use protoclaw_core::types::ChannelId;
use protoclaw_core::{CrashTracker, ExponentialBackoff, Manager, ManagerError};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::connection::{ChannelConnection, IncomingChannelMessage};
use crate::error::ChannelsError;
use crate::types::{ChannelCapabilities, ChannelInitializeResult};

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

/// Per-channel state: connection, config, crash recovery.
struct ChannelSlot {
    config: ChannelConfig,
    connection: Option<ChannelConnection>,
    channel_id: ChannelId,
    cancel_token: CancellationToken,
    backoff: ExponentialBackoff,
    crash_tracker: CrashTracker,
    disabled: bool,
}

/// Manages channel subprocesses with crash isolation.
///
/// Each channel runs as a subprocess communicating over JSON-RPC stdio.
/// A crash in one channel does not affect other channels or the sidecar.
pub struct ChannelsManager {
    channel_configs: Vec<ChannelConfig>,
    slots: Vec<ChannelSlot>,
    cmd_rx: Option<mpsc::Receiver<ChannelsCommand>>,
    cmd_tx: mpsc::Sender<ChannelsCommand>,
}

impl ChannelsManager {
    pub fn new(channel_configs: Vec<ChannelConfig>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(16);
        Self {
            channel_configs,
            slots: Vec::new(),
            cmd_rx: Some(cmd_rx),
            cmd_tx,
        }
    }

    pub fn command_sender(&self) -> mpsc::Sender<ChannelsCommand> {
        self.cmd_tx.clone()
    }

    /// Spawn and initialize a single channel subprocess.
    async fn spawn_and_initialize(
        config: &ChannelConfig,
        channel_id: &ChannelId,
    ) -> Result<(ChannelConnection, ChannelCapabilities), ChannelsError> {
        let mut conn = ChannelConnection::spawn(config, channel_id.clone())?;

        let params = serde_json::json!({
            "protocolVersion": 1,
            "channelId": channel_id.as_ref(),
        });

        let rx = conn.send_request("initialize", params).await?;
        let resp = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| ChannelsError::Timeout(Duration::from_secs(10)))?
            .map_err(|_| ChannelsError::ConnectionClosed)?;

        let result: ChannelInitializeResult = serde_json::from_value(resp)?;
        let caps = result.capabilities;
        conn.set_capabilities(caps.clone());

        Ok((conn, caps))
    }

    /// Handle a crashed channel: backoff, respawn, re-initialize.
    async fn handle_channel_crash(&mut self, slot_index: usize) {
        let slot = &mut self.slots[slot_index];
        let channel_name = &slot.config.name;

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

        match Self::spawn_and_initialize(&slot.config, &slot.channel_id).await {
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
                // For now, deliver to all active channels (routing by session_key
                // will be wired in Plan 02 with multi-session support).
                tracing::debug!(session_key = %session_key, "delivering message to channels");
                for slot in &self.slots {
                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "sessionId": session_key,
                            "content": content,
                        });
                        if let Err(e) = conn.send_notification("channel/deliverMessage", params).await {
                            tracing::warn!(
                                channel = %slot.config.name,
                                error = %e,
                                "failed to deliver message"
                            );
                        }
                    }
                }
            }
            ChannelsCommand::RoutePermission {
                session_key,
                request_id,
                description,
                options,
            } => {
                tracing::debug!(
                    session_key = %session_key,
                    request_id = %request_id,
                    "routing permission request to channels"
                );
                for slot in &self.slots {
                    if let Some(conn) = &slot.connection {
                        let params = serde_json::json!({
                            "requestId": request_id,
                            "sessionId": session_key,
                            "description": description,
                            "options": options,
                        });
                        if let Err(e) = conn.send_notification("channel/requestPermission", params).await {
                            tracing::warn!(
                                channel = %slot.config.name,
                                error = %e,
                                "failed to route permission"
                            );
                        }
                    }
                }
            }
            ChannelsCommand::Shutdown => {
                self.shutdown_all().await;
                return true;
            }
        }
        false
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
                match tokio::time::timeout(Duration::from_millis(1), conn.recv_incoming()).await {
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

        for config in &self.channel_configs {
            let channel_id = ChannelId::from(config.name.as_str());
            let cancel_token = parent_cancel.child_token();

            match Self::spawn_and_initialize(config, &channel_id).await {
                Ok((conn, caps)) => {
                    tracing::info!(
                        channel = %config.name,
                        streaming = caps.streaming,
                        rich_text = caps.rich_text,
                        "channel initialized"
                    );
                    self.slots.push(ChannelSlot {
                        config: config.clone(),
                        connection: Some(conn),
                        channel_id,
                        cancel_token,
                        backoff: ExponentialBackoff::default(),
                        crash_tracker: CrashTracker::default(),
                        disabled: false,
                    });
                }
                Err(e) => {
                    tracing::error!(
                        channel = %config.name,
                        error = %e,
                        "failed to initialize channel, continuing without it"
                    );
                    self.slots.push(ChannelSlot {
                        config: config.clone(),
                        connection: None,
                        channel_id,
                        cancel_token,
                        backoff: ExponentialBackoff::default(),
                        crash_tracker: CrashTracker::default(),
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

        tracing::info!(manager = self.name(), "manager running");

        loop {
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
                else => {
                    // Poll channels for incoming messages
                    if let Some((idx, msg)) = self.poll_channels().await {
                        match msg {
                            Some(incoming) => {
                                let channel_name = self.slots[idx].config.name.clone();
                                tracing::debug!(
                                    channel = %channel_name,
                                    "received incoming message from channel"
                                );
                                // Inbound routing (channel → agent) will be wired in Plan 02.
                                // For now, log the message.
                                match incoming {
                                    IncomingChannelMessage::ChannelRequest(v) => {
                                        tracing::info!(
                                            channel = %channel_name,
                                            method = %v["method"].as_str().unwrap_or("unknown"),
                                            "channel request (routing not yet wired)"
                                        );
                                    }
                                    IncomingChannelMessage::ChannelNotification(v) => {
                                        tracing::info!(
                                            channel = %channel_name,
                                            method = %v["method"].as_str().unwrap_or("unknown"),
                                            "channel notification (routing not yet wired)"
                                        );
                                    }
                                }
                            }
                            None => {
                                // Channel crashed (EOF)
                                let channel_name = self.slots[idx].config.name.clone();
                                tracing::warn!(channel = %channel_name, "channel subprocess exited");
                                self.handle_channel_crash(idx).await;
                            }
                        }
                    } else {
                        // No messages ready, yield briefly to avoid busy-loop
                        tokio::time::sleep(Duration::from_millis(50)).await;
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

    #[test]
    fn channels_manager_name() {
        let m = ChannelsManager::new(vec![]);
        assert_eq!(m.name(), "channels");
    }

    #[tokio::test]
    async fn channels_manager_start_with_no_channels() {
        let mut m = ChannelsManager::new(vec![]);
        let result = m.start().await;
        assert!(result.is_ok());
        assert!(m.slots.is_empty());
    }

    #[tokio::test]
    async fn channels_manager_health_check_no_channels() {
        let m = ChannelsManager::new(vec![]);
        assert!(!m.health_check().await);
    }

    #[tokio::test]
    async fn channels_manager_start_with_bad_binary_continues() {
        let configs = vec![ChannelConfig {
            name: "bad-channel".into(),
            binary: "nonexistent-binary-xyz-99999".into(),
            args: vec![],
        }];
        let mut m = ChannelsManager::new(configs);
        let result = m.start().await;
        // Should succeed — bad channels are logged but don't block startup
        assert!(result.is_ok());
        assert_eq!(m.slots.len(), 1);
        assert!(m.slots[0].connection.is_none());
    }

    #[tokio::test]
    async fn channels_manager_shutdown_command() {
        let m = ChannelsManager::new(vec![]);
        let tx = m.command_sender();
        tx.send(ChannelsCommand::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn channels_manager_crash_isolation() {
        // Verify that ChannelSlot crash tracking is per-channel
        let mut slot = ChannelSlot {
            config: ChannelConfig {
                name: "test".into(),
                binary: "true".into(),
                args: vec![],
            },
            connection: None,
            channel_id: ChannelId::from("test"),
            cancel_token: CancellationToken::new(),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
            disabled: false,
        };

        // Record crashes — should not affect other slots
        slot.crash_tracker.record_crash();
        slot.crash_tracker.record_crash();
        assert!(!slot.crash_tracker.is_crash_loop());
        assert!(!slot.disabled);
    }

    #[tokio::test]
    async fn channels_manager_crash_loop_disables_channel() {
        let configs = vec![ChannelConfig {
            name: "crasher".into(),
            binary: "nonexistent-binary-xyz-99999".into(),
            args: vec![],
        }];
        let mut m = ChannelsManager::new(configs);
        m.start().await.unwrap();

        // Simulate crash loop by recording enough crashes
        let slot = &mut m.slots[0];
        for _ in 0..5 {
            slot.crash_tracker.record_crash();
        }

        // Now handle_channel_crash should disable the channel
        m.handle_channel_crash(0).await;
        assert!(m.slots[0].disabled);
        assert!(m.slots[0].connection.is_none());
    }
}
