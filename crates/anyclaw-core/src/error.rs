/// Errors originating from the supervisor layer (boot failures, crash loops, shutdown timeouts).
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// A manager failed during its `start()` phase.
    #[error("failed to boot manager {manager}: {reason}")]
    BootFailed {
        /// Name of the manager that failed to boot.
        manager: String,
        /// Human-readable reason for the failure.
        reason: String,
    },
    /// A manager's `run()` task exited with an error.
    #[error("manager {manager} crashed: {reason}")]
    ManagerCrashed {
        /// Name of the crashed manager.
        manager: String,
        /// Human-readable crash reason.
        reason: String,
    },
    /// A manager did not shut down within the per-manager timeout.
    #[error("shutdown timed out for manager {manager}")]
    ShutdownTimeout {
        /// Name of the manager that timed out.
        manager: String,
    },
    /// A manager exceeded its crash-loop threshold (N crashes within a time window).
    #[error("crash loop detected for manager {manager}: {count} crashes in {window_secs}s")]
    CrashLoop {
        /// Name of the crash-looping manager.
        manager: String,
        /// Number of crashes recorded in the window.
        count: u32,
        /// Duration of the crash-tracking window in seconds.
        window_secs: u64,
    },
    /// A configuration error prevented supervisor startup.
    #[error("configuration error: {0}")]
    Config(String),
}

/// Errors originating from individual manager operations.
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    /// The manager's `start()` was not called before `run()`.
    #[error("manager not started")]
    NotStarted,
    /// Attempted to call `run()` on a manager that is already running.
    #[error("manager already running")]
    AlreadyRunning,
    /// A periodic health check failed.
    #[error("health check failed: {reason}")]
    HealthCheckFailed {
        /// Why the health check failed.
        reason: String,
    },
    /// Sending a command via [`ManagerHandle`](crate::ManagerHandle) failed (channel closed).
    #[error("command send failed: {0}")]
    SendFailed(String),
    /// Catch-all for internal errors that don't fit other variants.
    #[error("{0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_boot_failed_error_displayed_then_includes_manager_name() {
        let err = SupervisorError::BootFailed {
            manager: "tools".into(),
            reason: "port in use".into(),
        };
        assert_eq!(err.to_string(), "failed to boot manager tools: port in use");
    }

    #[test]
    fn when_crash_loop_error_displayed_then_includes_manager_name() {
        let err = SupervisorError::CrashLoop {
            manager: "agents".into(),
            count: 5,
            window_secs: 60,
        };
        assert_eq!(
            err.to_string(),
            "crash loop detected for manager agents: 5 crashes in 60s"
        );
    }

    #[test]
    fn when_manager_error_variants_displayed_then_each_formats_correctly() {
        assert_eq!(ManagerError::NotStarted.to_string(), "manager not started");
        assert_eq!(
            ManagerError::AlreadyRunning.to_string(),
            "manager already running"
        );
        assert_eq!(
            ManagerError::HealthCheckFailed {
                reason: "timeout".into()
            }
            .to_string(),
            "health check failed: timeout"
        );
        assert_eq!(
            ManagerError::SendFailed("channel closed".into()).to_string(),
            "command send failed: channel closed"
        );
        assert_eq!(ManagerError::Internal("oops".into()).to_string(), "oops");
    }

    #[test]
    fn when_error_types_checked_then_implement_std_error_trait() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<SupervisorError>();
        assert_error::<ManagerError>();
    }
}
