use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("Failed to spawn agent process: {0}")]
    SpawnFailed(String),
    #[error("Agent process exited unexpectedly: {0}")]
    ProcessExited(String),
    #[error("ACP protocol error: {0}")]
    Protocol(#[from] crate::acp_error::AcpError),
    #[error("Request timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Agent connection closed")]
    ConnectionClosed,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Docker error: {0}")]
    DockerError(String),
    #[error("Failed to pull image {image}: {reason}")]
    ImagePullFailed { image: String, reason: String },
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
