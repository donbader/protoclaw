use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcpError {
    #[error("protocol version mismatch: expected {expected}, got {got}")]
    ProtocolMismatch { expected: u32, got: u32 },

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("method not supported: {0}")]
    MethodNotSupported(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc { code: i64, message: String },
}
