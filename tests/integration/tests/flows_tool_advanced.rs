use std::time::Duration;

use anyclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, invalid_tool_config, multi_tool_config,
    sdk_tool_config, with_timeout,
};
use rstest::rstest;

#[rstest]
#[test_log::test(tokio::test)]
async fn when_two_tools_configured_and_message_sent_then_agent_echoes_back() {
    let config = multi_tool_config();
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
        .json(&serde_json::json!({"message": "multi-tool-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("multi-tool-test"));
    let saw_result = events.iter().any(|e| e.data == "Echo: multi-tool-test");

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
async fn when_tool_binary_missing_then_supervisor_boots_and_agent_still_echoes() {
    let config = invalid_tool_config();
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
        .json(&serde_json::json!({"message": "error-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("error-test"));
    assert!(
        saw_echo,
        "agent should still echo despite bad tool binary; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_disabled_tool_configured_then_supervisor_boots_normally() {
    let mut config = sdk_tool_config();
    config.tools_manager.tools.insert(
        "disabled-tool".to_string(),
        anyclaw_config::ToolConfig {
            tool_type: anyclaw_config::ToolType::Mcp,
            binary: Some("/nonexistent/disabled-tool-xyz".into()),
            args: vec![],
            enabled: false,
            module: None,
            description: String::new(),
            input_schema: None,
            sandbox: Default::default(),
            options: std::collections::HashMap::new(),
        },
    );

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
        .json(&serde_json::json!({"message": "disabled-tool-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("disabled-tool-test"));
    assert!(
        saw_echo,
        "agent should echo normally with disabled tool configured; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_tool_configured_and_large_payload_sent_then_full_content_echoed() {
    let config = sdk_tool_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let large_payload = "A".repeat(2000);

    let health = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200, "debug-http health check failed");

    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": large_payload}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(15)).await;

    let received_full = events.iter().any(|e| e.data.contains(&"A".repeat(2000)));
    assert!(
        received_full,
        "should have received full 2000-char payload via SSE without truncation; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_tool_configured_and_json_like_payload_sent_then_content_preserved() {
    let config = sdk_tool_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let json_payload = r#"{"key":"value","nested":{"a":1}}"#;

    let health = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200, "debug-http health check failed");

    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": json_payload}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let content_preserved = events.iter().any(|e| e.data.contains(json_payload));
    assert!(
        content_preserved,
        "should have received JSON-like payload exactly preserved via SSE; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_tool_configured_then_sequential_messages_both_echo() {
    let config = sdk_tool_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let health = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200, "debug-http health check failed");

    let mut sse1 = SseCollector::connect(port).await;

    let resp1 = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "first-tool-msg"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200, "first POST /message failed");

    let events1 = sse1.collect_events(Duration::from_secs(10)).await;
    let saw_first = events1
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("first-tool-msg"));
    assert!(
        saw_first,
        "should have received first echo via SSE; events: {events1:?}"
    );

    let mut sse2 = SseCollector::connect(port).await;

    let resp2 = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "second-tool-msg"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200, "second POST /message failed");

    let events2 = sse2.collect_events(Duration::from_secs(10)).await;
    let saw_second = events2
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("second-tool-msg"));
    assert!(
        saw_second,
        "should have received second echo via SSE; events: {events2:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}
