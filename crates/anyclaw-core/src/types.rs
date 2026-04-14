use serde::{Deserialize, Serialize};
use std::fmt;

/// Opaque identifier for an ACP session, wrapping a UUID string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

/// Opaque identifier for a channel instance (e.g. "telegram", "debug-http").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(String);

/// Opaque identifier for a manager (one of "tools", "agents", "channels").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManagerId(String);

/// Opaque identifier for a message, generated as UUID v4.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl ManagerId {
    /// Well-known manager name for the tools manager.
    pub const TOOLS: &str = "tools";
    /// Well-known manager name for the agents manager.
    pub const AGENTS: &str = "agents";
    /// Well-known manager name for the channels manager.
    pub const CHANNELS: &str = "channels";
}

impl MessageId {
    /// Generate a new random message ID (UUID v4).
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
}
