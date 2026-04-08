use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use protoclaw_core::{AgentsCommand, ManagerHandle};
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::error::ChannelsError;

#[derive(Clone)]
struct AppState {
    agents_handle: ManagerHandle<AgentsCommand>,
    channel_names: Vec<String>,
    mcp_server_names: Vec<String>,
}

pub struct DebugHttpChannel {
    port: u16,
    agents_handle: ManagerHandle<AgentsCommand>,
    port_tx: watch::Sender<u16>,
    channel_names: Vec<String>,
    mcp_server_names: Vec<String>,
}

impl DebugHttpChannel {
    pub fn new(port: u16, agents_handle: ManagerHandle<AgentsCommand>) -> Self {
        let (port_tx, _) = watch::channel(0u16);
        Self {
            port,
            agents_handle,
            port_tx,
            channel_names: Vec::new(),
            mcp_server_names: Vec::new(),
        }
    }

    pub fn with_port_tx(mut self, port_tx: watch::Sender<u16>) -> Self {
        self.port_tx = port_tx;
        self
    }

    pub fn with_names(
        mut self,
        channel_names: Vec<String>,
        mcp_server_names: Vec<String>,
    ) -> Self {
        self.channel_names = channel_names;
        self.mcp_server_names = mcp_server_names;
        self
    }

    pub fn port_rx(&self) -> watch::Receiver<u16> {
        self.port_tx.subscribe()
    }

    pub async fn run(self, cancel: CancellationToken) -> Result<(), ChannelsError> {
        let state = AppState {
            agents_handle: self.agents_handle.clone(),
            channel_names: self.channel_names.clone(),
            mcp_server_names: self.mcp_server_names.clone(),
        };

        let app = Router::new()
            .route("/health", get(handle_health))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", self.port)).await?;
        let addr = listener.local_addr()?;
        let _ = self.port_tx.send(addr.port());

        tracing::info!(port = addr.port(), "debug-http listening");

        axum::serve(listener, app)
            .with_graceful_shutdown(async move { cancel.cancelled().await })
            .await?;

        Ok(())
    }
}

async fn handle_health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (reply_tx, reply_rx) = oneshot::channel();
    let agent_status = if state
        .agents_handle
        .send(AgentsCommand::GetStatus { reply: reply_tx })
        .await
        .is_ok()
    {
        reply_rx.await.ok()
    } else {
        None
    };

    let agent_json = match agent_status {
        Some(statuses) => {
            let agents: Vec<_> = statuses.iter().map(|s| serde_json::json!({
                "name": s.name,
                "connected": s.connected,
                "session_count": s.session_count,
            })).collect();
            serde_json::json!(agents)
        }
        None => serde_json::json!([]),
    };

    Json(serde_json::json!({
        "status": "ok",
        "agent": agent_json,
        "channels": state.channel_names,
        "mcp_servers": state.mcp_server_names,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use protoclaw_core::{AgentStatusInfo, AgentsCommand};
    use protoclaw_core::ManagerHandle;

    #[test]
    fn when_debug_http_channel_created_then_instance_initialized() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<AgentsCommand>(16);
        let handle = ManagerHandle::new(tx);
        let ch = DebugHttpChannel::new(0, handle);
        assert_eq!(ch.port, 0);
        assert!(ch.channel_names.is_empty());
        assert!(ch.mcp_server_names.is_empty());
    }

    async fn spawn_test_server() -> (
        u16,
        CancellationToken,
        tokio::sync::mpsc::Receiver<AgentsCommand>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel::<AgentsCommand>(16);
        let handle = ManagerHandle::new(tx);
        let (port_tx, mut port_rx) = watch::channel(0u16);

        let state = AppState {
            agents_handle: handle,
            channel_names: vec!["debug-http".to_string()],
            mcp_server_names: vec![],
        };

        let app = Router::new()
            .route("/health", get(handle_health))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _ = port_tx.send(addr.port());

        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { cancel_clone.cancelled().await })
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = port_rx.changed().await;

        (addr.port(), cancel, rx)
    }

    #[tokio::test]
    async fn when_debug_http_health_endpoint_called_then_response_has_required_keys() {
        let (port, cancel, mut agents_rx) = spawn_test_server().await;

        tokio::spawn(async move {
            while let Some(cmd) = agents_rx.recv().await {
                if let AgentsCommand::GetStatus { reply } = cmd {
                    let _ = reply.send(vec![AgentStatusInfo {
                        name: "default".to_string(),
                        connected: false,
                        session_count: 0,
                    }]);
                }
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body.get("status").is_some(), "missing 'status' key");
        assert!(body.get("agent").is_some(), "missing 'agent' key");
        assert!(body.get("channels").is_some(), "missing 'channels' key");
        assert!(body.get("mcp_servers").is_some(), "missing 'mcp_servers' key");

        cancel.cancel();
    }

    #[tokio::test]
    async fn when_debug_http_health_endpoint_called_then_agent_has_connected_field() {
        let (port, cancel, mut agents_rx) = spawn_test_server().await;

        tokio::spawn(async move {
            while let Some(cmd) = agents_rx.recv().await {
                if let AgentsCommand::GetStatus { reply } = cmd {
                    let _ = reply.send(vec![AgentStatusInfo {
                        name: "default".to_string(),
                        connected: false,
                        session_count: 0,
                    }]);
                }
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = resp.json().await.unwrap();
        let agent = &body["agent"];
        assert!(agent.is_array(), "agent should be an array");

        cancel.cancel();
    }

    #[tokio::test]
    async fn when_debug_http_health_endpoint_called_then_channels_field_is_array() {
        let (port, cancel, mut agents_rx) = spawn_test_server().await;

        tokio::spawn(async move {
            while let Some(cmd) = agents_rx.recv().await {
                if let AgentsCommand::GetStatus { reply } = cmd {
                    let _ = reply.send(vec![AgentStatusInfo {
                        name: "default".to_string(),
                        connected: false,
                        session_count: 0,
                    }]);
                }
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .unwrap();

        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(
            body["channels"].is_array(),
            "'channels' should be an array"
        );

        cancel.cancel();
    }
}
