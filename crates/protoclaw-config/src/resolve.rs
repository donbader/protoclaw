/// Legacy flat-name aliases mapping old `@built-in/<name>` paths to new
/// `@built-in/{category}/<name>` canonical paths.
const LEGACY_ALIASES: &[(&str, &str)] = &[
    ("mock-agent", "agents/mock-agent"),
    ("telegram-channel", "channels/telegram"),
    ("debug-http", "channels/debug-http"),
    ("system-info", "tools/system-info"),
    ("opencode", "agents/opencode"),
    // Categorized but renamed — old binary name kept as alias
    ("agents/opencode", "agents/opencode-wrapper"),
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
            "/usr/local/bin/agents/opencode-wrapper"
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
        use crate::{DockerWorkspaceConfig, LocalWorkspaceConfig, PullPolicy, WorkspaceConfig};

        let mut local = LocalWorkspaceConfig {
            binary: "@built-in/agents/mock-agent".to_string(),
            working_dir: None,
            env: std::collections::HashMap::new(),
        };
        local.binary = resolve_binary_path(&local.binary, "/usr/local/bin");
        assert_eq!(local.binary, "/usr/local/bin/agents/mock-agent");

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
        };
        assert_eq!(docker.image, "my-agent:latest");

        let workspace = WorkspaceConfig::Local(LocalWorkspaceConfig {
            binary: "@built-in/tools/other".to_string(),
            working_dir: None,
            env: std::collections::HashMap::new(),
        });
        assert!(matches!(workspace, WorkspaceConfig::Local(_)));
    }
}
