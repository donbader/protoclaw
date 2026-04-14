use std::sync::Arc;

use anyclaw_config::ToolConfig;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool as RmcpTool};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceError};
use tokio::process::Command;

use crate::error::ToolsError;

/// Manages a single external MCP server subprocess spawned via rmcp's child process transport.
///
/// Each `ExternalMcpServer` owns an rmcp client connected to the subprocess's stdio.
/// Failed servers are logged and skipped — they don't block startup.
pub struct ExternalMcpServer {
    /// Logical tool name (matches the config key).
    pub name: String,
    client: Arc<RunningService<RoleClient, ()>>,
}

// D-03: Config options are arbitrary user-defined values (HashMap<String, serde_json::Value>
// in McpServerConfig). Cannot be typed — users define custom key/value pairs per tool.
fn serialize_option_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.to_owned(),
        other => other.to_string(),
    }
}

impl std::fmt::Debug for ExternalMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalMcpServer")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl ExternalMcpServer {
    /// Spawn an external MCP server subprocess from the given tool config.
    pub async fn spawn(name: &str, config: &ToolConfig) -> Result<Self, ToolsError> {
        let binary = config.binary.as_deref().ok_or_else(|| {
            ToolsError::ExternalServerFailed(format!("{name}: no binary specified"))
        })?;
        let mut cmd = Command::new(binary);
        cmd.args(&config.args);
        for (key, value) in &config.options {
            let val = serialize_option_value(value);
            cmd.env(key, &val);
        }

        let child_transport = rmcp::transport::child_process::TokioChildProcess::new(cmd)
            .map_err(|e| ToolsError::ExternalServerFailed(format!("{name}: {e}")))?;

        let client: RunningService<RoleClient, ()> = rmcp::serve_client((), child_transport)
            .await
            .map_err(|e| ToolsError::ExternalServerFailed(format!("{name}: {e}")))?;

        Ok(Self {
            name: name.to_string(),
            client: Arc::new(client),
        })
    }

    /// List all tools advertised by this external MCP server.
    pub async fn list_tools(&self) -> Result<Vec<RmcpTool>, ToolsError> {
        let result = self
            .client
            .list_tools(None)
            .await
            .map_err(|e: ServiceError| ToolsError::ProxyError(e.to_string()))?;
        Ok(result.tools)
    }

    /// Invoke a tool on this external MCP server.
    pub async fn call_tool(
        &self,
        params: CallToolRequestParams,
    ) -> Result<CallToolResult, ToolsError> {
        self.client
            .call_tool(params)
            .await
            .map_err(|e: ServiceError| ToolsError::ProxyError(e.to_string()))
    }

    /// Gracefully shut down the external MCP server subprocess.
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
            tool_type: anyclaw_config::ToolType::Mcp,
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
        assert!(
            err.contains("bad-server"),
            "error should contain server name: {err}"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_spawn_called_with_no_binary_then_returns_error() {
        let config = ToolConfig {
            tool_type: anyclaw_config::ToolType::Mcp,
            binary: None,
            args: vec![],
            enabled: true,
            module: None,
            description: String::new(),
            input_schema: None,
            sandbox: Default::default(),
            options: HashMap::new(),
        };
        let result = ExternalMcpServer::spawn("no-binary", &config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no binary specified"),
            "error should mention missing binary: {err}"
        );
    }

    #[rstest]
    #[case::string_value(serde_json::Value::String("hello".into()), "hello")]
    #[case::number_value(serde_json::json!(42), "42")]
    #[case::bool_true(serde_json::Value::Bool(true), "true")]
    #[case::bool_false(serde_json::Value::Bool(false), "false")]
    #[case::array_value(serde_json::json!([1, 2, 3]), "[1,2,3]")]
    #[case::object_value(serde_json::json!({"k": "v"}), r#"{"k":"v"}"#)]
    fn when_serialize_option_value_called_then_returns_expected_string(
        #[case] value: serde_json::Value,
        #[case] expected: &str,
    ) {
        assert_eq!(serialize_option_value(&value), expected);
    }
}
