use std::collections::HashMap;

use crate::paths::{debug_http_path, mock_agent_path, sdk_test_channel_path, sdk_test_tool_path};

pub fn mock_agent_config() -> protoclaw_config::ProtoclawConfig {
    mock_agent_config_with_options(HashMap::new())
}

pub fn mock_agent_config_with_options(
    options: HashMap<String, serde_json::Value>,
) -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "default".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options,
        },
    );

    let mut channels = HashMap::new();
    channels.insert(
        "debug-http".to_string(),
        protoclaw_config::ChannelConfig {
            binary: debug_http_path().to_string_lossy().to_string(),
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

    protoclaw_config::ProtoclawConfig {
        agents_manager: protoclaw_config::AgentsManagerConfig {
            agents,
            ..Default::default()
        },
        channels_manager: protoclaw_config::ChannelsManagerConfig {
            channels,
            ..Default::default()
        },
        tools_manager: protoclaw_config::ToolsManagerConfig::default(),
        supervisor: protoclaw_config::SupervisorConfig {
            shutdown_timeout_secs: 5,
            health_check_interval_secs: 1,
            max_restarts: 3,
            restart_window_secs: 60,
        },
        log_level: "info".into(),
        log_format: "pretty".into(),
        extensions_dir: "/usr/local/bin".into(),
    }
}

/// Config with a mock-agent, debug-http channel, and sdk-test-channel.
pub fn sdk_channel_config() -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "default".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        },
    );

    let mut channels = HashMap::new();
    channels.insert(
        "debug-http".to_string(),
        protoclaw_config::ChannelConfig {
            binary: debug_http_path().to_string_lossy().to_string(),
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
    channels.insert(
        "sdk-test-channel".to_string(),
        protoclaw_config::ChannelConfig {
            binary: sdk_test_channel_path().to_string_lossy().to_string(),
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

    protoclaw_config::ProtoclawConfig {
        agents_manager: protoclaw_config::AgentsManagerConfig {
            agents,
            ..Default::default()
        },
        channels_manager: protoclaw_config::ChannelsManagerConfig {
            channels,
            ..Default::default()
        },
        tools_manager: protoclaw_config::ToolsManagerConfig::default(),
        supervisor: protoclaw_config::SupervisorConfig {
            shutdown_timeout_secs: 5,
            health_check_interval_secs: 1,
            max_restarts: 3,
            restart_window_secs: 60,
        },
        log_level: "info".into(),
        log_format: "pretty".into(),
        extensions_dir: "/usr/local/bin".into(),
    }
}

/// Config with a mock-agent, debug-http channel, and sdk-test-tool registered as an MCP tool.
pub fn sdk_tool_config() -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "default".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec!["echo".into()],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        },
    );

    let mut channels = HashMap::new();
    channels.insert(
        "debug-http".to_string(),
        protoclaw_config::ChannelConfig {
            binary: debug_http_path().to_string_lossy().to_string(),
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

    let mut tools = HashMap::new();
    tools.insert(
        "echo".to_string(),
        protoclaw_config::ToolConfig {
            tool_type: "mcp".into(),
            binary: Some(sdk_test_tool_path().to_string_lossy().to_string()),
            args: vec![],
            enabled: true,
            module: None,
            description: String::new(),
            input_schema: None,
            sandbox: Default::default(),
            options: HashMap::new(),
        },
    );

    protoclaw_config::ProtoclawConfig {
        agents_manager: protoclaw_config::AgentsManagerConfig {
            agents,
            ..Default::default()
        },
        channels_manager: protoclaw_config::ChannelsManagerConfig {
            channels,
            ..Default::default()
        },
        tools_manager: protoclaw_config::ToolsManagerConfig { tools },
        supervisor: protoclaw_config::SupervisorConfig {
            shutdown_timeout_secs: 5,
            health_check_interval_secs: 1,
            max_restarts: 3,
            restart_window_secs: 60,
        },
        log_level: "info".into(),
        log_format: "pretty".into(),
        extensions_dir: "/usr/local/bin".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_agent_config_has_agent_binary() {
        let cfg = mock_agent_config();
        let agent = cfg
            .agents_manager
            .agents
            .get("default")
            .expect("default agent");
        match &agent.workspace {
            protoclaw_config::WorkspaceConfig::Local(local) => {
                assert!(
                    local.binary.contains("mock-agent"),
                    "binary: {}",
                    local.binary
                );
            }
            _ => panic!("expected Local workspace"),
        }
    }

    #[test]
    fn mock_agent_config_has_debug_http_channel() {
        let cfg = mock_agent_config();
        let ch = cfg
            .channels_manager
            .channels
            .get("debug-http")
            .expect("debug-http channel");
        assert!(ch.binary.contains("debug-http"), "binary: {}", ch.binary);
    }

    #[test]
    fn mock_agent_config_with_options_inserts_values() {
        let mut opts = HashMap::new();
        opts.insert("exit_after".into(), serde_json::json!(1));
        let cfg = mock_agent_config_with_options(opts);
        let agent = cfg
            .agents_manager
            .agents
            .get("default")
            .expect("default agent");
        assert_eq!(agent.options["exit_after"], serde_json::json!(1));
    }

    #[test]
    fn sdk_channel_config_has_sdk_test_channel() {
        let cfg = sdk_channel_config();
        let ch = cfg
            .channels_manager
            .channels
            .get("sdk-test-channel")
            .expect("sdk-test-channel");
        assert!(
            ch.binary.contains("sdk-test-channel"),
            "binary: {}",
            ch.binary
        );
        assert!(ch.enabled);
        assert_eq!(ch.agent, "default");
    }

    #[test]
    fn sdk_channel_config_has_debug_http() {
        let cfg = sdk_channel_config();
        assert!(cfg.channels_manager.channels.contains_key("debug-http"));
    }

    #[test]
    fn sdk_tool_config_has_echo_tool() {
        let cfg = sdk_tool_config();
        let tool = cfg.tools_manager.tools.get("echo").expect("echo tool");
        assert_eq!(tool.tool_type, "mcp");
        let binary = tool.binary.as_deref().expect("binary should be set");
        assert!(binary.contains("sdk-test-tool"), "binary: {binary}");
        assert!(tool.enabled);
    }

    #[test]
    fn sdk_tool_config_agent_has_echo_tool() {
        let cfg = sdk_tool_config();
        let agent = cfg
            .agents_manager
            .agents
            .get("default")
            .expect("default agent");
        assert!(agent.tools.contains(&"echo".to_string()));
    }
}
