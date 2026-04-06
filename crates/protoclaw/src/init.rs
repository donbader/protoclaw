use std::process::{Command, Stdio};

pub fn detect_agent_binary() -> Option<String> {
    for binary in &["opencode", "claude", "gemini"] {
        let ok = Command::new(binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(binary.to_string());
        }
    }
    None
}

pub fn generate_config_yaml(agent_binary: &str) -> String {
    format!(
        r#"# Protoclaw configuration
# Docs: https://github.com/user/protoclaw

agents-manager:
  agents:
    default:
      workspace:
        type: local
        binary: "{agent_binary}"
      args:
        - "acp"

# Channel subprocesses
channels-manager:
  channels:
    debug-http:
      binary: "protoclaw-debug-http"
      args:
        - "--port"
        - "3000"

# MCP tool servers (uncomment to add)
# tools-manager:
#   tools:
#     filesystem:
#       binary: "mcp-server-filesystem"
#       args:
#         - "--root"
#         - "."

supervisor:
  shutdown_timeout_secs: 30
  health_check_interval_secs: 5
  max_restarts: 5
  restart_window_secs: 60
"#
    )
}

pub fn run_init(config_path: &str, force: bool) -> anyhow::Result<()> {
    let path = std::path::Path::new(config_path);
    if path.exists() && !force {
        return Err(anyhow::anyhow!(
            "{config_path} already exists. Use --force to overwrite."
        ));
    }

    let binary = detect_agent_binary().unwrap_or_else(|| {
        eprintln!("Warning: no known agent binary found on PATH; defaulting to 'opencode'");
        "opencode".to_string()
    });

    let content = generate_config_yaml(&binary);
    std::fs::write(config_path, &content)?;

    println!("Created {config_path}");
    if detect_agent_binary().is_some() {
        println!("  Detected agent: {binary}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_generate_config_yaml_called_then_contains_agent_binary() {
        let yaml = generate_config_yaml("opencode");
        assert!(yaml.contains(r#"binary: "opencode""#));
    }

    #[test]
    fn when_generate_config_yaml_called_then_yaml_contains_comment_lines() {
        let yaml = generate_config_yaml("opencode");
        assert!(yaml.lines().any(|l| l.trim_start().starts_with('#')));
    }

    #[test]
    fn when_generate_config_yaml_called_then_contains_agents_manager_and_supervisor_sections() {
        let yaml = generate_config_yaml("opencode");
        assert!(yaml.contains("agents-manager:"));
        assert!(yaml.contains("supervisor:"));
    }

    #[test]
    fn when_generate_config_yaml_called_then_yaml_deserializes_to_valid_protoclaw_config() {
        let yaml = generate_config_yaml("opencode");
        let result = serde_yaml::from_str::<protoclaw_config::ProtoclawConfig>(&yaml);
        assert!(result.is_ok(), "YAML failed to parse: {:?}", result.err());
    }

    #[test]
    fn given_existing_config_when_run_init_without_force_then_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protoclaw.yaml");
        std::fs::write(&path, "existing").unwrap();
        let path_str = path.to_str().unwrap();
        let result = run_init(path_str, false);
        assert!(
            result.is_err(),
            "should refuse to overwrite without --force"
        );
    }

    #[test]
    fn given_existing_config_when_run_init_with_force_then_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protoclaw.yaml");
        std::fs::write(&path, "existing").unwrap();
        let path_str = path.to_str().unwrap();
        let result = run_init(path_str, true);
        assert!(
            result.is_ok(),
            "should overwrite with --force: {:?}",
            result.err()
        );
    }

    #[test]
    fn given_no_existing_config_when_run_init_then_creates_file_with_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protoclaw.yaml");
        let path_str = path.to_str().unwrap();
        run_init(path_str, false).unwrap();
        assert!(path.exists(), "file should be created");
        let content = std::fs::read_to_string(&path).unwrap();
        let result = serde_yaml::from_str::<protoclaw_config::ProtoclawConfig>(&content);
        assert!(
            result.is_ok(),
            "created YAML should be valid: {:?}",
            result.err()
        );
    }
}
