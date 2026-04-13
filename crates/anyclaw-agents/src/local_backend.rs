use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;

use crate::backend::ProcessBackend;
use crate::error::AgentsError;

pub struct LocalBackend {
    child: Child,
}

impl LocalBackend {
    pub fn new(child: Child) -> Self {
        Self { child }
    }
}

impl ProcessBackend for LocalBackend {
    fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    fn take_stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>> {
        self.child
            .stdin
            .take()
            .map(|s| Box::new(s) as Box<dyn AsyncWrite + Unpin + Send>)
    }

    fn take_stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        self.child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>)
    }

    fn take_stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>> {
        self.child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>)
    }

    fn kill(&mut self) -> Pin<Box<dyn Future<Output = Result<(), AgentsError>> + Send + '_>> {
        Box::pin(async move { self.child.kill().await.map_err(AgentsError::Io) })
    }

    fn wait(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>
    {
        Box::pin(async move { self.child.wait().await.map_err(AgentsError::Io) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    async fn spawn_cat() -> LocalBackend {
        let child = tokio::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn cat");
        LocalBackend::new(child)
    }

    #[rstest]
    #[tokio::test]
    async fn when_local_backend_created_from_child_then_is_alive_returns_true() {
        let mut backend = spawn_cat().await;
        assert!(backend.is_alive());
        let _ = backend.kill().await;
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_stdin_taken_once_then_returns_some() {
        let mut backend = spawn_cat().await;
        let stdin = backend.take_stdin();
        assert!(stdin.is_some());
        let _ = backend.kill().await;
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_stdin_taken_twice_then_second_returns_none() {
        let mut backend = spawn_cat().await;
        let _first = backend.take_stdin();
        let second = backend.take_stdin();
        assert!(second.is_none());
        let _ = backend.kill().await;
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_stdout_taken_then_returns_some() {
        let mut backend = spawn_cat().await;
        let stdout = backend.take_stdout();
        assert!(stdout.is_some());
        let _ = backend.kill().await;
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_stderr_taken_then_returns_some() {
        let mut backend = spawn_cat().await;
        let stderr = backend.take_stderr();
        assert!(stderr.is_some());
        let _ = backend.kill().await;
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_kill_called_on_running_process_then_succeeds() {
        let mut backend = spawn_cat().await;
        let result = backend.kill().await;
        assert!(result.is_ok());
        let _ = backend.wait().await;
    }

    #[rstest]
    #[tokio::test]
    async fn when_wait_called_after_kill_then_returns_exit_status() {
        let mut backend = spawn_cat().await;
        let _ = backend.kill().await;
        let result = backend.wait().await;
        assert!(result.is_ok());
    }
}
