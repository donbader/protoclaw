use protoclaw_sdk_types::{
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
}

impl<C: Channel> ChannelTester<C> {
    pub fn new(channel: C) -> Self {
        let (outbound_tx, outbound_rx) = mpsc::channel(64);
        Self {
            channel,
            outbound_rx,
            outbound_tx,
        }
    }

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
        });
        self.channel.on_initialize(params).await?;
        self.channel.on_ready(self.outbound_tx.clone()).await?;
        Ok(())
    }

    pub async fn deliver(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        self.channel.deliver_message(msg).await
    }

    pub async fn request_permission(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError> {
        self.channel.request_permission(req).await
    }

    pub fn channel_mut(&mut self) -> &mut C {
        &mut self.channel
    }

    pub fn channel(&self) -> &C {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockTestChannel {
        initialized: Arc<Mutex<bool>>,
        ready: Arc<Mutex<bool>>,
        delivered: Arc<Mutex<Vec<DeliverMessage>>>,
        outbound: Arc<Mutex<Option<mpsc::Sender<ChannelSendMessage>>>>,
    }

    impl MockTestChannel {
        fn new() -> Self {
            Self {
                initialized: Arc::new(Mutex::new(false)),
                ready: Arc::new(Mutex::new(false)),
                delivered: Arc::new(Mutex::new(Vec::new())),
                outbound: Arc::new(Mutex::new(None)),
            }
        }
    }

    #[async_trait]
    impl Channel for MockTestChannel {
        fn capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities {
                streaming: true,
                rich_text: false,
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
        ) -> Result<(), ChannelSdkError> {
            *self.ready.lock().unwrap() = true;
            *self.outbound.lock().unwrap() = Some(outbound);
            Ok(())
        }

        async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
            let maybe_tx = self.outbound.lock().unwrap().clone();
            if let Some(tx) = maybe_tx {
                let _ = tx
                    .send(ChannelSendMessage {
                        peer_info: protoclaw_sdk_types::PeerInfo {
                            channel_name: "test".into(),
                            peer_id: "p1".into(),
                            kind: "dm".into(),
                        },
                        content: msg.content.to_string(),
                    })
                    .await;
            }
            self.delivered.lock().unwrap().push(msg);
            Ok(())
        }

        async fn request_permission(
            &mut self,
            req: ChannelRequestPermission,
        ) -> Result<PermissionResponse, ChannelSdkError> {
            Ok(PermissionResponse {
                request_id: req.request_id,
                option_id: req.options.first().map(|o| o.option_id.clone()).unwrap_or_default(),
            })
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
            })
            .await
            .unwrap();

        assert_eq!(delivered.lock().unwrap().len(), 1);
        let msg = tester.outbound_rx.try_recv().unwrap();
        assert_eq!(msg.content, "\"hello\"");
    }

    #[rstest]
    #[tokio::test]
    async fn when_request_permission_called_then_channel_handles_it() {
        let mut tester = ChannelTester::new(MockTestChannel::new());
        tester.initialize(None).await.unwrap();

        let resp = tester
            .request_permission(ChannelRequestPermission {
                request_id: "perm-1".into(),
                session_id: "s1".into(),
                description: "Allow?".into(),
                options: vec![protoclaw_sdk_types::PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                }],
            })
            .await
            .unwrap();

        assert_eq!(resp.request_id, "perm-1");
        assert_eq!(resp.option_id, "allow");
    }
}
