use std::future::Future;
use std::pin::Pin;

use crate::error::ToolSdkError;

/// An MCP-compatible tool. Implement this trait to define tool metadata and execution logic.
/// The [`ToolServer`](crate::ToolServer) handles MCP framing; you only provide business logic.
///
/// D-03 boundary: Tool I/O uses `serde_json::Value` because tool input is defined by a
/// JSON Schema (returned by `input_schema`) and tool output is arbitrary JSON. There is no
/// fixed Rust type that can represent all possible tool schemas at compile time.
pub trait Tool: Send + Sync + 'static {
    /// Return the unique name of this tool.
    fn name(&self) -> &str;
    /// Return a human-readable description of what this tool does.
    fn description(&self) -> &str;
    /// Return the JSON Schema describing expected input.
    ///
    /// D-03: JSON Schema is a JSON document with no fixed Rust type — it must remain `Value`.
    fn input_schema(&self) -> serde_json::Value;
    /// Execute the tool with the given input and return the result.
    ///
    /// D-03: Tool input/output is defined by the tool's schema, not by a static Rust type.
    /// Each tool defines its own input shape via `input_schema`; the executor cannot know
    /// the concrete type at compile time.
    fn execute(
        &self,
        input: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, ToolSdkError>> + Send;
}

/// Dyn-compatible alias for [`Tool`]. Use `Box<dyn DynTool>` for runtime dispatch.
/// Implementors write `impl Tool for X`; the blanket impl provides `DynTool` automatically.
///
/// D-03 boundary: mirrors [`Tool`] — all Value usages flow from the Tool trait contract.
pub trait DynTool: Send + Sync + 'static {
    /// Dyn-compatible version of [`Tool::name`].
    fn name(&self) -> &str;
    /// Dyn-compatible version of [`Tool::description`].
    fn description(&self) -> &str;
    /// Dyn-compatible version of [`Tool::input_schema`].
    /// D-03: JSON Schema has no fixed Rust type.
    fn input_schema(&self) -> serde_json::Value;
    /// Dyn-compatible version of [`Tool::execute`].
    /// D-03: Tool I/O is schema-defined, not statically typed.
    fn execute<'a>(
        &'a self,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ToolSdkError>> + Send + 'a>>;
}

impl<T: Tool> DynTool for T {
    fn name(&self) -> &str {
        Tool::name(self)
    }
    fn description(&self) -> &str {
        Tool::description(self)
    }
    fn input_schema(&self) -> serde_json::Value {
        Tool::input_schema(self)
    }
    fn execute<'a>(
        &'a self,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ToolSdkError>> + Send + 'a>> {
        Box::pin(Tool::execute(self, input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    struct ContractTool;

    impl Tool for ContractTool {
        fn name(&self) -> &str {
            "contract-tool"
        }

        fn description(&self) -> &str {
            "Validates the Tool trait contract"
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                },
                "required": ["message"]
            })
        }

        async fn execute(
            &self,
            input: serde_json::Value,
        ) -> Result<serde_json::Value, ToolSdkError> {
            Ok(serde_json::json!({"echo": input}))
        }
    }

    #[rstest]
    #[test]
    fn when_tool_trait_metadata_accessed_then_contract_values_are_returned() {
        let tool = ContractTool;

        assert_eq!(Tool::name(&tool), "contract-tool");
        assert_eq!(
            Tool::description(&tool),
            "Validates the Tool trait contract"
        );
        assert_eq!(
            Tool::input_schema(&tool)["required"],
            serde_json::json!(["message"])
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_tool_trait_execute_called_then_contract_returns_json_value() {
        let tool = ContractTool;
        let input = serde_json::json!({"message": "hello"});

        let output = Tool::execute(&tool, input.clone()).await.unwrap();

        assert_eq!(output, serde_json::json!({"echo": input}));
    }

    #[rstest]
    #[tokio::test]
    async fn when_tool_boxed_as_dyn_tool_then_dyn_execute_uses_same_contract() {
        let tool: Box<dyn DynTool> = Box::new(ContractTool);
        let input = serde_json::json!({"message": "hello"});

        let output = tool.execute(input.clone()).await.unwrap();

        assert_eq!(tool.name(), "contract-tool");
        assert_eq!(output, serde_json::json!({"echo": input}));
    }
}
