pub mod adapter;
pub mod error;
pub mod generic;

pub use adapter::AgentAdapter;
pub use error::AgentSdkError;
pub use generic::GenericAcpAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn when_generic_adapter_on_initialize_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"protocolVersion": 1, "capabilities": {}});
        let output = adapter.on_initialize_result(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_result_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "sess-42"});
        let output = adapter.on_session_new_result(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn when_generic_adapter_cast_to_trait_object_then_compiles() {
        let _adapter: Box<dyn AgentAdapter> = Box::new(GenericAcpAdapter);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_initialize_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"protocolVersion": 1});
        let output = adapter.on_initialize_params(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_new_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": null});
        let output = adapter.on_session_new_params(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_prompt_params_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input =
            serde_json::json!({"sessionId": "s1", "message": {"role": "user", "content": "hi"}});
        let output = adapter
            .on_session_prompt_params(input.clone())
            .await
            .unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_session_update_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "s1", "type": "agent_message_chunk", "content": "hello"});
        let output = adapter.on_session_update(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn when_generic_adapter_on_permission_request_called_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"requestId": "r1", "description": "Allow?"});
        let output = adapter.on_permission_request(input.clone()).await.unwrap();
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
}
