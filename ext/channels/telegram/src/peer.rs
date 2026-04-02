use protoclaw_sdk_types::PeerInfo;

/// Convert a Telegram chat_id and chat type string into a PeerInfo.
///
/// chat_type values: "private", "group", "supergroup", "channel"
pub fn peer_info_from_chat(chat_id: i64, chat_type: &str) -> PeerInfo {
    let kind = match chat_type {
        "private" => "direct",
        "group" | "supergroup" => "group",
        "channel" => "channel",
        _ => "unknown",
    };
    PeerInfo {
        channel_name: "telegram".into(),
        peer_id: format!("telegram:{chat_id}"),
        kind: kind.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_chat_returns_direct_kind() {
        let info = peer_info_from_chat(12345, "private");
        assert_eq!(info.kind, "direct");
        assert_eq!(info.peer_id, "telegram:12345");
    }

    #[test]
    fn group_chat_returns_group_kind() {
        let info = peer_info_from_chat(-100123, "group");
        assert_eq!(info.kind, "group");
        assert_eq!(info.peer_id, "telegram:-100123");
    }

    #[test]
    fn supergroup_returns_group_kind() {
        let info = peer_info_from_chat(-100456, "supergroup");
        assert_eq!(info.kind, "group");
    }

    #[test]
    fn channel_chat_returns_channel_kind() {
        let info = peer_info_from_chat(-100789, "channel");
        assert_eq!(info.kind, "channel");
    }

    #[test]
    fn always_sets_channel_name_telegram() {
        let info = peer_info_from_chat(1, "private");
        assert_eq!(info.channel_name, "telegram");

        let info2 = peer_info_from_chat(-1, "group");
        assert_eq!(info2.channel_name, "telegram");
    }

    #[test]
    fn unknown_chat_type_returns_unknown_kind() {
        let info = peer_info_from_chat(1, "something_else");
        assert_eq!(info.kind, "unknown");
    }
}
