use std::time::Duration;

use anyclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config, with_timeout,
};
use rstest::rstest;

/// Streaming event ordering: thought chunks arrive before message chunks, message chunks before result.
/// Proves the SSE event ordering contract of the full ACP streaming pipeline.
#[rstest]
#[test_log::test(tokio::test)]
async fn when_message_sent_then_streaming_chunks_arrive_before_result() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "stream-order-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    // Partition by event type
    let thought_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.event_type.as_deref() == Some("thought"))
        .map(|(i, _)| i)
        .collect();

    let result_position = events
        .iter()
        .position(|e| e.data == "Echo: stream-order-test")
        .expect("should have received result event via SSE");

    // Chunk events are unnamed (no event_type) events that arrive before the result
    let chunk_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(i, e)| *i < result_position && e.event_type.is_none())
        .map(|(i, _)| i)
        .collect();

    assert!(
        !thought_positions.is_empty(),
        "expected thought events, got: {:?}",
        events
            .iter()
            .map(|e| (&e.event_type, &e.data))
            .collect::<Vec<_>>()
    );
    assert!(
        !chunk_positions.is_empty(),
        "expected message chunk events before result, got: {:?}",
        events
            .iter()
            .map(|e| (&e.event_type, &e.data))
            .collect::<Vec<_>>()
    );

    let last_thought = *thought_positions.last().unwrap();
    let first_chunk = *chunk_positions.first().unwrap();
    let last_chunk = *chunk_positions.last().unwrap();

    assert!(
        last_thought < first_chunk,
        "all thought events (last at {last_thought}) must arrive before message chunks (first at {first_chunk})"
    );
    assert!(
        last_chunk < result_position,
        "all message chunks (last at {last_chunk}) must arrive before result (at {result_position})"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Multi-turn conversation: send first message, collect response, send follow-up, collect second response.
/// Proves session persistence and correct routing across two separate prompt turns.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_first_response_complete_when_followup_sent_then_second_response_arrives() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    // First turn
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "turn-1"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let first_events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_first = first_events.iter().any(|e| e.data == "Echo: turn-1");
    assert!(
        saw_first,
        "should have received echo for turn-1, got: {first_events:?}"
    );

    // Reconnect SSE and send second turn
    let mut sse2 = SseCollector::connect(port).await;
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "turn-2"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let second_events = sse2.collect_events(Duration::from_secs(10)).await;
    let saw_second = second_events.iter().any(|e| e.data == "Echo: turn-2");
    assert!(
        saw_second,
        "should have received echo for turn-2, got: {second_events:?}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Large payload passthrough: 2000-char message flows through the full ACP pipeline without truncation.
/// Proves no size limit or truncation occurs at any pipeline stage.
#[rstest]
#[test_log::test(tokio::test)]
async fn when_large_payload_sent_then_response_contains_full_content() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let large_msg = "X".repeat(2000);

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": large_msg}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(15)).await;

    let saw_full_content = events.iter().any(|e| e.data.contains(&large_msg));
    assert!(
        saw_full_content,
        "expected at least one SSE event containing all 2000 chars, got events with data lengths: {:?}",
        events.iter().map(|e| e.data.len()).collect::<Vec<_>>()
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
