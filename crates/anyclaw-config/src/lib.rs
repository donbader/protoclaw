pub mod error;
pub mod parse;
pub mod resolve;
pub mod subst_yaml;
pub mod types;
pub mod validate;

pub use error::*;
pub use parse::*;
pub use resolve::*;
pub use types::*;
pub use validate::*;

use figment::{Figment, providers::Format};

impl AnyclawConfig {
    pub fn load(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let path = config_path.unwrap_or("anyclaw.yaml");

        if !std::path::Path::new(path).exists() {
            return Err(ConfigError::LoadFailed {
                path: path.to_string(),
                reason: format!("config file not found: {path}"),
            });
        }

        let config: Self = Figment::from(figment::providers::Yaml::string(DEFAULTS_YAML))
            .merge(subst_yaml::SubstYaml::file(path))
            .extract()
            .map_err(|e| ConfigError::LoadFailed {
                path: path.to_string(),
                reason: e.to_string(),
            })?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkspaceConfig;
    use figment::Jail;

    #[test]
    fn when_valid_config_file_exists_then_loads_all_sections() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                r#"
agents_manager:
  agents:
    default:
      workspace:
        type: local
        binary: "opencode"
      args:
        - "--headless"

channels_manager:
  channels:
    debug-http:
      binary: "anyclaw-debug-http"

tools_manager:
  tools:
    filesystem:
      binary: "mcp-server-filesystem"
      args:
        - "--root"
        - "/workspace"

supervisor:
  shutdown_timeout_secs: 15
"#,
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
            assert_eq!(config.agents_manager.agents.len(), 1);
            match &config.agents_manager.agents["default"].workspace {
                WorkspaceConfig::Local(local) => {
                    assert_eq!(local.binary, crate::StringOrArray::from("opencode"))
                }
                _ => panic!("expected Local variant"),
            }
            assert_eq!(config.channels_manager.channels.len(), 1);
            assert!(config.channels_manager.channels.contains_key("debug-http"));
            assert_eq!(config.tools_manager.tools.len(), 1);
            assert!(config.tools_manager.tools.contains_key("filesystem"));
            assert_eq!(config.supervisor.shutdown_timeout_secs, 15);
            Ok(())
        });
    }

    #[test]
    fn when_config_file_does_not_exist_then_returns_error_with_path() {
        Jail::expect_with(|_jail| {
            let result = AnyclawConfig::load(Some("nonexistent.yaml"));
            assert!(result.is_err());
            let err = result.unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("nonexistent.yaml"),
                "error should mention file path: {msg}"
            );
            Ok(())
        });
    }

    #[test]
    fn when_config_has_no_supervisor_section_then_uses_defaults() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                "agents_manager:\n  agents:\n    default:\n      workspace:\n        type: local\n        binary: \"opencode\"\n",
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
            assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
            assert_eq!(config.supervisor.health_check_interval_secs, 5);
            assert_eq!(config.supervisor.max_restarts, 5);
            assert_eq!(config.supervisor.restart_window_secs, 60);
            Ok(())
        });
    }

    #[test]
    fn when_config_has_no_channels_section_then_channels_map_is_empty() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                "agents_manager:\n  agents:\n    default:\n      workspace:\n        type: local\n        binary: \"opencode\"\n",
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
            assert!(config.channels_manager.channels.is_empty());
            Ok(())
        });
    }

    #[test]
    fn when_config_has_no_tools_section_then_tools_map_is_empty() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                "agents_manager:\n  agents:\n    default:\n      workspace:\n        type: local\n        binary: \"opencode\"\n",
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
            assert!(config.tools_manager.tools.is_empty());
            Ok(())
        });
    }

    #[test]
    fn when_yaml_has_unknown_keys_then_load_succeeds() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                "agents_manager:\n  agents:\n    default:\n      workspace:\n        type: local\n        binary: \"opencode\"\n      unknown_field: \"should be ignored\"\nsome_future_section:\n  key: \"value\"\n",
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml"));
            assert!(
                config.is_ok(),
                "unknown keys should be ignored: {:?}",
                config.err()
            );
            Ok(())
        });
    }

    #[test]
    fn when_config_has_only_agents_section_then_other_sections_default() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "anyclaw.yaml",
                "agents_manager:\n  agents:\n    default:\n      workspace:\n        type: local\n        binary: \"opencode\"\n",
            )?;
            let config = AnyclawConfig::load(Some("anyclaw.yaml")).unwrap();
            match &config.agents_manager.agents["default"].workspace {
                WorkspaceConfig::Local(local) => {
                    assert_eq!(local.binary, crate::StringOrArray::from("opencode"))
                }
                _ => panic!("expected Local variant"),
            }
            assert!(config.channels_manager.channels.is_empty());
            assert!(config.tools_manager.tools.is_empty());
            Ok(())
        });
    }

    #[test]
    fn when_config_file_missing_then_error_message_includes_path() {
        Jail::expect_with(|_jail| {
            let result = AnyclawConfig::load(Some("missing/path/anyclaw.yaml"));
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("missing/path/anyclaw.yaml"),
                "error should include file path: {msg}"
            );
            Ok(())
        });
    }
}
