use std::collections::HashMap;

use protoclaw_sdk_types::{ChannelSendMessage, PermissionResponse};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::Instant;

pub struct SharedState {
    pub outbound: Mutex<Option<mpsc::Sender<ChannelSendMessage>>>,
    pub active_messages: RwLock<HashMap<i64, i32>>,
    pub permission_resolvers: Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>,
    pub permission_messages: Mutex<HashMap<String, (i64, i32)>>,
    pub session_chat_map: RwLock<HashMap<String, i64>>,
    pub last_edit_time: RwLock<HashMap<i64, Instant>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            outbound: Mutex::new(None),
            active_messages: RwLock::new(HashMap::new()),
            permission_resolvers: Mutex::new(HashMap::new()),
            permission_messages: Mutex::new(HashMap::new()),
            session_chat_map: RwLock::new(HashMap::new()),
            last_edit_time: RwLock::new(HashMap::new()),
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
    }
}
