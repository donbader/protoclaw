use thiserror::Error;

/// Errors from channel subprocess operations.
#[derive(Debug, Error)]
pub enum ChannelsError {
    /// The channel binary could not be spawned.
    #[error("Failed to spawn channel subprocess: {0}")]
    SpawnFailed(String),
    /// The channel's stdio connection was closed unexpectedly.
    #[error("Channel connection closed")]
    ConnectionClosed,
    /// A channel request did not receive a response within the configured timeout.
    #[error("Channel request timed out after {0:?}")]
    Timeout(std::time::Duration),
    /// An I/O error during subprocess communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// The channel's `initialize` handshake failed.
    #[error("Channel initialize failed: {0}")]
    InitializeFailed(String),
    /// The debug-http server could not bind to the requested port.
    #[error("Failed to bind HTTP server: {0}")]
    BindFailed(String),
    /// A command sent to the agents manager failed.
    #[error("Agent command failed: {0}")]
    AgentCommand(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn assert_std_error<E: std::error::Error>(_: &E) {}

    #[rstest]
    fn when_spawn_failed_displayed_then_shows_channel_name() {
        let err = ChannelsError::SpawnFailed("telegram".into());
        assert_eq!(
            err.to_string(),
            "Failed to spawn channel subprocess: telegram"
        );
    }

    #[rstest]
    fn when_timeout_displayed_then_shows_duration() {
        let err = ChannelsError::Timeout(std::time::Duration::from_secs(10));
        assert_eq!(err.to_string(), "Channel request timed out after 10s");
    }

    #[rstest]
    fn when_initialize_failed_displayed_then_shows_reason() {
        let err = ChannelsError::InitializeFailed("bad token".into());
        assert_eq!(err.to_string(), "Channel initialize failed: bad token");
    }

    #[rstest]
    fn when_bind_failed_displayed_then_shows_address() {
        let err = ChannelsError::BindFailed("0.0.0.0:8080".into());
        assert_eq!(err.to_string(), "Failed to bind HTTP server: 0.0.0.0:8080");
    }

    #[rstest]
    fn when_channels_error_checked_then_implements_std_error() {
        let err = ChannelsError::ConnectionClosed;
        assert_std_error(&err);
    }
}
