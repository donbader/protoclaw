use std::future::Future;
use std::pin::Pin;

use anyclaw_sdk_types::{
    InitializeParams, InitializeResult, PermissionRequest, SessionNewParams, SessionNewResult,
    SessionPromptParams, SessionUpdateEvent,
};

use crate::error::AgentSdkError;

/// Per-method hooks for ACP lifecycle. Implement this trait to intercept and transform
/// ACP messages between the supervisor and agent subprocess.
///
/// All methods have default passthrough implementations — override only the hooks you need.
pub trait AgentAdapter: Send + Sync + 'static {
    /// Transform `initialize` request params before they reach the agent.
    fn on_initialize_params(
        &self,
        params: InitializeParams,
    ) -> impl Future<Output = Result<InitializeParams, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform `initialize` response before it reaches the supervisor.
    fn on_initialize_result(
        &self,
        result: InitializeResult,
    ) -> impl Future<Output = Result<InitializeResult, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    /// Transform `session/new` request params before they reach the agent.
    fn on_session_new_params(
        &self,
        params: SessionNewParams,
    ) -> impl Future<Output = Result<SessionNewParams, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform `session/new` response before it reaches the supervisor.
    fn on_session_new_result(
        &self,
        result: SessionNewResult,
    ) -> impl Future<Output = Result<SessionNewResult, AgentSdkError>> + Send {
        async move { Ok(result) }
    }

    /// Transform `session/prompt` request params before they reach the agent.
    fn on_session_prompt_params(
        &self,
        params: SessionPromptParams,
    ) -> impl Future<Output = Result<SessionPromptParams, AgentSdkError>> + Send {
        async move { Ok(params) }
    }

    /// Transform a `session/update` streaming event before it reaches the supervisor.
    fn on_session_update(
        &self,
        event: SessionUpdateEvent,
    ) -> impl Future<Output = Result<SessionUpdateEvent, AgentSdkError>> + Send {
        async move { Ok(event) }
    }

    /// Transform a permission request event before it reaches the supervisor.
    fn on_permission_request(
        &self,
        request: PermissionRequest,
    ) -> impl Future<Output = Result<PermissionRequest, AgentSdkError>> + Send {
        async move { Ok(request) }
    }
}

/// Dyn-compatible alias for [`AgentAdapter`]. Use `Box<dyn DynAgentAdapter>` for runtime dispatch.
/// Implementors write `impl AgentAdapter for X`; the blanket impl provides `DynAgentAdapter` automatically.
pub trait DynAgentAdapter: Send + Sync + 'static {
    /// Dyn-compatible version of [`AgentAdapter::on_initialize_params`].
    fn on_initialize_params<'a>(
        &'a self,
        params: InitializeParams,
    ) -> Pin<Box<dyn Future<Output = Result<InitializeParams, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_initialize_result`].
    fn on_initialize_result<'a>(
        &'a self,
        result: InitializeResult,
    ) -> Pin<Box<dyn Future<Output = Result<InitializeResult, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_new_params`].
    fn on_session_new_params<'a>(
        &'a self,
        params: SessionNewParams,
    ) -> Pin<Box<dyn Future<Output = Result<SessionNewParams, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_new_result`].
    fn on_session_new_result<'a>(
        &'a self,
        result: SessionNewResult,
    ) -> Pin<Box<dyn Future<Output = Result<SessionNewResult, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_prompt_params`].
    fn on_session_prompt_params<'a>(
        &'a self,
        params: SessionPromptParams,
    ) -> Pin<Box<dyn Future<Output = Result<SessionPromptParams, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_session_update`].
    fn on_session_update<'a>(
        &'a self,
        event: SessionUpdateEvent,
    ) -> Pin<Box<dyn Future<Output = Result<SessionUpdateEvent, AgentSdkError>> + Send + 'a>>;

    /// Dyn-compatible version of [`AgentAdapter::on_permission_request`].
    fn on_permission_request<'a>(
        &'a self,
        request: PermissionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionRequest, AgentSdkError>> + Send + 'a>>;
}

impl<T: AgentAdapter> DynAgentAdapter for T {
    fn on_initialize_params<'a>(
        &'a self,
        params: InitializeParams,
    ) -> Pin<Box<dyn Future<Output = Result<InitializeParams, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_params(self, params))
    }

    fn on_initialize_result<'a>(
        &'a self,
        result: InitializeResult,
    ) -> Pin<Box<dyn Future<Output = Result<InitializeResult, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_initialize_result(self, result))
    }

    fn on_session_new_params<'a>(
        &'a self,
        params: SessionNewParams,
    ) -> Pin<Box<dyn Future<Output = Result<SessionNewParams, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_params(self, params))
    }

    fn on_session_new_result<'a>(
        &'a self,
        result: SessionNewResult,
    ) -> Pin<Box<dyn Future<Output = Result<SessionNewResult, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_new_result(self, result))
    }

    fn on_session_prompt_params<'a>(
        &'a self,
        params: SessionPromptParams,
    ) -> Pin<Box<dyn Future<Output = Result<SessionPromptParams, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_prompt_params(self, params))
    }

    fn on_session_update<'a>(
        &'a self,
        event: SessionUpdateEvent,
    ) -> Pin<Box<dyn Future<Output = Result<SessionUpdateEvent, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_session_update(self, event))
    }

    fn on_permission_request<'a>(
        &'a self,
        request: PermissionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionRequest, AgentSdkError>> + Send + 'a>> {
        Box::pin(AgentAdapter::on_permission_request(self, request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_sdk_types::PermissionOption;
    use anyclaw_sdk_types::{
        ClientCapabilities, ContentPart, InitializeParams, InitializeResult, PermissionRequest,
        SessionNewParams, SessionNewResult, SessionPromptParams, SessionUpdateEvent,
        SessionUpdateType,
    };
    use rstest::rstest;

    struct DefaultAdapter;

    impl AgentAdapter for DefaultAdapter {}

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_initialize_params_then_passthrough() {
        let adapter = DefaultAdapter;
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        let output = AgentAdapter::on_initialize_params(&adapter, params.clone())
            .await
            .unwrap();
        assert_eq!(output, params);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_initialize_result_then_passthrough() {
        let adapter = DefaultAdapter;
        let result = InitializeResult {
            protocol_version: 1,
            agent_capabilities: None,
            defaults: None,
        };
        let output = AgentAdapter::on_initialize_result(&adapter, result.clone())
            .await
            .unwrap();
        assert_eq!(output, result);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_session_new_params_then_passthrough() {
        let adapter = DefaultAdapter;
        let params = SessionNewParams {
            session_id: None,
            cwd: "/tmp".into(),
            mcp_servers: vec![],
        };
        let output = AgentAdapter::on_session_new_params(&adapter, params.clone())
            .await
            .unwrap();
        assert_eq!(output, params);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_session_new_result_then_passthrough() {
        let adapter = DefaultAdapter;
        let result = SessionNewResult {
            session_id: "sess-1".into(),
        };
        let output = AgentAdapter::on_session_new_result(&adapter, result.clone())
            .await
            .unwrap();
        assert_eq!(output, result);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_session_prompt_params_then_passthrough() {
        let adapter = DefaultAdapter;
        let params = SessionPromptParams {
            session_id: "sess-1".into(),
            prompt: vec![ContentPart::text("hello")],
        };
        let output = AgentAdapter::on_session_prompt_params(&adapter, params.clone())
            .await
            .unwrap();
        assert_eq!(output, params);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_session_update_then_passthrough() {
        let adapter = DefaultAdapter;
        let event = SessionUpdateEvent {
            session_id: "sess-1".into(),
            update: SessionUpdateType::Result {
                content: Some("done".into()),
            },
        };
        let output = AgentAdapter::on_session_update(&adapter, event.clone())
            .await
            .unwrap();
        assert_eq!(output, event);
    }

    #[rstest]
    #[tokio::test]
    async fn when_default_adapter_on_permission_request_then_passthrough() {
        let adapter = DefaultAdapter;
        let request = PermissionRequest {
            request_id: "perm-1".into(),
            description: "Allow?".into(),
            options: vec![PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };
        let output = AgentAdapter::on_permission_request(&adapter, request.clone())
            .await
            .unwrap();
        assert_eq!(output, request);
    }

    struct PermissionRewritingAdapter;

    impl AgentAdapter for PermissionRewritingAdapter {
        async fn on_permission_request(
            &self,
            mut request: PermissionRequest,
        ) -> Result<PermissionRequest, AgentSdkError> {
            request.description = format!("REWRITTEN: {}", request.description);
            Ok(request)
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_custom_adapter_overrides_permission_request_then_transformed() {
        let adapter = PermissionRewritingAdapter;
        let request = PermissionRequest {
            request_id: "perm-1".into(),
            description: "Allow?".into(),
            options: vec![],
        };
        let output = AgentAdapter::on_permission_request(&adapter, request)
            .await
            .unwrap();
        assert_eq!(output.description, "REWRITTEN: Allow?");
        assert_eq!(output.request_id, "perm-1");
    }

    #[rstest]
    #[tokio::test]
    async fn when_custom_adapter_on_non_overridden_hook_then_passthrough() {
        let adapter = PermissionRewritingAdapter;
        let result = SessionNewResult {
            session_id: "sess-2".into(),
        };
        let output = AgentAdapter::on_session_new_result(&adapter, result.clone())
            .await
            .unwrap();
        assert_eq!(output, result);
    }

    #[rstest]
    #[tokio::test]
    async fn when_dyn_adapter_dispatches_typed_params_then_compiles() {
        let adapter: Box<dyn DynAgentAdapter> = Box::new(DefaultAdapter);
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        let output = adapter.on_initialize_params(params.clone()).await.unwrap();
        assert_eq!(output, params);
    }
}
