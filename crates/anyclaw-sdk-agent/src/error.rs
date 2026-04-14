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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_protocol_error_displayed_then_contains_message() {
        let err = AgentSdkError::Protocol("unexpected initialize".into());
        assert_eq!(err.to_string(), "Protocol error: unexpected initialize");
    }

    #[rstest]
    fn when_transform_error_displayed_then_contains_message() {
        let err = AgentSdkError::Transform("failed to inject prompt".into());
        assert_eq!(err.to_string(), "Transform error: failed to inject prompt");
    }

    #[rstest]
    fn when_serde_error_converted_then_displays_serialization_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: AgentSdkError = json_err.into();
        assert!(matches!(err, AgentSdkError::Serde(_)));
        assert!(err.to_string().starts_with("Serialization error:"));
    }

    #[rstest]
    fn when_agent_sdk_error_checked_then_implements_std_error() {
        let err = AgentSdkError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
