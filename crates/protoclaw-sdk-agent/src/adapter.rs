use std::future::Future;
use std::pin::Pin;

use crate::error::AgentSdkError;

/// Per-method hooks for ACP lifecycle. Implement this trait to intercept and transform
/// ACP messages between the supervisor and agent subprocess.
///
/// All methods have default passthrough implementations — override only the hooks you need.
pub trait AgentAdapter: Send + Sync + 'static {
    fn on_initialize_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    fn on_initialize_result(
        &self,
        result: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    fn on_session_new_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    fn on_session_new_result(
        &self,
        result: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    fn on_session_prompt_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    fn on_session_update(
        &self,
        event: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(event) }
    }

    fn on_permission_request(
        &self,
        request: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(request) }
    }
}

/// Dyn-compatible alias for [`AgentAdapter`]. Use `Box<dyn DynAgentAdapter>` for runtime dispatch.
/// Implementors write `impl AgentAdapter for X`; the blanket impl provides `DynAgentAdapter` automatically.
pub trait DynAgentAdapter: Send + Sync + 'static {
    fn on_initialize_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_initialize_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_session_new_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_session_new_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_session_prompt_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_session_update<'a>(
        &'a self,
        event: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    fn on_permission_request<'a>(
        &'a self,
        request: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;
}

impl<T: AgentAdapter> DynAgentAdapter for T {
    fn on_initialize_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_params(self, params))
    }

    fn on_initialize_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_result(self, result))
    }

    fn on_session_new_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_params(self, params))
    }

    fn on_session_new_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_result(self, result))
    }

    fn on_session_prompt_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_prompt_params(self, params))
    }

    fn on_session_update<'a>(
        &'a self,
        event: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_update(self, event))
    }

    fn on_permission_request<'a>(
        &'a self,
        request: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_permission_request(self, request))
    }
}
