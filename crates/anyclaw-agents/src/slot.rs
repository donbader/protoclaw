use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyclaw_config::AgentConfig;
use anyclaw_core::{CrashTracker, ExponentialBackoff, SessionKey, SlotLifecycle};
use tokio_util::sync::CancellationToken;

use crate::PendingPermission;
use crate::acp_types::{InitializeResult, SessionCapabilities};
use crate::connection::AgentConnection;

pub struct AgentSlot {
    pub(crate) name: String,
    pub(crate) config: AgentConfig,
    pub(crate) connection: Option<AgentConnection>,
    pub(crate) lifecycle: SlotLifecycle,
    pub(crate) agent_capabilities: Option<InitializeResult>,
    /// Negotiated ACP protocol version, set after successful initialize handshake.
    /// 0 means not yet initialized.
    pub(crate) protocol_version: u32,
    pub(crate) session_map: HashMap<SessionKey, String>,
    pub(crate) reverse_map: HashMap<String, SessionKey>,
    pub(crate) pending_permissions: HashMap<String, PendingPermission>,
    /// Sessions that were active before a crash. Populated by draining `session_map`
    /// when an agent process exits. Used by `prompt_session` to attempt `session/load`
    /// recovery before falling back to creating a fresh session.
    pub(crate) stale_sessions: HashMap<SessionKey, String>,
    /// ACP session IDs loaded via `session/load` that haven't received a `session/prompt`
    /// yet. Replay events from `session/load` are suppressed until the first prompt.
    pub(crate) awaiting_first_prompt: HashSet<String>,
    /// Latest `available_commands_update` content received from this agent slot.
    /// Buffered so it can be replayed to the channel on every `prompt_session` call,
    /// ensuring commands are synced even when the channel binary restarts independently.
    /// D-03: stores arbitrary agent-reported availableCommands payload (not just platform commands).
    pub(crate) last_available_commands: Option<serde_json::Value>,
    /// Cached tool context string built from tool descriptions fetched at session creation.
    /// Shared across all sessions on this slot since all see the same tools.
    /// `None` means not yet fetched or no tools configured.
    pub(crate) tool_context: Option<String>,
    /// ACP session IDs that have already received the tool context injection.
    /// Prevents re-injecting on subsequent prompts within the same session.
    pub(crate) tool_context_sent: HashSet<String>,
}

impl AgentSlot {
    pub fn new(name: String, config: AgentConfig, parent_cancel: &CancellationToken) -> Self {
        let disabled = !config.enabled;
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
        let mut lifecycle = SlotLifecycle::new(parent_cancel, backoff, crash_tracker);
        lifecycle.disabled = disabled;
        Self {
            name,
            config,
            connection: None,
            lifecycle,
            agent_capabilities: None,
            protocol_version: 0,
            session_map: HashMap::new(),
            reverse_map: HashMap::new(),
            pending_permissions: HashMap::new(),
            stale_sessions: HashMap::new(),
            awaiting_first_prompt: HashSet::new(),
            last_available_commands: None,
            tool_context: None,
            tool_context_sent: HashSet::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn has_session_capability(&self, check: fn(&SessionCapabilities) -> bool) -> bool {
        self.agent_capabilities
            .as_ref()
            .and_then(|r| r.agent_capabilities.as_ref())
            .map(|c| &c.session_capabilities)
            .is_some_and(check)
    }
}

pub fn find_slot_by_name(slots: &[AgentSlot], name: &str) -> Option<usize> {
    slots.iter().position(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    fn test_agent_config(enabled: bool) -> AgentConfig {
        AgentConfig {
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
                    binary: "test-binary".into(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            enabled,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        }
    }

    #[test]
    fn when_new_slot_created_then_state_is_empty() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("test-agent".into(), test_agent_config(true), &cancel);

        assert_eq!(slot.name(), "test-agent");
        assert!(slot.connection.is_none());
        assert!(!slot.lifecycle.disabled);
        assert!(slot.agent_capabilities.is_none());
        assert!(slot.session_map.is_empty());
        assert!(slot.reverse_map.is_empty());
        assert!(slot.pending_permissions.is_empty());
        assert!(slot.stale_sessions.is_empty());
        assert!(slot.awaiting_first_prompt.is_empty());
    }

    #[test]
    fn when_slot_created_for_disabled_agent_then_disabled_flag_is_true() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("disabled-agent".into(), test_agent_config(false), &cancel);

        assert!(slot.lifecycle.disabled);
        assert_eq!(slot.name(), "disabled-agent");
    }

    #[test]
    fn when_finding_slot_by_known_name_then_returns_correct_index() {
        let cancel = CancellationToken::new();
        let slots = vec![
            AgentSlot::new("alpha".into(), test_agent_config(true), &cancel),
            AgentSlot::new("beta".into(), test_agent_config(true), &cancel),
            AgentSlot::new("gamma".into(), test_agent_config(true), &cancel),
        ];

        assert_eq!(find_slot_by_name(&slots, "alpha"), Some(0));
        assert_eq!(find_slot_by_name(&slots, "beta"), Some(1));
        assert_eq!(find_slot_by_name(&slots, "gamma"), Some(2));
    }

    #[test]
    fn when_finding_slot_by_unknown_name_then_returns_none() {
        let cancel = CancellationToken::new();
        let slots = vec![AgentSlot::new(
            "alpha".into(),
            test_agent_config(true),
            &cancel,
        )];

        assert_eq!(find_slot_by_name(&slots, "nonexistent"), None);
    }

    #[test]
    fn when_finding_slot_in_empty_list_then_returns_none() {
        let slots: Vec<AgentSlot> = vec![];
        assert_eq!(find_slot_by_name(&slots, "any"), None);
    }

    #[test]
    fn when_session_inserted_into_slot_map_then_lookup_returns_it() {
        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("agent".into(), test_agent_config(true), &cancel);

        let key = SessionKey::new("telegram", "direct", "alice");
        slot.session_map
            .insert(key.clone(), "acp-sess-1".to_string());

        assert_eq!(slot.session_map.get(&key), Some(&"acp-sess-1".to_string()));
    }

    #[test]
    fn when_reverse_map_queried_then_returns_correct_session_key() {
        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("agent".into(), test_agent_config(true), &cancel);

        let key = SessionKey::new("debug-http", "local", "dev");
        let acp_id = "acp-sess-42".to_string();

        slot.session_map.insert(key.clone(), acp_id.clone());
        slot.reverse_map.insert(acp_id.clone(), key.clone());

        assert_eq!(slot.reverse_map.get(&acp_id), Some(&key));
    }

    #[test]
    fn when_slot_cancel_token_created_then_is_child_of_parent_token() {
        let parent = CancellationToken::new();
        let slot = AgentSlot::new("agent".into(), test_agent_config(true), &parent);

        assert!(!slot.lifecycle.cancel_token.is_cancelled());
        parent.cancel();
        assert!(slot.lifecycle.cancel_token.is_cancelled());
    }

    fn test_agent_config_with_crash_tracker(max_crashes: u32, window_secs: u64) -> AgentConfig {
        AgentConfig {
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
                    binary: "test-binary".into(),
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

    #[rstest]
    fn when_crash_loop_detected_then_slot_can_be_disabled() {
        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new(
            "loop-agent".into(),
            test_agent_config_with_crash_tracker(2, 60),
            &cancel,
        );

        assert!(!slot.lifecycle.crash_tracker.is_crash_loop());
        assert!(!slot.lifecycle.disabled);

        slot.lifecycle.crash_tracker.record_crash();
        assert!(!slot.lifecycle.crash_tracker.is_crash_loop());

        slot.lifecycle.crash_tracker.record_crash();
        assert!(slot.lifecycle.crash_tracker.is_crash_loop());

        slot.lifecycle.disabled = true;
        assert!(slot.lifecycle.disabled);
    }
}
