use std::path::{Path, PathBuf};

use anyclaw_config::WorkspaceConfig;
use anyclaw_jsonrpc::types::{JsonRpcRequest, JsonRpcResponse};

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

    pub(crate) async fn send_success_response(
        slot: &AgentSlot,
        request: &JsonRpcRequest,
        result: serde_json::Value,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = JsonRpcResponse::success(request.id.clone(), result);
            let _ = conn.send_raw(resp).await;
        }
    }

    pub(crate) async fn send_error_response(
        slot: &AgentSlot,
        request: &JsonRpcRequest,
        code: i64,
        message: &str,
    ) {
        if let Some(conn) = slot.connection.as_ref() {
            let resp = JsonRpcResponse::error(
                request.id.clone(),
                anyclaw_jsonrpc::types::JsonRpcError {
                    code,
                    message: message.to_string(),
                    data: None,
                },
            );
            let _ = conn.send_raw(resp).await;
        }
    }
}
