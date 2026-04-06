use async_trait::async_trait;
use protoclaw_sdk_types::{
    ChannelCapabilities, ChannelInitializeParams, ChannelRequestPermission,
    ChannelSendMessage, DeliverMessage, PermissionResponse, SessionCreated,
};
use tokio::sync::mpsc;

use crate::error::ChannelSdkError;

#[async_trait]
pub trait Channel: Send + 'static {
    fn capabilities(&self) -> ChannelCapabilities;

    async fn on_initialize(
        &mut self,
        _params: ChannelInitializeParams,
    ) -> Result<(), ChannelSdkError> {
        Ok(())
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
    ) -> Result<(), ChannelSdkError>;

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError>;

    async fn request_permission(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError>;

    async fn handle_unknown(
        &mut self,
        method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, ChannelSdkError> {
        Err(ChannelSdkError::Protocol(format!(
            "unknown method: {method}"
        )))
    }

    async fn on_session_created(
        &mut self,
        _msg: SessionCreated,
    ) -> Result<(), ChannelSdkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_sdk_types::{PermissionOption, PermissionResponse};
    use rstest::rstest;

    struct MockChannel;

    #[async_trait]
    impl Channel for MockChannel {
        fn capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities {
                streaming: true,
                rich_text: false,
            }
        }

        async fn on_ready(
            &mut self,
            _outbound: mpsc::Sender<ChannelSendMessage>,
        ) -> Result<(), ChannelSdkError> {
            Ok(())
        }

        async fn deliver_message(&mut self, _msg: DeliverMessage) -> Result<(), ChannelSdkError> {
            Ok(())
        }

        async fn request_permission(
            &mut self,
            req: ChannelRequestPermission,
        ) -> Result<PermissionResponse, ChannelSdkError> {
            Ok(PermissionResponse {
                request_id: req.request_id,
                option_id: "allow".into(),
            })
        }
    }

    #[test]
    fn when_channel_impl_created_then_compiles_and_instantiates() {
        let _ch = MockChannel;
    }

    #[test]
    fn when_channel_capabilities_queried_then_returns_configured_values() {
        let ch = MockChannel;
        let caps = ch.capabilities();
        assert!(caps.streaming);
        assert!(!caps.rich_text);
    }

    #[tokio::test]
    async fn when_on_ready_called_with_sender_then_returns_ok() {
        let mut ch = MockChannel;
        let (tx, _rx) = mpsc::channel(1);
        assert!(ch.on_ready(tx).await.is_ok());
    }

    #[tokio::test]
    async fn when_deliver_message_called_then_returns_ok() {
        let mut ch = MockChannel;
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!("hello"),
        };
        assert!(ch.deliver_message(msg).await.is_ok());
    }

    #[tokio::test]
    async fn when_request_permission_called_then_returns_allow_response() {
        let mut ch = MockChannel;
        let req = ChannelRequestPermission {
            request_id: "r1".into(),
            session_id: "s1".into(),
            description: "Allow?".into(),
            options: vec![PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };
        let resp = ch.request_permission(req).await.unwrap();
        assert_eq!(resp.request_id, "r1");
        assert_eq!(resp.option_id, "allow");
    }

    #[tokio::test]
    async fn when_handle_unknown_called_with_unknown_method_then_returns_error() {
        let mut ch = MockChannel;
        let result = ch
            .handle_unknown("foo/bar", serde_json::Value::Null)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("foo/bar"));
    }
}
