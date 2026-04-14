use serde::{Deserialize, Serialize};

/// Channel capabilities advertised during initialize handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCapabilities {
    /// Whether the channel supports streaming (chunked) delivery.
    pub streaming: bool,
    /// Whether the channel supports rich text (Markdown, HTML).
    pub rich_text: bool,
}

/// Initialize handshake — anyclaw sends to channel subprocess.
// Extensible: channel-specific options have channel-defined schemas (D-03)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInitializeParams {
    /// ACP protocol version for compatibility negotiation.
    pub protocol_version: u32,
    /// Unique identifier for this channel instance.
    pub channel_id: String,
    /// Optional ack configuration for reactions and typing indicators.
    #[serde(default)]
    pub ack: Option<ChannelAckConfig>,
    /// Channel-specific configuration from `anyclaw.yaml` `options` section.
    #[serde(default)]
    pub options: std::collections::HashMap<String, serde_json::Value>,
}

/// Initialize handshake — channel subprocess responds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInitializeResult {
    /// ACP protocol version the channel supports.
    pub protocol_version: u32,
    /// Capabilities the channel advertises.
    pub capabilities: ChannelCapabilities,
}

/// Anyclaw → Channel: deliver agent message/streaming update.
// Pass-through: agents manager mutates raw JSON (timestamps, normalization, command injection)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeliverMessage {
    /// ACP session that produced this content.
    pub session_id: String,
    /// Agent content payload (streaming update, result, thought, etc.).
    pub content: serde_json::Value,
}

/// Peer identity information for inbound messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    /// Name of the originating channel (e.g., `"telegram"`, `"debug-http"`).
    pub channel_name: String,
    /// Opaque identifier for the peer within the channel.
    pub peer_id: String,
    /// Conversation kind (e.g., `"direct"`, `"group"`, `"local"`).
    pub kind: String,
}

/// Channel → Anyclaw: user sent a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSendMessage {
    /// Identity of the user who sent the message.
    pub peer_info: PeerInfo,
    /// User message text content.
    pub content: String,
}

/// Helper for channel implementations to extract thought content from DeliverMessage.
/// When DeliverMessage.content has type "agent_thought_chunk", channels can use this
/// to deserialize the thought payload.
///
/// # Example
/// ```
/// use anyclaw_sdk_types::channel::ThoughtContent;
/// let content = serde_json::json!({
///     "sessionId": "s1",
///     "type": "agent_thought_chunk",
///     "content": "thinking..."
/// });
/// if let Some(thought) = ThoughtContent::from_content(&content) {
///     assert_eq!(thought.content, "thinking...");
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThoughtContent {
    /// ACP session that produced the thought.
    pub session_id: String,
    /// Content type discriminator (always `"agent_thought_chunk"`).
    #[serde(rename = "type")]
    pub update_type: String,
    /// The thought text content.
    pub content: String,
}

impl ThoughtContent {
    /// Try to extract thought content from a DeliverMessage content value.
    /// Returns Some if the content type is "agent_thought_chunk", None otherwise.
    pub fn from_content(content: &serde_json::Value) -> Option<Self> {
        let update_type = content.get("type")?.as_str()?;
        if update_type == "agent_thought_chunk" {
            serde_json::from_value(content.clone()).ok()
        } else {
            None
        }
    }
}

/// Typed dispatch over all content update types in a `DeliverMessage`.
///
/// Channels receive `DeliverMessage` with a JSON `content` field. Instead of
/// matching raw `content["update"]["sessionUpdate"]` strings, use
/// `ContentKind::from_content(&msg.content)` for typed dispatch.
///
/// # Example
/// ```
/// use anyclaw_sdk_types::channel::ContentKind;
/// let content = serde_json::json!({
///     "update": {
///         "sessionUpdate": "agent_message_chunk",
///         "content": "hello"
///     }
/// });
/// match ContentKind::from_content(&content) {
///     ContentKind::MessageChunk { text } => assert_eq!(text, "hello"),
///     _ => panic!("expected MessageChunk"),
/// }
/// ```
// ContentKind dispatches over raw DeliverMessage.content (Value pass-through)
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ContentKind {
    /// Agent thinking/reasoning content.
    Thought(ThoughtContent),
    /// Streaming chunk of agent response text.
    MessageChunk {
        /// The chunk text.
        text: String,
    },
    /// Final result text from the agent.
    Result {
        /// The result text.
        text: String,
    },
    /// Echo of user message chunk (for display).
    UserMessageChunk {
        /// The echoed user text.
        text: String,
    },
    /// Token usage update notification (no content fields).
    UsageUpdate,
    /// Agent invoked a tool.
    ToolCall {
        /// Tool name being called.
        name: String,
        /// Unique identifier for this tool invocation.
        tool_call_id: String,
        /// Tool input arguments, if any.
        // Extensible: tool input schema is tool-defined (D-03)
        input: Option<serde_json::Value>,
    },
    /// Progress/completion update for a tool call.
    ToolCallUpdate {
        /// Tool name.
        name: String,
        /// Identifier matching the originating `ToolCall`.
        tool_call_id: String,
        /// Execution status (e.g., `"completed"`, `"running"`).
        status: String,
        /// Tool output text, if available.
        output: Option<String>,
    },
    /// Agent-provided list of available commands (e.g., for Telegram / menu).
    AvailableCommandsUpdate {
        /// The commands payload from the agent (array of command objects).
        // Extensible: command descriptors have agent-defined schemas (D-03)
        commands: serde_json::Value,
    },
    /// Unrecognized content type.
    Unknown,
}

impl ContentKind {
    /// Classify a `DeliverMessage.content` value into a typed variant.
    ///
    /// Reads `content["update"]["sessionUpdate"]` as the type discriminator
    /// (the actual wire format both channels use).
    pub fn from_content(content: &serde_json::Value) -> Self {
        let Some(update) = content.get("update") else {
            return ContentKind::Unknown;
        };
        let Some(session_update) = update.get("sessionUpdate").and_then(|v| v.as_str()) else {
            return ContentKind::Unknown;
        };
        match session_update {
            "agent_thought_chunk" => {
                let text = extract_content_text(update);
                ContentKind::Thought(ThoughtContent {
                    session_id: String::new(),
                    update_type: "agent_thought_chunk".into(),
                    content: text,
                })
            }
            "agent_message_chunk" => ContentKind::MessageChunk {
                text: extract_content_text(update),
            },
            "result" => ContentKind::Result {
                text: extract_content_text(update),
            },
            "user_message_chunk" => ContentKind::UserMessageChunk {
                text: extract_content_text(update),
            },
            "usage_update" => ContentKind::UsageUpdate,
            "tool_call" => {
                let tool_call_id = update
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = update
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = update.get("input").cloned();
                ContentKind::ToolCall {
                    name,
                    tool_call_id,
                    input,
                }
            }
            "tool_call_update" => {
                let tool_call_id = update
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = update
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let status = update
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let output = update
                    .get("output")
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string);
                ContentKind::ToolCallUpdate {
                    name,
                    tool_call_id,
                    status,
                    output,
                }
            }
            "available_commands_update" => ContentKind::AvailableCommandsUpdate {
                commands: update
                    .get("availableCommands")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![])),
            },
            _ => ContentKind::Unknown,
        }
    }
}

/// Extract displayable text from `update["content"]`.
/// Handles content-part object format `{type, text}`,
/// plain string values, and falls back to empty string.
fn extract_content_text(update: &serde_json::Value) -> String {
    match update.get("content") {
        Some(c) => {
            if let Some(text) = c.get("text").and_then(|t| t.as_str()) {
                return text.to_string();
            }
            if let Some(s) = c.as_str() {
                return s.to_string();
            }
            String::new()
        }
        None => String::new(),
    }
}

/// Anyclaw → Channel: acknowledge message receipt.
/// Channel uses this to add emoji reaction and/or show typing indicator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AckNotification {
    /// ACP session the ack relates to.
    pub session_id: String,
    /// Channel that should display the ack.
    pub channel_name: String,
    /// Peer whose message triggered the ack.
    pub peer_id: String,
    /// Platform-specific message ID, if available.
    pub message_id: Option<String>,
}

/// Anyclaw → Channel: ack lifecycle event (e.g., response started).
/// Channel uses this to remove/replace reaction based on its ack config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AckLifecycleNotification {
    /// ACP session the lifecycle event relates to.
    pub session_id: String,
    /// Lifecycle action (e.g., `"response_started"`).
    pub action: String,
}

/// Ack configuration passed to channels via initialize handshake.
/// Lightweight mirror of config crate's AckConfig — SDK types must not depend on config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAckConfig {
    /// Whether to add emoji reactions on message receipt.
    pub reaction: bool,
    /// Whether to show typing indicators while processing.
    pub typing: bool,
    /// Emoji to use for the ack reaction (e.g., `"👀"`).
    pub reaction_emoji: String,
    /// How to handle the reaction when response starts (e.g., `"remove"`).
    pub reaction_lifecycle: String,
}

/// Channel → Anyclaw: user responded to permission prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRespondPermission {
    /// Identifier matching the originating permission request.
    pub request_id: String,
    /// The option the user selected.
    pub option_id: String,
}

/// Anyclaw → Channel: notify channel that a session was created for a peer.
/// Channels can use this to map ACP session IDs back to their internal identifiers
/// (e.g., Telegram chat IDs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreated {
    /// Newly created ACP session identifier.
    pub session_id: String,
    /// Peer whose message triggered session creation.
    pub peer_info: PeerInfo,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_serializing_channel_capabilities_then_uses_camel_case() {
        let caps = ChannelCapabilities {
            streaming: true,
            rich_text: false,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["streaming"], true);
        assert_eq!(json["richText"], false);
        assert!(json.get("rich_text").is_none());
        let deser: ChannelCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(deser, caps);
    }

    #[test]
    fn when_serializing_peer_info_then_uses_camel_case() {
        let info = PeerInfo {
            channel_name: "debug-http".into(),
            peer_id: "local:dev".into(),
            kind: "local".into(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["channelName"], "debug-http");
        assert_eq!(json["peerId"], "local:dev");
        assert!(json.get("channel_name").is_none());
        let deser: PeerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(deser, info);
    }

    #[test]
    fn when_serializing_deliver_message_then_uses_camel_case() {
        let msg = DeliverMessage {
            session_id: "sess-1".into(),
            content: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert!(json.get("session_id").is_none());
        let deser: DeliverMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn when_serializing_channel_send_message_then_uses_camel_case() {
        let msg = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "local:dev".into(),
                kind: "local".into(),
            },
            content: "hello agent".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["peerInfo"]["channelName"], "debug-http");
        assert_eq!(json["content"], "hello agent");
        let deser: ChannelSendMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn when_serializing_channel_respond_permission_then_uses_camel_case() {
        let resp = ChannelRespondPermission {
            request_id: "req-1".into(),
            option_id: "allow".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["requestId"], "req-1");
        assert_eq!(json["optionId"], "allow");
        assert!(json.get("request_id").is_none());
        let deser: ChannelRespondPermission = serde_json::from_value(json).unwrap();
        assert_eq!(deser, resp);
    }

    #[test]
    fn when_serializing_channel_initialize_params_then_uses_camel_case() {
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "ch-1".into(),
            ack: None,
            options: std::collections::HashMap::new(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["channelId"], "ch-1");
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser, params);
    }

    #[test]
    fn when_content_type_is_agent_thought_chunk_then_extracts_thought() {
        let content = serde_json::json!({
            "sessionId": "s1",
            "type": "agent_thought_chunk",
            "content": "Analyzing..."
        });
        let thought = ThoughtContent::from_content(&content).unwrap();
        assert_eq!(thought.session_id, "s1");
        assert_eq!(thought.update_type, "agent_thought_chunk");
        assert_eq!(thought.content, "Analyzing...");
    }

    #[test]
    fn when_content_type_is_not_agent_thought_chunk_then_returns_none() {
        let content = serde_json::json!({
            "sessionId": "s1",
            "type": "agent_message_chunk",
            "content": "Hello"
        });
        assert!(ThoughtContent::from_content(&content).is_none());
    }

    #[test]
    fn when_serializing_thought_content_then_uses_camel_case() {
        let thought = ThoughtContent {
            session_id: "s1".into(),
            update_type: "agent_thought_chunk".into(),
            content: "Thinking...".into(),
        };
        let json = serde_json::to_value(&thought).unwrap();
        assert_eq!(json["sessionId"], "s1");
        assert_eq!(json["type"], "agent_thought_chunk");
        let deser: ThoughtContent = serde_json::from_value(json).unwrap();
        assert_eq!(deser, thought);
    }

    #[test]
    fn when_deliver_message_content_is_thought_then_extracts_thought() {
        let msg = DeliverMessage {
            session_id: "sess-1".into(),
            content: serde_json::json!({
                "sessionId": "sess-1",
                "type": "agent_thought_chunk",
                "content": "deep thought"
            }),
        };
        let thought = ThoughtContent::from_content(&msg.content).unwrap();
        assert_eq!(thought.content, "deep thought");
    }

    #[test]
    fn when_serializing_channel_initialize_result_then_uses_camel_case() {
        let result = ChannelInitializeResult {
            protocol_version: 1,
            capabilities: ChannelCapabilities {
                streaming: true,
                rich_text: false,
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["capabilities"]["streaming"], true);
        let deser: ChannelInitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(deser, result);
    }

    #[test]
    fn when_serializing_ack_notification_then_uses_camel_case() {
        let ack = AckNotification {
            session_id: "sess-1".into(),
            channel_name: "telegram".into(),
            peer_id: "telegram:12345".into(),
            message_id: Some("msg-42".into()),
        };
        let json = serde_json::to_value(&ack).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["channelName"], "telegram");
        assert_eq!(json["peerId"], "telegram:12345");
        assert_eq!(json["messageId"], "msg-42");
        assert!(json.get("session_id").is_none());
        let deser: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser, ack);
    }

    #[test]
    fn when_ack_notification_has_no_message_id_then_field_is_null() {
        let ack = AckNotification {
            session_id: "sess-1".into(),
            channel_name: "debug-http".into(),
            peer_id: "local".into(),
            message_id: None,
        };
        let json = serde_json::to_value(&ack).unwrap();
        assert!(json["messageId"].is_null());
        let deser: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser.message_id, None);
    }

    #[test]
    fn when_serializing_ack_lifecycle_notification_then_uses_camel_case() {
        let lifecycle = AckLifecycleNotification {
            session_id: "sess-1".into(),
            action: "response_started".into(),
        };
        let json = serde_json::to_value(&lifecycle).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["action"], "response_started");
        let deser: AckLifecycleNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser, lifecycle);
    }

    #[test]
    fn when_serializing_channel_ack_config_then_uses_camel_case() {
        let cfg = ChannelAckConfig {
            reaction: true,
            typing: true,
            reaction_emoji: "👀".into(),
            reaction_lifecycle: "remove".into(),
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["reaction"], true);
        assert_eq!(json["typing"], true);
        assert_eq!(json["reactionEmoji"], "👀");
        assert_eq!(json["reactionLifecycle"], "remove");
        assert!(json.get("reaction_emoji").is_none());
        let deser: ChannelAckConfig = serde_json::from_value(json).unwrap();
        assert_eq!(deser, cfg);
    }

    #[test]
    fn when_channel_initialize_params_has_ack_then_ack_serialized_nested() {
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: Some(ChannelAckConfig {
                reaction: true,
                typing: true,
                reaction_emoji: "👀".into(),
                reaction_lifecycle: "remove".into(),
            }),
            options: std::collections::HashMap::new(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["ack"]["reaction"], true);
        assert_eq!(json["ack"]["reactionEmoji"], "👀");
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser, params);
    }

    #[test]
    fn when_channel_initialize_params_has_no_ack_field_then_ack_is_none() {
        let json = serde_json::json!({
            "protocolVersion": 1,
            "channelId": "debug-http"
        });
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser.ack, None);
    }

    #[rstest]
    fn when_content_is_thought_chunk_then_returns_thought() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "agent_thought_chunk",
                "content": "analyzing the problem..."
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::Thought(t) => assert_eq!(t.content, "analyzing the problem..."),
            other => panic!("expected Thought, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_message_chunk_then_returns_message_chunk() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": "hello world"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::MessageChunk { text } => assert_eq!(text, "hello world"),
            other => panic!("expected MessageChunk, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_result_then_returns_result() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "result",
                "content": "done"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::Result { text } => assert_eq!(text, "done"),
            other => panic!("expected Result, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_usage_update_then_returns_usage_update() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "usage_update",
                "content": {}
            }
        });
        let kind = ContentKind::from_content(&content);
        assert_eq!(kind, ContentKind::UsageUpdate);
    }

    #[rstest]
    fn when_content_is_user_message_chunk_then_returns_user_message_chunk() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "user_message_chunk",
                "content": "user said this"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::UserMessageChunk { text } => assert_eq!(text, "user said this"),
            other => panic!("expected UserMessageChunk, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_has_unknown_update_type_then_returns_unknown() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "some_future_type",
                "content": "whatever"
            }
        });
        assert_eq!(ContentKind::from_content(&content), ContentKind::Unknown);
    }

    #[rstest]
    fn when_content_has_no_update_key_then_returns_unknown() {
        let content = serde_json::json!({"text": "plain message"});
        assert_eq!(ContentKind::from_content(&content), ContentKind::Unknown);
    }

    #[rstest]
    fn when_content_is_tool_call_then_returns_tool_call() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call",
                "toolCallId": "tc-1",
                "name": "read_file",
                "input": {"path": "/tmp/foo.txt"}
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::ToolCall {
                name,
                tool_call_id,
                input,
            } => {
                assert_eq!(name, "read_file");
                assert_eq!(tool_call_id, "tc-1");
                assert!(input.is_some());
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_tool_call_without_optional_fields_then_returns_tool_call_with_defaults() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::ToolCall {
                name,
                tool_call_id,
                input,
            } => {
                assert_eq!(name, "");
                assert_eq!(tool_call_id, "");
                assert!(input.is_none());
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_tool_call_update_then_returns_tool_call_update() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "tc-1",
                "name": "read_file",
                "status": "completed",
                "output": "file contents here"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::ToolCallUpdate {
                name,
                tool_call_id,
                status,
                output,
            } => {
                assert_eq!(name, "read_file");
                assert_eq!(tool_call_id, "tc-1");
                assert_eq!(status, "completed");
                assert_eq!(output.as_deref(), Some("file contents here"));
            }
            other => panic!("expected ToolCallUpdate, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_tool_call_update_without_optional_fields_then_returns_defaults() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "tc-2"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::ToolCallUpdate {
                name,
                tool_call_id,
                status,
                output,
            } => {
                assert_eq!(tool_call_id, "tc-2");
                assert_eq!(name, "");
                assert_eq!(status, "");
                assert!(output.is_none());
            }
            other => panic!("expected ToolCallUpdate, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_is_thought_with_opencode_wrapper_then_extracts_text() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "agent_thought_chunk",
                "content": {"type": "text", "text": "wrapped thought"}
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::Thought(t) => assert_eq!(t.content, "wrapped thought"),
            other => panic!("expected Thought, got {:?}", other),
        }
    }

    #[test]
    fn when_session_created_serialized_then_uses_camel_case() {
        let sc = SessionCreated {
            session_id: "acp-sess-42".into(),
            peer_info: PeerInfo {
                channel_name: "telegram".into(),
                peer_id: "tg:99999".into(),
                kind: "user".into(),
            },
        };
        let json = serde_json::to_value(&sc).unwrap();
        assert_eq!(json["sessionId"], "acp-sess-42");
        assert!(json.get("session_id").is_none());
        assert_eq!(json["peerInfo"]["channelName"], "telegram");
        assert_eq!(json["peerInfo"]["peerId"], "tg:99999");
        assert!(json.get("peer_info").is_none());
        let deser: SessionCreated = serde_json::from_value(json).unwrap();
        assert_eq!(deser, sc);
    }

    #[rstest]
    fn when_content_has_available_commands_update_then_content_kind_is_available_commands_update() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "available_commands_update",
                "availableCommands": [{"name": "start", "description": "Start the bot"}]
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::AvailableCommandsUpdate { commands } => {
                assert!(commands.is_array());
                assert_eq!(commands.as_array().unwrap().len(), 1);
            }
            other => panic!("expected AvailableCommandsUpdate, got {:?}", other),
        }
    }

    #[rstest]
    fn when_content_has_available_commands_update_without_commands_then_defaults_to_empty_array() {
        let content = serde_json::json!({
            "update": {
                "sessionUpdate": "available_commands_update"
            }
        });
        let kind = ContentKind::from_content(&content);
        match kind {
            ContentKind::AvailableCommandsUpdate { commands } => {
                assert_eq!(commands, serde_json::Value::Array(vec![]));
            }
            other => panic!("expected AvailableCommandsUpdate, got {:?}", other),
        }
    }

    // ── Round-trip serde tests (04-02 Task 1) ──────────────────────────

    #[rstest]
    fn when_channel_capabilities_round_trips_then_identical() {
        let original = ChannelCapabilities {
            streaming: true,
            rich_text: false,
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_initialize_params_round_trips_then_identical() {
        let mut opts = std::collections::HashMap::new();
        opts.insert("token".into(), serde_json::json!("abc123"));
        let original = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: Some(ChannelAckConfig {
                reaction: true,
                typing: false,
                reaction_emoji: "👀".into(),
                reaction_lifecycle: "remove".into(),
            }),
            options: opts,
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_initialize_params_empty_options_round_trips_then_identical() {
        let original = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "debug-http".into(),
            ack: None,
            options: std::collections::HashMap::new(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_initialize_result_round_trips_then_identical() {
        let original = ChannelInitializeResult {
            protocol_version: 1,
            capabilities: ChannelCapabilities {
                streaming: true,
                rich_text: true,
            },
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelInitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_deliver_message_round_trips_then_identical() {
        let original = DeliverMessage {
            session_id: "ses-1".into(),
            content: serde_json::json!({"update": {"sessionUpdate": "result", "content": "done"}}),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: DeliverMessage = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_peer_info_round_trips_then_identical() {
        let original = PeerInfo {
            channel_name: "telegram".into(),
            peer_id: "user-42".into(),
            kind: "direct".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: PeerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_send_message_round_trips_then_identical() {
        let original = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "dev".into(),
                kind: "local".into(),
            },
            content: "hello agent".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelSendMessage = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_thought_content_round_trips_then_identical() {
        let original = ThoughtContent {
            session_id: "ses-1".into(),
            update_type: "agent_thought_chunk".into(),
            content: "analyzing...".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ThoughtContent = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_ack_notification_round_trips_then_identical() {
        let original = AckNotification {
            session_id: "ses-1".into(),
            channel_name: "telegram".into(),
            peer_id: "alice".into(),
            message_id: Some("msg-42".into()),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_ack_notification_no_message_id_round_trips_then_identical() {
        let original = AckNotification {
            session_id: "ses-1".into(),
            channel_name: "debug-http".into(),
            peer_id: "dev".into(),
            message_id: None,
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_ack_lifecycle_notification_round_trips_then_identical() {
        let original = AckLifecycleNotification {
            session_id: "ses-1".into(),
            action: "response_started".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: AckLifecycleNotification = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_ack_config_round_trips_then_identical() {
        let original = ChannelAckConfig {
            reaction: true,
            typing: true,
            reaction_emoji: "👀".into(),
            reaction_lifecycle: "remove".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelAckConfig = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_channel_respond_permission_round_trips_then_identical() {
        let original = ChannelRespondPermission {
            request_id: "req-1".into(),
            option_id: "allow".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ChannelRespondPermission = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_created_round_trips_then_identical() {
        let original = SessionCreated {
            session_id: "acp-sess-42".into(),
            peer_info: PeerInfo {
                channel_name: "telegram".into(),
                peer_id: "tg:99999".into(),
                kind: "direct".into(),
            },
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionCreated = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }
}
