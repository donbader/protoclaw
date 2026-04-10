use std::sync::Arc;
use std::time::Duration;

use protoclaw_sdk_channel::ChannelSdkError;
use protoclaw_sdk_types::ContentKind;
use teloxide::payloads::{EditMessageTextSetters, SendMessageSetters};
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId, ParseMode};
use tokio::time::Instant;

use crate::formatting::{close_open_tags, escape_html, format_telegram_html};
use crate::state::SharedState;
use crate::turn::{ChatTurn, TurnPhase};

struct PendingFlush {
    response: Option<(String, i32)>,
    thought_collapse: Option<(i32, f32)>,
}

fn flush_turn_data(turn: &mut ChatTurn) -> PendingFlush {
    let response = turn.take_response_for_finalize();
    let thought_collapse = turn.collapse_thought();
    turn.cleanup();
    PendingFlush {
        response,
        thought_collapse,
    }
}

async fn send_flush(bot: &Bot, state: &Arc<SharedState>, chat_id: i64, flush: &PendingFlush) {
    if let Some((ref text, msg_id)) = flush.response {
        if !text.is_empty() && msg_id != 0 {
            tracing::debug!(chat_id, msg_id, buf_len = text.len(), "flush: sending final edit before cleanup");
            let formatted = format_telegram_html(text);
            let chunks = split_message(&formatted, 4096);
            if let Err(e) = bot
                .edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunks[0])
                .parse_mode(ParseMode::Html)
                .await
            {
                tracing::warn!(%e, chat_id, msg_id, "flush: final edit failed");
            }
            for chunk in chunks.iter().skip(1) {
                let _ = bot.send_message(ChatId(chat_id), chunk)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        } else {
            tracing::debug!(chat_id, msg_id, buf_empty = text.is_empty(), "flush: skipping final edit (empty or no msg_id)");
        }
    } else {
        tracing::debug!(chat_id, "flush: no response to send");
    }
    if let Some((thought_msg_id, elapsed_secs)) = flush.thought_collapse {
        if thought_msg_id != 0 {
            let emoji = thought_emoji(state).await;
            let collapse_text = format!("{emoji} Thought for {elapsed_secs:.1}s");
            let _ = bot
                .edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &collapse_text)
                .parse_mode(ParseMode::Html)
                .await;
        }
    }
}

pub fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        let mut boundary = max_len;
        while boundary > 0 && !remaining.is_char_boundary(boundary) {
            boundary -= 1;
        }
        if boundary == 0 {
            boundary = max_len;
        }

        // Never split inside a <pre> block — walk forward to </pre> if needed
        let candidate = &remaining[..boundary];
        let open_count = candidate.matches("<pre>").count() + candidate.matches("<pre ").count();
        let close_count = candidate.matches("</pre>").count();
        if open_count > close_count {
            if let Some(close_pos) = remaining.find("</pre>") {
                let end = close_pos + "</pre>".len();
                if end <= remaining.len() {
                    boundary = end;
                }
            }
        }

        if boundary <= max_len {
            let candidate = &remaining[..boundary];
            if let Some(pos) = candidate.rfind("\n\n") {
                boundary = pos + 2;
            } else if let Some(pos) = candidate.rfind('\n') {
                boundary = pos + 1;
            } else if let Some(pos) = candidate.rfind(". ") {
                boundary = pos + 2;
            }
        }

        chunks.push(remaining[..boundary].to_string());
        remaining = &remaining[boundary..];
    }
    chunks
}

async fn thought_emoji(state: &SharedState) -> String {
    state.thought_emoji.read().await.clone()
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
    let kind = ContentKind::from_content(content);
    let message_id = update_obj
        .and_then(|u| u.get("messageId"))
        .and_then(|v| v.as_str());

    let origin_instant = content.get("_received_at_ms")
        .and_then(|v| v.as_u64())
        .and_then(|received_ms| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let age = Duration::from_millis(now_ms.saturating_sub(received_ms));
            Instant::now().checked_sub(age)
        });

    tracing::debug!(chat_id, ?kind, message_id, "deliver_to_chat");

    match kind {
        ContentKind::Thought(thought) => {
            {
                let flush = {
                    let mut turns = state.turns.write().await;
                    if let Some(mid) = message_id {
                        if let Some(turn) = turns.get_mut(&chat_id) {
                            if turn.is_different_turn(mid) {
                                tracing::info!(chat_id, old_message_id = %turn.message_id, new_message_id = mid, "messageId turn change detected in thought");
                                let flush = flush_turn_data(turn);
                                turns.remove(&chat_id);
                                Some(flush)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(flush) = &flush {
                    send_flush(bot, state, chat_id, flush).await;
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
                let flush = {
                    let mut turns = state.turns.write().await;
                    let is_finalizing = turns
                        .get(&chat_id)
                        .map(|t| matches!(t.phase, TurnPhase::Finalizing(_)))
                        .unwrap_or(false);
                    if is_finalizing {
                        let flush = turns.get_mut(&chat_id).map(flush_turn_data);
                        turns.remove(&chat_id);
                        flush
                    } else {
                        None
                    }
                };
                if let Some(flush) = &flush {
                    send_flush(bot, state, chat_id, flush).await;
                }
            }

            let thought_content = thought.content;

            let (accumulated, existing_thought_msg_id) = {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns.entry(chat_id).or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                turn.append_thought(&thought_content, 0, origin_instant);
                let track = turn.thought.as_ref().unwrap();
                let accumulated = track.buffer.clone();
                let existing_thought_msg_id = track.msg_id;
                if let Some(ref h) = track.debounce_handle {
                    h.abort();
                }
                turn.thought.as_mut().unwrap().debounce_handle = None;
                (accumulated, existing_thought_msg_id)
            };

            let emoji = thought_emoji(state).await;
            let thought_text = format!("{emoji} {}", escape_html(&accumulated));

            if existing_thought_msg_id == 0 {
                match bot.send_message(ChatId(chat_id), &thought_text)
                    .parse_mode(ParseMode::Html)
                    .await
                {
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
                    let debounce_ms = *state_clone.thought_debounce_ms.read().await;
                    tokio::time::sleep(Duration::from_millis(debounce_ms)).await;
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
                    let emoji = thought_emoji(&state_clone).await;
                    let text = format!("{emoji} {}", escape_html(&accumulated));
                    if thought_msg_id != 0 {
                        let _ = bot_clone
                            .edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &text)
                            .parse_mode(ParseMode::Html)
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
            Ok(())
        }

        ContentKind::MessageChunk { text: chunk_text } => {
            {
                let flush = {
                    let mut turns = state.turns.write().await;
                    if let Some(mid) = message_id {
                        if let Some(turn) = turns.get_mut(&chat_id) {
                            if turn.is_different_turn(mid) {
                                tracing::info!(chat_id, old_message_id = %turn.message_id, new_message_id = mid, "messageId turn change detected in message chunk");
                                let flush = flush_turn_data(turn);
                                turns.remove(&chat_id);
                                Some(flush)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(flush) = &flush {
                    send_flush(bot, state, chat_id, flush).await;
                }
            }

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
                turn.append_response(&chunk_text, 0);
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
                    let formatted = format_telegram_html(&accumulated);
                    let chunks = split_message(&formatted, 4096);
                    let _ = bot
                        .edit_message_text(
                            ChatId(chat_id),
                            MessageId(existing_response_msg_id),
                            &chunks[0],
                        )
                        .parse_mode(ParseMode::Html)
                        .await;
                    for chunk in chunks.iter().skip(1) {
                        let _ = bot.send_message(ChatId(chat_id), chunk)
                            .parse_mode(ParseMode::Html)
                            .await;
                    }
                }

                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let new_handle = tokio::spawn(async move {
                    let delay_ms = *state_clone.finalization_delay_ms.read().await;
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    let final_data = {
                        let mut turns = state_clone.turns.write().await;
                        turns.get_mut(&chat_id).and_then(|t| t.take_response_for_finalize())
                    };
                    if let Some((text, msg_id)) = final_data {
                        if !text.is_empty() && msg_id != 0 {
                            let formatted = format_telegram_html(&text);
                            let final_chunks = split_message(&formatted, 4096);
                            if let Err(e) = bot_clone
                                .edit_message_text(
                                    ChatId(chat_id),
                                    MessageId(msg_id),
                                    &final_chunks[0],
                                )
                                .parse_mode(ParseMode::Html)
                                .await
                            {
                                tracing::warn!(%e, chat_id, "failed to finalize message edit (late)");
                            }
                            for chunk in final_chunks.iter().skip(1) {
                                if let Err(e) =
                                    bot_clone.send_message(ChatId(chat_id), chunk)
                                        .parse_mode(ParseMode::Html)
                                        .await
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
                    let cooldown = Duration::from_millis(*state.response_edit_cooldown_ms.read().await);
                    let mut turns = state.turns.write().await;
                    turns
                        .get_mut(&chat_id)
                        .map(|t| t.can_edit_response(cooldown))
                        .unwrap_or(false)
                };
                if can_edit {
                    let formatted = close_open_tags(&format_telegram_html(&accumulated));
                    let chunks = split_message(&formatted, 4096);
                    if let Err(e) = bot
                        .edit_message_text(
                            ChatId(chat_id),
                            MessageId(existing_response_msg_id),
                            &chunks[0],
                        )
                        .parse_mode(ParseMode::Html)
                        .await
                    {
                        tracing::warn!(%e, chat_id, "failed to edit message chunk");
                    }
                }
            } else {
                let formatted = close_open_tags(&format_telegram_html(&accumulated));
                let chunks = split_message(&formatted, 4096);
                match bot.send_message(ChatId(chat_id), &chunks[0])
                    .parse_mode(ParseMode::Html)
                    .await
                {
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
            Ok(())
        }

        ContentKind::Result { .. } => {
            {
                let turns = state.turns.read().await;
                if let Some(turn) = turns.get(&chat_id) {
                    let has_response = turn.response.is_some();
                    let response_msg_id = turn.response.as_ref().map(|r| r.msg_id).unwrap_or(-1);
                    let response_buf_len = turn.response.as_ref().map(|r| r.buffer.len()).unwrap_or(0);
                    let has_thought = turn.thought.is_some();
                    let phase = match &turn.phase {
                        TurnPhase::Active => "active",
                        TurnPhase::Finalizing(_) => "finalizing",
                    };
                    tracing::debug!(
                        chat_id,
                        turn_message_id = %turn.message_id,
                        has_response,
                        response_msg_id,
                        response_buf_len,
                        has_thought,
                        phase,
                        "result received: turn state snapshot"
                    );
                } else {
                    tracing::debug!(chat_id, "result received: no turn exists for chat");
                }

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
                    let emoji = thought_emoji(state).await;
                    let collapse_text = format!("{emoji} Thought for {elapsed_secs:.1}s");
                    if let Err(e) = bot
                        .edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &collapse_text)
                        .parse_mode(ParseMode::Html)
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
                let delay_ms = *state_clone.finalization_delay_ms.read().await;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                let final_data = {
                    let mut turns = state_clone.turns.write().await;
                    turns.get_mut(&chat_id).and_then(|t| t.take_response_for_finalize())
                };
                if let Some((text, msg_id)) = final_data {
                    tracing::debug!(chat_id, msg_id, buf_len = text.len(), "result finalization timer: sending final edit");
                    if !text.is_empty() && msg_id != 0 {
                        let formatted = format_telegram_html(&text);
                        let final_chunks = split_message(&formatted, 4096);
                        if let Err(e) = bot_clone
                            .edit_message_text(ChatId(chat_id), MessageId(msg_id), &final_chunks[0])
                            .parse_mode(ParseMode::Html)
                            .await
                        {
                            tracing::warn!(%e, chat_id, "failed to finalize message edit");
                        }
                        for chunk in final_chunks.iter().skip(1) {
                            if let Err(e) = bot_clone.send_message(ChatId(chat_id), chunk)
                                .parse_mode(ParseMode::Html)
                                .await
                            {
                                tracing::warn!(%e, chat_id, "failed to send overflow chunk");
                            }
                        }
                    }
                } else {
                    tracing::debug!(chat_id, "result finalization timer: no response data to send");
                }
                state_clone.turns.write().await.remove(&chat_id);
            });

            let mut turns = state.turns.write().await;
            if let Some(turn) = turns.get_mut(&chat_id) {
                turn.begin_finalizing(handle);
            }

            Ok(())
        }

        ContentKind::UserMessageChunk { .. } | ContentKind::UsageUpdate => {
            Ok(())
        }

        ContentKind::Unknown => {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
    async fn when_unknown_update_type_with_empty_content_then_turn_preserved() {
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
        assert!(
            state.turns.read().await.get(&12345).is_some(),
            "unknown update types must not destroy active turns"
        );
    }
}
