use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    pub capabilities: ClientCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptCapabilities {
    #[serde(rename = "embeddedContext", skip_serializing_if = "Option::is_none")]
    pub embedded_context: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<serde_json::Value>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewParams {
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub cwd: String,
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: Vec<McpServerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text { text: String },
    Image { url: String },
}

impl ContentPart {
    pub fn text(s: impl Into<String>) -> Self {
        ContentPart::Text { text: s.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionPromptParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub prompt: Vec<ContentPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionCancelParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionLoadParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
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
    CurrentModeUpdate {
        #[serde(default)]
        mode: Option<String>,
    },
    ConfigOptionUpdate {
        #[serde(default, flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
    SessionInfoUpdate {
        #[serde(default, flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
}

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
}
