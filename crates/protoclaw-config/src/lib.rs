pub mod error;
pub mod resolve;
pub mod subst_toml;
pub mod types;
pub mod validate;

pub use error::*;
pub use resolve::*;
pub use types::*;
pub use validate::*;

use figment::{
    providers::{Env, Serialized},
    Figment,
};

impl ProtoclawConfig {
    pub fn load(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let path = config_path.unwrap_or("protoclaw.toml");

        if !std::path::Path::new(path).exists() {
            return Err(ConfigError::LoadFailed {
                path: path.to_string(),
                reason: format!("config file not found: {path}"),
            });
        }

        let config: Self = Figment::from(Serialized::defaults(SupervisorConfig::default()))
            .merge(subst_toml::SubstToml::file(path))
            .merge(Env::prefixed("PROTOCLAW_").split("__"))
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
    use figment::Jail;

    #[test]
    fn load_valid_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
                args = ["--headless"]

                [channels-manager.channels.debug-http]
                binary = "protoclaw-debug-http"

                [tools-manager.tools.filesystem]
                binary = "mcp-server-filesystem"
                args = ["--root", "/workspace"]

                [supervisor]
                shutdown_timeout_secs = 15
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.agents_manager.agents.len(), 1);
            assert_eq!(config.agents_manager.agents["default"].binary, "opencode");
            assert_eq!(
                config.agents_manager.agents["default"].args,
                vec!["--headless"]
            );
            assert_eq!(config.channels_manager.channels.len(), 1);
            assert!(config.channels_manager.channels.contains_key("debug-http"));
            assert_eq!(config.tools_manager.tools.len(), 1);
            assert!(config.tools_manager.tools.contains_key("filesystem"));
            assert_eq!(config.supervisor.shutdown_timeout_secs, 15);
            Ok(())
        });
    }

    #[test]
    fn missing_config_file_returns_error() {
        Jail::expect_with(|_jail| {
            let result = ProtoclawConfig::load(Some("nonexistent.toml"));
            assert!(result.is_err());
            let err = result.unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("nonexistent.toml"),
                "error should mention file path: {msg}"
            );
            Ok(())
        });
    }

    #[test]
    fn supervisor_defaults() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
            assert_eq!(config.supervisor.health_check_interval_secs, 5);
            assert_eq!(config.supervisor.max_restarts, 5);
            assert_eq!(config.supervisor.restart_window_secs, 60);
            Ok(())
        });
    }

    #[test]
    fn empty_channels_defaults_to_empty_map() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert!(config.channels_manager.channels.is_empty());
            Ok(())
        });
    }

    #[test]
    fn empty_tools_defaults_to_empty_map() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert!(config.tools_manager.tools.is_empty());
            Ok(())
        });
    }

    #[test]
    fn env_var_overrides_supervisor_shutdown_timeout() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"

                [supervisor]
                shutdown_timeout_secs = 30
            "#,
            )?;
            jail.set_env("PROTOCLAW_SUPERVISOR__SHUTDOWN_TIMEOUT_SECS", "60");
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.supervisor.shutdown_timeout_secs, 60);
            Ok(())
        });
    }

    #[test]
    fn unknown_keys_ignored() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
                unknown_field = "should be ignored"

                [some_future_section]
                key = "value"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml"));
            assert!(
                config.is_ok(),
                "unknown keys should be ignored: {:?}",
                config.err()
            );
            Ok(())
        });
    }

    #[test]
    fn config_with_only_agents_section() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agents-manager.agents.default]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.agents_manager.agents["default"].binary, "opencode");
            assert!(config.channels_manager.channels.is_empty());
            assert!(config.tools_manager.tools.is_empty());
            Ok(())
        });
    }

    #[test]
    fn config_error_displays_file_path() {
        Jail::expect_with(|_jail| {
            let result = ProtoclawConfig::load(Some("missing/path/protoclaw.toml"));
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("missing/path/protoclaw.toml"),
                "error should include file path: {msg}"
            );
            Ok(())
        });
    }
}
