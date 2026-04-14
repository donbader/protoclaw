use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool as RmcpTool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};

use crate::trait_def::DynTool;

/// MCP tool server that registers [`DynTool`] implementations and serves them over stdio.
///
/// Implements rmcp's `ServerHandler` to handle `list_tools` and `call_tool` MCP methods.
pub struct ToolServer {
    tools: HashMap<String, Box<dyn DynTool>>,
    server_info: ServerInfo,
}

impl ToolServer {
    /// Create a new server from a list of tools, registering each by name.
    pub fn new(tools: Vec<Box<dyn DynTool>>) -> Self {
        let map: HashMap<String, Box<dyn DynTool>> = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();

        let mut server_info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        server_info.server_info = Implementation::new("anyclaw-sdk-tool", "0.1.0");

        Self {
            tools: map,
            server_info,
        }
    }

    /// Run the MCP server over stdin/stdout until the client disconnects.
    pub async fn serve_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use rmcp::ServiceExt;
        let service = self
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await?;
        service.waiting().await?;
        Ok(())
    }

    /// Convert all registered tools into rmcp [`RmcpTool`] descriptors for `list_tools`.
    ///
    /// D-03: `input_schema()` returns `serde_json::Value` (JSON Schema has no fixed Rust type),
    /// so the conversion to rmcp's `Map<String, Value>` is inherently Value-based.
    pub fn build_tool_list(&self) -> Vec<RmcpTool> {
        self.tools
            .values()
            .map(|t| {
                let name = t.name().to_string();
                let desc = t.description().to_string();
                let schema = t.input_schema();
                let schema_obj: serde_json::Map<String, serde_json::Value> = match schema {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                };
                RmcpTool::new(name, desc, Arc::new(schema_obj))
            })
            .collect()
    }

    /// Dispatch a tool call by name, returning an MCP `CallToolResult`.
    ///
    /// Returns an MCP protocol error only for unknown tool names; execution
    /// failures are returned as `CallToolResult::error` (content-level).
    ///
    /// D-03: `arguments` is `Map<String, Value>` from rmcp's `CallToolRequestParam` —
    /// tool input shape is defined by the tool's JSON Schema, not a static Rust type.
    /// The conversion to `Value::Object` and pattern matching on `Value::String` in the
    /// result both flow from the Tool trait being Value-based (D-03).
    pub async fn dispatch_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| McpError::invalid_params(format!("unknown tool: {name}"), None))?;

        let input = arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);

        match tool.execute(input).await {
            Ok(output) => {
                let text = match output {
                    serde_json::Value::String(s) => s,
                    other => serde_json::to_string(&other).unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "failed to serialize tool output to string, using empty string");
                        String::default()
                    }),
                };
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

impl ServerHandler for ToolServer {
    fn get_info(&self) -> ServerInfo {
        self.server_info.clone()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.build_tool_list()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.dispatch_tool(request.name.as_ref(), request.arguments)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolSdkError;
    use crate::trait_def::Tool;
    use rstest::rstest;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input back"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "message": {"type": "string"} },
                "required": ["message"]
            })
        }
        async fn execute(
            &self,
            input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Ok(input)
        }
    }

    struct FailTool;

    impl Tool for FailTool {
        fn name(&self) -> &str {
            "fail"
        }
        fn description(&self) -> &str {
            "Always fails"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(
            &self,
            _input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Err(ToolSdkError::ExecutionFailed("intentional failure".into()))
        }
    }

    #[test]
    fn when_tool_server_constructed_with_tools_then_tools_registered() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);
        assert_eq!(server.tools.len(), 1);
    }

    #[test]
    fn when_tool_list_built_then_contains_all_registered_tools_with_metadata() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool), Box::new(FailTool)];
        let server = ToolServer::new(tools);
        let list = server.build_tool_list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"fail"));
        let echo = list.iter().find(|t| t.name.as_ref() == "echo").unwrap();
        assert_eq!(echo.description.as_deref(), Some("Echoes input back"));
    }

    #[tokio::test]
    async fn when_known_tool_dispatched_then_returns_successful_result() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);

        let mut args = serde_json::Map::new();
        args.insert("message".into(), serde_json::json!("hello"));

        let result = server.dispatch_tool("echo", Some(args)).await.unwrap();
        assert!(result.is_error.is_none() || result.is_error == Some(false));
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn when_unknown_tool_dispatched_then_returns_invalid_params_error() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);

        let result = server.dispatch_tool("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn when_tool_execute_returns_error_then_dispatch_returns_error_result() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(FailTool)];
        let server = ToolServer::new(tools);

        let result = server.dispatch_tool("fail", None).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn when_tool_sdk_error_checked_then_implements_std_error() {
        let err = ToolSdkError::ExecutionFailed("test".into());
        let _: &dyn std::error::Error = &err;
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn when_tool_impl_created_then_name_description_and_schema_accessible() {
        use crate::trait_def::Tool;
        let tool = EchoTool;
        assert_eq!(Tool::name(&tool), "echo");
        assert_eq!(Tool::description(&tool), "Echoes input back");
        let schema = Tool::input_schema(&tool);
        assert!(schema.is_object());
    }

    #[test]
    fn when_tool_cast_to_dyn_trait_object_then_compiles() {
        let _tool: Box<dyn DynTool> = Box::new(EchoTool);
    }

    struct StaticTool {
        tool_name: &'static str,
        payload: &'static str,
    }

    impl Tool for StaticTool {
        fn name(&self) -> &str {
            self.tool_name
        }

        fn description(&self) -> &str {
            "Returns a static payload"
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Ok(serde_json::json!({"tool": self.payload}))
        }
    }

    #[rstest]
    #[test]
    fn when_tool_server_new_called_then_tools_are_registered_by_name() {
        let tools: Vec<Box<dyn DynTool>> = vec![
            Box::new(StaticTool {
                tool_name: "alpha",
                payload: "A",
            }),
            Box::new(StaticTool {
                tool_name: "beta",
                payload: "B",
            }),
        ];
        let server = ToolServer::new(tools);

        assert_eq!(server.tools.len(), 2);
        assert!(server.tools.contains_key("alpha"));
        assert!(server.tools.contains_key("beta"));
    }

    #[rstest]
    #[tokio::test]
    async fn when_tool_server_dispatches_by_name_then_matching_tool_handles_request() {
        let tools: Vec<Box<dyn DynTool>> = vec![
            Box::new(StaticTool {
                tool_name: "alpha",
                payload: "A",
            }),
            Box::new(StaticTool {
                tool_name: "beta",
                payload: "B",
            }),
        ];
        let server = ToolServer::new(tools);

        let result = server.dispatch_tool("beta", None).await.unwrap();

        assert_eq!(result.is_error, Some(false));
        assert_eq!(
            result.content,
            vec![Content::text(r#"{"tool":"B"}"#.to_string())]
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_tool_server_dispatches_unknown_name_then_invalid_params_includes_name() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);

        let error = server
            .dispatch_tool("missing-tool", None)
            .await
            .unwrap_err();

        assert!(error.message.contains("unknown tool: missing-tool"));
    }
}
