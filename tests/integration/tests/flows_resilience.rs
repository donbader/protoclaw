use std::collections::HashMap;
use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config_with_options, with_timeout,
};
use rstest::rstest;

/// Multi-crash recovery: agent exits after each message, three full crash+recover cycles succeed.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_agent_exits_after_one_prompt_when_three_messages_sent_with_recovery_waits_then_all_three_recover()
 {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    // First message — agent processes then exits
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "first"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    // Wait for backoff recovery (initial backoff is 100ms, 2s is more than enough)
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Second message — agent crashes again, supervisor recovers
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "second"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "agent should have recovered from first crash"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Third message — proves repeated recovery cycles work
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "third"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "agent should have recovered from second crash"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    cancel.cancel();
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly: {result:?}");
}

/// SSE-verified recovery: after crash+recovery the agent actually echoes back via SSE.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_agent_exits_after_one_prompt_when_second_message_sent_after_recovery_then_sse_contains_echo()
 {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(2));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    // First message triggers crash after agent echoes it
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger-crash"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Reconnect SSE after recovery so we get a fresh broadcast subscription
    let mut sse = SseCollector::connect(port).await;

    // Second message: agent echoes then survives (crashes after prompt 2, but echo is sent first)
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "verify-echo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "agent should have recovered");

    let events = sse.collect_events(Duration::from_secs(5)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo:") && e.data.contains("verify-echo"));
    assert!(
        saw_echo,
        "recovered agent should echo 'verify-echo' via SSE; got: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly: {result:?}");
}

/// Health during recovery: channel health endpoint responds even while agent is in backoff.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_agent_crashed_when_health_checked_during_recovery_then_health_returns_ok() {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    // Trigger agent crash
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "crash-trigger"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 200ms — agent is in backoff, not yet recovered (initial backoff is 100ms, recovery not done)
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Health endpoint must respond independently of agent state
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health endpoint should respond during agent recovery"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly: {result:?}");
}

/// Channel independence: channel accepts HTTP requests while agent is in recovery window.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_agent_crashed_when_message_sent_during_recovery_then_channel_accepts_request() {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    // Trigger agent crash
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "crash-trigger"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 200ms — agent is in backoff window, channel should still accept requests
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Channel should queue the message even while agent is recovering
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "during-recovery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "channel should accept requests during agent recovery"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly: {result:?}");
}

/// Shutdown during backoff: supervisor exits cleanly when cancelled while agent is in backoff.
#[rstest]
#[test_log::test(tokio::test)]
async fn given_agent_in_backoff_when_supervisor_cancelled_then_exits_cleanly() {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    // Trigger agent crash
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "crash-trigger"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 200ms — agent is in backoff, not yet recovered
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Cancel while agent is in backoff — must not hang waiting for backoff to complete
    cancel.cancel();

    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(
        result.is_ok(),
        "supervisor should exit cleanly during backoff: {result:?}"
    );
}
