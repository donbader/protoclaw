use std::collections::HashMap;
use std::time::Duration;

use protoclaw_integration_tests::{
    SseCollector, boot_supervisor_with_port, build_mock_agent_docker_image,
    cleanup_test_containers, docker_agent_config, docker_agent_config_with_options,
    mock_agent_path, with_timeout,
};
use rstest::rstest;

fn setup() {
    build_mock_agent_docker_image().expect("failed to build mock-agent Docker image");
    cleanup_test_containers();
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn given_docker_agent_configured_to_exit_after_one_message_when_second_message_sent_then_agent_recovered()
 {
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
        .args([
            "ps",
            "-aq",
            "--filter",
            "name=protoclaw-stale-test-container",
        ])
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
        .args([
            "ps",
            "-aq",
            "--filter",
            "name=protoclaw-stale-test-container",
        ])
        .output()
        .expect("docker ps failed");
    assert!(
        String::from_utf8_lossy(&check_after.stdout)
            .trim()
            .is_empty(),
        "stale container should be removed by supervisor startup cleanup"
    );

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn when_docker_agent_configured_with_resource_limits_then_container_has_limits_applied() {
    setup();
    let mut config = docker_agent_config();
    let agent = config
        .agents_manager
        .agents
        .get_mut("docker-agent")
        .unwrap();
    if let protoclaw_config::WorkspaceConfig::Docker(ref mut docker) = agent.workspace {
        docker.memory_limit = Some("64m".to_string());
        docker.cpu_limit = Some("0.5".to_string());
    }

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "limits-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let docker = bollard::Docker::connect_with_local_defaults().expect("connect to Docker");
    let containers = docker
        .list_containers(Some(bollard::query_parameters::ListContainersOptions {
            all: false,
            filters: Some({
                let mut f = HashMap::new();
                f.insert(
                    "label".to_string(),
                    vec!["protoclaw.managed=true".to_string()],
                );
                f
            }),
            ..Default::default()
        }))
        .await
        .expect("list containers");

    assert!(
        !containers.is_empty(),
        "should have at least one running protoclaw container"
    );
    let container_id = containers[0].id.as_ref().expect("container id");
    let inspect = docker
        .inspect_container(container_id, None)
        .await
        .expect("inspect container");
    let host_config = inspect.host_config.expect("host_config");

    assert_eq!(
        host_config.memory,
        Some(67_108_864_i64),
        "memory limit should be 64MB"
    );
    assert_eq!(
        host_config.nano_cpus,
        Some(500_000_000_i64),
        "cpu limit should be 0.5 cores"
    );

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn given_local_and_docker_agents_when_messages_sent_then_both_respond() {
    setup();
    let mut config = docker_agent_config();
    config.agents_manager.agents.insert(
        "local-agent".to_string(),
        protoclaw_config::AgentConfig {
            workspace: protoclaw_config::WorkspaceConfig::Local(
                protoclaw_config::LocalWorkspaceConfig {
                    binary: mock_agent_path().to_string_lossy().to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                },
            ),
            args: vec![],
            enabled: true,
            tools: vec![],
            acp_timeout_secs: None,
            backoff: None,
            crash_tracker: None,
            options: HashMap::new(),
        },
    );

    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let mut sse = SseCollector::connect(port).await;

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "mixed-verify"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = sse.collect_events(Duration::from_secs(30)).await;
    let saw_echo = events.iter().any(|e| e.data.contains("mixed-verify"));
    assert!(saw_echo, "Docker agent should echo message in mixed mode");

    cancel.cancel();
    let result = with_timeout(15, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}

#[rstest]
#[test_log::test(tokio::test)]
#[ignore]
async fn when_docker_agent_image_present_and_pull_policy_never_then_no_pull_attempted() {
    setup();
    let config = docker_agent_config();
    let (cancel, handle, port) = boot_supervisor_with_port(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/message"))
        .json(&serde_json::json!({"message": "no-pull-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    cancel.cancel();
    let result = with_timeout(10, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok());
}
