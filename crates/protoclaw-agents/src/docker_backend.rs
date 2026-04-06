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
