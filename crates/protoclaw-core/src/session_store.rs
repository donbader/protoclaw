use std::future::Future;

use thiserror::Error;

/// A persisted session record stored by a [`SessionStore`].
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedSession {
    /// Unique session key (matches `SessionKey` used by the runtime).
    pub session_key: String,
    /// Name of the agent that owns this session.
    pub agent_name: String,
    /// The ACP-level session id assigned by the agent.
    pub acp_session_id: String,
    /// Unix timestamp (seconds) when the session was first created.
    pub created_at: i64,
    /// Unix timestamp (seconds) of the last observed activity.
    pub last_active_at: i64,
    /// Whether the session has been closed.
    pub closed: bool,
}

/// Errors produced by a [`SessionStore`] implementation.
#[derive(Debug, Error)]
pub enum SessionStoreError {
    /// A storage backend error (e.g. database I/O).
    #[error("backend error: {0}")]
    Backend(String),

    /// A serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Trait for persisting ACP session state across restarts.
///
/// All methods are async and the trait requires `Send + Sync + 'static` so
/// implementations can be stored in an `Arc` and shared across task boundaries.
///
/// The blanket implementations here use `async fn` in trait syntax (Rust 1.75+).
pub trait SessionStore: Send + Sync + 'static {
    /// Return all sessions that are not yet marked closed.
    fn load_open_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<PersistedSession>, SessionStoreError>> + Send;

    /// Insert or update a session record (keyed on `session.session_key`).
    fn upsert_session(
        &self,
        session: &PersistedSession,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send;

    /// Mark a session as closed by its session key.
    fn mark_closed(
        &self,
        session_key: &str,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send;

    /// Update the `last_active_at` timestamp for a session.
    fn update_last_active(
        &self,
        session_key: &str,
        timestamp: i64,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send;

    /// Delete sessions older than `max_age_secs` seconds (based on `last_active_at`).
    ///
    /// Returns the number of rows deleted.
    fn delete_expired(
        &self,
        max_age_secs: i64,
    ) -> impl Future<Output = Result<u64, SessionStoreError>> + Send;
}

/// A no-op [`SessionStore`] that discards all writes and returns empty reads.
///
/// Used as the default when no persistent store is configured.
#[derive(Debug, Clone, Default)]
pub struct NoopSessionStore;

impl SessionStore for NoopSessionStore {
    async fn load_open_sessions(&self) -> Result<Vec<PersistedSession>, SessionStoreError> {
        Ok(vec![])
    }

    async fn upsert_session(&self, _session: &PersistedSession) -> Result<(), SessionStoreError> {
        Ok(())
    }

    async fn mark_closed(&self, _session_key: &str) -> Result<(), SessionStoreError> {
        Ok(())
    }

    async fn update_last_active(
        &self,
        _session_key: &str,
        _timestamp: i64,
    ) -> Result<(), SessionStoreError> {
        Ok(())
    }

    async fn delete_expired(&self, _max_age_secs: i64) -> Result<u64, SessionStoreError> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn given_a_persisted_session() -> PersistedSession {
        PersistedSession {
            session_key: "key-1".to_string(),
            agent_name: "agent-a".to_string(),
            acp_session_id: "acp-123".to_string(),
            created_at: 1_000_000,
            last_active_at: 1_000_100,
            closed: false,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_load_open_sessions_called_then_returns_empty_vec() {
        let store = NoopSessionStore;
        let result = store.load_open_sessions().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_upsert_session_called_then_returns_ok(
        given_a_persisted_session: PersistedSession,
    ) {
        let store = NoopSessionStore;
        let result = store.upsert_session(&given_a_persisted_session).await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_mark_closed_called_then_returns_ok() {
        let store = NoopSessionStore;
        let result = store.mark_closed("key-1").await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_update_last_active_called_then_returns_ok() {
        let store = NoopSessionStore;
        let result = store.update_last_active("key-1", 9_999_999).await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_noop_delete_expired_called_then_returns_zero() {
        let store = NoopSessionStore;
        let result = store.delete_expired(7 * 24 * 3600).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[rstest]
    fn when_persisted_session_cloned_then_equals_original(
        given_a_persisted_session: PersistedSession,
    ) {
        let cloned = given_a_persisted_session.clone();
        assert_eq!(cloned, given_a_persisted_session);
    }

    #[rstest]
    fn when_session_store_error_backend_formatted_then_contains_message() {
        let err = SessionStoreError::Backend("disk full".to_string());
        assert!(err.to_string().contains("disk full"));
    }

    #[rstest]
    fn when_session_store_error_serialization_formatted_then_contains_message() {
        let err = SessionStoreError::Serialization("invalid utf-8".to_string());
        assert!(err.to_string().contains("invalid utf-8"));
    }

    #[rstest]
    fn when_noop_session_store_debug_formatted_then_does_not_panic() {
        let store = NoopSessionStore;
        let _ = format!("{store:?}");
    }
}
