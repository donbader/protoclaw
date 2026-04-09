use protoclaw_integration_tests::{boot_supervisor_with_port, mock_agent_config, with_timeout};
use rstest::rstest;

#[rstest]
#[test_log::test(tokio::test)]
async fn when_message_posted_with_no_body_then_returns_4xx() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_message_posted_with_empty_json_then_returns_4xx() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected 4xx for empty JSON, got {}",
        resp.status()
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}

#[rstest]
#[test_log::test(tokio::test)]
async fn when_message_posted_with_wrong_content_type_then_returns_4xx() {
    let config = mock_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .body("not json")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected 4xx for wrong content type, got {}",
        resp.status()
    );

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
