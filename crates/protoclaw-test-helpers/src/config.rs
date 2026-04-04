use std::collections::HashMap;

use crate::paths::{debug_http_path, mock_agent_path};

pub fn mock_agent_config() -> protoclaw_config::ProtoclawConfig {
    mock_agent_config_with_env(HashMap::new())
}

pub fn mock_agent_config_with_env(
    env: HashMap<String, String>,
) -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "default".to_string(),
        protoclaw_config::AgentConfig {
            binary: mock_agent_path().to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            env,
            working_dir: None,
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
        assert!(
            agent.binary.contains("mock-agent"),
            "binary: {}",
            agent.binary
        );
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
    fn mock_agent_config_with_env_inserts_vars() {
        let mut env = HashMap::new();
        env.insert("FOO".into(), "bar".into());
        let cfg = mock_agent_config_with_env(env);
        let agent = cfg
            .agents_manager
            .agents
            .get("default")
            .expect("default agent");
        assert_eq!(agent.env.get("FOO").unwrap(), "bar");
    }
}
