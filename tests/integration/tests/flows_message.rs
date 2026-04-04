use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout, SseCollector,
};

#[test_log::test(tokio::test)]
async fn flow_message_queued() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello world"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    // Give time for the message to flow through the pipeline
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[test_log::test(tokio::test)]
async fn flow_message_echo_via_sse() {
    let mut config = mock_agent_config();
    config.channels_manager.debounce.window_ms = 100;
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "ping"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("ping"));
    let saw_result = events.iter().any(|e| {
        serde_json::from_str::<serde_json::Value>(&e.data)
            .ok()
            .and_then(|v| v.get("type")?.as_str().map(|s| s == "result"))
            .unwrap_or(false)
    });
    assert!(saw_echo, "should have received echo chunk via SSE");
    assert!(saw_result, "should have received result via SSE");

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
