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
    use rstest::rstest;

    fn assert_std_error<E: std::error::Error>(_: &E) {}

    #[rstest]
    fn when_protocol_mismatch_displayed_then_shows_expected_and_got() {
        let err = AcpError::ProtocolMismatch {
            expected: 1,
            got: 2,
        };
        assert_eq!(
            err.to_string(),
            "protocol version mismatch: expected 1, got 2"
        );
    }

    #[rstest]
    fn when_session_not_found_displayed_then_shows_session_id() {
        let err = AcpError::SessionNotFound("sess-abc".into());
        assert_eq!(err.to_string(), "session not found: sess-abc");
    }

    #[rstest]
    fn when_method_not_supported_displayed_then_shows_method_name() {
        let err = AcpError::MethodNotSupported("session/unknown".into());
        assert_eq!(err.to_string(), "method not supported: session/unknown");
    }

    #[rstest]
    fn when_transport_error_displayed_then_shows_message() {
        let err = AcpError::Transport("pipe broken".into());
        assert_eq!(err.to_string(), "transport error: pipe broken");
    }

    #[rstest]
    fn when_jsonrpc_error_displayed_then_shows_code_and_message() {
        let err = AcpError::JsonRpc {
            code: -32600,
            message: "Invalid Request".into(),
        };
        assert_eq!(err.to_string(), "JSON-RPC error -32600: Invalid Request");
    }

    #[rstest]
    fn when_acp_error_checked_then_implements_std_error() {
        let err = AcpError::Transport("test".into());
        assert_std_error(&err);
    }
}
