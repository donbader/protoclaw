#[derive(Debug, thiserror::Error)]
pub enum AgentSdkError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Transform error: {0}")]
    Transform(String),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
