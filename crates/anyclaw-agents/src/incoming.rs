use std::sync::atomic::Ordering;

use anyclaw_jsonrpc::types::{JsonRpcRequest, JsonRpcResponse, RequestId};
use anyclaw_sdk_types::{ChannelEvent, PermissionOption};
use tokio::sync::mpsc;

use crate::acp_types::{SessionPushParams, SessionUpdateEvent, SessionUpdateType};
use crate::connection::IncomingMessage;
use crate::manager::{AgentsManager, PendingPermission, SlotIncoming};

impl AgentsManager {
    pub(crate) async fn handle_incoming(&mut self, slot_idx: usize, msg: IncomingMessage) {
        let request = match msg {
            IncomingMessage::AgentNotification(r) | IncomingMessage::AgentRequest(r) => r,
        };

        match request.method.as_str() {
            "session/update" => {
                // D-03: session/update params are forwarded as raw content to channels
                // (with timestamp injection, tool normalization, command merging).
                // Must stay as Value for content mutation pipeline.
                let params = request.params.unwrap_or(serde_json::Value::Null);
                self.handle_session_update(slot_idx, params).await;
            }
            "session/request_permission" => {
                self.handle_permission_request(slot_idx, &request).await;
            }
            "session/push" => {
                self.handle_session_push(slot_idx, &request).await;
            }
            "fs/read_text_file" => {
                Self::handle_fs_read(&self.slots[slot_idx], &request).await;
            }
            "fs/write_text_file" => {
                Self::handle_fs_write(&self.slots[slot_idx], &request).await;
            }
            _ => {
                Self::send_error_response(
                    &self.slots[slot_idx],
                    &request,
                    -32601,
                    "Method not found",
                )
                .await;
            }
        }
    }

    pub(crate) fn session_update_type_name(update: &SessionUpdateType) -> &'static str {
        match update {
            SessionUpdateType::AgentThoughtChunk { .. } => "agent_thought_chunk",
            SessionUpdateType::AgentMessageChunk { .. } => "agent_message_chunk",
            SessionUpdateType::Result { .. } => "result",
            SessionUpdateType::ToolCall { .. } => "tool_call",
            SessionUpdateType::ToolCallUpdate { .. } => "tool_call_update",
            SessionUpdateType::Plan { .. } => "plan",
            SessionUpdateType::UsageUpdate { .. } => "usage_update",
            SessionUpdateType::UserMessageChunk { .. } => "user_message_chunk",
            SessionUpdateType::AvailableCommandsUpdate { .. } => "available_commands_update",
            SessionUpdateType::CurrentModeUpdate { .. } => "extension:current_mode",
            SessionUpdateType::ConfigOptionUpdate { .. } => "extension:config_option",
            SessionUpdateType::SessionInfoUpdate { .. } => "extension:session_info",
            _ => "unknown",
        }
    }

    // D-03: agent content is arbitrary JSON that requires raw mutation (timestamps, normalization, command injection).
    // DeliverMessage.content stays as Value because agents manager injects _received_at_ms, normalizes
    // tool event fields, and merges platform commands — all operations on raw JSON structure.
    pub(crate) fn add_received_timestamp(content: &mut serde_json::Value) {
        if let Some(obj) = content.as_object_mut() {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "system time before UNIX_EPOCH, using zero duration");
                    std::time::Duration::default()
                })
                .as_millis() as u64;
            obj.insert("_received_at_ms".to_string(), serde_json::json!(now_ms));
        }
    }

    // D-03: content is raw agent JSON — inject_platform_commands merges platform command
    // descriptors into the agent's availableCommands array, requiring Value array manipulation.
    pub(crate) async fn forward_session_update(
        &mut self,
        slot_idx: usize,
        event: SessionUpdateEvent,
        mut content: serde_json::Value,
        seq: u64,
    ) {
        let update_type = Self::session_update_type_name(&event.update);
        tracing::debug!(agent = %self.slots[slot_idx].name(), session_id = %event.session_id, update_type, seq, "session update routed");

        let is_result = matches!(event.update, SessionUpdateType::Result { .. });
        Self::add_received_timestamp(&mut content);
        normalize_tool_event_fields(&mut content, update_type);

        if update_type == "available_commands_update" {
            if let Some(cmds) = content
                .pointer_mut("/update/availableCommands")
                .and_then(|v| v.as_array_mut())
                && let serde_json::Value::Array(platform_arr) =
                    crate::platform_commands::platform_commands_json()
            {
                cmds.extend(platform_arr);
            }
            self.slots[slot_idx].last_available_commands = Some(content.clone());
        }

        if self.slots[slot_idx]
            .awaiting_first_prompt
            .contains(&event.session_id)
        {
            tracing::debug!(
                agent = %self.slots[slot_idx].name(),
                session_id = %event.session_id,
                update_type,
                seq,
                "suppressed replay event during session/load"
            );
            return;
        }

        if let Some(session_key) = self.slots[slot_idx]
            .reverse_map
            .get(&event.session_id)
            .cloned()
            && let Some(sender) = &self.channels_sender
        {
            let _ = sender
                .send(ChannelEvent::DeliverMessage {
                    session_key: session_key.clone(),
                    content,
                })
                .await;

            if is_result {
                self.streaming_completed.insert(session_key);
            }
        }
    }

    pub(crate) async fn forward_malformed_update_error(
        &self,
        slot_idx: usize,
        params: &serde_json::Value,
        error: &serde_json::Error,
        seq: u64,
    ) {
        tracing::warn!(error = %error, raw_params = %params, seq, "session/update deserialization FAILED — update dropped");

        let Some(session_id) = params.get("sessionId").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(session_key) = self.slots[slot_idx].reverse_map.get(session_id).cloned() else {
            return;
        };
        let Some(sender) = &self.channels_sender else {
            return;
        };

        let error_content = serde_json::json!({
            "error": format!("Agent sent malformed update: {error}"),
            "update": { "sessionUpdate": "result" }
        });
        let _ = sender
            .send(ChannelEvent::DeliverMessage {
                session_key,
                content: error_content,
            })
            .await;
    }

    // D-03: session/update params are the raw agent content payload that gets forwarded
    // to channels after mutation (timestamps, tool normalization, command merging).
    // Deserialized into SessionUpdateEvent for typed dispatch, but the raw Value is
    // forwarded as DeliverMessage.content for channel consumption.
    pub(crate) async fn handle_session_update(
        &mut self,
        slot_idx: usize,
        params: serde_json::Value,
    ) {
        let seq = self.update_seq.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(raw_params = %params, seq, "session/update received — attempting deser");

        // Clone needed: typed event for dispatch + raw Value for content forwarding (D-03)
        match serde_json::from_value::<SessionUpdateEvent>(params.clone()) {
            Ok(event) => {
                self.forward_session_update(slot_idx, event, params, seq)
                    .await;
            }
            Err(error) => {
                self.forward_malformed_update_error(slot_idx, &params, &error, seq)
                    .await;
            }
        }
    }

    pub(crate) async fn handle_permission_request(
        &mut self,
        slot_idx: usize,
        request: &JsonRpcRequest,
    ) {
        // D-03: permission request params have agent-defined schemas (requestId location varies by agent)
        let params = request.params.as_ref();
        let request_id = params
            .and_then(|p| p["requestId"].as_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| {
                // OpenCode uses JSON-RPC id field instead of params.requestId
                match &request.id {
                    Some(RequestId::Number(n)) => n.to_string(),
                    Some(RequestId::String(s)) => s.clone(),
                    None => String::new(),
                }
            });
        let description = params
            .and_then(|p| {
                p["description"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| p["toolCall"]["title"].as_str())
            })
            .unwrap_or("Permission requested")
            .to_string();

        let options: Vec<PermissionOption> = params
            .and_then(|p| p.get("options"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| {
                tracing::warn!(%request_id, "malformed permission options, using empty list");
                Vec::new()
            });

        tracing::info!(agent = %self.slots[slot_idx].name(), %request_id, %description, "permission requested");

        let session_id = params.and_then(|p| p["sessionId"].as_str()).unwrap_or("");
        let routed = if let Some(session_key) =
            self.slots[slot_idx].reverse_map.get(session_id).cloned()
            && let Some(sender) = &self.channels_sender
        {
            sender
                .send(ChannelEvent::RoutePermission {
                    session_key,
                    request_id: request_id.clone(),
                    description: description.clone(),
                    options: options.clone(),
                })
                .await
                .is_ok()
        } else {
            false
        };

        if routed {
            self.slots[slot_idx].pending_permissions.insert(
                request_id,
                PendingPermission {
                    request: request.clone(),
                    description,
                    options,
                    received_at: std::time::Instant::now(),
                },
            );
        } else {
            tracing::warn!(
                agent = %self.slots[slot_idx].name(),
                %request_id,
                "permission not routable to channel, auto-approving"
            );
            let auto_option = options
                .first()
                .map(|o| o.option_id.clone())
                .unwrap_or_else(|| "once".to_string());
            if let Some(conn) = self.slots[slot_idx].connection.as_ref() {
                let resp = JsonRpcResponse::success(
                    request.id.clone(),
                    serde_json::json!({
                        "requestId": request_id,
                        "optionId": auto_option,
                    }),
                );
                let _ = conn.send_raw(resp).await;
            }
        }
    }

    pub(crate) async fn handle_session_push(&mut self, slot_idx: usize, request: &JsonRpcRequest) {
        let Some(params_val) = request.params.as_ref() else {
            Self::send_error_response(&self.slots[slot_idx], request, -32602, "Missing params")
                .await;
            return;
        };
        let push_params = match serde_json::from_value::<SessionPushParams>(params_val.clone()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "session/push params deserialization failed");
                Self::send_error_response(&self.slots[slot_idx], request, -32602, "Invalid params")
                    .await;
                return;
            }
        };

        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "push_message",
                "content": push_params.content,
            }
        });

        if let Some(session_key) = self.slots[slot_idx]
            .reverse_map
            .get(&push_params.session_id)
            .cloned()
            && let Some(sender) = &self.channels_sender
        {
            let _ = sender
                .send(ChannelEvent::DeliverMessage {
                    session_key,
                    content,
                })
                .await;

            if let Some(conn) = self.slots[slot_idx].connection.as_ref() {
                let resp = JsonRpcResponse::success(request.id.clone(), serde_json::json!({}));
                let _ = conn.send_raw(resp).await;
            }
        } else {
            tracing::warn!(
                session_id = %push_params.session_id,
                "session/push: no session mapping found"
            );
            Self::send_error_response(&self.slots[slot_idx], request, -32001, "Unknown session")
                .await;
        }
    }

    pub(crate) async fn handle_prompt_completion(
        &mut self,
        completion: crate::manager::PromptCompletion,
        incoming_rx: &mut mpsc::Receiver<SlotIncoming>,
    ) {
        // Drain any pending streaming events before sending SessionComplete.
        // The RPC response arrives after all streaming events on the agent's stdout,
        // but select! can pick completion_rx before incoming_rx is fully drained.
        while let Ok(slot_msg) = incoming_rx.try_recv() {
            match slot_msg.msg {
                Some(incoming_msg) => self.handle_incoming(slot_msg.slot_idx, incoming_msg).await,
                None => {
                    self.handle_crash(slot_msg.slot_idx).await;
                }
            }
        }

        let already_got_result = self.streaming_completed.remove(&completion.session_key);

        // When the agent reports "session not found", invalidate the stale mapping
        // so the next inbound prompt triggers heal_session() instead of reusing
        // the dead ACP session ID.
        if completion.session_expired {
            tracing::info!(
                session_key = %completion.session_key,
                "invalidating expired session mapping — next prompt will trigger recovery"
            );
            for slot in &mut self.slots {
                if let Some(acp_id) = slot.session_map.remove(&completion.session_key) {
                    slot.reverse_map.remove(&acp_id);
                    slot.tool_context_sent.remove(&acp_id);
                }
            }
        }

        if let Some(sender) = &self.channels_sender {
            if !already_got_result {
                let acp_session_id = self.slots.iter()
                    .find_map(|slot| slot.session_map.get(&completion.session_key).cloned())
                    .unwrap_or_else(|| {
                        tracing::warn!(session_key = %completion.session_key, "no acp_session_id in reverse_map for synthetic result");
                        String::new()
                    });

                let stop_reason_str = match completion.stop_reason {
                    anyclaw_sdk_types::acp::StopReason::EndTurn => "end_turn",
                    anyclaw_sdk_types::acp::StopReason::MaxTokens => "max_tokens",
                    anyclaw_sdk_types::acp::StopReason::MaxTurnRequests => "max_turn_requests",
                    anyclaw_sdk_types::acp::StopReason::Refusal => "refusal",
                    anyclaw_sdk_types::acp::StopReason::Cancelled => "cancelled",
                };
                let synthetic_result = serde_json::json!({
                    "sessionId": acp_session_id,
                    "update": {
                        "sessionUpdate": "result",
                        "stopReason": stop_reason_str,
                    }
                });
                let _ = sender
                    .send(ChannelEvent::DeliverMessage {
                        session_key: completion.session_key.clone(),
                        content: synthetic_result,
                    })
                    .await;
            }

            let _ = sender
                .send(ChannelEvent::SessionComplete {
                    session_key: completion.session_key.clone(),
                    stop_reason: completion.stop_reason,
                })
                .await;
        }

        // Drain queued messages and dispatch next batch
        if let Some((mut merged_content, mut merged_meta)) =
            self.queue.mark_idle(&completion.session_key)
        {
            let remaining = self.queue.drain_queued(&completion.session_key);
            for (extra_content, extra_meta) in remaining {
                merged_content.extend(extra_content);
                if extra_meta.is_some() {
                    merged_meta = extra_meta;
                }
            }

            let agent_name = self
                .slots
                .iter()
                .find(|s| s.session_map.contains_key(&completion.session_key))
                .map(|s| s.name.clone())
                .unwrap_or_default();

            if !agent_name.is_empty() {
                if let Some(sender) = &self.channels_sender {
                    let _ = sender
                        .send(ChannelEvent::DispatchStarted {
                            session_key: completion.session_key.clone(),
                        })
                        .await;
                }
                if let Err(e) = self
                    .prompt_session(
                        &agent_name,
                        &completion.session_key,
                        &merged_content,
                        merged_meta.as_ref(),
                    )
                    .await
                {
                    tracing::warn!(
                        session_key = %completion.session_key,
                        error = %e,
                        "failed to dispatch queued message after completion"
                    );
                }
            }
        }
    }
}

// D-03: agent content mutation — normalizes agent-specific wire quirks (title→name, rawOutput→output)
// into the canonical format that ContentKind expects. Operates on raw JSON structure.
pub(crate) fn normalize_tool_event_fields(content: &mut serde_json::Value, update_type: &str) {
    if update_type != "tool_call" && update_type != "tool_call_update" {
        return;
    }
    let Some(update) = content.get_mut("update").and_then(|u| u.as_object_mut()) else {
        return;
    };

    if !update.contains_key("name")
        && let Some(title) = update.remove("title")
    {
        update.insert("name".to_string(), title);
    }

    // Promote rawInput → input (if input is absent or empty)
    if !update.contains_key("input")
        && let Some(raw_input) = update.get("rawInput").cloned()
    {
        // Only promote if rawInput is a non-empty object
        if raw_input.is_object() && !raw_input.as_object().is_some_and(serde_json::Map::is_empty) {
            update.insert("input".to_string(), raw_input);
        }
    }

    if update_type == "tool_call_update"
        && !update.contains_key("output")
        && let Some(raw) = update
            .get("rawOutput")
            .and_then(|r| r.get("output"))
            .cloned()
    {
        update.insert("output".to_string(), raw);
    }
}
