use protoclaw_sdk_types::PermissionResponse;
use std::collections::HashMap;
use tokio::sync::oneshot;

/// Registry-only helper for managing permission request/response oneshot channels.
///
/// Channels compose this into their state struct and use it in their
/// `request_permission()` trait implementation.
///
/// # Usage
/// ```ignore
/// // In request_permission():
/// let rx = self.broker.register(&req.request_id);
/// // ... do channel-specific UI work (send buttons, etc.) ...
/// rx.await.map_err(|_| ChannelSdkError::Protocol("closed".into()))
///
/// // In resolution path (callback handler, HTTP endpoint, etc.):
/// self.broker.resolve(&request_id, &option_id);
/// ```
pub struct PermissionBroker {
    resolvers: HashMap<String, oneshot::Sender<PermissionResponse>>,
}

impl PermissionBroker {
    pub fn new() -> Self {
        Self {
            resolvers: HashMap::new(),
        }
    }

    /// Register a pending permission request. Returns a receiver that will
    /// resolve when `resolve()` is called with the same request_id.
    pub fn register(&mut self, request_id: &str) -> oneshot::Receiver<PermissionResponse> {
        let (tx, rx) = oneshot::channel();
        self.resolvers.insert(request_id.to_string(), tx);
        rx
    }

    /// Resolve a pending permission request. Returns true if the request_id
    /// was found and resolved, false if unknown.
    pub fn resolve(&mut self, request_id: &str, option_id: &str) -> bool {
        if let Some(tx) = self.resolvers.remove(request_id) {
            let _ = tx.send(PermissionResponse {
                request_id: request_id.to_string(),
                option_id: option_id.to_string(),
            });
            true
        } else {
            false
        }
    }
}

impl Default for PermissionBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn when_register_called_then_receiver_resolves_on_resolve() {
        let mut broker = PermissionBroker::new();
        let rx = broker.register("req-1");
        let resolved = broker.resolve("req-1", "allow");
        assert!(resolved);
        let resp = rx.await.unwrap();
        assert_eq!(resp.request_id, "req-1");
        assert_eq!(resp.option_id, "allow");
    }

    #[rstest]
    #[tokio::test]
    async fn when_resolve_called_with_unknown_id_then_returns_false() {
        let mut broker = PermissionBroker::new();
        assert!(!broker.resolve("nonexistent", "allow"));
    }

    #[rstest]
    #[tokio::test]
    async fn when_resolve_called_with_known_id_then_returns_true_and_removes_entry() {
        let mut broker = PermissionBroker::new();
        let _rx = broker.register("req-1");
        assert!(broker.resolve("req-1", "allow"));
        assert!(!broker.resolve("req-1", "allow"));
    }

    #[rstest]
    #[tokio::test]
    async fn when_multiple_registers_called_then_each_gets_independent_receiver() {
        let mut broker = PermissionBroker::new();
        let rx1 = broker.register("req-1");
        let rx2 = broker.register("req-2");

        broker.resolve("req-1", "allow");
        broker.resolve("req-2", "deny");

        let resp1 = rx1.await.unwrap();
        let resp2 = rx2.await.unwrap();
        assert_eq!(resp1.option_id, "allow");
        assert_eq!(resp2.option_id, "deny");
    }

    #[rstest]
    #[tokio::test]
    async fn when_broker_dropped_then_pending_receivers_get_error() {
        let rx = {
            let mut broker = PermissionBroker::new();
            broker.register("req-1")
        };
        assert!(rx.await.is_err());
    }
}
