use serde::{Deserialize, Serialize};

/// Channel capabilities advertised during initialize handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCapabilities {
    pub streaming: bool,
    pub rich_text: bool,
}

/// Initialize handshake — protoclaw sends to channel subprocess.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInitializeParams {
    pub protocol_version: u32,
    pub channel_id: String,
    #[serde(default)]
    pub ack: Option<ChannelAckConfig>,
}

/// Initialize handshake — channel subprocess responds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInitializeResult {
    pub protocol_version: u32,
    pub capabilities: ChannelCapabilities,
}

/// Protoclaw → Channel: deliver agent message/streaming update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeliverMessage {
    pub session_id: String,
    pub content: serde_json::Value,
}

/// Peer identity information for inbound messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub channel_name: String,
    pub peer_id: String,
    pub kind: String,
}

/// Channel → Protoclaw: user sent a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSendMessage {
    pub peer_info: PeerInfo,
    pub content: String,
}

/// Helper for channel implementations to extract thought content from DeliverMessage.
/// When DeliverMessage.content has type "agent_thought_chunk", channels can use this
/// to deserialize the thought payload.
///
/// # Example
/// ```
/// use protoclaw_sdk_types::channel::ThoughtContent;
/// let content = serde_json::json!({
///     "sessionId": "s1",
///     "type": "agent_thought_chunk",
///     "content": "thinking..."
/// });
/// if let Some(thought) = ThoughtContent::from_content(&content) {
///     assert_eq!(thought.content, "thinking...");
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThoughtContent {
    pub session_id: String,
    #[serde(rename = "type")]
    pub update_type: String,
    pub content: String,
}

impl ThoughtContent {
    /// Try to extract thought content from a DeliverMessage content value.
    /// Returns Some if the content type is "agent_thought_chunk", None otherwise.
    pub fn from_content(content: &serde_json::Value) -> Option<Self> {
        let update_type = content.get("type")?.as_str()?;
        if update_type == "agent_thought_chunk" {
            serde_json::from_value(content.clone()).ok()
        } else {
            None
        }
    }
}

/// Protoclaw → Channel: acknowledge message receipt.
/// Channel uses this to add emoji reaction and/or show typing indicator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AckNotification {
    pub session_id: String,
    pub channel_name: String,
    pub peer_id: String,
    pub message_id: Option<String>,
}

/// Protoclaw → Channel: ack lifecycle event (e.g., response started).
/// Channel uses this to remove/replace reaction based on its ack config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AckLifecycleNotification {
    pub session_id: String,
    pub action: String,
}

/// Ack configuration passed to channels via initialize handshake.
/// Lightweight mirror of config crate's AckConfig — SDK types must not depend on config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAckConfig {
    pub reaction: bool,
    pub typing: bool,
    pub reaction_emoji: String,
    pub reaction_lifecycle: String,
}

/// Channel → Protoclaw: user responded to permission prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRespondPermission {
    pub request_id: String,
    pub option_id: String,
}

/// Protoclaw → Channel: notify channel that a session was created for a peer.
/// Channels can use this to map ACP session IDs back to their internal identifiers
/// (e.g., Telegram chat IDs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreated {
    pub session_id: String,
    pub peer_info: PeerInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_capabilities_round_trip() {
        let caps = ChannelCapabilities {
            streaming: true,
            rich_text: false,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["streaming"], true);
        assert_eq!(json["richText"], false);
        assert!(json.get("rich_text").is_none());
        let deser: ChannelCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(deser, caps);
    }

    #[test]
    fn peer_info_round_trip() {
        let info = PeerInfo {
            channel_name: "debug-http".into(),
            peer_id: "local:dev".into(),
            kind: "local".into(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["channelName"], "debug-http");
        assert_eq!(json["peerId"], "local:dev");
        assert!(json.get("channel_name").is_none());
        let deser: PeerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(deser, info);
    }

    #[test]
    fn deliver_message_round_trip() {
        let msg = DeliverMessage {
            session_id: "sess-1".into(),
            content: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert!(json.get("session_id").is_none());
        let deser: DeliverMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn channel_send_message_round_trip() {
        let msg = ChannelSendMessage {
            peer_info: PeerInfo {
                channel_name: "debug-http".into(),
                peer_id: "local:dev".into(),
                kind: "local".into(),
            },
            content: "hello agent".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["peerInfo"]["channelName"], "debug-http");
        assert_eq!(json["content"], "hello agent");
        let deser: ChannelSendMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn channel_respond_permission_round_trip() {
        let resp = ChannelRespondPermission {
            request_id: "req-1".into(),
            option_id: "allow".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["requestId"], "req-1");
        assert_eq!(json["optionId"], "allow");
        assert!(json.get("request_id").is_none());
        let deser: ChannelRespondPermission = serde_json::from_value(json).unwrap();
        assert_eq!(deser, resp);
    }

    #[test]
    fn channel_initialize_params_round_trip() {
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "ch-1".into(),
            ack: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["channelId"], "ch-1");
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser, params);
    }

    #[test]
    fn thought_content_from_valid_thought() {
        let content = serde_json::json!({
            "sessionId": "s1",
            "type": "agent_thought_chunk",
            "content": "Analyzing..."
        });
        let thought = ThoughtContent::from_content(&content).unwrap();
        assert_eq!(thought.session_id, "s1");
        assert_eq!(thought.update_type, "agent_thought_chunk");
        assert_eq!(thought.content, "Analyzing...");
    }

    #[test]
    fn thought_content_from_non_thought_returns_none() {
        let content = serde_json::json!({
            "sessionId": "s1",
            "type": "agent_message_chunk",
            "content": "Hello"
        });
        assert!(ThoughtContent::from_content(&content).is_none());
    }

    #[test]
    fn thought_content_round_trip() {
        let thought = ThoughtContent {
            session_id: "s1".into(),
            update_type: "agent_thought_chunk".into(),
            content: "Thinking...".into(),
        };
        let json = serde_json::to_value(&thought).unwrap();
        assert_eq!(json["sessionId"], "s1");
        assert_eq!(json["type"], "agent_thought_chunk");
        let deser: ThoughtContent = serde_json::from_value(json).unwrap();
        assert_eq!(deser, thought);
    }

    #[test]
    fn thought_content_from_deliver_message_content() {
        let msg = DeliverMessage {
            session_id: "sess-1".into(),
            content: serde_json::json!({
                "sessionId": "sess-1",
                "type": "agent_thought_chunk",
                "content": "deep thought"
            }),
        };
        let thought = ThoughtContent::from_content(&msg.content).unwrap();
        assert_eq!(thought.content, "deep thought");
    }

    #[test]
    fn channel_initialize_result_round_trip() {
        let result = ChannelInitializeResult {
            protocol_version: 1,
            capabilities: ChannelCapabilities {
                streaming: true,
                rich_text: false,
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["capabilities"]["streaming"], true);
        let deser: ChannelInitializeResult = serde_json::from_value(json).unwrap();
        assert_eq!(deser, result);
    }

    #[test]
    fn ack_notification_round_trip() {
        let ack = AckNotification {
            session_id: "sess-1".into(),
            channel_name: "telegram".into(),
            peer_id: "telegram:12345".into(),
            message_id: Some("msg-42".into()),
        };
        let json = serde_json::to_value(&ack).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["channelName"], "telegram");
        assert_eq!(json["peerId"], "telegram:12345");
        assert_eq!(json["messageId"], "msg-42");
        assert!(json.get("session_id").is_none());
        let deser: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser, ack);
    }

    #[test]
    fn ack_notification_none_message_id() {
        let ack = AckNotification {
            session_id: "sess-1".into(),
            channel_name: "debug-http".into(),
            peer_id: "local".into(),
            message_id: None,
        };
        let json = serde_json::to_value(&ack).unwrap();
        assert!(json["messageId"].is_null());
        let deser: AckNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser.message_id, None);
    }

    #[test]
    fn ack_lifecycle_notification_round_trip() {
        let lifecycle = AckLifecycleNotification {
            session_id: "sess-1".into(),
            action: "response_started".into(),
        };
        let json = serde_json::to_value(&lifecycle).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["action"], "response_started");
        let deser: AckLifecycleNotification = serde_json::from_value(json).unwrap();
        assert_eq!(deser, lifecycle);
    }

    #[test]
    fn channel_ack_config_round_trip() {
        let cfg = ChannelAckConfig {
            reaction: true,
            typing: true,
            reaction_emoji: "👀".into(),
            reaction_lifecycle: "remove".into(),
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["reaction"], true);
        assert_eq!(json["typing"], true);
        assert_eq!(json["reactionEmoji"], "👀");
        assert_eq!(json["reactionLifecycle"], "remove");
        assert!(json.get("reaction_emoji").is_none());
        let deser: ChannelAckConfig = serde_json::from_value(json).unwrap();
        assert_eq!(deser, cfg);
    }

    #[test]
    fn channel_initialize_params_with_ack() {
        let params = ChannelInitializeParams {
            protocol_version: 1,
            channel_id: "telegram".into(),
            ack: Some(ChannelAckConfig {
                reaction: true,
                typing: true,
                reaction_emoji: "👀".into(),
                reaction_lifecycle: "remove".into(),
            }),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["ack"]["reaction"], true);
        assert_eq!(json["ack"]["reactionEmoji"], "👀");
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser, params);
    }

    #[test]
    fn channel_initialize_params_without_ack() {
        let json = serde_json::json!({
            "protocolVersion": 1,
            "channelId": "debug-http"
        });
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser.ack, None);
    }
}
