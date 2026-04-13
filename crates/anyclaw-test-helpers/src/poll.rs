use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;

/// Polls an async condition until it returns `Some(T)` or the timeout expires.
///
/// The condition closure is called at ~100ms intervals. Returns `Some(T)` if
/// the condition is satisfied before the deadline, `None` if the timeout is reached.
///
/// Reusable for integration tests that need to wait for async state changes
/// without hardcoded sleeps.
pub async fn wait_for_condition<F, Fut, T>(timeout_ms: u64, mut condition: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(val) = condition().await {
            return Some(val);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[rstest]
    #[tokio::test]
    async fn when_condition_met_immediately_then_returns_value() {
        let result = wait_for_condition(1000, || async { Some(42_u32) }).await;
        assert_eq!(result, Some(42));
    }

    #[rstest]
    #[tokio::test]
    async fn when_condition_met_after_retries_then_returns_value() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let result = wait_for_condition(5000, move || {
            let count = cc.fetch_add(1, Ordering::SeqCst);
            async move { if count >= 2 { Some("ok") } else { None } }
        })
        .await;
        assert_eq!(result, Some("ok"));
    }

    #[rstest]
    #[tokio::test]
    async fn when_condition_never_met_then_returns_none_after_timeout() {
        let result = wait_for_condition::<_, _, u32>(200, || async { None }).await;
        assert_eq!(result, None);
    }
}
