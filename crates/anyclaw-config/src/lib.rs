#![warn(missing_docs)]

//! Figment-based layered configuration for anyclaw.
//!
//! Loading order: embedded defaults → user YAML file (with `!env` tag resolution)
//! → environment variables (`ANYCLAW_` prefix, `__` separator).

/// YAML provider with `!env` environment variable tag resolution.
pub mod env_yaml;
/// Configuration error types.
pub mod error;
/// Per-extension sidecar defaults loading and merging.
pub mod extension_defaults;
/// K8s-style resource limit parsers (memory, CPU).
pub mod parse;
/// Binary path resolution (`@built-in/` prefix expansion).
pub mod resolve;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
/// All configuration structs and their serde defaults.
pub mod types;
/// Semantic config validation (binary existence, working dir checks, Docker limits).
pub mod validate;

pub use error::*;
pub use extension_defaults::*;
pub use parse::*;
pub use resolve::*;
pub use types::*;
pub use validate::*;

use figment::{Figment, providers::Format};

/// Generate the JSON Schema for `AnyclawConfig` as a `serde_json::Value`.
///
/// Uses schemars to derive the schema from Rust types. The output follows
/// JSON Schema Draft 2020-12.
#[allow(clippy::disallowed_types)] // Schema is inherently untyped JSON
pub fn generate_schema() -> serde_json::Value {
    let schema = schemars::schema_for!(AnyclawConfig);
    serde_json::to_value(schema).expect("schema serialization cannot fail")
}

impl AnyclawConfig {
    /// Load configuration from layered providers: defaults → YAML file → env vars.
    ///
    /// Returns [`ConfigError::LoadFailed`] if the config file does not exist or
    /// cannot be parsed.
    pub fn load(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let path = config_path.unwrap_or("anyclaw.yaml");

        if !std::path::Path::new(path).exists() {
            return Err(ConfigError::LoadFailed {
                path: path.to_string(),
                reason: format!("config file not found: {path}"),
            });
        }

        let config: Self = Figment::from(figment::providers::Yaml::string(DEFAULTS_YAML))
            .merge(env_yaml::EnvYaml::file(path))
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

    #[test]
    fn when_generate_schema_called_then_schema_has_schema_key_with_draft_2020() {
        let schema = generate_schema();
        let schema_key = schema["$schema"]
            .as_str()
            .expect("$schema must be a string");
        assert!(
            schema_key.contains("2020-12"),
            "$schema should reference Draft 2020-12, got: {schema_key}"
        );
    }

    #[test]
    fn when_generate_schema_called_then_title_is_anyclaw_config() {
        let schema = generate_schema();
        let title = schema["title"].as_str().expect("title must be a string");
        assert_eq!(title, "AnyclawConfig");
    }

    #[test]
    fn when_generate_schema_called_then_properties_contains_all_top_level_keys() {
        let schema = generate_schema();
        let properties = schema["properties"]
            .as_object()
            .expect("properties must be an object");
        let required_keys = [
            "log_level",
            "log_format",
            "extensions_dir",
            "agents_manager",
            "channels_manager",
            "tools_manager",
            "supervisor",
            "session_store",
        ];
        for key in required_keys {
            assert!(
                properties.contains_key(key),
                "properties missing key: {key}"
            );
        }
    }

    #[test]
    fn when_committed_schema_file_exists_then_it_matches_generate_schema_output() {
        let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ dir")
            .parent()
            .expect("repo root")
            .join("anyclaw.schema.json");
        assert!(
            schema_path.exists(),
            "anyclaw.schema.json missing at {}",
            schema_path.display()
        );
        let committed_raw =
            std::fs::read_to_string(&schema_path).expect("failed to read anyclaw.schema.json");
        let committed: serde_json::Value =
            serde_json::from_str(&committed_raw).expect("anyclaw.schema.json is not valid JSON");
        let generated = generate_schema();
        assert_eq!(
            committed, generated,
            "anyclaw.schema.json is out of date — run `cargo test -- generate_and_write_schema --ignored`"
        );
    }

    #[test]
    fn when_example_configs_loaded_then_they_validate_against_schema() {
        let schema = generate_schema();
        let validator = jsonschema::validator_for(&schema).expect("schema must compile");

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ dir")
            .parent()
            .expect("repo root");

        let patterns = ["examples/*/anyclaw.yaml", "examples/*/*/anyclaw.yaml"];

        let mut validated_count = 0;
        for pattern in patterns {
            let full_pattern = repo_root.join(pattern);
            let glob_str = full_pattern.to_str().expect("valid path");
            for entry in glob::glob(glob_str).expect("valid glob").flatten() {
                let content = std::fs::read_to_string(&entry)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", entry.display()));
                let mut yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", entry.display()));
                env_yaml::resolve_env_tags(&mut yaml_value).unwrap_or_else(|e| {
                    panic!("env tag resolution failed for {}: {e}", entry.display())
                });
                let json_value: serde_json::Value = serde_json::to_value(&yaml_value)
                    .unwrap_or_else(|e| {
                        panic!("YAML→JSON conversion failed for {}: {e}", entry.display())
                    });
                let errors: Vec<_> = validator.iter_errors(&json_value).collect();
                assert!(
                    errors.is_empty(),
                    "{} failed schema validation:\n{}",
                    entry.display(),
                    errors
                        .iter()
                        .map(|e| format!("  - {e}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                validated_count += 1;
            }
        }
        assert!(validated_count > 0, "no example configs found to validate");
    }

    #[test]
    #[ignore]
    fn generate_and_write_schema() {
        let schema = generate_schema();
        let pretty =
            serde_json::to_string_pretty(&schema).expect("schema serialization cannot fail");
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ dir")
            .parent()
            .expect("repo root");
        let schema_path = repo_root.join("anyclaw.schema.json");
        let content = format!("{pretty}\n");
        std::fs::write(&schema_path, &content)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", schema_path.display()));
        println!("Wrote {}", schema_path.display());
    }
}
