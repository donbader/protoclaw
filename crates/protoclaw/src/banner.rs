use protoclaw_config::ProtoclawConfig;

pub fn format_banner(config: &ProtoclawConfig, config_path: &str) -> String {
    let mut out = format!("protoclaw v{}\n", env!("CARGO_PKG_VERSION"));
    for agent in &config.agents {
        out.push_str(&format!(
            "  Agent:    {} [{}] (args: {})\n",
            agent.name,
            agent.binary,
            if agent.args.is_empty() {
                "(none)".to_string()
            } else {
                agent.args.join(" ")
            }
        ));
    }
    if config.agents.is_empty() {
        out.push_str("  Agent:    (none configured)\n");
    }
    for ch in &config.channels {
        out.push_str(&format!("  Channel:  {} ({})\n", ch.name, ch.binary));
    }
    for mcp in &config.mcp_servers {
        out.push_str(&format!("  MCP:      {} ({})\n", mcp.name, mcp.binary));
    }
    if config.mcp_servers.is_empty() {
        out.push_str("  MCP:      (none configured)\n");
    }
    out.push_str(&format!("  Config:   {}\n", config_path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_config::{
        AgentConfig, ChannelConfig, McpServerConfig, ProtoclawConfig, SupervisorConfig,
    };
    use std::collections::HashMap;

    fn make_config(
        agent_binary: &str,
        channels: Vec<(&str, &str)>,
        mcp_servers: Vec<(&str, &str)>,
    ) -> ProtoclawConfig {
        ProtoclawConfig {
            log_level: "info".into(),
            extensions_dir: "/usr/local/bin".into(),
            agent: None,
            agents: vec![AgentConfig {
                name: "default".to_string(),
                binary: agent_binary.to_string(),
                args: vec![],
                enabled: true,
                env: HashMap::new(),
                working_dir: None,
                tools: vec![],
            }],
            channels: channels
                .into_iter()
                .map(|(name, binary)| ChannelConfig {
                    name: name.to_string(),
                    binary: binary.to_string(),
                    args: vec![],
                    enabled: true,
                    agent: None,
                })
                .collect(),
            mcp_servers: mcp_servers
                .into_iter()
                .map(|(name, binary)| McpServerConfig {
                    name: name.to_string(),
                    binary: binary.to_string(),
                    args: vec![],
                    enabled: true,
                })
                .collect(),
            wasm_tools: vec![],
            supervisor: SupervisorConfig::default(),
        }
    }

    #[test]
    fn banner_contains_agent_channel_mcp_config() {
        let config = make_config(
            "opencode",
            vec![("debug-http", "protoclaw-debug-http")],
            vec![("filesystem", "mcp-filesystem")],
        );
        let output = format_banner(&config, "protoclaw.toml");
        assert!(output.contains("opencode"), "should contain agent binary");
        assert!(output.contains("debug-http"), "should contain channel name");
        assert!(output.contains("filesystem"), "should contain MCP name");
        assert!(
            output.contains("protoclaw.toml"),
            "should contain config path"
        );
    }

    #[test]
    fn banner_with_no_mcp_shows_none_configured() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "protoclaw.toml");
        assert!(
            output.contains("(none configured)"),
            "should say (none configured) when no MCP"
        );
    }

    #[test]
    fn banner_starts_with_protoclaw_v() {
        let config = make_config("opencode", vec![], vec![]);
        let output = format_banner(&config, "protoclaw.toml");
        assert!(
            output.starts_with("protoclaw v"),
            "should start with 'protoclaw v'"
        );
    }
}
