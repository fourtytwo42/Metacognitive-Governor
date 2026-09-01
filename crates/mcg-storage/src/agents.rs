use std::str::FromStr;

use chrono::{DateTime, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentRun, AgentRunState,
    AgentState, AgentTurnCheckpoint, AgentTurnResult, AgentUsageRecord, AssignedModel,
    ConfidenceEstimate, EvaluationReport, ExperimentState, ModelCallEvidence,
    OrganizationAssignments, OrganizationGenome, OrganizationalMetrics, ShadowWorkspaceState,
    UsageKind, WorkPlan,
};
use mcg_contracts::{
    CompletionCertificate, CompletionVerdict, SourceRef, StableTypeId, VersionedPayload,
};
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, ExperimentId, OrganizationId, OrganizationRevisionId,
    ProjectSnapshotId, RunId, TaskEnvelope, TaskId, TaskState, ToolRequestId,
};
use mcg_events::{EventEnvelope, EventKind, TaskCreatedPayload, TaskStateChangedPayload};
use mcg_tools::{ApprovalRequest, ApprovalResolution, ApprovalState, ToolOutcome};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    SqliteEventStore, StorageError,
    invocations::commit_completion_certificate_in_transaction,
    workflows::{
        AgentTurnVerificationTarget, AgentTurnWorkflowSettlement,
        resume_attached_agent_approval_wait_in_transaction,
        settle_attached_agent_turn_workflow_in_transaction,
    },
};

const EVALUATION_REPORT_HASH_DOMAIN: &str = "MetaCog.EvaluationReport";

/// Produces the immutable content-addressed reference consumed by independent Verify.
///
/// # Errors
/// Returns an error when the report cannot be represented by canonical v1 evidence.
pub fn evaluation_report_source_ref(report: &EvaluationReport) -> Result<SourceRef, StorageError> {
    let source_type = StableTypeId::parse("mcg.evaluation_report.v1")
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let payload =
        VersionedPayload::canonical_v1(source_type.clone(), &serde_json::to_value(report)?)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(SourceRef {
        source_type,
        locator: report.id.to_string(),
        content_hash: payload
            .content_hash(EVALUATION_REPORT_HASH_DOMAIN)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    })
}

fn ensure_assignable_organization(
    store: &SqliteEventStore,
    organization_id: OrganizationId,
) -> Result<(), StorageError> {
    let organization = store.organization(organization_id)?.ok_or_else(|| {
        StorageError::InvalidData(format!("organization {organization_id} was not found"))
    })?;
    organization
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    for required in [
        mcg_agents::RoleKind::Executive,
        mcg_agents::RoleKind::Coder,
        mcg_agents::RoleKind::Evaluator,
    ] {
        if !organization.roles.iter().any(|role| role.kind == required) {
            return Err(StorageError::InvalidData(format!(
                "assignable organizations require a {required:?} role"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentRecoveryReport {
    pub runs_marked_recovering: u64,
    pub instances_expired: u64,
    pub leases_expired: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentTurnCommit {
    pub run: AgentRun,
    pub instance: AgentInstance,
    pub lease: AgentLease,
    pub usage: Vec<AgentUsageRecord>,
    pub result: AgentTurnResult,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTurnDisposition {
    Waiting,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceRunReceipt {
    pub task: TaskEnvelope,
    pub run: AgentRun,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskLaunchReceipt {
    pub task: TaskEnvelope,
    pub run: AgentRun,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentResumeReceipt {
    pub task: TaskEnvelope,
    pub run: AgentRun,
    pub instance: AgentInstance,
    pub lease: AgentLease,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerEvaluationCheckpointState {
    Running,
    WaitingForApproval,
    Verifying,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumerEvaluationCheckpoint {
    pub schema_version: u16,
    pub task_id: mcg_domain::TaskId,
    pub run_id: RunId,
    pub project_snapshot_id: ProjectSnapshotId,
    pub state: ConsumerEvaluationCheckpointState,
    pub next_condition_index: usize,
    pub attempt: u16,
    pub request_id: Option<ToolRequestId>,
    #[serde(default)]
    pub prepared_evaluation_id: Option<mcg_domain::EvaluationId>,
    #[serde(default)]
    pub prepared_evaluated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub batch_approved: bool,
    pub results: Vec<mcg_agents::ConditionResult>,
    pub evidence: Vec<mcg_agents::EvaluationEvidence>,
    pub regressions: Vec<String>,
    pub objective_failures: u32,
    #[serde(default)]
    pub unsupported: u32,
    pub executed: u32,
    pub updated_at: DateTime<Utc>,
}

/// Root-produced independent verification certificate bound to one compatibility lease fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerEvaluationCompletion {
    pub lease_id: AgentLeaseId,
    pub certificate: CompletionCertificate,
    pub expected_revision: u64,
    pub expected_generation: u64,
}

impl SqliteEventStore {
    /// Loads the durable objective-evaluation checkpoint for one consumer task.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted checkpoint JSON is invalid.
    pub fn consumer_evaluation_checkpoint(
        &self,
        task_id: mcg_domain::TaskId,
    ) -> Result<Option<ConsumerEvaluationCheckpoint>, StorageError> {
        self.read_optional_json(
            "SELECT checkpoint_json FROM consumer_evaluation_checkpoints WHERE task_id = ?1",
            &task_id.to_string(),
        )
    }

    /// Persists a running evaluation checkpoint with a matching event.
    ///
    /// # Errors
    ///
    /// Returns an error for stale task/run authority or invalid checkpoint state.
    pub fn save_consumer_evaluation_checkpoint(
        &mut self,
        checkpoint: &ConsumerEvaluationCheckpoint,
    ) -> Result<EventEnvelope, StorageError> {
        if checkpoint.schema_version != AGENT_RUN_SCHEMA_VERSION
            || checkpoint.state != ConsumerEvaluationCheckpointState::Running
            || checkpoint.prepared_evaluation_id.is_some()
            || checkpoint.prepared_evaluated_at.is_some()
            || self.task_launch_run_id(checkpoint.task_id)? != Some(checkpoint.run_id)
        {
            return Err(StorageError::InvalidData(
                "consumer evaluation checkpoint identity or state is invalid".into(),
            ));
        }
        let task = self
            .load_task(checkpoint.task_id)?
            .ok_or(StorageError::TaskNotFound(checkpoint.task_id))?;
        let run = self
            .agent_run(checkpoint.run_id)?
            .ok_or(StorageError::AgentRunNotFound(checkpoint.run_id))?;
        if task.state != TaskState::Active || run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "running evaluation checkpoint has stale task/run authority".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        upsert_consumer_evaluation_checkpoint(&transaction, checkpoint)?;
        let event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::EvaluationCheckpointed,
                Some(checkpoint.task_id),
                serde_json::to_value(checkpoint)?,
            ),
        )?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically checkpoints an approval-required objective condition and
    /// moves the task into its visible waiting state.
    ///
    /// # Errors
    ///
    /// Returns an error unless the checkpoint identifies a pending approval
    /// and the task/run still hold active evaluation authority.
    pub fn wait_consumer_evaluation_approval(
        &mut self,
        checkpoint: &ConsumerEvaluationCheckpoint,
    ) -> Result<(TaskEnvelope, Vec<EventEnvelope>), StorageError> {
        let request_id = checkpoint.request_id.ok_or_else(|| {
            StorageError::InvalidData("waiting evaluation checkpoint has no request id".into())
        })?;
        if checkpoint.state != ConsumerEvaluationCheckpointState::WaitingForApproval
            || self.task_launch_run_id(checkpoint.task_id)? != Some(checkpoint.run_id)
            || !self
                .approval_for_request(request_id)?
                .is_some_and(|approval| approval.state == ApprovalState::Pending)
        {
            return Err(StorageError::Conflict(
                "evaluation checkpoint does not have one pending approval".into(),
            ));
        }
        let mut task = self
            .load_task(checkpoint.task_id)?
            .ok_or(StorageError::TaskNotFound(checkpoint.task_id))?;
        let run = self
            .agent_run(checkpoint.run_id)?
            .ok_or(StorageError::AgentRunNotFound(checkpoint.run_id))?;
        if task.state != TaskState::Active || run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "evaluation task/run changed before approval wait".into(),
            ));
        }
        task.transition_to(TaskState::WaitingForApproval)?;
        task.updated_at = checkpoint.updated_at;
        let transaction = self.connection.transaction()?;
        upsert_consumer_evaluation_checkpoint(&transaction, checkpoint)?;
        let checkpoint_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::EvaluationCheckpointed,
                Some(checkpoint.task_id),
                serde_json::to_value(checkpoint)?,
            ),
        )?;
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(checkpoint.task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::Active,
                    to: TaskState::WaitingForApproval,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        transaction.commit()?;
        Ok((task, vec![checkpoint_event, task_event]))
    }

    /// Restores a waiting evaluation checkpoint after its request is approved.
    /// Interrupted runs are restored from recovering because the checkpoint
    /// itself carries no active process authority.
    ///
    /// # Errors
    ///
    /// Returns an error unless checkpoint, approval, task, and run agree.
    pub fn resume_consumer_evaluation(
        &mut self,
        task_id: mcg_domain::TaskId,
        at: DateTime<Utc>,
    ) -> Result<(ConsumerEvaluationCheckpoint, Vec<EventEnvelope>), StorageError> {
        let mut checkpoint = self
            .consumer_evaluation_checkpoint(task_id)?
            .ok_or_else(|| StorageError::Conflict("task has no evaluation checkpoint".into()))?;
        let request_id = checkpoint.request_id.ok_or_else(|| {
            StorageError::InvalidData("waiting evaluation checkpoint has no request id".into())
        })?;
        if checkpoint.state != ConsumerEvaluationCheckpointState::WaitingForApproval
            || !self
                .approval_for_request(request_id)?
                .is_some_and(|approval| approval.state == ApprovalState::Approved)
        {
            return Err(StorageError::Conflict(
                "evaluation checkpoint approval has not been granted".into(),
            ));
        }
        let mut task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let mut run = self
            .agent_run(checkpoint.run_id)?
            .ok_or(StorageError::AgentRunNotFound(checkpoint.run_id))?;
        if task.state != TaskState::WaitingForApproval
            || !matches!(
                run.state,
                AgentRunState::Running | AgentRunState::Recovering
            )
        {
            return Err(StorageError::Conflict(
                "evaluation task/run cannot resume from current state".into(),
            ));
        }
        task.transition_to(TaskState::Active)?;
        task.updated_at = at;
        checkpoint.state = ConsumerEvaluationCheckpointState::Running;
        checkpoint.request_id = None;
        checkpoint.prepared_evaluation_id = None;
        checkpoint.prepared_evaluated_at = None;
        checkpoint.batch_approved = true;
        checkpoint.updated_at = at;
        let previous_run = run.state;
        run.state = AgentRunState::Running;
        run.updated_at = at;
        let transaction = self.connection.transaction()?;
        upsert_consumer_evaluation_checkpoint(&transaction, &checkpoint)?;
        let mut events = vec![Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::EvaluationCheckpointed,
                Some(task_id),
                serde_json::to_value(&checkpoint)?,
            ),
        )?];
        if previous_run != run.state {
            let changed = transaction.execute(
                "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
                 WHERE run_id = ?3 AND state = 'recovering'",
                params![
                    serde_json::to_string(&run)?,
                    at.to_rfc3339(),
                    run.id.to_string(),
                ],
            )?;
            ensure_single_change(changed, "recovering evaluation run changed before resume")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(task_id),
                    serde_json::json!({"from": previous_run, "to": run.state, "run": &run, "reason": "evaluation_approval_resumed"}),
                ),
            )?);
        }
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::WaitingForApproval,
                    to: TaskState::Active,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok((checkpoint, events))
    }

    /// Atomically starts a fresh attempt for a denied evaluation checkpoint.
    ///
    /// Returns `None` when the blocked task is not an evaluation retry.
    ///
    /// # Errors
    ///
    /// Returns an error for stale checkpoint/task/run state.
    pub fn retry_consumer_evaluation(
        &mut self,
        task_id: mcg_domain::TaskId,
        at: DateTime<Utc>,
    ) -> Result<Option<(TaskEnvelope, Vec<EventEnvelope>)>, StorageError> {
        let Some(mut checkpoint) = self.consumer_evaluation_checkpoint(task_id)? else {
            return Ok(None);
        };
        if checkpoint.state != ConsumerEvaluationCheckpointState::Blocked {
            return Ok(None);
        }
        let mut task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let mut run = self
            .agent_run(checkpoint.run_id)?
            .ok_or(StorageError::AgentRunNotFound(checkpoint.run_id))?;
        if task.state != TaskState::Blocked
            || !matches!(
                run.state,
                AgentRunState::Running | AgentRunState::Recovering
            )
        {
            return Err(StorageError::Conflict(
                "evaluation retry has stale task/run state".into(),
            ));
        }
        task.transition_to(TaskState::Active)?;
        task.updated_at = at;
        checkpoint.state = ConsumerEvaluationCheckpointState::Running;
        checkpoint.request_id = None;
        checkpoint.prepared_evaluation_id = None;
        checkpoint.prepared_evaluated_at = None;
        checkpoint.batch_approved = false;
        checkpoint.updated_at = at;
        let previous_run = run.state;
        run.state = AgentRunState::Running;
        run.updated_at = at;
        let transaction = self.connection.transaction()?;
        upsert_consumer_evaluation_checkpoint(&transaction, &checkpoint)?;
        let mut events = vec![Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::EvaluationCheckpointed,
                Some(task_id),
                serde_json::to_value(&checkpoint)?,
            ),
        )?];
        if previous_run != run.state {
            let changed = transaction.execute(
                "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
                 WHERE run_id = ?3 AND state = 'recovering'",
                params![
                    serde_json::to_string(&run)?,
                    at.to_rfc3339(),
                    run.id.to_string()
                ],
            )?;
            ensure_single_change(changed, "evaluation retry run changed concurrently")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(task_id),
                    serde_json::json!({"from": previous_run, "to": run.state, "run": &run, "reason": "evaluation_retry"}),
                ),
            )?);
        }
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::Blocked,
                    to: TaskState::Active,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(Some((task, events)))
    }

    /// Atomically denies/cancels a waiting objective-check approval and blocks
    /// the task for a fresh evaluation attempt.
    ///
    /// Returns `None` when the approval belongs to a different workflow.
    ///
    /// # Errors
    ///
    /// Returns an error for stale approval/checkpoint/task/run state.
    #[allow(clippy::too_many_lines)] // Approval, checkpoint, and projections share one transaction.
    pub fn reject_consumer_evaluation_approval(
        &mut self,
        resolution: &ApprovalResolution,
    ) -> Result<Option<(ApprovalRequest, Vec<EventEnvelope>)>, StorageError> {
        if !matches!(
            resolution.state,
            ApprovalState::Denied | ApprovalState::Cancelled
        ) {
            return Ok(None);
        }
        let mut approval = self
            .approval(resolution.approval_id)?
            .ok_or_else(|| StorageError::Conflict("approval was not found".into()))?;
        let Some(mut checkpoint) = self.consumer_evaluation_checkpoint(approval.task_id)? else {
            return Ok(None);
        };
        if checkpoint.state != ConsumerEvaluationCheckpointState::WaitingForApproval
            || checkpoint.request_id != Some(approval.tool_request_id)
        {
            return Ok(None);
        }
        if approval.state != ApprovalState::Pending || approval.expires_at <= resolution.resolved_at
        {
            return Err(StorageError::Conflict(
                "approval is no longer pending and unexpired".into(),
            ));
        }
        let mut task = self
            .load_task(approval.task_id)?
            .ok_or(StorageError::TaskNotFound(approval.task_id))?;
        let mut run = self
            .agent_run(checkpoint.run_id)?
            .ok_or(StorageError::AgentRunNotFound(checkpoint.run_id))?;
        if task.state != TaskState::WaitingForApproval
            || !matches!(
                run.state,
                AgentRunState::Running | AgentRunState::Recovering
            )
        {
            return Err(StorageError::Conflict(
                "evaluation rejection has stale task/run state".into(),
            ));
        }
        let at = resolution.resolved_at;
        approval.state = resolution.state;
        checkpoint.state = ConsumerEvaluationCheckpointState::Blocked;
        checkpoint.request_id = None;
        checkpoint.batch_approved = false;
        checkpoint.attempt = checkpoint.attempt.saturating_add(1);
        checkpoint.updated_at = at;
        task.transition_to(TaskState::Blocked)?;
        task.updated_at = at;
        let previous_run = run.state;
        run.state = AgentRunState::Running;
        run.updated_at = at;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE approvals SET state = ?1, approval_json = ?2, resolved_at = ?3
             WHERE approval_id = ?4 AND state = 'pending'",
            params![
                crate::approval_state_name(approval.state),
                serde_json::to_string(&approval)?,
                at.to_rfc3339(),
                approval.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "evaluation approval changed before rejection")?;
        upsert_consumer_evaluation_checkpoint(&transaction, &checkpoint)?;
        let mut events = vec![
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::ApprovalResolved,
                    Some(task.id),
                    serde_json::json!({"approval": approval, "resolution": resolution}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::EvaluationCheckpointed,
                    Some(task.id),
                    serde_json::to_value(&checkpoint)?,
                ),
            )?,
        ];
        if previous_run != run.state {
            let changed = transaction.execute(
                "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
                 WHERE run_id = ?3 AND state = 'recovering'",
                params![
                    serde_json::to_string(&run)?,
                    at.to_rfc3339(),
                    run.id.to_string()
                ],
            )?;
            ensure_single_change(changed, "evaluation rejection run changed concurrently")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(task.id),
                    serde_json::json!({"from": previous_run, "to": run.state, "run": &run, "reason": "evaluation_approval_rejected"}),
                ),
            )?);
        }
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task.id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::WaitingForApproval,
                    to: TaskState::Blocked,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(Some((approval, events)))
    }

    /// Atomically resolves a denied/cancelled approval for a launched waiting
    /// turn and revokes all authority retained for that turn.
    ///
    /// Returns `None` when the approval is not associated with a launched task,
    /// allowing ordinary standalone tool approvals to use the generic resolver.
    ///
    /// # Errors
    ///
    /// Returns an error unless the approval is pending and unexpired and its
    /// task has exactly one waiting instance with one active lease.
    #[allow(clippy::too_many_lines)] // Approval and authority settlement must remain one transaction.
    pub fn reject_waiting_agent_approval(
        &mut self,
        resolution: &ApprovalResolution,
    ) -> Result<Option<(ApprovalRequest, Vec<EventEnvelope>)>, StorageError> {
        if !matches!(
            resolution.state,
            ApprovalState::Denied | ApprovalState::Cancelled
        ) {
            return Err(StorageError::InvalidData(
                "waiting agent approval rejection must be denied or cancelled".into(),
            ));
        }
        let mut approval = self
            .approval(resolution.approval_id)?
            .ok_or_else(|| StorageError::Conflict("approval was not found".into()))?;
        let Some(run_id) = self.task_launch_run_id(approval.task_id)? else {
            return Ok(None);
        };
        if approval.state != ApprovalState::Pending || approval.expires_at <= resolution.resolved_at
        {
            return Err(StorageError::Conflict(
                "approval is no longer pending and unexpired".into(),
            ));
        }
        let mut task = self
            .load_task(approval.task_id)?
            .ok_or(StorageError::TaskNotFound(approval.task_id))?;
        let mut run = self
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let mut instances: Vec<AgentInstance> = self.read_json_rows(
            "SELECT instance_json FROM agent_instances
             WHERE run_id = ?1 AND state = 'waiting' ORDER BY instance_id",
            &run_id.to_string(),
        )?;
        if task.state != TaskState::WaitingForApproval
            || run.state != AgentRunState::Waiting
            || instances.len() != 1
        {
            return Err(StorageError::Conflict(
                "approval task does not have one waiting agent turn".into(),
            ));
        }
        let Some(mut instance) = instances.pop() else {
            return Err(StorageError::Conflict(
                "approval task does not have one waiting agent turn".into(),
            ));
        };
        let mut leases: Vec<AgentLease> = self.read_json_rows(
            "SELECT lease_json FROM agent_leases
             WHERE instance_id = ?1 AND state = 'active' ORDER BY lease_id",
            &instance.id.to_string(),
        )?;
        if leases.len() != 1 {
            return Err(StorageError::Conflict(
                "waiting approval agent does not have one active lease".into(),
            ));
        }
        let Some(mut lease) = leases.pop() else {
            return Err(StorageError::Conflict(
                "waiting approval agent does not have one active lease".into(),
            ));
        };
        let at = resolution.resolved_at;
        let checkpoint = self.agent_turn_checkpoint(lease.id)?.ok_or_else(|| {
            StorageError::Conflict("waiting approval turn has no durable checkpoint".into())
        })?;
        let usage = apply_checkpointed_turn_accounting(
            Some(&checkpoint),
            &mut run,
            &mut instance,
            &mut lease,
            at,
        )?;
        approval.state = resolution.state;
        task.transition_to(TaskState::Blocked)?;
        task.updated_at = at;
        let previous_run = run.state;
        run.state = AgentRunState::Running;
        run.updated_at = at;
        let previous_instance = instance.state;
        instance.state = AgentState::Failed;
        instance.updated_at = at;
        lease.state = AgentLeaseState::Failed;
        lease.heartbeat_at = at;
        let reason_code = if resolution.state == ApprovalState::Denied {
            "approval_denied"
        } else {
            "approval_cancelled"
        };
        let transaction = self.connection.transaction()?;
        settle_attached_agent_turn_workflow_in_transaction(
            &transaction,
            lease.id,
            AgentTurnWorkflowSettlement::Failed,
            at,
        )?;
        let mut events = Vec::new();
        for record in &usage {
            insert_agent_usage(&transaction, record)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentUsageRecorded,
                    Some(task.id),
                    serde_json::to_value(record)?,
                ),
            )?);
        }
        transaction.execute(
            "DELETE FROM agent_turn_checkpoints WHERE lease_id = ?1",
            [lease.id.to_string()],
        )?;
        let changed = transaction.execute(
            "UPDATE approvals SET state = ?1, approval_json = ?2, resolved_at = ?3
             WHERE approval_id = ?4 AND state = 'pending'",
            params![
                crate::approval_state_name(approval.state),
                serde_json::to_string(&approval)?,
                at.to_rfc3339(),
                approval.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "approval changed before rejection settlement")?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = 'failed', instance_json = ?1, updated_at = ?2
             WHERE instance_id = ?3 AND state = 'waiting'",
            params![
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(
            changed,
            "waiting instance changed before approval rejection",
        )?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = 'failed', lease_json = ?1, heartbeat_at = ?2
             WHERE lease_id = ?3 AND state = 'active'",
            params![
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "active lease changed before approval rejection")?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'waiting'",
            params![
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "waiting run changed before approval rejection")?;
        events.extend([
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::ApprovalResolved,
                    Some(task.id),
                    serde_json::json!({"approval": approval, "resolution": resolution}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(task.id),
                    serde_json::json!({"from": previous_instance, "to": AgentState::Failed, "instance": &instance, "reason": reason_code}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    Some(task.id),
                    serde_json::json!({"lease": &lease, "reason": reason_code}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(task.id),
                    serde_json::json!({"from": previous_run, "to": AgentRunState::Running, "run": &run, "reason": reason_code}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentTurnFailed,
                    Some(task.id),
                    serde_json::json!({"run_id": run.id, "instance_id": instance.id, "lease_id": lease.id, "reason_code": reason_code}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunMetricsUpdated,
                    Some(task.id),
                    serde_json::to_value(&run)?,
                ),
            )?,
        ]);
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task.id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::WaitingForApproval,
                    to: TaskState::Blocked,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(Some((approval, events)))
    }

    /// Atomically restores one approval-waiting consumer task/run/instance to
    /// running while retaining its original active lease.
    ///
    /// # Errors
    ///
    /// Returns an error unless exactly one waiting instance and active lease
    /// hold authority for the waiting launched run.
    #[allow(clippy::too_many_lines)] // Keep authority validation and its atomic projection/event commit together.
    pub fn resume_launched_agent_turn(
        &mut self,
        task_id: mcg_domain::TaskId,
        at: DateTime<Utc>,
    ) -> Result<AgentResumeReceipt, StorageError> {
        let run_id = self
            .task_launch_run_id(task_id)?
            .ok_or_else(|| StorageError::Conflict("task has no consumer launch".into()))?;
        let mut task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let mut run = self
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let mut instances: Vec<AgentInstance> = self.read_json_rows(
            "SELECT instance_json FROM agent_instances
             WHERE run_id = ?1 AND state = 'waiting' ORDER BY instance_id",
            &run_id.to_string(),
        )?;
        if task.state != TaskState::WaitingForApproval
            || run.state != AgentRunState::Waiting
            || instances.len() != 1
        {
            return Err(StorageError::Conflict(
                "consumer task does not have one approval-waiting turn".into(),
            ));
        }
        let Some(mut instance) = instances.pop() else {
            return Err(StorageError::Conflict(
                "consumer task does not have one approval-waiting turn".into(),
            ));
        };
        let mut leases: Vec<AgentLease> = self.read_json_rows(
            "SELECT lease_json FROM agent_leases
             WHERE instance_id = ?1 AND state = 'active' ORDER BY lease_id",
            &instance.id.to_string(),
        )?;
        if leases.len() != 1 {
            return Err(StorageError::Conflict(
                "approval-waiting instance does not have one active lease".into(),
            ));
        }
        let Some(lease) = leases.pop() else {
            return Err(StorageError::Conflict(
                "approval-waiting instance does not have one active lease".into(),
            ));
        };
        if lease.expires_at <= at {
            return Err(StorageError::Conflict(
                "approval-waiting agent lease has expired".into(),
            ));
        }
        task.transition_to(TaskState::Active)?;
        task.updated_at = at;
        let previous_run = run.state;
        run.state = AgentRunState::Running;
        run.updated_at = at;
        let previous_instance = instance.state;
        instance.state = AgentState::Running;
        instance.updated_at = at;
        let transaction = self.connection.transaction()?;
        resume_attached_agent_approval_wait_in_transaction(&transaction, lease.id, at)?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'waiting'",
            params![
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "waiting agent run changed before resume")?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = 'running', instance_json = ?1, updated_at = ?2
             WHERE instance_id = ?3 AND state = 'waiting'",
            params![
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "waiting agent instance changed before resume")?;
        let events = vec![
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(task_id),
                    serde_json::json!({"from": previous_run, "to": AgentRunState::Running, "run": &run, "reason": "approval_resumed"}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(task_id),
                    serde_json::json!({"from": previous_instance, "to": AgentState::Running, "instance": &instance, "reason": "approval_resumed"}),
                ),
            )?,
        ];
        let mut events = events;
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::WaitingForApproval,
                    to: TaskState::Active,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(AgentResumeReceipt {
            task,
            run,
            instance,
            lease,
            events,
        })
    }

    /// Returns the one consumer run bound to a task, when present.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted launch identity is invalid.
    pub fn task_launch_run_id(
        &self,
        task_id: mcg_domain::TaskId,
    ) -> Result<Option<RunId>, StorageError> {
        self.connection
            .query_row(
                "SELECT run_id FROM task_launches WHERE task_id = ?1",
                [task_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|run_id| {
                run_id.parse().map_err(|_| {
                    StorageError::InvalidData("stored task launch run id is invalid".into())
                })
            })
            .transpose()
    }

    /// Atomically cancels a task-launched run and every still-active agent
    /// authority before cancelling the task projection itself.
    ///
    /// Returns `None` when the task has no consumer launch record, allowing the
    /// caller to use the ordinary task-only transition path.
    ///
    /// # Errors
    ///
    /// Returns an error for illegal terminal transitions, corrupt launch
    /// identity, concurrent state changes, or failed persistence.
    #[allow(clippy::too_many_lines)]
    pub fn cancel_launched_task(
        &mut self,
        task_id: mcg_domain::TaskId,
    ) -> Result<Option<(TaskEnvelope, Vec<EventEnvelope>)>, StorageError> {
        let Some(run_id) = self
            .connection
            .query_row(
                "SELECT run_id FROM task_launches WHERE task_id = ?1",
                [task_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let run_id: RunId = run_id.parse().map_err(|_| {
            StorageError::InvalidData("stored task launch run id is invalid".into())
        })?;
        let mut task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let mut run = self
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        if run.task_id != task_id || !run.state.can_transition_to(AgentRunState::Cancelled) {
            return Err(StorageError::Conflict(
                "launched task run cannot be cancelled from its current state".into(),
            ));
        }
        let previous_task = task.state;
        task.transition_to(TaskState::Cancelled)?;
        let previous_run = run.state;
        run.state = AgentRunState::Cancelled;
        run.updated_at = task.updated_at;
        run.completed_at = Some(task.updated_at);
        let mut instances: Vec<AgentInstance> = self.read_json_rows(
            "SELECT instance_json FROM agent_instances
             WHERE run_id = ?1 AND state IN ('created', 'running', 'waiting', 'blocked')
             ORDER BY created_at, instance_id",
            &run_id.to_string(),
        )?;
        let mut leases: Vec<AgentLease> = self.read_json_rows(
            "SELECT agent_leases.lease_json FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1 AND agent_leases.state = 'active'
             ORDER BY agent_leases.acquired_at, agent_leases.lease_id",
            &run_id.to_string(),
        )?;
        let checkpoints: Vec<AgentTurnCheckpoint> = self.read_json_rows(
            "SELECT checkpoint_json FROM agent_turn_checkpoints
             WHERE task_id = ?1 ORDER BY lease_id",
            &task_id.to_string(),
        )?;
        let mut approvals: Vec<ApprovalRequest> = self.read_json_rows(
            "SELECT approval_json FROM approvals
             WHERE task_id = ?1 AND state = 'pending' ORDER BY requested_at, approval_id",
            &task_id.to_string(),
        )?;
        let mut usage = Vec::new();
        for checkpoint in &checkpoints {
            let instance_index = instances
                .iter()
                .position(|instance| instance.id == checkpoint.request.instance.id)
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "cancelled checkpoint has no active task instance".into(),
                    )
                })?;
            let lease_index = leases
                .iter()
                .position(|lease| lease.id == checkpoint.request.lease.id)
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "cancelled checkpoint has no active task lease".into(),
                    )
                })?;
            usage.extend(apply_checkpointed_turn_accounting(
                Some(checkpoint),
                &mut run,
                &mut instances[instance_index],
                &mut leases[lease_index],
                task.updated_at,
            )?);
        }
        let transaction = self.connection.transaction()?;
        let mut events = Vec::new();
        for record in &usage {
            insert_agent_usage(&transaction, record)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentUsageRecorded,
                    Some(task_id),
                    serde_json::to_value(record)?,
                ),
            )?);
        }
        transaction.execute(
            "DELETE FROM agent_turn_checkpoints WHERE task_id = ?1",
            [task_id.to_string()],
        )?;
        for approval in &mut approvals {
            approval.state = ApprovalState::Cancelled;
            let resolution = ApprovalResolution {
                approval_id: approval.id,
                state: ApprovalState::Cancelled,
                resolved_by: "system:task_cancelled".into(),
                reason: Some("task was cancelled before approval resolution".into()),
                resolved_at: task.updated_at,
            };
            let changed = transaction.execute(
                "UPDATE approvals SET state = 'cancelled', approval_json = ?1, resolved_at = ?2
                 WHERE approval_id = ?3 AND state = 'pending'",
                params![
                    serde_json::to_string(approval)?,
                    task.updated_at.to_rfc3339(),
                    approval.id.to_string(),
                ],
            )?;
            ensure_single_change(changed, "approval changed while cancelling task")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::ApprovalResolved,
                    Some(task_id),
                    serde_json::json!({"approval": approval, "resolution": resolution}),
                ),
            )?);
        }
        for instance in &mut instances {
            let previous = instance.state;
            instance.state = AgentState::Cancelled;
            instance.updated_at = task.updated_at;
            let changed = transaction.execute(
                "UPDATE agent_instances SET state = 'cancelled', instance_json = ?1, updated_at = ?2
                 WHERE instance_id = ?3 AND state = ?4",
                params![
                    serde_json::to_string(instance)?,
                    task.updated_at.to_rfc3339(),
                    instance.id.to_string(),
                    instance_state_name(previous),
                ],
            )?;
            ensure_single_change(changed, "agent instance changed while cancelling task")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(task_id),
                    serde_json::json!({"from": previous, "to": AgentState::Cancelled, "instance": instance, "reason": "task_cancelled"}),
                ),
            )?);
        }
        for lease in &mut leases {
            lease.state = AgentLeaseState::Terminated;
            lease.heartbeat_at = task.updated_at;
            let changed = transaction.execute(
                "UPDATE agent_leases SET state = 'terminated', lease_json = ?1, heartbeat_at = ?2
                 WHERE lease_id = ?3 AND state = 'active'",
                params![
                    serde_json::to_string(lease)?,
                    task.updated_at.to_rfc3339(),
                    lease.id.to_string(),
                ],
            )?;
            ensure_single_change(changed, "agent lease changed while cancelling task")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    Some(task_id),
                    serde_json::json!({"lease": lease, "reason": "task_cancelled"}),
                ),
            )?);
        }
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'cancelled', run_json = ?1, updated_at = ?2,
                    completed_at = ?2 WHERE run_id = ?3 AND state = ?4",
            params![
                serde_json::to_string(&run)?,
                task.updated_at.to_rfc3339(),
                run.id.to_string(),
                run_state_name(previous_run),
            ],
        )?;
        ensure_single_change(changed, "agent run changed while cancelling task")?;
        events.push(Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::AgentRunStateChanged,
                Some(task_id),
                serde_json::json!({"from": previous_run, "to": AgentRunState::Cancelled, "run": &run, "reason": "task_cancelled"}),
            ),
        )?);
        if !usage.is_empty() {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunMetricsUpdated,
                    Some(task_id),
                    serde_json::to_value(&run)?,
                ),
            )?);
        }
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: previous_task,
                    to: TaskState::Cancelled,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(Some((task, events)))
    }

    /// Atomically binds an existing pending task to its one consumer run and
    /// starts both projections. Repeating the operation returns the original
    /// run and never creates duplicate authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the task/run/plan identities or durable state are
    /// inconsistent, or persistence fails.
    #[allow(clippy::too_many_lines)]
    pub fn launch_task_agent_run(
        &mut self,
        task_id: mcg_domain::TaskId,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<TaskLaunchReceipt, StorageError> {
        if let Some(run_id) = self
            .connection
            .query_row(
                "SELECT run_id FROM task_launches WHERE task_id = ?1",
                [task_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            let run_id = run_id.parse().map_err(|_| {
                StorageError::InvalidData("stored task launch run id is invalid".into())
            })?;
            return Ok(TaskLaunchReceipt {
                task: self
                    .load_task(task_id)?
                    .ok_or(StorageError::TaskNotFound(task_id))?,
                run: self
                    .agent_run(run_id)?
                    .ok_or(StorageError::AgentRunNotFound(run_id))?,
                replayed: true,
                events: Vec::new(),
            });
        }

        let task = self
            .load_task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        if task.state != TaskState::Pending
            || run.schema_version != AGENT_RUN_SCHEMA_VERSION
            || run.state != AgentRunState::Planned
            || run.completed_at.is_some()
            || run.id != plan.run_id
            || task.id != run.task_id
            || task.id != plan.task_id
            || run.organization_revision_id != genome.revision_id
            || run.organization_revision_id != plan.organization_revision_id
        {
            return Err(StorageError::InvalidData(
                "task launch identities or initial states are inconsistent".into(),
            ));
        }
        if let Some(project_id) = task.project_id
            && self.project(project_id)?.is_none()
        {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        plan.validate(genome)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !budgets_within_parent(plan, &task.budget) || !metrics_valid(&run.metrics) {
            return Err(StorageError::InvalidData(
                "task launch metrics or aggregate work budgets are invalid".into(),
            ));
        }
        let stored_revision = self.organization_revision(genome.revision_id)?;
        let organization_exists = self.organization(genome.organization_id)?.is_some();
        let persist_organization = match stored_revision {
            Some(stored) if stored == *genome => false,
            Some(_) => {
                return Err(StorageError::Conflict(
                    "task launch organization revision does not match durable state".into(),
                ));
            }
            None if organization_exists => {
                return Err(StorageError::Conflict(
                    "task launch organization revision is not durable".into(),
                ));
            }
            None => {
                if genome.revision != 1 || genome.parent_revision_id.is_some() {
                    return Err(StorageError::InvalidData(
                        "a new task launch organization must begin at revision one".into(),
                    ));
                }
                true
            }
        };

        let mut active_task = task.clone();
        active_task.transition_to(TaskState::Active)?;
        let mut running_run = run.clone();
        running_run.state = AgentRunState::Running;
        running_run.updated_at = active_task.updated_at;
        let organization_event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let planned_event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(task.id),
            serde_json::json!({"run": run, "plan": plan}),
        );
        let active_event = EventEnvelope::new(
            EventKind::TaskStateChanged,
            Some(task.id),
            serde_json::to_value(TaskStateChangedPayload {
                from: TaskState::Pending,
                to: TaskState::Active,
            })?,
        );
        let running_event = EventEnvelope::new(
            EventKind::AgentRunStateChanged,
            Some(task.id),
            serde_json::json!({
                "from": AgentRunState::Planned,
                "to": AgentRunState::Running,
                "run": &running_run,
            }),
        );

        let transaction = self.connection.transaction()?;
        let mut events = Vec::new();
        if persist_organization {
            transaction.execute(
                "INSERT INTO organization_revisions(
                    revision_id, organization_id, revision, parent_revision_id,
                    organization_json, created_at
                 ) VALUES (?1, ?2, 1, NULL, ?3, ?4)",
                params![
                    genome.revision_id.to_string(),
                    genome.organization_id.to_string(),
                    serde_json::to_string(genome)?,
                    genome.created_at.to_rfc3339(),
                ],
            )?;
            transaction.execute(
                "INSERT INTO organizations(
                    organization_id, current_revision_id, organization_json, updated_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    genome.organization_id.to_string(),
                    genome.revision_id.to_string(),
                    serde_json::to_string(genome)?,
                    genome.created_at.to_rfc3339(),
                ],
            )?;
            events.push(Self::insert_event(&transaction, &organization_event)?);
        }
        transaction.execute(
            "INSERT INTO agent_runs(
                run_id, task_id, organization_revision_id, state, run_json,
                created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, 'planned', ?4, ?5, ?6, NULL)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                run.organization_revision_id.to_string(),
                serde_json::to_string(run)?,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
            ],
        )?;
        events.push(Self::insert_event(&transaction, &planned_event)?);
        let event = Self::insert_event(&transaction, &active_event)?;
        Self::upsert_task(&transaction, &active_task, event.sequence)?;
        events.push(event);
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'planned'",
            params![
                serde_json::to_string(&running_run)?,
                running_run.updated_at.to_rfc3339(),
                running_run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "task launch run could not start atomically")?;
        events.push(Self::insert_event(&transaction, &running_event)?);
        transaction.execute(
            "INSERT INTO task_launches(task_id, run_id, created_at) VALUES (?1, ?2, ?3)",
            params![
                task.id.to_string(),
                running_run.id.to_string(),
                running_run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(TaskLaunchReceipt {
            task: active_task,
            run: running_run,
            replayed: false,
            events,
        })
    }

    /// Resolves one durable external-interface request identity to its task and run.
    ///
    /// # Errors
    ///
    /// Returns an error when a stored identity is malformed or the query fails.
    pub fn interface_run_request(
        &self,
        interface: &str,
        request_key: &str,
    ) -> Result<Option<(TaskId, RunId)>, StorageError> {
        let stored = self
            .connection
            .query_row(
                "SELECT task_id, run_id FROM interface_run_requests
                 WHERE interface = ?1 AND request_key = ?2",
                params![interface, request_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        stored
            .map(|(task_id, run_id)| {
                Ok((
                    task_id.parse().map_err(|_| {
                        StorageError::InvalidData("stored interface task id is invalid".into())
                    })?,
                    run_id.parse().map_err(|_| {
                        StorageError::InvalidData("stored interface run id is invalid".into())
                    })?,
                ))
            })
            .transpose()
    }

    /// Atomically creates an idempotent interface-owned task, organization,
    /// immutable plan, active task projection, and running agent run. A true
    /// `consumer_launch` flag also commits the unique task launch mapping.
    ///
    /// # Errors
    ///
    /// A request key may only be reused with the same canonical request hash.
    /// All task, organization, run, plan, project, and budget invariants must
    /// validate before any row or event is committed.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn create_interface_agent_run(
        &mut self,
        interface: &str,
        request_key: &str,
        request_hash: &str,
        consumer_launch: bool,
        task: &TaskEnvelope,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<InterfaceRunReceipt, StorageError> {
        if interface.is_empty()
            || interface.len() > 32
            || request_key.is_empty()
            || request_key.len() > 128
            || request_hash.len() != 64
            || !request_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidData(
                "interface request identity is invalid".into(),
            ));
        }
        let existing = self
            .connection
            .query_row(
                "SELECT request_hash, task_id, run_id FROM interface_run_requests
                 WHERE interface = ?1 AND request_key = ?2",
                params![interface, request_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored_hash, task_id, run_id)) = existing {
            if stored_hash != request_hash {
                return Err(StorageError::Conflict(
                    "interface request key was already used with different input".into(),
                ));
            }
            let task_id = task_id.parse().map_err(|_| {
                StorageError::InvalidData("stored interface task id is invalid".into())
            })?;
            let run_id = run_id.parse().map_err(|_| {
                StorageError::InvalidData("stored interface run id is invalid".into())
            })?;
            if consumer_launch && self.task_launch_run_id(task_id)? != Some(run_id) {
                return Err(StorageError::Conflict(
                    "interface consumer launch identity is missing or inconsistent".into(),
                ));
            }
            return Ok(InterfaceRunReceipt {
                task: self
                    .load_task(task_id)?
                    .ok_or(StorageError::TaskNotFound(task_id))?,
                run: self
                    .agent_run(run_id)?
                    .ok_or(StorageError::AgentRunNotFound(run_id))?,
                replayed: true,
                events: Vec::new(),
            });
        }

        if task.state != TaskState::Pending
            || run.schema_version != AGENT_RUN_SCHEMA_VERSION
            || run.state != AgentRunState::Planned
            || run.completed_at.is_some()
            || run.id != plan.run_id
            || task.id != run.task_id
            || task.id != plan.task_id
            || run.organization_revision_id != genome.revision_id
            || run.organization_revision_id != plan.organization_revision_id
            || genome.revision != 1
            || genome.parent_revision_id.is_some()
        {
            return Err(StorageError::InvalidData(
                "new interface task, organization, run, and plan identities are inconsistent"
                    .into(),
            ));
        }
        if let Some(project_id) = task.project_id
            && self.project(project_id)?.is_none()
        {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        plan.validate(genome)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !budgets_within_parent(plan, &task.budget) || !metrics_valid(&run.metrics) {
            return Err(StorageError::InvalidData(
                "interface run metrics or aggregate work budgets are invalid".into(),
            ));
        }
        if self.organization(genome.organization_id)?.is_some() {
            return Err(StorageError::Conflict(
                "interface organization identity already exists".into(),
            ));
        }

        let mut active_task = task.clone();
        active_task.transition_to(TaskState::Active)?;
        let mut running_run = run.clone();
        running_run.state = AgentRunState::Running;
        running_run.updated_at = active_task.updated_at;
        let created_event = EventEnvelope::new(
            EventKind::TaskCreated,
            Some(task.id),
            serde_json::to_value(TaskCreatedPayload { task: task.clone() })?,
        );
        let organization_event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let planned_event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(task.id),
            serde_json::json!({"run": run, "plan": plan}),
        );
        let active_event = EventEnvelope::new(
            EventKind::TaskStateChanged,
            Some(task.id),
            serde_json::to_value(TaskStateChangedPayload {
                from: TaskState::Pending,
                to: TaskState::Active,
            })?,
        );
        let running_event = EventEnvelope::new(
            EventKind::AgentRunStateChanged,
            Some(task.id),
            serde_json::json!({
                "from": AgentRunState::Planned,
                "to": AgentRunState::Running,
                "run": &running_run,
            }),
        );

        let transaction = self.connection.transaction()?;
        let mut events = Vec::new();
        let event = Self::insert_event(&transaction, &created_event)?;
        Self::upsert_task(&transaction, task, event.sequence)?;
        events.push(event);
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, 1, NULL, ?3, ?4)",
            params![
                genome.revision_id.to_string(),
                genome.organization_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO organizations(
                organization_id, current_revision_id, organization_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                genome.organization_id.to_string(),
                genome.revision_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        events.push(Self::insert_event(&transaction, &organization_event)?);
        transaction.execute(
            "INSERT INTO agent_runs(
                run_id, task_id, organization_revision_id, state, run_json,
                created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, 'planned', ?4, ?5, ?6, NULL)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                run.organization_revision_id.to_string(),
                serde_json::to_string(run)?,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                run.id.to_string(),
                task.id.to_string(),
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
            ],
        )?;
        events.push(Self::insert_event(&transaction, &planned_event)?);
        let event = Self::insert_event(&transaction, &active_event)?;
        Self::upsert_task(&transaction, &active_task, event.sequence)?;
        events.push(event);
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = 'running', run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'planned'",
            params![
                serde_json::to_string(&running_run)?,
                running_run.updated_at.to_rfc3339(),
                running_run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "interface run could not start atomically")?;
        events.push(Self::insert_event(&transaction, &running_event)?);
        transaction.execute(
            "INSERT INTO interface_run_requests(
                interface, request_key, request_hash, task_id, run_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                interface,
                request_key,
                request_hash,
                task.id.to_string(),
                run.id.to_string(),
                running_run.updated_at.to_rfc3339(),
            ],
        )?;
        if consumer_launch {
            transaction.execute(
                "INSERT INTO task_launches(task_id, run_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    task.id.to_string(),
                    running_run.id.to_string(),
                    running_run.updated_at.to_rfc3339(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(InterfaceRunReceipt {
            task: active_task,
            run: running_run,
            replayed: false,
            events,
        })
    }

    /// Atomically stores an immutable organization revision, updates its current
    /// projection, and appends replay evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, stale, skipped, or conflicting revisions.
    pub fn record_organization_revision(
        &mut self,
        genome: &OrganizationGenome,
    ) -> Result<EventEnvelope, StorageError> {
        genome
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let current = self.organization(genome.organization_id)?;
        let inherited_base = genome
            .inherited_from_revision_id
            .map(|revision_id| {
                self.organization_revision(revision_id)?
                    .ok_or(StorageError::OrganizationRevisionNotFound(revision_id))
            })
            .transpose()?;
        if inherited_base
            .as_ref()
            .is_some_and(|base| base.organization_id == genome.organization_id)
        {
            return Err(StorageError::Conflict(
                "an organization cannot inherit from one of its own revisions".into(),
            ));
        }
        match current {
            None if genome.revision != 1 || genome.parent_revision_id.is_some() => {
                return Err(StorageError::Conflict(
                    "first organization revision must be revision 1 without a parent".into(),
                ));
            }
            Some(ref current)
                if genome.revision != current.revision.saturating_add(1)
                    || genome.parent_revision_id != Some(current.revision_id)
                    || genome.inherited_from_revision_id != current.inherited_from_revision_id =>
            {
                return Err(StorageError::Conflict(
                    "organization revision does not extend the current head or changes its inherited base"
                        .into(),
                ));
            }
            _ => {}
        }
        let event = EventEnvelope::new(
            EventKind::OrganizationRevisionRecorded,
            None,
            serde_json::to_value(genome)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO organization_revisions(
                revision_id, organization_id, revision, parent_revision_id,
                organization_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                genome.revision_id.to_string(),
                genome.organization_id.to_string(),
                i64::try_from(genome.revision).map_err(|_| StorageError::InvalidData(
                    "organization revision exceeds SQLite range".into()
                ))?,
                genome.parent_revision_id.map(|id| id.to_string()),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO organizations(
                organization_id, current_revision_id, organization_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(organization_id) DO UPDATE SET
                current_revision_id = excluded.current_revision_id,
                organization_json = excluded.organization_json,
                updated_at = excluded.updated_at",
            params![
                genome.organization_id.to_string(),
                genome.revision_id.to_string(),
                serde_json::to_string(genome)?,
                genome.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads the current organization projection.
    ///
    /// # Errors
    ///
    /// Returns an error when its persisted JSON is invalid.
    pub fn organization(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Option<OrganizationGenome>, StorageError> {
        self.read_optional_json(
            "SELECT organization_json FROM organizations WHERE organization_id = ?1",
            &organization_id.to_string(),
        )
    }

    /// Lists current organization projections in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn organizations(&self) -> Result<Vec<OrganizationGenome>, StorageError> {
        self.read_all_json("SELECT organization_json FROM organizations ORDER BY organization_id")
    }

    /// Ensures one reusable global organization exists and returns all durable assignments.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted organizations or assignments cannot be read, validated,
    /// or written.
    pub fn initialize_organization_assignments(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<OrganizationAssignments, StorageError> {
        if let Some(assignments) = self.organization_assignments()? {
            return Ok(assignments);
        }
        let global = self
            .organizations()?
            .into_iter()
            .filter(|organization| {
                organization.name == "MCG Consumer Default" && organization.validate().is_ok()
            })
            .max_by_key(|organization| organization.created_at)
            .unwrap_or_else(|| OrganizationGenome::consumer_default(now));
        if self.organization(global.organization_id)?.is_none() {
            self.record_organization_revision(&global)?;
        }
        self.set_global_organization_assignment(global.organization_id, now)?;
        self.organization_assignments()?.ok_or_else(|| {
            StorageError::InvalidData("organization assignments were not initialized".into())
        })
    }

    /// Loads the global organization assignment and all project overrides.
    ///
    /// # Errors
    ///
    /// Returns an error when assignment rows cannot be read or contain invalid identities or
    /// timestamps.
    pub fn organization_assignments(
        &self,
    ) -> Result<Option<OrganizationAssignments>, StorageError> {
        let global = self.connection.query_row(
            "SELECT organization_id, updated_at FROM organization_assignments WHERE scope_key = 'global'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).optional()?;
        let Some((global, global_updated_at)) = global else {
            return Ok(None);
        };
        let global_organization_id = OrganizationId::from_str(&global).map_err(|_| {
            StorageError::InvalidData("invalid global organization assignment".into())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT project_id, organization_id, updated_at FROM organization_assignments WHERE project_id IS NOT NULL ORDER BY project_id"
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut project_overrides = std::collections::BTreeMap::new();
        let mut updated_at = DateTime::parse_from_rfc3339(&global_updated_at)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?
            .with_timezone(&Utc);
        for row in rows {
            let (project, organization, changed) = row?;
            let project = mcg_domain::ProjectId::from_str(&project).map_err(|_| {
                StorageError::InvalidData("invalid project organization assignment".into())
            })?;
            let organization = OrganizationId::from_str(&organization).map_err(|_| {
                StorageError::InvalidData("invalid project organization assignment".into())
            })?;
            let changed = DateTime::parse_from_rfc3339(&changed)
                .map_err(|error| StorageError::InvalidData(error.to_string()))?
                .with_timezone(&Utc);
            updated_at = updated_at.max(changed);
            project_overrides.insert(project, organization);
        }
        Ok(Some(OrganizationAssignments {
            global_organization_id,
            project_overrides,
            updated_at,
        }))
    }

    /// Replaces the global organization assignment.
    ///
    /// # Errors
    ///
    /// Returns an error when the organization is not assignable or the assignment cannot be
    /// persisted and reloaded.
    pub fn set_global_organization_assignment(
        &mut self,
        organization_id: OrganizationId,
        now: DateTime<Utc>,
    ) -> Result<OrganizationAssignments, StorageError> {
        ensure_assignable_organization(self, organization_id)?;
        self.connection.execute(
            "INSERT INTO organization_assignments(scope_key, project_id, organization_id, updated_at)
             VALUES ('global', NULL, ?1, ?2)
             ON CONFLICT(scope_key) DO UPDATE SET organization_id = excluded.organization_id, updated_at = excluded.updated_at",
            params![organization_id.to_string(), now.to_rfc3339()],
        )?;
        self.organization_assignments()?
            .ok_or_else(|| StorageError::InvalidData("global assignment disappeared".into()))
    }

    /// Sets or clears one project's organization override.
    ///
    /// # Errors
    ///
    /// Returns an error when the project or organization does not exist, the organization is not
    /// assignable, or the assignment cannot be persisted and reloaded.
    pub fn set_project_organization_assignment(
        &mut self,
        project_id: mcg_domain::ProjectId,
        organization_id: Option<OrganizationId>,
        now: DateTime<Utc>,
    ) -> Result<OrganizationAssignments, StorageError> {
        if self.project(project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(project_id));
        }
        if let Some(organization_id) = organization_id {
            ensure_assignable_organization(self, organization_id)?;
            let scope = format!("project:{project_id}");
            self.connection.execute(
                "INSERT INTO organization_assignments(scope_key, project_id, organization_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(scope_key) DO UPDATE SET organization_id = excluded.organization_id, updated_at = excluded.updated_at",
                params![scope, project_id.to_string(), organization_id.to_string(), now.to_rfc3339()],
            )?;
        } else {
            self.connection.execute(
                "DELETE FROM organization_assignments WHERE project_id = ?1",
                [project_id.to_string()],
            )?;
        }
        self.organization_assignments()?
            .ok_or_else(|| StorageError::InvalidData("global assignment is missing".into()))
    }

    /// Lists recent agent runs in reverse update order under a bounded limit.
    ///
    /// # Errors
    ///
    /// Returns an error when run rows cannot be read or decoded.
    pub fn agent_runs(&self, limit: usize) -> Result<Vec<AgentRun>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        let mut statement = self.connection.prepare(
            "SELECT run_json FROM agent_runs ORDER BY updated_at DESC, run_id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Loads one immutable organization revision.
    ///
    /// # Errors
    ///
    /// Returns an error when its persisted JSON is invalid.
    pub fn organization_revision(
        &self,
        revision_id: OrganizationRevisionId,
    ) -> Result<Option<OrganizationGenome>, StorageError> {
        self.read_optional_json(
            "SELECT organization_json FROM organization_revisions WHERE revision_id = ?1",
            &revision_id.to_string(),
        )
    }

    /// Loads organization revisions in numeric order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be read or decoded.
    pub fn organization_history(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<OrganizationGenome>, StorageError> {
        self.read_json_rows(
            "SELECT organization_json FROM organization_revisions
             WHERE organization_id = ?1 ORDER BY revision",
            &organization_id.to_string(),
        )
    }

    /// Atomically persists a planned run, its immutable work plan, and event.
    ///
    /// # Errors
    ///
    /// The task and organization revision must exist and all identities and plan
    /// invariants must agree.
    pub fn create_agent_run(
        &mut self,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<EventEnvelope, StorageError> {
        if run.schema_version != AGENT_RUN_SCHEMA_VERSION
            || run.state != AgentRunState::Planned
            || run.completed_at.is_some()
            || run.id != plan.run_id
            || run.task_id != plan.task_id
            || run.organization_revision_id != plan.organization_revision_id
        {
            return Err(StorageError::InvalidData(
                "new agent run and work plan identities or initial state are inconsistent".into(),
            ));
        }
        let task = self
            .load_task(run.task_id)?
            .ok_or(StorageError::TaskNotFound(run.task_id))?;
        let genome = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        plan.validate(&genome)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !budgets_within_parent(plan, &task.budget) || !metrics_valid(&run.metrics) {
            return Err(StorageError::InvalidData(
                "agent run metrics or aggregate work budgets are invalid".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentRunPlanned,
            Some(run.task_id),
            serde_json::json!({"run": run, "plan": plan}),
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_runs(
                run_id, task_id, organization_revision_id, state, run_json,
                created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                run.id.to_string(),
                run.task_id.to_string(),
                run.organization_revision_id.to_string(),
                run_state_name(run.state),
                serde_json::to_string(run)?,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO work_plans(run_id, task_id, plan_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                run.id.to_string(),
                run.task_id.to_string(),
                serde_json::to_string(plan)?,
                plan.created_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads a current run projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_run(&self, run_id: RunId) -> Result<Option<AgentRun>, StorageError> {
        self.read_optional_json(
            "SELECT run_json FROM agent_runs WHERE run_id = ?1",
            &run_id.to_string(),
        )
    }

    /// Loads the immutable work plan for a run.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn work_plan(&self, run_id: RunId) -> Result<Option<WorkPlan>, StorageError> {
        self.read_optional_json(
            "SELECT plan_json FROM work_plans WHERE run_id = ?1",
            &run_id.to_string(),
        )
    }

    /// Transitions the run projection and appends its authoritative event.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown runs, illegal transitions, or stale writes.
    pub fn transition_agent_run(
        &mut self,
        run_id: RunId,
        next: AgentRunState,
        metrics: OrganizationalMetrics,
        at: DateTime<Utc>,
    ) -> Result<(AgentRun, EventEnvelope), StorageError> {
        let mut run = self
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let previous = run.state;
        if !previous.can_transition_to(next) {
            return Err(StorageError::Conflict(format!(
                "illegal agent run transition from {previous:?} to {next:?}"
            )));
        }
        if !metrics_valid(&metrics) {
            return Err(StorageError::InvalidData(
                "agent run metrics must be finite and non-negative".into(),
            ));
        }
        run.state = next;
        run.metrics = metrics;
        run.updated_at = at;
        run.completed_at = next.is_terminal().then_some(at);
        let event = EventEnvelope::new(
            EventKind::AgentRunStateChanged,
            Some(run.task_id),
            serde_json::json!({"from": previous, "to": next, "run": run}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = ?1, run_json = ?2, updated_at = ?3,
                completed_at = ?4 WHERE run_id = ?5 AND state = ?6",
            params![
                run_state_name(next),
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.completed_at.map(|value| value.to_rfc3339()),
                run_id.to_string(),
                run_state_name(previous),
            ],
        )?;
        ensure_single_change(changed, "agent run changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((run, event))
    }

    /// Persists a newly created ephemeral agent instance and event.
    ///
    /// # Errors
    ///
    /// The run, selected role, task, schema, and initial state must agree.
    pub fn record_agent_instance(
        &mut self,
        instance: &AgentInstance,
    ) -> Result<EventEnvelope, StorageError> {
        let run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        let plan = self
            .work_plan(instance.run_id)?
            .ok_or(StorageError::InvalidData(
                "agent run has no work plan".into(),
            ))?;
        if instance.schema_version != AGENT_RUN_SCHEMA_VERSION
            || instance.state != AgentState::Created
            || instance.task_id != run.task_id
            || !plan.selected_roles.contains(&instance.role_id)
            || instance.context_fingerprint.trim().is_empty()
            || instance.confidence.basis_points > 10_000
        {
            return Err(StorageError::InvalidData(
                "agent instance is inconsistent with its selected run role".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentInstanceRecorded,
            Some(instance.task_id),
            serde_json::to_value(instance)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_instances(
                instance_id, run_id, task_id, role_id, state, instance_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                instance.id.to_string(),
                instance.run_id.to_string(),
                instance.task_id.to_string(),
                instance.role_id.to_string(),
                instance_state_name(instance.state),
                serde_json::to_string(instance)?,
                instance.created_at.to_rfc3339(),
                instance.updated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads a current agent instance projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_instance(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<Option<AgentInstance>, StorageError> {
        self.read_optional_json(
            "SELECT instance_json FROM agent_instances WHERE instance_id = ?1",
            &instance_id.to_string(),
        )
    }

    /// Lists current agent instances for a run in creation order.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_instances(&self, run_id: RunId) -> Result<Vec<AgentInstance>, StorageError> {
        self.read_json_rows(
            "SELECT instance_json FROM agent_instances WHERE run_id = ?1
             ORDER BY created_at, instance_id",
            &run_id.to_string(),
        )
    }

    /// Transitions an agent instance and records confidence at that boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown instance or illegal/stale transition.
    pub fn transition_agent_instance(
        &mut self,
        instance_id: AgentInstanceId,
        next: AgentState,
        confidence: ConfidenceEstimate,
        at: DateTime<Utc>,
    ) -> Result<(AgentInstance, EventEnvelope), StorageError> {
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let previous = instance.state;
        if !instance_transition_allowed(previous, next) {
            return Err(StorageError::Conflict(format!(
                "illegal agent instance transition from {previous:?} to {next:?}"
            )));
        }
        if confidence.basis_points > 10_000 {
            return Err(StorageError::InvalidData(
                "agent confidence exceeds 10000 basis points".into(),
            ));
        }
        instance.state = next;
        instance.confidence = confidence;
        instance.updated_at = at;
        let event = EventEnvelope::new(
            EventKind::AgentInstanceStateChanged,
            Some(instance.task_id),
            serde_json::json!({"from": previous, "to": next, "instance": instance}),
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = ?1, instance_json = ?2, updated_at = ?3
             WHERE instance_id = ?4 AND state = ?5",
            params![
                instance_state_name(next),
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance_id.to_string(),
                instance_state_name(previous),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((instance, event))
    }

    /// Persists an active bounded lease for selected work.
    ///
    /// # Errors
    ///
    /// The instance and assigned work item must exist and agree on role.
    pub fn record_agent_lease(
        &mut self,
        lease: &AgentLease,
    ) -> Result<EventEnvelope, StorageError> {
        let instance = self
            .agent_instance(lease.instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(lease.instance_id))?;
        let plan = self
            .work_plan(instance.run_id)?
            .ok_or(StorageError::InvalidData(
                "agent run has no work plan".into(),
            ))?;
        let item = plan
            .items
            .iter()
            .find(|item| item.id == lease.work_item_id)
            .ok_or_else(|| {
                StorageError::InvalidData("lease references unknown work item".into())
            })?;
        if lease.schema_version != AGENT_RUN_SCHEMA_VERSION
            || lease.state != AgentLeaseState::Active
            || item.assigned_role != instance.role_id
            || lease.expires_at <= lease.acquired_at
            || lease.heartbeat_at < lease.acquired_at
            || lease.reporting_interval_seconds == 0
            || instance.state != AgentState::Running
            || !budget_values_valid(&lease.budget.limit)
            || !lease.budget.consumed_cost_usd.is_finite()
            || lease.budget.consumed_cost_usd < 0.0
            || lease.budget.is_exhausted()
        {
            return Err(StorageError::InvalidData(
                "agent lease is inconsistent with its instance, work, or bounds".into(),
            ));
        }
        let event = EventEnvelope::new(
            EventKind::AgentLeaseRecorded,
            Some(instance.task_id),
            serde_json::to_value(lease)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_leases(
                lease_id, instance_id, work_item_id, state, lease_json,
                acquired_at, expires_at, heartbeat_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                lease.id.to_string(),
                lease.instance_id.to_string(),
                lease.work_item_id.to_string(),
                lease_state_name(lease.state),
                serde_json::to_string(lease)?,
                lease.acquired_at.to_rfc3339(),
                lease.expires_at.to_rfc3339(),
                lease.heartbeat_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically creates fresh running instance authority and its active work lease.
    /// Dependency completion and organization concurrency limits are checked before
    /// any projection or event is written.
    ///
    /// # Errors
    ///
    /// Returns an error for stale run state, incomplete dependencies, duplicate
    /// work, excessive concurrency, inconsistent authority, or failed persistence.
    pub fn dispatch_agent_work(
        &mut self,
        created: &AgentInstance,
        lease: &AgentLease,
        at: DateTime<Utc>,
    ) -> Result<(AgentInstance, Vec<EventEnvelope>), StorageError> {
        self.dispatch_agent_work_with_retry_policy(created, lease, at, false)
    }

    /// Dispatches fresh authority after an authenticated owner explicitly asks
    /// to retry. This bypasses only the automatic-attempt ceiling; active or
    /// completed authority, dependencies, concurrency, capabilities, and all
    /// other lease invariants remain enforced.
    ///
    /// # Errors
    ///
    /// Returns the same authority and persistence errors as ordinary dispatch.
    pub fn dispatch_agent_work_for_explicit_retry(
        &mut self,
        created: &AgentInstance,
        lease: &AgentLease,
        at: DateTime<Utc>,
    ) -> Result<(AgentInstance, Vec<EventEnvelope>), StorageError> {
        self.dispatch_agent_work_with_retry_policy(created, lease, at, true)
    }

    #[allow(clippy::too_many_lines)] // Keep all authority validation and atomic writes visible together.
    fn dispatch_agent_work_with_retry_policy(
        &mut self,
        created: &AgentInstance,
        lease: &AgentLease,
        at: DateTime<Utc>,
        explicit_owner_retry: bool,
    ) -> Result<(AgentInstance, Vec<EventEnvelope>), StorageError> {
        let run = self
            .agent_run(created.run_id)?
            .ok_or(StorageError::AgentRunNotFound(created.run_id))?;
        if run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "agent work can only dispatch from a running run".into(),
            ));
        }
        let plan = self
            .work_plan(created.run_id)?
            .ok_or_else(|| StorageError::InvalidData("agent run has no work plan".into()))?;
        let organization = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let item = plan
            .items
            .iter()
            .find(|item| item.id == lease.work_item_id)
            .ok_or_else(|| {
                StorageError::InvalidData("lease references unknown work item".into())
            })?;
        let represented_roles: i64 = self.connection.query_row(
            "SELECT COUNT(DISTINCT role_id) FROM agent_instances WHERE run_id = ?1",
            [created.run_id.to_string()],
            |row| row.get(0),
        )?;
        let role_already_represented: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM agent_instances WHERE run_id = ?1 AND role_id = ?2
             )",
            params![created.run_id.to_string(), created.role_id.to_string()],
            |row| row.get(0),
        )?;
        let active_instances: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM agent_instances
             WHERE run_id = ?1 AND state IN ('created', 'running', 'waiting', 'blocked')",
            [created.run_id.to_string()],
            |row| row.get(0),
        )?;
        if !role_already_represented
            && represented_roles >= i64::from(organization.policy.max_total_agents)
        {
            return Err(StorageError::Conflict(
                "organization total role limit is exhausted".into(),
            ));
        }
        if active_instances >= i64::from(organization.policy.max_parallel_agents) {
            return Err(StorageError::Conflict(
                "organization parallel agent limit is exhausted".into(),
            ));
        }
        let prior_authority: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM agent_leases
                INNER JOIN agent_instances
                    ON agent_instances.instance_id = agent_leases.instance_id
                WHERE agent_instances.run_id = ?1 AND agent_leases.work_item_id = ?2
                  AND agent_leases.state IN ('active', 'completed')
             )",
            params![created.run_id.to_string(), lease.work_item_id.to_string()],
            |row| row.get(0),
        )?;
        if prior_authority {
            return Err(StorageError::Conflict(
                "work item is already active or complete".into(),
            ));
        }
        let prior_attempts: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1 AND agent_leases.work_item_id = ?2",
            params![created.run_id.to_string(), lease.work_item_id.to_string()],
            |row| row.get(0),
        )?;
        if !explicit_owner_retry && prior_attempts > i64::from(lease.max_retries) {
            return Err(StorageError::Conflict(
                "work item retry limit is exhausted".into(),
            ));
        }
        for dependency in &item.dependencies {
            let completed: bool = self.connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM agent_leases
                    INNER JOIN agent_instances
                        ON agent_instances.instance_id = agent_leases.instance_id
                    INNER JOIN agent_turn_results
                        ON agent_turn_results.lease_id = agent_leases.lease_id
                    WHERE agent_instances.run_id = ?1
                      AND agent_leases.work_item_id = ?2
                      AND agent_leases.state = 'completed'
                      AND agent_instances.state = 'completed'
                 )",
                params![created.run_id.to_string(), dependency.to_string()],
                |row| row.get(0),
            )?;
            if !completed {
                return Err(StorageError::Conflict(format!(
                    "work item dependency {dependency} is not complete"
                )));
            }
        }
        let lease_seconds = i64::try_from(organization.policy.default_lease_seconds)
            .map_err(|_| StorageError::InvalidData("default agent lease is too long".into()))?;
        let maximum_expiry = at
            .checked_add_signed(chrono::Duration::seconds(lease_seconds))
            .ok_or_else(|| {
                StorageError::InvalidData("default agent lease overflows time".into())
            })?;
        if created.schema_version != AGENT_RUN_SCHEMA_VERSION
            || created.state != AgentState::Created
            || created.task_id != run.task_id
            || created.role_id != item.assigned_role
            || created.context_fingerprint.trim().is_empty()
            || created.confidence.basis_points > 10_000
            || lease.schema_version != AGENT_RUN_SCHEMA_VERSION
            || lease.instance_id != created.id
            || lease.state != AgentLeaseState::Active
            || lease.goal != item.goal
            || lease.budget.limit != item.budget
            || lease.expires_at <= at
            || lease.expires_at > maximum_expiry
            || lease.acquired_at != at
            || lease.heartbeat_at != at
            || lease.reporting_interval_seconds == 0
            || lease.max_retries > 10
            || !budget_values_valid(&lease.budget.limit)
            || lease.budget.consumed_tokens != 0
            || lease.budget.consumed_cost_usd != 0.0
            || lease.budget.elapsed_seconds != 0
        {
            return Err(StorageError::InvalidData(
                "dispatched agent authority is inconsistent with its run and work item".into(),
            ));
        }
        let mut running = created.clone();
        running.state = AgentState::Running;
        running.updated_at = at;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_instances(
                instance_id, run_id, task_id, role_id, state, instance_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)",
            params![
                running.id.to_string(),
                running.run_id.to_string(),
                running.task_id.to_string(),
                running.role_id.to_string(),
                serde_json::to_string(&running)?,
                running.created_at.to_rfc3339(),
                running.updated_at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "INSERT INTO agent_leases(
                lease_id, instance_id, work_item_id, state, lease_json,
                acquired_at, expires_at, heartbeat_at
             ) VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7)",
            params![
                lease.id.to_string(),
                lease.instance_id.to_string(),
                lease.work_item_id.to_string(),
                serde_json::to_string(lease)?,
                lease.acquired_at.to_rfc3339(),
                lease.expires_at.to_rfc3339(),
                lease.heartbeat_at.to_rfc3339(),
            ],
        )?;
        let events = vec![
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceRecorded,
                    Some(created.task_id),
                    serde_json::to_value(created)?,
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(created.task_id),
                    serde_json::json!({"from": AgentState::Created, "to": AgentState::Running, "instance": &running}),
                ),
            )?,
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseRecorded,
                    Some(created.task_id),
                    serde_json::to_value(lease)?,
                ),
            )?,
        ];
        transaction.commit()?;
        Ok((running, events))
    }

    /// Loads a current agent lease projection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_lease(&self, lease_id: AgentLeaseId) -> Result<Option<AgentLease>, StorageError> {
        self.read_optional_json(
            "SELECT lease_json FROM agent_leases WHERE lease_id = ?1",
            &lease_id.to_string(),
        )
    }

    /// Lists current leases for every instance in one run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_leases_for_run(&self, run_id: RunId) -> Result<Vec<AgentLease>, StorageError> {
        self.read_json_rows(
            "SELECT agent_leases.lease_json FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1
             ORDER BY agent_leases.acquired_at, agent_leases.lease_id",
            &run_id.to_string(),
        )
    }

    /// Finishes an active lease with its final measured budget consumption.
    ///
    /// # Errors
    ///
    /// Only an active lease may transition and consumed values must be finite.
    pub fn finish_agent_lease(
        &mut self,
        lease_id: AgentLeaseId,
        next: AgentLeaseState,
        consumed_tokens: u64,
        consumed_cost_usd: f64,
        elapsed_seconds: u64,
        at: DateTime<Utc>,
    ) -> Result<(AgentLease, EventEnvelope), StorageError> {
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        if lease.state != AgentLeaseState::Active
            || next == AgentLeaseState::Active
            || !consumed_cost_usd.is_finite()
            || consumed_cost_usd < 0.0
            || at < lease.acquired_at
        {
            return Err(StorageError::Conflict(
                "agent lease cannot make the requested terminal transition".into(),
            ));
        }
        let instance = self
            .agent_instance(lease.instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(lease.instance_id))?;
        lease.state = next;
        lease.budget.consumed_tokens = consumed_tokens;
        lease.budget.consumed_cost_usd = consumed_cost_usd;
        lease.budget.elapsed_seconds = elapsed_seconds;
        lease.heartbeat_at = at;
        let event = EventEnvelope::new(
            EventKind::AgentLeaseStateChanged,
            Some(instance.task_id),
            serde_json::to_value(&lease)?,
        );
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = ?1, lease_json = ?2, heartbeat_at = ?3
             WHERE lease_id = ?4 AND state = 'active'",
            params![
                lease_state_name(next),
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease_id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed concurrently")?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((lease, event))
    }

    /// Atomically commits a completed model/tool turn, attributed usage, current
    /// run metrics, and terminal instance/lease projections.
    ///
    /// # Errors
    ///
    /// The run, instance, and lease must be active and mutually consistent. Any
    /// invalid evidence or failed write rolls back the complete turn commit.
    #[allow(clippy::too_many_lines)] // Transaction order is the crash-safety invariant.
    pub fn commit_agent_turn(
        &mut self,
        instance_id: AgentInstanceId,
        lease_id: AgentLeaseId,
        result: &AgentTurnResult,
        confidence: ConfidenceEstimate,
        shadow_experiment_id: Option<ExperimentId>,
        at: DateTime<Utc>,
    ) -> Result<AgentTurnCommit, StorageError> {
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        let mut run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        let run_revision = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let active_revision = self
            .organization(run_revision.organization_id)?
            .ok_or_else(|| StorageError::Conflict("active organization was not found".into()))?;
        let evaluation_only = active_revision.revision_id != run_revision.revision_id;
        let isolated_shadow_authorized = if evaluation_only && !result.tool_outcomes.is_empty() {
            let experiment_id = shadow_experiment_id.ok_or_else(|| {
                StorageError::InvalidData(
                    "inactive candidate tool outcomes require isolated shadow authority".into(),
                )
            })?;
            let experiment = self.experiment(experiment_id)?.ok_or_else(|| {
                StorageError::Conflict("shadow authority experiment was not found".into())
            })?;
            let workspace = self.shadow_workspace(experiment_id)?.ok_or_else(|| {
                StorageError::Conflict("shadow authority workspace was not found".into())
            })?;
            let task = self
                .load_task(run.task_id)?
                .ok_or(StorageError::TaskNotFound(run.task_id))?;
            experiment.state == ExperimentState::Running
                && experiment.candidate_revision_id == run.organization_revision_id
                && workspace.state == ShadowWorkspaceState::Ready
                && task.project_id == Some(workspace.project_id)
        } else {
            false
        };
        if instance.state != AgentState::Running
            || lease.state != AgentLeaseState::Active
            || lease.instance_id != instance.id
            || run.state != AgentRunState::Running
            || run.task_id != instance.task_id
            || confidence.basis_points > 10_000
            || at < lease.acquired_at
            || result.model_calls.is_empty()
            || result.response.id.trim().is_empty()
            || result.response.model != result.pinned_route.target.model_id
            || (evaluation_only && !result.tool_outcomes.is_empty() && !isolated_shadow_authorized)
            || result
                .model_calls
                .iter()
                .any(|call| call.target != result.pinned_route.target)
        {
            return Err(StorageError::InvalidData(
                "completed agent turn authority, identity, or evidence is inconsistent".into(),
            ));
        }

        let usage = agent_turn_usage_records(
            &run,
            instance.id,
            &result.model_calls,
            &result.tool_outcomes,
            at,
        );
        if usage.iter().any(|record| {
            record.cached_input_tokens > record.input_tokens
                || record
                    .cost_usd
                    .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        }) {
            return Err(StorageError::InvalidData(
                "completed agent turn contains invalid usage values".into(),
            ));
        }

        let turn_tokens = result
            .model_calls
            .iter()
            .map(|call| call.usage.total_tokens)
            .sum::<u64>();
        let turn_cost = result
            .model_calls
            .iter()
            .filter_map(|call| call.cost_usd)
            .sum::<f64>();
        let elapsed_seconds =
            u64::try_from((at - lease.acquired_at).num_seconds().max(0)).unwrap_or(u64::MAX);
        lease.state = AgentLeaseState::Completed;
        lease.budget.consumed_tokens = lease.budget.consumed_tokens.saturating_add(turn_tokens);
        lease.budget.consumed_cost_usd += turn_cost;
        lease.budget.elapsed_seconds = lease.budget.elapsed_seconds.max(elapsed_seconds);
        lease.heartbeat_at = at;
        instance.state = AgentState::Completed;
        instance.model = Some(AssignedModel {
            provider_connection_id: result.pinned_route.target.provider_id.clone(),
            model_id: result.pinned_route.target.model_id.clone(),
            route_reason: "pinned by the bounded route plan".into(),
        });
        instance.confidence = confidence;
        instance.updated_at = at;
        run.metrics.model_calls = run
            .metrics
            .model_calls
            .saturating_add(u32::try_from(result.model_calls.len()).unwrap_or(u32::MAX));
        run.metrics.tool_calls = run
            .metrics
            .tool_calls
            .saturating_add(u32::try_from(result.tool_outcomes.len()).unwrap_or(u32::MAX));
        run.metrics.tokens = run.metrics.tokens.saturating_add(turn_tokens);
        run.metrics.cost_usd += turn_cost;
        run.metrics.useful_work_seconds = run
            .metrics
            .useful_work_seconds
            .saturating_add(elapsed_seconds);
        run.updated_at = at;
        if !metrics_valid(&run.metrics) || !lease.budget.consumed_cost_usd.is_finite() {
            return Err(StorageError::InvalidData(
                "completed agent turn exceeds numeric storage bounds".into(),
            ));
        }

        let transaction = self.connection.transaction()?;
        settle_attached_agent_turn_workflow_in_transaction(
            &transaction,
            lease.id,
            AgentTurnWorkflowSettlement::WorkCompleted,
            at,
        )?;
        let mut events = Vec::new();
        for record in &usage {
            insert_agent_usage(&transaction, record)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentUsageRecorded,
                    Some(run.task_id),
                    serde_json::to_value(record)?,
                ),
            )?);
        }
        transaction.execute(
            "INSERT INTO agent_turn_results(
                lease_id, run_id, instance_id, task_id, result_json, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                lease.id.to_string(),
                run.id.to_string(),
                instance.id.to_string(),
                run.task_id.to_string(),
                serde_json::to_string(result)?,
                at.to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM agent_turn_checkpoints WHERE lease_id = ?1",
            [lease.id.to_string()],
        )?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = 'completed', lease_json = ?1, heartbeat_at = ?2
             WHERE lease_id = ?3 AND state = 'active'",
            params![
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed before turn commit")?;
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = 'completed', instance_json = ?1, updated_at = ?2
             WHERE instance_id = ?3 AND state = 'running'",
            params![
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed before turn commit")?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET run_json = ?1, updated_at = ?2
             WHERE run_id = ?3 AND state = 'running'",
            params![
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent run changed before turn commit")?;
        for (kind, payload) in [
            (
                EventKind::AgentLeaseStateChanged,
                serde_json::to_value(&lease)?,
            ),
            (
                EventKind::AgentInstanceStateChanged,
                serde_json::to_value(&instance)?,
            ),
            (
                EventKind::AgentRunMetricsUpdated,
                serde_json::to_value(&run)?,
            ),
            (EventKind::AgentTurnCompleted, serde_json::to_value(result)?),
        ] {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(kind, Some(run.task_id), payload),
            )?);
        }
        transaction.commit()?;
        Ok(AgentTurnCommit {
            run,
            instance,
            lease,
            usage,
            result: result.clone(),
            events,
        })
    }

    /// Loads the immutable completed turn result for an agent lease.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_turn_result(
        &self,
        lease_id: AgentLeaseId,
    ) -> Result<Option<AgentTurnResult>, StorageError> {
        self.read_optional_json(
            "SELECT result_json FROM agent_turn_results WHERE lease_id = ?1",
            &lease_id.to_string(),
        )
    }

    /// Replaces the live durable continuation point for one active agent lease.
    ///
    /// # Errors
    ///
    /// The checkpoint must match one active instance/lease/run authority chain.
    pub fn save_agent_turn_checkpoint(
        &mut self,
        checkpoint: &AgentTurnCheckpoint,
    ) -> Result<(), StorageError> {
        let instance = self.agent_instance(checkpoint.request.instance.id)?.ok_or(
            StorageError::AgentInstanceNotFound(checkpoint.request.instance.id),
        )?;
        let lease = self.agent_lease(checkpoint.request.lease.id)?.ok_or(
            StorageError::AgentLeaseNotFound(checkpoint.request.lease.id),
        )?;
        let run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        if checkpoint.schema_version != AGENT_RUN_SCHEMA_VERSION
            || lease.instance_id != instance.id
            || checkpoint.request.instance.run_id != run.id
            || checkpoint.request.instance.task_id != run.task_id
            || checkpoint.request.lease.instance_id != instance.id
            || !matches!(instance.state, AgentState::Running | AgentState::Waiting)
            || lease.state != AgentLeaseState::Active
            || !matches!(run.state, AgentRunState::Running | AgentRunState::Waiting)
            || checkpoint.model_calls.is_empty()
            || checkpoint.response.model != checkpoint.pinned_route.target.model_id
        {
            return Err(StorageError::InvalidData(
                "agent turn checkpoint authority or provider evidence is inconsistent".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO agent_turn_checkpoints(
                lease_id, run_id, instance_id, task_id, pending_tool_request_id,
                checkpoint_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(lease_id) DO UPDATE SET
                pending_tool_request_id = excluded.pending_tool_request_id,
                checkpoint_json = excluded.checkpoint_json,
                updated_at = excluded.updated_at",
            params![
                lease.id.to_string(),
                run.id.to_string(),
                instance.id.to_string(),
                run.task_id.to_string(),
                checkpoint.pending_tool_request_id.map(|id| id.to_string()),
                serde_json::to_string(checkpoint)?,
                checkpoint.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Loads the current continuation point for an agent lease.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid persisted JSON.
    pub fn agent_turn_checkpoint(
        &self,
        lease_id: AgentLeaseId,
    ) -> Result<Option<AgentTurnCheckpoint>, StorageError> {
        self.read_optional_json(
            "SELECT checkpoint_json FROM agent_turn_checkpoints WHERE lease_id = ?1",
            &lease_id.to_string(),
        )
    }

    /// Atomically settles a turn that could not complete. A failed turn moves
    /// any already-checkpointed provider/tool evidence into immutable usage and
    /// metrics before deleting its continuation state; it never invents a
    /// successful result.
    ///
    /// # Errors
    ///
    /// The instance and lease must still hold active authority. Waiting retains
    /// the lease for explicit approval resume; failed work terminates it.
    #[allow(clippy::too_many_lines)] // State and evidence writes form one visible transaction.
    pub fn settle_agent_turn_error(
        &mut self,
        instance_id: AgentInstanceId,
        lease_id: AgentLeaseId,
        disposition: AgentTurnDisposition,
        reason_code: &str,
        at: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        if reason_code.trim().is_empty() || reason_code.len() > 100 {
            return Err(StorageError::InvalidData(
                "agent turn failure reason code is invalid".into(),
            ));
        }
        let mut instance = self
            .agent_instance(instance_id)?
            .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
        let mut lease = self
            .agent_lease(lease_id)?
            .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
        let mut run = self
            .agent_run(instance.run_id)?
            .ok_or(StorageError::AgentRunNotFound(instance.run_id))?;
        let run_revision = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let active_revision = self
            .organization(run_revision.organization_id)?
            .ok_or_else(|| StorageError::Conflict("active organization was not found".into()))?;
        let evaluation_only = active_revision.revision_id != run_revision.revision_id;
        if instance.state != AgentState::Running
            || lease.state != AgentLeaseState::Active
            || lease.instance_id != instance.id
            || run.state != AgentRunState::Running
            || at < lease.acquired_at
        {
            return Err(StorageError::Conflict(
                "agent turn authority changed before error settlement".into(),
            ));
        }
        let checkpoint = if disposition == AgentTurnDisposition::Failed {
            self.agent_turn_checkpoint(lease.id)?
        } else {
            None
        };
        let usage = apply_checkpointed_turn_accounting(
            checkpoint.as_ref(),
            &mut run,
            &mut instance,
            &mut lease,
            at,
        )?;
        let (instance_state, lease_state, run_state, event_kind) = match disposition {
            AgentTurnDisposition::Waiting => (
                AgentState::Waiting,
                AgentLeaseState::Active,
                AgentRunState::Waiting,
                EventKind::AgentTurnWaiting,
            ),
            AgentTurnDisposition::Failed => (
                AgentState::Failed,
                AgentLeaseState::Failed,
                AgentRunState::Running,
                EventKind::AgentTurnFailed,
            ),
        };
        instance.state = instance_state;
        instance.updated_at = at;
        lease.state = lease_state;
        lease.heartbeat_at = at;
        run.state = run_state;
        run.updated_at = at;
        let mut task_transition = None;
        if !evaluation_only
            && let Some(mut task) = self.load_task(run.task_id)?
            && task.state == TaskState::Active
        {
            let next = match disposition {
                AgentTurnDisposition::Waiting => TaskState::WaitingForApproval,
                AgentTurnDisposition::Failed => TaskState::Blocked,
            };
            task.transition_to(next)?;
            task.updated_at = at;
            task_transition = Some((task, next));
        }
        let transaction = self.connection.transaction()?;
        settle_attached_agent_turn_workflow_in_transaction(
            &transaction,
            lease.id,
            match disposition {
                AgentTurnDisposition::Waiting => AgentTurnWorkflowSettlement::WaitingForApproval,
                AgentTurnDisposition::Failed => AgentTurnWorkflowSettlement::Failed,
            },
            at,
        )?;
        let mut events = Vec::new();
        for record in &usage {
            insert_agent_usage(&transaction, record)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentUsageRecorded,
                    Some(run.task_id),
                    serde_json::to_value(record)?,
                ),
            )?);
        }
        if disposition == AgentTurnDisposition::Failed {
            transaction.execute(
                "DELETE FROM agent_turn_checkpoints WHERE lease_id = ?1",
                [lease.id.to_string()],
            )?;
        }
        let changed = transaction.execute(
            "UPDATE agent_instances SET state = ?1, instance_json = ?2, updated_at = ?3
             WHERE instance_id = ?4 AND state = 'running'",
            params![
                instance_state_name(instance.state),
                serde_json::to_string(&instance)?,
                at.to_rfc3339(),
                instance.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent instance changed before error settlement")?;
        let changed = transaction.execute(
            "UPDATE agent_leases SET state = ?1, lease_json = ?2, heartbeat_at = ?3
             WHERE lease_id = ?4 AND state = 'active'",
            params![
                lease_state_name(lease.state),
                serde_json::to_string(&lease)?,
                at.to_rfc3339(),
                lease.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent lease changed before error settlement")?;
        let changed = transaction.execute(
            "UPDATE agent_runs SET state = ?1, run_json = ?2, updated_at = ?3
             WHERE run_id = ?4 AND state = 'running'",
            params![
                run_state_name(run.state),
                serde_json::to_string(&run)?,
                at.to_rfc3339(),
                run.id.to_string(),
            ],
        )?;
        ensure_single_change(changed, "agent run changed before error settlement")?;
        events.push(Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::AgentInstanceStateChanged,
                Some(run.task_id),
                serde_json::to_value(&instance)?,
            ),
        )?);
        if disposition == AgentTurnDisposition::Failed {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    Some(run.task_id),
                    serde_json::to_value(&lease)?,
                ),
            )?);
            if !usage.is_empty() {
                events.push(Self::insert_event(
                    &transaction,
                    &EventEnvelope::new(
                        EventKind::AgentRunMetricsUpdated,
                        Some(run.task_id),
                        serde_json::to_value(&run)?,
                    ),
                )?);
            }
        } else {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(run.task_id),
                    serde_json::to_value(&run)?,
                ),
            )?);
        }
        events.push(Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                event_kind,
                Some(run.task_id),
                serde_json::json!({
                    "run_id": run.id,
                    "instance_id": instance.id,
                    "lease_id": lease.id,
                    "reason_code": reason_code,
                }),
            ),
        )?);
        if let Some((task, next)) = task_transition {
            let event = Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::TaskStateChanged,
                    Some(run.task_id),
                    serde_json::to_value(TaskStateChangedPayload {
                        from: TaskState::Active,
                        to: next,
                    })?,
                ),
            )?;
            Self::upsert_task(&transaction, &task, event.sequence)?;
            events.push(event);
        }
        transaction.commit()?;
        Ok(events)
    }

    /// Records immutable provider, tool, or coordination usage evidence.
    ///
    /// # Errors
    ///
    /// Usage identity, kind-specific attribution, and numeric values are validated.
    pub fn record_agent_usage(
        &mut self,
        usage: &AgentUsageRecord,
    ) -> Result<EventEnvelope, StorageError> {
        let run = self
            .agent_run(usage.run_id)?
            .ok_or(StorageError::AgentRunNotFound(usage.run_id))?;
        let attribution_valid = match usage.kind {
            UsageKind::ProviderModel => {
                usage.provider_connection_id.is_some()
                    && usage.model_id.is_some()
                    && usage.tool_execution_id.is_none()
            }
            UsageKind::Tool => {
                usage.tool_execution_id.is_some()
                    && usage.provider_connection_id.is_none()
                    && usage.model_id.is_none()
            }
            UsageKind::Coordination => {
                usage.provider_connection_id.is_none()
                    && usage.model_id.is_none()
                    && usage.tool_execution_id.is_none()
            }
        };
        if usage.schema_version != AGENT_RUN_SCHEMA_VERSION
            || usage.task_id != run.task_id
            || !attribution_valid
            || usage.cached_input_tokens > usage.input_tokens
            || usage
                .cost_usd
                .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
        {
            return Err(StorageError::InvalidData(
                "agent usage attribution or numeric values are invalid".into(),
            ));
        }
        if let Some(instance_id) = usage.instance_id {
            let instance = self
                .agent_instance(instance_id)?
                .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
            if instance.run_id != usage.run_id {
                return Err(StorageError::InvalidData(
                    "agent usage instance belongs to a different run".into(),
                ));
            }
        }
        let event = EventEnvelope::new(
            EventKind::AgentUsageRecorded,
            Some(usage.task_id),
            serde_json::to_value(usage)?,
        );
        let transaction = self.connection.transaction()?;
        insert_agent_usage(&transaction, usage)?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Lists immutable usage evidence for a run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn agent_usage(&self, run_id: RunId) -> Result<Vec<AgentUsageRecord>, StorageError> {
        self.read_json_rows(
            "SELECT usage_json FROM agent_usage WHERE run_id = ?1
             ORDER BY occurred_at, usage_id",
            &run_id.to_string(),
        )
    }

    /// Records an immutable evidence-backed evaluation report.
    ///
    /// # Errors
    ///
    /// The report must identify its run/task and satisfy the genome evidence policy.
    pub fn record_evaluation(
        &mut self,
        report: &EvaluationReport,
    ) -> Result<EventEnvelope, StorageError> {
        self.validate_evaluation_report(report)?;
        let event = EventEnvelope::new(
            EventKind::EvaluationRecorded,
            Some(report.task_id),
            serde_json::to_value(report)?,
        );
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO evaluations(
                evaluation_id, run_id, task_id, evaluation_json, evaluated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                report.id.to_string(),
                report.run_id.to_string(),
                report.task_id.to_string(),
                serde_json::to_string(report)?,
                report.evaluated_at.to_rfc3339(),
            ],
        )?;
        let event = Self::insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Loads one immutable evaluation report.
    ///
    /// # Errors
    /// Returns an error when persisted JSON cannot be decoded.
    pub fn evaluation(
        &self,
        evaluation_id: mcg_domain::EvaluationId,
    ) -> Result<Option<EvaluationReport>, StorageError> {
        self.read_optional_json(
            "SELECT evaluation_json FROM evaluations WHERE evaluation_id = ?1",
            &evaluation_id.to_string(),
        )
    }

    /// Atomically freezes the stable objective report and its resumable Verifying checkpoint
    /// before any independent verifier can consume that evidence.
    ///
    /// # Errors
    /// Returns an error for incomplete work, stale task/run/checkpoint state, report identity
    /// substitution, an existing different report, or transaction failure.
    #[allow(clippy::too_many_lines)] // Keep the report/checkpoint evidence fence auditable.
    pub fn prepare_consumer_evaluation(
        &mut self,
        report: &EvaluationReport,
        checkpoint: &ConsumerEvaluationCheckpoint,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        let (run, plan) = self.validate_evaluation_report(report)?;
        if checkpoint.task_id != report.task_id
            || checkpoint.run_id != report.run_id
            || checkpoint.state != ConsumerEvaluationCheckpointState::Verifying
            || checkpoint.prepared_evaluation_id != Some(report.id)
            || checkpoint.prepared_evaluated_at != Some(report.evaluated_at)
            || checkpoint.request_id.is_some()
        {
            return Err(StorageError::Conflict(
                "prepared evaluation report does not match its Verifying checkpoint".into(),
            ));
        }
        if self.task_launch_run_id(report.task_id)? != Some(run.id) {
            return Err(StorageError::Conflict(
                "prepared evaluation run is not the task's consumer launch".into(),
            ));
        }
        let task = self
            .load_task(report.task_id)?
            .ok_or(StorageError::TaskNotFound(report.task_id))?;
        if task.state != TaskState::Active || run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "prepared evaluation requires an active task and running run".into(),
            ));
        }
        let completed_work = self
            .read_json_rows::<AgentLease>(
                "SELECT agent_leases.lease_json FROM agent_leases
                 INNER JOIN agent_instances
                    ON agent_instances.instance_id = agent_leases.instance_id
                 WHERE agent_instances.run_id = ?1 AND agent_leases.state = 'completed'",
                &run.id.to_string(),
            )?
            .into_iter()
            .map(|lease| lease.work_item_id)
            .collect::<std::collections::BTreeSet<_>>();
        if !plan
            .items
            .iter()
            .all(|item| completed_work.contains(&item.id))
        {
            return Err(StorageError::Conflict(
                "consumer evaluation cannot be prepared before every planned turn completes".into(),
            ));
        }
        let current = self
            .consumer_evaluation_checkpoint(report.task_id)?
            .ok_or_else(|| StorageError::Conflict("consumer checkpoint is missing".into()))?;
        let exact_replay = current == *checkpoint;
        let mut expected_running = checkpoint.clone();
        expected_running.state = ConsumerEvaluationCheckpointState::Running;
        expected_running.prepared_evaluation_id = None;
        expected_running.prepared_evaluated_at = None;
        expected_running.updated_at = current.updated_at;
        if exact_replay {
            let existing = self.evaluation(report.id)?.ok_or_else(|| {
                StorageError::Conflict("prepared evaluation report disappeared".into())
            })?;
            if existing != *report {
                return Err(StorageError::Conflict(
                    "prepared evaluation identity was replayed with different content".into(),
                ));
            }
            return Ok(Vec::new());
        }
        if current != expected_running {
            return Err(StorageError::Conflict(
                "consumer evaluation checkpoint changed before report preparation".into(),
            ));
        }
        if self.evaluation(report.id)?.is_some() {
            return Err(StorageError::Conflict(
                "prepared evaluation identity already belongs to a different checkpoint".into(),
            ));
        }

        let transaction = self.connection.transaction()?;
        let inserted = insert_or_validate_evaluation(&transaction, report)?;
        upsert_consumer_evaluation_checkpoint(&transaction, checkpoint)?;
        let mut events = vec![Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::EvaluationCheckpointed,
                Some(report.task_id),
                serde_json::to_value(checkpoint)?,
            ),
        )?];
        if inserted {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::EvaluationRecorded,
                    Some(report.task_id),
                    serde_json::to_value(report)?,
                ),
            )?);
        }
        transaction.commit()?;
        Ok(events)
    }

    /// Atomically records objective evaluation evidence and settles the
    /// consumer task/run projection from that evidence.
    ///
    /// Passed and failed reports terminally settle the run and task.
    /// Inconclusive reports leave the run retryable and block the task.
    ///
    /// # Errors
    ///
    /// Returns an error unless this is a running launched task whose every
    /// planned work item has completed durable agent authority.
    pub fn settle_consumer_evaluation(
        &mut self,
        report: &EvaluationReport,
        checkpoint: &ConsumerEvaluationCheckpoint,
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        self.settle_consumer_evaluation_with_completions(report, checkpoint, &[])
    }

    /// Atomically settles objective evaluation together with every attached compatibility
    /// workflow certificate and terminal transition.
    ///
    /// # Errors
    /// Returns an error for missing, duplicate, stale, substituted, or report-inconsistent
    /// completion evidence, or any ordinary consumer evaluation settlement failure.
    #[allow(clippy::too_many_lines)] // Evidence and all terminal projections are one transaction.
    pub fn settle_consumer_evaluation_with_completions(
        &mut self,
        report: &EvaluationReport,
        checkpoint: &ConsumerEvaluationCheckpoint,
        completions: &[ConsumerEvaluationCompletion],
    ) -> Result<Vec<EventEnvelope>, StorageError> {
        let (mut run, plan) = self.validate_evaluation_report(report)?;
        let prepared_matches = checkpoint.state == ConsumerEvaluationCheckpointState::Verifying
            && checkpoint.prepared_evaluation_id == Some(report.id)
            && checkpoint.prepared_evaluated_at == Some(report.evaluated_at);
        if checkpoint.task_id != report.task_id
            || checkpoint.run_id != report.run_id
            || (checkpoint.state != ConsumerEvaluationCheckpointState::Running && !prepared_matches)
        {
            return Err(StorageError::Conflict(
                "evaluation report does not match its running checkpoint".into(),
            ));
        }
        if self.task_launch_run_id(report.task_id)? != Some(run.id) {
            return Err(StorageError::Conflict(
                "evaluation run is not the task's consumer launch".into(),
            ));
        }
        let mut task = self
            .load_task(report.task_id)?
            .ok_or(StorageError::TaskNotFound(report.task_id))?;
        if task.state != TaskState::Active || run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "consumer evaluation requires an active task and running run".into(),
            ));
        }
        let completed_leases: Vec<AgentLease> = self.read_json_rows(
            "SELECT agent_leases.lease_json FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1 AND agent_leases.state = 'completed'
             ORDER BY agent_leases.acquired_at, agent_leases.lease_id",
            &run.id.to_string(),
        )?;
        let completed_work = completed_leases
            .iter()
            .map(|lease| lease.work_item_id)
            .collect::<std::collections::BTreeSet<_>>();
        if !plan
            .items
            .iter()
            .all(|item| completed_work.contains(&item.id))
        {
            return Err(StorageError::Conflict(
                "consumer evaluation cannot settle before every planned turn completes".into(),
            ));
        }
        let verification_targets = self.agent_turn_verification_targets(run.id)?;
        validate_consumer_evaluation_completions(report, &verification_targets, completions)?;
        let at = completions
            .iter()
            .map(|completion| completion.certificate.issued_at)
            .max()
            .unwrap_or(report.evaluated_at);
        let previous_run = run.state;
        let next_task = match report.outcome {
            mcg_agents::EvaluationOutcome::Passed => {
                run.state = AgentRunState::Completed;
                run.completed_at = Some(at);
                TaskState::Completed
            }
            mcg_agents::EvaluationOutcome::Failed => {
                run.state = AgentRunState::Failed;
                run.completed_at = Some(at);
                TaskState::Failed
            }
            mcg_agents::EvaluationOutcome::Inconclusive => TaskState::Blocked,
        };
        run.updated_at = at;
        task.transition_to(next_task)?;
        task.updated_at = at;
        let transaction = self.connection.transaction()?;
        for completion in completions {
            commit_completion_certificate_in_transaction(
                &transaction,
                &completion.certificate,
                completion.expected_revision,
                completion.expected_generation,
                at,
            )?;
        }
        let evaluation_inserted = insert_or_validate_evaluation(&transaction, report)?;
        let mut events = Vec::new();
        if report.outcome == mcg_agents::EvaluationOutcome::Inconclusive {
            let mut blocked = checkpoint.clone();
            blocked.state = ConsumerEvaluationCheckpointState::Blocked;
            blocked.attempt = blocked.attempt.saturating_add(1);
            blocked.request_id = None;
            blocked.updated_at = at;
            upsert_consumer_evaluation_checkpoint(&transaction, &blocked)?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::EvaluationCheckpointed,
                    Some(report.task_id),
                    serde_json::to_value(&blocked)?,
                ),
            )?);
        } else {
            let mut completed = checkpoint.clone();
            completed.state = ConsumerEvaluationCheckpointState::Completed;
            completed.request_id = None;
            completed.updated_at = at;
            upsert_consumer_evaluation_checkpoint(&transaction, &completed)?;
        }
        if evaluation_inserted {
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::EvaluationRecorded,
                    Some(report.task_id),
                    serde_json::to_value(report)?,
                ),
            )?);
        }
        if run.state != previous_run {
            let changed = transaction.execute(
                "UPDATE agent_runs SET state = ?1, run_json = ?2, updated_at = ?3,
                        completed_at = ?4 WHERE run_id = ?5 AND state = 'running'",
                params![
                    run_state_name(run.state),
                    serde_json::to_string(&run)?,
                    at.to_rfc3339(),
                    run.completed_at.map(|value| value.to_rfc3339()),
                    run.id.to_string(),
                ],
            )?;
            ensure_single_change(changed, "consumer run changed before evaluation settlement")?;
            events.push(Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(report.task_id),
                    serde_json::json!({"from": previous_run, "to": run.state, "run": &run, "reason": "objective_evaluation"}),
                ),
            )?);
        }
        let task_event = Self::insert_event(
            &transaction,
            &EventEnvelope::new(
                EventKind::TaskStateChanged,
                Some(report.task_id),
                serde_json::to_value(TaskStateChangedPayload {
                    from: TaskState::Active,
                    to: next_task,
                })?,
            ),
        )?;
        Self::upsert_task(&transaction, &task, task_event.sequence)?;
        events.push(task_event);
        transaction.commit()?;
        Ok(events)
    }

    fn validate_evaluation_report(
        &self,
        report: &EvaluationReport,
    ) -> Result<(AgentRun, WorkPlan), StorageError> {
        let run = self
            .agent_run(report.run_id)?
            .ok_or(StorageError::AgentRunNotFound(report.run_id))?;
        let genome = self
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let plan = self
            .work_plan(report.run_id)?
            .ok_or(StorageError::InvalidData(
                "evaluated agent run has no work plan".into(),
            ))?;
        let covers_plan = plan.items.iter().all(|item| {
            item.success_conditions.iter().all(|required| {
                report
                    .conditions
                    .iter()
                    .any(|result| result.condition == *required)
            })
        });
        let outcome_consistent = match report.outcome {
            mcg_agents::EvaluationOutcome::Passed => {
                report.conditions.iter().all(|condition| condition.passed)
            }
            mcg_agents::EvaluationOutcome::Failed => {
                report.conditions.iter().any(|condition| !condition.passed)
            }
            mcg_agents::EvaluationOutcome::Inconclusive => true,
        };
        let plan_requires_independent_evaluation = genome.evaluation.require_independent_evaluation
            || plan.items.iter().any(|item| {
                genome.roles.iter().any(|role| {
                    role.id == item.assigned_role && role.kind == mcg_agents::RoleKind::Evaluator
                })
            });
        let independent_evaluator_present =
            !plan_requires_independent_evaluation || report.evaluator_instance_id.is_some();
        if report.schema_version != AGENT_RUN_SCHEMA_VERSION
            || report.task_id != run.task_id
            || report.conditions.is_empty()
            || (genome.evaluation.require_objective_evidence && report.evidence.is_empty())
            || !independent_evaluator_present
            || report.confidence.basis_points > 10_000
            || !covers_plan
            || !outcome_consistent
        {
            return Err(StorageError::InvalidData(
                "evaluation does not satisfy its run or objective evidence policy".into(),
            ));
        }
        if let Some(instance_id) = report.evaluator_instance_id {
            let instance = self
                .agent_instance(instance_id)?
                .ok_or(StorageError::AgentInstanceNotFound(instance_id))?;
            if instance.run_id != report.run_id {
                return Err(StorageError::InvalidData(
                    "evaluation instance belongs to a different run".into(),
                ));
            }
            let evaluator = genome.roles.iter().any(|role| {
                role.id == instance.role_id && role.kind == mcg_agents::RoleKind::Evaluator
            });
            if !evaluator {
                return Err(StorageError::InvalidData(
                    "evaluation instance does not have the Evaluator role".into(),
                ));
            }
        }
        Ok((run, plan))
    }

    /// Lists immutable evaluation reports for a run.
    ///
    /// # Errors
    ///
    /// Returns an error when a row cannot be decoded.
    pub fn evaluations(&self, run_id: RunId) -> Result<Vec<EvaluationReport>, StorageError> {
        self.read_json_rows(
            "SELECT evaluation_json FROM evaluations WHERE run_id = ?1
             ORDER BY evaluated_at, evaluation_id",
            &run_id.to_string(),
        )
    }

    /// Converts interrupted run work into explicit recovering/expired states and
    /// appends an event for every recovered projection update.
    ///
    /// # Errors
    ///
    /// Recovery is atomic and fails without partial state changes.
    #[allow(clippy::too_many_lines)] // Recovery classification and projection changes share one transaction.
    pub fn recover_agent_work(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<AgentRecoveryReport, StorageError> {
        let checkpoints = self.read_all_json::<AgentTurnCheckpoint>(
            "SELECT checkpoint_json FROM agent_turn_checkpoints ORDER BY lease_id",
        )?;
        let approvals = self.read_all_json::<ApprovalRequest>(
            "SELECT approval_json FROM approvals WHERE state IN ('pending', 'approved') ORDER BY approval_id",
        )?;
        let resumable = checkpoints
            .iter()
            .filter(|checkpoint| {
                checkpoint
                    .pending_tool_request_id
                    .is_some_and(|request_id| {
                        approvals.iter().any(|approval| {
                            approval.tool_request_id == request_id
                                && approval.task_id == checkpoint.request.instance.task_id
                                && matches!(
                                    approval.state,
                                    ApprovalState::Pending | ApprovalState::Approved
                                )
                                && approval.expires_at > at
                        })
                    })
                    && checkpoint.request.lease.expires_at > at
            })
            .collect::<Vec<_>>();
        let resumable_runs = resumable
            .iter()
            .map(|checkpoint| checkpoint.request.instance.run_id)
            .collect::<std::collections::BTreeSet<_>>();
        let resumable_instances = resumable
            .iter()
            .map(|checkpoint| checkpoint.request.instance.id)
            .collect::<std::collections::BTreeSet<_>>();
        let resumable_leases = resumable
            .iter()
            .map(|checkpoint| checkpoint.request.lease.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut runs = self.read_all_json::<AgentRun>(
            "SELECT run_json FROM agent_runs WHERE state IN ('running', 'waiting') ORDER BY run_id",
        )?;
        let mut instances = self.read_all_json::<AgentInstance>(
            "SELECT instance_json FROM agent_instances
             WHERE state IN ('running', 'waiting', 'blocked') ORDER BY instance_id",
        )?;
        let mut leases = self.read_all_json::<AgentLease>(
            "SELECT lease_json FROM agent_leases WHERE state = 'active' ORDER BY lease_id",
        )?;
        runs.retain(|run| run.state != AgentRunState::Waiting || !resumable_runs.contains(&run.id));
        instances.retain(|instance| {
            instance.state != AgentState::Waiting || !resumable_instances.contains(&instance.id)
        });
        leases.retain(|lease| !resumable_leases.contains(&lease.id));
        let lease_instance_tasks = self
            .read_all_json::<AgentInstance>(
                "SELECT DISTINCT agent_instances.instance_json FROM agent_instances
                 INNER JOIN agent_leases ON agent_leases.instance_id = agent_instances.instance_id
                 WHERE agent_leases.state = 'active' ORDER BY agent_instances.instance_id",
            )?
            .into_iter()
            .map(|instance| (instance.id, instance.task_id))
            .collect::<std::collections::BTreeMap<_, _>>();
        let transaction = self.connection.transaction()?;
        for run in &mut runs {
            let previous = run.state;
            run.state = AgentRunState::Recovering;
            run.updated_at = at;
            transaction.execute(
                "UPDATE agent_runs SET state = 'recovering', run_json = ?1, updated_at = ?2
                 WHERE run_id = ?3 AND state = ?4",
                params![
                    serde_json::to_string(run)?,
                    at.to_rfc3339(),
                    run.id.to_string(),
                    run_state_name(previous),
                ],
            )?;
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentRunStateChanged,
                    Some(run.task_id),
                    serde_json::json!({"from": previous, "to": AgentRunState::Recovering, "run": run, "reason": "daemon_restart"}),
                ),
            )?;
        }
        for instance in &mut instances {
            let previous = instance.state;
            instance.state = AgentState::Expired;
            instance.updated_at = at;
            transaction.execute(
                "UPDATE agent_instances SET state = 'expired', instance_json = ?1, updated_at = ?2
                 WHERE instance_id = ?3 AND state = ?4",
                params![
                    serde_json::to_string(instance)?,
                    at.to_rfc3339(),
                    instance.id.to_string(),
                    instance_state_name(previous),
                ],
            )?;
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentInstanceStateChanged,
                    Some(instance.task_id),
                    serde_json::json!({"from": previous, "to": AgentState::Expired, "instance": instance, "reason": "daemon_restart"}),
                ),
            )?;
        }
        for lease in &mut leases {
            lease.state = AgentLeaseState::Expired;
            lease.heartbeat_at = at;
            transaction.execute(
                "UPDATE agent_leases SET state = 'expired', lease_json = ?1, heartbeat_at = ?2
                 WHERE lease_id = ?3 AND state = 'active'",
                params![
                    serde_json::to_string(lease)?,
                    at.to_rfc3339(),
                    lease.id.to_string(),
                ],
            )?;
            let task_id = lease_instance_tasks.get(&lease.instance_id).copied();
            Self::insert_event(
                &transaction,
                &EventEnvelope::new(
                    EventKind::AgentLeaseStateChanged,
                    task_id,
                    serde_json::json!({"lease": lease, "reason": "daemon_restart"}),
                ),
            )?;
        }
        transaction.commit()?;
        Ok(AgentRecoveryReport {
            runs_marked_recovering: runs.len() as u64,
            instances_expired: instances.len() as u64,
            leases_expired: leases.len() as u64,
        })
    }

    pub(crate) fn read_optional_json<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        parameter: &str,
    ) -> Result<Option<T>, StorageError> {
        let json = self
            .connection
            .query_row(sql, [parameter], |row| row.get::<_, String>(0))
            .optional()?;
        json.map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .transpose()
    }

    pub(crate) fn read_json_rows<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        parameter: &str,
    ) -> Result<Vec<T>, StorageError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement
            .query_map([parameter], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }

    pub(crate) fn read_all_json<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
    ) -> Result<Vec<T>, StorageError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|value| serde_json::from_str(&value).map_err(StorageError::from))
            .collect()
    }
}

fn insert_agent_usage(
    transaction: &Transaction<'_>,
    usage: &AgentUsageRecord,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO agent_usage(usage_id, run_id, task_id, usage_json, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            usage.id.to_string(),
            usage.run_id.to_string(),
            usage.task_id.to_string(),
            serde_json::to_string(usage)?,
            usage.occurred_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn agent_turn_usage_records(
    run: &AgentRun,
    instance_id: AgentInstanceId,
    model_calls: &[ModelCallEvidence],
    tool_outcomes: &[ToolOutcome],
    settled_at: DateTime<Utc>,
) -> Vec<AgentUsageRecord> {
    let mut usage = Vec::with_capacity(model_calls.len().saturating_add(tool_outcomes.len()));
    usage.extend(model_calls.iter().map(|call| AgentUsageRecord {
        schema_version: AGENT_RUN_SCHEMA_VERSION,
        id: mcg_domain::UsageRecordId::new(),
        run_id: run.id,
        task_id: run.task_id,
        instance_id: Some(instance_id),
        kind: UsageKind::ProviderModel,
        provider_connection_id: Some(call.target.provider_id.clone()),
        model_id: Some(call.target.model_id.clone()),
        tool_execution_id: None,
        input_tokens: call.usage.input_tokens,
        cached_input_tokens: call.usage.cached_input_tokens,
        output_tokens: call.usage.output_tokens,
        cost_usd: call.cost_usd,
        duration_ms: call.duration_ms,
        occurred_at: settled_at,
    }));
    usage.extend(tool_outcomes.iter().map(|outcome| {
        let duration_ms = u64::try_from(
            (outcome.finished_at - outcome.started_at)
                .num_milliseconds()
                .max(0),
        )
        .unwrap_or(u64::MAX);
        AgentUsageRecord {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: mcg_domain::UsageRecordId::new(),
            run_id: run.id,
            task_id: run.task_id,
            instance_id: Some(instance_id),
            kind: UsageKind::Tool,
            provider_connection_id: None,
            model_id: None,
            tool_execution_id: Some(outcome.execution_id),
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            cost_usd: None,
            duration_ms,
            occurred_at: outcome.finished_at,
        }
    }));
    usage
}

fn apply_checkpointed_turn_accounting(
    checkpoint: Option<&AgentTurnCheckpoint>,
    run: &mut AgentRun,
    instance: &mut AgentInstance,
    lease: &mut AgentLease,
    settled_at: DateTime<Utc>,
) -> Result<Vec<AgentUsageRecord>, StorageError> {
    lease.budget.elapsed_seconds = lease.budget.elapsed_seconds.max(
        u64::try_from((settled_at - lease.acquired_at).num_seconds().max(0)).unwrap_or(u64::MAX),
    );
    let Some(checkpoint) = checkpoint else {
        return Ok(Vec::new());
    };
    if checkpoint.request.instance.id != instance.id
        || checkpoint.request.instance.run_id != run.id
        || checkpoint.request.instance.task_id != run.task_id
        || checkpoint.request.lease.id != lease.id
        || checkpoint.request.lease.instance_id != instance.id
        || checkpoint.model_calls.is_empty()
        || checkpoint.response.model != checkpoint.pinned_route.target.model_id
        || checkpoint
            .model_calls
            .iter()
            .any(|call| call.target != checkpoint.pinned_route.target)
    {
        return Err(StorageError::InvalidData(
            "terminal agent turn checkpoint authority or evidence is inconsistent".into(),
        ));
    }
    let usage = agent_turn_usage_records(
        run,
        instance.id,
        &checkpoint.model_calls,
        &checkpoint.tool_outcomes,
        settled_at,
    );
    if usage.iter().any(|record| {
        record.cached_input_tokens > record.input_tokens
            || record
                .cost_usd
                .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    }) {
        return Err(StorageError::InvalidData(
            "terminal agent turn contains invalid usage values".into(),
        ));
    }
    let turn_tokens = checkpoint
        .model_calls
        .iter()
        .map(|call| call.usage.total_tokens)
        .sum::<u64>();
    let turn_cost = checkpoint
        .model_calls
        .iter()
        .filter_map(|call| call.cost_usd)
        .sum::<f64>();
    lease.budget.consumed_tokens = lease.budget.consumed_tokens.saturating_add(turn_tokens);
    lease.budget.consumed_cost_usd += turn_cost;
    instance.model = Some(AssignedModel {
        provider_connection_id: checkpoint.pinned_route.target.provider_id.clone(),
        model_id: checkpoint.pinned_route.target.model_id.clone(),
        route_reason: "pinned by the bounded route plan".into(),
    });
    run.metrics.model_calls = run
        .metrics
        .model_calls
        .saturating_add(u32::try_from(checkpoint.model_calls.len()).unwrap_or(u32::MAX));
    run.metrics.tool_calls = run
        .metrics
        .tool_calls
        .saturating_add(u32::try_from(checkpoint.tool_outcomes.len()).unwrap_or(u32::MAX));
    run.metrics.tokens = run.metrics.tokens.saturating_add(turn_tokens);
    run.metrics.cost_usd += turn_cost;
    if !metrics_valid(&run.metrics) || !lease.budget.consumed_cost_usd.is_finite() {
        return Err(StorageError::InvalidData(
            "terminal agent turn exceeds numeric storage bounds".into(),
        ));
    }
    Ok(usage)
}

fn upsert_consumer_evaluation_checkpoint(
    transaction: &Transaction<'_>,
    checkpoint: &ConsumerEvaluationCheckpoint,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO consumer_evaluation_checkpoints(
            task_id, run_id, state, checkpoint_json, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(task_id) DO UPDATE SET
            state = excluded.state,
            checkpoint_json = excluded.checkpoint_json,
            updated_at = excluded.updated_at
         WHERE consumer_evaluation_checkpoints.run_id = excluded.run_id",
        params![
            checkpoint.task_id.to_string(),
            checkpoint.run_id.to_string(),
            consumer_evaluation_checkpoint_state_name(checkpoint.state),
            serde_json::to_string(checkpoint)?,
            checkpoint.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn insert_or_validate_evaluation(
    transaction: &Transaction<'_>,
    report: &EvaluationReport,
) -> Result<bool, StorageError> {
    if let Some(existing_json) = transaction
        .query_row(
            "SELECT evaluation_json FROM evaluations WHERE evaluation_id = ?1",
            [report.id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        let existing: EvaluationReport = serde_json::from_str(&existing_json)?;
        if existing != *report {
            return Err(StorageError::Conflict(
                "evaluation identity was replayed with different content".into(),
            ));
        }
        return Ok(false);
    }
    transaction.execute(
        "INSERT INTO evaluations(
            evaluation_id, run_id, task_id, evaluation_json, evaluated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            report.id.to_string(),
            report.run_id.to_string(),
            report.task_id.to_string(),
            serde_json::to_string(report)?,
            report.evaluated_at.to_rfc3339(),
        ],
    )?;
    Ok(true)
}

fn validate_consumer_evaluation_completions(
    report: &EvaluationReport,
    targets: &[AgentTurnVerificationTarget],
    completions: &[ConsumerEvaluationCompletion],
) -> Result<(), StorageError> {
    if targets.len() != completions.len() {
        return Err(StorageError::Conflict(
            "consumer evaluation does not contain one certificate for every attached completed turn"
                .into(),
        ));
    }
    let report_source = evaluation_report_source_ref(report)?;
    let expected_verdict = match report.outcome {
        mcg_agents::EvaluationOutcome::Passed => CompletionVerdict::Satisfied,
        mcg_agents::EvaluationOutcome::Failed => CompletionVerdict::NotSatisfied,
        mcg_agents::EvaluationOutcome::Inconclusive => CompletionVerdict::Inconclusive,
    };
    let mut seen = std::collections::BTreeSet::new();
    for completion in completions {
        if !seen.insert(completion.lease_id) {
            return Err(StorageError::Conflict(
                "consumer evaluation contains a duplicate compatibility certificate".into(),
            ));
        }
        let target = targets
            .iter()
            .find(|target| target.lease_id == completion.lease_id)
            .ok_or_else(|| {
                StorageError::Conflict(
                    "consumer evaluation certificate does not match an attached completed turn"
                        .into(),
                )
            })?;
        let certificate = &completion.certificate;
        if completion.expected_revision != target.attempt.revision
            || completion.expected_generation != target.attempt.lease_generation
            || certificate.issued_at < report.evaluated_at
            || certificate.workflow_id != target.attempt.workflow_id
            || certificate.attempt_id != target.attempt.attempt_id
            || certificate.task_contract != target.attempt.task_contract
            || certificate.work_invocations != target.work_invocations
            || certificate.verdict != expected_verdict
            || !certificate.evidence_refs.contains(&report_source)
        {
            return Err(StorageError::Conflict(
                "consumer evaluation certificate substituted target, lineage, verdict, or report evidence"
                    .into(),
            ));
        }
        let expected_criteria = target
            .task_contract
            .acceptance_criteria
            .iter()
            .enumerate()
            .map(|(index, clause)| {
                let passed = target
                    .success_conditions
                    .get(index)
                    .and_then(|condition| {
                        report
                            .conditions
                            .iter()
                            .find(|result| result.condition == *condition)
                    })
                    .is_some_and(|result| result.passed);
                (clause.clause_id.as_str(), passed)
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        if certificate.criterion_results.len() != expected_criteria.len()
            || certificate.criterion_results.iter().any(|criterion| {
                expected_criteria.get(criterion.clause_id.as_str()) != Some(&criterion.passed)
                    || !criterion.evidence_refs.contains(&report_source)
            })
        {
            return Err(StorageError::Conflict(
                "consumer evaluation certificate criteria do not match objective report evidence"
                    .into(),
            ));
        }
    }
    Ok(())
}

const fn consumer_evaluation_checkpoint_state_name(
    state: ConsumerEvaluationCheckpointState,
) -> &'static str {
    match state {
        ConsumerEvaluationCheckpointState::Running => "running",
        ConsumerEvaluationCheckpointState::WaitingForApproval => "waiting_for_approval",
        ConsumerEvaluationCheckpointState::Verifying => "verifying",
        ConsumerEvaluationCheckpointState::Blocked => "blocked",
        ConsumerEvaluationCheckpointState::Completed => "completed",
    }
}

fn budgets_within_parent(plan: &WorkPlan, parent: &mcg_domain::TaskBudget) -> bool {
    if !budget_values_valid(parent)
        || plan
            .items
            .iter()
            .any(|item| !budget_values_valid(&item.budget))
    {
        return false;
    }
    let tokens = parent.max_tokens.is_none_or(|limit| {
        plan.items
            .iter()
            .try_fold(0_u64, |sum, item| sum.checked_add(item.budget.max_tokens?))
            .is_some_and(|sum| sum <= limit)
    });
    let duration = parent.max_duration_seconds.is_none_or(|limit| {
        plan.items
            .iter()
            .try_fold(0_u64, |sum, item| {
                sum.checked_add(item.budget.max_duration_seconds?)
            })
            .is_some_and(|sum| sum <= limit)
    });
    let cost = parent.max_cost_usd.is_none_or(|limit| {
        let values = plan
            .items
            .iter()
            .map(|item| item.budget.max_cost_usd)
            .collect::<Option<Vec<_>>>();
        values.is_some_and(|values| values.into_iter().sum::<f64>() <= limit + f64::EPSILON)
    });
    tokens && duration && cost
}

fn budget_values_valid(budget: &mcg_domain::TaskBudget) -> bool {
    budget
        .max_cost_usd
        .is_none_or(|cost| cost.is_finite() && cost >= 0.0)
}

fn metrics_valid(metrics: &OrganizationalMetrics) -> bool {
    metrics.cost_usd.is_finite() && metrics.cost_usd >= 0.0
}

fn ensure_single_change(changed: usize, detail: &str) -> Result<(), StorageError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::Conflict(detail.into()))
    }
}

const fn run_state_name(state: AgentRunState) -> &'static str {
    match state {
        AgentRunState::Planned => "planned",
        AgentRunState::Running => "running",
        AgentRunState::Waiting => "waiting",
        AgentRunState::Recovering => "recovering",
        AgentRunState::Completed => "completed",
        AgentRunState::Failed => "failed",
        AgentRunState::Cancelled => "cancelled",
    }
}

const fn instance_state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::Created => "created",
        AgentState::Running => "running",
        AgentState::Waiting => "waiting",
        AgentState::Blocked => "blocked",
        AgentState::Completed => "completed",
        AgentState::Failed => "failed",
        AgentState::Cancelled => "cancelled",
        AgentState::Expired => "expired",
    }
}

const fn lease_state_name(state: AgentLeaseState) -> &'static str {
    match state {
        AgentLeaseState::Active => "active",
        AgentLeaseState::Completed => "completed",
        AgentLeaseState::Failed => "failed",
        AgentLeaseState::Expired => "expired",
        AgentLeaseState::Reassigned => "reassigned",
        AgentLeaseState::Terminated => "terminated",
    }
}

const fn instance_transition_allowed(from: AgentState, to: AgentState) -> bool {
    matches!(
        (from, to),
        (
            AgentState::Created,
            AgentState::Running | AgentState::Cancelled
        ) | (
            AgentState::Running,
            AgentState::Waiting
                | AgentState::Blocked
                | AgentState::Completed
                | AgentState::Failed
                | AgentState::Cancelled
                | AgentState::Expired
        ) | (
            AgentState::Waiting | AgentState::Blocked,
            AgentState::Running | AgentState::Failed | AgentState::Cancelled | AgentState::Expired
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentLease, AgentRun, AgentUsageRecord, ConditionResult,
        EvaluationEvidence, EvaluationOutcome, ExecutivePlanner, InterventionLevel, LeaseBudget,
        PlannedWork, PlanningRequest, RoleKind, SUPERVISION_SCHEMA_VERSION, SuccessCondition,
        SupervisionEvidence, SupervisionLoop, SupervisionObservation, SupervisionSignal,
        TaskComplexity, UsageKind,
    };
    use mcg_contracts::{
        ArtifactRef, CompletionCertificateId, ContentHash, ContextManifest, ContextManifestId,
        ContextSelection, CorrelationId, CriterionResult, HashAlgorithm, InvocationId,
        InvocationManifest, InvocationRole, ModelRun, ModelRunId, ModelRunState, RuntimeActorId,
        RuntimeIdentity, RuntimeInstanceId, WorkflowState,
    };
    use mcg_domain::{
        AgentInstanceId, AgentLeaseId, EvaluationId, RunId, SupervisionObservationId, TaskBudget,
        UsageRecordId,
    };
    use sha2::Digest as _;
    use tempfile::tempdir;

    #[test]
    fn existing_task_launch_is_atomic_idempotent_and_restart_durable() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let task = TaskEnvelope::new("Build the selected project");
        let genome = OrganizationGenome::consumer_default(now);
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Build the selected project",
                RoleKind::Coder,
                vec![SuccessCondition::Custom {
                    description: "project meets the requested goal".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let mut store = SqliteEventStore::open(&database).expect("open store");
        store.create_task(&task).expect("create task");
        let created = store
            .launch_task_agent_run(task.id, &genome, &run, &plan)
            .expect("launch task");
        assert!(!created.replayed);
        assert_eq!(created.task.state, TaskState::Active);
        assert_eq!(created.run.state, AgentRunState::Running);
        assert_eq!(created.events.len(), 4);

        let replayed = store
            .launch_task_agent_run(task.id, &genome, &run, &plan)
            .expect("replay task launch");
        assert!(replayed.replayed);
        assert_eq!(replayed.run.id, run.id);
        assert!(replayed.events.is_empty());
        let (cancelled, events) = store
            .cancel_launched_task(task.id)
            .expect("cancel launch")
            .expect("launch cancellation");
        assert_eq!(cancelled.state, TaskState::Cancelled);
        assert_eq!(events.len(), 2);
        drop(store);

        let reopened = SqliteEventStore::open(&database).expect("reopen store");
        assert_eq!(
            reopened
                .load_task(task.id)
                .expect("task")
                .expect("task row")
                .state,
            TaskState::Cancelled
        );
        assert_eq!(
            reopened
                .agent_run(run.id)
                .expect("run")
                .expect("run row")
                .state,
            AgentRunState::Cancelled
        );
    }

    #[test]
    #[allow(clippy::similar_names, clippy::too_many_lines)] // Full compatibility authority/evidence chain proves atomic settlement.
    fn compatibility_certificate_and_legacy_terminal_projection_settle_atomically() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let now = Utc::now();
        let task = TaskEnvelope::new("complete one governed compatibility turn");
        store.create_task(&task).expect("task");
        let genome = OrganizationGenome::consumer_default(now);
        let condition = SuccessCondition::Custom {
            description: "return independently verified output".into(),
        };
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "complete governed Work",
                RoleKind::Coder,
                vec![condition.clone()],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        store
            .launch_task_agent_run(task.id, &genome, &run, &plan)
            .expect("launch");
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:atomic-compatibility".into(),
            confidence: ConfidenceEstimate::new(8_000, 0).expect("confidence"),
            created_at: now,
            updated_at: now,
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: Vec::new(),
            reporting_interval_seconds: 30,
            max_retries: 0,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now,
            expires_at: now + Duration::minutes(5),
            heartbeat_at: now,
        };
        store
            .dispatch_agent_work(&instance, &lease, now)
            .expect("dispatch");
        let authority = store
            .claim_active_agent_lease_workflow(lease.id, RuntimeActorId::new(), 120, now)
            .expect("claim canonical workflow");
        store
            .finish_agent_lease(
                lease.id,
                AgentLeaseState::Completed,
                0,
                0.0,
                1,
                now + Duration::seconds(1),
            )
            .expect("complete legacy lease");
        let verifying = store
            .transition_workflow_attempt(
                authority.workflow.attempt.attempt_id,
                authority.workflow.attempt.revision,
                authority.workflow.attempt.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "compatibility-work-completed",
                now + Duration::seconds(1),
            )
            .expect("begin Verify");
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: EvaluationId::new(),
            run_id: run.id,
            task_id: task.id,
            evaluator_instance_id: None,
            conditions: vec![ConditionResult {
                condition: condition.clone(),
                passed: true,
                detail: "objective evidence passed".into(),
            }],
            outcome: EvaluationOutcome::Passed,
            confidence: ConfidenceEstimate::new(9_500, 1).expect("confidence"),
            regressions: Vec::new(),
            evidence: vec![EvaluationEvidence {
                method: "fixture".into(),
                artifact_sha256: Some("a".repeat(64)),
                event_sequence: None,
                detail: "immutable objective evidence".into(),
            }],
            evaluated_at: now + Duration::seconds(2),
        };
        let report_source = evaluation_report_source_ref(&report).expect("report source");
        let checkpoint = ConsumerEvaluationCheckpoint {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            task_id: task.id,
            run_id: run.id,
            project_snapshot_id: ProjectSnapshotId::new(),
            state: ConsumerEvaluationCheckpointState::Verifying,
            next_condition_index: 1,
            attempt: 0,
            request_id: None,
            prepared_evaluation_id: Some(report.id),
            prepared_evaluated_at: Some(report.evaluated_at),
            batch_approved: false,
            results: report.conditions.clone(),
            evidence: report.evidence.clone(),
            regressions: Vec::new(),
            objective_failures: 0,
            unsupported: 0,
            executed: 1,
            updated_at: report.evaluated_at,
        };
        let mut running_checkpoint = checkpoint.clone();
        running_checkpoint.state = ConsumerEvaluationCheckpointState::Running;
        running_checkpoint.prepared_evaluation_id = None;
        running_checkpoint.prepared_evaluated_at = None;
        store
            .save_consumer_evaluation_checkpoint(&running_checkpoint)
            .expect("running evaluation checkpoint");
        store
            .prepare_consumer_evaluation(&report, &checkpoint)
            .expect("prepare evaluation");

        let content_hash = |label: &str| ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: "test.compatibility.settlement".into(),
            digest_hex: format!("{:x}", sha2::Sha256::digest(label.as_bytes())),
        };
        let runtime_build = content_hash("runtime-build");
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: runtime_build.clone(),
            protocol_version: mcg_contracts::PROTOCOL_V1,
            started_at: now,
        };
        let persist_completed = |store: &mut SqliteEventStore,
                                 role: InvocationRole,
                                 label: &str|
         -> (InvocationId, ArtifactRef) {
            let source = SourceRef {
                source_type: StableTypeId::parse("test.compatibility_evidence.v1").unwrap(),
                locator: label.into(),
                content_hash: content_hash(label),
            };
            let context = ContextManifest {
                id: ContextManifestId::new(),
                attempt_id: verifying.attempt_id,
                selected_sources: vec![ContextSelection {
                    source: source.clone(),
                    perspective: None,
                    influence: VersionedPayload::canonical_v1(
                        StableTypeId::parse("test.compatibility_influence.v1").unwrap(),
                        &serde_json::json!({"label": label}),
                    )
                    .unwrap(),
                    transformations: Vec::new(),
                    placement: 0,
                    approximate_tokens: 1,
                }],
                selection_policy: VersionedPayload::canonical_v1(
                    StableTypeId::parse("test.compatibility_policy.v1").unwrap(),
                    &serde_json::json!({"mode": "exact"}),
                )
                .unwrap(),
                expansion_steps: Vec::new(),
                approximate_tokens: 1,
            };
            let context_hash = store.persist_context_manifest(&context, now).unwrap();
            let invocation = InvocationManifest {
                id: InvocationId::new(),
                role,
                attempt_id: verifying.attempt_id,
                task_contract: verifying.task_contract.clone(),
                runtime_identity: runtime_identity.clone(),
                cognitive_configuration: content_hash("configuration"),
                role_manifest: content_hash(label),
                prompt_bundle: content_hash("prompt"),
                context_manifest: context_hash,
                model_route: VersionedPayload::canonical_v1(
                    StableTypeId::parse("test.compatibility_route.v1").unwrap(),
                    &serde_json::json!({"provider": "fixture"}),
                )
                .unwrap(),
                resolved_provider: source.clone(),
                resolved_model: source.clone(),
                observed_model_fingerprint: Some("fixture-v1".into()),
                sampling: VersionedPayload::canonical_v1(
                    StableTypeId::parse("test.compatibility_sampling.v1").unwrap(),
                    &serde_json::json!({"temperature": 0}),
                )
                .unwrap(),
                reasoning: None,
                available_tool_manifests: Vec::new(),
                selected_skill_manifests: Vec::new(),
                authority_envelope: source.clone(),
                egress_permit: None,
                repository_revision: None,
                environment: source,
                runtime_build: runtime_build.clone(),
                request_hash: content_hash("request"),
                response_hash: None,
            };
            store.persist_invocation_manifest(&invocation, now).unwrap();
            let planned = ModelRun {
                id: ModelRunId::new(),
                invocation_id: invocation.id,
                role,
                state: ModelRunState::Planned,
                provider_request_id: None,
                response_artifact: None,
                usage: None,
                observed_model_fingerprint: None,
                outcome_evidence: None,
                started_at: now,
                finished_at: None,
            };
            store.create_model_run(&planned, now).unwrap();
            let mut dispatched = planned.clone();
            dispatched.state = ModelRunState::Dispatched;
            dispatched.provider_request_id = Some(format!("{label}-request"));
            store.transition_model_run(&dispatched, 1, now).unwrap();
            let artifact = ArtifactRef {
                content_hash: content_hash(&format!("{label}-response")),
                media_type: "application/json".into(),
                byte_length: 32,
            };
            let mut completed = dispatched;
            completed.state = ModelRunState::Completed;
            completed.response_artifact = Some(artifact.clone());
            completed.finished_at = Some(report.evaluated_at);
            store
                .transition_model_run(&completed, 2, report.evaluated_at)
                .unwrap();
            (invocation.id, artifact)
        };
        let (work_invocation, work_artifact) =
            persist_completed(&mut store, InvocationRole::Work, "work");
        let (verify_invocation, verify_artifact) =
            persist_completed(&mut store, InvocationRole::Verify, "verify");
        let certificate = CompletionCertificate {
            id: CompletionCertificateId::new(),
            workflow_id: verifying.workflow_id,
            attempt_id: verifying.attempt_id,
            task_contract: verifying.task_contract.clone(),
            work_invocations: vec![work_invocation],
            verify_invocations: vec![verify_invocation],
            criterion_results: vec![CriterionResult {
                clause_id: "legacy-work-condition-1".into(),
                passed: true,
                evidence_refs: vec![report_source.clone()],
                detail_hash: content_hash("criterion-detail"),
            }],
            evidence_refs: vec![report_source.clone()],
            tests_run: vec![report_source],
            artifacts: vec![work_artifact, verify_artifact],
            known_untested_paths: Vec::new(),
            unresolved_assumptions: Vec::new(),
            remaining_risks: Vec::new(),
            verdict: CompletionVerdict::Satisfied,
            issued_at: report.evaluated_at + Duration::seconds(1),
        };
        let completion = ConsumerEvaluationCompletion {
            lease_id: lease.id,
            certificate: certificate.clone(),
            expected_revision: verifying.revision,
            expected_generation: verifying.lease_generation,
        };

        store
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_compatibility_task_settlement
                 BEFORE UPDATE ON tasks
                 BEGIN SELECT RAISE(ABORT, 'injected terminal projection failure'); END;",
            )
            .expect("failure trigger");
        assert!(
            store
                .settle_consumer_evaluation_with_completions(
                    &report,
                    &checkpoint,
                    std::slice::from_ref(&completion),
                )
                .is_err()
        );
        assert_eq!(
            store
                .workflow_attempt(verifying.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Verifying
        );
        assert_eq!(store.completion_certificate(certificate.id).unwrap(), None);
        assert_eq!(
            store.load_task(task.id).unwrap().unwrap().state,
            TaskState::Active
        );
        store
            .connection
            .execute_batch("DROP TRIGGER fail_compatibility_task_settlement;")
            .expect("drop trigger");

        store
            .settle_consumer_evaluation_with_completions(
                &report,
                &checkpoint,
                std::slice::from_ref(&completion),
            )
            .expect("atomic compatibility settlement");
        assert_eq!(
            store
                .workflow_attempt(verifying.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Succeeded
        );
        assert_eq!(
            store.completion_certificate(certificate.id).unwrap(),
            Some(certificate)
        );
        assert_eq!(
            store.load_task(task.id).unwrap().unwrap().state,
            TaskState::Completed
        );
        assert_eq!(
            store.agent_run(run.id).unwrap().unwrap().state,
            AgentRunState::Completed
        );
        assert_eq!(
            store
                .consumer_evaluation_checkpoint(task.id)
                .unwrap()
                .unwrap()
                .state,
            ConsumerEvaluationCheckpointState::Completed
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture proves atomic creation, exact replay/conflict, and both request/launch identities across reopen.
    fn interface_consumer_run_creation_is_atomic_idempotent_and_restart_durable() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let mut task = TaskEnvelope::new("Explain the fixture project");
        task.budget = TaskBudget {
            max_tokens: Some(4_000),
            max_cost_usd: Some(1.0),
            max_duration_seconds: Some(300),
        };
        let genome = OrganizationGenome::consumer_default(now);
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Explain the fixture project",
                RoleKind::Conversation,
                vec![SuccessCondition::Custom {
                    description: "return a grounded explanation".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let hash = "a".repeat(64);
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let created = store
            .create_interface_agent_run(
                "mcp",
                "fixture-request",
                &hash,
                true,
                &task,
                &genome,
                &run,
                &plan,
            )
            .expect("create interface run");
        assert!(!created.replayed);
        assert_eq!(created.task.state, TaskState::Active);
        assert_eq!(created.run.state, AgentRunState::Running);
        assert_eq!(created.events.len(), 5);
        assert_eq!(
            store.task_launch_run_id(task.id).expect("consumer launch"),
            Some(run.id)
        );
        assert_eq!(
            store
                .interface_run_request("mcp", "fixture-request")
                .expect("interface request"),
            Some((task.id, run.id))
        );
        let event_count = store.event_count().expect("event count");
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        let replayed = reopened
            .create_interface_agent_run(
                "mcp",
                "fixture-request",
                &hash,
                true,
                &task,
                &genome,
                &run,
                &plan,
            )
            .expect("replay interface run");
        assert!(replayed.replayed);
        assert_eq!(replayed.task.id, task.id);
        assert_eq!(replayed.run.id, run.id);
        assert!(replayed.events.is_empty());
        assert_eq!(
            reopened
                .task_launch_run_id(task.id)
                .expect("reopened consumer launch"),
            Some(run.id)
        );
        assert_eq!(
            reopened
                .interface_run_request("mcp", "fixture-request")
                .expect("reopened interface request"),
            Some((task.id, run.id))
        );
        assert_eq!(reopened.event_count().expect("event count"), event_count);
        assert!(
            reopened
                .create_interface_agent_run(
                    "mcp",
                    "fixture-request",
                    &"b".repeat(64),
                    true,
                    &task,
                    &genome,
                    &run,
                    &plan,
                )
                .is_err()
        );
        assert_eq!(reopened.event_count().expect("event count"), event_count);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Complete inactive-candidate authority chain is intentional.
    fn failed_inactive_candidate_turn_cannot_change_the_real_task() {
        let now = Utc::now();
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let task = TaskEnvelope::new("real task remains authoritative");
        store.create_task(&task).expect("task");
        store
            .transition_task(task.id, TaskState::Active)
            .expect("active task");
        let baseline = OrganizationGenome::consumer_default(now);
        store
            .record_organization_revision(&baseline)
            .expect("baseline");
        let mut candidate = baseline.clone();
        candidate.revision_id = OrganizationRevisionId::new();
        candidate.revision = 2;
        candidate.parent_revision_id = Some(baseline.revision_id);
        candidate.name = "evaluation-only candidate".into();
        candidate.created_at = now + Duration::seconds(1);
        store
            .record_candidate_organization_revision(&candidate)
            .expect("candidate");
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "evaluate without production authority",
                RoleKind::Coder,
                vec![SuccessCondition::Custom {
                    description: "produce evaluation evidence".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&candidate, &planning, now + Duration::seconds(2))
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: candidate.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now + Duration::seconds(2),
            updated_at: now + Duration::seconds(2),
            completed_at: None,
        };
        store.create_agent_run(&run, &plan).expect("run");
        store
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(3),
            )
            .expect("running");
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:evaluation-only".into(),
            confidence: ConfidenceEstimate::new(5_000, 0).expect("confidence"),
            created_at: now + Duration::seconds(3),
            updated_at: now + Duration::seconds(3),
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: Vec::new(),
            reporting_interval_seconds: 30,
            max_retries: 0,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now + Duration::seconds(3),
            expires_at: now + Duration::minutes(5),
            heartbeat_at: now + Duration::seconds(3),
        };
        let (running, _) = store
            .dispatch_agent_work(&instance, &lease, now + Duration::seconds(3))
            .expect("dispatch");
        store
            .settle_agent_turn_error(
                running.id,
                lease.id,
                AgentTurnDisposition::Failed,
                "shadow_model_failure",
                now + Duration::seconds(4),
            )
            .expect("settle shadow failure");
        assert_eq!(
            store
                .load_task(task.id)
                .expect("task")
                .expect("present")
                .state,
            TaskState::Active
        );
        assert_eq!(
            store
                .agent_instance(instance.id)
                .expect("instance")
                .expect("present")
                .state,
            AgentState::Failed
        );
    }

    #[test]
    fn organization_inheritance_is_pinned_and_survives_reopen() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let base = OrganizationGenome::consumer_default(now);
        store
            .record_organization_revision(&base)
            .expect("record base");
        let inherited = OrganizationGenome::inherit_from(&base, "Inherited fixture", now);
        store
            .record_organization_revision(&inherited)
            .expect("record inherited organization");

        let mut advanced = inherited.clone();
        advanced.revision_id = OrganizationRevisionId::new();
        advanced.revision = 2;
        advanced.parent_revision_id = Some(inherited.revision_id);
        advanced.name = "Inherited fixture revision 2".into();
        store
            .record_organization_revision(&advanced)
            .expect("advance inherited organization");

        let mut changed_base = advanced.clone();
        changed_base.revision_id = OrganizationRevisionId::new();
        changed_base.revision = 3;
        changed_base.parent_revision_id = Some(advanced.revision_id);
        changed_base.inherited_from_revision_id = None;
        assert!(matches!(
            store.record_organization_revision(&changed_base),
            Err(StorageError::Conflict(_))
        ));

        let mut missing_base = OrganizationGenome::consumer_default(now);
        missing_base.inherited_from_revision_id = Some(OrganizationRevisionId::new());
        assert!(matches!(
            store.record_organization_revision(&missing_base),
            Err(StorageError::OrganizationRevisionNotFound(_))
        ));
        drop(store);

        let reopened = SqliteEventStore::open(&database).expect("reopen store");
        let recovered = reopened
            .organization(advanced.organization_id)
            .expect("load inherited organization")
            .expect("inherited organization present");
        assert_eq!(recovered, advanced);
        assert_eq!(recovered.inherited_from_revision_id, Some(base.revision_id));
        assert_eq!(
            reopened
                .organization_revision(base.revision_id)
                .expect("load base revision"),
            Some(base)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One contiguous crash/reopen scenario is intentional evidence.
    fn organization_run_evidence_and_crash_recovery_survive_reopen() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("mcg.sqlite3");
        let now = Utc::now();
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let mut task = mcg_domain::TaskEnvelope::new("Build and evaluate the fixture site");
        task.budget = TaskBudget {
            max_tokens: Some(12_000),
            max_cost_usd: Some(5.0),
            max_duration_seconds: Some(900),
        };
        store.create_task(&task).expect("create task");
        store
            .transition_task(task.id, TaskState::Active)
            .expect("activate task");

        let genome = OrganizationGenome::consumer_default(now);
        store
            .record_organization_revision(&genome)
            .expect("record organization");
        let planned = PlannedWork::atomic(
            "Implement the fixture site",
            RoleKind::Coder,
            vec![SuccessCondition::Test {
                command: "npm test".into(),
            }],
        );
        let request = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![planned],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &request, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: request.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        store.create_agent_run(&run, &plan).expect("create run");
        store
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(1),
            )
            .expect("start run");

        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:fixture-context".into(),
            confidence: ConfidenceEstimate::new(7_500, 0).expect("confidence"),
            created_at: now,
            updated_at: now,
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: vec!["filesystem.project".into()],
            reporting_interval_seconds: 30,
            max_retries: 2,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now + Duration::seconds(2),
            expires_at: now + Duration::minutes(15),
            heartbeat_at: now + Duration::seconds(2),
        };
        let (running_instance, dispatch_events) = store
            .dispatch_agent_work(&instance, &lease, now + Duration::seconds(2))
            .expect("atomic dispatch");
        assert_eq!(running_instance.state, AgentState::Running);
        assert_eq!(dispatch_events.len(), 3);
        let event_count_after_dispatch = store.event_count().expect("event count");
        assert!(
            store
                .dispatch_agent_work(&instance, &lease, now + Duration::seconds(2))
                .is_err()
        );
        assert_eq!(
            store.event_count().expect("event count"),
            event_count_after_dispatch
        );

        let usage = AgentUsageRecord {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: UsageRecordId::new(),
            run_id: run.id,
            task_id: task.id,
            instance_id: Some(instance.id),
            kind: UsageKind::ProviderModel,
            provider_connection_id: Some("fixture-provider".into()),
            model_id: Some("fixture-model".into()),
            tool_execution_id: None,
            input_tokens: 100,
            cached_input_tokens: 20,
            output_tokens: 40,
            cost_usd: Some(0.01),
            duration_ms: 250,
            occurred_at: now + Duration::seconds(3),
        };
        store.record_agent_usage(&usage).expect("record usage");
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: EvaluationId::new(),
            run_id: run.id,
            task_id: task.id,
            evaluator_instance_id: None,
            conditions: vec![ConditionResult {
                condition: SuccessCondition::Test {
                    command: "npm test".into(),
                },
                passed: true,
                detail: "fixture suite passed".into(),
            }],
            outcome: EvaluationOutcome::Passed,
            confidence: ConfidenceEstimate::new(9_000, 1).expect("confidence"),
            regressions: Vec::new(),
            evidence: vec![EvaluationEvidence {
                method: "process_exit".into(),
                artifact_sha256: Some("fixture-sha256".into()),
                event_sequence: None,
                detail: "exit code 0".into(),
            }],
            evaluated_at: now + Duration::seconds(4),
        };
        store.record_evaluation(&report).expect("record evaluation");
        let automatic_observation = SupervisionObservation {
            schema_version: SUPERVISION_SCHEMA_VERSION,
            id: SupervisionObservationId::new(),
            loop_kind: SupervisionLoop::Fast,
            signal: SupervisionSignal::RepeatedFailure,
            severity_basis_points: 8_500,
            recommended_intervention: InterventionLevel::Redirect,
            task_id: Some(task.id),
            run_id: Some(run.id),
            organization_revision_id: Some(genome.revision_id),
            summary: "objective evaluation failed".into(),
            evidence: vec![SupervisionEvidence {
                method: "objective_evaluation".into(),
                artifact_sha256: Some("a".repeat(64)),
                event_sequence: None,
                detail: "durable automatic fixture".into(),
            }],
            observed_at: report.evaluated_at,
        };
        let analysis_hash = "b".repeat(64);
        let recorded = store
            .record_automatic_supervision(
                report.id,
                &analysis_hash,
                std::slice::from_ref(&automatic_observation),
                now + Duration::seconds(5),
            )
            .expect("record automatic supervision");
        assert!(!recorded.replayed);
        assert_eq!(recorded.events.len(), 1);
        assert!(
            store
                .record_automatic_supervision(
                    report.id,
                    &analysis_hash,
                    std::slice::from_ref(&automatic_observation),
                    now + Duration::seconds(6),
                )
                .expect("exact supervision replay")
                .replayed
        );
        let mut changed_observation = automatic_observation.clone();
        changed_observation.summary = "changed under same analysis".into();
        assert!(
            store
                .record_automatic_supervision(
                    report.id,
                    &analysis_hash,
                    &[changed_observation],
                    now + Duration::seconds(6),
                )
                .is_err()
        );
        assert!(
            store
                .record_automatic_supervision(
                    report.id,
                    &"c".repeat(64),
                    std::slice::from_ref(&automatic_observation),
                    now + Duration::seconds(6),
                )
                .is_err()
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM supervised_evaluations", [])
                .is_err()
        );
        drop(store);

        let mut reopened = SqliteEventStore::open(&database).expect("reopen store");
        let projection_recovery = reopened
            .recover_projections()
            .expect("rebuild task projections with dependent agent rows");
        assert_eq!(projection_recovery.tasks_rebuilt, 1);
        assert_eq!(
            reopened
                .organization_history(genome.organization_id)
                .expect("history"),
            vec![genome.clone()]
        );
        assert_eq!(
            reopened.work_plan(run.id).expect("plan"),
            Some(plan.clone())
        );
        assert_eq!(
            reopened.agent_usage(run.id).expect("usage"),
            vec![usage.clone()]
        );
        assert_eq!(
            reopened.evaluations(run.id).expect("evaluations"),
            vec![report.clone()]
        );
        assert!(
            reopened
                .unsupervised_evaluation_ids(32)
                .expect("supervision markers")
                .is_empty()
        );
        assert_eq!(
            reopened
                .supervision_observation(automatic_observation.id)
                .expect("automatic observation"),
            Some(automatic_observation)
        );

        let recovery = reopened
            .recover_agent_work(now + Duration::seconds(30))
            .expect("recover work");
        assert_eq!(
            recovery,
            AgentRecoveryReport {
                runs_marked_recovering: 1,
                instances_expired: 1,
                leases_expired: 1,
            }
        );
        assert_eq!(
            reopened
                .agent_run(run.id)
                .expect("run")
                .expect("present")
                .state,
            AgentRunState::Recovering
        );
        assert_eq!(
            reopened
                .agent_instance(instance.id)
                .expect("instance")
                .expect("present")
                .state,
            AgentState::Expired
        );
        assert_eq!(
            reopened
                .agent_lease(lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Expired
        );
        let events = reopened.events_for_task(task.id).expect("task events");
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == EventKind::AgentUsageRecorded)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == EventKind::EvaluationRecorded)
        );
        assert!(events.iter().any(|event| {
            event.kind == EventKind::AgentRunStateChanged
                && event.payload.get("reason").and_then(|value| value.as_str())
                    == Some("daemon_restart")
        }));

        reopened
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(31),
            )
            .expect("resume recovering run");
        let retry_instance = AgentInstance {
            id: AgentInstanceId::new(),
            state: AgentState::Created,
            created_at: now + Duration::seconds(31),
            updated_at: now + Duration::seconds(31),
            ..instance.clone()
        };
        reopened
            .record_agent_instance(&retry_instance)
            .expect("retry instance");
        reopened
            .transition_agent_instance(
                retry_instance.id,
                AgentState::Running,
                retry_instance.confidence,
                now + Duration::seconds(31),
            )
            .expect("start retry instance");
        let retry_lease = AgentLease {
            id: AgentLeaseId::new(),
            instance_id: retry_instance.id,
            state: AgentLeaseState::Active,
            acquired_at: now + Duration::seconds(31),
            expires_at: now + Duration::minutes(15),
            heartbeat_at: now + Duration::seconds(31),
            ..lease.clone()
        };
        reopened
            .record_agent_lease(&retry_lease)
            .expect("retry lease");
        reopened
            .settle_agent_turn_error(
                retry_instance.id,
                retry_lease.id,
                AgentTurnDisposition::Waiting,
                "approval_required",
                now + Duration::seconds(32),
            )
            .expect("wait for approval");
        assert_eq!(
            reopened
                .agent_run(run.id)
                .expect("run")
                .expect("present")
                .state,
            AgentRunState::Waiting
        );
        assert_eq!(
            reopened
                .agent_lease(retry_lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Active
        );
        assert_eq!(
            reopened
                .load_task(task.id)
                .expect("task")
                .expect("present")
                .state,
            TaskState::WaitingForApproval
        );
        reopened
            .transition_task(task.id, TaskState::Active)
            .expect("resume approved task");
        reopened
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                now + Duration::seconds(33),
            )
            .expect("resume approved run");
        reopened
            .transition_agent_instance(
                retry_instance.id,
                AgentState::Running,
                retry_instance.confidence,
                now + Duration::seconds(33),
            )
            .expect("resume approved instance");
        reopened
            .settle_agent_turn_error(
                retry_instance.id,
                retry_lease.id,
                AgentTurnDisposition::Failed,
                "provider_failure",
                now + Duration::seconds(34),
            )
            .expect("settle failure");
        assert_eq!(
            reopened
                .agent_instance(retry_instance.id)
                .expect("instance")
                .expect("present")
                .state,
            AgentState::Failed
        );
        assert_eq!(
            reopened
                .agent_lease(retry_lease.id)
                .expect("lease")
                .expect("present")
                .state,
            AgentLeaseState::Failed
        );
        assert_eq!(reopened.agent_usage(run.id).expect("usage"), vec![usage]);
        assert_eq!(
            reopened.agent_turn_result(retry_lease.id).expect("result"),
            None
        );
        assert_eq!(
            reopened
                .load_task(task.id)
                .expect("task")
                .expect("present")
                .state,
            TaskState::Blocked
        );
    }

    #[test]
    fn organization_assignment_bootstrap_is_idempotent_and_persistent() {
        let directory = tempdir().expect("temp directory");
        let database = directory.path().join("assignments.sqlite3");
        let now = Utc::now();
        let mut store = SqliteEventStore::open(&database).expect("open store");
        let first = store
            .initialize_organization_assignments(now)
            .expect("bootstrap assignments");
        let second = store
            .initialize_organization_assignments(now + Duration::seconds(1))
            .expect("repeat bootstrap");
        assert_eq!(first.global_organization_id, second.global_organization_id);
        assert_eq!(store.organizations().expect("organizations").len(), 1);

        let alternative = OrganizationGenome::consumer_default(now + Duration::seconds(2));
        store
            .record_organization_revision(&alternative)
            .expect("record alternative");
        store
            .set_global_organization_assignment(
                alternative.organization_id,
                now + Duration::seconds(3),
            )
            .expect("change default");
        drop(store);

        let reopened = SqliteEventStore::open(&database).expect("reopen store");
        assert_eq!(
            reopened
                .organization_assignments()
                .expect("load assignments")
                .expect("assignments present")
                .global_organization_id,
            alternative.organization_id
        );
    }
}
