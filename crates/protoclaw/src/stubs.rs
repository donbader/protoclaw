use protoclaw_core::{Manager, ManagerError};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum StubCommand {}

pub struct StubAgentsManager;
pub struct StubChannelsManager;

impl Manager for StubAgentsManager {
    type Command = StubCommand;

    fn name(&self) -> &str {
        "agents"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        tracing::info!(manager = self.name(), "manager started");
        Ok(())
    }

    async fn run(self, cancel: CancellationToken) -> Result<(), ManagerError> {
        tracing::info!(manager = self.name(), "manager running");
        cancel.cancelled().await;
        tracing::info!(manager = self.name(), "manager stopping");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        true
    }
}

impl Manager for StubChannelsManager {
    type Command = StubCommand;

    fn name(&self) -> &str {
        "channels"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        tracing::info!(manager = self.name(), "manager started");
        Ok(())
    }

    async fn run(self, cancel: CancellationToken) -> Result<(), ManagerError> {
        tracing::info!(manager = self.name(), "manager running");
        cancel.cancelled().await;
        tracing::info!(manager = self.name(), "manager stopping");
        Ok(())
    }

    async fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_agents_manager_name() {
        let m = StubAgentsManager;
        assert_eq!(m.name(), "agents");
    }

    #[test]
    fn stub_channels_manager_name() {
        let m = StubChannelsManager;
        assert_eq!(m.name(), "channels");
    }

    #[tokio::test]
    async fn stub_agents_start_returns_ok() {
        let mut m = StubAgentsManager;
        assert!(m.start().await.is_ok());
    }

    #[tokio::test]
    async fn stub_channels_start_returns_ok() {
        let mut m = StubChannelsManager;
        assert!(m.start().await.is_ok());
    }

    #[tokio::test]
    async fn stub_agents_health_check_returns_true() {
        let m = StubAgentsManager;
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn stub_channels_health_check_returns_true() {
        let m = StubChannelsManager;
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn stub_agents_run_blocks_until_cancelled() {
        let token = CancellationToken::new();
        let t = token.clone();
        let handle = tokio::spawn(async move {
            StubAgentsManager.run(t).await
        });
        token.cancel();
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn stub_channels_run_blocks_until_cancelled() {
        let token = CancellationToken::new();
        let t = token.clone();
        let handle = tokio::spawn(async move {
            StubChannelsManager.run(t).await
        });
        token.cancel();
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
