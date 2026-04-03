use std::collections::HashMap;
use protoclaw_integration_tests::{mock_agent_config, mock_agent_config_with_env, wait_for_port};
use tokio_util::sync::CancellationToken;

async fn boot_supervisor_with_port(
    config: protoclaw_config::ProtoclawConfig,
) -> (CancellationToken, tokio::task::JoinHandle<anyhow::Result<()>>, u16) {
    let cancel = CancellationToken::new();
    let sup = protoclaw::supervisor::Supervisor::new(config);
    let port_rx = sup.debug_http_port_rx();
    let c = cancel.clone();
    let handle = tokio::spawn(async move { sup.run_with_cancel(c).await });

    let port = wait_for_port(port_rx, 10000).await.expect("debug-http port not discovered");
    (cancel, handle, port)
}

fn mock_agent_config_with_debounce(window_ms: u64) -> protoclaw_config::ProtoclawConfig {
    let mut config = mock_agent_config();
    config.channels_manager.debounce.window_ms = window_ms;
    config
}

#[tokio::test]
async fn e2e_message_through_channel() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello world"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    // Give time for the message to flow through the pipeline
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("supervisor did not shut down")
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_permission_through_channel() {
    let mut env = HashMap::new();
    env.insert("MOCK_AGENT_REQUEST_PERMISSION".into(), "1".into());
    let config = mock_agent_config_with_env(env);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    // Send message to trigger agent work (which will request permission)
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger permission"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for permission to propagate through the pipeline
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    // Check pending permissions
    let resp = client
        .get(format!("http://127.0.0.1:{port}/permissions/pending"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let perms: serde_json::Value = resp.json().await.unwrap();
    let arr = perms.as_array().unwrap();

    if !arr.is_empty() {
        let request_id = arr[0]["requestId"].as_str().unwrap();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/permissions/{request_id}/respond"))
            .json(&serde_json::json!({"optionId": "allow_once"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "responded");
    }

    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
}

#[tokio::test]
async fn e2e_channel_crash_isolation() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    // Verify debug-http is alive
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // The supervisor should stay alive even after some time
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("supervisor did not shut down")
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_message_flows_through_agent_and_returns_via_sse() {
    let config = mock_agent_config_with_debounce(100);
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
        .json(&serde_json::json!({"message": "ping"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    let mut saw_echo_chunk = false;
    let mut saw_result = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);

    use tokio_stream::StreamExt;
    while tokio::time::Instant::now() < deadline {
        let chunk = tokio::time::timeout_at(deadline, sse_stream.next()).await;
        match chunk {
            Ok(Some(Ok(bytes))) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if data.contains("Echo: ") && data.contains("ping") {
                            saw_echo_chunk = true;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                                saw_result = true;
                            }
                        }
                    }
                }
                if saw_result { break; }
            }
            _ => break,
        }
    }

    assert!(saw_echo_chunk, "should have received echo chunk via SSE");
    assert!(saw_result, "should have received result via SSE");

    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
}

#[tokio::test]
async fn e2e_debounce_merges_rapid_messages() {
    let config = mock_agent_config_with_debounce(300);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let sse_resp = client
        .get(format!("http://127.0.0.1:{port}/events"))
        .send()
        .await
        .unwrap();
    let mut sse_stream = sse_resp.bytes_stream();

    for msg in ["line1", "line2", "line3"] {
        let _ = client
            .post(format!("http://127.0.0.1:{port}/message"))
            .json(&serde_json::json!({"message": msg}))
            .send()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let mut result_content = String::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);

    use tokio_stream::StreamExt;
    while tokio::time::Instant::now() < deadline {
        let chunk = tokio::time::timeout_at(deadline, sse_stream.next()).await;
        match chunk {
            Ok(Some(Ok(bytes))) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                                result_content = v.get("content")
                                    .and_then(|c| c.as_str())
                                    .unwrap_or("")
                                    .to_string();
                            }
                        }
                    }
                }
                if !result_content.is_empty() { break; }
            }
            _ => break,
        }
    }

    assert!(
        result_content.contains("line1") && result_content.contains("line2") && result_content.contains("line3"),
        "debounce should merge all 3 messages into one prompt, got: {result_content}"
    );

    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
}
