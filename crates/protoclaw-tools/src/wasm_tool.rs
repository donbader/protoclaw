use std::sync::Arc;

use async_trait::async_trait;
use protoclaw_config::ToolConfig;
use protoclaw_sdk_tool::{Tool, ToolSdkError};

use crate::wasm_runner::WasmToolRunner;

pub struct WasmTool {
    name: String,
    config: ToolConfig,
    module_bytes: Vec<u8>,
    runner: Arc<WasmToolRunner>,
}

impl WasmTool {
    pub fn new(name: String, config: ToolConfig, runner: Arc<WasmToolRunner>) -> Result<Self, ToolSdkError> {
        let module = config.module.as_ref()
            .ok_or_else(|| ToolSdkError::ExecutionFailed("no module path specified".into()))?;
        let module_bytes = std::fs::read(module).map_err(ToolSdkError::Io)?;
        Ok(Self {
            name,
            config,
            module_bytes,
            runner,
        })
    }
}

#[async_trait]
impl Tool for WasmTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn input_schema(&self) -> serde_json::Value {
        self.config
            .input_schema
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| serde_json::json!({"type": "object"}))
    }

    async fn execute(
        &self,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolSdkError> {
        let input_json =
            serde_json::to_string(&input).map_err(ToolSdkError::Serde)?;

        let output = self
            .runner
            .execute(&self.module_bytes, &input_json, &self.config.sandbox)
            .await
            .map_err(|e| ToolSdkError::ExecutionFailed(e.to_string()))?;

        let value: serde_json::Value = serde_json::from_str(&output)
            .unwrap_or_else(|_| serde_json::Value::String(output));

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_config::WasmSandboxConfig;
    use std::path::PathBuf;

    fn make_config(path: PathBuf) -> ToolConfig {
        ToolConfig {
            tool_type: "wasm".into(),
            binary: None,
            args: vec![],
            enabled: true,
            module: Some(path),
            description: "A test WASM tool".into(),
            input_schema: Some(r#"{"type":"object","properties":{"x":{"type":"number"}}}"#.into()),
            sandbox: WasmSandboxConfig::default(),
        }
    }

    #[tokio::test]
    async fn wasm_tool_new_nonexistent_file_returns_error() {
        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let config = make_config(PathBuf::from("/nonexistent/tool.wasm"));
        let result = WasmTool::new("test-tool".into(), config, runner);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wasm_tool_name_returns_config_name() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let tool = WasmTool::new("test-tool".into(), make_config(wasm_path), runner).unwrap();
        assert_eq!(tool.name(), "test-tool");
    }

    #[tokio::test]
    async fn wasm_tool_description_returns_config_description() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let tool = WasmTool::new("test-tool".into(), make_config(wasm_path), runner).unwrap();
        assert_eq!(tool.description(), "A test WASM tool");
    }

    #[tokio::test]
    async fn wasm_tool_input_schema_parses_from_config() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let tool = WasmTool::new("test-tool".into(), make_config(wasm_path), runner).unwrap();
        let schema = tool.input_schema();
        assert!(schema.is_object());
        assert!(schema.get("properties").is_some());
    }

    #[tokio::test]
    async fn wasm_tool_input_schema_defaults_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tool.wasm");
        let wat = r#"(module (memory (export "memory") 1) (func (export "_start")))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let mut config = make_config(wasm_path);
        config.input_schema = None;
        let tool = WasmTool::new("test-tool".into(), config, runner).unwrap();
        assert_eq!(tool.input_schema(), serde_json::json!({"type": "object"}));
    }

    #[tokio::test]
    async fn wasm_tool_execute_returns_json_output() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("echo.wasm");

        let wat = r#"
            (module
                (import "wasi_snapshot_preview1" "fd_read"
                    (func $fd_read (param i32 i32 i32 i32) (result i32)))
                (import "wasi_snapshot_preview1" "fd_write"
                    (func $fd_write (param i32 i32 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (func (export "_start")
                    (i32.store (i32.const 100) (i32.const 200))
                    (i32.store (i32.const 104) (i32.const 256))
                    (call $fd_read (i32.const 0) (i32.const 100) (i32.const 1) (i32.const 96))
                    drop
                    (i32.store (i32.const 108) (i32.const 200))
                    (i32.store (i32.const 112) (i32.load (i32.const 96)))
                    (call $fd_write (i32.const 1) (i32.const 108) (i32.const 1) (i32.const 96))
                    drop
                )
            )
        "#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, &bytes).unwrap();

        let runner = Arc::new(WasmToolRunner::new().unwrap());
        let tool = WasmTool::new("test-tool".into(), make_config(wasm_path), runner).unwrap();

        let input = serde_json::json!({"x": 42});
        let result = tool.execute(input.clone()).await.unwrap();
        assert_eq!(result, input);
    }
}
