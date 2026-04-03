use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level protoclaw configuration.
///
/// Loaded from layered providers: defaults → TOML file → environment variables.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoclawConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_extensions_dir")]
    pub extensions_dir: String,
    /// Legacy single-agent config (deprecated, use [[agents]]).
    #[serde(default)]
    pub agent: Option<LegacyAgentConfig>,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub wasm_tools: Vec<WasmToolConfig>,
    #[serde(default)]
    pub supervisor: SupervisorConfig,
}

/// Legacy single-agent config (deprecated, use [[agents]]).
///
/// Kept for backward compatibility with `[agent]` TOML format.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyAgentConfig {
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
}

/// Agent process configuration — the ACP-speaking AI agent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Channel subprocess configuration — user-facing interfaces.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub agent: Option<String>,
}

/// MCP tool server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmToolConfig {
    pub name: String,
    pub module: PathBuf,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: Option<String>,
    #[serde(default)]
    pub sandbox: WasmSandboxConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmSandboxConfig {
    #[serde(default = "default_fuel_limit")]
    pub fuel_limit: u64,
    #[serde(default = "default_epoch_timeout")]
    pub epoch_timeout_secs: u64,
    #[serde(default = "default_memory_limit")]
    pub memory_limit_bytes: u64,
    #[serde(default)]
    pub preopened_dirs: Vec<PreopenedDir>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreopenedDir {
    pub host: PathBuf,
    pub guest: String,
    #[serde(default)]
    pub readonly: bool,
}

fn default_fuel_limit() -> u64 {
    1_000_000
}
fn default_epoch_timeout() -> u64 {
    30
}
fn default_memory_limit() -> u64 {
    67_108_864
}
fn default_log_level() -> String {
    "info".into()
}
fn default_extensions_dir() -> String {
    "/usr/local/bin".into()
}
fn default_true() -> bool {
    true
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            fuel_limit: default_fuel_limit(),
            epoch_timeout_secs: default_epoch_timeout(),
            memory_limit_bytes: default_memory_limit(),
            preopened_dirs: Vec::new(),
        }
    }
}

impl ProtoclawConfig {
    pub fn normalize_agents(&mut self) {
        if self.agents.is_empty() {
            if let Some(legacy) = self.agent.take() {
                tracing::warn!("[agent] is deprecated, use [[agents]] instead");
                self.agents.push(AgentConfig {
                    name: "default".into(),
                    binary: legacy.binary,
                    args: legacy.args,
                    enabled: true,
                    env: legacy.env,
                    working_dir: legacy.working_dir,
                    tools: vec![],
                });
            }
        }
    }

    pub fn default_agent_name(&self) -> Option<&str> {
        self.agents
            .iter()
            .find(|a| a.enabled)
            .map(|a| a.name.as_str())
    }
}

/// Supervisor settings with sensible defaults.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupervisorConfig {
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    #[serde(default = "default_restart_window")]
    pub restart_window_secs: u64,
}

fn default_shutdown_timeout() -> u64 {
    30
}
fn default_health_interval() -> u64 {
    5
}
fn default_max_restarts() -> u32 {
    5
}
fn default_restart_window() -> u64 {
    60
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_secs: default_shutdown_timeout(),
            health_check_interval_secs: default_health_interval(),
            max_restarts: default_max_restarts(),
            restart_window_secs: default_restart_window(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_defaults_to_info() {
        let toml = r#"
            [[agents]]
            name = "default"
            binary = "opencode"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn log_level_from_toml() {
        let toml = r#"
            log_level = "debug"
            [[agents]]
            name = "default"
            binary = "opencode"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn extensions_dir_defaults() {
        let toml = r#"
            [[agents]]
            name = "default"
            binary = "opencode"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.extensions_dir, "/usr/local/bin");
    }

    #[test]
    fn extensions_dir_from_toml() {
        let toml = r#"
            extensions_dir = "/custom/path"
            [[agents]]
            name = "default"
            binary = "opencode"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.extensions_dir, "/custom/path");
    }

    #[test]
    fn channel_enabled_defaults_true() {
        let toml = r#"
            name = "debug-http"
            binary = "debug-http"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn channel_enabled_false() {
        let toml = r#"
            name = "telegram"
            binary = "telegram-channel"
            enabled = false
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn mcp_server_enabled_defaults_true() {
        let toml = r#"
            name = "filesystem"
            binary = "mcp-server-filesystem"
        "#;
        let config: McpServerConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn mcp_server_enabled_false() {
        let toml = r#"
            name = "filesystem"
            binary = "mcp-server-filesystem"
            enabled = false
        "#;
        let config: McpServerConfig = toml::from_str(toml).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn legacy_agent_config_with_env_deserializes() {
        let toml = r#"
            binary = "opencode"
            args = ["acp"]
            working_dir = "/workspace"

            [env]
            OPENCODE_ENABLE_QUESTION_TOOL = "1"
            MY_VAR = "hello"
        "#;
        let config: LegacyAgentConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.binary, "opencode");
        assert_eq!(config.env.len(), 2);
        assert_eq!(config.env["OPENCODE_ENABLE_QUESTION_TOOL"], "1");
        assert_eq!(config.working_dir, Some(PathBuf::from("/workspace")));
    }

    #[test]
    fn legacy_agent_config_env_defaults_empty() {
        let toml = r#"
            binary = "opencode"
        "#;
        let config: LegacyAgentConfig = toml::from_str(toml).unwrap();
        assert!(config.env.is_empty());
    }

    #[test]
    fn legacy_agent_config_working_dir_optional() {
        let toml = r#"
            binary = "opencode"
        "#;
        let config: LegacyAgentConfig = toml::from_str(toml).unwrap();
        assert!(config.working_dir.is_none());
    }

    #[test]
    fn wasm_sandbox_config_defaults() {
        let config = WasmSandboxConfig::default();
        assert_eq!(config.fuel_limit, 1_000_000);
        assert_eq!(config.epoch_timeout_secs, 30);
        assert_eq!(config.memory_limit_bytes, 67_108_864);
        assert!(config.preopened_dirs.is_empty());
    }

    #[test]
    fn wasm_tool_config_deserializes_full() {
        let toml = r#"
            name = "my-tool"
            module = "/path/to/tool.wasm"
            description = "A test tool"
            input_schema = '{"type": "object"}'

            [sandbox]
            fuel_limit = 500000
            epoch_timeout_secs = 10
            memory_limit_bytes = 33554432
        "#;
        let config: WasmToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "my-tool");
        assert_eq!(config.module, PathBuf::from("/path/to/tool.wasm"));
        assert_eq!(config.description, "A test tool");
        assert!(config.input_schema.is_some());
        assert_eq!(config.sandbox.fuel_limit, 500_000);
        assert_eq!(config.sandbox.epoch_timeout_secs, 10);
    }

    #[test]
    fn wasm_tool_config_deserializes_with_default_sandbox() {
        let toml = r#"
            name = "minimal"
            module = "tool.wasm"
        "#;
        let config: WasmToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "minimal");
        assert_eq!(config.sandbox.fuel_limit, 1_000_000);
        assert_eq!(config.sandbox.epoch_timeout_secs, 30);
    }

    #[test]
    fn agents_array_deserializes() {
        let toml = r#"
            [[agents]]
            name = "opencode"
            binary = "opencode"
            args = ["acp"]
            tools = ["system-info", "filesystem"]

            [agents.env]
            ANTHROPIC_API_KEY = "sk-test"

            [[agents]]
            name = "claude-code"
            binary = "claude"
            enabled = false
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.agents[0].name, "opencode");
        assert_eq!(config.agents[0].binary, "opencode");
        assert_eq!(config.agents[0].args, vec!["acp"]);
        assert!(config.agents[0].enabled);
        assert_eq!(config.agents[0].tools, vec!["system-info", "filesystem"]);
        assert_eq!(config.agents[0].env["ANTHROPIC_API_KEY"], "sk-test");
        assert_eq!(config.agents[1].name, "claude-code");
        assert!(!config.agents[1].enabled);
    }

    #[test]
    fn legacy_agent_normalizes_to_agents_vec() {
        let toml = r#"
            [agent]
            binary = "opencode"
            args = ["acp"]

            [agent.env]
            MY_KEY = "val"
        "#;
        let mut config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert!(config.agents.is_empty());
        config.normalize_agents();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].name, "default");
        assert_eq!(config.agents[0].binary, "opencode");
        assert_eq!(config.agents[0].args, vec!["acp"]);
        assert!(config.agents[0].enabled);
        assert_eq!(config.agents[0].env["MY_KEY"], "val");
        assert!(config.agents[0].tools.is_empty());
        assert!(config.agent.is_none());
    }

    #[test]
    fn agents_array_takes_precedence_over_legacy_agent() {
        let toml = r#"
            [agent]
            binary = "should-be-ignored"

            [[agents]]
            name = "real"
            binary = "opencode"
        "#;
        let mut config: ProtoclawConfig = toml::from_str(toml).unwrap();
        config.normalize_agents();
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].binary, "opencode");
    }

    #[test]
    fn agent_config_enabled_defaults_true() {
        let toml = r#"
            name = "test"
            binary = "test-agent"
        "#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert!(config.tools.is_empty());
        assert!(config.env.is_empty());
        assert!(config.args.is_empty());
    }

    #[test]
    fn channel_config_agent_field() {
        let toml = r#"
            name = "telegram"
            binary = "telegram-channel"
            agent = "opencode"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.agent, Some("opencode".to_string()));
    }

    #[test]
    fn channel_config_agent_defaults_none() {
        let toml = r#"
            name = "debug-http"
            binary = "debug-http"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert!(config.agent.is_none());
    }

    #[test]
    fn empty_config_normalize_agents_no_panic() {
        let toml = r#""#;
        let mut config: ProtoclawConfig = toml::from_str(toml).unwrap();
        config.normalize_agents();
        assert!(config.agents.is_empty());
    }

    #[test]
    fn default_agent_name_returns_first_enabled() {
        let toml = r#"
            [[agents]]
            name = "disabled-one"
            binary = "x"
            enabled = false

            [[agents]]
            name = "enabled-one"
            binary = "y"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.default_agent_name(), Some("enabled-one"));
    }

    #[test]
    fn default_agent_name_none_when_empty() {
        let toml = r#""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.default_agent_name(), None);
    }
}
