use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

#[test]
fn test_example_toml_parses() {
    let toml_path = workspace_root().join("examples/telegram-bot/protoclaw.toml");
    let content = std::fs::read_to_string(&toml_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", toml_path.display()));
    let config: protoclaw_config::ProtoclawConfig =
        toml::from_str(&content).unwrap_or_else(|e| panic!("failed to parse protoclaw.toml: {e}"));

    assert_eq!(config.agent.binary, "protoclaw");
}

#[test]
fn test_example_agent_args_contain_acp() {
    let toml_path = workspace_root().join("examples/telegram-bot/protoclaw.toml");
    let content = std::fs::read_to_string(&toml_path).unwrap();
    let config: protoclaw_config::ProtoclawConfig = toml::from_str(&content).unwrap();

    assert_eq!(config.agent.args, vec!["acp"]);
}

#[test]
fn test_example_has_telegram_channel() {
    let toml_path = workspace_root().join("examples/telegram-bot/protoclaw.toml");
    let content = std::fs::read_to_string(&toml_path).unwrap();
    let config: protoclaw_config::ProtoclawConfig = toml::from_str(&content).unwrap();

    assert_eq!(config.channels.len(), 1, "expected exactly one channel");
    assert_eq!(config.channels[0].name, "telegram");
    assert_eq!(config.channels[0].binary, "telegram-channel");
}

#[test]
fn test_env_example_exists_with_bot_token() {
    let env_path = workspace_root().join("examples/telegram-bot/.env.example");
    let content = std::fs::read_to_string(&env_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", env_path.display()));

    assert!(
        content.contains("TELEGRAM_BOT_TOKEN="),
        ".env.example must contain TELEGRAM_BOT_TOKEN="
    );
}
