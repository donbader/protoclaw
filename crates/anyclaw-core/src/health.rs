use serde::{Deserialize, Serialize};

/// Overall system health status reported by the admin endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All managers and agents are connected and operating normally.
    Healthy,
    /// At least one agent is disconnected or a manager is in a degraded state.
    Degraded,
}

/// Health information for a single agent subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    /// Agent name (matches the key in `agents_manager.agents` config).
    pub name: String,
    /// Whether the agent subprocess is currently connected.
    pub connected: bool,
    /// Number of active ACP sessions on this agent.
    pub session_count: usize,
}

/// Point-in-time health snapshot used by the admin `/health` endpoint.
///
/// Field layout matches the JSON schema that `status.rs` parses, so the CLI
/// command can deserialise the response without any extra mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Aggregate health status across all managers.
    pub status: HealthStatus,
    /// Per-agent health details.
    pub agents: Vec<AgentHealth>,
    /// Running channel names.
    pub channels: Vec<String>,
    /// Running MCP server names (top-level for CLI compatibility).
    pub mcp_servers: Vec<String>,
}

impl Default for HealthSnapshot {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            agents: Vec::new(),
            channels: Vec::new(),
            mcp_servers: Vec::new(),
        }
    }
}
