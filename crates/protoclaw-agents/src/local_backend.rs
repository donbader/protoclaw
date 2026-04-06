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
