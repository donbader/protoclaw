use std::collections::HashMap;

use protoclaw_sdk_channel::{ChannelAckConfig, PermissionBroker};
use protoclaw_sdk_types::ChannelSendMessage;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::turn::ChatTurn;

pub struct SharedState {
    pub outbound: Mutex<Option<mpsc::Sender<ChannelSendMessage>>>,
    pub permission_broker: Mutex<PermissionBroker>,
    pub permission_messages: Mutex<HashMap<String, (i64, i32)>>,
    pub session_chat_map: RwLock<HashMap<String, i64>>,
    pub last_message_ids: RwLock<HashMap<i64, i32>>,
    pub ack_config: RwLock<Option<ChannelAckConfig>>,
    pub turns: RwLock<HashMap<i64, ChatTurn>>,
    pub thought_emoji: RwLock<String>,
    pub response_edit_cooldown_ms: RwLock<u64>,
    pub thought_debounce_ms: RwLock<u64>,
    pub finalization_delay_ms: RwLock<u64>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            outbound: Mutex::new(None),
            permission_broker: Mutex::new(PermissionBroker::new()),
            permission_messages: Mutex::new(HashMap::new()),
            session_chat_map: RwLock::new(HashMap::new()),
            last_message_ids: RwLock::new(HashMap::new()),
            ack_config: RwLock::new(None),
            turns: RwLock::new(HashMap::new()),
            thought_emoji: RwLock::new("🧠".into()),
            response_edit_cooldown_ms: RwLock::new(1000),
            thought_debounce_ms: RwLock::new(400),
            finalization_delay_ms: RwLock::new(200),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn new_state_has_none_outbound() {
        let state = SharedState::new();
        assert!(state.outbound.lock().await.is_none());
    }

    #[tokio::test]
    async fn new_state_has_empty_maps() {
        let state = SharedState::new();
        assert!(state.permission_messages.lock().await.is_empty());
        assert!(state.session_chat_map.read().await.is_empty());
        assert!(state.last_message_ids.read().await.is_empty());
        assert!(state.ack_config.read().await.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn when_new_state_created_then_turns_empty() {
        let state = SharedState::new();
        assert!(state.turns.read().await.is_empty());
    }
}
