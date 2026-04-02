use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolsError {
    #[error("Failed to start MCP server: {0}")]
    ServerStart(String),
    #[error("MCP server not running")]
    ServerNotRunning,
    #[error("MCP host failed: {0}")]
    McpHostFailed(String),
    #[error("External MCP server failed: {0}")]
    ExternalServerFailed(String),
    #[error("Tool proxy error: {0}")]
    ProxyError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
