use std::sync::Arc;
use std::time::Duration;

use protoclaw_config::{ProtoclawConfig, resolve_all_binary_paths};
use protoclaw_core::{
    CrashAction, CrashTracker, ExponentialBackoff, HealthSnapshot, HealthStatus, Manager,
    ManagerError, ManagerHandle, SlotLifecycle,
};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use protoclaw_agents::{AgentsCommand, AgentsManager};
use protoclaw_channels::{ChannelsCommand, ChannelsManager};
use protoclaw_core::ChannelEvent;
use protoclaw_tools::{ToolsCommand, ToolsManager};

pub mod admin_server;

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("failed to boot manager '{manager}': {source}")]
    BootFailure {
        manager: String,
        #[source]
        source: ManagerError,
    },
}

pub struct Supervisor {
    config: ProtoclawConfig,
    tools_tx: Option<tokio::sync::mpsc::Sender<ToolsCommand>>,
    agents_cmd_tx: Option<tokio::sync::mpsc::Sender<AgentsCommand>>,
    channels_cmd_tx: Option<tokio::sync::mpsc::Sender<ChannelsCommand>>,
    channel_events_tx: Option<tokio::sync::mpsc::Sender<ChannelEvent>>,
    channel_events_rx: Option<tokio::sync::mpsc::Receiver<ChannelEvent>>,
    debug_http_port_tx: tokio::sync::watch::Sender<u16>,
    debug_http_port_rx: tokio::sync::watch::Receiver<u16>,
    boot_notify: Option<Arc<tokio::sync::Notify>>,
    health: Arc<RwLock<HealthSnapshot>>,
}

struct ManagerSlot {
    name: String,
    join_handle: Option<tokio::task::JoinHandle<Result<(), ManagerError>>>,
    lifecycle: SlotLifecycle,
}

const MANAGER_ORDER: [&str; 3] = ["tools", "agents", "channels"];

async fn shutdown_signal() {
    use tokio::signal::unix::SignalKind;

    let mut sigterm =
        tokio::signal::unix::signal(SignalKind::terminate()).expect("failed to register SIGTERM");
    let mut sigint =
        tokio::signal::unix::signal(SignalKind::interrupt()).expect("failed to register SIGINT");

    tokio::select! {
        _ = sigterm.recv() => tracing::info!("received SIGTERM"),
        _ = sigint.recv() => tracing::info!("received SIGINT"),
    }
}

impl Supervisor {
    pub fn new(mut config: ProtoclawConfig) -> Self {
        resolve_all_binary_paths(&mut config);

        let (channel_events_tx, channel_events_rx) =
            tokio::sync::mpsc::channel(protoclaw_core::constants::EVENT_CHANNEL_CAPACITY);
        let (debug_http_port_tx, debug_http_port_rx) = tokio::sync::watch::channel(0u16);
        Self {
            config,
            tools_tx: None,
            agents_cmd_tx: None,
            channels_cmd_tx: None,
            channel_events_tx: Some(channel_events_tx),
            channel_events_rx: Some(channel_events_rx),
            debug_http_port_tx,
            debug_http_port_rx,
            boot_notify: None,
            health: Arc::new(RwLock::new(HealthSnapshot::default())),
        }
    }

    pub fn debug_http_port_rx(&self) -> tokio::sync::watch::Receiver<u16> {
        self.debug_http_port_rx.clone()
    }

    #[cfg(test)]
    fn with_boot_notify(mut self, notify: Arc<tokio::sync::Notify>) -> Self {
        self.boot_notify = Some(notify);
        self
    }

    pub async fn run(self) -> Result<(), SupervisorError> {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let signal_handle = tokio::spawn(async move {
            shutdown_signal().await;
            cancel_clone.cancel();
        });

        let result = self.run_with_cancel(cancel.clone()).await;

        signal_handle.abort();
        result
    }

    pub async fn run_with_cancel(
        mut self,
        cancel: CancellationToken,
    ) -> Result<(), SupervisorError> {
        let per_manager_timeout =
            Duration::from_secs(self.config.supervisor.shutdown_timeout_secs / 3);
        let health_interval_secs = self.config.supervisor.health_check_interval_secs;
        let max_restarts = self.config.supervisor.max_restarts;
        let restart_window = Duration::from_secs(self.config.supervisor.restart_window_secs);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            slots.push(ManagerSlot {
                name: name.to_string(),
                join_handle: None,
                lifecycle: SlotLifecycle::new(
                    &cancel,
                    ExponentialBackoff::default(),
                    CrashTracker::new(max_restarts, restart_window),
                ),
            });
        }

        if let Err(e) = self.boot_managers(&mut slots).await {
            self.shutdown_ordered(&mut slots, per_manager_timeout).await;
            return Err(e);
        }

        tracing::info!("all managers booted");

        if let Some(notify) = &self.boot_notify {
            notify.notify_one();
        }

        admin_server::start(self.config.supervisor.admin_port, self.health.clone()).await;

        let mut health_interval = tokio::time::interval(Duration::from_secs(health_interval_secs));
        health_interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("shutdown signal received");
                    self.shutdown_ordered(&mut slots, per_manager_timeout).await;
                    break;
                }
                _ = health_interval.tick() => {
                    self.check_and_restart_managers(&mut slots, &cancel).await;
                }
            }
        }

        tracing::info!("supervisor exiting");
        Ok(())
    }

    async fn boot_managers(&mut self, slots: &mut [ManagerSlot]) -> Result<(), SupervisorError> {
        let (tools_tx, tools_rx) = tokio::sync::mpsc::channel::<ToolsCommand>(
            protoclaw_core::constants::CMD_CHANNEL_CAPACITY,
        );
        self.tools_tx = Some(tools_tx.clone());
        let mut tools_rx = Some(tools_rx);
        let mut channel_events_tx = self.channel_events_tx.take();
        let mut channel_events_rx = self.channel_events_rx.take();

        for slot in slots.iter_mut() {
            tracing::info!(manager = %slot.name, "booting");

            let ce_tx = if slot.name == "agents" {
                channel_events_tx.take()
            } else {
                None
            };
            let ce_rx = if slot.name == "channels" {
                channel_events_rx.take()
            } else {
                None
            };

            let mut manager = create_manager(
                &slot.name,
                &self.config,
                &tools_tx,
                tools_rx.take(),
                self.agents_cmd_tx.as_ref(),
                ce_tx,
                ce_rx,
            );

            if slot.name == "agents"
                && let ManagerKind::Agents(ref m) = manager
            {
                self.agents_cmd_tx = Some(m.command_sender());
            }

            if let Err(e) = manager.start().await {
                tracing::error!(manager = %slot.name, error = %e, "boot failed");
                return Err(SupervisorError::BootFailure {
                    manager: slot.name.clone(),
                    source: e,
                });
            }

            // After channels manager starts, grab port discovery and command sender
            if slot.name == "channels"
                && let ManagerKind::Channels(ref m) = manager
            {
                self.channels_cmd_tx = Some(m.command_sender());
                // Forward port discovery from channel subprocess to supervisor's watch
                if let Some(mut channel_port_rx) = m.channel_port("debug-http") {
                    let port_tx = self.debug_http_port_tx.clone();
                    tokio::spawn(async move {
                        while channel_port_rx.changed().await.is_ok() {
                            let port = *channel_port_rx.borrow();
                            if port != 0 {
                                let _ = port_tx.send(port);
                                break;
                            }
                        }
                    });
                }
            }

            let token = slot.lifecycle.cancel_token.clone();
            let handle = tokio::spawn(async move { manager.run(token).await });
            slot.join_handle = Some(handle);

            tracing::info!(manager = %slot.name, "booted");
        }
        Ok(())
    }

    async fn shutdown_ordered(&self, slots: &mut [ManagerSlot], per_manager_timeout: Duration) {
        for slot in slots.iter_mut().rev() {
            tracing::info!(manager = %slot.name, "shutting down");
            slot.lifecycle.cancel_token.cancel();

            if let Some(handle) = slot.join_handle.take() {
                match tokio::time::timeout(per_manager_timeout, handle).await {
                    Ok(Ok(Ok(()))) => {
                        tracing::info!(manager = %slot.name, "shut down cleanly");
                    }
                    Ok(Ok(Err(e))) => {
                        tracing::error!(manager = %slot.name, error = %e, "error during shutdown");
                    }
                    Ok(Err(e)) => {
                        tracing::error!(manager = %slot.name, error = %e, "panicked during shutdown");
                    }
                    Err(_) => {
                        tracing::warn!(manager = %slot.name, "shutdown timed out, aborting");
                        slot.join_handle.as_ref().inspect(|h| h.abort());
                    }
                }
            }
        }
    }

    async fn check_and_restart_managers(
        &mut self,
        slots: &mut [ManagerSlot],
        root_cancel: &CancellationToken,
    ) {
        for slot in slots.iter_mut() {
            let needs_restart = matches!(&slot.join_handle, Some(handle) if handle.is_finished());

            if !needs_restart {
                continue;
            }

            if let Some(handle) = slot.join_handle.take() {
                match handle.await {
                    Ok(Ok(())) => {
                        tracing::info!(manager = %slot.name, "exited cleanly, not restarting");
                        continue;
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(manager = %slot.name, error = %e, "crashed");
                    }
                    Err(e) => {
                        tracing::error!(manager = %slot.name, error = %e, "panicked");
                    }
                }
            }

            match slot.lifecycle.record_crash_and_check() {
                CrashAction::Disabled => {
                    tracing::error!(
                        manager = %slot.name,
                        max_restarts = self.config.supervisor.max_restarts,
                        restart_window_secs = self.config.supervisor.restart_window_secs,
                        "crash loop detected, marking disabled — restart circuit breaker tripped"
                    );
                    if slot.name == "agents" || slot.name == "channels" {
                        tracing::error!(
                            manager = %slot.name,
                            max_restarts = self.config.supervisor.max_restarts,
                            restart_window_secs = self.config.supervisor.restart_window_secs,
                            "critical manager crash loop — initiating shutdown"
                        );
                        root_cancel.cancel();
                    }
                    continue;
                }
                CrashAction::RestartAfter(delay) => {
                    tracing::info!(manager = %slot.name, delay_ms = delay.as_millis(), "restarting after backoff");
                    tokio::time::sleep(delay).await;
                }
            }

            let tools_tx = self.tools_tx.clone().unwrap_or_else(|| {
                let (tx, _) = tokio::sync::mpsc::channel::<ToolsCommand>(
                    protoclaw_core::constants::CMD_CHANNEL_CAPACITY,
                );
                tx
            });
            let tools_rx = if slot.name == "tools" {
                let (new_tx, rx) = tokio::sync::mpsc::channel::<ToolsCommand>(
                    protoclaw_core::constants::CMD_CHANNEL_CAPACITY,
                );
                self.tools_tx = Some(new_tx);
                Some(rx)
            } else {
                None
            };
            let mut manager = create_manager(
                &slot.name,
                &self.config,
                &tools_tx,
                tools_rx,
                self.agents_cmd_tx.as_ref(),
                None,
                None,
            );
            if let Err(e) = manager.start().await {
                tracing::error!(manager = %slot.name, error = %e, "restart boot failed");
                continue;
            }

            slot.lifecycle.cancel_token = root_cancel.child_token();
            let token = slot.lifecycle.cancel_token.clone();
            let handle = tokio::spawn(async move { manager.run(token).await });
            slot.join_handle = Some(handle);

            metrics::counter!("protoclaw_manager_restarts_total", "manager" => slot.name.clone())
                .increment(1);

            tracing::info!(manager = %slot.name, "restarted");
        }

        self.refresh_health_snapshot(slots).await;
    }

    async fn refresh_health_snapshot(&self, slots: &[ManagerSlot]) {
        let agents_running = slots
            .iter()
            .any(|s| s.name == "agents" && s.join_handle.is_some());
        let channels_running = slots
            .iter()
            .any(|s| s.name == "channels" && s.join_handle.is_some());

        let agents: Vec<protoclaw_core::AgentHealth> = self
            .config
            .agents_manager
            .agents
            .keys()
            .map(|name| protoclaw_core::AgentHealth {
                name: name.clone(),
                connected: agents_running,
                session_count: 0,
            })
            .collect();

        let channels: Vec<String> = if channels_running {
            self.config
                .channels_manager
                .channels
                .keys()
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let mcp_servers: Vec<String> = self
            .config
            .tools_manager
            .tools
            .iter()
            .filter(|(_, t)| t.enabled && t.tool_type == protoclaw_config::ToolType::Mcp)
            .map(|(name, _)| name.clone())
            .collect();

        let degraded = agents.iter().any(|a| !a.connected);
        let status = if degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut snapshot = self.health.write().await;
        *snapshot = HealthSnapshot {
            status,
            agents,
            channels,
            mcp_servers,
        };

        metrics::gauge!("protoclaw_agents_connected")
            .set(snapshot.agents.iter().filter(|a| a.connected).count() as f64);
        metrics::gauge!("protoclaw_channels_running").set(snapshot.channels.len() as f64);
    }
}

fn create_manager(
    name: &str,
    config: &ProtoclawConfig,
    tools_tx: &tokio::sync::mpsc::Sender<ToolsCommand>,
    tools_rx: Option<tokio::sync::mpsc::Receiver<ToolsCommand>>,
    agents_cmd_tx: Option<&tokio::sync::mpsc::Sender<AgentsCommand>>,
    channel_events_tx: Option<tokio::sync::mpsc::Sender<ChannelEvent>>,
    channel_events_rx: Option<tokio::sync::mpsc::Receiver<ChannelEvent>>,
) -> ManagerKind {
    match name {
        "tools" => {
            let m = ToolsManager::new(
                config.tools_manager.tools.clone(),
                config.tools_manager.tools_server_host.clone(),
            )
            .with_cmd_rx(tools_rx.expect("tools_rx required for tools manager"));
            ManagerKind::Tools(m)
        }
        "agents" => {
            let handle = protoclaw_core::ManagerHandle::new(tools_tx.clone());
            let mut agents = AgentsManager::new(config.agents_manager.clone(), handle)
                .with_log_level(config.log_level.clone());
            if let Some(tx) = channel_events_tx {
                agents = agents.with_channels_sender(tx);
            }
            ManagerKind::Agents(Box::new(agents))
        }
        "channels" => {
            let tx = agents_cmd_tx.expect("agents_cmd_tx required for channels manager");
            let agents_handle = ManagerHandle::new(tx.clone());
            let default_agent = config.default_agent_name().unwrap_or("default").to_string();
            let mut cm = ChannelsManager::new(
                config.channels_manager.channels.clone(),
                config.channels_manager.init_timeout_secs,
                config.channels_manager.exit_timeout_secs,
                default_agent,
            )
            .with_agents_handle(agents_handle)
            .with_permission_timeout(config.supervisor.permission_timeout_secs)
            .with_log_level(config.log_level.clone());
            if let Some(rx) = channel_events_rx {
                cm = cm.with_channel_events_rx(rx);
            }
            ManagerKind::Channels(cm)
        }
        _ => unreachable!("unknown manager: {name}"),
    }
}

enum ManagerKind {
    Tools(ToolsManager),
    Agents(Box<AgentsManager>),
    Channels(ChannelsManager),
}

impl ManagerKind {
    async fn start(&mut self) -> Result<(), ManagerError> {
        match self {
            Self::Tools(m) => m.start().await,
            Self::Agents(m) => m.start().await,
            Self::Channels(m) => m.start().await,
        }
    }

    async fn run(self, cancel: CancellationToken) -> Result<(), ManagerError> {
        match self {
            Self::Tools(m) => m.run(cancel).await,
            Self::Agents(m) => m.run(cancel).await,
            Self::Channels(m) => m.run(cancel).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn test_config() -> ProtoclawConfig {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mock_agent = std::path::Path::new(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("mock-agent");

        let mut agents = std::collections::HashMap::new();
        agents.insert(
            "default".to_string(),
            protoclaw_config::AgentConfig {
                workspace: protoclaw_config::WorkspaceConfig::Local(
                    protoclaw_config::LocalWorkspaceConfig {
                        binary: mock_agent.to_string_lossy().to_string().into(),
                        working_dir: None,
                        env: std::collections::HashMap::new(),
                    },
                ),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: std::collections::HashMap::new(),
            },
        );

        ProtoclawConfig {
            agents_manager: protoclaw_config::AgentsManagerConfig {
                agents,
                ..Default::default()
            },
            channels_manager: protoclaw_config::ChannelsManagerConfig::default(),
            tools_manager: protoclaw_config::ToolsManagerConfig::default(),
            supervisor: protoclaw_config::SupervisorConfig {
                shutdown_timeout_secs: 3,
                health_check_interval_secs: 60,
                max_restarts: 5,
                restart_window_secs: 60,
                admin_port: 3000,
                permission_timeout_secs: None,
            },
            log_level: "info".into(),
            log_format: protoclaw_config::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
        }
    }

    #[tokio::test]
    async fn when_debug_http_port_rx_called_then_returns_watch_receiver_with_initial_zero() {
        let sup = Supervisor::new(test_config());
        let rx = sup.debug_http_port_rx();
        assert_eq!(*rx.borrow(), 0, "initial port value should be 0");
    }

    #[tokio::test]
    async fn when_debug_http_port_rx_called_twice_then_both_receivers_see_same_value() {
        let sup = Supervisor::new(test_config());
        let rx1 = sup.debug_http_port_rx();
        let rx2 = sup.debug_http_port_rx();
        assert_eq!(*rx1.borrow(), *rx2.borrow());
    }

    #[tokio::test]
    async fn when_supervisor_created_then_debug_http_port_rx_is_cloneable() {
        let sup = Supervisor::new(test_config());
        let rx = sup.debug_http_port_rx();
        let _rx2 = rx.clone();
    }

    #[tokio::test]
    async fn when_supervisor_created_then_debug_http_port_tx_sends_and_rx_sees_update() {
        let sup = Supervisor::new(test_config());
        let mut rx = sup.debug_http_port_rx();
        sup.debug_http_port_tx.send(8080).unwrap();
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), 8080);
    }

    #[tokio::test]
    async fn when_supervisor_run_with_cancel_called_then_boots_all_managers_and_shuts_down_cleanly()
    {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        let boot_signal = Arc::new(tokio::sync::Notify::new());
        let boot_wait = boot_signal.clone();

        tokio::spawn(async move {
            boot_wait.notified().await;
            c.cancel();
        });

        let sup = Supervisor::new(test_config()).with_boot_notify(boot_signal);
        let result = sup.run_with_cancel(cancel).await;
        assert!(
            result.is_ok(),
            "supervisor should boot and shut down cleanly: {result:?}"
        );
    }

    #[tokio::test]
    async fn when_boot_managers_called_then_slots_ordered_tools_agents_channels() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let mut sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            slots.push(ManagerSlot {
                name: name.to_string(),
                join_handle: None,
                lifecycle: SlotLifecycle::new(
                    &cancel,
                    ExponentialBackoff::default(),
                    CrashTracker::default(),
                ),
            });
        }

        sup.boot_managers(&mut slots).await.unwrap();

        assert_eq!(slots[0].name, "tools");
        assert_eq!(slots[1].name, "agents");
        assert_eq!(slots[2].name, "channels");
        assert!(slots.iter().all(|s| s.join_handle.is_some()));

        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;
        drop(log);
    }

    #[tokio::test]
    async fn when_shutdown_ordered_called_then_cancels_channels_then_agents_then_tools() {
        let cancel = CancellationToken::new();
        let shutdown_order: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            let lifecycle = SlotLifecycle::new(
                &cancel,
                ExponentialBackoff::default(),
                CrashTracker::default(),
            );
            let order = shutdown_order.clone();
            let n = name.to_string();
            let t = lifecycle.cancel_token.clone();
            let handle = tokio::spawn(async move {
                t.cancelled().await;
                order.lock().unwrap().push(n);
                Ok::<(), ManagerError>(())
            });
            slots.push(ManagerSlot {
                name: name.to_string(),
                join_handle: Some(handle),
                lifecycle,
            });
        }

        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;

        let order = shutdown_order.lock().unwrap();
        assert_eq!(*order, vec!["channels", "agents", "tools"]);
    }

    #[tokio::test]
    async fn when_shutdown_ordered_called_with_timeout_then_completes_within_timeout() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            let lifecycle = SlotLifecycle::new(
                &cancel,
                ExponentialBackoff::default(),
                CrashTracker::default(),
            );
            let t = lifecycle.cancel_token.clone();
            let handle = tokio::spawn(async move {
                t.cancelled().await;
                Ok::<(), ManagerError>(())
            });
            slots.push(ManagerSlot {
                name: name.to_string(),
                join_handle: Some(handle),
                lifecycle,
            });
        }

        let start = tokio::time::Instant::now();
        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn given_stuck_manager_when_shutdown_ordered_called_then_aborts_after_timeout() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_millis(50);

        let mut slots = Vec::with_capacity(1);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok::<(), ManagerError>(())
        });
        slots.push(ManagerSlot {
            name: "stuck".to_string(),
            join_handle: Some(handle),
            lifecycle: SlotLifecycle::new(
                &cancel,
                ExponentialBackoff::default(),
                CrashTracker::default(),
            ),
        });

        let start = tokio::time::Instant::now();
        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn given_crashed_manager_when_check_and_restart_called_then_manager_is_restarted() {
        let cancel = CancellationToken::new();

        let mut config = test_config();
        config.supervisor.health_check_interval_secs = 1;
        let mut sup = Supervisor::new(config);

        let mut slots = Vec::with_capacity(1);
        let handle =
            tokio::spawn(
                async move { Err::<(), ManagerError>(ManagerError::Internal("crash".into())) },
            );
        slots.push(ManagerSlot {
            name: "tools".to_string(),
            join_handle: Some(handle),
            lifecycle: SlotLifecycle::new(
                &cancel,
                ExponentialBackoff::default(),
                CrashTracker::new(5, Duration::from_secs(60)),
            ),
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        sup.check_and_restart_managers(&mut slots, &cancel).await;

        assert!(
            slots[0].join_handle.is_some(),
            "manager should be restarted"
        );
        assert_eq!(slots[0].lifecycle.backoff.attempts(), 1);

        cancel.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn given_manager_in_crash_loop_when_check_and_restart_called_then_not_restarted() {
        let cancel = CancellationToken::new();
        let mut sup = Supervisor::new(test_config());

        let mut slots = Vec::with_capacity(1);
        let handle =
            tokio::spawn(
                async move { Err::<(), ManagerError>(ManagerError::Internal("crash".into())) },
            );

        let mut crash_tracker = CrashTracker::new(3, Duration::from_secs(60));
        crash_tracker.record_crash();
        crash_tracker.record_crash();

        let lifecycle = SlotLifecycle::new(&cancel, ExponentialBackoff::default(), crash_tracker);
        slots.push(ManagerSlot {
            name: "tools".to_string(),
            join_handle: Some(handle),
            lifecycle,
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        sup.check_and_restart_managers(&mut slots, &cancel).await;

        assert!(
            slots[0].join_handle.is_none(),
            "crash-looping manager should NOT be restarted"
        );
    }

    #[tokio::test]
    async fn when_root_cancellation_token_cancelled_then_all_child_tokens_cancelled() {
        let root = CancellationToken::new();
        let child1 = root.child_token();
        let child2 = root.child_token();
        let child3 = root.child_token();

        let h1 = tokio::spawn({
            let t = child1.clone();
            async move {
                t.cancelled().await;
                "child1"
            }
        });
        let h2 = tokio::spawn({
            let t = child2.clone();
            async move {
                t.cancelled().await;
                "child2"
            }
        });
        let h3 = tokio::spawn({
            let t = child3.clone();
            async move {
                t.cancelled().await;
                "child3"
            }
        });

        root.cancel();

        assert_eq!(h1.await.unwrap(), "child1");
        assert_eq!(h2.await.unwrap(), "child2");
        assert_eq!(h3.await.unwrap(), "child3");
    }

    #[tokio::test]
    async fn when_supervisor_run_with_cancel_signalled_after_boot_then_exits_cleanly_within_timeout()
     {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        let boot_signal = Arc::new(tokio::sync::Notify::new());
        let boot_wait = boot_signal.clone();

        tokio::spawn(async move {
            boot_wait.notified().await;
            c.cancel();
        });

        let sup = Supervisor::new(test_config()).with_boot_notify(boot_signal);
        let start = tokio::time::Instant::now();
        let result = sup.run_with_cancel(cancel).await;
        assert!(result.is_ok());
        assert!(start.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn given_repeated_crashes_when_check_and_restart_called_then_backoff_increments() {
        let cancel = CancellationToken::new();
        let mut sup = Supervisor::new(test_config());

        let mut slots = Vec::with_capacity(1);
        let handle =
            tokio::spawn(async { Err::<(), ManagerError>(ManagerError::Internal("crash".into())) });
        slots.push(ManagerSlot {
            name: "tools".to_string(),
            join_handle: Some(handle),
            lifecycle: SlotLifecycle::new(
                &cancel,
                ExponentialBackoff::default(),
                CrashTracker::new(10, Duration::from_secs(60)),
            ),
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        sup.check_and_restart_managers(&mut slots, &cancel).await;
        assert_eq!(slots[0].lifecycle.backoff.attempts(), 1);

        cancel.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }

        let handle2 = tokio::spawn(async {
            Err::<(), ManagerError>(ManagerError::Internal("crash2".into()))
        });
        slots[0].join_handle = Some(handle2);
        slots[0].lifecycle.cancel_token = CancellationToken::new();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let cancel2 = CancellationToken::new();
        sup.check_and_restart_managers(&mut slots, &cancel2).await;
        assert_eq!(slots[0].lifecycle.backoff.attempts(), 2);

        cancel2.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }
    }
}
