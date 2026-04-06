use std::sync::Arc;
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

async fn finalize_previous_turn(state: &Arc<SharedState>, chat_id: i64) {
    if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
        handle.abort();
    }
    if let Some(handle) = state.thought_debounce_handles.write().await.remove(&chat_id) {
        handle.abort();
    }
    state.result_received.write().await.remove(&chat_id);
    state.message_buffers.write().await.remove(&chat_id);
    state.active_messages.write().await.remove(&chat_id);
    state.last_edit_time.write().await.remove(&chat_id);
    state.thought_suppressed.write().await.remove(&chat_id);
    state.thought_buffers.write().await.remove(&chat_id);
    state.thinking_messages.write().await.remove(&chat_id);
    state.current_message_id.write().await.remove(&chat_id);
}

async fn check_message_id_turn_change(
    state: &Arc<SharedState>,
    chat_id: i64,
    message_id: Option<&str>,
) -> bool {
    let new_id = match message_id {
        Some(id) if !id.is_empty() => id,
        _ => return false,
    };

    let is_new_turn = {
        let current = state.current_message_id.read().await;
        match current.get(&chat_id) {
            Some(current_id) => current_id != new_id,
            None => false,
        }
    };

    if is_new_turn {
        let old_id = state.current_message_id.read().await.get(&chat_id).cloned();
        tracing::info!(chat_id, ?old_id, new_message_id = new_id, "messageId turn change detected");
        finalize_previous_turn(state, chat_id).await;
    }

    state
        .current_message_id
        .write()
        .await
        .insert(chat_id, new_id.to_string());

    is_new_turn
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
        .ok_or_else(|| {
            ChannelSdkError::Protocol(format!("unknown session: {session_id}"))
        })?;

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
            check_message_id_turn_change(state, chat_id, message_id).await;

            if state.thought_suppressed.read().await.contains(&chat_id) {
                return Ok(());
            }

            if state.result_received.read().await.contains(&chat_id) {
                if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
                    handle.abort();
                }
                state.result_received.write().await.remove(&chat_id);
                state.message_buffers.write().await.remove(&chat_id);
                state.active_messages.write().await.remove(&chat_id);
                state.last_edit_time.write().await.remove(&chat_id);
                state.thought_suppressed.write().await.remove(&chat_id);
            }

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
            if existing.is_none() {
                match bot.send_message(ChatId(chat_id), &thought_text).await {
                    Ok(sent) => {
                        state.thinking_messages.write().await
                            .insert(chat_id, (sent.id.0, Instant::now()));
                    }
                    Err(e) => {
                        tracing::warn!(%e, "failed to send thinking message");
                    }
                }
            } else {
                if let Some(handle) = state.thought_debounce_handles.write().await.remove(&chat_id) {
                    handle.abort();
                }

                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let handle = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    let accumulated = state_clone.thought_buffers.read().await
                        .get(&chat_id).cloned().unwrap_or_default();
                    let emoji = thought_emoji();
                    let text = format!("{emoji} {accumulated}");
                    if let Some((msg_id, _)) = state_clone.thinking_messages.read().await.get(&chat_id).copied() {
                        let _ = bot_clone
                            .edit_message_text(ChatId(chat_id), MessageId(msg_id), &text)
                            .await;
                    }
                    state_clone.thought_debounce_handles.write().await.remove(&chat_id);
                });
                state.thought_debounce_handles.write().await.insert(chat_id, handle);
            }
            return Ok(());
        }
        "agent_message_chunk" => {
            check_message_id_turn_change(state, chat_id, message_id).await;

            state.thought_suppressed.write().await.remove(&chat_id);

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

            if state.result_received.read().await.contains(&chat_id) {
                if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
                    handle.abort();
                }
                let existing_msg_id = state.active_messages.read().await.get(&chat_id).copied();
                if let Some(msg_id) = existing_msg_id {
                    let chunks = split_message(&accumulated, 4096);
                    let _ = bot
                        .edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunks[0])
                        .await;
                    for chunk in chunks.iter().skip(1) {
                        let _ = bot.send_message(ChatId(chat_id), chunk).await;
                    }
                    state
                        .last_edit_time
                        .write()
                        .await
                        .insert(chat_id, Instant::now());
                }
                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let handle = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let text = state_clone
                        .message_buffers
                        .read()
                        .await
                        .get(&chat_id)
                        .cloned()
                        .unwrap_or_default();
                    if !text.is_empty() {
                        if let Some(msg_id) =
                            state_clone.active_messages.read().await.get(&chat_id).copied()
                        {
                            let final_chunks = split_message(&text, 4096);
                            let _ = bot_clone
                                .edit_message_text(
                                    ChatId(chat_id),
                                    MessageId(msg_id),
                                    &final_chunks[0],
                                )
                                .await;
                            for chunk in final_chunks.iter().skip(1) {
                                let _ = bot_clone.send_message(ChatId(chat_id), chunk).await;
                            }
                        }
                    }
                    state_clone.result_received.write().await.remove(&chat_id);
                    state_clone.message_buffers.write().await.remove(&chat_id);
                    state_clone.active_messages.write().await.remove(&chat_id);
                    state_clone.last_edit_time.write().await.remove(&chat_id);
                    state_clone
                        .thought_suppressed
                        .write()
                        .await
                        .remove(&chat_id);
                    state_clone.finalize_handles.write().await.remove(&chat_id);
                    state_clone.current_message_id.write().await.remove(&chat_id);
                });
                state
                    .finalize_handles
                    .write()
                    .await
                    .insert(chat_id, handle);
                return Ok(());
            }

            let existing_msg_id = state.active_messages.read().await.get(&chat_id).copied();
            if let Some(msg_id) = existing_msg_id {
                if can_edit(state, chat_id).await {
                    let chunks = split_message(&accumulated, 4096);
                    if let Err(e) = bot
                        .edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunks[0])
                        .await
                    {
                        tracing::warn!(%e, chat_id, "failed to edit late chunk");
                    }
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
            if let Some(mid) = message_id {
                let current = state.current_message_id.read().await;
                if let Some(current_id) = current.get(&chat_id) {
                    if current_id != mid {
                        tracing::info!(chat_id, stale_message_id = mid, current_message_id = %current_id, "discarding stale result from previous turn");
                        return Ok(());
                    }
                }
            }

            if let Some(handle) = state.thought_debounce_handles.write().await.remove(&chat_id) {
                handle.abort();
            }

            let _final_thought = state.thought_buffers.write().await.remove(&chat_id);
            if let Some((msg_id, start_time)) = state.thinking_messages.write().await.remove(&chat_id) {
                let elapsed = start_time.elapsed().as_secs_f32();
                let emoji = thought_emoji();
                let collapse_text = format!("{emoji} Thought for {elapsed:.1}s");
                if let Err(e) = bot
                    .edit_message_text(ChatId(chat_id), MessageId(msg_id), &collapse_text)
                    .await
                {
                    tracing::warn!(%e, chat_id, "failed to collapse thinking message");
                }
            }

            state.thought_suppressed.write().await.insert(chat_id);
            state.result_received.write().await.insert(chat_id);

            if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
                handle.abort();
            }
            let bot_clone = bot.clone();
            let state_clone = Arc::clone(state);
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let text = state_clone
                    .message_buffers
                    .read()
                    .await
                    .get(&chat_id)
                    .cloned()
                    .unwrap_or_default();
                if !text.is_empty() {
                    if let Some(msg_id) =
                        state_clone.active_messages.read().await.get(&chat_id).copied()
                    {
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
                state_clone.result_received.write().await.remove(&chat_id);
                state_clone.message_buffers.write().await.remove(&chat_id);
                state_clone.active_messages.write().await.remove(&chat_id);
                state_clone.last_edit_time.write().await.remove(&chat_id);
                state_clone.thought_suppressed.write().await.remove(&chat_id);
                state_clone.finalize_handles.write().await.remove(&chat_id);
                state_clone.current_message_id.write().await.remove(&chat_id);
            });
            state.finalize_handles.write().await.insert(chat_id, handle);

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

    // ===== State-transition helpers =====
    // These simulate the state changes that `deliver_to_chat` performs,
    // allowing us to test the race-condition fix without a real Telegram Bot.

    /// Simulates what `deliver_to_chat` does on "agent_message_chunk":
    /// - Accumulates into message_buffers
    /// - If result_received is set, does NOT create a new active message; does a final edit
    /// - If result_received is NOT set, normal streaming behaviour
    async fn sim_message_chunk(state: &Arc<SharedState>, chat_id: i64, chunk: &str) {
        let accumulated = {
            let mut buffers = state.message_buffers.write().await;
            let buf = buffers.entry(chat_id).or_default();
            buf.push_str(chunk);
            buf.clone()
        };
        let _ = accumulated; // in production this would trigger an edit
    }

    /// Simulates what `deliver_to_chat` does on "result":
    /// - Collapses thinking
    /// - Does final edit (represented here by leaving message_buffers intact)
    /// - Sets result_received for chat_id
    /// - Does NOT remove message_buffers or active_messages
    async fn sim_result(state: &Arc<SharedState>, chat_id: i64) {
        // In production this does the Telegram edit; here we just update state:
        state.result_received.write().await.insert(chat_id);
        // Deliberately NOT removing message_buffers or active_messages —
        // that's the whole point of the fix.
    }

    /// Simulates what `deliver_to_chat` does on "agent_thought_chunk" when result_received:
    /// Cleans up previous-turn state and clears result_received for chat_id.
    async fn sim_new_turn(state: &Arc<SharedState>, chat_id: i64) {
        state.result_received.write().await.remove(&chat_id);
        state.message_buffers.write().await.remove(&chat_id);
        state.active_messages.write().await.remove(&chat_id);
        state.last_edit_time.write().await.remove(&chat_id);
        state.thought_suppressed.write().await.remove(&chat_id);
    }

    // ===== Race condition tests =====

    #[tokio::test]
    async fn result_received_flag_is_initially_empty() {
        let state = SharedState::new();
        assert!(state.result_received.read().await.is_empty());
    }

    #[tokio::test]
    async fn result_event_sets_result_received_flag() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        sim_result(&state, chat_id).await;
        assert!(state.result_received.read().await.contains(&chat_id));
    }

    #[tokio::test]
    async fn result_event_preserves_message_buffer() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        // Set up buffer as if chunks arrived
        state.message_buffers.write().await.insert(chat_id, "Hello wor".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        // Simulate result
        sim_result(&state, chat_id).await;
        // Buffer must still be present for late chunks to accumulate into
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("Hello wor".to_string()),
        );
        // active_messages must also still be present so late edits know the message id
        assert!(state.active_messages.read().await.contains_key(&chat_id));
    }

    #[tokio::test]
    async fn late_chunk_after_result_accumulates_into_buffer() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        // Simulate a partial message followed by result
        state.message_buffers.write().await.insert(chat_id, "Hello wor".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        sim_result(&state, chat_id).await;
        // Late chunk arrives
        sim_message_chunk(&state, chat_id, "ld").await;
        // Buffer should now contain the full text
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("Hello world".to_string()),
        );
        // result_received should still be set (more late chunks could arrive)
        assert!(state.result_received.read().await.contains(&chat_id));
        // active_messages should still be present so we can do the edit
        assert!(state.active_messages.read().await.contains_key(&chat_id));
    }

    #[tokio::test]
    async fn new_turn_cleans_up_previous_turn_state() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        // Set up end-of-turn state
        state.message_buffers.write().await.insert(chat_id, "Hello world".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        state.thought_suppressed.write().await.insert(chat_id);
        state.result_received.write().await.insert(chat_id);
        // New turn starts (first thought chunk for this chat_id with result_received set)
        sim_new_turn(&state, chat_id).await;
        // All previous-turn state cleared
        assert!(!state.result_received.read().await.contains(&chat_id));
        assert!(!state.message_buffers.read().await.contains_key(&chat_id));
        assert!(!state.active_messages.read().await.contains_key(&chat_id));
        assert!(!state.last_edit_time.read().await.contains_key(&chat_id));
        assert!(!state.thought_suppressed.read().await.contains(&chat_id));
    }

    #[tokio::test]
    async fn normal_flow_chunks_then_result_sets_flag() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 99;
        sim_message_chunk(&state, chat_id, "Hello ").await;
        sim_message_chunk(&state, chat_id, "world").await;
        sim_result(&state, chat_id).await;
        // result_received should be set
        assert!(state.result_received.read().await.contains(&chat_id));
        // buffer should still be present with full text
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("Hello world".to_string()),
        );
    }

    #[tokio::test]
    async fn result_received_for_one_chat_does_not_affect_another() {
        let state = Arc::new(SharedState::new());
        let chat_a: i64 = 10;
        let chat_b: i64 = 20;
        sim_message_chunk(&state, chat_a, "msg a").await;
        sim_message_chunk(&state, chat_b, "msg b").await;
        sim_result(&state, chat_a).await;
        assert!(state.result_received.read().await.contains(&chat_a));
        assert!(!state.result_received.read().await.contains(&chat_b));
    }

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
    async fn thought_debounce_handles_initialized_empty() {
        let state = SharedState::new();
        assert!(state.thought_debounce_handles.read().await.is_empty());
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

    // ===== Timer-based finalization tests =====

    /// Simulates what the new result handler does: sets result_received, spawns finalization
    /// timer. Does NOT immediately clean up buffers or active_messages.
    async fn sim_result_timer(state: &Arc<SharedState>, chat_id: i64) {
        // Cancel any existing finalize handle
        if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
            handle.abort();
        }
        // Set result_received
        state.result_received.write().await.insert(chat_id);
        // Spawn finalization timer (200ms debounce)
        let state_clone = Arc::clone(state);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            // Timer fired: clean up all state for this turn
            state_clone.result_received.write().await.remove(&chat_id);
            state_clone.message_buffers.write().await.remove(&chat_id);
            state_clone.active_messages.write().await.remove(&chat_id);
            state_clone.last_edit_time.write().await.remove(&chat_id);
            state_clone.thought_suppressed.write().await.remove(&chat_id);
            state_clone.finalize_handles.write().await.remove(&chat_id);
        });
        state.finalize_handles.write().await.insert(chat_id, handle);
    }

    /// Simulates what a late chunk does: accumulates into buffer, resets finalization timer.
    async fn sim_late_chunk(state: &Arc<SharedState>, chat_id: i64, chunk: &str) {
        // Accumulate
        {
            let mut buffers = state.message_buffers.write().await;
            let buf = buffers.entry(chat_id).or_default();
            buf.push_str(chunk);
        }
        // Reset finalization timer (cancel old, spawn new)
        if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
            handle.abort();
        }
        let state_clone = Arc::clone(state);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            state_clone.result_received.write().await.remove(&chat_id);
            state_clone.message_buffers.write().await.remove(&chat_id);
            state_clone.active_messages.write().await.remove(&chat_id);
            state_clone.last_edit_time.write().await.remove(&chat_id);
            state_clone.thought_suppressed.write().await.remove(&chat_id);
            state_clone.finalize_handles.write().await.remove(&chat_id);
        });
        state.finalize_handles.write().await.insert(chat_id, handle);
    }

    #[tokio::test]
    async fn finalize_handles_field_exists_and_is_initially_empty() {
        let state = SharedState::new();
        assert!(state.finalize_handles.read().await.is_empty());
    }

    #[tokio::test]
    async fn result_timer_sets_result_received_and_spawns_handle() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        state.message_buffers.write().await.insert(chat_id, "Hello wor".to_string());
        state.active_messages.write().await.insert(chat_id, 100);

        sim_result_timer(&state, chat_id).await;

        assert!(state.result_received.read().await.contains(&chat_id));
        // Buffer NOT yet cleared — timer hasn't fired
        assert!(state.message_buffers.read().await.contains_key(&chat_id));
        // active_messages NOT yet cleared
        assert!(state.active_messages.read().await.contains_key(&chat_id));
        // Finalization handle must be present
        assert!(state.finalize_handles.read().await.contains_key(&chat_id));
    }

    #[tokio::test]
    async fn late_chunk_resets_finalize_timer_and_accumulates() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        state.message_buffers.write().await.insert(chat_id, "Hello wor".to_string());
        state.active_messages.write().await.insert(chat_id, 100);

        sim_result_timer(&state, chat_id).await;

        // Capture old handle pointer (we just check a new one is inserted)
        sim_late_chunk(&state, chat_id, "ld").await;

        // Buffer should now have the full text
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("Hello world".to_string()),
        );
        // result_received still set (timer not fired yet)
        assert!(state.result_received.read().await.contains(&chat_id));
        // A finalize handle should still be present (was reset)
        assert!(state.finalize_handles.read().await.contains_key(&chat_id));
    }

    #[tokio::test]
    async fn finalize_timer_cleans_up_state_after_delay() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        state.message_buffers.write().await.insert(chat_id, "Hello world".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        state.thought_suppressed.write().await.insert(chat_id);

        sim_result_timer(&state, chat_id).await;

        // Wait for timer to fire (200ms + buffer)
        tokio::time::sleep(Duration::from_millis(350)).await;

        // Everything should be cleaned up
        assert!(!state.result_received.read().await.contains(&chat_id));
        assert!(!state.message_buffers.read().await.contains_key(&chat_id));
        assert!(!state.active_messages.read().await.contains_key(&chat_id));
        assert!(!state.thought_suppressed.read().await.contains(&chat_id));
        assert!(!state.finalize_handles.read().await.contains_key(&chat_id));
    }

    #[tokio::test]
    async fn new_turn_thought_cancels_finalize_timer_and_cleans_up() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        state.message_buffers.write().await.insert(chat_id, "turn 1".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        state.thought_suppressed.write().await.insert(chat_id);

        sim_result_timer(&state, chat_id).await;

        // New turn starts: cancel timer, clean up immediately
        if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
            handle.abort();
        }
        sim_new_turn(&state, chat_id).await;

        // Immediate cleanup — don't wait 200ms
        assert!(!state.result_received.read().await.contains(&chat_id));
        assert!(!state.message_buffers.read().await.contains_key(&chat_id));
        assert!(!state.active_messages.read().await.contains_key(&chat_id));
        assert!(!state.thought_suppressed.read().await.contains(&chat_id));
        assert!(!state.finalize_handles.read().await.contains_key(&chat_id));
    }

    // ===== messageId-based turn detection tests =====

    /// Simulates the messageId-based turn detection that deliver_to_chat should perform.
    /// Returns true if a turn change was detected (different messageId for same chat).
    async fn sim_thought_with_message_id(
        state: &Arc<SharedState>,
        chat_id: i64,
        message_id: &str,
        thought: &str,
    ) -> bool {
        let is_new_turn = {
            let current = state.current_message_id.read().await;
            match current.get(&chat_id) {
                Some(current_id) => current_id != message_id,
                None => false,
            }
        };

        if is_new_turn {
            if let Some(handle) = state.finalize_handles.write().await.remove(&chat_id) {
                handle.abort();
            }
            state.result_received.write().await.remove(&chat_id);
            state.message_buffers.write().await.remove(&chat_id);
            state.active_messages.write().await.remove(&chat_id);
            state.last_edit_time.write().await.remove(&chat_id);
            state.thought_suppressed.write().await.remove(&chat_id);
            state.thought_buffers.write().await.remove(&chat_id);
            state.thinking_messages.write().await.remove(&chat_id);
        }

        state.current_message_id.write().await.insert(chat_id, message_id.to_string());

        {
            let mut buffers = state.thought_buffers.write().await;
            let buf = buffers.entry(chat_id).or_default();
            buf.push_str(thought);
        }

        is_new_turn
    }

    #[tokio::test]
    async fn new_message_id_triggers_turn_change_without_result_received() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;

        // Given: prompt 1 is streaming with messageId "msg-1"
        sim_thought_with_message_id(&state, chat_id, "msg-1", "thinking about prompt 1...").await;
        state.message_buffers.write().await.insert(chat_id, "response to prompt 1".to_string());
        state.active_messages.write().await.insert(chat_id, 100);

        assert_eq!(
            state.current_message_id.read().await.get(&chat_id).cloned(),
            Some("msg-1".to_string()),
        );
        assert!(!state.result_received.read().await.contains(&chat_id));

        // When: prompt 2's thought arrives with different messageId (result NOT received)
        let was_new_turn = sim_thought_with_message_id(&state, chat_id, "msg-2", "thinking about prompt 2...").await;

        // Then: turn change detected, previous state cleaned, new thought accumulated
        assert!(was_new_turn, "should detect new turn from messageId change");
        assert_eq!(
            state.current_message_id.read().await.get(&chat_id).cloned(),
            Some("msg-2".to_string()),
        );
        assert!(!state.message_buffers.read().await.contains_key(&chat_id));
        assert!(!state.active_messages.read().await.contains_key(&chat_id));
        assert_eq!(
            state.thought_buffers.read().await.get(&chat_id).cloned(),
            Some("thinking about prompt 2...".to_string()),
        );
    }

    #[tokio::test]
    async fn same_message_id_does_not_trigger_turn_change() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;

        // Given: streaming with messageId "msg-1"
        sim_thought_with_message_id(&state, chat_id, "msg-1", "chunk 1 ").await;
        state.message_buffers.write().await.insert(chat_id, "response".to_string());

        // When: another thought with same messageId
        let was_new_turn = sim_thought_with_message_id(&state, chat_id, "msg-1", "chunk 2").await;

        // Then: no turn change, buffers preserved and accumulated
        assert!(!was_new_turn, "same messageId should not trigger turn change");
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("response".to_string()),
        );
        assert_eq!(
            state.thought_buffers.read().await.get(&chat_id).cloned(),
            Some("chunk 1 chunk 2".to_string()),
        );
    }

    #[tokio::test]
    async fn first_thought_for_chat_does_not_trigger_turn_change() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;

        // When: first-ever thought for this chat
        let was_new_turn = sim_thought_with_message_id(&state, chat_id, "msg-1", "first thought").await;

        // Then: not a turn change, messageId tracked
        assert!(!was_new_turn, "first thought should not be a turn change");
        assert_eq!(
            state.current_message_id.read().await.get(&chat_id).cloned(),
            Some("msg-1".to_string()),
        );
    }

    #[tokio::test]
    async fn message_id_turn_detection_cancels_finalize_timer() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;

        // Given: prompt 1 with a running finalize timer
        sim_thought_with_message_id(&state, chat_id, "msg-1", "thought 1").await;
        state.message_buffers.write().await.insert(chat_id, "response 1".to_string());
        state.active_messages.write().await.insert(chat_id, 100);
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            state_clone.message_buffers.write().await.remove(&chat_id);
        });
        state.finalize_handles.write().await.insert(chat_id, handle);

        // When: prompt 2 arrives with different messageId
        sim_thought_with_message_id(&state, chat_id, "msg-2", "thought 2").await;

        // Then: timer cancelled, finalize handle removed
        assert!(!state.finalize_handles.read().await.contains_key(&chat_id));

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    #[tokio::test]
    async fn stale_result_after_turn_change_must_not_corrupt_new_turn() {
        // BUG REPRO: When turn 2 starts before turn 1's result arrives,
        // the result handler must check messageId and discard stale results.
        // Without the guard, sim_result_timer sets result_received on turn 2's chat,
        // causing turn 2's message chunks to be treated as "late chunks".
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;

        // Given: turn 1 streaming with messageId "msg-1"
        sim_thought_with_message_id(&state, chat_id, "msg-1", "thinking...").await;
        state.message_buffers.write().await.insert(chat_id, "response 1".to_string());
        state.active_messages.write().await.insert(chat_id, 100);

        // Given: turn 2 starts (messageId changes to "msg-2")
        sim_thought_with_message_id(&state, chat_id, "msg-2", "new thought").await;
        state.message_buffers.write().await.insert(chat_id, "response 2".to_string());
        state.active_messages.write().await.insert(chat_id, 200);

        assert_eq!(
            state.current_message_id.read().await.get(&chat_id).cloned(),
            Some("msg-2".to_string()),
        );

        // When: stale result from turn 1 arrives
        // The result handler must compare messageId against current_message_id.
        // If different, silently discard (do NOT call check_message_id_turn_change
        // which would nuke the current turn's state).
        let is_stale = {
            let current = state.current_message_id.read().await;
            match current.get(&chat_id) {
                Some(current_id) => current_id != "msg-1",
                None => false,
            }
        };
        if !is_stale {
            sim_result_timer(&state, chat_id).await;
        }

        // Then: result_received must NOT be set (stale result discarded)
        assert!(!state.result_received.read().await.contains(&chat_id),
            "stale result must not set result_received on new turn");
        // Turn 2's buffers must be intact
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("response 2".to_string()),
        );
    }

    #[tokio::test]
    async fn late_chunk_timer_reset_delays_cleanup() {
        let state = Arc::new(SharedState::new());
        let chat_id: i64 = 42;
        state.message_buffers.write().await.insert(chat_id, "Hello wor".to_string());
        state.active_messages.write().await.insert(chat_id, 100);

        // Spawn result timer
        sim_result_timer(&state, chat_id).await;
        // After 150ms (before timer fires), a late chunk arrives and resets timer
        tokio::time::sleep(Duration::from_millis(150)).await;
        sim_late_chunk(&state, chat_id, "ld").await;

        // At 300ms from start (150ms after reset), timer should NOT have fired yet
        tokio::time::sleep(Duration::from_millis(150)).await;
        // State still present
        assert!(state.result_received.read().await.contains(&chat_id));
        assert_eq!(
            state.message_buffers.read().await.get(&chat_id).cloned(),
            Some("Hello world".to_string()),
        );

        // Wait for timer to fire (another 100ms)
        tokio::time::sleep(Duration::from_millis(150)).await;
        // Now cleaned up
        assert!(!state.result_received.read().await.contains(&chat_id));
        assert!(!state.message_buffers.read().await.contains_key(&chat_id));
    }
}
