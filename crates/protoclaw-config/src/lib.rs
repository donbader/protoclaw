pub mod error;
pub mod types;

pub use error::*;
pub use types::*;

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};

impl ProtoclawConfig {
    pub fn load(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let path = config_path.unwrap_or("protoclaw.toml");

        let config: Self = Figment::from(Serialized::defaults(SupervisorConfig::default()))
            .merge(Toml::file(path))
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
                [agent]
                binary = "opencode"
                args = ["--headless"]

                [[channels]]
                name = "debug-http"
                binary = "protoclaw-debug-http"

                [[mcp_servers]]
                name = "filesystem"
                binary = "mcp-server-filesystem"
                args = ["--root", "/workspace"]

                [supervisor]
                shutdown_timeout_secs = 15
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.agent.binary, "opencode");
            assert_eq!(config.agent.args, vec!["--headless"]);
            assert_eq!(config.channels.len(), 1);
            assert_eq!(config.channels[0].name, "debug-http");
            assert_eq!(config.mcp_servers.len(), 1);
            assert_eq!(config.mcp_servers[0].name, "filesystem");
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
                [agent]
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
    fn empty_channels_defaults_to_empty_vec() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agent]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert!(config.channels.is_empty());
            Ok(())
        });
    }

    #[test]
    fn empty_mcp_servers_defaults_to_empty_vec() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agent]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert!(config.mcp_servers.is_empty());
            Ok(())
        });
    }

    #[test]
    fn env_var_overrides_agent_binary() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agent]
                binary = "opencode"
            "#,
            )?;
            jail.set_env("PROTOCLAW_AGENT__BINARY", "claude-code");
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.agent.binary, "claude-code");
            Ok(())
        });
    }

    #[test]
    fn env_var_overrides_supervisor_shutdown_timeout() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agent]
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
                [agent]
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
    fn config_with_only_agent_section() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "protoclaw.toml",
                r#"
                [agent]
                binary = "opencode"
            "#,
            )?;
            let config = ProtoclawConfig::load(Some("protoclaw.toml")).unwrap();
            assert_eq!(config.agent.binary, "opencode");
            assert!(config.channels.is_empty());
            assert!(config.mcp_servers.is_empty());
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
