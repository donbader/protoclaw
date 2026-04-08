use std::sync::Arc;
use std::time::Duration;

use protoclaw_sdk_channel::ChannelSdkError;
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId};

use crate::state::SharedState;
use crate::turn::{ChatTurn, TurnPhase};

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

fn thought_emoji() -> String {
    std::env::var("TELEGRAM_THOUGHT_EMOJI").unwrap_or_else(|_| "🧠".to_string())
}

pub async fn deliver_to_chat(
    bot: &Bot,
    state: &Arc<SharedState>,
    session_id: &str,
    content: &serde_json::Value,
) -> Result<(), ChannelSdkError> {
    let chat_id = *state
        .session_chat_map
        .read()
        .await
        .get(session_id)
        .ok_or_else(|| ChannelSdkError::Protocol(format!("unknown session: {session_id}")))?;

    let update_obj = content.get("update");
    let update_type = update_obj
        .and_then(|u| u.get("sessionUpdate"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let message_id = update_obj
        .and_then(|u| u.get("messageId"))
        .and_then(|v| v.as_str());

    match update_type {
        "agent_thought_chunk" => {
            {
                let mut turns = state.turns.write().await;
                if let Some(mid) = message_id {
                    if let Some(turn) = turns.get_mut(&chat_id) {
                        if turn.is_different_turn(mid) {
                            tracing::info!(chat_id, old_message_id = %turn.message_id, new_message_id = mid, "messageId turn change detected in thought");
                            turn.cleanup();
                            turns.remove(&chat_id);
                        }
                    }
                }
            }

            {
                let turns = state.turns.read().await;
                if let Some(turn) = turns.get(&chat_id) {
                    if turn.thought.as_ref().map(|t| t.suppressed).unwrap_or(false) {
                        return Ok(());
                    }
                }
            }

            {
                let mut turns = state.turns.write().await;
                let is_finalizing = turns
                    .get(&chat_id)
                    .map(|t| matches!(t.phase, TurnPhase::Finalizing(_)))
                    .unwrap_or(false);
                if is_finalizing {
                    if let Some(turn) = turns.get_mut(&chat_id) {
                        turn.cleanup();
                    }
                    turns.remove(&chat_id);
                }
            }

            let thought_content = update_obj
                .and_then(|u| u.get("content"))
                .map(content_to_string)
                .unwrap_or_default();

            let (accumulated, existing_thought_msg_id) = {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns.entry(chat_id).or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                turn.append_thought(&thought_content, 0);
                let track = turn.thought.as_ref().unwrap();
                let accumulated = track.buffer.clone();
                let existing_thought_msg_id = track.msg_id;
                if let Some(ref h) = track.debounce_handle {
                    h.abort();
                }
                turn.thought.as_mut().unwrap().debounce_handle = None;
                (accumulated, existing_thought_msg_id)
            };

            let emoji = thought_emoji();
            let thought_text = format!("{emoji} {accumulated}");

            if existing_thought_msg_id == 0 {
                match bot.send_message(ChatId(chat_id), &thought_text).await {
                    Ok(sent) => {
                        let mut turns = state.turns.write().await;
                        if let Some(turn) = turns.get_mut(&chat_id) {
                            if let Some(track) = turn.thought.as_mut() {
                                track.msg_id = sent.id.0;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to send thinking message");
                    }
                }
            } else {
                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let handle = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    let (accumulated, thought_msg_id) = {
                        let turns = state_clone.turns.read().await;
                        match turns.get(&chat_id) {
                            Some(turn) => match &turn.thought {
                                Some(track) => (track.buffer.clone(), track.msg_id),
                                None => return,
                            },
                            None => return,
                        }
                    };
                    let emoji = thought_emoji();
                    let text = format!("{emoji} {accumulated}");
                    if thought_msg_id != 0 {
                        let _ = bot_clone
                            .edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &text)
                            .await;
                    }
                    let mut turns = state_clone.turns.write().await;
                    if let Some(turn) = turns.get_mut(&chat_id) {
                        if let Some(track) = turn.thought.as_mut() {
                            track.debounce_handle = None;
                        }
                    }
                });

                let mut turns = state.turns.write().await;
                if let Some(turn) = turns.get_mut(&chat_id) {
                    if let Some(track) = turn.thought.as_mut() {
                        track.debounce_handle = Some(handle);
                    }
                }
            }
            return Ok(());
        }

        "agent_message_chunk" => {
            {
                let mut turns = state.turns.write().await;
                if let Some(mid) = message_id {
                    if let Some(turn) = turns.get_mut(&chat_id) {
                        if turn.is_different_turn(mid) {
                            tracing::info!(chat_id, old_message_id = %turn.message_id, new_message_id = mid, "messageId turn change detected in message chunk");
                            turn.cleanup();
                            turns.remove(&chat_id);
                        }
                    }
                }
            }

            let chunk_content = update_obj
                .and_then(|u| u.get("content"))
                .map(content_to_string)
                .unwrap_or_default();

            let (accumulated, existing_response_msg_id, is_finalizing) = {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns.entry(chat_id).or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                if let Some(track) = turn.thought.as_mut() {
                    track.suppressed = false;
                }
                turn.append_response(&chunk_content, 0);
                let track = turn.response.as_ref().unwrap();
                let accumulated = track.buffer.clone();
                let existing_response_msg_id = track.msg_id;
                let is_finalizing = matches!(turn.phase, TurnPhase::Finalizing(_));
                (accumulated, existing_response_msg_id, is_finalizing)
            };

            if accumulated.is_empty() {
                return Ok(());
            }

            if is_finalizing {
                if existing_response_msg_id != 0 {
                    let chunks = split_message(&accumulated, 4096);
                    let _ = bot
                        .edit_message_text(
                            ChatId(chat_id),
                            MessageId(existing_response_msg_id),
                            &chunks[0],
                        )
                        .await;
                    for chunk in chunks.iter().skip(1) {
                        let _ = bot.send_message(ChatId(chat_id), chunk).await;
                    }
                }

                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let new_handle = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let final_data = {
                        let mut turns = state_clone.turns.write().await;
                        turns.get_mut(&chat_id).and_then(|t| t.take_response_for_finalize())
                    };
                    if let Some((text, msg_id)) = final_data {
                        if !text.is_empty() && msg_id != 0 {
                            let final_chunks = split_message(&text, 4096);
                            if let Err(e) = bot_clone
                                .edit_message_text(
                                    ChatId(chat_id),
                                    MessageId(msg_id),
                                    &final_chunks[0],
                                )
                                .await
                            {
                                tracing::warn!(%e, chat_id, "failed to finalize message edit (late)");
                            }
                            for chunk in final_chunks.iter().skip(1) {
                                if let Err(e) =
                                    bot_clone.send_message(ChatId(chat_id), chunk).await
                                {
                                    tracing::warn!(%e, chat_id, "failed to send overflow chunk (late)");
                                }
                            }
                        }
                    }
                    state_clone.turns.write().await.remove(&chat_id);
                });

                let mut turns = state.turns.write().await;
                if let Some(turn) = turns.get_mut(&chat_id) {
                    turn.begin_finalizing(new_handle);
                }
                return Ok(());
            }

            if existing_response_msg_id != 0 {
                let can_edit = {
                    let mut turns = state.turns.write().await;
                    turns
                        .get_mut(&chat_id)
                        .map(|t| t.can_edit_response())
                        .unwrap_or(false)
                };
                if can_edit {
                    let chunks = split_message(&accumulated, 4096);
                    if let Err(e) = bot
                        .edit_message_text(
                            ChatId(chat_id),
                            MessageId(existing_response_msg_id),
                            &chunks[0],
                        )
                        .await
                    {
                        tracing::warn!(%e, chat_id, "failed to edit message chunk");
                    }
                }
            } else {
                let chunks = split_message(&accumulated, 4096);
                match bot.send_message(ChatId(chat_id), &chunks[0]).await {
                    Ok(sent) => {
                        let mut turns = state.turns.write().await;
                        if let Some(turn) = turns.get_mut(&chat_id) {
                            if let Some(track) = turn.response.as_mut() {
                                track.msg_id = sent.id.0;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to send message chunk");
                    }
                }
            }
            return Ok(());
        }

        "result" => {
            {
                let turns = state.turns.read().await;
                if let Some(mid) = message_id {
                    if let Some(turn) = turns.get(&chat_id) {
                        if turn.is_different_turn(mid) {
                            tracing::info!(chat_id, stale_message_id = mid, current_message_id = %turn.message_id, "discarding stale result from previous turn");
                            return Ok(());
                        }
                    }
                }
            }

            let collapse_data = {
                let mut turns = state.turns.write().await;
                turns.get_mut(&chat_id).and_then(|t| t.collapse_thought())
            };

            if let Some((thought_msg_id, elapsed_secs)) = collapse_data {
                if thought_msg_id != 0 {
                    let emoji = thought_emoji();
                    let collapse_text = format!("{emoji} Thought for {elapsed_secs:.1}s");
                    if let Err(e) = bot
                        .edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &collapse_text)
                        .await
                    {
                        tracing::warn!(%e, chat_id, "failed to collapse thinking message");
                    }
                }
            }

            {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns.entry(chat_id).or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                if let Some(track) = turn.thought.as_mut() {
                    track.suppressed = true;
                } else {
                    use crate::turn::ThoughtTrack;
                    use tokio::time::Instant;
                    turn.thought = Some(ThoughtTrack {
                        msg_id: 0,
                        started_at: Instant::now(),
                        buffer: String::new(),
                        debounce_handle: None,
                        suppressed: true,
                    });
                }
            }

            let bot_clone = bot.clone();
            let state_clone = Arc::clone(state);
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let final_data = {
                    let mut turns = state_clone.turns.write().await;
                    turns.get_mut(&chat_id).and_then(|t| t.take_response_for_finalize())
                };
                if let Some((text, msg_id)) = final_data {
                    if !text.is_empty() && msg_id != 0 {
                        let final_chunks = split_message(&text, 4096);
                        if let Err(e) = bot_clone
                            .edit_message_text(ChatId(chat_id), MessageId(msg_id), &final_chunks[0])
                            .await
                        {
                            tracing::warn!(%e, chat_id, "failed to finalize message edit");
                        }
                        for chunk in final_chunks.iter().skip(1) {
                            if let Err(e) = bot_clone.send_message(ChatId(chat_id), chunk).await {
                                tracing::warn!(%e, chat_id, "failed to send overflow chunk");
                            }
                        }
                    }
                }
                state_clone.turns.write().await.remove(&chat_id);
            });

            let mut turns = state.turns.write().await;
            if let Some(turn) = turns.get_mut(&chat_id) {
                turn.begin_finalizing(handle);
            }

            return Ok(());
        }

        "user_message_chunk" => {
            return Ok(());
        }

        _ => {}
    }

    let text = update_obj
        .and_then(|u| u.get("content"))
        .map(content_to_string)
        .unwrap_or_default();

    if text.is_empty() {
        state.turns.write().await.remove(&chat_id);
        return Ok(());
    }

    let chunks = split_message(&text, 4096);

    for (i, chunk) in chunks.iter().enumerate() {
        let existing_msg_id = if i == 0 {
            state
                .turns
                .read()
                .await
                .get(&chat_id)
                .and_then(|t| t.response.as_ref())
                .map(|r| r.msg_id)
                .filter(|&id| id != 0)
        } else {
            None
        };

        if let Some(msg_id) = existing_msg_id {
            let can_edit = {
                let mut turns = state.turns.write().await;
                turns
                    .get_mut(&chat_id)
                    .map(|t| t.can_edit_response())
                    .unwrap_or(false)
            };
            if can_edit {
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
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns.entry(chat_id).or_insert_with(|| ChatTurn::new(mid));
                if turn.response.is_none() {
                    turn.append_response("", sent.id.0);
                } else if let Some(track) = turn.response.as_mut() {
                    track.msg_id = sent.id.0;
                }
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
    use std::sync::Arc;

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
        let state = Arc::new(SharedState::new());
        let bot = Bot::new("test-token");
        let content = serde_json::json!("hello");
        let result = deliver_to_chat(&bot, &state, "unknown-session", &content).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown session"));
    }

    #[tokio::test]
    async fn empty_content_clears_active_message() {
        let state = Arc::new(SharedState::new());
        state
            .session_chat_map
            .write()
            .await
            .insert("sess-1".into(), 12345);

        {
            let mut turns = state.turns.write().await;
            let mut turn = ChatTurn::new("msg-1".to_string());
            turn.append_response("some text", 99);
            turns.insert(12345, turn);
        }

        let bot = Bot::new("test-token");
        let content = serde_json::json!("");
        let result = deliver_to_chat(&bot, &state, "sess-1", &content).await;
        assert!(result.is_ok());
        assert!(state.turns.read().await.get(&12345).is_none());
    }
}
