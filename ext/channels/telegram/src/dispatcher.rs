use std::sync::Arc;

use protoclaw_sdk_channel::ChannelSdkError;
use protoclaw_sdk_types::ChannelSendMessage;
use teloxide::prelude::*;
use teloxide::types::{Chat, ChatKind, PublicChatKind};

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
        let chat_type = chat_type_str(&msg.chat);
        let _ = process_text_message(msg.chat.id.0, chat_type, text, &state).await;
    }
    Ok(())
}

pub async fn run_dispatcher(bot: Bot, state: Arc<SharedState>) {
    let handler = dptree::entry().branch(
        Update::filter_message().endpoint(handle_text_message),
    );

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
