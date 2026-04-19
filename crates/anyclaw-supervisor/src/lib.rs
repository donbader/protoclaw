#![warn(missing_docs)]

//! Supervisor: boot/shutdown orchestration, health monitoring, and crash recovery
//! for the three-manager architecture (tools → agents → channels).

use std::sync::Arc;
use std::time::Duration;

use anyclaw_config::{AnyclawConfig, resolve_all_binary_paths};
use anyclaw_core::{CrashTracker, ExponentialBackoff, HealthSnapshot, SlotLifecycle};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use anyclaw_core::ChannelEvent;
use anyclaw_tools::ToolsCommand;

use crate::factory::ManagerKind;

/// Admin HTTP server exposing `/health` and `/metrics` endpoints.
pub mod admin_server;

/// Manager creation factory, session store builder, and ManagerKind dispatch enum.
pub(crate) mod factory;
/// Health snapshot refresh and crash-restart monitoring.
pub(crate) mod health;
/// Signal handling and ordered manager shutdown.
pub(crate) mod shutdown;

/// Errors from the supervisor layer.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// A manager failed during its `start()` phase at boot time.
    #[error("failed to boot manager '{manager}': {source}")]
    BootFailure {
        /// Name of the manager that failed.
        manager: String,
        /// The underlying manager error.
        #[source]
        source: anyclaw_core::ManagerError,
    },
}

/// Orchestrates the three managers: boots them in order, monitors health,
/// restarts crashed managers with backoff, and shuts down in reverse order.
pub struct Supervisor {
    config: AnyclawConfig,
    tools_tx: Option<tokio::sync::mpsc::Sender<ToolsCommand>>,
    agents_cmd_tx: Option<tokio::sync::mpsc::Sender<anyclaw_agents::AgentsCommand>>,
    channels_cmd_tx: Option<tokio::sync::mpsc::Sender<anyclaw_channels::ChannelsCommand>>,
    channel_events_tx: Option<tokio::sync::mpsc::Sender<ChannelEvent>>,
    channel_events_rx: Option<tokio::sync::mpsc::Receiver<ChannelEvent>>,
    debug_http_port_tx: tokio::sync::watch::Sender<u16>,
    debug_http_port_rx: tokio::sync::watch::Receiver<u16>,
    boot_notify: Option<Arc<tokio::sync::Notify>>,
    health: Arc<RwLock<HealthSnapshot>>,
}

pub(crate) struct ManagerSlot {
    pub(crate) name: String,
    pub(crate) join_handle: Option<tokio::task::JoinHandle<Result<(), anyclaw_core::ManagerError>>>,
    pub(crate) lifecycle: SlotLifecycle,
}

// LIMITATION: Do not change MANAGER_ORDER
// Boot order tools → agents → channels is load-bearing. Tools must be ready before
// agents request MCP URLs during session/new. Agents must be ready before channels
// start routing inbound messages. Shutdown is reverse: channels stop accepting messages
// first, then agents finish in-flight sessions, then tools shut down. Changing this
// order causes race conditions where agents try to reach tools that aren't started yet,
// or channels route messages to agents that haven't initialized.
// See also: AGENTS.md §Anti-Patterns
pub(crate) const MANAGER_ORDER: [&str; 3] = ["tools", "agents", "channels"];

impl Supervisor {
    /// Create a new supervisor from the given config. Resolves all binary paths at construction.
    pub fn new(mut config: AnyclawConfig) -> Self {
        resolve_all_binary_paths(&mut config);

        let (channel_events_tx, channel_events_rx) =
            tokio::sync::mpsc::channel(anyclaw_core::constants::EVENT_CHANNEL_CAPACITY);
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

    /// Subscribe to debug-http port discovery updates.
    pub fn debug_http_port_rx(&self) -> tokio::sync::watch::Receiver<u16> {
        self.debug_http_port_rx.clone()
    }

    #[cfg(test)]
    fn with_boot_notify(mut self, notify: Arc<tokio::sync::Notify>) -> Self {
        self.boot_notify = Some(notify);
        self
    }

    /// Run the supervisor with automatic SIGTERM/SIGINT handling.
    pub async fn run(self) -> Result<(), SupervisorError> {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let signal_handle = tokio::spawn(async move {
            shutdown::shutdown_signal().await;
            cancel_clone.cancel();
        });

        let result = self.run_with_cancel(cancel.clone()).await;

        signal_handle.abort();
        result
    }

    /// Run the supervisor with an externally-provided cancellation token (used in tests).
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
            anyclaw_core::constants::CMD_CHANNEL_CAPACITY,
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

            let mut manager = factory::create_manager(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn test_config() -> AnyclawConfig {
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
            anyclaw_config::AgentConfig {
                workspace: anyclaw_config::WorkspaceConfig::Local(
                    anyclaw_config::LocalWorkspaceConfig {
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

        AnyclawConfig {
            agents_manager: anyclaw_config::AgentsManagerConfig {
                agents,
                ..Default::default()
            },
            channels_manager: anyclaw_config::ChannelsManagerConfig::default(),
            tools_manager: anyclaw_config::ToolsManagerConfig::default(),
            supervisor: anyclaw_config::SupervisorConfig {
                shutdown_timeout_secs: 3,
                health_check_interval_secs: 60,
                ..Default::default()
            },
            log_level: "info".into(),
            log_format: anyclaw_config::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            session_store: Default::default(),
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
                Ok::<(), anyclaw_core::ManagerError>(())
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
                Ok::<(), anyclaw_core::ManagerError>(())
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
            Ok::<(), anyclaw_core::ManagerError>(())
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
        let handle = tokio::spawn(async move {
            Err::<(), anyclaw_core::ManagerError>(anyclaw_core::ManagerError::Internal(
                "crash".into(),
            ))
        });
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
        let handle = tokio::spawn(async move {
            Err::<(), anyclaw_core::ManagerError>(anyclaw_core::ManagerError::Internal(
                "crash".into(),
            ))
        });

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
        let handle = tokio::spawn(async {
            Err::<(), anyclaw_core::ManagerError>(anyclaw_core::ManagerError::Internal(
                "crash".into(),
            ))
        });
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
            Err::<(), anyclaw_core::ManagerError>(anyclaw_core::ManagerError::Internal(
                "crash2".into(),
            ))
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
