/// Errors produced by channel SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum ChannelSdkError {
    /// An I/O error from the underlying transport.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A protocol-level error (e.g. unknown method, bad handshake).
    #[error("Protocol error: {0}")]
    Protocol(String),
    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_std_io_error_converted_then_wrapped_as_channel_sdk_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let err: ChannelSdkError = io_err.into();
        assert!(matches!(err, ChannelSdkError::Io(_)));
        assert!(err.to_string().contains("pipe broke"));
    }

    #[test]
    fn when_protocol_error_created_then_wraps_string_message() {
        let err = ChannelSdkError::Protocol("bad handshake".into());
        assert!(err.to_string().contains("bad handshake"));
    }

    #[test]
    fn when_json_parse_error_converted_then_wrapped_as_channel_sdk_serde_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: ChannelSdkError = json_err.into();
        assert!(matches!(err, ChannelSdkError::Serde(_)));
    }

    #[test]
    fn when_channel_sdk_error_checked_then_implements_std_error() {
        let err = ChannelSdkError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
