use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

#[test]
fn given_fake_agent_example_yaml_when_loaded_then_has_one_mock_agent() {
    let yaml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/anyclaw.yaml");
    let config = anyclaw_config::AnyclawConfig::load(Some(yaml_path.to_str().unwrap()))
        .unwrap_or_else(|e| panic!("failed to load anyclaw.yaml: {e}"));

    assert_eq!(config.agents_manager.agents.len(), 2);
    let mock = config
        .agents_manager
        .agents
        .get("mock")
        .expect("missing 'mock' agent");
    match &mock.workspace {
        anyclaw_config::WorkspaceConfig::Local(local) => {
            assert_eq!(
                local.binary,
                anyclaw_config::StringOrArray::from("@built-in/agents/mock-agent")
            );
        }
        other => panic!("expected Local workspace, got {other:?}"),
    }

    let mock_docker = config
        .agents_manager
        .agents
        .get("mock-docker")
        .expect("missing 'mock-docker' agent");
    assert!(!mock_docker.enabled);
    match &mock_docker.workspace {
        anyclaw_config::WorkspaceConfig::Docker(docker) => {
            assert_eq!(docker.image, "anyclaw-mock-agent:latest");
        }
        other => panic!("expected Docker workspace, got {other:?}"),
    }
}

#[test]
fn given_fake_agent_example_yaml_when_loaded_then_has_debug_http_and_telegram_channels() {
    let yaml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/anyclaw.yaml");
    let config = anyclaw_config::AnyclawConfig::load(Some(yaml_path.to_str().unwrap())).unwrap();

    assert_eq!(
        config.channels_manager.channels.len(),
        2,
        "expected two channels"
    );
    assert!(config.channels_manager.channels.contains_key("debug-http"));
    assert!(config.channels_manager.channels.contains_key("telegram"));
}

#[test]
fn given_real_agent_example_yaml_when_loaded_then_has_opencode_agent() {
    let yaml_path =
        workspace_root().join("examples/02-real-agent-telegram/agent-opencode/anyclaw.yaml");
    let config = anyclaw_config::AnyclawConfig::load(Some(yaml_path.to_str().unwrap()))
        .unwrap_or_else(|e| panic!("failed to load anyclaw.yaml: {e}"));

    assert_eq!(config.agents_manager.agents.len(), 1);
    let opencode = config
        .agents_manager
        .agents
        .get("opencode")
        .expect("missing 'opencode' agent");
    match &opencode.workspace {
        anyclaw_config::WorkspaceConfig::Docker(docker) => {
            assert_eq!(docker.image, "anyclaw-opencode-agent:latest");
            assert_eq!(
                docker.entrypoint,
                Some(anyclaw_config::StringOrArray(vec![
                    "opencode".into(),
                    "acp".into()
                ]))
            );
        }
        other => panic!("expected Docker workspace, got {other:?}"),
    }
    assert!(opencode.enabled);
}

#[test]
fn given_real_agent_example_yaml_when_loaded_then_has_two_channels_with_correct_routing() {
    let yaml_path =
        workspace_root().join("examples/02-real-agent-telegram/agent-opencode/anyclaw.yaml");
    let config = anyclaw_config::AnyclawConfig::load(Some(yaml_path.to_str().unwrap())).unwrap();

    assert_eq!(
        config.channels_manager.channels.len(),
        2,
        "expected two channels"
    );
    assert!(config.channels_manager.channels.contains_key("debug-http"));
    assert!(config.channels_manager.channels.contains_key("telegram"));
    assert_eq!(
        config.channels_manager.channels["debug-http"].agent,
        "opencode"
    );
}
