use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub struct ThoughtTrack {
    pub msg_id: i32,
    pub started_at: Instant,
    pub buffer: String,
    pub debounce_handle: Option<JoinHandle<()>>,
    pub suppressed: bool,
}

pub struct ResponseTrack {
    pub msg_id: i32,
    pub buffer: String,
    pub last_edit: Instant,
}

pub enum TurnPhase {
    Active,
    Finalizing(JoinHandle<()>),
}

pub struct ChatTurn {
    pub message_id: String,
    pub phase: TurnPhase,
    pub thought: Option<ThoughtTrack>,
    pub response: Option<ResponseTrack>,
}
