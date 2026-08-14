//! Crash-safe `SQLite` persistence for versioned events and rebuildable projections.

use std::{collections::HashMap, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use mcg_domain::{
    ApprovalId, LeaseId, TaskEnvelope, TaskId, TaskState, ToolExecutionId, ToolRequestId,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use mcg_tools::{
    ApprovalRequest, ApprovalResolution, ApprovalState, ExecutionLease, LeaseState, ToolAuditRecord,
};
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

const MIGRATION_3: &str = r"
CREATE TABLE IF NOT EXISTS approvals (
    approval_id TEXT PRIMARY KEY,
    tool_request_id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL,
    state TEXT NOT NULL,
    approval_json TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS approvals_task_state ON approvals(task_id, state);

CREATE TABLE IF NOT EXISTS execution_leases (
    lease_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL UNIQUE,
    tool_request_id TEXT NOT NULL,
    state TEXT NOT NULL,
    lease_json TEXT NOT NULL,
    acquired_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS execution_leases_request_state
    ON execution_leases(tool_request_id, state);

CREATE TABLE IF NOT EXISTS tool_audit (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    tool_request_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    phase TEXT NOT NULL,
    audit_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    UNIQUE(execution_id, phase)
);
CREATE INDEX IF NOT EXISTS tool_audit_task_sequence ON tool_audit(task_id, sequence);

CREATE TABLE IF NOT EXISTS artifacts (
    artifact_id TEXT PRIMARY KEY,
    sha256 TEXT NOT NULL,
    media_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS artifacts_sha256 ON artifacts(sha256);

CREATE TRIGGER IF NOT EXISTS tool_audit_no_update
BEFORE UPDATE ON tool_audit BEGIN
    SELECT RAISE(FAIL, 'tool audit records are immutable');
END;
CREATE TRIGGER IF NOT EXISTS tool_audit_no_delete
BEFORE DELETE ON tool_audit BEGIN
    SELECT RAISE(FAIL, 'tool audit records are immutable');
END;
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
    #[error("storage conflict: {0}")]
    Conflict(String),
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
        for (version, sql) in [
            (1_i64, MIGRATION_1),
            (2_i64, MIGRATION_2),
            (3_i64, MIGRATION_3),
        ] {
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

    /// Persists a pending approval and its event atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for non-pending input, duplicate request identity, or failed persistence.
    pub fn create_approval(
        &mut self,
        approval: &ApprovalRequest,
    ) -> Result<EventEnvelope, StorageError> {
        if approval.state != ApprovalState::Pending {
            return Err(StorageError::InvalidData(
                "new approvals must be pending".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::ApprovalRequested,
            Some(approval.task_id),
            serde_json::to_value(approval)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO approvals(
                approval_id, tool_request_id, task_id, state, approval_json,
                requested_at, expires_at, resolved_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                approval.id.to_string(),
                approval.tool_request_id.to_string(),
                approval.task_id.to_string(),
                approval_state_name(approval.state),
                serde_json::to_string(approval)?,
                approval.requested_at.to_rfc3339(),
                approval.expires_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads an approval by tool request, expiring it durably when necessary.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or expiration cannot be committed.
    pub fn approval_for_request(
        &mut self,
        request_id: ToolRequestId,
    ) -> Result<Option<ApprovalRequest>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT approval_json FROM approvals WHERE tool_request_id = ?1",
                [request_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(json) = json else {
            return Ok(None);
        };
        let mut approval: ApprovalRequest = serde_json::from_str(&json)?;
        if approval.state == ApprovalState::Pending && approval.expires_at <= Utc::now() {
            approval.state = ApprovalState::Expired;
            let event = EventEnvelope::new(
                EventKind::ApprovalResolved,
                Some(approval.task_id),
                serde_json::to_value(&approval)?,
            );
            let transaction = self.connection.transaction()?;
            transaction.execute(
                "UPDATE approvals SET state = ?1, approval_json = ?2, resolved_at = ?3
                 WHERE approval_id = ?4 AND state = 'pending'",
                params![
                    approval_state_name(approval.state),
                    serde_json::to_string(&approval)?,
                    Utc::now().to_rfc3339(),
                    approval.id.to_string(),
                ],
            )?;
            Self::insert_event(&transaction, &event)?;
            transaction.commit()?;
        }
        Ok(Some(approval))
    }

    /// Loads one approval by its durable identity.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn approval(
        &self,
        approval_id: ApprovalId,
    ) -> Result<Option<ApprovalRequest>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT approval_json FROM approvals WHERE approval_id = ?1",
                [approval_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Lists approvals in deterministic request order, optionally filtered by state.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn approvals(
        &self,
        state: Option<ApprovalState>,
    ) -> Result<Vec<ApprovalRequest>, StorageError> {
        let (sql, parameter) = state.map_or_else(
            || {
                (
                    "SELECT approval_json FROM approvals ORDER BY requested_at, approval_id",
                    None,
                )
            },
            |state| {
                (
                    "SELECT approval_json FROM approvals WHERE state = ?1
                     ORDER BY requested_at, approval_id",
                    Some(approval_state_name(state).to_owned()),
                )
            },
        );
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(parameter) = parameter {
            statement
                .query_map([parameter], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    /// Resolves one pending, unexpired approval and appends the resolution event atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the approval is absent, terminal, expired, or the resolution is invalid.
    pub fn resolve_approval(
        &mut self,
        resolution: &ApprovalResolution,
    ) -> Result<(ApprovalRequest, EventEnvelope), StorageError> {
        if !matches!(
            resolution.state,
            ApprovalState::Approved | ApprovalState::Denied | ApprovalState::Cancelled
        ) {
            return Err(StorageError::InvalidData(
                "approval resolution must be approved, denied, or cancelled".into(),
            ));
        }
        let json = self
            .connection
            .query_row(
                "SELECT approval_json FROM approvals WHERE approval_id = ?1",
                [resolution.approval_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("approval was not found".into()))?;
        let mut approval: ApprovalRequest = serde_json::from_str(&json)?;
        if approval.state != ApprovalState::Pending || approval.expires_at <= Utc::now() {
            return Err(StorageError::Conflict(
                "approval is no longer pending and unexpired".into(),
            ));
        }
        approval.state = resolution.state;
        let event = EventEnvelope::new(
            EventKind::ApprovalResolved,
            Some(approval.task_id),
            serde_json::json!({"approval": approval, "resolution": resolution}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE approvals SET state = ?1, approval_json = ?2, resolved_at = ?3
             WHERE approval_id = ?4 AND state = 'pending'",
            params![
                approval_state_name(approval.state),
                serde_json::to_string(&approval)?,
                resolution.resolved_at.to_rfc3339(),
                approval.id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "approval was resolved concurrently".into(),
            ));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((approval, event))
    }

    /// Appends an immutable tool audit record, artifact references, and matching event atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate phases, invalid sizes, or failed persistence.
    pub fn record_tool_audit(
        &mut self,
        audit: &ToolAuditRecord,
    ) -> Result<EventEnvelope, StorageError> {
        let event_kind = match audit.phase {
            mcg_tools::ToolAuditPhase::Decision => EventKind::ToolRequested,
            mcg_tools::ToolAuditPhase::Completion => EventKind::ToolCompleted,
        };
        let event = EventEnvelope::new(
            event_kind,
            Some(audit.task_id),
            serde_json::to_value(audit)?,
        );
        let transaction = self.connection.transaction()?;
        for artifact in &audit.artifacts {
            transaction.execute(
                "INSERT OR IGNORE INTO artifacts(
                    artifact_id, sha256, media_type, size_bytes, relative_path, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    artifact.id.to_string(),
                    artifact.sha256,
                    artifact.media_type,
                    i64::try_from(artifact.size_bytes).map_err(|_| {
                        StorageError::InvalidData("artifact size exceeds SQLite range".into())
                    })?,
                    artifact.relative_path,
                    audit.recorded_at.to_rfc3339(),
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO tool_audit(
                execution_id, tool_request_id, task_id, phase, audit_json, recorded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                audit.execution_id.to_string(),
                audit.request_id.to_string(),
                audit.task_id.to_string(),
                tool_audit_phase_name(audit.phase),
                serde_json::to_string(audit)?,
                audit.recorded_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads immutable tool audit history for a task.
    ///
    /// # Errors
    ///
    /// Returns an error when rows or JSON are invalid.
    pub fn tool_audit_for_task(
        &self,
        task_id: TaskId,
    ) -> Result<Vec<ToolAuditRecord>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT audit_json FROM tool_audit WHERE task_id = ?1 ORDER BY sequence")?;
        let rows = statement
            .query_map([task_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .collect()
    }

    /// Acquires a durable, exclusive execution lease for a tool request.
    ///
    /// # Errors
    ///
    /// Returns a conflict when an unexpired active lease already exists.
    pub fn acquire_tool_lease(
        &mut self,
        request_id: ToolRequestId,
        execution_id: ToolExecutionId,
        duration: chrono::Duration,
    ) -> Result<ExecutionLease, StorageError> {
        if duration <= chrono::Duration::zero() {
            return Err(StorageError::InvalidData(
                "lease duration must be positive".into(),
            ));
        }
        let now = Utc::now();
        let transaction = self.connection.transaction()?;
        let active: Option<String> = transaction
            .query_row(
                "SELECT lease_json FROM execution_leases
                 WHERE tool_request_id = ?1 AND state = 'active'
                 ORDER BY acquired_at DESC LIMIT 1",
                [request_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(json) = active {
            let mut prior: ExecutionLease = serde_json::from_str(&json)?;
            if prior.expires_at > now {
                return Err(StorageError::Conflict(
                    "tool request already has an active lease".into(),
                ));
            }
            prior.state = LeaseState::Expired;
            transaction.execute(
                "UPDATE execution_leases SET state = 'expired', lease_json = ?1
                 WHERE lease_id = ?2 AND state = 'active'",
                params![serde_json::to_string(&prior)?, prior.id.to_string()],
            )?;
        }
        let lease = ExecutionLease {
            schema_version: mcg_tools::TOOL_SCHEMA_VERSION,
            id: LeaseId::new(),
            execution_id,
            tool_request_id: request_id,
            state: LeaseState::Active,
            acquired_at: now,
            expires_at: now + duration,
            heartbeat_at: now,
        };
        transaction.execute(
            "INSERT INTO execution_leases(
                lease_id, execution_id, tool_request_id, state, lease_json,
                acquired_at, expires_at, heartbeat_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                lease.id.to_string(),
                lease.execution_id.to_string(),
                lease.tool_request_id.to_string(),
                lease_state_name(lease.state),
                serde_json::to_string(&lease)?,
                lease.acquired_at.to_rfc3339(),
                lease.expires_at.to_rfc3339(),
                lease.heartbeat_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(lease)
    }

    /// Finishes one active durable execution lease.
    ///
    /// # Errors
    ///
    /// Returns a conflict when the lease is absent, expired, or terminal.
    pub fn finish_tool_lease(
        &mut self,
        lease_id: LeaseId,
        state: LeaseState,
    ) -> Result<(), StorageError> {
        if state == LeaseState::Active {
            return Err(StorageError::InvalidData(
                "lease finish requires a terminal state".into(),
            ));
        }
        let json = self
            .connection
            .query_row(
                "SELECT lease_json FROM execution_leases WHERE lease_id = ?1",
                [lease_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("lease was not found".into()))?;
        let mut lease: ExecutionLease = serde_json::from_str(&json)?;
        if lease.state != LeaseState::Active || lease.expires_at <= Utc::now() {
            return Err(StorageError::Conflict("lease is not active".into()));
        }
        lease.state = state;
        lease.heartbeat_at = Utc::now();
        let changed = self.connection.execute(
            "UPDATE execution_leases SET state = ?1, lease_json = ?2, heartbeat_at = ?3
             WHERE lease_id = ?4 AND state = 'active'",
            params![
                lease_state_name(state),
                serde_json::to_string(&lease)?,
                lease.heartbeat_at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "lease was finished concurrently".into(),
            ));
        }
        Ok(())
    }

    /// Expires all active tool leases during daemon recovery.
    ///
    /// Host processes do not survive daemon ownership loss, so even a future
    /// wall-clock expiry cannot keep a recovered lease active.
    ///
    /// # Errors
    ///
    /// Returns an error when lease JSON is invalid or recovery cannot commit.
    pub fn recover_tool_leases(&mut self) -> Result<u64, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT lease_json FROM execution_leases WHERE state = 'active'")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut leases = rows
            .into_iter()
            .map(|json| serde_json::from_str::<ExecutionLease>(&json))
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = self.connection.transaction()?;
        for lease in &mut leases {
            lease.state = LeaseState::Expired;
            lease.heartbeat_at = Utc::now();
            transaction.execute(
                "UPDATE execution_leases SET state = 'expired', lease_json = ?1,
                    heartbeat_at = ?2 WHERE lease_id = ?3 AND state = 'active'",
                params![
                    serde_json::to_string(lease)?,
                    lease.heartbeat_at.to_rfc3339(),
                    lease.id.to_string(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(leases.len().try_into().unwrap_or(u64::MAX))
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

const fn approval_state_name(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Pending => "pending",
        ApprovalState::Approved => "approved",
        ApprovalState::Denied => "denied",
        ApprovalState::Expired => "expired",
        ApprovalState::Cancelled => "cancelled",
    }
}

const fn lease_state_name(state: LeaseState) -> &'static str {
    match state {
        LeaseState::Active => "active",
        LeaseState::Completed => "completed",
        LeaseState::Failed => "failed",
        LeaseState::Expired => "expired",
        LeaseState::Cancelled => "cancelled",
    }
}

const fn tool_audit_phase_name(phase: mcg_tools::ToolAuditPhase) -> &'static str {
    match phase {
        mcg_tools::ToolAuditPhase::Decision => "decision",
        mcg_tools::ToolAuditPhase::Completion => "completion",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{ApprovalId, ArtifactId};
    use mcg_tools::{
        ArtifactReference, TOOL_SCHEMA_VERSION, ToolAuditPhase, ToolOutcomeStatus,
        VerificationEvidence,
    };

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

    #[test]
    fn approval_survives_reopen_and_resolution_is_evented() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        let task_id = TaskId::new();
        let request_id = ToolRequestId::new();
        let approval = ApprovalRequest {
            schema_version: TOOL_SCHEMA_VERSION,
            id: ApprovalId::new(),
            tool_request_id: request_id,
            task_id,
            action: "fs.delete".into(),
            resource: "project/generated".into(),
            reason: "destructive action".into(),
            state: ApprovalState::Pending,
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        {
            let mut store = SqliteEventStore::open(&path).expect("store");
            store.create_approval(&approval).expect("create approval");
        }
        let mut reopened = SqliteEventStore::open(&path).expect("reopen");
        assert_eq!(
            reopened
                .approval_for_request(request_id)
                .expect("read approval"),
            Some(approval.clone())
        );
        let resolution = ApprovalResolution {
            approval_id: approval.id,
            state: ApprovalState::Approved,
            resolved_by: "owner".into(),
            reason: None,
            resolved_at: Utc::now(),
        };
        let (resolved, event) = reopened
            .resolve_approval(&resolution)
            .expect("resolve approval");
        assert_eq!(resolved.state, ApprovalState::Approved);
        assert_eq!(event.kind, EventKind::ApprovalResolved);
        assert_eq!(reopened.event_count().expect("event count"), 2);
    }

    #[test]
    fn tool_audit_is_immutable_and_registers_artifact_with_event() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let task_id = TaskId::new();
        let audit = ToolAuditRecord {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id: ToolExecutionId::new(),
            request_id: ToolRequestId::new(),
            task_id,
            tool: "fs.read".into(),
            resource: "project/file".into(),
            request_fingerprint: "a".repeat(64),
            phase: ToolAuditPhase::Completion,
            decision: "execution completed".into(),
            result: Some(ToolOutcomeStatus::Succeeded),
            artifacts: vec![ArtifactReference {
                id: ArtifactId::new(),
                sha256: "b".repeat(64),
                media_type: "text/plain".into(),
                size_bytes: 7,
                relative_path: "bb/hash".into(),
            }],
            verification: vec![VerificationEvidence {
                method: "sha256".into(),
                passed: true,
                detail: "b".repeat(64),
            }],
            recorded_at: Utc::now(),
        };
        let event = store.record_tool_audit(&audit).expect("record audit");
        assert_eq!(event.kind, EventKind::ToolCompleted);
        assert_eq!(
            store.tool_audit_for_task(task_id).expect("audit history"),
            vec![audit]
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM tool_audit", [])
                .is_err()
        );
        let artifacts: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM artifacts", [], |row| row.get(0))
            .expect("artifact count");
        assert_eq!(artifacts, 1);
    }

    #[test]
    fn restart_recovery_expires_active_tool_leases() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        let lease_id;
        {
            let mut store = SqliteEventStore::open(&path).expect("store");
            lease_id = store
                .acquire_tool_lease(
                    ToolRequestId::new(),
                    ToolExecutionId::new(),
                    chrono::Duration::hours(1),
                )
                .expect("lease")
                .id;
        }
        let mut reopened = SqliteEventStore::open(&path).expect("reopen");
        assert_eq!(reopened.recover_tool_leases().expect("recover leases"), 1);
        assert!(
            reopened
                .finish_tool_lease(lease_id, LeaseState::Completed)
                .is_err()
        );
    }
}
