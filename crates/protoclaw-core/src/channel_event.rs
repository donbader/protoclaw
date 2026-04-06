use serde::{Deserialize, Serialize};

use crate::types::SessionKey;

/// Events sent from AgentsManager to ChannelsManager via mpsc channel.
/// Defined in protoclaw-core to avoid circular dependency between agents and channels crates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelEvent {
    /// Deliver an agent session/update message to the originating channel.
    DeliverMessage {
        session_key: SessionKey,
        content: serde_json::Value,
    },
    /// Signal that the agent has finished processing a prompt for this session.
    /// Triggers queue flush: marks the session idle and dispatches any queued messages.
    /// Sent once per completed `session/prompt`, after all streaming updates have been forwarded.
    SessionComplete { session_key: SessionKey },
    /// Route a permission request to the originating channel.
    RoutePermission {
        session_key: SessionKey,
        request_id: String,
        description: String,
        options: serde_json::Value,
    },
    AckMessage {
        session_key: SessionKey,
        channel_name: String,
        peer_id: String,
        message_id: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_event_deliver_message_round_trip() {
        let event = ChannelEvent::DeliverMessage {
            session_key: SessionKey::new("debug-http", "local", "dev"),
            content: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(deser, ChannelEvent::DeliverMessage { .. }));
    }

    #[test]
    fn channel_event_route_permission_round_trip() {
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
    fn channel_event_ack_message_round_trip() {
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
    fn channel_event_ack_message_none_message_id() {
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
