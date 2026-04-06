//! Validation tests for example configuration files.
//!
//! Ensures every example's protoclaw.yaml parses against the current config schema
//! and every docker-compose.yml is syntactically valid.
//!
//! | Test                           | Validates                                      |
//! |--------------------------------|------------------------------------------------|
//! | example_01_config_parses       | Fake-agent example config loads via Figment     |
//! | example_02_config_parses       | Real-agent example config loads via Figment     |
//! | example_01_docker_compose_valid| Fake-agent docker-compose.yml syntax            |
//! | example_02_docker_compose_valid| Real-agent docker-compose.yml syntax            |

use figment::Jail;
use protoclaw_config::ProtoclawConfig;
use rstest::rstest;

#[test]
fn given_example_01_yaml_when_loaded_via_figment_jail_then_config_parses() {
    Jail::expect_with(|jail| {
        jail.set_env("TELEGRAM_BOT_TOKEN", "test-token");
        jail.set_env("TELEGRAM_ENABLED", "false");

        jail.create_file(
            "protoclaw.yaml",
            include_str!("../../../examples/01-fake-agent-telegram-bot/protoclaw.yaml"),
        )?;

        let config =
            ProtoclawConfig::load(Some("protoclaw.yaml")).expect("example 01 config should parse");

        assert!(config.agents_manager.agents.contains_key("mock"));
        assert!(config.channels_manager.channels.contains_key("debug-http"));
        assert!(config.channels_manager.channels.contains_key("telegram"));
        assert!(config.tools_manager.tools.contains_key("system-info"));

        Ok(())
    });
}

#[test]
fn given_example_02_yaml_when_loaded_via_figment_jail_then_config_parses() {
    Jail::expect_with(|jail| {
        jail.set_env("ANTHROPIC_API_KEY", "sk-test");
        jail.set_env("TELEGRAM_BOT_TOKEN", "test-token");
        jail.set_env("TELEGRAM_ENABLED", "false");

        jail.create_file(
            "protoclaw.yaml",
            include_str!("../../../examples/02-real-agents-telegram-bot/protoclaw.yaml"),
        )?;

        let config =
            ProtoclawConfig::load(Some("protoclaw.yaml")).expect("example 02 config should parse");

        assert!(config.agents_manager.agents.contains_key("opencode"));
        assert!(config.agents_manager.agents.contains_key("claude-code"));
        assert!(config.channels_manager.channels.contains_key("debug-http"));
        assert!(config.channels_manager.channels.contains_key("telegram"));

        Ok(())
    });
}

fn ensure_env_file(example_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let env_file = example_dir.join(".env");
    if env_file.exists() {
        return None;
    }
    let example_file = example_dir.join(".env.example");
    if example_file.exists() {
        std::fs::copy(&example_file, &env_file).expect("failed to copy .env.example to .env");
        Some(env_file)
    } else {
        std::fs::write(&env_file, "").expect("failed to create empty .env");
        Some(env_file)
    }
}

fn validate_docker_compose(example_name: &str) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let example_dir =
        std::path::PathBuf::from(format!("{manifest_dir}/../../examples/{example_name}"));
    let compose_path = example_dir.join("docker-compose.yml");

    let created_env = ensure_env_file(&example_dir);

    let output = std::process::Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_path.to_str().unwrap(),
            "config",
            "--quiet",
        ])
        .output()
        .expect("failed to run docker compose");

    if let Some(path) = created_env {
        let _ = std::fs::remove_file(path);
    }

    assert!(
        output.status.success(),
        "{example_name} docker-compose.yml is invalid: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore] // requires docker
fn given_example_01_when_docker_compose_config_runs_then_syntax_is_valid() {
    validate_docker_compose("01-fake-agent-telegram-bot");
}

#[test]
#[ignore] // requires docker
fn given_example_02_when_docker_compose_config_runs_then_syntax_is_valid() {
    validate_docker_compose("02-real-agents-telegram-bot");
}
