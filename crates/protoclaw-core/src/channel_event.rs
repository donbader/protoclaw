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
    /// Route a permission request to the originating channel.
    RoutePermission {
        session_key: SessionKey,
        request_id: String,
        description: String,
        options: serde_json::Value,
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
}
