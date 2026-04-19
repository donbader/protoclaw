use std::collections::HashMap;
use std::time::Duration;

use anyclaw_config::WorkspaceConfig;
use anyclaw_core::{CrashAction, SessionKey};
use anyclaw_sdk_types::ChannelEvent;

use crate::acp_types::SessionLoadParams;
use crate::connection::AgentConnection;
use crate::error::AgentsError;
use crate::fs_sandbox::resolve_agent_cwd;
use crate::manager::AgentsManager;

impl AgentsManager {
    pub(crate) async fn handle_crash(&mut self, slot_idx: usize) {
        let agent_name = self.slots[slot_idx].name().to_string();
        if !self.prepare_restart(slot_idx, &agent_name).await {
            return;
        }

        self.notify_crash_to_channels(slot_idx, &agent_name).await;

        if !self.respawn_and_initialize(slot_idx, &agent_name).await {
            return;
        }

        self.restore_or_start_session(slot_idx, &agent_name).await;
    }

    pub(crate) async fn prepare_restart(&mut self, slot_idx: usize, agent_name: &str) -> bool {
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

    async fn notify_crash_to_channels(&mut self, slot_idx: usize, agent_name: &str) {
        let Some(sender) = &self.channels_sender else {
            self.slots[slot_idx].active_prompts.clear();
            return;
        };
        let session_keys: Vec<SessionKey> =
            self.slots[slot_idx].session_map.keys().cloned().collect();
        if session_keys.is_empty() {
            self.slots[slot_idx].active_prompts.clear();
            return;
        }
        tracing::info!(
            agent = %agent_name,
            sessions = session_keys.len(),
            "notifying channels of agent crash"
        );
        for sk in &session_keys {
            // Sessions with an active prompt will be notified via the spawned
            // task's error path when response_rx drops — skip to avoid duplicates.
            let has_active_prompt = self.slots[slot_idx]
                .session_map
                .get(sk)
                .is_some_and(|acp_id| self.slots[slot_idx].active_prompts.contains_key(acp_id));
            if has_active_prompt {
                continue;
            }

            let error_content = serde_json::json!({
                "update": {
                    "sessionUpdate": "result",
                    "isError": true,
                    "content": format!("Agent crashed — restarting ({agent_name})"),
                }
            });
            let _ = sender
                .send(ChannelEvent::DeliverMessage {
                    session_key: sk.clone(),
                    content: error_content,
                })
                .await;
        }
        self.slots[slot_idx].active_prompts.clear();
    }

    pub(crate) async fn respawn_and_initialize(
        &mut self,
        slot_idx: usize,
        agent_name: &str,
    ) -> bool {
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

    pub(crate) async fn try_restore_session(
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
                        .and_then(|v| v.as_str())
                        .is_some() =>
                {
                    let returned_id = resp
                        .result
                        .as_ref()
                        .and_then(|r| r.get("sessionId"))
                        .and_then(|v| v.as_str())
                        .expect("guard verified sessionId is present")
                        .to_owned();
                    tracing::info!(
                        agent = %agent_name,
                        step = "resume_attempted",
                        success = true,
                        "session restored via session/resume"
                    );
                    let slot = &mut self.slots[slot_idx];
                    for (key, val) in slot.stale_sessions.drain() {
                        let id = if val == first_acp_id {
                            returned_id.clone()
                        } else {
                            val
                        };
                        slot.session_map.insert(key, id);
                    }
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
             session_id: first_acp_id.clone(),
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
                    .and_then(|v| v.as_str())
                    .is_some() =>
            {
                let returned_id = resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .and_then(|v| v.as_str())
                    .expect("guard verified sessionId is present")
                    .to_owned();
                tracing::info!(agent = %agent_name, "session restored via session/load");
                let slot = &mut self.slots[slot_idx];
                for (key, val) in slot.stale_sessions.drain() {
                    let id = if val == first_acp_id {
                        returned_id.clone()
                    } else {
                        val
                    };
                    slot.session_map.insert(key, id);
                }
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

    pub(crate) async fn restore_or_start_session(&mut self, slot_idx: usize, agent_name: &str) {
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

    /// Attempt to recover a missing session before a prompt:
    /// 1. Try `session/resume` if the agent supports it and a stale ACP session ID exists.
    /// 2. Try `session/load` if the agent supports it and a stale ACP session ID exists.
    /// 3. Fall back to `create_session` otherwise.
    pub(crate) async fn heal_session(
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
                && let Some(returned_id) = resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
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
                    .insert(session_key.clone(), returned_id.clone());
                slot.reverse_map
                    .insert(returned_id.clone(), session_key.clone());
                // No awaiting_first_prompt for resume — no replay needed.
                self.update_session_store(agent_name, session_key, &returned_id)
                    .await;
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
                && let Some(returned_id) = resp
                    .result
                    .as_ref()
                    .and_then(|r| r.get("sessionId"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
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
                slot.session_map
                    .insert(session_key.clone(), returned_id.clone());
                slot.reverse_map
                    .insert(returned_id.clone(), session_key.clone());
                slot.awaiting_first_prompt.insert(returned_id.clone());
                self.update_session_store(agent_name, session_key, &returned_id)
                    .await;
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

    async fn update_session_store(
        &self,
        agent_name: &str,
        session_key: &SessionKey,
        acp_session_id: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let persisted = anyclaw_core::PersistedSession {
            session_key: session_key.to_string(),
            agent_name: agent_name.to_string(),
            acp_session_id: acp_session_id.to_string(),
            created_at: now,
            last_active_at: now,
            closed: false,
        };
        if let Err(e) = self.session_store.upsert_session(&persisted).await {
            tracing::warn!(
                agent = %agent_name,
                session_key = %session_key,
                error = %e,
                "failed to persist recovered session to store"
            );
        }
    }

    /// Remove any Docker containers left over from a previous (crashed) run.
    ///
    /// Scans all configured agents for Docker workspaces, connects to the matching
    /// Docker daemon, and forcibly removes every container that carries the
    /// `anyclaw.managed=true` label.  Errors are logged as warnings; this
    /// method never propagates failures so that `start()` is not blocked by
    /// stale-container cleanup.
    pub(crate) async fn cleanup_stale_containers(&self) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot::AgentSlot;
    use anyclaw_config::{
        AgentConfig, AgentsManagerConfig, CrashTrackerConfig, LocalWorkspaceConfig, WorkspaceConfig,
    };
    use anyclaw_core::{ManagerHandle, ToolsCommand};
    use rstest::rstest;
    use std::collections::HashMap;

    fn test_agent_config_with_max_crashes(max_crashes: u32) -> AgentConfig {
        AgentConfig {
            workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                binary: "test-binary".into(),
                working_dir: None,
                env: HashMap::new(),
            }),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: Some(CrashTrackerConfig {
                max_crashes,
                window_secs: 60,
            }),
            options: HashMap::new(),
        }
    }

    fn make_tools_handle() -> (
        ManagerHandle<ToolsCommand>,
        tokio::sync::mpsc::Receiver<ToolsCommand>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        (ManagerHandle::new(tx), rx)
    }

    fn make_manager_with_slot(config: AgentConfig) -> AgentsManager {
        let (handle, _rx) = make_tools_handle();
        let manager_config = AgentsManagerConfig {
            agents: HashMap::new(),
            ..Default::default()
        };
        let mut m = AgentsManager::new(manager_config, handle);
        let cancel = m.parent_cancel.clone();
        let slot = AgentSlot::new("test-agent".into(), config, &cancel);
        m.slots.push(slot);
        m
    }

    #[rstest]
    #[tokio::test]
    async fn when_crash_loop_detected_then_prepare_restart_returns_false() {
        tokio::time::pause();
        let mut m = make_manager_with_slot(test_agent_config_with_max_crashes(1));
        let result = m.prepare_restart(0, "test-agent").await;
        assert!(!result);
        assert!(m.slots[0].lifecycle.disabled);
    }

    #[rstest]
    #[tokio::test]
    async fn when_restart_allowed_then_prepare_restart_returns_true() {
        tokio::time::pause();
        let mut m = make_manager_with_slot(test_agent_config_with_max_crashes(3));
        let result = m.prepare_restart(0, "test-agent").await;
        assert!(result);
    }

    #[rstest]
    #[tokio::test]
    async fn when_restart_allowed_with_no_connection_then_returns_true() {
        tokio::time::pause();
        let mut m = make_manager_with_slot(test_agent_config_with_max_crashes(3));
        assert!(m.slots[0].connection.is_none());
        let result = m.prepare_restart(0, "test-agent").await;
        assert!(result);
    }
}
