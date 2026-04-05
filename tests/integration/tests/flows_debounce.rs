use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout, SseCollector,
};

#[test_log::test(tokio::test)]
async fn flow_debounce_merges_messages() {
    let mut config = mock_agent_config();
    config.channels_manager.debounce.window_ms = 300;
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    for msg in ["line1", "line2", "line3"] {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": msg}))
            .send()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let events = sse.collect_events(Duration::from_secs(10)).await;
    let result_content = events
        .iter()
        .find_map(|e| {
            let v: serde_json::Value = serde_json::from_str(&e.data).ok()?;
            let update = v.get("update")?;
            if update.get("sessionUpdate")?.as_str()? == "result" {
                update.get("content")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    assert!(
        result_content.contains("line1")
            && result_content.contains("line2")
            && result_content.contains("line3"),
        "debounce should merge all 3 messages into one prompt, got: {result_content}"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
