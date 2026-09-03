use chrono::{DateTime, Utc};
use mcg_agents::{
    AgentInstance, AgentLease, AgentLeaseState, AgentRun, AgentRunState, AgentState,
    SuccessCondition, WorkItem, WorkPlan,
};
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionState, ArtifactRef, AttemptId, ConfigurationId,
    ContentHash, ContractClause, CorrelationId, EffectClass, EffectOutcome, EventId, InvocationId,
    InvocationManifest, InvocationRole, MAX_WORKFLOW_EVENT_PAGE_SIZE, MAX_WORKFLOW_SOURCE_LINKS,
    ModelRun, ModelRunState, PolicyBinding, RetryPolicy, RuntimeActorId, SourceRef, StableTypeId,
    TaskContract, TaskContractId, TaskContractStatus, VersionedPayload, VersionedRef,
    WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN, WaitReason, WorkflowAttempt, WorkflowEvent, WorkflowId,
    WorkflowSourceAttachment, WorkflowSourceLink, WorkflowState, WorkflowWaitRequest,
    WorkflowWakeBinding, WorkflowWakeRequest, decode_canonical_cbor_v1,
};
use mcg_domain::{
    AgentLeaseId, ApprovalId, CodexThreadBinding, PluginInstallation, PluginInvocationId,
    PluginInvocationRecord, PluginInvocationState, PluginRevision, RunId, TaskEnvelope, TaskId,
    TaskState, ToolRequestId, WorkItemId,
};
use mcg_projects::ProjectModel;
use mcg_tools::{ApprovalRequest, ApprovalState};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    SqliteEventStore, StorageError,
    actions::{cancellation_effect_status, fence_actions_for_workflow_cancellation},
    invocations::propagate_workflow_cancellation_to_model_runs,
    plugins::{
        complete_plugin_invocation_in_transaction, load_plugin_installation,
        load_plugin_invocation, load_plugin_revision, validate_plugin_revision,
        validate_running_invocation,
    },
};

const TASK_CONTRACT_HASH_DOMAIN: &str = "MetaCog.TaskContract";
const WORKFLOW_CREATED_TYPE_ID: &str = "mcg.workflow_created.v1";
const WORKFLOW_CLAIMED_TYPE_ID: &str = "mcg.workflow_claimed.v1";
const WORKFLOW_HEARTBEAT_TYPE_ID: &str = "mcg.workflow_heartbeat.v1";
const WORKFLOW_LEASE_EXPIRED_TYPE_ID: &str = "mcg.workflow_lease_expired.v1";
const WORKFLOW_WAIT_BOUND_TYPE_ID: &str = "mcg.workflow_wait_bound.v1";
const WORKFLOW_WAIT_SATISFIED_TYPE_ID: &str = "mcg.workflow_wait_satisfied.v1";
const WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID: &str = "mcg.workflow_cancellation_recovered.v1";
const WORKFLOW_RETRIED_TYPE_ID: &str = "mcg.workflow_retried.v1";
const WORKFLOW_TRANSITIONED_TYPE_ID: &str = "mcg.workflow_transitioned.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowRecoveryReport {
    pub inspected_nonterminal: u64,
    pub fenced_expired: u64,
    pub awaiting_reconciliation: u64,
    pub requeued_for_verification: u64,
    pub requeued_satisfied_dependencies: u64,
    pub requeued_elapsed_backoffs: u64,
    pub settled_cancellations: u64,
    pub cancellations_awaiting_reconciliation: u64,
    pub quarantined_cancellations: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowSourceLinkRecoveryReport {
    pub inspected_active_attachments: u64,
    pub reattached_workflows: u64,
    pub inserted_links: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContractHead {
    pub contract: TaskContract,
    pub revision: u64,
}

/// Atomic evidence that one active compatibility agent lease now has canonical authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnWorkflowAttachment {
    pub task_contract: TaskContract,
    pub attempt: WorkflowAttempt,
    pub attachment: WorkflowSourceAttachment,
    pub source_links: Vec<WorkflowSourceLink>,
    pub creation_event: WorkflowEvent,
}

/// Atomic authority returned when an active compatibility lease is attached and claimed.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTurnWorkflowAuthority {
    pub workflow: AgentTurnWorkflowAttachment,
    pub legacy_lease: AgentLease,
}

/// Canonical workflow derived only from one exact active durable plugin invocation/revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInvocationWorkflowAttachment {
    pub task_contract: TaskContract,
    pub attempt: WorkflowAttempt,
    pub attachment: WorkflowSourceAttachment,
    pub source_links: Vec<WorkflowSourceLink>,
    pub creation_event: WorkflowEvent,
}

/// Atomic authority returned when an active plugin invocation is attached and claimed.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInvocationWorkflowAuthority {
    pub workflow: PluginInvocationWorkflowAttachment,
    pub invocation: PluginInvocationRecord,
    pub revision: PluginRevision,
    pub installation: PluginInstallation,
    pub task: Option<TaskEnvelope>,
    pub project: Option<ProjectModel>,
}

#[derive(Debug, Clone)]
struct ActivePluginInvocationSource {
    invocation: PluginInvocationRecord,
    invocation_json: String,
    revision: PluginRevision,
    revision_json: String,
    installation: PluginInstallation,
    installation_json: String,
    task: Option<TaskEnvelope>,
    task_json: Option<String>,
    project: Option<ProjectModel>,
    project_json: Option<String>,
}

/// Exact completed compatibility Work evidence awaiting independent Verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnVerificationTarget {
    pub lease_id: AgentLeaseId,
    pub work_item_id: WorkItemId,
    pub success_conditions: Vec<SuccessCondition>,
    pub task_contract: TaskContract,
    pub attempt: WorkflowAttempt,
    pub work_invocations: Vec<InvocationId>,
    pub work_artifacts: Vec<ArtifactRef>,
    pub work_response_hash: ContentHash,
}

/// Canonical state proposed by the compatibility-agent transaction that owns the
/// corresponding legacy projection update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentTurnWorkflowSettlement {
    WaitingForApproval,
    WorkCompleted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ApprovalWaitBinding {
    schema_version: u16,
    approval: SourceRef,
    approval_id: ApprovalId,
    tool_request_id: ToolRequestId,
    task_id: TaskId,
    compatibility_agent_lease_id: AgentLeaseId,
    requested_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

struct StoredApproval {
    approval: ApprovalRequest,
    resolved_at: Option<DateTime<Utc>>,
}

/// Computes the authoritative canonical hash for a task-contract version.
///
/// # Errors
/// Returns a storage error when the contract cannot be represented by canonical v1 encoding.
pub fn task_contract_content_hash(contract: &TaskContract) -> Result<ContentHash, StorageError> {
    let value = serde_json::to_value(contract)?;
    let payload = VersionedPayload::canonical_v1(stable_type_id(TaskContract::TYPE_ID)?, &value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    payload
        .content_hash(TASK_CONTRACT_HASH_DOMAIN)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn create_workflow_in_transaction(
    transaction: &Transaction<'_>,
    contract: &TaskContract,
    attempt: &WorkflowAttempt,
    correlation_id: CorrelationId,
    now: DateTime<Utc>,
) -> Result<WorkflowEvent, StorageError> {
    validate_initial_workflow(contract, attempt)?;
    if let Some(existing) =
        load_attempt_by_idempotency(transaction, attempt.workflow_id, &attempt.idempotency_key)?
    {
        let event =
            load_event_by_idempotency(transaction, attempt.workflow_id, &attempt.idempotency_key)?
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "workflow exists without its atomic creation event".into(),
                    )
                })?;
        if event.type_id.as_str() != WORKFLOW_CREATED_TYPE_ID
            || event_attempt(&event)? != *attempt
            || existing.workflow_id != attempt.workflow_id
        {
            return Err(StorageError::Conflict(
                "workflow create idempotency key was replayed with different content".into(),
            ));
        }
        return Ok(event);
    }

    let contract_hash = task_contract_content_hash(contract)?;
    if attempt.task_contract.content_hash != contract_hash {
        return Err(StorageError::InvalidData(
            "workflow attempt does not bind the canonical task-contract hash".into(),
        ));
    }

    transaction.execute(
        "INSERT INTO task_contract_versions(
            task_contract_id, version, type_id, schema_version, content_hash_json,
            contract_json, status, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            contract.id.to_string(),
            sql_u64(contract.version, "task contract version")?,
            TaskContract::TYPE_ID,
            i64::from(TaskContract::SCHEMA_VERSION),
            serde_json::to_string(&contract_hash)?,
            serde_json::to_string(contract)?,
            task_contract_status_label(contract.status),
            contract.created_at.to_rfc3339(),
        ],
    )?;
    transaction.execute(
        "INSERT INTO task_contract_heads(task_contract_id, current_version, revision)
         VALUES (?1, ?2, 1)",
        params![
            contract.id.to_string(),
            sql_u64(contract.version, "task contract version")?,
        ],
    )?;
    for dependency in &contract.dependencies {
        transaction.execute(
            "INSERT INTO task_contract_dependencies(
                task_contract_id, task_contract_version, dependency_id, dependency_version
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                contract.id.to_string(),
                sql_u64(contract.version, "task contract version")?,
                dependency.id.to_string(),
                sql_u64(dependency.version, "dependency version")?,
            ],
        )?;
    }
    insert_attempt(transaction, attempt)?;

    let event = workflow_event(
        attempt,
        1,
        correlation_id,
        None,
        &attempt.idempotency_key,
        WORKFLOW_CREATED_TYPE_ID,
        now,
        &serde_json::json!({
            "task_contract_id": contract.id,
            "task_contract_version": contract.version,
            "attempt_id": attempt.attempt_id,
            "state": attempt.state,
            "attempt": attempt,
        }),
    )?;
    insert_event(transaction, &event)?;
    Ok(event)
}

impl SqliteEventStore {
    /// Atomically persists one task-contract version, its initial Ready attempt, and event 1.
    ///
    /// Replaying the exact `(workflow_id, idempotency_key)` request returns its original event.
    ///
    /// # Errors
    /// Returns a storage error for an invalid initial contract/attempt, conflicting replay, or
    /// failed transaction.
    pub fn create_workflow(
        &mut self,
        contract: &TaskContract,
        attempt: &WorkflowAttempt,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<WorkflowEvent, StorageError> {
        let transaction = self.connection.transaction()?;
        let event =
            create_workflow_in_transaction(&transaction, contract, attempt, correlation_id, now)?;
        transaction.commit()?;
        Ok(event)
    }

    /// Atomically creates one canonical workflow and its immutable pre-canonical source link.
    ///
    /// Exact replay returns the original evidence. Reusing either source identity or workflow
    /// identity with different content fails closed.
    ///
    /// # Errors
    /// Returns a storage error for invalid evidence, substituted bindings, conflicts, or commit
    /// failure.
    pub fn create_attached_workflow(
        &mut self,
        contract: &TaskContract,
        attempt: &WorkflowAttempt,
        attachment: &WorkflowSourceAttachment,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<(WorkflowEvent, WorkflowSourceAttachment), StorageError> {
        validate_workflow_source_attachment(contract, attempt, attachment)?;
        let transaction = self.connection.transaction()?;
        let event =
            create_workflow_in_transaction(&transaction, contract, attempt, correlation_id, now)?;
        let stored = insert_or_replay_workflow_source_attachment(&transaction, attachment)?;
        transaction.commit()?;
        Ok((event, stored))
    }

    /// Attaches one exact active compatibility agent lease to a newly canonical workflow.
    ///
    /// Root derives the contract and source snapshot from the authoritative task, run, work plan,
    /// instance, and lease rows inside the same transaction. Callers supply only the lease identity
    /// and observation time. Existing exact attachment evidence is returned without consulting
    /// mutable compatibility projections.
    ///
    /// # Errors
    /// Returns a storage error for unknown, expired, inactive, inconsistent, malformed, or
    /// concurrently substituted compatibility authority.
    pub fn attach_active_agent_lease_workflow(
        &mut self,
        lease_id: AgentLeaseId,
        now: DateTime<Utc>,
    ) -> Result<AgentTurnWorkflowAttachment, StorageError> {
        let transaction = self.connection.transaction()?;
        let receipt =
            attach_active_agent_lease_workflow_in_transaction(&transaction, lease_id, now)?;
        transaction.commit()?;
        Ok(receipt)
    }

    /// Atomically attaches and claims one currently active compatibility agent lease.
    ///
    /// Existing attachment evidence remains immutable, but every claim also reloads and validates
    /// the mutable compatibility chain so an expired, completed, or reassigned lease cannot create
    /// fresh canonical execution authority. Exact same-owner replay returns the current Running
    /// attempt; another owner or state fails closed.
    ///
    /// # Errors
    /// Returns a storage error for stale compatibility authority, conflicting ownership, invalid
    /// lease bounds, malformed evidence, or a failed transaction.
    pub fn claim_active_agent_lease_workflow(
        &mut self,
        lease_id: AgentLeaseId,
        lease_owner: RuntimeActorId,
        maximum_lease_seconds: u64,
        now: DateTime<Utc>,
    ) -> Result<AgentTurnWorkflowAuthority, StorageError> {
        if maximum_lease_seconds == 0 || maximum_lease_seconds > 24 * 60 * 60 {
            return Err(StorageError::InvalidData(
                "canonical agent workflow lease bound is invalid".into(),
            ));
        }
        let maximum_lease_seconds = i64::try_from(maximum_lease_seconds).map_err(|_| {
            StorageError::InvalidData("canonical agent workflow lease bound overflowed".into())
        })?;
        let transaction = self.connection.transaction()?;
        let mut workflow =
            attach_active_agent_lease_workflow_in_transaction(&transaction, lease_id, now)?;
        let source = load_active_agent_turn_source(&transaction, lease_id, now)?;
        if workflow.source_links.is_empty() {
            workflow.source_links =
                derive_agent_turn_source_links(&source, &workflow.attachment, now)?;
            for link in &workflow.source_links {
                insert_or_replay_workflow_source_link(&transaction, link)?;
            }
        }
        let maximum_expiry = now
            .checked_add_signed(chrono::Duration::seconds(maximum_lease_seconds))
            .ok_or_else(|| {
                StorageError::InvalidData("canonical agent workflow lease expiry overflowed".into())
            })?;
        let lease_expires_at = source.lease.expires_at.min(maximum_expiry);
        let current = load_attempt(&transaction, workflow.attempt.attempt_id)?.ok_or(
            StorageError::WorkflowAttemptNotFound(workflow.attempt.attempt_id),
        )?;
        let current = if current.state == WorkflowState::Waiting(WaitReason::Approval) {
            resume_approval_wait_in_transaction(&transaction, current, lease_id, now)?
        } else {
            current
        };
        let attempt = match current.state {
            WorkflowState::Ready => claim_workflow_attempt_in_transaction(
                &transaction,
                current.attempt_id,
                current.revision,
                current.lease_generation,
                lease_owner,
                lease_expires_at,
                CorrelationId::new(),
                &format!(
                    "claim-agent-lease:{lease_id}:{lease_owner}:{}:{}",
                    current.revision, current.lease_generation
                ),
                now,
            )?,
            WorkflowState::Running
                if current.lease_owner == Some(lease_owner)
                    && current.lease_expires_at.is_some_and(|expiry| expiry > now) =>
            {
                current
            }
            _ => {
                return Err(StorageError::Conflict(
                    "canonical agent workflow is not claimable by this Runtime actor".into(),
                ));
            }
        };
        workflow.attempt = attempt;
        transaction.commit()?;
        Ok(AgentTurnWorkflowAuthority {
            workflow,
            legacy_lease: source.lease,
        })
    }

    /// Atomically attaches and claims one currently Running plugin invocation.
    ///
    /// The store derives the canonical contract, workflow, source snapshot, revision, module,
    /// installation state, and plugin identity from authoritative rows. The caller supplies only
    /// the invocation identity, Root-owned actor, and bounded lease duration.
    ///
    /// # Errors
    /// Returns a storage error for stale/terminal invocation state, disabled or changed plugin
    /// authority, conflicting canonical ownership, invalid lease bounds, or damaged evidence.
    pub fn claim_active_plugin_invocation_workflow(
        &mut self,
        invocation_id: PluginInvocationId,
        lease_owner: RuntimeActorId,
        maximum_lease_seconds: u64,
        now: DateTime<Utc>,
    ) -> Result<PluginInvocationWorkflowAuthority, StorageError> {
        if maximum_lease_seconds == 0 || maximum_lease_seconds > 24 * 60 * 60 {
            return Err(StorageError::InvalidData(
                "canonical plugin workflow lease bound is invalid".into(),
            ));
        }
        let maximum_lease_seconds = i64::try_from(maximum_lease_seconds).map_err(|_| {
            StorageError::InvalidData("canonical plugin workflow lease bound overflowed".into())
        })?;
        let transaction = self.connection.transaction()?;
        let mut workflow = attach_active_plugin_invocation_workflow_in_transaction(
            &transaction,
            invocation_id,
            now,
        )?;
        let source = load_active_plugin_invocation_source(&transaction, invocation_id, now)?;
        let source_type = stable_type_id("mcg.plugin_invocation_source.v1")?;
        let current_source = plugin_invocation_source_snapshot(&source, source_type)?;
        let current_source_hash = current_source
            .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if workflow.attachment.source.content_hash != current_source_hash
            || workflow.attachment.source_snapshot != current_source
        {
            return Err(StorageError::Conflict(
                "plugin invocation authority changed after canonical attachment".into(),
            ));
        }
        let lease_expires_at = now
            .checked_add_signed(chrono::Duration::seconds(maximum_lease_seconds))
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "canonical plugin workflow lease expiry overflowed".into(),
                )
            })?;
        let current = load_attempt(&transaction, workflow.attempt.attempt_id)?.ok_or(
            StorageError::WorkflowAttemptNotFound(workflow.attempt.attempt_id),
        )?;
        let attempt = match current.state {
            WorkflowState::Ready => claim_workflow_attempt_in_transaction(
                &transaction,
                current.attempt_id,
                current.revision,
                current.lease_generation,
                lease_owner,
                lease_expires_at,
                CorrelationId::new(),
                &format!(
                    "claim-plugin-invocation:{invocation_id}:{lease_owner}:{}:{}",
                    current.revision, current.lease_generation
                ),
                now,
            )?,
            WorkflowState::Running
                if current.lease_owner == Some(lease_owner)
                    && current.lease_expires_at.is_some_and(|expiry| expiry > now) =>
            {
                current
            }
            _ => {
                return Err(StorageError::Conflict(
                    "canonical plugin workflow is not claimable by this Runtime actor".into(),
                ));
            }
        };
        workflow.attempt = attempt;
        transaction.commit()?;
        Ok(PluginInvocationWorkflowAuthority {
            workflow,
            invocation: source.invocation,
            revision: source.revision,
            installation: source.installation,
            task: source.task,
            project: source.project,
        })
    }

    /// Atomically settles one terminal plugin invocation and its internal authority workflow.
    ///
    /// A successful adapter workflow requires every exact source-bound action to have reached a
    /// definitive state. Observational actions must be `NoEffectConfirmed`; reversible actions
    /// must be committed from validated `EffectConfirmed` evidence. A failed invocation with an
    /// unresolved effect atomically waits for reconciliation instead of falsely closing its
    /// authority workflow. This internal compatibility workflow is not an ordinary cognitive task
    /// and therefore does not fabricate model Work/Verify invocations or a
    /// `CompletionCertificate`.
    ///
    /// # Errors
    /// Returns a storage error for missing attachment, stale Root ownership, substituted action
    /// evidence, nonterminal invocation data, replay mismatch, or transaction failure.
    pub fn complete_plugin_invocation_workflow(
        &mut self,
        completed: &PluginInvocationRecord,
        lease_owner: RuntimeActorId,
        settled_at: DateTime<Utc>,
    ) -> Result<(PluginInvocationRecord, WorkflowAttempt), StorageError> {
        let transaction = self.connection.transaction()?;
        let source_type = stable_type_id("mcg.plugin_invocation_source.v1")?;
        let attachment = load_workflow_source_attachment_by_source(
            &transaction,
            source_type.as_str(),
            &completed.id.to_string(),
        )?
        .ok_or_else(|| {
            StorageError::Conflict(
                "terminal plugin invocation has no canonical authority workflow".into(),
            )
        })?;
        let workflow = load_agent_turn_workflow_attachment(&transaction, attachment)?;
        let attempt = load_attempt(&transaction, workflow.attempt.attempt_id)?.ok_or(
            StorageError::WorkflowAttemptNotFound(workflow.attempt.attempt_id),
        )?;
        let successful = completed.state == PluginInvocationState::Succeeded;
        if !successful
            && !matches!(
                completed.state,
                PluginInvocationState::Failed | PluginInvocationState::Uncertain
            )
        {
            return Err(StorageError::InvalidData(
                "plugin workflow settlement requires a terminal invocation".into(),
            ));
        }
        let effects = cancellation_effect_status(&transaction, attempt.attempt_id)?;
        let cancellation_settlement = attempt.state == WorkflowState::CancelRequested;
        if cancellation_settlement && successful {
            return Err(StorageError::Conflict(
                "a plugin result cannot succeed after its workflow cancellation fence".into(),
            ));
        }
        let target_state = if cancellation_settlement && effects.unresolved == 0 {
            WorkflowState::Cancelled
        } else if cancellation_settlement
            && effects.reconciliation_required > 0
            && effects.quarantined == 0
        {
            WorkflowState::Waiting(WaitReason::Reconciliation)
        } else if cancellation_settlement {
            WorkflowState::Quarantined
        } else if successful {
            WorkflowState::Succeeded
        } else if effects.unresolved == 0 {
            WorkflowState::Failed
        } else if effects.reconciliation_required > 0 && effects.quarantined == 0 {
            WorkflowState::Waiting(WaitReason::Reconciliation)
        } else {
            WorkflowState::Quarantined
        };
        let exact_state_replay = attempt.state == target_state
            && (attempt.state.is_terminal()
                || attempt.state == WorkflowState::Waiting(WaitReason::Reconciliation));
        if exact_state_replay {
            // Exact terminal/wait replay is validated by the plugin record mutation below.
        } else {
            let valid_cancel_settlement = cancellation_settlement
                && attempt.lease_owner.is_none()
                && attempt.lease_expires_at.is_none()
                && completed.completed_at == Some(settled_at);
            let valid_active_settlement = attempt.state == WorkflowState::Running
                && attempt.lease_owner == Some(lease_owner)
                && attempt
                    .lease_expires_at
                    .is_some_and(|expires_at| expires_at > settled_at)
                && completed.completed_at == Some(settled_at);
            if !valid_cancel_settlement && !valid_active_settlement {
                return Err(StorageError::Conflict(
                    "plugin workflow is not held by the settling Root actor or cancellation fence"
                        .into(),
                ));
            }
        }
        if successful {
            validate_successful_plugin_action(&transaction, &workflow.attachment, &attempt)?;
        }
        let completed = complete_plugin_invocation_in_transaction(&transaction, completed)?;
        let settled = if exact_state_replay {
            attempt
        } else if target_state == WorkflowState::Succeeded {
            let verifying = transition_workflow_attempt_in_transaction(
                &transaction,
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                &format!("verify-plugin-invocation:{}", completed.id),
                settled_at,
            )?;
            transition_workflow_attempt_in_transaction(
                &transaction,
                verifying.attempt_id,
                verifying.revision,
                verifying.lease_generation,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                &format!("settle-plugin-invocation:{}", completed.id),
                settled_at,
            )?
        } else {
            transition_workflow_attempt_in_transaction(
                &transaction,
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                target_state,
                CorrelationId::new(),
                &format!("fail-plugin-invocation:{}", completed.id),
                settled_at,
            )?
        };
        transaction.commit()?;
        Ok((completed, settled))
    }

    /// Closes a failed plugin authority workflow after every uncertain effect is definitively
    /// reconciled.
    ///
    /// The terminal plugin invocation is already durable. This method only advances its internal
    /// compatibility workflow from `Waiting(Reconciliation)` to `Failed` once no governed action
    /// remains unresolved; it never redispatches plugin or effect work.
    ///
    /// # Errors
    /// Returns a storage error for substituted source attachment, contradictory invocation state,
    /// damaged action evidence, or transaction failure.
    pub fn settle_reconciled_plugin_invocation_workflow(
        &mut self,
        invocation_id: PluginInvocationId,
        settled_at: DateTime<Utc>,
    ) -> Result<Option<WorkflowAttempt>, StorageError> {
        let transaction = self.connection.transaction()?;
        let source_type = stable_type_id("mcg.plugin_invocation_source.v1")?;
        let Some(attachment) = load_workflow_source_attachment_by_source(
            &transaction,
            source_type.as_str(),
            &invocation_id.to_string(),
        )?
        else {
            return Ok(None);
        };
        let workflow = load_agent_turn_workflow_attachment(&transaction, attachment)?;
        let invocation = load_plugin_invocation(&transaction, invocation_id)?.ok_or_else(|| {
            StorageError::InvalidData("plugin workflow source invocation is missing".into())
        })?;
        if !matches!(
            invocation.state,
            PluginInvocationState::Failed | PluginInvocationState::Uncertain
        ) {
            return Err(StorageError::Conflict(
                "reconciled plugin workflow does not have a failed terminal invocation".into(),
            ));
        }
        let attempt = load_attempt(&transaction, workflow.attempt.attempt_id)?.ok_or(
            StorageError::WorkflowAttemptNotFound(workflow.attempt.attempt_id),
        )?;
        if attempt.state == WorkflowState::Failed {
            transaction.commit()?;
            return Ok(Some(attempt));
        }
        if attempt.state != WorkflowState::Waiting(WaitReason::Reconciliation) {
            transaction.commit()?;
            return Ok(None);
        }
        let effects = cancellation_effect_status(&transaction, attempt.attempt_id)?;
        if effects.unresolved > 0 {
            transaction.commit()?;
            return Ok(None);
        }
        let settled = transition_workflow_attempt_in_transaction(
            &transaction,
            attempt.attempt_id,
            attempt.revision,
            attempt.lease_generation,
            WorkflowState::Failed,
            CorrelationId::new(),
            &format!("settle-reconciled-plugin-invocation:{invocation_id}"),
            settled_at,
        )?;
        transaction.commit()?;
        Ok(Some(settled))
    }

    /// Lists every completed compatibility lease in one run that is durably attached to an exact
    /// Verifying workflow, including its objectively mappable planned conditions and completed
    /// Work invocation lineage.
    ///
    /// Legacy-only leases without a canonical source attachment are omitted. A present attachment
    /// is authoritative and therefore fails closed on stale state, changed lineage, or missing
    /// completed response evidence.
    ///
    /// # Errors
    /// Returns a storage error for malformed legacy/canonical rows, a non-Verifying attachment,
    /// substituted invocation lineage, or absent completed Work response evidence.
    pub fn agent_turn_verification_targets(
        &self,
        run_id: RunId,
    ) -> Result<Vec<AgentTurnVerificationTarget>, StorageError> {
        let leases: Vec<AgentLease> = self.read_json_rows(
            "SELECT agent_leases.lease_json FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             WHERE agent_instances.run_id = ?1 AND agent_leases.state = 'completed'
             ORDER BY agent_leases.acquired_at, agent_leases.lease_id",
            &run_id.to_string(),
        )?;
        let plan: WorkPlan = self
            .connection
            .query_row(
                "SELECT plan_json FROM work_plans WHERE run_id = ?1",
                [run_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json))
            .transpose()?
            .ok_or_else(|| StorageError::InvalidData("verification run has no work plan".into()))?;
        let source_type = stable_type_id("mcg.agent_turn_source.v1")?;
        let mut targets = Vec::new();
        for lease in leases {
            let Some(attachment) = load_workflow_source_attachment_by_source(
                &self.connection,
                source_type.as_str(),
                &lease.id.to_string(),
            )?
            else {
                continue;
            };
            let workflow = load_agent_turn_workflow_attachment(&self.connection, attachment)?;
            if workflow.attempt.state != WorkflowState::Verifying {
                return Err(StorageError::Conflict(
                    "attached completed agent turn is not awaiting independent verification".into(),
                ));
            }
            let item = plan
                .items
                .iter()
                .find(|item| item.id == lease.work_item_id)
                .ok_or_else(|| {
                    StorageError::Conflict(
                        "completed compatibility lease no longer matches its planned work item"
                            .into(),
                    )
                })?;
            let completed = completed_work_invocations(&self.connection, &workflow.attempt)?;
            let work_response_hash = completed
                .last()
                .map(|(_, artifact)| artifact.content_hash.clone())
                .ok_or_else(|| {
                    StorageError::Conflict(
                        "attached completed agent turn has no completed Work response".into(),
                    )
                })?;
            targets.push(AgentTurnVerificationTarget {
                lease_id: lease.id,
                work_item_id: lease.work_item_id,
                success_conditions: item.success_conditions.clone(),
                task_contract: workflow.task_contract,
                attempt: workflow.attempt,
                work_invocations: completed
                    .iter()
                    .map(|(invocation_id, _)| *invocation_id)
                    .collect(),
                work_artifacts: completed
                    .into_iter()
                    .map(|(_, artifact)| artifact)
                    .collect(),
                work_response_hash,
            });
        }
        Ok(targets)
    }
}

fn completed_work_invocations(
    connection: &Connection,
    attempt: &WorkflowAttempt,
) -> Result<Vec<(InvocationId, ArtifactRef)>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT invocation.manifest_json, version.run_json
         FROM invocation_manifests AS invocation
         JOIN model_run_heads AS head
           ON head.invocation_id = invocation.invocation_id
         JOIN model_run_versions AS version
           ON version.model_run_id = head.model_run_id
          AND version.revision = head.current_revision
         WHERE invocation.workflow_attempt_id = ?1
           AND invocation.role = 'work'
           AND version.state = 'completed'
         ORDER BY invocation.persisted_at, invocation.invocation_id",
    )?;
    let rows = statement.query_map([attempt.attempt_id.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut completed = Vec::new();
    for row in rows {
        let (manifest_json, run_json) = row?;
        let manifest: InvocationManifest = serde_json::from_str(&manifest_json)?;
        let run: ModelRun = serde_json::from_str(&run_json)?;
        manifest
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        run.validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if manifest.role != InvocationRole::Work
            || manifest.attempt_id != attempt.attempt_id
            || manifest.task_contract != attempt.task_contract
            || run.invocation_id != manifest.id
            || run.role != InvocationRole::Work
            || run.state != ModelRunState::Completed
        {
            return Err(StorageError::Conflict(
                "completed compatibility Work invocation lineage was substituted".into(),
            ));
        }
        let response_artifact = run.response_artifact.ok_or_else(|| {
            StorageError::InvalidData(
                "completed compatibility Work invocation has no response artifact".into(),
            )
        })?;
        completed.push((manifest.id, response_artifact));
    }
    Ok(completed)
}

fn attach_active_agent_lease_workflow_in_transaction(
    transaction: &Transaction<'_>,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<AgentTurnWorkflowAttachment, StorageError> {
    let source_type = stable_type_id("mcg.agent_turn_source.v1")?;
    let source_locator = lease_id.to_string();

    if let Some(attachment) = load_workflow_source_attachment_by_source(
        transaction,
        source_type.as_str(),
        &source_locator,
    )? {
        return load_agent_turn_workflow_attachment(transaction, attachment);
    }

    let source = load_active_agent_turn_source(transaction, lease_id, now)?;
    let (task_contract, mut attempt, attachment) =
        derive_agent_turn_workflow(&source, source_type, now)?;
    attempt.task_contract.content_hash = task_contract_content_hash(&task_contract)?;
    let mut attachment = attachment;
    attachment.task_contract = attempt.task_contract.clone();
    let source_links = derive_agent_turn_source_links(&source, &attachment, now)?;
    let creation_event = create_workflow_in_transaction(
        transaction,
        &task_contract,
        &attempt,
        CorrelationId::new(),
        now,
    )?;
    insert_or_replay_workflow_source_attachment(transaction, &attachment)?;
    for link in &source_links {
        insert_or_replay_workflow_source_link(transaction, link)?;
    }
    Ok(AgentTurnWorkflowAttachment {
        task_contract,
        attempt,
        attachment,
        source_links,
        creation_event,
    })
}

fn attach_active_plugin_invocation_workflow_in_transaction(
    transaction: &Transaction<'_>,
    invocation_id: PluginInvocationId,
    now: DateTime<Utc>,
) -> Result<PluginInvocationWorkflowAttachment, StorageError> {
    let source_type = stable_type_id("mcg.plugin_invocation_source.v1")?;
    let source_locator = invocation_id.to_string();
    if let Some(attachment) = load_workflow_source_attachment_by_source(
        transaction,
        source_type.as_str(),
        &source_locator,
    )? {
        let workflow = load_agent_turn_workflow_attachment(transaction, attachment)?;
        return Ok(PluginInvocationWorkflowAttachment {
            task_contract: workflow.task_contract,
            attempt: workflow.attempt,
            attachment: workflow.attachment,
            source_links: workflow.source_links,
            creation_event: workflow.creation_event,
        });
    }

    let source = load_active_plugin_invocation_source(transaction, invocation_id, now)?;
    let (task_contract, mut attempt, attachment) =
        derive_plugin_invocation_workflow(&source, source_type, now)?;
    attempt.task_contract.content_hash = task_contract_content_hash(&task_contract)?;
    let mut attachment = attachment;
    attachment.task_contract = attempt.task_contract.clone();
    let creation_event = create_workflow_in_transaction(
        transaction,
        &task_contract,
        &attempt,
        CorrelationId::new(),
        now,
    )?;
    insert_or_replay_workflow_source_attachment(transaction, &attachment)?;
    Ok(PluginInvocationWorkflowAttachment {
        task_contract,
        attempt,
        attachment,
        source_links: Vec::new(),
        creation_event,
    })
}

fn load_active_plugin_invocation_source(
    connection: &Connection,
    invocation_id: PluginInvocationId,
    now: DateTime<Utc>,
) -> Result<ActivePluginInvocationSource, StorageError> {
    let invocation = load_plugin_invocation(connection, invocation_id)?
        .ok_or_else(|| StorageError::InvalidData("plugin invocation was not found".into()))?;
    validate_running_invocation(&invocation)?;
    if invocation.state != PluginInvocationState::Running || invocation.started_at > now {
        return Err(StorageError::Conflict(
            "plugin invocation is not active at the authorization time".into(),
        ));
    }
    let revision = load_plugin_revision(connection, invocation.plugin_revision_id)?.ok_or(
        StorageError::PluginRevisionNotFound(invocation.plugin_revision_id),
    )?;
    validate_plugin_revision(&revision)?;
    let installation = load_plugin_installation(connection, &invocation.plugin_id)?
        .ok_or_else(|| StorageError::PluginNotFound(invocation.plugin_id.clone()))?;
    if !installation.enabled
        || installation.active_revision_id != revision.id
        || revision.manifest.id != invocation.plugin_id
    {
        return Err(StorageError::Conflict(
            "plugin installation authority changed before canonical execution".into(),
        ));
    }
    let (task, task_json, project, project_json) =
        if revision.manifest.requested_capabilities.is_empty() {
            (None, None, None, None)
        } else {
            let task_id = invocation.task_id.ok_or_else(|| {
                StorageError::Conflict("plugin broker authority requires an exact task".into())
            })?;
            let task_json = connection
                .query_row(
                    "SELECT task_json FROM tasks WHERE task_id = ?1",
                    [task_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::TaskNotFound(task_id))?;
            let task: TaskEnvelope = serde_json::from_str(&task_json)?;
            let project_id = task.project_id.ok_or_else(|| {
                StorageError::Conflict(
                    "plugin broker authority requires an exact task project".into(),
                )
            })?;
            let project_json = connection
                .query_row(
                    "SELECT project_json FROM projects WHERE project_id = ?1",
                    [project_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(StorageError::ProjectNotFound(project_id))?;
            let project: ProjectModel = serde_json::from_str(&project_json)?;
            (
                Some(task),
                Some(task_json),
                Some(project),
                Some(project_json),
            )
        };
    Ok(ActivePluginInvocationSource {
        invocation_json: serde_json::to_string(&invocation)?,
        revision_json: serde_json::to_string(&revision)?,
        installation_json: serde_json::to_string(&installation)?,
        invocation,
        revision,
        installation,
        task,
        task_json,
        project,
        project_json,
    })
}

fn plugin_invocation_source_snapshot(
    source: &ActivePluginInvocationSource,
    source_type: StableTypeId,
) -> Result<VersionedPayload, StorageError> {
    VersionedPayload::canonical_v1(
        source_type,
        &serde_json::json!({
            "invocation_id": source.invocation.id,
            "invocation_sha256": sha256_text(&source.invocation_json),
            "plugin_id": source.invocation.plugin_id,
            "plugin_revision_id": source.revision.id,
            "revision_sha256": sha256_text(&source.revision_json),
            "installation_revision": source.installation.revision,
            "installation_sha256": sha256_text(&source.installation_json),
            "module_sha256": source.revision.module_sha256,
            "input_sha256": source.invocation.input_sha256,
            "task_id": source.invocation.task_id,
            "task_sha256": source.task_json.as_deref().map(sha256_text),
            "project_id": source.project.as_ref().map(|project| project.project_id),
            "project_sha256": source.project_json.as_deref().map(sha256_text),
        }),
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))
}

#[allow(clippy::too_many_lines)]
fn derive_plugin_invocation_workflow(
    source: &ActivePluginInvocationSource,
    source_type: StableTypeId,
    now: DateTime<Utc>,
) -> Result<(TaskContract, WorkflowAttempt, WorkflowSourceAttachment), StorageError> {
    let snapshot = plugin_invocation_source_snapshot(source, source_type.clone())?;
    let source_ref = SourceRef {
        source_type,
        locator: source.invocation.id.to_string(),
        content_hash: snapshot
            .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    };
    let authority = agent_turn_policy(
        "mcg.plugin_invocation_authority_policy.v1",
        "MetaCog.PluginInvocationAuthority",
        &serde_json::json!({
            "invocation_id": source.invocation.id,
            "plugin_id": source.invocation.plugin_id,
            "plugin_revision_id": source.revision.id,
            "installation_revision": source.installation.revision,
        }),
    )?;
    let environment = agent_turn_policy(
        "mcg.plugin_invocation_environment_policy.v1",
        "MetaCog.PluginInvocationEnvironment",
        &serde_json::json!({
            "module_sha256": source.revision.module_sha256,
            "input_sha256": source.invocation.input_sha256,
            "task_id": source.invocation.task_id,
            "source_snapshot_hash": source_ref.content_hash,
        }),
    )?;
    let side_effects = agent_turn_policy(
        "mcg.plugin_invocation_side_effect_policy.v1",
        "MetaCog.PluginInvocationSideEffects",
        &serde_json::json!({
            "maximum_effect": "observational",
            "ambient_capabilities": [],
        }),
    )?;
    let risk = agent_turn_policy(
        "mcg.plugin_invocation_risk_policy.v1",
        "MetaCog.PluginInvocationRisk",
        &serde_json::json!({
            "isolation": "closed_ambient_wasip1",
            "requested_broker_capabilities": source.revision.manifest.requested_capabilities,
        }),
    )?;
    let budgets = agent_turn_policy(
        "mcg.plugin_invocation_budget_policy.v1",
        "MetaCog.PluginInvocationBudget",
        &serde_json::json!({"physical_runner_attempts": 1}),
    )?;
    let rollback = agent_turn_policy(
        "mcg.plugin_invocation_rollback_policy.v1",
        "MetaCog.PluginInvocationRollback",
        &serde_json::json!({"mode": "no_external_effect_to_rollback"}),
    )?;
    let task_contract_id = TaskContractId::new();
    let task_contract = TaskContract {
        id: task_contract_id,
        version: 1,
        previous_version: None,
        user_intent_ref: source_ref.clone(),
        objective: format!("Execute sandboxed plugin {}", source.invocation.plugin_id),
        non_goals: vec!["Grant ambient host authority to the WASI guest".into()],
        requirements: vec![ContractClause {
            clause_id: "exact-plugin-revision".into(),
            statement: format!(
                "Execute plugin {} revision {} with exact module and input digests",
                source.invocation.plugin_id, source.revision.id
            ),
            source_refs: vec![source_ref.clone()],
        }],
        constraints: vec![ContractClause {
            clause_id: "closed-ambient-authority".into(),
            statement: "The guest receives no ambient filesystem, network, process, environment, terminal, or secret authority".into(),
            source_refs: vec![source_ref.clone()],
        }],
        assumptions: Vec::new(),
        acceptance_criteria: vec![ContractClause {
            clause_id: "bounded-runner-result".into(),
            statement: "The fixed runner returns one identity-matched bounded result or a categorical failure".into(),
            source_refs: vec![source_ref.clone()],
        }],
        dependencies: Vec::new(),
        permitted_side_effects: side_effects,
        authority,
        environment: environment.clone(),
        risk,
        budgets,
        rollback: rollback.clone(),
        status: TaskContractStatus::Active,
        created_at: now,
    };
    let workflow_id = WorkflowId::new();
    let attempt_id = AttemptId::new();
    let attempt = WorkflowAttempt {
        workflow_id,
        attempt_id,
        task_contract: VersionedRef {
            id: task_contract_id,
            version: 1,
            content_hash: source_ref.content_hash.clone(),
        },
        parent_attempt_id: None,
        attempt_number: 1,
        state: WorkflowState::Ready,
        revision: 1,
        lease_owner: None,
        lease_generation: 0,
        lease_expires_at: None,
        heartbeat_at: None,
        checkpoint: None,
        environment_snapshot: environment.content_hash,
        configuration: VersionedRef {
            id: ConfigurationId::new(),
            version: 1,
            content_hash: source_ref.content_hash.clone(),
        },
        idempotency_key: format!("attach-plugin-invocation:{}", source.invocation.id),
        retry_policy: RetryPolicy {
            max_attempts: 1,
            initial_backoff_ms: 0,
            maximum_backoff_ms: 0,
        },
        compensation_policy: rollback,
        created_at: now,
        updated_at: now,
    };
    let attachment = WorkflowSourceAttachment {
        schema_version: WorkflowSourceAttachment::SCHEMA_VERSION,
        source: source_ref,
        source_snapshot: snapshot,
        workflow_id,
        attempt_id,
        task_contract: attempt.task_contract.clone(),
        attached_at: now,
    };
    Ok((task_contract, attempt, attachment))
}

fn validate_successful_plugin_action(
    transaction: &Transaction<'_>,
    attachment: &WorkflowSourceAttachment,
    attempt: &WorkflowAttempt,
) -> Result<(), StorageError> {
    let rows: Vec<(String, String)> = {
        let mut statement = transaction.prepare(
            "SELECT action_envelopes.envelope_json, action_executions.execution_json
             FROM action_envelopes
             INNER JOIN action_executions
                ON action_executions.action_id = action_envelopes.action_id
             WHERE action_envelopes.workflow_id = ?1
               AND action_envelopes.workflow_attempt_id = ?2
             ORDER BY action_executions.attempt_number, action_executions.execution_id",
        )?;
        let rows = statement.query_map(
            params![
                attempt.workflow_id.to_string(),
                attempt.attempt_id.to_string()
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if rows.is_empty() || rows.len() > 33 {
        return Err(StorageError::Conflict(
            "successful plugin workflow has an invalid governed action count".into(),
        ));
    }
    let mut runner_action_present = false;
    for (envelope_json, execution_json) in rows {
        let envelope: ActionEnvelope = serde_json::from_str(&envelope_json)?;
        let execution: ActionExecution = serde_json::from_str(&execution_json)?;
        let exact_source = envelope
            .origin
            .as_ref()
            .and_then(|origin| origin.invocation.as_ref())
            == Some(&attachment.source);
        runner_action_present |=
            envelope.operation.type_id.as_str() == "mcg.plugin_runner_invocation.v1";
        let definitive_outcome = match envelope.effect_class {
            EffectClass::Observational => {
                execution.state == ActionExecutionState::NoEffectConfirmed
                    && execution.outcome == Some(EffectOutcome::NoEffectConfirmed)
            }
            EffectClass::Reversible => {
                execution.state == ActionExecutionState::Committed
                    && matches!(
                        execution.outcome,
                        Some(EffectOutcome::EffectConfirmed { .. })
                    )
                    && envelope.compensation.is_some()
                    && envelope.reconciler.is_some()
            }
            EffectClass::Irreversible => {
                execution.state == ActionExecutionState::Committed
                    && matches!(
                        execution.outcome,
                        Some(EffectOutcome::EffectConfirmed { .. })
                    )
                    && envelope.elevated_authority.is_some()
                    && envelope.compensation.is_none()
                    && envelope.reconciler.is_some()
            }
            EffectClass::Consequential => false,
        };
        if envelope.workflow_id != attempt.workflow_id
            || envelope.attempt_id != attempt.attempt_id
            || envelope.task_contract != attempt.task_contract
            || execution.action_id != envelope.id
            || !definitive_outcome
            || !exact_source
        {
            return Err(StorageError::Conflict(
                "successful plugin workflow action evidence is missing or substituted".into(),
            ));
        }
    }
    if !runner_action_present {
        return Err(StorageError::Conflict(
            "successful plugin workflow has no exact protected runner action".into(),
        ));
    }
    Ok(())
}

fn approval_wait_source(approval: &ApprovalRequest) -> Result<SourceRef, StorageError> {
    let source_type = StableTypeId::parse("mcg.approval_wait_source.v1")
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let payload = VersionedPayload::canonical_v1(
        source_type.clone(),
        &serde_json::json!({
            "schema_version": approval.schema_version,
            "approval_id": approval.id,
            "tool_request_id": approval.tool_request_id,
            "task_id": approval.task_id,
            "action": approval.action,
            "resource": approval.resource,
            "reason": approval.reason,
            "origin": approval.origin,
            "requested_at": approval.requested_at,
            "expires_at": approval.expires_at,
        }),
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(SourceRef {
        source_type,
        locator: approval.id.to_string(),
        content_hash: payload
            .content_hash("MetaCog.ApprovalWaitSource")
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    })
}

fn approval_wait_binding(
    approval: &ApprovalRequest,
    lease_id: AgentLeaseId,
) -> Result<ApprovalWaitBinding, StorageError> {
    Ok(ApprovalWaitBinding {
        schema_version: 1,
        approval: approval_wait_source(approval)?,
        approval_id: approval.id,
        tool_request_id: approval.tool_request_id,
        task_id: approval.task_id,
        compatibility_agent_lease_id: lease_id,
        requested_at: approval.requested_at,
        expires_at: approval.expires_at,
    })
}

fn load_stored_approval(
    transaction: &Transaction<'_>,
    request_id: ToolRequestId,
) -> Result<Option<StoredApproval>, StorageError> {
    let row = transaction
        .query_row(
            "SELECT state, approval_json, resolved_at FROM approvals WHERE tool_request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((state, json, resolved_at)) = row else {
        return Ok(None);
    };
    let approval: ApprovalRequest = serde_json::from_str(&json)?;
    if state != crate::approval_state_name(approval.state) {
        return Err(StorageError::Conflict(
            "approval row state disagrees with its durable evidence".into(),
        ));
    }
    let resolved_at = resolved_at
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| {
                    StorageError::InvalidData(format!(
                        "stored approval resolution timestamp is invalid: {error}"
                    ))
                })
        })
        .transpose()?;
    Ok(Some(StoredApproval {
        approval,
        resolved_at,
    }))
}

fn load_approval_wait_binding_for_lease(
    transaction: &Transaction<'_>,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<ApprovalWaitBinding, StorageError> {
    let source = load_active_agent_turn_source(transaction, lease_id, now)?;
    let checkpoint = transaction
        .query_row(
            "SELECT pending_tool_request_id, checkpoint_json FROM agent_turn_checkpoints
             WHERE lease_id = ?1 AND run_id = ?2 AND instance_id = ?3 AND task_id = ?4",
            params![
                lease_id.to_string(),
                source.run.id.to_string(),
                source.instance.id.to_string(),
                source.task.id.to_string(),
            ],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| {
            StorageError::Conflict(
                "approval-waiting agent has no exact pending tool-request checkpoint".into(),
            )
        })?;
    let request_id = checkpoint
        .0
        .ok_or_else(|| {
            StorageError::Conflict(
                "approval-waiting agent checkpoint has no pending tool request".into(),
            )
        })?
        .parse::<ToolRequestId>()
        .map_err(|_| {
            StorageError::InvalidData("pending approval tool-request id is invalid".into())
        })?;
    let checkpoint_json: serde_json::Value = serde_json::from_str(&checkpoint.1)?;
    if checkpoint_json.get("pending_tool_request_id") != Some(&serde_json::json!(request_id))
        || checkpoint_json.pointer("/request/instance/id")
            != Some(&serde_json::json!(source.instance.id))
        || checkpoint_json.pointer("/request/instance/run_id")
            != Some(&serde_json::json!(source.run.id))
        || checkpoint_json.pointer("/request/instance/task_id")
            != Some(&serde_json::json!(source.task.id))
        || checkpoint_json.pointer("/request/lease/id") != Some(&serde_json::json!(source.lease.id))
        || checkpoint_json.pointer("/request/lease/instance_id")
            != Some(&serde_json::json!(source.instance.id))
    {
        return Err(StorageError::Conflict(
            "approval-waiting checkpoint evidence is missing or substituted".into(),
        ));
    }
    let stored = load_stored_approval(transaction, request_id)?.ok_or_else(|| {
        StorageError::Conflict("approval-waiting checkpoint has no durable approval".into())
    })?;
    if stored.approval.state != ApprovalState::Pending
        || stored.resolved_at.is_some()
        || stored.approval.task_id != source.task.id
        || stored.approval.expires_at <= now
    {
        return Err(StorageError::Conflict(
            "approval wait source is stale, resolved, expired, or belongs to another task".into(),
        ));
    }
    approval_wait_binding(&stored.approval, lease_id)
}

fn load_current_approval_wait_binding(
    transaction: &Transaction<'_>,
    attempt: &WorkflowAttempt,
) -> Result<ApprovalWaitBinding, StorageError> {
    let event_json = transaction
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [attempt.workflow_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::Conflict("approval wait has no binding event".into()))?;
    let event: WorkflowEvent = serde_json::from_str(&event_json)?;
    let payload = event_payload_value(&event)?;
    if event.type_id.as_str() != WORKFLOW_WAIT_BOUND_TYPE_ID
        || event.attempt_id != attempt.attempt_id
        || event_attempt(&event)? != *attempt
        || payload.get("attempt_id") != Some(&serde_json::json!(attempt.attempt_id))
        || payload.get("to")
            != Some(&serde_json::json!(WorkflowState::Waiting(
                WaitReason::Approval
            )))
    {
        return Err(StorageError::Conflict(
            "approval wait binding event is missing or substituted".into(),
        ));
    }
    serde_json::from_value(
        payload
            .get("approval_wait")
            .cloned()
            .ok_or_else(|| StorageError::Conflict("approval wait binding is missing".into()))?,
    )
    .map_err(StorageError::from)
}

fn resume_approval_wait_in_transaction(
    transaction: &Transaction<'_>,
    mut attempt: WorkflowAttempt,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<WorkflowAttempt, StorageError> {
    if attempt.state != WorkflowState::Waiting(WaitReason::Approval) {
        return Err(StorageError::Conflict(
            "only an approval-waiting workflow can resume through this path".into(),
        ));
    }
    let binding = load_current_approval_wait_binding(transaction, &attempt)?;
    if binding.compatibility_agent_lease_id != lease_id {
        return Err(StorageError::Conflict(
            "approval wake was bound to a different compatibility lease".into(),
        ));
    }
    let stored = load_stored_approval(transaction, binding.tool_request_id)?
        .ok_or_else(|| StorageError::Conflict("approval wake source no longer exists".into()))?;
    let expected_binding = approval_wait_binding(&stored.approval, lease_id)?;
    let resolved_at = stored.resolved_at.ok_or_else(|| {
        StorageError::Conflict("approval wake source has no durable resolution".into())
    })?;
    if expected_binding != binding
        || stored.approval.state != ApprovalState::Approved
        || resolved_at < binding.requested_at
        || resolved_at > binding.expires_at
    {
        return Err(StorageError::Conflict(
            "approval wake source is unresolved, expired, or substituted".into(),
        ));
    }
    let expected_revision = attempt.revision;
    let expected_generation = attempt.lease_generation;
    let previous_state = attempt.state;
    attempt.state = WorkflowState::Ready;
    attempt.revision = increment(attempt.revision, "workflow revision")?;
    attempt.updated_at = now;
    update_attempt_cas(
        transaction,
        &attempt,
        expected_revision,
        expected_generation,
    )?;
    let (sequence, cause) = next_event_position(transaction, attempt.workflow_id)?;
    let event = workflow_event(
        &attempt,
        sequence,
        CorrelationId::new(),
        cause,
        &format!("resume-agent-lease:{lease_id}:{expected_revision}:{expected_generation}"),
        WORKFLOW_WAIT_SATISFIED_TYPE_ID,
        now,
        &serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "from": previous_state,
            "to": attempt.state,
            "cancellation_effects": null,
            "compatibility_agent_lease_id": lease_id,
            "wait_reason": WaitReason::Approval,
            "approval_wait": binding,
            "approval_resolved_at": resolved_at,
            "attempt": attempt,
        }),
    )?;
    insert_event(transaction, &event)?;
    Ok(attempt)
}

/// Atomically satisfies the exact approval wake bound to an attached compatibility workflow.
/// Absence of an attachment preserves pre-cutover compatibility behavior.
pub(crate) fn resume_attached_agent_approval_wait_in_transaction(
    transaction: &Transaction<'_>,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    let source_type = stable_type_id("mcg.agent_turn_source.v1")?;
    let Some(attachment) = load_workflow_source_attachment_by_source(
        transaction,
        source_type.as_str(),
        &lease_id.to_string(),
    )?
    else {
        return Ok(None);
    };
    let workflow = load_agent_turn_workflow_attachment(transaction, attachment)?;
    resume_approval_wait_in_transaction(transaction, workflow.attempt, lease_id, now).map(Some)
}

/// Advances an attached compatibility-agent workflow inside the caller's legacy
/// settlement transaction. Absence of an attachment preserves pre-cutover
/// compatibility behavior; a present attachment is authoritative and therefore
/// fails the entire transaction if its canonical lease is stale or substituted.
pub(crate) fn settle_attached_agent_turn_workflow_in_transaction(
    transaction: &Transaction<'_>,
    lease_id: AgentLeaseId,
    settlement: AgentTurnWorkflowSettlement,
    now: DateTime<Utc>,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    let source_type = stable_type_id("mcg.agent_turn_source.v1")?;
    let Some(attachment) = load_workflow_source_attachment_by_source(
        transaction,
        source_type.as_str(),
        &lease_id.to_string(),
    )?
    else {
        return Ok(None);
    };
    let mut workflow = load_agent_turn_workflow_attachment(transaction, attachment)?;
    let attempt = &mut workflow.attempt;
    let active_running_authority = attempt.state == WorkflowState::Running
        && attempt.lease_owner.is_some()
        && attempt.lease_expires_at.is_some_and(|expiry| expiry > now);
    let approval_wait_authority = settlement == AgentTurnWorkflowSettlement::Failed
        && attempt.state == WorkflowState::Waiting(WaitReason::Approval)
        && attempt.lease_owner.is_none()
        && attempt.lease_expires_at.is_none();
    if !active_running_authority && !approval_wait_authority {
        return Err(StorageError::Conflict(
            "attached canonical workflow authority changed before agent-turn settlement".into(),
        ));
    }
    let expected_revision = attempt.revision;
    let expected_generation = attempt.lease_generation;
    let previous_state = attempt.state;
    let (next_state, label) = match settlement {
        AgentTurnWorkflowSettlement::WaitingForApproval => {
            (WorkflowState::Waiting(WaitReason::Approval), "approval")
        }
        AgentTurnWorkflowSettlement::WorkCompleted => (WorkflowState::Verifying, "work-completed"),
        AgentTurnWorkflowSettlement::Failed => (WorkflowState::Failed, "failed"),
    };
    if !previous_state.can_transition_to(next_state) {
        return Err(StorageError::Conflict(format!(
            "attached compatibility workflow cannot settle {previous_state:?} -> {next_state:?}"
        )));
    }
    attempt.state = next_state;
    attempt.revision = increment(attempt.revision, "workflow revision")?;
    if next_state != WorkflowState::Verifying {
        attempt.lease_owner = None;
        attempt.lease_expires_at = None;
        attempt.heartbeat_at = None;
    }
    attempt.updated_at = now;
    update_attempt_cas(transaction, attempt, expected_revision, expected_generation)?;
    let approval_wait = if settlement == AgentTurnWorkflowSettlement::WaitingForApproval {
        Some(load_approval_wait_binding_for_lease(
            transaction,
            lease_id,
            now,
        )?)
    } else {
        None
    };
    let (sequence, cause) = next_event_position(transaction, attempt.workflow_id)?;
    let event = workflow_event(
        attempt,
        sequence,
        CorrelationId::new(),
        cause,
        &format!("settle-agent-lease:{lease_id}:{label}:{expected_revision}:{expected_generation}"),
        if approval_wait.is_some() {
            WORKFLOW_WAIT_BOUND_TYPE_ID
        } else {
            WORKFLOW_TRANSITIONED_TYPE_ID
        },
        now,
        &serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "from": previous_state,
            "to": next_state,
            "cancellation_effects": null,
            "compatibility_agent_lease_id": lease_id,
            "settlement": label,
            "wait_reason": approval_wait.as_ref().map(|_| WaitReason::Approval),
            "approval_wait": approval_wait,
            "attempt": attempt,
        }),
    )?;
    insert_event(transaction, &event)?;
    Ok(Some(attempt.clone()))
}

/// Applies one ordinary workflow transition inside an existing Root-owned transaction.
/// Callers use this when another immutable authority record and the workflow settlement must
/// become visible together or not at all.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn transition_workflow_attempt_in_transaction(
    transaction: &Transaction<'_>,
    attempt_id: AttemptId,
    expected_revision: u64,
    expected_generation: u64,
    next_state: WorkflowState,
    correlation_id: CorrelationId,
    idempotency_key: &str,
    now: DateTime<Utc>,
) -> Result<WorkflowAttempt, StorageError> {
    if idempotency_key.is_empty() {
        return Err(StorageError::InvalidData(
            "workflow transition idempotency key cannot be empty".into(),
        ));
    }
    let mut attempt = load_attempt(transaction, attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
    if let Some(event) =
        load_event_by_idempotency(transaction, attempt.workflow_id, idempotency_key)?
    {
        let payload = event_payload_value(&event)?;
        if event.type_id.as_str() != WORKFLOW_TRANSITIONED_TYPE_ID
            || payload.get("attempt_id") != Some(&serde_json::json!(attempt.attempt_id))
            || payload.get("to") != Some(&serde_json::to_value(next_state)?)
        {
            return Err(StorageError::Conflict(
                "workflow transition idempotency key was replayed with different content".into(),
            ));
        }
        return event_attempt(&event);
    }
    ensure_expected(&attempt, expected_revision, expected_generation)?;
    if next_state == WorkflowState::Running || !attempt.state.can_transition_to(next_state) {
        return Err(StorageError::Conflict(format!(
            "illegal workflow transition {:?} -> {next_state:?}",
            attempt.state
        )));
    }
    if matches!(
        next_state,
        WorkflowState::Waiting(WaitReason::External | WaitReason::ProtectedComponent)
    ) {
        return Err(StorageError::Conflict(
            "external and protected-component waits require an immutable wake binding".into(),
        ));
    }

    let previous_state = attempt.state;
    let cancellation_model_runs = if next_state == WorkflowState::CancelRequested {
        Some(propagate_workflow_cancellation_to_model_runs(
            transaction,
            attempt.attempt_id,
            now,
        )?)
    } else {
        None
    };
    let cancellation_effects = if next_state == WorkflowState::CancelRequested {
        Some(fence_actions_for_workflow_cancellation(
            transaction,
            attempt.attempt_id,
            correlation_id,
            now,
        )?)
    } else if previous_state == WorkflowState::CancelRequested {
        let status = cancellation_effect_status(transaction, attempt.attempt_id)?;
        if next_state == WorkflowState::Cancelled && status.unresolved > 0 {
            return Err(StorageError::Conflict(
                "workflow cancellation cannot settle while an action effect is unresolved".into(),
            ));
        }
        if next_state == WorkflowState::Waiting(WaitReason::Reconciliation)
            && (status.reconciliation_required == 0 || status.quarantined > 0)
        {
            return Err(StorageError::Conflict(
                "workflow may wait for reconciliation only for unresolved reconcilable effects"
                    .into(),
            ));
        }
        Some(status)
    } else if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation)
        && next_state == WorkflowState::Cancelled
    {
        let status = cancellation_effect_status(transaction, attempt.attempt_id)?;
        if !is_cancellation_reconciliation_wait(transaction, &attempt)? || status.unresolved > 0 {
            return Err(StorageError::Conflict(
                "only a fully reconciled cancellation wait can settle as cancelled".into(),
            ));
        }
        Some(status)
    } else {
        None
    };
    attempt.state = next_state;
    attempt.revision = increment(attempt.revision, "workflow revision")?;
    if next_state == WorkflowState::CancelRequested {
        attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
    }
    if next_state != WorkflowState::Verifying {
        attempt.lease_owner = None;
        attempt.lease_expires_at = None;
        attempt.heartbeat_at = None;
    }
    attempt.updated_at = now;
    update_attempt_cas(
        transaction,
        &attempt,
        expected_revision,
        expected_generation,
    )?;

    let (sequence, cause) = next_event_position(transaction, attempt.workflow_id)?;
    let event = workflow_event(
        &attempt,
        sequence,
        correlation_id,
        cause,
        idempotency_key,
        WORKFLOW_TRANSITIONED_TYPE_ID,
        now,
        &serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "from": previous_state,
            "to": next_state,
            "cancellation_model_runs": cancellation_model_runs,
            "cancellation_effects": cancellation_effects,
            "attempt": attempt,
        }),
    )?;
    insert_event(transaction, &event)?;
    Ok(attempt)
}

/// Settles a cancellation-owned reconciliation wait immediately after the final effect is
/// resolved. This runs in the same transaction as the authoritative reconciliation result so a
/// crash cannot leave a fully reconciled cancellation dependent on a later startup scan.
pub(super) fn settle_reconciled_workflow_cancellation_in_transaction(
    transaction: &Transaction<'_>,
    attempt_id: AttemptId,
    reconciliation_job_id: mcg_contracts::ReconciliationJobId,
    correlation_id: CorrelationId,
    now: DateTime<Utc>,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    let attempt = load_attempt(transaction, attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
    let cancellation_owned = attempt.state == WorkflowState::CancelRequested
        || (attempt.state == WorkflowState::Waiting(WaitReason::Reconciliation)
            && is_cancellation_reconciliation_wait(transaction, &attempt)?);
    if !cancellation_owned {
        return Ok(None);
    }

    let status = cancellation_effect_status(transaction, attempt.attempt_id)?;
    let next_state = if status.quarantined > 0 {
        Some(WorkflowState::Quarantined)
    } else if status.unresolved == 0 {
        Some(WorkflowState::Cancelled)
    } else if attempt.state == WorkflowState::CancelRequested && status.reconciliation_required > 0
    {
        Some(WorkflowState::Waiting(WaitReason::Reconciliation))
    } else {
        None
    };
    let Some(next_state) = next_state else {
        return Ok(None);
    };

    transition_workflow_attempt_in_transaction(
        transaction,
        attempt.attempt_id,
        attempt.revision,
        attempt.lease_generation,
        next_state,
        correlation_id,
        &format!("settle-cancellation-reconciliation:{reconciliation_job_id}"),
        now,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn claim_workflow_attempt_in_transaction(
    transaction: &Transaction<'_>,
    attempt_id: AttemptId,
    expected_revision: u64,
    expected_generation: u64,
    lease_owner: RuntimeActorId,
    lease_expires_at: DateTime<Utc>,
    correlation_id: CorrelationId,
    idempotency_key: &str,
    now: DateTime<Utc>,
) -> Result<WorkflowAttempt, StorageError> {
    if idempotency_key.is_empty() || lease_expires_at <= now {
        return Err(StorageError::InvalidData(
            "workflow claim requires a nonempty key and future lease expiry".into(),
        ));
    }
    let mut attempt = load_attempt(transaction, attempt_id)?
        .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
    if let Some(event) =
        load_event_by_idempotency(transaction, attempt.workflow_id, idempotency_key)?
    {
        let original = event_attempt(&event)?;
        let expected = serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "lease_owner": lease_owner,
            "lease_generation": event.lease_generation,
            "lease_expires_at": lease_expires_at,
            "attempt": original,
        });
        if event.type_id.as_str() != WORKFLOW_CLAIMED_TYPE_ID
            || event_payload_value(&event)? != expected
        {
            return Err(StorageError::Conflict(
                "workflow claim idempotency key was replayed with different content".into(),
            ));
        }
        return Ok(original);
    }
    ensure_expected(&attempt, expected_revision, expected_generation)?;
    if attempt.state != WorkflowState::Ready {
        return Err(StorageError::Conflict(format!(
            "only a Ready workflow can be claimed; current state is {:?}",
            attempt.state
        )));
    }
    if !dependencies_satisfied(transaction, &attempt)? {
        return Err(StorageError::Conflict(
            "workflow task-contract dependencies are not yet satisfied".into(),
        ));
    }

    attempt.state = WorkflowState::Running;
    attempt.revision = increment(attempt.revision, "workflow revision")?;
    attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
    attempt.lease_owner = Some(lease_owner);
    attempt.lease_expires_at = Some(lease_expires_at);
    attempt.heartbeat_at = Some(now);
    attempt.updated_at = now;
    update_attempt_cas(
        transaction,
        &attempt,
        expected_revision,
        expected_generation,
    )?;

    let (sequence, cause) = next_event_position(transaction, attempt.workflow_id)?;
    let event = workflow_event(
        &attempt,
        sequence,
        correlation_id,
        cause,
        idempotency_key,
        WORKFLOW_CLAIMED_TYPE_ID,
        now,
        &serde_json::json!({
            "attempt_id": attempt.attempt_id,
            "lease_owner": lease_owner,
            "lease_generation": attempt.lease_generation,
            "lease_expires_at": lease_expires_at,
            "attempt": attempt,
        }),
    )?;
    insert_event(transaction, &event)?;
    Ok(attempt)
}

impl SqliteEventStore {
    /// Loads the immutable canonical attachment for one exact pre-canonical source identity.
    ///
    /// # Errors
    /// Returns a storage error when stored evidence is malformed or column bindings diverge.
    pub fn workflow_source_attachment(
        &self,
        source_type: &StableTypeId,
        source_locator: &str,
    ) -> Result<Option<WorkflowSourceAttachment>, StorageError> {
        load_workflow_source_attachment_by_source(
            &self.connection,
            source_type.as_str(),
            source_locator,
        )
    }

    /// Loads the immutable source attachment for one canonical workflow.
    ///
    /// # Errors
    /// Returns a storage error when stored evidence is malformed or column bindings diverge.
    pub fn workflow_source_attachment_for_workflow(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<Option<WorkflowSourceAttachment>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT source_type, source_locator, source_hash_json, workflow_id, attempt_id,
                        task_contract_id, task_contract_version, task_contract_hash_json,
                        attachment_json, attached_at
                 FROM workflow_source_attachments WHERE workflow_id = ?1",
                [workflow_id.to_string()],
                workflow_source_attachment_row,
            )
            .optional()?;
        row.map(|row| validate_loaded_workflow_source_attachment(&self.connection, row))
            .transpose()
    }

    /// Loads every canonical workflow linked to one broader compatibility source.
    ///
    /// One task, run, or conversation may return multiple child workflows. Results are bounded by
    /// the durable number of links and ordered deterministically by attachment time and workflow.
    ///
    /// # Errors
    /// Returns a storage error when stored evidence or its primary attachment is inconsistent.
    pub fn workflow_source_links(
        &self,
        source_type: &StableTypeId,
        source_locator: &str,
    ) -> Result<Vec<WorkflowSourceLink>, StorageError> {
        load_workflow_source_links_by_source(&self.connection, source_type.as_str(), source_locator)
    }

    /// Reconstructs missing compatibility lookup links for exact active primary attachments.
    ///
    /// This is a startup migration/recovery path for pre-schema-62 workflows. It revalidates the
    /// entire still-active task/run/instance/lease chain and the immutable primary attachment
    /// before inserting links. Inactive or expired compatibility work is deliberately not guessed.
    ///
    /// # Errors
    /// Returns a storage error for malformed or substituted active authority, invalid identifiers,
    /// or atomic insertion failure.
    pub fn recover_compatibility_workflow_source_links(
        &mut self,
        now: DateTime<Utc>,
    ) -> Result<WorkflowSourceLinkRecoveryReport, StorageError> {
        let transaction = self.connection.transaction()?;
        let source_type = stable_type_id("mcg.agent_turn_source.v1")?;
        let locators = {
            let mut statement = transaction.prepare(
                "SELECT attachment.source_locator, lease.expires_at
                 FROM workflow_source_attachments AS attachment
                 INNER JOIN agent_leases AS lease
                    ON lease.lease_id = attachment.source_locator
                 INNER JOIN agent_instances AS instance
                    ON instance.instance_id = lease.instance_id
                 INNER JOIN agent_runs AS run ON run.run_id = instance.run_id
                 INNER JOIN tasks AS task ON task.task_id = run.task_id
                 WHERE attachment.source_type = ?1
                   AND lease.state = 'active'
                   AND instance.state = 'running'
                   AND run.state = 'running'
                   AND task.state = 'active'
                   AND NOT EXISTS (
                       SELECT 1 FROM workflow_source_links AS link
                       WHERE link.workflow_id = attachment.workflow_id
                   )
                 ORDER BY attachment.source_locator",
            )?;
            statement
                .query_map([source_type.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let locators = locators
            .into_iter()
            .map(|(locator, expiry)| {
                let expiry = DateTime::parse_from_rfc3339(&expiry)
                    .map_err(|error| {
                        StorageError::InvalidData(format!(
                            "compatibility lease expiry is invalid during source recovery: {error}"
                        ))
                    })?
                    .with_timezone(&Utc);
                Ok((locator, expiry))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let locators = locators
            .into_iter()
            .filter_map(|(locator, expiry)| (expiry > now).then_some(locator))
            .collect::<Vec<_>>();
        let mut report = WorkflowSourceLinkRecoveryReport {
            inspected_active_attachments: u64::try_from(locators.len()).map_err(|_| {
                StorageError::InvalidData("workflow source recovery count overflowed".into())
            })?,
            reattached_workflows: 0,
            inserted_links: 0,
        };
        for locator in locators {
            let lease_id = locator.parse::<AgentLeaseId>().map_err(|error| {
                StorageError::InvalidData(format!(
                    "workflow source attachment lease locator is invalid: {error}"
                ))
            })?;
            let attachment = load_workflow_source_attachment_by_source(
                &transaction,
                source_type.as_str(),
                &locator,
            )?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "workflow source recovery attachment disappeared during transaction".into(),
                )
            })?;
            let validated = load_agent_turn_workflow_attachment(&transaction, attachment)?;
            if !validated.source_links.is_empty() {
                continue;
            }
            let source = load_active_agent_turn_source(&transaction, lease_id, now)?;
            let links = derive_agent_turn_source_links(&source, &validated.attachment, now)?;
            for link in &links {
                insert_or_replay_workflow_source_link(&transaction, link)?;
            }
            report.reattached_workflows = increment(
                report.reattached_workflows,
                "recovered workflow source attachment count",
            )?;
            report.inserted_links = report
                .inserted_links
                .checked_add(u64::try_from(links.len()).map_err(|_| {
                    StorageError::InvalidData(
                        "recovered workflow source link count overflowed".into(),
                    )
                })?)
                .ok_or_else(|| {
                    StorageError::InvalidData(
                        "recovered workflow source link count overflowed".into(),
                    )
                })?;
        }
        transaction.commit()?;
        Ok(report)
    }

    /// Loads one authoritative task-contract version.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn task_contract_version(
        &self,
        id: TaskContractId,
        version: u64,
    ) -> Result<Option<TaskContract>, StorageError> {
        self.connection
            .query_row(
                "SELECT contract_json FROM task_contract_versions
                 WHERE task_contract_id = ?1 AND version = ?2",
                params![id.to_string(), sql_u64(version, "task contract version")?],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Loads the immutable contract version selected by the authoritative head.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn task_contract_head(
        &self,
        id: TaskContractId,
    ) -> Result<Option<TaskContractHead>, StorageError> {
        self.connection
            .query_row(
                "SELECT version.contract_json, head.revision
                 FROM task_contract_heads AS head
                 JOIN task_contract_versions AS version
                   ON version.task_contract_id = head.task_contract_id
                  AND version.version = head.current_version
                 WHERE head.task_contract_id = ?1",
                [id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .map(|(json, revision)| {
                Ok(TaskContractHead {
                    contract: serde_json::from_str(&json)?,
                    revision: u64::try_from(revision).map_err(|_| {
                        StorageError::InvalidData("negative task-contract head revision".into())
                    })?,
                })
            })
            .transpose()
    }

    /// Appends one immutable material contract revision and advances its head with CAS.
    ///
    /// Exact replay returns the original resulting head. A reused key, stale head, skipped
    /// version, or incorrect previous-version hash fails closed.
    ///
    /// # Errors
    /// Returns a storage error for invalid content, conflicting replay/CAS, or transaction failure.
    pub fn revise_task_contract(
        &mut self,
        contract: &TaskContract,
        expected_head_revision: u64,
        idempotency_key: &str,
    ) -> Result<TaskContractHead, StorageError> {
        if idempotency_key.trim().is_empty() || expected_head_revision == 0 {
            return Err(StorageError::InvalidData(
                "task-contract revision requires a nonempty key and positive head revision".into(),
            ));
        }
        validate_contract(contract)?;
        let transaction = self.connection.transaction()?;
        if let Some((json, expected, resulting)) = transaction
            .query_row(
                "SELECT contract_json, expected_head_revision, resulting_head_revision
                 FROM task_contract_mutations
                 WHERE task_contract_id = ?1 AND idempotency_key = ?2",
                params![contract.id.to_string(), idempotency_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
        {
            let original: TaskContract = serde_json::from_str(&json)?;
            if original != *contract
                || expected != sql_u64(expected_head_revision, "expected head revision")?
            {
                return Err(StorageError::Conflict(
                    "task-contract idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(TaskContractHead {
                contract: original,
                revision: u64::try_from(resulting).map_err(|_| {
                    StorageError::InvalidData("negative task-contract head revision".into())
                })?,
            });
        }

        let (current_version, current_revision) = transaction
            .query_row(
                "SELECT current_version, revision FROM task_contract_heads
                 WHERE task_contract_id = ?1",
                [contract.id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StorageError::Conflict("task-contract head does not exist".into()))?;
        let current_version = u64::try_from(current_version)
            .map_err(|_| StorageError::InvalidData("negative task-contract version".into()))?;
        let current_revision = u64::try_from(current_revision)
            .map_err(|_| StorageError::InvalidData("negative task-contract revision".into()))?;
        if current_revision != expected_head_revision
            || contract.version != current_version.saturating_add(1)
        {
            return Err(StorageError::Conflict(format!(
                "stale task-contract head: expected revision/version {expected_head_revision}/{}, current {current_revision}/{current_version}",
                contract.version
            )));
        }
        let prior =
            load_task_contract(&transaction, contract.id, current_version)?.ok_or_else(|| {
                StorageError::InvalidData("task-contract head target is missing".into())
            })?;
        let expected_previous = VersionedRef {
            id: prior.id,
            version: prior.version,
            content_hash: task_contract_content_hash(&prior)?,
        };
        if contract.previous_version.as_ref() != Some(&expected_previous) {
            return Err(StorageError::Conflict(
                "task-contract revision does not bind the exact current head".into(),
            ));
        }
        insert_contract_version(&transaction, contract)?;
        let resulting_revision = increment(current_revision, "task-contract head revision")?;
        let changed = transaction.execute(
            "UPDATE task_contract_heads SET current_version = ?1, revision = ?2
             WHERE task_contract_id = ?3 AND current_version = ?4 AND revision = ?5",
            params![
                sql_u64(contract.version, "task contract version")?,
                sql_u64(resulting_revision, "task contract head revision")?,
                contract.id.to_string(),
                sql_u64(current_version, "current task contract version")?,
                sql_u64(current_revision, "current task contract head revision")?,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "task-contract head changed before commit".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO task_contract_mutations(
                task_contract_id, idempotency_key, contract_version, expected_head_revision,
                resulting_head_revision, contract_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                contract.id.to_string(),
                idempotency_key,
                sql_u64(contract.version, "task contract version")?,
                sql_u64(expected_head_revision, "expected head revision")?,
                sql_u64(resulting_revision, "resulting head revision")?,
                serde_json::to_string(contract)?,
                contract.created_at.to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(TaskContractHead {
            contract: contract.clone(),
            revision: resulting_revision,
        })
    }

    /// Loads one authoritative workflow attempt.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempt(
        &self,
        attempt_id: AttemptId,
    ) -> Result<Option<WorkflowAttempt>, StorageError> {
        load_attempt(&self.connection, attempt_id)
    }

    /// Lists the latest authoritative attempt for each canonical workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempts(&self) -> Result<Vec<WorkflowAttempt>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT candidate.attempt_json
             FROM workflow_attempts AS candidate
             WHERE candidate.attempt_number = (
                 SELECT MAX(latest.attempt_number) FROM workflow_attempts AS latest
                 WHERE latest.workflow_id = candidate.workflow_id
             )
             ORDER BY candidate.updated_at DESC, candidate.workflow_id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Loads the latest authoritative attempt for one canonical workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_attempt_for_workflow(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<Option<WorkflowAttempt>, StorageError> {
        self.connection
            .query_row(
                "SELECT attempt_json FROM workflow_attempts
                 WHERE workflow_id = ?1 ORDER BY attempt_number DESC LIMIT 1",
                [workflow_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
            .transpose()
    }

    /// Returns the ordered event history for one workflow.
    ///
    /// # Errors
    /// Returns a storage error for query or stored-data failures.
    pub fn workflow_events(
        &self,
        workflow_id: WorkflowId,
    ) -> Result<Vec<WorkflowEvent>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM workflow_events WHERE workflow_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([workflow_id.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .collect()
    }

    /// Returns one bounded contiguous page after an exclusive workflow sequence cursor together
    /// with the latest durable sequence visible when the page was read.
    ///
    /// # Errors
    /// Returns a storage error for invalid bounds/cursors, query failures, or malformed evidence.
    pub fn workflow_events_after(
        &self,
        workflow_id: WorkflowId,
        after_sequence: u64,
        limit: usize,
    ) -> Result<(Vec<WorkflowEvent>, u64), StorageError> {
        if limit == 0 || limit > MAX_WORKFLOW_EVENT_PAGE_SIZE {
            return Err(StorageError::InvalidData(
                "workflow event page limit is invalid".into(),
            ));
        }
        let sql_after = sql_u64(after_sequence, "workflow event cursor")?;
        let sql_limit = i64::try_from(limit).map_err(|_| {
            StorageError::InvalidData("workflow event page limit overflowed".into())
        })?;
        let mut statement = self.connection.prepare(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 AND sequence > ?2
             ORDER BY sequence LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![workflow_id.to_string(), sql_after, sql_limit],
            |row| row.get::<_, String>(0),
        )?;
        let events = rows
            .map(|row| {
                let json = row?;
                serde_json::from_str(&json).map_err(StorageError::from)
            })
            .collect::<Result<Vec<WorkflowEvent>, StorageError>>()?;
        drop(statement);
        let latest = self
            .connection
            .query_row(
                "SELECT MAX(sequence) FROM workflow_events WHERE workflow_id = ?1",
                [workflow_id.to_string()],
                |row| row.get::<_, Option<i64>>(0),
            )?
            .unwrap_or(0);
        let latest_sequence = u64::try_from(latest)
            .map_err(|_| StorageError::InvalidData("negative workflow event sequence".into()))?;
        if after_sequence > latest_sequence {
            return Err(StorageError::Conflict(
                "workflow event cursor is ahead of authoritative history".into(),
            ));
        }
        Ok((events, latest_sequence))
    }

    /// Atomically claims a Ready attempt and increments both revision and lease generation.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, invalid state, conflicting replay, or expiry.
    #[allow(clippy::too_many_arguments)]
    pub fn claim_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        lease_owner: RuntimeActorId,
        lease_expires_at: DateTime<Utc>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        let transaction = self.connection.transaction()?;
        let attempt = claim_workflow_attempt_in_transaction(
            &transaction,
            attempt_id,
            expected_revision,
            expected_generation,
            lease_owner,
            lease_expires_at,
            correlation_id,
            idempotency_key,
            now,
        )?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Extends the current lease without changing its owner, scope, or generation.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, wrong ownership, expired leases, shortened
    /// expiry, or idempotency substitution.
    #[allow(clippy::too_many_arguments)]
    pub fn heartbeat_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        lease_owner: RuntimeActorId,
        lease_expires_at: DateTime<Utc>,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() || lease_expires_at <= now {
            return Err(StorageError::InvalidData(
                "workflow heartbeat requires a nonempty key and future expiry".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, idempotency_key)?
        {
            let original = event_attempt(&event)?;
            let expected = serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": event.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": original,
            });
            if event.type_id.as_str() != WORKFLOW_HEARTBEAT_TYPE_ID
                || event_payload_value(&event)? != expected
            {
                return Err(StorageError::Conflict(
                    "workflow heartbeat idempotency key was replayed with different content".into(),
                ));
            }
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&attempt, expected_revision, expected_generation)?;
        if !matches!(
            attempt.state,
            WorkflowState::Running | WorkflowState::Verifying
        ) || attempt.lease_owner != Some(lease_owner)
            || attempt
                .lease_expires_at
                .is_none_or(|current| current <= now)
            || attempt
                .lease_expires_at
                .is_some_and(|current| lease_expires_at < current)
        {
            return Err(StorageError::Conflict(
                "heartbeat cannot revive, shorten, or transfer a workflow lease".into(),
            ));
        }

        attempt.revision = increment(attempt.revision, "workflow revision")?;
        attempt.lease_expires_at = Some(lease_expires_at);
        attempt.heartbeat_at = Some(now);
        attempt.updated_at = now;
        update_attempt_cas(
            &transaction,
            &attempt,
            expected_revision,
            expected_generation,
        )?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_HEARTBEAT_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "lease_owner": lease_owner,
                "lease_generation": attempt.lease_generation,
                "lease_expires_at": lease_expires_at,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Applies one legal Root-owned transition using revision and generation compare-and-swap.
    ///
    /// Transitioning to Running is reserved for [`Self::claim_workflow_attempt`]. Entering
    /// `CancelRequested` fences the current worker generation before the event is committed.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, illegal transitions, or replay substitution.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn transition_workflow_attempt(
        &mut self,
        attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        next_state: WorkflowState,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        let transaction = self.connection.transaction()?;
        let attempt = transition_workflow_attempt_in_transaction(
            &transaction,
            attempt_id,
            expected_revision,
            expected_generation,
            next_state,
            correlation_id,
            idempotency_key,
            now,
        )?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Atomically enters an external or protected-component wait with exact notifier authority.
    ///
    /// # Errors
    /// Returns a conflict for stale worker authority, unsupported wait reasons, invalid evidence,
    /// expired bounds, or replay substitution.
    #[allow(clippy::too_many_lines)]
    pub fn bind_workflow_wait(
        &mut self,
        request: &WorkflowWaitRequest,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<(WorkflowAttempt, WorkflowWakeBinding), StorageError> {
        if request.idempotency_key.trim().is_empty()
            || !matches!(
                request.wait_reason,
                WaitReason::External | WaitReason::ProtectedComponent
            )
            || request.expires_at <= now
        {
            return Err(StorageError::InvalidData(
                "workflow wait request has invalid reason, expiry, or idempotency key".into(),
            ));
        }
        request
            .condition
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, request.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(request.attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, &request.idempotency_key)?
        {
            let original = event_attempt(&event)?;
            let binding = workflow_wake_binding_from_event(&event)?;
            let payload = event_payload_value(&event)?;
            if event.type_id.as_str() != WORKFLOW_WAIT_BOUND_TYPE_ID
                || payload.get("expected_revision")
                    != Some(&serde_json::json!(request.expected_revision))
                || payload.get("expected_generation")
                    != Some(&serde_json::json!(request.expected_generation))
                || binding.attempt_id != request.attempt_id
                || binding.wait_reason != request.wait_reason
                || binding.condition != request.condition
                || binding.authorized_notifier != request.authorized_notifier
                || binding.expires_at != request.expires_at
            {
                return Err(StorageError::Conflict(
                    "workflow wait idempotency key was replayed with different authority".into(),
                ));
            }
            transaction.commit()?;
            return Ok((original, binding));
        }
        ensure_expected(
            &attempt,
            request.expected_revision,
            request.expected_generation,
        )?;
        if attempt.state != WorkflowState::Running
            || attempt.lease_owner.is_none()
            || attempt.lease_expires_at.is_none_or(|expiry| expiry <= now)
        {
            return Err(StorageError::Conflict(
                "only a live Running workflow authority may bind a wake condition".into(),
            ));
        }

        let previous_state = attempt.state;
        attempt.state = WorkflowState::Waiting(request.wait_reason);
        attempt.revision = increment(attempt.revision, "workflow revision")?;
        attempt.lease_owner = None;
        attempt.lease_expires_at = None;
        attempt.heartbeat_at = None;
        attempt.updated_at = now;
        let binding = WorkflowWakeBinding {
            schema_version: WorkflowWakeBinding::SCHEMA_VERSION,
            attempt_id: attempt.attempt_id,
            wait_reason: request.wait_reason,
            condition: request.condition.clone(),
            authorized_notifier: request.authorized_notifier,
            bound_revision: attempt.revision,
            bound_generation: attempt.lease_generation,
            bound_at: now,
            expires_at: request.expires_at,
        };
        binding
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        update_attempt_cas(
            &transaction,
            &attempt,
            request.expected_revision,
            request.expected_generation,
        )?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            &request.idempotency_key,
            WORKFLOW_WAIT_BOUND_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "from": previous_state,
                "to": attempt.state,
                "wait_reason": request.wait_reason,
                "expected_revision": request.expected_revision,
                "expected_generation": request.expected_generation,
                "workflow_wake": binding,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok((attempt, binding))
    }

    /// Atomically wakes one exact external or protected-component wait from immutable evidence.
    ///
    /// # Errors
    /// Returns a conflict for stale/substituted/expired authority, the wrong notifier, unmatched
    /// evidence, or replay substitution.
    #[allow(clippy::too_many_lines)]
    pub fn wake_workflow(
        &mut self,
        request: &WorkflowWakeRequest,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if request.idempotency_key.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "workflow wake idempotency key cannot be empty".into(),
            ));
        }
        request
            .binding_hash
            .validate()
            .and_then(|()| request.evidence.validate())
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let transaction = self.connection.transaction()?;
        let mut attempt = load_attempt(&transaction, request.attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(request.attempt_id))?;
        if let Some(event) =
            load_event_by_idempotency(&transaction, attempt.workflow_id, &request.idempotency_key)?
        {
            let original = event_attempt(&event)?;
            let payload = event_payload_value(&event)?;
            if event.type_id.as_str() != WORKFLOW_WAIT_SATISFIED_TYPE_ID
                || payload.get("expected_revision")
                    != Some(&serde_json::json!(request.expected_revision))
                || payload.get("expected_generation")
                    != Some(&serde_json::json!(request.expected_generation))
                || payload.get("binding_hash")
                    != Some(&serde_json::to_value(&request.binding_hash)?)
                || payload.get("notifier") != Some(&serde_json::json!(request.notifier))
                || payload.get("evidence") != Some(&serde_json::to_value(&request.evidence)?)
            {
                return Err(StorageError::Conflict(
                    "workflow wake idempotency key was replayed with different evidence".into(),
                ));
            }
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(
            &attempt,
            request.expected_revision,
            request.expected_generation,
        )?;
        let binding = load_current_workflow_wake_binding(&transaction, &attempt)?;
        if binding.attempt_id != attempt.attempt_id
            || binding.bound_revision != attempt.revision
            || binding.bound_generation != attempt.lease_generation
            || request.binding_hash
                != binding
                    .content_hash()
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?
            || request.notifier != binding.authorized_notifier
            || request.evidence != binding.condition
            || now > binding.expires_at
        {
            return Err(StorageError::Conflict(
                "workflow wake does not match the current immutable condition authority".into(),
            ));
        }

        let previous_state = attempt.state;
        attempt.state = WorkflowState::Ready;
        attempt.revision = increment(attempt.revision, "workflow revision")?;
        attempt.updated_at = now;
        update_attempt_cas(
            &transaction,
            &attempt,
            request.expected_revision,
            request.expected_generation,
        )?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            &request.idempotency_key,
            WORKFLOW_WAIT_SATISFIED_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "from": previous_state,
                "to": attempt.state,
                "wait_reason": binding.wait_reason,
                "expected_revision": request.expected_revision,
                "expected_generation": request.expected_generation,
                "binding_hash": request.binding_hash,
                "notifier": request.notifier,
                "evidence": request.evidence,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Creates a new Ready attempt instead of rewinding a terminal failed/cancelled attempt.
    ///
    /// # Errors
    /// Returns a conflict for stale expectations, ineligible state, exhausted retry policy, or
    /// idempotency substitution.
    #[allow(clippy::too_many_arguments)]
    pub fn retry_workflow_attempt(
        &mut self,
        prior_attempt_id: AttemptId,
        expected_revision: u64,
        expected_generation: u64,
        correlation_id: CorrelationId,
        idempotency_key: &str,
        now: DateTime<Utc>,
    ) -> Result<WorkflowAttempt, StorageError> {
        if idempotency_key.is_empty() {
            return Err(StorageError::InvalidData(
                "workflow retry idempotency key cannot be empty".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        let prior = load_attempt(&transaction, prior_attempt_id)?
            .ok_or(StorageError::WorkflowAttemptNotFound(prior_attempt_id))?;
        if let Some(existing) =
            load_attempt_by_idempotency(&transaction, prior.workflow_id, idempotency_key)?
        {
            let event =
                load_event_by_idempotency(&transaction, prior.workflow_id, idempotency_key)?
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "retry attempt exists without its atomic event".into(),
                        )
                    })?;
            if existing.parent_attempt_id != Some(prior_attempt_id)
                || event.type_id.as_str() != WORKFLOW_RETRIED_TYPE_ID
            {
                return Err(StorageError::Conflict(
                    "workflow retry idempotency key was replayed with different ancestry".into(),
                ));
            }
            let original = event_attempt(&event)?;
            transaction.commit()?;
            return Ok(original);
        }
        ensure_expected(&prior, expected_revision, expected_generation)?;
        if !matches!(
            prior.state,
            WorkflowState::Failed | WorkflowState::Cancelled
        ) || prior.attempt_number >= prior.retry_policy.max_attempts
        {
            return Err(StorageError::Conflict(
                "only failed or cancelled attempts within the retry ceiling may retry".into(),
            ));
        }
        let attempt = WorkflowAttempt {
            workflow_id: prior.workflow_id,
            attempt_id: AttemptId::new(),
            task_contract: prior.task_contract.clone(),
            parent_attempt_id: Some(prior.attempt_id),
            attempt_number: prior.attempt_number.checked_add(1).ok_or_else(|| {
                StorageError::InvalidData("workflow attempt number exhausted".into())
            })?,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: prior.environment_snapshot.clone(),
            configuration: prior.configuration.clone(),
            idempotency_key: idempotency_key.to_owned(),
            retry_policy: prior.retry_policy.clone(),
            compensation_policy: prior.compensation_policy.clone(),
            created_at: now,
            updated_at: now,
        };
        insert_attempt(&transaction, &attempt)?;
        let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
        let event = workflow_event(
            &attempt,
            sequence,
            correlation_id,
            cause,
            idempotency_key,
            WORKFLOW_RETRIED_TYPE_ID,
            now,
            &serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "parent_attempt_id": prior.attempt_id,
                "attempt_number": attempt.attempt_number,
                "state": attempt.state,
                "attempt": attempt,
            }),
        )?;
        insert_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(attempt)
    }

    /// Recovers workflow leases and objectively satisfied wait conditions during Root startup.
    ///
    /// Running work moves to `Waiting(Reconciliation)`; expired verification work can safely
    /// return to Ready. Dependency and retry-backoff waits resume only when their persisted
    /// condition is satisfied. Other waits, cancellation, and terminal work are not guessed.
    ///
    /// # Errors
    /// Returns a storage error for query, malformed stored data, or atomic update failures.
    #[allow(clippy::too_many_lines)]
    pub fn recover_expired_workflow_leases(
        &mut self,
        correlation_id: CorrelationId,
        now: DateTime<Utc>,
    ) -> Result<WorkflowRecoveryReport, StorageError> {
        let attempts = load_nonterminal_attempts(&self.connection)?;
        let mut report = WorkflowRecoveryReport {
            inspected_nonterminal: u64::try_from(attempts.len()).map_err(|_| {
                StorageError::InvalidData("workflow count exceeds supported range".into())
            })?,
            fenced_expired: 0,
            awaiting_reconciliation: 0,
            requeued_for_verification: 0,
            requeued_satisfied_dependencies: 0,
            requeued_elapsed_backoffs: 0,
            settled_cancellations: 0,
            cancellations_awaiting_reconciliation: 0,
            quarantined_cancellations: 0,
        };
        for mut attempt in attempts {
            let expired = matches!(
                attempt.state,
                WorkflowState::Running | WorkflowState::Verifying
            ) && attempt.lease_expires_at.is_some_and(|expiry| expiry <= now);
            let satisfied_dependency_wait = attempt.state
                == WorkflowState::Waiting(WaitReason::Dependency)
                && dependencies_satisfied(&self.connection, &attempt)?;
            let elapsed_retry_backoff = attempt.state
                == WorkflowState::Waiting(WaitReason::RetryBackoff)
                && retry_backoff_elapsed(&attempt, now);
            let is_cancellation_state = attempt.state == WorkflowState::CancelRequested
                || (attempt.state == WorkflowState::Waiting(WaitReason::Reconciliation)
                    && is_cancellation_reconciliation_wait(&self.connection, &attempt)?);
            let cancellation_effects = if is_cancellation_state {
                Some(cancellation_effect_status(
                    &self.connection,
                    attempt.attempt_id,
                )?)
            } else {
                None
            };
            let recovered_cancellation_state = cancellation_effects.and_then(|status| {
                if status.quarantined > 0 {
                    Some(WorkflowState::Quarantined)
                } else if status.reconciliation_required > 0 {
                    Some(WorkflowState::Waiting(WaitReason::Reconciliation))
                } else if status.unresolved == 0 {
                    Some(WorkflowState::Cancelled)
                } else {
                    None
                }
            });
            if !expired
                && !satisfied_dependency_wait
                && !elapsed_retry_backoff
                && recovered_cancellation_state.is_none()
            {
                continue;
            }
            let expected_revision = attempt.revision;
            let expected_generation = attempt.lease_generation;
            let previous_state = attempt.state;
            attempt.state = recovered_cancellation_state.unwrap_or_else(|| match previous_state {
                WorkflowState::Running => WorkflowState::Waiting(WaitReason::Reconciliation),
                WorkflowState::Verifying => WorkflowState::Ready,
                WorkflowState::Waiting(WaitReason::Dependency | WaitReason::RetryBackoff) => {
                    WorkflowState::Ready
                }
                _ => unreachable!("recovery filters restrict recoverable states"),
            });
            attempt.revision = increment(attempt.revision, "workflow revision")?;
            if expired {
                attempt.lease_generation = increment(attempt.lease_generation, "lease generation")?;
            }
            attempt.lease_owner = None;
            attempt.lease_expires_at = None;
            attempt.heartbeat_at = None;
            attempt.updated_at = now;

            let transaction = self.connection.transaction()?;
            update_attempt_cas(
                &transaction,
                &attempt,
                expected_revision,
                expected_generation,
            )?;
            let (sequence, cause) = next_event_position(&transaction, attempt.workflow_id)?;
            let (idempotency_key, type_id, recovery_reason) = if expired {
                (
                    format!(
                        "recover-expired:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_LEASE_EXPIRED_TYPE_ID,
                    "lease_expired",
                )
            } else if satisfied_dependency_wait {
                (
                    format!(
                        "recover-dependency:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_WAIT_SATISFIED_TYPE_ID,
                    "dependencies_satisfied",
                )
            } else if elapsed_retry_backoff {
                (
                    format!(
                        "recover-backoff:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_WAIT_SATISFIED_TYPE_ID,
                    "retry_backoff_elapsed",
                )
            } else {
                (
                    format!(
                        "recover-cancellation:{}:{expected_revision}:{expected_generation}",
                        attempt.attempt_id
                    ),
                    WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID,
                    match attempt.state {
                        WorkflowState::Cancelled => "cancellation_settled",
                        WorkflowState::Waiting(WaitReason::Reconciliation) => {
                            "cancellation_requires_reconciliation"
                        }
                        WorkflowState::Quarantined => "cancellation_quarantined",
                        _ => unreachable!("cancellation recovery target"),
                    },
                )
            };
            let event = workflow_event(
                &attempt,
                sequence,
                correlation_id,
                cause,
                &idempotency_key,
                type_id,
                now,
                &serde_json::json!({
                    "attempt_id": attempt.attempt_id,
                    "from": previous_state,
                    "to": attempt.state,
                    "fenced_generation": expected_generation,
                    "active_generation": attempt.lease_generation,
                    "recovery_reason": recovery_reason,
                    "cancellation_effects": cancellation_effects,
                    "attempt": attempt,
                }),
            )?;
            insert_event(&transaction, &event)?;
            transaction.commit()?;
            if expired {
                report.fenced_expired += 1;
            }
            match attempt.state {
                WorkflowState::Waiting(WaitReason::Reconciliation)
                    if previous_state == WorkflowState::CancelRequested =>
                {
                    report.cancellations_awaiting_reconciliation += 1;
                }
                WorkflowState::Waiting(WaitReason::Reconciliation) => {
                    report.awaiting_reconciliation += 1;
                }
                WorkflowState::Ready if previous_state == WorkflowState::Verifying => {
                    report.requeued_for_verification += 1;
                }
                WorkflowState::Ready if satisfied_dependency_wait => {
                    report.requeued_satisfied_dependencies += 1;
                }
                WorkflowState::Ready if elapsed_retry_backoff => {
                    report.requeued_elapsed_backoffs += 1;
                }
                WorkflowState::Cancelled if previous_state == WorkflowState::CancelRequested => {
                    report.settled_cancellations += 1;
                }
                WorkflowState::Quarantined if previous_state == WorkflowState::CancelRequested => {
                    report.quarantined_cancellations += 1;
                }
                WorkflowState::Cancelled
                    if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation) =>
                {
                    report.settled_cancellations += 1;
                }
                WorkflowState::Quarantined
                    if previous_state == WorkflowState::Waiting(WaitReason::Reconciliation) =>
                {
                    report.quarantined_cancellations += 1;
                }
                _ => {}
            }
        }
        Ok(report)
    }
}

fn workflow_wake_binding_from_event(
    event: &WorkflowEvent,
) -> Result<WorkflowWakeBinding, StorageError> {
    let payload = event_payload_value(event)?;
    let binding: WorkflowWakeBinding =
        serde_json::from_value(payload.get("workflow_wake").cloned().ok_or_else(|| {
            StorageError::InvalidData("workflow wait has no wake binding".into())
        })?)?;
    binding
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(binding)
}

fn load_current_workflow_wake_binding(
    connection: &rusqlite::Connection,
    attempt: &WorkflowAttempt,
) -> Result<WorkflowWakeBinding, StorageError> {
    if !matches!(
        attempt.state,
        WorkflowState::Waiting(WaitReason::External | WaitReason::ProtectedComponent)
    ) || attempt.lease_owner.is_some()
        || attempt.lease_expires_at.is_some()
    {
        return Err(StorageError::Conflict(
            "workflow is not in an externally signalled wait".into(),
        ));
    }
    let event_json = connection
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [attempt.workflow_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::InvalidData("workflow wait has no event".into()))?;
    let event: WorkflowEvent = serde_json::from_str(&event_json)?;
    let payload = event_payload_value(&event)?;
    let stored_attempt = event_attempt(&event)?;
    let binding = workflow_wake_binding_from_event(&event)?;
    if event.type_id.as_str() != WORKFLOW_WAIT_BOUND_TYPE_ID
        || event.attempt_id != attempt.attempt_id
        || event.attempt_revision != attempt.revision
        || event.lease_generation != attempt.lease_generation
        || stored_attempt != *attempt
        || binding.attempt_id != attempt.attempt_id
        || binding.bound_revision != attempt.revision
        || binding.bound_generation != attempt.lease_generation
        || attempt.state != WorkflowState::Waiting(binding.wait_reason)
        || payload.get("from") != Some(&serde_json::json!(WorkflowState::Running))
        || payload.get("to") != Some(&serde_json::json!(attempt.state))
    {
        return Err(StorageError::InvalidData(
            "workflow wake binding does not match the current wait".into(),
        ));
    }
    Ok(binding)
}

fn is_cancellation_reconciliation_wait(
    connection: &rusqlite::Connection,
    attempt: &WorkflowAttempt,
) -> Result<bool, StorageError> {
    if attempt.state != WorkflowState::Waiting(WaitReason::Reconciliation) {
        return Ok(false);
    }
    let event_json = connection
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [attempt.workflow_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(event_json) = event_json else {
        return Ok(false);
    };
    let event: WorkflowEvent = serde_json::from_str(&event_json)?;
    let payload = event_payload_value(&event)?;
    Ok(
        (event.type_id.as_str() == WORKFLOW_CANCELLATION_RECOVERED_TYPE_ID
            && payload.get("recovery_reason")
                == Some(&serde_json::json!("cancellation_requires_reconciliation")))
            || (event.type_id.as_str() == WORKFLOW_TRANSITIONED_TYPE_ID
                && payload.get("from") == Some(&serde_json::json!(WorkflowState::CancelRequested))
                && payload.get("to")
                    == Some(&serde_json::json!(WorkflowState::Waiting(
                        WaitReason::Reconciliation
                    )))),
    )
}

fn retry_backoff_elapsed(attempt: &WorkflowAttempt, now: DateTime<Utc>) -> bool {
    let elapsed_ms = now
        .signed_duration_since(attempt.updated_at)
        .num_milliseconds();
    elapsed_ms >= 0
        && u64::try_from(elapsed_ms)
            .is_ok_and(|elapsed| elapsed >= attempt.retry_policy.initial_backoff_ms)
}

struct ActiveAgentTurnSource {
    task: TaskEnvelope,
    task_json: String,
    run: AgentRun,
    run_json: String,
    plan_json: String,
    planned_item: WorkItem,
    instance: AgentInstance,
    instance_json: String,
    lease: AgentLease,
    lease_json: String,
    conversation: Option<CompatibilityConversationSource>,
}

struct CompatibilityConversationSource {
    conversation_id: String,
    conversation_json: String,
    turn_id: Option<String>,
    turn_json: Option<String>,
    binding_json: Option<String>,
}

type ActiveAgentTurnSourceRow = (String, String, String, String, String);

fn load_active_agent_turn_source(
    transaction: &Transaction<'_>,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<ActiveAgentTurnSource, StorageError> {
    let row = transaction
        .query_row(
            "SELECT tasks.task_json, agent_runs.run_json, work_plans.plan_json,
                    agent_instances.instance_json, agent_leases.lease_json
             FROM agent_leases
             INNER JOIN agent_instances
                ON agent_instances.instance_id = agent_leases.instance_id
             INNER JOIN agent_runs ON agent_runs.run_id = agent_instances.run_id
             INNER JOIN work_plans ON work_plans.run_id = agent_runs.run_id
             INNER JOIN tasks ON tasks.task_id = agent_runs.task_id
             WHERE agent_leases.lease_id = ?1",
            [lease_id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(StorageError::AgentLeaseNotFound(lease_id))?;
    let mut source = decode_active_agent_turn_source(row, lease_id, now)?;
    source.conversation = load_compatibility_conversation_source(
        transaction,
        source.task.thread_id.map(|id| id.to_string()).as_deref(),
        source.run.id,
    )?;
    Ok(source)
}

fn load_compatibility_conversation_source(
    connection: &Connection,
    legacy_thread_id: Option<&str>,
    run_id: RunId,
) -> Result<Option<CompatibilityConversationSource>, StorageError> {
    let mut rows = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT bindings.conversation_id, conversations.conversation_json,
                    bindings.conversation_turn_id, conversation_turns.turn_json,
                    bindings.binding_json
             FROM codex_thread_bindings AS bindings
             INNER JOIN conversations
                ON conversations.conversation_id = bindings.conversation_id
             LEFT JOIN conversation_turns
                ON conversation_turns.turn_id = bindings.conversation_turn_id
             WHERE bindings.run_id = ?1 AND bindings.conversation_id IS NOT NULL
             ORDER BY bindings.conversation_id, bindings.conversation_turn_id
             LIMIT 2",
        )?;
        statement
            .query_map([run_id.to_string()], |row| {
                Ok(CompatibilityConversationSource {
                    conversation_id: row.get(0)?,
                    conversation_json: row.get(1)?,
                    turn_id: row.get(2)?,
                    turn_json: row.get(3)?,
                    binding_json: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if rows.len() > 1 {
        return Err(StorageError::Conflict(
            "agent run has ambiguous conversation source bindings".into(),
        ));
    }
    if let Some(source) = rows.pop() {
        let binding: CodexThreadBinding =
            serde_json::from_str(source.binding_json.as_deref().ok_or_else(|| {
                StorageError::InvalidData("conversation binding is missing".into())
            })?)?;
        if binding.conversation_id.map(|id| id.to_string()) != Some(source.conversation_id.clone())
            || binding.conversation_turn_id.map(|id| id.to_string()) != source.turn_id
            || binding.run_id != Some(run_id)
        {
            return Err(StorageError::InvalidData(
                "conversation binding columns diverge from immutable evidence".into(),
            ));
        }
        return Ok(Some(source));
    }
    let Some(legacy_thread_id) = legacy_thread_id else {
        return Ok(None);
    };
    connection
        .query_row(
            "SELECT conversation_id, conversation_json
             FROM conversations WHERE legacy_thread_id = ?1",
            [legacy_thread_id],
            |row| {
                Ok(CompatibilityConversationSource {
                    conversation_id: row.get(0)?,
                    conversation_json: row.get(1)?,
                    turn_id: None,
                    turn_json: None,
                    binding_json: None,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

fn decode_active_agent_turn_source(
    row: ActiveAgentTurnSourceRow,
    lease_id: AgentLeaseId,
    now: DateTime<Utc>,
) -> Result<ActiveAgentTurnSource, StorageError> {
    let (task_json, run_json, plan_json, instance_json, lease_json) = row;
    let task: TaskEnvelope = serde_json::from_str(&task_json)?;
    let run: AgentRun = serde_json::from_str(&run_json)?;
    let plan: WorkPlan = serde_json::from_str(&plan_json)?;
    let instance: AgentInstance = serde_json::from_str(&instance_json)?;
    let lease: AgentLease = serde_json::from_str(&lease_json)?;
    let planned_item = plan
        .items
        .iter()
        .find(|item| item.id == lease.work_item_id)
        .cloned();
    if lease.id != lease_id
        || lease.instance_id != instance.id
        || instance.run_id != run.id
        || instance.task_id != task.id
        || run.task_id != task.id
        || plan.run_id != run.id
        || plan.task_id != task.id
        || planned_item.is_none()
        || task.state != TaskState::Active
        || run.state != AgentRunState::Running
        || instance.state != AgentState::Running
        || lease.state != AgentLeaseState::Active
        || lease.expires_at <= now
    {
        return Err(StorageError::Conflict(
            "agent turn source is inactive, expired, or identity-inconsistent".into(),
        ));
    }
    let planned_item = planned_item.ok_or_else(|| {
        StorageError::Conflict("agent turn source has no matching planned work item".into())
    })?;
    Ok(ActiveAgentTurnSource {
        task,
        task_json,
        run,
        run_json,
        plan_json,
        planned_item,
        instance,
        instance_json,
        lease,
        lease_json,
        conversation: None,
    })
}

#[allow(clippy::too_many_lines)] // Keep the complete legacy-to-canonical authority derivation auditable.
fn derive_agent_turn_workflow(
    source: &ActiveAgentTurnSource,
    source_type: StableTypeId,
    now: DateTime<Utc>,
) -> Result<(TaskContract, WorkflowAttempt, WorkflowSourceAttachment), StorageError> {
    let snapshot = VersionedPayload::canonical_v1(
        source_type.clone(),
        &serde_json::json!({
            "task_id": source.task.id,
            "task_schema_version": source.task.schema_version,
            "task_sha256": sha256_text(&source.task_json),
            "run_id": source.run.id,
            "run_schema_version": source.run.schema_version,
            "run_sha256": sha256_text(&source.run_json),
            "plan_sha256": sha256_text(&source.plan_json),
            "instance_id": source.instance.id,
            "instance_schema_version": source.instance.schema_version,
            "instance_sha256": sha256_text(&source.instance_json),
            "lease_id": source.lease.id,
            "lease_schema_version": source.lease.schema_version,
            "lease_sha256": sha256_text(&source.lease_json),
        }),
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let source_ref = SourceRef {
        source_type,
        locator: source.lease.id.to_string(),
        content_hash: snapshot
            .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    };
    let requirement = ContractClause {
        clause_id: "legacy-agent-lease-goal".into(),
        statement: source.lease.goal.clone(),
        source_refs: vec![source_ref.clone()],
    };
    let constraints = source
        .task
        .constraints
        .iter()
        .enumerate()
        .map(|(index, statement)| ContractClause {
            clause_id: format!("legacy-constraint-{}", index + 1),
            statement: statement.clone(),
            source_refs: vec![source_ref.clone()],
        })
        .collect();
    let acceptance_criteria = if source.planned_item.success_conditions.is_empty() {
        source
            .task
            .success_conditions
            .iter()
            .enumerate()
            .map(|(index, statement)| ContractClause {
                clause_id: format!("legacy-acceptance-{}", index + 1),
                statement: statement.clone(),
                source_refs: vec![source_ref.clone()],
            })
            .collect()
    } else {
        source
            .planned_item
            .success_conditions
            .iter()
            .enumerate()
            .map(|(index, condition)| {
                Ok(ContractClause {
                    clause_id: format!("legacy-work-condition-{}", index + 1),
                    statement: format!(
                        "Objective condition: {}",
                        serde_json::to_string(condition)?
                    ),
                    source_refs: vec![source_ref.clone()],
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?
    };
    let permitted_side_effects = agent_turn_policy(
        "mcg.agent_turn_side_effect_policy.v1",
        "MetaCog.AgentTurnSideEffects",
        &serde_json::json!({
            "lease_id": source.lease.id,
            "tool_permissions": source.lease.tool_permissions,
        }),
    )?;
    let authority = agent_turn_policy(
        "mcg.agent_turn_authority_policy.v1",
        "MetaCog.AgentTurnAuthority",
        &serde_json::json!({
            "task_id": source.task.id,
            "run_id": source.run.id,
            "instance_id": source.instance.id,
            "role_id": source.instance.role_id,
            "lease_id": source.lease.id,
            "work_item_id": source.lease.work_item_id,
        }),
    )?;
    let environment = agent_turn_policy(
        "mcg.agent_turn_environment_policy.v1",
        "MetaCog.AgentTurnEnvironment",
        &serde_json::json!({
            "project_id": source.task.project_id,
            "thread_id": source.task.thread_id,
            "context_fingerprint": source.instance.context_fingerprint,
            "source_snapshot_hash": source_ref.content_hash,
        }),
    )?;
    let risk = agent_turn_policy(
        "mcg.agent_turn_risk_policy.v1",
        "MetaCog.AgentTurnRisk",
        &serde_json::json!({"risk": source.task.risk}),
    )?;
    let budgets = agent_turn_policy(
        "mcg.agent_turn_budget_policy.v1",
        "MetaCog.AgentTurnBudget",
        &serde_json::json!({
            "max_tokens": source.lease.budget.limit.max_tokens,
            "max_cost_usd": optional_f64_string(source.lease.budget.limit.max_cost_usd)?,
            "max_duration_seconds": source.lease.budget.limit.max_duration_seconds,
            "consumed_tokens": source.lease.budget.consumed_tokens,
            "consumed_cost_usd": f64_string(source.lease.budget.consumed_cost_usd)?,
            "elapsed_seconds": source.lease.budget.elapsed_seconds,
        }),
    )?;
    let rollback = agent_turn_policy(
        "mcg.agent_turn_rollback_policy.v1",
        "MetaCog.AgentTurnRollback",
        &serde_json::json!({
            "mode": "reconcile_governed_effects",
            "legacy_source": source_ref,
        }),
    )?;
    let task_contract_id = TaskContractId::new();
    let task_contract = TaskContract {
        id: task_contract_id,
        version: 1,
        previous_version: None,
        user_intent_ref: source_ref.clone(),
        objective: source.task.goal.clone(),
        non_goals: Vec::new(),
        requirements: vec![requirement],
        constraints,
        assumptions: Vec::new(),
        acceptance_criteria,
        dependencies: Vec::new(),
        permitted_side_effects,
        authority,
        environment: environment.clone(),
        risk,
        budgets,
        rollback: rollback.clone(),
        status: TaskContractStatus::Active,
        created_at: now,
    };
    let workflow_id = WorkflowId::new();
    let attempt_id = AttemptId::new();
    let placeholder_contract_hash = source_ref.content_hash.clone();
    let attempt = WorkflowAttempt {
        workflow_id,
        attempt_id,
        task_contract: VersionedRef {
            id: task_contract_id,
            version: 1,
            content_hash: placeholder_contract_hash.clone(),
        },
        parent_attempt_id: None,
        attempt_number: 1,
        state: WorkflowState::Ready,
        revision: 1,
        lease_owner: None,
        lease_generation: 0,
        lease_expires_at: None,
        heartbeat_at: None,
        checkpoint: None,
        environment_snapshot: environment.content_hash,
        configuration: VersionedRef {
            id: ConfigurationId::new(),
            version: 1,
            content_hash: source_ref.content_hash.clone(),
        },
        idempotency_key: format!("attach-agent-lease:{}", source.lease.id),
        retry_policy: RetryPolicy {
            max_attempts: u32::from(source.lease.max_retries)
                .checked_add(1)
                .ok_or_else(|| StorageError::InvalidData("agent retry limit overflowed".into()))?,
            initial_backoff_ms: 1_000,
            maximum_backoff_ms: 60_000,
        },
        compensation_policy: rollback,
        created_at: now,
        updated_at: now,
    };
    let attachment = WorkflowSourceAttachment {
        schema_version: WorkflowSourceAttachment::SCHEMA_VERSION,
        source: source_ref,
        source_snapshot: snapshot,
        workflow_id,
        attempt_id,
        task_contract: attempt.task_contract.clone(),
        attached_at: now,
    };
    Ok((task_contract, attempt, attachment))
}

fn derive_agent_turn_source_links(
    source: &ActiveAgentTurnSource,
    attachment: &WorkflowSourceAttachment,
    now: DateTime<Utc>,
) -> Result<Vec<WorkflowSourceLink>, StorageError> {
    let mut snapshots = vec![
        VersionedPayload::canonical_v1(
            stable_type_id("mcg.compatibility_task_source.v1")?,
            &serde_json::json!({
                "task_id": source.task.id,
                "task_schema_version": source.task.schema_version,
                "task_sha256": sha256_text(&source.task_json),
            }),
        )
        .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        VersionedPayload::canonical_v1(
            stable_type_id("mcg.compatibility_agent_run_source.v1")?,
            &serde_json::json!({
                "run_id": source.run.id,
                "run_schema_version": source.run.schema_version,
                "run_sha256": sha256_text(&source.run_json),
                "task_id": source.task.id,
                "task_sha256": sha256_text(&source.task_json),
                "plan_sha256": sha256_text(&source.plan_json),
            }),
        )
        .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    ];
    if let Some(conversation) = &source.conversation {
        snapshots.push(
            VersionedPayload::canonical_v1(
                stable_type_id("mcg.compatibility_conversation_source.v1")?,
                &serde_json::json!({
                    "conversation_id": conversation.conversation_id,
                    "conversation_sha256": sha256_text(&conversation.conversation_json),
                    "turn_id": conversation.turn_id,
                    "turn_sha256": conversation.turn_json.as_deref().map(sha256_text),
                    "binding_sha256": conversation.binding_json.as_deref().map(sha256_text),
                    "task_id": source.task.id,
                    "run_id": source.run.id,
                }),
            )
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        );
    }
    let mut links: Vec<WorkflowSourceLink> = snapshots
        .into_iter()
        .map(|snapshot| {
            let locator = match snapshot.type_id.as_str() {
                "mcg.compatibility_task_source.v1" => source.task.id.to_string(),
                "mcg.compatibility_agent_run_source.v1" => source.run.id.to_string(),
                "mcg.compatibility_conversation_source.v1" => source
                    .conversation
                    .as_ref()
                    .map(|conversation| conversation.conversation_id.clone())
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "conversation source snapshot has no conversation".into(),
                        )
                    })?,
                _ => {
                    return Err(StorageError::InvalidData(
                        "unsupported compatibility workflow source type".into(),
                    ));
                }
            };
            let source_ref = SourceRef {
                source_type: snapshot.type_id.clone(),
                locator,
                content_hash: snapshot
                    .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?,
            };
            Ok(WorkflowSourceLink {
                schema_version: WorkflowSourceLink::SCHEMA_VERSION,
                source: source_ref,
                source_snapshot: snapshot,
                primary_source: attachment.source.clone(),
                workflow_id: attachment.workflow_id,
                attempt_id: attachment.attempt_id,
                task_contract: attachment.task_contract.clone(),
                linked_at: now,
            })
        })
        .collect::<Result<_, _>>()?;
    links.sort_by(|left, right| {
        left.source
            .source_type
            .as_str()
            .cmp(right.source.source_type.as_str())
            .then_with(|| left.source.locator.cmp(&right.source.locator))
    });
    Ok(links)
}

fn agent_turn_policy(
    type_id: &str,
    hash_domain: &str,
    value: &serde_json::Value,
) -> Result<PolicyBinding, StorageError> {
    let payload = VersionedPayload::canonical_v1(stable_type_id(type_id)?, value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let content_hash = payload
        .content_hash(hash_domain)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn f64_string(value: f64) -> Result<String, StorageError> {
    value
        .is_finite()
        .then(|| value.to_string())
        .ok_or_else(|| StorageError::InvalidData("agent budget contains a non-finite cost".into()))
}

fn optional_f64_string(value: Option<f64>) -> Result<Option<String>, StorageError> {
    value.map(f64_string).transpose()
}

fn insert_or_replay_workflow_source_link(
    transaction: &Transaction<'_>,
    link: &WorkflowSourceLink,
) -> Result<WorkflowSourceLink, StorageError> {
    validate_workflow_source_link(transaction, link)?;
    let existing = transaction
        .query_row(
            "SELECT source_type, source_locator, source_hash_json, workflow_id, attempt_id,
                    task_contract_id, task_contract_version, task_contract_hash_json,
                    primary_source_type, primary_source_locator, primary_source_hash_json,
                    link_json, linked_at
             FROM workflow_source_links
             WHERE source_type = ?1 AND source_locator = ?2 AND workflow_id = ?3",
            params![
                link.source.source_type.as_str(),
                link.source.locator.as_str(),
                link.workflow_id.to_string()
            ],
            workflow_source_link_row,
        )
        .optional()?;
    if let Some(existing) = existing {
        let existing = validate_loaded_workflow_source_link(transaction, existing)?;
        if existing != *link {
            return Err(StorageError::Conflict(
                "workflow source link was replayed with different evidence".into(),
            ));
        }
        return Ok(existing);
    }
    transaction.execute(
        "INSERT INTO workflow_source_links(
            source_type, source_locator, source_hash_json, workflow_id, attempt_id,
            task_contract_id, task_contract_version, task_contract_hash_json,
            primary_source_type, primary_source_locator, primary_source_hash_json,
            link_json, linked_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            link.source.source_type.as_str(),
            link.source.locator.as_str(),
            serde_json::to_string(&link.source.content_hash)?,
            link.workflow_id.to_string(),
            link.attempt_id.to_string(),
            link.task_contract.id.to_string(),
            sql_u64(link.task_contract.version, "task contract version")?,
            serde_json::to_string(&link.task_contract.content_hash)?,
            link.primary_source.source_type.as_str(),
            link.primary_source.locator.as_str(),
            serde_json::to_string(&link.primary_source.content_hash)?,
            serde_json::to_string(link)?,
            link.linked_at.to_rfc3339(),
        ],
    )?;
    Ok(link.clone())
}

type WorkflowSourceLinkRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn workflow_source_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkflowSourceLinkRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
    ))
}

fn load_workflow_source_links_by_source(
    connection: &Connection,
    source_type: &str,
    source_locator: &str,
) -> Result<Vec<WorkflowSourceLink>, StorageError> {
    let row_limit = i64::try_from(MAX_WORKFLOW_SOURCE_LINKS + 1).map_err(|_| {
        StorageError::InvalidData("workflow source-link inspection bound overflowed".into())
    })?;
    let mut statement = connection.prepare(
        "SELECT source_type, source_locator, source_hash_json, workflow_id, attempt_id,
                task_contract_id, task_contract_version, task_contract_hash_json,
                primary_source_type, primary_source_locator, primary_source_hash_json,
                link_json, linked_at
         FROM workflow_source_links
         WHERE source_type = ?1 AND source_locator = ?2
         ORDER BY linked_at, workflow_id LIMIT ?3",
    )?;
    let mut links = statement
        .query_map(
            params![source_type, source_locator, row_limit],
            workflow_source_link_row,
        )?
        .map(|row| validate_loaded_workflow_source_link(connection, row?))
        .collect::<Result<Vec<_>, _>>()?;
    if links.len() > MAX_WORKFLOW_SOURCE_LINKS {
        return Err(StorageError::InvalidData(
            "workflow source-link result exceeds the bounded inspection limit".into(),
        ));
    }
    links.shrink_to_fit();
    Ok(links)
}

fn load_workflow_source_links_for_workflow(
    connection: &Connection,
    workflow_id: WorkflowId,
) -> Result<Vec<WorkflowSourceLink>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT source_type, source_locator, source_hash_json, workflow_id, attempt_id,
                task_contract_id, task_contract_version, task_contract_hash_json,
                primary_source_type, primary_source_locator, primary_source_hash_json,
                link_json, linked_at
         FROM workflow_source_links
         WHERE workflow_id = ?1 ORDER BY source_type, source_locator",
    )?;
    statement
        .query_map([workflow_id.to_string()], workflow_source_link_row)?
        .map(|row| validate_loaded_workflow_source_link(connection, row?))
        .collect()
}

fn validate_loaded_workflow_source_link(
    connection: &Connection,
    row: WorkflowSourceLinkRow,
) -> Result<WorkflowSourceLink, StorageError> {
    let (
        source_type,
        source_locator,
        source_hash_json,
        workflow_id,
        attempt_id,
        task_contract_id,
        task_contract_version,
        task_contract_hash_json,
        primary_source_type,
        primary_source_locator,
        primary_source_hash_json,
        link_json,
        linked_at,
    ) = row;
    let link: WorkflowSourceLink = serde_json::from_str(&link_json)?;
    link.validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let source_hash: ContentHash = serde_json::from_str(&source_hash_json)?;
    let task_contract_hash: ContentHash = serde_json::from_str(&task_contract_hash_json)?;
    let primary_source_hash: ContentHash = serde_json::from_str(&primary_source_hash_json)?;
    let task_contract_version = u64::try_from(task_contract_version).map_err(|_| {
        StorageError::InvalidData("negative workflow source-link contract version".into())
    })?;
    if source_type != link.source.source_type.as_str()
        || source_locator != link.source.locator
        || source_hash != link.source.content_hash
        || workflow_id != link.workflow_id.to_string()
        || attempt_id != link.attempt_id.to_string()
        || task_contract_id != link.task_contract.id.to_string()
        || task_contract_version != link.task_contract.version
        || task_contract_hash != link.task_contract.content_hash
        || primary_source_type != link.primary_source.source_type.as_str()
        || primary_source_locator != link.primary_source.locator
        || primary_source_hash != link.primary_source.content_hash
        || linked_at != link.linked_at.to_rfc3339()
    {
        return Err(StorageError::InvalidData(
            "workflow source-link columns diverge from immutable evidence".into(),
        ));
    }
    validate_workflow_source_link(connection, &link)?;
    Ok(link)
}

fn validate_workflow_source_link(
    connection: &Connection,
    link: &WorkflowSourceLink,
) -> Result<(), StorageError> {
    link.validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let primary = load_workflow_source_attachment_by_source(
        connection,
        link.primary_source.source_type.as_str(),
        &link.primary_source.locator,
    )?
    .ok_or_else(|| {
        StorageError::Conflict("workflow source link has no primary attachment".into())
    })?;
    if primary.source != link.primary_source
        || primary.workflow_id != link.workflow_id
        || primary.attempt_id != link.attempt_id
        || primary.task_contract != link.task_contract
    {
        return Err(StorageError::Conflict(
            "workflow source link does not bind its exact primary workflow".into(),
        ));
    }
    Ok(())
}

fn insert_or_replay_workflow_source_attachment(
    transaction: &Transaction<'_>,
    attachment: &WorkflowSourceAttachment,
) -> Result<WorkflowSourceAttachment, StorageError> {
    if let Some(existing) = load_workflow_source_attachment_by_source(
        transaction,
        attachment.source.source_type.as_str(),
        &attachment.source.locator,
    )? {
        if existing != *attachment {
            return Err(StorageError::Conflict(
                "workflow source identity was replayed with different evidence".into(),
            ));
        }
        return Ok(existing);
    }
    transaction.execute(
        "INSERT INTO workflow_source_attachments(
            source_type, source_locator, source_hash_json, workflow_id, attempt_id,
            task_contract_id, task_contract_version, task_contract_hash_json,
            attachment_json, attached_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            attachment.source.source_type.as_str(),
            attachment.source.locator.as_str(),
            serde_json::to_string(&attachment.source.content_hash)?,
            attachment.workflow_id.to_string(),
            attachment.attempt_id.to_string(),
            attachment.task_contract.id.to_string(),
            sql_u64(attachment.task_contract.version, "task contract version")?,
            serde_json::to_string(&attachment.task_contract.content_hash)?,
            serde_json::to_string(attachment)?,
            attachment.attached_at.to_rfc3339(),
        ],
    )?;
    Ok(attachment.clone())
}

fn load_agent_turn_workflow_attachment(
    connection: &Connection,
    attachment: WorkflowSourceAttachment,
) -> Result<AgentTurnWorkflowAttachment, StorageError> {
    let attempt = load_attempt(connection, attachment.attempt_id)?.ok_or_else(|| {
        StorageError::InvalidData("workflow source attachment references a missing attempt".into())
    })?;
    let task_contract = load_task_contract(
        connection,
        attachment.task_contract.id,
        attachment.task_contract.version,
    )?
    .ok_or_else(|| {
        StorageError::InvalidData(
            "workflow source attachment references a missing task contract".into(),
        )
    })?;
    validate_workflow_source_attachment(&task_contract, &attempt, &attachment)?;
    let creation_event =
        load_event_by_idempotency(connection, attempt.workflow_id, &attempt.idempotency_key)?
            .ok_or_else(|| {
                StorageError::InvalidData(
                    "workflow source attachment references a missing creation event".into(),
                )
            })?;
    let created_attempt = event_attempt(&creation_event)?;
    validate_initial_workflow(&task_contract, &created_attempt)?;
    validate_workflow_source_attachment(&task_contract, &created_attempt, &attachment)?;
    if creation_event.type_id.as_str() != WORKFLOW_CREATED_TYPE_ID
        || created_attempt.workflow_id != attempt.workflow_id
        || created_attempt.attempt_id != attempt.attempt_id
        || created_attempt.task_contract != attempt.task_contract
        || created_attempt.parent_attempt_id != attempt.parent_attempt_id
        || created_attempt.attempt_number != attempt.attempt_number
        || created_attempt.environment_snapshot != attempt.environment_snapshot
        || created_attempt.configuration != attempt.configuration
        || created_attempt.idempotency_key != attempt.idempotency_key
        || created_attempt.retry_policy != attempt.retry_policy
        || created_attempt.compensation_policy != attempt.compensation_policy
        || created_attempt.created_at != attempt.created_at
    {
        return Err(StorageError::InvalidData(
            "workflow source attachment creation evidence is inconsistent".into(),
        ));
    }
    Ok(AgentTurnWorkflowAttachment {
        task_contract,
        attempt,
        attachment,
        source_links: load_workflow_source_links_for_workflow(
            connection,
            created_attempt.workflow_id,
        )?,
        creation_event,
    })
}

type WorkflowSourceAttachmentRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    String,
);

fn workflow_source_attachment_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WorkflowSourceAttachmentRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn load_workflow_source_attachment_by_source(
    connection: &Connection,
    source_type: &str,
    source_locator: &str,
) -> Result<Option<WorkflowSourceAttachment>, StorageError> {
    let row = connection
        .query_row(
            "SELECT source_type, source_locator, source_hash_json, workflow_id, attempt_id,
                    task_contract_id, task_contract_version, task_contract_hash_json,
                    attachment_json, attached_at
             FROM workflow_source_attachments
             WHERE source_type = ?1 AND source_locator = ?2",
            params![source_type, source_locator],
            workflow_source_attachment_row,
        )
        .optional()?;
    row.map(|row| validate_loaded_workflow_source_attachment(connection, row))
        .transpose()
}

fn validate_loaded_workflow_source_attachment(
    connection: &Connection,
    row: WorkflowSourceAttachmentRow,
) -> Result<WorkflowSourceAttachment, StorageError> {
    let attachment = validate_stored_workflow_source_attachment(row)?;
    let attempt = load_attempt(connection, attachment.attempt_id)?.ok_or_else(|| {
        StorageError::InvalidData("workflow source attachment references a missing attempt".into())
    })?;
    let task_contract = load_task_contract(
        connection,
        attachment.task_contract.id,
        attachment.task_contract.version,
    )?
    .ok_or_else(|| {
        StorageError::InvalidData(
            "workflow source attachment references a missing task contract".into(),
        )
    })?;
    validate_workflow_source_attachment(&task_contract, &attempt, &attachment)?;
    Ok(attachment)
}

fn validate_stored_workflow_source_attachment(
    row: WorkflowSourceAttachmentRow,
) -> Result<WorkflowSourceAttachment, StorageError> {
    let (
        source_type,
        source_locator,
        source_hash_json,
        workflow_id,
        attempt_id,
        task_contract_id,
        task_contract_version,
        task_contract_hash_json,
        attachment_json,
        attached_at,
    ) = row;
    let attachment: WorkflowSourceAttachment = serde_json::from_str(&attachment_json)?;
    attachment
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let source_hash: ContentHash = serde_json::from_str(&source_hash_json)?;
    let task_contract_hash: ContentHash = serde_json::from_str(&task_contract_hash_json)?;
    let task_contract_version = u64::try_from(task_contract_version).map_err(|_| {
        StorageError::InvalidData("negative workflow attachment contract version".into())
    })?;
    if source_type != attachment.source.source_type.as_str()
        || source_locator != attachment.source.locator
        || source_hash != attachment.source.content_hash
        || workflow_id != attachment.workflow_id.to_string()
        || attempt_id != attachment.attempt_id.to_string()
        || task_contract_id != attachment.task_contract.id.to_string()
        || task_contract_version != attachment.task_contract.version
        || task_contract_hash != attachment.task_contract.content_hash
        || attached_at != attachment.attached_at.to_rfc3339()
    {
        return Err(StorageError::InvalidData(
            "workflow source attachment columns diverge from immutable evidence".into(),
        ));
    }
    Ok(attachment)
}

fn validate_workflow_source_attachment(
    contract: &TaskContract,
    attempt: &WorkflowAttempt,
    attachment: &WorkflowSourceAttachment,
) -> Result<(), StorageError> {
    attachment
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let contract_hash = task_contract_content_hash(contract)?;
    if attachment.workflow_id != attempt.workflow_id
        || attachment.attempt_id != attempt.attempt_id
        || attachment.task_contract != attempt.task_contract
        || attachment.task_contract.id != contract.id
        || attachment.task_contract.version != contract.version
        || attachment.task_contract.content_hash != contract_hash
    {
        return Err(StorageError::Conflict(
            "workflow source attachment does not bind the exact canonical workflow".into(),
        ));
    }
    Ok(())
}

fn validate_initial_workflow(
    contract: &TaskContract,
    attempt: &WorkflowAttempt,
) -> Result<(), StorageError> {
    let distinct_dependencies = contract
        .dependencies
        .iter()
        .map(|dependency| (dependency.id, dependency.version))
        .collect::<std::collections::BTreeSet<_>>();
    let policy_bindings = [
        &contract.permitted_side_effects,
        &contract.authority,
        &contract.environment,
        &contract.risk,
        &contract.budgets,
        &contract.rollback,
        &attempt.compensation_policy,
    ];
    for binding in policy_bindings {
        let actual = binding
            .payload
            .content_hash(&binding.content_hash.domain)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if actual != binding.content_hash {
            return Err(StorageError::InvalidData(
                "policy binding hash does not match canonical payload".into(),
            ));
        }
    }
    let first_version = contract.version == 1 && contract.previous_version.is_none();
    if contract.objective.trim().is_empty()
        || !first_version
        || contract.status != TaskContractStatus::Active
        || contract
            .dependencies
            .iter()
            .any(|dependency| dependency.id == contract.id)
        || distinct_dependencies.len() != contract.dependencies.len()
        || attempt.task_contract.id != contract.id
        || attempt.task_contract.version != contract.version
        || attempt.parent_attempt_id.is_some()
        || attempt.attempt_number != 1
        || attempt.state != WorkflowState::Ready
        || attempt.revision != 1
        || attempt.lease_generation != 0
        || attempt.lease_owner.is_some()
        || attempt.lease_expires_at.is_some()
        || attempt.heartbeat_at.is_some()
        || attempt.idempotency_key.trim().is_empty()
        || attempt.retry_policy.max_attempts == 0
        || attempt.retry_policy.initial_backoff_ms > attempt.retry_policy.maximum_backoff_ms
        || attempt.created_at != attempt.updated_at
    {
        return Err(StorageError::InvalidData(
            "workflow must atomically begin as a valid active contract and unleased Ready attempt"
                .into(),
        ));
    }
    Ok(())
}

fn validate_contract(contract: &TaskContract) -> Result<(), StorageError> {
    let distinct_dependencies = contract
        .dependencies
        .iter()
        .map(|dependency| (dependency.id, dependency.version))
        .collect::<std::collections::BTreeSet<_>>();
    for binding in [
        &contract.permitted_side_effects,
        &contract.authority,
        &contract.environment,
        &contract.risk,
        &contract.budgets,
        &contract.rollback,
    ] {
        let actual = binding
            .payload
            .content_hash(&binding.content_hash.domain)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if actual != binding.content_hash {
            return Err(StorageError::InvalidData(
                "policy binding hash does not match canonical payload".into(),
            ));
        }
    }
    let version_lineage = contract.version == 1 && contract.previous_version.is_none()
        || contract.version > 1
            && contract.previous_version.as_ref().is_some_and(|previous| {
                previous.id == contract.id
                    && previous
                        .version
                        .checked_add(1)
                        .is_some_and(|version| version == contract.version)
            });
    if contract.objective.trim().is_empty()
        || !version_lineage
        || contract.status != TaskContractStatus::Active
        || contract
            .dependencies
            .iter()
            .any(|dependency| dependency.id == contract.id)
        || distinct_dependencies.len() != contract.dependencies.len()
    {
        return Err(StorageError::InvalidData(
            "task-contract revision is invalid or not active".into(),
        ));
    }
    Ok(())
}

fn insert_contract_version(
    transaction: &Transaction<'_>,
    contract: &TaskContract,
) -> Result<(), StorageError> {
    let contract_hash = task_contract_content_hash(contract)?;
    transaction.execute(
        "INSERT INTO task_contract_versions(
            task_contract_id, version, type_id, schema_version, content_hash_json,
            contract_json, status, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            contract.id.to_string(),
            sql_u64(contract.version, "task contract version")?,
            TaskContract::TYPE_ID,
            i64::from(TaskContract::SCHEMA_VERSION),
            serde_json::to_string(&contract_hash)?,
            serde_json::to_string(contract)?,
            task_contract_status_label(contract.status),
            contract.created_at.to_rfc3339(),
        ],
    )?;
    for dependency in &contract.dependencies {
        transaction.execute(
            "INSERT INTO task_contract_dependencies(
                task_contract_id, task_contract_version, dependency_id, dependency_version
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                contract.id.to_string(),
                sql_u64(contract.version, "task contract version")?,
                dependency.id.to_string(),
                sql_u64(dependency.version, "dependency version")?,
            ],
        )?;
    }
    Ok(())
}

fn load_task_contract(
    connection: &Connection,
    id: TaskContractId,
    version: u64,
) -> Result<Option<TaskContract>, StorageError> {
    connection
        .query_row(
            "SELECT contract_json FROM task_contract_versions
             WHERE task_contract_id = ?1 AND version = ?2",
            params![id.to_string(), sql_u64(version, "task contract version")?],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn insert_attempt(
    transaction: &Transaction<'_>,
    attempt: &WorkflowAttempt,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO workflow_attempts(
            attempt_id, workflow_id, attempt_number, parent_attempt_id,
            task_contract_id, task_contract_version, state, revision, lease_generation,
            lease_owner, lease_expires_at, heartbeat_at, idempotency_key, attempt_json,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            attempt.attempt_id.to_string(),
            attempt.workflow_id.to_string(),
            i64::from(attempt.attempt_number),
            attempt.parent_attempt_id.map(|id| id.to_string()),
            attempt.task_contract.id.to_string(),
            sql_u64(attempt.task_contract.version, "task contract version")?,
            state_label(attempt.state),
            sql_u64(attempt.revision, "workflow revision")?,
            sql_u64(attempt.lease_generation, "lease generation")?,
            attempt.lease_owner.map(|id| id.to_string()),
            attempt.lease_expires_at.map(|value| value.to_rfc3339()),
            attempt.heartbeat_at.map(|value| value.to_rfc3339()),
            attempt.idempotency_key,
            serde_json::to_string(attempt)?,
            attempt.created_at.to_rfc3339(),
            attempt.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn update_attempt_cas(
    transaction: &Transaction<'_>,
    attempt: &WorkflowAttempt,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE workflow_attempts SET
            state = ?1, revision = ?2, lease_generation = ?3, lease_owner = ?4,
            lease_expires_at = ?5, heartbeat_at = ?6, attempt_json = ?7, updated_at = ?8
         WHERE attempt_id = ?9 AND revision = ?10 AND lease_generation = ?11",
        params![
            state_label(attempt.state),
            sql_u64(attempt.revision, "workflow revision")?,
            sql_u64(attempt.lease_generation, "lease generation")?,
            attempt.lease_owner.map(|id| id.to_string()),
            attempt.lease_expires_at.map(|value| value.to_rfc3339()),
            attempt.heartbeat_at.map(|value| value.to_rfc3339()),
            serde_json::to_string(attempt)?,
            attempt.updated_at.to_rfc3339(),
            attempt.attempt_id.to_string(),
            sql_u64(expected_revision, "expected workflow revision")?,
            sql_u64(expected_generation, "expected lease generation")?,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::Conflict(
            "workflow revision or lease generation changed before commit".into(),
        ));
    }
    Ok(())
}

fn insert_event(transaction: &Transaction<'_>, event: &WorkflowEvent) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO workflow_events(
            event_id, workflow_id, sequence, attempt_id, attempt_revision, lease_generation,
            type_id, schema_version, correlation_id, causation_id, idempotency_key,
            event_json, occurred_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            event.id.to_string(),
            event.workflow_id.to_string(),
            sql_u64(event.sequence, "workflow event sequence")?,
            event.attempt_id.to_string(),
            sql_u64(event.attempt_revision, "workflow attempt revision")?,
            sql_u64(event.lease_generation, "lease generation")?,
            event.type_id.as_str(),
            i64::from(event.schema_version),
            event.correlation_id.to_string(),
            event.causation_id.map(|id| id.to_string()),
            event.idempotency_key,
            serde_json::to_string(event)?,
            event.occurred_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn load_attempt(
    connection: &Connection,
    attempt_id: AttemptId,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    connection
        .query_row(
            "SELECT attempt_json FROM workflow_attempts WHERE attempt_id = ?1",
            [attempt_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_attempt_by_idempotency(
    connection: &Connection,
    workflow_id: WorkflowId,
    idempotency_key: &str,
) -> Result<Option<WorkflowAttempt>, StorageError> {
    connection
        .query_row(
            "SELECT attempt_json FROM workflow_attempts
             WHERE workflow_id = ?1 AND idempotency_key = ?2",
            params![workflow_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_event_by_idempotency(
    connection: &Connection,
    workflow_id: WorkflowId,
    idempotency_key: &str,
) -> Result<Option<WorkflowEvent>, StorageError> {
    connection
        .query_row(
            "SELECT event_json FROM workflow_events
             WHERE workflow_id = ?1 AND idempotency_key = ?2",
            params![workflow_id.to_string(), idempotency_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|json| serde_json::from_str(&json).map_err(StorageError::from))
        .transpose()
}

fn load_nonterminal_attempts(
    connection: &Connection,
) -> Result<Vec<WorkflowAttempt>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attempt_json FROM workflow_attempts
         WHERE state NOT IN ('succeeded', 'failed', 'cancelled', 'quarantined')
         ORDER BY updated_at, workflow_id, attempt_number",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| {
        let json = row?;
        serde_json::from_str(&json).map_err(StorageError::from)
    })
    .collect()
}

fn dependencies_satisfied(
    connection: &Connection,
    attempt: &WorkflowAttempt,
) -> Result<bool, StorageError> {
    let unsatisfied = connection.query_row(
        "SELECT COUNT(*) FROM task_contract_dependencies AS dependency
         WHERE dependency.task_contract_id = ?1
           AND dependency.task_contract_version = ?2
           AND NOT EXISTS (
               SELECT 1 FROM workflow_attempts AS prerequisite
               WHERE prerequisite.task_contract_id = dependency.dependency_id
                 AND prerequisite.task_contract_version = dependency.dependency_version
                 AND prerequisite.state = 'succeeded'
           )",
        params![
            attempt.task_contract.id.to_string(),
            sql_u64(attempt.task_contract.version, "task contract version")?,
        ],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(unsatisfied == 0)
}

fn next_event_position(
    transaction: &Transaction<'_>,
    workflow_id: WorkflowId,
) -> Result<(u64, Option<EventId>), StorageError> {
    let current = transaction
        .query_row(
            "SELECT sequence, event_json FROM workflow_events
             WHERE workflow_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [workflow_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    match current {
        Some((sequence, json)) => {
            let event: WorkflowEvent = serde_json::from_str(&json)?;
            let sequence = u64::try_from(sequence).map_err(|_| {
                StorageError::InvalidData("negative workflow event sequence".into())
            })?;
            Ok((
                increment(sequence, "workflow event sequence")?,
                Some(event.id),
            ))
        }
        None => Ok((1, None)),
    }
}

#[allow(clippy::too_many_arguments)]
fn workflow_event(
    attempt: &WorkflowAttempt,
    sequence: u64,
    correlation_id: CorrelationId,
    causation_id: Option<EventId>,
    idempotency_key: &str,
    event_type: &str,
    occurred_at: DateTime<Utc>,
    value: &serde_json::Value,
) -> Result<WorkflowEvent, StorageError> {
    let type_id = stable_type_id(event_type)?;
    let payload = VersionedPayload::canonical_v1(type_id.clone(), value)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    Ok(WorkflowEvent {
        id: EventId::new(),
        type_id,
        schema_version: WorkflowEvent::SCHEMA_VERSION,
        sequence,
        workflow_id: attempt.workflow_id,
        attempt_id: attempt.attempt_id,
        attempt_revision: attempt.revision,
        lease_generation: attempt.lease_generation,
        correlation_id,
        causation_id,
        idempotency_key: idempotency_key.to_owned(),
        occurred_at,
        payload,
    })
}

fn ensure_expected(
    attempt: &WorkflowAttempt,
    expected_revision: u64,
    expected_generation: u64,
) -> Result<(), StorageError> {
    if attempt.revision != expected_revision || attempt.lease_generation != expected_generation {
        return Err(StorageError::Conflict(format!(
            "stale workflow expectation: revision/generation {expected_revision}/{expected_generation}, current {}/{}",
            attempt.revision, attempt.lease_generation
        )));
    }
    Ok(())
}

fn state_label(state: WorkflowState) -> &'static str {
    match state {
        WorkflowState::Ready => "ready",
        WorkflowState::Running => "running",
        WorkflowState::Waiting(_) => "waiting",
        WorkflowState::Verifying => "verifying",
        WorkflowState::Succeeded => "succeeded",
        WorkflowState::Failed => "failed",
        WorkflowState::CancelRequested => "cancel_requested",
        WorkflowState::Cancelled => "cancelled",
        WorkflowState::Quarantined => "quarantined",
    }
}

fn task_contract_status_label(status: TaskContractStatus) -> &'static str {
    match status {
        TaskContractStatus::Draft => "draft",
        TaskContractStatus::Active => "active",
        TaskContractStatus::Superseded => "superseded",
        TaskContractStatus::Cancelled => "cancelled",
    }
}

fn stable_type_id(value: &str) -> Result<StableTypeId, StorageError> {
    StableTypeId::parse(value).map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn event_payload_value(event: &WorkflowEvent) -> Result<serde_json::Value, StorageError> {
    decode_canonical_cbor_v1(&event.payload.bytes)
        .map_err(|error| StorageError::InvalidData(error.to_string()))
}

fn event_attempt(event: &WorkflowEvent) -> Result<WorkflowAttempt, StorageError> {
    let value = event_payload_value(event)?;
    serde_json::from_value(value.get("attempt").cloned().ok_or_else(|| {
        StorageError::InvalidData("workflow event has no attempt snapshot".into())
    })?)
    .map_err(StorageError::from)
}

fn sql_u64(value: u64, label: &str) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidData(format!("{label} exceeds SQLite integer range")))
}

fn increment(value: u64, label: &str) -> Result<u64, StorageError> {
    value
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidData(format!("{label} exhausted")))
}

#[cfg(test)]
mod tests {
    use crate::AgentTurnDisposition;
    use chrono::TimeZone;
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentTurnResult, ConfidenceEstimate, ExecutivePlanner,
        LeaseBudget, ModelCallEvidence, OrganizationGenome, OrganizationalMetrics, PlannedWork,
        PlanningRequest, RoleKind, SuccessCondition, TaskComplexity,
    };
    use mcg_contracts::{
        ConfigurationId, ContextManifest, ContextManifestId, ContextSelection, ContractClause,
        HashAlgorithm, InvocationManifest, ModelRun, ModelRunId, ModelRunState, PolicyBinding,
        RetryPolicy, RuntimeIdentity, RuntimeInstanceId, SourceRef, VersionedRef,
        WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN, WaitReason,
    };
    use mcg_domain::{AgentInstanceId, Conversation, RunId};
    use mcg_domain::{
        PLUGIN_CATALOG_SCHEMA_VERSION, PluginCapabilityGrant, PluginCapabilityScope,
        PluginInstallSource, PluginInvocationId, PluginInvocationRecord, PluginInvocationState,
        PluginRevision, PluginRevisionId, PluginSignature, PluginSignatureState, ProjectId,
    };
    use mcg_plugin_protocol::{
        PLUGIN_SCHEMA_VERSION, PluginCapability, PluginClass, PluginManifest,
    };
    use mcg_projects::ProjectDiscoverer;
    use mcg_providers::{
        CONTRACT_VERSION, CostEstimate, ModelResponse, PlannedRoute, PriceProvenance,
        ProviderModelRef, Usage,
    };

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap()
    }

    fn policy(label: &str) -> PolicyBinding {
        let payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.workflow_policy.v1").unwrap(),
            &serde_json::json!({"policy": label}),
        )
        .unwrap();
        let content_hash = payload.content_hash("TestWorkflowPolicy").unwrap();
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    fn plugin_revision(at: DateTime<Utc>) -> PluginRevision {
        PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.workflow-plugin".into(),
                display_name: "Workflow plugin fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: "a".repeat(64),
            module_size_bytes: 1_024,
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: at,
        }
    }

    fn hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.workflow_ref.v1").unwrap(),
            &serde_json::json!({"label": label}),
        )
        .unwrap()
        .content_hash("TestWorkflowRef")
        .unwrap()
    }

    fn fixture() -> (TaskContract, WorkflowAttempt) {
        let task_id = TaskContractId::new();
        let source = SourceRef {
            source_type: StableTypeId::parse("test.user_intent.v1").unwrap(),
            locator: "conversation:test".to_owned(),
            content_hash: hash("intent"),
        };
        let clause = ContractClause {
            clause_id: "criterion-1".to_owned(),
            statement: "result is durable".to_owned(),
            source_refs: vec![source.clone()],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source,
            objective: "create a durable workflow".to_owned(),
            non_goals: Vec::new(),
            requirements: vec![clause.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![clause],
            dependencies: Vec::new(),
            permitted_side_effects: policy("none"),
            authority: policy("root"),
            environment: policy("test"),
            risk: policy("low"),
            budgets: policy("bounded"),
            rollback: policy("not-required"),
            status: TaskContractStatus::Active,
            created_at: now(),
        };
        let placeholder = ContentHash {
            algorithm: HashAlgorithm::Blake3_256,
            domain: TASK_CONTRACT_HASH_DOMAIN.to_owned(),
            digest_hex: "0".repeat(64),
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: placeholder,
            },
            parent_attempt_id: None,
            attempt_number: 1,
            state: WorkflowState::Ready,
            revision: 1,
            lease_owner: None,
            lease_generation: 0,
            lease_expires_at: None,
            heartbeat_at: None,
            checkpoint: None,
            environment_snapshot: hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: hash("configuration"),
            },
            idempotency_key: "create-workflow-1".to_owned(),
            retry_policy: RetryPolicy {
                max_attempts: 3,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: policy("reconcile"),
            created_at: now(),
            updated_at: now(),
        };
        attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
        (contract, attempt)
    }

    fn source_attachment(attempt: &WorkflowAttempt, locator: &str) -> WorkflowSourceAttachment {
        let source_type = StableTypeId::parse("mcg.agent_turn_source.v1").unwrap();
        let source_snapshot = VersionedPayload::canonical_v1(
            source_type.clone(),
            &serde_json::json!({
                "lease_id": locator,
                "task_snapshot_sha256": "a".repeat(64),
                "run_snapshot_sha256": "b".repeat(64),
                "lease_snapshot_sha256": "c".repeat(64),
            }),
        )
        .unwrap();
        WorkflowSourceAttachment {
            schema_version: WorkflowSourceAttachment::SCHEMA_VERSION,
            source: SourceRef {
                source_type,
                locator: locator.to_owned(),
                content_hash: source_snapshot
                    .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
                    .unwrap(),
            },
            source_snapshot,
            workflow_id: attempt.workflow_id,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            attached_at: now(),
        }
    }

    fn waiting_fixture(
        store: &mut SqliteEventStore,
        reason: WaitReason,
        key: &str,
    ) -> WorkflowAttempt {
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(5),
                CorrelationId::new(),
                &format!("{key}-claim"),
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Waiting(reason),
                CorrelationId::new(),
                &format!("{key}-wait"),
                now(),
            )
            .unwrap()
    }

    fn wake_condition(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.workflow_wake_condition.v1").unwrap(),
            locator: format!("condition:{label}"),
            content_hash: hash(label),
        }
    }

    #[test]
    fn workflow_create_is_atomic_and_exact_replay_is_idempotent() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        let correlation = CorrelationId::new();
        let event = store
            .create_workflow(&contract, &attempt, correlation, now())
            .unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.attempt_revision, 1);
        assert_eq!(event.lease_generation, 0);
        assert_eq!(
            store.task_contract_version(contract.id, 1).unwrap(),
            Some(contract.clone())
        );
        assert_eq!(
            store.workflow_attempt(attempt.attempt_id).unwrap(),
            Some(attempt.clone())
        );
        assert_eq!(
            store
                .create_workflow(&contract, &attempt, correlation, now())
                .unwrap(),
            event
        );
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 1);
    }

    #[test]
    fn workflow_event_pages_are_bounded_contiguous_and_cursor_fenced() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(5),
                CorrelationId::new(),
                "event-page-claim",
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                claimed.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Failed,
                CorrelationId::new(),
                "event-page-fail",
                now(),
            )
            .unwrap();

        let (first, latest) = store
            .workflow_events_after(attempt.workflow_id, 0, 1)
            .unwrap();
        assert_eq!(latest, 3);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].sequence, 1);
        let (second, latest) = store
            .workflow_events_after(attempt.workflow_id, 1, 1)
            .unwrap();
        assert_eq!(latest, 3);
        assert_eq!(second[0].sequence, 2);
        let (tail, latest) = store
            .workflow_events_after(attempt.workflow_id, 2, 10)
            .unwrap();
        assert_eq!(latest, 3);
        assert_eq!(tail[0].sequence, 3);
        let (empty, latest) = store
            .workflow_events_after(attempt.workflow_id, 3, 10)
            .unwrap();
        assert_eq!(latest, 3);
        assert!(empty.is_empty());
        assert!(
            store
                .workflow_events_after(attempt.workflow_id, 4, 10)
                .is_err()
        );
        assert!(
            store
                .workflow_events_after(attempt.workflow_id, 0, 0)
                .is_err()
        );
        assert!(
            store
                .workflow_events_after(attempt.workflow_id, 0, MAX_WORKFLOW_EVENT_PAGE_SIZE + 1,)
                .is_err()
        );
    }

    #[test]
    fn attached_workflow_is_atomic_immutable_and_exactly_replayable() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        let attachment = source_attachment(&attempt, "lease:legacy-1");
        let correlation = CorrelationId::new();

        let (event, stored) = store
            .create_attached_workflow(&contract, &attempt, &attachment, correlation, now())
            .unwrap();
        assert_eq!(stored, attachment);
        assert_eq!(
            store
                .workflow_source_attachment(&attachment.source.source_type, "lease:legacy-1")
                .unwrap(),
            Some(attachment.clone())
        );
        assert_eq!(
            store
                .workflow_source_attachment_for_workflow(attempt.workflow_id)
                .unwrap(),
            Some(attachment.clone())
        );
        assert_eq!(
            store
                .create_attached_workflow(&contract, &attempt, &attachment, correlation, now(),)
                .unwrap(),
            (event, attachment.clone())
        );
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 1);

        assert!(
            store
                .connection
                .execute(
                    "UPDATE workflow_source_attachments SET source_locator = ?1",
                    ["lease:mutated"],
                )
                .is_err()
        );
        assert!(
            store
                .connection
                .execute("DELETE FROM workflow_source_attachments", [])
                .is_err()
        );
    }

    #[test]
    fn broader_source_links_preserve_one_to_many_workflow_fanout() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (first_contract, first_attempt) = fixture();
        let first_attachment = source_attachment(&first_attempt, "lease:fanout-1");
        store
            .create_attached_workflow(
                &first_contract,
                &first_attempt,
                &first_attachment,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let (second_contract, second_attempt) = fixture();
        let second_attachment = source_attachment(&second_attempt, "lease:fanout-2");
        store
            .create_attached_workflow(
                &second_contract,
                &second_attempt,
                &second_attachment,
                CorrelationId::new(),
                now(),
            )
            .unwrap();

        let source_type = stable_type_id("mcg.compatibility_task_source.v1").unwrap();
        let snapshot = VersionedPayload::canonical_v1(
            source_type.clone(),
            &serde_json::json!({"task_id": "task:fanout", "task_sha256": "a".repeat(64)}),
        )
        .unwrap();
        let source = SourceRef {
            source_type: source_type.clone(),
            locator: "task:fanout".into(),
            content_hash: snapshot
                .content_hash(WORKFLOW_SOURCE_SNAPSHOT_HASH_DOMAIN)
                .unwrap(),
        };
        let first_link = WorkflowSourceLink {
            schema_version: WorkflowSourceLink::SCHEMA_VERSION,
            source: source.clone(),
            source_snapshot: snapshot.clone(),
            primary_source: first_attachment.source.clone(),
            workflow_id: first_attempt.workflow_id,
            attempt_id: first_attempt.attempt_id,
            task_contract: first_attempt.task_contract.clone(),
            linked_at: now(),
        };
        let second_link = WorkflowSourceLink {
            schema_version: WorkflowSourceLink::SCHEMA_VERSION,
            source,
            source_snapshot: snapshot,
            primary_source: second_attachment.source.clone(),
            workflow_id: second_attempt.workflow_id,
            attempt_id: second_attempt.attempt_id,
            task_contract: second_attempt.task_contract.clone(),
            linked_at: now() + chrono::Duration::seconds(1),
        };
        let transaction = store.connection.transaction().unwrap();
        insert_or_replay_workflow_source_link(&transaction, &first_link).unwrap();
        insert_or_replay_workflow_source_link(&transaction, &second_link).unwrap();
        transaction.commit().unwrap();

        let links = store
            .workflow_source_links(&source_type, "task:fanout")
            .unwrap();
        assert_eq!(links, vec![first_link, second_link]);
        assert!(
            store
                .connection
                .execute("DELETE FROM workflow_source_links", [])
                .is_err()
        );
    }

    #[test]
    fn conflicting_source_attachment_rolls_back_the_entire_new_workflow() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (first_contract, first_attempt) = fixture();
        let first_attachment = source_attachment(&first_attempt, "lease:legacy-conflict");
        store
            .create_attached_workflow(
                &first_contract,
                &first_attempt,
                &first_attachment,
                CorrelationId::new(),
                now(),
            )
            .unwrap();

        let (second_contract, second_attempt) = fixture();
        let conflicting_attachment = source_attachment(&second_attempt, "lease:legacy-conflict");
        let error = store
            .create_attached_workflow(
                &second_contract,
                &second_attempt,
                &conflicting_attachment,
                CorrelationId::new(),
                now(),
            )
            .unwrap_err();
        assert!(matches!(error, StorageError::Conflict(_)));
        assert_eq!(
            store.task_contract_version(second_contract.id, 1).unwrap(),
            None
        );
        assert_eq!(
            store.workflow_attempt(second_attempt.attempt_id).unwrap(),
            None
        );
        assert!(
            store
                .workflow_events(second_attempt.workflow_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .workflow_source_attachment(
                    &first_attachment.source.source_type,
                    "lease:legacy-conflict",
                )
                .unwrap(),
            Some(first_attachment)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // The full persisted authority chain is intentional evidence.
    fn active_plugin_invocation_is_attached_claimed_and_revalidated() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let at = now();
        let revision = plugin_revision(at);
        let installation = store
            .install_unsigned_local_plugin_revision(&revision)
            .expect("install plugin");
        let installation = store
            .set_plugin_enabled(
                &revision.manifest.id,
                installation.revision,
                true,
                at + chrono::Duration::seconds(1),
            )
            .expect("enable plugin");
        let invocation = PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: revision.manifest.id.clone(),
            plugin_revision_id: revision.id,
            task_id: None,
            input_sha256: "b".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: at + chrono::Duration::seconds(2),
            completed_at: None,
        };
        store
            .begin_plugin_invocation(&invocation)
            .expect("begin invocation");
        let owner = RuntimeActorId::new();
        let authority = store
            .claim_active_plugin_invocation_workflow(
                invocation.id,
                owner,
                300,
                at + chrono::Duration::seconds(3),
            )
            .expect("claim canonical plugin workflow");
        assert_eq!(authority.invocation, invocation);
        assert_eq!(authority.revision, revision);
        assert_eq!(authority.workflow.attempt.state, WorkflowState::Running);
        assert_eq!(authority.workflow.attempt.lease_owner, Some(owner));
        assert_eq!(
            authority.workflow.attachment.source.locator,
            invocation.id.to_string()
        );
        assert_eq!(
            authority.workflow.task_contract.user_intent_ref,
            authority.workflow.attachment.source
        );

        let replay = store
            .claim_active_plugin_invocation_workflow(
                invocation.id,
                owner,
                300,
                at + chrono::Duration::seconds(4),
            )
            .expect("same-owner replay");
        assert_eq!(replay.workflow, authority.workflow);
        assert!(
            store
                .claim_active_plugin_invocation_workflow(
                    invocation.id,
                    RuntimeActorId::new(),
                    300,
                    at + chrono::Duration::seconds(4),
                )
                .is_err()
        );

        let mut unsupported_success = invocation.clone();
        unsupported_success.state = PluginInvocationState::Succeeded;
        unsupported_success.result_sha256 = Some("c".repeat(64));
        unsupported_success.completed_at = Some(at + chrono::Duration::seconds(5));
        assert!(
            store
                .complete_plugin_invocation_workflow(
                    &unsupported_success,
                    owner,
                    at + chrono::Duration::seconds(5),
                )
                .is_err()
        );
        assert_eq!(
            store
                .plugin_invocation(invocation.id)
                .expect("plugin invocation")
                .expect("running plugin")
                .state,
            PluginInvocationState::Running
        );
        assert_eq!(
            store
                .workflow_attempt(authority.workflow.attempt.attempt_id)
                .expect("workflow attempt")
                .expect("running workflow")
                .state,
            WorkflowState::Running
        );

        store
            .set_plugin_enabled(
                &revision.manifest.id,
                installation.revision,
                false,
                at + chrono::Duration::seconds(5),
            )
            .expect("disable plugin");
        assert!(
            store
                .claim_active_plugin_invocation_workflow(
                    invocation.id,
                    owner,
                    300,
                    at + chrono::Duration::seconds(6),
                )
                .is_err()
        );

        let mut completed = invocation.clone();
        completed.state = PluginInvocationState::Failed;
        completed.error_code = Some("authorization_denied".into());
        completed.completed_at = Some(at + chrono::Duration::seconds(7));
        let (stored, settled) = store
            .complete_plugin_invocation_workflow(
                &completed,
                owner,
                at + chrono::Duration::seconds(7),
            )
            .expect("atomically fail plugin workflow");
        assert_eq!(stored, completed);
        assert_eq!(settled.state, WorkflowState::Failed);
        assert_eq!(
            store
                .complete_plugin_invocation_workflow(
                    &completed,
                    owner,
                    at + chrono::Duration::seconds(7),
                )
                .expect("exact terminal replay"),
            (completed, settled)
        );
    }

    #[test]
    fn cancelled_plugin_invocation_settles_only_as_non_successful() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let at = now();
        let revision = plugin_revision(at);
        let installation = store
            .install_unsigned_local_plugin_revision(&revision)
            .expect("install plugin");
        store
            .set_plugin_enabled(
                &revision.manifest.id,
                installation.revision,
                true,
                at + chrono::Duration::seconds(1),
            )
            .expect("enable plugin");
        let invocation = PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: revision.manifest.id.clone(),
            plugin_revision_id: revision.id,
            task_id: None,
            input_sha256: "d".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: at + chrono::Duration::seconds(2),
            completed_at: None,
        };
        store
            .begin_plugin_invocation(&invocation)
            .expect("begin invocation");
        let owner = RuntimeActorId::new();
        let authority = store
            .claim_active_plugin_invocation_workflow(
                invocation.id,
                owner,
                300,
                at + chrono::Duration::seconds(3),
            )
            .expect("claim plugin workflow");
        let cancelling = store
            .transition_workflow_attempt(
                authority.workflow.attempt.attempt_id,
                authority.workflow.attempt.revision,
                authority.workflow.attempt.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-plugin-before-result",
                at + chrono::Duration::seconds(4),
            )
            .expect("fence plugin workflow");
        assert_eq!(cancelling.state, WorkflowState::CancelRequested);

        let mut stale_success = invocation.clone();
        stale_success.state = PluginInvocationState::Succeeded;
        stale_success.result_sha256 = Some("e".repeat(64));
        stale_success.completed_at = Some(at + chrono::Duration::seconds(5));
        assert!(
            store
                .complete_plugin_invocation_workflow(
                    &stale_success,
                    owner,
                    at + chrono::Duration::seconds(5),
                )
                .is_err()
        );

        let mut cancelled = invocation;
        cancelled.state = PluginInvocationState::Failed;
        cancelled.error_code = Some("workflow_cancelled".into());
        cancelled.completed_at = Some(at + chrono::Duration::seconds(5));
        let (stored, settled) = store
            .complete_plugin_invocation_workflow(
                &cancelled,
                owner,
                at + chrono::Duration::seconds(5),
            )
            .expect("settle cancelled plugin workflow");
        assert_eq!(stored, cancelled);
        assert_eq!(settled.state, WorkflowState::Cancelled);
        assert_eq!(settled.lease_generation, cancelling.lease_generation);
    }

    #[test]
    fn plugin_broker_authority_rejects_a_changed_project_snapshot() {
        let mut store = SqliteEventStore::open_in_memory().expect("store");
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("proof.txt"), "first").expect("project fixture");
        std::fs::write(
            project_root.path().join("Cargo.toml"),
            "[package]\nname='first'\nversion='0.1.0'\n",
        )
        .expect("project manifest");
        let project_id = ProjectId::new();
        let project =
            ProjectDiscoverer::discover(project_root.path(), project_id).expect("discover project");
        store
            .record_project_snapshot(&project)
            .expect("record project");
        let mut task = TaskEnvelope::new("exercise source-bound plugin authority");
        task.project_id = Some(project_id);
        store.create_task(&task).expect("create task");
        let at = now();
        let mut revision = plugin_revision(at);
        revision.manifest.requested_capabilities = vec![PluginCapability::FilesystemRead];
        let installation = store
            .install_unsigned_local_plugin_revision(&revision)
            .expect("install plugin");
        let installation = store
            .set_plugin_capability_grants(
                &revision.manifest.id,
                installation.revision,
                vec![PluginCapabilityGrant {
                    schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemRead,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
                at + chrono::Duration::seconds(1),
            )
            .expect("grant project read");
        store
            .set_plugin_enabled(
                &revision.manifest.id,
                installation.revision,
                true,
                at + chrono::Duration::seconds(2),
            )
            .expect("enable plugin");
        let invocation = PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: revision.manifest.id.clone(),
            plugin_revision_id: revision.id,
            task_id: Some(task.id),
            input_sha256: "b".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: at + chrono::Duration::seconds(3),
            completed_at: None,
        };
        store
            .begin_plugin_invocation(&invocation)
            .expect("begin invocation");
        let owner = RuntimeActorId::new();
        let authority = store
            .claim_active_plugin_invocation_workflow(
                invocation.id,
                owner,
                300,
                at + chrono::Duration::seconds(4),
            )
            .expect("claim source-bound plugin workflow");
        assert_eq!(authority.task, Some(task));
        assert_eq!(authority.project, Some(project));

        std::fs::write(
            project_root.path().join("Cargo.toml"),
            "[package]\nname='changed'\nversion='0.1.0'\n",
        )
        .expect("change project fixture");
        let changed = ProjectDiscoverer::discover(project_root.path(), project_id)
            .expect("rediscover changed project");
        store
            .record_project_snapshot(&changed)
            .expect("record changed project");
        let error = store
            .claim_active_plugin_invocation_workflow(
                invocation.id,
                owner,
                300,
                at + chrono::Duration::seconds(5),
            )
            .expect_err("changed project must invalidate plugin broker authority");
        assert!(matches!(error, StorageError::Conflict(_)));
        assert_eq!(
            store
                .workflow_attempt(authority.workflow.attempt.attempt_id)
                .expect("workflow attempt")
                .expect("running workflow")
                .state,
            WorkflowState::Running
        );
    }

    #[test]
    fn active_agent_lease_is_attached_from_exact_authoritative_source_rows() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("approval-wake.sqlite3");
        let mut store = SqliteEventStore::open(&database).unwrap();
        let observed_at = now();
        let task = TaskEnvelope::new("complete the compatibility agent turn");
        store.create_task(&task).unwrap();
        let task = store.transition_task(task.id, TaskState::Active).unwrap().0;
        let organization = OrganizationGenome::consumer_default(observed_at);
        store.record_organization_revision(&organization).unwrap();
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![
                PlannedWork::atomic(
                    "produce one governed answer",
                    RoleKind::Coder,
                    vec![SuccessCondition::Custom {
                        description: "return verified output".into(),
                    }],
                ),
                PlannedWork::atomic(
                    "exercise a governed failure",
                    RoleKind::Coder,
                    vec![SuccessCondition::Custom {
                        description: "retain exact failure evidence".into(),
                    }],
                ),
                PlannedWork::atomic(
                    "reject a stale governed settlement",
                    RoleKind::Coder,
                    vec![SuccessCondition::Custom {
                        description: "preserve the legacy authority on rollback".into(),
                    }],
                ),
            ],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&organization, &planning, observed_at)
            .unwrap();
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: organization.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: observed_at,
            updated_at: observed_at,
            completed_at: None,
        };
        store.create_agent_run(&run, &plan).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO task_launches(task_id, run_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    task.id.to_string(),
                    run.id.to_string(),
                    observed_at.to_rfc3339()
                ],
            )
            .unwrap();
        let run = store
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                observed_at,
            )
            .unwrap()
            .0;
        let conversation = Conversation::new("compatibility conversation", None);
        store.create_conversation(&conversation, None).unwrap();
        store
            .save_codex_thread_binding(&CodexThreadBinding {
                conversation_id: Some(conversation.id),
                conversation_turn_id: None,
                run_id: Some(run.id),
                agent_instance_id: None,
                agent_session_id: None,
                delegation_id: None,
                role_id: None,
                organization_revision_id: None,
                workspace_id: None,
                codex_thread_id: "codex-thread:workflow-source-link".into(),
                codex_turn_id: None,
                codex_item_id: None,
                codex_session_id: None,
                binary_version: "test".into(),
                protocol_version: "test".into(),
                ownership_source: Some("test".into()),
                manageable: true,
                last_reconciled_item_id: None,
                recovery_state: Default::default(),
            })
            .unwrap();
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:compatibility-context".into(),
            confidence: ConfidenceEstimate::new(5_000, 0).unwrap(),
            created_at: observed_at,
            updated_at: observed_at,
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
            tool_permissions: vec!["project.read".into()],
            reporting_interval_seconds: 30,
            max_retries: 2,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: observed_at,
            expires_at: observed_at
                + chrono::Duration::seconds(
                    i64::try_from(organization.policy.default_lease_seconds).unwrap(),
                ),
            heartbeat_at: observed_at,
        };
        let (running_instance, _) = store
            .dispatch_agent_work(&instance, &lease, observed_at)
            .unwrap();
        assert_eq!(running_instance.state, AgentState::Running);

        let receipt = store
            .attach_active_agent_lease_workflow(lease.id, observed_at)
            .unwrap();
        assert_eq!(receipt.task_contract.objective, task.goal);
        assert_eq!(receipt.task_contract.acceptance_criteria.len(), 1);
        assert_eq!(
            receipt.task_contract.acceptance_criteria[0].clause_id,
            "legacy-work-condition-1"
        );
        assert!(
            receipt.task_contract.acceptance_criteria[0]
                .statement
                .contains("return verified output")
        );
        assert_eq!(receipt.attempt.state, WorkflowState::Ready);
        assert_eq!(
            receipt.attempt.task_contract,
            receipt.attachment.task_contract
        );
        assert_eq!(
            receipt.attachment.source.source_type.as_str(),
            "mcg.agent_turn_source.v1"
        );
        assert_eq!(receipt.attachment.source.locator, lease.id.to_string());
        assert_eq!(receipt.source_links.len(), 3);
        let task_source_type = stable_type_id("mcg.compatibility_task_source.v1").unwrap();
        let run_source_type = stable_type_id("mcg.compatibility_agent_run_source.v1").unwrap();
        let conversation_source_type =
            stable_type_id("mcg.compatibility_conversation_source.v1").unwrap();
        assert_eq!(
            store
                .workflow_source_links(&task_source_type, &task.id.to_string())
                .unwrap(),
            vec![
                receipt
                    .source_links
                    .iter()
                    .find(|link| link.source.source_type == task_source_type)
                    .unwrap()
                    .clone()
            ]
        );
        assert_eq!(
            store
                .workflow_source_links(&conversation_source_type, &conversation.id.to_string(),)
                .unwrap(),
            vec![
                receipt
                    .source_links
                    .iter()
                    .find(|link| link.source.source_type == conversation_source_type)
                    .unwrap()
                    .clone()
            ]
        );
        assert_eq!(
            store
                .workflow_source_links(&run_source_type, &run.id.to_string())
                .unwrap(),
            vec![
                receipt
                    .source_links
                    .iter()
                    .find(|link| link.source.source_type == run_source_type)
                    .unwrap()
                    .clone()
            ]
        );
        assert!(
            receipt
                .source_links
                .iter()
                .all(|link| link.primary_source == receipt.attachment.source)
        );
        assert_eq!(receipt.creation_event.sequence, 1);
        assert_eq!(
            store
                .attach_active_agent_lease_workflow(
                    lease.id,
                    observed_at + chrono::Duration::hours(1),
                )
                .unwrap(),
            receipt
        );
        store
            .connection
            .execute_batch(
                "DROP TRIGGER workflow_source_links_no_delete;
                 DELETE FROM workflow_source_links;
                 CREATE TRIGGER workflow_source_links_no_delete
                 BEFORE DELETE ON workflow_source_links BEGIN
                     SELECT RAISE(FAIL, 'workflow source links are durable evidence');
                 END;",
            )
            .unwrap();
        drop(store);
        let mut store = SqliteEventStore::open(&database).unwrap();

        let recovery = store
            .recover_compatibility_workflow_source_links(observed_at)
            .unwrap();
        assert_eq!(recovery.inspected_active_attachments, 1);
        assert_eq!(recovery.reattached_workflows, 1);
        assert_eq!(recovery.inserted_links, 3);
        assert_eq!(
            store
                .recover_compatibility_workflow_source_links(observed_at)
                .unwrap(),
            WorkflowSourceLinkRecoveryReport {
                inspected_active_attachments: 0,
                reattached_workflows: 0,
                inserted_links: 0,
            },
            "startup replay must not duplicate immutable lookup evidence"
        );

        let owner = RuntimeActorId::new();
        let authority = store
            .claim_active_agent_lease_workflow(lease.id, owner, 120, observed_at)
            .unwrap();
        assert_eq!(authority.legacy_lease, lease);
        assert_eq!(authority.workflow.attempt.state, WorkflowState::Running);
        assert_eq!(authority.workflow.attempt.lease_owner, Some(owner));
        assert_eq!(authority.workflow.attempt.lease_generation, 1);
        assert_eq!(
            authority.workflow.source_links, receipt.source_links,
            "startup recovery backfills pre-schema-62 source links before the next claim"
        );
        assert_eq!(
            store
                .claim_active_agent_lease_workflow(
                    lease.id,
                    owner,
                    120,
                    observed_at + chrono::Duration::seconds(1),
                )
                .unwrap()
                .workflow
                .attempt,
            authority.workflow.attempt
        );
        assert!(matches!(
            store.claim_active_agent_lease_workflow(
                lease.id,
                RuntimeActorId::new(),
                120,
                observed_at + chrono::Duration::seconds(1),
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(
            store
                .claim_active_agent_lease_workflow(lease.id, owner, 120, lease.expires_at)
                .is_err()
        );
        assert_eq!(
            store
                .workflow_attempt(authority.workflow.attempt.attempt_id)
                .unwrap()
                .unwrap(),
            authority.workflow.attempt,
            "failed revalidation cannot mutate canonical authority"
        );

        let tool_request_id = ToolRequestId::new();
        let approval = ApprovalRequest {
            schema_version: mcg_tools::TOOL_SCHEMA_VERSION,
            id: ApprovalId::new(),
            tool_request_id,
            task_id: task.id,
            action: "fixture.write".into(),
            resource: "fixture.txt".into(),
            reason: "exercise an exact canonical approval wake".into(),
            origin: None,
            state: ApprovalState::Pending,
            requested_at: observed_at + chrono::Duration::seconds(1),
            expires_at: observed_at + chrono::Duration::days(30),
        };
        store.create_approval(&approval).unwrap();
        let checkpoint_json = serde_json::json!({
            "schema_version": AGENT_RUN_SCHEMA_VERSION,
            "request": {
                "instance": {
                    "id": instance.id,
                    "run_id": run.id,
                    "task_id": task.id,
                },
                "lease": {
                    "id": lease.id,
                    "instance_id": instance.id,
                },
            },
            "pending_tool_request_id": tool_request_id,
        });
        store
            .connection
            .execute(
                "INSERT INTO agent_turn_checkpoints(
                    lease_id, run_id, instance_id, task_id, pending_tool_request_id,
                    checkpoint_json, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    lease.id.to_string(),
                    run.id.to_string(),
                    instance.id.to_string(),
                    task.id.to_string(),
                    tool_request_id.to_string(),
                    serde_json::to_string(&checkpoint_json).unwrap(),
                    (observed_at + chrono::Duration::seconds(1)).to_rfc3339(),
                ],
            )
            .unwrap();
        store
            .settle_agent_turn_error(
                instance.id,
                lease.id,
                AgentTurnDisposition::Waiting,
                "approval_required",
                observed_at + chrono::Duration::seconds(2),
            )
            .expect("atomically wait in both projections");
        let waiting = store
            .workflow_attempt(authority.workflow.attempt.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(waiting.state, WorkflowState::Waiting(WaitReason::Approval));
        assert_eq!(waiting.revision, 3);
        assert_eq!(waiting.lease_generation, 1);
        assert!(waiting.lease_owner.is_none());
        let wait_event = store
            .workflow_events(waiting.workflow_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(wait_event.type_id.as_str(), WORKFLOW_WAIT_BOUND_TYPE_ID);
        let wait_payload = event_payload_value(&wait_event).unwrap();
        assert_eq!(
            wait_payload["approval_wait"]["approval_id"],
            serde_json::json!(approval.id)
        );
        drop(store);
        let mut store = SqliteEventStore::open(&database).unwrap();

        assert!(matches!(
            store.resume_launched_agent_turn(task.id, observed_at + chrono::Duration::seconds(3)),
            Err(StorageError::Conflict(_))
        ));
        assert_eq!(
            store.workflow_attempt(waiting.attempt_id).unwrap().unwrap(),
            waiting,
            "an unresolved approval cannot wake canonical work"
        );
        store
            .resolve_approval(&mcg_tools::ApprovalResolution {
                approval_id: approval.id,
                state: ApprovalState::Approved,
                resolved_by: "fixture-owner".into(),
                reason: Some("approved for the wake test".into()),
                resolved_at: observed_at + chrono::Duration::seconds(2),
            })
            .unwrap();
        let valid_resolved_at = (observed_at + chrono::Duration::seconds(2)).to_rfc3339();
        store
            .connection
            .execute(
                "UPDATE approvals SET resolved_at = ?1 WHERE approval_id = ?2",
                params![
                    (observed_at + chrono::Duration::days(31)).to_rfc3339(),
                    approval.id.to_string()
                ],
            )
            .unwrap();
        assert!(matches!(
            store.resume_launched_agent_turn(task.id, observed_at + chrono::Duration::seconds(3)),
            Err(StorageError::Conflict(_))
        ));
        store
            .connection
            .execute(
                "UPDATE approvals SET resolved_at = ?1 WHERE approval_id = ?2",
                params![valid_resolved_at, approval.id.to_string()],
            )
            .unwrap();
        let approved_json: String = store
            .connection
            .query_row(
                "SELECT approval_json FROM approvals WHERE approval_id = ?1",
                [approval.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        let mut substituted: ApprovalRequest = serde_json::from_str(&approved_json).unwrap();
        substituted.action = "substituted.action".into();
        store
            .connection
            .execute(
                "UPDATE approvals SET approval_json = ?1 WHERE approval_id = ?2",
                params![
                    serde_json::to_string(&substituted).unwrap(),
                    approval.id.to_string()
                ],
            )
            .unwrap();
        assert!(matches!(
            store.resume_launched_agent_turn(task.id, observed_at + chrono::Duration::seconds(3)),
            Err(StorageError::Conflict(_))
        ));
        assert_eq!(
            store.workflow_attempt(waiting.attempt_id).unwrap().unwrap(),
            waiting,
            "substituted approval evidence cannot wake canonical work"
        );
        store
            .connection
            .execute(
                "UPDATE approvals SET approval_json = ?1 WHERE approval_id = ?2",
                params![approved_json, approval.id.to_string()],
            )
            .unwrap();
        store
            .resume_launched_agent_turn(task.id, observed_at + chrono::Duration::seconds(3))
            .expect("atomically satisfy the canonical wait and resume legacy projections");
        let ready = store.workflow_attempt(waiting.attempt_id).unwrap().unwrap();
        assert_eq!(ready.state, WorkflowState::Ready);
        assert_eq!(ready.revision, 4);
        assert_eq!(ready.lease_generation, 1);
        assert!(ready.lease_owner.is_none());
        let resumed = store
            .claim_active_agent_lease_workflow(
                lease.id,
                owner,
                120,
                observed_at + chrono::Duration::seconds(3),
            )
            .expect("atomically resume and reclaim canonical work");
        assert_eq!(resumed.workflow.attempt.state, WorkflowState::Running);
        assert_eq!(resumed.workflow.attempt.revision, 5);
        assert_eq!(resumed.workflow.attempt.lease_generation, 2);
        assert_eq!(resumed.workflow.attempt.lease_owner, Some(owner));
        let resume_events = store
            .workflow_events(resumed.workflow.attempt.workflow_id)
            .unwrap();
        assert_eq!(
            resume_events[resume_events.len() - 2].type_id.as_str(),
            WORKFLOW_WAIT_SATISFIED_TYPE_ID
        );
        let replayed = store
            .claim_active_agent_lease_workflow(
                lease.id,
                owner,
                120,
                observed_at + chrono::Duration::seconds(4),
            )
            .expect("same-owner wake replay");
        assert_eq!(replayed.workflow.attempt, resumed.workflow.attempt);
        assert_eq!(
            store
                .workflow_events(resumed.workflow.attempt.workflow_id)
                .unwrap(),
            resume_events,
            "wake replay cannot append or redispatch"
        );

        let target = ProviderModelRef {
            provider_id: "fixture-provider".into(),
            model_id: "fixture-model".into(),
        };
        let response = ModelResponse {
            contract_version: CONTRACT_VERSION,
            id: "fixture-response".into(),
            model: target.model_id.clone(),
            status: Some("completed".into()),
            output: Vec::new(),
            usage: Usage {
                input_tokens: 8,
                output_tokens: 4,
                total_tokens: 12,
                ..Usage::default()
            },
            provider_metadata: serde_json::Value::Null,
        };
        let result = AgentTurnResult {
            pinned_route: PlannedRoute {
                target: target.clone(),
                estimated_cost: CostEstimate {
                    amount_usd: None,
                    provenance: PriceProvenance::Unknown,
                },
            },
            response: response.clone(),
            model_calls: vec![ModelCallEvidence {
                target,
                response_id: response.id.clone(),
                usage: response.usage.clone(),
                cost_usd: None,
                duration_ms: 10,
            }],
            tool_outcomes: Vec::new(),
        };
        store
            .commit_agent_turn(
                instance.id,
                lease.id,
                &result,
                instance.confidence,
                None,
                observed_at + chrono::Duration::seconds(4),
            )
            .expect("atomically persist Work and begin Verify");
        let verifying = store
            .workflow_attempt(authority.workflow.attempt.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(verifying.state, WorkflowState::Verifying);
        assert_eq!(verifying.revision, 6);
        assert_eq!(verifying.lease_generation, 2);
        assert_eq!(verifying.lease_owner, Some(owner));
        assert_eq!(
            store.workflow_events(verifying.workflow_id).unwrap().len(),
            6
        );
        assert!(matches!(
            store.agent_turn_verification_targets(run.id),
            Err(StorageError::Conflict(_))
        ));

        let evidence_source = SourceRef {
            source_type: StableTypeId::parse("test.compatibility_work_source.v1").unwrap(),
            locator: lease.id.to_string(),
            content_hash: hash("compatibility-work-source"),
        };
        let context = ContextManifest {
            id: ContextManifestId::new(),
            attempt_id: verifying.attempt_id,
            selected_sources: vec![ContextSelection {
                source: evidence_source.clone(),
                perspective: None,
                influence: VersionedPayload::canonical_v1(
                    StableTypeId::parse("test.compatibility_influence.v1").unwrap(),
                    &serde_json::json!({"influence": "trusted"}),
                )
                .unwrap(),
                transformations: Vec::new(),
                placement: 0,
                approximate_tokens: 1,
            }],
            selection_policy: VersionedPayload::canonical_v1(
                StableTypeId::parse("test.compatibility_context_policy.v1").unwrap(),
                &serde_json::json!({"maximum_tokens": 1}),
            )
            .unwrap(),
            expansion_steps: Vec::new(),
            approximate_tokens: 1,
        };
        let context_hash = store
            .persist_context_manifest(&context, observed_at)
            .expect("persist compatibility context");
        let runtime_build = hash("compatibility-runtime-build");
        let invocation = InvocationManifest {
            id: InvocationId::new(),
            role: InvocationRole::Work,
            attempt_id: verifying.attempt_id,
            task_contract: verifying.task_contract.clone(),
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId::new(),
                build_hash: runtime_build.clone(),
                protocol_version: mcg_contracts::PROTOCOL_V1,
                started_at: observed_at,
            },
            cognitive_configuration: hash("compatibility-cognition"),
            role_manifest: hash("compatibility-work-role"),
            prompt_bundle: hash("compatibility-prompt"),
            context_manifest: context_hash,
            model_route: VersionedPayload::canonical_v1(
                StableTypeId::parse("test.compatibility_model_route.v1").unwrap(),
                &serde_json::json!({"route": "fixture"}),
            )
            .unwrap(),
            resolved_provider: evidence_source.clone(),
            resolved_model: evidence_source.clone(),
            observed_model_fingerprint: Some("fixture-v1".into()),
            sampling: VersionedPayload::canonical_v1(
                StableTypeId::parse("test.compatibility_sampling.v1").unwrap(),
                &serde_json::json!({"temperature": 0}),
            )
            .unwrap(),
            reasoning: None,
            available_tool_manifests: Vec::new(),
            selected_skill_manifests: Vec::new(),
            authority_envelope: evidence_source.clone(),
            egress_permit: None,
            repository_revision: None,
            environment: evidence_source,
            runtime_build,
            request_hash: hash("compatibility-request"),
            response_hash: None,
        };
        store
            .persist_invocation_manifest(&invocation, observed_at)
            .expect("persist compatibility Work invocation");
        let planned_run = ModelRun {
            id: ModelRunId::new(),
            invocation_id: invocation.id,
            role: InvocationRole::Work,
            state: ModelRunState::Planned,
            provider_request_id: None,
            response_artifact: None,
            usage: None,
            observed_model_fingerprint: None,
            outcome_evidence: None,
            started_at: observed_at,
            finished_at: None,
        };
        store
            .create_model_run(&planned_run, observed_at)
            .expect("plan compatibility Work invocation");
        let mut dispatched_run = planned_run.clone();
        dispatched_run.state = ModelRunState::Dispatched;
        dispatched_run.provider_request_id = Some("fixture-request".into());
        store
            .transition_model_run(&dispatched_run, 1, observed_at)
            .expect("dispatch compatibility Work invocation");
        let work_response_hash = hash("compatibility-response");
        let mut completed_run = dispatched_run;
        completed_run.state = ModelRunState::Completed;
        completed_run.response_artifact = Some(ArtifactRef {
            content_hash: work_response_hash.clone(),
            media_type: "application/json".into(),
            byte_length: 32,
        });
        completed_run.finished_at = Some(observed_at + chrono::Duration::seconds(5));
        store
            .transition_model_run(
                &completed_run,
                2,
                observed_at + chrono::Duration::seconds(5),
            )
            .expect("complete compatibility Work invocation");
        let targets = store
            .agent_turn_verification_targets(run.id)
            .expect("load exact compatibility verification target");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].lease_id, lease.id);
        assert_eq!(targets[0].work_item_id, plan.items[0].id);
        assert_eq!(
            targets[0].success_conditions,
            plan.items[0].success_conditions
        );
        assert_eq!(targets[0].attempt, verifying);
        assert_eq!(targets[0].work_invocations, vec![invocation.id]);
        assert_eq!(
            targets[0].work_artifacts[0].content_hash,
            work_response_hash
        );
        assert_eq!(targets[0].work_response_hash, work_response_hash);

        let stale_instance = AgentInstance {
            id: AgentInstanceId::new(),
            role_id: plan.items[2].assigned_role,
            state: AgentState::Created,
            model: None,
            created_at: observed_at + chrono::Duration::seconds(5),
            updated_at: observed_at + chrono::Duration::seconds(5),
            ..instance.clone()
        };
        let stale_lease = AgentLease {
            id: AgentLeaseId::new(),
            instance_id: stale_instance.id,
            work_item_id: plan.items[2].id,
            goal: plan.items[2].goal.clone(),
            state: AgentLeaseState::Active,
            acquired_at: observed_at + chrono::Duration::seconds(5),
            expires_at: observed_at + chrono::Duration::minutes(5),
            heartbeat_at: observed_at + chrono::Duration::seconds(5),
            ..lease.clone()
        };
        store
            .dispatch_agent_work(
                &stale_instance,
                &stale_lease,
                observed_at + chrono::Duration::seconds(5),
            )
            .expect("dispatch stale-settlement fixture");
        let stale_authority = store
            .claim_active_agent_lease_workflow(
                stale_lease.id,
                owner,
                120,
                observed_at + chrono::Duration::seconds(5),
            )
            .expect("claim stale-settlement fixture");
        store
            .transition_workflow_attempt(
                stale_authority.workflow.attempt.attempt_id,
                stale_authority.workflow.attempt.revision,
                stale_authority.workflow.attempt.lease_generation,
                WorkflowState::Failed,
                CorrelationId::new(),
                "advance-before-legacy-settlement",
                observed_at + chrono::Duration::seconds(6),
            )
            .expect("advance canonical authority first");
        assert!(matches!(
            store.settle_agent_turn_error(
                stale_instance.id,
                stale_lease.id,
                AgentTurnDisposition::Failed,
                "late_provider_failure",
                observed_at + chrono::Duration::seconds(7),
            ),
            Err(StorageError::Conflict(_))
        ));
        assert_eq!(
            store
                .agent_instance(stale_instance.id)
                .unwrap()
                .unwrap()
                .state,
            AgentState::Running,
            "canonical conflict must roll back the legacy instance settlement"
        );
        assert_eq!(
            store.agent_lease(stale_lease.id).unwrap().unwrap().state,
            AgentLeaseState::Active,
            "canonical conflict must retain the legacy lease for recovery"
        );

        let failed_instance = AgentInstance {
            id: AgentInstanceId::new(),
            role_id: plan.items[1].assigned_role,
            state: AgentState::Created,
            model: None,
            created_at: observed_at + chrono::Duration::seconds(8),
            updated_at: observed_at + chrono::Duration::seconds(8),
            ..instance.clone()
        };
        let failed_lease = AgentLease {
            id: AgentLeaseId::new(),
            instance_id: failed_instance.id,
            work_item_id: plan.items[1].id,
            goal: plan.items[1].goal.clone(),
            state: AgentLeaseState::Active,
            acquired_at: observed_at + chrono::Duration::seconds(8),
            expires_at: observed_at + chrono::Duration::minutes(5),
            heartbeat_at: observed_at + chrono::Duration::seconds(8),
            ..lease.clone()
        };
        store
            .dispatch_agent_work(
                &failed_instance,
                &failed_lease,
                observed_at + chrono::Duration::seconds(8),
            )
            .expect("dispatch second compatibility item");
        let failed_authority = store
            .claim_active_agent_lease_workflow(
                failed_lease.id,
                owner,
                120,
                observed_at + chrono::Duration::seconds(8),
            )
            .expect("claim second canonical item");
        store
            .settle_agent_turn_error(
                failed_instance.id,
                failed_lease.id,
                AgentTurnDisposition::Failed,
                "provider_failure",
                observed_at + chrono::Duration::seconds(9),
            )
            .expect("atomically fail both projections");
        let failed = store
            .workflow_attempt(failed_authority.workflow.attempt.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(failed.state, WorkflowState::Failed);
        assert_eq!(failed.revision, 3);
        assert!(failed.lease_owner.is_none());
    }

    #[test]
    fn task_contract_v1_golden_fixture_hash_is_stable() {
        let contract: TaskContract = serde_json::from_str(include_str!(
            "../../mcg-contracts/fixtures/task-contract-v1.json"
        ))
        .expect("checked-in task-contract fixture");
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../mcg-contracts/fixtures/task-contract-v1.expected.json"
        ))
        .expect("checked-in task-contract expectation");
        let content_hash = task_contract_content_hash(&contract).unwrap();
        assert_eq!(TaskContract::TYPE_ID, expected["stable_type_id"]);
        assert_eq!(TaskContract::SCHEMA_VERSION, expected["schema_version"]);
        assert_eq!(content_hash.domain, expected["semantic_domain"]);
        assert_eq!(content_hash.digest_hex, expected["blake3_256"]);
    }

    #[test]
    fn task_contract_revisions_are_immutable_cas_bound_and_exactly_replayable() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        assert_eq!(
            store.task_contract_head(contract.id).unwrap().unwrap(),
            TaskContractHead {
                contract: contract.clone(),
                revision: 1,
            }
        );

        let mut revision = contract.clone();
        revision.version = 2;
        revision.previous_version = Some(VersionedRef {
            id: contract.id,
            version: contract.version,
            content_hash: task_contract_content_hash(&contract).unwrap(),
        });
        revision.objective = "create and revise a durable workflow".to_owned();
        revision.created_at = now() + chrono::Duration::seconds(1);
        let revised = store
            .revise_task_contract(&revision, 1, "contract-revision-2")
            .unwrap();
        assert_eq!(revised.revision, 2);
        assert_eq!(revised.contract, revision);
        assert_eq!(
            store.task_contract_version(contract.id, 1).unwrap(),
            Some(contract.clone())
        );
        assert_eq!(
            store.task_contract_head(contract.id).unwrap(),
            Some(revised.clone())
        );
        assert_eq!(
            store
                .revise_task_contract(&revision, 1, "contract-revision-2")
                .unwrap(),
            revised
        );

        let mut substituted = revision.clone();
        substituted.objective = "substituted content".to_owned();
        assert!(matches!(
            store.revise_task_contract(&substituted, 1, "contract-revision-2"),
            Err(StorageError::Conflict(_))
        ));

        let mut stale = revision.clone();
        stale.version = 3;
        stale.previous_version = Some(VersionedRef {
            id: revision.id,
            version: revision.version,
            content_hash: task_contract_content_hash(&revision).unwrap(),
        });
        assert!(matches!(
            store.revise_task_contract(&stale, 1, "contract-revision-3"),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn external_wake_is_exact_restart_safe_and_replay_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("external-wake.db");
        let at = now();
        let (contract, attempt) = fixture();
        let notifier = RuntimeActorId::new();
        let condition = wake_condition("external-complete");
        let (waiting, binding) = {
            let mut store = SqliteEventStore::open(&database).unwrap();
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), at)
                .unwrap();
            let claimed = store
                .claim_workflow_attempt(
                    attempt.attempt_id,
                    attempt.revision,
                    attempt.lease_generation,
                    RuntimeActorId::new(),
                    at + chrono::Duration::minutes(5),
                    CorrelationId::new(),
                    "external-wake-claim",
                    at,
                )
                .unwrap();
            assert!(matches!(
                store.transition_workflow_attempt(
                    claimed.attempt_id,
                    claimed.revision,
                    claimed.lease_generation,
                    WorkflowState::Waiting(WaitReason::External),
                    CorrelationId::new(),
                    "unbound-external-wait",
                    at,
                ),
                Err(StorageError::Conflict(_))
            ));
            let wait_request = WorkflowWaitRequest {
                attempt_id: claimed.attempt_id,
                expected_revision: claimed.revision,
                expected_generation: claimed.lease_generation,
                wait_reason: WaitReason::External,
                condition: condition.clone(),
                authorized_notifier: notifier,
                expires_at: at + chrono::Duration::minutes(10),
                idempotency_key: "bind-external-wake".into(),
            };
            let result = store
                .bind_workflow_wait(&wait_request, CorrelationId::new(), at)
                .unwrap();
            assert_eq!(
                store
                    .bind_workflow_wait(&wait_request, CorrelationId::new(), at)
                    .unwrap(),
                result
            );
            let mut substituted = wait_request;
            substituted.condition = wake_condition("substituted");
            assert!(matches!(
                store.bind_workflow_wait(&substituted, CorrelationId::new(), at),
                Err(StorageError::Conflict(_))
            ));
            result
        };
        assert_eq!(waiting.state, WorkflowState::Waiting(WaitReason::External));
        assert_eq!(binding.bound_revision, waiting.revision);

        let mut store = SqliteEventStore::open(&database).unwrap();
        let wake_request = WorkflowWakeRequest {
            attempt_id: waiting.attempt_id,
            expected_revision: waiting.revision,
            expected_generation: waiting.lease_generation,
            binding_hash: binding.content_hash().unwrap(),
            notifier,
            evidence: condition.clone(),
            idempotency_key: "wake-external".into(),
        };
        let mut wrong_notifier = wake_request.clone();
        wrong_notifier.notifier = RuntimeActorId::new();
        assert!(matches!(
            store.wake_workflow(
                &wrong_notifier,
                CorrelationId::new(),
                at + chrono::Duration::seconds(1)
            ),
            Err(StorageError::Conflict(_))
        ));
        let mut wrong_evidence = wake_request.clone();
        wrong_evidence.evidence = wake_condition("wrong-evidence");
        assert!(matches!(
            store.wake_workflow(
                &wrong_evidence,
                CorrelationId::new(),
                at + chrono::Duration::seconds(1)
            ),
            Err(StorageError::Conflict(_))
        ));
        let mut wrong_binding = wake_request.clone();
        wrong_binding.binding_hash.digest_hex = "b".repeat(64);
        assert!(matches!(
            store.wake_workflow(
                &wrong_binding,
                CorrelationId::new(),
                at + chrono::Duration::seconds(1)
            ),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            store.wake_workflow(
                &wake_request,
                CorrelationId::new(),
                at + chrono::Duration::minutes(11)
            ),
            Err(StorageError::Conflict(_))
        ));
        let ready = store
            .wake_workflow(
                &wake_request,
                CorrelationId::new(),
                at + chrono::Duration::seconds(2),
            )
            .unwrap();
        assert_eq!(ready.state, WorkflowState::Ready);
        assert_eq!(
            store
                .wake_workflow(
                    &wake_request,
                    CorrelationId::new(),
                    at + chrono::Duration::seconds(3)
                )
                .unwrap(),
            ready
        );
        let mut substituted_replay = wake_request;
        substituted_replay.expected_revision += 1;
        assert!(matches!(
            store.wake_workflow(
                &substituted_replay,
                CorrelationId::new(),
                at + chrono::Duration::seconds(3)
            ),
            Err(StorageError::Conflict(_))
        ));
        let events = store.workflow_events(ready.workflow_id).unwrap();
        assert_eq!(
            events.last().unwrap().type_id.as_str(),
            WORKFLOW_WAIT_SATISFIED_TYPE_ID
        );
    }

    #[test]
    fn protected_component_wake_requires_the_bound_notifier_and_evidence() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let at = now();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), at)
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                attempt.revision,
                attempt.lease_generation,
                RuntimeActorId::new(),
                at + chrono::Duration::minutes(5),
                CorrelationId::new(),
                "protected-wake-claim",
                at,
            )
            .unwrap();
        let notifier = RuntimeActorId::new();
        let condition = wake_condition("protected-complete");
        let (waiting, binding) = store
            .bind_workflow_wait(
                &WorkflowWaitRequest {
                    attempt_id: claimed.attempt_id,
                    expected_revision: claimed.revision,
                    expected_generation: claimed.lease_generation,
                    wait_reason: WaitReason::ProtectedComponent,
                    condition: condition.clone(),
                    authorized_notifier: notifier,
                    expires_at: at + chrono::Duration::minutes(10),
                    idempotency_key: "bind-protected-wake".into(),
                },
                CorrelationId::new(),
                at,
            )
            .unwrap();
        let ready = store
            .wake_workflow(
                &WorkflowWakeRequest {
                    attempt_id: waiting.attempt_id,
                    expected_revision: waiting.revision,
                    expected_generation: waiting.lease_generation,
                    binding_hash: binding.content_hash().unwrap(),
                    notifier,
                    evidence: condition,
                    idempotency_key: "wake-protected".into(),
                },
                CorrelationId::new(),
                at + chrono::Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(ready.state, WorkflowState::Ready);
    }

    #[test]
    fn claim_and_transition_use_revision_and_generation_cas() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "claim-1",
                now(),
            )
            .unwrap();
        assert_eq!(claimed.state, WorkflowState::Running);
        assert_eq!(claimed.revision, 2);
        assert_eq!(claimed.lease_generation, 1);
        assert_eq!(claimed.lease_owner, Some(owner));

        let stale = store.transition_workflow_attempt(
            attempt.attempt_id,
            1,
            0,
            WorkflowState::Verifying,
            CorrelationId::new(),
            "stale-transition",
            now(),
        );
        assert!(matches!(stale, Err(StorageError::Conflict(_))));

        let verifying = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                2,
                1,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "verify-1",
                now(),
            )
            .unwrap();
        assert_eq!(verifying.revision, 3);
        assert!(verifying.accepts_worker_result(attempt.attempt_id, 1));
        let succeeded = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                3,
                1,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "succeed-1",
                now(),
            )
            .unwrap();
        assert!(succeeded.state.is_terminal());
        assert!(!succeeded.accepts_worker_result(attempt.attempt_id, 1));
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 4);
    }

    #[test]
    fn cancellation_without_unresolved_effects_fences_worker_and_settles_cancelled() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "claim-cancel",
                now(),
            )
            .unwrap();
        let cancelling = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-request",
                now(),
            )
            .unwrap();
        assert_eq!(cancelling.lease_generation, running.lease_generation + 1);
        assert!(cancelling.lease_owner.is_none());
        assert!(!cancelling.accepts_worker_result(attempt.attempt_id, running.lease_generation));

        let invalid = store.transition_workflow_attempt(
            attempt.attempt_id,
            cancelling.revision,
            cancelling.lease_generation,
            WorkflowState::Waiting(WaitReason::User),
            CorrelationId::new(),
            "invalid-wait",
            now(),
        );
        assert!(matches!(invalid, Err(StorageError::Conflict(_))));
        let unsupported_reconciliation = store.transition_workflow_attempt(
            attempt.attempt_id,
            cancelling.revision,
            cancelling.lease_generation,
            WorkflowState::Waiting(WaitReason::Reconciliation),
            CorrelationId::new(),
            "reconcile-without-effect",
            now(),
        );
        assert!(matches!(
            unsupported_reconciliation,
            Err(StorageError::Conflict(_))
        ));
        let cancelled = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                cancelling.revision,
                cancelling.lease_generation,
                WorkflowState::Cancelled,
                CorrelationId::new(),
                "cancel-settled",
                now(),
            )
            .unwrap();
        assert_eq!(cancelled.state, WorkflowState::Cancelled);
    }

    #[test]
    fn idempotency_replay_rejects_claim_and_transition_substitution() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let expiry = now() + chrono::Duration::minutes(1);
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        let replay = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        assert_eq!(replay, claimed);
        let substituted_claim = store.claim_workflow_attempt(
            attempt.attempt_id,
            1,
            0,
            RuntimeActorId::new(),
            expiry,
            CorrelationId::new(),
            "claim-exact",
            now(),
        );
        assert!(matches!(substituted_claim, Err(StorageError::Conflict(_))));

        let verifying = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "transition-exact",
                now(),
            )
            .unwrap();
        let replay = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "transition-exact",
                now(),
            )
            .unwrap();
        assert_eq!(replay, verifying);

        // An exact claim replay must return the historical claimed projection even after the
        // authoritative attempt has advanced to Verifying.
        let late_claim_replay = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                expiry,
                CorrelationId::new(),
                "claim-exact",
                now(),
            )
            .unwrap();
        assert_eq!(late_claim_replay, claimed);
        assert_eq!(late_claim_replay.state, WorkflowState::Running);
        assert_eq!(
            store.workflow_attempt(attempt.attempt_id).unwrap().unwrap(),
            verifying
        );
        let substituted_transition = store.transition_workflow_attempt(
            attempt.attempt_id,
            claimed.revision,
            claimed.lease_generation,
            WorkflowState::Failed,
            CorrelationId::new(),
            "transition-exact",
            now(),
        );
        assert!(matches!(
            substituted_transition,
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn heartbeat_extends_without_transferring_or_reviving_a_lease() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let owner = RuntimeActorId::new();
        let claimed = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                owner,
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "heartbeat-claim",
                now(),
            )
            .unwrap();
        let heartbeat_at = now() + chrono::Duration::seconds(10);
        let extended_expiry = now() + chrono::Duration::minutes(2);
        let heartbeat = store
            .heartbeat_workflow_attempt(
                attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                owner,
                extended_expiry,
                CorrelationId::new(),
                "heartbeat-1",
                heartbeat_at,
            )
            .unwrap();
        assert_eq!(heartbeat.revision, claimed.revision + 1);
        assert_eq!(heartbeat.lease_generation, claimed.lease_generation);
        assert_eq!(heartbeat.lease_owner, Some(owner));
        assert_eq!(heartbeat.heartbeat_at, Some(heartbeat_at));
        assert_eq!(heartbeat.lease_expires_at, Some(extended_expiry));

        let transfer = store.heartbeat_workflow_attempt(
            attempt.attempt_id,
            heartbeat.revision,
            heartbeat.lease_generation,
            RuntimeActorId::new(),
            extended_expiry + chrono::Duration::minutes(1),
            CorrelationId::new(),
            "heartbeat-transfer",
            heartbeat_at,
        );
        assert!(matches!(transfer, Err(StorageError::Conflict(_))));
    }

    #[test]
    fn restart_scanner_fences_expired_work_without_reactivating_terminals() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "recovery-claim",
                now(),
            )
            .unwrap();
        let scan_time = now() + chrono::Duration::minutes(2);
        let report = store
            .recover_expired_workflow_leases(CorrelationId::new(), scan_time)
            .unwrap();
        assert_eq!(report.inspected_nonterminal, 1);
        assert_eq!(report.fenced_expired, 1);
        assert_eq!(report.awaiting_reconciliation, 1);
        assert_eq!(report.requeued_for_verification, 0);
        let recovered = store.workflow_attempt(attempt.attempt_id).unwrap().unwrap();
        assert_eq!(
            recovered.state,
            WorkflowState::Waiting(WaitReason::Reconciliation)
        );
        assert_eq!(recovered.revision, running.revision + 1);
        assert_eq!(recovered.lease_generation, running.lease_generation + 1);
        assert!(recovered.lease_owner.is_none());
        assert!(!recovered.accepts_worker_result(attempt.attempt_id, running.lease_generation));

        let second = store
            .recover_expired_workflow_leases(CorrelationId::new(), scan_time)
            .unwrap();
        assert_eq!(second.fenced_expired, 0);
        assert_eq!(store.workflow_events(attempt.workflow_id).unwrap().len(), 3);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn restart_scanner_requeues_expired_verification_and_ignores_terminal_history() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (verifying_contract, verifying_attempt) = fixture();
        store
            .create_workflow(
                &verifying_contract,
                &verifying_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let claimed = store
            .claim_workflow_attempt(
                verifying_attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "verify-recovery-claim",
                now(),
            )
            .unwrap();
        let verifying = store
            .transition_workflow_attempt(
                verifying_attempt.attempt_id,
                claimed.revision,
                claimed.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "verify-recovery-transition",
                now(),
            )
            .unwrap();

        let (terminal_contract, terminal_attempt) = fixture();
        store
            .create_workflow(
                &terminal_contract,
                &terminal_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let terminal_claim = store
            .claim_workflow_attempt(
                terminal_attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "terminal-recovery-claim",
                now(),
            )
            .unwrap();
        let terminal_verifying = store
            .transition_workflow_attempt(
                terminal_attempt.attempt_id,
                terminal_claim.revision,
                terminal_claim.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "terminal-recovery-verifying",
                now(),
            )
            .unwrap();
        let succeeded = store
            .transition_workflow_attempt(
                terminal_attempt.attempt_id,
                terminal_verifying.revision,
                terminal_verifying.lease_generation,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "terminal-recovery-succeeded",
                now(),
            )
            .unwrap();

        let report = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::minutes(2),
            )
            .unwrap();
        assert_eq!(report.inspected_nonterminal, 1);
        assert_eq!(report.fenced_expired, 1);
        assert_eq!(report.awaiting_reconciliation, 0);
        assert_eq!(report.requeued_for_verification, 1);

        let recovered = store
            .workflow_attempt(verifying_attempt.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.state, WorkflowState::Ready);
        assert_eq!(recovered.revision, verifying.revision + 1);
        assert_eq!(recovered.lease_generation, verifying.lease_generation + 1);
        assert!(recovered.lease_owner.is_none());
        assert_eq!(
            store
                .workflow_attempt(terminal_attempt.attempt_id)
                .unwrap()
                .unwrap(),
            succeeded
        );
    }

    #[test]
    fn restart_scanner_resumes_only_objectively_satisfied_wait_conditions() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let dependency = waiting_fixture(&mut store, WaitReason::Dependency, "dependency");
        let backoff = waiting_fixture(&mut store, WaitReason::RetryBackoff, "backoff");
        let approval = waiting_fixture(&mut store, WaitReason::Approval, "approval");

        let (cancel_contract, cancel_attempt) = fixture();
        store
            .create_workflow(
                &cancel_contract,
                &cancel_attempt,
                CorrelationId::new(),
                now(),
            )
            .unwrap();
        let cancel_running = store
            .claim_workflow_attempt(
                cancel_attempt.attempt_id,
                cancel_attempt.revision,
                cancel_attempt.lease_generation,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(5),
                CorrelationId::new(),
                "cancel-claim",
                now(),
            )
            .unwrap();
        let cancelling = store
            .transition_workflow_attempt(
                cancel_attempt.attempt_id,
                cancel_running.revision,
                cancel_running.lease_generation,
                WorkflowState::CancelRequested,
                CorrelationId::new(),
                "cancel-request",
                now(),
            )
            .unwrap();

        let early = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::milliseconds(99),
            )
            .unwrap();
        assert_eq!(early.requeued_satisfied_dependencies, 1);
        assert_eq!(early.requeued_elapsed_backoffs, 0);
        assert_eq!(early.settled_cancellations, 1);
        assert_eq!(
            store
                .workflow_attempt(dependency.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Ready
        );
        assert_eq!(
            store
                .workflow_attempt(backoff.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Waiting(WaitReason::RetryBackoff)
        );

        let due = store
            .recover_expired_workflow_leases(
                CorrelationId::new(),
                now() + chrono::Duration::milliseconds(100),
            )
            .unwrap();
        assert_eq!(due.requeued_satisfied_dependencies, 0);
        assert_eq!(due.requeued_elapsed_backoffs, 1);
        assert_eq!(
            store
                .workflow_attempt(backoff.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Ready
        );
        assert_eq!(
            store
                .workflow_attempt(approval.attempt_id)
                .unwrap()
                .unwrap()
                .state,
            WorkflowState::Waiting(WaitReason::Approval)
        );
        let settled = store
            .workflow_attempt(cancelling.attempt_id)
            .unwrap()
            .unwrap();
        assert_eq!(settled.state, WorkflowState::Cancelled);
        assert_eq!(settled.revision, cancelling.revision + 1);
    }

    #[test]
    fn retry_creates_acyclic_attempt_lineage_without_rewinding_terminal_history() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (contract, attempt) = fixture();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now())
            .unwrap();
        let running = store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "retry-claim",
                now(),
            )
            .unwrap();
        let failed = store
            .transition_workflow_attempt(
                attempt.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::Failed,
                CorrelationId::new(),
                "retry-fail",
                now(),
            )
            .unwrap();
        let retry = store
            .retry_workflow_attempt(
                failed.attempt_id,
                failed.revision,
                failed.lease_generation,
                CorrelationId::new(),
                "retry-attempt-2",
                now(),
            )
            .unwrap();
        assert_ne!(retry.attempt_id, failed.attempt_id);
        assert_eq!(retry.parent_attempt_id, Some(failed.attempt_id));
        assert_eq!(retry.attempt_number, 2);
        assert_eq!(retry.state, WorkflowState::Ready);
        assert_eq!(retry.revision, 1);
        assert_eq!(retry.lease_generation, 0);
        assert_eq!(
            store.workflow_attempt(failed.attempt_id).unwrap().unwrap(),
            failed
        );
        assert_eq!(
            store
                .retry_workflow_attempt(
                    failed.attempt_id,
                    failed.revision,
                    failed.lease_generation,
                    CorrelationId::new(),
                    "retry-attempt-2",
                    now(),
                )
                .unwrap(),
            retry
        );
    }

    #[test]
    fn dependency_ordering_blocks_claim_until_exact_prerequisite_succeeds() {
        let mut store = SqliteEventStore::open_in_memory().unwrap();
        let (prerequisite_contract, prerequisite) = fixture();
        store
            .create_workflow(
                &prerequisite_contract,
                &prerequisite,
                CorrelationId::new(),
                now(),
            )
            .unwrap();

        let (mut dependent_contract, mut dependent) = fixture();
        dependent_contract.dependencies = vec![prerequisite.task_contract.clone()];
        dependent.task_contract.content_hash =
            task_contract_content_hash(&dependent_contract).unwrap();
        store
            .create_workflow(&dependent_contract, &dependent, CorrelationId::new(), now())
            .unwrap();
        let blocked = store.claim_workflow_attempt(
            dependent.attempt_id,
            1,
            0,
            RuntimeActorId::new(),
            now() + chrono::Duration::minutes(1),
            CorrelationId::new(),
            "dependent-early-claim",
            now(),
        );
        assert!(matches!(blocked, Err(StorageError::Conflict(_))));

        let running = store
            .claim_workflow_attempt(
                prerequisite.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "prerequisite-claim",
                now(),
            )
            .unwrap();
        let verifying = store
            .transition_workflow_attempt(
                prerequisite.attempt_id,
                running.revision,
                running.lease_generation,
                WorkflowState::Verifying,
                CorrelationId::new(),
                "prerequisite-verify",
                now(),
            )
            .unwrap();
        store
            .transition_workflow_attempt(
                prerequisite.attempt_id,
                verifying.revision,
                verifying.lease_generation,
                WorkflowState::Succeeded,
                CorrelationId::new(),
                "prerequisite-succeed",
                now(),
            )
            .unwrap();

        let claimed = store
            .claim_workflow_attempt(
                dependent.attempt_id,
                1,
                0,
                RuntimeActorId::new(),
                now() + chrono::Duration::minutes(1),
                CorrelationId::new(),
                "dependent-late-claim",
                now(),
            )
            .unwrap();
        assert_eq!(claimed.state, WorkflowState::Running);
    }

    #[test]
    fn concurrent_claim_race_has_one_authoritative_winner() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workflow-race.sqlite3");
        let (contract, attempt) = fixture();
        {
            let mut store = SqliteEventStore::open(&database).unwrap();
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), now())
                .unwrap();
        }
        let attempt_id = attempt.attempt_id;
        let workflow_id = attempt.workflow_id;
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|index| {
                let database = database.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut store = SqliteEventStore::open(database).unwrap();
                    barrier.wait();
                    store
                        .claim_workflow_attempt(
                            attempt_id,
                            1,
                            0,
                            RuntimeActorId::new(),
                            now() + chrono::Duration::minutes(1),
                            CorrelationId::new(),
                            &format!("claim-race-{index}"),
                            now(),
                        )
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|succeeded| *succeeded)
            .count();
        assert_eq!(successes, 1);

        let store = SqliteEventStore::open(&database).unwrap();
        let authoritative = store.workflow_attempt(attempt_id).unwrap().unwrap();
        assert_eq!(authoritative.state, WorkflowState::Running);
        assert_eq!(authoritative.revision, 2);
        assert_eq!(authoritative.lease_generation, 1);
        assert_eq!(store.workflow_events(workflow_id).unwrap().len(), 2);
    }
}
