use std::collections::HashMap;

use crate::paths::{
    debug_http_path, mock_agent_path, sdk_test_channel_path, sdk_test_tool_path, workspace_root,
};

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

/// Config with a docker-workspace agent ("docker-agent") backed by `protoclaw-mock-agent:test`
/// and a debug-http channel. Uses `PullPolicy::Never` — image must be built first via
/// `build_mock_agent_docker_image()`.
pub fn docker_agent_config() -> protoclaw_config::ProtoclawConfig {
    docker_agent_config_with_options(HashMap::new())
}

/// Same as `docker_agent_config()` but passes `options` to the agent config (e.g. `exit_after`).
pub fn docker_agent_config_with_options(
    options: HashMap<String, serde_json::Value>,
) -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "docker-agent".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Docker(
                protoclaw_config::DockerWorkspaceConfig {
                    image: "protoclaw-mock-agent:test".to_string(),
                    entrypoint: None,
                    volumes: vec![],
                    env: HashMap::new(),
                    memory_limit: None,
                    cpu_limit: None,
                    docker_host: None,
                    network: None,
                    pull_policy: protoclaw_config::PullPolicy::Never,
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
            agent: "docker-agent".into(),
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

/// Build the Docker image `protoclaw-mock-agent:test` from the local workspace.
///
/// Copies `target/debug/mock-agent` into a temporary build context alongside
/// `tests/integration/Dockerfile.mock-agent`, then runs `docker build`.
///
/// Returns `Ok(())` on success or `Err(String)` with the error output on failure.
pub fn build_mock_agent_docker_image() -> Result<(), String> {
    let root = workspace_root();
    let mock_agent_binary = root.join("target/debug/mock-agent");
    let dockerfile = root.join("tests/integration/Dockerfile.mock-agent");

    if !mock_agent_binary.exists() {
        return Err(format!(
            "mock-agent binary not found at {}. Run `cargo build --bin mock-agent` first.",
            mock_agent_binary.display()
        ));
    }
    if !dockerfile.exists() {
        return Err(format!("Dockerfile not found at {}", dockerfile.display()));
    }

    let build_ctx = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let ctx_path = build_ctx.path();

    std::fs::copy(&mock_agent_binary, ctx_path.join("mock-agent"))
        .map_err(|e| format!("failed to copy mock-agent: {e}"))?;

    std::fs::copy(&dockerfile, ctx_path.join("Dockerfile"))
        .map_err(|e| format!("failed to copy Dockerfile: {e}"))?;

    let output = std::process::Command::new("docker")
        .args(["build", "-t", "protoclaw-mock-agent:test", "."])
        .current_dir(ctx_path)
        .output()
        .map_err(|e| format!("docker build failed to run: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "docker build failed (exit {}):\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

/// Stop and remove all containers managed by protoclaw that have the `protoclaw.managed=true` label.
///
/// Best-effort cleanup — ignores individual container removal errors. Intended to run before
/// Docker integration tests to ensure a clean environment.
pub fn cleanup_test_containers() {
    let list_output = std::process::Command::new("docker")
        .args(["ps", "-aq", "--filter", "label=protoclaw.managed=true"])
        .output();

    let ids_output = match list_output {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let ids_text = String::from_utf8_lossy(&ids_output.stdout);
    let ids: Vec<&str> = ids_text
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect();

    if ids.is_empty() {
        return;
    }

    let mut cmd = std::process::Command::new("docker");
    cmd.arg("rm").arg("-f");
    for id in &ids {
        cmd.arg(id);
    }
    let _ = cmd.output();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_mock_agent_config_called_then_default_agent_binary_contains_mock_agent() {
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
    fn when_mock_agent_config_called_then_debug_http_channel_binary_is_set() {
        let cfg = mock_agent_config();
        let ch = cfg
            .channels_manager
            .channels
            .get("debug-http")
            .expect("debug-http channel");
        assert!(ch.binary.contains("debug-http"), "binary: {}", ch.binary);
    }

    #[test]
    fn given_options_map_when_mock_agent_config_with_options_called_then_options_present_on_agent()
    {
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
    fn when_sdk_channel_config_called_then_sdk_test_channel_is_enabled_and_uses_correct_binary() {
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
    fn when_sdk_channel_config_called_then_debug_http_channel_is_present() {
        let cfg = sdk_channel_config();
        assert!(cfg.channels_manager.channels.contains_key("debug-http"));
    }

    #[test]
    fn when_sdk_tool_config_called_then_echo_tool_is_mcp_type_with_correct_binary() {
        let cfg = sdk_tool_config();
        let tool = cfg.tools_manager.tools.get("echo").expect("echo tool");
        assert_eq!(tool.tool_type, "mcp");
        let binary = tool.binary.as_deref().expect("binary should be set");
        assert!(binary.contains("sdk-test-tool"), "binary: {binary}");
        assert!(tool.enabled);
    }

    #[test]
    fn when_sdk_tool_config_called_then_default_agent_has_echo_in_tools_list() {
        let cfg = sdk_tool_config();
        let agent = cfg
            .agents_manager
            .agents
            .get("default")
            .expect("default agent");
        assert!(agent.tools.contains(&"echo".to_string()));
    }
}
