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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn assert_std_error<E: std::error::Error>(_: &E) {}

    #[rstest]
    fn when_server_start_error_displayed_then_shows_reason() {
        let err = ToolsError::ServerStart("port already in use".into());
        assert_eq!(
            err.to_string(),
            "Failed to start MCP server: port already in use"
        );
    }

    #[rstest]
    fn when_server_not_running_displayed_then_shows_message() {
        let err = ToolsError::ServerNotRunning;
        assert_eq!(err.to_string(), "MCP server not running");
    }

    #[rstest]
    fn when_mcp_host_failed_displayed_then_shows_reason() {
        let err = ToolsError::McpHostFailed("connection refused".into());
        assert_eq!(err.to_string(), "MCP host failed: connection refused");
    }

    #[rstest]
    fn when_tools_error_checked_then_implements_std_error() {
        let err = ToolsError::ServerNotRunning;
        assert_std_error(&err);
    }
}
