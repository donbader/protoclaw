pub async fn wait_for_port(
    mut port_rx: tokio::sync::watch::Receiver<u16>,
    timeout_ms: u64,
) -> Option<u16> {
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(timeout_ms) {
        let port = *port_rx.borrow();
        if port != 0 {
            return Some(port);
        }
        if port_rx.changed().await.is_err() {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[tokio::test]
    async fn given_port_sent_after_delay_when_wait_for_port_called_then_returns_port() {
        let (tx, rx) = tokio::sync::watch::channel(0u16);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            tx.send(8080).unwrap();
        });
        let port = wait_for_port(rx, 1000).await;
        assert_eq!(port, Some(8080));
    }

    #[tokio::test]
    async fn given_sender_dropped_when_wait_for_port_called_then_returns_none() {
        let (tx, rx) = tokio::sync::watch::channel(0u16);
        drop(tx);
        let port = wait_for_port(rx, 1000).await;
        assert_eq!(port, None);
    }
}
