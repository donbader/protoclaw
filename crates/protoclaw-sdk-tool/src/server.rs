use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool as RmcpTool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};

use crate::trait_def::Tool;

pub struct ToolServer {
    tools: HashMap<String, Box<dyn Tool>>,
    server_info: ServerInfo,
}

impl ToolServer {
    pub fn new(tools: Vec<Box<dyn Tool>>) -> Self {
        let map: HashMap<String, Box<dyn Tool>> =
            tools.into_iter().map(|t| (t.name().to_string(), t)).collect();

        let mut server_info = ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        );
        server_info.server_info = Implementation::new("protoclaw-sdk-tool", "0.1.0");

        Self {
            tools: map,
            server_info,
        }
    }

    pub async fn serve_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use rmcp::ServiceExt;
        let service = self
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await?;
        service.waiting().await?;
        Ok(())
    }

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

    pub async fn dispatch_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let tool = self.tools.get(name).ok_or_else(|| {
            McpError::invalid_params(format!("unknown tool: {name}"), None)
        })?;

        let input = arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);

        match tool.execute(input).await {
            Ok(output) => {
                let text = match output {
                    serde_json::Value::String(s) => s,
                    other => serde_json::to_string(&other).unwrap_or_default(),
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
        self.dispatch_tool(request.name.as_ref(), request.arguments).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::error::ToolSdkError;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str { "echo" }
        fn description(&self) -> &str { "Echoes input back" }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "message": {"type": "string"} },
                "required": ["message"]
            })
        }
        async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError> {
            Ok(input)
        }
    }

    struct FailTool;

    #[async_trait]
    impl Tool for FailTool {
        fn name(&self) -> &str { "fail" }
        fn description(&self) -> &str { "Always fails" }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError> {
            Err(ToolSdkError::ExecutionFailed("intentional failure".into()))
        }
    }

    #[test]
    fn tool_server_new_constructs_with_tools() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);
        assert_eq!(server.tools.len(), 1);
    }

    #[test]
    fn tool_server_generates_correct_tool_list() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool), Box::new(FailTool)];
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
    async fn tool_server_dispatches_to_correct_tool() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);

        let mut args = serde_json::Map::new();
        args.insert("message".into(), serde_json::json!("hello"));

        let result = server.dispatch_tool("echo", Some(args)).await.unwrap();
        assert!(result.is_error.is_none() || result.is_error == Some(false));
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn tool_server_returns_error_for_unknown_tool() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(EchoTool)];
        let server = ToolServer::new(tools);

        let result = server.dispatch_tool("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tool_server_returns_error_result_for_failing_tool() {
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(FailTool)];
        let server = ToolServer::new(tools);

        let result = server.dispatch_tool("fail", None).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn tool_error_implements_std_error() {
        let err = ToolSdkError::ExecutionFailed("test".into());
        let _: &dyn std::error::Error = &err;
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn tool_trait_mock_compiles() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
        assert_eq!(tool.description(), "Echoes input back");
        let schema = tool.input_schema();
        assert!(schema.is_object());
    }
}
