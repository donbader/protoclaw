use serde::{Deserialize, Serialize};

use crate::session_key::SessionKey;

/// Events sent from AgentsManager to ChannelsManager via mpsc channel.
/// Defined in protoclaw-sdk-types as the shared wire type for agent→channel routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelEvent {
    /// Deliver an agent session/update message to the originating channel.
    DeliverMessage {
        /// Routing key identifying which channel + conversation to target.
        session_key: SessionKey,
        /// Agent-produced content payload (streaming chunk, result, thought, etc.).
        content: serde_json::Value,
    },
    /// Signal that the agent has finished processing a prompt for this session.
    /// Triggers queue flush: marks the session idle and dispatches any queued messages.
    /// Sent once per completed `session/prompt`, after all streaming updates have been forwarded.
    SessionComplete {
        /// Routing key identifying the completed session.
        session_key: SessionKey,
    },
    /// Route a permission request to the originating channel.
    RoutePermission {
        /// Routing key identifying which channel + conversation to target.
        session_key: SessionKey,
        /// Unique identifier for correlating the permission response.
        request_id: String,
        /// Human-readable description of what is being requested.
        description: String,
        /// Permission options as a JSON array of `{optionId, label}` objects.
        options: serde_json::Value,
    },
    /// Acknowledge receipt of a message back to the originating channel.
    AckMessage {
        /// Routing key identifying the session to acknowledge.
        session_key: SessionKey,
        /// Name of the channel that sent the original message.
        channel_name: String,
        /// Peer identifier within the channel.
        peer_id: String,
        /// Optional platform-specific message identifier for targeted ack.
        message_id: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_deliver_message_event_serialized_then_deserializes_correctly() {
        let event = ChannelEvent::DeliverMessage {
            session_key: SessionKey::new("debug-http", "local", "dev"),
            content: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(deser, ChannelEvent::DeliverMessage { .. }));
    }

    #[test]
    fn when_route_permission_event_serialized_then_deserializes_correctly() {
        let event = ChannelEvent::RoutePermission {
            session_key: SessionKey::new("telegram", "direct", "alice"),
            request_id: "req-1".into(),
            description: "Allow file write?".into(),
            options: serde_json::json!([{"optionId": "allow", "label": "Allow"}]),
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(deser, ChannelEvent::RoutePermission { .. }));
    }

    #[test]
    fn when_ack_message_event_serialized_then_deserializes_correctly() {
        let event = ChannelEvent::AckMessage {
            session_key: SessionKey::new("telegram", "direct", "alice"),
            channel_name: "telegram".into(),
            peer_id: "alice".into(),
            message_id: Some("msg-123".into()),
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(deser, ChannelEvent::AckMessage { .. }));
    }

    #[test]
    fn when_ack_message_has_no_message_id_then_serializes_as_null() {
        let event = ChannelEvent::AckMessage {
            session_key: SessionKey::new("debug-http", "local", "dev"),
            channel_name: "debug-http".into(),
            peer_id: "dev".into(),
            message_id: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(
            deser,
            ChannelEvent::AckMessage {
                message_id: None,
                ..
            }
        ));
    }
}
