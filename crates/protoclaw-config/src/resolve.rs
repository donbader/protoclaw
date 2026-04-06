/// Resolve a binary path, expanding `@built-in/` prefix against extensions_dir.
///
/// - `@built-in/mock-agent` with extensions_dir `/usr/local/bin` → `/usr/local/bin/mock-agent`
/// - Absolute paths and relative names pass through unchanged.
pub fn resolve_binary_path(binary: &str, extensions_dir: &str) -> String {
    if let Some(name) = binary.strip_prefix("@built-in/") {
        format!("{}/{}", extensions_dir, name)
    } else {
        binary.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_binary_has_built_in_prefix_then_resolves_to_extensions_dir() {
        assert_eq!(
            resolve_binary_path("@built-in/mock-agent", "/usr/local/bin"),
            "/usr/local/bin/mock-agent"
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
            binary: "@built-in/mock-agent".to_string(),
            working_dir: None,
            env: std::collections::HashMap::new(),
        };
        local.binary = resolve_binary_path(&local.binary, "/usr/local/bin");
        assert_eq!(local.binary, "/usr/local/bin/mock-agent");

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
            binary: "@built-in/other".to_string(),
            working_dir: None,
            env: std::collections::HashMap::new(),
        });
        assert!(matches!(workspace, WorkspaceConfig::Local(_)));
    }
}
