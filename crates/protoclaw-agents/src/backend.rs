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
pub trait ProcessBackend: Send {
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
