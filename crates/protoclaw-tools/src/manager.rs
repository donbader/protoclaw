use std::collections::HashMap;
use std::sync::Arc;

use protoclaw_config::ToolConfig;
use protoclaw_core::{Manager, ManagerError};
use protoclaw_sdk_tool::Tool;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool as RmcpTool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};
use tokio_util::sync::CancellationToken;

use crate::external::ExternalMcpServer;
use crate::mcp_host::McpHost;
use crate::wasm_runner::WasmToolRunner;
use crate::wasm_tool::WasmTool;

#[derive(Clone)]
pub struct McpServerUrl {
    pub name: String,
    pub url: String,
}

pub enum ToolsCommand {
    GetMcpUrls {
        tool_names: Option<Vec<String>>,
        reply: tokio::sync::oneshot::Sender<Vec<McpServerUrl>>,
    },
    Shutdown,
}

pub struct AggregatedToolServer {
    native_host: Arc<McpHost>,
    external_servers: Arc<Vec<ExternalMcpServer>>,
    server_info: ServerInfo,
}

impl AggregatedToolServer {
    pub fn new(
        native_host: Arc<McpHost>,
        external_servers: Arc<Vec<ExternalMcpServer>>,
    ) -> Self {
        let mut server_info =
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        server_info.server_info = Implementation::new("protoclaw-tools", "0.1.0");
        Self {
            native_host,
            external_servers,
            server_info,
        }
    }

    async fn aggregate_tool_list(&self) -> Vec<RmcpTool> {
        let mut tools = self.native_host.tool_list();
        for ext in self.external_servers.iter() {
            if let Ok(ext_tools) = ext.list_tools().await {
                tools.extend(ext_tools);
            }
        }
        tools
    }

    async fn route_call(
        &self,
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        if let Ok(result) = self.native_host.dispatch_tool(name, args.clone()).await {
            return Ok(result);
        }
        for ext in self.external_servers.iter() {
            if let Ok(ext_tools) = ext.list_tools().await {
                if ext_tools.iter().any(|t| t.name.as_ref() == name) {
                    let mut params = CallToolRequestParams::new(name.to_string());
                    params.arguments = args;
                    return ext
                        .call_tool(params)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None));
                }
            }
        }
        Err(McpError::invalid_params(
            format!("unknown tool: {name}"),
            None,
        ))
    }
}

impl ServerHandler for AggregatedToolServer {
    fn get_info(&self) -> ServerInfo {
        self.server_info.clone()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self.aggregate_tool_list().await;
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.route_call(request.name.as_ref(), request.arguments)
            .await
    }
}

pub struct ToolsManager {
    tool_configs: HashMap<String, ToolConfig>,
    native_tools: Vec<Box<dyn Tool>>,
    server_urls: Vec<McpServerUrl>,
    server_handles: Vec<tokio::task::JoinHandle<()>>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<ToolsCommand>>,
    native_host: Option<Arc<McpHost>>,
    external_servers: Option<Arc<Vec<ExternalMcpServer>>>,
}

impl ToolsManager {
    pub fn new(tool_configs: HashMap<String, ToolConfig>) -> Self {
        Self {
            tool_configs,
            native_tools: Vec::new(),
            server_urls: Vec::new(),
            server_handles: Vec::new(),
            cmd_rx: None,
            native_host: None,
            external_servers: None,
        }
    }

    pub fn with_cmd_rx(mut self, rx: tokio::sync::mpsc::Receiver<ToolsCommand>) -> Self {
        self.cmd_rx = Some(rx);
        self
    }

    pub fn with_native_tools(mut self, tools: Vec<Box<dyn Tool>>) -> Self {
        self.native_tools = tools;
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
        let mut all_tools: Vec<Box<dyn Tool>> = std::mem::take(&mut self.native_tools);

        let wasm_configs: Vec<(String, ToolConfig)> = self.tool_configs.iter()
            .filter(|(_, c)| c.tool_type == "wasm" && c.enabled)
            .map(|(n, c)| (n.clone(), c.clone()))
            .collect();

        if !wasm_configs.is_empty() {
            let wasm_runner = Arc::new(
                WasmToolRunner::new()
                    .map_err(|e| ManagerError::Internal(format!("wasm runner: {e}")))?,
            );

            for (name, wasm_config) in &wasm_configs {
                match WasmTool::new(name.clone(), wasm_config.clone(), wasm_runner.clone()) {
                    Ok(tool) => {
                        tracing::info!(name = %tool.name(), "loaded WASM tool");
                        all_tools.push(Box::new(tool));
                    }
                    Err(e) => {
                        tracing::warn!(name = %name, error = %e, "failed to load WASM tool, skipping");
                    }
                }
            }
        }

        let native_host = Arc::new(McpHost::new(all_tools));
        self.native_host = Some(native_host.clone());

        let mut external_servers = Vec::new();
        let mcp_configs: Vec<(String, ToolConfig)> = self.tool_configs.iter()
            .filter(|(_, c)| c.tool_type == "mcp" && c.enabled)
            .map(|(n, c)| (n.clone(), c.clone()))
            .collect();

        for (name, config) in &mcp_configs {
            match ExternalMcpServer::spawn(name, config).await {
                Ok(server) => {
                    tracing::info!(name = %name, "spawned external MCP server");
                    external_servers.push(server);
                }
                Err(e) => {
                    tracing::warn!(name = %name, error = %e, "failed to spawn external MCP server, skipping");
                }
            }
        }
        let external_servers = Arc::new(external_servers);
        self.external_servers = Some(external_servers.clone());

        let aggregated = AggregatedToolServer::new(native_host, external_servers);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| ManagerError::Internal(format!("bind failed: {e}")))?;

        let addr = listener
            .local_addr()
            .map_err(|e| ManagerError::Internal(format!("local_addr failed: {e}")))?;

        let url = format!("http://127.0.0.1:{}", addr.port());
        tracing::info!(%url, "aggregated MCP server listening");

        self.server_urls.push(McpServerUrl {
            name: "protoclaw-tools".into(),
            url,
        });

        let handle = tokio::spawn(async move {
            let _aggregated = aggregated;
            loop {
                let Ok((_stream, _addr)) = listener.accept().await else {
                    break;
                };
                // TODO: Wire MCP protocol over accepted TCP streams (Phase 7+)
            }
        });
        self.server_handles.push(handle);

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
                        Some(ToolsCommand::GetMcpUrls { tool_names, reply }) => {
                            let urls = match tool_names {
                                Some(names) => self.server_urls.iter()
                                    .filter(|u| names.iter().any(|n| n == &u.name))
                                    .cloned()
                                    .collect(),
                                None => self.server_urls.clone(),
                            };
                            let _ = reply.send(urls);
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
        !self.server_urls.is_empty() || self.tool_configs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use protoclaw_sdk_tool::ToolSdkError;
    use rstest::rstest;

    struct DummyTool {
        tool_name: String,
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            &self.tool_name
        }
        fn description(&self) -> &str {
            "dummy"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(
            &self,
            input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Ok(input)
        }
    }

    #[test]
    fn when_tools_manager_name_queried_then_returns_tools() {
        let m = ToolsManager::new(HashMap::new());
        assert_eq!(m.name(), "tools");
    }

    #[tokio::test]
    async fn tools_manager_start_no_configs() {
        let mut m = ToolsManager::new(HashMap::new());
        assert!(m.start().await.is_ok());
        assert_eq!(m.server_urls().len(), 1);
        assert_eq!(m.server_urls()[0].name, "protoclaw-tools");
    }

    #[tokio::test]
    async fn tools_manager_health_check_no_configs() {
        let m = ToolsManager::new(HashMap::new());
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn tools_manager_health_check_after_start() {
        let mut m = ToolsManager::new(HashMap::new());
        m.start().await.unwrap();
        assert!(m.health_check().await);
        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_run_stops_on_cancel() {
        let mut m = ToolsManager::new(HashMap::new());
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

    #[tokio::test]
    async fn aggregated_tool_server_lists_native_tools() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(DummyTool { tool_name: "tool-a".into() }),
            Box::new(DummyTool { tool_name: "tool-b".into() }),
        ];
        let host = Arc::new(McpHost::new(tools));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let list = agg.aggregate_tool_list().await;
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"tool-a"));
        assert!(names.contains(&"tool-b"));
    }

    #[tokio::test]
    async fn aggregated_tool_server_routes_native_call() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(DummyTool { tool_name: "my-tool".into() }),
        ];
        let host = Arc::new(McpHost::new(tools));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("my-tool", None).await.unwrap();
        assert!(result.is_error.is_none() || result.is_error == Some(false));
    }

    #[tokio::test]
    async fn aggregated_tool_server_unknown_tool_returns_error() {
        let host = Arc::new(McpHost::new(vec![]));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tools_manager_with_native_tools_registers_them() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(DummyTool { tool_name: "native-1".into() }),
        ];
        let mut m = ToolsManager::new(HashMap::new()).with_native_tools(tools);
        m.start().await.unwrap();

        let host = m.native_host.as_ref().unwrap();
        let list = host.tool_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name.as_ref(), "native-1");

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_with_wasm_configs_loads_valid_tools() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let tool_configs = HashMap::from([("wasm-tool-1".to_string(), ToolConfig {
            tool_type: "wasm".into(),
            binary: None,
            args: vec![],
            enabled: true,
            module: Some(wasm_path),
            description: "test wasm tool".into(),
            input_schema: None,
            sandbox: protoclaw_config::WasmSandboxConfig::default(),
            options: HashMap::new(),
        })]);

        let mut m = ToolsManager::new(tool_configs);
        m.start().await.unwrap();

        let host = m.native_host.as_ref().unwrap();
        let list = host.tool_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name.as_ref(), "wasm-tool-1");

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_with_invalid_wasm_path_skips_and_continues() {
        let tool_configs = HashMap::from([("bad-tool".to_string(), ToolConfig {
            tool_type: "wasm".into(),
            binary: None,
            args: vec![],
            enabled: true,
            module: Some(std::path::PathBuf::from("/nonexistent/tool.wasm")),
            description: "bad".into(),
            input_schema: None,
            sandbox: protoclaw_config::WasmSandboxConfig::default(),
            options: HashMap::new(),
        })]);

        let mut m = ToolsManager::new(tool_configs);
        let result = m.start().await;
        assert!(result.is_ok());

        let host = m.native_host.as_ref().unwrap();
        let list = host.tool_list();
        assert!(list.is_empty());

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn tools_manager_wasm_tools_appear_in_aggregated_list() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let native_tools: Vec<Box<dyn Tool>> = vec![
            Box::new(DummyTool { tool_name: "native-1".into() }),
        ];
        let tool_configs = HashMap::from([("wasm-1".to_string(), ToolConfig {
            tool_type: "wasm".into(),
            binary: None,
            args: vec![],
            enabled: true,
            module: Some(wasm_path),
            description: "wasm".into(),
            input_schema: None,
            sandbox: protoclaw_config::WasmSandboxConfig::default(),
            options: HashMap::new(),
        })]);

        let mut m = ToolsManager::new(tool_configs).with_native_tools(native_tools);
        m.start().await.unwrap();

        let host = m.native_host.as_ref().unwrap();
        let list = host.tool_list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"native-1"));
        assert!(names.contains(&"wasm-1"));

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn get_mcp_urls_with_no_filter_returns_all() {
        let mut m = ToolsManager::new(HashMap::new());
        m.start().await.unwrap();

        m.server_urls.push(McpServerUrl { name: "tool-a".into(), url: "http://a".into() });
        m.server_urls.push(McpServerUrl { name: "tool-b".into(), url: "http://b".into() });

        let urls = m.server_urls.clone();
        let filtered: Vec<McpServerUrl> = match None::<Vec<String>> {
            Some(names) => urls.iter().filter(|u| names.iter().any(|n| n == &u.name)).cloned().collect(),
            None => urls,
        };
        assert_eq!(filtered.len(), 3);

        for h in &m.server_handles { h.abort(); }
    }

    #[tokio::test]
    async fn get_mcp_urls_with_filter_returns_matching_only() {
        let mut m = ToolsManager::new(HashMap::new());
        m.start().await.unwrap();
        m.server_urls.push(McpServerUrl { name: "system-info".into(), url: "http://si".into() });
        m.server_urls.push(McpServerUrl { name: "filesystem".into(), url: "http://fs".into() });

        let urls = m.server_urls.clone();
        let names = vec!["system-info".to_string()];
        let filtered: Vec<McpServerUrl> = urls.iter()
            .filter(|u| names.iter().any(|n| n == &u.name))
            .cloned()
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "system-info");

        for h in &m.server_handles { h.abort(); }
    }

    #[tokio::test]
    async fn get_mcp_urls_with_nonexistent_filter_returns_empty() {
        let mut m = ToolsManager::new(HashMap::new());
        m.start().await.unwrap();

        let urls = m.server_urls.clone();
        let names = vec!["nonexistent".to_string()];
        let filtered: Vec<McpServerUrl> = urls.iter()
            .filter(|u| names.iter().any(|n| n == &u.name))
            .cloned()
            .collect();
        assert!(filtered.is_empty());

        for h in &m.server_handles { h.abort(); }
    }

    #[tokio::test]
    async fn disabled_mcp_server_not_spawned() {
        let tool_configs = HashMap::from([("disabled-tool".to_string(), ToolConfig {
            tool_type: "mcp".into(),
            binary: Some("nonexistent-binary-xyz-99999".into()),
            args: vec![],
            enabled: false,
            module: None,
            description: String::new(),
            input_schema: None,
            sandbox: Default::default(),
            options: HashMap::new(),
        })]);
        let mut m = ToolsManager::new(tool_configs);
        m.start().await.unwrap();
        let ext = m.external_servers.as_ref().unwrap();
        assert!(ext.is_empty(), "disabled MCP server should not be spawned");
        for h in &m.server_handles {
            h.abort();
        }
    }
}
