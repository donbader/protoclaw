use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChannelsError {
    #[error("Failed to bind HTTP server: {0}")]
    BindFailed(String),
    #[error("Agent command failed: {0}")]
    AgentCommand(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
