use std::collections::{HashMap, HashSet, VecDeque};

use anyclaw_core::SessionKey;
use anyclaw_sdk_types::{ContentPart, MessageMetadata};

pub type RichPayload = (Vec<ContentPart>, Option<MessageMetadata>);

#[derive(Debug, PartialEq)]
pub enum QueueAction {
    Dispatch(RichPayload),
    Enqueued,
}

// LIMITATION: No rate limiting on inbound messages
// No rate limiting exists anywhere in the codebase. A misbehaving channel could flood
// the agent with messages, causing the session queue to grow unbounded and overwhelming
// the agent subprocess. Fix approach: add per-session or per-channel rate limiting in
// ChannelsManager before dispatching to agents.
// See also: CONCERNS.md §Architecture Concerns

pub struct SessionQueue {
    queues: HashMap<SessionKey, VecDeque<RichPayload>>,
    active: HashSet<SessionKey>,
}

impl SessionQueue {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
            active: HashSet::new(),
        }
    }

    pub fn push(
        &mut self,
        session_key: &SessionKey,
        content: Vec<ContentPart>,
        metadata: Option<MessageMetadata>,
    ) -> QueueAction {
        if self.active.contains(session_key) {
            self.queues
                .entry(session_key.clone())
                .or_default()
                .push_back((content, metadata));
            QueueAction::Enqueued
        } else {
            self.active.insert(session_key.clone());
            QueueAction::Dispatch((content, metadata))
        }
    }

    pub fn mark_idle(&mut self, session_key: &SessionKey) -> Option<RichPayload> {
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

    #[cfg(test)]
    pub fn has_pending(&self) -> bool {
        !self.queues.is_empty()
    }

    #[cfg(test)]
    pub fn queued_count(&self, session_key: &SessionKey) -> usize {
        self.queues.get(session_key).map_or(0, VecDeque::len)
    }

    pub fn is_active(&self, session_key: &SessionKey) -> bool {
        self.active.contains(session_key)
    }

    pub fn push_only(
        &mut self,
        session_key: &SessionKey,
        content: Vec<ContentPart>,
        metadata: Option<MessageMetadata>,
    ) -> bool {
        let was_idle = !self.active.contains(session_key);
        self.queues
            .entry(session_key.clone())
            .or_default()
            .push_back((content, metadata));
        was_idle
    }

    pub fn flush_pending(&mut self, session_key: &SessionKey) -> Option<RichPayload> {
        if self.active.contains(session_key) {
            return None;
        }
        if let Some(queue) = self.queues.remove(session_key) {
            if queue.is_empty() {
                return None;
            }
            self.active.insert(session_key.clone());
            let mut merged_content = Vec::new();
            let mut last_metadata = None;
            for (content, metadata) in queue {
                merged_content.extend(content);
                last_metadata = metadata;
            }
            Some((merged_content, last_metadata))
        } else {
            None
        }
    }

    pub fn drain_queued(&mut self, session_key: &SessionKey) -> Vec<RichPayload> {
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

    fn p(text: &str) -> RichPayload {
        (vec![ContentPart::text(text)], None)
    }

    #[test]
    fn when_new_session_queue_created_then_no_pending_messages() {
        let q = SessionQueue::new();
        assert!(!q.has_pending());
    }

    #[test]
    fn when_first_message_enqueued_then_dispatches_immediately() {
        let mut q = SessionQueue::new();
        let action = q.push(&key("alice"), vec![ContentPart::text("hello")], None);
        assert_eq!(action, QueueAction::Dispatch(p("hello")));
        assert!(q.is_active(&key("alice")));
    }

    #[test]
    fn when_second_message_enqueued_while_session_active_then_queued() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        let action = q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        assert_eq!(action, QueueAction::Enqueued);
        assert_eq!(q.queued_count(&key("alice")), 1);
    }

    #[test]
    fn when_mark_idle_called_with_queued_messages_then_returns_next() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg3")], None);

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, Some(p("msg2")));
        assert!(q.is_active(&key("alice")));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, Some(p("msg3")));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[test]
    fn when_mark_idle_called_with_empty_queue_then_session_goes_idle() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[test]
    fn when_two_sessions_used_then_queues_are_independent() {
        let mut q = SessionQueue::new();
        let a1 = q.push(&key("alice"), vec![ContentPart::text("a1")], None);
        let b1 = q.push(&key("bob"), vec![ContentPart::text("b1")], None);
        assert_eq!(a1, QueueAction::Dispatch(p("a1")));
        assert_eq!(b1, QueueAction::Dispatch(p("b1")));

        q.push(&key("alice"), vec![ContentPart::text("a2")], None);
        assert_eq!(q.queued_count(&key("alice")), 1);
        assert_eq!(q.queued_count(&key("bob")), 0);
    }

    #[test]
    fn when_multiple_messages_queued_then_dequeued_in_fifo_order() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("first")], None);
        q.push(&key("alice"), vec![ContentPart::text("second")], None);
        q.push(&key("alice"), vec![ContentPart::text("third")], None);

        assert_eq!(q.mark_idle(&key("alice")), Some(p("second")));
        assert_eq!(q.mark_idle(&key("alice")), Some(p("third")));
        assert_eq!(q.mark_idle(&key("alice")), None);
    }

    #[test]
    fn given_idle_session_when_new_message_enqueued_then_dispatches_immediately() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.mark_idle(&key("alice"));

        let action = q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        assert_eq!(action, QueueAction::Dispatch(p("msg2")));
    }

    #[test]
    fn when_messages_queued_then_has_pending_returns_true() {
        let mut q = SessionQueue::new();
        assert!(!q.has_pending());

        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        assert!(!q.has_pending());

        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
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
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg3")], None);

        let drained = q.drain_queued(&key("alice"));
        assert_eq!(drained, vec![p("msg2"), p("msg3")]);
        assert_eq!(q.queued_count(&key("alice")), 0);
        assert!(q.is_active(&key("alice")));
    }

    #[test]
    fn when_drain_queued_called_on_empty_queue_then_returns_empty_vec() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
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
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg3")], None);
        assert_eq!(q.queued_count(&key("alice")), 2);
    }

    #[rstest]
    fn when_session_active_then_is_active_returns_true() {
        let mut q = SessionQueue::new();
        q.push(&key("bob"), vec![ContentPart::text("hello")], None);
        assert!(q.is_active(&key("bob")));
    }

    #[rstest]
    fn when_session_idle_then_is_active_returns_false() {
        let q = SessionQueue::new();
        assert!(!q.is_active(&key("nobody")));
    }

    #[rstest]
    fn when_mark_idle_then_drain_queued_returns_remaining_messages_for_merge() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg3")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg4")], None);

        let first = q.mark_idle(&key("alice"));
        assert_eq!(first, Some(p("msg2")));

        let remaining = q.drain_queued(&key("alice"));
        assert_eq!(remaining, vec![p("msg3"), p("msg4")]);
        assert!(q.is_active(&key("alice")));
    }

    #[rstest]
    fn when_mark_idle_returns_single_queued_then_drain_returns_empty() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push(&key("alice"), vec![ContentPart::text("msg2")], None);

        let first = q.mark_idle(&key("alice"));
        assert_eq!(first, Some(p("msg2")));

        let remaining = q.drain_queued(&key("alice"));
        assert!(remaining.is_empty());
    }

    #[rstest]
    fn when_push_only_on_idle_session_then_returns_true() {
        let mut q = SessionQueue::new();
        let was_idle = q.push_only(&key("alice"), vec![ContentPart::text("hello")], None);
        assert!(was_idle);
    }

    #[rstest]
    fn when_push_only_on_active_session_then_returns_false() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        let was_idle = q.push_only(&key("alice"), vec![ContentPart::text("msg2")], None);
        assert!(!was_idle);
    }

    #[rstest]
    fn when_push_only_called_then_message_queued_not_dispatched() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("hello")], None);
        assert!(!q.is_active(&key("alice")));
        assert_eq!(q.queued_count(&key("alice")), 1);
    }

    #[rstest]
    fn when_push_only_called_multiple_times_then_all_messages_queued() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push_only(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push_only(&key("alice"), vec![ContentPart::text("msg3")], None);
        assert_eq!(q.queued_count(&key("alice")), 3);
        assert!(!q.is_active(&key("alice")));
    }

    #[rstest]
    fn when_flush_pending_on_idle_session_with_messages_then_returns_merged_content() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push_only(&key("alice"), vec![ContentPart::text("msg2")], None);
        q.push_only(&key("alice"), vec![ContentPart::text("msg3")], None);

        let merged = q.flush_pending(&key("alice"));
        assert_eq!(
            merged,
            Some((
                vec![
                    ContentPart::text("msg1"),
                    ContentPart::text("msg2"),
                    ContentPart::text("msg3"),
                ],
                None
            ))
        );
        assert!(q.is_active(&key("alice")));
        assert_eq!(q.queued_count(&key("alice")), 0);
    }

    #[rstest]
    fn when_flush_pending_on_active_session_then_returns_none() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.push_only(&key("alice"), vec![ContentPart::text("msg2")], None);

        let merged = q.flush_pending(&key("alice"));
        assert_eq!(merged, None);
        assert_eq!(q.queued_count(&key("alice")), 1);
    }

    #[rstest]
    fn when_flush_pending_on_empty_queue_then_returns_none() {
        let mut q = SessionQueue::new();
        let merged = q.flush_pending(&key("alice"));
        assert_eq!(merged, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[rstest]
    fn when_flush_pending_with_single_message_then_returns_that_message() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("only-one")], None);

        let merged = q.flush_pending(&key("alice"));
        assert_eq!(merged, Some(p("only-one")));
        assert!(q.is_active(&key("alice")));
    }

    #[rstest]
    fn given_flushed_session_when_mark_idle_then_session_goes_idle() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.flush_pending(&key("alice"));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[rstest]
    fn given_flushed_session_when_new_messages_arrive_then_queued_normally() {
        let mut q = SessionQueue::new();
        q.push_only(&key("alice"), vec![ContentPart::text("msg1")], None);
        q.flush_pending(&key("alice"));

        let action = q.push(&key("alice"), vec![ContentPart::text("msg2")], None);
        assert_eq!(action, QueueAction::Enqueued);
        assert_eq!(q.queued_count(&key("alice")), 1);
    }

    #[rstest]
    fn given_active_session_when_mark_idle_forced_then_next_message_dispatches() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("prompt")], None);
        assert!(q.is_active(&key("alice")));

        q.mark_idle(&key("alice"));
        assert!(!q.is_active(&key("alice")));

        let action = q.push(&key("alice"), vec![ContentPart::text("after-cancel")], None);
        assert_eq!(action, QueueAction::Dispatch(p("after-cancel")));
    }

    #[rstest]
    fn given_active_session_with_queued_when_mark_idle_forced_then_queued_returned() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("prompt")], None);
        q.push(
            &key("alice"),
            vec![ContentPart::text("queued-while-busy")],
            None,
        );
        assert_eq!(q.queued_count(&key("alice")), 1);

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, Some(p("queued-while-busy")));
    }

    #[rstest]
    fn given_idle_session_when_mark_idle_called_again_then_noop() {
        let mut q = SessionQueue::new();
        q.push(&key("alice"), vec![ContentPart::text("msg")], None);
        q.mark_idle(&key("alice"));
        assert!(!q.is_active(&key("alice")));

        let next = q.mark_idle(&key("alice"));
        assert_eq!(next, None);
        assert!(!q.is_active(&key("alice")));
    }

    #[rstest]
    fn when_flush_pending_metadata_taken_from_last_entry() {
        use anyclaw_sdk_types::MessageMetadata;
        let mut q = SessionQueue::new();
        let meta1 = Some(MessageMetadata {
            reply_to_message_id: Some("id-1".into()),
            thread_id: None,
        });
        let meta2 = Some(MessageMetadata {
            reply_to_message_id: Some("id-2".into()),
            thread_id: None,
        });
        q.push_only(&key("alice"), vec![ContentPart::text("a")], meta1);
        q.push_only(&key("alice"), vec![ContentPart::text("b")], meta2.clone());

        let (content, meta) = q
            .flush_pending(&key("alice"))
            .expect("should return payload");
        assert_eq!(content.len(), 2);
        assert_eq!(meta, meta2);
    }
}
