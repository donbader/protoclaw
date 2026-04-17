use std::process::{Command, Stdio};

/// Detect which agent binary (opencode, claude, gemini) is available on PATH.
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

/// Generate a starter `anyclaw.yaml` config file for the given agent binary.
pub fn generate_config_yaml(agent_binary: &str) -> String {
    format!(
        r#"# yaml-language-server: $schema=https://raw.githubusercontent.com/donbader/anyclaw/refs/heads/main/anyclaw.schema.json
# Anyclaw configuration
# Docs: https://github.com/user/anyclaw

agents_manager:
  agents:
    default:
      workspace:
        type: local
        binary: "{agent_binary}"
      args:
        - "acp"

# Channel subprocesses
channels_manager:
  channels:
    debug-http:
      binary: "anyclaw-debug-http"
      args:
        - "--port"
        - "3000"

# MCP tool servers (uncomment to add)
# tools_manager:
#   tools:
#     filesystem:
#       binary: "mcp-server-filesystem"
#       args:
#         - "--root"
#         - "."
"#
    )
}

/// Run the `anyclaw init` command: detect agent, generate config, write to disk.
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

    #[test]
    fn when_no_known_binary_on_path_then_detect_agent_binary_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = temp_env::with_var("PATH", Some(dir.path().to_str().unwrap()), || {
            detect_agent_binary()
        });
        assert!(result.is_none());
    }

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
    fn when_generate_config_yaml_called_then_contains_agents_and_channels_but_not_supervisor() {
        let yaml = generate_config_yaml("opencode");
        assert!(yaml.contains("agents_manager:"));
        assert!(yaml.contains("channels_manager:"));
        assert!(!yaml.contains("supervisor:"));
    }

    #[test]
    fn when_generate_config_yaml_called_then_yaml_deserializes_to_valid_anyclaw_config() {
        let yaml = generate_config_yaml("opencode");
        let result = serde_yaml::from_str::<anyclaw_config::AnyclawConfig>(&yaml);
        assert!(result.is_ok(), "YAML failed to parse: {:?}", result.err());
    }

    #[test]
    fn when_generate_config_yaml_called_then_first_line_is_yaml_language_server_modeline() {
        let yaml = generate_config_yaml("opencode");
        assert_eq!(
            yaml.lines().next().unwrap(),
            "# yaml-language-server: $schema=https://raw.githubusercontent.com/donbader/anyclaw/refs/heads/main/anyclaw.schema.json"
        );
    }

    #[test]
    fn given_existing_config_when_run_init_without_force_then_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anyclaw.yaml");
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
        let path = dir.path().join("anyclaw.yaml");
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
        let path = dir.path().join("anyclaw.yaml");
        let path_str = path.to_str().unwrap();
        run_init(path_str, false).unwrap();
        assert!(path.exists(), "file should be created");
        let content = std::fs::read_to_string(&path).unwrap();
        let result = serde_yaml::from_str::<anyclaw_config::AnyclawConfig>(&content);
        assert!(
            result.is_ok(),
            "created YAML should be valid: {:?}",
            result.err()
        );
    }
}
