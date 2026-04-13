use std::path::PathBuf;

pub fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ directory")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

pub fn mock_agent_path() -> PathBuf {
    workspace_root().join("target/debug/mock-agent")
}

pub fn debug_http_path() -> PathBuf {
    workspace_root().join("target/debug/debug-http")
}

pub fn sdk_test_channel_path() -> PathBuf {
    workspace_root().join("target/debug/sdk-test-channel")
}

pub fn sdk_test_tool_path() -> PathBuf {
    workspace_root().join("target/debug/sdk-test-tool")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_agent_path_ends_correctly() {
        let p = mock_agent_path();
        assert!(p.ends_with("target/debug/mock-agent"), "got: {p:?}");
    }

    #[test]
    fn debug_http_path_ends_correctly() {
        let p = debug_http_path();
        assert!(p.ends_with("target/debug/debug-http"), "got: {p:?}");
    }

    #[test]
    fn sdk_test_channel_path_ends_correctly() {
        let p = sdk_test_channel_path();
        assert!(p.ends_with("target/debug/sdk-test-channel"), "got: {p:?}");
    }

    #[test]
    fn sdk_test_tool_path_ends_correctly() {
        let p = sdk_test_tool_path();
        assert!(p.ends_with("target/debug/sdk-test-tool"), "got: {p:?}");
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
