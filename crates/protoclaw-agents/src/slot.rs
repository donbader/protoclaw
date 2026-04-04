use std::collections::HashMap;
use std::time::Duration;

use protoclaw_config::AgentConfig;
use protoclaw_core::{CrashTracker, ExponentialBackoff, SessionKey};
use tokio_util::sync::CancellationToken;

use crate::acp_types::InitializeResult;
use crate::connection::AgentConnection;
use crate::PendingPermission;

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
    use std::collections::HashMap;

    fn test_agent_config(enabled: bool) -> AgentConfig {
        AgentConfig {
            binary: "test-binary".to_string(),
            args: vec![],
            enabled,
            env: HashMap::new(),
            working_dir: None,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
        }
    }

    #[test]
    fn slot_new_creates_empty_state() {
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
    fn slot_disabled_agent_has_disabled_true() {
        let cancel = CancellationToken::new();
        let slot = AgentSlot::new("disabled-agent".into(), test_agent_config(false), &cancel);

        assert!(slot.disabled);
        assert_eq!(slot.name(), "disabled-agent");
    }

    #[test]
    fn find_slot_by_name_returns_correct_index() {
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
    fn find_slot_by_name_returns_none_for_unknown() {
        let cancel = CancellationToken::new();
        let slots = vec![AgentSlot::new(
            "alpha".into(),
            test_agent_config(true),
            &cancel,
        )];

        assert_eq!(find_slot_by_name(&slots, "nonexistent"), None);
    }

    #[test]
    fn find_slot_by_name_empty_slots() {
        let slots: Vec<AgentSlot> = vec![];
        assert_eq!(find_slot_by_name(&slots, "any"), None);
    }

    #[test]
    fn slot_session_map_insert_and_lookup() {
        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("agent".into(), test_agent_config(true), &cancel);

        let key = SessionKey::new("telegram", "direct", "alice");
        slot.session_map
            .insert(key.clone(), "acp-sess-1".to_string());

        assert_eq!(slot.session_map.get(&key), Some(&"acp-sess-1".to_string()));
    }

    #[test]
    fn slot_reverse_map_lookup_returns_correct_session_key() {
        let cancel = CancellationToken::new();
        let mut slot = AgentSlot::new("agent".into(), test_agent_config(true), &cancel);

        let key = SessionKey::new("debug-http", "local", "dev");
        let acp_id = "acp-sess-42".to_string();

        slot.session_map.insert(key.clone(), acp_id.clone());
        slot.reverse_map.insert(acp_id.clone(), key.clone());

        assert_eq!(slot.reverse_map.get(&acp_id), Some(&key));
    }

    #[test]
    fn slot_cancel_token_is_child_of_parent() {
        let parent = CancellationToken::new();
        let slot = AgentSlot::new("agent".into(), test_agent_config(true), &parent);

        assert!(!slot.cancel_token.is_cancelled());
        parent.cancel();
        assert!(slot.cancel_token.is_cancelled());
    }
}
