use protoclaw_config::{ProtoclawConfig, WorkspaceConfig};

pub fn format_banner(config: &ProtoclawConfig, config_path: &str) -> String {
    let mut out = format!("protoclaw v{}\n", env!("CARGO_PKG_VERSION"));
    for (name, agent) in &config.agents_manager.agents {
        let binary_display = match &agent.workspace {
            WorkspaceConfig::Local(local) => local.binary.clone(),
            WorkspaceConfig::Docker(docker) => format!("docker:{}", docker.image),
        };
        out.push_str(&format!(
            "  Agent:    {} [{}] (args: {})\n",
            name,
            binary_display,
            if agent.args.is_empty() {
                "(none)".to_string()
            } else {
                agent.args.join(" ")
            }
        ));
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
    use protoclaw_config::{
        AgentConfig, AgentsManagerConfig, ChannelConfig, ChannelsManagerConfig,
        LocalWorkspaceConfig, ProtoclawConfig, SupervisorConfig, ToolConfig, ToolsManagerConfig,
        WorkspaceConfig,
    };
    use std::collections::HashMap;

    fn make_config(
        agent_binary: &str,
        channels: Vec<(&str, &str)>,
        tools: Vec<(&str, &str)>,
    ) -> ProtoclawConfig {
        let mut agents = HashMap::new();
        agents.insert(
            "default".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                    binary: agent_binary.to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                }),
                args: vec![],
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
                    ack: Default::default(),
                    init_timeout_secs: None,
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
                    tool_type: "mcp".into(),
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

        ProtoclawConfig {
            log_level: "info".into(),
            log_format: "pretty".into(),
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                agents,
                ..Default::default()
            },
            channels_manager: ChannelsManagerConfig {
                channels: channel_map,
                ..Default::default()
            },
            tools_manager: ToolsManagerConfig { tools: tool_map },
            supervisor: SupervisorConfig::default(),
        }
    }

    #[test]
    fn banner_contains_agent_channel_tool_config() {
        let config = make_config(
            "opencode",
            vec![("debug-http", "protoclaw-debug-http")],
            vec![("filesystem", "mcp-filesystem")],
        );
        let output = format_banner(&config, "protoclaw.yaml");
        assert!(output.contains("opencode"), "should contain agent binary");
        assert!(output.contains("debug-http"), "should contain channel name");
        assert!(output.contains("filesystem"), "should contain tool name");
        assert!(
            output.contains("protoclaw.yaml"),
            "should contain config path"
        );
    }

    #[test]
    fn banner_with_no_tools_shows_none_configured() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "protoclaw.yaml");
        assert!(
            output.contains("(none configured)"),
            "should say (none configured) when no tools"
        );
    }

    #[test]
    fn banner_starts_with_protoclaw_v() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "protoclaw.yaml");
        assert!(
            output.starts_with("protoclaw v"),
            "should start with 'protoclaw v'"
        );
    }
}
