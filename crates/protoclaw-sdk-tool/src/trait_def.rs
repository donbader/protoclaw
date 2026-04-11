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
