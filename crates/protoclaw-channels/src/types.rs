// Re-export SDK types for backward compatibility
pub use protoclaw_sdk_types::channel::{
    ChannelCapabilities, ChannelInitializeParams, ChannelInitializeResult,
    ChannelRespondPermission, ChannelSendMessage, DeliverMessage, PeerInfo,
};
pub use protoclaw_sdk_types::permission::{
    ChannelRequestPermission, PermissionOption, PermissionRequest, PermissionResponse,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_capabilities_serde_round_trip() {
        let caps = ChannelCapabilities {
            streaming: true,
            rich_text: false,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["streaming"], true);
        assert_eq!(json["richText"], false);
        let deser: ChannelCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(deser, caps);
    }

    #[test]
    fn deliver_message_serde_camel_case() {
        let msg = DeliverMessage {
            session_id: "sess-1".into(),
            content: serde_json::json!({"text": "hello"}),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["sessionId"], "sess-1");
        assert!(json.get("content").is_some());
        let deser: DeliverMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn request_permission_serde() {
        let req = ChannelRequestPermission {
            request_id: "req-1".into(),
            session_id: "sess-1".into(),
            description: "Allow file write?".into(),
            options: vec![
                PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                },
                PermissionOption {
                    option_id: "deny".into(),
                    label: "Deny".into(),
                },
            ],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["requestId"], "req-1");
        assert_eq!(json["description"], "Allow file write?");
        assert_eq!(json["options"].as_array().unwrap().len(), 2);
        let deser: ChannelRequestPermission = serde_json::from_value(json).unwrap();
        assert_eq!(deser, req);
    }

    #[test]
    fn send_message_serde() {
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
        assert_eq!(json["peerInfo"]["peerId"], "local:dev");
        assert_eq!(json["content"], "hello agent");
        let deser: ChannelSendMessage = serde_json::from_value(json).unwrap();
        assert_eq!(deser, msg);
    }

    #[test]
    fn respond_permission_serde() {
        let resp = ChannelRespondPermission {
            request_id: "req-1".into(),
            option_id: "allow".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["requestId"], "req-1");
        assert_eq!(json["optionId"], "allow");
        let deser: ChannelRespondPermission = serde_json::from_value(json).unwrap();
        assert_eq!(deser, resp);
    }
}
