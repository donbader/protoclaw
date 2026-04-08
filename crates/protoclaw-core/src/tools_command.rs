use tokio::sync::oneshot;

#[derive(Clone)]
pub struct McpServerUrl {
    pub name: String,
    pub url: String,
}

pub enum ToolsCommand {
    GetMcpUrls {
        tool_names: Option<Vec<String>>,
        reply: oneshot::Sender<Vec<McpServerUrl>>,
    },
    Shutdown,
}
