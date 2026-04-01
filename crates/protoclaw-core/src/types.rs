use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManagerId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

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

    #[test]
    fn session_id_display_and_from_str() {
        let id = SessionId::from("sess-123");
        assert_eq!(id.to_string(), "sess-123");
        assert_eq!(id.as_ref(), "sess-123");
    }

    #[test]
    fn channel_id_display_and_from_string() {
        let id = ChannelId::from("telegram".to_string());
        assert_eq!(id.to_string(), "telegram");
        assert_eq!(id.as_ref(), "telegram");
    }

    #[test]
    fn manager_id_constants() {
        assert_eq!(ManagerId::TOOLS, "tools");
        assert_eq!(ManagerId::AGENTS, "agents");
        assert_eq!(ManagerId::CHANNELS, "channels");
    }

    #[test]
    fn manager_id_display() {
        let id = ManagerId::from("tools");
        assert_eq!(id.to_string(), "tools");
    }

    #[test]
    fn message_id_new_generates_unique_ids() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
        assert!(!id1.as_ref().is_empty());
    }

    #[test]
    fn message_id_default_generates_uuid() {
        let id = MessageId::default();
        assert_eq!(id.as_ref().len(), 36); // UUID v4 string length
    }
}
