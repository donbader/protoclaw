use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config, with_timeout,
};
use rstest::rstest;

#[test_log::test(tokio::test)]
async fn given_agent_thinking_enabled_when_message_sent_then_thought_events_precede_result() {
    let mut config = mock_agent_config();
    config
        .agents_manager
        .agents
        .get_mut("default")
        .unwrap()
        .options
        .insert("thinking".into(), serde_json::json!(true));
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "think-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let thought_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.event_type.as_deref() == Some("thought"))
        .map(|(i, _)| i)
        .collect();
    assert!(
        !thought_positions.is_empty(),
        "expected thought SSE events, got: {:?}",
        events
            .iter()
            .map(|e| (&e.event_type, &e.data))
            .collect::<Vec<_>>()
    );

    let result_position = events
        .iter()
        .position(|e| e.data == "Echo: think-test")
        .expect("should have received a result event via SSE");

    let last_thought = *thought_positions.last().unwrap();
    assert!(
        last_thought < result_position,
        "thought events (last at {last_thought}) must arrive before result (at {result_position})"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
