//! Named constants for protoclaw internals.
//!
//! Internal guards are NOT user-configurable — they protect implementation invariants.
//! Default value constants are used by `ExponentialBackoff::default()` and `CrashTracker::default()`.

// === Internal Guards (NOT user-configurable) ===

/// Poll timeout per connection in the agents/channels poll loop (milliseconds).
pub const POLL_TIMEOUT_MS: u64 = 1;
/// Sleep interval between poll sweeps to prevent busy-looping (milliseconds).
pub const POLL_INTERVAL_MS: u64 = 50;
/// Capacity of manager command channels (mpsc).
pub const CMD_CHANNEL_CAPACITY: usize = 16;
/// Capacity of the channel events pipe (supervisor → channels manager).
pub const EVENT_CHANNEL_CAPACITY: usize = 64;
/// Epoch ticker interval for WASM sandbox (seconds).
pub const EPOCH_TICK_INTERVAL_SECS: u64 = 1;
/// HTTP client timeout for `protoclaw status` command (seconds).
pub const STATUS_HTTP_TIMEOUT_SECS: u64 = 5;

// === Default Values (used by Default impls, mirrored in config serde defaults) ===

/// Default base delay for ExponentialBackoff (milliseconds).
pub const DEFAULT_BACKOFF_BASE_MS: u64 = 100;
/// Default max delay for ExponentialBackoff (seconds).
pub const DEFAULT_BACKOFF_MAX_SECS: u64 = 30;
/// Default max crashes before crash loop detection.
pub const DEFAULT_CRASH_MAX: u32 = 5;
/// Default crash tracking window (seconds).
pub const DEFAULT_CRASH_WINDOW_SECS: u64 = 60;

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn when_checking_poll_interval_then_matches_expected_value() {
        assert_eq!(POLL_INTERVAL_MS, 50);
    }

    #[test]
    fn when_checking_poll_timeout_then_matches_expected_value() {
        assert_eq!(POLL_TIMEOUT_MS, 1);
    }

    #[test]
    fn when_checking_cmd_channel_capacity_then_matches_expected_value() {
        assert_eq!(CMD_CHANNEL_CAPACITY, 16);
    }

    #[test]
    fn when_checking_event_channel_capacity_then_matches_expected_value() {
        assert_eq!(EVENT_CHANNEL_CAPACITY, 64);
    }

    #[test]
    fn when_checking_epoch_tick_interval_then_matches_expected_value() {
        assert_eq!(EPOCH_TICK_INTERVAL_SECS, 1);
    }

    #[test]
    fn when_checking_status_http_timeout_then_matches_expected_value() {
        assert_eq!(STATUS_HTTP_TIMEOUT_SECS, 5);
    }

    #[test]
    fn when_checking_backoff_base_ms_then_matches_expected_value() {
        assert_eq!(DEFAULT_BACKOFF_BASE_MS, 100);
    }

    #[test]
    fn when_checking_backoff_max_secs_then_matches_expected_value() {
        assert_eq!(DEFAULT_BACKOFF_MAX_SECS, 30);
    }

    #[test]
    fn when_checking_crash_max_then_matches_expected_value() {
        assert_eq!(DEFAULT_CRASH_MAX, 5);
    }

    #[test]
    fn when_checking_crash_window_secs_then_matches_expected_value() {
        assert_eq!(DEFAULT_CRASH_WINDOW_SECS, 60);
    }
}
