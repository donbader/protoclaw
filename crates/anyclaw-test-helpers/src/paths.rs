use std::path::PathBuf;

/// Return the workspace root directory (parent of `crates/`).
pub fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ directory")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Path to the built mock-agent binary in `ext/target/debug/`.
pub fn mock_agent_path() -> PathBuf {
    workspace_root().join("ext/target/debug/mock-agent")
}

/// Path to the built debug-http binary in `ext/target/debug/`.
pub fn debug_http_path() -> PathBuf {
    workspace_root().join("ext/target/debug/debug-http")
}

/// Path to the built sdk-test-channel binary in `ext/target/debug/`.
pub fn sdk_test_channel_path() -> PathBuf {
    workspace_root().join("ext/target/debug/sdk-test-channel")
}

/// Path to the built sdk-test-tool binary in `ext/target/debug/`.
pub fn sdk_test_tool_path() -> PathBuf {
    workspace_root().join("ext/target/debug/sdk-test-tool")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_agent_path_ends_correctly() {
        let p = mock_agent_path();
        assert!(p.ends_with("ext/target/debug/mock-agent"), "got: {p:?}");
    }

    #[test]
    fn debug_http_path_ends_correctly() {
        let p = debug_http_path();
        assert!(p.ends_with("ext/target/debug/debug-http"), "got: {p:?}");
    }

    #[test]
    fn sdk_test_channel_path_ends_correctly() {
        let p = sdk_test_channel_path();
        assert!(
            p.ends_with("ext/target/debug/sdk-test-channel"),
            "got: {p:?}"
        );
    }

    #[test]
    fn sdk_test_tool_path_ends_correctly() {
        let p = sdk_test_tool_path();
        assert!(p.ends_with("ext/target/debug/sdk-test-tool"), "got: {p:?}");
    }

    #[test]
    fn workspace_root_contains_cargo_toml() {
        let root = workspace_root();
        assert!(
            root.join("Cargo.toml").exists(),
            "workspace root should contain Cargo.toml"
        );
    }
}
