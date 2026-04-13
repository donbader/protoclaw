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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_mcp_server_url_constructed_then_fields_accessible() {
        let url = McpServerUrl {
            name: "my-tool".into(),
            url: "http://localhost:9090".into(),
        };
        assert_eq!(url.name, "my-tool");
        assert_eq!(url.url, "http://localhost:9090");
    }

    #[test]
    fn when_mcp_server_url_cloned_then_equal_to_original() {
        let url = McpServerUrl {
            name: "tool-a".into(),
            url: "http://127.0.0.1:8080".into(),
        };
        let cloned = url.clone();
        assert_eq!(cloned.name, url.name);
        assert_eq!(cloned.url, url.url);
    }

    #[test]
    fn when_tools_command_shutdown_constructed_then_is_unit_variant() {
        let cmd = ToolsCommand::Shutdown;
        assert!(matches!(cmd, ToolsCommand::Shutdown));
    }

    #[test]
    fn when_tools_command_get_mcp_urls_with_no_filter_constructed_then_tool_names_is_none() {
        let (tx, _rx) = oneshot::channel();
        let cmd = ToolsCommand::GetMcpUrls {
            tool_names: None,
            reply: tx,
        };
        match cmd {
            ToolsCommand::GetMcpUrls { tool_names, .. } => {
                assert!(tool_names.is_none());
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn when_tools_command_get_mcp_urls_with_filter_constructed_then_tool_names_contains_names() {
        let (tx, _rx) = oneshot::channel();
        let cmd = ToolsCommand::GetMcpUrls {
            tool_names: Some(vec!["tool-a".into(), "tool-b".into()]),
            reply: tx,
        };
        match cmd {
            ToolsCommand::GetMcpUrls {
                tool_names: Some(names),
                ..
            } => {
                assert_eq!(names.len(), 2);
                assert!(names.contains(&"tool-a".to_string()));
            }
            _ => panic!("unexpected variant"),
        }
    }
}
