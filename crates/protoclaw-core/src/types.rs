use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManagerId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

/// Session key encoding channel + conversation identity.
/// Format: "{channel_name}:{kind}:{peer_id}"
/// Examples: "debug-http:local:dev", "telegram:direct:alice"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey(String);

impl SessionKey {
    pub fn new(channel_name: &str, kind: &str, peer_id: &str) -> Self {
        Self(format!("{channel_name}:{kind}:{peer_id}"))
    }

    pub fn channel_name(&self) -> &str {
        self.0.split(':').next().unwrap_or("")
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SessionKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.matches(':').count() < 2 {
            return Err(format!(
                "invalid session key: expected 'channel:kind:peer', got '{s}'"
            ));
        }
        Ok(Self(s.to_owned()))
    }
}

impl From<String> for SessionKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl AsRef<str> for SessionKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ManagerId {
    pub const TOOLS: &str = "tools";
    pub const AGENTS: &str = "agents";
    pub const CHANNELS: &str = "channels";
}

impl MessageId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! impl_id_type {
    ($t:ident) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $t {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $t {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl AsRef<str> for $t {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

impl_id_type!(SessionId);
impl_id_type!(ChannelId);
impl_id_type!(ManagerId);
impl_id_type!(MessageId);

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_session_id_round_tripped_through_display_and_from_str_then_equal() {
        let id = SessionId::from("sess-123");
        assert_eq!(id.to_string(), "sess-123");
        assert_eq!(id.as_ref(), "sess-123");
    }

    #[test]
    fn when_channel_id_round_tripped_through_display_and_from_string_then_equal() {
        let id = ChannelId::from("telegram".to_string());
        assert_eq!(id.to_string(), "telegram");
        assert_eq!(id.as_ref(), "telegram");
    }

    #[test]
    fn when_manager_id_constants_checked_then_values_are_correct() {
        assert_eq!(ManagerId::TOOLS, "tools");
        assert_eq!(ManagerId::AGENTS, "agents");
        assert_eq!(ManagerId::CHANNELS, "channels");
    }

    #[test]
    fn when_manager_id_displayed_then_produces_expected_string() {
        let id = ManagerId::from("tools");
        assert_eq!(id.to_string(), "tools");
    }

    #[test]
    fn when_two_message_ids_created_then_they_are_unique() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
        assert!(!id1.as_ref().is_empty());
    }

    #[test]
    fn when_default_message_id_created_then_is_valid_uuid() {
        let id = MessageId::default();
        assert_eq!(id.as_ref().len(), 36); // UUID v4 string length
    }

    #[test]
    fn when_session_key_created_then_formatted_as_channel_slash_peer() {
        let key = SessionKey::new("debug-http", "local", "dev");
        assert_eq!(key.to_string(), "debug-http:local:dev");
    }

    #[test]
    fn when_channel_name_extracted_from_session_key_then_returns_first_segment() {
        let key = SessionKey::new("telegram", "direct", "alice");
        assert_eq!(key.channel_name(), "telegram");
    }

    #[test]
    fn when_session_key_round_tripped_through_display_and_from_str_then_equal() {
        let key = SessionKey::new("slack", "group", "general");
        let s = key.to_string();
        let parsed: SessionKey = s.parse().unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn when_two_equal_session_keys_compared_then_hash_and_eq_consistent() {
        let a = SessionKey::new("debug-http", "local", "dev");
        let b = SessionKey::new("debug-http", "local", "dev");
        assert_eq!(a, b);

        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
