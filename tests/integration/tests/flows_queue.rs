use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout, SseCollector,
};

/// Send 3 messages rapidly. With FIFO queue, each gets its own response in order
/// (no merging). Verify we see 3 separate echo responses containing each message.
#[test_log::test(tokio::test)]
async fn flow_queue_processes_messages_in_order() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    for msg in ["first", "second", "third"] {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": msg}))
            .send()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let events = sse.collect_events(Duration::from_secs(15)).await;

    let saw_first = events.iter().any(|e| e.data.contains("first"));
    let saw_second = events.iter().any(|e| e.data.contains("second"));
    let saw_third = events.iter().any(|e| e.data.contains("third"));

    assert!(saw_first, "should echo 'first', events: {events:?}");
    assert!(saw_second, "should echo 'second', events: {events:?}");
    assert!(saw_third, "should echo 'third', events: {events:?}");

    let pos_first = events.iter().position(|e| e.data.contains("first"));
    let pos_second = events.iter().position(|e| e.data.contains("second"));
    let pos_third = events.iter().position(|e| e.data.contains("third"));

    assert!(
        pos_first < pos_second && pos_second < pos_third,
        "messages must be processed in FIFO order: first@{pos_first:?}, second@{pos_second:?}, third@{pos_third:?}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
