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

    pub fn begin_finalizing(&mut self, handle: JoinHandle<()>) {
        if let TurnPhase::Finalizing(old) = &self.phase {
            old.abort();
        }
        self.phase = TurnPhase::Finalizing(handle);
    }

    pub fn take_response_for_finalize(&mut self) -> Option<(String, i32)> {
        self.response.as_ref().map(|r| (r.buffer.clone(), r.msg_id))
    }

    pub fn is_different_turn(&self, message_id: &str) -> bool {
        self.message_id != message_id
    }

    pub fn collapse_thought(&mut self) -> Option<(i32, f32)> {
        let track = self.thought.take()?;
        if let Some(h) = &track.debounce_handle {
            h.abort();
        }
        Some((track.msg_id, track.started_at.elapsed().as_secs_f32()))
    }

    pub fn cleanup(&mut self) {
        if let TurnPhase::Finalizing(handle) = &self.phase {
            handle.abort();
        }
        if let Some(ref track) = self.thought {
            if let Some(ref h) = track.debounce_handle {
                h.abort();
            }
        }
        self.thought = None;
        self.response = None;
        self.phase = TurnPhase::Active;
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

    #[rstest]
    #[tokio::test]
    async fn when_result_received_then_phase_becomes_finalizing() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello world", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        assert!(matches!(turn.phase, TurnPhase::Finalizing(_)));
    }

    #[rstest]
    fn when_finalized_then_response_buffer_returned() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello world", 100);
        let (response_text, response_msg_id) = turn.take_response_for_finalize().unwrap();
        assert_eq!(response_text, "hello world");
        assert_eq!(response_msg_id, 100);
    }

    #[rstest]
    #[tokio::test]
    async fn when_cleanup_called_then_handles_aborted_and_state_cleared() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_thought("thinking", 42);
        turn.append_response("text", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        turn.cleanup();
        assert!(turn.thought.is_none());
        assert!(turn.response.is_none());
        assert!(matches!(turn.phase, TurnPhase::Active));
    }

    #[rstest]
    fn given_different_message_id_when_checked_then_is_new_turn() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert!(turn.is_different_turn("msg-2"));
    }

    #[rstest]
    fn given_same_message_id_when_checked_then_not_new_turn() {
        let turn = ChatTurn::new("msg-1".to_string());
        assert!(!turn.is_different_turn("msg-1"));
    }

    #[rstest]
    fn when_stale_result_checked_then_detected() {
        let turn = ChatTurn::new("msg-2".to_string());
        assert!(turn.is_different_turn("msg-1"));
    }

    #[rstest]
    fn when_thought_collapsed_then_returns_elapsed_and_clears() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.thought = Some(ThoughtTrack {
            msg_id: 42,
            started_at: Instant::now(),
            buffer: "thinking...".to_string(),
            debounce_handle: None,
            suppressed: false,
        });
        let collapsed = turn.collapse_thought();
        assert!(collapsed.is_some());
        let (msg_id, elapsed_secs) = collapsed.unwrap();
        assert_eq!(msg_id, 42);
        assert!(elapsed_secs < 1.0);
        assert!(turn.thought.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn given_late_chunk_when_finalizing_then_buffer_grows() {
        let mut turn = ChatTurn::new("msg-1".to_string());
        turn.append_response("hello ", 100);
        let handle = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(10)).await });
        turn.begin_finalizing(handle);
        turn.append_response("world", 100);
        let (text, _) = turn.take_response_for_finalize().unwrap();
        assert_eq!(text, "hello world");
        assert!(matches!(turn.phase, TurnPhase::Finalizing(_)));
    }
}
