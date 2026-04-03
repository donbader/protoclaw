use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("Failed to spawn agent process: {0}")]
    SpawnFailed(String),
    #[error("Agent process exited unexpectedly: {0}")]
    ProcessExited(String),
    #[error("ACP protocol error: {0}")]
    Protocol(#[from] crate::acp_error::AcpError),
    #[error("Request timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Agent connection closed")]
    ConnectionClosed,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
}
