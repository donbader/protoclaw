use std::{collections::HashMap, time::Duration};

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_options, with_timeout, SseCollector,
};
use rstest::rstest;

/// Rapid-fire 10 messages with no delay between POSTs — queue must not drop any.
/// All 10 echo responses must arrive and FIFO ordering must be preserved.
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

    // Assert all 10 echoes present
    for i in 0..10 {
        let expected = format!("msg-{i}");
        let found = events.iter().any(|e| e.data.contains(&expected));
        assert!(
            found,
            "should echo '{expected}' but did not; events: {:?}",
            events.iter().map(|e| &e.data).collect::<Vec<_>>()
        );
    }

    // Assert FIFO ordering: position of msg-0 echo < msg-1 echo < ... < msg-9 echo
    let positions: Vec<usize> = (0..10)
        .map(|i| {
            let expected = format!("msg-{i}");
            events
                .iter()
                .position(|e| e.data.contains(&expected))
                .unwrap_or_else(|| panic!("echo for '{expected}' not found in events"))
        })
        .collect();

    for i in 1..10 {
        assert!(
            positions[i - 1] < positions[i],
            "FIFO violated: msg-{} at position {} must precede msg-{} at position {}",
            i - 1,
            positions[i - 1],
            i,
            positions[i]
        );
    }

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Messages sent with varying delays between POSTs must all arrive and maintain FIFO order.
/// Delays: [0ms, 50ms, 200ms, 0ms, 100ms].
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

    // Assert all 5 echoes present
    for i in 0..5 {
        let expected = format!("timed-{i}");
        let found = events.iter().any(|e| e.data.contains(&expected));
        assert!(
            found,
            "should echo '{expected}' but did not; events: {:?}",
            events.iter().map(|e| &e.data).collect::<Vec<_>>()
        );
    }

    // Assert FIFO ordering
    let positions: Vec<usize> = (0..5)
        .map(|i| {
            let expected = format!("timed-{i}");
            events
                .iter()
                .position(|e| e.data.contains(&expected))
                .unwrap_or_else(|| panic!("echo for '{expected}' not found in events"))
        })
        .collect();

    for i in 1..5 {
        assert!(
            positions[i - 1] < positions[i],
            "FIFO violated: timed-{} at position {} must precede timed-{} at position {}",
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
