use tokio::sync::oneshot;

use crate::SessionKey;
use protoclaw_sdk_types::PermissionOption;

#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    pub name: String,
    pub connected: bool,
    pub session_count: usize,
}

pub struct PendingPermissionInfo {
    pub request_id: String,
    pub description: String,
    pub options: Vec<PermissionOption>,
}

pub enum AgentsCommand {
    SendPrompt {
        message: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    CancelOperation,
    RespondPermission {
        request_id: String,
        option_id: String,
    },
    GetPendingPermissions {
        reply: oneshot::Sender<Vec<PendingPermissionInfo>>,
    },
    Shutdown,
    GetStatus {
        reply: oneshot::Sender<Vec<AgentStatusInfo>>,
    },
    CreateSession {
        agent_name: String,
        session_key: SessionKey,
        reply: oneshot::Sender<Result<String, String>>,
    },
    PromptSession {
        agent_name: String,
        session_key: SessionKey,
        message: String,
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
}
