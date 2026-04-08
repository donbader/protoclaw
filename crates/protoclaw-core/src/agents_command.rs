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
