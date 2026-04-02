pub mod adapter;
pub mod error;
pub mod generic;

pub use adapter::AgentAdapter;
pub use error::AgentSdkError;
pub use generic::GenericAcpAdapter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_adapter_implements_agent_adapter() {
        let _adapter: Box<dyn AgentAdapter> = Box::new(GenericAcpAdapter);
    }

    #[tokio::test]
    async fn generic_adapter_on_initialize_params_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"protocolVersion": 1});
        let output = adapter.on_initialize_params(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn generic_adapter_on_session_new_params_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": null});
        let output = adapter.on_session_new_params(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn generic_adapter_on_session_prompt_params_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "s1", "message": {"role": "user", "content": "hi"}});
        let output = adapter.on_session_prompt_params(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn generic_adapter_on_session_update_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"sessionId": "s1", "type": "agent_message_chunk", "content": "hello"});
        let output = adapter.on_session_update(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn generic_adapter_on_permission_request_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"requestId": "r1", "description": "Allow?"});
        let output = adapter.on_permission_request(input.clone()).await.unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn agent_sdk_error_implements_std_error() {
        let err = AgentSdkError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn agent_sdk_error_protocol_wraps_string() {
        let err = AgentSdkError::Protocol("bad handshake".into());
        assert!(matches!(err, AgentSdkError::Protocol(_)));
        assert!(err.to_string().contains("bad handshake"));
    }
}
