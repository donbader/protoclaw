use anyclaw_config::{AnyclawConfig, WorkspaceConfig};

/// Format the ASCII startup banner showing configured agents, channels, tools, and config path.
pub fn format_banner(config: &AnyclawConfig, config_path: &str) -> String {
    let mut out = format!("anyclaw v{}\n", env!("CARGO_PKG_VERSION"));
    for (name, agent) in &config.agents_manager.agents {
        let binary_display = match &agent.workspace {
            WorkspaceConfig::Local(local) => local.binary.to_string(),
            WorkspaceConfig::Docker(docker) => format!("docker:{}", docker.image),
        };
        out.push_str(&format!("  Agent:    {} [{}]\n", name, binary_display));
    }
    if config.agents_manager.agents.is_empty() {
        out.push_str("  Agent:    (none configured)\n");
    }
    for (name, ch) in &config.channels_manager.channels {
        out.push_str(&format!("  Channel:  {} ({})\n", name, ch.binary));
    }
    for (name, tool) in &config.tools_manager.tools {
        if let Some(ref bin) = tool.binary {
            out.push_str(&format!("  Tool:     {} ({})\n", name, bin));
        }
    }
    if config.tools_manager.tools.is_empty() {
        out.push_str("  Tool:     (none configured)\n");
    }
    out.push_str(&format!("  Config:   {}\n", config_path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_config::{
        AgentConfig, AgentsManagerConfig, AnyclawConfig, ChannelConfig, ChannelsManagerConfig,
        LocalWorkspaceConfig, SupervisorConfig, ToolConfig, ToolsManagerConfig, WorkspaceConfig,
    };

    use std::collections::HashMap;

    fn make_config(
        agent_binary: &str,
        channels: Vec<(&str, &str)>,
        tools: Vec<(&str, &str)>,
    ) -> AnyclawConfig {
        let mut agents = HashMap::new();
        agents.insert(
            "default".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                    binary: agent_binary.into(),
                    working_dir: None,
                    env: HashMap::new(),
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );

        let mut channel_map = HashMap::new();
        for (name, binary) in channels {
            channel_map.insert(
                name.to_string(),
                ChannelConfig {
                    binary: binary.to_string(),
                    args: vec![],
                    enabled: true,
                    agent: "default".into(),
                    init_timeout_secs: None,
                    exit_timeout_secs: None,
                    backoff: None,
                    crash_tracker: None,
                    options: HashMap::new(),
                },
            );
        }

        let mut tool_map = HashMap::new();
        for (name, binary) in tools {
            tool_map.insert(
                name.to_string(),
                ToolConfig {
                    tool_type: anyclaw_config::ToolType::Mcp,
                    binary: Some(binary.to_string()),
                    args: vec![],
                    enabled: true,
                    module: None,
                    description: String::new(),
                    input_schema: None,
                    sandbox: Default::default(),
                    options: HashMap::new(),
                },
            );
        }

        AnyclawConfig {
            log_level: "info".into(),
            log_format: anyclaw_config::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                agents,
                ..Default::default()
            },
            channels_manager: ChannelsManagerConfig {
                channels: channel_map,
                ..Default::default()
            },
            tools_manager: ToolsManagerConfig {
                tools: tool_map,
                ..Default::default()
            },
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        }
    }

    #[test]
    fn when_format_banner_called_with_full_config_then_contains_agent_channel_tool_and_config_path()
    {
        let config = make_config(
            "opencode",
            vec![("debug-http", "anyclaw-debug-http")],
            vec![("filesystem", "mcp-filesystem")],
        );
        let output = format_banner(&config, "anyclaw.yaml");
        assert!(output.contains("opencode"), "should contain agent binary");
        assert!(output.contains("debug-http"), "should contain channel name");
        assert!(output.contains("filesystem"), "should contain tool name");
        assert!(
            output.contains("anyclaw.yaml"),
            "should contain config path"
        );
    }

    #[test]
    fn given_no_tools_when_format_banner_called_then_shows_none_configured() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "anyclaw.yaml");
        assert!(
            output.contains("(none configured)"),
            "should say (none configured) when no tools"
        );
    }

    #[test]
    fn when_format_banner_called_then_output_starts_with_anyclaw_version() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "anyclaw.yaml");
        assert!(
            output.starts_with("anyclaw v"),
            "should start with 'anyclaw v'"
        );
    }
}
