pub fn format_status_output(health: &serde_json::Value) -> String {
    let mut out = String::new();

    if let Some(agents) = health["agents"].as_array() {
        if agents.is_empty() {
            out.push_str("Agents:      (none)\n");
        } else {
            out.push_str("Agents:\n");
            for agent in agents {
                let name = agent["name"].as_str().unwrap_or("unknown");
                let connected = agent["connected"].as_bool().unwrap_or(false);
                let sessions = agent["session_count"].as_u64().unwrap_or(0);
                let status = if connected {
                    "connected"
                } else {
                    "disconnected"
                };
                out.push_str(&format!("  - {} ({}", name, status));
                if connected {
                    out.push_str(&format!(", {} sessions", sessions));
                }
                out.push_str(")\n");
            }
        }
    } else if let Some(agent) = health.get("agent") {
        let connected = agent["connected"].as_bool().unwrap_or(false);
        out.push_str(&format!(
            "Agent:       {}\n",
            if connected {
                "connected"
            } else {
                "disconnected"
            }
        ));
        if let Some(sid) = agent["session_id"].as_str() {
            out.push_str(&format!("  Session:   {}\n", sid));
        }
    }

    match health["channels"].as_array() {
        Some(chs) if !chs.is_empty() => {
            out.push_str("Channels:\n");
            for ch in chs {
                out.push_str(&format!("  - {}\n", ch.as_str().unwrap_or("unknown")));
            }
        }
        _ => out.push_str("Channels:    (none)\n"),
    }

    match health["mcp_servers"].as_array() {
        Some(ms) if !ms.is_empty() => {
            out.push_str("MCP Servers:\n");
            for m in ms {
                out.push_str(&format!("  - {}\n", m.as_str().unwrap_or("unknown")));
            }
        }
        _ => out.push_str("MCP Servers: (none)\n"),
    }

    out
}

pub async fn run_status(port: u16) -> anyhow::Result<()> {
    let url = format!("http://127.0.0.1:{}/health", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            anyclaw_core::constants::STATUS_HTTP_TIMEOUT_SECS,
        ))
        .build()?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Cannot reach anyclaw at {}: {}", url, e))?;

    let health: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Invalid response from {}: {}", url, e))?;

    print!("{}", format_status_output(&health));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn when_format_status_called_with_connected_agent_then_shows_agent_channel_and_mcp() {
        let health = json!({
            "status": "ok",
            "agent": { "connected": true, "session_id": "abc" },
            "channels": ["debug-http"],
            "mcp_servers": ["fs"]
        });
        let output = format_status_output(&health);
        assert!(output.contains("Agent:"), "should contain 'Agent:'");
        assert!(output.contains("connected"), "should contain 'connected'");
        assert!(output.contains("debug-http"), "should contain channel name");
        assert!(output.contains("fs"), "should contain MCP server name");
    }

    #[test]
    fn given_disconnected_agent_when_format_status_called_then_shows_disconnected() {
        let health = json!({
            "status": "ok",
            "agent": { "connected": false },
            "channels": [],
            "mcp_servers": []
        });
        let output = format_status_output(&health);
        assert!(
            output.contains("disconnected"),
            "should say 'disconnected' when not connected"
        );
    }

    #[test]
    fn given_empty_channels_when_format_status_called_then_shows_none() {
        let health = json!({
            "status": "ok",
            "agent": { "connected": false },
            "channels": [],
            "mcp_servers": ["fs"]
        });
        let output = format_status_output(&health);
        assert!(
            output.contains("(none)"),
            "empty channels should show '(none)'"
        );
    }

    #[test]
    fn given_empty_mcp_servers_when_format_status_called_then_shows_none() {
        let health = json!({
            "status": "ok",
            "agent": { "connected": false },
            "channels": ["debug-http"],
            "mcp_servers": []
        });
        let output = format_status_output(&health);
        assert!(
            output.contains("(none)"),
            "empty mcp_servers should show '(none)'"
        );
    }

    #[test]
    fn when_format_status_called_with_multiple_agents_then_shows_all_agents_and_session_counts() {
        let health = json!({
            "agents": [
                { "name": "opencode", "connected": true, "session_count": 3 },
                { "name": "claude-code", "connected": false, "session_count": 0 }
            ],
            "channels": ["debug-http"],
            "mcp_servers": ["fs"]
        });
        let output = format_status_output(&health);
        assert!(output.contains("Agents:"), "should contain 'Agents:'");
        assert!(output.contains("opencode"), "should contain agent name");
        assert!(output.contains("connected"), "should show connected status");
        assert!(output.contains("3 sessions"), "should show session count");
        assert!(
            output.contains("claude-code"),
            "should contain second agent"
        );
        assert!(output.contains("disconnected"), "should show disconnected");
    }

    #[test]
    fn given_empty_agents_array_when_format_status_called_then_shows_none() {
        let health = json!({
            "agents": [],
            "channels": [],
            "mcp_servers": []
        });
        let output = format_status_output(&health);
        assert!(
            output.contains("(none)"),
            "empty agents should show '(none)'"
        );
    }

    #[test]
    fn given_legacy_single_agent_format_when_format_status_called_then_still_renders_correctly() {
        let health = json!({
            "agent": { "connected": true, "session_id": "abc" },
            "channels": ["debug-http"],
            "mcp_servers": []
        });
        let output = format_status_output(&health);
        assert!(output.contains("Agent:"), "legacy format should still work");
        assert!(output.contains("connected"), "should show connected");
    }

    #[tokio::test]
    async fn when_run_status_called_with_unreachable_port_then_returns_error() {
        let result = run_status(19999).await;
        assert!(
            result.is_err(),
            "run_status with unreachable port should return Err"
        );
    }
}
