use tokio::sync::oneshot;

/// URL of an MCP server endpoint that agents can connect to for tool access.
#[derive(Clone)]
pub struct McpServerUrl {
    /// Logical tool name (matches the config key).
    pub name: String,
    /// HTTP URL of the aggregated MCP endpoint.
    pub url: String,
}

/// Human-readable description of a tool, returned by the admin API.
#[derive(Clone, Debug)]
pub struct ToolDescription {
    /// Tool name as registered in the MCP host.
    pub name: String,
    /// Brief description of what the tool does.
    pub description: String,
}

/// Commands sent to the tools manager via [`ManagerHandle<ToolsCommand>`](crate::ManagerHandle).
pub enum ToolsCommand {
    /// Retrieve MCP server URLs, optionally filtered by tool name.
    GetMcpUrls {
        /// If `Some`, only return URLs for tools whose names are in this list.
        tool_names: Option<Vec<String>>,
        /// Oneshot channel for the URL list.
        reply: oneshot::Sender<Vec<McpServerUrl>>,
    },
    /// Retrieve tool descriptions, optionally filtered by tool name.
    GetToolDescriptions {
        /// If `Some`, only return descriptions for tools whose names are in this list.
        tool_names: Option<Vec<String>>,
        /// Oneshot channel for the description list.
        reply: oneshot::Sender<Vec<ToolDescription>>,
    },
    /// Request graceful shutdown of all tool subprocesses.
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

    #[test]
    fn when_tool_description_constructed_then_fields_accessible() {
        let desc = ToolDescription {
            name: "system-info".into(),
            description: "Returns system information".into(),
        };
        assert_eq!(desc.name, "system-info");
        assert_eq!(desc.description, "Returns system information");
    }

    #[test]
    fn when_tool_description_cloned_then_equal_to_original() {
        let desc = ToolDescription {
            name: "my-tool".into(),
            description: "Does things".into(),
        };
        let cloned = desc.clone();
        assert_eq!(cloned.name, desc.name);
        assert_eq!(cloned.description, desc.description);
    }

    #[test]
    fn when_get_tool_descriptions_with_no_filter_then_tool_names_is_none() {
        let (tx, _rx) = oneshot::channel();
        let cmd = ToolsCommand::GetToolDescriptions {
            tool_names: None,
            reply: tx,
        };
        match cmd {
            ToolsCommand::GetToolDescriptions { tool_names, .. } => {
                assert!(tool_names.is_none());
            }
            _ => panic!("unexpected variant"),
        }
    }
}
