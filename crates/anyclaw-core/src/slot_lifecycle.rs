use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::backoff::{CrashTracker, ExponentialBackoff};

/// Per-manager slot state: cancellation token, backoff, crash tracking, and disabled flag.
///
/// Each manager gets its own `SlotLifecycle` so crash isolation is per-manager —
/// one manager crashing does not affect the others' backoff or crash counters.
pub struct SlotLifecycle {
    /// Child token of the root cancel — cancelling this stops only this manager.
    pub cancel_token: CancellationToken,
    /// Exponential backoff calculator for restart delays.
    pub backoff: ExponentialBackoff,
    /// Sliding-window crash counter for crash-loop detection.
    pub crash_tracker: CrashTracker,
    /// Set `true` when the crash-loop threshold is exceeded; prevents further restarts.
    pub disabled: bool,
}

/// Action the supervisor should take after recording a crash.
pub enum CrashAction {
    /// The manager has been disabled due to crash-loop detection — do not restart.
    Disabled,
    /// Restart the manager after the given backoff delay.
    RestartAfter(Duration),
}

impl SlotLifecycle {
    /// Create a new lifecycle with a child cancellation token derived from `parent_cancel`.
    pub fn new(
        parent_cancel: &CancellationToken,
        backoff: ExponentialBackoff,
        crash_tracker: CrashTracker,
    ) -> Self {
        Self {
            cancel_token: parent_cancel.child_token(),
            backoff,
            crash_tracker,
            disabled: false,
        }
    }

    /// Record a crash and return the appropriate action (restart with backoff, or disable).
    pub fn record_crash_and_check(&mut self) -> CrashAction {
        self.crash_tracker.record_crash();
        if self.crash_tracker.is_crash_loop() {
            self.disabled = true;
            CrashAction::Disabled
        } else {
            let delay = self.backoff.next_delay();
            CrashAction::RestartAfter(delay)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::time::Duration;

    fn given_parent_cancel() -> CancellationToken {
        CancellationToken::new()
    }

    #[rstest]
    fn when_new_lifecycle_created_then_cancel_token_is_child_of_parent() {
        let parent = given_parent_cancel();
        let lifecycle = SlotLifecycle::new(
            &parent,
            ExponentialBackoff::default(),
            CrashTracker::default(),
        );
        assert!(!lifecycle.cancel_token.is_cancelled());
        parent.cancel();
        assert!(lifecycle.cancel_token.is_cancelled());
    }

    #[rstest]
    fn when_new_lifecycle_created_then_disabled_is_false() {
        let parent = given_parent_cancel();
        let lifecycle = SlotLifecycle::new(
            &parent,
            ExponentialBackoff::default(),
            CrashTracker::default(),
        );
        assert!(!lifecycle.disabled);
    }

    #[rstest]
    fn when_crash_loop_detected_then_disabled_and_returns_disabled_action() {
        let parent = given_parent_cancel();
        let mut lifecycle = SlotLifecycle::new(
            &parent,
            ExponentialBackoff::default(),
            CrashTracker::new(3, Duration::from_secs(60)),
        );
        lifecycle.record_crash_and_check();
        lifecycle.record_crash_and_check();
        let action = lifecycle.record_crash_and_check();
        assert!(lifecycle.disabled);
        assert!(matches!(action, CrashAction::Disabled));
    }

    #[rstest]
    fn when_crash_below_threshold_then_returns_restart_after() {
        let parent = given_parent_cancel();
        let mut lifecycle = SlotLifecycle::new(
            &parent,
            ExponentialBackoff::default(),
            CrashTracker::new(5, Duration::from_secs(60)),
        );
        let action = lifecycle.record_crash_and_check();
        assert!(!lifecycle.disabled);
        assert!(matches!(action, CrashAction::RestartAfter(_)));
    }
}
