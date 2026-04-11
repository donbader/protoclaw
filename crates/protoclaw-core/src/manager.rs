use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::ManagerError;

pub trait Manager: Send + 'static {
    type Command: Send + 'static;

    fn name(&self) -> &str;
    fn start(&mut self) -> impl std::future::Future<Output = Result<(), ManagerError>> + Send;
    fn run(
        self,
        cancel: CancellationToken,
    ) -> impl std::future::Future<Output = Result<(), ManagerError>> + Send;
    fn health_check(&self) -> impl std::future::Future<Output = bool> + Send;
}

#[derive(Debug)]
pub struct ManagerHandle<C: Send + 'static> {
    sender: mpsc::Sender<C>,
}

impl<C: Send + 'static> Clone for ManagerHandle<C> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<C: Send + 'static> ManagerHandle<C> {
    pub fn new(sender: mpsc::Sender<C>) -> Self {
        Self { sender }
    }

    pub async fn send(&self, cmd: C) -> Result<(), ManagerError> {
        self.sender
            .send(cmd)
            .await
            .map_err(|e| ManagerError::SendFailed(e.to_string()))
    }
}

#[derive(Debug)]
pub enum ManagerCommand {
    Shutdown,
    HealthCheck,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn when_command_sent_via_handle_then_receiver_gets_command() {
        let (tx, mut rx) = mpsc::channel::<ManagerCommand>(8);
        let handle = ManagerHandle::new(tx);

        handle.send(ManagerCommand::Shutdown).await.unwrap();

        let cmd = rx.recv().await.unwrap();
        assert!(matches!(cmd, ManagerCommand::Shutdown));
    }

    #[tokio::test]
    async fn when_channel_closed_then_handle_send_returns_error() {
        let (tx, rx) = mpsc::channel::<ManagerCommand>(8);
        let handle = ManagerHandle::new(tx);
        drop(rx);

        let result = handle.send(ManagerCommand::HealthCheck).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ManagerError::SendFailed(_)));
    }
}
