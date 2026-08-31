//! Protected Root process bootstrap and authoritative storage cutover.
//!
//! Production executables enter the application through this crate so the
//! existing compatibility API and every `SQLite` mutation share one Root owner.

pub mod runtime_worker;

pub use runtime_worker::{
    RuntimeWorkerCancellationHandle, RuntimeWorkerClient, RuntimeWorkerClientError,
    RuntimeWorkerReplacementReason,
};

use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{Duration, Utc};
use mcg_action_runner::{
    ActionRunnerClient, ActionRunnerClientError, ActionRunnerProtocolError, ActionRunnerRequest,
    ActionRunnerResponse, GOVERNED_ACTION_OPERATION_HASH_DOMAIN,
    GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN, GOVERNED_FILESYSTEM_OPERATION_TYPE,
    GovernedFilesystemAuthority, GovernedFilesystemObservation, GovernedFilesystemOperation,
    RunnerEffectOutcome, decode_governed_filesystem_authority,
    decode_governed_filesystem_observation, decode_governed_filesystem_operation,
    governed_filesystem_authority_binding, request_from_authorized_action,
    request_from_reconciliation_job, validate_governed_filesystem_envelope,
};
use mcg_config::AppConfig;
use mcg_contracts::{
    ActionCompensationReceipt, ActionEnvelope, ActionExecution, ActionExecutionId,
    ActionExecutionState, ActionId, ActionOrigin, ActionReceipt, ActionReceiptId,
    AuthenticatedRpcRequest, ContentHash, ContractError, CorrelationId, EffectClass, EffectOutcome,
    HashAlgorithm, InvocationId, OwnerActionCompensationRequest, PROTOCOL_V1, PolicyBinding,
    ReconciliationJob, ReconciliationJobId, ReconciliationObservation, RequestId, RootStatus,
    RpcEnvelope, RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, RuntimeInvocationRequest,
    RuntimeInvocationResult, SourceRef, StableTypeId, VersionedPayload, WorkflowAttempt,
    WorkflowCancelRequest, WorkflowId, WorkflowState, canonical_contract_hash,
    decode_canonical_cbor_v1,
};
use mcg_platform::{
    MAX_ROOT_INVOCATION_MODEL_RUNS, MAX_ROOT_INVOCATION_SUMMARIES, MAX_ROOT_WORKFLOW_INVOCATIONS,
    PlatformError, ROOT_ACTION_COMPENSATE_REQUEST_TYPE, ROOT_ACTION_COMPENSATE_RESPONSE_TYPE,
    ROOT_ACTION_LIST_REQUEST_TYPE, ROOT_ACTION_LIST_RESPONSE_TYPE, ROOT_ACTION_SHOW_REQUEST_TYPE,
    ROOT_ACTION_SHOW_RESPONSE_TYPE, ROOT_INVOCATION_LIST_REQUEST_TYPE,
    ROOT_INVOCATION_LIST_RESPONSE_TYPE, ROOT_INVOCATION_SHOW_REQUEST_TYPE,
    ROOT_INVOCATION_SHOW_RESPONSE_TYPE, ROOT_STATUS_REQUEST_TYPE, ROOT_STATUS_RESPONSE_TYPE,
    ROOT_WORKFLOW_ACTIVITY_REQUEST_TYPE, ROOT_WORKFLOW_ACTIVITY_RESPONSE_TYPE,
    ROOT_WORKFLOW_CANCEL_REQUEST_TYPE, ROOT_WORKFLOW_CANCEL_RESPONSE_TYPE,
    ROOT_WORKFLOW_LIST_REQUEST_TYPE, ROOT_WORKFLOW_LIST_RESPONSE_TYPE,
    ROOT_WORKFLOW_SHOW_REQUEST_TYPE, ROOT_WORKFLOW_SHOW_RESPONSE_TYPE, RootIpcSession,
    create_root_ipc_session, read_canonical_frame, remove_root_ipc_session, write_canonical_frame,
};
use mcg_runtime::{AppRuntime, RuntimeError};
use mcg_storage::{
    ReconciliationResolutionReceipt, RootMigrationReceipt, STORAGE_SCHEMA_VERSION, StorageError,
    acquire_data_directory_lock, create_backup_bundle_from_snapshot, inspect_database_before_root,
    snapshot_database_before_root,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Semaphore, oneshot},
    task::JoinHandle,
};
use tracing::{debug, info, warn};
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum ProtectedRootError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    ActionProtocol(#[from] ActionRunnerProtocolError),
    #[error(transparent)]
    ActionRunnerClient(#[from] ActionRunnerClientError),
    #[error(transparent)]
    RuntimeWorker(#[from] RuntimeWorkerClientError),
    #[error("authorized action execution was not found")]
    ActionNotFound,
    #[error("configured Root API address is invalid: {0}")]
    Address(#[from] std::net::AddrParseError),
}

#[derive(Debug, Clone)]
pub struct GovernedActionDispatchReceipt {
    pub execution: ActionExecution,
    pub reconciliation_job: Option<ReconciliationJob>,
    pub runner_response: Option<ActionRunnerResponse>,
}

#[derive(Debug, Clone)]
pub struct GovernedActionReconciliationReceipt {
    pub resolution: ReconciliationResolutionReceipt,
    pub runner_response: Option<ActionRunnerResponse>,
}

#[derive(Debug, Clone)]
pub struct RuntimeInvocationDispatchReceipt {
    pub result: RuntimeInvocationResult,
    pub response_artifact: mcg_storage::RuntimeResponseArtifact,
    pub completed_run: mcg_storage::ModelRunRecord,
}

#[derive(Debug, Clone)]
pub struct RuntimeInvocationCancellationReceipt {
    pub workflow: WorkflowAttempt,
    pub model_run: mcg_storage::ModelRunRecord,
    pub worker_generation: u64,
}

#[derive(Debug, Clone)]
pub enum RuntimeInvocationExecutionReceipt {
    Completed(Box<RuntimeInvocationDispatchReceipt>),
    Cancelled(Box<RuntimeInvocationCancellationReceipt>),
}

enum RuntimeInvocationRace {
    Result(Result<RuntimeInvocationResult, RuntimeWorkerClientError>),
    Cancellation(WorkflowCancelRequest),
}

/// Allocates a durable Root-owned generation and authenticates the fixed adjacent worker.
///
/// # Errors
/// Returns an error when generation allocation, launch, or handshake fails.
pub async fn launch_runtime_worker(
    runtime: &AppRuntime,
    root_executable: impl AsRef<Path>,
    data_directory: impl AsRef<Path>,
) -> Result<RuntimeWorkerClient, ProtectedRootError> {
    let generation = runtime.allocate_runtime_worker_generation()?;
    Ok(RuntimeWorkerClient::adjacent_to_root(root_executable, data_directory, generation).await?)
}

/// Evidence that Root reaped one idle Runtime generation before authenticating its replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWorkerReplacementReceipt {
    pub previous_generation: u64,
    pub replacement_generation: u64,
    pub reason: RuntimeWorkerReplacementReason,
}

/// Replaces an exclusively held idle worker when its process, channel, or session is unusable.
/// The prior exact child is reaped before Root allocates and launches the next durable generation.
///
/// # Errors
/// Returns an error when health inspection, exact-child reaping, generation allocation, launch,
/// or authentication fails. A failed replacement leaves the slot empty so stale work cannot run.
pub async fn replace_idle_runtime_worker_if_needed(
    runtime: &AppRuntime,
    root_executable: impl AsRef<Path>,
    data_directory: impl AsRef<Path>,
    worker: &mut Option<RuntimeWorkerClient>,
    now: chrono::DateTime<Utc>,
) -> Result<Option<RuntimeWorkerReplacementReceipt>, ProtectedRootError> {
    let reason = match worker.as_mut() {
        None => return Ok(None),
        Some(client) => match client.replacement_reason(now).await {
            Ok(Some(reason)) => reason,
            Ok(None) => return Ok(None),
            Err(error) => {
                if let Some(mut suspect) = worker.take() {
                    let _ = suspect.terminate().await;
                }
                return Err(error.into());
            }
        },
    };
    let mut previous = worker.take().ok_or(RuntimeWorkerClientError::Unavailable)?;
    let previous_generation = previous.worker_generation();
    previous.terminate().await?;
    let replacement = launch_runtime_worker(runtime, root_executable, data_directory).await?;
    let replacement_generation = replacement.worker_generation();
    *worker = Some(replacement);
    Ok(Some(RuntimeWorkerReplacementReceipt {
        previous_generation,
        replacement_generation,
        reason,
    }))
}

/// Persists exact Runtime intent before dispatch, sends it only to the authenticated generation,
/// and accepts the returned proposal only while the workflow revision and lease remain current.
/// Any post-dispatch transport or commit ambiguity is durably classified `OutcomeUnknown`.
///
/// # Errors
/// Returns an error for stale authority, persistence failure, worker failure, or substituted data.
pub async fn execute_runtime_invocation(
    runtime: &AppRuntime,
    client: &mut RuntimeWorkerClient,
    request: &RuntimeInvocationRequest,
    now: chrono::DateTime<Utc>,
) -> Result<RuntimeInvocationDispatchReceipt, ProtectedRootError> {
    match execute_runtime_invocation_with_cancellation(
        runtime,
        client,
        request,
        now,
        std::future::pending::<WorkflowCancelRequest>(),
    )
    .await?
    {
        RuntimeInvocationExecutionReceipt::Completed(receipt) => Ok(*receipt),
        RuntimeInvocationExecutionReceipt::Cancelled(_) => {
            Err(RuntimeWorkerClientError::InvalidResponse.into())
        }
    }
}

/// Executes a persisted invocation while an exact durable workflow cancellation can interrupt
/// the active child through independent process control. Root fences workflow state first,
/// kills/reaps the exact generation, then settles the dispatched model run `Cancelled`.
///
/// # Errors
/// Returns an error for stale authority, invalid cancellation identity, persistence failure,
/// worker failure, or substituted data. Post-dispatch ambiguity settles `OutcomeUnknown`.
pub async fn execute_runtime_invocation_with_cancellation<C>(
    runtime: &AppRuntime,
    client: &mut RuntimeWorkerClient,
    request: &RuntimeInvocationRequest,
    now: chrono::DateTime<Utc>,
    cancellation: C,
) -> Result<RuntimeInvocationExecutionReceipt, ProtectedRootError>
where
    C: std::future::Future<Output = WorkflowCancelRequest>,
{
    if request.worker_generation != client.worker_generation()
        || request.invocation_manifest.runtime_identity != client.hello().runtime_identity
        || request.invocation_manifest.runtime_build != client.hello().executable_hash
    {
        return Err(RuntimeWorkerClientError::InvalidResponse.into());
    }
    runtime.plan_runtime_invocation(request, now)?;
    runtime.dispatch_runtime_invocation(request, Utc::now())?;
    let cancellation_handle = client.cancellation_handle();
    let race = {
        let invocation = client.invoke(request);
        tokio::pin!(invocation);
        tokio::pin!(cancellation);
        tokio::select! {
            result = &mut invocation => RuntimeInvocationRace::Result(result),
            cancel = &mut cancellation => RuntimeInvocationRace::Cancellation(cancel),
        }
    };
    let result = match race {
        RuntimeInvocationRace::Cancellation(cancel) => {
            if cancel.attempt_id != request.invocation_manifest.attempt_id
                || cancel.expected_revision != request.attempt_revision
                || cancel.expected_generation != request.lease_generation
                || cancellation_handle.worker_generation() != request.worker_generation
            {
                let _ = cancellation_handle.terminate().await;
                runtime.settle_runtime_invocation_without_result(
                    request,
                    mcg_contracts::ModelRunState::OutcomeUnknown,
                    Utc::now(),
                )?;
                return Err(RuntimeWorkerClientError::InvalidResponse.into());
            }
            let workflow = runtime.cancel_canonical_workflow(&cancel)?;
            let termination = cancellation_handle.terminate().await;
            let model_run = runtime.settle_runtime_invocation_without_result(
                request,
                mcg_contracts::ModelRunState::Cancelled,
                Utc::now(),
            )?;
            termination?;
            return Ok(RuntimeInvocationExecutionReceipt::Cancelled(Box::new(
                RuntimeInvocationCancellationReceipt {
                    workflow,
                    model_run,
                    worker_generation: request.worker_generation,
                },
            )));
        }
        RuntimeInvocationRace::Result(Ok(result)) => result,
        RuntimeInvocationRace::Result(Err(error)) => {
            runtime.settle_runtime_invocation_without_result(
                request,
                mcg_contracts::ModelRunState::OutcomeUnknown,
                Utc::now(),
            )?;
            return Err(error.into());
        }
    };
    match runtime.complete_runtime_invocation(request, &result, Utc::now()) {
        Ok((completed_run, response_artifact)) => Ok(RuntimeInvocationExecutionReceipt::Completed(
            Box::new(RuntimeInvocationDispatchReceipt {
                result,
                response_artifact,
                completed_run,
            }),
        )),
        Err(error) => {
            let _ = runtime.settle_runtime_invocation_without_result(
                request,
                mcg_contracts::ModelRunState::OutcomeUnknown,
                Utc::now(),
            );
            Err(error.into())
        }
    }
}

#[async_trait::async_trait]
pub trait GovernedActionRunnerGateway: Send + Sync {
    async fn invoke(
        &self,
        request: &ActionRunnerRequest,
    ) -> Result<ActionRunnerResponse, ActionRunnerClientError>;
}

#[async_trait::async_trait]
impl GovernedActionRunnerGateway for ActionRunnerClient {
    async fn invoke(
        &self,
        request: &ActionRunnerRequest,
    ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
        ActionRunnerClient::invoke(self, request).await
    }
}

/// Executes one already-authorized filesystem action and durably classifies its physical result.
///
/// Root persists dispatch before process launch. A lost/invalid/late response after possible
/// launch becomes `OutcomeUnknown` and reconciliation work; a proven pre-launch failure becomes
/// an observed no-effect result. The physical operation is never blindly repeated here.
///
/// # Errors
///
/// Returns an error before physical execution for missing, stale, or invalid authority, or when
/// authoritative state cannot be committed. Post-launch runner failures are converted into
/// durable effect outcomes rather than returned as ordinary execution errors.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn execute_authorized_filesystem_action(
    runtime: &AppRuntime,
    client: &dyn GovernedActionRunnerGateway,
    execution_id: ActionExecutionId,
    expected_revision: u64,
    expected_generation: u64,
    executor: RuntimeActorId,
    now: chrono::DateTime<Utc>,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    let (envelope, current) = runtime
        .canonical_action_for_execution(execution_id)?
        .ok_or(ProtectedRootError::ActionNotFound)?;
    if current.revision != expected_revision || current.lease_generation != expected_generation {
        return Err(ProtectedRootError::Storage(StorageError::Conflict(
            "action dispatch revision or generation is stale".into(),
        )));
    }
    validate_governed_filesystem_envelope(&envelope, now)?;
    let correlation_id = CorrelationId::new();
    let dispatched = runtime.dispatch_canonical_action(
        execution_id,
        expected_revision,
        expected_generation,
        executor,
        correlation_id,
        &format!("filesystem-dispatch:{execution_id}:{expected_revision}:{expected_generation}"),
        now,
    )?;
    let request_id = Uuid::new_v4();
    let request = request_from_authorized_action(&envelope, &dispatched, request_id, now)?;
    match client.invoke(&request).await {
        Ok(response) => classify_runner_response(
            runtime,
            &envelope,
            &dispatched,
            correlation_id,
            request_id,
            response,
        ),
        Err(error) => classify_runner_failure(
            runtime,
            &envelope,
            &dispatched,
            correlation_id,
            request_id,
            &error,
        ),
    }
}

const OWNER_COMPENSATION_EVENT_LIMIT: usize = 100;
const OWNER_COMPENSATION_AUTHORITY_TYPE: &str = "mcg.owner_action_compensation_authorization.v1";
const OWNER_COMPENSATION_AUTHORITY_DOMAIN: &str = "MetaCog.OwnerActionCompensationAuthorization";
const OWNER_COMPENSATION_BUDGET_TYPE: &str = "mcg.owner_action_compensation_budget.v1";
const OWNER_COMPENSATION_BUDGET_DOMAIN: &str = "MetaCog.OwnerActionCompensationBudget";
const ACTION_ENVELOPE_HASH_DOMAIN: &str = "MetaCog.ActionEnvelope";

/// Initiates one separately authorized compensation using only Root-derived immutable intent.
///
/// The owner request cannot choose an operation, actor, capability root, effect class, authority,
/// or reconciliation adapter. Exact idempotent replay returns the existing causal action and never
/// redispatches an action whose durable state has advanced beyond `Authorized`.
///
/// # Errors
/// Returns an error for missing, stale, ineligible, substituted, or unconfirmed intent and for
/// action-runner or durable-state failures.
pub async fn execute_owner_authorized_filesystem_compensation(
    runtime: &AppRuntime,
    client: &dyn GovernedActionRunnerGateway,
    request: &OwnerActionCompensationRequest,
    now: chrono::DateTime<Utc>,
) -> Result<ActionCompensationReceipt, ProtectedRootError> {
    request.validate()?;
    let original = runtime
        .canonical_action(request.action_id, OWNER_COMPENSATION_EVENT_LIMIT)?
        .ok_or(ProtectedRootError::ActionNotFound)?;
    let request_binding = owner_compensation_authority_binding(request)?;
    let (compensation_action_id, compensation_execution_id) =
        deterministic_compensation_ids(request);

    if let Some(existing) =
        runtime.canonical_action(compensation_action_id, OWNER_COMPENSATION_EVENT_LIMIT)?
    {
        validate_existing_compensation(&original, &existing, request, &request_binding)?;
        let execution = latest_action_execution(&existing)?;
        if execution.state == ActionExecutionState::Authorized {
            execute_authorized_filesystem_action(
                runtime,
                client,
                execution.id,
                execution.revision,
                execution.lease_generation,
                existing.envelope.actor,
                now,
            )
            .await?;
        }
        return compensation_receipt(runtime, request.action_id, true);
    }

    let original_authority = validate_owner_compensation_target(runtime, &original, request)?;
    let (envelope, execution) = owner_compensation_authorization(
        &original,
        original_authority,
        request,
        (compensation_action_id, compensation_execution_id),
        request_binding.clone(),
        now,
    )?;
    if let Err(error) = runtime.authorize_canonical_action(
        &envelope,
        &execution,
        request.expected_workflow_revision,
        request.expected_workflow_generation,
        CorrelationId::new(),
        now,
    ) {
        if let Some(existing) =
            runtime.canonical_action(compensation_action_id, OWNER_COMPENSATION_EVENT_LIMIT)?
        {
            validate_existing_compensation(&original, &existing, request, &request_binding)?;
            return compensation_receipt(runtime, request.action_id, true);
        }
        return Err(error.into());
    }
    execute_authorized_filesystem_action(
        runtime,
        client,
        execution.id,
        execution.revision,
        execution.lease_generation,
        envelope.actor,
        now,
    )
    .await?;
    compensation_receipt(runtime, request.action_id, false)
}

fn validate_owner_compensation_target(
    runtime: &AppRuntime,
    original: &mcg_contracts::ActionInspection,
    request: &OwnerActionCompensationRequest,
) -> Result<GovernedFilesystemAuthority, ProtectedRootError> {
    let original_execution = latest_action_execution(original)?;
    if original_execution.id != request.execution_id
        || original_execution.revision != request.expected_action_revision
        || original_execution.lease_generation != request.expected_action_generation
        || original_execution.state != ActionExecutionState::Committed
        || original.envelope.effect_class != EffectClass::Reversible
        || original.envelope.compensation.is_none()
        || original.envelope.reconciler.is_none()
        || !original.compensation_actions.is_empty()
    {
        return Err(StorageError::Conflict(
            "owner compensation request is stale or the original action is ineligible".into(),
        )
        .into());
    }
    let workflow = runtime
        .canonical_workflow(original.envelope.workflow_id)?
        .ok_or_else(|| StorageError::Conflict("compensated workflow was not found".into()))?;
    if workflow.attempt.attempt_id != original.envelope.attempt_id
        || workflow.attempt.revision != request.expected_workflow_revision
        || workflow.attempt.lease_generation != request.expected_workflow_generation
        || workflow.attempt.lease_owner != Some(original.envelope.actor)
        || !matches!(
            workflow.attempt.state,
            WorkflowState::Running | WorkflowState::Verifying
        )
    {
        return Err(StorageError::Conflict(
            "owner compensation workflow fence is stale or no longer active".into(),
        )
        .into());
    }
    decode_original_filesystem_authority(&original.envelope)
}

fn owner_compensation_authorization(
    original: &mcg_contracts::ActionInspection,
    original_authority: GovernedFilesystemAuthority,
    request: &OwnerActionCompensationRequest,
    ids: (ActionId, ActionExecutionId),
    request_binding: PolicyBinding,
    now: chrono::DateTime<Utc>,
) -> Result<(ActionEnvelope, ActionExecution), ProtectedRootError> {
    let (action_id, execution_id) = ids;
    let operation =
        original.envelope.compensation.clone().ok_or_else(|| {
            StorageError::Conflict("original action has no compensation plan".into())
        })?;
    let operation_hash = operation.content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)?;
    let authority = governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
        action_id,
        actor: original.envelope.actor,
        capability_root: original_authority.capability_root,
        operation_hash: operation_hash.clone(),
        effect_class: EffectClass::Reversible,
        expires_at: now + Duration::minutes(5),
    })
    .map_err(|_| ActionRunnerProtocolError::Invalid("compensation authority"))?;
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: original.envelope.workflow_id,
        attempt_id: original.envelope.attempt_id,
        task_contract: original.envelope.task_contract.clone(),
        actor: original.envelope.actor,
        origin: Some(owner_compensation_origin(&original.envelope, request)?),
        authority,
        elevated_authority: Some(request_binding),
        expected_versions: Vec::new(),
        operation,
        operation_hash,
        effect_class: EffectClass::Reversible,
        preconditions: vec![VersionedPayload::canonical_v1(
            StableTypeId::parse(OwnerActionCompensationRequest::TYPE_ID)?,
            &serde_json::to_value(request)?,
        )?],
        budget_reservation: owner_compensation_budget_binding(request)?,
        idempotency_key: format!("owner-compensation:{}", request.idempotency_key.trim()),
        compensation: None,
        compensates_action: Some(original.envelope.id),
        reconciler: original.envelope.reconciler.clone(),
        reconciliation_deadline: Some(now + Duration::minutes(5)),
        created_at: now,
    };
    validate_governed_filesystem_envelope(&envelope, now)?;
    let execution = ActionExecution {
        id: execution_id,
        action_id,
        attempt_number: 1,
        state: ActionExecutionState::Authorized,
        revision: 1,
        lease_generation: 0,
        executor: None,
        outcome: None,
        created_at: now,
        updated_at: now,
    };
    Ok((envelope, execution))
}

fn latest_action_execution(
    inspection: &mcg_contracts::ActionInspection,
) -> Result<ActionExecution, ProtectedRootError> {
    inspection
        .executions
        .last()
        .cloned()
        .ok_or_else(|| StorageError::InvalidData("action has no execution".into()).into())
}

fn compensation_receipt(
    runtime: &AppRuntime,
    original_action_id: ActionId,
    replayed: bool,
) -> Result<ActionCompensationReceipt, ProtectedRootError> {
    let original = runtime
        .canonical_action(original_action_id, OWNER_COMPENSATION_EVENT_LIMIT)?
        .ok_or(ProtectedRootError::ActionNotFound)?;
    let compensation_action_id = original
        .compensation_actions
        .last()
        .copied()
        .ok_or_else(|| StorageError::InvalidData("compensation action was not linked".into()))?;
    let compensation = runtime
        .canonical_action(compensation_action_id, OWNER_COMPENSATION_EVENT_LIMIT)?
        .ok_or_else(|| StorageError::InvalidData("compensation action was not found".into()))?;
    Ok(ActionCompensationReceipt {
        original_action_id,
        original_execution: latest_action_execution(&original)?,
        compensation,
        replayed,
    })
}

fn owner_compensation_authority_binding(
    request: &OwnerActionCompensationRequest,
) -> Result<PolicyBinding, ProtectedRootError> {
    policy_binding(
        OWNER_COMPENSATION_AUTHORITY_TYPE,
        OWNER_COMPENSATION_AUTHORITY_DOMAIN,
        &serde_json::json!({"owner_request": request}),
    )
}

fn owner_compensation_budget_binding(
    request: &OwnerActionCompensationRequest,
) -> Result<PolicyBinding, ProtectedRootError> {
    policy_binding(
        OWNER_COMPENSATION_BUDGET_TYPE,
        OWNER_COMPENSATION_BUDGET_DOMAIN,
        &serde_json::json!({
            "original_action_id": request.action_id,
            "idempotency_key": request.idempotency_key.trim(),
            "maximum_compensation_actions": 1,
        }),
    )
}

fn policy_binding(
    type_id: &str,
    domain: &str,
    value: &serde_json::Value,
) -> Result<PolicyBinding, ProtectedRootError> {
    let payload = VersionedPayload::canonical_v1(StableTypeId::parse(type_id)?, value)?;
    let content_hash = payload.content_hash(domain)?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

fn owner_compensation_origin(
    original: &ActionEnvelope,
    request: &OwnerActionCompensationRequest,
) -> Result<ActionOrigin, ProtectedRootError> {
    let request_hash = canonical_contract_hash(
        OwnerActionCompensationRequest::TYPE_ID,
        OwnerActionCompensationRequest::HASH_DOMAIN,
        request,
    )?;
    let original_hash = canonical_contract_hash(
        ActionEnvelope::TYPE_ID,
        ACTION_ENVELOPE_HASH_DOMAIN,
        original,
    )?;
    Ok(ActionOrigin {
        conversation: None,
        invocation: None,
        command: None,
        approval: None,
        agent: None,
        evidence: vec![
            SourceRef {
                source_type: StableTypeId::parse(ActionEnvelope::TYPE_ID)?,
                locator: original.id.to_string(),
                content_hash: original_hash,
            },
            SourceRef {
                source_type: StableTypeId::parse(OwnerActionCompensationRequest::TYPE_ID)?,
                locator: format!(
                    "{}:owner-compensation:{}",
                    original.id,
                    request.idempotency_key.trim()
                ),
                content_hash: request_hash,
            },
        ],
    })
}

fn decode_original_filesystem_authority(
    envelope: &ActionEnvelope,
) -> Result<GovernedFilesystemAuthority, ProtectedRootError> {
    if envelope.authority.content_hash.domain != GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN)?
            != envelope.authority.content_hash
    {
        return Err(ActionRunnerProtocolError::Invalid("original authority binding").into());
    }
    let authority = decode_governed_filesystem_authority(&envelope.authority.payload)?;
    if authority.action_id != envelope.id
        || authority.actor != envelope.actor
        || authority.operation_hash != envelope.operation_hash
        || authority.effect_class != envelope.effect_class
        || !authority.capability_root.is_absolute()
    {
        return Err(ActionRunnerProtocolError::Invalid("original authority scope").into());
    }
    Ok(authority)
}

fn validate_existing_compensation(
    original: &mcg_contracts::ActionInspection,
    existing: &mcg_contracts::ActionInspection,
    request: &OwnerActionCompensationRequest,
    request_binding: &PolicyBinding,
) -> Result<(), ProtectedRootError> {
    if existing.envelope.compensates_action != Some(original.envelope.id)
        || existing.envelope.workflow_id != original.envelope.workflow_id
        || existing.envelope.attempt_id != original.envelope.attempt_id
        || existing.envelope.task_contract != original.envelope.task_contract
        || existing.envelope.actor != original.envelope.actor
        || original.envelope.compensation.as_ref() != Some(&existing.envelope.operation)
        || existing.envelope.elevated_authority.as_ref() != Some(request_binding)
        || !original
            .compensation_actions
            .contains(&existing.envelope.id)
        || latest_action_execution(existing)?.id != deterministic_compensation_ids(request).1
    {
        return Err(StorageError::Conflict(
            "owner compensation idempotency key was replayed with substituted intent".into(),
        )
        .into());
    }
    Ok(())
}

fn deterministic_compensation_ids(
    request: &OwnerActionCompensationRequest,
) -> (ActionId, ActionExecutionId) {
    (
        ActionId(deterministic_compensation_uuid(
            b"MetaCog.OwnerActionCompensation.Action.v1\0",
            request,
        )),
        ActionExecutionId(deterministic_compensation_uuid(
            b"MetaCog.OwnerActionCompensation.Execution.v1\0",
            request,
        )),
    )
}

fn deterministic_compensation_uuid(
    domain: &[u8],
    request: &OwnerActionCompensationRequest,
) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(request.action_id.0.as_bytes());
    hasher.update(request.idempotency_key.trim().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Inspects and resolves one due governed-filesystem reconciliation job.
///
/// Root first claims the existing job and fences the associated action execution. It then invokes
/// only the immutable inspection adapter stored in that job. Exact evidence resolves occurrence
/// or non-occurrence; ambiguous or unavailable evidence remains unknown and is retried or
/// escalated by the storage policy. The original effect operation is never dispatched here.
///
/// # Errors
///
/// Returns an error when authority or job bindings are invalid, the lease is stale, or durable
/// claim/resolution fails. Runner failures are converted into durable `StillUnknown` evidence.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn reconcile_filesystem_action(
    runtime: &AppRuntime,
    client: &dyn GovernedActionRunnerGateway,
    job_id: ReconciliationJobId,
    expected_job_revision: u64,
    expected_job_generation: u64,
    expected_execution_revision: u64,
    expected_execution_generation: u64,
    worker: RuntimeActorId,
    now: chrono::DateTime<Utc>,
) -> Result<GovernedActionReconciliationReceipt, ProtectedRootError> {
    let job = runtime
        .canonical_reconciliation_job(job_id)?
        .ok_or(ProtectedRootError::ActionNotFound)?;
    let (envelope, execution) = runtime
        .canonical_action_for_execution(job.execution_id)?
        .ok_or(ProtectedRootError::ActionNotFound)?;
    if job.revision != expected_job_revision
        || job.lease_generation != expected_job_generation
        || execution.revision != expected_execution_revision
        || execution.lease_generation != expected_execution_generation
        || job.action_id != envelope.id
        || job.execution_id != execution.id
        || envelope.reconciler.as_ref() != Some(&job.adapter)
        || envelope.effect_class == EffectClass::Observational
    {
        return Err(ProtectedRootError::Storage(StorageError::Conflict(
            "filesystem reconciliation binding is stale or invalid".into(),
        )));
    }
    validate_governed_filesystem_envelope(&envelope, now)?;
    let lease_expires_at = std::cmp::min(now + Duration::minutes(2), job.deadline);
    let correlation_id = CorrelationId::new();
    let claimed = runtime.claim_canonical_reconciliation(
        job.id,
        expected_job_revision,
        expected_job_generation,
        expected_execution_revision,
        expected_execution_generation,
        worker,
        lease_expires_at,
        correlation_id,
        &format!(
            "filesystem-reconcile-claim:{job_id}:{expected_job_revision}:{expected_job_generation}"
        ),
        now,
    )?;
    let request_id = Uuid::new_v4();
    let request = request_from_reconciliation_job(
        &envelope,
        &claimed.execution,
        &claimed.job,
        request_id,
        now,
    )?;
    let runner_result = client.invoke(&request).await;
    let resolved_at = Utc::now();
    let (observation, runner_response) = match runner_result {
        Ok(response) if response.outcome == RunnerEffectOutcome::Observational => {
            let physical = response
                .observation
                .as_ref()
                .ok_or(ActionRunnerProtocolError::Invalid("runner observation"))
                .and_then(decode_governed_filesystem_observation)?;
            (
                interpret_filesystem_reconciliation(
                    &envelope,
                    &claimed.execution,
                    response
                        .observation
                        .clone()
                        .ok_or(ActionRunnerProtocolError::Invalid("runner observation"))?,
                    &physical,
                    resolved_at,
                    claimed.job.deadline,
                )?,
                Some(response),
            )
        }
        Ok(response) => (
            ReconciliationObservation::StillUnknown {
                hint: runner_outcome_payload(
                    "mcg.action_reconciliation_runner_unknown.v1",
                    &serde_json::json!({
                        "request_id": request_id,
                        "outcome": response.outcome,
                        "error_code": response.error_code,
                    }),
                )?,
                next_attempt_at: next_reconciliation_attempt(resolved_at, claimed.job.deadline),
            },
            Some(response),
        ),
        Err(error) => (
            ReconciliationObservation::StillUnknown {
                hint: runner_outcome_payload(
                    "mcg.action_reconciliation_transport_unknown.v1",
                    &serde_json::json!({
                        "request_id": request_id,
                        "error_code": action_runner_client_error_code(&error),
                    }),
                )?,
                next_attempt_at: next_reconciliation_attempt(resolved_at, claimed.job.deadline),
            },
            None,
        ),
    };
    let resolution = runtime.resolve_canonical_reconciliation(
        claimed.job.id,
        claimed.job.revision,
        claimed.job.lease_generation,
        claimed.execution.revision,
        claimed.execution.lease_generation,
        worker,
        &observation,
        correlation_id,
        &format!(
            "filesystem-reconcile-resolve:{}:{}:{}",
            claimed.job.id, claimed.job.revision, claimed.job.lease_generation
        ),
        resolved_at,
    )?;
    Ok(GovernedActionReconciliationReceipt {
        resolution,
        runner_response,
    })
}

fn interpret_filesystem_reconciliation(
    envelope: &mcg_contracts::ActionEnvelope,
    execution: &ActionExecution,
    observation_payload: VersionedPayload,
    observation: &GovernedFilesystemObservation,
    observed_at: chrono::DateTime<Utc>,
    deadline: chrono::DateTime<Utc>,
) -> Result<ReconciliationObservation, ProtectedRootError> {
    let operation = decode_governed_filesystem_operation(&envelope.operation)?;
    let expected_sha256 = match &operation {
        GovernedFilesystemOperation::CreateFile { .. } => {
            let compensation = envelope
                .compensation
                .as_ref()
                .ok_or(ActionRunnerProtocolError::Invalid("create compensation"))?;
            match decode_governed_filesystem_operation(compensation)? {
                GovernedFilesystemOperation::RemoveCreatedFile {
                    expected_sha256, ..
                } => expected_sha256,
                _ => {
                    return Err(ActionRunnerProtocolError::Invalid("create compensation").into());
                }
            }
        }
        GovernedFilesystemOperation::RemoveCreatedFile {
            expected_sha256, ..
        } => expected_sha256.clone(),
        GovernedFilesystemOperation::ReadFile { .. }
        | GovernedFilesystemOperation::InspectFile { .. } => {
            return Err(ActionRunnerProtocolError::Invalid("observational reconciliation").into());
        }
    };
    let exact_original_exists =
        observation.exists && observation.sha256.as_deref() == Some(expected_sha256.as_str());
    let confirmed_occurred = match operation {
        GovernedFilesystemOperation::CreateFile { .. } => exact_original_exists,
        GovernedFilesystemOperation::RemoveCreatedFile { .. } => !observation.exists,
        GovernedFilesystemOperation::ReadFile { .. }
        | GovernedFilesystemOperation::InspectFile { .. } => false,
    };
    let confirmed_not_occurred = match operation {
        GovernedFilesystemOperation::CreateFile { .. } => !observation.exists,
        GovernedFilesystemOperation::RemoveCreatedFile { .. } => exact_original_exists,
        GovernedFilesystemOperation::ReadFile { .. }
        | GovernedFilesystemOperation::InspectFile { .. } => false,
    };
    if confirmed_occurred {
        return Ok(ReconciliationObservation::ConfirmedOccurred {
            receipt: ActionReceipt {
                id: ActionReceiptId::new(),
                action_id: execution.action_id,
                execution_id: execution.id,
                observation: observation_payload,
                observed_at,
            },
            evidence: vec![],
        });
    }
    if confirmed_not_occurred {
        return Ok(ReconciliationObservation::ConfirmedNotOccurred);
    }
    Ok(ReconciliationObservation::StillUnknown {
        hint: runner_outcome_payload(
            "mcg.action_reconciliation_ambiguous_file.v1",
            &serde_json::json!({
                "relative_path": observation.relative_path,
                "exists": observation.exists,
                "sha256": observation.sha256,
                "expected_sha256": expected_sha256,
            }),
        )?,
        next_attempt_at: next_reconciliation_attempt(observed_at, deadline),
    })
}

fn next_reconciliation_attempt(
    now: chrono::DateTime<Utc>,
    deadline: chrono::DateTime<Utc>,
) -> chrono::DateTime<Utc> {
    std::cmp::min(now + Duration::seconds(30), deadline)
}

fn classify_runner_response(
    runtime: &AppRuntime,
    envelope: &mcg_contracts::ActionEnvelope,
    dispatched: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    response: ActionRunnerResponse,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    match response.outcome {
        RunnerEffectOutcome::OutcomeUnknown => {
            let hint = runner_outcome_payload(
                "mcg.action_runner_unknown.v1",
                &serde_json::json!({
                    "request_id": request_id,
                    "error_code": response.error_code,
                    "outcome": response.outcome,
                }),
            )?;
            if envelope.effect_class == EffectClass::Observational {
                return classify_proven_no_effect(
                    runtime,
                    dispatched,
                    correlation_id,
                    request_id,
                    hint,
                    response.finished_at,
                    Some(response),
                );
            }
            classify_unknown(
                runtime,
                envelope,
                dispatched,
                correlation_id,
                request_id,
                hint,
                Some(response),
            )
        }
        RunnerEffectOutcome::Observational | RunnerEffectOutcome::EffectConfirmed => {
            let observation = response
                .observation
                .clone()
                .ok_or(ActionRunnerProtocolError::Invalid("runner observation"))?;
            let observed = observe_runner_result(
                runtime,
                dispatched,
                correlation_id,
                request_id,
                observation,
                response.finished_at,
            )?;
            let outcome = if response.outcome == RunnerEffectOutcome::EffectConfirmed {
                EffectOutcome::EffectConfirmed { evidence: vec![] }
            } else {
                EffectOutcome::NoEffectConfirmed
            };
            classify_observed(
                runtime,
                &observed,
                correlation_id,
                request_id,
                outcome,
                Some(response),
            )
        }
        RunnerEffectOutcome::NoEffectConfirmed | RunnerEffectOutcome::Denied => {
            let observation = runner_outcome_payload(
                "mcg.action_runner_no_effect.v1",
                &serde_json::json!({
                    "request_id": request_id,
                    "error_code": response.error_code,
                    "outcome": response.outcome,
                }),
            )?;
            let observed = observe_runner_result(
                runtime,
                dispatched,
                correlation_id,
                request_id,
                observation,
                response.finished_at,
            )?;
            classify_observed(
                runtime,
                &observed,
                correlation_id,
                request_id,
                EffectOutcome::NoEffectConfirmed,
                Some(response),
            )
        }
    }
}

fn classify_runner_failure(
    runtime: &AppRuntime,
    envelope: &mcg_contracts::ActionEnvelope,
    dispatched: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    error: &ActionRunnerClientError,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    let may_have_occurred = error.effect_may_have_occurred();
    let error_code = action_runner_client_error_code(error);
    let observation = runner_outcome_payload(
        if may_have_occurred {
            "mcg.action_runner_transport_unknown.v1"
        } else {
            "mcg.action_runner_launch_no_effect.v1"
        },
        &serde_json::json!({
            "request_id": request_id,
            "error_code": error_code,
        }),
    )?;
    if envelope.effect_class == EffectClass::Observational {
        return classify_proven_no_effect(
            runtime,
            dispatched,
            correlation_id,
            request_id,
            observation,
            Utc::now(),
            None,
        );
    }
    if may_have_occurred {
        classify_unknown(
            runtime,
            envelope,
            dispatched,
            correlation_id,
            request_id,
            observation,
            None,
        )
    } else {
        let observed = observe_runner_result(
            runtime,
            dispatched,
            correlation_id,
            request_id,
            observation,
            Utc::now(),
        )?;
        classify_observed(
            runtime,
            &observed,
            correlation_id,
            request_id,
            EffectOutcome::NoEffectConfirmed,
            None,
        )
    }
}

fn classify_proven_no_effect(
    runtime: &AppRuntime,
    dispatched: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    observation: VersionedPayload,
    observed_at: chrono::DateTime<Utc>,
    runner_response: Option<ActionRunnerResponse>,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    let observed = observe_runner_result(
        runtime,
        dispatched,
        correlation_id,
        request_id,
        observation,
        observed_at,
    )?;
    classify_observed(
        runtime,
        &observed,
        correlation_id,
        request_id,
        EffectOutcome::NoEffectConfirmed,
        runner_response,
    )
}

fn observe_runner_result(
    runtime: &AppRuntime,
    dispatched: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    observation: VersionedPayload,
    observed_at: chrono::DateTime<Utc>,
) -> Result<ActionExecution, ProtectedRootError> {
    let receipt = ActionReceipt {
        id: ActionReceiptId::new(),
        action_id: dispatched.action_id,
        execution_id: dispatched.id,
        observation,
        observed_at,
    };
    Ok(runtime.observe_canonical_action(
        &receipt,
        dispatched.revision,
        dispatched.lease_generation,
        correlation_id,
        &format!("filesystem-observe:{request_id}"),
        observed_at,
    )?)
}

fn classify_observed(
    runtime: &AppRuntime,
    observed: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    outcome: EffectOutcome,
    runner_response: Option<ActionRunnerResponse>,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    let (execution, reconciliation_job) = runtime.classify_canonical_action(
        observed.id,
        observed.revision,
        observed.lease_generation,
        outcome,
        None,
        correlation_id,
        &format!("filesystem-classify:{request_id}"),
        Utc::now(),
    )?;
    Ok(GovernedActionDispatchReceipt {
        execution,
        reconciliation_job,
        runner_response,
    })
}

fn classify_unknown(
    runtime: &AppRuntime,
    envelope: &mcg_contracts::ActionEnvelope,
    dispatched: &ActionExecution,
    correlation_id: CorrelationId,
    request_id: Uuid,
    hint: VersionedPayload,
    runner_response: Option<ActionRunnerResponse>,
) -> Result<GovernedActionDispatchReceipt, ProtectedRootError> {
    let (execution, reconciliation_job) = runtime.classify_canonical_action(
        dispatched.id,
        dispatched.revision,
        dispatched.lease_generation,
        EffectOutcome::OutcomeUnknown { hint },
        envelope.reconciliation_deadline,
        correlation_id,
        &format!("filesystem-classify:{request_id}"),
        Utc::now(),
    )?;
    Ok(GovernedActionDispatchReceipt {
        execution,
        reconciliation_job,
        runner_response,
    })
}

fn runner_outcome_payload(
    type_id: &str,
    value: &serde_json::Value,
) -> Result<VersionedPayload, ProtectedRootError> {
    Ok(VersionedPayload::canonical_v1(
        StableTypeId::parse(type_id)?,
        value,
    )?)
}

const fn action_runner_client_error_code(error: &ActionRunnerClientError) -> &'static str {
    match error {
        ActionRunnerClientError::Unavailable => "unavailable",
        ActionRunnerClientError::RequestTooLarge => "request_too_large",
        ActionRunnerClientError::Launch => "launch_failed",
        ActionRunnerClientError::Delivery => "delivery_failed",
        ActionRunnerClientError::Timeout => "timeout",
        ActionRunnerClientError::ResponseTooLarge => "response_too_large",
        ActionRunnerClientError::DiagnosticOutput => "diagnostic_output",
        ActionRunnerClientError::InvalidResponse => "invalid_response",
        ActionRunnerClientError::Process => "process_failed",
    }
}

/// Opens the sole authoritative runtime after a verified ownership-migration backup.
///
/// # Errors
/// Returns a protected-root error when locking, backup, migration, identity
/// initialization, recovery, or executable hashing fails.
pub fn open(config: AppConfig) -> Result<Arc<AppRuntime>, ProtectedRootError> {
    fs::create_dir_all(&config.data_directory)?;
    let data_lock = acquire_data_directory_lock(&config.data_directory)?;
    let migration_receipt = prepare_root_migration(&config)?;
    let runtime = Arc::new(AppRuntime::open_with_data_directory_lock(
        config, data_lock,
    )?);
    let identity = current_runtime_identity()?;
    runtime.initialize_root_ownership(identity, migration_receipt)?;
    let recovered_at = Utc::now();
    let reconciliation_recovery = runtime.recover_action_reconciliations(recovered_at)?;
    if reconciliation_recovery.requeued > 0 || reconciliation_recovery.escalated > 0 {
        info!(
            inspected_running = reconciliation_recovery.inspected_running,
            requeued = reconciliation_recovery.requeued,
            escalated = reconciliation_recovery.escalated,
            "fenced expired action reconciliation without redispatching external effects"
        );
    }
    let action_recovery = runtime.recover_stranded_actions(recovered_at)?;
    if action_recovery.inspected > 0 {
        info!(
            inspected = action_recovery.inspected,
            no_effect_confirmed = action_recovery.no_effect_confirmed,
            outcome_unknown = action_recovery.outcome_unknown,
            escalated = action_recovery.escalated,
            "classified stranded action executions without redispatching external effects"
        );
    }
    let workflow_recovery = runtime.recover_expired_workflows(recovered_at)?;
    if workflow_recovery.fenced_expired > 0
        || workflow_recovery.settled_cancellations > 0
        || workflow_recovery.cancellations_awaiting_reconciliation > 0
        || workflow_recovery.quarantined_cancellations > 0
    {
        info!(
            fenced_expired = workflow_recovery.fenced_expired,
            awaiting_reconciliation = workflow_recovery.awaiting_reconciliation,
            requeued_for_verification = workflow_recovery.requeued_for_verification,
            requeued_satisfied_dependencies = workflow_recovery.requeued_satisfied_dependencies,
            requeued_elapsed_backoffs = workflow_recovery.requeued_elapsed_backoffs,
            settled_cancellations = workflow_recovery.settled_cancellations,
            cancellations_awaiting_reconciliation =
                workflow_recovery.cancellations_awaiting_reconciliation,
            quarantined_cancellations = workflow_recovery.quarantined_cancellations,
            "recovered canonical workflow leases, waits, and cancellation settlement"
        );
    }
    Ok(runtime)
}

fn start_filesystem_reconciliation_worker(runtime: Arc<AppRuntime>) -> Option<JoinHandle<()>> {
    let executable = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            warn!(%error, "filesystem action runner discovery is unavailable");
            return None;
        }
    };
    let client = match ActionRunnerClient::adjacent_to(executable) {
        Ok(client) => client,
        Err(error) => {
            warn!(%error, "filesystem action runner is unavailable; reconciliation remains durable");
            return None;
        }
    };
    Some(tokio::spawn(async move {
        let worker = RuntimeActorId::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            interval.tick().await;
            let now = Utc::now();
            let jobs = match runtime.due_canonical_reconciliations(now, 32) {
                Ok(jobs) => jobs,
                Err(error) => {
                    warn!(%error, "could not query due action reconciliation work");
                    continue;
                }
            };
            for job in jobs {
                if job.adapter.type_id.as_str() != GOVERNED_FILESYSTEM_OPERATION_TYPE {
                    continue;
                }
                let execution = match runtime.canonical_action_for_execution(job.execution_id) {
                    Ok(Some((_, execution))) => execution,
                    Ok(None) => {
                        warn!(job_id = %job.id, "reconciliation action execution disappeared");
                        continue;
                    }
                    Err(error) => {
                        warn!(job_id = %job.id, %error, "could not load reconciliation action");
                        continue;
                    }
                };
                match reconcile_filesystem_action(
                    &runtime,
                    &client,
                    job.id,
                    job.revision,
                    job.lease_generation,
                    execution.revision,
                    execution.lease_generation,
                    worker,
                    now,
                )
                .await
                {
                    Ok(receipt) => debug!(
                        job_id = %receipt.resolution.job.id,
                        action_id = %receipt.resolution.job.action_id,
                        state = ?receipt.resolution.job.state,
                        "completed governed filesystem reconciliation attempt"
                    ),
                    Err(error) => debug!(
                        job_id = %job.id,
                        %error,
                        "governed filesystem reconciliation attempt did not commit"
                    ),
                }
            }
        }
    }))
}

const RUNTIME_WORKER_HEALTH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

struct RuntimeWorkerSupervisor {
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl RuntimeWorkerSupervisor {
    fn start(
        runtime: Arc<AppRuntime>,
        root_executable: PathBuf,
        data_directory: PathBuf,
        worker: RuntimeWorkerClient,
    ) -> Self {
        let (shutdown, mut shutdown_requested) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut worker = Some(worker);
            let mut interval = tokio::time::interval(RUNTIME_WORKER_HEALTH_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = &mut shutdown_requested => break,
                    _ = interval.tick() => {
                        if worker.is_none() {
                            match launch_runtime_worker(&runtime, &root_executable, &data_directory).await {
                                Ok(replacement) => {
                                    info!(
                                        generation = replacement.worker_generation(),
                                        process_id = replacement.hello().process_id,
                                        "restored authenticated idle cognition Runtime worker"
                                    );
                                    worker = Some(replacement);
                                }
                                Err(error) => warn!(%error, "idle cognition Runtime worker remains unavailable"),
                            }
                            continue;
                        }
                        match replace_idle_runtime_worker_if_needed(
                            &runtime,
                            &root_executable,
                            &data_directory,
                            &mut worker,
                            Utc::now(),
                        )
                        .await
                        {
                            Ok(Some(receipt)) => info!(
                                previous_generation = receipt.previous_generation,
                                replacement_generation = receipt.replacement_generation,
                                reason = ?receipt.reason,
                                "replaced idle cognition Runtime worker"
                            ),
                            Ok(None) => {}
                            Err(error) => warn!(%error, "could not replace idle cognition Runtime worker"),
                        }
                    }
                }
            }
            if let Some(worker) = worker
                && let Err(error) = worker.shutdown().await
            {
                warn!(%error, "Runtime worker required forced cleanup during Root shutdown");
            }
        });
        Self { shutdown, task }
    }

    async fn shutdown(self) {
        let _ = self.shutdown.send(());
        if let Err(error) = self.task.await {
            warn!(%error, "Runtime worker supervisor did not stop cleanly");
        }
    }
}

/// Runs the Root-owned compatibility API until shutdown.
///
/// # Errors
/// Returns startup, migration, recovery, or server failures.
pub async fn serve(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = config.bind_address.parse()?;
    let data_directory = config.data_directory.clone();
    let runtime = open(config)?;
    let root_executable = std::env::current_exe()?;
    let runtime_worker_name = if cfg!(windows) {
        "mcg-runtime-worker.exe"
    } else {
        "mcg-runtime-worker"
    };
    let runtime_worker_path = root_executable
        .parent()
        .map(|parent| parent.join(runtime_worker_name));
    let runtime_worker = if runtime_worker_path
        .as_ref()
        .is_some_and(|path| path.is_file())
    {
        let worker = launch_runtime_worker(&runtime, &root_executable, &data_directory).await?;
        info!(
            process_id = worker.hello().process_id,
            generation = worker.worker_generation(),
            runtime_instance = %worker.hello().runtime_identity.instance_id,
            "authenticated adjacent cognition Runtime worker"
        );
        Some(worker)
    } else {
        warn!(
            path = ?runtime_worker_path,
            "Runtime worker sidecar is absent; deterministic cognition dispatch is unavailable"
        );
        None
    };
    let runtime_worker_supervisor = runtime_worker.map(|worker| {
        RuntimeWorkerSupervisor::start(
            runtime.clone(),
            root_executable.clone(),
            data_directory.clone(),
            worker,
        )
    });

    // Internal role threads are recoverable implementation details, not user-owned tasks.
    match runtime.archive_owned_codex_threads_from_recents().await {
        Ok(archived) if archived > 0 => info!(archived, "archived idle MCG threads"),
        Ok(_) => {}
        Err(error) => warn!(%error, "could not archive idle MCG threads"),
    }

    let session = create_root_ipc_session(&data_directory)?;
    let session_guard = RootSessionGuard {
        data_directory,
        session_id: session.session_id,
    };
    let ipc = start_root_ipc(runtime.clone(), session)?;
    let reconciliation = start_filesystem_reconciliation_worker(runtime.clone());
    info!(%address, "starting MetaCog Protected Root compatibility API");
    let api_result = mcg_api::serve(address, runtime).await;
    if let Some(reconciliation) = reconciliation {
        reconciliation.abort();
    }
    ipc.abort();
    if let Some(supervisor) = runtime_worker_supervisor {
        supervisor.shutdown().await;
    }
    drop(session_guard);
    api_result?;
    info!("MetaCog Protected Root stopped cleanly");
    Ok(())
}

const ROOT_IPC_CONNECTION_LIMIT: usize = 32;
const ROOT_WRITER_QUEUE_CAPACITY: u32 = 64;
const ROOT_REPLAY_WINDOW: usize = 4096;

struct RootSessionGuard {
    data_directory: PathBuf,
    session_id: mcg_contracts::SessionId,
}

impl Drop for RootSessionGuard {
    fn drop(&mut self) {
        remove_root_ipc_session(&self.data_directory, self.session_id);
        #[cfg(unix)]
        {
            let _ = fs::remove_file(mcg_platform::root_ipc_endpoint(&self.data_directory));
        }
    }
}

#[derive(Default)]
struct ReplayWindow {
    order: VecDeque<RequestId>,
    seen: BTreeSet<RequestId>,
}

#[derive(serde::Deserialize)]
struct WorkflowShowRequest {
    workflow_id: WorkflowId,
}

#[derive(serde::Deserialize)]
struct WorkflowActivityRequest {
    workflow_id: WorkflowId,
    invocation_limit: usize,
}

#[derive(serde::Deserialize)]
struct ActionListRequest {
    limit: usize,
}

#[derive(serde::Deserialize)]
struct ActionShowRequest {
    action_id: ActionId,
    event_limit: usize,
}

#[derive(serde::Deserialize)]
struct InvocationListRequest {
    limit: usize,
}

#[derive(serde::Deserialize)]
struct InvocationShowRequest {
    invocation_id: InvocationId,
    model_run_limit: usize,
}

impl ReplayWindow {
    fn insert(&mut self, request_id: RequestId) -> bool {
        if !self.seen.insert(request_id) {
            return false;
        }
        self.order.push_back(request_id);
        if self.order.len() > ROOT_REPLAY_WINDOW
            && let Some(expired) = self.order.pop_front()
        {
            self.seen.remove(&expired);
        }
        true
    }
}

#[cfg(windows)]
fn start_root_ipc(
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
) -> Result<JoinHandle<()>, ProtectedRootError> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&session.endpoint)?;
    let replay = Arc::new(Mutex::new(ReplayWindow::default()));
    let permits = Arc::new(Semaphore::new(ROOT_IPC_CONNECTION_LIMIT));
    Ok(tokio::spawn(async move {
        let mut next = first;
        loop {
            if let Err(error) = next.connect().await {
                warn!(%error, "Protected Root named-pipe accept failed");
                break;
            }
            let connected = next;
            next = match ServerOptions::new()
                .reject_remote_clients(true)
                .create(&session.endpoint)
            {
                Ok(server) => server,
                Err(error) => {
                    warn!(%error, "Protected Root named-pipe instance creation failed");
                    break;
                }
            };
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let runtime = runtime.clone();
            let session = session.clone();
            let replay = replay.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = handle_root_ipc(connected, runtime, session, replay).await {
                    warn!(%error, "Protected Root IPC request rejected");
                }
            });
        }
    }))
}

#[cfg(unix)]
fn start_root_ipc(
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
) -> Result<JoinHandle<()>, ProtectedRootError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let endpoint = PathBuf::from(&session.endpoint);
    let _ = fs::remove_file(&endpoint);
    let listener = tokio::net::UnixListener::bind(&endpoint)?;
    fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
    let owner_uid = fs::metadata(endpoint.parent().unwrap_or(Path::new("/")))?.uid();
    let replay = Arc::new(Mutex::new(ReplayWindow::default()));
    let permits = Arc::new(Semaphore::new(ROOT_IPC_CONNECTION_LIMIT));
    Ok(tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(error) => {
                    warn!(%error, "Protected Root Unix-socket accept failed");
                    break;
                }
            };
            match stream.peer_cred() {
                Ok(credential) if credential.uid() == owner_uid => {}
                _ => continue,
            }
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let runtime = runtime.clone();
            let session = session.clone();
            let replay = replay.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = handle_root_ipc(stream, runtime, session, replay).await {
                    warn!(%error, "Protected Root IPC request rejected");
                }
            });
        }
    }))
}

async fn handle_root_ipc<S>(
    stream: S,
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
    replay: Arc<Mutex<ReplayWindow>>,
) -> Result<(), ProtectedRootError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handle_root_ipc_with_action_runner(stream, runtime, session, replay, None).await
}

async fn handle_root_ipc_with_action_runner<S>(
    mut stream: S,
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
    replay: Arc<Mutex<ReplayWindow>>,
    action_runner: Option<Arc<dyn GovernedActionRunnerGateway>>,
) -> Result<(), ProtectedRootError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut request: AuthenticatedRpcRequest = read_canonical_frame(&mut stream).await?;
    request.envelope.validate()?;
    let request_type = request.envelope.message_type.as_str().to_owned();
    let supported = matches!(
        request_type.as_str(),
        ROOT_STATUS_REQUEST_TYPE
            | ROOT_WORKFLOW_LIST_REQUEST_TYPE
            | ROOT_WORKFLOW_SHOW_REQUEST_TYPE
            | ROOT_WORKFLOW_ACTIVITY_REQUEST_TYPE
            | ROOT_WORKFLOW_CANCEL_REQUEST_TYPE
            | ROOT_ACTION_LIST_REQUEST_TYPE
            | ROOT_ACTION_SHOW_REQUEST_TYPE
            | ROOT_ACTION_COMPENSATE_REQUEST_TYPE
            | ROOT_INVOCATION_LIST_REQUEST_TYPE
            | ROOT_INVOCATION_SHOW_REQUEST_TYPE
    );
    if !session.verifies(&request.session_capability)
        || request.envelope.session_id != session.session_id
        || !supported
        || request.envelope.payload.type_id.as_str() != request_type
        || request
            .envelope
            .expires_at
            .is_none_or(|deadline| deadline < Utc::now())
    {
        return Err(PlatformError::InvalidSession.into());
    }
    request.session_capability.zeroize();
    if !replay
        .lock()
        .map_err(|_| PlatformError::InvalidSession)?
        .insert(request.envelope.request_id)
    {
        return Err(PlatformError::InvalidSession.into());
    }
    let ownership = runtime
        .root_ownership()?
        .ok_or_else(|| StorageError::InvalidData("Root ownership is not initialized".into()))?;
    let request_value = decode_canonical_cbor_v1(&request.envelope.payload.bytes)?;
    let (response_type, response_value) = dispatch_root_ipc_request(
        &runtime,
        &session,
        &ownership,
        request_type.as_str(),
        request_value,
        action_runner,
    )
    .await?;
    let response = RpcEnvelope {
        protocol_version: PROTOCOL_V1,
        request_id: request.envelope.request_id,
        correlation_id: request.envelope.correlation_id,
        session_id: session.session_id,
        caller_instance: ownership.runtime_identity.instance_id,
        message_type: StableTypeId::parse(response_type)?,
        expires_at: Some(Utc::now() + chrono::Duration::seconds(5)),
        payload: VersionedPayload::canonical_v1(
            StableTypeId::parse(response_type)?,
            &response_value,
        )?,
    };
    write_canonical_frame(&mut stream, &response).await?;
    Ok(())
}

async fn dispatch_root_ipc_request(
    runtime: &AppRuntime,
    session: &RootIpcSession,
    ownership: &mcg_storage::RootOwnership,
    request_type: &str,
    request_value: serde_json::Value,
    action_runner: Option<Arc<dyn GovernedActionRunnerGateway>>,
) -> Result<(&'static str, serde_json::Value), ProtectedRootError> {
    Ok(match request_type {
        ROOT_STATUS_REQUEST_TYPE => (
            ROOT_STATUS_RESPONSE_TYPE,
            serde_json::to_value(RootStatus {
                schema_version: 1,
                service: "protected-root".to_owned(),
                process_id: std::process::id(),
                protocol_version: PROTOCOL_V1,
                session_id: session.session_id,
                runtime_identity: ownership.runtime_identity.clone(),
                storage_schema_version: STORAGE_SCHEMA_VERSION,
                writer_queue_capacity: ROOT_WRITER_QUEUE_CAPACITY,
                migration_receipt: ownership.migration_receipt.clone(),
            })?,
        ),
        ROOT_WORKFLOW_LIST_REQUEST_TYPE => (
            ROOT_WORKFLOW_LIST_RESPONSE_TYPE,
            serde_json::to_value(runtime.canonical_workflows()?)?,
        ),
        ROOT_WORKFLOW_SHOW_REQUEST_TYPE => {
            let show: WorkflowShowRequest = serde_json::from_value(request_value)?;
            (
                ROOT_WORKFLOW_SHOW_RESPONSE_TYPE,
                serde_json::to_value(runtime.canonical_workflow(show.workflow_id)?)?,
            )
        }
        ROOT_WORKFLOW_ACTIVITY_REQUEST_TYPE => {
            let activity: WorkflowActivityRequest = serde_json::from_value(request_value)?;
            if activity.invocation_limit == 0
                || activity.invocation_limit > MAX_ROOT_WORKFLOW_INVOCATIONS
            {
                return Err(PlatformError::InvalidInspectionBound.into());
            }
            (
                ROOT_WORKFLOW_ACTIVITY_RESPONSE_TYPE,
                serde_json::to_value(runtime.canonical_workflow_activity(
                    activity.workflow_id,
                    activity.invocation_limit,
                )?)?,
            )
        }
        ROOT_WORKFLOW_CANCEL_REQUEST_TYPE => {
            let cancel: WorkflowCancelRequest = serde_json::from_value(request_value)?;
            (
                ROOT_WORKFLOW_CANCEL_RESPONSE_TYPE,
                serde_json::to_value(runtime.cancel_canonical_workflow(&cancel)?)?,
            )
        }
        ROOT_ACTION_LIST_REQUEST_TYPE => {
            let list: ActionListRequest = serde_json::from_value(request_value)?;
            (
                ROOT_ACTION_LIST_RESPONSE_TYPE,
                serde_json::to_value(runtime.canonical_actions(list.limit)?)?,
            )
        }
        ROOT_ACTION_SHOW_REQUEST_TYPE => {
            let show: ActionShowRequest = serde_json::from_value(request_value)?;
            (
                ROOT_ACTION_SHOW_RESPONSE_TYPE,
                serde_json::to_value(runtime.canonical_action(show.action_id, show.event_limit)?)?,
            )
        }
        ROOT_ACTION_COMPENSATE_REQUEST_TYPE => {
            let compensate: OwnerActionCompensationRequest = serde_json::from_value(request_value)?;
            let runner: Arc<dyn GovernedActionRunnerGateway> = match action_runner {
                Some(runner) => runner,
                None => Arc::new(ActionRunnerClient::adjacent_to(std::env::current_exe()?)?),
            };
            let receipt = execute_owner_authorized_filesystem_compensation(
                runtime,
                runner.as_ref(),
                &compensate,
                Utc::now(),
            )
            .await?;
            (
                ROOT_ACTION_COMPENSATE_RESPONSE_TYPE,
                serde_json::to_value(receipt)?,
            )
        }
        ROOT_INVOCATION_LIST_REQUEST_TYPE => {
            let list: InvocationListRequest = serde_json::from_value(request_value)?;
            if list.limit == 0 || list.limit > MAX_ROOT_INVOCATION_SUMMARIES {
                return Err(PlatformError::InvalidInspectionBound.into());
            }
            (
                ROOT_INVOCATION_LIST_RESPONSE_TYPE,
                serde_json::to_value(runtime.canonical_invocations(list.limit)?)?,
            )
        }
        ROOT_INVOCATION_SHOW_REQUEST_TYPE => {
            let show: InvocationShowRequest = serde_json::from_value(request_value)?;
            if show.model_run_limit == 0 || show.model_run_limit > MAX_ROOT_INVOCATION_MODEL_RUNS {
                return Err(PlatformError::InvalidInspectionBound.into());
            }
            (
                ROOT_INVOCATION_SHOW_RESPONSE_TYPE,
                serde_json::to_value(
                    runtime.runtime_invocation(show.invocation_id, show.model_run_limit)?,
                )?,
            )
        }
        _ => return Err(PlatformError::InvalidSession.into()),
    })
}

fn prepare_root_migration(config: &AppConfig) -> Result<RootMigrationReceipt, ProtectedRootError> {
    let database = config.database_path();
    let now = Utc::now();
    if !database.exists() {
        return Ok(RootMigrationReceipt {
            schema_version: 1,
            from_storage_schema: 0,
            to_storage_schema: STORAGE_SCHEMA_VERSION,
            backup_path: None,
            backup_sha256: None,
            migrated_at: now,
        });
    }

    let report = inspect_database_before_root(&database)?;
    if report.has_root_ownership {
        return Ok(RootMigrationReceipt {
            schema_version: 1,
            from_storage_schema: report.schema_version,
            to_storage_schema: STORAGE_SCHEMA_VERSION,
            backup_path: None,
            backup_sha256: None,
            migrated_at: now,
        });
    }

    let backup_directory = config.data_directory.join("backups");
    fs::create_dir_all(&backup_directory)?;
    let nonce = Uuid::new_v4().simple();
    let snapshot = backup_directory.join(format!(".pre-root-{nonce}.sqlite3"));
    let backup = backup_directory.join(format!(
        "pre-root-schema-{}-{}-{nonce}.mcgb",
        report.schema_version,
        now.format("%Y%m%dT%H%M%SZ")
    ));
    let result = (|| {
        snapshot_database_before_root(&database, &snapshot)?;
        create_backup_bundle_from_snapshot(
            &snapshot,
            &config.data_directory,
            &backup,
            env!("CARGO_PKG_VERSION"),
            now,
        )
    })();
    remove_snapshot_files(&snapshot);
    let summary = result?;
    #[cfg(debug_assertions)]
    if std::env::var_os("MCG_TEST_INTERRUPT_AFTER_ROOT_BACKUP").is_some() {
        return Err(ProtectedRootError::Io(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "test interruption after verified Root migration backup",
        )));
    }
    Ok(RootMigrationReceipt {
        schema_version: 1,
        from_storage_schema: report.schema_version,
        to_storage_schema: STORAGE_SCHEMA_VERSION,
        backup_path: Some(path_relative_to(&config.data_directory, &summary.path)),
        backup_sha256: Some(summary.sha256),
        migrated_at: now,
    })
}

fn current_runtime_identity() -> Result<RuntimeIdentity, ProtectedRootError> {
    let executable = std::env::current_exe()?;
    let bytes = fs::read(executable)?;
    let digest = Sha256::digest(bytes);
    Ok(RuntimeIdentity {
        instance_id: RuntimeInstanceId::new(),
        build_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: "RootExecutable".to_owned(),
            digest_hex: format!("{digest:x}"),
        },
        protocol_version: PROTOCOL_V1,
        started_at: Utc::now(),
    })
}

fn remove_snapshot_files(snapshot: &Path) {
    let mut wal = snapshot.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = snapshot.as_os_str().to_os_string();
    shm.push("-shm");
    for path in [snapshot.to_path_buf(), wal.into(), shm.into()] {
        let _ = fs::remove_file(path);
    }
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_action_runner::{
        ACTION_RUNNER_CONTENT_MAX_BYTES, GOVERNED_FILESYSTEM_OBSERVATION_TYPE,
        GovernedFilesystemAuthority, GovernedFilesystemOperation,
        governed_filesystem_authority_binding, governed_filesystem_operation_payload,
    };
    use mcg_contracts::{
        ActionEnvelope, ActionExecutionState, ActionId, ActionValidation, AttemptId,
        ConfigurationId, ContextManifest, ContextManifestId, ContextSelection, ContractClause,
        CorrelationId, EffectClass, HashAlgorithm, InvocationManifest, InvocationRole, ModelRun,
        ModelRunId, ModelRunState, PolicyBinding, RetryPolicy, RuntimeActorId, SessionId,
        SourceRef, TaskContract, TaskContractId, TaskContractStatus, VersionedRef, WaitReason,
        WorkflowAttempt, WorkflowId, WorkflowState,
    };
    use mcg_platform::{
        cancel_root_workflow, exchange_root_rpc, query_root_action, query_root_actions,
        query_root_invocation, query_root_invocations, query_root_status, query_root_workflow,
        query_root_workflow_activity, query_root_workflows, root_ipc_session_path,
    };
    use mcg_storage::{SqliteEventStore, task_contract_content_hash, verify_backup_bundle};

    fn workflow_hash(label: &str) -> ContentHash {
        VersionedPayload::canonical_v1(
            StableTypeId::parse("test.root_workflow_ref.v1").expect("reference type"),
            &serde_json::json!({"label": label}),
        )
        .expect("reference payload")
        .content_hash("TestRootWorkflowRef")
        .expect("reference hash")
    }

    fn workflow_policy(label: &str) -> PolicyBinding {
        let payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.root_workflow_policy.v1").expect("policy type"),
            &serde_json::json!({"policy": label}),
        )
        .expect("policy payload");
        let content_hash = payload
            .content_hash("TestRootWorkflowPolicy")
            .expect("policy hash");
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    fn root_invocation_payload(type_id: &str, label: &str) -> VersionedPayload {
        VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).expect("invocation payload type"),
            &serde_json::json!({"label": label}),
        )
        .expect("invocation payload")
    }

    fn root_invocation_source(label: &str) -> SourceRef {
        SourceRef {
            source_type: StableTypeId::parse("test.root_invocation_source.v1")
                .expect("invocation source type"),
            locator: format!("test:{label}"),
            content_hash: workflow_hash(label),
        }
    }

    fn persist_root_invocation(
        store: &mut SqliteEventStore,
        attempt: &WorkflowAttempt,
        at: chrono::DateTime<Utc>,
    ) -> (InvocationManifest, ModelRun) {
        let context = ContextManifest {
            id: ContextManifestId::new(),
            attempt_id: attempt.attempt_id,
            selected_sources: vec![ContextSelection {
                source: root_invocation_source("owner-context"),
                perspective: None,
                influence: root_invocation_payload("test.root_influence.v1", "trusted"),
                transformations: Vec::new(),
                placement: 0,
                approximate_tokens: 7,
            }],
            selection_policy: root_invocation_payload("test.root_context_policy.v1", "bounded"),
            expansion_steps: Vec::new(),
            approximate_tokens: 7,
        };
        let context_hash = store
            .persist_context_manifest(&context, at)
            .expect("persist Root IPC context");
        let runtime_build = workflow_hash("runtime-build");
        let manifest = InvocationManifest {
            id: InvocationId::new(),
            role: InvocationRole::Work,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId::new(),
                build_hash: runtime_build.clone(),
                protocol_version: PROTOCOL_V1,
                started_at: at,
            },
            cognitive_configuration: workflow_hash("cognitive-config"),
            role_manifest: workflow_hash("work-role"),
            prompt_bundle: workflow_hash("prompt-bundle"),
            context_manifest: context_hash,
            model_route: root_invocation_payload("test.root_model_route.v1", "deterministic"),
            resolved_provider: root_invocation_source("deterministic-provider"),
            resolved_model: root_invocation_source("deterministic-model"),
            observed_model_fingerprint: Some("deterministic-v1".into()),
            sampling: root_invocation_payload("test.root_sampling.v1", "zero"),
            reasoning: None,
            available_tool_manifests: Vec::new(),
            selected_skill_manifests: Vec::new(),
            authority_envelope: root_invocation_source("root-authority"),
            egress_permit: None,
            repository_revision: Some(workflow_hash("repository")),
            environment: root_invocation_source("isolated-environment"),
            runtime_build,
            request_hash: workflow_hash("request"),
            response_hash: None,
        };
        store
            .persist_invocation_manifest(&manifest, at)
            .expect("persist Root IPC invocation");
        let run = ModelRun {
            id: ModelRunId::new(),
            invocation_id: manifest.id,
            role: manifest.role,
            state: ModelRunState::Planned,
            provider_request_id: None,
            response_artifact: None,
            usage: None,
            started_at: at,
            finished_at: None,
        };
        store
            .create_model_run(&run, at)
            .expect("persist Root IPC model run");
        (manifest, run)
    }

    #[derive(Clone, Copy)]
    enum GatewayOutcome {
        Confirmed,
        LostAfterEffect,
        LostWithoutEffect,
        LaunchFailed,
    }

    struct ScriptedActionGateway(GatewayOutcome);

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for ScriptedActionGateway {
        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            match self.0 {
                GatewayOutcome::LaunchFailed => Err(ActionRunnerClientError::Launch),
                GatewayOutcome::LostWithoutEffect => Err(ActionRunnerClientError::Timeout),
                GatewayOutcome::LostAfterEffect => {
                    fs::write(
                        request.capability_root.join("authorized.txt"),
                        b"authorized",
                    )
                    .expect("script physical effect");
                    Err(ActionRunnerClientError::Timeout)
                }
                GatewayOutcome::Confirmed => {
                    let operation = decode_governed_filesystem_operation(&request.operation)
                        .expect("script operation");
                    let exists = match operation {
                        GovernedFilesystemOperation::CreateFile { .. } => {
                            fs::write(
                                request.capability_root.join("authorized.txt"),
                                b"authorized",
                            )
                            .expect("script physical effect");
                            true
                        }
                        GovernedFilesystemOperation::RemoveCreatedFile { .. } => {
                            fs::remove_file(request.capability_root.join("authorized.txt"))
                                .expect("script compensation effect");
                            false
                        }
                        _ => panic!("unexpected scripted effect"),
                    };
                    let now = Utc::now();
                    let observation = VersionedPayload::canonical_v1(
                        StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
                            .expect("observation type"),
                        &serde_json::json!({
                            "relative_path": "authorized.txt",
                            "exists": exists,
                            "sha256": exists.then(|| format!("{:x}", Sha256::digest(b"authorized"))),
                            "size_bytes": exists.then_some(10),
                            "content_base64": null,
                        }),
                    )
                    .expect("observation payload");
                    Ok(ActionRunnerResponse {
                        protocol_version: mcg_action_runner::ACTION_RUNNER_PROTOCOL_VERSION,
                        request_id: Some(request.request_id),
                        action_id: Some(request.action_id),
                        execution_id: Some(request.execution_id),
                        outcome: RunnerEffectOutcome::EffectConfirmed,
                        observation: Some(observation),
                        error_code: None,
                        detail: "confirmed scripted effect".into(),
                        started_at: now,
                        finished_at: now,
                    })
                }
            }
        }
    }

    struct InspectingActionGateway;

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for InspectingActionGateway {
        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            assert!(matches!(
                decode_governed_filesystem_operation(&request.operation)
                    .expect("reconciliation operation"),
                GovernedFilesystemOperation::InspectFile { .. }
            ));
            let bytes = fs::read(request.capability_root.join("authorized.txt")).ok();
            let now = Utc::now();
            let observation = VersionedPayload::canonical_v1(
                StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
                    .expect("observation type"),
                &serde_json::json!({
                    "relative_path": "authorized.txt",
                    "exists": bytes.is_some(),
                    "sha256": bytes.as_ref().map(|value| format!("{:x}", Sha256::digest(value))),
                    "size_bytes": bytes.as_ref().map(Vec::len),
                    "content_base64": null,
                }),
            )
            .expect("observation payload");
            Ok(ActionRunnerResponse {
                protocol_version: mcg_action_runner::ACTION_RUNNER_PROTOCOL_VERSION,
                request_id: Some(request.request_id),
                action_id: Some(request.action_id),
                execution_id: Some(request.execution_id),
                outcome: RunnerEffectOutcome::Observational,
                observation: Some(observation),
                error_code: None,
                detail: "scripted reconciliation inspection".into(),
                started_at: now,
                finished_at: now,
            })
        }
    }

    fn expired_workflow_fixture(at: chrono::DateTime<Utc>) -> (TaskContract, WorkflowAttempt) {
        let task_id = TaskContractId::new();
        let source = SourceRef {
            source_type: StableTypeId::parse("test.root_user_intent.v1").expect("source type"),
            locator: "test:root-restart".into(),
            content_hash: workflow_hash("intent"),
        };
        let criterion = ContractClause {
            clause_id: "restart".into(),
            statement: "expired work is fenced on Root restart".into(),
            source_refs: vec![source.clone()],
        };
        let contract = TaskContract {
            id: task_id,
            version: 1,
            previous_version: None,
            user_intent_ref: source,
            objective: "prove Root restart recovery".into(),
            non_goals: Vec::new(),
            requirements: vec![criterion.clone()],
            constraints: Vec::new(),
            assumptions: Vec::new(),
            acceptance_criteria: vec![criterion],
            dependencies: Vec::new(),
            permitted_side_effects: workflow_policy("none"),
            authority: workflow_policy("root"),
            environment: workflow_policy("test"),
            risk: workflow_policy("low"),
            budgets: workflow_policy("bounded"),
            rollback: workflow_policy("not-required"),
            status: TaskContractStatus::Active,
            created_at: at,
        };
        let mut attempt = WorkflowAttempt {
            workflow_id: WorkflowId::new(),
            attempt_id: AttemptId::new(),
            task_contract: VersionedRef {
                id: task_id,
                version: 1,
                content_hash: ContentHash {
                    algorithm: HashAlgorithm::Blake3_256,
                    domain: "MetaCog.TaskContract".into(),
                    digest_hex: "0".repeat(64),
                },
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
            environment_snapshot: workflow_hash("environment"),
            configuration: VersionedRef {
                id: ConfigurationId::new(),
                version: 1,
                content_hash: workflow_hash("configuration"),
            },
            idempotency_key: "root-restart-create".into(),
            retry_policy: RetryPolicy {
                max_attempts: 2,
                initial_backoff_ms: 100,
                maximum_backoff_ms: 1_000,
            },
            compensation_policy: workflow_policy("reconcile"),
            created_at: at,
            updated_at: at,
        };
        attempt.task_contract.content_hash =
            task_contract_content_hash(&contract).expect("task contract hash");
        (contract, attempt)
    }

    fn status_request(session: &RootIpcSession) -> AuthenticatedRpcRequest {
        AuthenticatedRpcRequest {
            envelope: RpcEnvelope {
                protocol_version: PROTOCOL_V1,
                request_id: RequestId::new(),
                correlation_id: CorrelationId::new(),
                session_id: session.session_id,
                caller_instance: RuntimeInstanceId::new(),
                message_type: StableTypeId::parse(ROOT_STATUS_REQUEST_TYPE)
                    .expect("status request type"),
                expires_at: Some(Utc::now() + chrono::Duration::seconds(5)),
                payload: VersionedPayload::canonical_v1(
                    StableTypeId::parse(ROOT_STATUS_REQUEST_TYPE).expect("status request type"),
                    &serde_json::json!({}),
                )
                .expect("status payload"),
            },
            session_capability: session.capability.clone(),
        }
    }

    #[test]
    fn existing_database_is_backed_up_and_root_identity_is_restart_stable() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        drop(SqliteEventStore::open(config.database_path()).expect("existing database"));

        let first = open(config.clone()).expect("first Protected Root open");
        let first_ownership = first
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        let backup = first_ownership
            .migration_receipt
            .backup_path
            .as_deref()
            .map(|path| temporary.path().join(path))
            .expect("verified pre-Root backup path");
        let backup_summary = verify_backup_bundle(&backup).expect("verified pre-Root backup");
        assert_eq!(
            first_ownership.migration_receipt.backup_sha256.as_deref(),
            Some(backup_summary.sha256.as_str())
        );
        assert!(
            open(config.clone()).is_err(),
            "a competing Root must not acquire authoritative write ownership"
        );
        let identity = first_ownership.runtime_identity.instance_id;
        drop(first);

        let second = open(config).expect("restarted Protected Root");
        let second_ownership = second
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        assert_eq!(second_ownership.runtime_identity.instance_id, identity);
        assert_eq!(
            second_ownership.migration_receipt,
            first_ownership.migration_receipt
        );
        assert_eq!(
            fs::read_dir(temporary.path().join("backups"))
                .expect("backup directory")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "mcgb"))
                .count(),
            1
        );
    }

    #[test]
    fn root_startup_fences_an_expired_canonical_workflow_generation() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let seeded_at = Utc::now() - chrono::Duration::minutes(3);
        let (contract, attempt) = expired_workflow_fixture(seeded_at);
        {
            let mut store = SqliteEventStore::open(config.database_path()).expect("workflow store");
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), seeded_at)
                .expect("create workflow");
            store
                .claim_workflow_attempt(
                    attempt.attempt_id,
                    1,
                    0,
                    RuntimeActorId::new(),
                    seeded_at + chrono::Duration::minutes(1),
                    CorrelationId::new(),
                    "root-restart-claim",
                    seeded_at,
                )
                .expect("claim workflow before simulated death");
        }

        let runtime = open(config.clone()).expect("restart Protected Root");
        drop(runtime);
        let store = SqliteEventStore::open(config.database_path()).expect("reopen recovered store");
        let recovered = store
            .workflow_attempt(attempt.attempt_id)
            .expect("workflow query")
            .expect("recovered workflow");
        assert_eq!(
            recovered.state,
            WorkflowState::Waiting(WaitReason::Reconciliation)
        );
        assert_eq!(recovered.revision, 3);
        assert_eq!(recovered.lease_generation, 2);
        assert!(recovered.lease_owner.is_none());
        assert_eq!(
            store
                .workflow_events(attempt.workflow_id)
                .expect("workflow events")
                .len(),
            3
        );
    }

    #[test]
    fn root_startup_resumes_elapsed_backoff_and_settles_effect_free_cancellation() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let seeded_at = Utc::now() - chrono::Duration::minutes(3);
        let (backoff_contract, backoff_attempt) = expired_workflow_fixture(seeded_at);
        let (approval_contract, approval_attempt) = expired_workflow_fixture(seeded_at);
        let (cancel_contract, cancel_attempt) = expired_workflow_fixture(seeded_at);
        let (backoff_waiting, approval_waiting, cancelling) = {
            let mut store = SqliteEventStore::open(config.database_path()).expect("workflow store");
            let mut seed = |contract: &TaskContract,
                            attempt: &WorkflowAttempt,
                            next_state: WorkflowState,
                            key: &str| {
                store
                    .create_workflow(contract, attempt, CorrelationId::new(), seeded_at)
                    .expect("create workflow");
                let running = store
                    .claim_workflow_attempt(
                        attempt.attempt_id,
                        attempt.revision,
                        attempt.lease_generation,
                        RuntimeActorId::new(),
                        seeded_at + chrono::Duration::minutes(5),
                        CorrelationId::new(),
                        &format!("{key}-claim"),
                        seeded_at,
                    )
                    .expect("claim workflow");
                store
                    .transition_workflow_attempt(
                        attempt.attempt_id,
                        running.revision,
                        running.lease_generation,
                        next_state,
                        CorrelationId::new(),
                        &format!("{key}-transition"),
                        seeded_at,
                    )
                    .expect("park workflow")
            };
            (
                seed(
                    &backoff_contract,
                    &backoff_attempt,
                    WorkflowState::Waiting(WaitReason::RetryBackoff),
                    "backoff",
                ),
                seed(
                    &approval_contract,
                    &approval_attempt,
                    WorkflowState::Waiting(WaitReason::Approval),
                    "approval",
                ),
                seed(
                    &cancel_contract,
                    &cancel_attempt,
                    WorkflowState::CancelRequested,
                    "cancel",
                ),
            )
        };

        let runtime = open(config.clone()).expect("restart Protected Root");
        drop(runtime);
        let store = SqliteEventStore::open(config.database_path()).expect("reopen recovered store");
        let recovered_backoff = store
            .workflow_attempt(backoff_waiting.attempt_id)
            .expect("backoff query")
            .expect("backoff workflow");
        assert_eq!(recovered_backoff.state, WorkflowState::Ready);
        assert_eq!(recovered_backoff.revision, backoff_waiting.revision + 1);
        assert_eq!(
            store
                .workflow_attempt(approval_waiting.attempt_id)
                .expect("approval query")
                .expect("approval workflow"),
            approval_waiting
        );
        let cancelled = store
            .workflow_attempt(cancelling.attempt_id)
            .expect("cancellation query")
            .expect("cancelling workflow");
        assert_eq!(cancelled.state, WorkflowState::Cancelled);
        assert_eq!(cancelled.revision, cancelling.revision + 1);
    }

    #[test]
    fn interrupted_after_backup_retries_without_mutating_the_source() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        drop(SqliteEventStore::open(config.database_path()).expect("existing database"));

        let interrupted = prepare_root_migration(&config).expect("pre-migration backup");
        let interrupted_backup = temporary.path().join(
            interrupted
                .backup_path
                .as_deref()
                .expect("interrupted migration backup"),
        );
        verify_backup_bundle(&interrupted_backup).expect("verified interrupted backup");
        let before_retry = inspect_database_before_root(&config.database_path())
            .expect("source remains inspectable");
        assert!(!before_retry.has_root_ownership);

        let runtime = open(config).expect("retry Protected Root migration");
        let ownership = runtime
            .root_ownership()
            .expect("ownership query")
            .expect("retry ownership");
        let retry_backup = temporary.path().join(
            ownership
                .migration_receipt
                .backup_path
                .as_deref()
                .expect("retry backup"),
        );
        assert_ne!(retry_backup, interrupted_backup);
        verify_backup_bundle(&retry_backup).expect("verified retry backup");
    }

    #[tokio::test]
    async fn authenticated_local_ipc_reports_the_same_root_identity() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = open(config).expect("Protected Root open");
        let ownership = runtime
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let status = query_root_status(temporary.path())
            .await
            .expect("authenticated status");
        assert_eq!(status.service, "protected-root");
        assert_eq!(status.session_id, session.session_id);
        assert_eq!(
            status.runtime_identity.instance_id,
            ownership.runtime_identity.instance_id
        );
        assert_eq!(status.storage_schema_version, STORAGE_SCHEMA_VERSION);

        let replayed = status_request(&session);
        exchange_root_rpc(&session, &replayed)
            .await
            .expect("first request with unique request ID");
        assert!(exchange_root_rpc(&session, &replayed).await.is_err());

        let mut expired = status_request(&session);
        expired.envelope.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(exchange_root_rpc(&session, &expired).await.is_err());

        let mut unsupported_protocol = status_request(&session);
        unsupported_protocol.envelope.protocol_version = PROTOCOL_V1 + 1;
        assert!(
            exchange_root_rpc(&session, &unsupported_protocol)
                .await
                .is_err()
        );

        let mut stale_session = status_request(&session);
        stale_session.envelope.session_id = SessionId::new();
        assert!(exchange_root_rpc(&session, &stale_session).await.is_err());

        let mut forged = session.clone();
        forged.capability = "0".repeat(64);
        fs::write(
            root_ipc_session_path(temporary.path()),
            serde_json::to_vec(&forged).expect("forged session JSON"),
        )
        .expect("replace client session");
        assert!(query_root_status(temporary.path()).await.is_err());

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }

    #[tokio::test]
    async fn authenticated_local_ipc_lists_shows_and_fences_workflow_cancellation() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let seeded_at = Utc::now();
        let (contract, attempt) = expired_workflow_fixture(seeded_at);
        let owner = RuntimeActorId::new();
        {
            let mut store = SqliteEventStore::open(config.database_path()).expect("workflow store");
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), seeded_at)
                .expect("create workflow");
            store
                .claim_workflow_attempt(
                    attempt.attempt_id,
                    1,
                    0,
                    owner,
                    seeded_at + chrono::Duration::minutes(5),
                    CorrelationId::new(),
                    "ipc-workflow-claim",
                    seeded_at,
                )
                .expect("claim workflow");
        }
        let runtime = open(config).expect("Protected Root open");
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let listed = query_root_workflows(temporary.path())
            .await
            .expect("workflow list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].workflow_id, attempt.workflow_id);
        let shown = query_root_workflow(temporary.path(), attempt.workflow_id)
            .await
            .expect("workflow show")
            .expect("workflow inspection");
        assert_eq!(shown.task_contract, contract);
        assert_eq!(shown.attempt.state, WorkflowState::Running);
        assert_eq!(shown.events.len(), 2);

        let request = WorkflowCancelRequest {
            attempt_id: shown.attempt.attempt_id,
            expected_revision: shown.attempt.revision,
            expected_generation: shown.attempt.lease_generation,
            idempotency_key: "ipc-workflow-cancel".into(),
        };
        let cancelling = cancel_root_workflow(temporary.path(), &request)
            .await
            .expect("cancel workflow");
        assert_eq!(cancelling.state, WorkflowState::CancelRequested);
        assert_eq!(
            cancelling.lease_generation,
            shown.attempt.lease_generation + 1
        );
        assert_eq!(
            cancel_root_workflow(temporary.path(), &request)
                .await
                .expect("exact cancellation replay"),
            cancelling
        );
        let after = query_root_workflow(temporary.path(), attempt.workflow_id)
            .await
            .expect("workflow after cancellation")
            .expect("workflow inspection after cancellation");
        assert_eq!(after.attempt, cancelling);
        assert_eq!(after.events.len(), 3);

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }

    #[tokio::test]
    async fn authenticated_local_ipc_lists_and_shows_bounded_invocation_evidence() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let seeded_at = Utc::now();
        let (contract, attempt) = expired_workflow_fixture(seeded_at);
        let (manifest, run) = {
            let mut store =
                SqliteEventStore::open(config.database_path()).expect("invocation store");
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), seeded_at)
                .expect("create invocation workflow");
            persist_root_invocation(&mut store, &attempt, seeded_at)
        };
        let runtime = open(config).expect("Protected Root open");
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let listed = query_root_invocations(temporary.path(), 10)
            .await
            .expect("invocation list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].invocation_id, manifest.id);
        let activity = query_root_workflow_activity(temporary.path(), attempt.workflow_id, 10)
            .await
            .expect("workflow activity")
            .expect("workflow activity inspection");
        assert_eq!(activity.workflow.task_contract, contract);
        assert_eq!(activity.invocations, listed);
        assert!(!activity.invocations_truncated);
        assert!(
            query_root_workflow_activity(temporary.path(), attempt.workflow_id, 0)
                .await
                .is_err()
        );
        assert_eq!(listed[0].workflow_id, attempt.workflow_id);
        assert_eq!(listed[0].model_run_id, Some(run.id));
        assert_eq!(listed[0].model_run_state, Some(ModelRunState::Planned));

        let shown = query_root_invocation(temporary.path(), manifest.id, 10)
            .await
            .expect("invocation show")
            .expect("invocation inspection");
        assert_eq!(shown.manifest, manifest);
        assert_eq!(shown.model_runs.len(), 1);
        assert_eq!(shown.model_runs[0].run, run);
        assert!(!shown.model_runs_truncated);
        assert!(query_root_invocations(temporary.path(), 0).await.is_err());
        assert!(
            query_root_invocation(temporary.path(), manifest.id, 0)
                .await
                .is_err()
        );

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }

    fn authorized_filesystem_fixture() -> (
        tempfile::TempDir,
        AppConfig,
        Arc<AppRuntime>,
        ActionExecution,
        RuntimeActorId,
        chrono::DateTime<Utc>,
    ) {
        let temporary = tempfile::tempdir().expect("temporary action Root");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let now = Utc::now();
        let (contract, attempt) = expired_workflow_fixture(now);
        let actor = RuntimeActorId::new();
        let running = {
            let mut store = SqliteEventStore::open(config.database_path()).expect("action store");
            store
                .create_workflow(&contract, &attempt, CorrelationId::new(), now)
                .expect("create action workflow");
            store
                .claim_workflow_attempt(
                    attempt.attempt_id,
                    attempt.revision,
                    attempt.lease_generation,
                    actor,
                    now + chrono::Duration::minutes(10),
                    CorrelationId::new(),
                    "action-workflow-claim",
                    now,
                )
                .expect("claim action workflow")
        };
        let runtime = open(config.clone()).expect("open action Root");
        let capability_root = temporary.path().join("capability-root");
        fs::create_dir(&capability_root).expect("create capability root");
        let action_id = ActionId::new();
        let operation = GovernedFilesystemOperation::CreateFile {
            relative_path: "authorized.txt".into(),
            content_base64: "YXV0aG9yaXplZA==".into(),
        };
        let (operation, operation_hash) =
            governed_filesystem_operation_payload(&operation).expect("operation payload");
        let (compensation, _) = governed_filesystem_operation_payload(
            &GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "authorized.txt".into(),
                expected_sha256: format!("{:x}", Sha256::digest(b"authorized")),
            },
        )
        .expect("compensation payload");
        let (reconciler, _) =
            governed_filesystem_operation_payload(&GovernedFilesystemOperation::InspectFile {
                relative_path: "authorized.txt".into(),
                max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
            })
            .expect("reconciler payload");
        let authority = governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
            action_id,
            actor,
            capability_root,
            operation_hash: operation_hash.clone(),
            effect_class: EffectClass::Reversible,
            expires_at: now + chrono::Duration::minutes(5),
        })
        .expect("authority payload");
        let envelope = ActionEnvelope {
            id: action_id,
            workflow_id: running.workflow_id,
            attempt_id: running.attempt_id,
            task_contract: VersionedRef {
                id: contract.id,
                version: contract.version,
                content_hash: task_contract_content_hash(&contract).expect("contract hash"),
            },
            actor,
            origin: None,
            authority,
            elevated_authority: None,
            expected_versions: vec![],
            operation,
            operation_hash,
            effect_class: EffectClass::Reversible,
            preconditions: vec![],
            budget_reservation: workflow_policy("action-budget"),
            idempotency_key: "authorized-filesystem-create".into(),
            compensation: Some(compensation),
            compensates_action: None,
            reconciler: Some(reconciler),
            reconciliation_deadline: Some(now + chrono::Duration::minutes(5)),
            created_at: now,
        };
        let execution = ActionExecution {
            id: ActionExecutionId::new(),
            action_id,
            attempt_number: 1,
            state: ActionExecutionState::Authorized,
            revision: 1,
            lease_generation: 0,
            executor: None,
            outcome: None,
            created_at: now,
            updated_at: now,
        };
        runtime
            .authorize_canonical_action(
                &envelope,
                &execution,
                running.revision,
                running.lease_generation,
                CorrelationId::new(),
                now,
            )
            .expect("authorize filesystem action");
        (temporary, config, runtime, execution, actor, now)
    }

    #[tokio::test]
    async fn root_classifies_confirmed_lost_and_prelaunch_filesystem_effects() {
        for (gateway_outcome, expected_state, effect_exists, expects_reconciliation) in [
            (
                GatewayOutcome::Confirmed,
                ActionExecutionState::EffectConfirmed,
                true,
                false,
            ),
            (
                GatewayOutcome::LostAfterEffect,
                ActionExecutionState::OutcomeUnknown,
                true,
                true,
            ),
            (
                GatewayOutcome::LostWithoutEffect,
                ActionExecutionState::OutcomeUnknown,
                false,
                true,
            ),
            (
                GatewayOutcome::LaunchFailed,
                ActionExecutionState::NoEffectConfirmed,
                false,
                false,
            ),
        ] {
            let (temporary, config, runtime, execution, _actor, now) =
                authorized_filesystem_fixture();
            let receipt = execute_authorized_filesystem_action(
                &runtime,
                &ScriptedActionGateway(gateway_outcome),
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                now,
            )
            .await
            .expect("execute governed action");
            assert_eq!(receipt.execution.state, expected_state);
            assert_eq!(receipt.reconciliation_job.is_some(), expects_reconciliation);
            assert_eq!(
                temporary
                    .path()
                    .join("capability-root/authorized.txt")
                    .exists(),
                effect_exists
            );
            let final_state = if let Some(job) = receipt.reconciliation_job {
                let reconcile_at = std::cmp::max(Utc::now(), job.next_attempt_at);
                let reconciled = reconcile_filesystem_action(
                    &runtime,
                    &InspectingActionGateway,
                    job.id,
                    job.revision,
                    job.lease_generation,
                    receipt.execution.revision,
                    receipt.execution.lease_generation,
                    RuntimeActorId::new(),
                    reconcile_at,
                )
                .await
                .expect("reconcile governed action");
                assert_eq!(
                    reconciled.resolution.job.state,
                    mcg_contracts::ReconciliationJobState::Resolved
                );
                if effect_exists {
                    ActionExecutionState::EffectConfirmed
                } else {
                    ActionExecutionState::NoEffectConfirmed
                }
            } else {
                expected_state
            };
            drop(runtime);
            let store =
                SqliteEventStore::open(config.database_path()).expect("reopen action store");
            assert_eq!(
                store
                    .action_execution(execution.id)
                    .expect("execution query")
                    .expect("durable execution")
                    .state,
                final_state
            );
        }
    }

    async fn commit_confirmed_filesystem_action(
        runtime: &AppRuntime,
        execution: &ActionExecution,
        actor: RuntimeActorId,
        now: chrono::DateTime<Utc>,
    ) -> ActionExecution {
        let dispatched = execute_authorized_filesystem_action(
            runtime,
            &ScriptedActionGateway(GatewayOutcome::Confirmed),
            execution.id,
            execution.revision,
            execution.lease_generation,
            actor,
            now,
        )
        .await
        .expect("confirmed action dispatch");
        let validation_time = now + chrono::Duration::milliseconds(1);
        let postcondition = dispatched
            .runner_response
            .as_ref()
            .and_then(|response| response.observation.clone())
            .expect("confirmed postcondition");
        let validation = ActionValidation {
            action_id: dispatched.execution.action_id,
            execution_id: dispatched.execution.id,
            validator: actor,
            evidence: Vec::new(),
            postconditions: vec![postcondition],
            validated_at: validation_time,
        };
        let validated = runtime
            .validate_canonical_action_effect(
                dispatched.execution.id,
                dispatched.execution.revision,
                dispatched.execution.lease_generation,
                &validation,
                CorrelationId::new(),
                &format!("test-validate:{}", dispatched.execution.action_id),
                validation_time,
            )
            .expect("validate confirmed action");
        let validation_event = runtime
            .canonical_action(dispatched.execution.action_id, 100)
            .expect("inspect validated action")
            .expect("validated action exists")
            .events
            .last()
            .expect("validation event")
            .id;
        runtime
            .commit_canonical_action_effect(
                validated.id,
                validated.revision,
                validated.lease_generation,
                validation_event,
                CorrelationId::new(),
                &format!("test-commit:{}", validated.action_id),
                validation_time + chrono::Duration::milliseconds(1),
            )
            .expect("commit validated action")
            .execution
    }

    #[tokio::test]
    async fn owner_compensation_is_root_derived_fenced_and_exactly_replay_safe() {
        let (temporary, _config, runtime, execution, actor, now) = authorized_filesystem_fixture();
        let committed = commit_confirmed_filesystem_action(&runtime, &execution, actor, now).await;
        assert_eq!(committed.state, ActionExecutionState::Committed);
        assert!(
            temporary
                .path()
                .join("capability-root/authorized.txt")
                .exists()
        );
        let workflow = runtime
            .canonical_workflow(
                runtime
                    .canonical_action(committed.action_id, 100)
                    .unwrap()
                    .unwrap()
                    .envelope
                    .workflow_id,
            )
            .unwrap()
            .unwrap();
        let request = OwnerActionCompensationRequest {
            action_id: committed.action_id,
            execution_id: committed.id,
            expected_action_revision: committed.revision,
            expected_action_generation: committed.lease_generation,
            expected_workflow_revision: workflow.attempt.revision,
            expected_workflow_generation: workflow.attempt.lease_generation,
            reason: "The owner requested removal of the exact file created by this action.".into(),
            confirmed: true,
            idempotency_key: "owner-compensation-test-1".into(),
        };
        let initiated = execute_owner_authorized_filesystem_compensation(
            &runtime,
            &ScriptedActionGateway(GatewayOutcome::Confirmed),
            &request,
            now + chrono::Duration::seconds(1),
        )
        .await
        .expect("initiate owner compensation");
        assert!(!initiated.replayed);
        assert_eq!(
            initiated.compensation.envelope.compensates_action,
            Some(committed.action_id)
        );
        assert_eq!(
            initiated.compensation.envelope.operation,
            runtime
                .canonical_action(committed.action_id, 100)
                .unwrap()
                .unwrap()
                .envelope
                .compensation
                .unwrap()
        );
        assert_eq!(
            initiated.compensation.executions.last().unwrap().state,
            ActionExecutionState::EffectConfirmed
        );
        assert!(
            !temporary
                .path()
                .join("capability-root/authorized.txt")
                .exists()
        );

        let replayed = execute_owner_authorized_filesystem_compensation(
            &runtime,
            &ScriptedActionGateway(GatewayOutcome::LaunchFailed),
            &request,
            now + chrono::Duration::seconds(2),
        )
        .await
        .expect("replay owner compensation");
        assert!(replayed.replayed);
        assert_eq!(
            replayed.compensation.envelope.id,
            initiated.compensation.envelope.id
        );
        assert_eq!(
            replayed.compensation.executions.last().unwrap().state,
            ActionExecutionState::EffectConfirmed
        );

        let mut substituted = request.clone();
        substituted.reason =
            "Substitute a different owner authorization under the same key.".into();
        assert!(
            execute_owner_authorized_filesystem_compensation(
                &runtime,
                &ScriptedActionGateway(GatewayOutcome::Confirmed),
                &substituted,
                now + chrono::Duration::milliseconds(2_500),
            )
            .await
            .is_err()
        );

        let mut stale = request.clone();
        stale.idempotency_key = "owner-compensation-stale".into();
        stale.expected_action_revision -= 1;
        assert!(
            execute_owner_authorized_filesystem_compensation(
                &runtime,
                &ScriptedActionGateway(GatewayOutcome::Confirmed),
                &stale,
                now + chrono::Duration::seconds(3),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn authenticated_root_ipc_accepts_only_the_typed_owner_compensation_request() {
        let (temporary, _config, runtime, execution, actor, now) = authorized_filesystem_fixture();
        let committed = commit_confirmed_filesystem_action(&runtime, &execution, actor, now).await;
        let workflow_id = runtime
            .canonical_action(committed.action_id, 100)
            .unwrap()
            .unwrap()
            .envelope
            .workflow_id;
        let workflow = runtime.canonical_workflow(workflow_id).unwrap().unwrap();
        let compensation = OwnerActionCompensationRequest {
            action_id: committed.action_id,
            execution_id: committed.id,
            expected_action_revision: committed.revision,
            expected_action_generation: committed.lease_generation,
            expected_workflow_revision: workflow.attempt.revision,
            expected_workflow_generation: workflow.attempt.lease_generation,
            reason: "Exercise the authenticated Root compensation protocol.".into(),
            confirmed: true,
            idempotency_key: "owner-compensation-ipc-1".into(),
        };
        let session = create_root_ipc_session(temporary.path()).expect("create Root IPC session");
        let request_id = RequestId::new();
        let correlation_id = CorrelationId::new();
        let request = AuthenticatedRpcRequest {
            envelope: RpcEnvelope {
                protocol_version: PROTOCOL_V1,
                request_id,
                correlation_id,
                session_id: session.session_id,
                caller_instance: RuntimeInstanceId::new(),
                message_type: StableTypeId::parse(ROOT_ACTION_COMPENSATE_REQUEST_TYPE).unwrap(),
                expires_at: Some(Utc::now() + chrono::Duration::seconds(5)),
                payload: VersionedPayload::canonical_v1(
                    StableTypeId::parse(ROOT_ACTION_COMPENSATE_REQUEST_TYPE).unwrap(),
                    &serde_json::to_value(&compensation).unwrap(),
                )
                .unwrap(),
            },
            session_capability: session.capability.clone(),
        };
        let (mut client, server) = tokio::io::duplex(mcg_platform::MAX_ROOT_IPC_FRAME_BYTES);
        let server_task = tokio::spawn(handle_root_ipc_with_action_runner(
            server,
            runtime,
            session.clone(),
            Arc::new(Mutex::new(ReplayWindow::default())),
            Some(Arc::new(ScriptedActionGateway(GatewayOutcome::Confirmed))),
        ));
        write_canonical_frame(&mut client, &request)
            .await
            .expect("write compensation request");
        let response: RpcEnvelope = read_canonical_frame(&mut client)
            .await
            .expect("read compensation response");
        server_task
            .await
            .expect("Root IPC task")
            .expect("Root IPC response");
        assert_eq!(response.correlation_id, correlation_id);
        assert_eq!(
            response.message_type.as_str(),
            ROOT_ACTION_COMPENSATE_RESPONSE_TYPE
        );
        let value = decode_canonical_cbor_v1(&response.payload.bytes).unwrap();
        let receipt: ActionCompensationReceipt = serde_json::from_value(value).unwrap();
        assert_eq!(receipt.original_action_id, committed.action_id);
        assert_eq!(
            receipt.compensation.envelope.compensates_action,
            Some(committed.action_id)
        );
        assert!(
            !temporary
                .path()
                .join("capability-root/authorized.txt")
                .exists()
        );
        remove_root_ipc_session(temporary.path(), session.session_id);
    }

    #[test]
    fn root_restart_turns_a_stranded_dispatch_into_reconciliation_without_redispatch() {
        let (_temporary, config, runtime, execution, _actor, now) = authorized_filesystem_fixture();
        let dispatched = runtime
            .dispatch_canonical_action(
                execution.id,
                execution.revision,
                execution.lease_generation,
                RuntimeActorId::new(),
                CorrelationId::new(),
                "root-restart-stranded-dispatch",
                now,
            )
            .expect("persist stranded dispatch");
        assert_eq!(dispatched.state, ActionExecutionState::Executing);
        drop(runtime);

        let reopened = open(config).expect("reopen Root after stranded dispatch");
        let (_, recovered) = reopened
            .canonical_action_for_execution(execution.id)
            .expect("load recovered action")
            .expect("recovered action exists");
        assert_eq!(recovered.state, ActionExecutionState::OutcomeUnknown);
        assert_eq!(
            reopened
                .due_canonical_reconciliations(Utc::now(), 10)
                .expect("due recovery jobs")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn reconciliation_never_claims_changed_owner_content_as_the_original_effect() {
        let (temporary, _config, runtime, execution, _actor, now) = authorized_filesystem_fixture();
        let dispatched = execute_authorized_filesystem_action(
            &runtime,
            &ScriptedActionGateway(GatewayOutcome::LostAfterEffect),
            execution.id,
            execution.revision,
            execution.lease_generation,
            RuntimeActorId::new(),
            now,
        )
        .await
        .expect("execute lost-response action");
        let job = dispatched.reconciliation_job.expect("reconciliation job");
        fs::write(
            temporary.path().join("capability-root/authorized.txt"),
            b"changed owner content",
        )
        .expect("replace with changed content");
        let reconcile_at = std::cmp::max(Utc::now(), job.next_attempt_at);
        let reconciled = reconcile_filesystem_action(
            &runtime,
            &InspectingActionGateway,
            job.id,
            job.revision,
            job.lease_generation,
            dispatched.execution.revision,
            dispatched.execution.lease_generation,
            RuntimeActorId::new(),
            reconcile_at,
        )
        .await
        .expect("defer ambiguous reconciliation");
        assert_eq!(
            reconciled.resolution.job.state,
            mcg_contracts::ReconciliationJobState::Pending
        );
        assert_eq!(
            reconciled.resolution.execution.state,
            ActionExecutionState::OutcomeUnknown
        );
        assert_eq!(
            fs::read(temporary.path().join("capability-root/authorized.txt"))
                .expect("changed content remains"),
            b"changed owner content"
        );
    }

    #[tokio::test]
    async fn authenticated_local_ipc_lists_and_shows_bounded_canonical_action_evidence() {
        let (temporary, _config, runtime, execution, _actor, _now) =
            authorized_filesystem_fixture();
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let actions = query_root_actions(temporary.path(), 10)
            .await
            .expect("action list");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, execution.action_id);
        assert_eq!(actions[0].state, ActionExecutionState::Authorized);
        let inspection = query_root_action(temporary.path(), execution.action_id, 10)
            .await
            .expect("action show")
            .expect("action inspection");
        assert_eq!(inspection.envelope.id, execution.action_id);
        assert_eq!(inspection.executions, vec![execution]);
        assert!(inspection.receipts.is_empty());
        assert!(inspection.reconciliation_jobs.is_empty());
        assert_eq!(inspection.events.len(), 1);

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }
}
