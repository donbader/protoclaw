use std::sync::Arc;
use std::time::Duration;

use protoclaw_config::{resolve_binary_path, ProtoclawConfig};
use protoclaw_core::{CrashTracker, ExponentialBackoff, Manager, ManagerError, ManagerHandle};
use tokio_util::sync::CancellationToken;

use protoclaw_agents::{AgentsCommand, AgentsManager};
use protoclaw_channels::{ChannelsCommand, ChannelsManager};
use protoclaw_core::ChannelEvent;
use protoclaw_tools::{ToolsCommand, ToolsManager};

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
}

struct ManagerSlot {
    name: String,
    cancel_token: CancellationToken,
    join_handle: Option<tokio::task::JoinHandle<Result<(), ManagerError>>>,
    backoff: ExponentialBackoff,
    crash_tracker: CrashTracker,
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
        let extensions_dir = config.extensions_dir.clone();
        for (_, agent) in &mut config.agents_manager.agents {
            agent.binary = resolve_binary_path(&agent.binary, &extensions_dir);
        }
        for (_, ch) in &mut config.channels_manager.channels {
            ch.binary = resolve_binary_path(&ch.binary, &extensions_dir);
        }
        for (_, tool) in &mut config.tools_manager.tools {
            if let Some(ref mut bin) = tool.binary {
                *bin = resolve_binary_path(bin, &extensions_dir);
            }
        }

        let (channel_events_tx, channel_events_rx) = tokio::sync::mpsc::channel(64);
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

    pub async fn run(self) -> anyhow::Result<()> {
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

    pub async fn run_with_cancel(mut self, cancel: CancellationToken) -> anyhow::Result<()> {
        let per_manager_timeout = Duration::from_secs(
            self.config.supervisor.shutdown_timeout_secs / 3,
        );
        let health_interval_secs = self.config.supervisor.health_check_interval_secs;
        let max_restarts = self.config.supervisor.max_restarts;
        let restart_window = Duration::from_secs(self.config.supervisor.restart_window_secs);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            let child_token = cancel.child_token();
            slots.push(ManagerSlot {
                name: name.to_string(),
                cancel_token: child_token,
                join_handle: None,
                backoff: ExponentialBackoff::default(),
                crash_tracker: CrashTracker::new(max_restarts, restart_window),
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

        let mut health_interval = tokio::time::interval(
            Duration::from_secs(health_interval_secs),
        );
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

    async fn boot_managers(&mut self, slots: &mut [ManagerSlot]) -> anyhow::Result<()> {
        let (tools_tx, tools_rx) = tokio::sync::mpsc::channel::<ToolsCommand>(16);
        self.tools_tx = Some(tools_tx.clone());
        let mut tools_rx = Some(tools_rx);
        let mut channel_events_tx = self.channel_events_tx.take();
        let mut channel_events_rx = self.channel_events_rx.take();

        for slot in slots.iter_mut() {
            tracing::info!(manager = %slot.name, "booting");

            let ce_tx = if slot.name == "agents" { channel_events_tx.take() } else { None };
            let ce_rx = if slot.name == "channels" { channel_events_rx.take() } else { None };

            let mut manager = create_manager(
                &slot.name,
                &self.config,
                &tools_tx,
                tools_rx.take(),
                self.agents_cmd_tx.as_ref(),
                ce_tx,
                ce_rx,
            );

            if slot.name == "agents" {
                if let ManagerKind::Agents(ref m) = manager {
                    self.agents_cmd_tx = Some(m.command_sender());
                }
            }

            if let Err(e) = manager.start().await {
                tracing::error!(manager = %slot.name, error = %e, "boot failed");
                return Err(anyhow::anyhow!("failed to boot {}: {e}", slot.name));
            }

            // After channels manager starts, grab port discovery and command sender
            if slot.name == "channels" {
                if let ManagerKind::Channels(ref m) = manager {
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
            }

            let token = slot.cancel_token.clone();
            let handle = tokio::spawn(async move { manager.run(token).await });
            slot.join_handle = Some(handle);

            tracing::info!(manager = %slot.name, "booted");
        }
        Ok(())
    }

    async fn shutdown_ordered(&self, slots: &mut [ManagerSlot], per_manager_timeout: Duration) {
        for slot in slots.iter_mut().rev() {
            tracing::info!(manager = %slot.name, "shutting down");
            slot.cancel_token.cancel();

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

            slot.crash_tracker.record_crash();

            if slot.crash_tracker.is_crash_loop() {
                tracing::error!(
                    manager = %slot.name,
                    "crash loop detected, not restarting"
                );
                continue;
            }

            let delay = slot.backoff.next_delay();
            tracing::info!(manager = %slot.name, delay_ms = delay.as_millis(), "restarting after backoff");
            tokio::time::sleep(delay).await;

            let tools_tx = self.tools_tx.clone().unwrap_or_else(|| {
                let (tx, _) = tokio::sync::mpsc::channel::<ToolsCommand>(16);
                tx
            });
            let tools_rx = if slot.name == "tools" {
                let (new_tx, rx) = tokio::sync::mpsc::channel::<ToolsCommand>(16);
                self.tools_tx = Some(new_tx);
                Some(rx)
            } else {
                None
            };
            let mut manager = create_manager(&slot.name, &self.config, &tools_tx, tools_rx, self.agents_cmd_tx.as_ref(), None, None);
            if let Err(e) = manager.start().await {
                tracing::error!(manager = %slot.name, error = %e, "restart boot failed");
                continue;
            }

            slot.cancel_token = root_cancel.child_token();
            let token = slot.cancel_token.clone();
            let handle = tokio::spawn(async move { manager.run(token).await });
            slot.join_handle = Some(handle);

            tracing::info!(manager = %slot.name, "restarted");
        }
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
            let m = ToolsManager::new(config.tools_manager.tools.clone())
                .with_cmd_rx(tools_rx.expect("tools_rx required for tools manager"));
            ManagerKind::Tools(m)
        }
        "agents" => {
            let handle = protoclaw_core::ManagerHandle::new(tools_tx.clone());
            let mut agents = AgentsManager::new(config.agents_manager.agents.clone(), handle);
            if let Some(tx) = channel_events_tx {
                agents = agents.with_channels_sender(tx);
            }
            ManagerKind::Agents(Box::new(agents))
        }
        "channels" => {
            let tx = agents_cmd_tx.expect("agents_cmd_tx required for channels manager");
            let agents_handle = ManagerHandle::new(tx.clone());
            let default_agent = config.default_agent_name()
                .unwrap_or("default")
                .to_string();
            let mut cm = ChannelsManager::new(config.channels_manager.channels.clone(), default_agent)
                .with_agents_handle(agents_handle);
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
    #[allow(dead_code)]
    fn name(&self) -> &str {
        match self {
            Self::Tools(m) => m.name(),
            Self::Agents(m) => m.name(),
            Self::Channels(m) => m.name(),
        }
    }

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
        agents.insert("default".to_string(), protoclaw_config::AgentConfig {
            binary: mock_agent.to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            env: std::collections::HashMap::new(),
            working_dir: None,
            tools: vec![],
        });

        ProtoclawConfig {
            agents_manager: protoclaw_config::AgentsManagerConfig { agents },
            channels_manager: protoclaw_config::ChannelsManagerConfig::default(),
            tools_manager: protoclaw_config::ToolsManagerConfig::default(),
            supervisor: protoclaw_config::SupervisorConfig {
                shutdown_timeout_secs: 3,
                health_check_interval_secs: 60,
                max_restarts: 5,
                restart_window_secs: 60,
            },
            log_level: "info".into(),
            extensions_dir: "/usr/local/bin".into(),
        }
    }

    #[tokio::test]
    async fn supervisor_boots_and_shuts_down() {
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
        assert!(result.is_ok(), "supervisor should boot and shut down cleanly: {result:?}");
    }

    #[tokio::test]
    async fn boot_order_is_tools_agents_channels() {
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let mut sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            slots.push(ManagerSlot {
                name: name.to_string(),
                cancel_token: cancel.child_token(),
                join_handle: None,
                backoff: ExponentialBackoff::default(),
                crash_tracker: CrashTracker::default(),
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
    async fn shutdown_order_is_channels_agents_tools() {
        let cancel = CancellationToken::new();
        let shutdown_order: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            let token = cancel.child_token();
            let order = shutdown_order.clone();
            let n = name.to_string();
            let t = token.clone();
            let handle = tokio::spawn(async move {
                t.cancelled().await;
                order.lock().unwrap().push(n);
                Ok::<(), ManagerError>(())
            });
            slots.push(ManagerSlot {
                name: name.to_string(),
                cancel_token: token,
                join_handle: Some(handle),
                backoff: ExponentialBackoff::default(),
                crash_tracker: CrashTracker::default(),
            });
        }

        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;

        let order = shutdown_order.lock().unwrap();
        assert_eq!(*order, vec!["channels", "agents", "tools"]);
    }

    #[tokio::test]
    async fn shutdown_completes_within_timeout() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_secs(1);

        let mut slots = Vec::with_capacity(3);
        for &name in &MANAGER_ORDER {
            let token = cancel.child_token();
            let t = token.clone();
            let handle = tokio::spawn(async move {
                t.cancelled().await;
                Ok::<(), ManagerError>(())
            });
            slots.push(ManagerSlot {
                name: name.to_string(),
                cancel_token: token,
                join_handle: Some(handle),
                backoff: ExponentialBackoff::default(),
                crash_tracker: CrashTracker::default(),
            });
        }

        let start = tokio::time::Instant::now();
        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn shutdown_aborts_on_timeout() {
        let cancel = CancellationToken::new();
        let sup = Supervisor::new(test_config());
        let per_manager_timeout = Duration::from_millis(50);

        let mut slots = Vec::with_capacity(1);
        let token = cancel.child_token();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok::<(), ManagerError>(())
        });
        slots.push(ManagerSlot {
            name: "stuck".to_string(),
            cancel_token: token,
            join_handle: Some(handle),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::default(),
        });

        let start = tokio::time::Instant::now();
        sup.shutdown_ordered(&mut slots, per_manager_timeout).await;
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn crash_triggers_restart() {
        let cancel = CancellationToken::new();

        let mut config = test_config();
        config.supervisor.health_check_interval_secs = 1;
        let mut sup = Supervisor::new(config);

        let mut slots = Vec::with_capacity(1);
        let token = cancel.child_token();
        let handle = tokio::spawn(async move {
            Err::<(), ManagerError>(ManagerError::Internal("crash".into()))
        });
        slots.push(ManagerSlot {
            name: "tools".to_string(),
            cancel_token: token,
            join_handle: Some(handle),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::new(5, Duration::from_secs(60)),
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        sup.check_and_restart_managers(&mut slots, &cancel).await;

        assert!(slots[0].join_handle.is_some(), "manager should be restarted");
        assert_eq!(slots[0].backoff.attempts(), 1);

        cancel.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn crash_loop_stops_restarting() {
        let cancel = CancellationToken::new();
        let mut sup = Supervisor::new(test_config());

        let mut slots = Vec::with_capacity(1);
        let token = cancel.child_token();
        let handle = tokio::spawn(async move {
            Err::<(), ManagerError>(ManagerError::Internal("crash".into()))
        });

        let mut crash_tracker = CrashTracker::new(3, Duration::from_secs(60));
        crash_tracker.record_crash();
        crash_tracker.record_crash();

        slots.push(ManagerSlot {
            name: "tools".to_string(),
            cancel_token: token,
            join_handle: Some(handle),
            backoff: ExponentialBackoff::default(),
            crash_tracker,
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        sup.check_and_restart_managers(&mut slots, &cancel).await;

        assert!(
            slots[0].join_handle.is_none(),
            "crash-looping manager should NOT be restarted"
        );
    }

    #[tokio::test]
    async fn cancellation_token_hierarchy_cascades() {
        let root = CancellationToken::new();
        let child1 = root.child_token();
        let child2 = root.child_token();
        let child3 = root.child_token();

        let h1 = tokio::spawn({
            let t = child1.clone();
            async move { t.cancelled().await; "child1" }
        });
        let h2 = tokio::spawn({
            let t = child2.clone();
            async move { t.cancelled().await; "child2" }
        });
        let h3 = tokio::spawn({
            let t = child3.clone();
            async move { t.cancelled().await; "child3" }
        });

        root.cancel();

        assert_eq!(h1.await.unwrap(), "child1");
        assert_eq!(h2.await.unwrap(), "child2");
        assert_eq!(h3.await.unwrap(), "child3");
    }

    #[tokio::test]
    async fn supervisor_run_exits_on_cancel() {
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
    async fn restart_uses_exponential_backoff() {
        let cancel = CancellationToken::new();
        let mut sup = Supervisor::new(test_config());

        let mut slots = Vec::with_capacity(1);
        let token = cancel.child_token();
        let handle = tokio::spawn(async {
            Err::<(), ManagerError>(ManagerError::Internal("crash".into()))
        });
        slots.push(ManagerSlot {
            name: "tools".to_string(),
            cancel_token: token,
            join_handle: Some(handle),
            backoff: ExponentialBackoff::default(),
            crash_tracker: CrashTracker::new(10, Duration::from_secs(60)),
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        sup.check_and_restart_managers(&mut slots, &cancel).await;
        assert_eq!(slots[0].backoff.attempts(), 1);

        cancel.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }

        let handle2 = tokio::spawn(async {
            Err::<(), ManagerError>(ManagerError::Internal("crash2".into()))
        });
        slots[0].join_handle = Some(handle2);
        slots[0].cancel_token = CancellationToken::new();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let cancel2 = CancellationToken::new();
        sup.check_and_restart_managers(&mut slots, &cancel2).await;
        assert_eq!(slots[0].backoff.attempts(), 2);

        cancel2.cancel();
        if let Some(h) = slots[0].join_handle.take() {
            let _ = h.await;
        }
    }
}
