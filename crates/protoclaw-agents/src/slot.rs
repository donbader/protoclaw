use std::collections::HashMap;
use std::time::Duration;

use protoclaw_config::AgentConfig;
use protoclaw_core::{CrashTracker, ExponentialBackoff, SessionKey};
use tokio_util::sync::CancellationToken;

use crate::PendingPermission;
use crate::acp_types::InitializeResult;
use crate::connection::AgentConnection;

/// Per-agent state: connection, config, crash recovery, session routing.
/// Mirrors ChannelSlot pattern from ChannelsManager.
pub struct AgentSlot {
    pub name: String,
    pub config: AgentConfig,
    pub connection: Option<AgentConnection>,
    pub cancel_token: CancellationToken,
    pub backoff: ExponentialBackoff,
    pub crash_tracker: CrashTracker,
    pub disabled: bool,
    pub agent_capabilities: Option<InitializeResult>,
    /// Maps channel+peer identity → ACP session ID (per-agent, not shared).
    pub session_map: HashMap<SessionKey, String>,
    /// Reverse: ACP session ID → SessionKey (per-agent).
    pub reverse_map: HashMap<String, SessionKey>,
    pub pending_permissions: HashMap<String, PendingPermission>,
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
        Self {
            name,
            config,
            connection: None,
            cancel_token: parent_cancel.child_token(),
            backoff,
            crash_tracker,
            disabled,
            agent_capabilities: None,
            session_map: HashMap::new(),
            reverse_map: HashMap::new(),
            pending_permissions: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
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
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: "test-binary".to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
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
        assert!(!slot.disabled);
        assert!(slot.agent_capabilities.is_none());
        assert!(slot.session_map.is_empty());
        assert!(slot.reverse_map.is_empty());
        assert!(slot.pending_permissions.is_empty());
    }

    #[test]
    fn when_slot_created_for_disabled_agent_then_disabled_flag_is_true() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("disabled-agent".into(), test_agent_config(false), &cancel);

        assert!(slot.disabled);
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

        assert!(!slot.cancel_token.is_cancelled());
        parent.cancel();
        assert!(slot.cancel_token.is_cancelled());
    }

    fn test_agent_config_with_crash_tracker(max_crashes: u32, window_secs: u64) -> AgentConfig {
        AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: "test-binary".to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: Some(protoclaw_config::CrashTrackerConfig {
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

        // Before any crashes, not a crash loop
        assert!(!slot.crash_tracker.is_crash_loop());
        assert!(!slot.disabled);

        // Record first crash — not yet at threshold (2 crashes required)
        slot.crash_tracker.record_crash();
        assert!(!slot.crash_tracker.is_crash_loop());

        // Record second crash — reaches threshold, crash loop detected
        slot.crash_tracker.record_crash();
        assert!(slot.crash_tracker.is_crash_loop());

        // Manager would set disabled = true when crash loop is detected
        slot.disabled = true;
        assert!(slot.disabled);
    }
}
