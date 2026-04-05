use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

#[test]
fn test_fake_agent_example_yaml_parses() {
    let yaml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/protoclaw.yaml");
    let config = protoclaw_config::ProtoclawConfig::load(Some(yaml_path.to_str().unwrap()))
        .unwrap_or_else(|e| panic!("failed to load protoclaw.yaml: {e}"));

    assert_eq!(config.agents_manager.agents.len(), 1);
    let mock = config
        .agents_manager
        .agents
        .get("mock")
        .expect("missing 'mock' agent");
    match &mock.workspace {
        protoclaw_config::WorkspaceConfig::Local(local) => {
            assert_eq!(local.binary, "@built-in/mock-agent");
        }
        other => panic!("expected Local workspace, got {other:?}"),
    }
}

#[test]
fn test_fake_agent_example_has_channels() {
    let yaml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/protoclaw.yaml");
    let config =
        protoclaw_config::ProtoclawConfig::load(Some(yaml_path.to_str().unwrap())).unwrap();

    assert_eq!(
        config.channels_manager.channels.len(),
        2,
        "expected two channels"
    );
    assert!(config.channels_manager.channels.contains_key("debug-http"));
    assert!(config.channels_manager.channels.contains_key("telegram"));
}

#[test]
fn test_real_agent_example_yaml_parses() {
    let yaml_path = workspace_root().join("examples/02-real-agents-telegram-bot/protoclaw.yaml");
    let config = protoclaw_config::ProtoclawConfig::load(Some(yaml_path.to_str().unwrap()))
        .unwrap_or_else(|e| panic!("failed to load protoclaw.yaml: {e}"));

    assert_eq!(config.agents_manager.agents.len(), 2);
    let opencode = config
        .agents_manager
        .agents
        .get("opencode")
        .expect("missing 'opencode' agent");
    match &opencode.workspace {
        protoclaw_config::WorkspaceConfig::Local(local) => {
            assert_eq!(local.binary, "opencode");
        }
        other => panic!("expected Local workspace, got {other:?}"),
    }
    assert!(opencode.enabled);

    let claude = config
        .agents_manager
        .agents
        .get("claude-code")
        .expect("missing 'claude-code' agent");
    match &claude.workspace {
        protoclaw_config::WorkspaceConfig::Local(local) => {
            assert_eq!(local.binary, "claude");
        }
        other => panic!("expected Local workspace, got {other:?}"),
    }
    assert!(!claude.enabled);
}

#[test]
fn test_real_agent_example_has_channels() {
    let yaml_path = workspace_root().join("examples/02-real-agents-telegram-bot/protoclaw.yaml");
    let config =
        protoclaw_config::ProtoclawConfig::load(Some(yaml_path.to_str().unwrap())).unwrap();

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
