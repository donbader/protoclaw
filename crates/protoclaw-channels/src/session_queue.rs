use std::collections::{HashMap, HashSet, VecDeque};

use protoclaw_core::SessionKey;

/// Result of pushing a message into the session queue.
#[derive(Debug, PartialEq)]
pub enum QueueAction {
    /// Session is idle — dispatch this message immediately.
    Dispatch(String),
    /// Session is busy — message enqueued for later.
    Enqueued,
}

/// Per-session FIFO queue. Each session processes one message at a time.
///
/// When a session is idle, the first message dispatches immediately.
/// Subsequent messages enqueue until the session becomes idle again.
pub struct SessionQueue {
    queues: HashMap<SessionKey, VecDeque<String>>,
    active: HashSet<SessionKey>,
}

impl SessionQueue {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
            active: HashSet::new(),
        }
    }

    /// Push a message for a session.
    ///
    /// Returns `Dispatch` if the session is idle (caller should dispatch now),
    /// or `Enqueued` if the session is busy (message queued for later).
    pub fn push(&mut self, session_key: &SessionKey, message: String) -> QueueAction {
        if self.active.contains(session_key) {
            self.queues
                .entry(session_key.clone())
                .or_default()
                .push_back(message);
            QueueAction::Enqueued
        } else {
            self.active.insert(session_key.clone());
            QueueAction::Dispatch(message)
        }
    }

    /// Mark a session as idle after it finishes processing.
    ///
    /// Returns the next queued message if one exists (caller should dispatch it),
    /// or `None` if the queue is empty (session goes fully idle).
    pub fn mark_idle(&mut self, session_key: &SessionKey) -> Option<String> {
        if let Some(queue) = self.queues.get_mut(session_key) {
            if let Some(next) = queue.pop_front() {
                if queue.is_empty() {
                    self.queues.remove(session_key);
                }
                return Some(next);
            }
            self.queues.remove(session_key);
        }
        self.active.remove(session_key);
        None
    }

    /// Check if any session has pending queued messages.
    pub fn has_pending(&self) -> bool {
        !self.queues.is_empty()
    }

    /// Number of messages queued for a specific session (not counting the active one).
    pub fn queued_count(&self, session_key: &SessionKey) -> usize {
        self.queues.get(session_key).map_or(0, |q| q.len())
    }

    /// Whether a session is currently active (processing a message).
    pub fn is_active(&self, session_key: &SessionKey) -> bool {
        self.active.contains(session_key)
    }

    /// Drain all queued messages for a session without marking it idle.
    /// Returns the messages in FIFO order. The session remains active.
    pub fn drain_queued(&mut self, session_key: &SessionKey) -> Vec<String> {
        if let Some(queue) = self.queues.remove(session_key) {
            queue.into_iter().collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for SessionQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn key(name: &str) -> SessionKey {
        SessionKey::new("test", "local", name)
    }

    #[test]
    fn when_new_session_queue_created_then_no_pending_messages() {
        let q = SessionQueue::new();
        assert!(!q.has_pending());
    }

    #[test]
    fn when_first_message_enqueued_then_dispatches_immediately() {
        let mut q = SessionQueue::new();
        let action = q.push(&key("alice"), "hello".into());
        assert_eq!(action, QueueAction::Dispatch("hello".into()));
        assert!(q.is_active(&key("alice")));
    }

    #[test]
    fn when_second_message_enqueued_while_session_active_then_queued() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        let action = q.push(&key("alice"), "msg2".into());
        assert_eq!(action, QueueAction::Enqueued);
        assert_eq!(q.queued_count(&key("alice")), 1);
    }

    #[test]
    fn when_mark_idle_called_with_queued_messages_then_returns_next() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        q.push(&key("alice"), "msg2".into());
        q.push(&key("alice"), "msg3".into());

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, Some("msg2".into()));
        assert!(q.is_active(&key("alice")));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, Some("msg3".into()));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[test]
    fn when_mark_idle_called_with_empty_queue_then_session_goes_idle() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[test]
    fn when_two_sessions_used_then_queues_are_independent() {
        let mut q = SessionQueue::new();
        let a1 = q.push(&key("alice"), "a1".into());
        let b1 = q.push(&key("bob"), "b1".into());
        assert_eq!(a1, QueueAction::Dispatch("a1".into()));
        assert_eq!(b1, QueueAction::Dispatch("b1".into()));

        q.push(&key("alice"), "a2".into());
        assert_eq!(q.queued_count(&key("alice")), 1);
        assert_eq!(q.queued_count(&key("bob")), 0);
    }

    #[test]
    fn when_multiple_messages_queued_then_dequeued_in_fifo_order() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "first".into());
        q.push(&key("alice"), "second".into());
        q.push(&key("alice"), "third".into());

        assert_eq!(q.mark_idle(&key("alice")), Some("second".into()));
        assert_eq!(q.mark_idle(&key("alice")), Some("third".into()));
        assert_eq!(q.mark_idle(&key("alice")), None);
    }

    #[test]
    fn given_idle_session_when_new_message_enqueued_then_dispatches_immediately() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        q.mark_idle(&key("alice"));

        let action = q.push(&key("alice"), "msg2".into());
        assert_eq!(action, QueueAction::Dispatch("msg2".into()));
    }

    #[test]
    fn when_messages_queued_then_has_pending_returns_true() {
        let mut q = SessionQueue::new();
        assert!(!q.has_pending());

        q.push(&key("alice"), "msg1".into());
        assert!(!q.has_pending());

        q.push(&key("alice"), "msg2".into());
        assert!(q.has_pending());

        q.mark_idle(&key("alice"));
        assert!(!q.has_pending());
    }

    #[test]
    fn when_mark_idle_called_for_unknown_session_then_is_noop() {
        let mut q = SessionQueue::new();
        assert_eq!(q.mark_idle(&key("unknown")), None);
    }

    #[test]
    fn when_drain_queued_called_then_returns_all_queued_messages() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        q.push(&key("alice"), "msg2".into());
        q.push(&key("alice"), "msg3".into());

        let drained = q.drain_queued(&key("alice"));
        assert_eq!(drained, vec!["msg2".to_string(), "msg3".to_string()]);
        assert_eq!(q.queued_count(&key("alice")), 0);
        assert!(q.is_active(&key("alice")));
    }

    #[test]
    fn when_drain_queued_called_on_empty_queue_then_returns_empty_vec() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        let drained = q.drain_queued(&key("alice"));
        assert!(drained.is_empty());
    }

    #[test]
    fn when_drain_queued_called_for_unknown_session_then_returns_empty_vec() {
        let mut q = SessionQueue::new();
        let drained = q.drain_queued(&key("unknown"));
        assert!(drained.is_empty());
    }

    #[rstest]
    fn when_messages_queued_then_queued_count_returns_correct_number() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), "msg1".into());
        q.push(&key("alice"), "msg2".into());
        q.push(&key("alice"), "msg3".into());
        assert_eq!(q.queued_count(&key("alice")), 2);
    }

    #[rstest]
    fn when_session_active_then_is_active_returns_true() {
        let mut q = SessionQueue::new();
        q.push(&key("bob"), "hello".into());
        assert!(q.is_active(&key("bob")));
    }

    #[rstest]
    fn when_session_idle_then_is_active_returns_false() {
        let q = SessionQueue::new();
        assert!(!q.is_active(&key("nobody")));
    }
}
