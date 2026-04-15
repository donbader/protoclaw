use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::AgentsError;

/// Abstraction over an agent subprocess backend.
///
/// Decouples `AgentConnection` from `tokio::process::Child`, enabling
/// Docker-based agents (Phase 25) without changing connection logic.
///
/// All async methods use `Pin<Box<dyn Future>>` for object safety — the trait
/// can be used as `dyn ProcessBackend` without `async_trait`.
pub trait ProcessBackend: Send + Sync {
    /// Returns `true` if the underlying process is still running.
    fn is_alive(&mut self) -> bool;

    /// Take ownership of the stdin pipe. Returns `None` if already taken.
    fn take_stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>>;

    /// Take ownership of the stdout pipe. Returns `None` if already taken.
    fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>>;

    /// Take ownership of the stderr pipe. Returns `None` if already taken.
    fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>>;

    /// Kill the underlying process.
    fn kill(&mut self) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>>;

    /// Wait for the underlying process to exit and return its exit status.
    fn wait(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    struct MockBackend {
        alive: bool,
    }

    impl ProcessBackend for MockBackend {
        fn is_alive(&mut self) -> bool {
            self.alive
        }

        fn take_stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>> {
            None
        }

        fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
            None
        }

        fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
            None
        }

        fn kill(&mut self) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }

        fn wait(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>
        {
            Box::pin(async {
                // Use a real ExitStatus from a trivially-succeeding command
                let status = std::process::Command::new("true")
                    .status()
                    .map_err(AgentsError::Io)?;
                Ok(status)
            })
        }
    }

    #[rstest]
    fn when_process_backend_used_as_trait_object_then_compiles() {
        let mut backend: Box<dyn ProcessBackend> = Box::new(MockBackend { alive: true });
        assert!(backend.is_alive());
        assert!(backend.take_stdin().is_none());
        assert!(backend.take_stdout().is_none());
        assert!(backend.take_stderr().is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_mock_backend_killed_then_returns_ok() {
        let mut backend: Box<dyn ProcessBackend> = Box::new(MockBackend { alive: false });
        assert!(!backend.is_alive());
        let result = backend.kill().await;
        assert!(result.is_ok());
    }
}
