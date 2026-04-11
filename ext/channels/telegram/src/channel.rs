use std::sync::Arc;

use async_trait::async_trait;
use protoclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelSdkError, ChannelSendMessage};
use protoclaw_sdk_types::{
    AckLifecycleNotification, AckNotification, ChannelInitializeParams, ChannelRequestPermission,
    DeliverMessage, PermissionResponse, SessionCreated,
};
use teloxide::Bot;
use teloxide::payloads::{SendMessageSetters, SetMessageReactionSetters};
use teloxide::prelude::Requester;
use teloxide::types::{ChatId, MessageId, ReactionType};
use tokio::sync::mpsc;

use crate::state::SharedState;

pub struct TelegramChannel {
    pub(crate) state: Arc<SharedState>,
    pub(crate) bot: Option<Bot>,
}

impl TelegramChannel {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self { state, bot: None }
    }

    fn bot(&self) -> Result<&Bot, ChannelSdkError> {
        self.bot
            .as_ref()
            .ok_or_else(|| ChannelSdkError::Protocol("bot not initialized".into()))
    }

    fn parse_chat_id(peer_id: &str) -> Option<i64> {
        peer_id
            .strip_prefix("telegram:")
            .and_then(|s| s.parse().ok())
    }

    async fn handle_ack_message(&self, ack: AckNotification) {
        let ack_cfg = self.state.ack_config.read().await;
        let cfg = match ack_cfg.as_ref() {
            Some(c) => c,
            None => return,
        };

        let bot = match &self.bot {
            Some(b) => b,
            None => return,
        };

        let chat_id = match Self::parse_chat_id(&ack.peer_id) {
            Some(id) => id,
            None => return,
        };

        if cfg.reaction {
            if let Some(&msg_id) = self.state.last_message_ids.read().await.get(&chat_id) {
                let reaction = ReactionType::Emoji {
                    emoji: cfg.reaction_emoji.clone(),
                };
                let bot_clone = bot.clone();
                let _ = crate::deliver::retry_telegram_op("ack_set_reaction", chat_id, || {
                    let bot_clone = bot_clone.clone();
                    let reaction = reaction.clone();
                    async move {
                        bot_clone
                            .set_message_reaction(ChatId(chat_id), MessageId(msg_id))
                            .reaction(vec![reaction])
                            .await
                    }
                })
                .await;
            }
        }

        if cfg.typing {
            let bot_clone = bot.clone();
            let _ = crate::deliver::retry_telegram_op("ack_send_typing", chat_id, || {
                let bot_clone = bot_clone.clone();
                async move {
                    bot_clone
                        .send_chat_action(ChatId(chat_id), teloxide::types::ChatAction::Typing)
                        .await
                }
            })
            .await;
        }
    }

    async fn handle_ack_lifecycle(&self, lifecycle: AckLifecycleNotification) {
        if lifecycle.action != "response_started" {
            return;
        }

        let bot = match &self.bot {
            Some(b) => b,
            None => return,
        };

        let ack_cfg = self.state.ack_config.read().await;
        let cfg = match ack_cfg.as_ref() {
            Some(c) => c,
            None => return,
        };

        if !cfg.reaction {
            return;
        }

        let session_chat = self.state.session_chat_map.read().await;
        let chat_id = match session_chat.get(&lifecycle.session_id) {
            Some(&id) => id,
            None => return,
        };
        drop(session_chat);

        let msg_id = match self.state.last_message_ids.read().await.get(&chat_id) {
            Some(&id) => id,
            None => return,
        };

        match cfg.reaction_lifecycle.as_str() {
            "remove" => {
                let bot_clone = bot.clone();
                let _ = crate::deliver::retry_telegram_op(
                    "ack_lifecycle_remove_reaction",
                    chat_id,
                    || {
                        let bot_clone = bot_clone.clone();
                        async move {
                            bot_clone
                                .set_message_reaction(ChatId(chat_id), MessageId(msg_id))
                                .reaction(Vec::<ReactionType>::new())
                                .await
                        }
                    },
                )
                .await;
            }
            "replace_done" => {
                let done_reaction = ReactionType::Emoji {
                    emoji: "✅".into()
                };
                let bot_clone = bot.clone();
                let _ = crate::deliver::retry_telegram_op(
                    "ack_lifecycle_done_reaction",
                    chat_id,
                    || {
                        let bot_clone = bot_clone.clone();
                        let done_reaction = done_reaction.clone();
                        async move {
                            bot_clone
                                .set_message_reaction(ChatId(chat_id), MessageId(msg_id))
                                .reaction(vec![done_reaction])
                                .await
                        }
                    },
                )
                .await;
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: true,
            rich_text: true,
        }
    }

    async fn on_initialize(
        &mut self,
        params: ChannelInitializeParams,
    ) -> Result<(), ChannelSdkError> {
        if let Some(ack) = params.ack {
            *self.state.ack_config.write().await = Some(ack);
        }
        if let Some(emoji) = params
            .options
            .get("TELEGRAM_THOUGHT_EMOJI")
            .and_then(|v| v.as_str())
        {
            *self.state.thought_emoji.write().await = emoji.to_string();
        }
        if let Some(v) = params
            .options
            .get("TELEGRAM_RESPONSE_EDIT_COOLDOWN_MS")
            .and_then(|v| v.as_u64())
        {
            *self.state.response_edit_cooldown_ms.write().await = v;
        }
        if let Some(v) = params
            .options
            .get("TELEGRAM_THOUGHT_DEBOUNCE_MS")
            .and_then(|v| v.as_u64())
        {
            *self.state.thought_debounce_ms.write().await = v;
        }
        if let Some(v) = params
            .options
            .get("TELEGRAM_FINALIZATION_DELAY_MS")
            .and_then(|v| v.as_u64())
        {
            *self.state.finalization_delay_ms.write().await = v;
        }
        let token = params
            .options
            .get("TELEGRAM_BOT_TOKEN")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ChannelSdkError::Protocol(
                    "TELEGRAM_BOT_TOKEN must be set in channel options".into(),
                )
            })?;
        self.bot = Some(Bot::new(token));
        Ok(())
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
    ) -> Result<(), ChannelSdkError> {
        let bot = self.bot()?.clone();
        *self.state.outbound.lock().await = Some(outbound);
        tokio::spawn(crate::dispatcher::run_dispatcher(bot, self.state.clone()));
        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        crate::deliver::deliver_to_chat(self.bot()?, &self.state, &msg.session_id, &msg.content)
            .await
    }

    async fn on_session_created(&mut self, msg: SessionCreated) -> Result<(), ChannelSdkError> {
        let chat_id: i64 = msg
            .peer_info
            .peer_id
            .strip_prefix("telegram:")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                ChannelSdkError::Protocol(format!(
                    "invalid peer_id for telegram: {}",
                    msg.peer_info.peer_id
                ))
            })?;
        self.state
            .session_chat_map
            .write()
            .await
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

        let keyboard = crate::permissions::build_permission_keyboard(&req.request_id, &req.options);

        let sent = self
            .bot()?
            .send_message(teloxide::types::ChatId(chat_id), &req.description)
            .reply_markup(keyboard)
            .await
            .map_err(|e| ChannelSdkError::Protocol(format!("telegram send error: {e}")))?;

        self.state
            .permission_messages
            .lock()
            .await
            .insert(req.request_id.clone(), (chat_id, sent.id.0));

        let rx = self
            .state
            .permission_broker
            .lock()
            .await
            .register(&req.request_id);

        rx.await
            .map_err(|_| ChannelSdkError::Protocol("permission response channel closed".into()))
    }

    async fn handle_unknown(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ChannelSdkError> {
        match method {
            "channel/ackMessage" => {
                if let Ok(ack) = serde_json::from_value::<AckNotification>(params) {
                    self.handle_ack_message(ack).await;
                }
                Ok(serde_json::Value::Null)
            }
            "channel/ackLifecycle" => {
                if let Ok(lifecycle) = serde_json::from_value::<AckLifecycleNotification>(params) {
                    self.handle_ack_lifecycle(lifecycle).await;
                }
                Ok(serde_json::Value::Null)
            }
            _ => Err(ChannelSdkError::Protocol(format!(
                "unknown method: {method}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protoclaw_sdk_channel::ChannelAckConfig;

    fn make_channel() -> TelegramChannel {
        let state = Arc::new(SharedState::new());
        TelegramChannel::new(state)
    }

    fn make_options_with_token() -> std::collections::HashMap<String, serde_json::Value> {
        let mut options = std::collections::HashMap::new();
        options.insert("TELEGRAM_BOT_TOKEN".into(), serde_json::json!("test-token"));
        options
    }

    #[test]
    fn capabilities_streaming_true_rich_text_true() {
        let ch = make_channel();
        let caps = ch.capabilities();
        assert!(caps.streaming);
        assert!(caps.rich_text);
    }

    #[tokio::test]
    async fn on_ready_stores_outbound_sender() {
        let state = Arc::new(SharedState::new());
        let mut ch = TelegramChannel::new(state.clone());
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: None,
            options: make_options_with_token(),
        };
        ch.on_initialize(params).await.unwrap();
        let (tx, _rx) = mpsc::channel(16);
        ch.on_ready(tx).await.unwrap();
        assert!(state.outbound.lock().await.is_some());
    }

    #[tokio::test]
    async fn on_initialize_stores_ack_config() {
        let state = Arc::new(SharedState::new());
        let mut ch = TelegramChannel::new(state.clone());
        let mut options = make_options_with_token();
        options.insert("TELEGRAM_THOUGHT_EMOJI".into(), serde_json::json!("💭"));
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: Some(ChannelAckConfig {
                reaction: true,
                typing: true,
                reaction_emoji: "👀".into(),
                reaction_lifecycle: "remove".into(),
            }),
            options,
        };
        ch.on_initialize(params).await.unwrap();
        let cfg = state.ack_config.read().await;
        assert!(cfg.is_some());
        let cfg = cfg.as_ref().unwrap();
        assert!(cfg.reaction);
        assert!(cfg.typing);
        assert_eq!(cfg.reaction_emoji, "👀");
    }

    #[tokio::test]
    async fn on_initialize_without_ack_leaves_none() {
        let state = Arc::new(SharedState::new());
        let mut ch = TelegramChannel::new(state.clone());
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: None,
            options: make_options_with_token(),
        };
        ch.on_initialize(params).await.unwrap();
        assert!(state.ack_config.read().await.is_none());
    }

    #[test]
    fn parse_chat_id_valid() {
        assert_eq!(
            TelegramChannel::parse_chat_id("telegram:12345"),
            Some(12345)
        );
    }

    #[test]
    fn parse_chat_id_invalid() {
        assert_eq!(TelegramChannel::parse_chat_id("slack:12345"), None);
        assert_eq!(TelegramChannel::parse_chat_id("telegram:abc"), None);
    }
}
