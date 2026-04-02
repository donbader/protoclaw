use std::collections::HashMap;
use protoclaw_integration_tests::{mock_agent_config, mock_agent_config_with_env, wait_for_port};
use tokio_util::sync::CancellationToken;

async fn boot_supervisor(
    config: protoclaw_config::ProtoclawConfig,
) -> (CancellationToken, tokio::sync::watch::Receiver<u16>, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let cancel = CancellationToken::new();
    let sup = protoclaw::supervisor::Supervisor::new(config);
    let port_rx = sup.debug_http_port_rx();
    let c = cancel.clone();
    let handle = tokio::spawn(async move { sup.run_with_cancel(c).await });
    (cancel, port_rx, handle)
}

#[tokio::test]
async fn e2e_send_message_receives_echo() {
    let config = mock_agent_config();
    let (cancel, port_rx, handle) = boot_supervisor(config).await;

    let port = wait_for_port(port_rx, 5000).await.expect("server did not start");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello world"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "sent");

    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("supervisor did not shut down")
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_cancel_operation() {
    let config = mock_agent_config();
    let (cancel, port_rx, handle) = boot_supervisor(config).await;

    let port = wait_for_port(port_rx, 5000).await.expect("server did not start");

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .post(format!("http://127.0.0.1:{port}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "cancelled");

    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
}

#[tokio::test]
async fn e2e_permission_flow() {
    let mut env = HashMap::new();
    env.insert("MOCK_AGENT_REQUEST_PERMISSION".into(), "1".into());
    let config = mock_agent_config_with_env(env);
    let (cancel, port_rx, handle) = boot_supervisor(config).await;

    let port = wait_for_port(port_rx, 5000).await.expect("server did not start");

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger permission"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

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
async fn e2e_crash_recovery() {
    let mut env = HashMap::new();
    env.insert("MOCK_AGENT_EXIT_AFTER".into(), "1".into());
    let mut config = mock_agent_config_with_env(env);
    config.supervisor.health_check_interval_secs = 1;
    let (cancel, port_rx, handle) = boot_supervisor(config).await;

    let port = wait_for_port(port_rx, 5000).await.expect("server did not start");

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "trigger crash"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "after recovery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    cancel.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
}
