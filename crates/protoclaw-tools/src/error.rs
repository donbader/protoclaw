use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolsError {
    #[error("Failed to start MCP server: {0}")]
    ServerStart(String),
    #[error("MCP server not running")]
    ServerNotRunning,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
