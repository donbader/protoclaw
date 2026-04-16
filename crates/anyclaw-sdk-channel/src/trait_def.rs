use anyclaw_sdk_types::{
    ChannelCapabilities, ChannelInitializeParams, ChannelRequestPermission, ChannelSendMessage,
    DeliverMessage, PermissionResponse, SessionCreated,
};
use tokio::sync::mpsc;

use crate::error::ChannelSdkError;

/// Messaging channel integration. Implement this trait to connect anyclaw to a platform
/// (e.g. Telegram, Slack, HTTP). The [`ChannelHarness`](crate::ChannelHarness) drives the
/// JSON-RPC lifecycle; you only provide business logic here.
#[allow(async_fn_in_trait)]
pub trait Channel: Send + 'static {
    /// Return the capabilities this channel supports (streaming, rich text).
    fn capabilities(&self) -> ChannelCapabilities;

    /// Return default option values for this channel.
    /// These are merged into the channel's configured options at startup (user values win).
    /// Override to provide channel-specific defaults.
    fn defaults(&self) -> Option<std::collections::HashMap<String, serde_json::Value>> {
        None
    }

    /// Called with supervisor-provided params during the initialize handshake.
    /// Override to extract config from `params.options`. Default: no-op.
    async fn on_initialize(
        &mut self,
        _params: ChannelInitializeParams,
    ) -> Result<(), ChannelSdkError> {
        Ok(())
    }

    /// Called after initialization completes. Store the `outbound` sender to
    /// send user messages back to anyclaw. Store the `permission_tx` sender
    /// to send permission responses asynchronously when the user responds.
    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
        permission_tx: mpsc::Sender<PermissionResponse>,
    ) -> Result<(), ChannelSdkError>;

    /// Render an agent response to this channel's platform.
    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError>;

    /// Show a permission prompt to the user. Return immediately after displaying
    /// the UI (e.g. inline keyboard). When the user responds, send the
    /// [`PermissionResponse`] through the `permission_tx` provided in [`Channel::on_ready`].
    ///
    /// This method must NOT block waiting for the user's response — doing so
    /// would stall delivery of subsequent messages from the agent.
    async fn show_permission_prompt(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<(), ChannelSdkError>;

    /// Handle an unrecognized JSON-RPC method. Default: return a protocol error.
    ///
    /// D-03 boundary: params and return type stay as `serde_json::Value` because unknown
    /// methods have no schema — the channel cannot know the shape at compile time.
    async fn handle_unknown(
        &mut self,
        method: &str,
        // D-03: unknown method params have no fixed schema
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, ChannelSdkError> {
        Err(ChannelSdkError::Protocol(format!(
            "unknown method: {method}"
        )))
    }

    /// Called when the supervisor creates a new session. Default: no-op.
    async fn on_session_created(&mut self, _msg: SessionCreated) -> Result<(), ChannelSdkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyclaw_sdk_types::{PermissionOption, PermissionResponse};

    struct MockChannel;

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
            _permission_tx: mpsc::Sender<PermissionResponse>,
        ) -> Result<(), ChannelSdkError> {
            Ok(())
        }

        async fn deliver_message(&mut self, _msg: DeliverMessage) -> Result<(), ChannelSdkError> {
            Ok(())
        }

        async fn show_permission_prompt(
            &mut self,
            _req: ChannelRequestPermission,
        ) -> Result<(), ChannelSdkError> {
            Ok(())
        }
    }

    #[test]
    fn when_channel_impl_created_then_compiles_and_instantiates() {
        let _ch = MockChannel;
    }

    #[test]
    fn when_defaults_not_overridden_then_returns_none() {
        let ch = MockChannel;
        assert!(ch.defaults().is_none());
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
        let (perm_tx, _perm_rx) = mpsc::channel(1);
        assert!(ch.on_ready(tx, perm_tx).await.is_ok());
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
    async fn when_show_permission_prompt_called_then_returns_ok() {
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
        assert!(ch.show_permission_prompt(req).await.is_ok());
    }

    #[tokio::test]
    async fn when_handle_unknown_called_with_unknown_method_then_returns_error() {
        let mut ch = MockChannel;
        let result = ch.handle_unknown("foo/bar", serde_json::Value::Null).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("foo/bar"));
    }
}
