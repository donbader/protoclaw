use protoclaw_core::ManagerHandle;
use tokio::sync::mpsc;

pub fn make_handle<C: Send + 'static>(buffer: usize) -> (ManagerHandle<C>, mpsc::Receiver<C>) {
    let (tx, rx) = mpsc::channel(buffer);
    (ManagerHandle::new(tx), rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn make_handle_sends_and_receives() {
        let (handle, mut rx) = make_handle::<String>(8);
        handle.send("hello".into()).await.unwrap();
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg, "hello");
    }
}
