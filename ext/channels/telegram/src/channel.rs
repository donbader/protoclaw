use std::sync::Arc;

use async_trait::async_trait;
use protoclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelSdkError, ChannelSendMessage};
use protoclaw_sdk_types::{ChannelRequestPermission, DeliverMessage, PermissionResponse};
use teloxide::Bot;
use tokio::sync::mpsc;

use crate::state::SharedState;

pub struct TelegramChannel {
    pub(crate) state: Arc<SharedState>,
    pub(crate) bot: Bot,
}

impl TelegramChannel {
    pub fn new(state: Arc<SharedState>, bot: Bot) -> Self {
        Self { state, bot }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: true,
            rich_text: false,
        }
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
    ) -> Result<(), ChannelSdkError> {
        *self.state.outbound.lock().await = Some(outbound);
        tokio::spawn(crate::dispatcher::run_dispatcher(
            self.bot.clone(),
            self.state.clone(),
        ));
        Ok(())
    }

    async fn deliver_message(&mut self, _msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        todo!("Implemented in Plan 07-02")
    }

    async fn request_permission(
        &mut self,
        _req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError> {
        todo!("Implemented in Plan 07-03")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel() -> TelegramChannel {
        let state = Arc::new(SharedState::new());
        let bot = Bot::new("test-token");
        TelegramChannel::new(state, bot)
    }

    #[test]
    fn capabilities_streaming_true_rich_text_false() {
        let ch = make_channel();
        let caps = ch.capabilities();
        assert!(caps.streaming);
        assert!(!caps.rich_text);
    }

    #[tokio::test]
    async fn on_ready_stores_outbound_sender() {
        let state = Arc::new(SharedState::new());
        let bot = Bot::new("test-token");
        let mut ch = TelegramChannel::new(state.clone(), bot);
        let (tx, _rx) = mpsc::channel(16);
        ch.on_ready(tx).await.unwrap();
        assert!(state.outbound.lock().await.is_some());
    }
}
