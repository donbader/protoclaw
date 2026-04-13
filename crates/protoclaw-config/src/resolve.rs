/// Legacy flat-name aliases mapping old `@built-in/<name>` paths to new
/// `@built-in/{category}/<name>` canonical paths.
const LEGACY_ALIASES: &[(&str, &str)] = &[
    ("mock-agent", "agents/mock-agent"),
    ("telegram-channel", "channels/telegram"),
    ("debug-http", "channels/debug-http"),
    ("system-info", "tools/system-info"),
    ("opencode", "agents/opencode"),
    // Categorized but renamed — old binary names kept as aliases
    ("agents/opencode", "agents/acp-bridge"),
    ("agents/opencode-wrapper", "agents/acp-bridge"),
    ("acp", "agents/acp-bridge"),
];

/// Resolve a binary path, expanding `@built-in/` prefix against extensions_dir.
///
/// Canonical form: `@built-in/{agents,channels,tools}/<name>` resolves to
/// `extensions_dir/{agents,channels,tools}/<name>`.
///
/// Legacy flat paths (e.g. `@built-in/mock-agent`) are resolved via built-in
/// aliases with a deprecation warning logged via `tracing::warn!`.
///
/// Absolute paths and relative names pass through unchanged.
pub fn resolve_binary_path(binary: &str, extensions_dir: &str) -> String {
    if let Some(name) = binary.strip_prefix("@built-in/") {
        if name.starts_with("agents/")
            || name.starts_with("channels/")
            || name.starts_with("tools/")
        {
            if let Some((_, canonical)) = LEGACY_ALIASES.iter().find(|(old, _)| *old == name) {
                tracing::warn!(
                    "deprecated @built-in/ path '{}', use '@built-in/{}' instead",
                    name,
                    canonical
                );
                return format!("{}/{}", extensions_dir, canonical);
            }
            return format!("{}/{}", extensions_dir, name);
        }

        if let Some((_, canonical)) = LEGACY_ALIASES.iter().find(|(old, _)| *old == name) {
            tracing::warn!(
                "deprecated @built-in/ path '{}', use '@built-in/{}' instead",
                name,
                canonical
            );
            return format!("{}/{}", extensions_dir, canonical);
        }

        format!("{}/{}", extensions_dir, name)
    } else {
        binary.to_string()
    }
}

pub fn resolve_all_binary_paths(config: &mut crate::ProtoclawConfig) {
    let ext = config.extensions_dir.clone();
    for agent in config.agents_manager.agents.values_mut() {
        match &mut agent.workspace {
            crate::WorkspaceConfig::Local(local) => {
                local.binary.0[0] = resolve_binary_path(&local.binary.0[0], &ext);
            }
            crate::WorkspaceConfig::Docker(docker) => {
                if let Some(ref mut ep) = docker.entrypoint {
                    ep.0[0] = resolve_binary_path(&ep.0[0], &ext);
                }
            }
        }
    }
    for ch in config.channels_manager.channels.values_mut() {
        ch.binary = resolve_binary_path(&ch.binary, &ext);
    }
    for tool in config.tools_manager.tools.values_mut() {
        if let Some(ref mut bin) = tool.binary {
            *bin = resolve_binary_path(bin, &ext);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::agent("@built-in/agents/mock-agent", "/usr/local/bin/agents/mock-agent")]
    #[case::channel("@built-in/channels/telegram", "/usr/local/bin/channels/telegram")]
    #[case::tool("@built-in/tools/system-info", "/usr/local/bin/tools/system-info")]
    fn when_binary_has_categorized_built_in_prefix_then_resolves_with_category(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(resolve_binary_path(input, "/usr/local/bin"), expected);
    }

    #[rstest]
    #[case::flat_agent("@built-in/mock-agent", "/usr/local/bin/agents/mock-agent")]
    #[case::flat_channel("@built-in/telegram-channel", "/usr/local/bin/channels/telegram")]
    #[case::flat_debug_http("@built-in/debug-http", "/usr/local/bin/channels/debug-http")]
    #[case::flat_tool("@built-in/system-info", "/usr/local/bin/tools/system-info")]
    #[case::flat_opencode("@built-in/opencode", "/usr/local/bin/agents/opencode")]
    fn when_binary_has_legacy_flat_prefix_then_resolves_via_alias(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(resolve_binary_path(input, "/usr/local/bin"), expected);
    }

    #[rstest]
    fn when_binary_has_categorized_legacy_alias_then_resolves_via_alias() {
        assert_eq!(
            resolve_binary_path("@built-in/agents/opencode", "/usr/local/bin"),
            "/usr/local/bin/agents/acp-bridge"
        );
    }

    #[rstest]
    fn when_binary_has_opencode_wrapper_alias_then_resolves_to_acp_bridge() {
        assert_eq!(
            resolve_binary_path("@built-in/agents/opencode-wrapper", "/usr/local/bin"),
            "/usr/local/bin/agents/acp-bridge"
        );
    }

    #[rstest]
    fn when_binary_has_flat_acp_alias_then_resolves_to_acp_bridge() {
        assert_eq!(
            resolve_binary_path("@built-in/acp", "/usr/local/bin"),
            "/usr/local/bin/agents/acp-bridge"
        );
    }

    #[rstest]
    fn when_binary_has_unknown_flat_prefix_then_resolves_directly() {
        assert_eq!(
            resolve_binary_path("@built-in/unknown-thing", "/usr/local/bin"),
            "/usr/local/bin/unknown-thing"
        );
    }

    #[test]
    fn when_binary_is_absolute_path_then_returned_unchanged() {
        assert_eq!(
            resolve_binary_path("/absolute/path/agent", "/usr/local/bin"),
            "/absolute/path/agent"
        );
    }

    #[test]
    fn when_binary_is_relative_name_then_returned_unchanged() {
        assert_eq!(
            resolve_binary_path("relative-binary", "/usr/local/bin"),
            "relative-binary"
        );
    }

    #[test]
    fn when_resolving_local_workspace_binary_then_built_in_prefix_expanded() {
        use crate::{
            DockerWorkspaceConfig, LocalWorkspaceConfig, PullPolicy, StringOrArray, WorkspaceConfig,
        };

        let mut local = LocalWorkspaceConfig {
            binary: StringOrArray(vec!["@built-in/agents/mock-agent".to_string()]),
            working_dir: None,
            env: std::collections::HashMap::new(),
        };
        local.binary.0[0] = resolve_binary_path(&local.binary.0[0], "/usr/local/bin");
        assert_eq!(
            local.binary,
            StringOrArray(vec!["/usr/local/bin/agents/mock-agent".into()])
        );

        let docker = DockerWorkspaceConfig {
            image: "my-agent:latest".to_string(),
            entrypoint: None,
            volumes: vec![],
            env: std::collections::HashMap::new(),
            memory_limit: None,
            cpu_limit: None,
            docker_host: None,
            network: None,
            pull_policy: PullPolicy::default(),
            working_dir: None,
        };
        assert_eq!(docker.image, "my-agent:latest");

        let workspace = WorkspaceConfig::Local(LocalWorkspaceConfig {
            binary: StringOrArray(vec!["@built-in/tools/other".to_string()]),
            working_dir: None,
            env: std::collections::HashMap::new(),
        });
        assert!(matches!(workspace, WorkspaceConfig::Local(_)));
    }

    #[rstest]
    fn when_resolve_all_called_then_docker_entrypoint_resolved() {
        use crate::{
            AgentConfig, AgentsManagerConfig, ChannelsManagerConfig, DockerWorkspaceConfig,
            ProtoclawConfig, PullPolicy, StringOrArray, SupervisorConfig, ToolsManagerConfig,
            WorkspaceConfig,
        };
        use std::collections::HashMap;

        let mut config = ProtoclawConfig {
            log_level: "info".into(),
            log_format: crate::LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                acp_timeout_secs: 30,
                shutdown_grace_ms: 5000,
                agents: HashMap::from([(
                    "test-agent".into(),
                    AgentConfig {
                        workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                            image: "my-agent:latest".into(),
                            entrypoint: Some(StringOrArray(vec![
                                "@built-in/agents/opencode-wrapper".into(),
                            ])),
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
                        backoff: None,
                        crash_tracker: None,
                        acp_timeout_secs: None,
                        options: HashMap::new(),
                    },
                )]),
            },
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
        };

        resolve_all_binary_paths(&mut config);

        let agent = &config.agents_manager.agents["test-agent"];
        match &agent.workspace {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(
                    d.entrypoint.as_ref().map(|ep| ep.0[0].as_str()),
                    Some("/usr/local/bin/agents/acp-bridge")
                );
            }
            _ => panic!("expected Docker workspace"),
        }
    }
}
