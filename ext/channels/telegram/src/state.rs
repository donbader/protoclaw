use std::collections::{HashMap, HashSet};

use protoclaw_sdk_channel::ChannelAckConfig;
use protoclaw_sdk_types::{ChannelSendMessage, PermissionResponse};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::Instant;

pub struct SharedState {
    pub outbound: Mutex<Option<mpsc::Sender<ChannelSendMessage>>>,
    pub active_messages: RwLock<HashMap<i64, i32>>,
    pub message_buffers: RwLock<HashMap<i64, String>>,
    pub permission_resolvers: Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>,
    pub permission_messages: Mutex<HashMap<String, (i64, i32)>>,
    pub session_chat_map: RwLock<HashMap<String, i64>>,
    pub last_edit_time: RwLock<HashMap<i64, Instant>>,
    pub thinking_messages: RwLock<HashMap<i64, (i32, Instant)>>,
    pub thought_buffers: RwLock<HashMap<i64, String>>,
    pub thought_debounce_handles: RwLock<HashMap<i64, tokio::task::JoinHandle<()>>>,
    pub thought_suppressed: RwLock<HashSet<i64>>,
    pub result_received: RwLock<HashSet<i64>>,
    pub finalize_handles: RwLock<HashMap<i64, tokio::task::JoinHandle<()>>>,
    pub current_message_id: RwLock<HashMap<i64, String>>,
    pub last_message_ids: RwLock<HashMap<i64, i32>>,
    pub ack_config: RwLock<Option<ChannelAckConfig>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            outbound: Mutex::new(None),
            active_messages: RwLock::new(HashMap::new()),
            message_buffers: RwLock::new(HashMap::new()),
            permission_resolvers: Mutex::new(HashMap::new()),
            permission_messages: Mutex::new(HashMap::new()),
            session_chat_map: RwLock::new(HashMap::new()),
            last_edit_time: RwLock::new(HashMap::new()),
            thinking_messages: RwLock::new(HashMap::new()),
            thought_buffers: RwLock::new(HashMap::new()),
            thought_debounce_handles: RwLock::new(HashMap::new()),
            thought_suppressed: RwLock::new(HashSet::new()),
            result_received: RwLock::new(HashSet::new()),
            finalize_handles: RwLock::new(HashMap::new()),
            current_message_id: RwLock::new(HashMap::new()),
            last_message_ids: RwLock::new(HashMap::new()),
            ack_config: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_has_none_outbound() {
        let state = SharedState::new();
        assert!(state.outbound.lock().await.is_none());
    }

    #[tokio::test]
    async fn new_state_has_empty_maps() {
        let state = SharedState::new();
        assert!(state.active_messages.read().await.is_empty());
        assert!(state.permission_resolvers.lock().await.is_empty());
        assert!(state.permission_messages.lock().await.is_empty());
        assert!(state.session_chat_map.read().await.is_empty());
        assert!(state.last_message_ids.read().await.is_empty());
        assert!(state.ack_config.read().await.is_none());
    }

    #[tokio::test]
    async fn thought_debounce_handles_initialized_empty() {
        let state = SharedState::new();
        assert!(state.thought_debounce_handles.read().await.is_empty());
    }

    #[tokio::test]
    async fn finalize_handles_initialized_empty() {
        let state = SharedState::new();
        assert!(state.finalize_handles.read().await.is_empty());
    }
}
