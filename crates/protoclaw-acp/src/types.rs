use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// === Client → Agent types ===

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
    #[serde(rename = "mcpServers", skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionPromptParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub message: PromptMessage,
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
pub struct SessionCloseParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

// === Agent → Client types ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionUpdateType {
    AgentMessageChunk {
        content: String,
    },
    AgentThoughtChunk {
        content: String,
    },
    ToolCallUpdate {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        status: ToolCallStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    Plan {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUpdateEvent {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(flatten)]
    pub update: SessionUpdateType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionOption {
    #[serde(rename = "optionId")]
    pub option_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRequest {
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub description: String,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionResponse {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "optionId")]
    pub option_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsReadRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsWriteRequest {
    pub path: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_params_serializes_camelcase() {
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert!(json.get("protocol_version").is_none());
    }

    #[test]
    fn initialize_result_deserializes_opencode_response() {
        let json = r#"{
            "protocolVersion": 1,
            "loadSession": true,
            "mcpCapabilities": { "http": true, "sse": true },
            "promptCapabilities": { "embeddedContext": true, "image": true },
            "sessionCapabilities": { "fork": {}, "list": {}, "resume": {} }
        }"#;
        let result: InitializeResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.protocol_version, 1);
        assert_eq!(result.load_session, Some(true));
        let mcp = result.mcp_capabilities.unwrap();
        assert_eq!(mcp.http, Some(true));
        assert_eq!(mcp.sse, Some(true));
        let prompt = result.prompt_capabilities.unwrap();
        assert_eq!(prompt.embedded_context, Some(true));
        assert_eq!(prompt.image, Some(true));
        assert!(result.session_capabilities.is_some());
    }

    #[test]
    fn session_new_params_includes_mcp_servers() {
        let params = SessionNewParams {
            session_id: Some("s1".to_string()),
            mcp_servers: Some(vec![McpServerInfo {
                name: "workspace".to_string(),
                server_type: "http".to_string(),
                url: "http://127.0.0.1:9000".to_string(),
                headers: None,
            }]),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["sessionId"], "s1");
        assert_eq!(json["mcpServers"][0]["name"], "workspace");
        assert_eq!(json["mcpServers"][0]["type"], "http");
        assert_eq!(json["mcpServers"][0]["url"], "http://127.0.0.1:9000");
    }

    #[test]
    fn session_update_deserializes_agent_message_chunk() {
        let json = r#"{"sessionId":"s1","type":"agent_message_chunk","content":"hello"}"#;
        let event: SessionUpdateEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.session_id, "s1");
        match event.update {
            SessionUpdateType::AgentMessageChunk { content } => {
                assert_eq!(content, "hello");
            }
            _ => panic!("expected AgentMessageChunk"),
        }
    }

    #[test]
    fn session_update_deserializes_tool_call_update() {
        let json = r#"{
            "sessionId": "s1",
            "type": "tool_call_update",
            "toolCallId": "tc-1",
            "name": "read_file",
            "status": "in_progress",
            "input": {"path": "/tmp/test.rs"}
        }"#;
        let event: SessionUpdateEvent = serde_json::from_str(json).unwrap();
        match event.update {
            SessionUpdateType::ToolCallUpdate {
                tool_call_id,
                name,
                status,
                input,
                output,
            } => {
                assert_eq!(tool_call_id, "tc-1");
                assert_eq!(name, Some("read_file".to_string()));
                assert_eq!(status, ToolCallStatus::InProgress);
                assert!(input.is_some());
                assert!(output.is_none());
            }
            _ => panic!("expected ToolCallUpdate"),
        }
    }

    #[test]
    fn session_update_deserializes_usage_update() {
        let json = r#"{
            "sessionId": "s1",
            "type": "usage_update",
            "inputTokens": 100,
            "outputTokens": 50
        }"#;
        let event: SessionUpdateEvent = serde_json::from_str(json).unwrap();
        match event.update {
            SessionUpdateType::UsageUpdate {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(input_tokens, Some(100));
                assert_eq!(output_tokens, Some(50));
            }
            _ => panic!("expected UsageUpdate"),
        }
    }

    #[test]
    fn session_update_deserializes_result() {
        let json = r#"{"sessionId":"s1","type":"result","content":"done"}"#;
        let event: SessionUpdateEvent = serde_json::from_str(json).unwrap();
        match event.update {
            SessionUpdateType::Result { content } => {
                assert_eq!(content, Some("done".to_string()));
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn permission_request_deserializes() {
        let json = r#"{
            "requestId": "perm-1",
            "description": "Allow file write to /tmp/test.rs?",
            "options": [
                {"optionId": "allow_once", "label": "Allow once"},
                {"optionId": "reject_once", "label": "Reject"}
            ]
        }"#;
        let req: PermissionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.request_id, "perm-1");
        assert_eq!(req.options.len(), 2);
        assert_eq!(req.options[0].option_id, "allow_once");
    }

    #[test]
    fn permission_response_serializes_camelcase() {
        let resp = PermissionResponse {
            request_id: "perm-1".to_string(),
            option_id: "allow_once".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["requestId"], "perm-1");
        assert_eq!(json["optionId"], "allow_once");
        assert!(json.get("request_id").is_none());
    }

    #[test]
    fn fs_write_request_deserializes() {
        let json = r#"{"path": "/tmp/test.rs", "content": "fn main() {}"}"#;
        let req: FsWriteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "/tmp/test.rs");
        assert_eq!(req.content, "fn main() {}");
    }

    #[test]
    fn tool_call_status_serializes_lowercase() {
        let status = ToolCallStatus::InProgress;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "in_progress");
    }

    #[test]
    fn session_prompt_params_serializes_camelcase() {
        let params = SessionPromptParams {
            session_id: "s1".to_string(),
            message: PromptMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["sessionId"], "s1");
        assert!(json.get("session_id").is_none());
    }
}
