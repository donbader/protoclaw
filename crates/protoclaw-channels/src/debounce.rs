use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use protoclaw_config::DebounceConfig;
use protoclaw_core::SessionKey;

/// Result of pushing a message into the debounce buffer.
#[derive(Debug, PartialEq)]
pub enum DebounceAction {
    /// Message buffered, waiting for window to expire. Do not dispatch yet.
    Buffered,
    /// Debounce disabled — dispatch this message immediately.
    Immediate(String),
    /// Session is mid-response — message queued for next turn.
    Queued,
}

/// Per-session debounce state.
struct SessionBuffer {
    messages: Vec<String>,
    last_push: Instant,
}

/// Manages per-session message debouncing with sliding window timers.
pub struct DebounceBuffer {
    config: DebounceConfig,
    buffers: HashMap<SessionKey, SessionBuffer>,
    /// Sessions currently receiving agent responses (mid-response).
    active_sessions: HashSet<SessionKey>,
    /// Messages queued during mid-response, to fire after agent finishes.
    queued: HashMap<SessionKey, Vec<String>>,
}

impl DebounceBuffer {
    pub fn new(config: DebounceConfig) -> Self {
        Self {
            config,
            buffers: HashMap::new(),
            active_sessions: HashSet::new(),
            queued: HashMap::new(),
        }
    }

    pub fn push(&mut self, session_key: &SessionKey, message: String) -> DebounceAction {
        if !self.config.enabled {
            return DebounceAction::Immediate(message);
        }

        if self.active_sessions.contains(session_key) && self.config.mid_response == "queue" {
            self.queued
                .entry(session_key.clone())
                .or_default()
                .push(message);
            return DebounceAction::Queued;
        }

        let entry = self.buffers.entry(session_key.clone()).or_insert_with(|| {
            SessionBuffer {
                messages: Vec::new(),
                last_push: Instant::now(),
            }
        });
        entry.messages.push(message);
        entry.last_push = Instant::now();
        DebounceAction::Buffered
    }

    pub fn ready_sessions(&self) -> Vec<SessionKey> {
        let window = Duration::from_millis(self.config.window_ms);
        let now = Instant::now();
        self.buffers
            .iter()
            .filter(|(_, buf)| now.duration_since(buf.last_push) >= window)
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub fn drain(&mut self, session_key: &SessionKey) -> Option<String> {
        let buf = self.buffers.remove(session_key)?;
        Some(buf.messages.join(&self.config.separator))
    }

    pub fn mark_session_active(&mut self, session_key: &SessionKey) {
        self.active_sessions.insert(session_key.clone());
    }

    pub fn mark_session_idle(&mut self, session_key: &SessionKey) {
        self.active_sessions.remove(session_key);
        if let Some(msgs) = self.queued.remove(session_key) {
            if !msgs.is_empty() {
                self.buffers.insert(session_key.clone(), SessionBuffer {
                    messages: msgs,
                    last_push: Instant::now(),
                });
            }
        }
    }

    pub fn drain_queued(&mut self, session_key: &SessionKey) -> Option<String> {
        let msgs = self.queued.remove(session_key)?;
        if msgs.is_empty() {
            return None;
        }
        Some(msgs.join(&self.config.separator))
    }

    pub fn has_pending(&self) -> bool {
        !self.buffers.is_empty() || !self.queued.is_empty()
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        let window = Duration::from_millis(self.config.window_ms);
        self.buffers
            .values()
            .map(|buf| buf.last_push + window)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> DebounceConfig {
        DebounceConfig {
            enabled: true,
            window_ms: 50,
            separator: "\n".into(),
            mid_response: "queue".into(),
        }
    }

    fn disabled_config() -> DebounceConfig {
        DebounceConfig {
            enabled: false,
            ..default_config()
        }
    }

    fn key(name: &str) -> SessionKey {
        SessionKey::new("test", "local", name)
    }

    #[test]
    fn new_creates_empty_buffer() {
        let buf = DebounceBuffer::new(default_config());
        assert!(!buf.has_pending());
        assert!(buf.next_deadline().is_none());
    }

    #[test]
    fn push_returns_buffered_when_enabled() {
        let mut buf = DebounceBuffer::new(default_config());
        let action = buf.push(&key("alice"), "hello".into());
        assert_eq!(action, DebounceAction::Buffered);
    }

    #[test]
    fn push_second_message_resets_timer_returns_buffered() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        let action = buf.push(&key("alice"), "msg2".into());
        assert_eq!(action, DebounceAction::Buffered);
    }

    #[test]
    fn drain_returns_merged_messages() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        buf.push(&key("alice"), "msg2".into());
        let merged = buf.drain(&key("alice"));
        assert_eq!(merged, Some("msg1\nmsg2".into()));
    }

    #[test]
    fn drain_unknown_session_returns_none() {
        let mut buf = DebounceBuffer::new(default_config());
        assert_eq!(buf.drain(&key("unknown")), None);
    }

    #[test]
    fn drain_removes_entry() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        buf.drain(&key("alice"));
        assert_eq!(buf.drain(&key("alice")), None);
    }

    #[tokio::test]
    async fn ready_sessions_returns_expired_keys() {
        let config = DebounceConfig {
            window_ms: 10,
            ..default_config()
        };
        let mut buf = DebounceBuffer::new(config);
        buf.push(&key("alice"), "msg1".into());
        tokio::time::sleep(Duration::from_millis(20)).await;
        let ready = buf.ready_sessions();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], key("alice"));
    }

    #[test]
    fn ready_sessions_excludes_recent_pushes() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        let ready = buf.ready_sessions();
        assert!(ready.is_empty());
    }

    #[test]
    fn push_disabled_returns_immediate() {
        let mut buf = DebounceBuffer::new(disabled_config());
        let action = buf.push(&key("alice"), "hello".into());
        assert_eq!(action, DebounceAction::Immediate("hello".into()));
    }

    #[test]
    fn push_disabled_does_not_buffer() {
        let mut buf = DebounceBuffer::new(disabled_config());
        buf.push(&key("alice"), "hello".into());
        assert!(!buf.has_pending());
    }

    #[test]
    fn push_mid_response_returns_queued() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        let action = buf.push(&key("alice"), "msg1".into());
        assert_eq!(action, DebounceAction::Queued);
    }

    #[test]
    fn drain_queued_returns_merged_queued_messages() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        buf.push(&key("alice"), "msg1".into());
        buf.push(&key("alice"), "msg2".into());
        let merged = buf.drain_queued(&key("alice"));
        assert_eq!(merged, Some("msg1\nmsg2".into()));
    }

    #[test]
    fn drain_queued_unknown_returns_none() {
        let mut buf = DebounceBuffer::new(default_config());
        assert_eq!(buf.drain_queued(&key("unknown")), None);
    }

    #[test]
    fn mark_session_idle_removes_from_active() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        buf.mark_session_idle(&key("alice"));
        let action = buf.push(&key("alice"), "msg1".into());
        assert_eq!(action, DebounceAction::Buffered);
    }

    #[test]
    fn has_pending_true_when_buffered() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        assert!(buf.has_pending());
    }

    #[test]
    fn has_pending_true_when_queued() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        buf.push(&key("alice"), "msg1".into());
        assert!(buf.has_pending());
    }

    #[test]
    fn next_deadline_returns_earliest() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "msg1".into());
        let deadline = buf.next_deadline();
        assert!(deadline.is_some());
        let remaining = deadline.unwrap().duration_since(Instant::now());
        assert!(remaining.as_millis() <= 50);
    }

    #[test]
    fn next_deadline_none_when_empty() {
        let buf = DebounceBuffer::new(default_config());
        assert!(buf.next_deadline().is_none());
    }

    #[test]
    fn separate_sessions_independent() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.push(&key("alice"), "a1".into());
        buf.push(&key("bob"), "b1".into());
        buf.push(&key("alice"), "a2".into());

        let alice = buf.drain(&key("alice"));
        assert_eq!(alice, Some("a1\na2".into()));

        let bob = buf.drain(&key("bob"));
        assert_eq!(bob, Some("b1".into()));
    }

    #[test]
    fn custom_separator() {
        let config = DebounceConfig {
            separator: " | ".into(),
            ..default_config()
        };
        let mut buf = DebounceBuffer::new(config);
        buf.push(&key("alice"), "msg1".into());
        buf.push(&key("alice"), "msg2".into());
        let merged = buf.drain(&key("alice"));
        assert_eq!(merged, Some("msg1 | msg2".into()));
    }

    #[test]
    fn mark_session_idle_moves_queued_to_buffer() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        buf.push(&key("alice"), "q1".into());
        buf.push(&key("alice"), "q2".into());
        assert!(!buf.buffers.contains_key(&key("alice")));
        buf.mark_session_idle(&key("alice"));
        assert!(buf.buffers.contains_key(&key("alice")));
        assert!(!buf.queued.contains_key(&key("alice")));
        let merged = buf.drain(&key("alice"));
        assert_eq!(merged, Some("q1\nq2".into()));
    }

    #[test]
    fn post_response_window_accumulates_then_flushes() {
        let mut buf = DebounceBuffer::new(default_config());
        buf.mark_session_active(&key("alice"));
        buf.push(&key("alice"), "q1".into());
        buf.mark_session_idle(&key("alice"));
        let action = buf.push(&key("alice"), "extra".into());
        assert_eq!(action, DebounceAction::Buffered);
        let merged = buf.drain(&key("alice"));
        assert_eq!(merged, Some("q1\nextra".into()));
    }

    #[tokio::test]
    async fn event_loop_pattern_protects_against_stale_deadline() {
        let config = DebounceConfig {
            window_ms: 500,
            ..default_config()
        };
        let mut buf = DebounceBuffer::new(config);

        buf.push(&key("alice"), "msg1".into());
        let deadline_std = buf.next_deadline().expect("should have deadline");
        let deadline_tokio = tokio::time::Instant::from_std(deadline_std);

        tokio::time::sleep(Duration::from_millis(100)).await;
        buf.push(&key("alice"), "msg2".into());

        tokio::time::sleep_until(deadline_tokio).await;

        let ready = buf.ready_sessions();
        assert!(
            ready.is_empty(),
            "ready_sessions() should NOT return session after last_push reset, got {:?}",
            ready
        );

        tokio::time::sleep(Duration::from_millis(150)).await;
        let ready = buf.ready_sessions();
        assert_eq!(ready.len(), 1, "session should be ready now");

        let merged = buf.drain(&key("alice"));
        assert_eq!(merged, Some("msg1\nmsg2".into()));
    }

    #[tokio::test]
    async fn from_std_conversion_does_not_fire_early() {
        let config = DebounceConfig {
            window_ms: 300,
            ..default_config()
        };
        let mut buf = DebounceBuffer::new(config);
        buf.push(&key("alice"), "msg1".into());

        let deadline_std = buf.next_deadline().unwrap();
        let deadline_tokio = tokio::time::Instant::from_std(deadline_std);

        let before = std::time::Instant::now();
        tokio::time::sleep_until(deadline_tokio).await;
        let elapsed = before.elapsed();

        assert!(
            elapsed >= Duration::from_millis(250),
            "sleep_until fired too early: {:?} (expected ~300ms)",
            elapsed
        );
    }
}
