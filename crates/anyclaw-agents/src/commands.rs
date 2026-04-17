use std::sync::Arc;

use anyclaw_core::{
    AgentStatusInfo, AgentsCommand, PendingPermissionInfo, PersistedSession, SessionKey,
};
use anyclaw_jsonrpc::types::JsonRpcResponse;
use anyclaw_sdk_types::ChannelEvent;
use anyclaw_sdk_types::acp::StopReason;

use anyclaw_sdk_types::MessageMetadata;

use crate::acp_types::{
    ContentPart, PromptResponse, SessionCancelParams, SessionForkParams, SessionForkResult,
    SessionListParams, SessionPromptParams, content_parts_to_blocks,
};
use crate::error::AgentsError;
use crate::manager::{AgentsManager, PromptCompletion};
use crate::slot::find_slot_by_name;

impl AgentsManager {
    pub(crate) async fn handle_command(&mut self, cmd: AgentsCommand) -> bool {
        match cmd {
            AgentsCommand::SendPrompt { message, reply } => {
                let result = if let Some(slot) = self.slots.first() {
                    Self::send_prompt_to_slot(slot, &message).await
                } else {
                    Err(AgentsError::ConnectionClosed)
                };
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::CancelOperation => {
                for slot in &self.slots {
                    if let Some(conn) = &slot.connection {
                        for acp_id in slot.session_map.values() {
                            let params = serde_json::to_value(SessionCancelParams {
                                session_id: acp_id.clone(),
                            })
                            .ok();
                            if let Some(p) = params {
                                let _ = conn.send_notification("session/cancel", p).await;
                            }
                        }
                    }
                }
            }
            AgentsCommand::RespondPermission {
                request_id,
                option_id,
            } => {
                for slot in &mut self.slots {
                    if let Some(perm) = slot.pending_permissions.remove(&request_id) {
                        tracing::info!(agent = %slot.name(), %request_id, %option_id, "permission response received from channel");
                        if let Some(conn) = slot.connection.as_ref() {
                            let resp = JsonRpcResponse::success(
                                perm.request.id.clone(),
                                serde_json::json!({
                                    "outcome": {
                                        "outcome": "selected",
                                        "optionId": option_id,
                                    }
                                }),
                            );
                            let _ = conn.send_raw(resp).await;
                            tracing::info!(agent = %slot.name(), %request_id, "permission response sent to agent");
                        }
                        break;
                    }
                }
            }
            AgentsCommand::GetPendingPermissions { reply } => {
                let mut infos = Vec::new();
                for slot in &self.slots {
                    for (id, p) in &slot.pending_permissions {
                        infos.push(PendingPermissionInfo {
                            request_id: id.clone(),
                            description: p.description.clone(),
                            options: p.options.clone(),
                        });
                    }
                }
                let _ = reply.send(infos);
            }
            AgentsCommand::Shutdown => {
                self.shutdown_all().await;
                return true;
            }
            AgentsCommand::GetStatus { reply } => {
                let statuses: Vec<AgentStatusInfo> = self
                    .slots
                    .iter()
                    .map(|slot| AgentStatusInfo {
                        name: slot.name().to_string(),
                        connected: slot.connection.is_some(),
                        session_count: slot.session_map.len(),
                    })
                    .collect();
                let _ = reply.send(statuses);
            }
            AgentsCommand::CreateSession {
                agent_name,
                session_key,
                reply,
            } => {
                let slot_idx = find_slot_by_name(&self.slots, &agent_name);
                let has_stale = slot_idx
                    .map(|idx| self.slots[idx].stale_sessions.contains_key(&session_key))
                    .unwrap_or(false);

                let result = if has_stale {
                    let idx = slot_idx.expect("slot_idx must be Some when has_stale is true");
                    match self.heal_session(idx, &agent_name, &session_key).await {
                        Ok(()) => self.slots[idx]
                            .session_map
                            .get(&session_key)
                            .cloned()
                            .ok_or(AgentsError::ConnectionClosed),
                        Err(e) => Err(e),
                    }
                } else {
                    self.create_session(&agent_name, session_key).await
                };
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::EnqueueMessage {
                agent_name,
                session_key,
                content,
                metadata,
                reply,
            } => {
                // Platform commands bypass the queue entirely
                if let Some(cmd) = extract_command_text(&content)
                    .and_then(crate::platform_commands::match_platform_command)
                {
                    let slot_idx = find_slot_by_name(&self.slots, &agent_name);
                    let result = if let Some(idx) = slot_idx {
                        self.handle_platform_command(cmd.name, idx, &agent_name, &session_key)
                            .await
                    } else {
                        Err(AgentsError::AgentNotFound(agent_name.clone()))
                    };
                    let _ = reply.send(result.map_err(|e| e.to_string()));
                } else if self.queue.is_active(&session_key) {
                    self.queue.push(&session_key, content, metadata);
                    let _ = reply.send(Ok(()));
                } else {
                    self.queue.push_only(&session_key, content, metadata);
                    let flush_result = self.flush_and_dispatch(&agent_name, &session_key).await;
                    let _ = reply.send(flush_result.map_err(|e| e.to_string()));
                }
            }
            AgentsCommand::ForkSession {
                agent_name,
                session_key,
                reply,
            } => {
                let result = self.fork_session(&agent_name, &session_key).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::ListSessions { agent_name, reply } => {
                let result = self.list_sessions(&agent_name).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
            AgentsCommand::CancelSession {
                agent_name,
                session_key,
                reply,
            } => {
                let result = self.cancel_session(&agent_name, &session_key).await;
                let _ = reply.send(result.map_err(|e| e.to_string()));
            }
        }
        false
    }

    pub(crate) async fn send_prompt_to_slot(
        slot: &crate::slot::AgentSlot,
        message: &str,
    ) -> Result<(), AgentsError> {
        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let acp_id = slot
            .session_map
            .values()
            .next()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_id.clone(),
            prompt: content_parts_to_blocks(vec![ContentPart::text(message)]),
            meta: None,
        })?;

        let _response_rx = conn.send_request("session/prompt", params).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(agent = %agent_name, session_key = %session_key))]
    pub(crate) async fn create_session(
        &mut self,
        agent_name: &str,
        session_key: SessionKey,
    ) -> Result<String, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if let Some(acp_id) = slot.session_map.get(&session_key) {
            return Ok(acp_id.clone());
        }

        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        let acp_session_id =
            Self::start_session(&mut self.slots[slot_idx], &self.tools_handle, acp_timeout).await?;

        // Cache tool context on first session creation for this slot.
        if self.slots[slot_idx].tool_context.is_none() {
            self.slots[slot_idx].tool_context = self.fetch_tool_context(slot_idx).await;
        }

        let slot = &mut self.slots[slot_idx];
        slot.session_map
            .insert(session_key.clone(), acp_session_id.clone());
        slot.reverse_map
            .insert(acp_session_id.clone(), session_key.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let persisted = PersistedSession {
            session_key: session_key.to_string(),
            agent_name: agent_name.to_string(),
            acp_session_id: acp_session_id.clone(),
            created_at: now,
            last_active_at: now,
            closed: false,
        };
        if let Err(e) = self.session_store.upsert_session(&persisted).await {
            tracing::warn!(
                agent = %agent_name,
                session_key = %session_key,
                error = %e,
                "failed to persist new session to store"
            );
        }

        tracing::info!(agent = %agent_name, session_key = %acp_session_id, "multi-session created");
        Ok(acp_session_id)
    }

    pub(crate) async fn flush_and_dispatch(
        &mut self,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        let Some((content, metadata)) = self.queue.flush_pending(session_key) else {
            return Ok(());
        };

        if let Some(sender) = &self.channels_sender {
            let _ = sender
                .send(ChannelEvent::DispatchStarted {
                    session_key: session_key.clone(),
                })
                .await;
        }

        self.prompt_session(agent_name, session_key, &content, metadata.as_ref())
            .await
    }

    pub(crate) async fn prompt_session(
        &mut self,
        agent_name: &str,
        session_key: &SessionKey,
        content: &[ContentPart],
        metadata: Option<&MessageMetadata>,
    ) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        // Platform commands are handled in the agents layer — not forwarded to the agent process.
        if let Some(cmd) =
            extract_command_text(content).and_then(crate::platform_commands::match_platform_command)
        {
            return self
                .handle_platform_command(cmd.name, slot_idx, agent_name, session_key)
                .await;
        }

        if !self.slots[slot_idx].session_map.contains_key(session_key) {
            self.heal_session(slot_idx, agent_name, session_key).await?;
        }

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        self.slots[slot_idx]
            .awaiting_first_prompt
            .remove(&acp_session_id);

        // Build prompt parts, injecting tool context on first prompt per session.
        let mut prompt_parts = Vec::new();
        if !self.slots[slot_idx]
            .tool_context_sent
            .contains(&acp_session_id)
            && let Some(ctx) = self.slots[slot_idx].tool_context.as_deref()
        {
            prompt_parts.push(ContentPart::text(ctx));
            self.slots[slot_idx]
                .tool_context_sent
                .insert(acp_session_id.clone());
        }
        if let Some(meta) = metadata
            && let Some(context_text) = build_reply_context(meta)
        {
            prompt_parts.push(ContentPart::text(context_text));
        }
        prompt_parts.extend_from_slice(content);

        let slot = &self.slots[slot_idx];
        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;

        let params = serde_json::to_value(SessionPromptParams {
            session_id: acp_session_id.clone(),
            prompt: content_parts_to_blocks(prompt_parts),
            meta: None,
        })?;

        let response_rx = conn.send_request("session/prompt", params).await?;

        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let sk_string = session_key.to_string();
            let store = Arc::clone(&self.session_store);
            tokio::spawn(async move {
                if let Err(e) = store.update_last_active(&sk_string, now).await {
                    tracing::warn!(
                        session_key = %sk_string,
                        error = %e,
                        "failed to update last_active in store"
                    );
                }
            });
        }

        {
            let completion_tx = self.completion_tx.clone();
            let channels_tx = self.channels_sender.clone();
            let sk = session_key.clone();
            tokio::spawn(async move {
                match response_rx.await {
                    Ok(response) => {
                        // Check if the agent returned a JSON-RPC error
                        let mut session_expired = false;
                        let stop_reason;
                        if let Some(error) = &response.error {
                            let msg = &error.message;
                            tracing::warn!(session_key = %sk, error = %msg, "agent returned error for prompt");

                            // Detect "session not found" so the stale mapping gets
                            // invalidated in handle_prompt_completion, allowing the
                            // next prompt to trigger heal_session.
                            let combined = format!(
                                "{} {}",
                                msg,
                                error
                                    .data
                                    .as_ref()
                                    .map(std::string::ToString::to_string)
                                    .unwrap_or_default()
                            );
                            if combined.to_lowercase().contains("session not found") {
                                session_expired = true;
                            }
                            stop_reason = anyclaw_sdk_types::acp::StopReason::Refusal;

                            if let Some(sender) = &channels_tx {
                                let error_content = serde_json::json!({
                                    "error": msg,
                                    "update": { "sessionUpdate": "result" }
                                });
                                let _ = sender
                                    .send(ChannelEvent::DeliverMessage {
                                        session_key: sk.clone(),
                                        content: error_content,
                                    })
                                    .await;
                            }
                        } else {
                            let prompt_resp: PromptResponse =
                                serde_json::from_value(response.result.unwrap_or_default())
                                    .unwrap_or_else(|e| {
                                        tracing::warn!(session_key = %sk, error = %e, "failed to parse PromptResponse, defaulting");
                                        PromptResponse { stop_reason: anyclaw_sdk_types::acp::StopReason::EndTurn }
                                    });
                            stop_reason = prompt_resp.stop_reason;
                        }
                        let _ = completion_tx
                            .send(PromptCompletion {
                                session_key: sk,
                                session_expired,
                                stop_reason,
                            })
                            .await;
                    }
                    Err(_) => {
                        tracing::warn!(session_key = %sk, "prompt response channel dropped");
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_platform_command(
        &mut self,
        command: &str,
        slot_idx: usize,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        match command {
            "new" => {
                self.slots[slot_idx].session_map.remove(session_key);
                self.slots[slot_idx]
                    .reverse_map
                    .retain(|_, v| v != session_key);
                let acp_id = self.create_session(agent_name, session_key.clone()).await?;
                tracing::info!(
                    agent = %agent_name,
                    session_key = %session_key,
                    acp_session_id = %acp_id,
                    "platform command /new: fresh session created"
                );
                self.send_synthetic_chunk(session_key, "New conversation started.")
                    .await;
                self.queue.mark_idle(session_key);
                Ok(())
            }
            "cancel" => {
                let cancelled = self.cancel_session(agent_name, session_key).await.is_ok();
                if cancelled {
                    tracing::info!(
                        agent = %agent_name,
                        session_key = %session_key,
                        "platform command /cancel: session cancelled"
                    );
                    self.send_synthetic_chunk(session_key, "Operation cancelled.")
                        .await;
                    // Mark queue idle so subsequent messages aren't stuck.
                    // If the agent also responds, mark_idle on an idle session is a no-op.
                    self.queue.mark_idle(session_key);
                } else {
                    self.send_synthetic_response(
                        session_key,
                        "No active operation to cancel.",
                        StopReason::Cancelled,
                    )
                    .await;
                }
                Ok(())
            }
            _ => {
                tracing::warn!(command = %command, "unknown platform command — ignoring");
                Ok(())
            }
        }
    }

    async fn send_synthetic_chunk(&self, session_key: &SessionKey, message: &str) {
        let Some(sender) = &self.channels_sender else {
            return;
        };
        let _ = sender
            .send(ChannelEvent::DispatchStarted {
                session_key: session_key.clone(),
            })
            .await;
        let _ = sender
            .send(ChannelEvent::DeliverMessage {
                session_key: session_key.clone(),
                content: serde_json::json!({
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": message
                    }
                }),
            })
            .await;
    }

    async fn send_synthetic_response(
        &self,
        session_key: &SessionKey,
        message: &str,
        stop_reason: StopReason,
    ) {
        self.send_synthetic_chunk(session_key, message).await;
        let Some(sender) = &self.channels_sender else {
            return;
        };
        let _ = sender
            .send(ChannelEvent::DeliverMessage {
                session_key: session_key.clone(),
                content: serde_json::json!({
                    "update": {
                        "sessionUpdate": "result",
                        "content": ""
                    }
                }),
            })
            .await;
        let _ = sender
            .send(ChannelEvent::SessionComplete {
                session_key: session_key.clone(),
                stop_reason,
            })
            .await;
    }

    pub(crate) async fn fork_session(
        &mut self,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<String, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if !slot.has_session_capability(|c| c.fork.is_some()) {
            return Err(AgentsError::CapabilityNotSupported("fork".into()));
        }

        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionForkParams {
            session_id: acp_session_id,
        })?;
        let rx = conn.send_request("session/fork", params).await?;

        let acp_timeout = Self::acp_timeout_for(&self.slots[slot_idx].config, &self.manager_config);
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let result: SessionForkResult = serde_json::from_value(resp.result.unwrap_or_default())?;

        let fork_key = SessionKey::new(session_key.channel_name(), "fork", &result.session_id);
        let slot = &mut self.slots[slot_idx];
        slot.session_map
            .insert(fork_key.clone(), result.session_id.clone());
        slot.reverse_map.insert(result.session_id.clone(), fork_key);

        tracing::info!(agent = %agent_name, forked_session_id = %result.session_id, "session forked");
        Ok(result.session_id)
    }

    pub(crate) async fn list_sessions(
        &self,
        agent_name: &str,
    ) -> Result<anyclaw_sdk_types::SessionListResult, AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        if !slot.has_session_capability(|c| c.list.is_some()) {
            return Err(AgentsError::CapabilityNotSupported("list".into()));
        }

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionListParams {})?;
        let rx = conn.send_request("session/list", params).await?;

        let acp_timeout = Self::acp_timeout_for(&slot.config, &self.manager_config);
        let resp = tokio::time::timeout(acp_timeout, rx)
            .await
            .map_err(|_| AgentsError::Timeout(acp_timeout))?
            .map_err(|_| AgentsError::ConnectionClosed)?;

        let typed: anyclaw_sdk_types::SessionListResult =
            serde_json::from_value(resp.result.unwrap_or_default())?;
        Ok(typed)
    }

    pub(crate) async fn cancel_session(
        &self,
        agent_name: &str,
        session_key: &SessionKey,
    ) -> Result<(), AgentsError> {
        let slot_idx = find_slot_by_name(&self.slots, agent_name)
            .ok_or_else(|| AgentsError::AgentNotFound(agent_name.to_string()))?;

        let slot = &self.slots[slot_idx];
        let acp_session_id = slot
            .session_map
            .get(session_key)
            .ok_or(AgentsError::ConnectionClosed)?
            .clone();

        let conn = slot
            .connection
            .as_ref()
            .ok_or(AgentsError::ConnectionClosed)?;
        let params = serde_json::to_value(SessionCancelParams {
            session_id: acp_session_id,
        })?;
        conn.send_notification("session/cancel", params).await?;
        Ok(())
    }
}

pub(crate) fn extract_command_text(content: &[ContentPart]) -> Option<&str> {
    if content.len() == 1
        && let ContentPart::Text { text } = &content[0]
    {
        return Some(text.as_str());
    }
    None
}

/// Build a `[Context: ...]` string from reply/thread metadata for prompt injection.
/// Returns `None` if metadata has no actionable context.
pub(crate) fn build_reply_context(meta: &MessageMetadata) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(ref thread_id) = meta.thread_id {
        parts.push(format!("thread {thread_id}"));
    }
    if let Some(ref reply_text) = meta.reply_to_text {
        parts.push(format!("replying to: \"{reply_text}\""));
    } else if let Some(ref reply_id) = meta.reply_to_message_id {
        parts.push(format!("reply to message {reply_id}"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("[Context: {}]", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_reply_text_present_then_shows_quoted_text() {
        let meta = MessageMetadata {
            reply_to_message_id: Some("5398".into()),
            reply_to_text: Some("hello world".into()),
            thread_id: None,
        };
        let result = build_reply_context(&meta).unwrap();
        assert_eq!(result, r#"[Context: replying to: "hello world"]"#);
    }

    #[rstest]
    fn when_only_reply_id_then_falls_back_to_id() {
        let meta = MessageMetadata {
            reply_to_message_id: Some("5398".into()),
            reply_to_text: None,
            thread_id: None,
        };
        let result = build_reply_context(&meta).unwrap();
        assert_eq!(result, "[Context: reply to message 5398]");
    }

    #[rstest]
    fn when_thread_and_reply_text_then_both_shown() {
        let meta = MessageMetadata {
            reply_to_message_id: Some("100".into()),
            reply_to_text: Some("quoted".into()),
            thread_id: Some("42".into()),
        };
        let result = build_reply_context(&meta).unwrap();
        assert_eq!(result, r#"[Context: thread 42, replying to: "quoted"]"#);
    }

    #[rstest]
    fn when_only_thread_then_shows_thread() {
        let meta = MessageMetadata {
            reply_to_message_id: None,
            reply_to_text: None,
            thread_id: Some("42".into()),
        };
        let result = build_reply_context(&meta).unwrap();
        assert_eq!(result, "[Context: thread 42]");
    }

    #[rstest]
    fn when_all_none_then_returns_none() {
        let meta = MessageMetadata {
            reply_to_message_id: None,
            reply_to_text: None,
            thread_id: None,
        };
        assert!(build_reply_context(&meta).is_none());
    }
}
