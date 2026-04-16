//! ACP (Agent Client Protocol) wire types.
//!
//! These types define the JSON-RPC 2.0 message structures used in the ACP protocol
//! for communication between the anyclaw supervisor and AI agent subprocesses.
//!
//! All serializable types use `camelCase` JSON field names matching the wire format.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use agent_client_protocol_schema::{AgentCapabilities, McpCapabilities, PromptCapabilities};

/// Session capabilities supported by the agent.
///
/// Anyclaw keeps a local definition because the `fork` and `resume` fields are
/// behind unstable feature flags in the official crate. The stable-only
/// `official::SessionCapabilities` only exposes `list`, which is wire-compatible.
pub use agent_client_protocol_schema::SessionCapabilities;

/// Capabilities advertised by the supervisor to the agent during `initialize`.
// Extensible: experimental capabilities have agent-defined schemas (D-03)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    /// Experimental capability extensions; omitted when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for the `initialize` request (supervisor → agent).
// Extensible: runtime options have deployment-defined schemas (D-03)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    /// ACP protocol version the supervisor is requesting.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    /// Capabilities the supervisor is advertising to the agent.
    pub capabilities: ClientCapabilities,
    /// Arbitrary runtime options forwarded from `anyclaw.yaml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// Result returned by the agent in response to `initialize`.
///
/// Uses the official `AgentCapabilities` type (from `agent_client_protocol_schema`)
/// nested under `agentCapabilities`, matching the wire format real ACP agents emit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    /// ACP protocol version the agent has accepted.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    /// Capabilities advertised by the agent, nested per the ACP wire format.
    #[serde(
        rename = "agentCapabilities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_capabilities: Option<AgentCapabilities>,
    /// Default option values reported by the agent during `initialize`.
    ///
    /// The manager merges these into the agent's options map; user-provided
    /// options always take precedence over defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<HashMap<String, serde_json::Value>>,
}

/// Describes a single MCP server to be passed to the agent on `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerInfo {
    /// Human-readable name identifying this MCP server.
    pub name: String,
    /// Transport type, e.g. `"stdio"` or `"sse"`.
    #[serde(rename = "type")]
    pub server_type: String,
    /// URL for HTTP/SSE transports; empty for stdio.
    pub url: String,
    /// Executable path for stdio transport.
    #[serde(default)]
    pub command: String,
    /// Command-line arguments for stdio transport.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for stdio transport.
    #[serde(default)]
    pub env: Vec<String>,
    /// HTTP headers for HTTP/SSE transports, as `[name, value]` pairs.
    #[serde(default)]
    pub headers: Vec<Vec<String>>,
}

/// Parameters for the `session/new` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewParams {
    /// Optional client-provided session ID; agent may use it or generate its own.
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Working directory the agent should use for this session.
    pub cwd: String,
    /// MCP servers available to the agent for this session.
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: Vec<McpServerInfo>,
}

/// Result returned by the agent in response to `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewResult {
    /// Session ID assigned by the agent for the new session.
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// A single content element in a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    /// Plain text content.
    Text {
        /// The text string.
        text: String,
    },
    /// Image content referenced by URL.
    Image {
        /// URL pointing to the image resource.
        url: String,
    },
    /// File content referenced by URL.
    File {
        /// URL pointing to the file resource.
        url: String,
        /// Optional filename hint for display or download.
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        /// Optional MIME type of the file.
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    /// Audio content referenced by URL.
    Audio {
        /// URL pointing to the audio resource.
        url: String,
        /// Optional MIME type of the audio.
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

impl ContentPart {
    /// Convenience constructor for text content.
    pub fn text(s: impl Into<String>) -> Self {
        ContentPart::Text { text: s.into() }
    }
}

impl Default for ContentPart {
    fn default() -> Self {
        ContentPart::Text {
            text: String::new(),
        }
    }
}

/// Parameters for the `session/prompt` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionPromptParams {
    /// ID of the session to send the prompt to.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Content parts forming the prompt message.
    pub prompt: Vec<ContentPart>,
}

/// Parameters for the `session/cancel` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionCancelParams {
    /// ID of the session whose active operation should be cancelled.
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Parameters for the `session/fork` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionForkParams {
    /// ID of the session to fork.
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Result returned by the agent in response to `session/fork`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionForkResult {
    /// ID of the newly forked session.
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Parameters for the `session/list` request (supervisor → agent).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionListParams {}

/// Result returned by the agent in response to `session/list`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionListResult {
    /// List of sessions currently known to the agent.
    pub sessions: Vec<SessionInfo>,
}

/// Metadata for a single session returned in `SessionListResult`.
// Extensible: agent-defined metadata has agent-specific schemas (D-03)
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    /// Unique identifier for this session.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Arbitrary agent-defined metadata for this session.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Parameters for the `session/load` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionLoadParams {
    /// ID of the session to load/resume.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Working directory for the resumed session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// MCP servers available for the resumed session.
    #[serde(rename = "mcpServers", skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerInfo>>,
}

/// Status of a tool call, used in `SessionUpdateType::ToolCallUpdate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// Tool call has been queued but not yet started.
    Pending,
    /// Tool call is currently executing.
    InProgress,
    /// Tool call finished successfully.
    Completed,
    /// Tool call terminated with an error.
    Failed,
}

/// Variants of streaming update events sent by the agent via `session/update`.
// Extensible: tool inputs, command descriptors, and config/session extras have agent-defined schemas (D-03)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionUpdateType {
    /// A chunk of the agent's outgoing message text.
    AgentMessageChunk {
        /// Partial message content for this chunk.
        #[serde(default)]
        content: ContentPart,
        /// Optional message ID grouping chunks belonging to the same message.
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    /// A chunk of the agent's internal reasoning/thought stream.
    AgentThoughtChunk {
        /// Partial thought content for this chunk.
        #[serde(default)]
        content: ContentPart,
        /// Optional message ID grouping chunks belonging to the same thought.
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    /// Notification that the agent is initiating a tool call.
    ToolCall {
        /// Unique identifier for this tool call invocation.
        #[serde(rename = "toolCallId", default)]
        tool_call_id: Option<String>,
        /// Name of the tool being called.
        #[serde(default)]
        name: Option<String>,
        /// Input arguments passed to the tool.
        #[serde(default)]
        input: Option<HashMap<String, serde_json::Value>>,
    },
    /// Status update for an in-progress tool call.
    ToolCallUpdate {
        /// Identifier of the tool call being updated.
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        /// Name of the tool, if known at update time.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Current execution status of the tool call.
        #[serde(default)]
        status: Option<ToolCallStatus>,
        /// Input arguments, if available at update time.
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<HashMap<String, serde_json::Value>>,
        /// Output produced by the tool call, if completed.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Agent's execution plan or reasoning outline.
    Plan {
        /// Plan content emitted by the agent.
        #[serde(default)]
        content: ContentPart,
    },
    /// Token usage statistics for the current session turn.
    UsageUpdate {
        /// Number of input tokens consumed.
        #[serde(rename = "inputTokens", skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        /// Number of output tokens generated.
        #[serde(rename = "outputTokens", skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        /// Number of tokens read from the prompt cache.
        #[serde(rename = "cacheReadTokens", skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<u64>,
        /// Number of tokens written to the prompt cache.
        #[serde(rename = "cacheWriteTokens", skip_serializing_if = "Option::is_none")]
        cache_write_tokens: Option<u64>,
    },
    /// Extension type — not part of core ACP. Agents MAY emit this as an early completion hint before the RPC response.
    Result {
        /// The agent's concluding response content, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        /// Whether the result represents an error condition.
        /// Defaults to `false` so agents that don't send this field are treated as success.
        #[serde(default)]
        is_error: bool,
    },
    /// A chunk of the user's message being echoed back.
    UserMessageChunk {
        /// Partial user message content for this chunk.
        #[serde(default)]
        content: ContentPart,
        /// Optional message ID grouping chunks belonging to the same user message.
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    /// Current list of slash-commands the agent exposes to the channel.
    AvailableCommandsUpdate {
        /// Command descriptors; schema is agent-defined.
        #[serde(default)]
        commands: Vec<serde_json::Value>,
    },
    /// Extension type — not part of core ACP. Carries the agent's current operating mode.
    CurrentModeUpdate {
        /// The agent's current mode identifier.
        #[serde(default)]
        mode: Option<String>,
    },
    /// Extension type — not part of core ACP. Carries config option updates from the agent.
    ConfigOptionUpdate {
        /// Flattened map of config option key-value pairs.
        #[serde(default, flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
    /// Extension type — not part of core ACP. Carries session metadata updates from the agent.
    SessionInfoUpdate {
        /// Flattened map of session metadata key-value pairs.
        #[serde(default, flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
}

/// A streaming update event sent by the agent via `session/update`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUpdateEvent {
    /// ID of the session this update belongs to.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// The update payload, discriminated by `sessionUpdate` tag.
    pub update: SessionUpdateType,
}

/// Why the agent stopped generating output for this prompt turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Agent completed its response naturally.
    #[default]
    EndTurn,
    /// Agent reached the maximum token limit.
    MaxTokens,
    /// Agent reached the maximum number of turn requests.
    MaxTurnRequests,
    /// Agent refused to respond to the prompt.
    Refusal,
    /// The prompt was cancelled before completion.
    Cancelled,
}

/// Parsed body of a successful `session/prompt` JSON-RPC response.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    /// Why the agent stopped. Defaults to `EndTurn` when absent.
    #[serde(default)]
    pub stop_reason: StopReason,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_text_content_part_serialized_then_includes_type_tag() {
        let part = ContentPart::text("hello");
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json, serde_json::json!({"type": "text", "text": "hello"}));
    }

    #[test]
    fn when_session_prompt_params_serialized_then_matches_wire_format() {
        let params = SessionPromptParams {
            session_id: "ses-1".into(),
            prompt: vec![ContentPart::text("hi")],
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["sessionId"], "ses-1");
        let prompt = &json["prompt"];
        assert!(prompt.is_array());
        assert_eq!(prompt[0]["type"], "text");
        assert_eq!(prompt[0]["text"], "hi");
    }

    #[test]
    fn when_session_prompt_params_serialized_then_no_role_wrapper_present() {
        let params = SessionPromptParams {
            session_id: "ses-1".into(),
            prompt: vec![ContentPart::text("hi")],
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json["prompt"][0].get("role").is_none());
        assert!(json["prompt"][0].get("content").is_none());
    }

    #[test]
    fn when_image_content_part_serialized_then_produces_correct_json() {
        let part = ContentPart::Image {
            url: "http://example.com/img.png".into(),
        };
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["url"], "http://example.com/img.png");
    }

    #[rstest]
    #[case::agent_message_chunk(SessionUpdateEvent {
        session_id: "ses-abc".into(),
        update: SessionUpdateType::AgentMessageChunk {
            content: ContentPart::text("hello"),
            message_id: Some("msg-1".into()),
        },
    })]
    #[case::result_variant(SessionUpdateEvent {
        session_id: "ses-xyz".into(),
        update: SessionUpdateType::Result {
            content: Some("final answer".into()),
            is_error: false,
        },
    })]
    #[case::usage_update(SessionUpdateEvent {
        session_id: "ses-usage".into(),
        update: SessionUpdateType::UsageUpdate {
            input_tokens: Some(10),
            output_tokens: Some(20),
            cache_read_tokens: None,
            cache_write_tokens: None,
        },
    })]
    fn when_session_update_event_serialized_then_round_trips_correctly(
        #[case] event: SessionUpdateEvent,
    ) {
        let json = serde_json::to_string(&event).expect("serialize");
        let deserialized: SessionUpdateEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, deserialized);
    }

    #[rstest]
    fn when_session_fork_params_serialized_then_session_id_is_camel_case() {
        let params = SessionForkParams {
            session_id: "ses-1".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["sessionId"], "ses-1");
    }

    #[rstest]
    fn when_session_fork_result_deserialized_then_session_id_populated() {
        let json = serde_json::json!({"sessionId": "ses-forked"});
        let result: SessionForkResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.session_id, "ses-forked");
    }

    #[rstest]
    fn when_session_list_result_deserialized_then_sessions_populated() {
        let json = serde_json::json!({
            "sessions": [
                {"sessionId": "ses-1", "metadata": {}},
                {"sessionId": "ses-2"}
            ]
        });
        let result: SessionListResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.sessions.len(), 2);
        assert_eq!(result.sessions[0].session_id, "ses-1");
        assert_eq!(result.sessions[1].session_id, "ses-2");
    }

    // ── Round-trip tests for typed replacements (Task 1) ──────────────

    #[rstest]
    fn when_client_capabilities_with_no_experimental_serialized_then_field_omitted() {
        let caps = ClientCapabilities { experimental: None };
        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("experimental").is_none());
    }

    #[rstest]
    fn when_client_capabilities_with_experimental_round_trips() {
        let mut exp = HashMap::new();
        exp.insert("foo".into(), serde_json::json!(42));
        let caps = ClientCapabilities {
            experimental: Some(exp),
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: ClientCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, back);
    }

    #[rstest]
    fn when_initialize_params_with_no_options_round_trips() {
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: InitializeParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, back);
    }

    #[rstest]
    fn when_initialize_params_with_options_round_trips() {
        let mut opts = HashMap::new();
        opts.insert("key".into(), serde_json::json!("val"));
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: Some(opts),
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: InitializeParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, back);
    }

    #[rstest]
    fn when_session_info_metadata_round_trips_as_hashmap() {
        let json = serde_json::json!({
            "sessionId": "ses-1",
            "metadata": {"key": "value"}
        });
        let info: SessionInfo = serde_json::from_value(json).unwrap();
        assert_eq!(
            info.metadata.get("key").and_then(|v| v.as_str()),
            Some("value")
        );
    }

    #[rstest]
    fn when_session_info_missing_metadata_then_defaults_to_empty() {
        let json = serde_json::json!({"sessionId": "ses-1"});
        let info: SessionInfo = serde_json::from_value(json).unwrap();
        assert!(info.metadata.is_empty());
    }

    #[rstest]
    fn when_agent_message_chunk_content_round_trips_as_content_part() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::AgentMessageChunk {
                content: ContentPart::text("hello"),
                message_id: Some("msg-1".into()),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_agent_thought_chunk_content_round_trips_as_content_part() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::AgentThoughtChunk {
                content: ContentPart::text("thinking..."),
                message_id: None,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_tool_call_input_round_trips_as_typed_hashmap() {
        let mut input = HashMap::new();
        input.insert("arg1".into(), serde_json::json!("val1"));
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::ToolCall {
                tool_call_id: Some("tc-1".into()),
                name: Some("my_tool".into()),
                input: Some(input),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_available_commands_update_round_trips_as_vec() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::AvailableCommandsUpdate {
                commands: vec![serde_json::json!({"name": "/help"})],
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_config_option_update_extra_round_trips_via_flatten() {
        let json_str = r#"{"sessionId":"ses-1","update":{"sessionUpdate":"config_option_update","key":"value"}}"#;
        let event: SessionUpdateEvent = serde_json::from_str(json_str).unwrap();
        if let SessionUpdateType::ConfigOptionUpdate { ref extra } = event.update {
            assert_eq!(extra.get("key").and_then(|v| v.as_str()), Some("value"));
        } else {
            panic!("expected ConfigOptionUpdate");
        }
        // Round-trip
        let back_json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&back_json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_session_info_update_extra_round_trips_via_flatten() {
        let json_str = r#"{"sessionId":"ses-1","update":{"sessionUpdate":"session_info_update","info":"data"}}"#;
        let event: SessionUpdateEvent = serde_json::from_str(json_str).unwrap();
        if let SessionUpdateType::SessionInfoUpdate { ref extra } = event.update {
            assert_eq!(extra.get("info").and_then(|v| v.as_str()), Some("data"));
        } else {
            panic!("expected SessionInfoUpdate");
        }
        let back_json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&back_json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_plan_content_round_trips_as_content_part() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::Plan {
                content: ContentPart::text("step 1: do thing"),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_user_message_chunk_content_round_trips_as_content_part() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::UserMessageChunk {
                content: ContentPart::text("user says hi"),
                message_id: Some("umsg-1".into()),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[rstest]
    fn when_content_part_default_then_empty_text() {
        let part = ContentPart::default();
        assert_eq!(
            part,
            ContentPart::Text {
                text: String::new()
            }
        );
    }

    // ── Existing tests ──────────────────────────────────────────────

    // ── Round-trip serde tests (04-02 Task 1) ──────────────────────────

    #[rstest]
    fn when_client_capabilities_round_trips_then_identical() {
        let mut exp = HashMap::new();
        exp.insert("beta".into(), serde_json::json!(true));
        let original = ClientCapabilities {
            experimental: Some(exp),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: ClientCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_client_capabilities_none_round_trips_then_identical() {
        let original = ClientCapabilities { experimental: None };
        let json = serde_json::to_string(&original).unwrap();
        let restored: ClientCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_initialize_params_round_trips_then_identical() {
        let mut opts = HashMap::new();
        opts.insert("model".into(), serde_json::json!("gpt-4"));
        let original = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: Some(opts),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_initialize_result_round_trips_then_identical() {
        let original = InitializeResult {
            protocol_version: 1,
            agent_capabilities: None,
            defaults: None,
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: InitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_mcp_server_info_round_trips_then_identical() {
        let original = McpServerInfo {
            name: "tools-server".into(),
            server_type: "sse".into(),
            url: "http://localhost:8080".into(),
            command: String::new(),
            args: vec![],
            env: vec![],
            headers: vec![vec!["Authorization".into(), "Bearer tok".into()]],
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: McpServerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_new_params_with_session_id_round_trips_then_identical() {
        let original = SessionNewParams {
            session_id: Some("ses-custom".into()),
            cwd: "/tmp".into(),
            mcp_servers: vec![McpServerInfo {
                name: "s1".into(),
                server_type: "stdio".into(),
                url: String::new(),
                command: "/usr/bin/tool".into(),
                args: vec!["--flag".into()],
                env: vec!["KEY=VAL".into()],
                headers: vec![],
            }],
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionNewParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_new_params_without_session_id_round_trips_then_identical() {
        let original = SessionNewParams {
            session_id: None,
            cwd: "/home/user".into(),
            mcp_servers: vec![],
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionNewParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_new_result_round_trips_then_identical() {
        let original = SessionNewResult {
            session_id: "ses-abc123".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionNewResult = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn when_file_content_part_serialized_then_includes_type_tag() {
        let part = ContentPart::File {
            url: "https://example.com/doc.pdf".into(),
            filename: Some("doc.pdf".into()),
            mime_type: Some("application/pdf".into()),
        };
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "file");
        assert_eq!(json["url"], "https://example.com/doc.pdf");
        assert_eq!(json["filename"], "doc.pdf");
        assert_eq!(json["mimeType"], "application/pdf");
    }

    #[test]
    fn when_audio_content_part_serialized_then_includes_type_tag() {
        let part = ContentPart::Audio {
            url: "https://example.com/clip.mp3".into(),
            mime_type: Some("audio/mpeg".into()),
        };
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "audio");
        assert_eq!(json["url"], "https://example.com/clip.mp3");
        assert_eq!(json["mimeType"], "audio/mpeg");
    }

    #[test]
    fn when_file_content_part_with_no_optionals_then_fields_absent() {
        let part = ContentPart::File {
            url: "https://example.com/doc.pdf".into(),
            filename: None,
            mime_type: None,
        };
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "file");
        assert_eq!(json["url"], "https://example.com/doc.pdf");
        assert!(json.get("filename").is_none());
        assert!(json.get("mimeType").is_none());
    }

    #[rstest]
    #[case::text(ContentPart::Text { text: "hello".into() })]
    #[case::image(ContentPart::Image { url: "http://example.com/img.png".into() })]
    #[case::empty_text(ContentPart::default())]
    #[case::file_full(ContentPart::File {
        url: "https://example.com/doc.pdf".into(),
        filename: Some("doc.pdf".into()),
        mime_type: Some("application/pdf".into()),
    })]
    #[case::file_url_only(ContentPart::File {
        url: "https://example.com/doc.pdf".into(),
        filename: None,
        mime_type: None,
    })]
    #[case::audio_full(ContentPart::Audio {
        url: "https://example.com/clip.mp3".into(),
        mime_type: Some("audio/mpeg".into()),
    })]
    #[case::audio_url_only(ContentPart::Audio {
        url: "https://example.com/clip.mp3".into(),
        mime_type: None,
    })]
    fn when_content_part_round_trips_then_identical(#[case] original: ContentPart) {
        let json = serde_json::to_value(&original).unwrap();
        let restored: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_prompt_params_round_trips_then_identical() {
        let original = SessionPromptParams {
            session_id: "ses-1".into(),
            prompt: vec![
                ContentPart::text("hello"),
                ContentPart::Image {
                    url: "http://img".into(),
                },
            ],
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionPromptParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_cancel_params_round_trips_then_identical() {
        let original = SessionCancelParams {
            session_id: "ses-cancel".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionCancelParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_fork_params_round_trips_then_identical() {
        let original = SessionForkParams {
            session_id: "ses-fork".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionForkParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_fork_result_round_trips_then_identical() {
        let original = SessionForkResult {
            session_id: "ses-forked".into(),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionForkResult = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_list_params_round_trips_then_identical() {
        let original = SessionListParams {};
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionListParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_list_result_round_trips_then_identical() {
        let original = SessionListResult {
            sessions: vec![
                SessionInfo {
                    session_id: "ses-1".into(),
                    metadata: HashMap::new(),
                },
                SessionInfo {
                    session_id: "ses-2".into(),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("key".into(), serde_json::json!("val"));
                        m
                    },
                },
            ],
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionListResult = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_info_round_trips_then_identical() {
        let original = SessionInfo {
            session_id: "ses-info".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("agent".into(), serde_json::json!("opencode"));
                m
            },
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionInfo = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_load_params_round_trips_then_identical() {
        let original = SessionLoadParams {
            session_id: "ses-load".into(),
            cwd: Some("/workspace".into()),
            mcp_servers: Some(vec![]),
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionLoadParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_load_params_minimal_round_trips_then_identical() {
        let original = SessionLoadParams {
            session_id: "ses-load-min".into(),
            cwd: None,
            mcp_servers: None,
        };
        let json = serde_json::to_value(&original).unwrap();
        let restored: SessionLoadParams = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    #[case::pending(ToolCallStatus::Pending)]
    #[case::in_progress(ToolCallStatus::InProgress)]
    #[case::completed(ToolCallStatus::Completed)]
    #[case::failed(ToolCallStatus::Failed)]
    fn when_tool_call_status_round_trips_then_identical(#[case] original: ToolCallStatus) {
        let json = serde_json::to_value(&original).unwrap();
        let restored: ToolCallStatus = serde_json::from_value(json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_update_event_tool_call_update_round_trips_then_identical() {
        let original = SessionUpdateEvent {
            session_id: "ses-tcu".into(),
            update: SessionUpdateType::ToolCallUpdate {
                tool_call_id: "tc-1".into(),
                name: Some("read_file".into()),
                status: Some(ToolCallStatus::Completed),
                input: None,
                output: Some("file contents".into()),
            },
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[rstest]
    fn when_session_update_event_current_mode_round_trips_then_identical() {
        let original = SessionUpdateEvent {
            session_id: "ses-mode".into(),
            update: SessionUpdateType::CurrentModeUpdate {
                mode: Some("code".into()),
            },
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: SessionUpdateEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    /// Real OpenCode `initialize` response with nested `agentCapabilities` object.
    /// The official ACP spec wraps capabilities under `agentCapabilities`; anyclaw's
    /// old flat `InitializeResult` misses them, causing a deserialization bug.
    #[test]
    fn when_opencode_initialize_response_deserialized_then_agent_capabilities_populated() {
        let json = serde_json::json!({
            "protocolVersion": 1,
            "agentCapabilities": {
                "loadSession": true,
                "promptCapabilities": {
                    "embeddedContext": true,
                    "image": false
                },
                "mcpCapabilities": {
                    "http": false,
                    "sse": true
                },
                "sessionCapabilities": {
                    "list": {}
                }
            }
        });
        let result: InitializeResult = serde_json::from_value(json).unwrap();
        let caps = result
            .agent_capabilities
            .expect("agent_capabilities should be present");
        assert!(caps.load_session, "loadSession should be true");
        assert!(
            caps.prompt_capabilities.embedded_context,
            "embeddedContext should be true"
        );
        assert!(caps.mcp_capabilities.sse, "sse should be true");
        assert!(
            caps.session_capabilities.list.is_some(),
            "session list capability should be present"
        );
    }
}
