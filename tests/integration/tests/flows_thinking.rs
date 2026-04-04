use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config, with_timeout,
};
use tokio_stream::StreamExt;

#[test_log::test(tokio::test)]
async fn flow_thinking_chunks() {
    let mut config = mock_agent_config();
    config.channels_manager.debounce.window_ms = 100;
    config
        .agents_manager
        .agents
        .get_mut("default")
        .unwrap()
        .options
        .insert("thinking".into(), serde_json::json!(true));
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let sse_resp = client
        .get(format!("http://127.0.0.1:{port}/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(sse_resp.status(), 200);
    let mut sse_stream = sse_resp.bytes_stream();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "think-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let mut saw_thought = false;
    let mut saw_result = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    while tokio::time::Instant::now() < deadline {
        let chunk = tokio::time::timeout_at(deadline, sse_stream.next()).await;
        match chunk {
            Ok(Some(Ok(bytes))) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.starts_with("event:") && line.contains("thought") {
                        saw_thought = true;
                    }
                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                                saw_result = true;
                            }
                        }
                    }
                }
                if saw_result {
                    break;
                }
            }
            _ => break,
        }
    }

    assert!(
        saw_thought,
        "should have received at least one thought chunk via SSE"
    );
    assert!(
        saw_result,
        "thinking agent should complete and return result via SSE"
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
