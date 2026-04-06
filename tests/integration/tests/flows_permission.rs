use std::collections::HashMap;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_options, with_timeout,
};
use rstest::rstest;

#[test_log::test(tokio::test)]
async fn given_agent_requests_permission_when_responded_to_then_permission_acknowledged() {
    let mut opts = HashMap::new();
    opts.insert("request_permission".into(), serde_json::json!(true));
    let config = mock_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger permission"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

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
            .post(format!(
                "http://127.0.0.1:{port}/permissions/{request_id}/respond"
            ))
            .json(&serde_json::json!({"optionId": "allow_once"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "responded");
    }

    cancel.cancel();
    let _ = with_timeout(5, handle).await;
}
