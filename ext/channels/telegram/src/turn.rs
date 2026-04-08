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

    pub fn append_response(&mut self, text: &str, msg_id: i32) {
        match &mut self.response {
            Some(track) => {
                track.buffer.push_str(text);
            }
            None => {
                self.response = Some(ResponseTrack {
                    msg_id,
                    buffer: text.to_string(),
                    last_edit: Instant::now(),
                });
            }
        }
    }

    pub fn can_edit_response(&mut self) -> bool {
        match &mut self.response {
            Some(track) => {
                if track.last_edit.elapsed() < Duration::from_secs(1) {
                    return false;
                }
                track.last_edit = Instant::now();
                true
            }
            None => false,
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

    #[rstest]
    fn when_response_appended_then_buffer_accumulates() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello ", 100);
        turn.append_response("world", 100);
        let track = turn.response.as_ref().unwrap();
        assert_eq!(track.buffer, "hello world");
        assert_eq!(track.msg_id, 100);
    }

    #[rstest]
    fn when_can_edit_checked_within_cooldown_then_false() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("text", 100);
        // last_edit is Instant::now(), so within 1s cooldown
        assert!(!turn.can_edit_response());
    }

    #[rstest]
    #[tokio::test]
    async fn given_response_after_cooldown_when_can_edit_then_true() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.response = Some(ResponseTrack {
            msg_id: 100,
            buffer: "text".to_string(),
            last_edit: Instant::now() - Duration::from_secs(2),
        });
        assert!(turn.can_edit_response());
    }
}
