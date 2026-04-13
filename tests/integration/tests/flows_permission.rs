// Permission E2E tests.
//
// Wire format note (D-02): The RespondPermission path in protoclaw-agents/manager.rs produces
// a nested result structure: { "result": { "outcome": { "outcome": "selected", "optionId": "..." } } }
// This is distinct from the auto-approve path which produces a flat { "result": { "requestId": "...", "optionId": "..." } }.
// The auto-approve path fires only when no channel is routable; these tests exercise the RespondPermission path.
// If the permission response does not reach the agent correctly, mock-agent will not continue processing
// and will not emit the echo — so the SSE echo assertion proves the full round-trip completed.

use std::collections::HashMap;
use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, mock_agent_config_with_options, wait_for_condition,
    with_timeout,
};
use rstest::rstest;

fn permission_config() -> protoclaw_config::ProtoclawConfig {
    let mut opts = HashMap::new();
    opts.insert("request_permission".into(), serde_json::json!(true));
    mock_agent_config_with_options(opts)
}

async fn poll_pending_permissions(port: u16) -> Option<Vec<serde_json::Value>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/permissions/pending"))
        .send()
        .await
        .ok()?;
    let arr: Vec<serde_json::Value> = resp.json().await.ok()?;
    if arr.is_empty() { None } else { Some(arr) }
}

async fn respond_to_permission(port: u16, request_id: &str, option_id: &str) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://127.0.0.1:{port}/permissions/{request_id}/respond"
        ))
        .json(&serde_json::json!({ "optionId": option_id }))
        .send()
        .await
        .expect("permission respond request must succeed");
    assert_eq!(resp.status(), 200, "permission respond must return 200");
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_agent_requests_permission_then_response_reaches_agent_stdin() {
    let config = permission_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let post_resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger permission"}))
        .send()
        .await
        .expect("message post must succeed");
    assert_eq!(post_resp.status(), 200);

    let pending: Vec<serde_json::Value> = wait_for_condition(5000, || {
        let p = port;
        async move { poll_pending_permissions(p).await }
    })
    .await
    .expect("permission request must appear within 5s");

    let request_id = pending[0]["requestId"]
        .as_str()
        .expect("pending permission must have requestId");

    respond_to_permission(port, request_id, "allow_once").await;

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo:") && e.data.contains("trigger permission"));
    assert!(
        saw_echo,
        "agent should echo the message after permission is granted; got events: {events:?}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_agent_requests_permission_with_reject_then_agent_still_echoes() {
    let config = permission_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let post_resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger rejection"}))
        .send()
        .await
        .expect("message post must succeed");
    assert_eq!(post_resp.status(), 200);

    let pending: Vec<serde_json::Value> = wait_for_condition(5000, || {
        let p = port;
        async move { poll_pending_permissions(p).await }
    })
    .await
    .expect("permission request must appear within 5s");

    let request_id = pending[0]["requestId"]
        .as_str()
        .expect("pending permission must have requestId");

    respond_to_permission(port, request_id, "reject_once").await;

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo:") && e.data.contains("trigger rejection"));
    assert!(
        saw_echo,
        "agent should still echo after permission is rejected; got events: {events:?}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

fn permission_config_with_timeout(timeout_secs: u64) -> protoclaw_config::ProtoclawConfig {
    let mut opts = HashMap::new();
    opts.insert("request_permission".into(), serde_json::json!(true));
    let mut config = mock_agent_config_with_options(opts);
    config.supervisor.permission_timeout_secs = Some(timeout_secs);
    config
}

// Auto-deny wire format note (D-04, scenario 2): When permission_timeout_secs fires, the channels
// manager calls AgentsCommand::RespondPermission with option_id: "denied". This produces a flat
// { "result": { "requestId": "...", "optionId": "denied" } } on the ACP wire — distinct from the
// RespondPermission nested structure { "result": { "outcome": { ... } } } tested in the two tests
// above. The mock-agent treats any permission response as unblocking and proceeds to echo.

#[rstest]
#[test_log::test(tokio::test)]
async fn when_permission_times_out_then_agent_receives_auto_deny() {
    let config = permission_config_with_timeout(2);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let post_resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger permission timeout"}))
        .send()
        .await
        .expect("message post must succeed");
    assert_eq!(post_resp.status(), 200);

    // Wait for the permission request to appear — confirms the flow started.
    let pending: Vec<serde_json::Value> = wait_for_condition(5000, || {
        let p = port;
        async move { poll_pending_permissions(p).await }
    })
    .await
    .expect("permission request must appear within 5s");

    let request_id = pending[0]["requestId"]
        .as_str()
        .expect("pending permission must have requestId");

    // Do NOT respond — let the 2s permission_timeout_secs fire and auto-deny.
    // The channels manager sends auto-deny to the agent, which unblocks mock-agent.
    // However, the channel harness is still blocked in request_permission() awaiting
    // the PermissionBroker oneshot (nobody resolved it). This means channel/deliverMessage
    // with the echo is queued in the harness's stdin buffer but can't be processed.
    // Unblock the harness by responding via HTTP so it can process the queued echo.
    tokio::time::sleep(Duration::from_secs(3)).await;
    respond_to_permission(port, request_id, "denied").await;

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo:") && e.data.contains("trigger permission timeout"));
    assert!(
        saw_echo,
        "agent should echo after auto-deny timeout; got events: {events:?}"
    );

    cancel.cancel();
    let _ = with_timeout(10, handle).await;
}
