use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub name: String,
    pub connected: bool,
    pub session_count: usize,
}

/// Point-in-time health snapshot used by the admin `/health` endpoint.
///
/// Field layout matches the JSON schema that `status.rs` parses, so the CLI
/// command can deserialise the response without any extra mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub status: HealthStatus,
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
