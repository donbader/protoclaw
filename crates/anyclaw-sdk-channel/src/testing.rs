use anyclaw_sdk_types::{
    ChannelCapabilities, ChannelInitializeParams, ChannelRequestPermission, ChannelSendMessage,
    DeliverMessage, PermissionResponse,
};
use tokio::sync::mpsc;

use crate::error::ChannelSdkError;
use crate::trait_def::Channel;

/// Test wrapper for any [`Channel`] implementation.
///
/// Provides typed methods that mirror the harness dispatch without JSON-RPC framing.
/// Channel authors use this to unit-test their impl directly.
pub struct ChannelTester<C: Channel> {
    channel: C,
    /// Receive outbound messages the channel sends via its outbound sender.
    pub outbound_rx: mpsc::Receiver<ChannelSendMessage>,
    outbound_tx: mpsc::Sender<ChannelSendMessage>,
    /// Receive permission responses the channel sends asynchronously.
    pub permission_rx: mpsc::Receiver<PermissionResponse>,
    permission_tx: mpsc::Sender<PermissionResponse>,
}

impl<C: Channel> ChannelTester<C> {
    /// Create a new tester wrapping the given channel, with buffered channels.
    pub fn new(channel: C) -> Self {
        let (outbound_tx, outbound_rx) = mpsc::channel(64);
        let (permission_tx, permission_rx) = mpsc::channel(16);
        Self {
            channel,
            outbound_rx,
            outbound_tx,
            permission_rx,
            permission_tx,
        }
    }

    /// Query the wrapped channel's capabilities.
    pub fn capabilities(&self) -> ChannelCapabilities {
        self.channel.capabilities()
    }

    /// Run the initialize lifecycle: on_initialize + on_ready.
    /// Pass None for default params (protocol_version=1, channel_id="test", no ack).
    pub async fn initialize(
        &mut self,
        params: Option<ChannelInitializeParams>,
    ) -> Result<(), ChannelSdkError> {
        let params = params.unwrap_or(ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "test".into(),
            ack: None,
            options: std::collections::HashMap::new(),
        });
        self.channel.on_initialize(params).await?;
        self.channel
            .on_ready(self.outbound_tx.clone(), self.permission_tx.clone())
            .await?;
        Ok(())
    }

    /// Deliver an agent message to the channel under test.
    pub async fn deliver(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        self.channel.deliver_message(msg).await
    }

    /// Show a permission prompt on the channel under test.
    pub async fn show_permission_prompt(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<(), ChannelSdkError> {
        self.channel.show_permission_prompt(req).await
    }

    /// Get a mutable reference to the wrapped channel.
    pub fn channel_mut(&mut self) -> &mut C {
        &mut self.channel
    }

    /// Get a shared reference to the wrapped channel.
    pub fn channel(&self) -> &C {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockTestChannel {
        initialized: Arc<Mutex<bool>>,
        ready: Arc<Mutex<bool>>,
        delivered: Arc<Mutex<Vec<DeliverMessage>>>,
        outbound: Arc<Mutex<Option<mpsc::Sender<ChannelSendMessage>>>>,
        permission_tx: Arc<Mutex<Option<mpsc::Sender<PermissionResponse>>>>,
    }

    impl MockTestChannel {
        fn new() -> Self {
            Self {
                initialized: Arc::new(Mutex::new(false)),
                ready: Arc::new(Mutex::new(false)),
                delivered: Arc::new(Mutex::new(Vec::new())),
                outbound: Arc::new(Mutex::new(None)),
                permission_tx: Arc::new(Mutex::new(None)),
            }
        }
    }

    impl Channel for MockTestChannel {
        fn capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities {
                streaming: true,
                rich_text: false,
                media: false,
            }
        }

        async fn on_initialize(
            &mut self,
            _params: ChannelInitializeParams,
        ) -> Result<(), ChannelSdkError> {
            *self.initialized.lock().unwrap() = true;
            Ok(())
        }

        async fn on_ready(
            &mut self,
            outbound: mpsc::Sender<ChannelSendMessage>,
            permission_tx: mpsc::Sender<PermissionResponse>,
        ) -> Result<(), ChannelSdkError> {
            *self.ready.lock().unwrap() = true;
            *self.outbound.lock().unwrap() = Some(outbound);
            *self.permission_tx.lock().unwrap() = Some(permission_tx);
            Ok(())
        }

        async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
            let maybe_tx = self.outbound.lock().unwrap().clone();
            if let Some(tx) = maybe_tx {
                let _ = tx
                    .send(ChannelSendMessage {
                        peer_info: anyclaw_sdk_types::PeerInfo {
                            channel_name: "test".into(),
                            peer_id: "p1".into(),
                            kind: "dm".into(),
                        },
                        content: vec![anyclaw_sdk_types::ContentPart::text(
                            msg.content.to_string(),
                        )],
                        metadata: None,
                        meta: None,
                        sender_info: None,
                        was_mentioned: None,
                    })
                    .await;
            }
            self.delivered.lock().unwrap().push(msg);
            Ok(())
        }

        async fn show_permission_prompt(
            &mut self,
            req: ChannelRequestPermission,
        ) -> Result<(), ChannelSdkError> {
            let tx = self.permission_tx.lock().unwrap().clone();
            if let Some(tx) = tx {
                let _ = tx
                    .send(PermissionResponse {
                        request_id: req.request_id,
                        option_id: req
                            .options
                            .first()
                            .map(|o| o.option_id.clone())
                            .unwrap_or_default(),
                    })
                    .await;
            }
            Ok(())
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_channel_tester_created_then_provides_outbound_rx() {
        let tester = ChannelTester::new(MockTestChannel::new());
        assert!(tester.capabilities().streaming);
        assert!(!tester.capabilities().rich_text);
    }

    #[rstest]
    #[tokio::test]
    async fn when_initialize_called_then_channel_on_initialize_and_on_ready_invoked() {
        let ch = MockTestChannel::new();
        let initialized = ch.initialized.clone();
        let ready = ch.ready.clone();
        let mut tester = ChannelTester::new(ch);

        tester.initialize(None).await.unwrap();

        assert!(*initialized.lock().unwrap());
        assert!(*ready.lock().unwrap());
    }

    #[rstest]
    #[tokio::test]
    async fn when_deliver_called_then_channel_receives_and_outbound_available() {
        let ch = MockTestChannel::new();
        let delivered = ch.delivered.clone();
        let mut tester = ChannelTester::new(ch);
        tester.initialize(None).await.unwrap();

        tester
            .deliver(DeliverMessage {
                session_id: "s1".into(),
                content: serde_json::json!("hello"),
                meta: None,
            })
            .await
            .unwrap();

        assert_eq!(delivered.lock().unwrap().len(), 1);
        let msg = tester.outbound_rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![anyclaw_sdk_types::ContentPart::text("\"hello\"")]
        );
    }

    #[rstest]
    #[tokio::test]
    async fn when_show_permission_prompt_called_then_channel_sends_response() {
        let mut tester = ChannelTester::new(MockTestChannel::new());
        tester.initialize(None).await.unwrap();

        tester
            .show_permission_prompt(ChannelRequestPermission {
                request_id: "perm-1".into(),
                session_id: "s1".into(),
                description: "Allow?".into(),
                options: vec![anyclaw_sdk_types::PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                }],
            })
            .await
            .unwrap();

        let resp = tester.permission_rx.try_recv().unwrap();
        assert_eq!(resp.request_id, "perm-1");
        assert_eq!(resp.option_id, "allow");
    }
}
