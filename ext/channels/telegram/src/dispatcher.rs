use std::sync::Arc;

use anyclaw_sdk_channel::ChannelSdkError;
use anyclaw_sdk_types::ChannelSendMessage;
use anyclaw_sdk_types::MessageMetadata;
use anyclaw_sdk_types::acp::ContentPart;
use teloxide::prelude::*;
use teloxide::types::{Chat, ChatId, ChatKind, InlineKeyboardMarkup, MessageId, PublicChatKind};

use crate::peer::peer_info_from_chat;
use crate::state::SharedState;

fn reply_metadata_from_message(msg: &Message) -> Option<MessageMetadata> {
    let reply_msg = msg.reply_to_message();
    let reply_id = reply_msg.map(|r| r.id.0.to_string());
    let reply_text = reply_msg.and_then(|r| r.text().or_else(|| r.caption()).map(str::to_string));
    let thread_id = msg.thread_id.map(|t| t.0.0.to_string());

    if reply_id.is_none() && thread_id.is_none() {
        return None;
    }

    Some(MessageMetadata {
        reply_to_message_id: reply_id,
        reply_to_text: reply_text,
        thread_id,
    })
}

pub fn chat_type_str(chat: &Chat) -> &str {
    match &chat.kind {
        ChatKind::Private(_) => "private",
        ChatKind::Public(public) => match &public.kind {
            PublicChatKind::Group => "group",
            PublicChatKind::Supergroup(_) => "supergroup",
            PublicChatKind::Channel(_) => "channel",
        },
    }
}

pub async fn process_text_message(
    chat_id: i64,
    chat_type: &str,
    text: &str,
    metadata: Option<MessageMetadata>,
    state: &SharedState,
) -> Result<(), ChannelSdkError> {
    let guard = state.outbound.lock().await;
    let Some(outbound) = guard.as_ref() else {
        return Ok(());
    };

    let peer_info = peer_info_from_chat(chat_id, chat_type);
    let msg = ChannelSendMessage {
        peer_info,
        content: vec![ContentPart::text(text)],
        metadata,
        meta: None,
    };
    outbound
        .send(msg)
        .await
        .map_err(|e| ChannelSdkError::Protocol(format!("outbound send failed: {e}")))?;
    Ok(())
}

async fn handle_text_message(
    msg: Message,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        tracing::debug!(
            chat_id = msg.chat.id.0,
            msg_id = msg.id.0,
            telegram_date = msg.date.timestamp(),
            "inbound message"
        );
        state
            .last_message_ids
            .write()
            .await
            .insert(msg.chat.id.0, msg.id.0);
        let chat_type = chat_type_str(&msg.chat);
        let _ = process_text_message(
            msg.chat.id.0,
            chat_type,
            text,
            reply_metadata_from_message(&msg),
            &state,
        )
        .await;
    }
    Ok(())
}

async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(data) = q.data.as_deref() else {
        return Ok(());
    };

    let Some((request_id, option_id)) = crate::permissions::parse_callback_data(data) else {
        return Ok(());
    };

    tracing::info!(%request_id, %option_id, "callback query received");

    let query_id = q.id.clone();
    let bot_clone = bot.clone();
    let _ = crate::deliver::retry_telegram_op("answer_callback_query", 0, || {
        let bot_clone = bot_clone.clone();
        let query_id = query_id.clone();
        async move { bot_clone.answer_callback_query(query_id).await }
    })
    .await;

    if let Some((chat_id, msg_id)) = state
        .permission_messages
        .lock()
        .await
        .get(request_id)
        .copied()
    {
        let empty_kb =
            InlineKeyboardMarkup::new(Vec::<Vec<teloxide::types::InlineKeyboardButton>>::new());
        let bot_clone2 = bot.clone();
        let empty_kb_clone = empty_kb.clone();
        let _ = crate::deliver::retry_telegram_op("clear_permission_keyboard", chat_id, || {
            let bot_clone2 = bot_clone2.clone();
            let empty_kb_clone = empty_kb_clone.clone();
            async move {
                bot_clone2
                    .edit_message_reply_markup(ChatId(chat_id), MessageId(msg_id))
                    .reply_markup(empty_kb_clone)
                    .await
            }
        })
        .await;
    }

    crate::permissions::process_callback(request_id, option_id, &state).await;

    Ok(())
}

pub async fn run_dispatcher(bot: Bot, state: Arc<SharedState>) {
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_text_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback_query));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn process_text_sends_channel_send_message() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(12345, "private", "hello", None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, vec![ContentPart::text("hello")]);
        assert_eq!(msg.peer_info.channel_name, "telegram");
        assert_eq!(msg.peer_info.peer_id, "telegram:12345");
    }

    #[tokio::test]
    async fn process_text_private_chat_sets_direct_kind() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(100, "private", "hi", None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.peer_info.kind, "direct");
    }

    #[tokio::test]
    async fn process_text_group_chat_sets_group_kind() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(-100123, "group", "hi", None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.peer_info.kind, "group");
    }

    #[tokio::test]
    async fn process_text_does_nothing_when_outbound_is_none() {
        let state = SharedState::new();
        let result = process_text_message(1, "private", "hi", None, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn process_text_ignores_empty_text() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(1, "private", "", None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, vec![ContentPart::text("")]);
    }

    fn telegram_msg(json: serde_json::Value) -> Message {
        serde_json::from_value(json).expect("fixture must be valid teloxide Message")
    }

    fn base_chat() -> serde_json::Value {
        serde_json::json!({ "id": 1, "type": "private" })
    }

    #[test]
    fn when_text_reply_then_extracts_id_and_text() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 100, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 99, "date": 0, "chat": base_chat(),
                "text": "quoted text"
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_message_id.as_deref(), Some("99"));
        assert_eq!(meta.reply_to_text.as_deref(), Some("quoted text"));
        assert_eq!(meta.thread_id, None);
    }

    #[test]
    fn when_media_reply_with_caption_then_extracts_caption() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 101, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 50, "date": 0, "chat": base_chat(),
                "caption": "photo caption",
                "photo": [{"file_id": "x", "file_unique_id": "y", "width": 100, "height": 100}]
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_message_id.as_deref(), Some("50"));
        assert_eq!(meta.reply_to_text.as_deref(), Some("photo caption"));
    }

    #[test]
    fn when_no_reply_then_returns_none() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 100, "date": 0, "chat": base_chat(),
            "text": "plain message"
        }));
        assert!(reply_metadata_from_message(&msg).is_none());
    }

    #[test]
    fn when_media_reply_without_text_or_caption_then_text_is_none() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 102, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 60, "date": 0, "chat": base_chat(),
                "photo": [{"file_id": "x", "file_unique_id": "y", "width": 100, "height": 100}]
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_message_id.as_deref(), Some("60"));
        assert_eq!(meta.reply_to_text, None);
    }
}
