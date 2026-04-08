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

impl ChatTurn {
    pub fn new(message_id: String) -> Self {
        Self {
            message_id,
            phase: TurnPhase::Active,
            thought: None,
            response: None,
        }
    }

    pub fn append_thought(&mut self, text: &str, msg_id: i32) {
        match &mut self.thought {
            Some(track) => {
                track.buffer.push_str(text);
            }
            None => {
                self.thought = Some(ThoughtTrack {
                    msg_id,
                    started_at: Instant::now(),
                    buffer: text.to_string(),
                    debounce_handle: None,
                    suppressed: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn given_no_turn_when_thought_arrives_then_turn_created() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert_eq!(turn.message_id, "msg-1");
        assert!(matches!(turn.phase, TurnPhase::Active));
        assert!(turn.thought.is_none());
        assert!(turn.response.is_none());
    }

    #[rstest]
    fn when_thought_appended_then_buffer_accumulates() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_thought("hello ", 42);
        turn.append_thought("world", 42);
        let track = turn.thought.as_ref().unwrap();
        assert_eq!(track.buffer, "hello world");
        assert_eq!(track.msg_id, 42);
    }
}
