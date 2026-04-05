use std::time::Duration;

use protoclaw_sdk_channel::ChannelSdkError;
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId};
use tokio::time::Instant;

use crate::state::SharedState;

pub fn content_to_string(content: &serde_json::Value) -> String {
    // OpenCode sends content as {"type": "text", "text": "actual text"}
    if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    match content {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let end = if remaining.len() <= max_len {
            remaining.len()
        } else {
            let mut boundary = max_len;
            while boundary > 0 && !remaining.is_char_boundary(boundary) {
                boundary -= 1;
            }
            if boundary == 0 {
                max_len
            } else {
                boundary
            }
        };
        chunks.push(remaining[..end].to_string());
        remaining = &remaining[end..];
    }
    chunks
}

async fn can_edit(state: &SharedState, chat_id: i64) -> bool {
    let last = state.last_edit_time.read().await.get(&chat_id).copied();
    if let Some(last) = last {
        if last.elapsed() < Duration::from_secs(1) {
            return false;
        }
    }
    state
        .last_edit_time
        .write()
        .await
        .insert(chat_id, Instant::now());
    true
}

fn thought_emoji() -> String {
    std::env::var("TELEGRAM_THOUGHT_EMOJI").unwrap_or_else(|_| "🧠".to_string())
}

pub async fn deliver_to_chat(
    bot: &Bot,
    state: &SharedState,
    session_id: &str,
    content: &serde_json::Value,
) -> Result<(), ChannelSdkError> {
    let chat_id = *state
        .session_chat_map
        .read()
        .await
        .get(session_id)
        .ok_or_else(|| {
            ChannelSdkError::Protocol(format!("unknown session: {session_id}"))
        })?;

    let update_obj = content.get("update");
    let update_type = update_obj
        .and_then(|u| u.get("sessionUpdate"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    match update_type {
        "agent_thought_chunk" => {
            let thought_content = update_obj
                .and_then(|u| u.get("content"))
                .map(content_to_string)
                .unwrap_or_default();

            let accumulated = {
                let mut buffers = state.thought_buffers.write().await;
                let buf = buffers.entry(chat_id).or_default();
                buf.push_str(&thought_content);
                buf.clone()
            };

            let emoji = thought_emoji();
            let thought_text = format!("{emoji} {accumulated}");

            let existing = state.thinking_messages.read().await.get(&chat_id).map(|(mid, _)| *mid);
            if let Some(msg_id) = existing {
                if can_edit(state, chat_id).await {
                    let _ = bot
                        .edit_message_text(ChatId(chat_id), MessageId(msg_id), &thought_text)
                        .await;
                }
            } else {
                match bot.send_message(ChatId(chat_id), &thought_text).await {
                    Ok(sent) => {
                        state.thinking_messages.write().await
                            .insert(chat_id, (sent.id.0, Instant::now()));
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to send thinking message");
                    }
                }
            }
            return Ok(());
        }
        "agent_message_chunk" => {
            let chunk_content = update_obj
                .and_then(|u| u.get("content"))
                .map(content_to_string)
                .unwrap_or_default();

            let accumulated = {
                let mut buffers = state.message_buffers.write().await;
                let buf = buffers.entry(chat_id).or_default();
                buf.push_str(&chunk_content);
                buf.clone()
            };

            if accumulated.is_empty() {
                return Ok(());
            }

            let existing_msg_id = state.active_messages.read().await.get(&chat_id).copied();
            if let Some(msg_id) = existing_msg_id {
                if can_edit(state, chat_id).await {
                    let chunks = split_message(&accumulated, 4096);
                    let _ = bot
                        .edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunks[0])
                        .await;
                }
            } else {
                let chunks = split_message(&accumulated, 4096);
                match bot.send_message(ChatId(chat_id), &chunks[0]).await {
                    Ok(sent) => {
                        state
                            .active_messages
                            .write()
                            .await
                            .insert(chat_id, sent.id.0);
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to send message chunk");
                    }
                }
            }
            return Ok(());
        }
        "result" => {
            let final_thought = state.thought_buffers.write().await.remove(&chat_id);
            if let Some((msg_id, start_time)) = state.thinking_messages.write().await.remove(&chat_id) {
                let elapsed = start_time.elapsed().as_secs_f32();
                let emoji = thought_emoji();
                let collapse_text = if let Some(thought) = final_thought {
                    let truncated = if thought.len() > 100 {
                        let mut end = 100;
                        while end > 0 && !thought.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{emoji} Thought for {elapsed:.1}s: {}…", &thought[..end])
                    } else {
                        format!("{emoji} Thought for {elapsed:.1}s: {thought}")
                    };
                    truncated
                } else {
                    format!("{emoji} Thought for {elapsed:.1}s")
                };
                let _ = bot
                    .edit_message_text(ChatId(chat_id), MessageId(msg_id), &collapse_text)
                    .await;
            }

            let final_message = state.message_buffers.write().await.remove(&chat_id);
            if let Some(text) = final_message {
                if let Some(msg_id) = state.active_messages.read().await.get(&chat_id).copied() {
                    let chunks = split_message(&text, 4096);
                    let _ = bot
                        .edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunks[0])
                        .await;
                }
            }

            state.active_messages.write().await.remove(&chat_id);
            state.last_edit_time.write().await.remove(&chat_id);
            return Ok(());
        }
        _ => {}
    }

    let text = update_obj
        .and_then(|u| u.get("content"))
        .map(content_to_string)
        .unwrap_or_default();

    if text.is_empty() {
        state.active_messages.write().await.remove(&chat_id);
        return Ok(());
    }

    let chunks = split_message(&text, 4096);

    for (i, chunk) in chunks.iter().enumerate() {
        let existing_msg_id = if i == 0 {
            state.active_messages.read().await.get(&chat_id).copied()
        } else {
            None
        };

        if let Some(msg_id) = existing_msg_id {
            if can_edit(state, chat_id).await {
                let edit_result = bot
                    .edit_message_text(ChatId(chat_id), MessageId(msg_id), chunk)
                    .await;
                match edit_result {
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::warn!(%e, "failed to edit message text");
                    }
                }
            } else {
                continue;
            }
        }

        match bot.send_message(ChatId(chat_id), chunk).await {
            Ok(sent) => {
                state
                    .active_messages
                    .write()
                    .await
                    .insert(chat_id, sent.id.0);
            }
            Err(e) => {
                tracing::warn!(%e, "failed to send message");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_to_string_extracts_plain_string() {
        let val = serde_json::Value::String("hello".into());
        assert_eq!(content_to_string(&val), "hello");
    }

    #[test]
    fn content_to_string_serializes_object() {
        let val = serde_json::json!({"key": "value"});
        let result = content_to_string(&val);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn content_to_string_serializes_number() {
        let val = serde_json::json!(42);
        assert_eq!(content_to_string(&val), "42");
    }

    #[test]
    fn split_message_short_text_returns_single_chunk() {
        let chunks = split_message("hello", 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello");
    }

    #[test]
    fn split_message_long_text_splits_at_boundary() {
        let text = "a".repeat(8192);
        let chunks = split_message(&text, 4096);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 4096);
        assert_eq!(chunks[1].len(), 4096);
    }

    #[test]
    fn split_message_respects_utf8_boundaries() {
        let text = "é".repeat(3000);
        let chunks = split_message(&text, 4096);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.is_char_boundary(chunk.len()));
        }
    }

    #[test]
    fn split_message_empty_returns_single_empty() {
        let chunks = split_message("", 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[tokio::test]
    async fn extract_chat_id_returns_error_for_unknown_session() {
        let state = SharedState::new();
        let bot = Bot::new("test-token");
        let content = serde_json::json!("hello");
        let result = deliver_to_chat(&bot, &state, "unknown-session", &content).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown session"));
    }

    #[tokio::test]
    async fn empty_content_clears_active_message() {
        let state = SharedState::new();
        state
            .session_chat_map
            .write()
            .await
            .insert("sess-1".into(), 12345);
        state.active_messages.write().await.insert(12345, 99);

        let bot = Bot::new("test-token");
        let content = serde_json::json!("");
        let result = deliver_to_chat(&bot, &state, "sess-1", &content).await;
        assert!(result.is_ok());
        assert!(state.active_messages.read().await.get(&12345).is_none());
    }

    #[tokio::test]
    async fn thinking_messages_state_initialized_empty() {
        let state = SharedState::new();
        assert!(state.thinking_messages.read().await.is_empty());
    }

    #[tokio::test]
    async fn thinking_messages_tracks_chat_id_to_msg_and_time() {
        let state = SharedState::new();
        let now = Instant::now();
        state.thinking_messages.write().await.insert(12345, (42, now));
        let entry = state.thinking_messages.read().await.get(&12345).copied();
        assert!(entry.is_some());
        let (msg_id, _start) = entry.unwrap();
        assert_eq!(msg_id, 42);
    }
}
