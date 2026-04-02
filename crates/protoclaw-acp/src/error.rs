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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_error_display_protocol_mismatch() {
        let err = AcpError::ProtocolMismatch {
            expected: 1,
            got: 2,
        };
        assert_eq!(
            err.to_string(),
            "protocol version mismatch: expected 1, got 2"
        );
    }

    #[test]
    fn acp_error_display_session_not_found() {
        let err = AcpError::SessionNotFound("sess-123".to_string());
        assert_eq!(err.to_string(), "session not found: sess-123");
    }

    #[test]
    fn acp_error_display_method_not_supported() {
        let err = AcpError::MethodNotSupported("session/fork".to_string());
        assert_eq!(err.to_string(), "method not supported: session/fork");
    }

    #[test]
    fn acp_error_display_transport() {
        let err = AcpError::Transport("broken pipe".to_string());
        assert_eq!(err.to_string(), "transport error: broken pipe");
    }
}
