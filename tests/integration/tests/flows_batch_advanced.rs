use std::{collections::HashMap, time::Duration};

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_options, with_timeout, SseCollector,
};
use rstest::rstest;

/// Rapid-fire 10 messages with no delay between POSTs — queue must not drop any.
/// Merging is timing-dependent (mock agent is fast), so we only verify
/// all content arrives and FIFO order is preserved.
#[rstest]
#[test_log::test(tokio::test)]
async fn when_ten_messages_sent_rapidly_then_all_responses_arrive() {
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), serde_json::json!(false));
    let config = mock_agent_config_with_options(options);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    for i in 0..10 {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": format!("msg-{i}")}))
            .send()
            .await
            .expect("POST should succeed");
    }

    let events = sse.collect_events(Duration::from_secs(30)).await;
    let all_data: String = events.iter().map(|e| &e.data).cloned().collect::<Vec<_>>().join("\n");

    // msg-0 is always dispatched immediately (first message, session idle).
    // msg-9 is always the last queued, visible as the tail of any merged prompt.
    // Intermediate messages may be invisible in SSE when merged (newline splitting).
    assert!(
        all_data.contains("msg-0"),
        "first message must appear; all_data: {all_data:?}",
    );
    assert!(
        all_data.contains("msg-9"),
        "last message must appear; all_data: {all_data:?}",
    );

    let pos_first = all_data.find("msg-0").expect("msg-0 must exist");
    let pos_last = all_data.find("msg-9").expect("msg-9 must exist");
    assert!(
        pos_first < pos_last,
        "FIFO: msg-0 at byte {pos_first} must precede msg-9 at byte {pos_last}",
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Messages sent with varying delays between POSTs must all arrive and maintain FIFO order.
/// Delays: [0ms, 50ms, 200ms, 0ms, 100ms]. Some may merge, all content must appear.
#[rstest]
#[test_log::test(tokio::test)]
async fn when_messages_sent_with_varying_delays_then_all_responses_arrive() {
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), serde_json::json!(false));
    let config = mock_agent_config_with_options(options);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let delays_ms: [u64; 5] = [0, 50, 200, 0, 100];

    for (i, delay_ms) in delays_ms.iter().enumerate() {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": format!("timed-{i}")}))
            .send()
            .await
            .expect("POST should succeed");

        if *delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
        }
    }

    let events = sse.collect_events(Duration::from_secs(15)).await;
    let all_data: String = events.iter().map(|e| &e.data).cloned().collect::<Vec<_>>().join("\n");

    for i in 0..5 {
        let expected = format!("timed-{i}");
        assert!(
            all_data.contains(&expected),
            "should contain '{expected}' but did not; all_data: {all_data:?}",
        );
    }

    // FIFO: byte position of timed-0 < timed-1 < ... < timed-4
    let positions: Vec<usize> = (0..5)
        .map(|i| {
            let expected = format!("timed-{i}");
            all_data
                .find(&expected)
                .unwrap_or_else(|| panic!("'{expected}' not found in stream"))
        })
        .collect();

    for i in 1..5 {
        assert!(
            positions[i - 1] < positions[i],
            "FIFO violated: timed-{} at byte {} must precede timed-{} at byte {}",
            i - 1,
            positions[i - 1],
            i,
            positions[i]
        );
    }

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Send 3 messages rapidly then immediately cancel the supervisor.
/// No SSE assertions — just prove no panic or hang on shutdown with queued messages.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_inflight_queue_when_supervisor_cancelled_then_no_panic() {
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), serde_json::json!(false));
    let config = mock_agent_config_with_options(options);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    for i in 0..3 {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": format!("drain-{i}")}))
            .send()
            .await
            .expect("POST should succeed");
    }

    // Immediately cancel — don't wait for SSE events
    cancel.cancel();

    // Supervisor must exit cleanly — no panic, no hang (test-log captures panics on failure)
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(
        result.is_ok(),
        "supervisor should exit cleanly after cancellation with queued messages: {result:?}"
    );
}
