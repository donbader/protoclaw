use std::time::{Duration, Instant};

use crate::constants;

pub struct ExponentialBackoff {
    current: Duration,
    max: Duration,
    base: Duration,
    attempts: u32,
}

impl ExponentialBackoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            current: base,
            max,
            base,
            attempts: 0,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = (self.current * 2).min(self.max);
        self.attempts += 1;
        delay
    }

    pub fn reset(&mut self) {
        self.current = self.base;
        self.attempts = 0;
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(
            Duration::from_millis(constants::DEFAULT_BACKOFF_BASE_MS),
            Duration::from_secs(constants::DEFAULT_BACKOFF_MAX_SECS),
        )
    }
}

pub struct CrashTracker {
    timestamps: Vec<Instant>,
    max_crashes: u32,
    window: Duration,
}

impl CrashTracker {
    pub fn new(max_crashes: u32, window: Duration) -> Self {
        Self {
            timestamps: Vec::new(),
            max_crashes,
            window,
        }
    }

    pub fn record_crash(&mut self) {
        let now = Instant::now();
        self.timestamps
            .retain(|t| now.duration_since(*t) < self.window);
        self.timestamps.push(now);
    }

    pub fn is_crash_loop(&self) -> bool {
        let now = Instant::now();
        let recent = self
            .timestamps
            .iter()
            .filter(|t| now.duration_since(**t) < self.window)
            .count();
        recent >= self.max_crashes as usize
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

impl Default for CrashTracker {
    fn default() -> Self {
        Self::new(
            constants::DEFAULT_CRASH_MAX,
            Duration::from_secs(constants::DEFAULT_CRASH_WINDOW_SECS),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn when_new_backoff_created_then_initial_delay_is_100ms() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn when_next_called_repeatedly_then_delay_doubles_each_time() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.next_delay(), Duration::from_millis(200));
        assert_eq!(b.next_delay(), Duration::from_millis(400));
        assert_eq!(b.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn when_delay_exceeds_max_then_capped_at_30s() {
        let mut b = ExponentialBackoff::default();
        for _ in 0..20 {
            b.next_delay();
        }
        assert_eq!(b.next_delay(), Duration::from_secs(30));
    }

    #[test]
    fn when_reset_called_then_delay_returns_to_base() {
        let mut b = ExponentialBackoff::default();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.attempts(), 1);
    }

    #[test]
    fn when_next_called_multiple_times_then_attempt_count_increments() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.attempts(), 0);
        b.next_delay();
        assert_eq!(b.attempts(), 1);
        b.next_delay();
        assert_eq!(b.attempts(), 2);
    }

    #[test]
    fn when_default_backoff_created_then_params_match_constants() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn given_rapid_crashes_when_threshold_exceeded_then_crash_loop_detected() {
        let mut tracker = CrashTracker::new(3, Duration::from_secs(60));
        assert!(!tracker.is_crash_loop());

        tracker.record_crash();
        tracker.record_crash();
        assert!(!tracker.is_crash_loop());

        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }

    #[test]
    fn when_reset_called_then_crash_history_cleared() {
        let mut tracker = CrashTracker::new(3, Duration::from_secs(60));
        tracker.record_crash();
        tracker.record_crash();
        tracker.record_crash();
        assert!(tracker.is_crash_loop());

        tracker.reset();
        assert!(!tracker.is_crash_loop());
    }

    #[test]
    fn when_default_crash_tracker_created_then_max_is_5_in_60s() {
        let mut tracker = CrashTracker::default();
        for _ in 0..4 {
            tracker.record_crash();
        }
        assert!(!tracker.is_crash_loop());
        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }
}
