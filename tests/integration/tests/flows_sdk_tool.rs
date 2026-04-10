use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, sdk_tool_config, with_timeout, SseCollector,
};
use rstest::rstest;

#[test_log::test(tokio::test)]
async fn when_sdk_tool_configured_and_message_sent_then_agent_echoes_back_with_result_event() {
    let config = sdk_tool_config();
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
        .json(&serde_json::json!({"message": "test-with-tool"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("test-with-tool"));
    let saw_result = events
        .iter()
        .any(|e| e.data == "Echo: test-with-tool");

    assert!(
        saw_echo,
        "should have received echo chunk via SSE; events: {events:?}"
    );
    assert!(
        saw_result,
        "should have received full echo result via SSE; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn given_tools_configured_when_session_created_then_agent_receives_mcp_server_urls() {
    let mut config = sdk_tool_config();
    config
        .agents_manager
        .agents
        .get_mut("default")
        .unwrap()
        .options
        .insert("echo_mcp_count".to_string(), serde_json::json!(true));

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
        .json(&serde_json::json!({"message": "mcp-url-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_mcp_url = events.iter().any(|e| {
        if let Some(pos) = e.data.find("[mcp:") {
            let after = &e.data[pos + 5..];
            after
                .split(']')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| n >= 1)
                .unwrap_or(false)
        } else {
            false
        }
    });

    assert!(
        saw_mcp_url,
        "agent result should contain [mcp:N] with N>=1 confirming mcp_servers were passed in session/new; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}
