use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config, with_timeout,
};
use rstest::rstest;

/// Send 3 messages rapidly. With FIFO queue + merging, queued messages may be
/// joined into fewer agent turns. Verify all content arrives and FIFO order holds.
#[test_log::test(tokio::test)]
async fn when_three_messages_sent_rapidly_then_responses_arrive_in_fifo_order() {
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
    let all_data: String = events
        .iter()
        .map(|e| e.data.clone())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        all_data.contains("first"),
        "should contain 'first', all_data: {all_data:?}"
    );
    assert!(
        all_data.contains("third"),
        "should contain 'third', all_data: {all_data:?}"
    );

    let pos_first = all_data.find("first").expect("first must exist");
    let pos_third = all_data.find("third").expect("third must exist");
    assert!(
        pos_first < pos_third,
        "FIFO: first at byte {pos_first} must precede third at byte {pos_third}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
