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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    fn when_health_status_healthy_serialized_then_lowercase_string() {
        let val = serde_json::to_value(&HealthStatus::Healthy).unwrap();
        assert_eq!(val, json!("healthy"));
    }

    #[rstest]
    fn when_health_status_degraded_serialized_then_lowercase_string() {
        let val = serde_json::to_value(&HealthStatus::Degraded).unwrap();
        assert_eq!(val, json!("degraded"));
    }

    #[rstest]
    #[case::healthy(HealthStatus::Healthy)]
    #[case::degraded(HealthStatus::Degraded)]
    fn when_health_status_round_trips_then_identical(#[case] status: HealthStatus) {
        let json = serde_json::to_string(&status).unwrap();
        let restored: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, restored);
    }

    #[rstest]
    fn when_agent_health_round_trips_then_identical() {
        let original = AgentHealth {
            name: "test-agent".into(),
            connected: true,
            session_count: 3,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AgentHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(original.name, restored.name);
        assert_eq!(original.connected, restored.connected);
        assert_eq!(original.session_count, restored.session_count);
    }

    #[rstest]
    fn when_health_snapshot_default_then_healthy_with_empty_vecs() {
        let snap = HealthSnapshot::default();
        assert_eq!(snap.status, HealthStatus::Healthy);
        assert!(snap.agents.is_empty());
        assert!(snap.channels.is_empty());
        assert!(snap.mcp_servers.is_empty());
    }

    #[rstest]
    fn when_health_snapshot_populated_round_trips_then_identical() {
        let original = HealthSnapshot {
            status: HealthStatus::Degraded,
            agents: vec![
                AgentHealth {
                    name: "agent-1".into(),
                    connected: true,
                    session_count: 2,
                },
                AgentHealth {
                    name: "agent-2".into(),
                    connected: false,
                    session_count: 0,
                },
            ],
            channels: vec!["telegram".into(), "debug-http".into()],
            mcp_servers: vec!["tools-server".into()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: HealthSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(original.status, restored.status);
        assert_eq!(original.agents.len(), restored.agents.len());
        assert_eq!(original.channels, restored.channels);
        assert_eq!(original.mcp_servers, restored.mcp_servers);
    }
}
