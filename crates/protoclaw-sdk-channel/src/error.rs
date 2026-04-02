#[derive(Debug, thiserror::Error)]
pub enum ChannelSdkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_wraps_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let err: ChannelSdkError = io_err.into();
        assert!(matches!(err, ChannelSdkError::Io(_)));
        assert!(err.to_string().contains("pipe broke"));
    }

    #[test]
    fn protocol_error_wraps_string() {
        let err = ChannelSdkError::Protocol("bad handshake".into());
        assert!(err.to_string().contains("bad handshake"));
    }

    #[test]
    fn serde_error_wraps_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: ChannelSdkError = json_err.into();
        assert!(matches!(err, ChannelSdkError::Serde(_)));
    }

    #[test]
    fn implements_std_error() {
        let err = ChannelSdkError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
