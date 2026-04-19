use serde::{Deserialize, Serialize};

use crate::acp::StopReason;
use crate::session_key::SessionKey;

/// Events sent from AgentsManager to ChannelsManager via mpsc channel.
/// Defined in anyclaw-sdk-types as the shared wire type for agent→channel routing.
// DeliverMessage.content is raw JSON mutated by agents manager (pass-through)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
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
        /// Why the agent stopped generating output.
        stop_reason: StopReason,
    },
    /// Route a permission request to the originating channel.
    RoutePermission {
        /// Routing key identifying which channel + conversation to target.
        session_key: SessionKey,
        /// Unique identifier for correlating the permission response.
        request_id: String,
        /// Human-readable description of what is being requested.
        description: String,
        /// Permission options as typed `PermissionOption` structs.
        options: Vec<crate::permission::PermissionOption>,
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
    /// Signal that the agents manager is dispatching a prompt for this session.
    /// Channels uses this to send ack + typing indicator to the channel subprocess.
    DispatchStarted {
        /// Routing key identifying the session being dispatched.
        session_key: SessionKey,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
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

    #[rstest]
    #[test]
    fn when_route_permission_event_serialized_then_deserializes_correctly() {
        let event = ChannelEvent::RoutePermission {
            session_key: SessionKey::new("telegram", "direct", "alice"),
            request_id: "req-1".into(),
            description: "Allow file write?".into(),
            options: vec![crate::permission::PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };
        let json = serde_json::to_value(&event).unwrap();
        let deser: ChannelEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(deser, ChannelEvent::RoutePermission { .. }));
    }

    #[rstest]
    #[test]
    fn when_route_permission_options_round_trip_then_typed_vec() {
        let event = ChannelEvent::RoutePermission {
            session_key: SessionKey::new("telegram", "direct", "alice"),
            request_id: "req-2".into(),
            description: "Allow network?".into(),
            options: vec![
                crate::permission::PermissionOption {
                    option_id: "allow_once".into(),
                    label: "Allow once".into(),
                },
                crate::permission::PermissionOption {
                    option_id: "deny".into(),
                    label: "Deny".into(),
                },
            ],
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: ChannelEvent = serde_json::from_str(&json).unwrap();
        if let ChannelEvent::RoutePermission { options, .. } = back {
            assert_eq!(options.len(), 2);
            assert_eq!(options[0].option_id, "allow_once");
            assert_eq!(options[1].label, "Deny");
        } else {
            panic!("expected RoutePermission");
        }
    }

    #[rstest]
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

    #[rstest]
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

    // ── Round-trip serde tests (04-02 Task 1) ──────────────────────────

    #[rstest]
    #[case::deliver_message(ChannelEvent::DeliverMessage {
        session_key: SessionKey::new("debug-http", "local", "dev"),
        content: serde_json::json!({"text": "hello"}),
    })]
    #[case::session_complete(ChannelEvent::SessionComplete {
        session_key: SessionKey::new("telegram", "direct", "alice"),
        stop_reason: StopReason::EndTurn,
    })]
    #[case::ack_message_with_id(ChannelEvent::AckMessage {
        session_key: SessionKey::new("telegram", "direct", "bob"),
        channel_name: "telegram".into(),
        peer_id: "bob".into(),
        message_id: Some("msg-99".into()),
    })]
    #[case::ack_message_without_id(ChannelEvent::AckMessage {
        session_key: SessionKey::new("debug-http", "local", "dev"),
        channel_name: "debug-http".into(),
        peer_id: "dev".into(),
        message_id: None,
    })]
    #[case::dispatch_started(ChannelEvent::DispatchStarted {
        session_key: SessionKey::new("telegram", "direct", "alice"),
    })]
    fn when_channel_event_variant_round_trips_then_deserializes_to_same_variant(
        #[case] original: ChannelEvent,
    ) {
        let json = serde_json::to_string(&original).unwrap();
        let restored: ChannelEvent = serde_json::from_str(&json).unwrap();
        // ChannelEvent doesn't derive PartialEq, so compare serialized JSON
        let original_json = serde_json::to_value(&original).unwrap();
        let restored_json = serde_json::to_value(&restored).unwrap();
        assert_eq!(original_json, restored_json);
    }

    #[rstest]
    fn when_route_permission_event_round_trips_then_identical() {
        let original = ChannelEvent::RoutePermission {
            session_key: SessionKey::new("telegram", "direct", "alice"),
            request_id: "req-1".into(),
            description: "Allow file write?".into(),
            options: vec![
                crate::permission::PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                },
                crate::permission::PermissionOption {
                    option_id: "deny".into(),
                    label: "Deny".into(),
                },
            ],
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: ChannelEvent = serde_json::from_str(&json).unwrap();
        let original_json = serde_json::to_value(&original).unwrap();
        let restored_json = serde_json::to_value(&restored).unwrap();
        assert_eq!(original_json, restored_json);
    }
}
