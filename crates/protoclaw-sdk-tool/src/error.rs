/// Errors produced by tool SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolSdkError {
    /// Tool logic failed with a user-facing message.
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    /// The caller supplied invalid input to the tool.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    /// An I/O error occurred during tool execution.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
