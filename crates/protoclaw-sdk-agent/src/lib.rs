//! Agent adapter SDK for protoclaw.
//!
//! Provides the [`AgentAdapter`] trait for intercepting and transforming ACP
//! protocol messages, and [`GenericAcpAdapter`] as a zero-cost passthrough default.
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

    #[tokio::test]
    async fn when_generic_adapter_on_initialize_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"protocolVersion": 1, "capabilities": {}});
        let output = AgentAdapter::on_initialize_result(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "sess-42"});
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
        let input = serde_json::json!({"protocolVersion": 1});
        let output = AgentAdapter::on_initialize_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": null});
        let output = AgentAdapter::on_session_new_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_prompt_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input =
            serde_json::json!({"sessionId": "s1", "message": {"role": "user", "content": "hi"}});
        let output = AgentAdapter::on_session_prompt_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_update_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "s1", "type": "agent_message_chunk", "content": "hello"});
        let output = AgentAdapter::on_session_update(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_permission_request_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"requestId": "r1", "description": "Allow?"});
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
            mut params: serde_json::Value,
        ) -> Result<serde_json::Value, AgentSdkError> {
            if let Some(obj) = params.as_object_mut() {
                obj.insert("injected".into(), serde_json::json!(true));
            }
            Ok(params)
        }
    }

    #[tokio::test]
    async fn when_custom_adapter_overrides_hook_then_transformed_value_returned() {
        let adapter = InjectingAdapter;
        let input =
            serde_json::json!({"sessionId": "s1", "message": {"role": "user", "content": "hi"}});
        let output = AgentAdapter::on_session_prompt_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(output["injected"], serde_json::json!(true));
        assert_eq!(output["sessionId"], serde_json::json!("s1"));
    }

    #[tokio::test]
    async fn when_custom_adapter_non_overridden_hook_called_then_passthrough() {
        let adapter = InjectingAdapter;
        let input = serde_json::json!({"protocolVersion": 1});
        let output = AgentAdapter::on_initialize_params(&adapter, input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }
}
