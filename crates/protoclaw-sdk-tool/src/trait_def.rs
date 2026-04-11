use std::future::Future;
use std::pin::Pin;

use crate::error::ToolSdkError;

pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn execute(
        &self,
        input: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, ToolSdkError>> + Send;
}

/// Dyn-compatible alias for [`Tool`]. Use `Box<dyn DynTool>` for runtime dispatch.
/// Implementors write `impl Tool for X`; the blanket impl provides `DynTool` automatically.
pub trait DynTool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
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
        assert_eq!(Tool::description(&tool), "Validates the Tool trait contract");
        assert_eq!(Tool::input_schema(&tool)["required"], serde_json::json!(["message"]));
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
