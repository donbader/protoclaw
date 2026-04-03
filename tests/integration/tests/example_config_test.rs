use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

#[test]
fn test_fake_agent_example_toml_parses() {
    let toml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/protoclaw.toml");
    let config = protoclaw_config::ProtoclawConfig::load(Some(toml_path.to_str().unwrap()))
        .unwrap_or_else(|e| panic!("failed to load protoclaw.toml: {e}"));

    assert_eq!(config.agents_manager.agents.len(), 1);
    let mock = config
        .agents_manager
        .agents
        .get("mock")
        .expect("missing 'mock' agent");
    assert_eq!(mock.binary, "@built-in/mock-agent");
}

#[test]
fn test_fake_agent_example_has_channels() {
    let toml_path = workspace_root().join("examples/01-fake-agent-telegram-bot/protoclaw.toml");
    let config =
        protoclaw_config::ProtoclawConfig::load(Some(toml_path.to_str().unwrap())).unwrap();

    assert_eq!(
        config.channels_manager.channels.len(),
        2,
        "expected two channels"
    );
    assert!(config.channels_manager.channels.contains_key("debug-http"));
    assert!(config.channels_manager.channels.contains_key("telegram"));
}
