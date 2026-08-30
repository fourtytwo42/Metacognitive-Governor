//! Crash-safe `SQLite` persistence for versioned events and rebuildable projections.

use std::{collections::HashMap, path::Path, time::Duration};

use chrono::{DateTime, Utc};
use mcg_agents::{AgentInstance, AgentLease, AgentRun, WorkPlan};
pub use mcg_contracts::RootMigrationReceipt;
use mcg_contracts::RuntimeIdentity;
use mcg_domain::{
    AgentContextCheckpoint, AgentCredentialAudit, AgentDelegation, AgentMessage, AgentWake,
    AgentWakeId, AgentWakeKind, AgentWakeState, AgentWakeTarget, ApprovalId, BlackboardEntry,
    CloudDiffApply, CloudDiffApplyId, CodexThreadBinding, Conversation, ConversationActivity,
    ConversationAgentSession, ConversationId, ConversationMessage, ConversationMessageId,
    ConversationPreview, ConversationPreviewId, ConversationPreviewState, ConversationTeamState,
    ConversationTurn, ConversationTurnId, ConversationView, CoordinationBatch, CoordinationBatchId,
    CoordinationBatchState, LeaseId, ManagedWorktree, MemoryClaimId, MemoryTransferId, ProjectId,
    ProjectSnapshotId, SharedContextHit, SharedContextSourceKind, TaskEnvelope, TaskId, TaskState,
    ToolExecutionId, ToolRequestId, effective_delegation_attempt_ids,
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

mod actions;
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
mod workflows;

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

pub use actions::{
    ActionAuthorizationReceipt, ReconciliationClaimReceipt, ReconciliationRecoveryReport,
    ReconciliationResolutionReceipt,
};
pub use agents::{
    AgentRecoveryReport, AgentTurnCommit, AgentTurnDisposition, ConsumerEvaluationCheckpoint,
    ConsumerEvaluationCheckpointState, InterfaceRunReceipt,
};
pub use experiments::{
    AutomaticExperimentDisposition, AutomaticExperimentOutcome, AutomaticSupervisionMutation,
    CandidateRevisionMutation, ExperimentMutation, GovernorProposalMutation,
};
pub use plugins::PluginRecoveryReport;
pub use scheduler::{ScheduleClaim, ScheduleWakeMutation};
pub use shadow_workspaces::ShadowWorkspaceMutation;
pub use workflows::{TaskContractHead, WorkflowRecoveryReport, task_contract_content_hash};

type EventRow = (i64, String, i64, String, Option<String>, String, String);
pub const STORAGE_SCHEMA_VERSION: i64 = 54;

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

const MIGRATION_39: &str = r"
CREATE VIRTUAL TABLE IF NOT EXISTS conversation_context_fts USING fts5(
    source_id UNINDEXED,
    conversation_id UNINDEXED,
    delegation_id UNINDEXED,
    author_role UNINDEXED,
    source_kind UNINDEXED,
    created_at UNINDEXED,
    content
);
INSERT INTO conversation_context_fts(source_id, conversation_id, delegation_id, author_role, source_kind, created_at, content)
SELECT message_id, conversation_id, NULL, json_extract(message_json, '$.role'), 'conversation_message', created_at,
       json_extract(message_json, '$.content')
FROM conversation_messages
WHERE NOT EXISTS (SELECT 1 FROM conversation_context_fts LIMIT 1);
INSERT INTO conversation_context_fts(source_id, conversation_id, delegation_id, author_role, source_kind, created_at, content)
SELECT message_id, conversation_id, delegation_id, 'agent', 'agent_message', created_at,
       json_extract(message_json, '$.content')
FROM agent_messages
WHERE NOT EXISTS (
    SELECT 1 FROM conversation_context_fts context_row
    WHERE context_row.source_kind='agent_message' AND context_row.source_id=agent_messages.message_id
);
INSERT INTO conversation_context_fts(source_id, conversation_id, delegation_id, author_role, source_kind, created_at, content)
SELECT entry_id, conversation_id, delegation_id, 'agent', 'blackboard', created_at, content
FROM blackboard_entries
WHERE NOT EXISTS (
    SELECT 1 FROM conversation_context_fts context_row
    WHERE context_row.source_kind='blackboard' AND context_row.source_id=blackboard_entries.entry_id
);
";

const MIGRATION_40: &str = r"
CREATE TABLE IF NOT EXISTS delegation_retry_requests (
    conversation_id TEXT NOT NULL,
    prior_delegation_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    new_delegation_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(conversation_id, prior_delegation_id, idempotency_key),
    UNIQUE(new_delegation_id),
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(prior_delegation_id) REFERENCES agent_delegations(delegation_id)
);
CREATE TABLE IF NOT EXISTS hidden_delegation_tasks (
    task_id TEXT PRIMARY KEY,
    delegation_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(task_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id)
);
CREATE TABLE IF NOT EXISTS worktree_baseline_entries (
    worktree_id TEXT NOT NULL,
    normalized_path TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    file_mode INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    PRIMARY KEY(worktree_id, normalized_path),
    FOREIGN KEY(worktree_id) REFERENCES managed_worktrees(worktree_id)
);
CREATE TABLE IF NOT EXISTS worktree_baseline_exclusions (
    worktree_id TEXT NOT NULL,
    normalized_path TEXT NOT NULL,
    reason TEXT NOT NULL,
    review_blocker INTEGER NOT NULL,
    PRIMARY KEY(worktree_id, normalized_path),
    FOREIGN KEY(worktree_id) REFERENCES managed_worktrees(worktree_id)
);
CREATE TABLE IF NOT EXISTS integration_journals (
    journal_id TEXT PRIMARY KEY,
    delegation_id TEXT NOT NULL UNIQUE,
    worktree_id TEXT NOT NULL,
    state TEXT NOT NULL,
    journal_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id),
    FOREIGN KEY(worktree_id) REFERENCES managed_worktrees(worktree_id)
);
CREATE TABLE IF NOT EXISTS agent_credential_audit (
    audit_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    delegation_id TEXT,
    project_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    nonce_sha256 TEXT NOT NULL,
    audit_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(session_id) REFERENCES conversation_agent_sessions(session_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id),
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS agent_credential_audit_scope
    ON agent_credential_audit(conversation_id, session_id, occurred_at, audit_id);
";

// Schema 36 predated role-specific conversation teams and allowed only one Codex thread binding
// per conversation. A two-agent conversation intentionally owns one persistent Chat thread and
// one distinct thread for every Coding Agent delegation, so conversation_id is now a lookup key
// rather than a uniqueness boundary. binding_key remains the authoritative unique identity.
const MIGRATION_41: &str = r"
DROP INDEX IF EXISTS codex_bindings_conversation;
CREATE INDEX IF NOT EXISTS codex_bindings_conversation_lookup
    ON codex_thread_bindings(conversation_id, updated_at DESC, binding_key);
";

const MIGRATION_42: &str = r"
CREATE TABLE IF NOT EXISTS agent_context_checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    room TEXT NOT NULL,
    revision INTEGER NOT NULL,
    checkpoint_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(conversation_id, session_id, room, revision),
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(session_id) REFERENCES conversation_agent_sessions(session_id)
);
CREATE INDEX IF NOT EXISTS agent_context_checkpoints_scope
    ON agent_context_checkpoints(conversation_id, session_id, room, revision DESC);
";

const MIGRATION_43: &str = r"
CREATE TABLE IF NOT EXISTS coordination_batches (
    batch_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    origin_turn_id TEXT NOT NULL,
    coordinator_session_id TEXT NOT NULL,
    state TEXT NOT NULL,
    batch_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(origin_turn_id) REFERENCES conversation_turns(turn_id),
    FOREIGN KEY(coordinator_session_id) REFERENCES conversation_agent_sessions(session_id)
);
CREATE INDEX IF NOT EXISTS coordination_batches_conversation
    ON coordination_batches(conversation_id, created_at, batch_id);
CREATE TABLE IF NOT EXISTS agent_wakes (
    wake_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    target TEXT NOT NULL,
    kind TEXT NOT NULL,
    state TEXT NOT NULL,
    claimed_until TEXT,
    wake_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(batch_id) REFERENCES coordination_batches(batch_id)
);
CREATE INDEX IF NOT EXISTS agent_wakes_pending
    ON agent_wakes(state, target, created_at, wake_id);
CREATE INDEX IF NOT EXISTS agent_wakes_conversation
    ON agent_wakes(conversation_id, created_at, wake_id);
";

const MIGRATION_44: &str = r"
ALTER TABLE projects ADD COLUMN archived_at TEXT;
ALTER TABLE projects ADD COLUMN removed_at TEXT;
CREATE INDEX IF NOT EXISTS projects_lifecycle
    ON projects(removed_at, archived_at, root, project_id);
";

const MIGRATION_45: &str = r"
CREATE TABLE IF NOT EXISTS conversation_previews (
    preview_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    delegation_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    state TEXT NOT NULL,
    preview_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(batch_id, delegation_id),
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id),
    FOREIGN KEY(batch_id) REFERENCES coordination_batches(batch_id),
    FOREIGN KEY(delegation_id) REFERENCES agent_delegations(delegation_id),
    FOREIGN KEY(project_id) REFERENCES projects(project_id)
);
CREATE INDEX IF NOT EXISTS conversation_previews_conversation
    ON conversation_previews(conversation_id, created_at, preview_id);
";

// Delegation contracts, preflight receipts, context budgets, classified recovery, and outcome
// telemetry are embedded in the versioned JSON projections so schema-45 rows remain readable.
// These expression indexes make the new governance facts queryable without duplicating state.
const MIGRATION_46: &str = r"
CREATE INDEX IF NOT EXISTS agent_delegations_governance_failure
    ON agent_delegations(
        json_extract(delegation_json, '$.governance.failure_class'),
        updated_at,
        delegation_id
    );
CREATE INDEX IF NOT EXISTS agent_delegations_governance_preflight
    ON agent_delegations(
        json_extract(delegation_json, '$.governance.preflight.passed'),
        updated_at,
        delegation_id
    );
CREATE INDEX IF NOT EXISTS coordination_batches_routing
    ON coordination_batches(
        json_extract(batch_json, '$.coordinator_bypassed'),
        updated_at,
        batch_id
    );
";

const MIGRATION_47: &str = r"
CREATE TABLE IF NOT EXISTS governor_proposals (
    proposal_id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    baseline_revision_id TEXT NOT NULL,
    target TEXT NOT NULL,
    risk TEXT NOT NULL,
    state TEXT NOT NULL,
    proposal_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS governor_proposals_organization_state
    ON governor_proposals(organization_id, state, updated_at, proposal_id);
CREATE TRIGGER IF NOT EXISTS governor_proposals_no_delete
BEFORE DELETE ON governor_proposals BEGIN
    SELECT RAISE(FAIL, 'governor proposals are durable audit records');
END;
";

const MIGRATION_48: &str = r"
CREATE TABLE IF NOT EXISTS conversation_tombstones (
    conversation_id TEXT PRIMARY KEY,
    parent_conversation_id TEXT,
    deleted_at TEXT NOT NULL,
    deleted_child_count INTEGER NOT NULL,
    reason TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS conversation_tombstones_no_update
BEFORE UPDATE ON conversation_tombstones BEGIN
    SELECT RAISE(FAIL, 'conversation deletion tombstones are immutable');
END;
CREATE TRIGGER IF NOT EXISTS conversation_tombstones_no_delete
BEFORE DELETE ON conversation_tombstones BEGIN
    SELECT RAISE(FAIL, 'conversation deletion tombstones are immutable');
END;
";

const MIGRATION_49: &str = r"
CREATE TABLE IF NOT EXISTS cli_tui_preferences (
    owner_key TEXT PRIMARY KEY CHECK(owner_key = 'local-owner'),
    preferences_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
";

const MIGRATION_50: &str = r"
CREATE TABLE IF NOT EXISTS cloud_diff_applies (
    apply_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    state TEXT NOT NULL,
    apply_json TEXT NOT NULL,
    patch_blob BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(project_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS cloud_diff_applies_project_updated
    ON cloud_diff_applies(project_id, updated_at DESC, apply_id);
";

const MIGRATION_51: &str = r"
CREATE TABLE IF NOT EXISTS root_ownership (
    owner_key TEXT PRIMARY KEY CHECK(owner_key = 'protected-root'),
    runtime_identity_json TEXT NOT NULL,
    migration_receipt_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
";

const MIGRATION_52: &str = r"
CREATE TABLE IF NOT EXISTS task_contract_versions (
    task_contract_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version > 0),
    type_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    content_hash_json TEXT NOT NULL,
    contract_json TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_contract_id, version)
);
CREATE TABLE IF NOT EXISTS task_contract_heads (
    task_contract_id TEXT PRIMARY KEY,
    current_version INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    FOREIGN KEY(task_contract_id, current_version)
        REFERENCES task_contract_versions(task_contract_id, version)
);
CREATE TABLE IF NOT EXISTS task_contract_dependencies (
    task_contract_id TEXT NOT NULL,
    task_contract_version INTEGER NOT NULL,
    dependency_id TEXT NOT NULL,
    dependency_version INTEGER NOT NULL,
    PRIMARY KEY(task_contract_id, task_contract_version, dependency_id, dependency_version),
    FOREIGN KEY(task_contract_id, task_contract_version)
        REFERENCES task_contract_versions(task_contract_id, version),
    FOREIGN KEY(dependency_id, dependency_version)
        REFERENCES task_contract_versions(task_contract_id, version),
    CHECK(task_contract_id <> dependency_id)
);
CREATE TABLE IF NOT EXISTS workflow_attempts (
    attempt_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL CHECK(attempt_number > 0),
    parent_attempt_id TEXT,
    task_contract_id TEXT NOT NULL,
    task_contract_version INTEGER NOT NULL,
    state TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    lease_generation INTEGER NOT NULL CHECK(lease_generation >= 0),
    lease_owner TEXT,
    lease_expires_at TEXT,
    heartbeat_at TEXT,
    idempotency_key TEXT NOT NULL,
    attempt_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(workflow_id, attempt_number),
    UNIQUE(workflow_id, idempotency_key),
    FOREIGN KEY(task_contract_id, task_contract_version)
        REFERENCES task_contract_versions(task_contract_id, version),
    FOREIGN KEY(parent_attempt_id) REFERENCES workflow_attempts(attempt_id)
);
CREATE INDEX IF NOT EXISTS workflow_attempts_state_updated
    ON workflow_attempts(state, updated_at, workflow_id);
CREATE TABLE IF NOT EXISTS workflow_events (
    event_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence > 0),
    attempt_id TEXT NOT NULL,
    attempt_revision INTEGER NOT NULL CHECK(attempt_revision > 0),
    lease_generation INTEGER NOT NULL CHECK(lease_generation >= 0),
    type_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    correlation_id TEXT NOT NULL,
    causation_id TEXT,
    idempotency_key TEXT NOT NULL,
    event_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(workflow_id, sequence),
    UNIQUE(workflow_id, idempotency_key),
    FOREIGN KEY(attempt_id) REFERENCES workflow_attempts(attempt_id),
    FOREIGN KEY(causation_id) REFERENCES workflow_events(event_id)
);
CREATE INDEX IF NOT EXISTS workflow_events_attempt_sequence
    ON workflow_events(attempt_id, sequence);
";

const MIGRATION_53: &str = r"
CREATE TABLE IF NOT EXISTS task_contract_mutations (
    task_contract_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    contract_version INTEGER NOT NULL,
    expected_head_revision INTEGER NOT NULL CHECK(expected_head_revision > 0),
    resulting_head_revision INTEGER NOT NULL CHECK(resulting_head_revision > 1),
    contract_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_contract_id, idempotency_key),
    UNIQUE(task_contract_id, contract_version),
    FOREIGN KEY(task_contract_id, contract_version)
        REFERENCES task_contract_versions(task_contract_id, version)
);
";

const MIGRATION_54: &str = r"
CREATE TABLE IF NOT EXISTS action_envelopes (
    action_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    workflow_attempt_id TEXT NOT NULL,
    task_contract_id TEXT NOT NULL,
    task_contract_version INTEGER NOT NULL,
    operation_hash_json TEXT NOT NULL,
    effect_class TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    envelope_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(workflow_id, idempotency_key),
    FOREIGN KEY(workflow_attempt_id) REFERENCES workflow_attempts(attempt_id),
    FOREIGN KEY(task_contract_id, task_contract_version)
        REFERENCES task_contract_versions(task_contract_id, version)
);
CREATE INDEX IF NOT EXISTS action_envelopes_workflow_attempt
    ON action_envelopes(workflow_id, workflow_attempt_id, created_at, action_id);
CREATE TABLE IF NOT EXISTS action_executions (
    execution_id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL CHECK(attempt_number > 0),
    state TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    lease_generation INTEGER NOT NULL CHECK(lease_generation >= 0),
    executor TEXT,
    execution_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(action_id, attempt_number),
    FOREIGN KEY(action_id) REFERENCES action_envelopes(action_id)
);
CREATE INDEX IF NOT EXISTS action_executions_state_updated
    ON action_executions(state, updated_at, action_id);
CREATE TABLE IF NOT EXISTS action_receipts (
    receipt_id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    execution_id TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    FOREIGN KEY(action_id) REFERENCES action_envelopes(action_id),
    FOREIGN KEY(execution_id) REFERENCES action_executions(execution_id)
);
CREATE INDEX IF NOT EXISTS action_receipts_execution_observed
    ON action_receipts(execution_id, observed_at, receipt_id);
CREATE TABLE IF NOT EXISTS action_events (
    event_id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence > 0),
    execution_id TEXT,
    type_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    correlation_id TEXT NOT NULL,
    causation_id TEXT,
    idempotency_key TEXT NOT NULL,
    event_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(action_id, sequence),
    UNIQUE(action_id, idempotency_key),
    FOREIGN KEY(action_id) REFERENCES action_envelopes(action_id),
    FOREIGN KEY(execution_id) REFERENCES action_executions(execution_id),
    FOREIGN KEY(causation_id) REFERENCES action_events(event_id)
);
CREATE TABLE IF NOT EXISTS reconciliation_jobs (
    reconciliation_job_id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    execution_id TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    lease_generation INTEGER NOT NULL CHECK(lease_generation >= 0),
    lease_owner TEXT,
    lease_expires_at TEXT,
    next_attempt_at TEXT NOT NULL,
    deadline TEXT NOT NULL,
    job_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(action_id) REFERENCES action_envelopes(action_id),
    FOREIGN KEY(execution_id) REFERENCES action_executions(execution_id)
);
CREATE INDEX IF NOT EXISTS reconciliation_jobs_state_due
    ON reconciliation_jobs(state, next_attempt_at, deadline, reconciliation_job_id);
CREATE INDEX IF NOT EXISTS reconciliation_jobs_lease_expiry
    ON reconciliation_jobs(state, lease_expires_at, reconciliation_job_id);
CREATE TRIGGER IF NOT EXISTS action_envelopes_no_update
BEFORE UPDATE ON action_envelopes BEGIN
    SELECT RAISE(FAIL, 'action envelopes are immutable intent');
END;
CREATE TRIGGER IF NOT EXISTS action_envelopes_no_delete
BEFORE DELETE ON action_envelopes BEGIN
    SELECT RAISE(FAIL, 'action envelopes are durable history');
END;
CREATE TRIGGER IF NOT EXISTS action_receipts_no_update
BEFORE UPDATE ON action_receipts BEGIN
    SELECT RAISE(FAIL, 'action receipts are immutable observations');
END;
CREATE TRIGGER IF NOT EXISTS action_receipts_no_delete
BEFORE DELETE ON action_receipts BEGIN
    SELECT RAISE(FAIL, 'action receipts are durable history');
END;
CREATE TRIGGER IF NOT EXISTS action_events_no_update
BEFORE UPDATE ON action_events BEGIN
    SELECT RAISE(FAIL, 'action events are immutable history');
END;
CREATE TRIGGER IF NOT EXISTS action_events_no_delete
BEFORE DELETE ON action_events BEGIN
    SELECT RAISE(FAIL, 'action events are durable history');
END;
";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RootOwnership {
    pub runtime_identity: RuntimeIdentity,
    pub migration_receipt: RootMigrationReceipt,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreRootDatabaseReport {
    pub schema_version: i64,
    pub has_root_ownership: bool,
}

/// Inspects an existing database without applying migrations or taking write authority.
///
/// # Errors
/// Returns a storage error for unreadable, corrupt, or invalid schema metadata.
pub fn inspect_database_before_root(path: &Path) -> Result<PreRootDatabaseReport, StorageError> {
    use rusqlite::OpenFlags;

    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?;
    if integrity != "ok" {
        return Err(StorageError::InvalidData(format!(
            "database integrity check failed before Root migration: {integrity}"
        )));
    }
    let schema_exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mcg_schema')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let schema_version = if schema_exists {
        connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM mcg_schema",
            [],
            |row| row.get(0),
        )?
    } else {
        0
    };
    if schema_version > STORAGE_SCHEMA_VERSION {
        return Err(StorageError::InvalidData(format!(
            "database schema version {schema_version} is newer than supported version {STORAGE_SCHEMA_VERSION}"
        )));
    }
    let ownership_table = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'root_ownership')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let has_root_ownership = ownership_table
        && connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM root_ownership WHERE owner_key = 'protected-root')",
            [],
            |row| row.get::<_, bool>(0),
        )?;
    Ok(PreRootDatabaseReport {
        schema_version,
        has_root_ownership,
    })
}

/// Creates a consistent `SQLite` snapshot without opening the source as a migrator.
///
/// # Errors
/// Returns a storage error when the source cannot be opened read-only or copied.
pub fn snapshot_database_before_root(
    source: &Path,
    destination: &Path,
) -> Result<(), StorageError> {
    use rusqlite::OpenFlags;

    let connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.backup(rusqlite::MAIN_DB, destination, None)?;
    Ok(())
}

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
    #[error("workflow attempt {0} was not found")]
    WorkflowAttemptNotFound(mcg_contracts::AttemptId),
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
            (39_i64, MIGRATION_39),
            (40_i64, MIGRATION_40),
            (41_i64, MIGRATION_41),
            (42_i64, MIGRATION_42),
            (43_i64, MIGRATION_43),
            (44_i64, MIGRATION_44),
            (45_i64, MIGRATION_45),
            (46_i64, MIGRATION_46),
            (47_i64, MIGRATION_47),
            (48_i64, MIGRATION_48),
            (49_i64, MIGRATION_49),
            (50_i64, MIGRATION_50),
            (51_i64, MIGRATION_51),
            (52_i64, MIGRATION_52),
            (53_i64, MIGRATION_53),
            (54_i64, MIGRATION_54),
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

    /// Creates the immutable installation-level Root ownership record exactly once.
    ///
    /// Existing ownership is returned unchanged so a process restart cannot silently
    /// replace the migrated installation identity or its backup receipt.
    ///
    /// # Errors
    /// Returns a storage error for malformed stored data or a failed transaction.
    pub fn initialize_root_ownership(
        &mut self,
        runtime_identity: &RuntimeIdentity,
        migration_receipt: &RootMigrationReceipt,
        now: DateTime<Utc>,
    ) -> Result<RootOwnership, StorageError> {
        if let Some(existing) = self.root_ownership()? {
            return Ok(existing);
        }
        if migration_receipt.schema_version != 1
            || migration_receipt.to_storage_schema != STORAGE_SCHEMA_VERSION
            || runtime_identity.protocol_version == 0
        {
            return Err(StorageError::InvalidData(
                "Root ownership metadata is invalid".into(),
            ));
        }
        let ownership = RootOwnership {
            runtime_identity: runtime_identity.clone(),
            migration_receipt: migration_receipt.clone(),
            created_at: now,
        };
        self.connection.execute(
            "INSERT INTO root_ownership(owner_key, runtime_identity_json, migration_receipt_json, created_at)
             VALUES ('protected-root', ?1, ?2, ?3)",
            params![
                serde_json::to_string(runtime_identity)?,
                serde_json::to_string(migration_receipt)?,
                now.to_rfc3339(),
            ],
        )?;
        Ok(ownership)
    }

    /// Loads the immutable Root ownership and migration receipt.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn root_ownership(&self) -> Result<Option<RootOwnership>, StorageError> {
        self.connection
            .query_row(
                "SELECT runtime_identity_json, migration_receipt_json, created_at
                 FROM root_ownership WHERE owner_key = 'protected-root'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .map(|(identity, receipt, created_at)| {
                Ok(RootOwnership {
                    runtime_identity: serde_json::from_str(&identity)?,
                    migration_receipt: serde_json::from_str(&receipt)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map_err(|error| StorageError::InvalidData(error.to_string()))?
                        .with_timezone(&Utc),
                })
            })
            .transpose()
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

    pub fn cli_tui_preferences(&self) -> Result<mcg_config::CliTuiPreferences, StorageError> {
        self.connection
            .query_row(
                "SELECT preferences_json FROM cli_tui_preferences WHERE owner_key='local-owner'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
            .map(|preferences| preferences.unwrap_or_default())
    }

    pub fn save_cli_tui_preferences(
        &mut self,
        preferences: &mcg_config::CliTuiPreferences,
    ) -> Result<(), StorageError> {
        if !preferences.is_valid() {
            return Err(StorageError::InvalidData(
                "CLI TUI preferences are invalid".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO cli_tui_preferences(owner_key, preferences_json, updated_at)
             VALUES ('local-owner', ?1, ?2)
             ON CONFLICT(owner_key) DO UPDATE SET
                preferences_json=excluded.preferences_json,
                updated_at=excluded.updated_at",
            params![serde_json::to_string(preferences)?, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn artifact_relative_path_is_registered(
        &self,
        relative_path: &str,
    ) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM artifacts WHERE relative_path=?1)",
                [relative_path],
                |row| row.get::<_, bool>(0),
            )
            .map_err(StorageError::from)
    }

    /// Permanently removes one conversation tree and retains only content-free tombstones.
    pub fn delete_conversation_tree(
        &mut self,
        root_id: ConversationId,
        deleted_at: DateTime<Utc>,
    ) -> Result<(Vec<ConversationId>, EventEnvelope), StorageError> {
        let conversations = self.conversations(true)?;
        if !conversations
            .iter()
            .any(|conversation| conversation.id == root_id)
        {
            return Err(StorageError::ConversationNotFound(root_id));
        }
        let mut targets = vec![root_id];
        let mut cursor = 0_usize;
        while cursor < targets.len() {
            let parent = targets[cursor];
            let children = conversations
                .iter()
                .filter(|conversation| {
                    conversation.parent_conversation_id == Some(parent)
                        && !targets.contains(&conversation.id)
                })
                .map(|conversation| conversation.id)
                .collect::<Vec<_>>();
            targets.extend(children);
            cursor += 1;
        }
        let target_strings = targets.iter().map(ToString::to_string).collect::<Vec<_>>();
        let transaction = self.connection.transaction()?;
        for id in &target_strings {
            transaction.execute(
                "DELETE FROM events WHERE EXISTS (
                    SELECT 1 FROM json_tree(events.payload)
                    WHERE json_tree.key = 'conversation_id' AND json_tree.value = ?1
                )",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_previews WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute("DELETE FROM agent_wakes WHERE conversation_id=?1", [id])?;
            transaction.execute(
                "DELETE FROM agent_credential_audit WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM agent_context_checkpoints WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM delegation_retry_requests WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM hidden_delegation_tasks WHERE delegation_id IN (
                    SELECT delegation_id FROM agent_delegations WHERE conversation_id=?1
                )",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM integration_journals WHERE delegation_id IN (
                    SELECT delegation_id FROM agent_delegations WHERE conversation_id=?1
                )",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM worktree_baseline_entries WHERE worktree_id IN (
                    SELECT worktree_id FROM managed_worktrees WHERE conversation_id=?1
                )",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM worktree_baseline_exclusions WHERE worktree_id IN (
                    SELECT worktree_id FROM managed_worktrees WHERE conversation_id=?1
                )",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM managed_worktrees WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM coordination_batches WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM blackboard_entries WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute("DELETE FROM agent_messages WHERE conversation_id=?1", [id])?;
            transaction.execute(
                "DELETE FROM agent_delegations WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM codex_thread_bindings WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_activities WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_messages WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_turns WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_agent_sessions WHERE conversation_id=?1",
                [id],
            )?;
            transaction.execute(
                "DELETE FROM conversation_context_fts WHERE conversation_id=?1",
                [id],
            )?;
        }
        let child_count = targets.len().saturating_sub(1);
        for conversation in conversations
            .iter()
            .filter(|conversation| targets.contains(&conversation.id))
        {
            transaction.execute(
                "INSERT INTO conversation_tombstones(
                    conversation_id, parent_conversation_id, deleted_at, deleted_child_count, reason
                 ) VALUES (?1, ?2, ?3, ?4, 'owner_confirmed')",
                params![
                    conversation.id.to_string(),
                    conversation.parent_conversation_id.map(|id| id.to_string()),
                    deleted_at.to_rfc3339(),
                    if conversation.id == root_id {
                        i64::try_from(child_count).unwrap_or(i64::MAX)
                    } else {
                        0
                    },
                ],
            )?;
        }
        for id in &target_strings {
            transaction.execute("DELETE FROM conversations WHERE conversation_id=?1", [id])?;
        }
        let event = EventEnvelope::new(
            EventKind::ConversationDeleted,
            None,
            serde_json::json!({
                "conversation_id": root_id,
                "deleted_child_count": child_count,
                "deleted_at": deleted_at,
            }),
        );
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((targets, event))
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
        index_conversation_context(
            &transaction,
            &message.id.to_string(),
            message.conversation_id,
            None,
            conversation_role_name(message.role),
            "conversation_message",
            message.created_at,
            &message.content,
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
        index_conversation_context(
            &transaction,
            &message.id.to_string(),
            message.conversation_id,
            None,
            conversation_role_name(message.role),
            "conversation_message",
            message.created_at,
            &message.content,
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

    /// Atomically creates the hidden task and complete single-Coder execution authority for a
    /// delegation. No Executive Agent is involved and no partial authority survives failure.
    pub fn create_delegation_execution(
        &mut self,
        task: &TaskEnvelope,
        run: &AgentRun,
        plan: &WorkPlan,
        instance: &AgentInstance,
        lease: &AgentLease,
        delegation: &AgentDelegation,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        if delegation.task_id != Some(task.id)
            || delegation.run_id != Some(run.id)
            || delegation.agent_instance_id != Some(instance.id)
            || delegation.agent_lease_id != Some(lease.id)
            || run.task_id != task.id
            || plan.task_id != task.id
            || plan.run_id != run.id
            || instance.task_id != task.id
            || instance.run_id != run.id
            || instance.role_id
                != plan
                    .items
                    .first()
                    .map(|item| item.assigned_role)
                    .ok_or_else(|| StorageError::InvalidData("delegation plan is empty".into()))?
            || plan.items.len() != 1
            || lease.instance_id != instance.id
            || lease.work_item_id != plan.items[0].id
        {
            return Err(StorageError::InvalidData(
                "delegation execution identities are inconsistent".into(),
            ));
        }
        let task_event = EventEnvelope::new(
            EventKind::TaskCreated,
            Some(task.id),
            serde_json::to_value(TaskCreatedPayload { task: task.clone() })?,
        );
        let run_event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(task.id),
            serde_json::json!({"run":run,"plan":plan,"delegation_id":delegation.id}),
        );
        let instance_event = EventEnvelope::new(
            EventKind::AgentInstanceRecorded,
            Some(task.id),
            serde_json::to_value(instance)?,
        );
        let lease_event = EventEnvelope::new(
            EventKind::AgentLeaseRecorded,
            Some(task.id),
            serde_json::to_value(lease)?,
        );
        let delegation_event = EventEnvelope::new(
            EventKind::AgentDelegationUpdated,
            Some(task.id),
            serde_json::json!({"conversation_id":delegation.conversation_id,"delegation":delegation}),
        );
        let transaction = self.connection.transaction()?;
        let task_event = Self::insert_event(&transaction, &task_event)?;
        Self::upsert_task(&transaction, task, task_event.sequence)?;
        transaction.execute(
            "INSERT INTO agent_runs(run_id, task_id, organization_revision_id, state, run_json, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![run.id.to_string(), task.id.to_string(), run.organization_revision_id.to_string(), serde_json::to_string(&run.state)?.trim_matches('"'), serde_json::to_string(run)?, run.created_at.to_rfc3339(), run.updated_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![run.id.to_string(), task.id.to_string(), serde_json::to_string(plan)?, plan.created_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO agent_instances(instance_id, run_id, task_id, role_id, state, instance_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![instance.id.to_string(), run.id.to_string(), task.id.to_string(), instance.role_id.to_string(), serde_json::to_string(&instance.state)?.trim_matches('"'), serde_json::to_string(instance)?, instance.created_at.to_rfc3339(), instance.updated_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO agent_leases(lease_id, instance_id, work_item_id, state, lease_json, acquired_at, expires_at, heartbeat_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![lease.id.to_string(), instance.id.to_string(), lease.work_item_id.to_string(), serde_json::to_string(&lease.state)?.trim_matches('"'), serde_json::to_string(lease)?, lease.acquired_at.to_rfc3339(), lease.expires_at.to_rfc3339(), lease.heartbeat_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO agent_delegations(delegation_id, conversation_id, state, delegation_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![delegation.id.to_string(), delegation.conversation_id.to_string(), serde_json::to_string(&delegation.state)?, serde_json::to_string(delegation)?, delegation.created_at.to_rfc3339(), delegation.updated_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO hidden_delegation_tasks(task_id, delegation_id, created_at) VALUES (?1, ?2, ?3)",
            params![task.id.to_string(), delegation.id.to_string(), delegation.created_at.to_rfc3339()],
        )?;
        let run_event = Self::insert_event(&transaction, &run_event)?;
        let instance_event = Self::insert_event(&transaction, &instance_event)?;
        let lease_event = Self::insert_event(&transaction, &lease_event)?;
        let delegation_event = Self::insert_event(&transaction, &delegation_event)?;
        transaction.commit()?;
        Ok(vec![
            task_event,
            run_event,
            instance_event,
            lease_event,
            delegation_event,
        ])
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
        let author_role = transaction
            .query_row(
                "SELECT json_extract(session_json, '$.kind') FROM conversation_agent_sessions WHERE session_id=?1",
                [message.from_session_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| "agent".into());
        index_conversation_context(
            &transaction,
            &message.id.to_string(),
            message.conversation_id,
            message.delegation_id,
            &author_role,
            "agent_message",
            message.created_at,
            &message.content,
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Persists the owner-to-Coordinator handoff as one recovery boundary. A crash can expose
    /// either all three projections (message, batch, wake) or none of them, never an orphaned
    /// backstage message that appears accepted without runnable coordination state.
    pub fn append_coordination_request(
        &mut self,
        message: &AgentMessage,
        batch: &CoordinationBatch,
        wake: &AgentWake,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        if message.conversation_id != batch.conversation_id
            || wake.conversation_id != batch.conversation_id
            || wake.coordination_batch_id != batch.id
            || wake.source_message_id != Some(message.id)
        {
            return Err(StorageError::InvalidData(
                "coordination request identities do not match".into(),
            ));
        }
        let message_event = EventEnvelope::new(
            EventKind::AgentMessageRecorded,
            None,
            serde_json::json!({"conversation_id": message.conversation_id, "message": message}),
        );
        let batch_event = EventEnvelope::new(
            EventKind::CoordinationBatchUpdated,
            None,
            serde_json::json!({"conversation_id": batch.conversation_id, "batch": batch}),
        );
        let wake_event = EventEnvelope::new(
            EventKind::AgentWakeUpdated,
            None,
            serde_json::json!({"conversation_id": wake.conversation_id, "wake": wake}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_messages(message_id, conversation_id, delegation_id, message_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![message.id.to_string(), message.conversation_id.to_string(), message.delegation_id.map(|id| id.to_string()), serde_json::to_string(message)?, message.created_at.to_rfc3339()],
        )?;
        let author_role = transaction.query_row(
            "SELECT json_extract(session_json, '$.kind') FROM conversation_agent_sessions WHERE session_id=?1",
            [message.from_session_id.to_string()],
            |row| row.get::<_, String>(0),
        ).optional()?.unwrap_or_else(|| "agent".into());
        index_conversation_context(
            &transaction,
            &message.id.to_string(),
            message.conversation_id,
            message.delegation_id,
            &author_role,
            "agent_message",
            message.created_at,
            &message.content,
        )?;
        transaction.execute(
            "INSERT INTO coordination_batches(batch_id, conversation_id, origin_turn_id, coordinator_session_id, state, batch_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![batch.id.to_string(), batch.conversation_id.to_string(), batch.origin_turn_id.to_string(), batch.coordinator_session_id.to_string(), serde_json::to_string(&batch.state)?.trim_matches('"'), serde_json::to_string(batch)?, batch.created_at.to_rfc3339(), batch.updated_at.to_rfc3339()],
        )?;
        transaction.execute(
            "INSERT INTO agent_wakes(wake_id, idempotency_key, conversation_id, batch_id, target, kind, state, claimed_until, wake_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![wake.id.to_string(), wake.idempotency_key, wake.conversation_id.to_string(), wake.coordination_batch_id.to_string(), serde_json::to_string(&wake.target)?.trim_matches('"'), serde_json::to_string(&wake.kind)?.trim_matches('"'), serde_json::to_string(&wake.state)?.trim_matches('"'), wake.claimed_until.map(|value| value.to_rfc3339()), serde_json::to_string(wake)?, wake.created_at.to_rfc3339(), wake.updated_at.to_rfc3339()],
        )?;
        let message_event = Self::insert_event(&transaction, &message_event)?;
        let batch_event = Self::insert_event(&transaction, &batch_event)?;
        let wake_event = Self::insert_event(&transaction, &wake_event)?;
        transaction.commit()?;
        Ok(vec![message_event, batch_event, wake_event])
    }

    /// Atomically records the Coordinator callback, its one Chat wake, and the batch's
    /// `chat_pending` projection. Repeated wake keys return the original wake without adding a
    /// duplicate internal message or owner-visible completion path.
    pub fn append_chat_callback(
        &mut self,
        message: &AgentMessage,
        wake: &AgentWake,
        batch: &CoordinationBatch,
    ) -> Result<(AgentWake, Vec<EventEnvelope>), StorageError> {
        if let Some(existing) = self
            .connection
            .query_row(
                "SELECT wake_json FROM agent_wakes WHERE idempotency_key=?1",
                [&wake.idempotency_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok((serde_json::from_str(&existing)?, Vec::new()));
        }
        if message.conversation_id != batch.conversation_id
            || wake.conversation_id != batch.conversation_id
            || wake.coordination_batch_id != batch.id
            || wake.source_message_id != Some(message.id)
        {
            return Err(StorageError::InvalidData(
                "Chat callback identities do not match".into(),
            ));
        }
        let message_event = EventEnvelope::new(
            EventKind::AgentMessageRecorded,
            None,
            serde_json::json!({"conversation_id": message.conversation_id, "message": message}),
        );
        let wake_event = EventEnvelope::new(
            EventKind::AgentWakeUpdated,
            None,
            serde_json::json!({"conversation_id": wake.conversation_id, "wake": wake}),
        );
        let batch_event = EventEnvelope::new(
            EventKind::CoordinationBatchUpdated,
            None,
            serde_json::json!({"conversation_id": batch.conversation_id, "batch": batch}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_messages(message_id, conversation_id, delegation_id, message_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![message.id.to_string(), message.conversation_id.to_string(), message.delegation_id.map(|id| id.to_string()), serde_json::to_string(message)?, message.created_at.to_rfc3339()],
        )?;
        let author_role = transaction.query_row(
            "SELECT json_extract(session_json, '$.kind') FROM conversation_agent_sessions WHERE session_id=?1",
            [message.from_session_id.to_string()],
            |row| row.get::<_, String>(0),
        ).optional()?.unwrap_or_else(|| "agent".into());
        index_conversation_context(
            &transaction,
            &message.id.to_string(),
            message.conversation_id,
            message.delegation_id,
            &author_role,
            "agent_message",
            message.created_at,
            &message.content,
        )?;
        transaction.execute(
            "INSERT INTO agent_wakes(wake_id, idempotency_key, conversation_id, batch_id, target, kind, state, claimed_until, wake_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![wake.id.to_string(), wake.idempotency_key, wake.conversation_id.to_string(), wake.coordination_batch_id.to_string(), serde_json::to_string(&wake.target)?.trim_matches('"'), serde_json::to_string(&wake.kind)?.trim_matches('"'), serde_json::to_string(&wake.state)?.trim_matches('"'), wake.claimed_until.map(|value| value.to_rfc3339()), serde_json::to_string(wake)?, wake.created_at.to_rfc3339(), wake.updated_at.to_rfc3339()],
        )?;
        transaction.execute(
            "UPDATE coordination_batches SET state=?2, batch_json=?3, updated_at=?4 WHERE batch_id=?1",
            params![batch.id.to_string(), serde_json::to_string(&batch.state)?.trim_matches('"'), serde_json::to_string(batch)?, batch.updated_at.to_rfc3339()],
        )?;
        let message_event = Self::insert_event(&transaction, &message_event)?;
        let wake_event = Self::insert_event(&transaction, &wake_event)?;
        let batch_event = Self::insert_event(&transaction, &batch_event)?;
        transaction.commit()?;
        Ok((wake.clone(), vec![message_event, wake_event, batch_event]))
    }

    pub fn save_agent_context_checkpoint(
        &mut self,
        checkpoint: &AgentContextCheckpoint,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::AgentContextCheckpointUpdated,
            None,
            serde_json::json!({
                "conversation_id": checkpoint.conversation_id,
                "checkpoint": checkpoint
            }),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_context_checkpoints(checkpoint_id, conversation_id, session_id, room, revision, checkpoint_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                checkpoint.id.to_string(),
                checkpoint.conversation_id.to_string(),
                checkpoint.session_id.to_string(),
                serde_json::to_string(&checkpoint.room)?.trim_matches('"'),
                i64::try_from(checkpoint.revision).map_err(|_| StorageError::InvalidData("context checkpoint revision exceeds SQLite range".into()))?,
                serde_json::to_string(checkpoint)?,
                checkpoint.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn save_coordination_batch(
        &mut self,
        batch: &CoordinationBatch,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::CoordinationBatchUpdated,
            None,
            serde_json::json!({"conversation_id": batch.conversation_id, "batch": batch}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO coordination_batches(batch_id, conversation_id, origin_turn_id, coordinator_session_id, state, batch_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(batch_id) DO UPDATE SET state=excluded.state, batch_json=excluded.batch_json, updated_at=excluded.updated_at",
            params![
                batch.id.to_string(), batch.conversation_id.to_string(), batch.origin_turn_id.to_string(),
                batch.coordinator_session_id.to_string(), serde_json::to_string(&batch.state)?.trim_matches('"'),
                serde_json::to_string(batch)?, batch.created_at.to_rfc3339(), batch.updated_at.to_rfc3339()
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn enqueue_agent_wake(
        &mut self,
        wake: &AgentWake,
    ) -> Result<(AgentWake, Option<EventEnvelope>), StorageError> {
        if let Some(existing) = self
            .connection
            .query_row(
                "SELECT wake_json FROM agent_wakes WHERE idempotency_key=?1",
                [&wake.idempotency_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok((serde_json::from_str(&existing)?, None));
        }
        let event = EventEnvelope::new(
            EventKind::AgentWakeUpdated,
            None,
            serde_json::json!({"conversation_id": wake.conversation_id, "wake": wake}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_wakes(wake_id, idempotency_key, conversation_id, batch_id, target, kind, state, claimed_until, wake_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                wake.id.to_string(), wake.idempotency_key, wake.conversation_id.to_string(),
                wake.coordination_batch_id.to_string(), serde_json::to_string(&wake.target)?.trim_matches('"'),
                serde_json::to_string(&wake.kind)?.trim_matches('"'), serde_json::to_string(&wake.state)?.trim_matches('"'),
                wake.claimed_until.map(|value| value.to_rfc3339()), serde_json::to_string(wake)?,
                wake.created_at.to_rfc3339(), wake.updated_at.to_rfc3339()
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((wake.clone(), Some(event)))
    }

    pub fn save_agent_wake(&mut self, wake: &AgentWake) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::AgentWakeUpdated,
            None,
            serde_json::json!({"conversation_id": wake.conversation_id, "wake": wake}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE agent_wakes SET state=?2, claimed_until=?3, wake_json=?4, updated_at=?5 WHERE wake_id=?1",
            params![wake.id.to_string(), serde_json::to_string(&wake.state)?.trim_matches('"'), wake.claimed_until.map(|value| value.to_rfc3339()), serde_json::to_string(wake)?, wake.updated_at.to_rfc3339()],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn coordination_batch(
        &self,
        batch_id: CoordinationBatchId,
    ) -> Result<Option<CoordinationBatch>, StorageError> {
        self.connection
            .query_row(
                "SELECT batch_json FROM coordination_batches WHERE batch_id=?1",
                [batch_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn agent_wake(&self, wake_id: AgentWakeId) -> Result<Option<AgentWake>, StorageError> {
        self.connection
            .query_row(
                "SELECT wake_json FROM agent_wakes WHERE wake_id=?1",
                [wake_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn pending_agent_wakes(&self) -> Result<Vec<AgentWake>, StorageError> {
        let now = Utc::now().to_rfc3339();
        let mut statement = self.connection.prepare(
            "SELECT wake_json FROM agent_wakes WHERE state='pending' OR (state='claimed' AND claimed_until IS NOT NULL AND claimed_until < ?1) ORDER BY created_at, wake_id",
        )?;
        statement
            .query_map([now], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    pub fn save_conversation_preview(
        &mut self,
        preview: &ConversationPreview,
    ) -> Result<EventEnvelope, StorageError> {
        let event = EventEnvelope::new(
            EventKind::ConversationPreviewUpdated,
            None,
            serde_json::json!({"conversation_id": preview.conversation_id, "preview": preview}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO conversation_previews(preview_id, conversation_id, batch_id, delegation_id, project_id, state, preview_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(preview_id) DO UPDATE SET state=excluded.state, preview_json=excluded.preview_json, updated_at=excluded.updated_at",
            params![
                preview.id.to_string(), preview.conversation_id.to_string(),
                preview.coordination_batch_id.to_string(), preview.delegation_id.to_string(),
                preview.project_id.to_string(), serde_json::to_string(&preview.state)?.trim_matches('"'),
                serde_json::to_string(preview)?, preview.created_at.to_rfc3339(), preview.updated_at.to_rfc3339()
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn conversation_preview(
        &self,
        preview_id: ConversationPreviewId,
    ) -> Result<Option<ConversationPreview>, StorageError> {
        self.connection
            .query_row(
                "SELECT preview_json FROM conversation_previews WHERE preview_id=?1",
                [preview_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn conversation_preview_for_delegation(
        &self,
        delegation_id: mcg_domain::AgentDelegationId,
    ) -> Result<Option<ConversationPreview>, StorageError> {
        self.connection
            .query_row(
                "SELECT preview_json FROM conversation_previews WHERE delegation_id=?1",
                [delegation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn stop_ephemeral_conversation_previews(&mut self) -> Result<u64, StorageError> {
        let mut previews = {
            let mut statement = self
                .connection
                .prepare("SELECT preview_json FROM conversation_previews WHERE state='running'")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| {
                    serde_json::from_str::<ConversationPreview>(&row?).map_err(StorageError::from)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut stopped = 0_u64;
        for preview in &mut previews {
            preview.state = ConversationPreviewState::Stopped;
            preview.safe_url = None;
            preview.last_error = Some("MCG restarted; open the preview to start it again".into());
            preview.updated_at = Utc::now();
            self.save_conversation_preview(preview)?;
            stopped = stopped.saturating_add(1);
        }
        Ok(stopped)
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
        let author_role = transaction
            .query_row(
                "SELECT json_extract(session_json, '$.kind') FROM conversation_agent_sessions WHERE session_id=?1",
                [entry.author_session_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| "agent".into());
        index_conversation_context(
            &transaction,
            &entry.id.to_string(),
            entry.conversation_id,
            entry.delegation_id,
            &author_role,
            "blackboard",
            entry.created_at,
            &entry.content,
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
        transaction.execute(
            "DELETE FROM worktree_baseline_entries WHERE worktree_id=?1",
            [worktree.id.to_string()],
        )?;
        for entry in &worktree.baseline_entries {
            transaction.execute(
                "INSERT INTO worktree_baseline_entries(worktree_id, normalized_path, source_kind, size_bytes, file_mode, sha256)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    worktree.id.to_string(),
                    entry.normalized_path,
                    serde_json::to_string(&entry.source_kind)?,
                    i64::try_from(entry.size).map_err(|_| StorageError::InvalidData("baseline file size exceeds SQLite range".into()))?,
                    i64::from(entry.mode),
                    entry.sha256,
                ],
            )?;
        }
        transaction.execute(
            "DELETE FROM worktree_baseline_exclusions WHERE worktree_id=?1",
            [worktree.id.to_string()],
        )?;
        for exclusion in &worktree.baseline_exclusions {
            transaction.execute(
                "INSERT INTO worktree_baseline_exclusions(worktree_id, normalized_path, reason, review_blocker)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    worktree.id.to_string(),
                    exclusion.normalized_path,
                    exclusion.reason,
                    exclusion.review_blocker,
                ],
            )?;
        }
        if let Some(journal) = &worktree.integration_journal {
            transaction.execute(
                "INSERT INTO integration_journals(journal_id, delegation_id, worktree_id, state, journal_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(journal_id) DO UPDATE SET state=excluded.state, journal_json=excluded.journal_json, updated_at=excluded.updated_at",
                params![
                    journal.id.to_string(),
                    journal.delegation_id.to_string(),
                    journal.worktree_id.to_string(),
                    serde_json::to_string(&journal.state)?,
                    serde_json::to_string(journal)?,
                    journal.created_at.to_rfc3339(),
                    journal.updated_at.to_rfc3339(),
                ],
            )?;
        }
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Persists one governed Codex cloud-diff application and its bounded patch atomically.
    pub fn save_cloud_diff_apply(
        &mut self,
        apply: &CloudDiffApply,
        patch: &[u8],
    ) -> Result<EventEnvelope, StorageError> {
        if patch.len() > 16 * 1_024 * 1_024 {
            return Err(StorageError::InvalidData(
                "cloud diff patch exceeds the 16 MiB storage limit".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::CloudDiffApplyUpdated,
            None,
            serde_json::json!({"project_id": apply.project_id, "apply": apply}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO cloud_diff_applies(apply_id, project_id, task_id, state, apply_json, patch_blob, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(apply_id) DO UPDATE SET state=excluded.state, apply_json=excluded.apply_json, patch_blob=excluded.patch_blob, updated_at=excluded.updated_at",
            params![
                apply.id.to_string(),
                apply.project_id.to_string(),
                apply.task_id,
                serde_json::to_string(&apply.state)?,
                serde_json::to_string(apply)?,
                patch,
                apply.created_at.to_rfc3339(),
                apply.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    pub fn cloud_diff_apply(
        &self,
        apply_id: CloudDiffApplyId,
    ) -> Result<Option<(CloudDiffApply, Vec<u8>)>, StorageError> {
        self.connection
            .query_row(
                "SELECT apply_json, patch_blob FROM cloud_diff_applies WHERE apply_id=?1",
                [apply_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?
            .map(|(value, patch)| {
                serde_json::from_str(&value)
                    .map(|apply| (apply, patch))
                    .map_err(StorageError::from)
            })
            .transpose()
    }

    pub fn cloud_diff_applies(
        &self,
        project_id: Option<ProjectId>,
    ) -> Result<Vec<CloudDiffApply>, StorageError> {
        let mut applies = Vec::new();
        if let Some(project_id) = project_id {
            let mut statement = self.connection.prepare(
                "SELECT apply_json FROM cloud_diff_applies WHERE project_id=?1 ORDER BY updated_at DESC, apply_id",
            )?;
            let rows =
                statement.query_map([project_id.to_string()], |row| row.get::<_, String>(0))?;
            for row in rows {
                applies.push(serde_json::from_str(&row?)?);
            }
        } else {
            let mut statement = self.connection.prepare(
                "SELECT apply_json FROM cloud_diff_applies ORDER BY updated_at DESC, apply_id",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                applies.push(serde_json::from_str(&row?)?);
            }
        }
        Ok(applies)
    }

    /// Appends secret-free credential lifecycle evidence. Bearer and bootstrap values are never
    /// accepted by this interface, preventing authority material from entering durable storage.
    pub fn append_agent_credential_audit(
        &mut self,
        audit: &AgentCredentialAudit,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO agent_credential_audit(audit_id, conversation_id, session_id, delegation_id, project_id, kind, nonce_sha256, audit_json, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                audit.id.to_string(),
                audit.conversation_id.to_string(),
                audit.session_id.to_string(),
                audit.delegation_id.map(|id| id.to_string()),
                audit.project_id.to_string(),
                serde_json::to_string(&audit.kind)?,
                audit.nonce_sha256,
                serde_json::to_string(audit)?,
                audit.occurred_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Claims a retry idempotency key, returning the first delegation identity for duplicates.
    pub fn claim_delegation_retry(
        &mut self,
        conversation_id: ConversationId,
        prior_delegation_id: mcg_domain::AgentDelegationId,
        idempotency_key: &str,
        new_delegation_id: mcg_domain::AgentDelegationId,
        created_at: DateTime<Utc>,
    ) -> Result<mcg_domain::AgentDelegationId, StorageError> {
        let key = idempotency_key.trim();
        if key.is_empty() || key.len() > 200 {
            return Err(StorageError::InvalidData(
                "retry idempotency key must contain 1 to 200 characters".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO delegation_retry_requests(conversation_id, prior_delegation_id, idempotency_key, new_delegation_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(conversation_id, prior_delegation_id, idempotency_key) DO NOTHING",
            params![
                conversation_id.to_string(),
                prior_delegation_id.to_string(),
                key,
                new_delegation_id.to_string(),
                created_at.to_rfc3339(),
            ],
        )?;
        let claimed: String = self.connection.query_row(
            "SELECT new_delegation_id FROM delegation_retry_requests
             WHERE conversation_id=?1 AND prior_delegation_id=?2 AND idempotency_key=?3",
            params![
                conversation_id.to_string(),
                prior_delegation_id.to_string(),
                key
            ],
            |row| row.get(0),
        )?;
        claimed.parse().map_err(|_| {
            StorageError::InvalidData("stored retry delegation identity is invalid".into())
        })
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
            context_checkpoints: rows(
                &self.connection,
                "SELECT checkpoint_json FROM agent_context_checkpoints WHERE conversation_id=?1 ORDER BY created_at, checkpoint_id",
                conversation_id,
            )?,
            coordination_batches: rows(
                &self.connection,
                "SELECT batch_json FROM coordination_batches WHERE conversation_id=?1 ORDER BY created_at, batch_id",
                conversation_id,
            )?,
            wakes: rows(
                &self.connection,
                "SELECT wake_json FROM agent_wakes WHERE conversation_id=?1 ORDER BY created_at, wake_id",
                conversation_id,
            )?,
            previews: rows(
                &self.connection,
                "SELECT preview_json FROM conversation_previews WHERE conversation_id=?1 ORDER BY created_at, preview_id",
                conversation_id,
            )?,
        })
    }

    pub fn agent_delegation(
        &self,
        delegation_id: mcg_domain::AgentDelegationId,
    ) -> Result<Option<AgentDelegation>, StorageError> {
        self.connection
            .query_row(
                "SELECT delegation_json FROM agent_delegations WHERE delegation_id=?1",
                [delegation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub fn managed_worktree_for_delegation(
        &self,
        delegation_id: mcg_domain::AgentDelegationId,
    ) -> Result<Option<ManagedWorktree>, StorageError> {
        self.connection
            .query_row(
                "SELECT worktree_json FROM managed_worktrees WHERE delegation_id=?1",
                [delegation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    /// Lists every durable managed worktree, including retained work that belongs to an unloaded
    /// or archived conversation.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried or a stored projection is invalid.
    pub fn managed_worktrees(&self) -> Result<Vec<ManagedWorktree>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT worktree_json FROM managed_worktrees ORDER BY updated_at DESC, worktree_id",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
            .collect()
    }

    pub fn search_conversation_context(
        &self,
        conversation_id: ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SharedContextHit>, StorageError> {
        let expression = query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .take(12)
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        if expression.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(
            "SELECT source_id, delegation_id, author_role, source_kind, created_at,
                    snippet(conversation_context_fts, 6, '', '', ' … ', 24)
             FROM conversation_context_fts
             WHERE conversation_context_fts MATCH ?1 AND conversation_id=?2
             ORDER BY rank LIMIT ?3",
        )?;
        statement
            .query_map(
                params![
                    expression,
                    conversation_id.to_string(),
                    limit.clamp(1, 100) as i64
                ],
                |row| {
                    let delegation_id = row
                        .get::<_, Option<String>>(1)?
                        .map(|value| value.parse())
                        .transpose()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    let source_kind = match row.get::<_, String>(3)?.as_str() {
                        "conversation_message" => SharedContextSourceKind::ConversationMessage,
                        "agent_message" => SharedContextSourceKind::AgentMessage,
                        "blackboard" => SharedContextSourceKind::Blackboard,
                        _ => SharedContextSourceKind::ProjectContext,
                    };
                    let created_at = DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?
                        .with_timezone(&Utc);
                    Ok(SharedContextHit {
                        source_id: row.get(0)?,
                        conversation_id,
                        delegation_id,
                        author_role: row.get(2)?,
                        source_kind,
                        created_at,
                        snippet: row.get(5)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn recover_conversation_team_work(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let delegations = {
            let mut statement = self.connection.prepare(
                "SELECT delegation_json FROM agent_delegations ORDER BY created_at, delegation_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<AgentDelegation>, _>>()?
        };
        let mut recovered = 0_u64;
        for mut delegation in delegations
            .iter()
            .filter(|delegation| {
                !delegation.state.is_terminal()
                    || delegation.state == mcg_domain::AgentDelegationState::Reported
            })
            .cloned()
        {
            if delegation.coordination_batch_id.is_none() {
                let origin_turn = self.connection.query_row(
                    "SELECT turn_json FROM conversation_turns WHERE conversation_id=?1 AND created_at<=?2 ORDER BY created_at DESC, turn_id DESC LIMIT 1",
                    params![delegation.conversation_id.to_string(), delegation.created_at.to_rfc3339()],
                    |row| row.get::<_, String>(0),
                ).optional()?.map(|json| serde_json::from_str::<ConversationTurn>(&json)).transpose()?;
                let coordinator = self.connection.query_row(
                    "SELECT session_json FROM conversation_agent_sessions WHERE conversation_id=?1 AND json_extract(session_json, '$.kind')='coordinator' ORDER BY created_at DESC, session_id DESC LIMIT 1",
                    [delegation.conversation_id.to_string()],
                    |row| row.get::<_, String>(0),
                ).optional()?.map(|json| serde_json::from_str::<ConversationAgentSession>(&json)).transpose()?;
                if let (Some(origin_turn), Some(coordinator)) = (origin_turn, coordinator) {
                    let batch_id = CoordinationBatchId::new();
                    let batch = CoordinationBatch {
                        schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
                        id: batch_id,
                        conversation_id: delegation.conversation_id,
                        origin_turn_id: origin_turn.id,
                        coordinator_session_id: coordinator.id,
                        delivery_target: mcg_domain::ConversationDeliveryTarget::CustomProject,
                        delivery_rationale: Some("Recovered legacy delegation; delivery target was not recorded".into()),
                        site_owner_session_id: None,
                        state: CoordinationBatchState::RecoveryRequired,
                        revision: 1,
                        delegation_ids: vec![delegation.id],
                        coordinator_turn_count: 0,
                        coordinator_approximate_tokens: 0,
                        coordinator_bypassed: false,
                        routing_rationale: Some("Recovered legacy coordination batch".into()),
                        owner_intervention_count: 0,
                        last_error: Some("Legacy schema-42 work requires explicit delivery recovery after restart.".into()),
                        created_at: delegation.created_at,
                        updated_at: now,
                    };
                    self.save_coordination_batch(&batch)?;
                    let wake = AgentWake {
                        schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
                        id: AgentWakeId::new(),
                        idempotency_key: format!("recovery:{batch_id}:final_review"),
                        conversation_id: delegation.conversation_id,
                        coordination_batch_id: batch_id,
                        delegation_id: Some(delegation.id),
                        source_message_id: None,
                        target_session_id: coordinator.id,
                        target: AgentWakeTarget::Coordinator,
                        kind: AgentWakeKind::FinalReview,
                        state: AgentWakeState::RecoveryRequired,
                        attempt_count: 0,
                        claimed_until: None,
                        delegation_revisions: vec![(delegation.id, delegation.revision)],
                        payload: "Legacy delegation retained for explicit review; no accepted model turn was replayed.".into(),
                        last_error: Some("origin reconstructed during schema-43 recovery".into()),
                        created_at: now,
                        updated_at: now,
                    };
                    self.enqueue_agent_wake(&wake)?;
                    delegation.coordination_batch_id = Some(batch_id);
                    delegation.origin_turn_id = Some(origin_turn.id);
                } else {
                    delegation.error = Some("Delivery recovery could not identify the originating owner turn. Isolated work is retained and was not attached to another conversation turn.".into());
                }
            }

            if matches!(
                delegation.state,
                mcg_domain::AgentDelegationState::Requested
                    | mcg_domain::AgentDelegationState::Preparing
                    | mcg_domain::AgentDelegationState::Running
                    | mcg_domain::AgentDelegationState::Waiting
                    | mcg_domain::AgentDelegationState::Integrating
            ) {
                delegation.state = mcg_domain::AgentDelegationState::Blocked;
                delegation.error = Some(
                    "MCG restarted while this coding delegation was active. The accepted work was not replayed; isolated work is retained for review.".into(),
                );
            }
            delegation.revision = delegation.revision.saturating_add(1);
            delegation.updated_at = now;
            self.save_agent_delegation(&delegation)?;
            if let Some(mut worktree) = self.managed_worktree_for_delegation(delegation.id)?
                && !matches!(worktree.state, mcg_domain::ManagedWorktreeState::Removed)
            {
                worktree.state = mcg_domain::ManagedWorktreeState::Retained;
                worktree.revision = worktree.revision.saturating_add(1);
                worktree.retained_reason = Some("runtime restart recovery boundary".into());
                worktree.updated_at = now;
                self.save_managed_worktree(&worktree)?;
            }
            recovered = recovered.saturating_add(1);
        }

        // Older runtimes could leave a batch indefinitely `running` when every Coder
        // failed before worktree preparation completed. No accepted work exists to
        // replay in this case, so make the failure visible deterministically and close
        // the batch instead of requiring an owner to discover a wedged receipt.
        let nonterminal_batches = {
            let mut statement = self.connection.prepare(
                "SELECT batch_json FROM coordination_batches
                 WHERE state NOT IN ('completed', 'failed')
                 ORDER BY created_at, batch_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<CoordinationBatch>, _>>()?
        };
        for mut batch in nonterminal_batches {
            let batch_delegations = delegations
                .iter()
                .filter(|item| item.coordination_batch_id == Some(batch.id))
                .collect::<Vec<_>>();
            let effective_ids = effective_delegation_attempt_ids(
                batch_delegations
                    .iter()
                    .map(|item| (item.id, item.retry_of_id)),
            );
            let effective = batch_delegations
                .iter()
                .filter(|item| effective_ids.contains(&item.id))
                .copied()
                .collect::<Vec<_>>();
            if effective.is_empty()
                || !effective.iter().all(|item| {
                    matches!(
                        item.state,
                        mcg_domain::AgentDelegationState::Failed
                            | mcg_domain::AgentDelegationState::Cancelled
                    )
                })
            {
                continue;
            }
            let failure_summary = effective
                .iter()
                .filter_map(|item| item.error.as_deref())
                .take(3)
                .collect::<Vec<_>>()
                .join("; ");
            let chat_session = self
                .connection
                .query_row(
                    "SELECT session_json FROM conversation_agent_sessions
                 WHERE conversation_id=?1 AND json_extract(session_json, '$.kind')='chat'
                 ORDER BY created_at, session_id LIMIT 1",
                    [batch.conversation_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|json| serde_json::from_str::<ConversationAgentSession>(&json))
                .transpose()?;
            if let Some(chat_session) = chat_session {
                let wake = AgentWake {
                    schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
                    id: AgentWakeId::new(),
                    idempotency_key: format!("recovery:{}:terminal_failure_chat", batch.id),
                    conversation_id: batch.conversation_id,
                    coordination_batch_id: batch.id,
                    delegation_id: None,
                    source_message_id: None,
                    target_session_id: chat_session.id,
                    target: AgentWakeTarget::Chat,
                    kind: AgentWakeKind::ChatCompletion,
                    state: AgentWakeState::Completed,
                    attempt_count: 0,
                    claimed_until: None,
                    delegation_revisions: effective
                        .iter()
                        .map(|item| (item.id, item.revision))
                        .collect(),
                    payload: "Deterministic restart recovery surfaced a terminal Coder failure."
                        .into(),
                    last_error: Some(
                        "all effective Coders were already terminal before restart".into(),
                    ),
                    created_at: now,
                    updated_at: now,
                };
                let (_, wake_event) = self.enqueue_agent_wake(&wake)?;
                if wake_event.is_some() {
                    self.save_conversation_message(&ConversationMessage {
                        schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
                        id: ConversationMessageId::new(),
                        conversation_id: batch.conversation_id,
                        turn_id: batch.origin_turn_id,
                        role: mcg_domain::ConversationRole::Assistant,
                        content: format!(
                            "The coding batch could not start or complete. No accepted Coder work was replayed. {}",
                            if failure_summary.is_empty() {
                                "Inspect the retained delegation evidence for details.".into()
                            } else {
                                format!("Failure: {failure_summary}")
                            }
                        ),
                        mentions: Vec::new(),
                        codex_item_id: None,
                        created_at: now,
                    })?;
                }
            }
            batch.state = CoordinationBatchState::Failed;
            batch.revision = batch.revision.saturating_add(1);
            batch.last_error = Some(if failure_summary.is_empty() {
                "all effective Coding Agents failed before completion".into()
            } else {
                failure_summary
            });
            batch.updated_at = now;
            self.save_coordination_batch(&batch)?;
            recovered = recovered.saturating_add(1);
        }

        let wakes = {
            let mut statement = self.connection.prepare(
                "SELECT wake_json FROM agent_wakes WHERE state IN ('pending', 'claimed') ORDER BY created_at, wake_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<AgentWake>, _>>()?
        };
        for mut wake in wakes.iter().cloned() {
            if wake.kind == AgentWakeKind::ChatCompletion
                && let Some(mut batch) = self.coordination_batch(wake.coordination_batch_id)?
            {
                let visible_exists = self.connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND turn_id=?2 AND role='assistant' AND created_at>=?3)",
                    params![wake.conversation_id.to_string(), batch.origin_turn_id.to_string(), wake.created_at.to_rfc3339()],
                    |row| row.get::<_, bool>(0),
                )?;
                if !visible_exists {
                    let message = ConversationMessage {
                        schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
                        id: ConversationMessageId::new(),
                        conversation_id: wake.conversation_id,
                        turn_id: batch.origin_turn_id,
                        role: mcg_domain::ConversationRole::Assistant,
                        content: "The agent team finished, but MCG restarted before the Chat Agent could safely synthesize the result. The durable inline Coordinator and Coding Agent cards contain the completed report and evidence.".into(),
                        mentions: Vec::new(),
                        codex_item_id: None,
                        created_at: now,
                    };
                    self.save_conversation_message(&message)?;
                }
                wake.state = AgentWakeState::Completed;
                wake.claimed_until = None;
                wake.last_error =
                    Some("deterministic restart fallback made the callback durably visible".into());
                wake.updated_at = now;
                self.save_agent_wake(&wake)?;
                batch.state = CoordinationBatchState::Completed;
                batch.revision = batch.revision.saturating_add(1);
                batch.last_error.clone_from(&wake.last_error);
                batch.updated_at = now;
                self.save_coordination_batch(&batch)?;
                recovered = recovered.saturating_add(1);
                continue;
            }
            wake.state = AgentWakeState::RecoveryRequired;
            wake.claimed_until = None;
            wake.last_error = Some("Service restarted before callback completion; accepted model work was not replayed automatically.".into());
            wake.updated_at = now;
            self.save_agent_wake(&wake)?;
            if let Some(mut batch) = self.coordination_batch(wake.coordination_batch_id)?
                && !matches!(
                    batch.state,
                    CoordinationBatchState::Completed | CoordinationBatchState::Failed
                )
            {
                batch.state = CoordinationBatchState::RecoveryRequired;
                batch.revision = batch.revision.saturating_add(1);
                batch.last_error.clone_from(&wake.last_error);
                batch.updated_at = now;
                self.save_coordination_batch(&batch)?;
            }
            recovered = recovered.saturating_add(1);
        }

        // A failed retry attempt remains durable history, but it must not poison a later
        // successful descendant. Older runtimes finalized batches by inspecting every attempt.
        // Repair only when every effective leaf is authoritatively successful and a completed
        // Chat callback already made that generation visible; this never invents or replays work.
        let failed_batches = {
            let mut statement = self.connection.prepare(
                "SELECT batch_json FROM coordination_batches WHERE state='failed' ORDER BY created_at, batch_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<CoordinationBatch>, _>>()?
        };
        for mut batch in failed_batches {
            let batch_delegations = delegations
                .iter()
                .filter(|item| item.coordination_batch_id == Some(batch.id))
                .collect::<Vec<_>>();
            let effective_ids = effective_delegation_attempt_ids(
                batch_delegations
                    .iter()
                    .map(|item| (item.id, item.retry_of_id)),
            );
            let effective_success = !effective_ids.is_empty()
                && batch_delegations
                    .iter()
                    .filter(|item| effective_ids.contains(&item.id))
                    .all(|item| {
                        matches!(
                            item.state,
                            mcg_domain::AgentDelegationState::Reported
                                | mcg_domain::AgentDelegationState::Completed
                        ) && item.governance.outcome.successful
                            && item.integration_ready
                            && item.unresolved_issues.is_empty()
                    });
            let completed_callbacks = {
                let mut statement = self.connection.prepare(
                    "SELECT wake_json FROM agent_wakes WHERE batch_id=?1 AND kind='chat_completion' AND state='completed' ORDER BY created_at, wake_id",
                )?;
                statement
                    .query_map([batch.id.to_string()], |row| row.get::<_, String>(0))?
                    .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                    .collect::<Result<Vec<AgentWake>, _>>()?
            };
            let callback_visible = completed_callbacks.iter().any(|wake| {
                wake.coordination_batch_id == batch.id
                    && effective_ids.iter().all(|id| {
                        wake.delegation_revisions
                            .iter()
                            .any(|(wake_id, _)| wake_id == id)
                    })
            });
            if effective_success && callback_visible {
                batch.state = CoordinationBatchState::Completed;
                batch.revision = batch.revision.saturating_add(1);
                batch.last_error = Some(
                    "Recovered successful retry lineage; superseded failed attempts remain in history."
                        .into(),
                );
                batch.updated_at = now;
                self.save_coordination_batch(&batch)?;
                recovered = recovered.saturating_add(1);
            }
        }
        Ok(recovered)
    }

    /// Settles ordinary Chat turns and Chat-agent session states left active by a prior
    /// service process. The restarted process cannot prove completion for an accepted Codex
    /// turn and must never resend it automatically; a turn that was never durably accepted is
    /// safe for the owner to retry.
    pub fn recover_conversation_chat_work(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let turns = {
            let mut statement = self.connection.prepare(
                "SELECT turn_json FROM conversation_turns WHERE state IN ('pending', 'streaming', 'waiting_for_approval', 'recovering') ORDER BY created_at, turn_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<ConversationTurn>, _>>()?
        };
        let mut recovered = 0_u64;
        for mut turn in turns {
            if turn.codex_turn_id.is_some() {
                turn.state = mcg_domain::ConversationTurnState::Indeterminate;
                turn.error = Some(
                    "MCG restarted after Codex accepted this conversation turn, but completion could not be proven. It was not replayed automatically; retry as a new message if needed."
                        .into(),
                );
            } else {
                turn.state = mcg_domain::ConversationTurnState::Failed;
                turn.error = Some(
                    "MCG restarted before this conversation turn was durably accepted. It is safe to retry."
                        .into(),
                );
            }
            turn.updated_at = now;
            self.save_conversation_turn(&turn)?;
            recovered = recovered.saturating_add(1);
        }

        let sessions = {
            let mut statement = self.connection.prepare(
                "SELECT session_json FROM conversation_agent_sessions WHERE json_extract(session_json, '$.kind') = 'chat' AND json_extract(session_json, '$.state') IN ('starting', 'working', 'waiting') ORDER BY created_at, session_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| serde_json::from_str(&row?).map_err(StorageError::from))
                .collect::<Result<Vec<ConversationAgentSession>, _>>()?
        };
        for mut session in sessions {
            session.state = mcg_domain::ConversationAgentSessionState::Ready;
            session.updated_at = now;
            self.save_conversation_agent_session(&session)?;
        }
        Ok(recovered)
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

    pub fn codex_thread_binding_for_delegation(
        &self,
        delegation_id: mcg_domain::AgentDelegationId,
    ) -> Result<Option<CodexThreadBinding>, StorageError> {
        self.connection
            .query_row(
                "SELECT binding_json FROM codex_thread_bindings WHERE binding_key = ?1",
                [format!("delegation:{delegation_id}")],
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
                mentions: Vec::new(),
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
        let mut statement = self.connection.prepare(
            "SELECT task_json FROM tasks
                 WHERE task_id NOT IN (SELECT task_id FROM hidden_delegation_tasks)
                 ORDER BY updated_at DESC, task_id ASC",
        )?;
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
                updated_at = excluded.updated_at,
                archived_at = NULL,
                removed_at = NULL",
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

    /// Loads a catalog project by its canonical root, including archived and removed entries.
    /// This lets rediscovery revive the stable project identity instead of colliding with the
    /// root uniqueness constraint or silently creating a second history.
    pub fn catalog_project_by_root(
        &self,
        root: &str,
    ) -> Result<Option<ProjectModel>, StorageError> {
        let json = self
            .connection
            .query_row(
                "SELECT project_json FROM projects WHERE root = ?1",
                [root],
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

    /// Lists active current project projections in stable root order.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn projects(&self) -> Result<Vec<ProjectModel>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT project_json FROM projects
                 WHERE archived_at IS NULL AND removed_at IS NULL
                 ORDER BY root, project_id",
        )?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    /// Lists archived project projections in stable root order.
    ///
    /// # Errors
    ///
    /// Returns an error when stored JSON is invalid or the query fails.
    pub fn archived_projects(&self) -> Result<Vec<ProjectModel>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT project_json FROM projects
             WHERE archived_at IS NOT NULL AND removed_at IS NULL
             ORDER BY root, project_id",
        )?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    /// Archives or restores a project catalog entry without touching its directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the project is unknown, removed, or storage cannot commit.
    pub fn archive_project(
        &mut self,
        project_id: ProjectId,
        archived: bool,
    ) -> Result<ProjectModel, StorageError> {
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        let changed = self.connection.execute(
            "UPDATE projects
             SET archived_at = ?2, updated_at = ?3
             WHERE project_id = ?1 AND removed_at IS NULL",
            params![
                project_id.to_string(),
                archived.then(|| Utc::now().to_rfc3339()),
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        Ok(project)
    }

    /// Removes a project from the MCG catalog without deleting source files or remote data.
    ///
    /// # Errors
    ///
    /// Returns an error when the project is unknown, already removed, or storage cannot commit.
    pub fn remove_project(&mut self, project_id: ProjectId) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let changed = self.connection.execute(
            "UPDATE projects
             SET archived_at = NULL, removed_at = ?2, updated_at = ?2
             WHERE project_id = ?1 AND removed_at IS NULL",
            params![project_id.to_string(), now],
        )?;
        if changed != 1 {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        Ok(())
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

#[allow(clippy::too_many_arguments)]
fn index_conversation_context(
    transaction: &Transaction<'_>,
    source_id: &str,
    conversation_id: ConversationId,
    delegation_id: Option<mcg_domain::AgentDelegationId>,
    author_role: &str,
    source_kind: &str,
    created_at: DateTime<Utc>,
    content: &str,
) -> Result<(), StorageError> {
    transaction.execute(
        "DELETE FROM conversation_context_fts WHERE source_id=?1 AND source_kind=?2",
        params![source_id, source_kind],
    )?;
    transaction.execute(
        "INSERT INTO conversation_context_fts(source_id, conversation_id, delegation_id, author_role, source_kind, created_at, content)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            source_id,
            conversation_id.to_string(),
            delegation_id.map(|id| id.to_string()),
            author_role,
            source_kind,
            created_at.to_rfc3339(),
            content,
        ],
    )?;
    Ok(())
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
    fn schema_fifty_database_is_detected_and_migrated_to_root_ownership_schema() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        let project_root = directory.path().join("representative-project");
        std::fs::create_dir(&project_root).expect("project directory");
        std::fs::write(project_root.join("Cargo.toml"), "[package]\nname='fixture'")
            .expect("project marker");
        let project = ProjectDiscoverer::discover(&project_root, ProjectId::new())
            .expect("representative project");
        let conversation = Conversation::new("representative existing conversation", None);
        {
            let mut store = SqliteEventStore::open(&path).expect("current store");
            store
                .record_project_snapshot(&project)
                .expect("existing project");
            store
                .create_conversation(&conversation, Some("legacy-thread"))
                .expect("existing conversation");
            store
                .connection
                .execute_batch(
                    "DROP TABLE reconciliation_jobs;
                     DROP TABLE action_events;
                     DROP TABLE action_receipts;
                     DROP TABLE action_executions;
                     DROP TABLE action_envelopes;
                     DROP TABLE task_contract_mutations;
                     DROP TABLE workflow_events;
                     DROP TABLE workflow_attempts;
                     DROP TABLE task_contract_dependencies;
                     DROP TABLE task_contract_heads;
                     DROP TABLE task_contract_versions;
                     DROP TABLE root_ownership;
                     DELETE FROM mcg_schema WHERE version IN (51, 52, 53, 54);",
                )
                .expect("downgrade fixture to schema 50");
        }

        let before = inspect_database_before_root(&path).expect("read-only pre-Root inspection");
        assert_eq!(before.schema_version, 50);
        assert!(!before.has_root_ownership);

        let migrated = SqliteEventStore::open(&path).expect("migrate schema 50 database");
        assert_eq!(
            migrated
                .connection
                .query_row("SELECT MAX(version) FROM mcg_schema", [], |row| row
                    .get::<_, i64>(0))
                .expect("schema version"),
            STORAGE_SCHEMA_VERSION
        );
        assert!(
            migrated
                .root_ownership()
                .expect("ownership query")
                .is_none()
        );
        assert_eq!(
            migrated
                .conversation(conversation.id)
                .expect("conversation query"),
            Some(conversation)
        );
        assert_eq!(migrated.projects().expect("project query"), vec![project]);
    }

    #[test]
    fn authoritative_store_enforces_pragmas_and_allows_wal_readers_during_write() {
        use rusqlite::OpenFlags;

        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        let store = SqliteEventStore::open(&path).expect("authoritative store");
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .expect("journal mode")
                .to_ascii_lowercase(),
            "wal"
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .expect("foreign keys"),
            1
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))
                .expect("busy timeout"),
            5_000
        );

        store
            .connection
            .execute_batch("BEGIN IMMEDIATE")
            .expect("hold writer transaction");
        let reader_path = path.clone();
        let version = std::thread::spawn(move || {
            let reader = Connection::open_with_flags(
                reader_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .expect("read-only connection");
            reader
                .query_row("SELECT MAX(version) FROM mcg_schema", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("concurrent WAL read")
        })
        .join()
        .expect("reader thread");
        store
            .connection
            .execute_batch("ROLLBACK")
            .expect("release writer transaction");
        assert_eq!(version, STORAGE_SCHEMA_VERSION);
    }

    #[test]
    fn newer_authoritative_schema_is_rejected_without_mutation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("mcg.db");
        {
            let store = SqliteEventStore::open(&path).expect("current store");
            store
                .connection
                .execute(
                    "INSERT INTO mcg_schema(version) VALUES (?1)",
                    [STORAGE_SCHEMA_VERSION + 1],
                )
                .expect("newer schema fixture");
        }

        assert!(matches!(
            inspect_database_before_root(&path),
            Err(StorageError::InvalidData(message)) if message.contains("newer than supported")
        ));
        assert!(matches!(
            SqliteEventStore::open(&path),
            Err(StorageError::InvalidData(message)) if message.contains("newer than supported")
        ));
    }

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
    fn conversation_can_persist_chat_and_distinct_coder_thread_bindings() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let conversation = Conversation::new("two role threads", None);
        store
            .create_conversation(&conversation, None)
            .expect("conversation should persist");
        let common = |codex_thread_id: &str| CodexThreadBinding {
            conversation_id: Some(conversation.id),
            conversation_turn_id: None,
            run_id: None,
            agent_instance_id: None,
            agent_session_id: Some(mcg_domain::ConversationAgentSessionId::new()),
            delegation_id: None,
            role_id: None,
            organization_revision_id: None,
            workspace_id: None,
            codex_thread_id: codex_thread_id.into(),
            codex_turn_id: None,
            codex_item_id: None,
            codex_session_id: None,
            binary_version: "test".into(),
            protocol_version: "test".into(),
            ownership_source: Some("mcg".into()),
            manageable: true,
            last_reconciled_item_id: None,
            recovery_state: mcg_domain::CodexRecoveryState::Healthy,
        };
        let chat = common("chat-thread");
        store
            .save_codex_thread_binding(&chat)
            .expect("chat binding should persist");

        let delegation_id = mcg_domain::AgentDelegationId::new();
        let coder = CodexThreadBinding {
            delegation_id: Some(delegation_id),
            codex_thread_id: "coder-thread".into(),
            ..common("unused")
        };
        store
            .save_codex_thread_binding(&coder)
            .expect("coder binding may share the conversation");

        assert_eq!(
            store
                .codex_thread_binding_for_conversation(conversation.id)
                .expect("chat lookup"),
            Some(chat)
        );
        assert_eq!(
            store
                .codex_thread_binding_for_delegation(delegation_id)
                .expect("coder lookup"),
            Some(coder)
        );
        assert_eq!(
            store.codex_thread_bindings().expect("all bindings").len(),
            2
        );
    }

    #[test]
    fn cloud_diff_apply_and_patch_are_atomic_bounded_and_project_scoped() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]").expect("manifest");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover project");
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        store
            .record_project_snapshot(&project)
            .expect("record project");
        let now = Utc::now();
        let apply = CloudDiffApply {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: CloudDiffApplyId::new(),
            project_id,
            task_id: "task-123".into(),
            state: mcg_domain::CloudDiffApplyState::Ready,
            root: project_root
                .path()
                .join("shadow")
                .to_string_lossy()
                .into_owned(),
            branch: "mcg/cloud/fixture".into(),
            base_head: "abc123".into(),
            baseline_head: "abc123".into(),
            baseline_sha256: "a".repeat(64),
            baseline_entries: vec![],
            baseline_exclusions: vec![],
            changed_paths: vec!["src/lib.rs".into()],
            touched_paths: vec![],
            patch_sha256: Some("b".repeat(64)),
            output: Some("prepared".into()),
            error: None,
            revision: 2,
            created_at: now,
            updated_at: now,
        };
        let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n";
        let event = store
            .save_cloud_diff_apply(&apply, patch)
            .expect("save cloud diff");
        assert_eq!(event.kind, EventKind::CloudDiffApplyUpdated);
        assert_eq!(
            store.cloud_diff_apply(apply.id).expect("load cloud diff"),
            Some((apply.clone(), patch.to_vec()))
        );
        assert_eq!(
            store
                .cloud_diff_applies(Some(project_id))
                .expect("project applies"),
            vec![apply.clone()]
        );
        assert!(
            store
                .save_cloud_diff_apply(&apply, &vec![0; 16 * 1_024 * 1_024 + 1])
                .is_err()
        );
        assert_eq!(
            store.cloud_diff_apply(apply.id).expect("unchanged record"),
            Some((apply, patch.to_vec()))
        );
    }

    #[test]
    fn confirmed_conversation_tree_deletion_removes_content_and_retains_tombstones() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let parent = Conversation::new("private parent content", None);
        store
            .create_conversation(&parent, None)
            .expect("parent should persist");
        let mut child = Conversation::new("private child content", None);
        child.parent_conversation_id = Some(parent.id);
        child.visibility = mcg_domain::ConversationVisibility::Ephemeral;
        store
            .create_conversation(&child, None)
            .expect("child should persist");

        let (deleted, event) = store
            .delete_conversation_tree(parent.id, Utc::now())
            .expect("tree deletion should settle");
        assert_eq!(deleted.len(), 2);
        assert_eq!(event.kind, EventKind::ConversationDeleted);
        assert!(
            store
                .conversation(parent.id)
                .expect("parent lookup")
                .is_none()
        );
        assert!(
            store
                .conversation(child.id)
                .expect("child lookup")
                .is_none()
        );
        let tombstones: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM conversation_tombstones", [], |row| {
                row.get(0)
            })
            .expect("tombstone count");
        assert_eq!(tombstones, 2);
        let serialized_events: String = store
            .connection
            .query_row("SELECT group_concat(payload, '') FROM events", [], |row| {
                row.get(0)
            })
            .expect("remaining event payloads");
        assert!(!serialized_events.contains("private parent content"));
        assert!(!serialized_events.contains("private child content"));
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
            agent_session_id: None,
            delegation_id: None,
            codex_item_id: "command-1".into(),
            kind: mcg_domain::ConversationActivityKind::Command,
            state: mcg_domain::ConversationActivityState::Running,
            command: Some("cargo check".into()),
            cwd: Some("C:\\workspace".into()),
            output: Some("Checking".into()),
            exit_code: None,
            duration_ms: None,
            codex_payload: None,
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
    fn restart_recovery_settles_orphaned_chat_turns_and_releases_chat_session() {
        let mut store = SqliteEventStore::open_in_memory().expect("store should open");
        let conversation = Conversation::new("recover chat", None);
        store
            .create_conversation(&conversation, None)
            .expect("conversation should persist");
        let started_at = Utc::now();
        let pending = ConversationTurn {
            schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
            id: ConversationTurnId::new(),
            conversation_id: conversation.id,
            state: mcg_domain::ConversationTurnState::Streaming,
            execution_preferences: mcg_domain::ExecutionPreferences::default(),
            codex_thread_id: Some("chat-thread".into()),
            codex_turn_id: None,
            task_id: None,
            run_id: None,
            error: None,
            created_at: started_at,
            updated_at: started_at,
        };
        let mut accepted = pending.clone();
        accepted.id = ConversationTurnId::new();
        accepted.codex_turn_id = Some("accepted-turn".into());
        store
            .save_conversation_turn(&pending)
            .expect("pending turn");
        store
            .save_conversation_turn(&accepted)
            .expect("accepted turn");
        let chat = ConversationAgentSession {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::ConversationAgentSessionId::new(),
            conversation_id: conversation.id,
            role_id: mcg_domain::RoleId::new(),
            organization_revision_id: mcg_domain::OrganizationRevisionId::new(),
            kind: mcg_domain::ConversationAgentKind::Chat,
            display_name: "Chat Agent".into(),
            parent_session_id: None,
            state: mcg_domain::ConversationAgentSessionState::Working,
            codex_thread_id: Some("chat-thread".into()),
            model: Some("fixture".into()),
            prompt_revision: 1,
            created_at: started_at,
            updated_at: started_at,
        };
        store
            .save_conversation_agent_session(&chat)
            .expect("chat session");

        let recovered_at = started_at + chrono::Duration::seconds(1);
        assert_eq!(
            store
                .recover_conversation_chat_work(recovered_at)
                .expect("chat recovery"),
            2
        );
        let view = store
            .conversation_view(conversation.id)
            .expect("view")
            .expect("conversation");
        let retryable = view
            .turns
            .iter()
            .find(|turn| turn.codex_turn_id.is_none())
            .expect("retryable turn");
        let indeterminate = view
            .turns
            .iter()
            .find(|turn| turn.codex_turn_id.is_some())
            .expect("accepted turn");
        assert_eq!(retryable.state, mcg_domain::ConversationTurnState::Failed);
        assert!(
            retryable
                .error
                .as_deref()
                .is_some_and(|error| error.contains("safe to retry"))
        );
        assert_eq!(
            indeterminate.state,
            mcg_domain::ConversationTurnState::Indeterminate
        );
        assert!(
            indeterminate
                .error
                .as_deref()
                .is_some_and(|error| error.contains("not replayed"))
        );
        assert_eq!(
            view.team.sessions[0].state,
            mcg_domain::ConversationAgentSessionState::Ready
        );
        assert_eq!(
            store
                .recover_conversation_chat_work(recovered_at)
                .expect("idempotent chat recovery"),
            0
        );
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
            display_name: "Chat Agent".into(),
            parent_session_id: None,
            state: mcg_domain::ConversationAgentSessionState::Ready,
            codex_thread_id: Some("chat-thread".into()),
            model: Some("fixture".into()),
            prompt_revision: 3,
            created_at: now,
            updated_at: now,
        };
        let mut coder = chat.clone();
        let mut coordinator = chat.clone();
        coordinator.id = mcg_domain::ConversationAgentSessionId::new();
        coordinator.role_id = mcg_domain::RoleId::new();
        coordinator.kind = mcg_domain::ConversationAgentKind::Coordinator;
        coordinator.display_name = "Coordinator".into();
        coordinator.parent_session_id = Some(chat.id);
        coordinator.codex_thread_id = Some("coordinator-thread".into());
        coder.id = mcg_domain::ConversationAgentSessionId::new();
        coder.role_id = mcg_domain::RoleId::new();
        coder.kind = mcg_domain::ConversationAgentKind::Coder;
        coder.display_name = "Coding Agent Atlas".into();
        coder.parent_session_id = Some(coordinator.id);
        coder.codex_thread_id = Some("coder-thread".into());
        store
            .save_conversation_agent_session(&chat)
            .expect("chat session");
        store
            .save_conversation_agent_session(&coordinator)
            .expect("coordinator session");
        store
            .save_conversation_agent_session(&coder)
            .expect("coder session");
        let delegation = AgentDelegation {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::AgentDelegationId::new(),
            conversation_id: first.id,
            source_session_id: coordinator.id,
            coder_session_id: Some(coder.id),
            project_id: ProjectId::new(),
            organization_revision_id: chat.organization_revision_id,
            revision: 1,
            prompt_revision: chat.prompt_revision,
            permission_mode: None,
            state: mcg_domain::AgentDelegationState::Running,
            brief: mcg_domain::CodingDelegationBrief {
                objective: "change code".into(),
                scope: vec![],
                constraints: vec![],
                success_conditions: vec![],
                context_references: vec![],
            },
            governance: mcg_domain::DelegationGovernance::default(),
            task_id: None,
            run_id: None,
            agent_instance_id: None,
            agent_lease_id: None,
            worktree_id: None,
            retry_of_id: None,
            idempotency_key: None,
            coordination_batch_id: None,
            origin_turn_id: None,
            coordinator_decision: None,
            coordinator_rationale: None,
            coordinator_reviewed_at: None,
            summary: None,
            changed_files: vec![],
            checks: vec![],
            evidence: vec![],
            unresolved_issues: vec![],
            integration_ready: false,
            report_sha256: None,
            accepted_codex_item_id: None,
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
            from_session_id: coordinator.id,
            to_session_id: coder.id,
            kind: mcg_domain::AgentMessageKind::Delegation,
            room: mcg_domain::AgentRoomKind::Backstage,
            tool_name: None,
            correlation_id: Some(delegation.id.to_string()),
            content: "bounded brief".into(),
            created_at: now,
        };
        store.append_agent_message(&message).expect("message");
        let entry = BlackboardEntry {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::BlackboardEntryId::new(),
            conversation_id: first.id,
            delegation_id: Some(delegation.id),
            author_session_id: coordinator.id,
            kind: mcg_domain::BlackboardEntryKind::Goal,
            content: "shared goal".into(),
            source_references: vec![],
            supersedes_id: None,
            created_at: now,
        };
        store.append_blackboard_entry(&entry).expect("blackboard");

        let hits = store
            .search_conversation_context(first.id, "shared", 10)
            .expect("scoped context search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source_id, entry.id.to_string());
        assert_eq!(hits[0].author_role, "coordinator");
        assert_eq!(hits[0].delegation_id, Some(delegation.id));
        assert!(
            store
                .search_conversation_context(second.id, "shared", 10)
                .expect("unrelated context search")
                .is_empty()
        );

        let checkpoint = mcg_domain::AgentContextCheckpoint {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::AgentContextCheckpointId::new(),
            conversation_id: first.id,
            session_id: coordinator.id,
            room: mcg_domain::AgentRoomKind::Backstage,
            revision: 1,
            through_message_id: Some(message.id),
            summary: "Coordinator delegated bounded work to Coding Agent Atlas.".into(),
            source_references: vec![message.id.to_string(), entry.id.to_string()],
            approximate_tokens: 12,
            cause: mcg_domain::AgentContextCheckpointCause::Legacy,
            content_sha256: None,
            created_at: now,
        };
        store
            .save_agent_context_checkpoint(&checkpoint)
            .expect("context checkpoint");

        let first_team = store.conversation_team_state(first.id).expect("first team");
        assert_eq!(first_team.sessions.len(), 3);
        assert!(first_team.sessions.contains(&chat));
        assert!(first_team.sessions.contains(&coordinator));
        assert!(first_team.sessions.contains(&coder));
        assert_eq!(first_team.delegations, vec![delegation.clone()]);
        assert_eq!(first_team.messages, vec![message]);
        assert_eq!(first_team.blackboard, vec![entry]);
        assert_eq!(first_team.context_checkpoints, vec![checkpoint]);
        assert_eq!(
            store
                .conversation_team_state(second.id)
                .expect("second team"),
            ConversationTeamState::default()
        );

        let origin_turn = ConversationTurn {
            schema_version: mcg_domain::CONVERSATION_SCHEMA_VERSION,
            id: ConversationTurnId::new(),
            conversation_id: first.id,
            state: mcg_domain::ConversationTurnState::Completed,
            execution_preferences: mcg_domain::ExecutionPreferences::default(),
            codex_thread_id: None,
            codex_turn_id: None,
            task_id: None,
            run_id: None,
            error: None,
            created_at: now,
            updated_at: now,
        };
        store
            .save_conversation_turn(&origin_turn)
            .expect("origin turn");
        let request_message = AgentMessage {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::AgentMessageId::new(),
            conversation_id: first.id,
            delegation_id: None,
            from_session_id: chat.id,
            to_session_id: coordinator.id,
            kind: mcg_domain::AgentMessageKind::Delegation,
            room: mcg_domain::AgentRoomKind::Backstage,
            tool_name: None,
            correlation_id: None,
            content: "coordinate atomically".into(),
            created_at: now,
        };
        let batch = CoordinationBatch {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: CoordinationBatchId::new(),
            conversation_id: first.id,
            origin_turn_id: origin_turn.id,
            coordinator_session_id: coordinator.id,
            delivery_target: mcg_domain::ConversationDeliveryTarget::CustomProject,
            delivery_rationale: None,
            site_owner_session_id: None,
            state: CoordinationBatchState::Open,
            revision: 1,
            delegation_ids: vec![],
            coordinator_turn_count: 0,
            coordinator_approximate_tokens: 0,
            coordinator_bypassed: false,
            routing_rationale: None,
            owner_intervention_count: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        let wake = AgentWake {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: AgentWakeId::new(),
            idempotency_key: format!("{}:initial", batch.id),
            conversation_id: first.id,
            coordination_batch_id: batch.id,
            delegation_id: None,
            source_message_id: Some(request_message.id),
            target_session_id: coordinator.id,
            target: AgentWakeTarget::Coordinator,
            kind: AgentWakeKind::InitialRequest,
            state: AgentWakeState::Claimed,
            attempt_count: 1,
            claimed_until: Some(now + chrono::Duration::minutes(5)),
            delegation_revisions: vec![],
            payload: request_message.content.clone(),
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        let events = store
            .append_coordination_request(&request_message, &batch, &wake)
            .expect("atomic coordination request");
        assert_eq!(events.len(), 3);
        let coordinated = store
            .conversation_team_state(first.id)
            .expect("coordinated team");
        assert_eq!(coordinated.coordination_batches, vec![batch.clone()]);
        assert_eq!(coordinated.wakes, vec![wake]);

        let worktree = ManagedWorktree {
            schema_version: mcg_domain::CONVERSATION_TEAM_SCHEMA_VERSION,
            id: mcg_domain::ManagedWorktreeId::new(),
            conversation_id: first.id,
            delegation_id: delegation.id,
            project_id: delegation.project_id,
            root: "C:\\managed\\fixture".into(),
            branch: "codex/fixture".into(),
            base_head: "abc123".into(),
            baseline_head: Some("abc123".into()),
            baseline_sha256: "fixture-hash".into(),
            workspace_mode: mcg_domain::ManagedWorkspaceMode::GitWorktree,
            baseline_confidence: mcg_domain::BaselineConfidence::Verified,
            baseline_entries: vec![],
            baseline_exclusions: vec![],
            integration_journal: None,
            state: mcg_domain::ManagedWorktreeState::Ready,
            revision: 1,
            retained_reason: None,
            storage_bytes: 42,
            created_at: now,
            updated_at: now,
        };
        store
            .connection
            .execute(
                "INSERT INTO projects(project_id, root, current_snapshot_id, project_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    delegation.project_id.to_string(),
                    "C:\\fixture-project",
                    ProjectSnapshotId::new().to_string(),
                    "{}",
                    now.to_rfc3339(),
                ],
            )
            .expect("fixture project projection");
        store
            .save_managed_worktree(&worktree)
            .expect("managed worktree");
        assert_eq!(
            store.recover_conversation_team_work(now).expect("recovery"),
            2
        );
        let recovered = store
            .conversation_team_state(first.id)
            .expect("recovered team");
        assert_eq!(
            recovered.delegations[0].state,
            mcg_domain::AgentDelegationState::Blocked
        );
        assert_eq!(recovered.delegations[0].revision, 2);
        assert_eq!(
            recovered.worktrees[0].state,
            mcg_domain::ManagedWorktreeState::Retained
        );
        assert_eq!(recovered.worktrees[0].revision, 2);
        assert!(
            recovered.worktrees[0]
                .retained_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("restart"))
        );
        assert_eq!(
            recovered.coordination_batches[0].state,
            CoordinationBatchState::RecoveryRequired
        );
        assert_eq!(recovered.wakes[0].state, AgentWakeState::RecoveryRequired);
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
    fn project_archive_restore_and_removal_are_catalog_only() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]").expect("manifest");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover");
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        store
            .record_project_snapshot(&project)
            .expect("record project");

        assert_eq!(
            store.projects().expect("active projects"),
            vec![project.clone()]
        );
        store.archive_project(project_id, true).expect("archive");
        assert!(store.projects().expect("active projects").is_empty());
        assert_eq!(
            store.archived_projects().expect("archived projects"),
            vec![project.clone()]
        );
        assert!(project_root.path().join("Cargo.toml").is_file());

        store.archive_project(project_id, false).expect("restore");
        assert_eq!(
            store.projects().expect("restored projects"),
            vec![project.clone()]
        );
        store
            .remove_project(project_id)
            .expect("remove from catalog");
        assert!(store.projects().expect("active projects").is_empty());
        assert!(
            store
                .archived_projects()
                .expect("archived projects")
                .is_empty()
        );
        assert_eq!(
            store.project(project_id).expect("historical project"),
            Some(project.clone())
        );
        assert!(project_root.path().join("Cargo.toml").is_file());

        assert_eq!(
            store
                .catalog_project_by_root(&project.root)
                .expect("catalog project by root"),
            Some(project.clone())
        );
        let rediscovered =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("rediscover");
        store
            .record_project_snapshot(&rediscovered)
            .expect("revive removed project");
        assert_eq!(
            store.projects().expect("revived projects"),
            vec![rediscovered]
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
