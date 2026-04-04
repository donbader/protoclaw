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

    #[tokio::test]
    async fn with_timeout_completes_fast_future() {
        let result = with_timeout(5, async { 42 }).await;
        assert_eq!(result, 42);
    }
}
