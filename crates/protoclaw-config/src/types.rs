use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level protoclaw configuration.
///
/// Loaded from layered providers: defaults → TOML file → environment variables.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoclawConfig {
    pub agent: AgentConfig,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub wasm_tools: Vec<WasmToolConfig>,
    #[serde(default)]
    pub supervisor: SupervisorConfig,
}

/// Agent process configuration — the ACP-speaking AI agent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
}

/// Channel subprocess configuration — user-facing interfaces.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// MCP tool server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
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
    fn agent_config_with_env_deserializes() {
        let toml = r#"
            binary = "opencode"
            args = ["acp"]
            working_dir = "/workspace"

            [env]
            OPENCODE_ENABLE_QUESTION_TOOL = "1"
            MY_VAR = "hello"
        "#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.binary, "opencode");
        assert_eq!(config.env.len(), 2);
        assert_eq!(config.env["OPENCODE_ENABLE_QUESTION_TOOL"], "1");
        assert_eq!(config.working_dir, Some(PathBuf::from("/workspace")));
    }

    #[test]
    fn agent_config_env_defaults_empty() {
        let toml = r#"
            binary = "opencode"
        "#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
        assert!(config.env.is_empty());
    }

    #[test]
    fn agent_config_working_dir_optional() {
        let toml = r#"
            binary = "opencode"
        "#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
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
}
