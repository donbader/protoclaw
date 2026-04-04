use protoclaw_integration_tests::{boot_supervisor_with_port, mock_agent_config, with_timeout};

#[test_log::test(tokio::test)]
async fn flow_boot_and_shutdown() {
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
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}
