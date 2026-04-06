use std::collections::HashMap;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_options, with_timeout,
};
use rstest::rstest;

#[test_log::test(tokio::test)]
async fn given_agent_configured_to_exit_after_one_message_when_second_message_sent_then_agent_recovered() {
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "first"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "after crash"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "agent should have recovered from crash");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}
