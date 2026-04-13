use std::sync::Arc;

use protoclaw_sdk_channel::ChannelSdkError;
use protoclaw_sdk_types::ChannelSendMessage;
use teloxide::prelude::*;
use teloxide::types::{Chat, ChatId, ChatKind, InlineKeyboardMarkup, MessageId, PublicChatKind};

use crate::peer::peer_info_from_chat;
use crate::state::SharedState;

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
    state: &SharedState,
) -> Result<(), ChannelSdkError> {
    let guard = state.outbound.lock().await;
    let outbound = match guard.as_ref() {
        Some(tx) => tx,
        None => return Ok(()),
    };

    let peer_info = peer_info_from_chat(chat_id, chat_type);
    let msg = ChannelSendMessage {
        peer_info,
        content: text.to_string(),
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
        let _ = process_text_message(msg.chat.id.0, chat_type, text, &state).await;
    }
    Ok(())
}

async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = match q.data.as_deref() {
        Some(d) => d,
        None => return Ok(()),
    };

    let (request_id, option_id) = match crate::permissions::parse_callback_data(data) {
        Some(pair) => pair,
        None => return Ok(()),
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

        process_text_message(12345, "private", "hello", &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.peer_info.channel_name, "telegram");
        assert_eq!(msg.peer_info.peer_id, "telegram:12345");
    }

    #[tokio::test]
    async fn process_text_private_chat_sets_direct_kind() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(100, "private", "hi", &state)
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

        process_text_message(-100123, "group", "hi", &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.peer_info.kind, "group");
    }

    #[tokio::test]
    async fn process_text_does_nothing_when_outbound_is_none() {
        let state = SharedState::new();
        let result = process_text_message(1, "private", "hi", &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn process_text_ignores_empty_text() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(1, "private", "", &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, "");
    }
}
