//! Crash-safe `SQLite` persistence for versioned events and rebuildable projections.

use std::{collections::HashMap, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use mcg_domain::{
    AgentDelegation, AgentMessage, ApprovalId, BlackboardEntry, CodexThreadBinding, Conversation,
    ConversationActivity, ConversationAgentSession, ConversationId, ConversationMessage,
    ConversationMessageId, ConversationTeamState, ConversationTurn, ConversationTurnId,
    ConversationView, LeaseId, ManagedWorktree, MemoryClaimId, MemoryTransferId, ProjectId,
    ProjectSnapshotId, TaskEnvelope, TaskId, TaskState, ToolExecutionId, ToolRequestId,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use mcg_projects::{
    MEMORY_TRANSFER_SCHEMA_VERSION, MemoryClaim, MemoryState, ProjectMemoryTransfer, ProjectModel,
    ProvenanceKind, memory_claim_sha256,
};
use mcg_tools::{
    ApprovalRequest, ApprovalResolution, ApprovalState, ExecutionLease, LeaseState, ToolAuditRecord,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;
use uuid::Uuid;

mod agents;
mod backup;
mod backup_secrets;
mod experiments;
mod mcp;
mod organization_sharing;
mod platform;
mod plugins;
mod providers;
mod reviewed_topology_plans;
mod scheduler;
mod settings;
mod shadow_workspaces;
mod sites;
mod ssh;
mod ssh_workflows;

pub use backup::{
    BACKUP_BUNDLE_SCHEMA_VERSION, BackupBundleEntry, BackupBundleManifest, BackupBundleSummary,
    DataDirectoryLock, acquire_data_directory_lock, create_backup_bundle,
    create_backup_bundle_from_snapshot, create_encrypted_backup_bundle_from_snapshot,
    decrypt_backup_bundle_secrets, restore_backup_bundle, restore_encrypted_backup_bundle,
    verify_backup_bundle,
};
pub use backup_secrets::{
    BACKUP_SECRET_ENCRYPTION_SCHEMA_VERSION, BACKUP_SECRET_ENTRY, BackupMcpSecret,
    BackupPassphrase, BackupProviderSecret, BackupSecretEncryptionMetadata, BackupSecretSnapshot,
    decrypt_backup_secrets, encrypt_backup_secrets,
};

pub use agents::{
    AgentRecoveryReport, AgentTurnCommit, AgentTurnDisposition, ConsumerEvaluationCheckpoint,
    ConsumerEvaluationCheckpointState, InterfaceRunReceipt,
};
pub use experiments::{
    AutomaticExperimentDisposition, AutomaticExperimentOutcome, AutomaticSupervisionMutation,
    CandidateRevisionMutation, ExperimentMutation,
};
pub use plugins::PluginRecoveryReport;
pub use scheduler::{ScheduleClaim, ScheduleWakeMutation};
pub use shadow_workspaces::ShadowWorkspaceMutation;

type EventRow = (i64, String, i64, String, Option<String>, String, String);
pub const STORAGE_SCHEMA_VERSION: i64 = 38;

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

const MIGRATION_11: &str = r"
CREATE TABLE IF NOT EXISTS consumer_evaluation_checkpoints (
    task_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL,
    checkpoint_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id)
);
CREATE INDEX IF NOT EXISTS consumer_evaluation_checkpoints_state
    ON consumer_evaluation_checkpoints(state, updated_at);
";

const MIGRATION_12: &str = r"
CREATE TABLE IF NOT EXISTS agent_turn_checkpoints (
    lease_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    instance_id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL,
    pending_tool_request_id TEXT,
    checkpoint_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(lease_id) REFERENCES agent_leases(lease_id),
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(instance_id) REFERENCES agent_instances(instance_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS agent_turn_checkpoints_task
    ON agent_turn_checkpoints(task_id, updated_at);
";

const MIGRATION_13: &str = r"
CREATE TABLE IF NOT EXISTS schedules (
    schedule_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    next_run_at TEXT,
    schedule_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS schedules_due
    ON schedules(state, next_run_at, schedule_id);
CREATE TABLE IF NOT EXISTS schedule_occurrences (
    occurrence_id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL,
    task_id TEXT UNIQUE,
    due_at TEXT NOT NULL,
    state TEXT NOT NULL,
    lease_owner TEXT,
    lease_expires_at TEXT,
    occurrence_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(schedule_id, due_at),
    FOREIGN KEY(schedule_id) REFERENCES schedules(schedule_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS schedule_occurrences_claim
    ON schedule_occurrences(state, lease_expires_at, occurrence_id);
CREATE INDEX IF NOT EXISTS schedule_occurrences_history
    ON schedule_occurrences(schedule_id, due_at, occurrence_id);
";

const MIGRATION_14: &str = r"
CREATE TABLE IF NOT EXISTS schedule_wake_requests (
    wake_id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL,
    wake_json TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    FOREIGN KEY(schedule_id) REFERENCES schedules(schedule_id)
);
CREATE INDEX IF NOT EXISTS schedule_wake_history
    ON schedule_wake_requests(schedule_id, requested_at, wake_id);
CREATE TABLE IF NOT EXISTS scheduler_notifications (
    notification_id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL,
    occurrence_id TEXT,
    severity TEXT NOT NULL,
    is_read INTEGER NOT NULL,
    notification_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    read_at TEXT,
    FOREIGN KEY(schedule_id) REFERENCES schedules(schedule_id),
    FOREIGN KEY(occurrence_id) REFERENCES schedule_occurrences(occurrence_id)
);
CREATE INDEX IF NOT EXISTS scheduler_notifications_unread
    ON scheduler_notifications(is_read, created_at, notification_id);
";

const MIGRATION_15: &str = r"
CREATE TABLE IF NOT EXISTS platform_capability_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    snapshot_json TEXT NOT NULL,
    observed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS platform_capability_snapshots_observed
    ON platform_capability_snapshots(observed_at, snapshot_id);
CREATE TRIGGER IF NOT EXISTS platform_capability_snapshots_no_update
BEFORE UPDATE ON platform_capability_snapshots BEGIN
    SELECT RAISE(FAIL, 'platform capability snapshots are immutable');
END;
CREATE TRIGGER IF NOT EXISTS platform_capability_snapshots_no_delete
BEFORE DELETE ON platform_capability_snapshots BEGIN
    SELECT RAISE(FAIL, 'platform capability snapshots are immutable');
END;

CREATE TABLE IF NOT EXISTS platform_sessions (
    session_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    state TEXT NOT NULL,
    owner TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    session_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    closed_at TEXT
);
CREATE INDEX IF NOT EXISTS platform_sessions_state_expiry
    ON platform_sessions(state, expires_at, session_id);

CREATE TABLE IF NOT EXISTS platform_session_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL,
    from_state TEXT,
    to_state TEXT NOT NULL,
    event_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES platform_sessions(session_id)
);
CREATE INDEX IF NOT EXISTS platform_session_events_history
    ON platform_session_events(session_id, sequence);
CREATE TRIGGER IF NOT EXISTS platform_session_events_no_update
BEFORE UPDATE ON platform_session_events BEGIN
    SELECT RAISE(FAIL, 'platform session events are immutable');
END;
CREATE TRIGGER IF NOT EXISTS platform_session_events_no_delete
BEFORE DELETE ON platform_session_events BEGIN
    SELECT RAISE(FAIL, 'platform session events are immutable');
END;
";

const MIGRATION_16: &str = r"
CREATE TABLE IF NOT EXISTS computer_action_batches (
    batch_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    state TEXT NOT NULL,
    batch_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES platform_sessions(session_id)
);
CREATE INDEX IF NOT EXISTS computer_action_batches_session
    ON computer_action_batches(session_id, created_at, batch_id);
CREATE TRIGGER IF NOT EXISTS computer_action_batches_no_delete
BEFORE DELETE ON computer_action_batches BEGIN
    SELECT RAISE(FAIL, 'computer action batch history is immutable');
END;
";

const MIGRATION_17: &str = r"
CREATE TABLE IF NOT EXISTS sites (
    site_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    state TEXT NOT NULL,
    revision INTEGER NOT NULL,
    definition_hash TEXT NOT NULL,
    site_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS sites_project_state
    ON sites(project_id, state, updated_at, site_id);

CREATE TABLE IF NOT EXISTS site_revisions (
    revision_id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    definition_hash TEXT NOT NULL,
    revision_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(site_id, revision),
    FOREIGN KEY(site_id) REFERENCES sites(site_id)
);
CREATE TRIGGER IF NOT EXISTS site_revisions_no_update
BEFORE UPDATE ON site_revisions BEGIN
    SELECT RAISE(FAIL, 'site revisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS site_revisions_no_delete
BEFORE DELETE ON site_revisions BEGIN
    SELECT RAISE(FAIL, 'site revisions are immutable');
END;

CREATE TABLE IF NOT EXISTS site_deployments (
    deployment_id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL,
    state TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    deployment_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(site_id) REFERENCES sites(site_id)
);
CREATE INDEX IF NOT EXISTS site_deployments_history
    ON site_deployments(site_id, created_at, deployment_id);
CREATE TRIGGER IF NOT EXISTS site_deployments_no_delete
BEFORE DELETE ON site_deployments BEGIN
    SELECT RAISE(FAIL, 'site deployment history is immutable');
END;

CREATE TABLE IF NOT EXISTS site_health_checks (
    check_id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    check_json TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    FOREIGN KEY(site_id) REFERENCES sites(site_id)
);
CREATE INDEX IF NOT EXISTS site_health_checks_history
    ON site_health_checks(site_id, checked_at, check_id);
CREATE TRIGGER IF NOT EXISTS site_health_checks_no_update
BEFORE UPDATE ON site_health_checks BEGIN
    SELECT RAISE(FAIL, 'site health observations are immutable');
END;
CREATE TRIGGER IF NOT EXISTS site_health_checks_no_delete
BEFORE DELETE ON site_health_checks BEGIN
    SELECT RAISE(FAIL, 'site health observations are immutable');
END;
";

const MIGRATION_18: &str = r"
CREATE TABLE IF NOT EXISTS site_monitors (
    site_id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL UNIQUE,
    monitor_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(site_id) REFERENCES sites(site_id),
    FOREIGN KEY(schedule_id) REFERENCES schedules(schedule_id)
);
";

const MIGRATION_19: &str = r"
CREATE TABLE IF NOT EXISTS ssh_hosts (
    host_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    revision INTEGER NOT NULL,
    definition_hash TEXT NOT NULL,
    host_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ssh_hosts_state ON ssh_hosts(state, updated_at, host_id);

CREATE TABLE IF NOT EXISTS ssh_host_revisions (
    revision_id TEXT PRIMARY KEY,
    host_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    definition_hash TEXT NOT NULL,
    revision_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(host_id, revision),
    FOREIGN KEY(host_id) REFERENCES ssh_hosts(host_id)
);
CREATE TRIGGER IF NOT EXISTS ssh_host_revisions_no_update
BEFORE UPDATE ON ssh_host_revisions BEGIN
    SELECT RAISE(FAIL, 'SSH host revisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS ssh_host_revisions_no_delete
BEFORE DELETE ON ssh_host_revisions BEGIN
    SELECT RAISE(FAIL, 'SSH host revisions are immutable');
END;

CREATE TABLE IF NOT EXISTS ssh_operations (
    operation_id TEXT PRIMARY KEY,
    host_id TEXT NOT NULL,
    state TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    operation_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(host_id) REFERENCES ssh_hosts(host_id)
);
CREATE INDEX IF NOT EXISTS ssh_operations_history
    ON ssh_operations(host_id, created_at, operation_id);
CREATE TRIGGER IF NOT EXISTS ssh_operations_no_delete
BEFORE DELETE ON ssh_operations BEGIN
    SELECT RAISE(FAIL, 'SSH operation history is immutable');
END;
";

const MIGRATION_20: &str = r"
CREATE TABLE IF NOT EXISTS supervision_observations (
    observation_id TEXT PRIMARY KEY,
    loop_kind TEXT NOT NULL,
    signal TEXT NOT NULL,
    observation_json TEXT NOT NULL,
    observed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS supervision_observations_time
    ON supervision_observations(observed_at, observation_id);
CREATE TRIGGER IF NOT EXISTS supervision_observations_no_update
BEFORE UPDATE ON supervision_observations BEGIN
    SELECT RAISE(FAIL, 'supervision observations are immutable');
END;
CREATE TRIGGER IF NOT EXISTS supervision_observations_no_delete
BEFORE DELETE ON supervision_observations BEGIN
    SELECT RAISE(FAIL, 'supervision observations are immutable');
END;

CREATE TABLE IF NOT EXISTS experiments (
    experiment_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    baseline_revision_id TEXT NOT NULL,
    candidate_revision_id TEXT NOT NULL,
    mode TEXT NOT NULL,
    state TEXT NOT NULL,
    experiment_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(organization_id) REFERENCES organizations(organization_id),
    FOREIGN KEY(baseline_revision_id) REFERENCES organization_revisions(revision_id),
    FOREIGN KEY(candidate_revision_id) REFERENCES organization_revisions(revision_id)
);
CREATE INDEX IF NOT EXISTS experiments_organization_state
    ON experiments(organization_id, state, updated_at, experiment_id);
CREATE TRIGGER IF NOT EXISTS experiments_no_delete
BEFORE DELETE ON experiments BEGIN
    SELECT RAISE(FAIL, 'experiment history cannot be deleted');
END;

CREATE TABLE IF NOT EXISTS shadow_execution_receipts (
    candidate_run_id TEXT PRIMARY KEY,
    experiment_id TEXT NOT NULL,
    source_task_id TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    FOREIGN KEY(experiment_id) REFERENCES experiments(experiment_id),
    FOREIGN KEY(source_task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS shadow_execution_receipts_experiment
    ON shadow_execution_receipts(experiment_id, completed_at, candidate_run_id);
CREATE TRIGGER IF NOT EXISTS shadow_execution_receipts_no_update
BEFORE UPDATE ON shadow_execution_receipts BEGIN
    SELECT RAISE(FAIL, 'shadow execution receipts are immutable');
END;
CREATE TRIGGER IF NOT EXISTS shadow_execution_receipts_no_delete
BEFORE DELETE ON shadow_execution_receipts BEGIN
    SELECT RAISE(FAIL, 'shadow execution receipts are immutable');
END;

CREATE TABLE IF NOT EXISTS experiment_benchmarks (
    benchmark_id TEXT PRIMARY KEY,
    experiment_id TEXT NOT NULL,
    scenario_key TEXT NOT NULL,
    benchmark_json TEXT NOT NULL,
    measured_at TEXT NOT NULL,
    UNIQUE(experiment_id, scenario_key),
    FOREIGN KEY(experiment_id) REFERENCES experiments(experiment_id)
);
CREATE INDEX IF NOT EXISTS experiment_benchmarks_history
    ON experiment_benchmarks(experiment_id, measured_at, benchmark_id);
CREATE TRIGGER IF NOT EXISTS experiment_benchmarks_no_update
BEFORE UPDATE ON experiment_benchmarks BEGIN
    SELECT RAISE(FAIL, 'experiment benchmarks are immutable');
END;
CREATE TRIGGER IF NOT EXISTS experiment_benchmarks_no_delete
BEFORE DELETE ON experiment_benchmarks BEGIN
    SELECT RAISE(FAIL, 'experiment benchmarks are immutable');
END;

CREATE TABLE IF NOT EXISTS experiment_decisions (
    decision_id TEXT PRIMARY KEY,
    experiment_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    decision_json TEXT NOT NULL,
    decided_at TEXT NOT NULL,
    FOREIGN KEY(experiment_id) REFERENCES experiments(experiment_id)
);
CREATE INDEX IF NOT EXISTS experiment_decisions_history
    ON experiment_decisions(experiment_id, decided_at, decision_id);
CREATE TRIGGER IF NOT EXISTS experiment_decisions_no_update
BEFORE UPDATE ON experiment_decisions BEGIN
    SELECT RAISE(FAIL, 'experiment decisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS experiment_decisions_no_delete
BEFORE DELETE ON experiment_decisions BEGIN
    SELECT RAISE(FAIL, 'experiment decisions are immutable');
END;
";

const MIGRATION_21: &str = r"
CREATE TABLE IF NOT EXISTS supervised_evaluations (
    evaluation_id TEXT PRIMARY KEY,
    analysis_sha256 TEXT NOT NULL,
    observation_ids_json TEXT NOT NULL,
    processed_at TEXT NOT NULL,
    FOREIGN KEY(evaluation_id) REFERENCES evaluations(evaluation_id)
);
CREATE TRIGGER IF NOT EXISTS supervised_evaluations_no_update
BEFORE UPDATE ON supervised_evaluations BEGIN
    SELECT RAISE(FAIL, 'automatic supervision history is immutable');
END;
CREATE TRIGGER IF NOT EXISTS supervised_evaluations_no_delete
BEFORE DELETE ON supervised_evaluations BEGIN
    SELECT RAISE(FAIL, 'automatic supervision history is immutable');
END;
";

const MIGRATION_22: &str = r"
CREATE TABLE IF NOT EXISTS automatic_experiment_outcomes (
    observation_id TEXT PRIMARY KEY,
    disposition TEXT NOT NULL,
    candidate_revision_id TEXT,
    experiment_id TEXT,
    detail TEXT NOT NULL,
    processed_at TEXT NOT NULL,
    FOREIGN KEY(observation_id) REFERENCES supervision_observations(observation_id),
    FOREIGN KEY(candidate_revision_id) REFERENCES organization_revisions(revision_id),
    FOREIGN KEY(experiment_id) REFERENCES experiments(experiment_id)
);
CREATE TRIGGER IF NOT EXISTS automatic_experiment_outcomes_no_update
BEFORE UPDATE ON automatic_experiment_outcomes BEGIN
    SELECT RAISE(FAIL, 'automatic experiment history is immutable');
END;
CREATE TRIGGER IF NOT EXISTS automatic_experiment_outcomes_no_delete
BEFORE DELETE ON automatic_experiment_outcomes BEGIN
    SELECT RAISE(FAIL, 'automatic experiment history is immutable');
END;
";

const MIGRATION_23: &str = r"
CREATE TABLE IF NOT EXISTS shadow_workspaces (
    experiment_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    state TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    workspace_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(experiment_id) REFERENCES experiments(experiment_id),
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS shadow_workspaces_state
    ON shadow_workspaces(state, updated_at, experiment_id);
CREATE TRIGGER IF NOT EXISTS shadow_workspaces_no_delete
BEFORE DELETE ON shadow_workspaces BEGIN
    SELECT RAISE(FAIL, 'shadow workspace history cannot be deleted');
END;
";

const MIGRATION_24: &str = r"
CREATE TABLE IF NOT EXISTS provider_proxy_conversations (
    conversation_id TEXT PRIMARY KEY,
    project_id TEXT,
    task_id TEXT,
    conversation_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS provider_proxy_conversations_context
    ON provider_proxy_conversations(project_id, task_id, updated_at, conversation_id);
CREATE TRIGGER IF NOT EXISTS provider_proxy_conversations_no_delete
BEFORE DELETE ON provider_proxy_conversations BEGIN
    SELECT RAISE(FAIL, 'provider proxy conversation history cannot be deleted');
END;

CREATE TABLE IF NOT EXISTS provider_proxy_calls (
    call_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    call_json TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES provider_proxy_conversations(conversation_id)
);
CREATE INDEX IF NOT EXISTS provider_proxy_calls_conversation_time
    ON provider_proxy_calls(conversation_id, completed_at, call_id);
CREATE TRIGGER IF NOT EXISTS provider_proxy_calls_no_update
BEFORE UPDATE ON provider_proxy_calls BEGIN
    SELECT RAISE(FAIL, 'provider proxy call history is immutable');
END;
CREATE TRIGGER IF NOT EXISTS provider_proxy_calls_no_delete
BEFORE DELETE ON provider_proxy_calls BEGIN
    SELECT RAISE(FAIL, 'provider proxy call history is immutable');
END;
";

const MIGRATION_25: &str = r"
CREATE TABLE IF NOT EXISTS user_settings (
    owner_key TEXT PRIMARY KEY CHECK(owner_key = 'local-owner'),
    revision INTEGER NOT NULL,
    settings_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS user_settings_no_delete
BEFORE DELETE ON user_settings BEGIN
    SELECT RAISE(FAIL, 'user settings cannot be deleted');
END;

CREATE TABLE IF NOT EXISTS user_settings_revisions (
    revision INTEGER PRIMARY KEY,
    settings_json TEXT NOT NULL,
    reason TEXT NOT NULL,
    changed_at TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS user_settings_revisions_no_update
BEFORE UPDATE ON user_settings_revisions BEGIN
    SELECT RAISE(FAIL, 'user settings history is immutable');
END;
CREATE TRIGGER IF NOT EXISTS user_settings_revisions_no_delete
BEFORE DELETE ON user_settings_revisions BEGIN
    SELECT RAISE(FAIL, 'user settings history is immutable');
END;
";

const MIGRATION_26: &str = r"
CREATE TABLE IF NOT EXISTS plugin_revisions (
    revision_id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    module_sha256 TEXT NOT NULL,
    revision_json TEXT NOT NULL,
    installed_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS plugin_revisions_history
    ON plugin_revisions(plugin_id, installed_at, revision_id);
CREATE TRIGGER IF NOT EXISTS plugin_revisions_no_update
BEFORE UPDATE ON plugin_revisions BEGIN
    SELECT RAISE(FAIL, 'plugin revisions are immutable');
END;
CREATE TRIGGER IF NOT EXISTS plugin_revisions_no_delete
BEFORE DELETE ON plugin_revisions BEGIN
    SELECT RAISE(FAIL, 'plugin revisions are immutable');
END;

CREATE TABLE IF NOT EXISTS plugin_installations (
    plugin_id TEXT PRIMARY KEY,
    active_revision_id TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0, 1)),
    revision INTEGER NOT NULL,
    installation_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(active_revision_id) REFERENCES plugin_revisions(revision_id)
);
CREATE TRIGGER IF NOT EXISTS plugin_installations_no_delete
BEFORE DELETE ON plugin_installations BEGIN
    SELECT RAISE(FAIL, 'plugin installations cannot be deleted');
END;

CREATE TABLE IF NOT EXISTS plugin_invocations (
    invocation_id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    plugin_revision_id TEXT NOT NULL,
    task_id TEXT,
    input_sha256 TEXT NOT NULL,
    state TEXT NOT NULL,
    invocation_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(plugin_revision_id) REFERENCES plugin_revisions(revision_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS plugin_invocations_plugin_time
    ON plugin_invocations(plugin_id, started_at, invocation_id);
CREATE INDEX IF NOT EXISTS plugin_invocations_task_time
    ON plugin_invocations(task_id, started_at, invocation_id);
CREATE TRIGGER IF NOT EXISTS plugin_invocations_no_delete
BEFORE DELETE ON plugin_invocations BEGIN
    SELECT RAISE(FAIL, 'plugin invocations cannot be deleted');
END;
CREATE TRIGGER IF NOT EXISTS plugin_invocations_guard_update
BEFORE UPDATE ON plugin_invocations
WHEN OLD.state <> 'running'
  OR NEW.state NOT IN ('succeeded', 'failed', 'uncertain')
  OR NEW.invocation_id <> OLD.invocation_id
  OR NEW.plugin_id <> OLD.plugin_id
  OR NEW.plugin_revision_id <> OLD.plugin_revision_id
  OR NEW.task_id IS NOT OLD.task_id
  OR NEW.input_sha256 <> OLD.input_sha256
  OR NEW.started_at <> OLD.started_at
  OR NEW.completed_at IS NULL
BEGIN
    SELECT RAISE(FAIL, 'invalid plugin invocation transition');
END;

CREATE TABLE IF NOT EXISTS plugin_invocation_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    invocation_id TEXT NOT NULL,
    state TEXT NOT NULL,
    invocation_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    UNIQUE(invocation_id, state),
    FOREIGN KEY(invocation_id) REFERENCES plugin_invocations(invocation_id)
);
CREATE INDEX IF NOT EXISTS plugin_invocation_events_history
    ON plugin_invocation_events(invocation_id, sequence);
CREATE TRIGGER IF NOT EXISTS plugin_invocation_events_no_update
BEFORE UPDATE ON plugin_invocation_events BEGIN
    SELECT RAISE(FAIL, 'plugin invocation history is immutable');
END;
CREATE TRIGGER IF NOT EXISTS plugin_invocation_events_no_delete
BEFORE DELETE ON plugin_invocation_events BEGIN
    SELECT RAISE(FAIL, 'plugin invocation history is immutable');
END;
";

const MIGRATION_27: &str = r"
CREATE TABLE IF NOT EXISTS provider_proxy_turn_checkpoints (
    call_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    request_sha256 TEXT NOT NULL,
    checkpoint_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS provider_proxy_turn_checkpoints_task
    ON provider_proxy_turn_checkpoints(task_id, updated_at, call_id);
";

const MIGRATION_28: &str = r"
CREATE TABLE IF NOT EXISTS provider_proxy_ensemble_checkpoints (
    call_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    project_id TEXT,
    task_id TEXT,
    request_sha256 TEXT NOT NULL,
    checkpoint_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS provider_proxy_ensemble_checkpoints_task
    ON provider_proxy_ensemble_checkpoints(task_id, updated_at, call_id);
";

const MIGRATION_29: &str = r"
CREATE TABLE IF NOT EXISTS provider_proxy_dispatches (
    client_request_id TEXT PRIMARY KEY,
    call_id TEXT NOT NULL,
    purpose TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal <= 1024),
    state TEXT NOT NULL CHECK(state IN ('started', 'completed')),
    dispatch_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE(call_id, purpose, ordinal)
);
CREATE INDEX IF NOT EXISTS provider_proxy_dispatches_call
    ON provider_proxy_dispatches(call_id, ordinal, purpose);
CREATE TRIGGER IF NOT EXISTS provider_proxy_dispatches_no_delete
BEFORE DELETE ON provider_proxy_dispatches
BEGIN
    SELECT RAISE(ABORT, 'provider dispatch evidence is append-only');
END;
CREATE TRIGGER IF NOT EXISTS provider_proxy_dispatches_monotonic_update
BEFORE UPDATE ON provider_proxy_dispatches
WHEN OLD.state <> 'started'
  OR NEW.state <> 'completed'
  OR OLD.client_request_id <> NEW.client_request_id
  OR OLD.call_id <> NEW.call_id
  OR OLD.purpose <> NEW.purpose
  OR OLD.ordinal <> NEW.ordinal
  OR OLD.started_at <> NEW.started_at
  OR NEW.completed_at IS NULL
BEGIN
    SELECT RAISE(ABORT, 'provider dispatch evidence may only advance once');
END;
";

const MIGRATION_30: &str = r"
DROP TRIGGER IF EXISTS provider_proxy_dispatches_no_delete;
DROP TRIGGER IF EXISTS provider_proxy_dispatches_monotonic_update;
DROP INDEX IF EXISTS provider_proxy_dispatches_call;
CREATE TABLE provider_proxy_dispatches_v30 (
    client_request_id TEXT PRIMARY KEY,
    call_id TEXT NOT NULL,
    purpose TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal <= 1024),
    state TEXT NOT NULL CHECK(state IN ('started', 'completed', 'failed')),
    dispatch_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE(call_id, purpose, ordinal)
);
INSERT INTO provider_proxy_dispatches_v30(
    client_request_id, call_id, purpose, ordinal, state,
    dispatch_json, started_at, completed_at
)
SELECT client_request_id, call_id, purpose, ordinal, state,
       dispatch_json, started_at, completed_at
FROM provider_proxy_dispatches;
DROP TABLE provider_proxy_dispatches;
ALTER TABLE provider_proxy_dispatches_v30 RENAME TO provider_proxy_dispatches;
CREATE INDEX provider_proxy_dispatches_call
    ON provider_proxy_dispatches(call_id, ordinal, purpose);
CREATE TRIGGER provider_proxy_dispatches_no_delete
BEFORE DELETE ON provider_proxy_dispatches
BEGIN
    SELECT RAISE(ABORT, 'provider dispatch evidence is append-only');
END;
CREATE TRIGGER provider_proxy_dispatches_monotonic_update
BEFORE UPDATE ON provider_proxy_dispatches
WHEN OLD.state <> 'started'
  OR NEW.state NOT IN ('completed', 'failed')
  OR OLD.client_request_id <> NEW.client_request_id
  OR OLD.call_id <> NEW.call_id
  OR OLD.purpose <> NEW.purpose
  OR OLD.ordinal <> NEW.ordinal
  OR OLD.started_at <> NEW.started_at
  OR NEW.completed_at IS NULL
BEGIN
    SELECT RAISE(ABORT, 'provider dispatch evidence may only advance once');
END;
";

const MIGRATION_31: &str = r"
CREATE TABLE IF NOT EXISTS ssh_workflows (
    workflow_id TEXT PRIMARY KEY,
    task_id TEXT,
    state TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    workflow_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(task_id)
);
CREATE INDEX IF NOT EXISTS ssh_workflows_state_time
    ON ssh_workflows(state, updated_at, workflow_id);
CREATE INDEX IF NOT EXISTS ssh_workflows_task_time
    ON ssh_workflows(task_id, created_at, workflow_id);
CREATE TRIGGER IF NOT EXISTS ssh_workflows_no_delete
BEFORE DELETE ON ssh_workflows BEGIN
    SELECT RAISE(FAIL, 'SSH workflow history cannot be deleted');
END;
";

const MIGRATION_32: &str = r"
CREATE TABLE IF NOT EXISTS project_memory_transfers (
    transfer_id TEXT PRIMARY KEY,
    source_project_id TEXT NOT NULL,
    source_claim_id TEXT NOT NULL,
    source_claim_sha256 TEXT NOT NULL,
    target_project_id TEXT NOT NULL,
    target_claim_id TEXT NOT NULL UNIQUE,
    transfer_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(source_project_id) REFERENCES projects(project_id),
    FOREIGN KEY(source_claim_id) REFERENCES memory_claims(claim_id),
    FOREIGN KEY(target_project_id) REFERENCES projects(project_id),
    FOREIGN KEY(target_claim_id) REFERENCES memory_claims(claim_id)
);
CREATE INDEX IF NOT EXISTS project_memory_transfers_source
    ON project_memory_transfers(source_project_id, created_at, transfer_id);
CREATE INDEX IF NOT EXISTS project_memory_transfers_target
    ON project_memory_transfers(target_project_id, created_at, transfer_id);
CREATE TRIGGER IF NOT EXISTS project_memory_transfers_no_update
BEFORE UPDATE ON project_memory_transfers BEGIN
    SELECT RAISE(FAIL, 'project memory transfer evidence is immutable');
END;
CREATE TRIGGER IF NOT EXISTS project_memory_transfers_no_delete
BEFORE DELETE ON project_memory_transfers BEGIN
    SELECT RAISE(FAIL, 'project memory transfer evidence is immutable');
END;
";

const MIGRATION_33: &str = r"
CREATE TABLE IF NOT EXISTS reviewed_topology_plans (
    plan_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_snapshot_id TEXT NOT NULL,
    state TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(project_snapshot_id) REFERENCES project_snapshots(snapshot_id)
);
CREATE INDEX IF NOT EXISTS reviewed_topology_plans_project_time
    ON reviewed_topology_plans(project_id, created_at, plan_id);
CREATE INDEX IF NOT EXISTS reviewed_topology_plans_state_time
    ON reviewed_topology_plans(state, updated_at, plan_id);
CREATE TRIGGER IF NOT EXISTS reviewed_topology_plans_no_delete
BEFORE DELETE ON reviewed_topology_plans BEGIN
    SELECT RAISE(FAIL, 'reviewed topology plan history cannot be deleted');
END;
";

const MIGRATION_34: &str = r"
CREATE TABLE IF NOT EXISTS organization_publishers (
    publisher_id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL,
    state TEXT NOT NULL,
    publisher_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS organization_publishers_state_id
    ON organization_publishers(state, publisher_id);
CREATE TRIGGER IF NOT EXISTS organization_publishers_no_delete
BEFORE DELETE ON organization_publishers BEGIN
    SELECT RAISE(FAIL, 'organization publisher trust history cannot be deleted');
END;
CREATE TRIGGER IF NOT EXISTS organization_publishers_monotonic_update
BEFORE UPDATE ON organization_publishers
WHEN NEW.publisher_id <> OLD.publisher_id
  OR NEW.revision <> OLD.revision + 1
  OR OLD.state = 'revoked'
  OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'organization publisher trust may only advance monotonically');
END;

CREATE TABLE IF NOT EXISTS organization_share_reviews (
    review_id TEXT PRIMARY KEY,
    package_sha256 TEXT NOT NULL,
    publisher_id TEXT NOT NULL,
    state TEXT NOT NULL,
    import_id TEXT UNIQUE,
    organization_id TEXT UNIQUE,
    review_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(publisher_id) REFERENCES organization_publishers(publisher_id),
    FOREIGN KEY(organization_id) REFERENCES organizations(organization_id)
);
CREATE INDEX IF NOT EXISTS organization_share_reviews_state_time
    ON organization_share_reviews(state, created_at, review_id);
CREATE INDEX IF NOT EXISTS organization_share_reviews_package
    ON organization_share_reviews(package_sha256, created_at, review_id);
CREATE TRIGGER IF NOT EXISTS organization_share_reviews_no_delete
BEFORE DELETE ON organization_share_reviews BEGIN
    SELECT RAISE(FAIL, 'organization share review history cannot be deleted');
END;
CREATE TRIGGER IF NOT EXISTS organization_share_reviews_monotonic_update
BEFORE UPDATE ON organization_share_reviews
WHEN OLD.state <> 'pending'
  OR NEW.state NOT IN ('imported', 'rejected')
  OR NEW.review_id <> OLD.review_id
  OR NEW.package_sha256 <> OLD.package_sha256
  OR NEW.publisher_id <> OLD.publisher_id
  OR NEW.created_at <> OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'organization share review may only resolve once');
END;
";

const MIGRATION_35: &str = r"
CREATE TABLE IF NOT EXISTS organization_assignments (
    scope_key TEXT PRIMARY KEY,
    project_id TEXT UNIQUE,
    organization_id TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((scope_key = 'global' AND project_id IS NULL) OR
           (scope_key = 'project:' || project_id AND project_id IS NOT NULL)),
    FOREIGN KEY(project_id) REFERENCES projects(project_id),
    FOREIGN KEY(organization_id) REFERENCES organizations(organization_id)
);
CREATE INDEX IF NOT EXISTS organization_assignments_organization
    ON organization_assignments(organization_id, scope_key);
";

const MIGRATION_36: &str = r"
CREATE TABLE IF NOT EXISTS conversations (
    conversation_id TEXT PRIMARY KEY,
    project_id TEXT,
    state TEXT NOT NULL,
    legacy_thread_id TEXT UNIQUE,
    conversation_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS conversations_state_time
    ON conversations(state, updated_at DESC, conversation_id);
CREATE TABLE IF NOT EXISTS conversation_turns (
    turn_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    state TEXT NOT NULL,
    codex_thread_id TEXT,
    codex_turn_id TEXT,
    turn_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id)
);
CREATE INDEX IF NOT EXISTS conversation_turns_conversation_time
    ON conversation_turns(conversation_id, created_at, turn_id);
CREATE TABLE IF NOT EXISTS conversation_messages (
    message_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    role TEXT NOT NULL,
    message_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(turn_id) REFERENCES conversation_turns(turn_id)
);
CREATE INDEX IF NOT EXISTS conversation_messages_conversation_time
    ON conversation_messages(conversation_id, created_at, message_id);
CREATE TABLE IF NOT EXISTS codex_thread_bindings (
    binding_key TEXT PRIMARY KEY,
    conversation_id TEXT,
    conversation_turn_id TEXT,
    run_id TEXT,
    agent_instance_id TEXT,
    codex_thread_id TEXT NOT NULL,
    binding_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(conversation_turn_id) REFERENCES conversation_turns(turn_id),
    FOREIGN KEY(run_id) REFERENCES agent_runs(run_id),
    FOREIGN KEY(agent_instance_id) REFERENCES agent_instances(instance_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS codex_bindings_conversation
    ON codex_thread_bindings(conversation_id) WHERE conversation_id IS NOT NULL;
";

const MIGRATION_37: &str = r"
CREATE TABLE IF NOT EXISTS conversation_activities (
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    codex_item_id TEXT NOT NULL,
    activity_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(turn_id, codex_item_id),
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(turn_id) REFERENCES conversation_turns(turn_id)
);
CREATE INDEX IF NOT EXISTS conversation_activities_conversation_time
    ON conversation_activities(conversation_id, started_at, codex_item_id);
";

const MIGRATION_38: &str = r"
CREATE TABLE IF NOT EXISTS conversation_agent_sessions (
    session_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    state TEXT NOT NULL,
    session_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id)
);
CREATE INDEX IF NOT EXISTS conversation_agent_sessions_conversation
    ON conversation_agent_sessions(conversation_id, kind, created_at, session_id);
CREATE TABLE IF NOT EXISTS agent_delegations (
    delegation_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    state TEXT NOT NULL,
    delegation_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id)
);
CREATE INDEX IF NOT EXISTS agent_delegations_conversation
    ON agent_delegations(conversation_id, created_at, delegation_id);
CREATE TABLE IF NOT EXISTS agent_messages (
    message_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    delegation_id TEXT,
    message_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id)
);
CREATE INDEX IF NOT EXISTS agent_messages_conversation
    ON agent_messages(conversation_id, created_at, message_id);
CREATE TABLE IF NOT EXISTS blackboard_entries (
    entry_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    delegation_id TEXT,
    entry_json TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id)
);
CREATE INDEX IF NOT EXISTS blackboard_entries_conversation
    ON blackboard_entries(conversation_id, created_at, entry_id);
CREATE TABLE IF NOT EXISTS managed_worktrees (
    worktree_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    delegation_id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL,
    state TEXT NOT NULL,
    worktree_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id),
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS managed_worktrees_project_state
    ON managed_worktrees(project_id, state, updated_at, worktree_id);
";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("stored data is invalid: {0}")]
    InvalidData(String),
    #[error("task {0} was not found")]
    TaskNotFound(TaskId),
    #[error("conversation {0} was not found")]
    ConversationNotFound(ConversationId),
    #[error("conversation turn {0} was not found")]
    ConversationTurnNotFound(ConversationTurnId),
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
    #[error("schedule {0} was not found")]
    ScheduleNotFound(mcg_domain::ScheduleId),
    #[error("platform session {0} was not found")]
    PlatformSessionNotFound(mcg_domain::PlatformSessionId),
    #[error("site {0} was not found")]
    SiteNotFound(mcg_domain::SiteId),
    #[error("SSH host {0} was not found")]
    SshHostNotFound(mcg_domain::SshHostId),
    #[error("SSH workflow {0} was not found")]
    SshWorkflowNotFound(mcg_domain::SshWorkflowId),
    #[error("reviewed topology plan {0} was not found")]
    ReviewedTopologyPlanNotFound(mcg_domain::ReviewedTopologyPlanId),
    #[error("organization share review {0} was not found")]
    OrganizationShareReviewNotFound(mcg_domain::OrganizationShareReviewId),
    #[error("backup {0} was not found")]
    BackupNotFound(Uuid),
    #[error("plugin {0} was not found")]
    PluginNotFound(String),
    #[error("plugin revision {0} was not found")]
    PluginRevisionNotFound(mcg_domain::PluginRevisionId),
    #[error("storage conflict: {0}")]
    Conflict(String),
    #[error("protected credential store operation failed: {0}")]
    ProtectedStore(String),
    #[error(transparent)]
    InvalidTransition(#[from] mcg_domain::InvalidTaskTransition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryReport {
    pub events_replayed: u64,
    pub tasks_rebuilt: u64,
}

/// Secret-free, read-only evidence about the durable event store.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StorageDiagnostics {
    pub integrity: String,
    pub schema_version: i64,
    pub event_count: u64,
    pub task_count: u64,
    pub artifact_count: u64,
    pub plugin_count: u64,
    pub plugin_invocation_count: u64,
    pub provider_dispatch_started_count: u64,
    pub provider_dispatch_completed_count: u64,
    pub provider_dispatch_failed_count: u64,
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
        let newest: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM mcg_schema",
            [],
            |row| row.get(0),
        )?;
        if newest > STORAGE_SCHEMA_VERSION {
            return Err(StorageError::InvalidData(format!(
                "database schema version {newest} is newer than supported version {STORAGE_SCHEMA_VERSION}"
            )));
        }
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
            (11_i64, MIGRATION_11),
            (12_i64, MIGRATION_12),
            (13_i64, MIGRATION_13),
            (14_i64, MIGRATION_14),
            (15_i64, MIGRATION_15),
            (16_i64, MIGRATION_16),
            (17_i64, MIGRATION_17),
            (18_i64, MIGRATION_18),
            (19_i64, MIGRATION_19),
            (20_i64, MIGRATION_20),
            (21_i64, MIGRATION_21),
            (22_i64, MIGRATION_22),
            (23_i64, MIGRATION_23),
            (24_i64, MIGRATION_24),
            (25_i64, MIGRATION_25),
            (26_i64, MIGRATION_26),
            (27_i64, MIGRATION_27),
            (28_i64, MIGRATION_28),
            (29_i64, MIGRATION_29),
            (30_i64, MIGRATION_30),
            (31_i64, MIGRATION_31),
            (32_i64, MIGRATION_32),
            (33_i64, MIGRATION_33),
            (34_i64, MIGRATION_34),
            (35_i64, MIGRATION_35),
            (36_i64, MIGRATION_36),
            (37_i64, MIGRATION_37),
            (38_i64, MIGRATION_38),
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

    /// Persists a chat independently of task/run workflows and records it on the global event log.
    pub fn create_conversation(
        &mut self,
        conversation: &Conversation,
        legacy_thread_id: Option<&str>,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationCreated,
            None,
            serde_json::json!({"conversation_id": conversation.id, "conversation": conversation}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversations(conversation_id, project_id, state, legacy_thread_id, conversation_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                conversation.id.to_string(),
                conversation.project_id.map(|id| id.to_string()),
                conversation_state_name(conversation.state),
                legacy_thread_id,
                serde_json::to_string(conversation)?,
                conversation.created_at.to_rfc3339(),
                conversation.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn update_conversation(
        &mut self,
        conversation: &Conversation,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationUpdated,
            None,
            serde_json::json!({"conversation_id": conversation.id, "conversation": conversation}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations SET project_id = ?2, state = ?3, conversation_json = ?4, updated_at = ?5 WHERE conversation_id = ?1",
            params![
                conversation.id.to_string(),
                conversation.project_id.map(|id| id.to_string()),
                conversation_state_name(conversation.state),
                serde_json::to_string(conversation)?,
                conversation.updated_at.to_rfc3339(),
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::ConversationNotFound(conversation.id));
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_conversation_turn(
        &mut self,
        turn: &ConversationTurn,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationTurnUpdated,
            turn.task_id,
            serde_json::json!({"conversation_id": turn.conversation_id, "turn": turn}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_turns(turn_id, conversation_id, state, codex_thread_id, codex_turn_id, turn_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(turn_id) DO UPDATE SET state=excluded.state, codex_thread_id=excluded.codex_thread_id,
             codex_turn_id=excluded.codex_turn_id, turn_json=excluded.turn_json, updated_at=excluded.updated_at",
            params![
                turn.id.to_string(), turn.conversation_id.to_string(), conversation_turn_state_name(turn.state),
                turn.codex_thread_id.as_deref(), turn.codex_turn_id.as_deref(), serde_json::to_string(turn)?,
                turn.created_at.to_rfc3339(), turn.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn append_conversation_message(
        &mut self,
        message: &ConversationMessage,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationMessageRecorded,
            None,
            serde_json::json!({"conversation_id": message.conversation_id, "message": message}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_messages(message_id, conversation_id, turn_id, role, message_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                message.id.to_string(), message.conversation_id.to_string(), message.turn_id.to_string(),
                conversation_role_name(message.role), serde_json::to_string(message)?, message.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Persists a conversation message, replacing its projection when streamed
    /// assistant content grows while retaining the stable message identity.
    pub fn save_conversation_message(
        &mut self,
        message: &ConversationMessage,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationMessageRecorded,
            None,
            serde_json::json!({"conversation_id": message.conversation_id, "message": message}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_messages(message_id, conversation_id, turn_id, role, message_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(message_id) DO UPDATE SET conversation_id=excluded.conversation_id,
             turn_id=excluded.turn_id, role=excluded.role, message_json=excluded.message_json",
            params![
                message.id.to_string(), message.conversation_id.to_string(), message.turn_id.to_string(),
                conversation_role_name(message.role), serde_json::to_string(message)?, message.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_conversation_activity(
        &mut self,
        activity: &ConversationActivity,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationActivityUpdated,
            None,
            serde_json::json!({"conversation_id": activity.conversation_id, "activity": activity}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_activities(conversation_id, turn_id, codex_item_id, activity_json, started_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(turn_id, codex_item_id) DO UPDATE SET activity_json=excluded.activity_json, updated_at=excluded.updated_at",
            params![
                activity.conversation_id.to_string(), activity.turn_id.to_string(), activity.codex_item_id,
                serde_json::to_string(activity)?, activity.started_at.to_rfc3339(), activity.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Records one raw Codex app-server notification for ordered replay and audit.
    pub fn record_codex_harness_event(
        &mut self,
        payload: serde_json::Value,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(EventKind::CodexHarnessEvent, None, payload);
        let transaction = self.connection.transaction()?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_conversation_agent_session(
        &mut self,
        session: &ConversationAgentSession,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationAgentSessionUpdated,
            None,
            serde_json::json!({"conversation_id": session.conversation_id, "session": session}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_agent_sessions(session_id, conversation_id, kind, state, session_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET state=excluded.state, session_json=excluded.session_json, updated_at=excluded.updated_at",
            params![session.id.to_string(), session.conversation_id.to_string(), serde_json::to_string(&session.kind)?, serde_json::to_string(&session.state)?, serde_json::to_string(session)?, session.created_at.to_rfc3339(), session.updated_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_agent_delegation(
        &mut self,
        delegation: &AgentDelegation,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::AgentDelegationUpdated,
            delegation.task_id,
            serde_json::json!({"conversation_id": delegation.conversation_id, "delegation": delegation}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_delegations(delegation_id, conversation_id, state, delegation_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(delegation_id) DO UPDATE SET state=excluded.state, delegation_json=excluded.delegation_json, updated_at=excluded.updated_at",
            params![delegation.id.to_string(), delegation.conversation_id.to_string(), serde_json::to_string(&delegation.state)?, serde_json::to_string(delegation)?, delegation.created_at.to_rfc3339(), delegation.updated_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn append_agent_message(
        &mut self,
        message: &AgentMessage,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::AgentMessageRecorded,
            None,
            serde_json::json!({"conversation_id": message.conversation_id, "message": message}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_messages(message_id, conversation_id, delegation_id, message_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![message.id.to_string(), message.conversation_id.to_string(), message.delegation_id.map(|id| id.to_string()), serde_json::to_string(message)?, message.created_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn append_blackboard_entry(
        &mut self,
        entry: &BlackboardEntry,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::BlackboardEntryRecorded,
            None,
            serde_json::json!({"conversation_id": entry.conversation_id, "entry": entry}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO blackboard_entries(entry_id, conversation_id, delegation_id, entry_json, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![entry.id.to_string(), entry.conversation_id.to_string(), entry.delegation_id.map(|id| id.to_string()), serde_json::to_string(entry)?, entry.content, entry.created_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_managed_worktree(
        &mut self,
        worktree: &ManagedWorktree,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ManagedWorktreeUpdated,
            None,
            serde_json::json!({"conversation_id": worktree.conversation_id, "worktree": worktree}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO managed_worktrees(worktree_id, conversation_id, delegation_id, project_id, state, worktree_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(worktree_id) DO UPDATE SET state=excluded.state, worktree_json=excluded.worktree_json, updated_at=excluded.updated_at",
            params![worktree.id.to_string(), worktree.conversation_id.to_string(), worktree.delegation_id.to_string(), worktree.project_id.to_string(), serde_json::to_string(&worktree.state)?, serde_json::to_string(worktree)?, worktree.created_at.to_rfc3339(), worktree.updated_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn conversation_team_state(
        &self,
        conversation_id: ConversationId,
    ) -> Result<ConversationTeamState, StorageError> {
        fn rows<T: serde::de::DeserializeOwned>(
            connection: &Connection,
            sql: &str,
            conversation_id: ConversationId,
        ) -> Result<Vec<T>, StorageError> {
            let mut statement = connection.prepare(sql)?;
            statement
                .query_map([conversation_id.to_string()], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect()
        }
        Ok(ConversationTeamState {
            sessions: rows(
                &self.connection,
                "SELECT session_json FROM conversation_agent_sessions WHERE conversation_id=?1 ORDER BY kind, created_at, session_id",
                conversation_id,
            )?,
            delegations: rows(
                &self.connection,
                "SELECT delegation_json FROM agent_delegations WHERE conversation_id=?1 ORDER BY created_at, delegation_id",
                conversation_id,
            )?,
            messages: rows(
                &self.connection,
                "SELECT message_json FROM agent_messages WHERE conversation_id=?1 ORDER BY created_at, message_id",
                conversation_id,
            )?,
            blackboard: rows(
                &self.connection,
                "SELECT entry_json FROM blackboard_entries WHERE conversation_id=?1 ORDER BY created_at, entry_id",
                conversation_id,
            )?,
            worktrees: rows(
                &self.connection,
                "SELECT worktree_json FROM managed_worktrees WHERE conversation_id=?1 ORDER BY created_at, worktree_id",
                conversation_id,
            )?,
        })
    }

    pub fn save_codex_thread_binding(
        &mut self,
        binding: &CodexThreadBinding,
    ) -> Result<(), StorageError> {
        let binding_key = binding.delegation_id.map_or_else(
            || {
                binding.conversation_id.map_or_else(
                    || format!("thread:{}", binding.codex_thread_id),
                    |conversation_id| format!("conversation:{conversation_id}"),
                )
            },
            |delegation_id| format!("delegation:{delegation_id}"),
        );
        self.connection.execute(
            "INSERT INTO codex_thread_bindings(binding_key, conversation_id, conversation_turn_id, run_id, agent_instance_id, codex_thread_id, binding_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(binding_key) DO UPDATE SET conversation_turn_id=excluded.conversation_turn_id, run_id=excluded.run_id,
             agent_instance_id=excluded.agent_instance_id, codex_thread_id=excluded.codex_thread_id,
             binding_json=excluded.binding_json, updated_at=excluded.updated_at",
            params![
                binding_key,
                binding.conversation_id.map(|id| id.to_string()),
                binding.conversation_turn_id.map(|id| id.to_string()),
                binding.run_id.map(|id| id.to_string()),
                binding.agent_instance_id.map(|id| id.to_string()),
                binding.codex_thread_id,
                serde_json::to_string(binding)?,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn codex_thread_binding_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<CodexThreadBinding>, StorageError> {
        self.connection
            .query_row(
                "SELECT binding_json FROM codex_thread_bindings WHERE binding_key = ?1",
                [format!("conversation:{conversation_id}")],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    pub fn codex_thread_bindings(&self) -> Result<Vec<CodexThreadBinding>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT binding_json FROM codex_thread_bindings ORDER BY updated_at DESC")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| {
                let json = row?;
                serde_json::from_str(&json).map_err(StorageError::from)
            })
            .collect()
    }

    pub fn conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<Conversation>, StorageError> {
        self.connection
            .query_row(
                "SELECT conversation_json FROM conversations WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn conversations(&self, include_archived: bool) -> Result<Vec<Conversation>, StorageError> {
        let sql = if include_archived {
            "SELECT conversation_json FROM conversations ORDER BY updated_at DESC, conversation_id"
        } else {
            "SELECT conversation_json FROM conversations WHERE state = 'active' ORDER BY updated_at DESC, conversation_id"
        };
        let mut statement = self.connection.prepare(sql)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    pub fn conversation_view(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<ConversationView>, StorageError> {
        let Some(conversation) = self.conversation(conversation_id)? else {
            return Ok(None);
        };
        let mut turn_statement = self.connection.prepare(
            "SELECT turn_json FROM conversation_turns WHERE conversation_id = ?1 ORDER BY created_at, turn_id"
        )?;
        let turns = turn_statement
            .query_map([conversation_id.to_string()], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        let mut message_statement = self.connection.prepare(
            "SELECT message_json FROM conversation_messages WHERE conversation_id = ?1 ORDER BY created_at, message_id"
        )?;
        let messages = message_statement
            .query_map([conversation_id.to_string()], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        let mut activity_statement = self.connection.prepare(
            "SELECT activity_json FROM conversation_activities WHERE conversation_id = ?1 ORDER BY started_at, codex_item_id"
        )?;
        let activities = activity_statement
            .query_map([conversation_id.to_string()], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ConversationView {
            conversation,
            turns,
            messages,
            activities,
            team: self.conversation_team_state(conversation_id)?,
        }))
    }

    /// Idempotently projects pre-v0.2 task-backed chat threads into conversations.
    pub fn migrate_legacy_task_chats(&mut self) -> Result<u64, StorageError> {
        let tasks = self.tasks()?;
        let mut migrated = 0_u64;
        for task in tasks.into_iter().filter(|task| task.thread_id.is_some()) {
            let legacy = task.thread_id.expect("filtered thread id").to_string();
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE legacy_thread_id = ?1)",
                [&legacy],
                |row| row.get(0),
            )?;
            if exists {
                continue;
            }
            let conversation = Conversation::new(task.goal.clone(), task.project_id);
            self.create_conversation(&conversation, Some(&legacy))?;
            let now = Utc::now();
            let turn = ConversationTurn {
                schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
                id: ConversationTurnId::new(),
                conversation_id: conversation.id,
                state: if task.state == TaskState::Completed {
                    mcg_domain::ConversationTurnState::Completed
                } else {
                    mcg_domain::ConversationTurnState::Recovering
                },
                execution_preferences: Default::default(),
                codex_thread_id: None,
                codex_turn_id: None,
                task_id: Some(task.id),
                run_id: None,
                error: None,
                created_at: task.created_at,
                updated_at: now,
            };
            self.save_conversation_turn(&turn)?;
            self.append_conversation_message(&ConversationMessage {
                schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
                id: ConversationMessageId::new(),
                conversation_id: conversation.id,
                turn_id: turn.id,
                role: mcg_domain::ConversationRole::User,
                content: task.goal,
                codex_item_id: None,
                created_at: task.created_at,
            })?;
            migrated = migrated.saturating_add(1);
        }
        Ok(migrated)
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

    /// Loads one immutable project snapshot by its durable identity.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn project_snapshot(
        &self,
        snapshot_id: ProjectSnapshotId,
    ) -> Result<Option<ProjectModel>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT snapshot_json FROM project_snapshots WHERE snapshot_id = ?1",
                [snapshot_id.to_string()],
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

    /// Atomically copies one reviewed durable claim into another project and
    /// retains immutable, idempotent transfer evidence.
    ///
    /// # Errors
    ///
    /// Missing projects/source claims, stale or changed source evidence, target
    /// key collisions, changed transfer-id reuse, and forged copies fail closed.
    #[allow(clippy::too_many_lines)] // Keep every atomic transfer invariant adjacent and auditable.
    pub fn create_project_memory_transfer(
        &mut self,
        requested: &ProjectMemoryTransfer,
        target_claim: &MemoryClaim,
    ) -> Result<
        (
            ProjectMemoryTransfer,
            MemoryClaim,
            Option<EventEnvelope>,
            bool,
        ),
        StorageError,
    > {
        if let Some(existing) = self.project_memory_transfer(requested.id)? {
            if !same_transfer_intent(&existing, requested) {
                return Err(StorageError::Conflict(
                    "cross-project memory transfer id was reused with changed input".into(),
                ));
            }
            let claim = self
                .memory_claim(existing.target_claim_id)?
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "stored memory transfer target claim is missing".into(),
                    )
                })?;
            return Ok((existing, claim, None, true));
        }
        if requested.schema_version != MEMORY_TRANSFER_SCHEMA_VERSION
            || requested.source_project_id == requested.target_project_id
            || requested.target_claim_id != target_claim.id
            || requested.reason.trim().is_empty()
            || requested.reason.len() > 512
            || requested.reason.chars().any(char::is_control)
        {
            return Err(StorageError::InvalidData(
                "cross-project memory transfer contract is invalid".into(),
            ));
        }
        if requested.source_claim_sha256.len() != 64
            || !requested
                .source_claim_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::InvalidData(
                "cross-project memory transfer digest is malformed".into(),
            ));
        }
        if self.project(requested.source_project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(requested.source_project_id));
        }
        if self.project(requested.target_project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(requested.target_project_id));
        }
        let source = self
            .memory_claim(requested.source_claim_id)?
            .ok_or(StorageError::MemoryClaimNotFound(requested.source_claim_id))?;
        let source_sha256 = memory_claim_sha256(&source)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if source.project_id != requested.source_project_id
            || source.state != MemoryState::Durable
            || !source.is_fresh_at(Utc::now())
            || source_sha256 != requested.source_claim_sha256
        {
            return Err(StorageError::Conflict(
                "reviewed source claim is stale, non-durable, or changed".into(),
            ));
        }
        let expected_source = format!("cross-project-transfer:{}", requested.id);
        if target_claim.project_id != requested.target_project_id
            || target_claim.state != MemoryState::New
            || target_claim.supersedes.is_some()
            || target_claim.namespace != source.namespace
            || target_claim.key != source.key
            || target_claim.value != source.value
            || target_claim.confidence.to_bits() != source.confidence.to_bits()
            || target_claim.fresh_until != source.fresh_until
            || target_claim.correction_count != 0
            || target_claim.usefulness_count != 0
            || target_claim.provenance.source_kind != ProvenanceKind::Imported
            || target_claim.provenance.source != expected_source
            || target_claim.provenance.task_id.is_some()
            || target_claim.provenance.artifact_sha256.as_deref()
                != Some(requested.source_claim_sha256.as_str())
        {
            return Err(StorageError::InvalidData(
                "target memory claim does not match the reviewed source".into(),
            ));
        }
        let collision = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_claims
                WHERE project_id = ?1 AND namespace = ?2 AND claim_key = ?3
                  AND state IN ('new', 'working', 'durable', 'decaying')
             )",
            params![
                requested.target_project_id.to_string(),
                target_claim.namespace,
                target_claim.key
            ],
            |row| row.get::<_, bool>(0),
        )?;
        if collision {
            return Err(StorageError::Conflict(
                "target project already has an active claim for this namespace and key".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::MemoryClaimTransferred,
            None,
            serde_json::json!({"transfer": requested, "claim": target_claim}),
        );
        let transaction = self.connection.transaction()?;
        Self::insert_memory_claim(&transaction, target_claim)?;
        transaction.execute(
            "INSERT INTO project_memory_transfers(
                transfer_id, source_project_id, source_claim_id, source_claim_sha256,
                target_project_id, target_claim_id, transfer_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                requested.id.to_string(),
                requested.source_project_id.to_string(),
                requested.source_claim_id.to_string(),
                requested.source_claim_sha256,
                requested.target_project_id.to_string(),
                requested.target_claim_id.to_string(),
                serde_json::to_string(requested)?,
                requested.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((requested.clone(), target_claim.clone(), Some(event), false))
    }

    /// Loads immutable transfer evidence by caller-owned identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed stored JSON or a failed query.
    pub fn project_memory_transfer(
        &self,
        transfer_id: MemoryTransferId,
    ) -> Result<Option<ProjectMemoryTransfer>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT transfer_json FROM project_memory_transfers WHERE transfer_id = ?1",
                [transfer_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Lists inbound and outbound immutable transfer evidence for one project.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed stored JSON or a failed query.
    pub fn project_memory_transfers(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectMemoryTransfer>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT transfer_json FROM project_memory_transfers
             WHERE source_project_id = ?1 OR target_project_id = ?1
             ORDER BY created_at, transfer_id",
        )?;
        let rows = statement
            .query_map([project_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
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

    /// Loads a bounded, deterministic batch of claim heads eligible for
    /// automatic freshness consolidation.
    ///
    /// Expired new/working/durable claims enter decay. Claims that have already
    /// remained decaying through the supplied grace cutoff become retractable.
    /// `SQLite`'s `datetime` conversion is used so equivalent RFC 3339 offsets do
    /// not depend on lexical ordering.
    ///
    /// # Errors
    ///
    /// Invalid bounds, malformed stored JSON, or query failures are returned.
    pub fn memory_consolidation_candidates(
        &self,
        now: DateTime<Utc>,
        decay_cutoff: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<MemoryClaim>, StorageError> {
        if limit == 0 || limit > 1_024 {
            return Err(StorageError::InvalidData(
                "memory consolidation limit must be between 1 and 1024".into(),
            ));
        }
        let limit = i64::try_from(limit).map_err(|_| {
            StorageError::InvalidData("memory consolidation limit is too large".into())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT claim_json FROM memory_claims
             WHERE (
                 state IN ('new', 'working', 'durable')
                 AND fresh_until IS NOT NULL
                 AND datetime(fresh_until) <= datetime(?1)
             ) OR (
                 state = 'decaying'
                 AND datetime(observed_at) <= datetime(?2)
             )
             ORDER BY project_id, namespace, claim_key, datetime(observed_at), claim_id
             LIMIT ?3",
        )?;
        let rows = statement
            .query_map(
                params![now.to_rfc3339(), decay_cutoff.to_rfc3339(), limit],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
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

    /// Runs read-only integrity and inventory checks without exposing stored content.
    ///
    /// # Errors
    /// Returns [`StorageError`] when `SQLite` cannot complete a diagnostic query.
    pub fn diagnostics(&self) -> Result<StorageDiagnostics, StorageError> {
        fn count(connection: &Connection, table: &str) -> Result<u64, StorageError> {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            let count: i64 = connection.query_row(&sql, [], |row| row.get(0))?;
            u64::try_from(count)
                .map_err(|_| StorageError::InvalidData(format!("negative {table} count")))
        }

        let integrity = self
            .connection
            .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))?;
        let schema_version = self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM mcg_schema",
            [],
            |row| row.get(0),
        )?;
        let provider_dispatch_started_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM provider_proxy_dispatches WHERE state = 'started'",
            [],
            |row| row.get(0),
        )?;
        let provider_dispatch_started_count = u64::try_from(provider_dispatch_started_count)
            .map_err(|_| StorageError::InvalidData("negative started dispatch count".into()))?;
        let provider_dispatch_completed_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM provider_proxy_dispatches WHERE state = 'completed'",
            [],
            |row| row.get(0),
        )?;
        let provider_dispatch_completed_count = u64::try_from(provider_dispatch_completed_count)
            .map_err(|_| StorageError::InvalidData("negative completed dispatch count".into()))?;
        let provider_dispatch_failed_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM provider_proxy_dispatches WHERE state = 'failed'",
            [],
            |row| row.get(0),
        )?;
        let provider_dispatch_failed_count = u64::try_from(provider_dispatch_failed_count)
            .map_err(|_| StorageError::InvalidData("negative failed dispatch count".into()))?;
        Ok(StorageDiagnostics {
            integrity,
            schema_version,
            event_count: count(&self.connection, "events")?,
            task_count: count(&self.connection, "tasks")?,
            artifact_count: count(&self.connection, "artifacts")?,
            plugin_count: count(&self.connection, "plugin_installations")?,
            plugin_invocation_count: count(&self.connection, "plugin_invocations")?,
            provider_dispatch_started_count,
            provider_dispatch_completed_count,
            provider_dispatch_failed_count,
        })
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

const fn conversation_state_name(state: mcg_domain::ConversationState) -> &'static str {
    match state {
        mcg_domain::ConversationState::Active => "active",
        mcg_domain::ConversationState::Archived => "archived",
    }
}

const fn conversation_turn_state_name(state: mcg_domain::ConversationTurnState) -> &'static str {
    match state {
        mcg_domain::ConversationTurnState::Pending => "pending",
        mcg_domain::ConversationTurnState::Streaming => "streaming",
        mcg_domain::ConversationTurnState::WaitingForApproval => "waiting_for_approval",
        mcg_domain::ConversationTurnState::Recovering => "recovering",
        mcg_domain::ConversationTurnState::Indeterminate => "indeterminate",
        mcg_domain::ConversationTurnState::Completed => "completed",
        mcg_domain::ConversationTurnState::Failed => "failed",
        mcg_domain::ConversationTurnState::Interrupted => "interrupted",
    }
}

const fn conversation_role_name(role: mcg_domain::ConversationRole) -> &'static str {
    match role {
        mcg_domain::ConversationRole::User => "user",
        mcg_domain::ConversationRole::Assistant => "assistant",
        mcg_domain::ConversationRole::System => "system",
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

fn same_transfer_intent(
    existing: &ProjectMemoryTransfer,
    requested: &ProjectMemoryTransfer,
) -> bool {
    existing.schema_version == requested.schema_version
        && existing.id == requested.id
        && existing.source_project_id == requested.source_project_id
        && existing.source_claim_id == requested.source_claim_id
        && existing.source_claim_sha256 == requested.source_claim_sha256
        && existing.target_project_id == requested.target_project_id
        && existing.reason == requested.reason
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
    use mcg_domain::{ApprovalId, ArtifactId, MemoryTransferId};
    use mcg_projects::{
        MemoryLifecycle, ProjectDiscoverer, memory_claim_sha256, prepare_project_memory_transfer,
        user_provenance,
    };
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
    fn conversation_activity_is_upserted_and_returned_with_the_view() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let conversation = Conversation::new("show command activity", None);
        store
            .create_conversation(&conversation, None)
            .expect("conversation should persist");
        let now = Utc::now();
        let turn = ConversationTurn {
            schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
            id: ConversationTurnId::new(),
            conversation_id: conversation.id,
            state: mcg_domain::ConversationTurnState::Streaming,
            execution_preferences: mcg_domain::ExecutionPreferences::default(),
            codex_thread_id: Some("thread-1".into()),
            codex_turn_id: Some("turn-1".into()),
            task_id: None,
            run_id: None,
            error: None,
            created_at: now,
            updated_at: now,
        };
        store
            .save_conversation_turn(&turn)
            .expect("turn should persist");
        let mut activity = ConversationActivity {
            schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
            conversation_id: conversation.id,
            turn_id: turn.id,
            codex_item_id: "command-1".into(),
            kind: mcg_domain::ConversationActivityKind::Command,
            state: mcg_domain::ConversationActivityState::Running,
            command: Some("cargo check".into()),
            cwd: Some("C:\\workspace".into()),
            output: Some("Checking".into()),
            exit_code: None,
            duration_ms: None,
            started_at: now,
            updated_at: now,
        };
        store
            .save_conversation_activity(&activity)
            .expect("activity should persist");
        activity.state = mcg_domain::ConversationActivityState::Completed;
        activity.output = Some("Finished".into());
        activity.exit_code = Some(0);
        store
            .save_conversation_activity(&activity)
            .expect("activity update should persist");

        let view = store
            .conversation_view(conversation.id)
            .expect("view should load")
            .expect("conversation should exist");
        assert_eq!(view.activities, vec![activity]);
    }

    #[test]
    fn conversation_team_mailbox_and_blackboard_are_durable_and_scoped() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let first = Conversation::new("two agent chat", None);
        let second = Conversation::new("unrelated chat", None);
        store.create_conversation(&first, None).expect("first chat");
        store
            .create_conversation(&second, None)
            .expect("second chat");
        let now = Utc::now();
        let chat = ConversationAgentSession {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::ConversationAgentSessionId::new(),
            conversation_id: first.id,
            role_id: mcg_domain::RoleId::new(),
            organization_revision_id: mcg_domain::OrganizationRevisionId::new(),
            kind: mcg_domain::ConversationAgentKind::Chat,
            state: mcg_domain::ConversationAgentSessionState::Ready,
            codex_thread_id: Some("chat-thread".into()),
            model: Some("fixture".into()),
            prompt_revision: 3,
            created_at: now,
            updated_at: now,
        };
        let mut coder = chat.clone();
        coder.id = mcg_domain::ConversationAgentSessionId::new();
        coder.role_id = mcg_domain::RoleId::new();
        coder.kind = mcg_domain::ConversationAgentKind::Coder;
        coder.codex_thread_id = Some("coder-thread".into());
        store
            .save_conversation_agent_session(&chat)
            .expect("chat session");
        store
            .save_conversation_agent_session(&coder)
            .expect("coder session");
        let delegation = AgentDelegation {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::AgentDelegationId::new(),
            conversation_id: first.id,
            source_session_id: chat.id,
            coder_session_id: Some(coder.id),
            project_id: ProjectId::new(),
            organization_revision_id: chat.organization_revision_id,
            state: mcg_domain::AgentDelegationState::Running,
            brief: mcg_domain::CodingDelegationBrief {
                objective: "change code".into(),
                scope: vec![],
                constraints: vec![],
                success_conditions: vec![],
                context_references: vec![],
            },
            task_id: None,
            run_id: None,
            worktree_id: None,
            summary: None,
            changed_files: vec![],
            checks: vec![],
            error: None,
            created_at: now,
            updated_at: now,
        };
        store
            .save_agent_delegation(&delegation)
            .expect("delegation");
        let message = AgentMessage {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::AgentMessageId::new(),
            conversation_id: first.id,
            delegation_id: Some(delegation.id),
            from_session_id: chat.id,
            to_session_id: coder.id,
            kind: mcg_domain::AgentMessageKind::Delegation,
            content: "bounded brief".into(),
            created_at: now,
        };
        store.append_agent_message(&message).expect("message");
        let entry = BlackboardEntry {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::BlackboardEntryId::new(),
            conversation_id: first.id,
            delegation_id: Some(delegation.id),
            author_session_id: chat.id,
            kind: mcg_domain::BlackboardEntryKind::Goal,
            content: "shared goal".into(),
            source_references: vec![],
            supersedes_id: None,
            created_at: now,
        };
        store.append_blackboard_entry(&entry).expect("blackboard");

        let first_team = store.conversation_team_state(first.id).expect("first team");
        assert_eq!(first_team.sessions, vec![chat, coder]);
        assert_eq!(first_team.delegations, vec![delegation]);
        assert_eq!(first_team.messages, vec![message]);
        assert_eq!(first_team.blackboard, vec![entry]);
        assert_eq!(
            store
                .conversation_team_state(second.id)
                .expect("second team"),
            ConversationTeamState::default()
        );
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
    fn diagnostics_are_read_only_and_report_content_free_counts() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        store
            .create_task(&TaskEnvelope::new("private task content"))
            .expect("task should persist");

        let report = store.diagnostics().expect("diagnostics should run");

        assert_eq!(report.integrity, "ok");
        assert_eq!(report.schema_version, STORAGE_SCHEMA_VERSION);
        assert_eq!(report.event_count, 1);
        assert_eq!(report.task_count, 1);
        assert_eq!(report.artifact_count, 0);
        assert_eq!(report.provider_dispatch_started_count, 0);
        assert_eq!(report.provider_dispatch_completed_count, 0);
        assert_eq!(report.provider_dispatch_failed_count, 0);
        assert_eq!(store.event_count().expect("count after diagnostics"), 1);
        assert!(
            !serde_json::to_string(&report)
                .expect("diagnostic JSON")
                .contains("private task content")
        );
    }

    #[test]
    fn migration_30_preserves_started_dispatch_and_allows_definite_failure() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let database = temporary.path().join("migration-29.sqlite3");
        let connection = Connection::open(&database).expect("legacy database");
        connection
            .execute_batch(
                "CREATE TABLE mcg_schema (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );",
            )
            .expect("schema table");
        connection
            .execute_batch(MIGRATION_29)
            .expect("migration 29 shape");
        connection
            .execute("INSERT INTO mcg_schema(version) VALUES (29)", [])
            .expect("migration version");
        let call_id = mcg_domain::ProxyCallId::new();
        let started_at = Utc::now();
        let client_request_id = "mcg-migration-29-started";
        let dispatch_json = serde_json::json!({
            "schema_version": mcg_providers::PROVIDER_DISPATCH_SCHEMA_VERSION,
            "call_id": call_id,
            "client_request_id": client_request_id,
            "target": {"provider_id":"legacy-provider","model_id":"legacy-model"},
            "purpose":"primary",
            "ordinal":0,
            "state":"started",
            "response_id":null,
            "started_at":started_at,
            "completed_at":null
        });
        connection
            .execute(
                "INSERT INTO provider_proxy_dispatches(
                    client_request_id, call_id, purpose, ordinal, state,
                    dispatch_json, started_at, completed_at
                 ) VALUES (?1, ?2, 'primary', 0, 'started', ?3, ?4, NULL)",
                params![
                    client_request_id,
                    call_id.to_string(),
                    dispatch_json.to_string(),
                    started_at.to_rfc3339(),
                ],
            )
            .expect("legacy dispatch");
        drop(connection);

        let mut store = SqliteEventStore::open(&database).expect("migrated store");
        assert_eq!(
            store.diagnostics().expect("diagnostics").schema_version,
            STORAGE_SCHEMA_VERSION
        );
        let started = store
            .provider_dispatch(client_request_id)
            .expect("dispatch lookup")
            .expect("preserved dispatch");
        assert_eq!(started.state, mcg_providers::ProviderDispatchState::Started);
        assert_eq!(started.failure, None);
        let failed = store
            .fail_provider_dispatch(
                client_request_id,
                mcg_providers::ProviderDispatchFailure::Http,
                Utc::now(),
            )
            .expect("definite failure");
        assert_eq!(failed.state, mcg_providers::ProviderDispatchState::Failed);
        assert_eq!(
            failed.failure,
            Some(mcg_providers::ProviderDispatchFailure::Http)
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
            reopened.project_snapshot(first.id).expect("exact snapshot"),
            Some(first.clone())
        );
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

    #[test]
    #[allow(clippy::too_many_lines)] // One lifecycle fixture proves rollback, reopen, replay, and collisions.
    fn cross_project_memory_transfer_is_atomic_idempotent_and_restart_durable() {
        let roots = tempfile::tempdir().expect("project roots");
        let source_root = roots.path().join("source");
        let target_root = roots.path().join("target");
        std::fs::create_dir_all(&source_root).expect("source root");
        std::fs::create_dir_all(&target_root).expect("target root");
        std::fs::write(source_root.join("package.json"), "{}").expect("source manifest");
        std::fs::write(target_root.join("Cargo.toml"), "[workspace]").expect("target manifest");
        let source_project_id = ProjectId::new();
        let target_project_id = ProjectId::new();
        let source_project =
            ProjectDiscoverer::discover(&source_root, source_project_id).expect("source project");
        let target_project =
            ProjectDiscoverer::discover(&target_root, target_project_id).expect("target project");
        let database = tempfile::tempdir().expect("database");
        let path = database.path().join("mcg.db");
        let mut source = MemoryLifecycle::create(
            source_project_id,
            "project",
            "package_manager",
            serde_json::json!("pnpm"),
            0.9,
            user_provenance("owner-confirmed source"),
            Some(Utc::now() + chrono::Duration::days(7)),
        )
        .expect("source claim");
        {
            let mut store = SqliteEventStore::open(&path).expect("store");
            store
                .record_project_snapshot(&source_project)
                .expect("source project");
            store
                .record_project_snapshot(&target_project)
                .expect("target project");
            store.create_memory_claim(&source).expect("source claim");
            let first = MemoryLifecycle::reinforced(&mut source, user_provenance("use one"))
                .expect("first reinforcement");
            store
                .revise_memory_claim(&first)
                .expect("first reinforcement storage");
            source = first;
            let second = MemoryLifecycle::reinforced(&mut source, user_provenance("use two"))
                .expect("second reinforcement");
            store
                .revise_memory_claim(&second)
                .expect("second reinforcement storage");
            source = second;
        }
        assert_eq!(source.state, MemoryState::Durable);
        let digest = memory_claim_sha256(&source).expect("digest");
        let transfer_id = MemoryTransferId::new();
        let (transfer, target_claim) = prepare_project_memory_transfer(
            transfer_id,
            &source,
            &digest,
            target_project_id,
            "Owner reviewed the package-manager convention",
        )
        .expect("prepare transfer");

        let mut reopened = SqliteEventStore::open(&path).expect("reopen");
        reopened
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_memory_transfer
                 BEFORE INSERT ON project_memory_transfers BEGIN
                    SELECT RAISE(FAIL, 'injected transfer failure');
                 END;",
            )
            .expect("failure trigger");
        let events_before = reopened.event_count().expect("event count");
        assert!(
            reopened
                .create_project_memory_transfer(&transfer, &target_claim)
                .is_err()
        );
        assert!(
            reopened
                .memory_claim(target_claim.id)
                .expect("target query")
                .is_none()
        );
        assert!(
            reopened
                .project_memory_transfer(transfer_id)
                .expect("transfer query")
                .is_none()
        );
        assert_eq!(reopened.event_count().expect("event count"), events_before);
        reopened
            .connection
            .execute_batch("DROP TRIGGER reject_memory_transfer;")
            .expect("drop failure trigger");

        let (stored, stored_claim, event, replayed) = reopened
            .create_project_memory_transfer(&transfer, &target_claim)
            .expect("transfer");
        assert!(!replayed);
        assert_eq!(stored, transfer);
        assert_eq!(stored_claim, target_claim);
        assert_eq!(
            event.expect("transfer event").kind,
            EventKind::MemoryClaimTransferred
        );
        let events_after = reopened.event_count().expect("event count");
        drop(reopened);

        let mut restarted = SqliteEventStore::open(&path).expect("restart");
        let (replayed_transfer, replayed_claim, event, replayed) = restarted
            .create_project_memory_transfer(&transfer, &target_claim)
            .expect("idempotent replay");
        assert!(replayed);
        assert_eq!(replayed_transfer, transfer);
        assert_eq!(replayed_claim, target_claim);
        assert!(event.is_none());
        assert_eq!(restarted.event_count().expect("event count"), events_after);
        assert_eq!(
            restarted
                .project_memory_transfers(source_project_id)
                .expect("source transfers"),
            vec![transfer.clone()]
        );
        assert_eq!(
            restarted
                .project_memory_transfers(target_project_id)
                .expect("target transfers"),
            vec![transfer.clone()]
        );
        assert!(
            restarted
                .connection
                .execute("DELETE FROM project_memory_transfers", [])
                .is_err()
        );

        let (changed, changed_claim) = prepare_project_memory_transfer(
            transfer_id,
            &source,
            &digest,
            target_project_id,
            "A changed review reason",
        )
        .expect("changed request");
        assert!(matches!(
            restarted.create_project_memory_transfer(&changed, &changed_claim),
            Err(StorageError::Conflict(_))
        ));
        let (collision, collision_claim) = prepare_project_memory_transfer(
            MemoryTransferId::new(),
            &source,
            &digest,
            target_project_id,
            "Another transfer into the occupied target key",
        )
        .expect("collision request");
        assert!(matches!(
            restarted.create_project_memory_transfer(&collision, &collision_claim),
            Err(StorageError::Conflict(_))
        ));
    }
}
