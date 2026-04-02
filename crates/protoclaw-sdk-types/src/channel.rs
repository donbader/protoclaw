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

/// Channel → Protoclaw: user responded to permission prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRespondPermission {
    pub request_id: String,
    pub option_id: String,
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
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["channelId"], "ch-1");
        let deser: ChannelInitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(deser, params);
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
}
