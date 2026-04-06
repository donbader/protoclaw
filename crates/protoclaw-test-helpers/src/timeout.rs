use std::future::Future;
use std::time::Duration;

pub async fn with_timeout<F, T>(secs: u64, fut: F) -> T
where
    F: Future<Output = T>,
{
    tokio::time::timeout(Duration::from_secs(secs), fut)
        .await
        .unwrap_or_else(|_| panic!("test timed out after {secs}s"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn when_with_timeout_called_with_fast_future_then_returns_result() {
        let result = with_timeout(5, async { 42 }).await;
        assert_eq!(result, 42);
    }
}
