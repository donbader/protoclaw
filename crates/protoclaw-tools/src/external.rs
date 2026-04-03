use std::sync::Arc;

use protoclaw_config::McpServerConfig;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool as RmcpTool};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceError};
use tokio::process::Command;

use crate::error::ToolsError;

pub struct ExternalMcpServer {
    pub name: String,
    client: Arc<RunningService<RoleClient, ()>>,
}

impl std::fmt::Debug for ExternalMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalMcpServer")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl ExternalMcpServer {
    pub async fn spawn(config: &McpServerConfig) -> Result<Self, ToolsError> {
        let mut cmd = Command::new(&config.binary);
        cmd.args(&config.args);

        let child_transport =
            rmcp::transport::child_process::TokioChildProcess::new(cmd)
                .map_err(|e| ToolsError::ExternalServerFailed(format!("{}: {e}", config.name)))?;

        let client: RunningService<RoleClient, ()> =
            rmcp::serve_client((), child_transport)
                .await
                .map_err(|e| ToolsError::ExternalServerFailed(format!("{}: {e}", config.name)))?;

        Ok(Self {
            name: config.name.clone(),
            client: Arc::new(client),
        })
    }

    pub async fn list_tools(&self) -> Result<Vec<RmcpTool>, ToolsError> {
        let result = self
            .client
            .list_tools(None)
            .await
            .map_err(|e: ServiceError| ToolsError::ProxyError(e.to_string()))?;
        Ok(result.tools)
    }

    pub async fn call_tool(
        &self,
        params: CallToolRequestParams,
    ) -> Result<CallToolResult, ToolsError> {
        self.client
            .call_tool(params)
            .await
            .map_err(|e: ServiceError| ToolsError::ProxyError(e.to_string()))
    }

    pub async fn shutdown(self) {
        if let Ok(client) = Arc::try_unwrap(self.client) {
            let _ = client.cancel().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn external_mcp_server_spawn_nonexistent_binary_returns_error() {
        let config = McpServerConfig {
            name: "bad-server".into(),
            binary: "/nonexistent/binary/path".into(),
            args: vec![],
            enabled: true,
        };
        let result = ExternalMcpServer::spawn(&config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("bad-server"), "error should contain server name: {err}");
    }
}
