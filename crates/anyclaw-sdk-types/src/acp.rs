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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    /// Experimental capability extensions; omitted when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Parameters for the `initialize` request (supervisor → agent).
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
}

impl ContentPart {
    /// Convenience constructor for text content.
    pub fn text(s: impl Into<String>) -> Self {
        ContentPart::Text { text: s.into() }
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
#[derive(Debug, Deserialize, PartialEq)]
pub struct SessionListResult {
    /// List of sessions currently known to the agent.
    pub sessions: Vec<SessionInfo>,
}

/// Metadata for a single session returned in `SessionListResult`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct SessionInfo {
    /// Unique identifier for this session.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Arbitrary agent-defined metadata for this session.
    #[serde(default)]
    pub metadata: serde_json::Value,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionUpdateType {
    /// A chunk of the agent's outgoing message text.
    AgentMessageChunk {
        /// Partial message content for this chunk.
        #[serde(default)]
        content: serde_json::Value,
        /// Optional message ID grouping chunks belonging to the same message.
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    /// A chunk of the agent's internal reasoning/thought stream.
    AgentThoughtChunk {
        /// Partial thought content for this chunk.
        #[serde(default)]
        content: serde_json::Value,
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
        input: Option<serde_json::Value>,
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
        input: Option<serde_json::Value>,
        /// Output produced by the tool call, if completed.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// Agent's execution plan or reasoning outline.
    Plan {
        /// Plan content emitted by the agent.
        #[serde(default)]
        content: serde_json::Value,
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
    /// Final result produced by the agent for this prompt turn.
    Result {
        /// The agent's concluding response content, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
    /// A chunk of the user's message being echoed back.
    UserMessageChunk {
        /// Partial user message content for this chunk.
        #[serde(default)]
        content: serde_json::Value,
        /// Optional message ID grouping chunks belonging to the same user message.
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    /// Current list of slash-commands the agent exposes to the channel.
    AvailableCommandsUpdate {
        /// Command descriptors; schema is agent-defined.
        #[serde(default)]
        commands: serde_json::Value,
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
        extra: serde_json::Map<String, serde_json::Value>,
    },
    /// Extension type — not part of core ACP. Carries session metadata updates from the agent.
    SessionInfoUpdate {
        /// Flattened map of session metadata key-value pairs.
        #[serde(default, flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
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
            content: serde_json::json!("hello"),
            message_id: Some("msg-1".into()),
        },
    })]
    #[case::result_variant(SessionUpdateEvent {
        session_id: "ses-xyz".into(),
        update: SessionUpdateType::Result {
            content: Some("final answer".into()),
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
