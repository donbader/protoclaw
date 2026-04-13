use anyclaw_integration_tests::{boot_supervisor_with_port, mock_agent_config, with_timeout};

/// GET /health returns 200 with {"status": "ok"} (channel-level health).
#[test_log::test(tokio::test)]
async fn when_supervisor_running_then_health_endpoint_returns_200_with_status_ok() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should exit cleanly");
}
