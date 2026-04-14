use anyclaw_sdk_tool::{DynTool, ToolServer};

#[cfg(test)]
use anyclaw_sdk_tool::Tool;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool as RmcpTool};

/// In-process MCP host wrapping a ToolServer.
///
/// McpHost holds native tools (built-in + WASM) and exposes them for
/// aggregation with external MCP server tools.
pub struct McpHost {
    server: ToolServer,
}

impl McpHost {
    pub fn new(tools: Vec<Box<dyn DynTool>>) -> Self {
        Self {
            server: ToolServer::new(tools),
        }
    }

    pub fn tool_list(&self) -> Vec<RmcpTool> {
        self.server.build_tool_list()
    }

    // D-03: args use serde_json::Value — tool call arguments are arbitrary JSON
    // defined by each tool's input_schema. Cannot be typed at this layer.
    pub async fn dispatch_tool(
        &self,
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        self.server.dispatch_tool(name, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_sdk_tool::ToolSdkError;

    // D-03: EchoTool implements the Tool trait which uses serde_json::Value
    // for input_schema/execute — extensible tool boundary, cannot be typed.
    #[allow(clippy::disallowed_types)]
    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {"msg": {"type": "string"}}})
        }
        async fn execute(
            &self,
            input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Ok(input)
        }
    }

    #[test]
    fn when_mcp_host_created_with_no_tools_then_tool_list_is_empty() {
        let host = McpHost::new(vec![]);
        assert!(host.tool_list().is_empty());
    }

    #[test]
    fn when_mcp_host_created_with_tools_then_tool_list_contains_them() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let host = McpHost::new(tools);
        let list = host.tool_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name.as_ref(), "echo");
    }

    #[tokio::test]
    async fn when_known_tool_dispatched_via_mcp_host_then_returns_result() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let host = McpHost::new(tools);

        let mut args = serde_json::Map::new();
        args.insert("msg".into(), serde_json::json!("hello"));

        let result = host.dispatch_tool("echo", Some(args)).await.unwrap();
        assert!(result.is_error.is_none() || result.is_error == Some(false));
    }

    #[tokio::test]
    async fn when_unknown_tool_dispatched_via_mcp_host_then_returns_error() {
        let host = McpHost::new(vec![]);
        let result = host.dispatch_tool("nonexistent", None).await;
        assert!(result.is_err());
    }
}
