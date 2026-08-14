//! Crash-safe `SQLite` persistence for versioned events and rebuildable projections.

use std::{collections::HashMap, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use mcg_domain::{TaskEnvelope, TaskId, TaskState};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;
use uuid::Uuid;

type EventRow = (i64, String, i64, String, Option<String>, String, String);

const MIGRATION_1: &str = r"
CREATE TABLE IF NOT EXISTS events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL,
    occurred_at TEXT NOT NULL,
    task_id TEXT,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL
);
";

const MIGRATION_2: &str = r"
CREATE INDEX IF NOT EXISTS events_task_sequence ON events(task_id, sequence);
CREATE TABLE IF NOT EXISTS tasks (
    task_id TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL,
    state TEXT NOT NULL,
    task_json TEXT NOT NULL,
    last_event_sequence INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("stored data is invalid: {0}")]
    InvalidData(String),
    #[error("task {0} was not found")]
    TaskNotFound(TaskId),
    #[error(transparent)]
    InvalidTransition(#[from] mcg_domain::InvalidTaskTransition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryReport {
    pub events_replayed: u64,
    pub tasks_rebuilt: u64,
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
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        Self::configure(&connection, true)?;
        let mut store = Self { connection };
        store.apply_migrations()?;
        Ok(store)
    }

    /// Opens an isolated in-memory store and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the in-memory database cannot be initialized.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        Self::configure(&connection, false)?;
        let mut store = Self { connection };
        store.apply_migrations()?;
        Ok(store)
    }

    fn configure(connection: &Connection, file_backed: bool) -> Result<(), StorageError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")?;
        if file_backed {
            connection.execute_batch("PRAGMA journal_mode = WAL;")?;
        }
        Ok(())
    }

    fn apply_migrations(&mut self) -> Result<(), StorageError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS mcg_schema (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )?;
        for (version, sql) in [(1_i64, MIGRATION_1), (2_i64, MIGRATION_2)] {
            let applied = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM mcg_schema WHERE version = ?1)",
                [version],
                |row| row.get::<_, bool>(0),
            )?;
            if !applied {
                let transaction = self.connection.transaction()?;
                transaction.execute_batch(sql)?;
                transaction.execute("INSERT INTO mcg_schema(version) VALUES (?1)", [version])?;
                transaction.commit()?;
            }
        }
        Ok(())
    }

    /// Atomically appends `task_created` and writes its task projection.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] without committing either write when any write fails.
    pub fn create_task(&mut self, task: &TaskEnvelope) -> Result<EventEnvelope, StorageError> {
        let payload = serde_json::to_value(TaskCreatedPayload { task: task.clone() })?;
        let event = EventEnvelope::new(EventKind::TaskCreated, Some(task.id), payload);
        let transaction = self.connection.transaction()?;
        let event = Self::insert_event(&transaction, &event)?;
        Self::upsert_task(&transaction, task, event.sequence)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically applies a legal task state transition and appends its event.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown tasks, illegal transitions, or failed persistence.
    pub fn transition_task(
        &mut self,
        task_id: TaskId,
        next: TaskState,
    ) -> Result<(TaskEnvelope, EventEnvelope), StorageError> {
        let mut task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let previous = task.state;
        task.transition_to(next)?;
        let payload = serde_json::to_value(TaskStateChangedPayload {
            from: previous,
            to: next,
        })?;
        let event = EventEnvelope::new(EventKind::TaskStateChanged, Some(task_id), payload);
        let transaction = self.connection.transaction()?;
        let event = Self::insert_event(&transaction, &event)?;
        Self::upsert_task(&transaction, &task, event.sequence)?;
        transaction.commit()?;
        Ok((task, event))
    }

    fn insert_event(
        transaction: &Transaction<'_>,
        event: &EventEnvelope,
    ) -> Result<EventEnvelope, StorageError> {
        transaction.execute(
            "INSERT INTO events(event_id, schema_version, occurred_at, task_id, kind, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id.to_string(),
                event.schema_version,
                event.occurred_at.to_rfc3339(),
                event.task_id.map(|id| id.to_string()),
                event.kind.as_str(),
                serde_json::to_string(&event.payload)?,
            ],
        )?;
        let sequence = transaction.last_insert_rowid().try_into().map_err(|_| {
            StorageError::InvalidData("negative event sequence returned by SQLite".to_owned())
        })?;
        Ok(event.clone().with_sequence(sequence))
    }

    fn upsert_task(
        transaction: &Transaction<'_>,
        task: &TaskEnvelope,
        sequence: u64,
    ) -> Result<(), StorageError> {
        transaction.execute(
            "INSERT INTO tasks(
                task_id, schema_version, state, task_json, last_event_sequence, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(task_id) DO UPDATE SET
                schema_version = excluded.schema_version,
                state = excluded.state,
                task_json = excluded.task_json,
                last_event_sequence = excluded.last_event_sequence,
                updated_at = excluded.updated_at",
            params![
                task.id.to_string(),
                task.schema_version,
                task.state.to_string(),
                serde_json::to_string(task)?,
                i64::try_from(sequence).map_err(|_| StorageError::InvalidData(
                    "event sequence exceeds SQLite range".to_owned()
                ))?,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Loads the current task projection.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when stored JSON or the query is invalid.
    pub fn load_task(&self, task_id: TaskId) -> Result<Option<TaskEnvelope>, StorageError> {
        let task_json = self
            .connection
            .query_row(
                "SELECT task_json FROM tasks WHERE task_id = ?1",
                [task_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        task_json
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Returns all events for a task in durable sequence order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when an event row cannot be decoded.
    pub fn events_for_task(&self, task_id: TaskId) -> Result<Vec<EventEnvelope>, StorageError> {
        self.read_events(
            "SELECT sequence, event_id, schema_version, occurred_at, task_id, kind, payload
             FROM events WHERE task_id = ?1 ORDER BY sequence",
            Some(task_id.to_string()),
        )
    }

    /// Returns all durable events after a global sequence for stream resumption.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when an event row cannot be decoded.
    pub fn events_after(&self, sequence: u64) -> Result<Vec<EventEnvelope>, StorageError> {
        let sequence = i64::try_from(sequence).map_err(|_| {
            StorageError::InvalidData("event sequence exceeds SQLite range".to_owned())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT sequence, event_id, schema_version, occurred_at, task_id, kind, payload
             FROM events WHERE sequence > ?1 ORDER BY sequence",
        )?;
        let rows = statement
            .query_map([sequence], Self::map_event_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(Self::decode_event).collect()
    }

    fn all_events(&self) -> Result<Vec<EventEnvelope>, StorageError> {
        self.read_events(
            "SELECT sequence, event_id, schema_version, occurred_at, task_id, kind, payload
             FROM events ORDER BY sequence",
            None,
        )
    }

    fn read_events(
        &self,
        sql: &str,
        task_id: Option<String>,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(task_id) = task_id {
            statement
                .query_map([task_id], Self::map_event_row)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map([], Self::map_event_row)?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter().map(Self::decode_event).collect()
    }

    fn map_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
        ))
    }

    fn decode_event(row: EventRow) -> Result<EventEnvelope, StorageError> {
        let (sequence, event_id, schema_version, occurred_at, task_id, kind, payload) = row;
        Ok(EventEnvelope {
            id: Uuid::parse_str(&event_id)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?,
            schema_version: u16::try_from(schema_version).map_err(|_| {
                StorageError::InvalidData("event schema version exceeds u16".to_owned())
            })?,
            sequence: u64::try_from(sequence)
                .map_err(|_| StorageError::InvalidData("negative event sequence".to_owned()))?,
            occurred_at: DateTime::parse_from_rfc3339(&occurred_at)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?
                .with_timezone(&Utc),
            task_id: task_id
                .map(|value| {
                    value
                        .parse()
                        .map_err(|error: uuid::Error| StorageError::InvalidData(error.to_string()))
                })
                .transpose()?,
            kind: EventKind::parse(&kind).map_err(StorageError::InvalidData)?,
            payload: serde_json::from_str(&payload)?,
        })
    }

    /// Rebuilds task projections from the authoritative event log.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] and leaves prior projections intact if replay fails.
    pub fn recover_projections(&mut self) -> Result<RecoveryReport, StorageError> {
        let events = self.all_events()?;
        let mut tasks: HashMap<TaskId, (TaskEnvelope, u64)> = HashMap::new();
        for event in &events {
            match event.kind {
                EventKind::TaskCreated => {
                    let payload: TaskCreatedPayload =
                        serde_json::from_value(event.payload.clone())?;
                    tasks.insert(payload.task.id, (payload.task, event.sequence));
                }
                EventKind::TaskStateChanged => {
                    if let Some(task_id) = event.task_id {
                        let payload: TaskStateChangedPayload =
                            serde_json::from_value(event.payload.clone())?;
                        let (task, sequence) = tasks.get_mut(&task_id).ok_or_else(|| {
                            StorageError::InvalidData(format!(
                                "state event {} precedes task creation",
                                event.sequence
                            ))
                        })?;
                        if task.state != payload.from || !task.state.can_transition_to(payload.to) {
                            return Err(StorageError::InvalidData(format!(
                                "invalid replay transition at event {}",
                                event.sequence
                            )));
                        }
                        task.state = payload.to;
                        task.updated_at = event.occurred_at;
                        *sequence = event.sequence;
                    }
                }
                _ => {}
            }
        }

        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM tasks", [])?;
        for (task, sequence) in tasks.values() {
            Self::upsert_task(&transaction, task, *sequence)?;
        }
        transaction.commit()?;
        Ok(RecoveryReport {
            events_replayed: events.len().try_into().unwrap_or(u64::MAX),
            tasks_rebuilt: tasks.len().try_into().unwrap_or(u64::MAX),
        })
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
        u64::try_from(count)
            .map_err(|_| StorageError::InvalidData("negative event count".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_and_event_survive_reopen_with_consistent_sequence() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        let task = TaskEnvelope::new("survive restart");

        {
            let mut store = SqliteEventStore::open(&path).expect("store should open");
            let event = store.create_task(&task).expect("task should persist");
            assert_eq!(event.sequence, 1);
        }

        let mut reopened = SqliteEventStore::open(&path).expect("store should reopen");
        let report = reopened
            .recover_projections()
            .expect("recovery should replay");
        let recovered = reopened
            .load_task(task.id)
            .expect("projection should read")
            .expect("task should exist");
        let events = reopened
            .events_for_task(task.id)
            .expect("events should read");

        assert_eq!(report.tasks_rebuilt, 1);
        assert_eq!(recovered, task);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id, Some(task.id));
    }

    #[test]
    fn recovery_repairs_a_damaged_projection_from_events() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let task = TaskEnvelope::new("repair projection");
        store.create_task(&task).expect("task should persist");
        store
            .connection
            .execute("DELETE FROM tasks", [])
            .expect("test should damage projection");

        assert!(
            store
                .load_task(task.id)
                .expect("query should work")
                .is_none()
        );
        store.recover_projections().expect("recovery should repair");
        assert_eq!(
            store.load_task(task.id).expect("query should work"),
            Some(task)
        );
    }

    #[test]
    fn transitions_update_event_and_projection_atomically() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let task = TaskEnvelope::new("transition");
        store.create_task(&task).expect("task should persist");
        let (active, event) = store
            .transition_task(task.id, TaskState::Active)
            .expect("transition should persist");

        assert_eq!(active.state, TaskState::Active);
        assert_eq!(event.sequence, 2);
        assert_eq!(store.event_count().expect("count should read"), 2);
        assert_eq!(
            store.load_task(task.id).expect("query should work"),
            Some(active)
        );
    }

    #[test]
    fn projection_failure_rolls_back_the_event_append() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_task_projection
                 BEFORE INSERT ON tasks
                 BEGIN
                    SELECT RAISE(FAIL, 'injected projection failure');
                 END;",
            )
            .expect("failure trigger should install");

        let result = store.create_task(&TaskEnvelope::new("must be atomic"));

        assert!(result.is_err());
        assert_eq!(store.event_count().expect("count should read"), 0);
    }
}
