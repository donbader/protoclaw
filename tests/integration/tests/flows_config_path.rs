use anyclaw_config::AnyclawConfig;
use anyclaw_integration_tests::{boot_supervisor_with_port, mock_agent_config, with_timeout};

#[test_log::test(tokio::test)]
async fn given_config_written_to_temp_path_when_loaded_and_supervisor_booted_then_health_responds()
{
    // Write a valid config to a temp file at a non-default path
    let config = mock_agent_config();
    let yaml = serde_yaml::to_string(&config).expect("serialize config to yaml");

    let temp_path =
        std::env::temp_dir().join(format!("anyclaw-test-{}.yaml", std::process::id()));
    std::fs::write(&temp_path, &yaml).expect("write temp config file");

    let path_str = temp_path.to_str().expect("temp path is valid UTF-8");

    let loaded =
        AnyclawConfig::load(Some(path_str)).expect("config should load from custom path");

    let (cancel, handle, port) = boot_supervisor_with_port(loaded).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .expect("health request should succeed");
    assert_eq!(resp.status(), 200);

    cancel.cancel();
    let result = with_timeout(5, handle)
        .await
        .expect("supervisor task panicked");
    assert!(result.is_ok(), "supervisor should shut down cleanly");

    std::fs::remove_file(&temp_path).expect("remove temp config file");
}

#[test]
fn given_missing_config_path_when_load_called_then_error_contains_filename() {
    let missing = "/tmp/nonexistent-anyclaw-test.yaml";
    let result = AnyclawConfig::load(Some(missing));

    assert!(result.is_err(), "should fail for nonexistent config path");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nonexistent-anyclaw-test.yaml"),
        "error message should contain the file path, got: {err_msg}"
    );
}
