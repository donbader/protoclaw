use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::backoff::{CrashTracker, ExponentialBackoff};

pub struct SlotLifecycle {
    pub cancel_token: CancellationToken,
    pub backoff: ExponentialBackoff,
    pub crash_tracker: CrashTracker,
    pub disabled: bool,
}

pub enum CrashAction {
    Disabled,
    RestartAfter(Duration),
}

impl SlotLifecycle {
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
