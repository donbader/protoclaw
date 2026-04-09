use std::{collections::HashMap, time::Duration};

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_options, with_timeout, SseCollector,
};
use rstest::rstest;

fn slow_agent_config() -> protoclaw_config::ProtoclawConfig {
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), serde_json::json!(true));
    options.insert("thinking_time_ms".to_string(), serde_json::json!(1000));
    mock_agent_config_with_options(options)
}

/// Send 5 messages rapidly while agent has thinking_time_ms=1000.
/// First message dispatches immediately. While agent is busy thinking,
/// remaining messages queue. On completion, queued messages merge into
/// a single prompt. Verify the echo contains multiple messages joined by \n.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_slow_agent_when_messages_sent_rapidly_then_queued_messages_merge_into_single_prompt()
{
    let config = slow_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    for i in 0..5 {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": format!("merge-{i}")}))
            .send()
            .await
            .expect("POST should succeed");
    }

    let events = sse.collect_events(Duration::from_secs(20)).await;
    let all_data: String = events
        .iter()
        .map(|e| e.data.clone())
        .collect::<Vec<_>>()
        .join("\n");

    for i in 0..5 {
        let expected = format!("merge-{i}");
        assert!(
            all_data.contains(&expected),
            "should contain '{expected}'; all_data: {all_data:?}",
        );
    }

    let has_merged_echo = events.iter().any(|e| {
        let mut count = 0;
        for i in 0..5 {
            if e.data.contains(&format!("merge-{i}")) {
                count += 1;
            }
        }
        count >= 2
    });
    assert!(
        has_merged_echo,
        "at least one SSE event should contain 2+ merged messages; events: {events:?}",
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

/// Rapid-fire 10 messages with no delay — verify all content arrives and FIFO order holds.
/// Without thinking_time_ms, merging is not guaranteed (agent may process faster than messages arrive).
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
    let all_data: String = events
        .iter()
        .map(|e| &e.data)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

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

/// Messages sent with varying delays must all arrive and maintain FIFO order.
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
    let all_data: String = events
        .iter()
        .map(|e| &e.data)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    for i in 0..5 {
        let expected = format!("timed-{i}");
        assert!(
            all_data.contains(&expected),
            "should contain '{expected}' but did not; all_data: {all_data:?}",
        );
    }

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

    cancel.cancel();

    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(
        result.is_ok(),
        "supervisor should exit cleanly after cancellation with queued messages: {result:?}"
    );
}
