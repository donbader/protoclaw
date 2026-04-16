use std::collections::HashMap;

// Grandfathered: extension defaults are arbitrary key-value maps (same as entity options fields)
#[allow(clippy::disallowed_types)]
fn merge_sidecar_into_options(binary_path: &str, options: &mut HashMap<String, serde_json::Value>) {
    let sidecar_path = format!("{binary_path}.defaults.yaml");
    let path = std::path::Path::new(&sidecar_path);

    if !path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %sidecar_path, error = %e, "failed to read extension defaults");
            return;
        }
    };

    #[allow(clippy::disallowed_types)]
    let map: HashMap<String, serde_json::Value> = match serde_yaml::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(path = %sidecar_path, error = %e, "skipping malformed extension defaults");
            return;
        }
    };

    tracing::trace!(
        path = %sidecar_path,
        keys = ?map.keys().collect::<Vec<_>>(),
        "loaded extension defaults"
    );

    for (k, v) in map {
        options.entry(k).or_insert(v);
    }
}

/// Load per-extension sidecar defaults and merge into entity options.
///
/// For each agent/channel/tool with a resolved binary path, checks for
/// `<binary_path>.defaults.yaml`. If found, parses it as a flat YAML map
/// and merges into the entity's `options` HashMap. User-provided options
/// take precedence over extension defaults.
///
/// Called after [`resolve_all_binary_paths()`](crate::resolve_all_binary_paths)
/// in `Supervisor::new()`.
pub fn load_extension_defaults(config: &mut crate::AnyclawConfig) {
    for agent in config.agents_manager.agents.values_mut() {
        if let crate::WorkspaceConfig::Local(ref local) = agent.workspace {
            let binary_path = local.binary.0[0].clone();
            merge_sidecar_into_options(&binary_path, &mut agent.options);
        }
    }

    for ch in config.channels_manager.channels.values_mut() {
        let binary_path = ch.binary.clone();
        merge_sidecar_into_options(&binary_path, &mut ch.options);
    }

    for tool in config.tools_manager.tools.values_mut() {
        if let Some(ref bin) = tool.binary {
            let binary_path = bin.clone();
            merge_sidecar_into_options(&binary_path, &mut tool.options);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentConfig, AgentsManagerConfig, AnyclawConfig, ChannelConfig, ChannelsManagerConfig,
        DockerWorkspaceConfig, LocalWorkspaceConfig, PullPolicy, StringOrArray, SupervisorConfig,
        ToolConfig, ToolsManagerConfig, WorkspaceConfig,
    };
    use rstest::rstest;
    use tempfile::TempDir;

    fn config_with_local_agent(
        binary_path: &str,
        options: HashMap<String, serde_json::Value>,
    ) -> AnyclawConfig {
        AnyclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                acp_timeout_secs: 30,
                shutdown_grace_ms: 100,
                agents: HashMap::from([(
                    "test-agent".into(),
                    AgentConfig {
                        workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                            binary: StringOrArray(vec![binary_path.to_string()]),
                            working_dir: None,
                            env: HashMap::new(),
                        }),
                        enabled: true,
                        tools: vec![],
                        acp_timeout_secs: None,
                        backoff: None,
                        crash_tracker: None,
                        options,
                    },
                )]),
            },
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        }
    }

    fn setup_sidecar(sidecar_content: Option<&str>) -> (TempDir, String) {
        let dir = TempDir::new().expect("create temp dir");
        let binary_path = dir.path().join("my-extension");
        std::fs::write(&binary_path, "#!/bin/sh\n").expect("write fake binary");

        if let Some(content) = sidecar_content {
            let sidecar_path = dir.path().join("my-extension.defaults.yaml");
            std::fs::write(&sidecar_path, content).expect("write sidecar");
        }

        let path_str = binary_path.to_str().expect("valid utf8").to_string();
        (dir, path_str)
    }

    #[rstest]
    fn when_agent_has_sidecar_defaults_then_options_merged() {
        let (_dir, binary_path) = setup_sidecar(Some("key: value\n"));
        let mut config = config_with_local_agent(&binary_path, HashMap::new());

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        assert_eq!(agent.options["key"], serde_json::json!("value"));
    }

    #[rstest]
    fn when_sidecar_missing_then_options_unchanged() {
        let (_dir, binary_path) = setup_sidecar(None);
        let mut config = config_with_local_agent(&binary_path, HashMap::new());

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        assert!(agent.options.is_empty());
    }

    #[rstest]
    fn when_user_options_exist_then_user_wins_over_extension() {
        let (_dir, binary_path) = setup_sidecar(Some("key: ext_val\n"));
        let user_options = HashMap::from([("key".to_string(), serde_json::json!("user_val"))]);
        let mut config = config_with_local_agent(&binary_path, user_options);

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        assert_eq!(agent.options["key"], serde_json::json!("user_val"));
    }

    #[rstest]
    fn when_agent_is_docker_workspace_then_skipped() {
        let mut config = AnyclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                acp_timeout_secs: 30,
                shutdown_grace_ms: 100,
                agents: HashMap::from([(
                    "docker-agent".into(),
                    AgentConfig {
                        workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                            image: "my-agent:latest".into(),
                            entrypoint: None,
                            volumes: vec![],
                            env: HashMap::new(),
                            memory_limit: None,
                            cpu_limit: None,
                            docker_host: None,
                            network: None,
                            pull_policy: PullPolicy::default(),
                            working_dir: None,
                        }),
                        enabled: true,
                        tools: vec![],
                        acp_timeout_secs: None,
                        backoff: None,
                        crash_tracker: None,
                        options: HashMap::new(),
                    },
                )]),
            },
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        };

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["docker-agent"];
        assert!(agent.options.is_empty());
    }

    #[rstest]
    fn when_channel_has_sidecar_then_options_merged() {
        let (_dir, binary_path) = setup_sidecar(Some("debounce_ms: 500\n"));
        let mut config = AnyclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig::default(),
            channels_manager: ChannelsManagerConfig {
                init_timeout_secs: 10,
                exit_timeout_secs: 5,
                channels: HashMap::from([(
                    "test-channel".into(),
                    ChannelConfig {
                        binary: binary_path,
                        args: vec![],
                        enabled: true,
                        agent: "default".into(),
                        init_timeout_secs: None,
                        exit_timeout_secs: None,
                        backoff: None,
                        crash_tracker: None,
                        options: HashMap::new(),
                    },
                )]),
            },
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        };

        load_extension_defaults(&mut config);

        let ch = &config.channels_manager.channels["test-channel"];
        assert_eq!(ch.options["debounce_ms"], serde_json::json!(500));
    }

    #[rstest]
    fn when_tool_has_sidecar_then_options_merged() {
        let (_dir, binary_path) = setup_sidecar(Some("timeout: 30\n"));
        let mut config = AnyclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig::default(),
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig {
                tools: HashMap::from([(
                    "test-tool".into(),
                    ToolConfig {
                        tool_type: crate::ToolType::Mcp,
                        binary: Some(binary_path),
                        args: vec![],
                        enabled: true,
                        module: None,
                        description: String::new(),
                        input_schema: None,
                        sandbox: Default::default(),
                        options: HashMap::new(),
                    },
                )]),
                tools_server_host: "127.0.0.1".into(),
            },
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        };

        load_extension_defaults(&mut config);

        let tool = &config.tools_manager.tools["test-tool"];
        assert_eq!(tool.options["timeout"], serde_json::json!(30));
    }

    #[rstest]
    fn when_tool_has_no_binary_then_skipped() {
        let mut config = AnyclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig::default(),
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig {
                tools: HashMap::from([(
                    "wasm-tool".into(),
                    ToolConfig {
                        tool_type: crate::ToolType::Wasm,
                        binary: None,
                        args: vec![],
                        enabled: true,
                        module: Some("/path/to/tool.wasm".into()),
                        description: String::new(),
                        input_schema: None,
                        sandbox: Default::default(),
                        options: HashMap::new(),
                    },
                )]),
                tools_server_host: "127.0.0.1".into(),
            },
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        };

        load_extension_defaults(&mut config);

        let tool = &config.tools_manager.tools["wasm-tool"];
        assert!(tool.options.is_empty());
    }

    #[rstest]
    fn when_sidecar_yaml_is_malformed_then_skipped_with_warning() {
        let (_dir, binary_path) = setup_sidecar(Some(":::bad yaml{{{"));
        let mut config = config_with_local_agent(&binary_path, HashMap::new());

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        assert!(agent.options.is_empty());
    }

    #[rstest]
    fn when_extension_provides_new_key_and_user_provides_different_key_then_both_present() {
        let (_dir, binary_path) = setup_sidecar(Some("a: 1\n"));
        let user_options = HashMap::from([("b".to_string(), serde_json::json!(2))]);
        let mut config = config_with_local_agent(&binary_path, user_options);

        load_extension_defaults(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        assert_eq!(agent.options["a"], serde_json::json!(1));
        assert_eq!(agent.options["b"], serde_json::json!(2));
        assert_eq!(agent.options.len(), 2);
    }
}
