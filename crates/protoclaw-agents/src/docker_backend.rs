use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::backend::ProcessBackend;
use crate::error::AgentsError;

pub struct DockerBackend;

impl ProcessBackend for DockerBackend {
    fn is_alive(&mut self) -> bool {
        false
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
        Box::pin(async move { Ok(()) })
    }

    fn wait(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<std::process::ExitStatus, AgentsError>> + Send + '_>>
    {
        Box::pin(async move {
            std::process::Command::new("true")
                .status()
                .map_err(AgentsError::Io)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_docker_backend_is_alive_then_returns_false() {
        let mut backend = DockerBackend;
        assert!(!backend.is_alive());
    }

    #[rstest]
    fn when_docker_backend_take_stdin_then_returns_none() {
        let mut backend = DockerBackend;
        assert!(backend.take_stdin().is_none());
    }

    #[rstest]
    fn when_docker_backend_take_stdout_then_returns_none() {
        let mut backend = DockerBackend;
        assert!(backend.take_stdout().is_none());
    }

    #[rstest]
    fn when_docker_backend_take_stderr_then_returns_none() {
        let mut backend = DockerBackend;
        assert!(backend.take_stderr().is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_docker_backend_kill_then_returns_ok() {
        let mut backend = DockerBackend;
        let result = backend.kill().await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_docker_backend_wait_then_returns_exit_status() {
        let mut backend = DockerBackend;
        let result = backend.wait().await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.success());
    }
}
