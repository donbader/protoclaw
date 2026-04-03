use std::collections::HashMap;

pub fn mock_agent_path() -> String {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("target");
    path.push("debug");
    path.push("mock-agent");
    path.to_string_lossy().to_string()
}

pub fn debug_http_path() -> String {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("target");
    path.push("debug");
    path.push("debug-http");
    path.to_string_lossy().to_string()
}

pub fn mock_agent_config() -> protoclaw_config::ProtoclawConfig {
    mock_agent_config_with_env(HashMap::new())
}

pub fn mock_agent_config_with_env(env: HashMap<String, String>) -> protoclaw_config::ProtoclawConfig {
    protoclaw_config::ProtoclawConfig {
        agent: None,
        agents: vec![protoclaw_config::AgentConfig {
            name: "default".into(),
            binary: mock_agent_path(),
            args: vec![],
            enabled: true,
            env,
            working_dir: None,
            tools: vec![],
        }],
        channels: vec![protoclaw_config::ChannelConfig {
            name: "debug-http".into(),
            binary: debug_http_path(),
            args: vec![],
            enabled: true,
            agent: None,
        }],
        mcp_servers: vec![],
        wasm_tools: vec![],
        supervisor: protoclaw_config::SupervisorConfig {
            shutdown_timeout_secs: 5,
            health_check_interval_secs: 1,
            max_restarts: 3,
            restart_window_secs: 60,
        },
        log_level: "info".into(),
        extensions_dir: "/usr/local/bin".into(),
    }
}

pub async fn wait_for_port(mut port_rx: tokio::sync::watch::Receiver<u16>, timeout_ms: u64) -> Option<u16> {
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(timeout_ms) {
        let port = *port_rx.borrow();
        if port != 0 {
            return Some(port);
        }
        if port_rx.changed().await.is_err() {
            return None;
        }
    }
    None
}
