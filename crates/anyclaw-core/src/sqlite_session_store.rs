use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use crate::context_store::{ContextMessage, ContextStore};
use crate::session_store::{PersistedSession, SessionStore, SessionStoreError};

/// SQLite-backed [`SessionStore`] implementation using rusqlite (bundled).
///
/// All database operations run on a blocking thread via `tokio::task::spawn_blocking`
/// to avoid blocking the async runtime. The connection is wrapped in `Arc<Mutex<_>>`
/// so the store can be cloned and shared across tasks.
#[derive(Clone)]
pub struct SqliteSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSessionStore {
    /// Open (or create) a SQLite database at the given file path.
    pub fn open(path: &str) -> Result<Self, SessionStoreError> {
        let conn = Connection::open(path).map_err(|e| SessionStoreError::Backend(e.to_string()))?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory SQLite database (useful for tests and when no persistence is needed).
    pub fn open_in_memory() -> Result<Self, SessionStoreError> {
        let conn =
            Connection::open_in_memory().map_err(|e| SessionStoreError::Backend(e.to_string()))?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_schema(conn: &Connection) -> Result<(), SessionStoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sessions (
                 session_key     TEXT PRIMARY KEY,
                 agent_name      TEXT NOT NULL,
                 acp_session_id  TEXT NOT NULL,
                 created_at      INTEGER NOT NULL,
                 last_active_at  INTEGER NOT NULL,
                 closed          INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS context_messages (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 group_key   TEXT NOT NULL,
                 sender      TEXT NOT NULL,
                 content     TEXT NOT NULL,
                 timestamp   INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_context_messages_group_key
                 ON context_messages (group_key);",
        )
        .map_err(|e| SessionStoreError::Backend(e.to_string()))
    }
}

impl SessionStore for SqliteSessionStore {
    async fn load_open_sessions(&self) -> Result<Vec<PersistedSession>, SessionStoreError> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT session_key, agent_name, acp_session_id, created_at, last_active_at, closed
                     FROM sessions WHERE closed = 0",
                )
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(PersistedSession {
                        session_key: row.get(0)?,
                        agent_name: row.get(1)?,
                        acp_session_id: row.get(2)?,
                        created_at: row.get(3)?,
                        last_active_at: row.get(4)?,
                        closed: row.get::<_, i64>(5)? != 0,
                    })
                })
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            rows.map(|r| r.map_err(|e| SessionStoreError::Backend(e.to_string())))
                .collect()
        })
        .await
        .map_err(|e| SessionStoreError::Backend(e.to_string()))?
    }

    async fn upsert_session(&self, session: &PersistedSession) -> Result<(), SessionStoreError> {
        let conn = Arc::clone(&self.conn);
        let session = session.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            conn.execute(
                "INSERT OR REPLACE INTO sessions
                     (session_key, agent_name, acp_session_id, created_at, last_active_at, closed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    session.session_key,
                    session.agent_name,
                    session.acp_session_id,
                    session.created_at,
                    session.last_active_at,
                    session.closed as i64,
                ],
            )
            .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| SessionStoreError::Backend(e.to_string()))?
    }

    async fn mark_closed(&self, session_key: &str) -> Result<(), SessionStoreError> {
        let conn = Arc::clone(&self.conn);
        let session_key = session_key.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            conn.execute(
                "UPDATE sessions SET closed = 1 WHERE session_key = ?1",
                params![session_key],
            )
            .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| SessionStoreError::Backend(e.to_string()))?
    }

    async fn update_last_active(
        &self,
        session_key: &str,
        timestamp: i64,
    ) -> Result<(), SessionStoreError> {
        let conn = Arc::clone(&self.conn);
        let session_key = session_key.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            conn.execute(
                "UPDATE sessions SET last_active_at = ?1 WHERE session_key = ?2",
                params![timestamp, session_key],
            )
            .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| SessionStoreError::Backend(e.to_string()))?
    }

    async fn delete_expired(&self, max_age_secs: i64) -> Result<u64, SessionStoreError> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let cutoff = now - max_age_secs;
            let deleted = conn
                .execute(
                    "DELETE FROM sessions WHERE last_active_at < ?1",
                    params![cutoff],
                )
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
            Ok(deleted as u64)
        })
        .await
        .map_err(|e| SessionStoreError::Backend(e.to_string()))?
    }
}

impl ContextStore for SqliteSessionStore {
    fn store_context(
        &self,
        msg: &ContextMessage,
        limit: usize,
    ) -> impl Future<Output = Result<(), SessionStoreError>> + Send {
        let conn = Arc::clone(&self.conn);
        let msg = msg.clone();
        async move {
            if limit == 0 {
                return Ok(());
            }
            let limit = limit as i64;
            tokio::task::spawn_blocking(move || {
                let conn = conn
                    .lock()
                    .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                conn.execute(
                    "INSERT INTO context_messages (group_key, sender, content, timestamp)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![msg.group_key, msg.sender, msg.content, msg.timestamp],
                )
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                conn.execute(
                    "DELETE FROM context_messages
                     WHERE group_key = ?1
                       AND id NOT IN (
                           SELECT id FROM context_messages
                           WHERE group_key = ?1
                           ORDER BY id DESC
                           LIMIT ?2
                       )",
                    params![msg.group_key, limit],
                )
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                Ok(())
            })
            .await
            .map_err(|e| SessionStoreError::Backend(e.to_string()))?
        }
    }

    fn take_context(
        &self,
        group_key: &str,
    ) -> impl Future<Output = Result<Vec<ContextMessage>, SessionStoreError>> + Send {
        let conn = Arc::clone(&self.conn);
        let group_key = group_key.to_string();
        async move {
            tokio::task::spawn_blocking(move || {
                let conn = conn
                    .lock()
                    .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                let mut stmt = conn
                    .prepare(
                        "SELECT group_key, sender, content, timestamp
                         FROM context_messages
                         WHERE group_key = ?1
                         ORDER BY id ASC",
                    )
                    .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                let rows = stmt
                    .query_map(params![group_key], |row| {
                        Ok(ContextMessage {
                            group_key: row.get(0)?,
                            sender: row.get(1)?,
                            content: row.get(2)?,
                            timestamp: row.get(3)?,
                        })
                    })
                    .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                let messages: Result<Vec<ContextMessage>, SessionStoreError> = rows
                    .map(|r| r.map_err(|e| SessionStoreError::Backend(e.to_string())))
                    .collect();
                let messages = messages?;
                conn.execute(
                    "DELETE FROM context_messages WHERE group_key = ?1",
                    params![group_key],
                )
                .map_err(|e| SessionStoreError::Backend(e.to_string()))?;
                Ok(messages)
            })
            .await
            .map_err(|e| SessionStoreError::Backend(e.to_string()))?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn given_an_in_memory_store() -> SqliteSessionStore {
        SqliteSessionStore::open_in_memory().expect("in-memory store should open")
    }

    #[fixture]
    fn given_a_session() -> PersistedSession {
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
    async fn when_open_in_memory_called_then_store_is_created(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert!(sessions.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_upsert_session_called_then_session_is_retrievable(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("upsert should succeed");

        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_key, "key-1");
        assert_eq!(sessions[0].agent_name, "agent-a");
        assert_eq!(sessions[0].acp_session_id, "acp-123");
        assert!(!sessions[0].closed);
    }

    #[rstest]
    #[tokio::test]
    async fn when_upsert_called_twice_then_record_is_updated_not_duplicated(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("first upsert should succeed");

        let mut updated = given_a_session.clone();
        updated.last_active_at = 2_000_000;
        given_an_in_memory_store
            .upsert_session(&updated)
            .await
            .expect("second upsert should succeed");

        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].last_active_at, 2_000_000);
    }

    #[rstest]
    #[tokio::test]
    async fn when_mark_closed_called_then_session_excluded_from_open_sessions(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("upsert should succeed");
        given_an_in_memory_store
            .mark_closed("key-1")
            .await
            .expect("mark_closed should succeed");

        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert!(sessions.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_update_last_active_called_then_timestamp_is_updated(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("upsert should succeed");
        given_an_in_memory_store
            .update_last_active("key-1", 9_999_999)
            .await
            .expect("update_last_active should succeed");

        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert_eq!(sessions[0].last_active_at, 9_999_999);
    }

    #[rstest]
    #[tokio::test]
    async fn when_delete_expired_called_then_old_sessions_removed(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let old_session = PersistedSession {
            session_key: "old-key".to_string(),
            agent_name: "agent-b".to_string(),
            acp_session_id: "acp-old".to_string(),
            created_at: 1,
            last_active_at: 1,
            closed: false,
        };
        given_an_in_memory_store
            .upsert_session(&old_session)
            .await
            .expect("upsert should succeed");

        let deleted = given_an_in_memory_store
            .delete_expired(1)
            .await
            .expect("delete_expired should succeed");
        assert_eq!(deleted, 1);

        let sessions = given_an_in_memory_store
            .load_open_sessions()
            .await
            .expect("load should succeed");
        assert!(sessions.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_delete_expired_called_with_recent_sessions_then_nothing_deleted(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("upsert should succeed");

        let deleted = given_an_in_memory_store
            .delete_expired(100 * 365 * 24 * 3600)
            .await
            .expect("delete_expired should succeed");
        assert_eq!(deleted, 0);
    }

    #[rstest]
    #[tokio::test]
    async fn when_mark_closed_on_nonexistent_key_then_returns_ok(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let result = given_an_in_memory_store.mark_closed("nonexistent").await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn when_sqlite_store_cloned_then_shares_same_connection(
        given_an_in_memory_store: SqliteSessionStore,
        given_a_session: PersistedSession,
    ) {
        let cloned_store = given_an_in_memory_store.clone();
        given_an_in_memory_store
            .upsert_session(&given_a_session)
            .await
            .expect("upsert should succeed");

        let sessions = cloned_store
            .load_open_sessions()
            .await
            .expect("load via clone should succeed");
        assert_eq!(sessions.len(), 1);
    }

    #[rstest]
    #[tokio::test]
    async fn when_store_context_called_then_take_context_returns_messages(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let msg = ContextMessage {
            group_key: "telegram:-100123456".to_string(),
            sender: "alice".to_string(),
            content: "hello".to_string(),
            timestamp: 1_000_000,
        };
        ContextStore::store_context(&given_an_in_memory_store, &msg, 50)
            .await
            .expect("store_context should succeed");

        let messages = ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("take_context should succeed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "alice");
        assert_eq!(messages[0].content, "hello");
    }

    #[rstest]
    #[tokio::test]
    async fn when_take_context_called_then_messages_are_deleted(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let msg = ContextMessage {
            group_key: "telegram:-100123456".to_string(),
            sender: "bob".to_string(),
            content: "hi".to_string(),
            timestamp: 1_000_001,
        };
        ContextStore::store_context(&given_an_in_memory_store, &msg, 50)
            .await
            .expect("store_context should succeed");
        ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("first take should succeed");

        let second = ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("second take should succeed");
        assert!(second.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_store_context_exceeds_limit_then_oldest_trimmed(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        for i in 0..5i64 {
            let msg = ContextMessage {
                group_key: "telegram:-100123456".to_string(),
                sender: format!("user{i}"),
                content: format!("msg{i}"),
                timestamp: 1_000_000 + i,
            };
            ContextStore::store_context(&given_an_in_memory_store, &msg, 3)
                .await
                .expect("store_context should succeed");
        }

        let messages = ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("take_context should succeed");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "msg2");
        assert_eq!(messages[2].content, "msg4");
    }

    #[rstest]
    #[tokio::test]
    async fn when_store_context_with_limit_zero_then_not_stored(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let msg = ContextMessage {
            group_key: "telegram:-100123456".to_string(),
            sender: "carol".to_string(),
            content: "should not store".to_string(),
            timestamp: 1_000_002,
        };
        ContextStore::store_context(&given_an_in_memory_store, &msg, 0)
            .await
            .expect("store_context with limit 0 should succeed");

        let messages = ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("take_context should succeed");
        assert!(messages.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_take_context_on_empty_group_then_returns_empty_vec(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        let result =
            ContextStore::take_context(&given_an_in_memory_store, "telegram:-100000000").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn when_context_messages_stored_then_returned_in_insertion_order(
        given_an_in_memory_store: SqliteSessionStore,
    ) {
        for i in 0..3i64 {
            let msg = ContextMessage {
                group_key: "telegram:-100123456".to_string(),
                sender: format!("user{i}"),
                content: format!("msg{i}"),
                timestamp: 1_000_000 + i,
            };
            ContextStore::store_context(&given_an_in_memory_store, &msg, 50)
                .await
                .expect("store_context should succeed");
        }

        let messages = ContextStore::take_context(&given_an_in_memory_store, "telegram:-100123456")
            .await
            .expect("take_context should succeed");
        assert_eq!(messages[0].content, "msg0");
        assert_eq!(messages[1].content, "msg1");
        assert_eq!(messages[2].content, "msg2");
    }
}
