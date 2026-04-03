use std::sync::Arc;

use async_trait::async_trait;
use protoclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelSdkError, ChannelSendMessage};
use protoclaw_sdk_types::{ChannelRequestPermission, DeliverMessage, PermissionResponse, SessionCreated};
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::Requester;
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

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        crate::deliver::deliver_to_chat(&self.bot, &self.state, &msg.session_id, &msg.content)
            .await
    }

    async fn on_session_created(&mut self, msg: SessionCreated) -> Result<(), ChannelSdkError> {
        let chat_id: i64 = msg.peer_info.peer_id
            .strip_prefix("telegram:")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| ChannelSdkError::Protocol(
                format!("invalid peer_id for telegram: {}", msg.peer_info.peer_id)
            ))?;
        self.state.session_chat_map.write().await
            .insert(msg.session_id, chat_id);
        Ok(())
    }

    async fn request_permission(
        &mut self,
        req: ChannelRequestPermission,
    ) -> Result<PermissionResponse, ChannelSdkError> {
        let chat_id = *self
            .state
            .session_chat_map
            .read()
            .await
            .get(&req.session_id)
            .ok_or_else(|| {
                ChannelSdkError::Protocol(format!(
                    "unknown session for permission: {}",
                    req.session_id
                ))
            })?;

        let keyboard =
            crate::permissions::build_permission_keyboard(&req.request_id, &req.options);

        let sent = self
            .bot
            .send_message(teloxide::types::ChatId(chat_id), &req.description)
            .reply_markup(keyboard)
            .await
            .map_err(|e| ChannelSdkError::Protocol(format!("telegram send error: {e}")))?;

        self.state
            .permission_messages
            .lock()
            .await
            .insert(req.request_id.clone(), (chat_id, sent.id.0));

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.state
            .permission_resolvers
            .lock()
            .await
            .insert(req.request_id.clone(), tx);

        rx.await.map_err(|_| {
            ChannelSdkError::Protocol("permission response channel closed".into())
        })
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
