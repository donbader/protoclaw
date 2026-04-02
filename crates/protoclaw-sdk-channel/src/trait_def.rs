use async_trait::async_trait;
use protoclaw_sdk_types::{
    ChannelCapabilities, ChannelRequestPermission, ChannelSendMessage, DeliverMessage,
    PermissionResponse,
};
use tokio::sync::mpsc;

use crate::error::ChannelSdkError;

#[async_trait]
pub trait Channel: Send + 'static {
    fn capabilities(&self) -> ChannelCapabilities;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_sdk_types::{PermissionOption, PermissionResponse};

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
    fn mock_channel_compiles_and_instantiates() {
        let _ch = MockChannel;
    }

    #[test]
    fn mock_channel_capabilities() {
        let ch = MockChannel;
        let caps = ch.capabilities();
        assert!(caps.streaming);
        assert!(!caps.rich_text);
    }

    #[tokio::test]
    async fn mock_channel_on_ready() {
        let mut ch = MockChannel;
        let (tx, _rx) = mpsc::channel(1);
        assert!(ch.on_ready(tx).await.is_ok());
    }

    #[tokio::test]
    async fn mock_channel_deliver_message() {
        let mut ch = MockChannel;
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!("hello"),
        };
        assert!(ch.deliver_message(msg).await.is_ok());
    }

    #[tokio::test]
    async fn mock_channel_request_permission() {
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
    async fn mock_channel_handle_unknown_returns_error() {
        let mut ch = MockChannel;
        let result = ch
            .handle_unknown("foo/bar", serde_json::Value::Null)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("foo/bar"));
    }
}
