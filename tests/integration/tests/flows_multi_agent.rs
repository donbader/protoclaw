use std::collections::HashMap;
use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, debug_http_path, mock_agent_path, with_timeout,
};

/// Two agents (agent-a, agent-b) with different echo_prefix. Channel routes to agent-a.
/// Verify response contains "agent-a:" and NOT "agent-b:".
#[test_log::test(tokio::test)]
async fn given_two_agents_when_channel_routes_to_agent_a_then_only_agent_a_responds() {
    let config = build_two_agent_config("agent-a");

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let mut sse = SseCollector::connect(port).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_agent_a = events.iter().any(|e| e.data.contains("agent-a:"));
    let saw_agent_b = events.iter().any(|e| e.data.contains("agent-b:"));

    assert!(
        saw_agent_a,
        "expected echo from agent-a, events: {events:?}"
    );
    assert!(
        !saw_agent_b,
        "should NOT see agent-b in response, events: {events:?}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Channel routes to nonexistent agent. Verify the system handles it
/// without hanging — either boot fails or an error response arrives.
#[test_log::test(tokio::test)]
async fn given_two_agents_when_channel_routes_to_nonexistent_agent_then_no_echo_received() {
    let config = build_two_agent_config("nonexistent-agent");

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let mut sse = SseCollector::connect(port).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Collect with short timeout — we expect either an error event or no response
    let events = sse.collect_events(Duration::from_secs(5)).await;

    // The message should NOT produce a successful echo from any agent.
    // Current behavior: channels_manager logs a warning and the message is dropped.
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("agent-a:") || e.data.contains("agent-b:"));
    assert!(
        !saw_echo,
        "should NOT see echo from any agent when routing to nonexistent agent, events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly");
}

fn build_two_agent_config(channel_routes_to: &str) -> protoclaw_config::ProtoclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "agent-a".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string().into(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::from([("echo_prefix".into(), serde_json::json!("agent-a"))]),
        },
    );
    agents.insert(
        "agent-b".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string().into(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::from([("echo_prefix".into(), serde_json::json!("agent-b"))]),
        },
    );

    let mut channels = HashMap::new();
    channels.insert(
        "debug-http".to_string(),
        protoclaw_config::ChannelConfig {
            binary: debug_http_path().to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            agent: channel_routes_to.into(),
            ack: Default::default(),
            init_timeout_secs: None,
            exit_timeout_secs: None,
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
            admin_port: 3000,
            permission_timeout_secs: None,
        },
        log_level: "info".into(),
        log_format: protoclaw_config::LogFormat::Pretty,
        extensions_dir: "/usr/local/bin".into(),
    }
}
