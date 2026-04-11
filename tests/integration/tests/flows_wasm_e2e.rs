use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, wasm_tool_config, with_timeout,
};
use rstest::rstest;

#[rstest]
#[test_log::test(tokio::test)]
async fn given_wasm_tool_configured_when_message_sent_then_agent_echoes_back_with_result_event() {
    let config = wasm_tool_config();
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
        .json(&serde_json::json!({"message": "wasm-tool-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("wasm-tool-test"));
    let saw_result = events.iter().any(|e| e.data == "Echo: wasm-tool-test");

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
async fn given_wasm_tool_configured_when_session_created_then_wasm_tool_does_not_add_mcp_server_urls()
 {
    let mut config = wasm_tool_config();
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
        .json(&serde_json::json!({"message": "mcp-count-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_zero_mcp = events.iter().any(|e| e.data.contains("[mcp:0]"));

    assert!(
        saw_zero_mcp,
        "WASM tools are loaded into the native host, not registered as external MCP servers — \
         agent should receive [mcp:0] confirming no external server URLs were sent in session/new; \
         events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}
