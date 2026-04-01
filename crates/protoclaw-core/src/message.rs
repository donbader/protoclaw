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
    PermissionRequest { prompt: String },
    PermissionResponse { granted: bool },
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

    #[test]
    fn internal_message_construction() {
        let source = ChannelId::from("telegram");
        let msg = InternalMessage::text(source.clone(), "hello");

        assert_eq!(msg.source, source);
        assert!(matches!(msg.content, MessageContent::Text(ref t) if t == "hello"));
        assert!(msg.metadata.session_id.is_none());
        assert!(msg.metadata.reply_to.is_none());
    }

    #[test]
    fn internal_message_with_metadata() {
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
    fn message_content_permission_response() {
        let content = MessageContent::PermissionResponse { granted: true };
        assert!(matches!(
            content,
            MessageContent::PermissionResponse { granted: true }
        ));
    }

    #[test]
    fn internal_message_serializes_to_json() {
        let msg = InternalMessage::text(ChannelId::from("test"), "hi");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"source\""));
        assert!(json.contains("\"content\""));
    }
}
