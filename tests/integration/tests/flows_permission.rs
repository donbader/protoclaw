use std::collections::HashMap;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, mock_agent_config_with_env, with_timeout,
};

#[test_log::test(tokio::test)]
async fn flow_permission_request_and_respond() {
    let mut env = HashMap::new();
    env.insert("MOCK_AGENT_REQUEST_PERMISSION".into(), "1".into());
    let config = mock_agent_config_with_env(env);
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
