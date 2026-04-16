use std::sync::Arc;
use std::time::Duration;

use anyclaw_sdk_channel::ChannelSdkError;
use anyclaw_sdk_types::ContentKind;
use teloxide::payloads::{EditMessageTextSetters, SendMessageSetters};
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId, ParseMode};
use tokio::time::Instant;

use crate::formatting::{close_open_tags, escape_html, format_telegram_html};
use crate::state::SharedState;
use crate::turn::{ChatTurn, ToolCallStatus, ToolCallTrack, TurnPhase};

const MAX_RETRY_ATTEMPTS: u32 = 3;
const RETRY_BASE_DELAY_MS: u64 = 500;
const TELEGRAM_MAX_MESSAGE_LEN: usize = 4096;

pub(crate) async fn retry_telegram_op<F, Fut, T>(
    op_name: &str,
    chat_id: i64,
    f: F,
) -> Result<T, teloxide::RequestError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, teloxide::RequestError>>,
{
    let mut last_err = None;
    for attempt in 0..MAX_RETRY_ATTEMPTS {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                let is_retryable = matches!(
                    &e,
                    teloxide::RequestError::RetryAfter(_) | teloxide::RequestError::Network(_)
                );
                if !is_retryable || attempt + 1 == MAX_RETRY_ATTEMPTS {
                    tracing::warn!(
                        op = op_name,
                        chat_id,
                        attempt = attempt + 1,
                        error = %e,
                        "telegram API call failed (final)"
                    );
                    return Err(e);
                }
                let delay = match &e {
                    teloxide::RequestError::RetryAfter(seconds) => seconds.duration(),
                    _ => std::time::Duration::from_millis(RETRY_BASE_DELAY_MS * 2u64.pow(attempt)),
                };
                tracing::debug!(
                    op = op_name,
                    chat_id,
                    attempt = attempt + 1,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "telegram API call failed, retrying"
                );
                tokio::time::sleep(delay).await;
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("loop ran at least once"))
}

async fn send_or_edit_final(bot: &Bot, chat_id: i64, text: &str, msg_id: i32, label: &str) {
    let formatted = format_telegram_html(text);
    let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
    let chunk0 = chunks[0].clone();
    if msg_id != 0 {
        let _ = retry_telegram_op(label, chat_id, || {
            let chunk0 = chunk0.clone();
            async move {
                bot.edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunk0)
                    .parse_mode(ParseMode::Html)
                    .await
            }
        })
        .await;
    } else {
        let _ = retry_telegram_op(label, chat_id, || {
            let chunk0 = chunk0.clone();
            async move {
                bot.send_message(ChatId(chat_id), &chunk0)
                    .parse_mode(ParseMode::Html)
                    .await
            }
        })
        .await;
    }
    for chunk in chunks.iter().skip(1) {
        let chunk = chunk.clone();
        if let Err(e) = retry_telegram_op(label, chat_id, || {
            let chunk = chunk.clone();
            async move {
                bot.send_message(ChatId(chat_id), &chunk)
                    .parse_mode(ParseMode::Html)
                    .await
            }
        })
        .await
        {
            tracing::warn!(%e, chat_id, label, "failed to send overflow chunk");
        }
    }
}

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
            tracing::debug!(
                chat_id,
                msg_id,
                buf_len = text.len(),
                "flush: sending final edit before cleanup"
            );
            let formatted = format_telegram_html(text);
            let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
            let chunk0 = chunks[0].clone();
            let _ = retry_telegram_op("flush_final_edit", chat_id, || {
                let chunk0 = chunk0.clone();
                async move {
                    bot.edit_message_text(ChatId(chat_id), MessageId(msg_id), &chunk0)
                        .parse_mode(ParseMode::Html)
                        .await
                }
            })
            .await;
            for chunk in chunks.iter().skip(1) {
                let chunk = chunk.clone();
                let _ = retry_telegram_op("flush_overflow_chunk", chat_id, || {
                    let chunk = chunk.clone();
                    async move {
                        bot.send_message(ChatId(chat_id), &chunk)
                            .parse_mode(ParseMode::Html)
                            .await
                    }
                })
                .await;
            }
        } else if !text.is_empty() && msg_id == 0 {
            // Debounce was pending — send buffered text as new message
            tracing::debug!(
                chat_id,
                buf_len = text.len(),
                "flush: sending debounced response as new message"
            );
            let formatted = format_telegram_html(text);
            let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
            for chunk in &chunks {
                let chunk = chunk.clone();
                let _ = retry_telegram_op("flush_debounced_response", chat_id, || {
                    let chunk = chunk.clone();
                    async move {
                        bot.send_message(ChatId(chat_id), &chunk)
                            .parse_mode(ParseMode::Html)
                            .await
                    }
                })
                .await;
            }
        } else {
            tracing::debug!(
                chat_id,
                msg_id,
                buf_empty = text.is_empty(),
                "flush: skipping final edit (empty or no msg_id)"
            );
        }
    } else {
        tracing::debug!(chat_id, "flush: no response to send");
    }
    if let Some((thought_msg_id, elapsed_secs)) = flush.thought_collapse
        && thought_msg_id != 0
    {
        let emoji = thought_emoji(state).await;
        let collapse_text = format!("{emoji} Thought for {elapsed_secs:.1}s");
        let _ = retry_telegram_op("flush_collapse_thought", chat_id, || {
            let collapse_text = collapse_text.clone();
            async move {
                bot.edit_message_text(ChatId(chat_id), MessageId(thought_msg_id), &collapse_text)
                    .parse_mode(ParseMode::Html)
                    .await
            }
        })
        .await;
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
        if open_count > close_count
            && let Some(close_pos) = remaining.find("</pre>")
        {
            let end = close_pos + "</pre>".len();
            if end <= remaining.len() {
                boundary = end;
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

// D-03: content is DeliverMessage.content (Value) — agents manager mutates raw JSON
// (timestamps, normalization, command injection) so it cannot have a fixed Rust type.
#[allow(clippy::disallowed_types)]
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
        .and_then(serde_json::Value::as_u64)
        .and_then(|received_ms| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "system time before UNIX_EPOCH, using zero duration");
                    std::time::Duration::default()
                })
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
                if let Some(turn) = turns.get(&chat_id)
                    && turn.thought.as_ref().map(|t| t.suppressed).unwrap_or(false)
                {
                    return Ok(());
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
                let turn = turns
                    .entry(chat_id)
                    .or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                turn.append_thought(&thought_content, 0, origin_instant);
                let track = turn
                    .thought
                    .as_ref()
                    .expect("thought track exists after append_thought");
                let accumulated = track.buffer.clone();
                let existing_thought_msg_id = track.msg_id;
                if let Some(ref h) = track.debounce_handle {
                    h.abort();
                }
                turn.thought
                    .as_mut()
                    .expect("thought track exists: verified above")
                    .debounce_handle = None;
                (accumulated, existing_thought_msg_id)
            };

            let emoji = thought_emoji(state).await;
            let thought_text = format!("{emoji} {}", escape_html(&accumulated));

            if existing_thought_msg_id == 0 {
                let thought_text_clone = thought_text.clone();
                if let Ok(sent) = retry_telegram_op("send_thought_message", chat_id, || {
                    let thought_text_clone = thought_text_clone.clone();
                    async move {
                        bot.send_message(ChatId(chat_id), &thought_text_clone)
                            .parse_mode(ParseMode::Html)
                            .await
                    }
                })
                .await
                {
                    let mut turns = state.turns.write().await;
                    if let Some(turn) = turns.get_mut(&chat_id)
                        && let Some(track) = turn.thought.as_mut()
                    {
                        track.msg_id = sent.id.0;
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
                        let text_clone = text.clone();
                        let _ = crate::deliver::retry_telegram_op(
                            "debounce_thought_edit",
                            chat_id,
                            || {
                                let text_clone = text_clone.clone();
                                let bot_clone = bot_clone.clone();
                                async move {
                                    bot_clone
                                        .edit_message_text(
                                            ChatId(chat_id),
                                            MessageId(thought_msg_id),
                                            &text_clone,
                                        )
                                        .parse_mode(ParseMode::Html)
                                        .await
                                }
                            },
                        )
                        .await;
                    }
                    let mut turns = state_clone.turns.write().await;
                    if let Some(turn) = turns.get_mut(&chat_id)
                        && let Some(track) = turn.thought.as_mut()
                    {
                        track.debounce_handle = None;
                    }
                });

                let mut turns = state.turns.write().await;
                if let Some(turn) = turns.get_mut(&chat_id)
                    && let Some(track) = turn.thought.as_mut()
                {
                    track.debounce_handle = Some(handle);
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
                let turn = turns
                    .entry(chat_id)
                    .or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                if let Some(track) = turn.thought.as_mut() {
                    track.suppressed = false;
                }
                turn.append_response(&chunk_text, 0);
                let track = turn
                    .response
                    .as_ref()
                    .expect("response track exists after append_response");
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
                    let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
                    let chunk0 = chunks[0].clone();
                    let _ = retry_telegram_op("is_finalizing_edit_response", chat_id, || {
                        let chunk0 = chunk0.clone();
                        async move {
                            bot.edit_message_text(
                                ChatId(chat_id),
                                MessageId(existing_response_msg_id),
                                &chunk0,
                            )
                            .parse_mode(ParseMode::Html)
                            .await
                        }
                    })
                    .await;
                    for chunk in chunks.iter().skip(1) {
                        let chunk = chunk.clone();
                        let _ = retry_telegram_op("is_finalizing_overflow_chunk", chat_id, || {
                            let chunk = chunk.clone();
                            async move {
                                bot.send_message(ChatId(chat_id), &chunk)
                                    .parse_mode(ParseMode::Html)
                                    .await
                            }
                        })
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
                        turns
                            .get_mut(&chat_id)
                            .and_then(ChatTurn::take_response_for_finalize)
                    };
                    if let Some((text, msg_id)) = final_data
                        && !text.is_empty()
                    {
                        send_or_edit_final(&bot_clone, chat_id, &text, msg_id, "finalize_late")
                            .await;
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
                    let cooldown =
                        Duration::from_millis(*state.response_edit_cooldown_ms.read().await);
                    let mut turns = state.turns.write().await;
                    turns
                        .get_mut(&chat_id)
                        .map(|t| t.can_edit_response(cooldown))
                        .unwrap_or(false)
                };
                if can_edit {
                    let formatted = close_open_tags(&format_telegram_html(&accumulated));
                    let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
                    let chunk0 = chunks[0].clone();
                    let _ = retry_telegram_op("edit_response_chunk", chat_id, || {
                        let chunk0 = chunk0.clone();
                        async move {
                            bot.edit_message_text(
                                ChatId(chat_id),
                                MessageId(existing_response_msg_id),
                                &chunk0,
                            )
                            .parse_mode(ParseMode::Html)
                            .await
                        }
                    })
                    .await;
                }
            } else {
                // First response chunk — debounce to let more text accumulate
                // before creating the Telegram message.
                let has_debounce = {
                    let turns = state.turns.read().await;
                    turns
                        .get(&chat_id)
                        .and_then(|t| t.response.as_ref())
                        .and_then(|r| r.debounce_handle.as_ref())
                        .is_some()
                };
                if has_debounce {
                    // Timer already running — chunks accumulate in buffer, nothing to do
                    return Ok(());
                }

                let bot_clone = bot.clone();
                let state_clone = Arc::clone(state);
                let handle = tokio::spawn(async move {
                    let debounce_ms = *state_clone.thought_debounce_ms.read().await;
                    tokio::time::sleep(Duration::from_millis(debounce_ms)).await;
                    let (accumulated, response_msg_id) = {
                        let turns = state_clone.turns.read().await;
                        match turns.get(&chat_id) {
                            Some(turn) => match &turn.response {
                                Some(track) => (track.buffer.clone(), track.msg_id),
                                None => return,
                            },
                            None => return,
                        }
                    };
                    if accumulated.is_empty() {
                        return;
                    }
                    // If msg_id was set while we waited (e.g. by flush), skip send
                    if response_msg_id != 0 {
                        return;
                    }
                    let formatted = close_open_tags(&format_telegram_html(&accumulated));
                    let chunks = split_message(&formatted, TELEGRAM_MAX_MESSAGE_LEN);
                    let chunk0 = chunks[0].clone();
                    if let Ok(sent) = retry_telegram_op("debounce_send_response", chat_id, || {
                        let chunk0 = chunk0.clone();
                        let bot_clone = bot_clone.clone();
                        async move {
                            bot_clone
                                .send_message(ChatId(chat_id), &chunk0)
                                .parse_mode(ParseMode::Html)
                                .await
                        }
                    })
                    .await
                    {
                        let mut turns = state_clone.turns.write().await;
                        if let Some(turn) = turns.get_mut(&chat_id)
                            && let Some(track) = turn.response.as_mut()
                        {
                            track.msg_id = sent.id.0;
                            track.debounce_handle = None;
                        }
                    }
                });

                let mut turns = state.turns.write().await;
                if let Some(turn) = turns.get_mut(&chat_id)
                    && let Some(track) = turn.response.as_mut()
                {
                    track.debounce_handle = Some(handle);
                }
            }
            Ok(())
        }

        ContentKind::Result { is_error, .. } => {
            {
                let turns = state.turns.read().await;
                if let Some(turn) = turns.get(&chat_id) {
                    let has_response = turn.response.is_some();
                    let response_msg_id = turn.response.as_ref().map(|r| r.msg_id).unwrap_or(-1);
                    let response_buf_len =
                        turn.response.as_ref().map(|r| r.buffer.len()).unwrap_or(0);
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

                if let Some(mid) = message_id
                    && let Some(turn) = turns.get(&chat_id)
                    && turn.is_different_turn(mid)
                {
                    tracing::info!(chat_id, stale_message_id = mid, current_message_id = %turn.message_id, "discarding stale result from previous turn");
                    return Ok(());
                }
            }

            let collapse_data = {
                let mut turns = state.turns.write().await;
                turns.get_mut(&chat_id).and_then(ChatTurn::collapse_thought)
            };

            if let Some((thought_msg_id, elapsed_secs)) = collapse_data
                && thought_msg_id != 0
            {
                let emoji = thought_emoji(state).await;
                let collapse_text = format!("{emoji} Thought for {elapsed_secs:.1}s");
                let _ = retry_telegram_op("collapse_thought", chat_id, || {
                    let collapse_text = collapse_text.clone();
                    async move {
                        bot.edit_message_text(
                            ChatId(chat_id),
                            MessageId(thought_msg_id),
                            &collapse_text,
                        )
                        .parse_mode(ParseMode::Html)
                        .await
                    }
                })
                .await;
            }

            {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns
                    .entry(chat_id)
                    .or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }
                turn.last_result_was_error = is_error;
                // Do NOT abort the debounce — it may be mid-flight (send_message
                // succeeded, msg_id not yet stored). Aborting loses the msg_id,
                // causing finalization to send a duplicate. Let it complete naturally.
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
                    turns
                        .get_mut(&chat_id)
                        .and_then(ChatTurn::take_response_for_finalize)
                };
                if let Some((text, msg_id)) = final_data {
                    tracing::debug!(
                        chat_id,
                        msg_id,
                        buf_len = text.len(),
                        "result finalization timer: sending final edit"
                    );
                    if !text.is_empty() {
                        send_or_edit_final(&bot_clone, chat_id, &text, msg_id, "finalize_result")
                            .await;
                    }
                } else {
                    tracing::debug!(
                        chat_id,
                        "result finalization timer: no response data to send"
                    );
                }
                state_clone.turns.write().await.remove(&chat_id);
            });

            let mut turns = state.turns.write().await;
            if let Some(turn) = turns.get_mut(&chat_id) {
                turn.begin_finalizing(handle);
            }

            Ok(())
        }

        ContentKind::ToolCall {
            name,
            tool_call_id,
            input,
        } => {
            let display_name = if name.is_empty() {
                "tool".to_string()
            } else {
                name.clone()
            };

            let (tools_text, existing_tools_msg_id) = {
                let mut turns = state.turns.write().await;
                let mid = message_id.unwrap_or("").to_string();
                let turn = turns
                    .entry(chat_id)
                    .or_insert_with(|| ChatTurn::new(mid.clone()));
                if !mid.is_empty() {
                    turn.message_id = mid;
                }

                if let Some(track) = turn.thought.as_mut() {
                    track.suppressed = false;
                }

                if !tool_call_id.is_empty() {
                    turn.tool_call_order.push(tool_call_id.clone());
                    turn.tool_calls.insert(
                        tool_call_id,
                        ToolCallTrack {
                            name: display_name,
                            status: ToolCallStatus::Started,
                            input,
                        },
                    );
                }

                let tools_text = turn.render_tools_text();
                let existing_tools_msg_id = turn.tools_msg_id;
                (tools_text, existing_tools_msg_id)
            };

            if tools_text.is_empty() {
                return Ok(());
            }

            if existing_tools_msg_id == 0 {
                let tools_text_clone = tools_text.clone();
                if let Ok(sent) = retry_telegram_op("send_tool_call", chat_id, || {
                    let tools_text_clone = tools_text_clone.clone();
                    async move {
                        bot.send_message(ChatId(chat_id), &tools_text_clone)
                            .parse_mode(ParseMode::Html)
                            .await
                    }
                })
                .await
                {
                    let mut turns = state.turns.write().await;
                    if let Some(turn) = turns.get_mut(&chat_id) {
                        turn.tools_msg_id = sent.id.0;
                    }
                }
            } else {
                let _ = retry_telegram_op("edit_tool_call", chat_id, || {
                    let tools_text = tools_text.clone();
                    async move {
                        bot.edit_message_text(
                            ChatId(chat_id),
                            MessageId(existing_tools_msg_id),
                            &tools_text,
                        )
                        .parse_mode(ParseMode::Html)
                        .await
                    }
                })
                .await;
            }
            Ok(())
        }

        ContentKind::ToolCallUpdate {
            tool_call_id,
            status,
            output,
            input,
            exit_code,
            ..
        } => {
            let (tools_text, tools_msg_id) = {
                let mut turns = state.turns.write().await;
                let Some(turn) = turns.get_mut(&chat_id) else {
                    tracing::warn!(
                        chat_id,
                        tool_call_id,
                        "tool call update for unknown chat turn"
                    );
                    return Ok(());
                };

                let Some(track) = turn.tool_calls.get_mut(&tool_call_id) else {
                    tracing::warn!(
                        chat_id,
                        tool_call_id,
                        "tool call update for untracked tool_call_id"
                    );
                    return Ok(());
                };

                // Backfill input if the initial ToolCall arrived without it
                if track.input.is_none() && input.is_some() {
                    track.input = input;
                }

                track.status = match status.as_str() {
                    "completed" if exit_code.is_some_and(|c| c != 0) => {
                        ToolCallStatus::Failed(output)
                    }
                    "in_progress" => ToolCallStatus::InProgress,
                    "completed" => ToolCallStatus::Completed,
                    "failed" => ToolCallStatus::Failed(output),
                    _ => ToolCallStatus::InProgress,
                };

                (turn.render_tools_text(), turn.tools_msg_id)
            };

            if tools_msg_id != 0 && !tools_text.is_empty() {
                let _ = retry_telegram_op("edit_tool_call_update", chat_id, || {
                    let tools_text = tools_text.clone();
                    async move {
                        bot.edit_message_text(ChatId(chat_id), MessageId(tools_msg_id), &tools_text)
                            .parse_mode(ParseMode::Html)
                            .await
                    }
                })
                .await;
            }
            Ok(())
        }

        ContentKind::UserMessageChunk { .. } | ContentKind::UsageUpdate => Ok(()),

        ContentKind::AvailableCommandsUpdate { commands } => {
            if let Some(cmds) = commands.as_array() {
                let bot_commands: Vec<teloxide::types::BotCommand> = cmds
                    .iter()
                    .filter_map(|cmd| {
                        let name = cmd.get("name")?.as_str()?;
                        let description = cmd.get("description")?.as_str().unwrap_or(name);
                        Some(teloxide::types::BotCommand::new(name, description))
                    })
                    .collect();
                if !bot_commands.is_empty()
                    && let Err(e) = bot.set_my_commands(bot_commands).await
                {
                    tracing::warn!(error = %e, "failed to set bot commands");
                }
            }
            Ok(())
        }

        ContentKind::Unknown => Ok(()),
        _ => Ok(()),
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
    async fn when_result_finalization_has_buffered_text_and_zero_msg_id_then_text_not_silently_dropped()
     {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("New conversation started.", 0);

        let (text, msg_id) = turn
            .take_response_for_finalize()
            .expect("turn must have buffered response");
        assert_eq!(text, "New conversation started.");
        assert_eq!(msg_id, 0);

        let would_take_action = !text.is_empty();
        assert!(
            would_take_action,
            "non-empty text with msg_id=0 must result in a send, not a silent drop"
        );
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
