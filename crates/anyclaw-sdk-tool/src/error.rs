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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_execution_failed_displayed_then_contains_message() {
        let err = ToolSdkError::ExecutionFailed("timeout".into());
        assert_eq!(err.to_string(), "Tool execution failed: timeout");
    }

    #[rstest]
    fn when_invalid_input_displayed_then_contains_message() {
        let err = ToolSdkError::InvalidInput("missing required field 'path'".into());
        assert_eq!(
            err.to_string(),
            "Invalid input: missing required field 'path'"
        );
    }

    #[rstest]
    fn when_io_error_converted_then_displays_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: ToolSdkError = io_err.into();
        assert!(matches!(err, ToolSdkError::Io(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[rstest]
    fn when_serde_error_converted_then_displays_serialization_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad json").unwrap_err();
        let err: ToolSdkError = json_err.into();
        assert!(matches!(err, ToolSdkError::Serde(_)));
        assert!(err.to_string().starts_with("Serialization error:"));
    }

    #[rstest]
    fn when_tool_sdk_error_checked_then_implements_std_error() {
        let err = ToolSdkError::ExecutionFailed("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
