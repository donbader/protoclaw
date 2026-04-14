//! Agent adapter SDK for anyclaw.
//!
//! Provides the [`AgentAdapter`] trait for intercepting and transforming ACP
//! protocol messages, and [`GenericAcpAdapter`] as a zero-cost passthrough default.
//!
//! # Stability
//!
//! This crate is **unstable** — APIs may change between releases.
//! Enums marked `#[non_exhaustive]` will have new variants added; match arms must include `_`.
#![warn(missing_docs)]

/// ACP message adapter trait and dyn-compatible wrapper.
pub mod adapter;
/// Error types for the agent SDK.
pub mod error;
/// Zero-cost passthrough adapter implementation.
pub mod generic;

pub use adapter::{AgentAdapter, DynAgentAdapter};
pub use error::AgentSdkError;
pub use generic::GenericAcpAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use adapter::AgentAdapter;
    use anyclaw_sdk_types::{
        ClientCapabilities, ContentPart, InitializeParams, InitializeResult, PermissionOption,
        PermissionRequest, SessionNewParams, SessionNewResult, SessionPromptParams,
        SessionUpdateEvent, SessionUpdateType,
    };

    #[tokio::test]
    async fn when_generic_adapter_on_initialize_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = InitializeResult {
            protocol_version: 1,
            agent_capabilities: None,
        };
        let output = AgentAdapter::on_initialize_result(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = SessionNewResult {
            session_id: "sess-42".into(),
        };
        let output = AgentAdapter::on_session_new_result(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn when_generic_adapter_cast_to_dyn_trait_object_then_compiles() {
        let _adapter: Box<dyn DynAgentAdapter> = Box::new(GenericAcpAdapter);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_initialize_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        let output = AgentAdapter::on_initialize_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = SessionNewParams {
            session_id: None,
            cwd: "/tmp".into(),
            mcp_servers: vec![],
        };
        let output = AgentAdapter::on_session_new_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_prompt_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = SessionPromptParams {
            session_id: "s1".into(),
            prompt: vec![ContentPart::text("hi")],
        };
        let output = AgentAdapter::on_session_prompt_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_update_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = SessionUpdateEvent {
            session_id: "s1".into(),
            update: SessionUpdateType::Result {
                content: Some("hello".into()),
            },
        };
        let output = AgentAdapter::on_session_update(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_permission_request_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = PermissionRequest {
            request_id: "r1".into(),
            description: "Allow?".into(),
            options: vec![PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };
        let output = AgentAdapter::on_permission_request(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn when_agent_sdk_error_checked_then_implements_std_error() {
        let err = AgentSdkError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn when_protocol_error_created_then_wraps_string_message() {
        let err = AgentSdkError::Protocol("bad handshake".into());
        assert!(matches!(err, AgentSdkError::Protocol(_)));
        assert!(err.to_string().contains("bad handshake"));
    }

    struct InjectingAdapter;

    impl AgentAdapter for InjectingAdapter {
        async fn on_session_prompt_params(
            &self,
            mut params: SessionPromptParams,
        ) -> Result<SessionPromptParams, AgentSdkError> {
            params.prompt.push(ContentPart::text("injected"));
            Ok(params)
        }
    }

    #[tokio::test]
    async fn when_custom_adapter_overrides_hook_then_transformed_value_returned() {
        let adapter = InjectingAdapter;
        let input = SessionPromptParams {
            session_id: "s1".into(),
            prompt: vec![ContentPart::text("hi")],
        };
        let output = AgentAdapter::on_session_prompt_params(&adapter, input)
            .await
            .unwrap();
        assert_eq!(output.prompt.len(), 2);
        assert_eq!(output.session_id, "s1");
    }

    #[tokio::test]
    async fn when_custom_adapter_non_overridden_hook_called_then_passthrough() {
        let adapter = InjectingAdapter;
        let input = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        let output = AgentAdapter::on_initialize_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }
}
