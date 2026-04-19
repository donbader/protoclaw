use anyclaw_core::CrashAction;
use tokio_util::sync::CancellationToken;

use anyclaw_tools::ToolsCommand;

use crate::ManagerSlot;
use crate::Supervisor;
use crate::factory::{Stores, create_manager};

impl Supervisor {
    pub(crate) async fn check_and_restart_managers(
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
                    anyclaw_core::constants::CMD_CHANNEL_CAPACITY,
                );
                tx
            });
            let tools_rx = if slot.name == "tools" {
                let (new_tx, rx) = tokio::sync::mpsc::channel::<ToolsCommand>(
                    anyclaw_core::constants::CMD_CHANNEL_CAPACITY,
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
                Some(Stores {
                    session: std::sync::Arc::clone(&self.session_store),
                    context: std::sync::Arc::clone(&self.context_store),
                }),
            );
            if let Err(e) = manager.start().await {
                tracing::error!(manager = %slot.name, error = %e, "restart boot failed");
                continue;
            }

            slot.lifecycle.cancel_token = root_cancel.child_token();
            let token = slot.lifecycle.cancel_token.clone();
            let handle = tokio::spawn(async move { manager.run(token).await });
            slot.join_handle = Some(handle);

            metrics::counter!("anyclaw_manager_restarts_total", "manager" => slot.name.clone())
                .increment(1);

            tracing::info!(manager = %slot.name, "restarted");
        }

        self.refresh_health_snapshot(slots).await;
    }

    pub(crate) async fn refresh_health_snapshot(&self, slots: &[ManagerSlot]) {
        let agents_running = slots
            .iter()
            .any(|s| s.name == "agents" && s.join_handle.is_some());
        let channels_running = slots
            .iter()
            .any(|s| s.name == "channels" && s.join_handle.is_some());

        let agents: Vec<anyclaw_core::AgentHealth> = self
            .config
            .agents_manager
            .agents
            .keys()
            .map(|name| anyclaw_core::AgentHealth {
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
            .filter(|(_, t)| t.enabled && t.tool_type == anyclaw_config::ToolType::Mcp)
            .map(|(name, _)| name.clone())
            .collect();

        let degraded = agents.iter().any(|a| !a.connected);
        let status = if degraded {
            anyclaw_core::HealthStatus::Degraded
        } else {
            anyclaw_core::HealthStatus::Healthy
        };

        let mut snapshot = self.health.write().await;
        *snapshot = anyclaw_core::HealthSnapshot {
            status,
            agents,
            channels,
            mcp_servers,
        };

        metrics::gauge!("anyclaw_agents_connected")
            .set(snapshot.agents.iter().filter(|a| a.connected).count() as f64);
        metrics::gauge!("anyclaw_channels_running").set(snapshot.channels.len() as f64);
    }
}
