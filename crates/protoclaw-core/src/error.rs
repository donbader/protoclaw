#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("failed to boot manager {manager}: {reason}")]
    BootFailed { manager: String, reason: String },
    #[error("manager {manager} crashed: {reason}")]
    ManagerCrashed { manager: String, reason: String },
    #[error("shutdown timed out for manager {manager}")]
    ShutdownTimeout { manager: String },
    #[error("crash loop detected for manager {manager}: {count} crashes in {window_secs}s")]
    CrashLoop {
        manager: String,
        count: u32,
        window_secs: u64,
    },
    #[error("configuration error: {0}")]
    Config(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("manager not started")]
    NotStarted,
    #[error("manager already running")]
    AlreadyRunning,
    #[error("health check failed: {reason}")]
    HealthCheckFailed { reason: String },
    #[error("command send failed: {0}")]
    SendFailed(String),
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
