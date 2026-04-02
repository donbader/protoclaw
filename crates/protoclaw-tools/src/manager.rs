use protoclaw_config::McpServerConfig;
use protoclaw_core::{Manager, ManagerError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct McpServerUrl {
    pub name: String,
    pub url: String,
}

pub enum ToolsCommand {
    GetMcpUrls {
        reply: tokio::sync::oneshot::Sender<Vec<McpServerUrl>>,
    },
    Shutdown,
}

pub struct ToolsManager {
    configs: Vec<McpServerConfig>,
    server_urls: Vec<McpServerUrl>,
    server_handles: Vec<tokio::task::JoinHandle<()>>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<ToolsCommand>>,
}

impl ToolsManager {
    pub fn new(configs: Vec<McpServerConfig>) -> Self {
        Self {
            configs,
            server_urls: Vec::new(),
            server_handles: Vec::new(),
            cmd_rx: None,
        }
    }

    pub fn with_cmd_rx(mut self, rx: tokio::sync::mpsc::Receiver<ToolsCommand>) -> Self {
        self.cmd_rx = Some(rx);
        self
    }

    pub fn server_urls(&self) -> &[McpServerUrl] {
        &self.server_urls
    }
}

impl Manager for ToolsManager {
    type Command = ToolsCommand;

    fn name(&self) -> &str {
        "tools"
    }

    async fn start(&mut self) -> Result<(), ManagerError> {
        for config in &self.configs {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| ManagerError::Internal(format!("bind failed for {}: {e}", config.name)))?;

            let addr = listener
                .local_addr()
                .map_err(|e| ManagerError::Internal(format!("local_addr failed: {e}")))?;

            let url = format!("http://127.0.0.1:{}", addr.port());
            tracing::info!(name = %config.name, %url, "MCP server listening");

            self.server_urls.push(McpServerUrl {
                name: config.name.clone(),
                url,
            });

            let handle = tokio::spawn(async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };
                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf).await;
                        let resp = b"{\"jsonrpc\":\"2.0\",\"result\":{}}";
                        let _ = stream.write_all(resp).await;
                    });
                }
            });

            self.server_handles.push(handle);
        }

        tracing::info!(manager = self.name(), count = self.server_urls.len(), "manager started");
        Ok(())
    }

    async fn run(mut self, cancel: CancellationToken) -> Result<(), ManagerError> {
        let mut rx = self.cmd_rx.take().unwrap_or_else(|| {
            let (_tx, rx) = tokio::sync::mpsc::channel::<ToolsCommand>(16);
            rx
        });

        tracing::info!(manager = self.name(), "manager running");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!(manager = self.name(), "manager stopping");
                    break;
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(ToolsCommand::GetMcpUrls { reply }) => {
                            let _ = reply.send(self.server_urls.clone());
                        }
                        Some(ToolsCommand::Shutdown) | None => {
                            break;
                        }
                    }
                }
            }
        }

        for handle in &self.server_handles {
            handle.abort();
        }

        Ok(())
    }

    async fn health_check(&self) -> bool {
        !self.server_urls.is_empty() || self.configs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_manager_name() {
        let m = ToolsManager::new(vec![]);
        assert_eq!(m.name(), "tools");
    }

    #[tokio::test]
    async fn tools_manager_start_no_configs() {
        let mut m = ToolsManager::new(vec![]);
        assert!(m.start().await.is_ok());
        assert!(m.server_urls().is_empty());
    }

    #[tokio::test]
    async fn tools_manager_start_with_config() {
        let config = McpServerConfig {
            name: "test-mcp".into(),
            binary: "unused".into(),
            args: vec![],
        };
        let mut m = ToolsManager::new(vec![config]);
        assert!(m.start().await.is_ok());
        assert_eq!(m.server_urls().len(), 1);
        assert!(m.server_urls()[0].url.starts_with("http://127.0.0.1:"));
        assert_eq!(m.server_urls()[0].name, "test-mcp");

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_health_check_no_configs() {
        let m = ToolsManager::new(vec![]);
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn tools_manager_health_check_after_start() {
        let config = McpServerConfig {
            name: "test".into(),
            binary: "unused".into(),
            args: vec![],
        };
        let mut m = ToolsManager::new(vec![config]);
        m.start().await.unwrap();
        assert!(m.health_check().await);

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_run_stops_on_cancel() {
        let mut m = ToolsManager::new(vec![]);
        m.start().await.unwrap();

        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            c.cancel();
        });

        let result = m.run(cancel).await;
        assert!(result.is_ok());
    }
}
