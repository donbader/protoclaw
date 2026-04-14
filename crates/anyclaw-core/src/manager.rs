use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::ManagerError;

/// Contract that every manager (tools, agents, channels) must implement.
///
/// Lifecycle: construct → [`start()`](Self::start) (synchronous setup: spawn subprocesses,
/// bind ports) → [`run()`](Self::run) (async event loop, consumes `self`).
/// Both phases are required and must be called in order.
pub trait Manager: Send + 'static {
    /// The command type this manager accepts via its [`ManagerHandle`].
    type Command: Send + 'static;

    /// Human-readable name used in logs and health checks (e.g. "tools", "agents", "channels").
    fn name(&self) -> &str;
    /// Synchronous setup phase: spawn subprocesses, bind ports, validate config.
    fn start(&mut self) -> impl std::future::Future<Output = Result<(), ManagerError>> + Send;
    /// Async event loop that processes commands until the cancellation token fires.
    /// Consumes `self` — a manager cannot be run twice.
    fn run(
        self,
        cancel: CancellationToken,
    ) -> impl std::future::Future<Output = Result<(), ManagerError>> + Send;
    /// Return `true` if the manager is operating normally.
    fn health_check(&self) -> impl std::future::Future<Output = bool> + Send;
}

/// Typed wrapper around `mpsc::Sender<C>` for sending commands to a manager.
///
/// This is the only sanctioned way to communicate across manager boundaries.
/// Cloneable so multiple producers (e.g. supervisor + other managers) can hold a handle.
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
    /// Wrap an existing `mpsc::Sender` as a typed manager handle.
    pub fn new(sender: mpsc::Sender<C>) -> Self {
        Self { sender }
    }

    /// Send a command to the manager, returning [`ManagerError::SendFailed`] if the channel is closed.
    pub async fn send(&self, cmd: C) -> Result<(), ManagerError> {
        self.sender
            .send(cmd)
            .await
            .map_err(|e| ManagerError::SendFailed(e.to_string()))
    }
}

/// Generic manager commands used in tests and as a baseline command set.
#[derive(Debug)]
pub enum ManagerCommand {
    /// Request a graceful shutdown.
    Shutdown,
    /// Request a health check.
    HealthCheck,
}

#[cfg(test)]
mod tests {
    use super::*;

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
