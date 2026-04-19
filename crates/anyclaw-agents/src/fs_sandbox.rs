use std::path::{Path, PathBuf};

use anyclaw_config::WorkspaceConfig;
use anyclaw_jsonrpc::types::JsonRpcRequest;

use crate::manager::AgentsManager;
use crate::slot::AgentSlot;

/// Resolve the effective working directory for an agent from its workspace config.
pub(crate) fn resolve_agent_cwd(workspace: &WorkspaceConfig) -> PathBuf {
    match workspace {
        WorkspaceConfig::Local(local) => local
            .working_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
        WorkspaceConfig::Docker(docker) => docker
            .working_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
    }
}

/// Validate that `requested` resolves to a path inside `sandbox_root`.
/// Uses `canonicalize()` so symlinks cannot escape the sandbox.
/// Returns the canonical resolved path on success.
pub(crate) fn validate_fs_path(sandbox_root: &Path, requested: &str) -> Result<PathBuf, String> {
    let requested_path = Path::new(requested);
    let resolved = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        sandbox_root.join(requested_path)
    };
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("path resolution failed: {e}"))?;
    let canonical_root = sandbox_root
        .canonicalize()
        .map_err(|e| format!("sandbox root resolution failed: {e}"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err("path outside allowed directory".into());
    }
    Ok(canonical)
}

/// Validate that `requested` resolves to a path whose *parent directory* is inside `sandbox_root`.
/// Used for writes where the file may not yet exist.
/// Returns the validated write path (canonical parent + filename) on success.
pub(crate) fn validate_fs_write_path(
    sandbox_root: &Path,
    requested: &str,
) -> Result<PathBuf, String> {
    let requested_path = Path::new(requested);
    let resolved = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        sandbox_root.join(requested_path)
    };
    let parent = resolved
        .parent()
        .ok_or_else(|| "invalid path: no parent directory".to_string())?;
    let filename = resolved
        .file_name()
        .ok_or_else(|| "invalid path: no filename".to_string())?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("parent directory resolution failed: {e}"))?;
    let canonical_root = sandbox_root
        .canonicalize()
        .map_err(|e| format!("sandbox root resolution failed: {e}"))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err("path outside allowed directory".into());
    }
    Ok(canonical_parent.join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_config::{DockerWorkspaceConfig, LocalWorkspaceConfig, StringOrArray};
    use rstest::rstest;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn given_local_workspace(working_dir: Option<PathBuf>) -> WorkspaceConfig {
        WorkspaceConfig::Local(LocalWorkspaceConfig {
            binary: StringOrArray::from("test-agent"),
            working_dir,
            env: HashMap::new(),
        })
    }

    fn given_docker_workspace(working_dir: Option<PathBuf>) -> WorkspaceConfig {
        WorkspaceConfig::Docker(DockerWorkspaceConfig {
            image: "test-image:latest".into(),
            entrypoint: None,
            volumes: vec![],
            env: HashMap::new(),
            memory_limit: None,
            cpu_limit: None,
            docker_host: None,
            network: None,
            pull_policy: Default::default(),
            working_dir,
            extra_hosts: vec![],
        })
    }

    #[rstest]
    fn when_local_workspace_has_working_dir_then_resolves_to_it() {
        let ws = given_local_workspace(Some(PathBuf::from("/tmp/agent")));
        assert_eq!(resolve_agent_cwd(&ws), PathBuf::from("/tmp/agent"));
    }

    #[rstest]
    fn when_local_workspace_has_no_working_dir_then_resolves_to_current_dir() {
        let ws = given_local_workspace(None);
        let cwd = resolve_agent_cwd(&ws);
        assert!(!cwd.as_os_str().is_empty());
    }

    #[rstest]
    fn when_docker_workspace_has_working_dir_then_resolves_to_it() {
        let ws = given_docker_workspace(Some(PathBuf::from("/workspace")));
        assert_eq!(resolve_agent_cwd(&ws), PathBuf::from("/workspace"));
    }

    #[rstest]
    fn when_docker_workspace_has_no_working_dir_then_resolves_to_current_dir() {
        let ws = given_docker_workspace(None);
        let cwd = resolve_agent_cwd(&ws);
        assert!(!cwd.as_os_str().is_empty());
    }

    #[rstest]
    fn when_absolute_path_inside_sandbox_then_validate_fs_path_returns_ok() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "").unwrap();
        let result = validate_fs_path(dir.path(), file.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[rstest]
    fn when_relative_path_inside_sandbox_then_validate_fs_path_returns_ok() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("rel.txt"), "").unwrap();
        let result = validate_fs_path(dir.path(), "rel.txt");
        assert!(result.is_ok());
    }

    #[rstest]
    fn when_path_traverses_outside_sandbox_then_validate_fs_path_returns_err() {
        let dir = TempDir::new().unwrap();
        let result = validate_fs_path(dir.path(), "../outside.txt");
        assert!(result.is_err());
    }

    #[rstest]
    fn when_path_does_not_exist_then_validate_fs_path_returns_err() {
        let dir = TempDir::new().unwrap();
        let result = validate_fs_path(dir.path(), "nonexistent.txt");
        assert!(result.is_err());
    }

    #[rstest]
    fn when_write_path_parent_inside_sandbox_then_validate_fs_write_path_returns_ok() {
        let dir = TempDir::new().unwrap();
        let result = validate_fs_write_path(dir.path(), "newfile.txt");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            dir.path().canonicalize().unwrap().join("newfile.txt")
        );
    }

    #[rstest]
    fn when_write_path_parent_outside_sandbox_then_validate_fs_write_path_returns_err() {
        let dir = TempDir::new().unwrap();
        let result = validate_fs_write_path(dir.path(), "../outside/file.txt");
        assert!(result.is_err());
    }

    #[rstest]
    fn when_write_path_parent_does_not_exist_then_validate_fs_write_path_returns_err() {
        let dir = TempDir::new().unwrap();
        let result = validate_fs_write_path(dir.path(), "nonexistent_subdir/file.txt");
        assert!(result.is_err());
    }
}

impl AgentsManager {
    pub(crate) async fn handle_fs_read(slot: &AgentSlot, request: &JsonRpcRequest) {
        // D-03: fs request params have agent-defined path/content fields
        let params = request.params.as_ref();
        let path = params.and_then(|p| p["path"].as_str()).unwrap_or("");
        let sandbox_root = resolve_agent_cwd(&slot.config.workspace);
        let resolved = match validate_fs_path(&sandbox_root, path) {
            Ok(p) => p,
            Err(msg) => {
                Self::send_error_response(slot, request, -32000, &msg).await;
                return;
            }
        };
        match tokio::fs::read_to_string(&resolved).await {
            Ok(content) => {
                Self::send_success_response(
                    slot,
                    request,
                    serde_json::json!({ "content": content }),
                )
                .await;
            }
            Err(e) => {
                Self::send_error_response(slot, request, -32000, &e.to_string()).await;
            }
        }
    }

    pub(crate) async fn handle_fs_write(slot: &AgentSlot, request: &JsonRpcRequest) {
        // D-03: fs request params have agent-defined path/content fields
        let params = request.params.as_ref();
        let path = params.and_then(|p| p["path"].as_str()).unwrap_or("");
        let content = params.and_then(|p| p["content"].as_str()).unwrap_or("");
        let sandbox_root = resolve_agent_cwd(&slot.config.workspace);
        let resolved = match validate_fs_write_path(&sandbox_root, path) {
            Ok(p) => p,
            Err(msg) => {
                Self::send_error_response(slot, request, -32000, &msg).await;
                return;
            }
        };
        match tokio::fs::write(&resolved, content).await {
            Ok(()) => {
                Self::send_success_response(slot, request, serde_json::json!({})).await;
            }
            Err(e) => {
                Self::send_error_response(slot, request, -32000, &e.to_string()).await;
            }
        }
    }

    // D-03: FS response result is agent-defined JSON — typed at the agent boundary, not here
    #[allow(clippy::disallowed_types)]
    pub(crate) async fn send_success_response(
        _slot: &AgentSlot,
        request: &JsonRpcRequest,
        _result: serde_json::Value,
    ) {
        tracing::debug!(method = %request.method, "send_success_response called on legacy JSON-RPC path (no-op)");
    }

    pub(crate) async fn send_error_response(
        _slot: &AgentSlot,
        request: &JsonRpcRequest,
        code: i64,
        message: &str,
    ) {
        tracing::debug!(method = %request.method, code, message, "send_error_response called on legacy JSON-RPC path (no-op)");
    }
}
