use std::future::Future;
use std::pin::Pin;

use crate::error::AgentSdkError;

/// Per-method hooks for ACP lifecycle. Implement this trait to intercept and transform
/// ACP messages between the supervisor and agent subprocess.
///
/// All methods have default passthrough implementations — override only the hooks you need.
pub trait AgentAdapter: Send + Sync + 'static {
    /// Transform `initialize` request params before they reach the agent.
    fn on_initialize_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform `initialize` response before it reaches the supervisor.
    fn on_initialize_result(
        &self,
        result: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    /// Transform `session/new` request params before they reach the agent.
    fn on_session_new_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform `session/new` response before it reaches the supervisor.
    fn on_session_new_result(
        &self,
        result: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    /// Transform `session/prompt` request params before they reach the agent.
    fn on_session_prompt_params(
        &self,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform a `session/update` streaming event before it reaches the supervisor.
    fn on_session_update(
        &self,
        event: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, AgentSdkError>> + Send {
        async move { Ok(event) }
    }

    /// Transform a permission request event before it reaches the supervisor.
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
    /// Dyn-compatible version of [`AgentAdapter::on_initialize_params`].
    fn on_initialize_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_initialize_result`].
    fn on_initialize_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_new_params`].
    fn on_session_new_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_new_result`].
    fn on_session_new_result<'a>(
        &'a self,
        result: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_prompt_params`].
    fn on_session_prompt_params<'a>(
        &'a self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_update`].
    fn on_session_update<'a>(
        &'a self,
        event: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_permission_request`].
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    type AdapterHook = for<'a> fn(
        &'a DefaultAdapter,
        serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>>;

    struct DefaultAdapter;

    impl AgentAdapter for DefaultAdapter {}

    struct PermissionRewritingAdapter;

    impl AgentAdapter for PermissionRewritingAdapter {
        async fn on_permission_request(
            &self,
            mut request: serde_json::Value,
        ) -> Result<serde_json::Value, AgentSdkError> {
            request["approved"] = serde_json::json!(true);
            Ok(request)
        }
    }

    fn call_on_initialize_params<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_params(adapter, value))
    }

    fn call_on_initialize_result<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_result(adapter, value))
    }

    fn call_on_session_new_params<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_params(adapter, value))
    }

    fn call_on_session_new_result<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_result(adapter, value))
    }

    fn call_on_session_prompt_params<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_prompt_params(adapter, value))
    }

    fn call_on_session_update<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_update(adapter, value))
    }

    fn call_on_permission_request<'a>(
        adapter: &'a DefaultAdapter,
        value: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_permission_request(adapter, value))
    }

    #[rstest]
    #[case::initialize_params(
        serde_json::json!({"protocolVersion": 1}),
        call_on_initialize_params
    )]
    #[case::initialize_result(
        serde_json::json!({"capabilities": {"streaming": true}}),
        call_on_initialize_result
    )]
    #[case::session_new_params(
        serde_json::json!({"sessionId": null}),
        call_on_session_new_params
    )]
    #[case::session_new_result(
        serde_json::json!({"sessionId": "sess-1"}),
        call_on_session_new_result
    )]
    #[case::session_prompt_params(
        serde_json::json!({"message": {"role": "user", "content": "hello"}}),
        call_on_session_prompt_params
    )]
    #[case::session_update(
        serde_json::json!({"type": "agent_message_chunk", "content": "hello"}),
        call_on_session_update
    )]
    #[case::permission_request(
        serde_json::json!({"requestId": "perm-1", "description": "Allow?"}),
        call_on_permission_request
    )]
    #[tokio::test]
    async fn when_default_adapter_hook_called_then_passthrough(
        #[case] input: serde_json::Value,
        #[case] hook: AdapterHook,
    ) {
        let adapter = DefaultAdapter;

        let output = hook(&adapter, input.clone()).await.unwrap();

        assert_eq!(output, input);
    }

    #[rstest]
    #[tokio::test]
    async fn when_custom_adapter_overrides_permission_request_then_overridden_value_returned() {
        let adapter = PermissionRewritingAdapter;
        let input = serde_json::json!({"requestId": "perm-1", "description": "Allow?"});

        let output = AgentAdapter::on_permission_request(&adapter, input).await.unwrap();

        assert_eq!(output["approved"], serde_json::json!(true));
        assert_eq!(output["requestId"], serde_json::json!("perm-1"));
    }

    #[rstest]
    #[tokio::test]
    async fn when_custom_adapter_on_non_overridden_hook_called_then_default_passthrough_used() {
        let adapter = PermissionRewritingAdapter;
        let input = serde_json::json!({"sessionId": "sess-2"});

        let output = AgentAdapter::on_session_new_result(&adapter, input.clone())
            .await
            .unwrap();

        assert_eq!(output, input);
    }
}
