use async_trait::async_trait;
use protoclaw_sdk_channel::{
    Channel, ChannelCapabilities, ChannelHarness, ChannelSdkError, ChannelSendMessage,
    DeliverMessage, PeerInfo,
};
use protoclaw_sdk_types::{ChannelRequestPermission, PermissionResponse};
use tokio::sync::mpsc;

struct SdkTestChannel {
    outbound: Option<mpsc::Sender<ChannelSendMessage>>,
}

#[async_trait]
impl Channel for SdkTestChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: false,
            rich_text: false,
        }
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
    ) -> Result<(), ChannelSdkError> {
        self.outbound = Some(outbound);
        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        let content_str = match &msg.content {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if let Some(outbound) = &self.outbound {
            let send_msg = ChannelSendMessage {
                peer_info: PeerInfo {
                    channel_name: "sdk-test-channel".into(),
                    peer_id: "test".into(),
                    kind: "test".into(),
                },
                content: content_str,
            };
            outbound.send(send_msg).await.ok();
        }
        Ok(())
    }

    async fn request_permission(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError> {
        let option_id = req
            .options
            .first()
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| "allow".into());
        Ok(PermissionResponse {
            request_id: req.request_id,
            option_id,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ChannelHarness::new(SdkTestChannel { outbound: None })
        .run_stdio()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_no_streaming() {
        let ch = SdkTestChannel { outbound: None };
        let caps = ch.capabilities();
        assert!(!caps.streaming);
        assert!(!caps.rich_text);
    }

    #[tokio::test]
    async fn on_ready_stores_sender() {
        let mut ch = SdkTestChannel { outbound: None };
        assert!(ch.outbound.is_none());
        let (tx, _rx) = mpsc::channel(1);
        ch.on_ready(tx).await.unwrap();
        assert!(ch.outbound.is_some());
    }

    #[tokio::test]
    async fn deliver_echoes_back() {
        let (tx, mut rx) = mpsc::channel(4);
        let mut ch = SdkTestChannel { outbound: Some(tx) };
        let msg = DeliverMessage {
            session_id: "s1".into(),
            content: serde_json::json!("hello from agent"),
        };
        ch.deliver_message(msg).await.unwrap();
        let received = rx.recv().await.expect("should receive echoed message");
        assert_eq!(received.content, "hello from agent");
        assert_eq!(received.peer_info.channel_name, "sdk-test-channel");
    }
}
