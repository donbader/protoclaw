use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config, with_timeout,
};

#[test_log::test(tokio::test)]
async fn when_supervisor_boots_then_acp_session_established_and_health_responds() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[test_log::test(tokio::test)]
async fn when_message_sent_then_acp_prompt_array_format_produces_echo_response() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "wire-format-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("wire-format-test"));
    assert!(
        saw_echo,
        "mock-agent echoed back, proving session/prompt used correct prompt[] array format"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

#[test_log::test(tokio::test)]
async fn given_agent_emits_non_json_startup_noise_when_message_sent_then_agent_still_responds() {
    let mut config = mock_agent_config();
    let agent = config.agents_manager.agents.get_mut("default").unwrap();
    if let protoclaw_config::WorkspaceConfig::Local(ref mut local) = agent.workspace {
        local.binary.0.push("--noisy-startup".into());
    }

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "noise-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("noise-test"));
    assert!(
        saw_echo,
        "agent must respond despite emitting non-JSON startup output to stdout"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
