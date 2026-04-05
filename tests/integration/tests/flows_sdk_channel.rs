use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, sdk_channel_config, with_timeout, SseCollector,
};

#[test_log::test(tokio::test)]
async fn flow_sdk_channel_round_trip() {
    let config = sdk_channel_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let health = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), 200, "debug-http health check failed");

    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "sdk-channel-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "POST /message failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    let events = sse.collect_events(Duration::from_secs(10)).await;

    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("sdk-channel-test"));
    let saw_result = events.iter().any(|e| {
        serde_json::from_str::<serde_json::Value>(&e.data)
            .ok()
            .and_then(|v| v.get("update")?.get("sessionUpdate")?.as_str().map(|s| s == "result"))
            .unwrap_or(false)
    });

    assert!(
        saw_echo,
        "should have received echo chunk via SSE; events: {events:?}"
    );
    assert!(
        saw_result,
        "should have received result event via SSE; events: {events:?}"
    );

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");
}
