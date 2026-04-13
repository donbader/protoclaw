//! ACP (Agent Client Protocol) wire types.
//!
//! These types define the JSON-RPC 2.0 message structures used in the ACP protocol
//! for communication between the protoclaw supervisor and AI agent subprocesses.
//!
//! All serializable types use `camelCase` JSON field names matching the wire format.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capabilities advertised by the supervisor to the agent during `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Parameters for the `initialize` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    pub capabilities: ClientCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// MCP transport capabilities advertised by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse: Option<bool>,
}

/// Prompt feature capabilities advertised by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptCapabilities {
    #[serde(rename = "embeddedContext", skip_serializing_if = "Option::is_none")]
    pub embedded_context: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
}

/// Session management capabilities advertised by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<serde_json::Value>,
}

/// Result returned by the agent in response to `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    #[serde(rename = "loadSession", skip_serializing_if = "Option::is_none")]
    pub load_session: Option<bool>,
    #[serde(rename = "mcpCapabilities", skip_serializing_if = "Option::is_none")]
    pub mcp_capabilities: Option<McpCapabilities>,
    #[serde(rename = "promptCapabilities", skip_serializing_if = "Option::is_none")]
    pub prompt_capabilities: Option<PromptCapabilities>,
    #[serde(
        rename = "sessionCapabilities",
        skip_serializing_if = "Option::is_none"
    )]
    pub session_capabilities: Option<SessionCapabilities>,
}

/// Describes a single MCP server to be passed to the agent on `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub url: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub headers: Vec<Vec<String>>,
}

/// Parameters for the `session/new` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewParams {
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub cwd: String,
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: Vec<McpServerInfo>,
}

/// Result returned by the agent in response to `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// A single content element in a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text { text: String },
    Image { url: String },
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
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub prompt: Vec<ContentPart>,
}

/// Parameters for the `session/cancel` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionCancelParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Parameters for the `session/fork` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionForkParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Result returned by the agent in response to `session/fork`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionForkResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Parameters for the `session/list` request (supervisor → agent).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionListParams {}

/// Result returned by the agent in response to `session/list`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct SessionListResult {
    pub sessions: Vec<SessionInfo>,
}

/// Metadata for a single session returned in `SessionListResult`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct SessionInfo {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Parameters for the `session/load` request (supervisor → agent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionLoadParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// Status of a tool call, used in `SessionUpdateType::ToolCallUpdate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Variants of streaming update events sent by the agent via `session/update`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionUpdateType {
    AgentMessageChunk {
        #[serde(default)]
        content: serde_json::Value,
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    AgentThoughtChunk {
        #[serde(default)]
        content: serde_json::Value,
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    ToolCall {
        #[serde(rename = "toolCallId", default)]
        tool_call_id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        input: Option<serde_json::Value>,
    },
    ToolCallUpdate {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default)]
        status: Option<ToolCallStatus>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    Plan {
        #[serde(default)]
        content: serde_json::Value,
    },
    UsageUpdate {
        #[serde(rename = "inputTokens", skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        #[serde(rename = "outputTokens", skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        #[serde(rename = "cacheReadTokens", skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<u64>,
        #[serde(rename = "cacheWriteTokens", skip_serializing_if = "Option::is_none")]
        cache_write_tokens: Option<u64>,
    },
    Result {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
    UserMessageChunk {
        #[serde(default)]
        content: serde_json::Value,
        #[serde(rename = "messageId", default)]
        message_id: Option<String>,
    },
    AvailableCommandsUpdate {
        #[serde(default)]
        commands: serde_json::Value,
    },
    /// Extension type — not part of core ACP. Carries the agent's current operating mode.
    CurrentModeUpdate {
        #[serde(default)]
        mode: Option<String>,
    },
    /// Extension type — not part of core ACP. Carries config option updates from the agent.
    ConfigOptionUpdate {
        #[serde(default, flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    /// Extension type — not part of core ACP. Carries session metadata updates from the agent.
    SessionInfoUpdate {
        #[serde(default, flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
}

/// A streaming update event sent by the agent via `session/update`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUpdateEvent {
    #[serde(rename = "sessionId")]
    pub session_id: String,
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
}
