use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChannelsError {
    #[error("Failed to spawn channel subprocess: {0}")]
    SpawnFailed(String),
    #[error("Channel connection closed")]
    ConnectionClosed,
    #[error("Channel request timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Channel initialize failed: {0}")]
    InitializeFailed(String),
    #[error("Failed to bind HTTP server: {0}")]
    BindFailed(String),
    #[error("Agent command failed: {0}")]
    AgentCommand(String),
}
