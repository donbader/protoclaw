use std::collections::HashMap;
use std::time::Duration;

use anyclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, debug_http_path, mock_agent_path, with_timeout,
};
use anyclaw_test_helpers::sdk_test_channel_path;
use rstest::rstest;

/// Two channels (debug-http → agent-a, sdk-test-channel → agent-b), two agents with distinct
/// echo prefixes. Sends a message via debug-http and verifies that only agent-a responds on the
/// SSE stream — agent-b's session is isolated and produces no cross-channel bleed.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_two_channels_routing_to_different_agents_when_messages_sent_then_responses_isolated()
{
    let config = build_dual_channel_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let health = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200, "debug-http health check failed");

    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "isolation-check"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message to debug-http failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_agent_a = events
        .iter()
        .any(|e| e.data.contains("agent-a:") || e.data.contains("Echo: isolation-check"));

    let saw_agent_b = events.iter().any(|e| e.data.contains("agent-b:"));

    assert!(
        saw_agent_a,
        "expected a response from agent-a via debug-http SSE; got events: {events:?}"
    );
    assert!(
        !saw_agent_b,
        "agent-b response must NOT bleed into debug-http SSE stream; got events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

fn build_dual_channel_config() -> anyclaw_config::AnyclawConfig {
    let mut agents = HashMap::new();
    agents.insert(
        "agent-a".to_string(),
        anyclaw_config::AgentConfig {
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
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
        anyclaw_config::AgentConfig {
            workspace: anyclaw_config::WorkspaceConfig::Local(
                anyclaw_config::LocalWorkspaceConfig {
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
        anyclaw_config::ChannelConfig {
            binary: debug_http_path().to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            agent: "agent-a".into(),
            init_timeout_secs: None,
            exit_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        },
    );
    channels.insert(
        "sdk-test-channel".to_string(),
        anyclaw_config::ChannelConfig {
            binary: sdk_test_channel_path().to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            agent: "agent-b".into(),
            init_timeout_secs: None,
            exit_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        },
    );

    anyclaw_config::AnyclawConfig {
        agents_manager: anyclaw_config::AgentsManagerConfig {
            agents,
            ..Default::default()
        },
        channels_manager: anyclaw_config::ChannelsManagerConfig {
            channels,
            ..Default::default()
        },
        tools_manager: anyclaw_config::ToolsManagerConfig::default(),
        supervisor: anyclaw_config::SupervisorConfig {
            shutdown_timeout_secs: 5,
            health_check_interval_secs: 1,
            max_restarts: 3,
            ..Default::default()
        },
        log_level: "info".into(),
        log_format: anyclaw_config::LogFormat::Pretty,
        extensions_dir: "/usr/local/bin".into(),
        session_store: Default::default(),
    }
}
