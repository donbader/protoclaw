use std::time::{Duration, Instant};

use crate::constants;

/// Exponential backoff calculator for manager restart delays.
///
/// Starts at a base delay and doubles on each attempt up to a configurable cap.
/// Used by the supervisor to avoid hammering a repeatedly-failing subprocess.
pub struct ExponentialBackoff {
    current: Duration,
    max: Duration,
    base: Duration,
    attempts: u32,
}

impl ExponentialBackoff {
    /// Create a new backoff with the given base delay and maximum cap.
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            current: base,
            max,
            base,
            attempts: 0,
        }
    }

    /// Return the current delay and advance to the next (doubled, capped at max).
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = (self.current * 2).min(self.max);
        self.attempts += 1;
        delay
    }

    /// Reset the backoff to its initial base delay and zero the attempt counter.
    pub fn reset(&mut self) {
        self.current = self.base;
        self.attempts = 0;
    }

    /// Number of delays consumed so far (incremented by each [`next_delay`](Self::next_delay) call).
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

/// Sliding-window crash counter that detects crash loops.
///
/// Records crash timestamps and checks whether the count within a rolling
/// time window exceeds a threshold. When it does, the supervisor marks the
/// manager as disabled and stops restarting it.
pub struct CrashTracker {
    timestamps: Vec<Instant>,
    max_crashes: u32,
    window: Duration,
    long_max_crashes: u32,
    long_window: Duration,
    total_crashes: u32,
}

impl CrashTracker {
    /// Create a tracker that trips after `max_crashes` within `window`.
    pub fn new(max_crashes: u32, window: Duration) -> Self {
        Self {
            timestamps: Vec::new(),
            max_crashes,
            window,
            long_max_crashes: constants::DEFAULT_CRASH_LONG_MAX,
            long_window: Duration::from_secs(constants::DEFAULT_CRASH_LONG_WINDOW_SECS),
            total_crashes: 0,
        }
    }

    /// Configure a long-horizon crash window that catches low-frequency
    /// but persistent crashes (e.g., every 10 minutes for an hour).
    pub fn with_long_horizon(mut self, max_crashes: u32, window: Duration) -> Self {
        self.long_max_crashes = max_crashes;
        self.long_window = window;
        self
    }

    /// Record a crash and prune timestamps outside the window.
    pub fn record_crash(&mut self) {
        let now = Instant::now();
        self.timestamps
            .retain(|t| now.duration_since(*t) < self.long_window);
        self.timestamps.push(now);
        self.total_crashes += 1;
    }

    /// Check whether the number of recent crashes meets or exceeds the threshold.
    /// Checks both the short sliding window and the long-horizon counter.
    pub fn is_crash_loop(&self) -> bool {
        let now = Instant::now();
        let short_recent = self
            .timestamps
            .iter()
            .filter(|t| now.duration_since(**t) < self.window)
            .count();
        if short_recent >= self.max_crashes as usize {
            return true;
        }
        let long_recent = self
            .timestamps
            .iter()
            .filter(|t| now.duration_since(**t) < self.long_window)
            .count();
        long_recent >= self.long_max_crashes as usize
    }

    /// Clear all recorded crash timestamps.
    pub fn reset(&mut self) {
        self.timestamps.clear();
        self.total_crashes = 0;
    }

    /// Total number of crashes recorded since creation or last reset.
    pub fn total_crashes(&self) -> u32 {
        self.total_crashes
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

    #[test]
    fn given_low_frequency_crashes_when_long_horizon_exceeded_then_crash_loop_detected() {
        let mut tracker = CrashTracker::new(5, Duration::from_secs(60))
            .with_long_horizon(3, Duration::from_secs(3600));
        tracker.record_crash();
        assert!(!tracker.is_crash_loop());
        tracker.record_crash();
        assert!(!tracker.is_crash_loop());
        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }

    #[test]
    fn when_reset_called_then_total_crashes_cleared() {
        let mut tracker = CrashTracker::new(5, Duration::from_secs(60));
        tracker.record_crash();
        tracker.record_crash();
        assert_eq!(tracker.total_crashes(), 2);
        tracker.reset();
        assert_eq!(tracker.total_crashes(), 0);
    }

    #[test]
    fn given_default_long_horizon_when_10_crashes_in_hour_then_crash_loop_detected() {
        let mut tracker = CrashTracker::new(100, Duration::from_secs(60))
            .with_long_horizon(10, Duration::from_secs(3600));
        for _ in 0..9 {
            tracker.record_crash();
        }
        assert!(!tracker.is_crash_loop());
        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }
}
