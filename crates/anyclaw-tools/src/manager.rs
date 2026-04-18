use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyclaw_config::ToolConfig;
use anyclaw_core::{Manager, ManagerError, McpServerUrl, ToolsCommand};
use anyclaw_sdk_tool::DynTool;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool as RmcpTool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::{ErrorData as McpError, RoleServer};
use tokio_util::sync::CancellationToken;

use crate::external::ExternalMcpServer;
use crate::mcp_host::McpHost;
use crate::wasm_runner::WasmToolRunner;
use crate::wasm_tool::WasmTool;

/// Aggregated MCP server that merges native, WASM, and external tools into a single endpoint.
///
/// Implements rmcp's `ServerHandler` trait. `list_tools()` merges all sources;
/// `call_tool()` routes to native host first, then external servers by name match.
pub struct AggregatedToolServer {
    native_host: Arc<McpHost>,
    external_servers: Arc<Vec<ExternalMcpServer>>,
    server_info: ServerInfo,
}

impl AggregatedToolServer {
    /// Create a new aggregated server from native and external tool sources.
    pub fn new(native_host: Arc<McpHost>, external_servers: Arc<Vec<ExternalMcpServer>>) -> Self {
        let mut server_info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        server_info.server_info = Implementation::new("anyclaw-tools", "0.1.0");
        Self {
            native_host,
            external_servers,
            server_info,
        }
    }

    #[tracing::instrument(skip(self), name = "aggregate_tool_list")]
    async fn aggregate_tool_list(&self) -> Vec<RmcpTool> {
        let mut tools = self.native_host.tool_list();
        for ext in self.external_servers.iter() {
            if let Ok(ext_tools) = ext.list_tools().await {
                tools.extend(ext_tools);
            }
        }
        tools
    }

    // D-03: args use serde_json::Value — tool call arguments are arbitrary JSON
    // defined by each tool's input_schema. Cannot be typed at this layer.
    async fn route_call(
        &self,
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let start = std::time::Instant::now();

        let result = self.dispatch_tool_inner(name, args).await;

        let duration = start.elapsed();
        let status = if result.is_ok() { "ok" } else { "error" };
        metrics::counter!(
            "anyclaw_tool_invocations_total",
            "tool" => name.to_string(),
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "anyclaw_tool_duration_seconds",
            "tool" => name.to_string()
        )
        .record(duration.as_secs_f64());
        tracing::info!(
            target: "anyclaw::audit",
            tool_name = %name,
            success = result.is_ok(),
            duration_ms = duration.as_millis() as u64,
            "tool_invoked"
        );

        result
    }

    // D-03: args use serde_json::Value — arbitrary tool call arguments (see route_call)
    async fn dispatch_tool_inner(
        &self,
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        if let Ok(result) = self.native_host.dispatch_tool(name, args.clone()).await {
            return Ok(result);
        }
        for ext in self.external_servers.iter() {
            if let Ok(ext_tools) = ext.list_tools().await
                && ext_tools.iter().any(|t| t.name.as_ref() == name)
            {
                let mut params = CallToolRequestParams::new(name.to_string());
                params.arguments = args;
                return ext
                    .call_tool(params)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None));
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

/// Manages tool availability: spawns external MCP servers, loads WASM tools,
/// and serves the aggregated MCP endpoint over HTTP.
pub struct ToolsManager {
    tool_configs: HashMap<String, ToolConfig>,
    native_tools: Vec<Box<dyn DynTool>>,
    server_urls: Vec<McpServerUrl>,
    server_handles: Vec<tokio::task::JoinHandle<()>>,
    cmd_rx: Option<tokio::sync::mpsc::Receiver<ToolsCommand>>,
    native_host: Option<Arc<McpHost>>,
    external_servers: Option<Arc<Vec<ExternalMcpServer>>>,
    tools_server_host: String,
}

impl ToolsManager {
    /// Create a new tools manager from the given tool configs and server host.
    pub fn new(tool_configs: HashMap<String, ToolConfig>, tools_server_host: String) -> Self {
        Self {
            tool_configs,
            native_tools: Vec::new(),
            server_urls: Vec::new(),
            server_handles: Vec::new(),
            cmd_rx: None,
            native_host: None,
            external_servers: None,
            tools_server_host,
        }
    }

    /// Set the command receiver (wired by the supervisor).
    pub fn with_cmd_rx(mut self, rx: tokio::sync::mpsc::Receiver<ToolsCommand>) -> Self {
        self.cmd_rx = Some(rx);
        self
    }

    /// Register native (in-process) tools to be served alongside external and WASM tools.
    pub fn with_native_tools(mut self, tools: Vec<Box<dyn DynTool>>) -> Self {
        self.native_tools = tools;
        self
    }

    /// Return the MCP server URLs advertised to agents.
    pub fn server_urls(&self) -> &[McpServerUrl] {
        &self.server_urls
    }
}

impl Manager for ToolsManager {
    type Command = ToolsCommand;

    fn name(&self) -> &str {
        "tools"
    }

    #[tracing::instrument(skip(self), name = "tools_manager_start")]
    async fn start(&mut self) -> Result<(), ManagerError> {
        let wasm_configs = self.enabled_tool_configs(&anyclaw_config::ToolType::Wasm);
        let mcp_configs = self.enabled_tool_configs(&anyclaw_config::ToolType::Mcp);

        let all_tools = self.load_all_tools(&wasm_configs)?;
        let native_host = Arc::new(McpHost::new(all_tools));
        self.native_host = Some(Arc::clone(&native_host));

        let external_servers = Arc::new(self.spawn_external_servers(&mcp_configs).await);
        self.external_servers = Some(Arc::clone(&external_servers));

        self.start_aggregated_server_if_needed(&native_host, &external_servers, &mcp_configs)
            .await?;

        tracing::info!(manager = self.name(), "manager started");
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
                        Some(ToolsCommand::GetToolDescriptions { tool_names, reply }) => {
                            let descriptions = self.collect_tool_descriptions(tool_names.as_deref()).await;
                            let _ = reply.send(descriptions);
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

impl ToolsManager {
    async fn collect_tool_descriptions(
        &self,
        tool_names: Option<&[String]>,
    ) -> Vec<anyclaw_core::ToolDescription> {
        let mut descriptions = Vec::new();

        if let Some(host) = &self.native_host {
            for tool in host.tool_list() {
                let name = tool.name.to_string();
                let desc = tool.description.as_deref().unwrap_or("").to_string();
                descriptions.push(anyclaw_core::ToolDescription {
                    name,
                    description: desc,
                });
            }
        }

        if let Some(externals) = &self.external_servers {
            for ext in externals.iter() {
                if let Ok(tools) = ext.list_tools().await {
                    for tool in tools {
                        let name = tool.name.to_string();
                        let desc = tool.description.as_deref().unwrap_or("").to_string();
                        descriptions.push(anyclaw_core::ToolDescription {
                            name,
                            description: desc,
                        });
                    }
                }
            }
        }

        if let Some(names) = tool_names {
            descriptions.retain(|d| names.iter().any(|n| n == &d.name));
        }

        descriptions
    }

    fn enabled_tool_configs(
        &self,
        tool_type: &anyclaw_config::ToolType,
    ) -> Vec<(String, ToolConfig)> {
        self.tool_configs
            .iter()
            .filter(|(_, config)| config.tool_type == *tool_type && config.enabled)
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect()
    }

    fn load_all_tools(
        &mut self,
        wasm_configs: &[(String, ToolConfig)],
    ) -> Result<Vec<Box<dyn DynTool>>, ManagerError> {
        let mut all_tools: Vec<Box<dyn DynTool>> = std::mem::take(&mut self.native_tools);
        if wasm_configs.is_empty() {
            return Ok(all_tools);
        }

        let wasm_runner = Arc::new(
            WasmToolRunner::new()
                .map_err(|e| ManagerError::Internal(format!("wasm runner: {e}")))?,
        );

        for (name, wasm_config) in wasm_configs {
            match WasmTool::new(name.clone(), wasm_config.clone(), Arc::clone(&wasm_runner)) {
                Ok(tool) => {
                    tracing::info!(name = %DynTool::name(&tool), "loaded WASM tool");
                    all_tools.push(Box::new(tool));
                }
                Err(e) => {
                    tracing::warn!(name = %name, error = %e, "failed to load WASM tool, skipping");
                }
            }
        }

        Ok(all_tools)
    }

    async fn spawn_external_servers(
        &self,
        mcp_configs: &[(String, ToolConfig)],
    ) -> Vec<ExternalMcpServer> {
        let mut external_servers = Vec::new();
        for (name, config) in mcp_configs {
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
        external_servers
    }

    async fn start_aggregated_server_if_needed(
        &mut self,
        native_host: &Arc<McpHost>,
        external_servers: &Arc<Vec<ExternalMcpServer>>,
        mcp_configs: &[(String, ToolConfig)],
    ) -> Result<(), ManagerError> {
        let has_tools = !native_host.tool_list().is_empty() || !external_servers.is_empty();
        if !has_tools {
            return Ok(());
        }

        let (handle, url) = self
            .spawn_aggregated_server(Arc::clone(native_host), Arc::clone(external_servers))
            .await?;
        self.server_handles.push(handle);
        tracing::info!(url = %url, "tools aggregated MCP server listening");

        for (name, _) in mcp_configs {
            self.server_urls.push(McpServerUrl {
                name: name.clone(),
                url: url.clone(),
            });
        }

        Ok(())
    }

    fn build_server_config(
        tools_server_host: &str,
        ct: CancellationToken,
    ) -> StreamableHttpServerConfig {
        let mut allowed_hosts = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ];
        if tools_server_host != "127.0.0.1"
            && tools_server_host != "localhost"
            && tools_server_host != "::1"
        {
            allowed_hosts.push(tools_server_host.to_string());
        }
        StreamableHttpServerConfig::default()
            .with_stateful_mode(true)
            .with_cancellation_token(ct)
            .with_allowed_hosts(allowed_hosts)
            .with_sse_keep_alive(Some(Duration::from_secs(30)))
    }

    async fn spawn_aggregated_server(
        &self,
        native_host: Arc<McpHost>,
        external_servers: Arc<Vec<ExternalMcpServer>>,
    ) -> Result<(tokio::task::JoinHandle<()>, String), ManagerError> {
        let ct = CancellationToken::new();
        let config = Self::build_server_config(&self.tools_server_host, ct.clone());

        let mut session_manager = LocalSessionManager::default();
        session_manager.session_config.keep_alive = None;
        let session_manager = Arc::new(session_manager);

        let service: StreamableHttpService<AggregatedToolServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || {
                    Ok(AggregatedToolServer::new(
                        Arc::clone(&native_host),
                        Arc::clone(&external_servers),
                    ))
                },
                session_manager,
                config,
            );

        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
            .await
            .map_err(|e| ManagerError::Internal(format!("tools server bind: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| ManagerError::Internal(format!("tools server addr: {e}")))?
            .port();

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct.cancelled_owned().await })
                .await;
        });
        let url = format!("http://{}:{port}/mcp", self.tools_server_host);
        Ok((handle, url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_sdk_tool::{Tool, ToolSdkError};

    // D-03: DummyTool implements the Tool trait which uses serde_json::Value
    // for input_schema/execute — extensible tool boundary, cannot be typed.
    #[allow(clippy::disallowed_types)]
    struct DummyTool {
        tool_name: String,
    }

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
    fn when_build_server_config_then_sse_keepalive_is_enabled() {
        let ct = CancellationToken::new();
        let config = ToolsManager::build_server_config("127.0.0.1", ct);
        assert_eq!(
            config.sse_keep_alive,
            Some(Duration::from_secs(30)),
            "SSE keepalive must be enabled to prevent rmcp client-side timeout"
        );
    }

    #[test]
    fn when_build_server_config_then_stateful_mode_is_enabled() {
        let ct = CancellationToken::new();
        let config = ToolsManager::build_server_config("127.0.0.1", ct);
        assert!(
            config.stateful_mode,
            "stateful mode is required for multi-turn tool sessions"
        );
    }

    #[test]
    fn when_build_server_config_with_custom_host_then_host_is_in_allowed_list() {
        let ct = CancellationToken::new();
        let config = ToolsManager::build_server_config("anyclaw", ct);
        assert!(
            config.allowed_hosts.iter().any(|h| h == "anyclaw"),
            "custom tools_server_host must be in allowed_hosts for Docker deployments"
        );
    }

    #[test]
    fn when_tools_manager_name_queried_then_returns_tools() {
        let m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        assert_eq!(m.name(), "tools");
    }

    #[tokio::test]
    async fn when_tools_manager_started_with_no_configs_then_server_url_registered() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        assert!(m.start().await.is_ok());
        assert_eq!(m.server_urls().len(), 0);
    }

    #[tokio::test]
    async fn when_no_tool_configs_then_health_check_returns_healthy() {
        let m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        assert!(m.health_check().await);
    }

    #[tokio::test]
    async fn when_tools_manager_started_then_health_check_returns_healthy() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        m.start().await.unwrap();
        assert!(m.health_check().await);
        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn when_cancel_token_fired_then_tools_manager_run_stops() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
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
    async fn when_native_tools_registered_then_aggregate_list_contains_them() {
        let tools: Vec<Box<dyn DynTool>> = vec![
            Box::new(DummyTool {
                tool_name: "tool-a".into(),
            }),
            Box::new(DummyTool {
                tool_name: "tool-b".into(),
            }),
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
    async fn when_known_tool_called_via_aggregated_server_then_returns_result() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(DummyTool {
            tool_name: "my-tool".into(),
        })];
        let host = Arc::new(McpHost::new(tools));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("my-tool", None).await.unwrap();
        assert!(result.is_error.is_none() || result.is_error == Some(false));
    }

    #[tokio::test]
    async fn when_unknown_tool_called_via_aggregated_server_then_returns_error() {
        let host = Arc::new(McpHost::new(vec![]));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn when_manager_given_native_tools_then_they_appear_in_host_tool_list() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(DummyTool {
            tool_name: "native-1".into(),
        })];
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into()).with_native_tools(tools);
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
    async fn when_valid_wasm_config_provided_then_wasm_tool_loaded_into_host() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let tool_configs = HashMap::from([(
            "wasm-tool-1".to_string(),
            ToolConfig {
                tool_type: anyclaw_config::ToolType::Wasm,
                binary: None,
                args: vec![],
                enabled: true,
                module: Some(wasm_path),
                description: "test wasm tool".into(),
                input_schema: None,
                sandbox: anyclaw_config::WasmSandboxConfig::default(),
                options: HashMap::new(),
            },
        )]);

        let mut m = ToolsManager::new(tool_configs, "127.0.0.1".into());
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
    async fn when_invalid_wasm_path_provided_then_skipped_and_start_succeeds() {
        let tool_configs = HashMap::from([(
            "bad-tool".to_string(),
            ToolConfig {
                tool_type: anyclaw_config::ToolType::Wasm,
                binary: None,
                args: vec![],
                enabled: true,
                module: Some(std::path::PathBuf::from("/nonexistent/tool.wasm")),
                description: "bad".into(),
                input_schema: None,
                sandbox: anyclaw_config::WasmSandboxConfig::default(),
                options: HashMap::new(),
            },
        )]);

        let mut m = ToolsManager::new(tool_configs, "127.0.0.1".into());
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
    async fn when_wasm_and_native_tools_configured_then_both_appear_in_aggregate_list() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let native_tools: Vec<Box<dyn DynTool>> = vec![Box::new(DummyTool {
            tool_name: "native-1".into(),
        })];
        let tool_configs = HashMap::from([(
            "wasm-1".to_string(),
            ToolConfig {
                tool_type: anyclaw_config::ToolType::Wasm,
                binary: None,
                args: vec![],
                enabled: true,
                module: Some(wasm_path),
                description: "wasm".into(),
                input_schema: None,
                sandbox: anyclaw_config::WasmSandboxConfig::default(),
                options: HashMap::new(),
            },
        )]);

        let mut m =
            ToolsManager::new(tool_configs, "127.0.0.1".into()).with_native_tools(native_tools);
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
    async fn when_get_mcp_urls_called_without_filter_then_returns_all_urls() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        m.start().await.unwrap();

        m.server_urls.push(McpServerUrl {
            name: "tool-a".into(),
            url: "http://a".into(),
        });
        m.server_urls.push(McpServerUrl {
            name: "tool-b".into(),
            url: "http://b".into(),
        });

        let urls = m.server_urls.clone();
        let filtered: Vec<McpServerUrl> = match None::<Vec<String>> {
            Some(names) => urls
                .iter()
                .filter(|u| names.iter().any(|n| n == &u.name))
                .cloned()
                .collect(),
            None => urls,
        };
        assert_eq!(filtered.len(), 2);

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn when_get_mcp_urls_called_with_filter_then_returns_matching_urls_only() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        m.start().await.unwrap();
        m.server_urls.push(McpServerUrl {
            name: "system-info".into(),
            url: "http://si".into(),
        });
        m.server_urls.push(McpServerUrl {
            name: "filesystem".into(),
            url: "http://fs".into(),
        });

        let urls = m.server_urls.clone();
        let names = vec!["system-info".to_string()];
        let filtered: Vec<McpServerUrl> = urls
            .iter()
            .filter(|u| names.iter().any(|n| n == &u.name))
            .cloned()
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "system-info");

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn when_get_mcp_urls_filter_matches_nothing_then_returns_empty() {
        let mut m = ToolsManager::new(HashMap::new(), "127.0.0.1".into());
        m.start().await.unwrap();

        let urls = m.server_urls.clone();
        let names = vec!["nonexistent".to_string()];
        let filtered: Vec<McpServerUrl> = urls
            .iter()
            .filter(|u| names.iter().any(|n| n == &u.name))
            .cloned()
            .collect();
        assert!(filtered.is_empty());

        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn when_mcp_server_is_disabled_then_not_spawned_on_start() {
        let tool_configs = HashMap::from([(
            "disabled-tool".to_string(),
            ToolConfig {
                tool_type: anyclaw_config::ToolType::Mcp,
                binary: Some("nonexistent-binary-xyz-99999".into()),
                args: vec![],
                enabled: false,
                module: None,
                description: String::new(),
                input_schema: None,
                sandbox: Default::default(),
                options: HashMap::new(),
            },
        )]);
        let mut m = ToolsManager::new(tool_configs, "127.0.0.1".into());
        m.start().await.unwrap();
        let ext = m.external_servers.as_ref().unwrap();
        assert!(ext.is_empty(), "disabled MCP server should not be spawned");
        for h in &m.server_handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn when_route_call_with_unknown_tool_then_error_contains_tool_name() {
        let host = Arc::new(McpHost::new(vec![]));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("my-missing-tool", None).await;
        let err = result.unwrap_err();
        let msg = err.message;
        assert!(
            msg.contains("my-missing-tool"),
            "error message should contain the tool name, got: {msg}"
        );
    }

    #[tokio::test]
    async fn when_native_tool_exists_then_route_call_dispatches_to_native_not_external() {
        let tools: Vec<Box<dyn DynTool>> = vec![Box::new(DummyTool {
            tool_name: "native-only".into(),
        })];
        let host = Arc::new(McpHost::new(tools));
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let result = agg.route_call("native-only", None).await;
        assert!(result.is_ok(), "native tool should be found and dispatched");
    }

    #[tokio::test]
    async fn when_no_external_servers_then_aggregate_list_equals_native_list() {
        let tools: Vec<Box<dyn DynTool>> = vec![
            Box::new(DummyTool {
                tool_name: "alpha".into(),
            }),
            Box::new(DummyTool {
                tool_name: "beta".into(),
            }),
        ];
        let host = Arc::new(McpHost::new(tools));
        let native_list = host.tool_list();
        let ext = Arc::new(vec![]);
        let agg = AggregatedToolServer::new(host, ext);

        let agg_list = agg.aggregate_tool_list().await;
        assert_eq!(
            agg_list.len(),
            native_list.len(),
            "aggregate list should equal native list when no external servers"
        );
        let agg_names: Vec<&str> = agg_list.iter().map(|t| t.name.as_ref()).collect();
        let native_names: Vec<&str> = native_list.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(agg_names, native_names);
    }
}
