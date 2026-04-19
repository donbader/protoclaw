use std::future::Future;
use std::pin::Pin;

use crate::session_store::SessionStoreError;

/// A single message stored as context for a group chat.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMessage {
    /// Opaque key identifying the group, e.g. `"telegram:-100123456"`.
    pub group_key: String,
    /// Display name of the message sender.
    pub sender: String,
    /// Text content of the message.
    pub content: String,
    /// Unix timestamp (seconds) when the message was sent.
    pub timestamp: i64,
}

/// Trait for buffering unmentioned group-chat messages as context.
///
/// Implementations store messages keyed by `group_key` and return them
/// atomically (read + delete) when the bot is mentioned, so the agent
/// receives recent conversational context alongside the triggering message.
pub trait ContextStore: Send + Sync + 'static {
    /// Persist `msg` for later retrieval.
    ///
    /// After inserting, trims the buffer so it holds at most `limit` messages
    /// (oldest rows deleted first). If `limit` is 0 this is a no-op.
    fn store_context(
        &self,
        msg: &ContextMessage,
        limit: usize,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send;

    /// Remove and return all buffered messages for `group_key`, oldest first.
    ///
    /// Returns an empty `Vec` when nothing is buffered.
    fn take_context(
        &self,
        group_key: &str,
    ) -> impl Future<Output = Result<Vec<ContextMessage>, SessionStoreError>> + Send;
}

/// Object-safe alias for [`ContextStore`]. Use `Arc<dyn DynContextStore>` for runtime dispatch.
/// Implementors write `impl ContextStore for X`; the blanket impl provides `DynContextStore`.
pub trait DynContextStore: Send + Sync + 'static {
    /// Persist `msg` and trim buffer to `limit` (boxed future for object safety).
    fn store_context<'a>(
        &'a self,
        msg: &'a ContextMessage,
        limit: usize,
    ) -> Pin<Box<dyn Future<Output = Result<(), SessionStoreError>> + Send + 'a>>;

    /// Remove and return all buffered messages for `group_key` (boxed future for object safety).
    fn take_context<'a>(
        &'a self,
        group_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ContextMessage>, SessionStoreError>> + Send + 'a>>;
}

impl<T: ContextStore> DynContextStore for T {
    fn store_context<'a>(
        &'a self,
        msg: &'a ContextMessage,
        limit: usize,
    ) -> Pin<Box<dyn Future<Output = Result<(), SessionStoreError>> + Send + 'a>> {
        Box::pin(ContextStore::store_context(self, msg, limit))
    }

    fn take_context<'a>(
        &'a self,
        group_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ContextMessage>, SessionStoreError>> + Send + 'a>>
    {
        Box::pin(ContextStore::take_context(self, group_key))
    }
}

/// A no-op [`ContextStore`] that discards writes and returns empty reads.
///
/// Used as the default when context history buffering is not configured.
#[derive(Debug, Clone, Default)]
pub struct NoopContextStore;

impl ContextStore for NoopContextStore {
    fn store_context(
        &self,
        _msg: &ContextMessage,
        _limit: usize,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send {
        std::future::ready(Ok(()))
    }

    fn take_context(
        &self,
        _group_key: &str,
    ) -> impl Future<Output = Result<Vec<ContextMessage>, SessionStoreError>> + Send {
        std::future::ready(Ok(vec![]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn given_a_context_message() -> ContextMessage {
        ContextMessage {
            group_key: "telegram:-100123456".to_string(),
            sender: "alice".to_string(),
            content: "hello world".to_string(),
            timestamp: 1_000_000,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_store_context_called_then_returns_ok(
        given_a_context_message: ContextMessage,
    ) {
        let store = NoopContextStore;
        let result = ContextStore::store_context(&store, &given_a_context_message, 50).await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_take_context_called_then_returns_empty_vec() {
        let store = NoopContextStore;
        let result = ContextStore::take_context(&store, "telegram:-100123456").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[rstest]
    fn when_context_message_cloned_then_equals_original(given_a_context_message: ContextMessage) {
        let cloned = given_a_context_message.clone();
        assert_eq!(cloned, given_a_context_message);
    }

    #[rstest]
    fn when_noop_context_store_debug_formatted_then_does_not_panic() {
        let store = NoopContextStore;
        let _ = format!("{store:?}");
    }
}
