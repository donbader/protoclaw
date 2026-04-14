use tokio::sync::oneshot;

use crate::SessionKey;
use anyclaw_sdk_types::PermissionOption;
use anyclaw_sdk_types::SessionListResult;

/// Status snapshot for a single agent, returned by the admin API.
#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    /// Agent name (matches the config key).
    pub name: String,
    /// Whether the agent subprocess is currently connected.
    pub connected: bool,
    /// Number of active ACP sessions.
    pub session_count: usize,
}

/// A pending permission request awaiting user response, surfaced via the admin API.
pub struct PendingPermissionInfo {
    /// The ACP request ID that must be echoed back in the response.
    pub request_id: String,
    /// Human-readable description of what the agent is requesting.
    pub description: String,
    /// Available response options (e.g. "Allow", "Deny").
    pub options: Vec<PermissionOption>,
}

/// Commands sent to the agents manager via [`ManagerHandle<AgentsCommand>`](crate::ManagerHandle).
pub enum AgentsCommand {
    /// Send a prompt to the default agent session (legacy single-session path).
    SendPrompt {
        /// The user message text.
        message: String,
        /// Oneshot channel for the result.
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Cancel any in-progress operation on the default agent.
    CancelOperation,
    /// Respond to a pending permission request from an agent.
    RespondPermission {
        /// The ACP request ID being responded to.
        request_id: String,
        /// The selected option ID (e.g. "allow", "deny").
        option_id: String,
    },
    /// Retrieve all pending permission requests across all agents.
    GetPendingPermissions {
        /// Oneshot channel for the list of pending permissions.
        reply: oneshot::Sender<Vec<PendingPermissionInfo>>,
    },
    /// Request graceful shutdown of all agent subprocesses.
    Shutdown,
    /// Retrieve status information for all configured agents.
    GetStatus {
        /// Oneshot channel for the status list.
        reply: oneshot::Sender<Vec<AgentStatusInfo>>,
    },
    /// Create a new ACP session for a specific agent and session key.
    CreateSession {
        /// Which agent to create the session on.
        agent_name: String,
        /// Channel-derived session identity (e.g. "telegram:user:12345").
        session_key: SessionKey,
        /// Oneshot channel returning the ACP session ID on success.
        reply: oneshot::Sender<Result<String, String>>,
    },
    /// Send a user message to an existing session.
    PromptSession {
        /// Which agent owns the session.
        agent_name: String,
        /// Session identity to route the prompt to.
        session_key: SessionKey,
        /// The user message text.
        message: String,
        /// Oneshot channel for the result.
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Fork an existing session (creates a new session branching from the current state).
    ForkSession {
        /// Which agent owns the session to fork.
        agent_name: String,
        /// Session identity to fork from.
        session_key: SessionKey,
        /// Oneshot channel returning the new ACP session ID on success.
        reply: oneshot::Sender<Result<String, String>>,
    },
    /// List all sessions on a specific agent.
    ListSessions {
        /// Which agent to query.
        agent_name: String,
        /// Oneshot channel for the session list.
        reply: oneshot::Sender<Result<SessionListResult, String>>,
    },
    /// Cancel an in-progress operation on a specific session.
    CancelSession {
        /// Which agent owns the session.
        agent_name: String,
        /// Session identity to cancel.
        session_key: SessionKey,
        /// Oneshot channel for the result.
        reply: oneshot::Sender<Result<(), String>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_agent_status_info_constructed_then_fields_accessible() {
        let info = AgentStatusInfo {
            name: "my-agent".into(),
            connected: true,
            session_count: 3,
        };
        assert_eq!(info.name, "my-agent");
        assert!(info.connected);
        assert_eq!(info.session_count, 3);
    }

    #[test]
    fn when_agent_status_info_cloned_then_equal_to_original() {
        let info = AgentStatusInfo {
            name: "agent-a".into(),
            connected: false,
            session_count: 0,
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert_eq!(cloned.connected, info.connected);
        assert_eq!(cloned.session_count, info.session_count);
    }

    #[test]
    fn when_pending_permission_info_constructed_then_fields_accessible() {
        let opt = PermissionOption {
            option_id: "allow".into(),
            label: "Allow".into(),
        };
        let pending = PendingPermissionInfo {
            request_id: "req-1".into(),
            description: "Allow file write?".into(),
            options: vec![opt],
        };
        assert_eq!(pending.request_id, "req-1");
        assert_eq!(pending.options.len(), 1);
        assert_eq!(pending.options[0].option_id, "allow");
    }

    #[test]
    fn when_agents_command_cancel_operation_constructed_then_is_unit_variant() {
        let cmd = AgentsCommand::CancelOperation;
        assert!(matches!(cmd, AgentsCommand::CancelOperation));
    }

    #[test]
    fn when_agents_command_shutdown_constructed_then_is_unit_variant() {
        let cmd = AgentsCommand::Shutdown;
        assert!(matches!(cmd, AgentsCommand::Shutdown));
    }

    #[test]
    fn when_agents_command_respond_permission_constructed_then_fields_accessible() {
        let cmd = AgentsCommand::RespondPermission {
            request_id: "r1".into(),
            option_id: "allow".into(),
        };
        match cmd {
            AgentsCommand::RespondPermission {
                request_id,
                option_id,
            } => {
                assert_eq!(request_id, "r1");
                assert_eq!(option_id, "allow");
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn when_list_sessions_reply_sent_then_typed_result_received() {
        use anyclaw_sdk_types::SessionInfo;
        use std::collections::HashMap;

        let (tx, rx) = oneshot::channel();
        let result = SessionListResult {
            sessions: vec![SessionInfo {
                session_id: "ses-1".into(),
                metadata: HashMap::new(),
            }],
        };
        tx.send(Ok(result)).expect("send should succeed");
        let received: Result<SessionListResult, String> =
            rx.blocking_recv().expect("recv should succeed");
        let list = received.expect("result should be Ok");
        assert_eq!(list.sessions.len(), 1);
        assert_eq!(list.sessions[0].session_id, "ses-1");
    }
}
