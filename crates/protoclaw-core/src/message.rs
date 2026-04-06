use serde::{Deserialize, Serialize};

use crate::types::{ChannelId, MessageId, SessionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalMessage {
    pub id: MessageId,
    pub source: ChannelId,
    pub content: MessageContent,
    pub metadata: MessageMetadata,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    PermissionRequest {
        prompt: String,
    },
    PermissionResponse {
        granted: bool,
    },
    AgentTextDelta(String),
    AgentThoughtDelta(String),
    ToolCallStarted {
        tool_call_id: String,
        name: String,
        input: Option<serde_json::Value>,
    },
    ToolCallCompleted {
        tool_call_id: String,
        name: String,
        output: Option<String>,
        success: bool,
    },
    UsageUpdate {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    AgentResponseComplete {
        content: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub session_id: Option<SessionId>,
    pub reply_to: Option<MessageId>,
}

impl InternalMessage {
    pub fn text(source: ChannelId, text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            source,
            content: MessageContent::Text(text.into()),
            metadata: MessageMetadata::default(),
            timestamp: std::time::SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_internal_message_constructed_then_fields_set_correctly() {
        let source = ChannelId::from("telegram");
        let msg = InternalMessage::text(source.clone(), "hello");

        assert_eq!(msg.source, source);
        assert!(matches!(msg.content, MessageContent::Text(ref t) if t == "hello"));
        assert!(msg.metadata.session_id.is_none());
        assert!(msg.metadata.reply_to.is_none());
    }

    #[test]
    fn when_internal_message_has_metadata_then_metadata_accessible() {
        let msg = InternalMessage {
            id: MessageId::new(),
            source: ChannelId::from("slack"),
            content: MessageContent::PermissionRequest {
                prompt: "allow file access?".into(),
            },
            metadata: MessageMetadata {
                session_id: Some(SessionId::from("sess-1")),
                reply_to: Some(MessageId::from("msg-prev")),
            },
            timestamp: std::time::SystemTime::now(),
        };

        assert!(msg.metadata.session_id.is_some());
        assert!(msg.metadata.reply_to.is_some());
        assert!(matches!(
            msg.content,
            MessageContent::PermissionRequest { .. }
        ));
    }

    #[test]
    fn when_permission_response_content_created_then_serializes_correctly() {
        let content = MessageContent::PermissionResponse { granted: true };
        assert!(matches!(
            content,
            MessageContent::PermissionResponse { granted: true }
        ));
    }

    #[test]
    fn when_internal_message_serialized_then_produces_valid_json() {
        let msg = InternalMessage::text(ChannelId::from("test"), "hi");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"source\""));
        assert!(json.contains("\"content\""));
    }

    #[test]
    fn when_agent_text_delta_content_created_then_serializes_correctly() {
        let content = MessageContent::AgentTextDelta("chunk".to_string());
        assert!(matches!(content, MessageContent::AgentTextDelta(ref s) if s == "chunk"));
    }

    #[test]
    fn when_tool_call_started_content_created_then_serializes_correctly() {
        let content = MessageContent::ToolCallStarted {
            tool_call_id: "tc-1".to_string(),
            name: "read_file".to_string(),
            input: Some(serde_json::json!({"path": "/tmp"})),
        };
        assert!(
            matches!(content, MessageContent::ToolCallStarted { ref name, .. } if name == "read_file")
        );
    }

    #[test]
    fn when_streaming_variant_contents_serialized_then_each_produces_correct_json() {
        let content = MessageContent::AgentResponseComplete {
            content: Some("done".to_string()),
        };
        let json = serde_json::to_string(&content).unwrap();
        let roundtrip: MessageContent = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(roundtrip, MessageContent::AgentResponseComplete { content: Some(ref s) } if s == "done")
        );
    }
}
