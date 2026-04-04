use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout, SseCollector,
};

/// Send message, cancel after first SSE event arrives, assert full response + clean exit.
#[test_log::test(tokio::test)]
async fn flow_graceful_shutdown_waits_for_inflight() {
    let mut config = mock_agent_config();
    config.channels_manager.debounce.window_ms = 100;

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let mut sse = SseCollector::connect(port).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "slow-response"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for first SSE event to confirm the message is being processed
    let first = sse.next_event(Duration::from_secs(10)).await;
    assert!(first.is_some(), "should receive at least one SSE event");

    // Cancel mid-flight — supervisor should still deliver remaining events
    cancel.cancel();

    let remaining = sse.collect_events(Duration::from_secs(10)).await;
    let mut all_events = vec![first.unwrap()];
    all_events.extend(remaining);

    let saw_echo = all_events
        .iter()
        .any(|e| e.data.contains("Echo:") && e.data.contains("slow-response"));
    assert!(
        saw_echo,
        "response should arrive despite shutdown signal, events: {all_events:?}"
    );

    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(
        result.is_ok(),
        "supervisor should exit cleanly: {result:?}"
    );
}
