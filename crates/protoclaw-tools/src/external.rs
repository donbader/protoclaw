use std::sync::Arc;

use protoclaw_config::ToolConfig;
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
    pub async fn spawn(name: &str, config: &ToolConfig) -> Result<Self, ToolsError> {
        let binary = config.binary.as_deref()
            .ok_or_else(|| ToolsError::ExternalServerFailed(format!("{name}: no binary specified")))?;
        let mut cmd = Command::new(binary);
        cmd.args(&config.args);

        let child_transport =
            rmcp::transport::child_process::TokioChildProcess::new(cmd)
                .map_err(|e| ToolsError::ExternalServerFailed(format!("{name}: {e}")))?;

        let client: RunningService<RoleClient, ()> =
            rmcp::serve_client((), child_transport)
                .await
                .map_err(|e| ToolsError::ExternalServerFailed(format!("{name}: {e}")))?;

        Ok(Self {
            name: name.to_string(),
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
    use rstest::rstest;
    use std::collections::HashMap;

    #[tokio::test]
    async fn when_external_mcp_server_spawned_with_nonexistent_binary_then_returns_error() {
        let config = ToolConfig {
            tool_type: "mcp".into(),
            binary: Some("/nonexistent/binary/path".into()),
            args: vec![],
            enabled: true,
            module: None,
            description: String::new(),
            input_schema: None,
            sandbox: Default::default(),
            options: HashMap::new(),
        };
        let result = ExternalMcpServer::spawn("bad-server", &config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("bad-server"), "error should contain server name: {err}");
    }
}
