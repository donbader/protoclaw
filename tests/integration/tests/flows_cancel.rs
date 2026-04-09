use protoclaw_integration_tests::{boot_supervisor_with_port, mock_agent_config, with_timeout};
use rstest::rstest;

#[rstest]
#[test_log::test(tokio::test)]
async fn when_cancel_posted_then_returns_cancelled_status() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "cancelled");

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
