use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, build_mock_agent_docker_image,
    cleanup_test_containers, docker_agent_config, with_timeout,
};
use rstest::rstest;

fn setup() {
    build_mock_agent_docker_image().expect("failed to build mock-agent Docker image");
    cleanup_test_containers();
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn when_docker_agent_spawned_then_sse_echo_works() {
    setup();
    let config = docker_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();
    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "docker-ping"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(30)).await;
    let saw_echo = events
        .iter()
        .any(|e| e.data.contains("Echo: ") && e.data.contains("docker-ping"));
    assert!(saw_echo, "Docker agent should echo message via SSE");

    cancel.cancel();
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn when_docker_agent_running_and_supervisor_cancelled_then_container_removed() {
    setup();
    let config = docker_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "pre-shutdown"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    tokio::time::sleep(Duration::from_secs(2)).await;

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());

    let output = std::process::Command::new("docker")
        .args(["ps", "-aq", "--filter", "label=protoclaw.managed=true"])
        .output()
        .expect("docker ps failed");
    let remaining = String::from_utf8_lossy(&output.stdout);
    assert!(
        remaining.trim().is_empty(),
        "no protoclaw containers should remain after shutdown, found: {remaining}"
    );
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn when_message_posted_to_docker_agent_then_response_status_is_queued() {
    setup();
    let config = docker_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "hello docker"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");

    tokio::time::sleep(Duration::from_millis(500)).await;
    cancel.cancel();
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}
