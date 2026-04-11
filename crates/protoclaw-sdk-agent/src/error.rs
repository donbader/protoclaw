/// Errors produced by agent SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentSdkError {
    /// An ACP protocol-level error (e.g. unexpected message format).
    #[error("Protocol error: {0}")]
    Protocol(String),
    /// A message transformation hook failed.
    #[error("Transform error: {0}")]
    Transform(String),
    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
