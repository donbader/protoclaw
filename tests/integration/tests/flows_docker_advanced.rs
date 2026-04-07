use std::collections::HashMap;
use std::time::Duration;

use protoclaw_integration_tests::{
    boot_supervisor_with_port, build_mock_agent_docker_image, cleanup_test_containers,
    docker_agent_config, docker_agent_config_with_options, with_timeout,
};
use rstest::rstest;

fn setup() {
    build_mock_agent_docker_image().expect("failed to build mock-agent Docker image");
    cleanup_test_containers();
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn given_docker_agent_configured_to_exit_after_one_message_when_second_message_sent_then_agent_recovered(
) {
    setup();
    let mut opts = HashMap::new();
    opts.insert("exit_after".into(), serde_json::json!(1));
    let config = docker_agent_config_with_options(opts);
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "first"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(Duration::from_secs(5)).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "after crash"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "agent should have recovered from Docker crash"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn given_stale_container_exists_when_supervisor_starts_then_stale_container_removed() {
    setup();

    let stale_output = std::process::Command::new("docker")
        .args([
            "create",
            "--name",
            "protoclaw-stale-test-container",
            "--label",
            "protoclaw.managed=true",
            "--label",
            "protoclaw.agent=docker-agent",
            "protoclaw-mock-agent:test",
        ])
        .output()
        .expect("failed to create stale container");
    assert!(
        stale_output.status.success(),
        "failed to create stale container: {}",
        String::from_utf8_lossy(&stale_output.stderr)
    );

    let check = std::process::Command::new("docker")
        .args(["ps", "-aq", "--filter", "name=protoclaw-stale-test-container"])
        .output()
        .expect("docker ps failed");
    assert!(
        !String::from_utf8_lossy(&check.stdout).trim().is_empty(),
        "stale container should exist before supervisor start"
    );

    let config = docker_agent_config();
    let (cancel, handle, _port) = boot_supervisor_with_port(config).await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let check_after = std::process::Command::new("docker")
        .args(["ps", "-aq", "--filter", "name=protoclaw-stale-test-container"])
        .output()
        .expect("docker ps failed");
    assert!(
        String::from_utf8_lossy(&check_after.stdout).trim().is_empty(),
        "stale container should be removed by supervisor startup cleanup"
    );

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}
