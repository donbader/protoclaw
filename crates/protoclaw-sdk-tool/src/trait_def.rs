use crate::error::ToolSdkError;
use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolSdkError>;
}
