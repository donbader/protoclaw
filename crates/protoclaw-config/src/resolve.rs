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

    #[test]
    fn resolves_built_in_prefix() {
        assert_eq!(
            resolve_binary_path("@built-in/mock-agent", "/usr/local/bin"),
            "/usr/local/bin/mock-agent"
        );
    }

    #[test]
    fn absolute_path_unchanged() {
        assert_eq!(
            resolve_binary_path("/absolute/path/agent", "/usr/local/bin"),
            "/absolute/path/agent"
        );
    }

    #[test]
    fn relative_path_unchanged() {
        assert_eq!(
            resolve_binary_path("relative-binary", "/usr/local/bin"),
            "relative-binary"
        );
    }

    #[test]
    fn resolve_only_applies_to_local_workspace() {
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
