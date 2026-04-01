use std::time::{Duration, Instant};

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
        Self::new(Duration::from_millis(100), Duration::from_secs(30))
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
        Self::new(5, Duration::from_secs(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_starts_at_100ms() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn backoff_doubles_each_call() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.next_delay(), Duration::from_millis(200));
        assert_eq!(b.next_delay(), Duration::from_millis(400));
        assert_eq!(b.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn backoff_caps_at_30s() {
        let mut b = ExponentialBackoff::default();
        for _ in 0..20 {
            b.next_delay();
        }
        assert_eq!(b.next_delay(), Duration::from_secs(30));
    }

    #[test]
    fn backoff_resets_to_base() {
        let mut b = ExponentialBackoff::default();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.attempts(), 1);
    }

    #[test]
    fn backoff_tracks_attempts() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.attempts(), 0);
        b.next_delay();
        assert_eq!(b.attempts(), 1);
        b.next_delay();
        assert_eq!(b.attempts(), 2);
    }

    #[test]
    fn backoff_default_has_correct_params() {
        let mut b = ExponentialBackoff::default();
        assert_eq!(b.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn crash_tracker_detects_crash_loop() {
        let mut tracker = CrashTracker::new(3, Duration::from_secs(60));
        assert!(!tracker.is_crash_loop());

        tracker.record_crash();
        tracker.record_crash();
        assert!(!tracker.is_crash_loop());

        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }

    #[test]
    fn crash_tracker_reset_clears_history() {
        let mut tracker = CrashTracker::new(3, Duration::from_secs(60));
        tracker.record_crash();
        tracker.record_crash();
        tracker.record_crash();
        assert!(tracker.is_crash_loop());

        tracker.reset();
        assert!(!tracker.is_crash_loop());
    }

    #[test]
    fn crash_tracker_default_is_5_in_60s() {
        let mut tracker = CrashTracker::default();
        for _ in 0..4 {
            tracker.record_crash();
        }
        assert!(!tracker.is_crash_loop());
        tracker.record_crash();
        assert!(tracker.is_crash_loop());
    }
}
