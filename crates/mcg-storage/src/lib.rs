//! Crash-safe `SQLite` persistence for versioned events and rebuildable projections.

use std::{collections::HashMap, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use mcg_domain::{
    ApprovalId, LeaseId, MemoryClaimId, ProjectId, TaskEnvelope, TaskId, TaskState,
    ToolExecutionId, ToolRequestId,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use mcg_projects::{MemoryClaim, MemoryState, ProjectModel};
use mcg_tools::{
    ApprovalRequest, ApprovalResolution, ApprovalState, ExecutionLease, LeaseState, ToolAuditRecord,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;
use uuid::Uuid;

mod agents;
mod mcp;
mod providers;

pub use agents::{AgentRecoveryReport, AgentTurnCommit, AgentTurnDisposition, InterfaceRunReceipt};

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

const MIGRATION_4: &str = r"
CREATE TABLE IF NOT EXISTS projects (
    project_id TEXT PRIMARY KEY,
    root TEXT NOT NULL UNIQUE,
    current_snapshot_id TEXT NOT NULL,
    project_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS project_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX IF NOT EXISTS project_snapshots_history
    ON project_snapshots(project_id, observed_at, snapshot_id);
CREATE TRIGGER IF NOT EXISTS project_snapshots_no_update
BEFORE UPDATE ON project_snapshots BEGIN
    SELECT RAISE(FAIL, 'project snapshots are immutable');
END;
CREATE TRIGGER IF NOT EXISTS project_snapshots_no_delete
BEFORE DELETE ON project_snapshots BEGIN
    SELECT RAISE(FAIL, 'project snapshots are immutable');
END;

CREATE TABLE IF NOT EXISTS memory_claims (
    claim_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    namespace TEXT NOT NULL,
    claim_key TEXT NOT NULL,
    state TEXT NOT NULL,
    claim_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    fresh_until TEXT,
    supersedes TEXT UNIQUE,
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(supersedes) REFERENCES memory_claims(claim_id)
);
CREATE INDEX IF NOT EXISTS memory_claims_project_key_history
    ON memory_claims(project_id, namespace, claim_key, observed_at, claim_id);
CREATE UNIQUE INDEX IF NOT EXISTS memory_claims_active_head
    ON memory_claims(project_id, namespace, claim_key)
    WHERE state IN ('new', 'working', 'durable', 'decaying');
CREATE TRIGGER IF NOT EXISTS memory_claims_no_delete
BEFORE DELETE ON memory_claims BEGIN
    SELECT RAISE(FAIL, 'memory claim versions cannot be deleted');
END;
";

const MIGRATION_5: &str = r"
CREATE TABLE IF NOT EXISTS organizations (
    organization_id TEXT PRIMARY KEY,
    current_revision_id TEXT NOT NULL,
    organization_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS organization_revisions (
    revision_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    parent_revision_id TEXT,
    organization_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(organization_id, revision)
);
CREATE INDEX IF NOT EXISTS organization_revision_history
    ON organization_revisions(organization_id, revision);

CREATE TABLE IF NOT EXISTS agent_runs (
    run_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    organization_revision_id TEXT NOT NULL,
    state TEXT NOT NULL,
    run_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
    FOREIGN KEY(organization_revision_id) REFERENCES organization_revisions(revision_id)
);
CREATE INDEX IF NOT EXISTS agent_runs_task_created ON agent_runs(task_id, created_at, run_id);
CREATE TABLE IF NOT EXISTS work_plans (
    run_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    plan_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);

CREATE TABLE IF NOT EXISTS agent_instances (
    instance_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    state TEXT NOT NULL,
    instance_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS agent_instances_run_state ON agent_instances(run_id, state);
CREATE TABLE IF NOT EXISTS agent_leases (
    lease_id TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    state TEXT NOT NULL,
    lease_json TEXT NOT NULL,
    acquired_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    FOREIGN KEY(instance_id) REFERENCES agent_instances(instance_id)
);
CREATE INDEX IF NOT EXISTS agent_leases_instance_state ON agent_leases(instance_id, state);

CREATE TABLE IF NOT EXISTS agent_usage (
    usage_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    usage_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS agent_usage_run_time ON agent_usage(run_id, occurred_at, usage_id);
CREATE TABLE IF NOT EXISTS evaluations (
    evaluation_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    evaluation_json TEXT NOT NULL,
    evaluated_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS evaluations_run_time
    ON evaluations(run_id, evaluated_at, evaluation_id);

CREATE TRIGGER IF NOT EXISTS organization_revisions_no_update
BEFORE UPDATE ON organization_revisions BEGIN
    SELECT RAISE(FAIL, 'organization revisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS organization_revisions_no_delete
BEFORE DELETE ON organization_revisions BEGIN
    SELECT RAISE(FAIL, 'organization revisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS work_plans_no_update
BEFORE UPDATE ON work_plans BEGIN SELECT RAISE(FAIL, 'work plans are immutable'); END;
CREATE TRIGGER IF NOT EXISTS work_plans_no_delete
BEFORE DELETE ON work_plans BEGIN SELECT RAISE(FAIL, 'work plans are immutable'); END;
CREATE TRIGGER IF NOT EXISTS agent_usage_no_update
BEFORE UPDATE ON agent_usage BEGIN SELECT RAISE(FAIL, 'agent usage is immutable'); END;
CREATE TRIGGER IF NOT EXISTS agent_usage_no_delete
BEFORE DELETE ON agent_usage BEGIN SELECT RAISE(FAIL, 'agent usage is immutable'); END;
CREATE TRIGGER IF NOT EXISTS evaluations_no_update
BEFORE UPDATE ON evaluations BEGIN SELECT RAISE(FAIL, 'evaluations are immutable'); END;
CREATE TRIGGER IF NOT EXISTS evaluations_no_delete
BEFORE DELETE ON evaluations BEGIN SELECT RAISE(FAIL, 'evaluations are immutable'); END;
";

const MIGRATION_6: &str = r"
CREATE TABLE IF NOT EXISTS agent_turn_results (
    lease_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    result_json TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(instance_id) REFERENCES agent_instances(instance_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS agent_turn_results_run
    ON agent_turn_results(run_id, completed_at, lease_id);
CREATE TRIGGER IF NOT EXISTS agent_turn_results_no_update
BEFORE UPDATE ON agent_turn_results BEGIN
    SELECT RAISE(FAIL, 'agent turn results are immutable');
END;
CREATE TRIGGER IF NOT EXISTS agent_turn_results_no_delete
BEFORE DELETE ON agent_turn_results BEGIN
    SELECT RAISE(FAIL, 'agent turn results are immutable');
END;
";

const MIGRATION_7: &str = r"
CREATE TABLE IF NOT EXISTS provider_connections (
    connection_id TEXT PRIMARY KEY,
    connection_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS provider_catalogs (
    connection_id TEXT PRIMARY KEY,
    catalog_json TEXT NOT NULL,
    discovered_at TEXT NOT NULL,
    FOREIGN KEY(connection_id) REFERENCES provider_connections(connection_id)
);
";

const MIGRATION_8: &str = r"
CREATE TABLE IF NOT EXISTS interface_run_requests (
    interface TEXT NOT NULL,
    request_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    task_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    PRIMARY KEY(interface, request_key),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id)
);
CREATE TRIGGER IF NOT EXISTS interface_run_requests_no_update
BEFORE UPDATE ON interface_run_requests BEGIN
    SELECT RAISE(FAIL, 'interface run request records are immutable');
END;
CREATE TRIGGER IF NOT EXISTS interface_run_requests_no_delete
BEFORE DELETE ON interface_run_requests BEGIN
    SELECT RAISE(FAIL, 'interface run request records are immutable');
END;
";

const MIGRATION_9: &str = r"
CREATE TABLE IF NOT EXISTS mcp_connections (
    connection_id TEXT PRIMARY KEY,
    connection_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS mcp_tool_catalogs (
    connection_id TEXT PRIMARY KEY,
    catalog_json TEXT NOT NULL,
    discovered_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY(connection_id) REFERENCES mcp_connections(connection_id)
);
";

const MIGRATION_10: &str = r"
CREATE TABLE IF NOT EXISTS task_launches (
    task_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id)
);
CREATE TRIGGER IF NOT EXISTS task_launches_no_update
BEFORE UPDATE ON task_launches BEGIN
    SELECT RAISE(FAIL, 'task launch records are immutable');
END;
CREATE TRIGGER IF NOT EXISTS task_launches_no_delete
BEFORE DELETE ON task_launches BEGIN
    SELECT RAISE(FAIL, 'task launch records are immutable');
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
    #[error("project {0} was not found")]
    ProjectNotFound(ProjectId),
    #[error("memory claim {0} was not found")]
    MemoryClaimNotFound(MemoryClaimId),
    #[error("organization revision {0} was not found")]
    OrganizationRevisionNotFound(mcg_domain::OrganizationRevisionId),
    #[error("agent run {0} was not found")]
    AgentRunNotFound(mcg_domain::RunId),
    #[error("agent instance {0} was not found")]
    AgentInstanceNotFound(mcg_domain::AgentInstanceId),
    #[error("agent lease {0} was not found")]
    AgentLeaseNotFound(mcg_domain::AgentLeaseId),
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
            (4_i64, MIGRATION_4),
            (5_i64, MIGRATION_5),
            (6_i64, MIGRATION_6),
            (7_i64, MIGRATION_7),
            (8_i64, MIGRATION_8),
            (9_i64, MIGRATION_9),
            (10_i64, MIGRATION_10),
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

    /// Lists current task projections in newest-updated order with a stable id
    /// tie-breaker.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when a row cannot be read or decoded.
    pub fn tasks(&self) -> Result<Vec<TaskEnvelope>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT task_json FROM tasks ORDER BY updated_at DESC, task_id ASC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
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

    /// Atomically records an immutable project snapshot, updates the current
    /// project projection, and appends its discovery event.
    ///
    /// # Errors
    ///
    /// A canonical root cannot be reassigned to another project identity.
    pub fn record_project_snapshot(
        &mut self,
        model: &ProjectModel,
    ) -> Result<EventEnvelope, StorageError> {
        let owner = self
            .connection
            .query_row(
                "SELECT project_id FROM projects WHERE root = ?1",
                [&model.root],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if owner.is_some_and(|owner| owner != model.project_id.to_string()) {
            return Err(StorageError::Conflict(
                "project root is already registered to a different project".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::ProjectDiscovered,
            None,
            serde_json::to_value(model)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO projects(project_id, root, current_snapshot_id, project_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project_id) DO UPDATE SET
                root = excluded.root,
                current_snapshot_id = excluded.current_snapshot_id,
                project_json = excluded.project_json,
                updated_at = excluded.updated_at",
            params![
                model.project_id.to_string(),
                model.root,
                model.id.to_string(),
                serde_json::to_string(model)?,
                model.observed_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO project_snapshots(
                snapshot_id, project_id, fingerprint, snapshot_json, observed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                model.id.to_string(),
                model.project_id.to_string(),
                model.fingerprint,
                serde_json::to_string(model)?,
                model.observed_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads the current project snapshot projection.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn project(&self, project_id: ProjectId) -> Result<Option<ProjectModel>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT project_json FROM projects WHERE project_id = ?1",
                [project_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Lists current project projections in stable root order.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn projects(&self) -> Result<Vec<ProjectModel>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT project_json FROM projects ORDER BY root, project_id")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    /// Loads immutable discovery history for one project.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn project_history(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectModel>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT snapshot_json FROM project_snapshots
             WHERE project_id = ?1 ORDER BY observed_at, snapshot_id",
        )?;
        let rows = statement
            .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    /// Atomically records a new claim version and its event.
    ///
    /// # Errors
    ///
    /// New claims must be unlinked `new` versions and their project must exist.
    pub fn create_memory_claim(
        &mut self,
        claim: &MemoryClaim,
    ) -> Result<EventEnvelope, StorageError> {
        if claim.state != MemoryState::New || claim.supersedes.is_some() {
            return Err(StorageError::InvalidData(
                "new memory claims must have new state and no predecessor".into(),
            ));
        }
        if self.project(claim.project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(claim.project_id));
        }
        let event = EventEnvelope::new(
            EventKind::MemoryClaimRecorded,
            None,
            serde_json::to_value(claim)?,
        );
        let transaction = self.connection.transaction()?;
        Self::insert_memory_claim(&transaction, claim)?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically supersedes one active claim with a validated corrected version.
    ///
    /// # Errors
    ///
    /// Returns a conflict for stale heads, mismatched identity/scope, or concurrent correction.
    pub fn correct_memory_claim(
        &mut self,
        replacement: &MemoryClaim,
    ) -> Result<(MemoryClaim, EventEnvelope), StorageError> {
        self.replace_memory_claim(replacement, true)
    }

    /// Atomically records a reinforcement, decay, or retraction successor.
    ///
    /// # Errors
    ///
    /// Returns a conflict for stale heads and invalid data for forged transitions.
    pub fn revise_memory_claim(
        &mut self,
        replacement: &MemoryClaim,
    ) -> Result<(MemoryClaim, EventEnvelope), StorageError> {
        self.replace_memory_claim(replacement, false)
    }

    fn replace_memory_claim(
        &mut self,
        replacement: &MemoryClaim,
        correction: bool,
    ) -> Result<(MemoryClaim, EventEnvelope), StorageError> {
        let previous_id = replacement.supersedes.ok_or_else(|| {
            StorageError::InvalidData("memory revision requires a predecessor".into())
        })?;
        let json = self
            .connection
            .query_row(
                "SELECT claim_json FROM memory_claims WHERE claim_id = ?1",
                [previous_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StorageError::MemoryClaimNotFound(previous_id))?;
        let mut previous: MemoryClaim = serde_json::from_str(&json)?;
        if !matches!(
            previous.state,
            MemoryState::New | MemoryState::Working | MemoryState::Durable | MemoryState::Decaying
        ) {
            return Err(StorageError::Conflict(
                "memory claim is no longer the active version".into(),
            ));
        }
        let common_invalid = replacement.project_id != previous.project_id
            || replacement.namespace != previous.namespace
            || replacement.key != previous.key
            || replacement.schema_version != previous.schema_version;
        let correction_invalid = correction
            && (replacement.correction_count != previous.correction_count.saturating_add(1)
                || !matches!(
                    replacement.state,
                    MemoryState::Working | MemoryState::Durable
                ));
        let lifecycle_invalid = !correction
            && (replacement.value != previous.value
                || replacement.confidence.to_bits() != previous.confidence.to_bits()
                || replacement.fresh_until != previous.fresh_until
                || replacement.correction_count != previous.correction_count
                || !matches!(
                    (replacement.state, replacement.usefulness_count),
                    (MemoryState::Working | MemoryState::Durable, count)
                        if count == previous.usefulness_count.saturating_add(1)
                ) && !matches!(
                    replacement.state,
                    MemoryState::Decaying | MemoryState::Retracted
                ));
        if common_invalid || correction_invalid || lifecycle_invalid {
            return Err(StorageError::InvalidData(
                "memory revision does not preserve its version-chain invariants".into(),
            ));
        }
        if !correction
            && matches!(
                replacement.state,
                MemoryState::Decaying | MemoryState::Retracted
            )
            && replacement.usefulness_count != previous.usefulness_count
        {
            return Err(StorageError::InvalidData(
                "memory lifecycle transition changed usefulness unexpectedly".into(),
            ));
        }
        previous.state = MemoryState::Superseded;
        let event = EventEnvelope::new(
            if correction {
                EventKind::MemoryClaimCorrected
            } else {
                EventKind::MemoryClaimStateChanged
            },
            None,
            serde_json::json!({"previous": previous, "replacement": replacement}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_claims SET state = 'superseded', claim_json = ?1
             WHERE claim_id = ?2
               AND state IN ('new', 'working', 'durable', 'decaying')",
            params![serde_json::to_string(&previous)?, previous.id.to_string()],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "memory claim was corrected concurrently".into(),
            ));
        }
        Self::insert_memory_claim(&transaction, replacement)?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((previous, event))
    }

    fn insert_memory_claim(
        transaction: &Transaction<'_>,
        claim: &MemoryClaim,
    ) -> Result<(), StorageError> {
        transaction.execute(
            "INSERT INTO memory_claims(
                claim_id, project_id, namespace, claim_key, state, claim_json,
                observed_at, fresh_until, supersedes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                claim.id.to_string(),
                claim.project_id.to_string(),
                claim.namespace,
                claim.key,
                memory_state_name(claim.state),
                serde_json::to_string(claim)?,
                claim.observed_at.to_rfc3339(),
                claim.fresh_until.map(|value| value.to_rfc3339()),
                claim.supersedes.map(|value| value.to_string()),
            ],
        )?;
        Ok(())
    }

    /// Loads current claim heads, optionally restricted to a namespace and freshness.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn memory_claims(
        &self,
        project_id: ProjectId,
        namespace: Option<&str>,
        fresh_only: bool,
    ) -> Result<Vec<MemoryClaim>, StorageError> {
        let (sql, namespace) = namespace.map_or_else(
            || {
                (
                    "SELECT claim_json FROM memory_claims
                     WHERE project_id = ?1
                       AND state IN ('new', 'working', 'durable', 'decaying')
                     ORDER BY namespace, claim_key, observed_at",
                    None,
                )
            },
            |namespace| {
                (
                    "SELECT claim_json FROM memory_claims
                     WHERE project_id = ?1 AND namespace = ?2
                       AND state IN ('new', 'working', 'durable', 'decaying')
                     ORDER BY namespace, claim_key, observed_at",
                    Some(namespace),
                )
            },
        );
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(namespace) = namespace {
            statement
                .query_map(params![project_id.to_string(), namespace], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let now = Utc::now();
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .filter(|result: &Result<MemoryClaim, StorageError>| {
                !fresh_only || result.as_ref().is_ok_and(|claim| claim.is_fresh_at(now))
            })
            .collect()
    }

    /// Loads one memory claim version by identity.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn memory_claim(
        &self,
        claim_id: MemoryClaimId,
    ) -> Result<Option<MemoryClaim>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT claim_json FROM memory_claims WHERE claim_id = ?1",
                [claim_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Loads every immutable version for one scoped claim key.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn memory_claim_history(
        &self,
        project_id: ProjectId,
        namespace: &str,
        key: &str,
    ) -> Result<Vec<MemoryClaim>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT claim_json FROM memory_claims
             WHERE project_id = ?1 AND namespace = ?2 AND claim_key = ?3
             ORDER BY observed_at, claim_id",
        )?;
        let rows = statement
            .query_map(params![project_id.to_string(), namespace, key], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
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
        for (task, sequence) in tasks.values() {
            Self::upsert_task(&transaction, task, *sequence)?;
        }
        let projected_task_ids = {
            let mut statement = transaction.prepare("SELECT task_id FROM tasks")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for projected_task_id in projected_task_ids {
            let task_id = projected_task_id.parse::<TaskId>().map_err(|error| {
                StorageError::InvalidData(format!(
                    "invalid task projection identifier {projected_task_id}: {error}"
                ))
            })?;
            if !tasks.contains_key(&task_id) {
                transaction.execute("DELETE FROM tasks WHERE task_id = ?1", [projected_task_id])?;
            }
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

const fn memory_state_name(state: MemoryState) -> &'static str {
    match state {
        MemoryState::New => "new",
        MemoryState::Working => "working",
        MemoryState::Durable => "durable",
        MemoryState::Decaying => "decaying",
        MemoryState::Superseded => "superseded",
        MemoryState::Retracted => "retracted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_domain::{ApprovalId, ArtifactId};
    use mcg_projects::{MemoryLifecycle, ProjectDiscoverer, user_provenance};
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

    #[test]
    fn project_snapshot_history_survives_restart_and_is_immutable() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]").expect("manifest");
        let directory = tempfile::tempdir().expect("database directory");
        let path = directory.path().join("mcg.db");
        let project_id = ProjectId::new();
        let first = ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover");
        {
            let mut store = SqliteEventStore::open(&path).expect("store");
            let event = store
                .record_project_snapshot(&first)
                .expect("record project");
            assert_eq!(event.kind, EventKind::ProjectDiscovered);
        }
        std::fs::write(
            project_root.path().join("Cargo.toml"),
            "[workspace]\nmembers = []",
        )
        .expect("manifest update");
        let second =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("rediscover");
        assert_ne!(first.fingerprint, second.fingerprint);

        let mut reopened = SqliteEventStore::open(&path).expect("reopen");
        reopened
            .record_project_snapshot(&second)
            .expect("record second snapshot");
        assert_eq!(reopened.project(project_id).expect("project"), Some(second));
        assert_eq!(
            reopened
                .project_history(project_id)
                .expect("snapshot history"),
            vec![first, reopened.project(project_id).unwrap().unwrap()]
        );
        assert!(
            reopened
                .connection
                .execute("DELETE FROM project_snapshots", [])
                .is_err()
        );
    }

    #[test]
    fn memory_correction_is_atomic_versioned_and_restart_durable() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("package.json"), "{}").expect("manifest");
        let directory = tempfile::tempdir().expect("database directory");
        let path = directory.path().join("mcg.db");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover");
        let mut original = MemoryLifecycle::create(
            project_id,
            "project",
            "package_manager",
            serde_json::json!("npm"),
            0.7,
            user_provenance("fixture"),
            None,
        )
        .expect("claim");
        {
            let mut store = SqliteEventStore::open(&path).expect("store");
            store
                .record_project_snapshot(&project)
                .expect("record project");
            store.create_memory_claim(&original).expect("record claim");
        }
        let replacement = MemoryLifecycle::correct(
            &mut original,
            serde_json::json!("pnpm"),
            1.0,
            user_provenance("owner correction"),
            None,
        )
        .expect("correction");
        let mut reopened = SqliteEventStore::open(&path).expect("reopen");
        let (superseded, event) = reopened
            .correct_memory_claim(&replacement)
            .expect("persist correction");
        assert_eq!(superseded.state, MemoryState::Superseded);
        assert_eq!(event.kind, EventKind::MemoryClaimCorrected);
        assert!(reopened.correct_memory_claim(&replacement).is_err());
        assert_eq!(
            reopened
                .memory_claims(project_id, Some("project"), true)
                .expect("current claims"),
            vec![replacement.clone()]
        );
        let history = reopened
            .memory_claim_history(project_id, "project", "package_manager")
            .expect("history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].state, MemoryState::Superseded);
        assert_eq!(history[1], replacement);
        assert_eq!(reopened.event_count().expect("events"), 3);
        assert!(
            reopened
                .connection
                .execute("DELETE FROM memory_claims", [])
                .is_err()
        );
    }
}
