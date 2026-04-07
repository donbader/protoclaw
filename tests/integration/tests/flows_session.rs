use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout, SseCollector,
};
use rstest::rstest;

/// Full ACP session lifecycle: init → session/new → session/prompt → streaming chunks → result.
/// Proves the entire pipeline by asserting thought events arrive and the echo result contains the message.
#[rstest]
#[test_log::test(tokio::test)]
async fn when_message_sent_then_full_acp_session_lifecycle_completes() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "lifecycle-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(10)).await;

    // Prove the full pipeline: at least one thought event arrived (session/new + session/prompt ran)
    let thought_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.event_type.as_deref() == Some("thought"))
        .map(|(i, _)| i)
        .collect();
    assert!(
        !thought_positions.is_empty(),
        "expected thought events proving ACP session pipeline ran, got: {:?}",
        events.iter().map(|e| (&e.event_type, &e.data)).collect::<Vec<_>>()
    );

    // Prove the result arrived
    let result_position = events
        .iter()
        .position(|e| e.data == "Echo: lifecycle-test")
        .expect("should have received result event via SSE");

    // Thought events must precede the result
    let last_thought = *thought_positions.last().unwrap();
    assert!(
        last_thought < result_position,
        "thought events (last at {last_thought}) must arrive before result (at {result_position})"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Session persistence across messages: second prompt reuses existing session without re-running session/new.
/// Proves by asserting both echo results appear in order.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_active_session_when_second_message_sent_then_session_persists() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    // Send first message
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "first-turn"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Give first message time to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send second message
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "second-turn"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Collect all events for both messages
    let events = sse.collect_events(Duration::from_secs(15)).await;

    let pos_first = events
        .iter()
        .position(|e| e.data == "Echo: first-turn")
        .expect("should have received echo for first-turn");
    let pos_second = events
        .iter()
        .position(|e| e.data == "Echo: second-turn")
        .expect("should have received echo for second-turn");

    assert!(
        pos_first < pos_second,
        "first-turn echo (at {pos_first}) must arrive before second-turn echo (at {pos_second})"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Session cleanup on shutdown: cancel supervisor while a session is active.
/// Proves no panic or hang — supervisor exits cleanly.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_active_session_when_supervisor_cancelled_then_shutdown_completes_cleanly() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "shutdown-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for first SSE event to confirm session is active
    let first = sse.next_event(Duration::from_secs(10)).await;
    assert!(
        first.is_some(),
        "should receive at least one SSE event before cancelling"
    );

    // Cancel supervisor with active session
    cancel.cancel();

    // Supervisor must exit cleanly — no panic, no hang
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(
        result.is_ok(),
        "supervisor should exit cleanly with active session: {result:?}"
    );
}
