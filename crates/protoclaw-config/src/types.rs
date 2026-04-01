use serde::{Deserialize, Serialize};

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
    pub supervisor: SupervisorConfig,
}

/// Agent process configuration — the ACP-speaking AI agent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
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
