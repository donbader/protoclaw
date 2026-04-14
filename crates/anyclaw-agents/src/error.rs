use thiserror::Error;

/// Manager-level errors for agent subprocess operations.
#[derive(Debug, Error)]
pub enum AgentsError {
    /// The agent binary could not be spawned.
    #[error("Failed to spawn agent process: {0}")]
    SpawnFailed(String),
    /// The agent process exited before we expected it to.
    #[error("Agent process exited unexpectedly: {0}")]
    ProcessExited(String),
    /// An ACP protocol-level error (wraps [`AcpError`](crate::acp_error::AcpError)).
    #[error("ACP protocol error: {0}")]
    Protocol(#[from] crate::acp_error::AcpError),
    /// An ACP request did not receive a response within the configured timeout.
    #[error("Request timed out after {0:?}")]
    Timeout(std::time::Duration),
    /// The agent's stdio connection was closed unexpectedly.
    #[error("Agent connection closed")]
    ConnectionClosed,
    /// An I/O error during subprocess communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// No agent with the given name exists in the configuration.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    /// The agent does not advertise a required ACP capability.
    #[error("Agent does not support capability: {0}")]
    CapabilityNotSupported(String),
    /// A Docker API error (container create, start, attach, etc.).
    #[error("Docker error: {0}")]
    DockerError(String),
    /// Docker image pull failed.
    #[error("Failed to pull image {image}: {reason}")]
    ImagePullFailed {
        /// The image that failed to pull.
        image: String,
        /// Why the pull failed.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn assert_std_error<E: std::error::Error>(_: &E) {}

    #[rstest]
    fn when_spawn_failed_displayed_then_shows_binary_name() {
        let err = AgentsError::SpawnFailed("claude-code".into());
        assert_eq!(
            err.to_string(),
            "Failed to spawn agent process: claude-code"
        );
    }

    #[rstest]
    fn when_process_exited_displayed_then_shows_reason() {
        let err = AgentsError::ProcessExited("exit code 1".into());
        assert_eq!(
            err.to_string(),
            "Agent process exited unexpectedly: exit code 1"
        );
    }

    #[rstest]
    fn when_protocol_error_wraps_acp_error_then_displays_cause() {
        let acp = crate::acp_error::AcpError::SessionNotFound("s1".into());
        let err = AgentsError::Protocol(acp);
        assert_eq!(err.to_string(), "ACP protocol error: session not found: s1");
    }

    #[rstest]
    fn when_timeout_displayed_then_shows_duration() {
        let err = AgentsError::Timeout(std::time::Duration::from_secs(30));
        assert_eq!(err.to_string(), "Request timed out after 30s");
    }

    #[rstest]
    fn when_connection_closed_displayed_then_shows_message() {
        let err = AgentsError::ConnectionClosed;
        assert_eq!(err.to_string(), "Agent connection closed");
    }

    #[rstest]
    fn when_io_error_converted_then_wraps_correctly() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe");
        let err = AgentsError::Io(io_err);
        assert!(err.to_string().contains("IO error"));
    }

    #[rstest]
    fn when_agents_error_checked_then_implements_std_error() {
        let err = AgentsError::ConnectionClosed;
        assert_std_error(&err);
    }

    #[rstest]
    fn when_docker_error_displayed_then_shows_message() {
        let err = AgentsError::DockerError("connection refused".into());
        assert_eq!(err.to_string(), "Docker error: connection refused");
    }

    #[rstest]
    fn when_image_pull_failed_displayed_then_shows_image_and_reason() {
        let err = AgentsError::ImagePullFailed {
            image: "myrepo/agent:latest".into(),
            reason: "manifest unknown".into(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to pull image myrepo/agent:latest: manifest unknown"
        );
    }
}
