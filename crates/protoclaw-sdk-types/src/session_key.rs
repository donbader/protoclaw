use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Session key encoding channel + conversation identity.
/// Format: "{channel_name}:{kind}:{peer_id}"
/// Examples: "debug-http:local:dev", "telegram:direct:alice"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey(String);

impl SessionKey {
    /// Create a new session key from channel name, conversation kind, and peer identifier.
    pub fn new(channel_name: &str, kind: &str, peer_id: &str) -> Self {
        Self(format!("{channel_name}:{kind}:{peer_id}"))
    }

    /// Extract the channel name (first colon-delimited segment) from the key.
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

#[cfg(test)]
mod tests {
    use super::*;

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
