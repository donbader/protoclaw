use async_trait::async_trait;
use crate::error::AgentSdkError;

#[async_trait]
pub trait AgentAdapter: Send + Sync + 'static {
    async fn on_initialize_params(&self, params: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(params)
    }

    async fn on_initialize_result(&self, result: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(result)
    }

    async fn on_session_new_params(&self, params: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(params)
    }

    async fn on_session_new_result(&self, result: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(result)
    }

    async fn on_session_prompt_params(&self, params: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(params)
    }

    async fn on_session_update(&self, event: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(event)
    }

    async fn on_permission_request(&self, request: serde_json::Value) -> Result<serde_json::Value, AgentSdkError> {
        Ok(request)
    }
}
