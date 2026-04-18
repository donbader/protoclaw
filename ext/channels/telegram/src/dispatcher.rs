use std::sync::Arc;

use anyclaw_sdk_channel::ChannelSdkError;
use anyclaw_sdk_types::ChannelSendMessage;
use anyclaw_sdk_types::MessageMetadata;
use anyclaw_sdk_types::acp::ContentPart;
use teloxide::net::Download;
use teloxide::prelude::*;
use teloxide::types::{
    Chat, ChatId, ChatKind, InlineKeyboardMarkup, MediaKind, MessageId, MessageKind, PublicChatKind,
};

use crate::access_control::{
    AccessDecision, SenderIdentity, evaluate_access, should_suppress_reply_context,
};
use crate::peer::peer_info_from_chat;
use crate::state::SharedState;

fn media_type_from_message(msg: &Message) -> Option<&'static str> {
    if let MessageKind::Common(common) = &msg.kind {
        match &common.media_kind {
            MediaKind::Photo(_) => Some("image"),
            MediaKind::Video(_) => Some("video"),
            MediaKind::Audio(_) => Some("audio"),
            MediaKind::Voice(_) => Some("voice"),
            MediaKind::VideoNote(_) => Some("video_note"),
            MediaKind::Animation(_) => Some("animation"),
            MediaKind::Document(_) => Some("document"),
            MediaKind::Sticker(_) => Some("sticker"),
            MediaKind::Location(_) => Some("location"),
            MediaKind::Contact(_) => Some("contact"),
            _ => None,
        }
    } else {
        None
    }
}

fn reply_metadata_from_message(msg: &Message) -> Option<MessageMetadata> {
    let reply_msg = msg.reply_to_message();
    let reply_id = reply_msg.map(|r| r.id.0.to_string());
    let thread_id = msg.thread_id.map(|t| t.0.0.to_string());

    if reply_id.is_none() && thread_id.is_none() {
        return None;
    }

    let (reply_text, is_quote) = if let Some(quote) = msg.quote() {
        (Some(quote.text.clone()), Some(true))
    } else if let Some(r) = reply_msg {
        let text = r.text().or_else(|| r.caption()).map(str::to_string);
        (text, None)
    } else {
        (None, None)
    };

    let reply_to_sender = reply_msg.and_then(|r| {
        r.from.as_ref().map(|u| {
            u.username
                .as_deref()
                .map(String::from)
                .unwrap_or_else(|| u.first_name.clone())
        })
    });
    let reply_to_sender_id = reply_msg.and_then(|r| r.from.as_ref().map(|u| u.id.0.to_string()));
    let reply_to_media_type = reply_msg
        .filter(|_| reply_text.is_none())
        .and_then(media_type_from_message)
        .map(String::from);

    Some(MessageMetadata {
        reply_to_message_id: reply_id,
        reply_to_text: reply_text,
        reply_to_sender,
        reply_to_sender_id,
        reply_to_is_quote: is_quote,
        reply_to_media_type,
        thread_id,
    })
}

fn is_group_chat(chat: &Chat) -> bool {
    matches!(&chat.kind, ChatKind::Public(_))
}

fn text_mentions_bot(text: &str, bot_username: &str) -> bool {
    let mention = format!("@{}", bot_username.to_lowercase());
    text.to_lowercase().contains(&mention)
}

fn entity_mentions_bot(msg: &Message, bot_username: &str) -> bool {
    let Some(text) = msg.text().or(msg.caption()) else {
        return false;
    };
    let entities = msg
        .entities()
        .or(msg.caption_entities())
        .unwrap_or_default();
    let target = format!("@{}", bot_username.to_lowercase());
    for entity in entities {
        if entity.kind == teloxide::types::MessageEntityKind::Mention {
            let slice = &text[entity.offset..entity.offset + entity.length];
            if slice.to_lowercase() == target {
                return true;
            }
        }
    }
    false
}

fn is_reply_to_bot(msg: &Message, bot_id: Option<u64>) -> bool {
    let Some(bot_id) = bot_id else { return false };
    msg.reply_to_message()
        .and_then(|r| r.from.as_ref())
        .is_some_and(|u| u.id.0 == bot_id)
}

fn sender_from_message(msg: &Message) -> SenderIdentity {
    match msg.from.as_ref() {
        Some(user) => SenderIdentity {
            user_id: Some(user.id.0 as i64),
            username: user.username.clone(),
        },
        None => SenderIdentity::default(),
    }
}

async fn check_access(msg: &Message, state: &SharedState) -> AccessDecision {
    let is_group = is_group_chat(&msg.chat);
    let sender = sender_from_message(msg);
    let bot_mentioned = if is_group {
        let username_guard = state.bot_username.read().await;
        let bot_id = *state.bot_id.read().await;
        let explicit = match username_guard.as_deref() {
            Some(username) => {
                entity_mentions_bot(msg, username) || {
                    let text = msg.text().or(msg.caption()).unwrap_or_default();
                    text_mentions_bot(text, username)
                }
            }
            None => false,
        };
        let implicit = is_reply_to_bot(msg, bot_id);
        explicit || implicit
    } else {
        false
    };

    let access_cfg = state.access_config.read().await;
    evaluate_access(&access_cfg, msg.chat.id.0, &sender, is_group, bot_mentioned)
}

async fn maybe_suppress_reply_context(
    msg: &Message,
    state: &SharedState,
) -> Option<MessageMetadata> {
    let is_group = is_group_chat(&msg.chat);
    if !is_group {
        return reply_metadata_from_message(msg);
    }

    let reply_sender = msg
        .reply_to_message()
        .and_then(|r| r.from.as_ref())
        .map(|u| SenderIdentity {
            user_id: Some(u.id.0 as i64),
            username: u.username.clone(),
        })
        .unwrap_or_default();

    let access_cfg = state.access_config.read().await;
    if should_suppress_reply_context(&access_cfg, msg.chat.id.0, &reply_sender, true) {
        tracing::debug!(
            chat_id = msg.chat.id.0,
            ?reply_sender,
            "suppressing reply context for non-allowlisted sender"
        );
        return None;
    }

    reply_metadata_from_message(msg)
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
    reply_image_url: Option<&str>,
    metadata: Option<MessageMetadata>,
    state: &SharedState,
) -> Result<(), ChannelSdkError> {
    let guard = state.outbound.lock().await;
    let Some(outbound) = guard.as_ref() else {
        return Ok(());
    };

    let mut content = Vec::new();
    if let Some(url) = reply_image_url {
        content.push(ContentPart::Image {
            url: url.to_string(),
        });
    }
    content.push(ContentPart::text(text));

    let peer_info = peer_info_from_chat(chat_id, chat_type);
    let msg = ChannelSendMessage {
        peer_info,
        content,
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
    bot: Bot,
    msg: Message,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match check_access(&msg, &state).await {
        AccessDecision::Allow => {}
        AccessDecision::Deny(reason) => {
            tracing::debug!(
                chat_id = msg.chat.id.0,
                ?reason,
                "access denied, dropping message"
            );
            return Ok(());
        }
        AccessDecision::SkipNoMention => {
            tracing::debug!(
                chat_id = msg.chat.id.0,
                "bot not mentioned in group, skipping"
            );
            return Ok(());
        }
    }

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
        let reply_image = resolve_reply_photo(&bot, &msg).await;
        let metadata = maybe_suppress_reply_context(&msg, &state).await;
        let _ = process_text_message(
            msg.chat.id.0,
            chat_type,
            text,
            reply_image.as_deref(),
            metadata,
            &state,
        )
        .await;
    } else if media_type_from_message(&msg).is_some() {
        handle_media_message(bot, msg, state).await?;
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

fn media_label_from_type(media_type: &str) -> &'static str {
    match media_type {
        "image" => "📷 Photo",
        "video" => "🎬 Video",
        "audio" => "🎵 Audio",
        "voice" => "🎤 Voice",
        "video_note" => "📹 Video note",
        "animation" => "🎞 GIF",
        "document" => "📎 Document",
        "sticker" => "🏷 Sticker",
        "location" => "📍 Location",
        "contact" => "👤 Contact",
        other => {
            tracing::warn!(
                media_type = other,
                "unknown media type, using fallback label"
            );
            "📎 Media"
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process_media_message(
    chat_id: i64,
    chat_type: &str,
    media_type: &str,
    caption: Option<&str>,
    image_url: Option<&str>,
    reply_image_url: Option<&str>,
    metadata: Option<MessageMetadata>,
    state: &SharedState,
) -> Result<(), ChannelSdkError> {
    let guard = state.outbound.lock().await;
    let Some(outbound) = guard.as_ref() else {
        return Ok(());
    };

    let label = media_label_from_type(media_type);
    let mut content = Vec::new();

    if let Some(url) = reply_image_url {
        content.push(ContentPart::Image {
            url: url.to_string(),
        });
    }

    if let Some(url) = image_url {
        content.push(ContentPart::Image {
            url: url.to_string(),
        });
        if let Some(cap) = caption {
            content.push(ContentPart::text(cap));
        }
    } else {
        let text = match caption {
            Some(cap) => format!("[{label}] {cap}"),
            None => format!("[{label}]"),
        };
        content.push(ContentPart::text(&text));
    }

    let peer_info = peer_info_from_chat(chat_id, chat_type);
    let msg = ChannelSendMessage {
        peer_info,
        content,
        metadata,
        meta: None,
    };
    outbound
        .send(msg)
        .await
        .map_err(|e| ChannelSdkError::Protocol(format!("outbound send failed: {e}")))?;
    Ok(())
}

async fn handle_media_message(
    bot: Bot,
    msg: Message,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(media_type) = media_type_from_message(&msg) else {
        return Ok(());
    };
    let caption = msg.caption();
    tracing::debug!(
        chat_id = msg.chat.id.0,
        msg_id = msg.id.0,
        telegram_date = msg.date.timestamp(),
        media_type,
        "inbound media message"
    );
    state
        .last_message_ids
        .write()
        .await
        .insert(msg.chat.id.0, msg.id.0);

    let image_url = if media_type == "image" {
        resolve_photo_url(&bot, &msg).await
    } else {
        None
    };

    let reply_image = resolve_reply_photo(&bot, &msg).await;

    let chat_type = chat_type_str(&msg.chat);
    let metadata = maybe_suppress_reply_context(&msg, &state).await;
    let _ = process_media_message(
        msg.chat.id.0,
        chat_type,
        media_type,
        caption,
        image_url.as_deref(),
        reply_image.as_deref(),
        metadata,
        &state,
    )
    .await;
    Ok(())
}

async fn resolve_photo_url(bot: &Bot, msg: &Message) -> Option<String> {
    let photos = msg.photo()?;
    let largest = photos.iter().max_by_key(|p| p.width * p.height)?;
    match bot.get_file(largest.file.id.clone()).await {
        Ok(file) => {
            let mut buf = Vec::new();
            if let Err(e) = bot.download_file(&file.path, &mut buf).await {
                tracing::warn!(error = %e, "failed to download photo from Telegram");
                return None;
            }
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);
            Some(format!("data:image/jpeg;base64,{b64}"))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to get file from Telegram API");
            None
        }
    }
}

async fn resolve_reply_photo(bot: &Bot, msg: &Message) -> Option<String> {
    let reply_msg = msg.reply_to_message()?;
    resolve_photo_url(bot, reply_msg).await
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

        process_text_message(12345, "private", "hello", None, None, &state)
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

        process_text_message(100, "private", "hi", None, None, &state)
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

        process_text_message(-100123, "group", "hi", None, None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.peer_info.kind, "group");
    }

    #[tokio::test]
    async fn process_text_does_nothing_when_outbound_is_none() {
        let state = SharedState::new();
        let result = process_text_message(1, "private", "hi", None, None, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn process_text_ignores_empty_text() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(1, "private", "", None, None, &state)
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

    fn base_user() -> serde_json::Value {
        serde_json::json!({ "id": 42, "is_bot": false, "first_name": "Alice", "username": "alice_dev" })
    }

    #[test]
    fn when_text_reply_then_extracts_id_text_and_sender() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 100, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 99, "date": 0, "chat": base_chat(),
                "from": base_user(),
                "text": "quoted text"
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_message_id.as_deref(), Some("99"));
        assert_eq!(meta.reply_to_text.as_deref(), Some("quoted text"));
        assert_eq!(meta.reply_to_sender.as_deref(), Some("alice_dev"));
        assert_eq!(meta.reply_to_sender_id.as_deref(), Some("42"));
        assert_eq!(meta.reply_to_is_quote, None);
        assert_eq!(meta.reply_to_media_type, None);
        assert_eq!(meta.thread_id, None);
    }

    #[test]
    fn when_partial_quote_then_prefers_quote_text() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 103, "date": 0, "chat": base_chat(),
            "text": "user message",
            "quote": { "text": "partial selection", "position": 5 },
            "reply_to_message": {
                "message_id": 99, "date": 0, "chat": base_chat(),
                "from": base_user(),
                "text": "full original message with partial selection in it"
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_text.as_deref(), Some("partial selection"));
        assert_eq!(meta.reply_to_is_quote, Some(true));
    }

    #[test]
    fn when_media_reply_with_caption_then_extracts_caption() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 101, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 50, "date": 0, "chat": base_chat(),
                "from": base_user(),
                "caption": "photo caption",
                "photo": [{"file_id": "x", "file_unique_id": "y", "width": 100, "height": 100}]
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_message_id.as_deref(), Some("50"));
        assert_eq!(meta.reply_to_text.as_deref(), Some("photo caption"));
        assert_eq!(meta.reply_to_media_type, None);
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
    fn when_media_reply_without_text_or_caption_then_media_type_set() {
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
        assert_eq!(meta.reply_to_media_type.as_deref(), Some("image"));
    }

    #[test]
    fn when_sender_has_no_username_then_uses_first_name() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 104, "date": 0, "chat": base_chat(),
            "text": "user message",
            "reply_to_message": {
                "message_id": 70, "date": 0, "chat": base_chat(),
                "from": { "id": 99, "is_bot": false, "first_name": "Bob" },
                "text": "hi"
            }
        }));
        let meta = reply_metadata_from_message(&msg).unwrap();
        assert_eq!(meta.reply_to_sender.as_deref(), Some("Bob"));
    }

    #[tokio::test]
    async fn when_process_media_with_image_url_then_sends_image_content_part() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            12345,
            "private",
            "image",
            Some("a nice photo"),
            Some("https://api.telegram.org/file/bot123/photos/file_0.jpg"),
            None,
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![
                ContentPart::Image {
                    url: "https://api.telegram.org/file/bot123/photos/file_0.jpg".into()
                },
                ContentPart::text("a nice photo"),
            ]
        );
    }

    #[tokio::test]
    async fn when_process_media_with_image_url_no_caption_then_sends_image_only() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            12345,
            "private",
            "image",
            None,
            Some("https://api.telegram.org/file/bot123/photos/file_0.jpg"),
            None,
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![ContentPart::Image {
                url: "https://api.telegram.org/file/bot123/photos/file_0.jpg".into()
            }]
        );
    }

    #[tokio::test]
    async fn when_process_media_with_caption_then_sends_label_and_caption() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            12345,
            "private",
            "image",
            Some("a nice photo"),
            None,
            None,
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![ContentPart::text("[📷 Photo] a nice photo")]
        );
    }

    #[tokio::test]
    async fn when_process_media_without_caption_then_sends_placeholder() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(12345, "private", "image", None, None, None, None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, vec![ContentPart::text("[📷 Photo]")]);
    }

    #[tokio::test]
    async fn when_process_media_does_nothing_when_outbound_none() {
        let state = SharedState::new();
        let result =
            process_media_message(1, "private", "image", None, None, None, None, &state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_process_media_video_then_sends_video_label() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            1,
            "private",
            "video",
            Some("my video"),
            None,
            None,
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, vec![ContentPart::text("[🎬 Video] my video")]);
    }

    #[test]
    fn when_media_type_image_then_label_is_photo() {
        assert_eq!(media_label_from_type("image"), "📷 Photo");
    }

    #[test]
    fn when_media_type_video_then_label_is_video() {
        assert_eq!(media_label_from_type("video"), "🎬 Video");
    }

    #[test]
    fn when_media_type_audio_then_label_is_audio() {
        assert_eq!(media_label_from_type("audio"), "🎵 Audio");
    }

    #[test]
    fn when_media_type_voice_then_label_is_voice() {
        assert_eq!(media_label_from_type("voice"), "🎤 Voice");
    }

    #[test]
    fn when_media_type_video_note_then_label_is_video_note() {
        assert_eq!(media_label_from_type("video_note"), "📹 Video note");
    }

    #[test]
    fn when_media_type_animation_then_label_is_gif() {
        assert_eq!(media_label_from_type("animation"), "🎞 GIF");
    }

    #[test]
    fn when_media_type_document_then_label_is_document() {
        assert_eq!(media_label_from_type("document"), "📎 Document");
    }

    #[test]
    fn when_media_type_sticker_then_label_is_sticker() {
        assert_eq!(media_label_from_type("sticker"), "🏷 Sticker");
    }

    #[test]
    fn when_media_type_location_then_label_is_location() {
        assert_eq!(media_label_from_type("location"), "📍 Location");
    }

    #[test]
    fn when_media_type_contact_then_label_is_contact() {
        assert_eq!(media_label_from_type("contact"), "👤 Contact");
    }

    #[tokio::test]
    async fn when_photo_message_without_caption_then_sends_photo_placeholder() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 200, "date": 0, "chat": base_chat(),
            "photo": [{"file_id": "a", "file_unique_id": "b", "width": 100, "height": 100}]
        }));

        handle_text_message(Bot::new("test"), msg, state)
            .await
            .unwrap();

        let sent = rx.try_recv().unwrap();
        assert_eq!(sent.content, vec![ContentPart::text("[📷 Photo]")]);
    }

    #[tokio::test]
    async fn when_photo_message_with_caption_then_sends_label_and_caption() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 201, "date": 0, "chat": base_chat(),
            "caption": "look at this",
            "photo": [{"file_id": "a", "file_unique_id": "b", "width": 100, "height": 100}]
        }));

        handle_text_message(Bot::new("test"), msg, state)
            .await
            .unwrap();

        let sent = rx.try_recv().unwrap();
        assert_eq!(
            sent.content,
            vec![ContentPart::text("[📷 Photo] look at this")]
        );
    }

    #[tokio::test]
    async fn when_video_message_then_sends_video_placeholder() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 202, "date": 0, "chat": base_chat(),
            "from": base_user(),
            "video": {
                "file_id": "v1", "file_unique_id": "vu1",
                "width": 640, "height": 480, "duration": 10,
                "file_size": 1000, "mime_type": "video/mp4"
            }
        }));

        handle_text_message(Bot::new("test"), msg, state)
            .await
            .unwrap();

        let sent = rx.try_recv().unwrap();
        assert_eq!(sent.content, vec![ContentPart::text("[🎬 Video]")]);
    }

    #[tokio::test]
    async fn when_document_message_with_caption_then_sends_label_and_caption() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 203, "date": 0, "chat": base_chat(),
            "caption": "here is the file",
            "document": {
                "file_id": "d1", "file_unique_id": "du1"
            }
        }));

        handle_text_message(Bot::new("test"), msg, state)
            .await
            .unwrap();

        let sent = rx.try_recv().unwrap();
        assert_eq!(
            sent.content,
            vec![ContentPart::text("[📎 Document] here is the file")]
        );
    }

    #[tokio::test]
    async fn when_media_message_then_updates_last_message_id() {
        let state = SharedState::new();
        let (tx, _rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 205, "date": 0, "chat": base_chat(),
            "photo": [{"file_id": "a", "file_unique_id": "b", "width": 100, "height": 100}]
        }));

        handle_text_message(Bot::new("test"), msg, Arc::clone(&state))
            .await
            .unwrap();

        let last = state.last_message_ids.read().await;
        assert_eq!(last.get(&1).copied(), Some(205));
    }

    #[tokio::test]
    async fn when_photo_message_sets_correct_peer_info() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);
        let state = Arc::new(state);

        let msg = telegram_msg(serde_json::json!({
            "message_id": 206, "date": 0, "chat": base_chat(),
            "photo": [{"file_id": "a", "file_unique_id": "b", "width": 100, "height": 100}]
        }));

        handle_text_message(Bot::new("test"), msg, state)
            .await
            .unwrap();

        let sent = rx.try_recv().unwrap();
        assert_eq!(sent.peer_info.channel_name, "telegram");
        assert_eq!(sent.peer_info.peer_id, "telegram:1");
        assert_eq!(sent.peer_info.kind, "direct");
    }

    #[test]
    fn when_video_json_then_media_type_is_video() {
        let msg = telegram_msg(serde_json::json!({
            "message_id": 202, "date": 0, "chat": base_chat(),
            "from": base_user(),
            "video": {
                "file_id": "v1", "file_unique_id": "vu1",
                "width": 640, "height": 480, "duration": 10,
                "file_size": 1000, "mime_type": "video/mp4"
            }
        }));
        assert_eq!(media_type_from_message(&msg), Some("video"));
    }

    #[tokio::test]
    async fn when_process_text_with_reply_image_then_image_before_text() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(
            1,
            "private",
            "what is this?",
            Some("data:image/jpeg;base64,/9j/reply"),
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![
                ContentPart::Image {
                    url: "data:image/jpeg;base64,/9j/reply".into()
                },
                ContentPart::text("what is this?"),
            ]
        );
    }

    #[tokio::test]
    async fn when_process_text_without_reply_image_then_text_only() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_text_message(1, "private", "hello", None, None, &state)
            .await
            .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.content, vec![ContentPart::text("hello")]);
    }

    #[tokio::test]
    async fn when_process_media_with_reply_image_then_reply_image_first() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            1,
            "private",
            "video",
            Some("cool video"),
            None,
            Some("data:image/jpeg;base64,/9j/reply"),
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![
                ContentPart::Image {
                    url: "data:image/jpeg;base64,/9j/reply".into()
                },
                ContentPart::text("[🎬 Video] cool video"),
            ]
        );
    }

    #[tokio::test]
    async fn when_process_media_image_with_reply_image_then_both_images() {
        let state = SharedState::new();
        let (tx, mut rx) = mpsc::channel(16);
        *state.outbound.lock().await = Some(tx);

        process_media_message(
            1,
            "private",
            "image",
            None,
            Some("data:image/jpeg;base64,/9j/direct"),
            Some("data:image/jpeg;base64,/9j/reply"),
            None,
            &state,
        )
        .await
        .unwrap();

        let msg = rx.try_recv().unwrap();
        assert_eq!(
            msg.content,
            vec![
                ContentPart::Image {
                    url: "data:image/jpeg;base64,/9j/reply".into()
                },
                ContentPart::Image {
                    url: "data:image/jpeg;base64,/9j/direct".into()
                },
            ]
        );
    }
}
