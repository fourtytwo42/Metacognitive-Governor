//! `SQLite` persistence for versioned MCG events and projections.

use mcg_events::EventEnvelope;
use rusqlite::{Connection, params};
use thiserror::Error;

const INITIAL_MIGRATION: &str = r"
CREATE TABLE IF NOT EXISTS mcg_schema (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL,
    occurred_at TEXT NOT NULL,
    task_id TEXT,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL
);
INSERT OR IGNORE INTO mcg_schema(version) VALUES (1);
";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub struct SqliteEventStore {
    connection: Connection,
}

impl SqliteEventStore {
    /// Opens or creates a file-backed event store and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the database cannot be opened or migrated.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        connection.execute_batch(INITIAL_MIGRATION)?;
        Ok(Self { connection })
    }

    /// Opens an isolated in-memory store and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the in-memory database cannot be initialized.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(INITIAL_MIGRATION)?;
        Ok(Self { connection })
    }

    /// Appends an event and returns its durable sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when serialization or insertion fails.
    pub fn append(&self, event: &EventEnvelope) -> Result<u64, StorageError> {
        self.connection.execute(
            "INSERT INTO events(event_id, schema_version, occurred_at, task_id, kind, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id.to_string(),
                event.schema_version,
                event.occurred_at.to_rfc3339(),
                event.task_id.map(|id| id.to_string()),
                format!("{:?}", event.kind),
                serde_json::to_string(&event.payload)?,
            ],
        )?;

        Ok(self.connection.last_insert_rowid().try_into().unwrap_or(0))
    }

    /// Returns the number of stored events.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the count query fails.
    pub fn event_count(&self) -> Result<u64, StorageError> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count.try_into().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use mcg_events::{EventEnvelope, EventKind};
    use serde_json::json;

    use super::*;

    #[test]
    fn migration_and_append_create_durable_event_sequence() {
        let store = SqliteEventStore::open_in_memory().expect("store should open");
        let event = EventEnvelope::new(EventKind::TaskCreated, None, json!({"goal": "test"}));

        let sequence = store.append(&event).expect("event should append");

        assert_eq!(sequence, 1);
        assert_eq!(store.event_count().expect("count should read"), 1);
    }
}
