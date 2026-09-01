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
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, Weak},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{Duration, Utc};
use futures_core::Stream;
use futures_util::StreamExt as _;
use mcg_action_runner::{
    ACTION_RUNNER_CONTENT_MAX_BYTES, ActionRunnerAuthorization, ActionRunnerCapabilityEvidence,
    ActionRunnerClient, ActionRunnerClientError, ActionRunnerProtocolError, ActionRunnerRequest,
    ActionRunnerResponse, GOVERNED_ACTION_OPERATION_HASH_DOMAIN,
    GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN, GOVERNED_FILESYSTEM_OPERATION_TYPE,
    GovernedFilesystemAuthority, GovernedFilesystemObservation, GovernedFilesystemOperation,
    RunnerEffectOutcome, decode_governed_filesystem_authority,
    decode_governed_filesystem_observation, decode_governed_filesystem_operation,
    governed_filesystem_authority_binding, governed_filesystem_tool_manifest,
    governed_filesystem_tool_policy, request_from_authorized_action,
    request_from_reconciliation_job, validate_governed_filesystem_envelope,
};
use mcg_agents::{EvaluationOutcome, EvaluationReport};
use mcg_config::AppConfig;
use mcg_contracts::{
    ActionCompensationReceipt, ActionEnvelope, ActionExecution, ActionExecutionId,
    ActionExecutionState, ActionId, ActionOrigin, ActionReceipt, ActionReceiptId, ActionValidation,
    ArtifactRef, AuthenticatedRpcRequest, CompletionCertificate, CompletionCertificateId,
    CompletionVerdict, ContentHash, ContextManifest, ContextManifestId, ContextSelection,
    ContractError, CorrelationId, DeterministicCriterionEvidence, DeterministicVerifyInput,
    EffectClass, EffectOutcome, FilesystemAccess, HashAlgorithm, InvocationId, InvocationManifest,
    InvocationRole, ModelBudget, ModelDataClassification, ModelDestinationClass,
    ModelDisclosureManifest, ModelEgressPolicy, ModelEgressRoute, ModelInvocationPermit,
    ModelInvocationProposal, ModelRunCostProvenance, ModelRunErrorClass, ModelRunId, ModelUsage,
    OwnerActionCompensationRequest, PROTOCOL_V1, PermitId, PolicyBinding,
    RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN, ReconciliationJob, ReconciliationJobId,
    ReconciliationObservation, RequestId, RootStatus, RpcEnvelope, RuntimeActorId, RuntimeIdentity,
    RuntimeInstanceId, RuntimeInvocationRequest, RuntimeInvocationResult, SandboxCapabilityProfile,
    SecretHandleId, SourceRef, StableTypeId, ToolInvocationPermit, ToolInvocationProposal,
    ToolManifest, ToolPolicy, VersionedPayload, WorkflowAttempt, WorkflowCancelRequest, WorkflowId,
    WorkflowState, canonical_contract_hash, decode_canonical_cbor_v1,
    tool_invocation_proposal_hash,
};
use mcg_domain::{
    AgentLeaseId, EvaluationId, PluginCapabilityGrant, PluginCapabilityScope, PluginInvocationId,
    PluginRevisionId, ToolExecutionId, ToolRequestId,
};
use mcg_platform::{
    MAX_ROOT_INVOCATION_MODEL_RUNS, MAX_ROOT_INVOCATION_SUMMARIES,
    MAX_ROOT_TOOL_AUTHORIZATION_SUMMARIES, MAX_ROOT_WORKFLOW_INVOCATIONS, PlatformError,
    ROOT_ACTION_COMPENSATE_REQUEST_TYPE, ROOT_ACTION_COMPENSATE_RESPONSE_TYPE,
    ROOT_ACTION_LIST_REQUEST_TYPE, ROOT_ACTION_LIST_RESPONSE_TYPE, ROOT_ACTION_SHOW_REQUEST_TYPE,
    ROOT_ACTION_SHOW_RESPONSE_TYPE, ROOT_CAPABILITY_LIST_REQUEST_TYPE,
    ROOT_CAPABILITY_LIST_RESPONSE_TYPE, ROOT_CAPABILITY_SHOW_REQUEST_TYPE,
    ROOT_CAPABILITY_SHOW_RESPONSE_TYPE, ROOT_INVOCATION_LIST_REQUEST_TYPE,
    ROOT_INVOCATION_LIST_RESPONSE_TYPE, ROOT_INVOCATION_SHOW_REQUEST_TYPE,
    ROOT_INVOCATION_SHOW_RESPONSE_TYPE, ROOT_STATUS_REQUEST_TYPE, ROOT_STATUS_RESPONSE_TYPE,
    ROOT_WORKFLOW_ACTIVITY_REQUEST_TYPE, ROOT_WORKFLOW_ACTIVITY_RESPONSE_TYPE,
    ROOT_WORKFLOW_CANCEL_REQUEST_TYPE, ROOT_WORKFLOW_CANCEL_RESPONSE_TYPE,
    ROOT_WORKFLOW_LIST_REQUEST_TYPE, ROOT_WORKFLOW_LIST_RESPONSE_TYPE,
    ROOT_WORKFLOW_SHOW_REQUEST_TYPE, ROOT_WORKFLOW_SHOW_RESPONSE_TYPE, RootIpcSession,
    create_root_ipc_session, read_canonical_frame, remove_root_ipc_session, write_canonical_frame,
};
use mcg_plugin_protocol::{
    PluginCapability, PluginCapabilityOperation, PluginCapabilityOutcome, PluginCapabilityRequest,
    PluginGuestTurn, PluginHostTurn,
};
use mcg_plugin_runner::{
    PluginRunnerAuthorization, PluginRunnerClient, PluginRunnerClientError, PluginRunnerRequest,
    PluginRunnerResponse, current_execution_platform as plugin_execution_platform,
    governed_plugin_tool_manifest, governed_plugin_tool_policy, plugin_runner_invocation_artifact,
    plugin_runner_invocation_payload, plugin_runner_sandbox_profile,
};
use mcg_provider_runner::{
    ProviderRunnerAuthorization, ProviderRunnerClient, ProviderRunnerClientError,
    ProviderRunnerExecution, ProviderRunnerRequest, RunnerSecretHandleBinding, RunnerSecretSlot,
    model_provider_request_artifact as runner_request_artifact,
    model_provider_response_artifact as runner_response_artifact,
    model_provider_stream_request_artifact as runner_stream_request_artifact,
};
use mcg_providers::{
    CONTRACT_VERSION, ModelDescriptor, ModelRequest, ModelResponse, ModelStreamEvent,
    ProviderConnection, ProviderError, ProviderKind, ProviderStream, actual_cost,
};
use mcg_runtime::{
    AppRuntime, ProtectedAgentModelDispatcher, ProtectedCompatibilityVerifier,
    ProtectedPluginDispatcher, RuntimeError,
};
use mcg_storage::{
    AgentTurnVerificationTarget, AgentTurnWorkflowAttachment, ConsumerEvaluationCompletion,
    ModelEgressAuthorizationReceipt, ModelSecretHandleBinding, ModelSecretSlot,
    ReconciliationClaimReceipt, ReconciliationResolutionReceipt, RootMigrationReceipt,
    STORAGE_SCHEMA_VERSION, StorageError, ToolCapabilityAuthorizationReceipt,
    acquire_data_directory_lock, create_backup_bundle_from_snapshot, evaluation_report_source_ref,
    inspect_database_before_root, model_descriptor_source_ref, model_destination_source_ref,
    model_disclosure_hash, model_egress_policy_binding, model_egress_route_payload,
    model_proposal_hash, model_provider_source_ref, model_secret_slots_for_auth,
    snapshot_database_before_root, tool_policy_binding,
};
use mcg_tools::{
    TOOL_SCHEMA_VERSION, ToolError, ToolOutcome, ToolOutcomeStatus, VerificationEvidence,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Semaphore, mpsc, oneshot},
    task::JoinHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;
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
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("retryable provider gateway failure before provider contact: {0}")]
    RetryableProviderBeforeContact(ProviderError),
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

/// Root-owned, content-minimized authorization for one exact governed provider request.
pub type RuntimeModelEgressAuthorizationReceipt = ModelEgressAuthorizationReceipt;

/// Root-owned, content-minimized authorization for one exact governed Tool action.
pub type RuntimeToolCapabilityAuthorizationReceipt = ToolCapabilityAuthorizationReceipt;

#[derive(Debug, Clone)]
pub struct GovernedModelProviderDispatchReceipt {
    pub response: ModelResponse,
    pub completed_run: mcg_storage::ModelRunRecord,
}

/// Creates or replays the canonical workflow authority for one active compatibility agent lease.
///
/// Protected Root accepts only the opaque legacy lease identity. The authoritative store derives
/// the exact source snapshot, `TaskContract`, workflow, and attempt atomically, preventing Runtime
/// from supplying broader or substituted authority.
///
/// # Errors
/// Returns a protected-root error for unknown, inactive, expired, inconsistent, or unreadable
/// legacy authority.
pub fn attach_active_agent_lease_workflow(
    runtime: &AppRuntime,
    lease_id: AgentLeaseId,
    now: chrono::DateTime<Utc>,
) -> Result<AgentTurnWorkflowAttachment, ProtectedRootError> {
    Ok(runtime.root_attach_active_agent_lease_workflow(lease_id, now)?)
}

/// Atomically creates/reloads and claims canonical authority for one live compatibility lease.
///
/// # Errors
/// Returns an error when the legacy chain is stale, the canonical attempt is not Ready or already
/// owned by another actor, or durable evidence cannot be validated.
pub fn claim_active_agent_lease_workflow(
    runtime: &AppRuntime,
    lease_id: AgentLeaseId,
    lease_owner: RuntimeActorId,
    maximum_lease_seconds: u64,
    now: chrono::DateTime<Utc>,
) -> Result<mcg_storage::AgentTurnWorkflowAuthority, ProtectedRootError> {
    Ok(runtime.root_claim_active_agent_lease_workflow(
        lease_id,
        lease_owner,
        maximum_lease_seconds,
        now,
    )?)
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

/// Validates an untrusted Runtime model proposal against exact persisted context/provider state
/// and a Root-owned policy, then issues one immutable request-bound permit and opaque handles.
/// No credential value crosses this boundary.
///
/// Exact proposal/policy replay returns the original authority. Any route, request, disclosure,
/// budget, classification, expiry, or provider-state substitution fails closed.
///
/// # Errors
/// Returns an error for malformed, stale, unsupported, substituted, or over-policy proposals.
pub fn authorize_runtime_model_egress(
    runtime: &AppRuntime,
    context: &ContextManifest,
    proposal: &ModelInvocationProposal,
    expected_runtime_identity: &RuntimeIdentity,
    policy: &ModelEgressPolicy,
    now: chrono::DateTime<Utc>,
) -> Result<RuntimeModelEgressAuthorizationReceipt, ProtectedRootError> {
    proposal.validate()?;
    policy.validate()?;
    expected_runtime_identity.validate()?;
    if proposal.runtime_identity != *expected_runtime_identity {
        return Err(StorageError::Conflict(
            "model proposal does not belong to the authenticated Runtime identity".into(),
        )
        .into());
    }
    if context.attempt_id != proposal.attempt_id {
        return Err(StorageError::Conflict(
            "model proposal context belongs to a different workflow attempt".into(),
        )
        .into());
    }
    let context_hash = runtime.persist_model_egress_context(context, now)?;
    if proposal.disclosure.context_manifest != context_hash {
        return Err(StorageError::Conflict(
            "model proposal substituted its persisted context manifest".into(),
        )
        .into());
    }
    let policy_binding = model_egress_policy_binding(policy)?;
    if let Some(existing) =
        runtime.active_model_egress_authorization(proposal.invocation_id, now)?
    {
        if existing.proposal != *proposal || existing.permit.policy != policy_binding {
            return Err(StorageError::Conflict(
                "model egress replay substituted proposal or Root policy".into(),
            )
            .into());
        }
        return Ok(existing);
    }

    let (connection, catalog) = runtime
        .model_egress_route_state(&proposal.route.provider_id)?
        .ok_or_else(|| StorageError::InvalidData("model egress route was not found".into()))?;
    connection
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let destination_class = classify_model_destination(&connection)?;
    if proposal.route.destination != connection.base_url
        || proposal.route.destination_class != destination_class
        || !policy.allowed_routes.contains(&proposal.route)
    {
        return Err(StorageError::Conflict(
            "model proposal route is not the exact Root-allowed provider destination".into(),
        )
        .into());
    }
    let model = catalog
        .models
        .iter()
        .find(|model| model.id == proposal.route.model_id)
        .ok_or_else(|| StorageError::InvalidData("model egress model was not found".into()))?;
    let allowed_data_classification = proposal
        .data_classification
        .max(policy.minimum_data_classification);
    if allowed_data_classification > policy.maximum_data_classification
        || !proposal.requested_limits.fits_within(policy.maximum_limits)
    {
        return Err(StorageError::Conflict(
            "model proposal classification or budget exceeds Root policy".into(),
        )
        .into());
    }

    let permit_id = PermitId::new();
    let secret_slots = model_secret_slots_for_auth(connection.auth_mode);
    let secret_handles = secret_slots
        .iter()
        .map(|_| SecretHandleId::new())
        .collect::<Vec<_>>();
    let permit = ModelInvocationPermit {
        id: permit_id,
        invocation_id: proposal.invocation_id,
        attempt_id: proposal.attempt_id,
        attempt_revision: proposal.attempt_revision,
        lease_generation: proposal.lease_generation,
        runtime_actor: proposal.runtime_actor,
        runtime_identity: proposal.runtime_identity.clone(),
        role: proposal.role,
        task_contract: proposal.task_contract.clone(),
        proposal_hash: model_proposal_hash(proposal)?,
        request_hash: proposal.request_hash.clone(),
        resolved_provider: model_provider_source_ref(&connection)?,
        resolved_model: model_descriptor_source_ref(&connection.id, model)?,
        destination: model_destination_source_ref(&connection.base_url)?,
        destination_class,
        allowed_data_classification,
        disclosure_hash: model_disclosure_hash(&proposal.disclosure)?,
        limits: proposal.requested_limits,
        policy: policy_binding,
        expires_at: proposal.deadline,
        secret_handles: secret_handles.clone(),
        issued_at: now,
    };
    let bindings = secret_handles
        .into_iter()
        .zip(secret_slots)
        .map(|(handle_id, slot)| ModelSecretHandleBinding {
            handle_id,
            permit_id,
            connection_id: connection.id.clone(),
            slot,
            expires_at: permit.expires_at,
        })
        .collect::<Vec<_>>();
    Ok(runtime.authorize_model_egress(proposal, policy, &permit, &bindings, now)?)
}

/// Applies the Stage 6 Tool manifest and capability-intersection kernel inside Protected Root.
///
/// This function issues no ambient authority: the exact manifest version, argument artifact,
/// action/workflow fences, Runtime identity, effect, scopes, resource ceilings, sandbox
/// guarantees, platform, Root policy, and expiry are all bound into one immutable permit.
/// Registration or capability-index presence alone is never sufficient.
///
/// # Errors
/// Returns an error for malformed, substituted, over-scoped, under-isolated, unsupported,
/// expired, or unauthenticated proposals.
pub fn authorize_runtime_tool_capability(
    proposal: &ToolInvocationProposal,
    expected_runtime_identity: &RuntimeIdentity,
    manifest: &ToolManifest,
    policy: &ToolPolicy,
    available_sandbox: SandboxCapabilityProfile,
    platform: mcg_contracts::ExecutionPlatform,
    now: chrono::DateTime<Utc>,
) -> Result<ToolInvocationPermit, ProtectedRootError> {
    proposal.validate()?;
    expected_runtime_identity.validate()?;
    manifest.validate()?;
    policy.validate()?;
    if proposal.runtime_identity != *expected_runtime_identity {
        return Err(StorageError::Conflict(
            "tool proposal does not belong to the authenticated Runtime identity".into(),
        )
        .into());
    }

    let manifest_ref = manifest.versioned_ref()?;
    if proposal.platform != platform
        || proposal.tool_manifest != manifest_ref
        || !policy.allowed_manifests.contains(&manifest_ref)
        || !manifest.platform_support.contains(&platform)
    {
        return Err(StorageError::Conflict(
            "tool proposal does not resolve to an exact Root-allowed manifest and platform".into(),
        )
        .into());
    }
    if !manifest.allows_effect(proposal.requested_effect_class)
        || !policy.allows_effect(proposal.requested_effect_class)
    {
        return Err(StorageError::Conflict(
            "tool proposal effect exceeds manifest or Root policy".into(),
        )
        .into());
    }
    if proposal.requested_capabilities != manifest.required_capabilities
        || !proposal
            .requested_capabilities
            .is_subset_of(&policy.maximum_capabilities)
    {
        return Err(StorageError::Conflict(
            "tool proposal capabilities differ from the manifest or exceed Root policy".into(),
        )
        .into());
    }
    if !proposal
        .requested_limits
        .fits_within(manifest.resource_ceiling)
        || !proposal.requested_limits.fits_within(policy.maximum_limits)
    {
        return Err(StorageError::Conflict(
            "tool proposal resource limits exceed the manifest or Root policy".into(),
        )
        .into());
    }
    if proposal.required_sandbox != manifest.sandbox_requirement
        || !available_sandbox.satisfies(proposal.required_sandbox)
    {
        return Err(StorageError::Conflict(
            "host sandbox guarantees do not satisfy the exact Tool manifest".into(),
        )
        .into());
    }
    let maximum_lifetime = i64::try_from(policy.maximum_permit_lifetime_seconds)
        .map_err(|_| StorageError::InvalidData("tool permit lifetime is invalid".into()))?;
    if proposal.deadline <= now || proposal.deadline > now + Duration::seconds(maximum_lifetime) {
        return Err(
            StorageError::Conflict("tool proposal deadline exceeds Root policy".into()).into(),
        );
    }

    let policy_binding = tool_policy_binding(policy)?;
    let permit = ToolInvocationPermit {
        id: PermitId::new(),
        action_id: proposal.action_id,
        attempt_id: proposal.attempt_id,
        attempt_revision: proposal.attempt_revision,
        lease_generation: proposal.lease_generation,
        runtime_actor: proposal.runtime_actor,
        runtime_identity: proposal.runtime_identity.clone(),
        task_contract: proposal.task_contract.clone(),
        tool_manifest: manifest_ref,
        proposal_hash: tool_invocation_proposal_hash(proposal)?,
        argument_hash: proposal.argument_hash.clone(),
        allowed_effect_class: proposal.requested_effect_class,
        capabilities: proposal.requested_capabilities.clone(),
        limits: proposal.requested_limits,
        sandbox: available_sandbox,
        platform,
        policy: policy_binding,
        expires_at: proposal.deadline,
        secret_handles: proposal
            .requested_capabilities
            .secret_slots
            .iter()
            .map(|_| SecretHandleId::new())
            .collect(),
        issued_at: now,
    };
    permit.validate()?;
    Ok(permit)
}

/// Intersects one untrusted Runtime Tool proposal with the exact registered manifest, Root policy,
/// and measured host guarantees, then atomically persists the resulting permit and opaque handles.
/// Exact active replay returns the original durable authority; no manifest or profile registration
/// grants execution authority by itself.
///
/// # Errors
/// Returns an error for unregistered, inactive, expired, substituted, over-scoped,
/// under-isolated, unsupported, or unauthenticated proposals.
#[allow(clippy::too_many_arguments)]
pub fn authorize_and_persist_runtime_tool_capability(
    runtime: &AppRuntime,
    proposal: &ToolInvocationProposal,
    expected_runtime_identity: &RuntimeIdentity,
    manifest: &ToolManifest,
    policy: &ToolPolicy,
    available_sandbox: SandboxCapabilityProfile,
    platform: mcg_contracts::ExecutionPlatform,
    now: chrono::DateTime<Utc>,
) -> Result<RuntimeToolCapabilityAuthorizationReceipt, ProtectedRootError> {
    let registered = runtime
        .tool_manifest(manifest.id, manifest.version)?
        .ok_or_else(|| StorageError::InvalidData("Tool manifest is not registered".into()))?;
    if registered != *manifest {
        return Err(StorageError::Conflict(
            "Root Tool authorization received a manifest different from durable evidence".into(),
        )
        .into());
    }
    let expected_policy = tool_policy_binding(policy)?;
    if let Some(existing) = runtime.active_tool_capability_authorization(proposal.action_id, now)? {
        if existing.proposal != *proposal
            || existing.permit.policy != expected_policy
            || existing.permit.sandbox != available_sandbox
            || existing.permit.platform != platform
            || existing.permit.tool_manifest != manifest.versioned_ref()?
            || existing.permit.runtime_identity != *expected_runtime_identity
        {
            return Err(StorageError::Conflict(
                "Root Tool authorization replay substituted proposal, policy, manifest, or host"
                    .into(),
            )
            .into());
        }
        return Ok(existing);
    }
    let permit = authorize_runtime_tool_capability(
        proposal,
        expected_runtime_identity,
        manifest,
        policy,
        available_sandbox,
        platform,
        now,
    )?;
    Ok(runtime.authorize_tool_capability(proposal, policy, &permit, now)?)
}

#[allow(clippy::too_many_arguments)]
fn authorize_and_persist_reconciliation_tool_capability(
    runtime: &AppRuntime,
    proposal: &ToolInvocationProposal,
    expected_runtime_identity: &RuntimeIdentity,
    manifest: &ToolManifest,
    policy: &ToolPolicy,
    available_sandbox: SandboxCapabilityProfile,
    platform: mcg_contracts::ExecutionPlatform,
    job_id: ReconciliationJobId,
    now: chrono::DateTime<Utc>,
) -> Result<RuntimeToolCapabilityAuthorizationReceipt, ProtectedRootError> {
    let registered = runtime
        .tool_manifest(manifest.id, manifest.version)?
        .ok_or_else(|| StorageError::InvalidData("Tool manifest is not registered".into()))?;
    if registered != *manifest {
        return Err(StorageError::Conflict(
            "reconciliation Tool manifest differs from durable evidence".into(),
        )
        .into());
    }
    let permit = authorize_runtime_tool_capability(
        proposal,
        expected_runtime_identity,
        manifest,
        policy,
        available_sandbox,
        platform,
        now,
    )?;
    Ok(runtime.authorize_tool_reconciliation_capability(proposal, policy, &permit, job_id, now)?)
}

/// Canonically binds the exact provider request bytes authorized by Root.
///
/// # Errors
/// Returns an error for non-canonical or oversized provider request content.
pub fn model_provider_request_artifact(
    request: &ModelRequest,
) -> Result<ArtifactRef, ProtectedRootError> {
    runner_request_artifact(request).map_err(ProtectedRootError::Serialization)
}

/// Canonically binds streaming execution plus the exact provider request bytes authorized by Root.
///
/// # Errors
/// Returns an error for non-canonical or oversized provider request content.
pub fn model_provider_stream_request_artifact(
    request: &ModelRequest,
) -> Result<ArtifactRef, ProtectedRootError> {
    runner_stream_request_artifact(request).map_err(ProtectedRootError::Serialization)
}

fn model_provider_response_artifact(
    response: &ModelResponse,
) -> Result<ArtifactRef, ProtectedRootError> {
    runner_response_artifact(response).map_err(ProtectedRootError::Serialization)
}

const OBSERVED_MODEL_FINGERPRINT_TYPE: &str = "mcg.observed_model_fingerprint.v1";
const OBSERVED_MODEL_FINGERPRINT_DOMAIN: &str = "MetaCog.ObservedModelFingerprint";

#[derive(serde::Serialize)]
struct ObservedModelFingerprintInput<'a> {
    resolved_provider: &'a SourceRef,
    resolved_model: &'a SourceRef,
    request_hash: &'a ContentHash,
    policy_hash: &'a ContentHash,
    response_model: &'a str,
    reported_metadata_hashes: BTreeMap<String, String>,
}

fn observed_model_fingerprint(
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    response: &ModelResponse,
) -> Result<String, ProtectedRootError> {
    let mut reported_metadata_hashes = BTreeMap::new();
    for key in [
        "system_fingerprint",
        "model_revision",
        "revision",
        "version",
        "build",
        "service_tier",
    ] {
        if let Some(value) = response.provider_metadata.get(key) {
            let bytes = serde_json::to_vec(value)?;
            reported_metadata_hashes.insert(key.to_owned(), format!("{:x}", Sha256::digest(bytes)));
        }
    }
    let hash = canonical_contract_hash(
        OBSERVED_MODEL_FINGERPRINT_TYPE,
        OBSERVED_MODEL_FINGERPRINT_DOMAIN,
        &ObservedModelFingerprintInput {
            resolved_provider: &authorization.permit.resolved_provider,
            resolved_model: &authorization.permit.resolved_model,
            request_hash: &authorization.proposal.request_hash,
            policy_hash: &authorization.permit.policy.content_hash,
            response_model: &response.model,
            reported_metadata_hashes,
        },
    )?;
    let algorithm = match hash.algorithm {
        HashAlgorithm::Sha256 => "sha256",
        HashAlgorithm::Blake3_256 => "blake3-256",
    };
    Ok(format!("{}:{algorithm}:{}", hash.domain, hash.digest_hex))
}

fn observed_model_cost(
    runtime: &AppRuntime,
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    response: &ModelResponse,
) -> Result<(Option<u64>, ModelRunCostProvenance), ProtectedRootError> {
    let Some((_, catalog)) =
        runtime.model_egress_route_state(&authorization.proposal.route.provider_id)?
    else {
        return Ok((None, ModelRunCostProvenance::Unknown));
    };
    let Some(model) = catalog
        .models
        .iter()
        .find(|model| model.id == authorization.proposal.route.model_id)
    else {
        return Ok((None, ModelRunCostProvenance::Unknown));
    };
    if model_descriptor_source_ref(&authorization.proposal.route.provider_id, model)?
        != authorization.permit.resolved_model
    {
        return Ok((None, ModelRunCostProvenance::Unknown));
    }
    let Some(price) = model.price.as_ref().filter(|price| price.currency == "USD") else {
        return Ok((None, ModelRunCostProvenance::Unknown));
    };
    let Some(amount_usd) = actual_cost(Some(price), &response.usage).amount_usd else {
        return Ok((None, ModelRunCostProvenance::Unknown));
    };
    let microusd = amount_usd * 1_000_000.0;
    if !microusd.is_finite() || microusd < 0.0 {
        return Err(StorageError::InvalidData(
            "provider catalog produced an invalid observed cost".into(),
        )
        .into());
    }
    let microusd = format!("{microusd:.0}").parse::<u64>().map_err(|_| {
        StorageError::InvalidData("provider catalog cost exceeds the evidence range".into())
    })?;
    Ok((
        Some(microusd),
        ModelRunCostProvenance::ProviderCatalog {
            pricing_source: authorization.permit.resolved_model.clone(),
        },
    ))
}

#[async_trait::async_trait]
pub trait GovernedModelProviderGateway: Send + Sync {
    async fn invoke(
        &self,
        authorization: &RuntimeModelEgressAuthorizationReceipt,
        request: &ModelRequest,
    ) -> Result<ModelResponse, GovernedModelProviderGatewayError>;

    async fn stream(
        &self,
        _authorization: &RuntimeModelEgressAuthorizationReceipt,
        _request: &ModelRequest,
    ) -> Result<GovernedModelProviderStream, GovernedModelProviderGatewayError> {
        Err(GovernedModelProviderGatewayError::new(
            GovernedProviderFailureClass::FinalBeforeProviderContact,
            ProviderError::Configuration("governed provider streaming is unavailable".into()),
        ))
    }
}

pub type GovernedModelProviderStream =
    Pin<Box<dyn Stream<Item = Result<ModelStreamEvent, GovernedModelProviderGatewayError>> + Send>>;

struct CancellableGovernedProviderStream {
    receiver: UnboundedReceiverStream<Result<ModelStreamEvent, ProviderError>>,
    cancel: Option<oneshot::Sender<()>>,
}

impl Stream for CancellableGovernedProviderStream {
    type Item = Result<ModelStreamEvent, ProviderError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

impl Drop for CancellableGovernedProviderStream {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

/// Root's conservative classification of a physical gateway failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernedProviderFailureClass {
    /// The gateway proved that no provider contact occurred and a new authorized attempt may help.
    RetryableBeforeProviderContact,
    /// The gateway proved that no provider contact occurred, but repeating cannot safely help.
    FinalBeforeProviderContact,
    /// Provider contact or billing may have occurred; automatic retry is forbidden.
    OutcomeUnknown,
}

/// One bounded gateway error with an explicit provider-contact classification.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct GovernedModelProviderGatewayError {
    class: GovernedProviderFailureClass,
    error: ProviderError,
}

impl GovernedModelProviderGatewayError {
    #[must_use]
    pub const fn new(class: GovernedProviderFailureClass, error: ProviderError) -> Self {
        Self { class, error }
    }

    #[must_use]
    pub const fn class(&self) -> GovernedProviderFailureClass {
        self.class
    }

    #[must_use]
    pub fn into_provider_error(self) -> ProviderError {
        self.error
    }
}

/// Fixed adjacent process gateway for one exact Root-resolved provider and model.
#[derive(Debug, Clone)]
pub struct IsolatedProviderRunnerGateway {
    client: ProviderRunnerClient,
    connection: ProviderConnection,
    model: ModelDescriptor,
}

impl IsolatedProviderRunnerGateway {
    /// Resolves only the fixed provider runner adjacent to the trusted Root executable.
    ///
    /// # Errors
    /// Returns an error when the fixed adjacent runner is unavailable or unsafe.
    pub fn adjacent_to(
        root_executable: impl AsRef<Path>,
        connection: ProviderConnection,
        model: ModelDescriptor,
    ) -> Result<Self, ProtectedRootError> {
        Ok(Self {
            client: ProviderRunnerClient::adjacent_to(root_executable)
                .map_err(|error| ProviderError::Configuration(error.to_string()))?,
            connection,
            model,
        })
    }
}

#[async_trait::async_trait]
impl GovernedModelProviderGateway for IsolatedProviderRunnerGateway {
    async fn invoke(
        &self,
        authorization: &RuntimeModelEgressAuthorizationReceipt,
        request: &ModelRequest,
    ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
        self.client
            .invoke(&ProviderRunnerRequest {
                protocol_version: mcg_provider_runner::PROVIDER_RUNNER_PROTOCOL_VERSION,
                request_id: Uuid::new_v4(),
                execution: mcg_provider_runner::ProviderRunnerExecution::Complete,
                authorization: provider_runner_authorization(authorization),
                connection: self.connection.clone(),
                model: self.model.clone(),
                request: request.clone(),
            })
            .await
            .map_err(|error| classify_provider_runner_failure(&error))
    }

    async fn stream(
        &self,
        authorization: &RuntimeModelEgressAuthorizationReceipt,
        request: &ModelRequest,
    ) -> Result<GovernedModelProviderStream, GovernedModelProviderGatewayError> {
        let stream = self
            .client
            .stream(&ProviderRunnerRequest {
                protocol_version: mcg_provider_runner::PROVIDER_RUNNER_PROTOCOL_VERSION,
                request_id: Uuid::new_v4(),
                execution: ProviderRunnerExecution::Stream,
                authorization: provider_runner_authorization(authorization),
                connection: self.connection.clone(),
                model: self.model.clone(),
                request: request.clone(),
            })
            .await
            .map_err(|error| classify_provider_runner_failure(&error))?;
        Ok(Box::pin(stream.map(|event| {
            event.map_err(|error| classify_provider_runner_failure(&error))
        })))
    }
}

fn classify_provider_runner_failure(
    error: &ProviderRunnerClientError,
) -> GovernedModelProviderGatewayError {
    let class = match error {
        ProviderRunnerClientError::Unavailable | ProviderRunnerClientError::Launch => {
            GovernedProviderFailureClass::RetryableBeforeProviderContact
        }
        ProviderRunnerClientError::ExecutableChanged
        | ProviderRunnerClientError::RequestTooLarge
        | ProviderRunnerClientError::Denied(_) => {
            GovernedProviderFailureClass::FinalBeforeProviderContact
        }
        ProviderRunnerClientError::Delivery
        | ProviderRunnerClientError::Timeout
        | ProviderRunnerClientError::ResponseTooLarge
        | ProviderRunnerClientError::DiagnosticOutput
        | ProviderRunnerClientError::InvalidResponse
        | ProviderRunnerClientError::Process
        | ProviderRunnerClientError::Provider(_) => GovernedProviderFailureClass::OutcomeUnknown,
    };
    GovernedModelProviderGatewayError::new(
        class,
        ProviderError::Request {
            message: error.to_string(),
            retriable: class == GovernedProviderFailureClass::RetryableBeforeProviderContact,
        },
    )
}

const COMPATIBILITY_AGENT_WORKFLOW_LEASE_SECONDS: u64 = 5 * 60;
const COMPATIBILITY_AGENT_PERMIT_SECONDS: i64 = 2 * 60;
const COMPATIBILITY_AGENT_CALL_MAX_COST_MICROUSD: u64 = 5_000_000;
const COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS: u32 = 2;
const GOVERNED_PROVIDER_AUTHORITY_POLL_MILLISECONDS: u64 = 25;
const PLUGIN_WORKFLOW_LEASE_SECONDS: u64 = 5 * 60;
const PLUGIN_PERMIT_SECONDS: i64 = 2 * 60;

struct RootAgentModelDispatcher {
    runtime: Weak<AppRuntime>,
    root_executable: PathBuf,
    runtime_actor: RuntimeActorId,
    runtime_identity: RuntimeIdentity,
}

struct RootPluginDispatcher {
    runtime: Weak<AppRuntime>,
    root_executable: PathBuf,
    runtime_actor: RuntimeActorId,
    runtime_identity: RuntimeIdentity,
    capability_sessions: Mutex<BTreeMap<PluginInvocationId, PluginHostTurn>>,
    #[cfg(test)]
    test_gateway: Option<Arc<dyn RootPluginRunnerGateway>>,
    #[cfg(test)]
    test_action_gateway: Option<Arc<dyn GovernedActionRunnerGateway>>,
}

#[async_trait::async_trait]
trait RootPluginRunnerGateway: Send + Sync {
    fn runner_source(&self) -> &SourceRef;

    async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError>;
}

#[async_trait::async_trait]
impl RootPluginRunnerGateway for PluginRunnerClient {
    fn runner_source(&self) -> &SourceRef {
        PluginRunnerClient::runner_source(self)
    }

    async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
        PluginRunnerClient::invoke(self, request).await
    }
}

struct RootCompatibilityVerifier {
    runtime: Weak<AppRuntime>,
    root_executable: PathBuf,
    data_directory: PathBuf,
}

#[async_trait::async_trait]
impl ProtectedPluginDispatcher for RootPluginDispatcher {
    #[allow(clippy::too_many_lines)]
    async fn invoke(
        &self,
        invocation_id: PluginInvocationId,
        plugin_revision_id: PluginRevisionId,
        mut request: PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
        if request.authorization.is_some() {
            return Err(PluginRunnerClientError::Denied(
                "Runtime cannot author plugin Tool authority".into(),
            ));
        }
        let runtime = self.runtime.upgrade().ok_or_else(|| {
            PluginRunnerClientError::Denied("Protected Root runtime is unavailable".into())
        })?;
        let now = Utc::now();
        let authority = runtime
            .root_claim_active_plugin_invocation_workflow(
                invocation_id,
                self.runtime_actor,
                PLUGIN_WORKFLOW_LEASE_SECONDS,
                now,
            )
            .map_err(protected_plugin_denied)?;
        if authority.invocation.id != invocation_id
            || authority.invocation.plugin_revision_id != plugin_revision_id
            || authority.revision.id != plugin_revision_id
            || request.manifest != authority.revision.manifest
            || request.module_sha256 != authority.revision.module_sha256
        {
            return Err(PluginRunnerClientError::Denied(
                "plugin request differs from the exact durable invocation revision".into(),
            ));
        }
        let capability_aware = !authority
            .revision
            .manifest
            .requested_capabilities
            .is_empty();
        let original_input = if capability_aware {
            validate_protected_plugin_host_turn(
                &self.capability_sessions,
                invocation_id,
                &request.input,
            )?
        } else {
            request.input.clone()
        };
        let input_bytes = serde_json::to_vec(&original_input)
            .map_err(|error| PluginRunnerClientError::Denied(error.to_string()))?;
        if format!("{:x}", Sha256::digest(input_bytes)) != authority.invocation.input_sha256 {
            if capability_aware && let Ok(mut sessions) = self.capability_sessions.lock() {
                sessions.remove(&invocation_id);
            }
            return Err(PluginRunnerClientError::Denied(
                "plugin request input differs from the durable invocation".into(),
            ));
        }

        #[cfg(test)]
        let client: Arc<dyn RootPluginRunnerGateway> = if let Some(gateway) = &self.test_gateway {
            gateway.clone()
        } else {
            Arc::new(PluginRunnerClient::adjacent_to(&self.root_executable)?)
        };
        #[cfg(not(test))]
        let client: Arc<dyn RootPluginRunnerGateway> =
            Arc::new(PluginRunnerClient::adjacent_to(&self.root_executable)?);
        let platform = plugin_execution_platform();
        let manifest = governed_plugin_tool_manifest(
            client.runner_source(),
            &plugin_revision_id.to_string(),
            &request.manifest,
            &request.module_sha256,
            &request.limits,
            platform,
        )
        .map_err(PluginRunnerClientError::Denied)?;
        runtime
            .register_tool_manifest(&manifest, now)
            .map_err(protected_plugin_denied)?;
        let operation = plugin_runner_invocation_payload(
            &invocation_id.to_string(),
            &plugin_revision_id.to_string(),
            &request.manifest,
            &request.module_sha256,
            &request.input,
            &request.limits,
        )
        .map_err(PluginRunnerClientError::Denied)?;
        let arguments = plugin_runner_invocation_artifact(
            &invocation_id.to_string(),
            &plugin_revision_id.to_string(),
            &request.manifest,
            &request.module_sha256,
            &request.input,
            &request.limits,
        )
        .map_err(PluginRunnerClientError::Denied)?;
        let attempt = authority.workflow.attempt;
        let lease_expiry = attempt.lease_expires_at.ok_or_else(|| {
            PluginRunnerClientError::Denied("plugin workflow has no active lease".into())
        })?;
        let deadline = lease_expiry.min(now + Duration::seconds(PLUGIN_PERMIT_SECONDS));
        let action_id = ActionId::new();
        let execution_id = ActionExecutionId::new();
        let action_authority = policy_binding(
            "mcg.plugin_action_authority.v1",
            "MetaCog.PluginActionAuthority",
            &serde_json::json!({
                "invocation_id": invocation_id,
                "plugin_revision_id": plugin_revision_id,
                "source": authority.workflow.attachment.source,
            }),
        )
        .map_err(protected_plugin_denied)?;
        let budget = policy_binding(
            "mcg.plugin_action_budget.v1",
            "MetaCog.PluginActionBudget",
            &serde_json::json!({"limits": request.limits}),
        )
        .map_err(protected_plugin_denied)?;
        let envelope = ActionEnvelope {
            id: action_id,
            workflow_id: attempt.workflow_id,
            attempt_id: attempt.attempt_id,
            task_contract: attempt.task_contract.clone(),
            actor: self.runtime_actor,
            origin: Some(ActionOrigin {
                conversation: None,
                invocation: Some(authority.workflow.attachment.source),
                command: None,
                approval: None,
                agent: None,
                evidence: Vec::new(),
            }),
            authority: action_authority,
            elevated_authority: None,
            expected_versions: Vec::new(),
            operation,
            operation_hash: arguments.content_hash.clone(),
            effect_class: EffectClass::Observational,
            preconditions: Vec::new(),
            budget_reservation: budget,
            idempotency_key: format!("plugin-runner:{invocation_id}:{action_id}"),
            compensation: None,
            compensates_action: None,
            reconciler: None,
            reconciliation_deadline: None,
            created_at: now,
        };
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
        runtime
            .authorize_canonical_action(
                &envelope,
                &execution,
                attempt.revision,
                attempt.lease_generation,
                CorrelationId::new(),
                now,
            )
            .map_err(protected_plugin_denied)?;
        let proposal = ToolInvocationProposal {
            action_id,
            attempt_id: attempt.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            runtime_actor: self.runtime_actor,
            runtime_identity: self.runtime_identity.clone(),
            task_contract: attempt.task_contract,
            tool_manifest: manifest
                .versioned_ref()
                .map_err(|error| PluginRunnerClientError::Denied(error.to_string()))?,
            arguments,
            argument_hash: envelope.operation_hash.clone(),
            requested_effect_class: EffectClass::Observational,
            requested_capabilities: manifest.required_capabilities.clone(),
            requested_limits: manifest.resource_ceiling,
            required_sandbox: manifest.sandbox_requirement,
            platform,
            deadline,
        };
        let policy =
            governed_plugin_tool_policy(&manifest).map_err(PluginRunnerClientError::Denied)?;
        let receipt = authorize_and_persist_runtime_tool_capability(
            &runtime,
            &proposal,
            &self.runtime_identity,
            &manifest,
            &policy,
            plugin_runner_sandbox_profile(),
            platform,
            now,
        )
        .map_err(protected_plugin_denied)?;
        request.authorization = Some(PluginRunnerAuthorization {
            invocation_id: invocation_id.to_string(),
            plugin_revision_id: plugin_revision_id.to_string(),
            manifest,
            proposal: receipt.proposal,
            proposal_hash: receipt.proposal_hash,
            policy,
            permit: receipt.permit,
            permit_hash: receipt.permit_hash,
            permit_ref: receipt.permit_ref,
        });
        let dispatched = runtime
            .dispatch_canonical_action(
                execution_id,
                1,
                0,
                self.runtime_actor,
                CorrelationId::new(),
                &format!("dispatch-plugin-runner:{action_id}"),
                Utc::now(),
            )
            .map_err(protected_plugin_denied)?;
        let response = client.invoke(&request).await;
        if settle_observational_plugin_action(
            &runtime,
            action_id,
            execution_id,
            &dispatched,
            response.as_ref().ok(),
            response.as_ref().err(),
        )
        .is_err()
        {
            return Err(PluginRunnerClientError::Process);
        }
        if capability_aware {
            let awaiting_capability = response.as_ref().is_ok_and(|response| {
                response
                    .result
                    .as_ref()
                    .and_then(|result| {
                        serde_json::from_value::<PluginGuestTurn>(result.output.clone()).ok()
                    })
                    .is_some_and(|turn| matches!(turn, PluginGuestTurn::CapabilityRequest { .. }))
            });
            if !awaiting_capability && let Ok(mut sessions) = self.capability_sessions.lock() {
                sessions.remove(&invocation_id);
            }
        }
        response
    }

    async fn execute_capability(
        &self,
        invocation_id: PluginInvocationId,
        plugin_revision_id: PluginRevisionId,
        request: PluginCapabilityRequest,
    ) -> Result<PluginCapabilityOutcome, ToolError> {
        let result =
            execute_protected_plugin_capability(self, invocation_id, plugin_revision_id, request)
                .await;
        if result.is_err()
            && let Ok(mut sessions) = self.capability_sessions.lock()
        {
            sessions.remove(&invocation_id);
        }
        result
    }
}

fn validate_protected_plugin_host_turn(
    sessions: &Mutex<BTreeMap<PluginInvocationId, PluginHostTurn>>,
    invocation_id: PluginInvocationId,
    input: &serde_json::Value,
) -> Result<serde_json::Value, PluginRunnerClientError> {
    let host_turn: PluginHostTurn = serde_json::from_value(input.clone()).map_err(|_| {
        PluginRunnerClientError::Denied(
            "plugin host turn differs from the protected broker contract".into(),
        )
    })?;
    let mut sessions = sessions.lock().map_err(|_| {
        PluginRunnerClientError::Denied("plugin capability session is unavailable".into())
    })?;
    if let Some(expected) = sessions.get(&invocation_id) {
        if expected != &host_turn {
            return Err(PluginRunnerClientError::Denied(
                "plugin host turn substituted protected capability evidence".into(),
            ));
        }
    } else if host_turn.outcomes.is_empty() {
        sessions.insert(invocation_id, host_turn.clone());
    } else {
        return Err(PluginRunnerClientError::Denied(
            "plugin host turn supplied outcomes before protected dispatch".into(),
        ));
    }
    Ok(host_turn.original_input)
}

fn plugin_capability_request_is_fresh(session: &PluginHostTurn, request_id: &str) -> bool {
    !session.outcomes.iter().any(|outcome| match outcome {
        PluginCapabilityOutcome::Succeeded {
            request_id: observed,
            ..
        }
        | PluginCapabilityOutcome::Failed {
            request_id: observed,
            ..
        } => observed == request_id,
    })
}

#[allow(clippy::too_many_lines)]
async fn execute_protected_plugin_capability(
    dispatcher: &RootPluginDispatcher,
    invocation_id: PluginInvocationId,
    plugin_revision_id: PluginRevisionId,
    request: PluginCapabilityRequest,
) -> Result<PluginCapabilityOutcome, ToolError> {
    let runtime = dispatcher
        .runtime
        .upgrade()
        .ok_or_else(|| ToolError::Denied("Protected Root runtime is unavailable".into()))?;
    let now = Utc::now();
    let authority = runtime
        .root_claim_active_plugin_invocation_workflow(
            invocation_id,
            dispatcher.runtime_actor,
            PLUGIN_WORKFLOW_LEASE_SECONDS,
            now,
        )
        .map_err(protected_plugin_tool_denied)?;
    if authority.invocation.id != invocation_id
        || authority.invocation.plugin_revision_id != plugin_revision_id
        || authority.revision.id != plugin_revision_id
    {
        return Err(ToolError::Denied(
            "plugin capability differs from the exact durable invocation revision".into(),
        ));
    }
    request
        .validate(&authority.revision.manifest)
        .map_err(|error| ToolError::Denied(error.to_string()))?;
    {
        let sessions = dispatcher
            .capability_sessions
            .lock()
            .map_err(|_| ToolError::Execution("plugin capability session is unavailable".into()))?;
        let session = sessions.get(&invocation_id).ok_or_else(|| {
            ToolError::Denied("plugin capability request preceded its protected guest turn".into())
        })?;
        if !plugin_capability_request_is_fresh(session, &request.id) {
            return Err(ToolError::Denied(
                "plugin capability request identity was replayed".into(),
            ));
        }
    }

    let filesystem_write = match request.operation.clone() {
        PluginCapabilityOperation::FilesystemWrite {
            path,
            content,
            create_new,
        } => Some((path, content, create_new)),
        _ => None,
    };
    if let Some((path, content, create_new)) = filesystem_write {
        require_create_only_plugin_write(create_new)?;
        return execute_plugin_filesystem_create(
            dispatcher,
            runtime.as_ref(),
            &authority,
            request,
            path,
            content,
            now,
        )
        .await;
    }
    let (relative_path, max_bytes) = match &request.operation {
        PluginCapabilityOperation::FilesystemRead { path, max_bytes } => (path.clone(), *max_bytes),
        _ => {
            return Err(ToolError::Denied(
                "plugin capability operation is not yet permit-consuming".into(),
            ));
        }
    };
    require_plugin_task_project_read(&authority.installation.capability_grants)?;
    let task = authority
        .task
        .as_ref()
        .ok_or_else(|| ToolError::Denied("plugin capability has no source-bound task".into()))?;
    let project = authority
        .project
        .as_ref()
        .ok_or_else(|| ToolError::Denied("plugin capability has no source-bound project".into()))?;
    if task.project_id != Some(project.project_id) {
        return Err(ToolError::Denied(
            "plugin capability task and project authority diverge".into(),
        ));
    }
    let capability_root = PathBuf::from(&project.root);
    let operation = GovernedFilesystemOperation::ReadFile {
        relative_path,
        max_bytes,
    };
    let (operation, operation_hash) =
        mcg_action_runner::governed_filesystem_operation_payload(&operation)
            .map_err(ToolError::Denied)?;
    let attempt = &authority.workflow.attempt;
    let lease_expiry = attempt.lease_expires_at.ok_or_else(|| {
        ToolError::Denied("plugin capability workflow has no active lease".into())
    })?;
    let deadline = lease_expiry.min(now + Duration::seconds(PLUGIN_PERMIT_SECONDS));
    let action_id = ActionId::new();
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
    let action_authority = policy_binding(
        "mcg.plugin_capability_authority.v1",
        "MetaCog.PluginCapabilityAuthority",
        &serde_json::json!({
            "invocation_id": invocation_id,
            "plugin_revision_id": plugin_revision_id,
            "capability_request": request,
            "source": authority.workflow.attachment.source,
        }),
    )
    .map_err(protected_plugin_tool_denied)?;
    let budget = policy_binding(
        "mcg.plugin_capability_budget.v1",
        "MetaCog.PluginCapabilityBudget",
        &serde_json::json!({"maximum_read_bytes": max_bytes}),
    )
    .map_err(protected_plugin_tool_denied)?;
    let filesystem_authority =
        governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
            action_id,
            actor: dispatcher.runtime_actor,
            capability_root,
            operation_hash: operation_hash.clone(),
            effect_class: EffectClass::Observational,
            expires_at: deadline,
        })
        .map_err(ToolError::Denied)?;
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: attempt.workflow_id,
        attempt_id: attempt.attempt_id,
        task_contract: attempt.task_contract.clone(),
        actor: dispatcher.runtime_actor,
        origin: Some(ActionOrigin {
            conversation: None,
            invocation: Some(authority.workflow.attachment.source),
            command: None,
            approval: None,
            agent: None,
            evidence: Vec::new(),
        }),
        authority: filesystem_authority,
        elevated_authority: Some(action_authority),
        expected_versions: Vec::new(),
        operation,
        operation_hash,
        effect_class: EffectClass::Observational,
        preconditions: Vec::new(),
        budget_reservation: budget,
        idempotency_key: format!("plugin-capability:{invocation_id}:{}", request.id),
        compensation: None,
        compensates_action: None,
        reconciler: None,
        reconciliation_deadline: None,
        created_at: now,
    };
    runtime
        .authorize_canonical_action(
            &envelope,
            &execution,
            attempt.revision,
            attempt.lease_generation,
            CorrelationId::new(),
            now,
        )
        .map_err(protected_plugin_tool_denied)?;

    #[cfg(test)]
    let client: Arc<dyn GovernedActionRunnerGateway> =
        if let Some(gateway) = &dispatcher.test_action_gateway {
            gateway.clone()
        } else {
            Arc::new(
                ActionRunnerClient::adjacent_to(&dispatcher.root_executable)
                    .map_err(|error| ToolError::Execution(error.to_string()))?,
            )
        };
    #[cfg(not(test))]
    let client: Arc<dyn GovernedActionRunnerGateway> = Arc::new(
        ActionRunnerClient::adjacent_to(&dispatcher.root_executable)
            .map_err(|error| ToolError::Execution(error.to_string()))?,
    );
    let receipt = execute_authorized_filesystem_action(
        &runtime,
        client.as_ref(),
        execution.id,
        execution.revision,
        execution.lease_generation,
        dispatcher.runtime_actor,
        now,
    )
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
    let outcome = plugin_filesystem_read_outcome(&request.id, receipt)?;
    let mut sessions = dispatcher
        .capability_sessions
        .lock()
        .map_err(|_| ToolError::Execution("plugin capability session is unavailable".into()))?;
    let session = sessions.get_mut(&invocation_id).ok_or_else(|| {
        ToolError::Execution("plugin capability session disappeared during dispatch".into())
    })?;
    session.outcomes.push(outcome.clone());
    Ok(outcome)
}

fn require_create_only_plugin_write(create_new: bool) -> Result<(), ToolError> {
    if create_new {
        return Ok(());
    }
    Err(ToolError::Denied(
        "plugin filesystem replacement is awaiting exact reversible preimage authority".into(),
    ))
}

fn require_plugin_task_project_read(grants: &[PluginCapabilityGrant]) -> Result<(), ToolError> {
    let granted = grants.iter().any(|grant| {
        grant.capability == PluginCapability::FilesystemRead
            && grant.scopes.contains(&PluginCapabilityScope::TaskProject)
    });
    if granted {
        Ok(())
    } else {
        Err(ToolError::Denied(
            "plugin filesystem read is not granted for its task project".into(),
        ))
    }
}

fn require_plugin_task_project_write(grants: &[PluginCapabilityGrant]) -> Result<(), ToolError> {
    let granted = grants.iter().any(|grant| {
        grant.capability == PluginCapability::FilesystemWrite
            && grant.scopes.contains(&PluginCapabilityScope::TaskProject)
    });
    if granted {
        Ok(())
    } else {
        Err(ToolError::Denied(
            "plugin filesystem write is not granted for its task project".into(),
        ))
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn execute_plugin_filesystem_create(
    dispatcher: &RootPluginDispatcher,
    runtime: &AppRuntime,
    authority: &mcg_storage::PluginInvocationWorkflowAuthority,
    request: PluginCapabilityRequest,
    relative_path: String,
    content: String,
    now: chrono::DateTime<Utc>,
) -> Result<PluginCapabilityOutcome, ToolError> {
    require_plugin_task_project_write(&authority.installation.capability_grants)?;
    let task = authority
        .task
        .as_ref()
        .ok_or_else(|| ToolError::Denied("plugin capability has no source-bound task".into()))?;
    let project = authority
        .project
        .as_ref()
        .ok_or_else(|| ToolError::Denied("plugin capability has no source-bound project".into()))?;
    if task.project_id != Some(project.project_id) {
        return Err(ToolError::Denied(
            "plugin capability task and project authority diverge".into(),
        ));
    }
    let content_bytes = content.as_bytes();
    if u64::try_from(content_bytes.len()).unwrap_or(u64::MAX) > ACTION_RUNNER_CONTENT_MAX_BYTES {
        return Err(ToolError::Denied(
            "plugin filesystem write exceeds the governed action-runner bound".into(),
        ));
    }
    let content_sha256 = format!("{:x}", Sha256::digest(content_bytes));
    let operation = GovernedFilesystemOperation::CreateFile {
        relative_path: relative_path.clone(),
        content_base64: STANDARD.encode(content_bytes),
    };
    let compensation = GovernedFilesystemOperation::RemoveCreatedFile {
        relative_path: relative_path.clone(),
        expected_sha256: content_sha256.clone(),
    };
    let reconciler = GovernedFilesystemOperation::InspectFile {
        relative_path,
        max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
    };
    let (operation, operation_hash) =
        mcg_action_runner::governed_filesystem_operation_payload(&operation)
            .map_err(ToolError::Denied)?;
    let compensation = mcg_action_runner::governed_filesystem_operation_payload(&compensation)
        .map_err(ToolError::Denied)?
        .0;
    let reconciler = mcg_action_runner::governed_filesystem_operation_payload(&reconciler)
        .map_err(ToolError::Denied)?
        .0;
    let attempt = &authority.workflow.attempt;
    let lease_expiry = attempt.lease_expires_at.ok_or_else(|| {
        ToolError::Denied("plugin capability workflow has no active lease".into())
    })?;
    let deadline = lease_expiry.min(now + Duration::seconds(PLUGIN_PERMIT_SECONDS));
    let action_id = ActionId::new();
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
    let action_authority = policy_binding(
        "mcg.plugin_capability_authority.v1",
        "MetaCog.PluginCapabilityAuthority",
        &serde_json::json!({
            "invocation_id": authority.invocation.id,
            "plugin_revision_id": authority.revision.id,
            "capability_request": request,
            "source": authority.workflow.attachment.source,
        }),
    )
    .map_err(protected_plugin_tool_denied)?;
    let budget = policy_binding(
        "mcg.plugin_capability_budget.v1",
        "MetaCog.PluginCapabilityBudget",
        &serde_json::json!({
            "maximum_write_bytes": content_bytes.len(),
            "create_new": true,
        }),
    )
    .map_err(protected_plugin_tool_denied)?;
    let filesystem_authority =
        governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
            action_id,
            actor: dispatcher.runtime_actor,
            capability_root: PathBuf::from(&project.root),
            operation_hash: operation_hash.clone(),
            effect_class: EffectClass::Reversible,
            expires_at: deadline,
        })
        .map_err(ToolError::Denied)?;
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: attempt.workflow_id,
        attempt_id: attempt.attempt_id,
        task_contract: attempt.task_contract.clone(),
        actor: dispatcher.runtime_actor,
        origin: Some(ActionOrigin {
            conversation: None,
            invocation: Some(authority.workflow.attachment.source.clone()),
            command: None,
            approval: None,
            agent: None,
            evidence: Vec::new(),
        }),
        authority: filesystem_authority,
        elevated_authority: Some(action_authority),
        expected_versions: Vec::new(),
        operation,
        operation_hash,
        effect_class: EffectClass::Reversible,
        preconditions: Vec::new(),
        budget_reservation: budget,
        idempotency_key: format!(
            "plugin-capability:{}:{}",
            authority.invocation.id, request.id
        ),
        compensation: Some(compensation),
        compensates_action: None,
        reconciler: Some(reconciler),
        reconciliation_deadline: Some(deadline),
        created_at: now,
    };
    runtime
        .authorize_canonical_action(
            &envelope,
            &execution,
            attempt.revision,
            attempt.lease_generation,
            CorrelationId::new(),
            now,
        )
        .map_err(protected_plugin_tool_denied)?;

    #[cfg(test)]
    let client: Arc<dyn GovernedActionRunnerGateway> =
        if let Some(gateway) = &dispatcher.test_action_gateway {
            gateway.clone()
        } else {
            Arc::new(
                ActionRunnerClient::adjacent_to(&dispatcher.root_executable)
                    .map_err(|error| ToolError::Execution(error.to_string()))?,
            )
        };
    #[cfg(not(test))]
    let client: Arc<dyn GovernedActionRunnerGateway> = Arc::new(
        ActionRunnerClient::adjacent_to(&dispatcher.root_executable)
            .map_err(|error| ToolError::Execution(error.to_string()))?,
    );
    let receipt = execute_authorized_filesystem_action(
        runtime,
        client.as_ref(),
        execution.id,
        execution.revision,
        execution.lease_generation,
        dispatcher.runtime_actor,
        now,
    )
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
    let receipt =
        commit_confirmed_plugin_filesystem_effect(runtime, dispatcher.runtime_actor, receipt)?;
    let outcome = plugin_filesystem_create_outcome(&request.id, &content_sha256, receipt)?;
    let mut sessions = dispatcher
        .capability_sessions
        .lock()
        .map_err(|_| ToolError::Execution("plugin capability session is unavailable".into()))?;
    let session = sessions.get_mut(&authority.invocation.id).ok_or_else(|| {
        ToolError::Execution("plugin capability session disappeared during dispatch".into())
    })?;
    session.outcomes.push(outcome.clone());
    Ok(outcome)
}

fn commit_confirmed_plugin_filesystem_effect(
    runtime: &AppRuntime,
    validator: RuntimeActorId,
    mut receipt: GovernedActionDispatchReceipt,
) -> Result<GovernedActionDispatchReceipt, ToolError> {
    if !matches!(
        receipt.execution.outcome,
        Some(EffectOutcome::EffectConfirmed { .. })
    ) {
        return Ok(receipt);
    }
    let postcondition = receipt
        .runner_response
        .as_ref()
        .and_then(|response| response.observation.clone())
        .ok_or_else(|| ToolError::Execution("filesystem write postcondition is missing".into()))?;
    let now = Utc::now();
    let validation = ActionValidation {
        action_id: receipt.execution.action_id,
        execution_id: receipt.execution.id,
        validator,
        evidence: Vec::new(),
        postconditions: vec![postcondition],
        validated_at: now,
    };
    let validated = runtime
        .validate_canonical_action_effect(
            receipt.execution.id,
            receipt.execution.revision,
            receipt.execution.lease_generation,
            &validation,
            CorrelationId::new(),
            &format!("validate-plugin-filesystem:{}", receipt.execution.action_id),
            now,
        )
        .map_err(|error| ToolError::Execution(error.to_string()))?;
    let validation_event = runtime
        .canonical_action(validated.action_id, 32)
        .map_err(|error| ToolError::Execution(error.to_string()))?
        .and_then(|inspection| inspection.events.last().cloned())
        .ok_or_else(|| {
            ToolError::Execution("filesystem write validation event is missing".into())
        })?;
    receipt.execution = runtime
        .commit_canonical_action_effect(
            validated.id,
            validated.revision,
            validated.lease_generation,
            validation_event.id,
            CorrelationId::new(),
            &format!("commit-plugin-filesystem:{}", validated.action_id),
            Utc::now(),
        )
        .map_err(|error| ToolError::Execution(error.to_string()))?
        .execution;
    Ok(receipt)
}

fn plugin_filesystem_create_outcome(
    capability_request_id: &str,
    expected_sha256: &str,
    receipt: GovernedActionDispatchReceipt,
) -> Result<PluginCapabilityOutcome, ToolError> {
    match (&receipt.execution.state, &receipt.execution.outcome) {
        (ActionExecutionState::Committed, Some(EffectOutcome::EffectConfirmed { evidence })) => {
            Ok(PluginCapabilityOutcome::Succeeded {
                request_id: capability_request_id.into(),
                outcome: serde_json::json!({
                    "status": ToolOutcomeStatus::Succeeded,
                    "summary": "created governed project file",
                    "exit_code": null,
                    "stdout": null,
                    "stderr": null,
                    "artifacts": [],
                    "verification": [{
                        "method": "protected-root-filesystem-permit",
                        "passed": true,
                        "detail": format!(
                            "sha256={expected_sha256} evidence_count={}",
                            evidence.len()
                        ),
                    }],
                }),
            })
        }
        (
            ActionExecutionState::NoEffectConfirmed | ActionExecutionState::Denied,
            Some(EffectOutcome::NoEffectConfirmed),
        ) => Ok(PluginCapabilityOutcome::Failed {
            request_id: capability_request_id.into(),
            error_code: receipt
                .runner_response
                .and_then(|response| response.error_code)
                .unwrap_or_else(|| "filesystem_write_failed".into()),
        }),
        _ => Err(ToolError::Execution(
            "governed filesystem write outcome requires reconciliation".into(),
        )),
    }
}

fn plugin_filesystem_read_outcome(
    capability_request_id: &str,
    receipt: GovernedActionDispatchReceipt,
) -> Result<PluginCapabilityOutcome, ToolError> {
    let response = receipt.runner_response.ok_or_else(|| {
        ToolError::Execution("governed filesystem read returned no child evidence".into())
    })?;
    if response.outcome != RunnerEffectOutcome::Observational {
        return if matches!(
            response.outcome,
            RunnerEffectOutcome::Denied | RunnerEffectOutcome::NoEffectConfirmed
        ) {
            Ok(PluginCapabilityOutcome::Failed {
                request_id: capability_request_id.into(),
                error_code: response
                    .error_code
                    .unwrap_or_else(|| "filesystem_read_failed".into()),
            })
        } else {
            Err(ToolError::Execution(
                "governed filesystem read outcome is uncertain".into(),
            ))
        };
    }
    let observation = response
        .observation
        .as_ref()
        .ok_or_else(|| ToolError::Execution("filesystem read observation is missing".into()))
        .and_then(|payload| {
            decode_governed_filesystem_observation(payload)
                .map_err(|error| ToolError::Execution(error.to_string()))
        })?;
    let bytes = observation
        .content_base64
        .as_deref()
        .ok_or_else(|| ToolError::Execution("filesystem read content is missing".into()))
        .and_then(|content| {
            STANDARD
                .decode(content)
                .map_err(|error| ToolError::Execution(error.to_string()))
        })?;
    let outcome = ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id: ToolExecutionId::new(),
        request_id: ToolRequestId::new(),
        status: ToolOutcomeStatus::Succeeded,
        summary: format!(
            "read governed project file {} ({} bytes)",
            observation.relative_path,
            bytes.len()
        ),
        exit_code: None,
        stdout: Some(String::from_utf8_lossy(&bytes).into_owned()),
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: "protected-root-filesystem-permit".into(),
            passed: true,
            detail: format!(
                "sha256={} size_bytes={}",
                observation.sha256.unwrap_or_default(),
                observation.size_bytes.unwrap_or_default()
            ),
        }],
        started_at: response.started_at,
        finished_at: response.finished_at,
    };
    Ok(PluginCapabilityOutcome::Succeeded {
        request_id: capability_request_id.into(),
        outcome: serde_json::json!({
            "status": outcome.status,
            "summary": outcome.summary,
            "exit_code": outcome.exit_code,
            "stdout": outcome.stdout,
            "stderr": outcome.stderr,
            "artifacts": outcome.artifacts,
            "verification": outcome.verification,
        }),
    })
}

fn protected_plugin_tool_denied(error: impl std::fmt::Display) -> ToolError {
    ToolError::Denied(error.to_string())
}

fn protected_plugin_denied(error: impl std::fmt::Display) -> PluginRunnerClientError {
    PluginRunnerClientError::Denied(error.to_string())
}

fn settle_observational_plugin_action(
    runtime: &AppRuntime,
    action_id: ActionId,
    execution_id: ActionExecutionId,
    dispatched: &ActionExecution,
    response: Option<&PluginRunnerResponse>,
    error: Option<&PluginRunnerClientError>,
) -> Result<(), RuntimeError> {
    let now = Utc::now();
    let response_sha256 = response
        .map(serde_json::to_vec)
        .transpose()
        .map_err(StorageError::from)?
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    let observation = VersionedPayload::canonical_v1(
        StableTypeId::parse("mcg.plugin_runner_observation.v1")
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        &serde_json::json!({
            "response_sha256": response_sha256,
            "error_code": error.map(plugin_runner_client_error_code),
        }),
    )
    .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let observed = runtime.observe_canonical_action(
        &ActionReceipt {
            id: ActionReceiptId::new(),
            action_id,
            execution_id,
            observation,
            observed_at: now,
        },
        dispatched.revision,
        dispatched.lease_generation,
        CorrelationId::new(),
        &format!("observe-plugin-runner:{action_id}"),
        now,
    )?;
    runtime.classify_canonical_action(
        execution_id,
        observed.revision,
        observed.lease_generation,
        EffectOutcome::NoEffectConfirmed,
        None,
        CorrelationId::new(),
        &format!("classify-plugin-runner:{action_id}"),
        now,
    )?;
    Ok(())
}

const fn plugin_runner_client_error_code(error: &PluginRunnerClientError) -> &'static str {
    match error {
        PluginRunnerClientError::Unavailable => "unavailable",
        PluginRunnerClientError::ExecutableChanged => "executable_changed",
        PluginRunnerClientError::Denied(_) => "denied",
        PluginRunnerClientError::RequestTooLarge => "request_too_large",
        PluginRunnerClientError::Timeout => "timeout",
        PluginRunnerClientError::ResponseTooLarge => "response_too_large",
        PluginRunnerClientError::DiagnosticOutput => "diagnostic_output",
        PluginRunnerClientError::InvalidResponse => "invalid_response",
        PluginRunnerClientError::Process => "process",
    }
}

#[async_trait::async_trait]
impl ProtectedAgentModelDispatcher for RootAgentModelDispatcher {
    async fn create_response(
        &self,
        lease_id: AgentLeaseId,
        target: &mcg_providers::ProviderModelRef,
        request: ModelRequest,
    ) -> Result<ModelResponse, ProviderError> {
        let runtime = self.runtime.upgrade().ok_or_else(|| {
            ProviderError::Configuration("Protected Root runtime is unavailable".into())
        })?;
        let _oauth_execution_guard = runtime
            .lock_openai_oauth_execution(&target.provider_id)
            .await
            .map_err(protected_agent_provider_error)?;
        let (connection, catalog) = runtime
            .model_egress_route_state(&target.provider_id)
            .map_err(protected_agent_provider_error)?
            .ok_or_else(|| ProviderError::Configuration("provider route is unavailable".into()))?;
        let model = catalog
            .models
            .into_iter()
            .find(|model| model.id == target.model_id)
            .ok_or_else(|| ProviderError::Configuration("provider model is unavailable".into()))?;
        let gateway = IsolatedProviderRunnerGateway::adjacent_to(
            &self.root_executable,
            connection.clone(),
            model.clone(),
        )
        .map_err(protected_agent_provider_error)?;
        execute_compatibility_agent_model_invocation(
            &runtime,
            &gateway,
            &connection,
            &model,
            self.runtime_actor,
            &self.runtime_identity,
            lease_id,
            target,
            request,
            Utc::now(),
        )
        .await
        .map_err(protected_agent_provider_error)
    }

    async fn stream_response(
        &self,
        lease_id: AgentLeaseId,
        target: &mcg_providers::ProviderModelRef,
        request: ModelRequest,
    ) -> Result<ProviderStream, ProviderError> {
        let runtime = self.runtime.upgrade().ok_or_else(|| {
            ProviderError::Configuration("Protected Root runtime is unavailable".into())
        })?;
        let oauth_execution_guard = runtime
            .lock_openai_oauth_execution(&target.provider_id)
            .await
            .map_err(protected_agent_provider_error)?;
        let (connection, catalog) = runtime
            .model_egress_route_state(&target.provider_id)
            .map_err(protected_agent_provider_error)?
            .ok_or_else(|| ProviderError::Configuration("provider route is unavailable".into()))?;
        let model = catalog
            .models
            .into_iter()
            .find(|model| model.id == target.model_id)
            .ok_or_else(|| ProviderError::Configuration("provider model is unavailable".into()))?;
        if model.capabilities.streaming != Some(true) {
            return Err(ProviderError::Configuration(
                "the governed provider model does not declare streaming support".into(),
            ));
        }
        let gateway = IsolatedProviderRunnerGateway::adjacent_to(
            &self.root_executable,
            connection.clone(),
            model.clone(),
        )
        .map_err(protected_agent_provider_error)?;
        let stream = execute_compatibility_agent_model_stream(
            runtime,
            &gateway,
            &connection,
            &model,
            self.runtime_actor,
            &self.runtime_identity,
            lease_id,
            target,
            request,
            Utc::now(),
        )
        .await
        .map_err(protected_agent_provider_error)?;
        Ok(Box::pin(stream.map(move |event| {
            let _guard = &oauth_execution_guard;
            event
        })))
    }
}

#[async_trait::async_trait]
impl ProtectedCompatibilityVerifier for RootCompatibilityVerifier {
    async fn verify_consumer_evaluation(
        &self,
        evaluation_id: EvaluationId,
    ) -> Result<Vec<ConsumerEvaluationCompletion>, String> {
        let runtime = self
            .runtime
            .upgrade()
            .ok_or_else(|| "Protected Root runtime is unavailable".to_owned())?;
        let report = runtime
            .root_prepared_consumer_evaluation(evaluation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "prepared evaluation report is unavailable".to_owned())?;
        let targets = runtime
            .root_agent_turn_verification_targets(report.run_id)
            .map_err(|error| error.to_string())?;
        verify_compatibility_consumer_evaluation(
            &runtime,
            &self.root_executable,
            &self.data_directory,
            &report,
            &targets,
        )
        .await
        .map_err(|error| error.to_string())
    }
}

fn compatibility_verify_payload(
    type_id: &str,
    value: &serde_json::Value,
) -> Result<VersionedPayload, ProtectedRootError> {
    Ok(VersionedPayload::canonical_v1(
        StableTypeId::parse(type_id)?,
        value,
    )?)
}

fn compatibility_verify_hash(domain: &str, value: &[u8]) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(b"MetaCog.CompatibilityIndependentVerify:v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(value);
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        domain: domain.into(),
        digest_hex: format!("{:x}", hasher.finalize()),
    }
}

fn compatibility_verify_source(
    type_id: &str,
    locator: impl Into<String>,
    domain: &str,
    value: &[u8],
) -> Result<SourceRef, ProtectedRootError> {
    Ok(SourceRef {
        source_type: StableTypeId::parse(type_id)?,
        locator: locator.into(),
        content_hash: compatibility_verify_hash(domain, value),
    })
}

#[allow(clippy::too_many_lines)] // Keep one complete immutable Verify manifest auditable.
fn compatibility_verify_request(
    client: &RuntimeWorkerClient,
    target: &AgentTurnVerificationTarget,
    report: &EvaluationReport,
    report_source: &SourceRef,
) -> Result<RuntimeInvocationRequest, ProtectedRootError> {
    let criteria = target
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
            DeterministicCriterionEvidence {
                clause_id: clause.clause_id.clone(),
                passed,
                evidence_refs: vec![report_source.clone()],
            }
        })
        .collect::<Vec<_>>();
    let verify_input = DeterministicVerifyInput {
        work_response_hash: target.work_response_hash.clone(),
        criteria,
    };
    verify_input.validate()?;
    let input = compatibility_verify_payload(
        DeterministicVerifyInput::TYPE_ID,
        &serde_json::to_value(&verify_input)?,
    )?;
    let influence = compatibility_verify_payload(
        "mcg.compatibility_verify_influence.v1",
        &serde_json::json!({"influence": "objective_evaluation"}),
    )?;
    let selection_policy = compatibility_verify_payload(
        "mcg.compatibility_verify_context_policy.v1",
        &serde_json::json!({"mode": "exact_prepared_evaluation_only"}),
    )?;
    let context_manifest = ContextManifest {
        id: ContextManifestId::new(),
        attempt_id: target.attempt.attempt_id,
        selected_sources: vec![ContextSelection {
            source: report_source.clone(),
            perspective: None,
            influence,
            transformations: Vec::new(),
            placement: 0,
            approximate_tokens: 1,
        }],
        selection_policy,
        expansion_steps: Vec::new(),
        approximate_tokens: 1,
    };
    let context_hash = canonical_contract_hash(
        ContextManifest::TYPE_ID,
        "MetaCog.ContextManifest",
        &context_manifest,
    )?;
    let route = compatibility_verify_payload(
        "mcg.compatibility_verify_route.v1",
        &serde_json::json!({"provider": "deterministic", "role": "verify"}),
    )?;
    let sampling = compatibility_verify_payload(
        "mcg.compatibility_verify_sampling.v1",
        &serde_json::json!({"temperature_basis_points": 0}),
    )?;
    let task_contract_locator = format!(
        "{}@{}",
        target.task_contract.id, target.task_contract.version
    );
    let invocation_manifest = InvocationManifest {
        id: InvocationId::new(),
        role: InvocationRole::Verify,
        attempt_id: target.attempt.attempt_id,
        task_contract: target.attempt.task_contract.clone(),
        runtime_identity: client.hello().runtime_identity.clone(),
        cognitive_configuration: compatibility_verify_hash(
            "MetaCog.CompatibilityVerifyConfiguration",
            b"deterministic-v1",
        ),
        role_manifest: compatibility_verify_hash(
            "MetaCog.CompatibilityVerifyRole",
            b"independent-verify-v1",
        ),
        prompt_bundle: compatibility_verify_hash(
            "MetaCog.CompatibilityVerifyPrompt",
            b"evaluate-exact-objective-evidence-v1",
        ),
        context_manifest: context_hash,
        model_route: route,
        resolved_provider: compatibility_verify_source(
            "mcg.deterministic_provider.v1",
            "protected-root",
            "MetaCog.CompatibilityVerifyProvider",
            b"protected-root-deterministic-provider-v1",
        )?,
        resolved_model: compatibility_verify_source(
            "mcg.deterministic_model.v1",
            "independent-verify-v1",
            "MetaCog.CompatibilityVerifyModel",
            b"independent-verify-v1",
        )?,
        observed_model_fingerprint: Some("mcg-runtime-worker:deterministic-verify-v1".into()),
        sampling,
        reasoning: None,
        available_tool_manifests: Vec::new(),
        selected_skill_manifests: Vec::new(),
        authority_envelope: report_source.clone(),
        egress_permit: None,
        repository_revision: None,
        environment: SourceRef {
            source_type: StableTypeId::parse("mcg.task_contract_environment.v1")?,
            locator: task_contract_locator,
            content_hash: target.task_contract.environment.content_hash.clone(),
        },
        runtime_build: client.hello().executable_hash.clone(),
        request_hash: input.content_hash(RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN)?,
        response_hash: None,
    };
    let request = RuntimeInvocationRequest {
        worker_generation: client.worker_generation(),
        attempt_revision: target.attempt.revision,
        lease_generation: target.attempt.lease_generation,
        context_manifest,
        invocation_manifest,
        model_run_id: ModelRunId::new(),
        input,
    };
    request.validate()?;
    Ok(request)
}

fn validate_compatibility_verify_result(
    request: &RuntimeInvocationRequest,
    receipt: &RuntimeInvocationDispatchReceipt,
    report_source: &SourceRef,
) -> Result<(), ProtectedRootError> {
    let input: DeterministicVerifyInput =
        serde_json::from_value(decode_canonical_cbor_v1(&request.input.bytes)?)?;
    let expected_verdict = if input.criteria.iter().all(|criterion| criterion.passed) {
        CompletionVerdict::Satisfied
    } else {
        CompletionVerdict::NotSatisfied
    };
    let exact_criteria = receipt.result.criterion_results.len() == input.criteria.len()
        && receipt
            .result
            .criterion_results
            .iter()
            .zip(&input.criteria)
            .all(|(actual, expected)| {
                actual.clause_id == expected.clause_id
                    && actual.passed == expected.passed
                    && actual.evidence_refs == expected.evidence_refs
                    && actual.evidence_refs.contains(report_source)
            });
    if !exact_criteria || receipt.result.verdict != Some(expected_verdict) {
        return Err(StorageError::Conflict(
            "independent Verify response substituted objective criteria or verdict".into(),
        )
        .into());
    }
    Ok(())
}

/// Executes independent deterministic Verify for every completed attached compatibility Work
/// target and returns Root-owned completion certificates. Verify is observational and may be
/// safely repeated after a crash before the final atomic settlement; every attempt remains in
/// immutable invocation history.
///
/// # Errors
/// Returns an error for stale canonical authority, malformed report/target evidence, worker
/// authentication or execution failure, or a substituted Verify response.
pub async fn verify_compatibility_consumer_evaluation(
    runtime: &AppRuntime,
    root_executable: impl AsRef<Path>,
    data_directory: impl AsRef<Path>,
    report: &EvaluationReport,
    targets: &[AgentTurnVerificationTarget],
) -> Result<Vec<ConsumerEvaluationCompletion>, ProtectedRootError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let report_source = evaluation_report_source_ref(report)?;
    let mut client = launch_runtime_worker(runtime, root_executable, data_directory).await?;
    let execution = async {
        let mut completions = Vec::with_capacity(targets.len());
        for target in targets {
            let request = compatibility_verify_request(&client, target, report, &report_source)?;
            let receipt =
                execute_runtime_invocation(runtime, &mut client, &request, Utc::now()).await?;
            validate_compatibility_verify_result(&request, &receipt, &report_source)?;
            let mut artifacts = target.work_artifacts.clone();
            let verify_artifact = ArtifactRef {
                content_hash: receipt.response_artifact.response_hash.clone(),
                media_type: "application/vnd.metacog.runtime-response+cbor".into(),
                byte_length: u64::try_from(receipt.response_artifact.response.bytes.len())
                    .map_err(|_| {
                        StorageError::InvalidData(
                            "Verify response artifact length exceeds its wire range".into(),
                        )
                    })?,
            };
            if !artifacts.contains(&verify_artifact) {
                artifacts.push(verify_artifact);
            }
            let verdict = match report.outcome {
                EvaluationOutcome::Passed => CompletionVerdict::Satisfied,
                EvaluationOutcome::Failed => CompletionVerdict::NotSatisfied,
                EvaluationOutcome::Inconclusive => CompletionVerdict::Inconclusive,
            };
            let certificate = CompletionCertificate {
                id: CompletionCertificateId::new(),
                workflow_id: target.attempt.workflow_id,
                attempt_id: target.attempt.attempt_id,
                task_contract: target.attempt.task_contract.clone(),
                work_invocations: target.work_invocations.clone(),
                verify_invocations: vec![request.invocation_manifest.id],
                criterion_results: receipt.result.criterion_results.clone(),
                evidence_refs: vec![report_source.clone()],
                tests_run: vec![report_source.clone()],
                artifacts,
                known_untested_paths: Vec::new(),
                unresolved_assumptions: Vec::new(),
                remaining_risks: Vec::new(),
                verdict,
                issued_at: Utc::now(),
            };
            certificate.validate()?;
            completions.push(ConsumerEvaluationCompletion {
                lease_id: target.lease_id,
                certificate,
                expected_revision: target.attempt.revision,
                expected_generation: target.attempt.lease_generation,
            });
        }
        Ok(completions)
    }
    .await;
    match execution {
        Ok(completions) => {
            client.shutdown().await?;
            Ok(completions)
        }
        Err(error) => {
            let _ = client.terminate().await;
            Err(error)
        }
    }
}

fn protected_agent_provider_error(error: impl std::fmt::Display) -> ProviderError {
    ProviderError::Request {
        message: error.to_string(),
        retriable: false,
    }
}

fn agent_provider_payload(
    type_id: &str,
    value: &serde_json::Value,
) -> Result<VersionedPayload, ProtectedRootError> {
    Ok(VersionedPayload::canonical_v1(
        StableTypeId::parse(type_id)?,
        value,
    )?)
}

fn agent_provider_hash(domain: &str, bytes: &[u8]) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(b"MetaCog.CompatibilityAgentProviderEvidence:v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes);
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        domain: domain.into(),
        digest_hex: format!("{:x}", hasher.finalize()),
    }
}

fn remaining_compatibility_cost_micro_usd(
    maximum_cost_usd: Option<f64>,
    consumed_cost_usd: f64,
) -> Result<u64, ProtectedRootError> {
    if !consumed_cost_usd.is_finite() || consumed_cost_usd < 0.0 {
        return Err(
            StorageError::InvalidData("agent lease consumed cost is invalid".into()).into(),
        );
    }
    let Some(maximum_cost_usd) = maximum_cost_usd else {
        return Ok(COMPATIBILITY_AGENT_CALL_MAX_COST_MICROUSD);
    };
    let remaining = maximum_cost_usd - consumed_cost_usd;
    if !maximum_cost_usd.is_finite() || maximum_cost_usd < 0.0 || remaining <= 0.0 {
        return Err(
            StorageError::Conflict("agent lease has no remaining cost authority".into()).into(),
        );
    }
    let micro_usd = (remaining * 1_000_000.0).floor();
    if !micro_usd.is_finite() || micro_usd < 1.0 {
        return Err(StorageError::Conflict(
            "agent lease cost authority is below one micro-USD".into(),
        )
        .into());
    }
    if micro_usd >= 5_000_000.0 {
        return Ok(COMPATIBILITY_AGENT_CALL_MAX_COST_MICROUSD);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bounded = micro_usd as u64;
    Ok(bounded)
}

/// Executes one compatibility-agent model request through canonical workflow authority only.
///
/// Every physical call receives a distinct invocation, permit, stable client request ID, and model
/// run. Root derives disclosure and budgets from persisted source authority and the exact request;
/// the supplied gateway cannot be reached before all durable authorization evidence is committed.
/// Root permits at most one retry, and only after the gateway proves that the preceding attempt
/// did not contact the provider. Ambiguous outcomes are never retried automatically.
///
/// # Errors
/// Returns a protected-root error for stale source/workflow/provider authority, invalid budgets,
/// request substitution, gateway failure, invalid response, or durable settlement failure.
#[allow(clippy::too_many_arguments)]
pub async fn execute_compatibility_agent_model_invocation(
    runtime: &AppRuntime,
    gateway: &dyn GovernedModelProviderGateway,
    connection: &ProviderConnection,
    model: &ModelDescriptor,
    runtime_actor: RuntimeActorId,
    runtime_identity: &RuntimeIdentity,
    lease_id: AgentLeaseId,
    target: &mcg_providers::ProviderModelRef,
    request: ModelRequest,
    now: chrono::DateTime<Utc>,
) -> Result<ModelResponse, ProtectedRootError> {
    let logical_request_id = Uuid::new_v4();
    let logical_request_hash = model_provider_request_artifact(&request)?.content_hash;
    let mut retry_of_invocation_id = None;
    for physical_attempt in 1..=COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS {
        let invocation_id = InvocationId::new();
        let attempt_now = now
            .checked_add_signed(chrono::Duration::milliseconds(i64::from(
                physical_attempt.saturating_sub(1),
            )))
            .ok_or_else(|| StorageError::InvalidData("provider retry time overflowed".into()))?;
        let prepared = prepare_compatibility_agent_model_attempt(
            runtime,
            connection,
            model,
            runtime_actor,
            runtime_identity,
            lease_id,
            target,
            request.clone(),
            logical_request_id,
            &logical_request_hash,
            physical_attempt,
            retry_of_invocation_id,
            invocation_id,
            false,
            attempt_now,
        )?;
        match execute_authorized_model_invocation(
            runtime,
            gateway,
            &prepared.manifest,
            prepared.model_run_id,
            &prepared.request,
            attempt_now,
        )
        .await
        {
            Ok(receipt) => return Ok(receipt.response),
            Err(ProtectedRootError::RetryableProviderBeforeContact(_))
                if physical_attempt < COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS =>
            {
                retry_of_invocation_id = Some(invocation_id);
            }
            Err(ProtectedRootError::RetryableProviderBeforeContact(error)) => {
                return Err(ProviderError::Request {
                    message: format!(
                        "Root provider retry policy exhausted after {physical_attempt} physical attempts: {error}"
                    ),
                    retriable: false,
                }
                .into());
            }
            Err(error) => return Err(error),
        }
    }
    Err(StorageError::Conflict("provider retry policy exhausted without a result".into()).into())
}

/// Starts one compatibility-agent model stream through canonical workflow authority only.
///
/// A pre-contact transient may receive one fresh physical invocation before any stream is exposed.
/// Once a stream is returned, every event belongs to that one attempt and no later failure or
/// cancellation can trigger an automatic retry.
///
/// # Errors
/// Returns a protected-root error for stale source/workflow/provider authority, invalid budgets,
/// request substitution, pre-contact gateway failure, or durable planning/dispatch failure.
#[allow(clippy::too_many_arguments)]
pub async fn execute_compatibility_agent_model_stream(
    runtime: Arc<AppRuntime>,
    gateway: &dyn GovernedModelProviderGateway,
    connection: &ProviderConnection,
    model: &ModelDescriptor,
    runtime_actor: RuntimeActorId,
    runtime_identity: &RuntimeIdentity,
    lease_id: AgentLeaseId,
    target: &mcg_providers::ProviderModelRef,
    request: ModelRequest,
    now: chrono::DateTime<Utc>,
) -> Result<ProviderStream, ProtectedRootError> {
    let logical_request_id = Uuid::new_v4();
    let logical_request_hash = model_provider_stream_request_artifact(&request)?.content_hash;
    let mut retry_of_invocation_id = None;
    for physical_attempt in 1..=COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS {
        let invocation_id = InvocationId::new();
        let attempt_now = now
            .checked_add_signed(chrono::Duration::milliseconds(i64::from(
                physical_attempt.saturating_sub(1),
            )))
            .ok_or_else(|| StorageError::InvalidData("provider retry time overflowed".into()))?;
        let prepared = prepare_compatibility_agent_model_attempt(
            &runtime,
            connection,
            model,
            runtime_actor,
            runtime_identity,
            lease_id,
            target,
            request.clone(),
            logical_request_id,
            &logical_request_hash,
            physical_attempt,
            retry_of_invocation_id,
            invocation_id,
            true,
            attempt_now,
        )?;
        match execute_authorized_model_stream(
            runtime.clone(),
            gateway,
            &prepared.manifest,
            prepared.model_run_id,
            &prepared.request,
            attempt_now,
        )
        .await
        {
            Ok(stream) => return Ok(stream),
            Err(ProtectedRootError::RetryableProviderBeforeContact(_))
                if physical_attempt < COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS =>
            {
                retry_of_invocation_id = Some(invocation_id);
            }
            Err(ProtectedRootError::RetryableProviderBeforeContact(error)) => {
                return Err(ProviderError::Request {
                    message: format!(
                        "Root provider stream retry policy exhausted after {physical_attempt} physical attempts: {error}"
                    ),
                    retriable: false,
                }
                .into());
            }
            Err(error) => return Err(error),
        }
    }
    Err(
        StorageError::Conflict("provider stream retry policy exhausted without a result".into())
            .into(),
    )
}

struct PreparedCompatibilityModelAttempt {
    manifest: InvocationManifest,
    request: ModelRequest,
    model_run_id: ModelRunId,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn prepare_compatibility_agent_model_attempt(
    runtime: &AppRuntime,
    connection: &ProviderConnection,
    model: &ModelDescriptor,
    runtime_actor: RuntimeActorId,
    runtime_identity: &RuntimeIdentity,
    lease_id: AgentLeaseId,
    target: &mcg_providers::ProviderModelRef,
    mut request: ModelRequest,
    logical_request_id: Uuid,
    logical_request_hash: &ContentHash,
    physical_attempt: u32,
    retry_of_invocation_id: Option<InvocationId>,
    invocation_id: InvocationId,
    streaming: bool,
    now: chrono::DateTime<Utc>,
) -> Result<PreparedCompatibilityModelAttempt, ProtectedRootError> {
    runtime_identity.validate()?;
    if connection.id != target.provider_id
        || model.id != target.model_id
        || request.model != model.id
    {
        return Err(StorageError::Conflict(
            "compatibility agent provider route or request model was substituted".into(),
        )
        .into());
    }
    let authority = claim_active_agent_lease_workflow(
        runtime,
        lease_id,
        runtime_actor,
        COMPATIBILITY_AGENT_WORKFLOW_LEASE_SECONDS,
        now,
    )?;
    let attempt = &authority.workflow.attempt;
    request.client_request_id = Some(format!("mcg-governed-{invocation_id}"));
    let request_artifact = if streaming {
        model_provider_stream_request_artifact(&request)?
    } else {
        model_provider_request_artifact(&request)?
    };
    let approximate_input_tokens = request_artifact.byte_length.saturating_add(3) / 4;
    let output_tokens = request
        .max_output_tokens
        .filter(|tokens| *tokens > 0)
        .ok_or_else(|| {
            StorageError::InvalidData(
                "compatibility agent request requires a nonzero output-token limit".into(),
            )
        })?;
    let mut maximum_input_tokens = model
        .context_window
        .unwrap_or_else(|| request_artifact.byte_length.max(1));
    if let Some(maximum_tokens) = authority.legacy_lease.budget.limit.max_tokens {
        let remaining = maximum_tokens
            .checked_sub(authority.legacy_lease.budget.consumed_tokens)
            .ok_or_else(|| {
                StorageError::Conflict("agent lease token authority is exhausted".into())
            })?;
        maximum_input_tokens =
            maximum_input_tokens.min(remaining.checked_sub(output_tokens).ok_or_else(|| {
                StorageError::Conflict("agent lease token authority is exhausted".into())
            })?);
    }
    if maximum_input_tokens == 0 || approximate_input_tokens > maximum_input_tokens {
        return Err(StorageError::Conflict(
            "compatibility agent request exceeds its input-token authority".into(),
        )
        .into());
    }
    let maximum_cost_micro_usd = remaining_compatibility_cost_micro_usd(
        authority.legacy_lease.budget.limit.max_cost_usd,
        authority.legacy_lease.budget.consumed_cost_usd,
    )?;
    let lease_expiry = attempt.lease_expires_at.ok_or_else(|| {
        StorageError::Conflict("canonical agent workflow has no active lease expiry".into())
    })?;
    let permit_expiry = lease_expiry.min(
        now.checked_add_signed(chrono::Duration::seconds(
            COMPATIBILITY_AGENT_PERMIT_SECONDS,
        ))
        .ok_or_else(|| StorageError::InvalidData("model permit expiry overflowed".into()))?,
    );
    if permit_expiry <= now {
        return Err(StorageError::Conflict("canonical agent workflow lease expired".into()).into());
    }
    let context_source = authority.workflow.attachment.source.clone();
    let influence = agent_provider_payload(
        "mcg.agent_model_request_influence.v1",
        &serde_json::json!({
            "lease_id": lease_id,
            "provider_id": target.provider_id,
            "model_id": target.model_id,
            "logical_request_id": logical_request_id,
            "logical_request_hash": logical_request_hash,
            "physical_attempt": physical_attempt,
            "maximum_physical_attempts": COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS,
            "retry_of_invocation_id": retry_of_invocation_id,
            "request_hash": request_artifact.content_hash,
        }),
    )?;
    let context = ContextManifest {
        id: ContextManifestId::new(),
        attempt_id: attempt.attempt_id,
        selected_sources: vec![ContextSelection {
            source: context_source.clone(),
            perspective: None,
            influence,
            transformations: Vec::new(),
            placement: 0,
            approximate_tokens: approximate_input_tokens,
        }],
        selection_policy: agent_provider_payload(
            "mcg.agent_model_context_selection.v1",
            &serde_json::json!({
                "mode": if streaming {
                    "exact_compatibility_stream_request"
                } else {
                    "exact_compatibility_request"
                },
                "source_snapshot_hash": context_source.content_hash,
                "logical_request_id": logical_request_id,
                "logical_request_hash": logical_request_hash,
                "physical_attempt": physical_attempt,
                "maximum_physical_attempts": COMPATIBILITY_AGENT_PROVIDER_MAX_ATTEMPTS,
                "retry_of_invocation_id": retry_of_invocation_id,
                "request_hash": request_artifact.content_hash,
            }),
        )?,
        expansion_steps: Vec::new(),
        approximate_tokens: approximate_input_tokens,
    };
    let context_hash = canonical_contract_hash(
        ContextManifest::TYPE_ID,
        "MetaCog.ContextManifest",
        &context,
    )?;
    let route = ModelEgressRoute {
        provider_id: connection.id.clone(),
        model_id: model.id.clone(),
        destination: connection.base_url.clone(),
        destination_class: classify_model_destination(connection)?,
    };
    let limits = ModelBudget {
        max_input_tokens: maximum_input_tokens,
        max_output_tokens: output_tokens,
        max_cost_microusd: maximum_cost_micro_usd,
    };
    let proposal = ModelInvocationProposal {
        invocation_id,
        attempt_id: attempt.attempt_id,
        attempt_revision: attempt.revision,
        lease_generation: attempt.lease_generation,
        runtime_actor,
        runtime_identity: runtime_identity.clone(),
        role: InvocationRole::Work,
        task_contract: attempt.task_contract.clone(),
        request_artifact: request_artifact.clone(),
        request_hash: request_artifact.content_hash.clone(),
        route: route.clone(),
        data_classification: ModelDataClassification::Restricted,
        disclosure: ModelDisclosureManifest {
            context_manifest: context_hash.clone(),
            disclosed_sources: vec![context_source.clone()],
            approximate_input_tokens,
        },
        requested_limits: limits,
        deadline: permit_expiry,
    };
    let policy = ModelEgressPolicy {
        allowed_routes: vec![route.clone()],
        minimum_data_classification: ModelDataClassification::Restricted,
        maximum_data_classification: ModelDataClassification::Restricted,
        maximum_limits: limits,
        maximum_permit_lifetime_seconds: u64::try_from(COMPATIBILITY_AGENT_PERMIT_SECONDS)
            .unwrap_or(120),
    };
    let authorization = authorize_runtime_model_egress(
        runtime,
        &context,
        &proposal,
        runtime_identity,
        &policy,
        now,
    )?;
    let instructions = serde_json::to_vec(&request.instructions)?;
    let role_manifest = agent_provider_hash("MetaCog.AgentRoleManifest", &instructions);
    let prompt_bundle = agent_provider_hash(
        "MetaCog.AgentPromptBundle",
        request_artifact.content_hash.digest_hex.as_bytes(),
    );
    let available_tool_manifests = request
        .tools
        .iter()
        .map(|tool| {
            serde_json::to_vec(tool)
                .map(|bytes| agent_provider_hash("MetaCog.AgentToolManifest", &bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sampling = agent_provider_payload(
        "mcg.agent_model_sampling.v1",
        &serde_json::json!({
            "temperature": request.temperature.map(|value| value.to_string()),
            "top_p": request.top_p.map(|value| value.to_string()),
            "max_output_tokens": request.max_output_tokens,
        }),
    )?;
    let reasoning = request
        .reasoning
        .as_ref()
        .map(|reasoning| {
            agent_provider_payload(
                "mcg.agent_model_reasoning.v1",
                &serde_json::to_value(reasoning).map_err(ProtectedRootError::Serialization)?,
            )
        })
        .transpose()?;
    let manifest = InvocationManifest {
        id: invocation_id,
        role: InvocationRole::Work,
        attempt_id: attempt.attempt_id,
        task_contract: attempt.task_contract.clone(),
        runtime_identity: runtime_identity.clone(),
        cognitive_configuration: attempt.configuration.content_hash.clone(),
        role_manifest,
        prompt_bundle,
        context_manifest: context_hash,
        model_route: model_egress_route_payload(&route)?,
        resolved_provider: authorization.permit.resolved_provider.clone(),
        resolved_model: authorization.permit.resolved_model.clone(),
        observed_model_fingerprint: None,
        sampling,
        reasoning,
        available_tool_manifests,
        selected_skill_manifests: Vec::new(),
        authority_envelope: context_source,
        egress_permit: Some(authorization.permit_ref),
        repository_revision: None,
        environment: SourceRef {
            source_type: StableTypeId::parse("mcg.agent_turn_environment.v1")?,
            locator: attempt.workflow_id.to_string(),
            content_hash: attempt.environment_snapshot.clone(),
        },
        runtime_build: runtime_identity.build_hash.clone(),
        request_hash: request_artifact.content_hash,
        response_hash: None,
    };
    Ok(PreparedCompatibilityModelAttempt {
        manifest,
        request,
        model_run_id: ModelRunId::new(),
    })
}

fn provider_runner_authorization(
    authorization: &RuntimeModelEgressAuthorizationReceipt,
) -> ProviderRunnerAuthorization {
    ProviderRunnerAuthorization {
        proposal: authorization.proposal.clone(),
        proposal_hash: authorization.proposal_hash.clone(),
        permit: authorization.permit.clone(),
        permit_hash: authorization.permit_hash.clone(),
        permit_ref: authorization.permit_ref.clone(),
        secret_handles: authorization
            .secret_handles
            .iter()
            .map(|binding| RunnerSecretHandleBinding {
                handle_id: binding.handle_id,
                permit_id: binding.permit_id,
                connection_id: binding.connection_id.clone(),
                slot: match binding.slot {
                    ModelSecretSlot::Primary => RunnerSecretSlot::Primary,
                    ModelSecretSlot::OAuthIdToken => RunnerSecretSlot::OAuthIdToken,
                    ModelSecretSlot::OAuthRefreshToken => RunnerSecretSlot::OAuthRefreshToken,
                },
                expires_at: binding.expires_at,
            })
            .collect(),
    }
}

/// Executes one governed request through the fixed adjacent isolated provider process.
///
/// Root reloads the current provider/catalog evidence used to configure the child. The child
/// independently validates those identities against the permit and loads any credential only
/// from its exact authorized operating-system slot.
///
/// # Errors
/// Returns an error for stale authority, missing provider evidence, runner failure, invalid
/// response, or durable settlement failure.
pub async fn execute_authorized_model_invocation_isolated(
    runtime: &AppRuntime,
    root_executable: impl AsRef<Path>,
    manifest: &InvocationManifest,
    model_run_id: ModelRunId,
    request: &ModelRequest,
    now: chrono::DateTime<Utc>,
) -> Result<GovernedModelProviderDispatchReceipt, ProtectedRootError> {
    let authorization = runtime
        .active_model_egress_authorization(manifest.id, now)?
        .ok_or_else(|| {
            StorageError::Conflict("model egress authorization does not exist".into())
        })?;
    let _oauth_execution_guard = runtime
        .lock_openai_oauth_execution(&authorization.proposal.route.provider_id)
        .await?;
    let (connection, catalog) = runtime
        .model_egress_route_state(&authorization.proposal.route.provider_id)?
        .ok_or_else(|| StorageError::Conflict("model egress provider state disappeared".into()))?;
    let model = catalog
        .models
        .into_iter()
        .find(|model| model.id == authorization.proposal.route.model_id)
        .ok_or_else(|| StorageError::Conflict("model egress model state disappeared".into()))?;
    let gateway = IsolatedProviderRunnerGateway::adjacent_to(root_executable, connection, model)?;
    execute_authorized_model_invocation(runtime, &gateway, manifest, model_run_id, request, now)
        .await
}

async fn invoke_governed_gateway_while_authorized(
    runtime: &AppRuntime,
    gateway: &dyn GovernedModelProviderGateway,
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    request: &ModelRequest,
) -> Option<Result<ModelResponse, GovernedModelProviderGatewayError>> {
    let mut gateway_call = Box::pin(gateway.invoke(authorization, request));
    let mut authority_poll = tokio::time::interval(std::time::Duration::from_millis(
        GOVERNED_PROVIDER_AUTHORITY_POLL_MILLISECONDS,
    ));
    authority_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            result = &mut gateway_call => return Some(result),
            _ = authority_poll.tick() => {
                let still_authorized = matches!(
                    runtime.active_model_egress_authorization(
                        authorization.proposal.invocation_id,
                        Utc::now(),
                    ),
                    Ok(Some(current)) if current == *authorization
                );
                if !still_authorized {
                    return None;
                }
            }
        }
    }
}

fn settle_governed_provider_authority_loss(
    runtime: &AppRuntime,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
) -> Result<ProtectedRootError, ProtectedRootError> {
    runtime.settle_governed_provider_invocation_unknown(invocation_id, model_run_id, Utc::now())?;
    Ok(StorageError::Conflict(
        "governed provider execution was cancelled after dispatch or lost current authority".into(),
    )
    .into())
}

async fn dispatch_governed_gateway(
    runtime: &AppRuntime,
    gateway: &dyn GovernedModelProviderGateway,
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
    request: &ModelRequest,
) -> Result<ModelResponse, ProtectedRootError> {
    let Some(gateway_result) =
        invoke_governed_gateway_while_authorized(runtime, gateway, authorization, request).await
    else {
        return Err(settle_governed_provider_authority_loss(
            runtime,
            invocation_id,
            model_run_id,
        )?);
    };
    match gateway_result {
        Ok(response) => Ok(response),
        Err(error) => {
            let class = error.class();
            let provider_error = error.into_provider_error();
            match class {
                GovernedProviderFailureClass::RetryableBeforeProviderContact
                | GovernedProviderFailureClass::FinalBeforeProviderContact => {
                    let error_class =
                        if class == GovernedProviderFailureClass::RetryableBeforeProviderContact {
                            ModelRunErrorClass::RetryableBeforeProviderContact
                        } else {
                            ModelRunErrorClass::FinalBeforeProviderContact
                        };
                    runtime.fail_governed_provider_invocation_before_contact(
                        invocation_id,
                        model_run_id,
                        error_class,
                        Utc::now(),
                    )?;
                    if class == GovernedProviderFailureClass::RetryableBeforeProviderContact {
                        return Err(ProtectedRootError::RetryableProviderBeforeContact(
                            provider_error,
                        ));
                    }
                }
                GovernedProviderFailureClass::OutcomeUnknown => {
                    runtime.settle_governed_provider_invocation_unknown(
                        invocation_id,
                        model_run_id,
                        Utc::now(),
                    )?;
                }
            }
            Err(provider_error.into())
        }
    }
}

/// Executes one exact provider request only through its active Root-issued egress permit.
///
/// The immutable invocation manifest and planned model run are persisted before dispatch. Root
/// revalidates the request before calling the gateway and revalidates the workflow lease, permit,
/// provider catalog, response identity, and usage before accepting a result. Raw response content
/// is returned to the caller but not retained in generic durable state.
///
/// # Errors
/// Returns an error for stale or substituted authority, malformed request/response content,
/// provider failure, budget excess, or durable-state failure. Once dispatch is persisted, any
/// failure that cannot prove no provider contact conservatively settles the run `OutcomeUnknown`.
pub async fn execute_authorized_model_invocation(
    runtime: &AppRuntime,
    gateway: &dyn GovernedModelProviderGateway,
    manifest: &InvocationManifest,
    model_run_id: ModelRunId,
    request: &ModelRequest,
    now: chrono::DateTime<Utc>,
) -> Result<GovernedModelProviderDispatchReceipt, ProtectedRootError> {
    let authorization = runtime
        .active_model_egress_authorization(manifest.id, now)?
        .ok_or_else(|| {
            StorageError::Conflict("model egress authorization does not exist".into())
        })?;
    validate_governed_provider_request(&authorization, manifest, request, false, now)?;
    runtime.plan_governed_provider_invocation(manifest, model_run_id, now)?;
    let provider_request_id = request.client_request_id.as_deref().ok_or_else(|| {
        StorageError::InvalidData(
            "governed provider request requires a stable client request id".into(),
        )
    })?;
    runtime.dispatch_governed_provider_invocation(
        manifest.id,
        model_run_id,
        provider_request_id,
        now,
    )?;

    let response = dispatch_governed_gateway(
        runtime,
        gateway,
        &authorization,
        manifest.id,
        model_run_id,
        request,
    )
    .await?;
    let completed_at = Utc::now();
    if let Err(error) = validate_governed_provider_response(&authorization, &response) {
        runtime.settle_governed_provider_invocation_unknown(
            manifest.id,
            model_run_id,
            completed_at,
        )?;
        return Err(error);
    }
    let response_artifact = match model_provider_response_artifact(&response) {
        Ok(artifact) => artifact,
        Err(error) => {
            runtime.settle_governed_provider_invocation_unknown(
                manifest.id,
                model_run_id,
                completed_at,
            )?;
            return Err(error);
        }
    };
    let fingerprint = match observed_model_fingerprint(&authorization, &response) {
        Ok(fingerprint) => fingerprint,
        Err(error) => {
            runtime.settle_governed_provider_invocation_unknown(
                manifest.id,
                model_run_id,
                completed_at,
            )?;
            return Err(error);
        }
    };
    let (actual_cost_microusd, cost_provenance) =
        match observed_model_cost(runtime, &authorization, &response) {
            Ok(cost) => cost,
            Err(error) => {
                runtime.settle_governed_provider_invocation_unknown(
                    manifest.id,
                    model_run_id,
                    completed_at,
                )?;
                return Err(error);
            }
        };
    let completed_run = match runtime.complete_governed_provider_invocation(
        manifest.id,
        model_run_id,
        response_artifact,
        ModelUsage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cached_input_tokens: response.usage.cached_input_tokens,
            reasoning_tokens: response.usage.reasoning_tokens,
        },
        fingerprint,
        actual_cost_microusd,
        cost_provenance,
        completed_at,
    ) {
        Ok(completed) => completed,
        Err(error) => {
            runtime.settle_governed_provider_invocation_unknown(
                manifest.id,
                model_run_id,
                completed_at,
            )?;
            return Err(error.into());
        }
    };
    Ok(GovernedModelProviderDispatchReceipt {
        response,
        completed_run,
    })
}

async fn open_governed_stream_while_authorized(
    runtime: &AppRuntime,
    gateway: &dyn GovernedModelProviderGateway,
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    request: &ModelRequest,
) -> Option<Result<GovernedModelProviderStream, GovernedModelProviderGatewayError>> {
    let mut stream_call = Box::pin(gateway.stream(authorization, request));
    let mut authority_poll = tokio::time::interval(std::time::Duration::from_millis(
        GOVERNED_PROVIDER_AUTHORITY_POLL_MILLISECONDS,
    ));
    authority_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            result = &mut stream_call => return Some(result),
            _ = authority_poll.tick() => {
                if !governed_provider_authorization_is_current(runtime, authorization) {
                    return None;
                }
            }
        }
    }
}

fn governed_provider_authorization_is_current(
    runtime: &AppRuntime,
    authorization: &RuntimeModelEgressAuthorizationReceipt,
) -> bool {
    matches!(
        runtime.active_model_egress_authorization(
            authorization.proposal.invocation_id,
            Utc::now(),
        ),
        Ok(Some(current)) if current == *authorization
    )
}

/// Starts one exact true-streaming provider request under an active Root-issued permit.
///
/// Planning and dispatch are durable before the stream is opened. Intermediate events are
/// bounded, correlated by the isolated runner, and provisional: only a validated terminal
/// `Completed` response can settle the `ModelRun`. Dropping the returned stream or losing exact
/// authority tears down the child and settles the possible-contact run `OutcomeUnknown`.
///
/// # Errors
/// Returns an error for stale/substituted authority, pre-contact gateway failure, or durable
/// planning/dispatch failure. Failures after a stream is returned appear as a terminal stream item.
pub async fn execute_authorized_model_stream(
    runtime: Arc<AppRuntime>,
    gateway: &dyn GovernedModelProviderGateway,
    manifest: &InvocationManifest,
    model_run_id: ModelRunId,
    request: &ModelRequest,
    now: chrono::DateTime<Utc>,
) -> Result<ProviderStream, ProtectedRootError> {
    let authorization = runtime
        .active_model_egress_authorization(manifest.id, now)?
        .ok_or_else(|| {
            StorageError::Conflict("model egress authorization does not exist".into())
        })?;
    validate_governed_provider_request(&authorization, manifest, request, true, now)?;
    runtime.plan_governed_provider_invocation(manifest, model_run_id, now)?;
    let provider_request_id = request.client_request_id.as_deref().ok_or_else(|| {
        StorageError::InvalidData(
            "governed provider request requires a stable client request id".into(),
        )
    })?;
    runtime.dispatch_governed_provider_invocation(
        manifest.id,
        model_run_id,
        provider_request_id,
        now,
    )?;
    let Some(stream_result) =
        open_governed_stream_while_authorized(&runtime, gateway, &authorization, request).await
    else {
        return Err(settle_governed_provider_authority_loss(
            &runtime,
            manifest.id,
            model_run_id,
        )?);
    };
    let stream = match stream_result {
        Ok(stream) => stream,
        Err(error) => {
            let class = error.class();
            let provider_error = error.into_provider_error();
            match class {
                GovernedProviderFailureClass::RetryableBeforeProviderContact
                | GovernedProviderFailureClass::FinalBeforeProviderContact => {
                    let error_class =
                        if class == GovernedProviderFailureClass::RetryableBeforeProviderContact {
                            ModelRunErrorClass::RetryableBeforeProviderContact
                        } else {
                            ModelRunErrorClass::FinalBeforeProviderContact
                        };
                    runtime.fail_governed_provider_invocation_before_contact(
                        manifest.id,
                        model_run_id,
                        error_class,
                        Utc::now(),
                    )?;
                    if class == GovernedProviderFailureClass::RetryableBeforeProviderContact {
                        return Err(ProtectedRootError::RetryableProviderBeforeContact(
                            provider_error,
                        ));
                    }
                }
                GovernedProviderFailureClass::OutcomeUnknown => {
                    runtime.settle_governed_provider_invocation_unknown(
                        manifest.id,
                        model_run_id,
                        Utc::now(),
                    )?;
                }
            }
            return Err(provider_error.into());
        }
    };
    let (sender, receiver) = mpsc::unbounded_channel();
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    tokio::spawn(run_authorized_model_stream(
        runtime,
        authorization,
        manifest.id,
        model_run_id,
        stream,
        cancel_receiver,
        sender,
    ));
    Ok(Box::pin(CancellableGovernedProviderStream {
        receiver: UnboundedReceiverStream::new(receiver),
        cancel: Some(cancel_sender),
    }))
}

async fn run_authorized_model_stream(
    runtime: Arc<AppRuntime>,
    authorization: RuntimeModelEgressAuthorizationReceipt,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
    mut stream: GovernedModelProviderStream,
    mut cancel: oneshot::Receiver<()>,
    sender: mpsc::UnboundedSender<Result<ModelStreamEvent, ProviderError>>,
) {
    let mut authority_poll = tokio::time::interval(std::time::Duration::from_millis(
        GOVERNED_PROVIDER_AUTHORITY_POLL_MILLISECONDS,
    ));
    authority_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut validation = GovernedStreamValidation::default();
    loop {
        let next = tokio::select! {
            _ = &mut cancel => {
                settle_dropped_governed_stream(&runtime, invocation_id, model_run_id);
                return;
            }
            _ = authority_poll.tick() => {
                if governed_provider_authorization_is_current(&runtime, &authorization) {
                    continue;
                }
                settle_authority_lost_governed_stream(
                    &runtime,
                    invocation_id,
                    model_run_id,
                    &sender,
                );
                return;
            }
            event = stream.next() => event,
        };
        let Some(event) = next else {
            settle_incomplete_governed_stream(
                &runtime,
                invocation_id,
                model_run_id,
                &sender,
                "governed provider stream ended without a completed response",
            );
            return;
        };
        let Ok(event) = event else {
            settle_incomplete_governed_stream(
                &runtime,
                invocation_id,
                model_run_id,
                &sender,
                "governed provider stream failed after dispatch",
            );
            return;
        };
        if validation.observe(&authorization, &event).is_err() {
            settle_incomplete_governed_stream(
                &runtime,
                invocation_id,
                model_run_id,
                &sender,
                "governed provider stream event failed Root validation",
            );
            return;
        }
        if let ModelStreamEvent::Completed { response } = &event {
            let completed_at = Utc::now();
            let completion = model_provider_response_artifact(response).and_then(|artifact| {
                let fingerprint = observed_model_fingerprint(&authorization, response)?;
                let (actual_cost_microusd, cost_provenance) =
                    observed_model_cost(&runtime, &authorization, response)?;
                runtime
                    .complete_governed_provider_invocation(
                        invocation_id,
                        model_run_id,
                        artifact,
                        ModelUsage {
                            input_tokens: response.usage.input_tokens,
                            output_tokens: response.usage.output_tokens,
                            cached_input_tokens: response.usage.cached_input_tokens,
                            reasoning_tokens: response.usage.reasoning_tokens,
                        },
                        fingerprint,
                        actual_cost_microusd,
                        cost_provenance,
                        completed_at,
                    )
                    .map_err(ProtectedRootError::from)
            });
            if completion.is_err() {
                settle_incomplete_governed_stream(
                    &runtime,
                    invocation_id,
                    model_run_id,
                    &sender,
                    "governed provider stream completion failed durable settlement",
                );
                return;
            }
            let _ = sender.send(Ok(event));
            return;
        }
        if sender.send(Ok(event)).is_err() {
            settle_dropped_governed_stream(&runtime, invocation_id, model_run_id);
            return;
        }
    }
}

fn settle_dropped_governed_stream(
    runtime: &AppRuntime,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
) {
    let _ = runtime.settle_governed_provider_invocation_unknown(
        invocation_id,
        model_run_id,
        Utc::now(),
    );
}

fn settle_authority_lost_governed_stream(
    runtime: &AppRuntime,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
    sender: &mpsc::UnboundedSender<Result<ModelStreamEvent, ProviderError>>,
) {
    let settlement = runtime.settle_governed_provider_invocation_unknown(
        invocation_id,
        model_run_id,
        Utc::now(),
    );
    let error = settlement.map_or_else(
        |_| {
            ProviderError::Configuration(
                "governed provider stream authority-loss settlement failed".into(),
            )
        },
        |_| ProviderError::Request {
            message: "governed provider stream was cancelled or lost current authority".into(),
            retriable: false,
        },
    );
    let _ = sender.send(Err(error));
}

#[derive(Default)]
struct GovernedStreamValidation {
    response_id: Option<String>,
    total_bytes: usize,
}

impl GovernedStreamValidation {
    fn observe(
        &mut self,
        authorization: &RuntimeModelEgressAuthorizationReceipt,
        event: &ModelStreamEvent,
    ) -> Result<(), ProtectedRootError> {
        let event_bytes = serde_json::to_vec(event)?;
        self.total_bytes = self.total_bytes.saturating_add(event_bytes.len());
        if event_bytes.is_empty()
            || self.total_bytes > mcg_provider_runner::PROVIDER_RUNNER_STREAM_TOTAL_MAX_BYTES
        {
            return Err(ProviderError::Incompatible(
                "provider stream exceeded its Root output bound".into(),
            )
            .into());
        }
        match event {
            ModelStreamEvent::ResponseCreated { id, model } => {
                if self.response_id.is_some()
                    || !valid_stream_identifier(id, true)
                    || model.as_ref().is_some_and(|model| {
                        model != &authorization.proposal.route.model_id
                            || !valid_stream_identifier(model, true)
                    })
                {
                    return Err(ProviderError::Incompatible(
                        "provider stream created identity is invalid".into(),
                    )
                    .into());
                }
                self.response_id = Some(id.clone());
            }
            ModelStreamEvent::OutputTextDelta { item_id, delta } => {
                if self.response_id.is_none()
                    || item_id
                        .as_ref()
                        .is_some_and(|item_id| !valid_stream_identifier(item_id, false))
                    || delta.chars().any(|character| character == '\0')
                {
                    return Err(ProviderError::Incompatible(
                        "provider text stream event is invalid".into(),
                    )
                    .into());
                }
            }
            ModelStreamEvent::ToolArgumentsDelta { item_id, delta } => {
                if self.response_id.is_none()
                    || !valid_stream_identifier(item_id, true)
                    || delta.chars().any(|character| character == '\0')
                {
                    return Err(ProviderError::Incompatible(
                        "provider tool stream event is invalid".into(),
                    )
                    .into());
                }
            }
            ModelStreamEvent::OutputItemAdded { .. } => {
                if self.response_id.is_none() {
                    return Err(ProviderError::Incompatible(
                        "provider output item preceded stream creation".into(),
                    )
                    .into());
                }
            }
            ModelStreamEvent::Completed { response } => {
                validate_governed_provider_response(authorization, response)?;
                if self.response_id.as_deref() != Some(response.id.as_str()) {
                    return Err(ProviderError::Incompatible(
                        "provider stream completion identity changed".into(),
                    )
                    .into());
                }
            }
            ModelStreamEvent::ProviderEvent { event_type, .. } => {
                if !valid_stream_identifier(event_type, true) {
                    return Err(ProviderError::Incompatible(
                        "provider stream event type is invalid".into(),
                    )
                    .into());
                }
            }
        }
        Ok(())
    }
}

fn valid_stream_identifier(value: &str, required: bool) -> bool {
    (!required || !value.is_empty()) && value.len() <= 4_096 && !value.chars().any(char::is_control)
}

fn settle_incomplete_governed_stream(
    runtime: &AppRuntime,
    invocation_id: InvocationId,
    model_run_id: ModelRunId,
    sender: &mpsc::UnboundedSender<Result<ModelStreamEvent, ProviderError>>,
    detail: &str,
) {
    let settlement = runtime.settle_governed_provider_invocation_unknown(
        invocation_id,
        model_run_id,
        Utc::now(),
    );
    let error = settlement.map_or_else(
        |_| ProviderError::Configuration("governed provider stream settlement failed".into()),
        |_| ProviderError::Incompatible(detail.into()),
    );
    let _ = sender.send(Err(error));
}

fn validate_governed_provider_request(
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    manifest: &InvocationManifest,
    request: &ModelRequest,
    streaming: bool,
    now: chrono::DateTime<Utc>,
) -> Result<(), ProtectedRootError> {
    let request_artifact = if streaming {
        model_provider_stream_request_artifact(request)?
    } else {
        model_provider_request_artifact(request)?
    };
    if now < authorization.permit.issued_at
        || now >= authorization.permit.expires_at
        || authorization.proposal.invocation_id != manifest.id
        || authorization.permit.invocation_id != manifest.id
        || authorization.permit.runtime_identity != manifest.runtime_identity
        || authorization.permit.role != manifest.role
        || authorization.permit.task_contract != manifest.task_contract
        || authorization.permit_ref
            != *manifest.egress_permit.as_ref().ok_or_else(|| {
                StorageError::Conflict(
                    "governed provider manifest omitted its egress permit".into(),
                )
            })?
        || authorization.proposal.request_artifact != request_artifact
        || authorization.proposal.request_hash != request_artifact.content_hash
        || authorization.permit.request_hash != request_artifact.content_hash
        || request.contract_version != CONTRACT_VERSION
        || request.model != authorization.proposal.route.model_id
        || request.max_output_tokens.is_some_and(|limit| {
            limit == 0 || limit > authorization.permit.limits.max_output_tokens
        })
    {
        return Err(StorageError::Conflict(
            "provider request does not bind the exact active Root authorization".into(),
        )
        .into());
    }
    let request_id = request.client_request_id.as_deref().ok_or_else(|| {
        StorageError::InvalidData(
            "governed provider request requires a stable client request id".into(),
        )
    })?;
    if request_id.is_empty() || request_id.len() > 128 || request_id.chars().any(char::is_control) {
        return Err(StorageError::InvalidData(
            "governed provider client request id is invalid".into(),
        )
        .into());
    }
    Ok(())
}

fn validate_governed_provider_response(
    authorization: &RuntimeModelEgressAuthorizationReceipt,
    response: &ModelResponse,
) -> Result<(), ProtectedRootError> {
    if response.contract_version != CONTRACT_VERSION
        || response.model != authorization.proposal.route.model_id
        || response.id.is_empty()
        || response.id.len() > 4_096
        || response.id.chars().any(char::is_control)
        || response.usage.input_tokens > authorization.permit.limits.max_input_tokens
        || response.usage.output_tokens > authorization.permit.limits.max_output_tokens
        || response.usage.cached_input_tokens > response.usage.input_tokens
        || response.usage.reasoning_tokens > response.usage.output_tokens
        || response.usage.total_tokens
            != response
                .usage
                .input_tokens
                .saturating_add(response.usage.output_tokens)
    {
        return Err(ProviderError::Incompatible(
            "provider response identity or usage violates its Root permit".into(),
        )
        .into());
    }
    Ok(())
}

fn classify_model_destination(
    connection: &ProviderConnection,
) -> Result<ModelDestinationClass, ProtectedRootError> {
    let url = url::Url::parse(&connection.base_url)
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(StorageError::InvalidData(
            "model provider destination contains credentials, query, or fragment".into(),
        )
        .into());
    }
    let host = url.host_str().ok_or_else(|| {
        StorageError::InvalidData("model provider destination has no host".into())
    })?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if loopback {
        return Ok(ModelDestinationClass::LocalLoopback);
    }
    if url.scheme() != "https" {
        return Err(StorageError::Conflict(
            "remote model egress requires an HTTPS destination".into(),
        )
        .into());
    }
    let first_party_host = host.eq_ignore_ascii_case("openai.com")
        || host
            .to_ascii_lowercase()
            .strip_suffix(".openai.com")
            .is_some_and(|prefix| !prefix.is_empty())
        || host.eq_ignore_ascii_case("chatgpt.com")
        || host
            .to_ascii_lowercase()
            .strip_suffix(".chatgpt.com")
            .is_some_and(|prefix| !prefix.is_empty());
    Ok(
        if connection.kind == ProviderKind::OpenAi && first_party_host {
            ModelDestinationClass::FirstPartyRemote
        } else {
            ModelDestinationClass::ThirdPartyRemote
        },
    )
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
    /// Returns the exact executable, platform, sandbox, and resource evidence for this gateway.
    ///
    /// # Errors
    /// Returns unavailable when the fixed runner identity cannot be measured.
    fn capability_evidence(
        &self,
    ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError>;

    async fn invoke(
        &self,
        request: &ActionRunnerRequest,
    ) -> Result<ActionRunnerResponse, ActionRunnerClientError>;
}

#[async_trait::async_trait]
impl GovernedActionRunnerGateway for ActionRunnerClient {
    fn capability_evidence(
        &self,
    ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
        Ok(ActionRunnerClient::capability_evidence(self).clone())
    }

    async fn invoke(
        &self,
        request: &ActionRunnerRequest,
    ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
        ActionRunnerClient::invoke(self, request).await
    }
}

fn ensure_filesystem_tool_authorization(
    runtime: &AppRuntime,
    client: &dyn GovernedActionRunnerGateway,
    envelope: &ActionEnvelope,
    now: chrono::DateTime<Utc>,
) -> Result<ActionRunnerAuthorization, ProtectedRootError> {
    let filesystem_authority = validate_governed_filesystem_envelope(envelope, now)?;
    let evidence = client.capability_evidence()?;
    let access = if envelope.effect_class == EffectClass::Observational {
        FilesystemAccess::Read
    } else {
        FilesystemAccess::Write
    };
    let manifest =
        governed_filesystem_tool_manifest(&evidence, &filesystem_authority.capability_root, access)
            .map_err(StorageError::InvalidData)?;
    runtime.register_tool_manifest(&manifest, now)?;
    let workflow = runtime
        .canonical_workflow(envelope.workflow_id)?
        .ok_or_else(|| StorageError::InvalidData("Tool workflow is missing".into()))?;
    let attempt = workflow.attempt;
    if attempt.attempt_id != envelope.attempt_id
        || attempt.task_contract != envelope.task_contract
        || attempt.state != WorkflowState::Running
        || attempt.lease_owner != Some(envelope.actor)
        || attempt.lease_expires_at.is_none_or(|expiry| expiry <= now)
    {
        return Err(StorageError::Conflict(
            "filesystem Tool action does not belong to the exact active workflow lease".into(),
        )
        .into());
    }
    let root_identity = runtime
        .root_ownership()?
        .ok_or_else(|| StorageError::InvalidData("Root ownership identity is missing".into()))?
        .runtime_identity;
    let mut deadline = filesystem_authority.expires_at;
    if let Some(reconciliation_deadline) = envelope.reconciliation_deadline {
        deadline = std::cmp::min(deadline, reconciliation_deadline);
    }
    if let Some(lease_expiry) = attempt.lease_expires_at {
        deadline = std::cmp::min(deadline, lease_expiry);
    }
    if deadline <= now {
        return Err(StorageError::Conflict("filesystem Tool authority has expired".into()).into());
    }
    let argument_length = u64::try_from(envelope.operation.bytes.len())
        .map_err(|_| StorageError::InvalidData("Tool argument length overflowed".into()))?;
    let proposal = ToolInvocationProposal {
        action_id: envelope.id,
        attempt_id: envelope.attempt_id,
        attempt_revision: attempt.revision,
        lease_generation: attempt.lease_generation,
        runtime_actor: envelope.actor,
        runtime_identity: root_identity.clone(),
        task_contract: envelope.task_contract.clone(),
        tool_manifest: manifest.versioned_ref()?,
        arguments: ArtifactRef {
            content_hash: envelope.operation_hash.clone(),
            media_type: "application/vnd.metacog.governed-filesystem-operation+cbor".into(),
            byte_length: argument_length,
        },
        argument_hash: envelope.operation_hash.clone(),
        requested_effect_class: envelope.effect_class,
        requested_capabilities: manifest.required_capabilities.clone(),
        requested_limits: manifest.resource_ceiling,
        required_sandbox: manifest.sandbox_requirement,
        platform: evidence.platform,
        deadline,
    };
    let policy = governed_filesystem_tool_policy(&manifest).map_err(StorageError::InvalidData)?;
    let receipt = authorize_and_persist_runtime_tool_capability(
        runtime,
        &proposal,
        &root_identity,
        &manifest,
        &policy,
        evidence.available_sandbox,
        evidence.platform,
        now,
    )?;
    Ok(ActionRunnerAuthorization {
        manifest,
        proposal: receipt.proposal,
        proposal_hash: receipt.proposal_hash,
        policy,
        permit: receipt.permit,
        permit_hash: receipt.permit_hash,
        permit_ref: receipt.permit_ref,
    })
}

const RECONCILIATION_PROBE_BINDING_TYPE: &str = "mcg.reconciliation_probe_binding.v1";
const RECONCILIATION_PROBE_BUDGET_TYPE: &str = "mcg.reconciliation_probe_budget.v1";

fn reconciliation_probe_ids(
    job_id: ReconciliationJobId,
    job_generation: u64,
) -> (ActionId, ActionExecutionId) {
    let mut action_hasher = Sha256::new();
    action_hasher.update(b"MetaCog.ReconciliationProbe.Action.v1\0");
    action_hasher.update(job_id.0.as_bytes());
    action_hasher.update(job_generation.to_be_bytes());
    let action_digest = action_hasher.finalize();
    let mut action_bytes = [0_u8; 16];
    action_bytes.copy_from_slice(&action_digest[..16]);
    action_bytes[6] = (action_bytes[6] & 0x0f) | 0x40;
    action_bytes[8] = (action_bytes[8] & 0x3f) | 0x80;

    let mut execution_hasher = Sha256::new();
    execution_hasher.update(b"MetaCog.ReconciliationProbe.Execution.v1\0");
    execution_hasher.update(job_id.0.as_bytes());
    execution_hasher.update(job_generation.to_be_bytes());
    let execution_digest = execution_hasher.finalize();
    let mut execution_bytes = [0_u8; 16];
    execution_bytes.copy_from_slice(&execution_digest[..16]);
    execution_bytes[6] = (execution_bytes[6] & 0x0f) | 0x40;
    execution_bytes[8] = (execution_bytes[8] & 0x3f) | 0x80;

    (
        ActionId(Uuid::from_bytes(action_bytes)),
        ActionExecutionId(Uuid::from_bytes(execution_bytes)),
    )
}

fn retire_previous_reconciliation_probe(
    runtime: &AppRuntime,
    job: &ReconciliationJob,
    now: chrono::DateTime<Utc>,
) -> Result<(), ProtectedRootError> {
    let Some(previous_generation) = job
        .lease_generation
        .checked_sub(1)
        .filter(|value| *value > 0)
    else {
        return Ok(());
    };
    let (action_id, _) = reconciliation_probe_ids(job.id, previous_generation);
    let Some(probe) = runtime.canonical_action(action_id, 32)? else {
        return Ok(());
    };
    let execution = latest_action_execution(&probe)?;
    if execution.state == ActionExecutionState::Authorized {
        runtime.abandon_authorized_observational_action(
            execution.id,
            execution.revision,
            execution.lease_generation,
            CorrelationId::new(),
            &format!(
                "abandon-reconciliation-probe:{}:{previous_generation}",
                job.id
            ),
            now,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn authorize_filesystem_reconciliation_probe(
    runtime: &AppRuntime,
    client: &dyn GovernedActionRunnerGateway,
    target_envelope: &ActionEnvelope,
    claimed: &ReconciliationClaimReceipt,
    now: chrono::DateTime<Utc>,
) -> Result<(ActionEnvelope, ActionExecution, ActionRunnerAuthorization), ProtectedRootError> {
    let target_authority = decode_original_filesystem_authority(target_envelope)?;
    let worker = claimed.job.lease_owner.ok_or_else(|| {
        StorageError::Conflict("reconciliation probe job has no current owner".into())
    })?;
    let lease_expiry = claimed.job.lease_expires_at.ok_or_else(|| {
        StorageError::Conflict("reconciliation probe job has no current lease".into())
    })?;
    let workflow = runtime
        .canonical_workflow(target_envelope.workflow_id)?
        .ok_or_else(|| StorageError::InvalidData("reconciliation workflow disappeared".into()))?;
    if workflow.attempt.attempt_id != target_envelope.attempt_id
        || workflow.attempt.task_contract != target_envelope.task_contract
        || workflow.attempt.state
            != WorkflowState::Waiting(mcg_contracts::WaitReason::Reconciliation)
        || workflow.attempt.lease_owner.is_some()
        || workflow.attempt.lease_expires_at.is_some()
    {
        return Err(StorageError::Conflict(
            "reconciliation probe workflow is not exactly fenced".into(),
        )
        .into());
    }
    let operation = claimed.job.adapter.clone();
    let operation_hash = operation.content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)?;
    let deadline = std::cmp::min(
        std::cmp::min(lease_expiry, claimed.job.deadline),
        now + Duration::seconds(PLUGIN_PERMIT_SECONDS),
    );
    if deadline <= now {
        return Err(StorageError::Conflict("reconciliation probe authority expired".into()).into());
    }
    let (action_id, execution_id) =
        reconciliation_probe_ids(claimed.job.id, claimed.job.lease_generation);
    let binding = VersionedPayload::canonical_v1(
        StableTypeId::parse(RECONCILIATION_PROBE_BINDING_TYPE)?,
        &serde_json::json!({
            "job_id": claimed.job.id,
            "job_revision": claimed.job.revision,
            "job_generation": claimed.job.lease_generation,
            "target_action_id": claimed.job.action_id,
            "target_execution_id": claimed.job.execution_id,
            "target_revision": claimed.execution.revision,
            "target_generation": claimed.execution.lease_generation,
            "adapter_hash": operation_hash,
        }),
    )?;
    let target_hash = canonical_contract_hash(
        ActionEnvelope::TYPE_ID,
        ACTION_ENVELOPE_HASH_DOMAIN,
        target_envelope,
    )?;
    let job_hash = canonical_contract_hash(
        ReconciliationJob::TYPE_ID,
        "MetaCog.ReconciliationJob",
        &claimed.job,
    )?;
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: target_envelope.workflow_id,
        attempt_id: target_envelope.attempt_id,
        task_contract: target_envelope.task_contract.clone(),
        actor: worker,
        origin: Some(ActionOrigin {
            conversation: None,
            invocation: None,
            command: None,
            approval: None,
            agent: None,
            evidence: vec![
                SourceRef {
                    source_type: StableTypeId::parse(ActionEnvelope::TYPE_ID)?,
                    locator: target_envelope.id.to_string(),
                    content_hash: target_hash,
                },
                SourceRef {
                    source_type: StableTypeId::parse(ReconciliationJob::TYPE_ID)?,
                    locator: claimed.job.id.to_string(),
                    content_hash: job_hash,
                },
            ],
        }),
        authority: governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
            action_id,
            actor: worker,
            capability_root: target_authority.capability_root.clone(),
            operation_hash: operation_hash.clone(),
            effect_class: EffectClass::Observational,
            expires_at: deadline,
        })
        .map_err(|_| ActionRunnerProtocolError::Invalid("reconciliation probe authority"))?,
        elevated_authority: None,
        expected_versions: Vec::new(),
        operation,
        operation_hash: operation_hash.clone(),
        effect_class: EffectClass::Observational,
        preconditions: vec![binding],
        budget_reservation: policy_binding(
            RECONCILIATION_PROBE_BUDGET_TYPE,
            "MetaCog.ReconciliationProbeBudget",
            &serde_json::json!({
                "job_id": claimed.job.id,
                "job_generation": claimed.job.lease_generation,
                "maximum_observations": 1,
            }),
        )?,
        idempotency_key: format!(
            "reconciliation-probe:{}:{}",
            claimed.job.id, claimed.job.lease_generation
        ),
        compensation: None,
        compensates_action: None,
        reconciler: None,
        reconciliation_deadline: None,
        created_at: now,
    };
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
    runtime.authorize_canonical_reconciliation_probe(
        &envelope,
        &execution,
        claimed.job.id,
        claimed.job.revision,
        claimed.job.lease_generation,
        claimed.execution.revision,
        claimed.execution.lease_generation,
        CorrelationId::new(),
        now,
    )?;

    let evidence = client.capability_evidence()?;
    let manifest = governed_filesystem_tool_manifest(
        &evidence,
        &target_authority.capability_root,
        FilesystemAccess::Read,
    )
    .map_err(StorageError::InvalidData)?;
    runtime.register_tool_manifest(&manifest, now)?;
    let root_identity = runtime
        .root_ownership()?
        .ok_or_else(|| StorageError::InvalidData("Root ownership identity is missing".into()))?
        .runtime_identity;
    let proposal = ToolInvocationProposal {
        action_id,
        attempt_id: envelope.attempt_id,
        attempt_revision: workflow.attempt.revision,
        lease_generation: workflow.attempt.lease_generation,
        runtime_actor: worker,
        runtime_identity: root_identity.clone(),
        task_contract: envelope.task_contract.clone(),
        tool_manifest: manifest.versioned_ref()?,
        arguments: ArtifactRef {
            content_hash: operation_hash.clone(),
            media_type: "application/vnd.metacog.governed-filesystem-operation+cbor".into(),
            byte_length: u64::try_from(envelope.operation.bytes.len())
                .map_err(|_| StorageError::InvalidData("Tool argument length overflowed".into()))?,
        },
        argument_hash: operation_hash,
        requested_effect_class: EffectClass::Observational,
        requested_capabilities: manifest.required_capabilities.clone(),
        requested_limits: manifest.resource_ceiling,
        required_sandbox: manifest.sandbox_requirement,
        platform: evidence.platform,
        deadline,
    };
    let policy = governed_filesystem_tool_policy(&manifest).map_err(StorageError::InvalidData)?;
    let receipt = authorize_and_persist_reconciliation_tool_capability(
        runtime,
        &proposal,
        &root_identity,
        &manifest,
        &policy,
        evidence.available_sandbox,
        evidence.platform,
        claimed.job.id,
        now,
    )?;
    Ok((
        envelope,
        execution,
        ActionRunnerAuthorization {
            manifest,
            proposal: receipt.proposal,
            proposal_hash: receipt.proposal_hash,
            policy,
            permit: receipt.permit,
            permit_hash: receipt.permit_hash,
            permit_ref: receipt.permit_ref,
        },
    ))
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
    let authorization = ensure_filesystem_tool_authorization(runtime, client, &envelope, now)?;
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
    let request =
        request_from_authorized_action(&envelope, &dispatched, authorization, request_id, now)?;
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
    let workflow = runtime
        .canonical_workflow(envelope.workflow_id)?
        .ok_or_else(|| StorageError::InvalidData("reconciliation workflow disappeared".into()))?;
    let active_authorization = match workflow.attempt.state {
        WorkflowState::Running => Some(ensure_filesystem_tool_authorization(
            runtime, client, &envelope, now,
        )?),
        WorkflowState::Waiting(mcg_contracts::WaitReason::Reconciliation)
            if workflow.attempt.lease_owner.is_none()
                && workflow.attempt.lease_expires_at.is_none() =>
        {
            None
        }
        _ => {
            return Err(StorageError::Conflict(
                "filesystem reconciliation workflow is neither active nor exactly fenced".into(),
            )
            .into());
        }
    };
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
    let (request, probe_dispatch) = if let Some(authorization) = active_authorization {
        (
            request_from_reconciliation_job(
                &envelope,
                &claimed.execution,
                &claimed.job,
                authorization,
                request_id,
                now,
            )?,
            None,
        )
    } else {
        retire_previous_reconciliation_probe(runtime, &claimed.job, now)?;
        let (probe_envelope, probe_execution, authorization) =
            authorize_filesystem_reconciliation_probe(runtime, client, &envelope, &claimed, now)?;
        let probe_correlation_id = CorrelationId::new();
        let dispatched_probe = runtime.dispatch_canonical_action(
            probe_execution.id,
            probe_execution.revision,
            probe_execution.lease_generation,
            worker,
            probe_correlation_id,
            &format!(
                "filesystem-reconcile-probe-dispatch:{}:{}",
                claimed.job.id, claimed.job.lease_generation
            ),
            now,
        )?;
        (
            request_from_authorized_action(
                &probe_envelope,
                &dispatched_probe,
                authorization,
                request_id,
                now,
            )?,
            Some((probe_envelope, dispatched_probe, probe_correlation_id)),
        )
    };
    let runner_result = client.invoke(&request).await;
    if let Some((probe_envelope, dispatched_probe, probe_correlation_id)) = probe_dispatch {
        match &runner_result {
            Ok(response) => {
                classify_runner_response(
                    runtime,
                    &probe_envelope,
                    &dispatched_probe,
                    probe_correlation_id,
                    request_id,
                    response.clone(),
                )?;
            }
            Err(error) => {
                classify_runner_failure(
                    runtime,
                    &probe_envelope,
                    &dispatched_probe,
                    probe_correlation_id,
                    request_id,
                    error,
                )?;
            }
        }
    }
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
    let data_directory = config.data_directory.clone();
    let root_executable = std::env::current_exe()?;
    let runtime = Arc::new(AppRuntime::open_with_data_directory_lock(
        config, data_lock,
    )?);
    let identity = current_runtime_identity()?;
    runtime.initialize_root_ownership(identity.clone(), migration_receipt)?;
    runtime.install_protected_plugin_dispatcher(Arc::new(RootPluginDispatcher {
        runtime: Arc::downgrade(&runtime),
        root_executable: root_executable.clone(),
        runtime_actor: RuntimeActorId(identity.instance_id.0),
        runtime_identity: identity.clone(),
        capability_sessions: Mutex::new(BTreeMap::new()),
        #[cfg(test)]
        test_gateway: None,
        #[cfg(test)]
        test_action_gateway: None,
    }))?;
    runtime.install_protected_agent_model_dispatcher(Arc::new(RootAgentModelDispatcher {
        runtime: Arc::downgrade(&runtime),
        root_executable: root_executable.clone(),
        runtime_actor: RuntimeActorId(identity.instance_id.0),
        runtime_identity: identity,
    }))?;
    runtime.install_protected_compatibility_verifier(Arc::new(RootCompatibilityVerifier {
        runtime: Arc::downgrade(&runtime),
        root_executable,
        data_directory,
    }))?;
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
                    Ok(receipt) => {
                        debug!(
                            job_id = %receipt.resolution.job.id,
                            action_id = %receipt.resolution.job.action_id,
                            state = ?receipt.resolution.job.state,
                            "completed governed filesystem reconciliation attempt"
                        );
                        if let Err(error) = commit_reconciled_plugin_filesystem_effect(
                            &runtime,
                            &receipt.resolution,
                            worker,
                            Utc::now(),
                        ) {
                            warn!(
                                action_id = %receipt.resolution.job.action_id,
                                %error,
                                "could not validate and commit reconciled plugin effect"
                            );
                            continue;
                        }
                        if receipt.resolution.job.state
                            == mcg_contracts::ReconciliationJobState::Resolved
                            && let Err(error) = settle_reconciled_plugin_action_workflow(
                                &runtime,
                                receipt.resolution.job.action_id,
                                Utc::now(),
                            )
                        {
                            warn!(
                                action_id = %receipt.resolution.job.action_id,
                                %error,
                                "could not settle reconciled plugin authority workflow"
                            );
                        }
                    }
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

fn settle_reconciled_plugin_action_workflow(
    runtime: &AppRuntime,
    action_id: ActionId,
    settled_at: chrono::DateTime<Utc>,
) -> Result<(), RuntimeError> {
    let Some(action) = runtime.canonical_action(action_id, 32)? else {
        return Ok(());
    };
    let Some(source) = action
        .envelope
        .origin
        .as_ref()
        .and_then(|origin| origin.invocation.as_ref())
    else {
        return Ok(());
    };
    if source.source_type.as_str() != "mcg.plugin_invocation_source.v1" {
        return Ok(());
    }
    let invocation_id = source
        .locator
        .parse::<PluginInvocationId>()
        .map_err(|error| RuntimeError::Storage(StorageError::InvalidData(error.to_string())))?;
    runtime.root_settle_reconciled_plugin_invocation_workflow(invocation_id, settled_at)?;
    Ok(())
}

fn commit_reconciled_plugin_filesystem_effect(
    runtime: &AppRuntime,
    resolution: &ReconciliationResolutionReceipt,
    validator: RuntimeActorId,
    validated_at: chrono::DateTime<Utc>,
) -> Result<(), RuntimeError> {
    if resolution.job.state != mcg_contracts::ReconciliationJobState::Resolved
        || resolution.execution.state != ActionExecutionState::EffectConfirmed
    {
        return Ok(());
    }
    let Some(action) = runtime.canonical_action(resolution.job.action_id, 32)? else {
        return Ok(());
    };
    let Some(source) = action
        .envelope
        .origin
        .as_ref()
        .and_then(|origin| origin.invocation.as_ref())
    else {
        return Ok(());
    };
    if source.source_type.as_str() != "mcg.plugin_invocation_source.v1" {
        return Ok(());
    }
    if !matches!(
        decode_governed_filesystem_operation(&action.envelope.operation)
            .map_err(|error| RuntimeError::Storage(StorageError::InvalidData(error.to_string())))?,
        GovernedFilesystemOperation::CreateFile { .. }
    ) {
        return Err(RuntimeError::Storage(StorageError::InvalidData(
            "reconciled plugin effect is not an exact create-only filesystem action".into(),
        )));
    }
    let postcondition = resolution
        .receipt
        .as_ref()
        .map(|receipt| receipt.observation.clone())
        .ok_or_else(|| {
            RuntimeError::Storage(StorageError::InvalidData(
                "reconciled plugin effect has no independent observation".into(),
            ))
        })?;
    let validation = ActionValidation {
        action_id: resolution.execution.action_id,
        execution_id: resolution.execution.id,
        validator,
        evidence: Vec::new(),
        postconditions: vec![postcondition],
        validated_at,
    };
    let validated = runtime.validate_canonical_action_effect(
        resolution.execution.id,
        resolution.execution.revision,
        resolution.execution.lease_generation,
        &validation,
        CorrelationId::new(),
        &format!(
            "validate-reconciled-plugin-filesystem:{}",
            resolution.execution.action_id
        ),
        validated_at,
    )?;
    let validation_event = runtime
        .canonical_action(validated.action_id, 32)?
        .and_then(|inspection| inspection.events.last().cloned())
        .ok_or_else(|| {
            RuntimeError::Storage(StorageError::InvalidData(
                "reconciled plugin validation event is missing".into(),
            ))
        })?;
    runtime.commit_canonical_action_effect(
        validated.id,
        validated.revision,
        validated.lease_generation,
        validation_event.id,
        CorrelationId::new(),
        &format!(
            "commit-reconciled-plugin-filesystem:{}",
            validated.action_id
        ),
        validated_at,
    )?;
    Ok(())
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

#[derive(serde::Deserialize)]
struct CapabilityListRequest {
    limit: usize,
}

#[derive(serde::Deserialize)]
struct CapabilityShowRequest {
    action_id: ActionId,
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
            | ROOT_CAPABILITY_LIST_REQUEST_TYPE
            | ROOT_CAPABILITY_SHOW_REQUEST_TYPE
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
        ROOT_CAPABILITY_LIST_REQUEST_TYPE => {
            let list: CapabilityListRequest = serde_json::from_value(request_value)?;
            if list.limit == 0 || list.limit > MAX_ROOT_TOOL_AUTHORIZATION_SUMMARIES {
                return Err(PlatformError::InvalidInspectionBound.into());
            }
            (
                ROOT_CAPABILITY_LIST_RESPONSE_TYPE,
                serde_json::to_value(runtime.tool_authorizations(list.limit, Utc::now())?)?,
            )
        }
        ROOT_CAPABILITY_SHOW_REQUEST_TYPE => {
            let show: CapabilityShowRequest = serde_json::from_value(request_value)?;
            (
                ROOT_CAPABILITY_SHOW_RESPONSE_TYPE,
                serde_json::to_value(runtime.tool_authorization(show.action_id, Utc::now())?)?,
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
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentRun,
        AgentRunState, AgentState, ConfidenceEstimate, ExecutivePlanner, LeaseBudget,
        OrganizationGenome, OrganizationalMetrics, PlannedWork, PlanningRequest, RoleKind,
        SuccessCondition, TaskComplexity,
    };
    use mcg_contracts::{
        ActionEnvelope, ActionExecutionState, ActionId, ActionValidation, AttemptId,
        ConfigurationId, ContextManifest, ContextManifestId, ContextSelection, ContractClause,
        CorrelationId, EffectClass, HashAlgorithm, InvocationManifest, InvocationRole, ModelBudget,
        ModelDataClassification, ModelDisclosureManifest, ModelEgressRoute, ModelRun, ModelRunId,
        ModelRunState, PolicyBinding, RetryPolicy, RuntimeActorId, SessionId, SourceRef,
        TaskContract, TaskContractId, TaskContractStatus, VersionedRef, WaitReason,
        WorkflowAttempt, WorkflowId, WorkflowState,
    };
    use mcg_domain::{
        AgentInstanceId, PLUGIN_CATALOG_SCHEMA_VERSION, PluginCapabilityGrant,
        PluginCapabilityScope, PluginInstallSource, PluginInvocationId, PluginRevision,
        PluginRevisionId, PluginSignature, PluginSignatureState, ProjectId, RunId, TaskEnvelope,
        TaskState,
    };
    use mcg_platform::{
        cancel_root_workflow, exchange_root_rpc, query_root_action, query_root_actions,
        query_root_capabilities, query_root_capability, query_root_invocation,
        query_root_invocations, query_root_status, query_root_workflow,
        query_root_workflow_activity, query_root_workflows, root_ipc_session_path,
    };
    use mcg_plugin_protocol::{
        PLUGIN_SCHEMA_VERSION, PluginCapability, PluginCapabilityOperation,
        PluginCapabilityRequest, PluginClass, PluginGuestTurn, PluginHostTurn, PluginManifest,
        PluginResult,
    };
    use mcg_providers::{
        CONTRACT_VERSION, ContentPart, Message, MessageRole, ModelCapabilities, ModelDescriptor,
        ModelRequest, ModelResponse, OutputItem, ProviderAuthMode, ProviderConnection,
        ProviderKind, ProviderModelCatalog, Usage,
    };
    use mcg_storage::{SqliteEventStore, task_contract_content_hash, verify_backup_bundle};

    #[test]
    fn plugin_filesystem_replacement_fails_closed_without_preimage_authority() {
        require_create_only_plugin_write(true).expect("create-only writes are eligible");
        let error = require_create_only_plugin_write(false)
            .expect_err("replacement must require exact reversible preimage authority");
        assert!(matches!(
            error,
            ToolError::Denied(ref message)
                if message
                    == "plugin filesystem replacement is awaiting exact reversible preimage authority"
        ));
    }

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

    #[allow(clippy::too_many_lines)]
    fn seed_compatibility_agent_provider(
        store: &mut SqliteEventStore,
        at: chrono::DateTime<Utc>,
    ) -> (AgentLease, ProviderConnection, ModelDescriptor) {
        let task = TaskEnvelope::new("complete one governed compatibility model call");
        store.create_task(&task).expect("create compatibility task");
        let task = store
            .transition_task(task.id, TaskState::Active)
            .expect("activate compatibility task")
            .0;
        let organization = OrganizationGenome::consumer_default(at);
        store
            .record_organization_revision(&organization)
            .expect("record compatibility organization");
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "produce one governed provider response",
                RoleKind::Coder,
                vec![SuccessCondition::Custom {
                    description: "the response is permit bound".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&organization, &planning, at)
            .expect("plan compatibility work");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: organization.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: at,
            updated_at: at,
            completed_at: None,
        };
        store
            .create_agent_run(&run, &plan)
            .expect("create compatibility run");
        store
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
                at,
            )
            .expect("start compatibility run");
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:governed-provider-context".into(),
            confidence: ConfidenceEstimate::new(5_000, 0).expect("confidence"),
            created_at: at,
            updated_at: at,
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
            max_retries: 2,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: at,
            expires_at: at + chrono::Duration::minutes(10),
            heartbeat_at: at,
        };
        store
            .dispatch_agent_work(&instance, &lease, at)
            .expect("dispatch compatibility agent");
        let connection = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "compatibility_governed_fixture".into(),
            display_name: "Compatibility governed fixture".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://provider.example/v1".into(),
            auth_mode: ProviderAuthMode::None,
            organization: None,
            project: None,
        };
        let model = ModelDescriptor {
            id: "governed-agent-model".into(),
            display_name: "Governed agent model".into(),
            owned_by: Some("fixture".into()),
            context_window: Some(8_192),
            capabilities: ModelCapabilities {
                responses: Some(true),
                streaming: Some(true),
                ..ModelCapabilities::default()
            },
            price: Some(mcg_providers::ModelPrice {
                input_per_million: Some(1.0),
                cached_input_per_million: Some(0.5),
                output_per_million: Some(2.0),
                currency: "USD".into(),
                provenance: mcg_providers::PriceProvenance::McgCatalog {
                    catalog_date: "2026-09-01".into(),
                },
            }),
            metadata: serde_json::Value::Null,
        };
        store
            .record_provider_configuration(
                &connection,
                &ProviderModelCatalog {
                    connection_id: connection.id.clone(),
                    models: vec![model.clone()],
                    discovered_at: at,
                },
                at,
            )
            .expect("record governed provider configuration");
        (lease, connection, model)
    }

    struct DeterministicModelGateway {
        calls: Mutex<usize>,
        response: ModelResponse,
    }

    struct RecordingPluginGateway {
        runner: SourceRef,
        requests: Arc<Mutex<Vec<PluginRunnerRequest>>>,
    }

    struct ScriptedProtectedPluginGateway {
        runner: SourceRef,
        requests: Arc<Mutex<Vec<PluginRunnerRequest>>>,
        turns: Mutex<VecDeque<PluginGuestTurn>>,
    }

    #[async_trait::async_trait]
    impl RootPluginRunnerGateway for RecordingPluginGateway {
        fn runner_source(&self) -> &SourceRef {
            &self.runner
        }

        async fn invoke(
            &self,
            request: &PluginRunnerRequest,
        ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
            if request.authorization.is_none() {
                return Err(PluginRunnerClientError::Denied(
                    "fixture received no Root authority".into(),
                ));
            }
            self.requests
                .lock()
                .expect("plugin requests")
                .push(request.clone());
            Ok(PluginRunnerResponse {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: Some(request.manifest.id.clone()),
                state: mcg_plugin_runner::PluginRunnerState::Succeeded,
                result: Some(PluginResult {
                    schema_version: PLUGIN_SCHEMA_VERSION,
                    plugin_id: request.manifest.id.clone(),
                    output: request.input.clone(),
                }),
                error_code: None,
                detail: None,
            })
        }
    }

    #[async_trait::async_trait]
    impl RootPluginRunnerGateway for ScriptedProtectedPluginGateway {
        fn runner_source(&self) -> &SourceRef {
            &self.runner
        }

        async fn invoke(
            &self,
            request: &PluginRunnerRequest,
        ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
            if request.authorization.is_none() {
                return Err(PluginRunnerClientError::Denied(
                    "fixture received no Root authority".into(),
                ));
            }
            self.requests
                .lock()
                .expect("plugin requests")
                .push(request.clone());
            let turn = self
                .turns
                .lock()
                .expect("plugin turns")
                .pop_front()
                .ok_or(PluginRunnerClientError::InvalidResponse)?;
            Ok(PluginRunnerResponse {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: Some(request.manifest.id.clone()),
                state: mcg_plugin_runner::PluginRunnerState::Succeeded,
                result: Some(PluginResult {
                    schema_version: PLUGIN_SCHEMA_VERSION,
                    plugin_id: request.manifest.id.clone(),
                    output: serde_json::to_value(turn)
                        .map_err(|_| PluginRunnerClientError::InvalidResponse)?,
                }),
                error_code: None,
                detail: None,
            })
        }
    }

    struct PluginReadActionGateway {
        calls: Arc<Mutex<Vec<ActionRunnerRequest>>>,
    }

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for PluginReadActionGateway {
        fn capability_evidence(
            &self,
        ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
            Ok(scripted_action_capability_evidence())
        }

        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            assert_eq!(request.authorization.proposal.action_id, request.action_id);
            assert_eq!(request.authorization.permit.action_id, request.action_id);
            assert_eq!(
                request.authorization.proposal.argument_hash,
                request.envelope.operation_hash
            );
            let GovernedFilesystemOperation::ReadFile {
                relative_path,
                max_bytes,
            } = decode_governed_filesystem_operation(&request.operation)
                .expect("governed plugin read operation")
            else {
                panic!("plugin read gateway received another operation")
            };
            let bytes = fs::read(request.capability_root.join(&relative_path))
                .expect("read protected plugin fixture");
            assert!(u64::try_from(bytes.len()).expect("fixture length") <= max_bytes);
            self.calls
                .lock()
                .expect("plugin read calls")
                .push(request.clone());
            let now = Utc::now();
            let observation = VersionedPayload::canonical_v1(
                StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
                    .expect("observation type"),
                &serde_json::json!({
                    "relative_path": relative_path,
                    "exists": true,
                    "sha256": format!("{:x}", Sha256::digest(&bytes)),
                    "size_bytes": bytes.len(),
                    "content_base64": STANDARD.encode(&bytes),
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
                detail: "read protected plugin fixture".into(),
                started_at: now,
                finished_at: now,
            })
        }
    }

    struct PluginCreateActionGateway {
        calls: Arc<Mutex<Vec<ActionRunnerRequest>>>,
    }

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for PluginCreateActionGateway {
        fn capability_evidence(
            &self,
        ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
            Ok(scripted_action_capability_evidence())
        }

        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            assert_eq!(request.authorization.proposal.action_id, request.action_id);
            assert_eq!(request.authorization.permit.action_id, request.action_id);
            assert_eq!(
                request.authorization.proposal.requested_effect_class,
                EffectClass::Reversible
            );
            assert_eq!(
                request.authorization.permit.allowed_effect_class,
                EffectClass::Reversible
            );
            let GovernedFilesystemOperation::CreateFile {
                relative_path,
                content_base64,
            } = decode_governed_filesystem_operation(&request.operation)
                .expect("governed plugin create operation")
            else {
                panic!("plugin create gateway received another operation")
            };
            let bytes = STANDARD
                .decode(content_base64)
                .expect("decode protected create bytes");
            let path = request.capability_root.join(&relative_path);
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            std::io::Write::write_all(
                &mut options
                    .open(&path)
                    .expect("create protected plugin fixture"),
                &bytes,
            )
            .expect("write protected plugin fixture");
            self.calls
                .lock()
                .expect("plugin create calls")
                .push(request.clone());
            let now = Utc::now();
            let observation = VersionedPayload::canonical_v1(
                StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
                    .expect("observation type"),
                &serde_json::json!({
                    "relative_path": relative_path,
                    "exists": true,
                    "sha256": format!("{:x}", Sha256::digest(&bytes)),
                    "size_bytes": bytes.len(),
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
                detail: "created protected plugin fixture".into(),
                started_at: now,
                finished_at: now,
            })
        }
    }

    struct PluginLostCreateResponseGateway {
        calls: Arc<Mutex<Vec<ActionRunnerRequest>>>,
    }

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for PluginLostCreateResponseGateway {
        fn capability_evidence(
            &self,
        ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
            Ok(scripted_action_capability_evidence())
        }

        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            self.calls
                .lock()
                .expect("lost create calls")
                .push(request.clone());
            match decode_governed_filesystem_operation(&request.operation)
                .expect("lost create or reconciliation operation")
            {
                GovernedFilesystemOperation::CreateFile {
                    relative_path,
                    content_base64,
                } => {
                    let bytes = STANDARD
                        .decode(content_base64)
                        .map_err(|_| ActionRunnerClientError::InvalidResponse)?;
                    fs::write(request.capability_root.join(relative_path), bytes)
                        .map_err(|_| ActionRunnerClientError::Process)?;
                    Err(ActionRunnerClientError::Process)
                }
                GovernedFilesystemOperation::InspectFile { relative_path, .. } => {
                    assert_eq!(
                        request.authorization.proposal.requested_effect_class,
                        EffectClass::Observational
                    );
                    assert_eq!(
                        request.authorization.permit.allowed_effect_class,
                        EffectClass::Observational
                    );
                    let bytes = fs::read(request.capability_root.join(&relative_path))
                        .map_err(|_| ActionRunnerClientError::Process)?;
                    let now = Utc::now();
                    Ok(ActionRunnerResponse {
                        protocol_version: mcg_action_runner::ACTION_RUNNER_PROTOCOL_VERSION,
                        request_id: Some(request.request_id),
                        action_id: Some(request.action_id),
                        execution_id: Some(request.execution_id),
                        outcome: RunnerEffectOutcome::Observational,
                        observation: Some(
                            VersionedPayload::canonical_v1(
                                StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
                                    .expect("observation type"),
                                &serde_json::json!({
                                    "relative_path": relative_path,
                                    "exists": true,
                                    "sha256": format!("{:x}", Sha256::digest(&bytes)),
                                    "size_bytes": bytes.len(),
                                    "content_base64": null,
                                }),
                            )
                            .expect("reconciliation observation"),
                        ),
                        error_code: None,
                        detail: "inspected lost plugin create".into(),
                        started_at: now,
                        finished_at: now,
                    })
                }
                _ => Err(ActionRunnerClientError::InvalidResponse),
            }
        }
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for DeterministicModelGateway {
        async fn invoke(
            &self,
            _authorization: &RuntimeModelEgressAuthorizationReceipt,
            _request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            *self.calls.lock().expect("model gateway call lock") += 1;
            Ok(self.response.clone())
        }
    }

    struct RecordingAgentProviderGateway {
        calls: Mutex<Vec<(RuntimeModelEgressAuthorizationReceipt, ModelRequest)>>,
        response: ModelResponse,
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for RecordingAgentProviderGateway {
        async fn invoke(
            &self,
            authorization: &RuntimeModelEgressAuthorizationReceipt,
            request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            self.calls
                .lock()
                .expect("agent provider call lock")
                .push((authorization.clone(), request.clone()));
            Ok(self.response.clone())
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ScriptedGatewayFailure {
        RetryFirst,
        AlwaysRetryable,
        FinalBeforeContact,
        OutcomeUnknown,
    }

    struct ScriptedAgentProviderGateway {
        calls: Mutex<Vec<(RuntimeModelEgressAuthorizationReceipt, ModelRequest)>>,
        response: ModelResponse,
        failure: ScriptedGatewayFailure,
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for ScriptedAgentProviderGateway {
        async fn invoke(
            &self,
            authorization: &RuntimeModelEgressAuthorizationReceipt,
            request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            let mut calls = self.calls.lock().expect("scripted gateway calls");
            calls.push((authorization.clone(), request.clone()));
            let call_number = calls.len();
            drop(calls);
            let class = match self.failure {
                ScriptedGatewayFailure::RetryFirst if call_number == 1 => {
                    Some(GovernedProviderFailureClass::RetryableBeforeProviderContact)
                }
                ScriptedGatewayFailure::RetryFirst => None,
                ScriptedGatewayFailure::AlwaysRetryable => {
                    Some(GovernedProviderFailureClass::RetryableBeforeProviderContact)
                }
                ScriptedGatewayFailure::FinalBeforeContact => {
                    Some(GovernedProviderFailureClass::FinalBeforeProviderContact)
                }
                ScriptedGatewayFailure::OutcomeUnknown => {
                    Some(GovernedProviderFailureClass::OutcomeUnknown)
                }
            };
            class.map_or_else(
                || Ok(self.response.clone()),
                |class| {
                    Err(GovernedModelProviderGatewayError::new(
                        class,
                        ProviderError::Request {
                            message: "scripted governed gateway failure".into(),
                            retriable: class
                                == GovernedProviderFailureClass::RetryableBeforeProviderContact,
                        },
                    ))
                },
            )
        }
    }

    struct CancellingAgentProviderGateway<'a> {
        runtime: &'a AppRuntime,
        calls: Mutex<Vec<InvocationId>>,
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for CancellingAgentProviderGateway<'_> {
        async fn invoke(
            &self,
            authorization: &RuntimeModelEgressAuthorizationReceipt,
            _request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            self.calls
                .lock()
                .expect("cancelling gateway calls")
                .push(authorization.proposal.invocation_id);
            self.runtime
                .cancel_canonical_workflow(&WorkflowCancelRequest {
                    attempt_id: authorization.proposal.attempt_id,
                    expected_revision: authorization.proposal.attempt_revision,
                    expected_generation: authorization.proposal.lease_generation,
                    idempotency_key: format!(
                        "cancel-provider-{}",
                        authorization.proposal.invocation_id
                    ),
                })
                .expect("cancel provider workflow");
            std::future::pending().await
        }
    }

    struct ControlledStreamingGateway {
        calls: Mutex<Vec<(RuntimeModelEgressAuthorizationReceipt, ModelRequest)>>,
        response: ModelResponse,
        release_completion: Arc<tokio::sync::Notify>,
        retry_first: bool,
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for ControlledStreamingGateway {
        async fn invoke(
            &self,
            _authorization: &RuntimeModelEgressAuthorizationReceipt,
            _request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            Err(GovernedModelProviderGatewayError::new(
                GovernedProviderFailureClass::FinalBeforeProviderContact,
                ProviderError::Configuration("buffered invocation is not expected".into()),
            ))
        }

        async fn stream(
            &self,
            authorization: &RuntimeModelEgressAuthorizationReceipt,
            request: &ModelRequest,
        ) -> Result<GovernedModelProviderStream, GovernedModelProviderGatewayError> {
            let call_number = {
                let mut calls = self.calls.lock().expect("streaming gateway calls");
                calls.push((authorization.clone(), request.clone()));
                calls.len()
            };
            if self.retry_first && call_number == 1 {
                return Err(GovernedModelProviderGatewayError::new(
                    GovernedProviderFailureClass::RetryableBeforeProviderContact,
                    ProviderError::Request {
                        message: "stream runner was unavailable before provider contact".into(),
                        retriable: true,
                    },
                ));
            }
            let response = self.response.clone();
            let release = self.release_completion.clone();
            Ok(Box::pin(futures_util::stream::unfold(
                (0_u8, response, release),
                |(step, response, release)| async move {
                    let event = match step {
                        0 => ModelStreamEvent::ResponseCreated {
                            id: response.id.clone(),
                            model: Some(response.model.clone()),
                        },
                        1 => ModelStreamEvent::OutputTextDelta {
                            item_id: Some("stream-message".into()),
                            delta: "incremental".into(),
                        },
                        2 => {
                            release.notified().await;
                            ModelStreamEvent::Completed {
                                response: response.clone(),
                            }
                        }
                        _ => return None,
                    };
                    Some((Ok(event), (step.saturating_add(1), response, release)))
                },
            )))
        }
    }

    struct LeaseAdvancingModelGateway<'a> {
        runtime: &'a AppRuntime,
        attempt_id: AttemptId,
        attempt_revision: u64,
        lease_generation: u64,
        response: ModelResponse,
        at: chrono::DateTime<Utc>,
    }

    #[async_trait::async_trait]
    impl GovernedModelProviderGateway for LeaseAdvancingModelGateway<'_> {
        async fn invoke(
            &self,
            _authorization: &RuntimeModelEgressAuthorizationReceipt,
            _request: &ModelRequest,
        ) -> Result<ModelResponse, GovernedModelProviderGatewayError> {
            self.runtime
                .begin_runtime_verification(
                    self.attempt_id,
                    self.attempt_revision,
                    self.lease_generation,
                    "provider-post-dispatch-lease-advance",
                    self.at,
                )
                .expect("advance workflow lease during provider dispatch");
            Ok(self.response.clone())
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
            observed_model_fingerprint: None,
            outcome_evidence: None,
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

    fn scripted_action_capability_evidence() -> ActionRunnerCapabilityEvidence {
        ActionRunnerCapabilityEvidence {
            runner: root_invocation_source("scripted-action-runner"),
            platform: mcg_action_runner::current_execution_platform(),
            available_sandbox: mcg_action_runner::action_runner_sandbox_profile(),
            resource_ceiling: mcg_contracts::ToolResourceLimits {
                timeout_ms: 30_000,
                max_output_bytes: u64::try_from(
                    mcg_action_runner::ACTION_RUNNER_RESPONSE_MAX_BYTES,
                )
                .expect("response bound"),
                max_memory_bytes: 0,
                max_child_processes: 0,
            },
        }
    }

    struct ScriptedActionGateway(GatewayOutcome);

    #[async_trait::async_trait]
    impl GovernedActionRunnerGateway for ScriptedActionGateway {
        fn capability_evidence(
            &self,
        ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
            Ok(scripted_action_capability_evidence())
        }

        async fn invoke(
            &self,
            request: &ActionRunnerRequest,
        ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
            assert_eq!(request.envelope.id, request.action_id);
            assert_eq!(request.authorization.proposal.action_id, request.action_id);
            assert_eq!(request.authorization.permit.action_id, request.action_id);
            assert_eq!(
                request.authorization.proposal.argument_hash,
                request.envelope.operation_hash
            );
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
        fn capability_evidence(
            &self,
        ) -> Result<ActionRunnerCapabilityEvidence, ActionRunnerClientError> {
            Ok(scripted_action_capability_evidence())
        }

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
    fn provider_runner_failures_use_conservative_contact_classification() {
        for (error, expected) in [
            (
                ProviderRunnerClientError::Launch,
                GovernedProviderFailureClass::RetryableBeforeProviderContact,
            ),
            (
                ProviderRunnerClientError::ExecutableChanged,
                GovernedProviderFailureClass::FinalBeforeProviderContact,
            ),
            (
                ProviderRunnerClientError::Denied("authority_mismatch".into()),
                GovernedProviderFailureClass::FinalBeforeProviderContact,
            ),
            (
                ProviderRunnerClientError::Delivery,
                GovernedProviderFailureClass::OutcomeUnknown,
            ),
            (
                ProviderRunnerClientError::Provider("provider_timeout".into()),
                GovernedProviderFailureClass::OutcomeUnknown,
            ),
        ] {
            assert_eq!(classify_provider_runner_failure(&error).class(), expected);
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn compatibility_agent_call_claims_canonical_authority_and_uses_one_permit_gateway() {
        let temporary = tempfile::tempdir().expect("temporary compatibility provider directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let at = Utc::now();
        let runtime = open(config.clone()).expect("open governed compatibility Root");
        let (lease, connection, model) = {
            let mut store = SqliteEventStore::open(config.database_path())
                .expect("compatibility provider store");
            seed_compatibility_agent_provider(&mut store, at)
        };
        let identity = current_runtime_identity().expect("runtime identity");
        let actor = RuntimeActorId(identity.instance_id.0);
        let target = mcg_providers::ProviderModelRef {
            provider_id: connection.id.clone(),
            model_id: model.id.clone(),
        };
        let request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: None,
            instructions: Some("Use only the exact bounded request.".into()),
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Return the governed result.".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(64),
            metadata: serde_json::json!({"agent_lease_id": lease.id}),
        };
        let gateway = RecordingAgentProviderGateway {
            calls: Mutex::new(Vec::new()),
            response: ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "governed-agent-response".into(),
                model: model.id.clone(),
                status: Some("completed".into()),
                output: vec![OutputItem::Message {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: "governed".into(),
                    }],
                }],
                usage: Usage {
                    input_tokens: 16,
                    output_tokens: 1,
                    total_tokens: 17,
                    ..Usage::default()
                },
                provider_metadata: serde_json::Value::Null,
            },
        };

        let response = execute_compatibility_agent_model_invocation(
            &runtime,
            &gateway,
            &connection,
            &model,
            actor,
            &identity,
            lease.id,
            &target,
            request,
            at,
        )
        .await
        .expect("governed compatibility invocation");
        assert_eq!(response.id, "governed-agent-response");
        let calls = gateway.calls.lock().expect("governed gateway calls");
        assert_eq!(calls.len(), 1);
        let (authorization, dispatched_request) = &calls[0];
        assert_eq!(authorization.proposal.runtime_actor, actor);
        assert_eq!(authorization.proposal.runtime_identity, identity);
        assert_eq!(authorization.proposal.attempt_revision, 2);
        assert_eq!(authorization.proposal.lease_generation, 1);
        assert_eq!(
            authorization.proposal.request_hash,
            model_provider_request_artifact(dispatched_request)
                .expect("dispatched request artifact")
                .content_hash
        );
        assert!(
            dispatched_request
                .client_request_id
                .as_deref()
                .is_some_and(|id| id.starts_with("mcg-governed-"))
        );
        assert_eq!(
            runtime
                .canonical_workflows()
                .expect("canonical workflows")
                .into_iter()
                .map(|attempt| attempt.state)
                .collect::<Vec<_>>(),
            vec![WorkflowState::Running]
        );
        let inspection = runtime
            .runtime_invocation(authorization.proposal.invocation_id, 10)
            .expect("invocation inspection")
            .expect("governed invocation exists");
        assert_eq!(
            inspection.manifest.egress_permit,
            Some(authorization.permit_ref.clone())
        );
        let completed_run = &inspection
            .model_runs
            .last()
            .expect("completed model run")
            .run;
        assert_eq!(completed_run.state, ModelRunState::Completed);
        let outcome = completed_run
            .outcome_evidence
            .as_ref()
            .expect("completed provider outcome evidence");
        assert_eq!(outcome.actual_cost_microusd, Some(18));
        assert!(matches!(
            outcome.cost_provenance,
            ModelRunCostProvenance::ProviderCatalog { .. }
        ));
        assert!(outcome.error_class.is_none());
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn compatibility_provider_retries_only_proven_no_contact_with_fresh_authority() {
        for (failure, expected_calls, expected_states) in [
            (
                ScriptedGatewayFailure::RetryFirst,
                2,
                vec![ModelRunState::Failed, ModelRunState::Completed],
            ),
            (
                ScriptedGatewayFailure::FinalBeforeContact,
                1,
                vec![ModelRunState::Failed],
            ),
            (
                ScriptedGatewayFailure::OutcomeUnknown,
                1,
                vec![ModelRunState::OutcomeUnknown],
            ),
            (
                ScriptedGatewayFailure::AlwaysRetryable,
                2,
                vec![ModelRunState::Failed, ModelRunState::Failed],
            ),
        ] {
            let temporary = tempfile::tempdir().expect("temporary compatibility retry directory");
            let config = AppConfig::default().with_data_directory(temporary.path());
            let at = Utc::now();
            let runtime = open(config.clone()).expect("open compatibility retry Root");
            let (lease, connection, model) = {
                let mut store = SqliteEventStore::open(config.database_path())
                    .expect("compatibility retry store");
                seed_compatibility_agent_provider(&mut store, at)
            };
            let identity = current_runtime_identity().expect("runtime identity");
            let actor = RuntimeActorId(identity.instance_id.0);
            let target = mcg_providers::ProviderModelRef {
                provider_id: connection.id.clone(),
                model_id: model.id.clone(),
            };
            let request = ModelRequest {
                contract_version: CONTRACT_VERSION,
                model: model.id.clone(),
                client_request_id: None,
                instructions: Some("Use the bounded retry fixture.".into()),
                messages: vec![Message {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Return the retry result.".into(),
                    }],
                    name: None,
                }],
                tools: Vec::new(),
                tool_choice: None,
                structured_output: None,
                reasoning: None,
                temperature: None,
                top_p: None,
                max_output_tokens: Some(64),
                metadata: serde_json::json!({"agent_lease_id": lease.id}),
            };
            let gateway = ScriptedAgentProviderGateway {
                calls: Mutex::new(Vec::new()),
                response: ModelResponse {
                    contract_version: CONTRACT_VERSION,
                    id: "governed-retry-response".into(),
                    model: model.id.clone(),
                    status: Some("completed".into()),
                    output: vec![OutputItem::Message {
                        role: MessageRole::Assistant,
                        content: vec![ContentPart::Text {
                            text: "retried safely".into(),
                        }],
                    }],
                    usage: Usage {
                        input_tokens: 16,
                        output_tokens: 2,
                        total_tokens: 18,
                        ..Usage::default()
                    },
                    provider_metadata: serde_json::Value::Null,
                },
                failure,
            };

            let result = execute_compatibility_agent_model_invocation(
                &runtime,
                &gateway,
                &connection,
                &model,
                actor,
                &identity,
                lease.id,
                &target,
                request,
                at,
            )
            .await;
            assert_eq!(
                result.is_ok(),
                failure == ScriptedGatewayFailure::RetryFirst
            );
            let calls = gateway.calls.lock().expect("scripted retry calls");
            assert_eq!(calls.len(), expected_calls);
            let invocation_ids = calls
                .iter()
                .map(|(authorization, _)| authorization.proposal.invocation_id)
                .collect::<Vec<_>>();
            let permit_ids = calls
                .iter()
                .map(|(authorization, _)| authorization.permit.id)
                .collect::<Vec<_>>();
            let client_request_ids = calls
                .iter()
                .map(|(_, request)| {
                    request
                        .client_request_id
                        .clone()
                        .expect("governed client request id")
                })
                .collect::<Vec<_>>();
            assert_eq!(
                invocation_ids.iter().collect::<BTreeSet<_>>().len(),
                expected_calls
            );
            assert_eq!(
                permit_ids.iter().collect::<BTreeSet<_>>().len(),
                expected_calls
            );
            assert_eq!(
                client_request_ids.iter().collect::<BTreeSet<_>>().len(),
                expected_calls
            );

            let mut retry_payloads = Vec::new();
            let mut model_run_ids = Vec::new();
            for (index, invocation_id) in invocation_ids.iter().enumerate() {
                let inspection = runtime
                    .runtime_invocation(*invocation_id, 10)
                    .expect("retry invocation inspection")
                    .expect("retry invocation exists");
                let run = &inspection.model_runs.last().expect("retry model run").run;
                assert_eq!(run.state, expected_states[index]);
                let outcome = run
                    .outcome_evidence
                    .as_ref()
                    .expect("retry outcome evidence");
                match run.state {
                    ModelRunState::Completed => {
                        assert_eq!(outcome.actual_cost_microusd, Some(20));
                        assert!(outcome.error_class.is_none());
                    }
                    ModelRunState::Failed => {
                        assert_eq!(outcome.actual_cost_microusd, Some(0));
                        assert!(matches!(
                            outcome.error_class,
                            Some(
                                ModelRunErrorClass::RetryableBeforeProviderContact
                                    | ModelRunErrorClass::FinalBeforeProviderContact
                            )
                        ));
                    }
                    ModelRunState::OutcomeUnknown => {
                        assert!(outcome.actual_cost_microusd.is_none());
                        assert_eq!(
                            outcome.error_class,
                            Some(ModelRunErrorClass::OutcomeUnknown)
                        );
                    }
                    _ => panic!("unexpected retry terminal state"),
                }
                model_run_ids.push(run.id);
                retry_payloads.push(
                    decode_canonical_cbor_v1(
                        &inspection.context_manifest.selected_sources[0]
                            .influence
                            .bytes,
                    )
                    .expect("retry lineage payload"),
                );
            }
            assert_eq!(
                model_run_ids.iter().collect::<BTreeSet<_>>().len(),
                expected_calls
            );
            if matches!(
                failure,
                ScriptedGatewayFailure::RetryFirst | ScriptedGatewayFailure::AlwaysRetryable
            ) {
                assert_eq!(retry_payloads[0]["physical_attempt"], 1);
                assert_eq!(retry_payloads[1]["physical_attempt"], 2);
                assert_eq!(
                    retry_payloads[0]["logical_request_id"],
                    retry_payloads[1]["logical_request_id"]
                );
                assert_eq!(
                    retry_payloads[0]["logical_request_hash"],
                    retry_payloads[1]["logical_request_hash"]
                );
                assert!(retry_payloads[0]["retry_of_invocation_id"].is_null());
                assert_eq!(
                    retry_payloads[1]["retry_of_invocation_id"],
                    invocation_ids[0].to_string()
                );
            }
            drop(calls);
            drop(runtime);

            let reopened = open(config).expect("reopen compatibility retry Root");
            for (index, invocation_id) in invocation_ids.iter().enumerate() {
                let inspection = reopened
                    .runtime_invocation(*invocation_id, 10)
                    .expect("reopened retry invocation")
                    .expect("persisted retry invocation");
                assert_eq!(
                    inspection
                        .model_runs
                        .last()
                        .expect("persisted retry model run")
                        .run
                        .state,
                    expected_states[index]
                );
            }
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn compatibility_provider_cancellation_stops_waiting_and_fences_late_output() {
        let temporary = tempfile::tempdir().expect("temporary provider cancellation directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let at = Utc::now();
        let runtime = open(config.clone()).expect("open provider cancellation Root");
        let (lease, connection, model) = {
            let mut store = SqliteEventStore::open(config.database_path())
                .expect("provider cancellation store");
            seed_compatibility_agent_provider(&mut store, at)
        };
        let identity = current_runtime_identity().expect("runtime identity");
        let actor = RuntimeActorId(identity.instance_id.0);
        let target = mcg_providers::ProviderModelRef {
            provider_id: connection.id.clone(),
            model_id: model.id.clone(),
        };
        let request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: None,
            instructions: Some("Wait for governed cancellation.".into()),
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Do not outlive cancellation.".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(64),
            metadata: serde_json::json!({"agent_lease_id": lease.id}),
        };
        let gateway = CancellingAgentProviderGateway {
            runtime: &runtime,
            calls: Mutex::new(Vec::new()),
        };

        let result = execute_compatibility_agent_model_invocation(
            &runtime,
            &gateway,
            &connection,
            &model,
            actor,
            &identity,
            lease.id,
            &target,
            request,
            at,
        )
        .await;
        assert!(result.is_err());
        let invocation_ids = gateway
            .calls
            .lock()
            .expect("cancelling gateway calls")
            .clone();
        assert_eq!(invocation_ids.len(), 1, "cancellation must never retry");
        let workflow = runtime
            .canonical_workflows()
            .expect("cancelled provider workflow")
            .pop()
            .expect("provider workflow");
        assert_eq!(workflow.state, WorkflowState::CancelRequested);
        let inspection = runtime
            .runtime_invocation(invocation_ids[0], 10)
            .expect("cancelled provider invocation")
            .expect("cancelled provider invocation exists");
        assert_eq!(
            inspection
                .model_runs
                .last()
                .expect("cancelled provider model run")
                .run
                .state,
            ModelRunState::OutcomeUnknown
        );
        assert!(
            inspection
                .model_runs
                .last()
                .expect("cancelled stream model run")
                .run
                .observed_model_fingerprint
                .is_none(),
            "possible-contact cancellation must not invent observed completion identity"
        );
        let outcome = inspection
            .model_runs
            .last()
            .and_then(|record| record.run.outcome_evidence.as_ref())
            .expect("possible-contact cancellation outcome evidence");
        assert!(outcome.actual_cost_microusd.is_none());
        assert_eq!(
            outcome.error_class,
            Some(ModelRunErrorClass::OutcomeUnknown)
        );
        drop(gateway);
        drop(runtime);

        let reopened = open(config).expect("reopen provider cancellation Root");
        let inspection = reopened
            .runtime_invocation(invocation_ids[0], 10)
            .expect("reopened cancelled provider invocation")
            .expect("persisted cancelled provider invocation");
        assert_eq!(
            inspection
                .model_runs
                .last()
                .expect("persisted cancelled provider model run")
                .run
                .state,
            ModelRunState::OutcomeUnknown
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn compatibility_provider_stream_is_incremental_and_commits_only_at_completion()
    -> Result<(), ProtectedRootError> {
        let temporary = tempfile::tempdir().expect("temporary provider stream directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let at = Utc::now();
        let runtime = open(config.clone()).expect("open provider stream Root");
        let (lease, connection, model) = {
            let mut store =
                SqliteEventStore::open(config.database_path()).expect("provider stream store");
            seed_compatibility_agent_provider(&mut store, at)
        };
        let identity = current_runtime_identity().expect("runtime identity");
        let actor = RuntimeActorId(identity.instance_id.0);
        let target = mcg_providers::ProviderModelRef {
            provider_id: connection.id.clone(),
            model_id: model.id.clone(),
        };
        let request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: None,
            instructions: Some("Stream under exact Root authority.".into()),
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Return an incremental response.".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(64),
            metadata: serde_json::json!({"agent_lease_id": lease.id}),
        };
        let release_completion = Arc::new(tokio::sync::Notify::new());
        let gateway = ControlledStreamingGateway {
            calls: Mutex::new(Vec::new()),
            response: ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "governed-stream-response".into(),
                model: model.id.clone(),
                status: Some("completed".into()),
                output: vec![OutputItem::Message {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: "incremental".into(),
                    }],
                }],
                usage: Usage {
                    input_tokens: 16,
                    output_tokens: 1,
                    total_tokens: 17,
                    ..Usage::default()
                },
                provider_metadata: serde_json::Value::Null,
            },
            release_completion: release_completion.clone(),
            retry_first: true,
        };
        let mut stream = execute_compatibility_agent_model_stream(
            runtime.clone(),
            &gateway,
            &connection,
            &model,
            actor,
            &identity,
            lease.id,
            &target,
            request,
            at,
        )
        .await
        .expect("governed provider stream");
        let (authorization, dispatched_request, first_invocation_id) = {
            let calls = gateway.calls.lock().expect("streaming gateway calls");
            assert_eq!(calls.len(), 2, "one proved pre-contact stream retry");
            let (authorization, dispatched_request) = calls[1].clone();
            (
                authorization,
                dispatched_request,
                calls[0].0.proposal.invocation_id,
            )
        };
        let expected_fingerprint = observed_model_fingerprint(&authorization, &gateway.response)?;
        assert_eq!(
            authorization.proposal.request_hash,
            model_provider_stream_request_artifact(&dispatched_request)
                .expect("stream request artifact")
                .content_hash
        );
        assert_ne!(
            authorization.proposal.request_hash,
            model_provider_request_artifact(&dispatched_request)
                .expect("buffered request artifact")
                .content_hash
        );
        let first_attempt = runtime
            .runtime_invocation(first_invocation_id, 10)?
            .expect("first stream attempt");
        assert_eq!(
            first_attempt
                .model_runs
                .last()
                .expect("failed first stream model run")
                .run
                .state,
            ModelRunState::Failed
        );
        let first_lineage: serde_json::Value = decode_canonical_cbor_v1(
            &first_attempt.context_manifest.selected_sources[0]
                .influence
                .bytes,
        )?;
        let second_before_completion = runtime
            .runtime_invocation(authorization.proposal.invocation_id, 10)?
            .expect("second stream attempt");
        let second_lineage: serde_json::Value = decode_canonical_cbor_v1(
            &second_before_completion.context_manifest.selected_sources[0]
                .influence
                .bytes,
        )?;
        assert_eq!(
            first_lineage["logical_request_id"],
            second_lineage["logical_request_id"]
        );
        assert_eq!(
            first_lineage["logical_request_hash"],
            second_lineage["logical_request_hash"]
        );
        assert_eq!(
            second_lineage["retry_of_invocation_id"],
            first_invocation_id.to_string()
        );
        assert!(matches!(
            stream.next().await.expect("response-created stream item")?,
            ModelStreamEvent::ResponseCreated { ref id, .. } if id == "governed-stream-response"
        ));
        assert!(matches!(
            stream.next().await.expect("delta stream item")?,
            ModelStreamEvent::OutputTextDelta { ref delta, .. } if delta == "incremental"
        ));
        let before_completion = runtime
            .runtime_invocation(authorization.proposal.invocation_id, 10)
            .expect("stream invocation inspection")
            .expect("stream invocation exists");
        assert_eq!(
            before_completion
                .model_runs
                .last()
                .expect("dispatched stream model run")
                .run
                .state,
            ModelRunState::Dispatched
        );
        release_completion.notify_one();
        assert!(matches!(
            stream.next().await.expect("completed stream item")?,
            ModelStreamEvent::Completed { ref response }
                if response.id == "governed-stream-response"
        ));
        assert!(stream.next().await.is_none());
        let invocation_id = authorization.proposal.invocation_id;
        let completed = runtime
            .runtime_invocation(invocation_id, 10)
            .expect("completed stream invocation")
            .expect("completed stream invocation exists");
        assert_eq!(
            completed
                .model_runs
                .last()
                .expect("completed stream model run")
                .run
                .state,
            ModelRunState::Completed
        );
        assert_eq!(
            completed
                .model_runs
                .last()
                .and_then(|record| record.run.observed_model_fingerprint.as_deref()),
            Some(expected_fingerprint.as_str())
        );
        assert_eq!(
            completed
                .model_runs
                .last()
                .and_then(|record| record.run.outcome_evidence.as_ref())
                .and_then(|evidence| evidence.actual_cost_microusd),
            Some(18)
        );
        drop(stream);
        drop(gateway);
        drop(runtime);

        let reopened = open(config).expect("reopen completed stream Root");
        let completed = reopened
            .runtime_invocation(invocation_id, 10)
            .expect("reopened stream invocation")
            .expect("persisted stream invocation");
        assert_eq!(
            completed
                .model_runs
                .last()
                .expect("persisted completed stream model run")
                .run
                .state,
            ModelRunState::Completed
        );
        assert_eq!(
            completed
                .model_runs
                .last()
                .and_then(|record| record.run.observed_model_fingerprint.as_deref()),
            Some(expected_fingerprint.as_str())
        );
        assert_eq!(
            completed
                .model_runs
                .last()
                .and_then(|record| record.run.outcome_evidence.as_ref())
                .and_then(|evidence| evidence.actual_cost_microusd),
            Some(18)
        );
        Ok::<(), ProtectedRootError>(())
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn compatibility_provider_stream_cancellation_is_unknown_and_never_retried()
    -> Result<(), ProtectedRootError> {
        let temporary = tempfile::tempdir().expect("temporary stream cancellation directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let at = Utc::now();
        let runtime = open(config.clone()).expect("open stream cancellation Root");
        let (lease, connection, model) = {
            let mut store =
                SqliteEventStore::open(config.database_path()).expect("stream cancellation store");
            seed_compatibility_agent_provider(&mut store, at)
        };
        let identity = current_runtime_identity().expect("runtime identity");
        let actor = RuntimeActorId(identity.instance_id.0);
        let target = mcg_providers::ProviderModelRef {
            provider_id: connection.id.clone(),
            model_id: model.id.clone(),
        };
        let request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: None,
            instructions: Some("Cancel the governed stream after one delta.".into()),
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Do not commit late stream output.".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(64),
            metadata: serde_json::json!({"agent_lease_id": lease.id}),
        };
        let release_completion = Arc::new(tokio::sync::Notify::new());
        let gateway = ControlledStreamingGateway {
            calls: Mutex::new(Vec::new()),
            response: ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "cancelled-stream-response".into(),
                model: model.id.clone(),
                status: Some("completed".into()),
                output: vec![OutputItem::Message {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: "must not commit".into(),
                    }],
                }],
                usage: Usage {
                    input_tokens: 16,
                    output_tokens: 3,
                    total_tokens: 19,
                    ..Usage::default()
                },
                provider_metadata: serde_json::Value::Null,
            },
            release_completion: release_completion.clone(),
            retry_first: false,
        };
        let mut stream = execute_compatibility_agent_model_stream(
            runtime.clone(),
            &gateway,
            &connection,
            &model,
            actor,
            &identity,
            lease.id,
            &target,
            request,
            at,
        )
        .await?;
        assert!(matches!(
            stream.next().await.expect("created stream event")?,
            ModelStreamEvent::ResponseCreated { .. }
        ));
        assert!(matches!(
            stream.next().await.expect("delta stream event")?,
            ModelStreamEvent::OutputTextDelta { .. }
        ));
        let authorization = gateway
            .calls
            .lock()
            .expect("stream cancellation calls")
            .first()
            .expect("stream cancellation call")
            .0
            .clone();
        runtime.cancel_canonical_workflow(&WorkflowCancelRequest {
            attempt_id: authorization.proposal.attempt_id,
            expected_revision: authorization.proposal.attempt_revision,
            expected_generation: authorization.proposal.lease_generation,
            idempotency_key: format!(
                "cancel-provider-stream-{}",
                authorization.proposal.invocation_id
            ),
        })?;
        let cancelled = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("stream cancellation deadline")
            .expect("stream cancellation item");
        assert!(cancelled.is_err());
        assert!(stream.next().await.is_none());
        assert_eq!(
            gateway
                .calls
                .lock()
                .expect("stream cancellation calls")
                .len(),
            1,
            "a possible-contact stream must never retry"
        );
        let invocation_id = authorization.proposal.invocation_id;
        let inspection = runtime
            .runtime_invocation(invocation_id, 10)?
            .expect("cancelled stream invocation");
        assert_eq!(
            inspection
                .model_runs
                .last()
                .expect("cancelled stream model run")
                .run
                .state,
            ModelRunState::OutcomeUnknown
        );
        assert_eq!(
            runtime
                .canonical_workflows()?
                .pop()
                .expect("cancelled stream workflow")
                .state,
            WorkflowState::CancelRequested
        );
        release_completion.notify_one();
        drop(stream);
        drop(gateway);
        drop(runtime);

        let reopened = open(config).expect("reopen cancelled stream Root");
        let inspection = reopened
            .runtime_invocation(invocation_id, 10)?
            .expect("persisted cancelled stream invocation");
        assert_eq!(
            inspection
                .model_runs
                .last()
                .expect("persisted cancelled stream model run")
                .run
                .state,
            ModelRunState::OutcomeUnknown
        );
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn root_model_egress_permit_is_route_disclosure_budget_and_secret_handle_bound() {
        let temporary = tempfile::tempdir().expect("temporary Root egress directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let at = Utc::now();
        let (contract, initial_attempt) = expired_workflow_fixture(at);
        let runtime_actor = RuntimeActorId::new();
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: workflow_hash("governed-provider-runtime-build"),
            protocol_version: PROTOCOL_V1,
            started_at: at,
        };
        let connection = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "root-egress-fixture".into(),
            display_name: "Root Egress Fixture".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://provider.example/v1".into(),
            auth_mode: ProviderAuthMode::ApiKey,
            organization: None,
            project: None,
        };
        let model = ModelDescriptor {
            id: "fixture-model".into(),
            display_name: "Fixture Model".into(),
            owned_by: Some("fixture".into()),
            context_window: Some(16_384),
            capabilities: ModelCapabilities {
                responses: Some(true),
                ..ModelCapabilities::default()
            },
            price: Some(mcg_providers::ModelPrice {
                input_per_million: Some(1.0),
                cached_input_per_million: Some(1.0),
                output_per_million: Some(2.0),
                currency: "USD".into(),
                provenance: mcg_providers::PriceProvenance::McgCatalog {
                    catalog_date: "2026-09-01".into(),
                },
            }),
            metadata: serde_json::Value::Null,
        };
        {
            let mut store = SqliteEventStore::open(config.database_path()).expect("egress store");
            store
                .create_workflow(&contract, &initial_attempt, CorrelationId::new(), at)
                .expect("create egress workflow");
            store
                .record_provider_configuration(
                    &connection,
                    &ProviderModelCatalog {
                        connection_id: connection.id.clone(),
                        models: vec![model],
                        discovered_at: at,
                    },
                    at,
                )
                .expect("record egress provider");
            store
                .claim_workflow_attempt(
                    initial_attempt.attempt_id,
                    initial_attempt.revision,
                    initial_attempt.lease_generation,
                    runtime_actor,
                    at + chrono::Duration::minutes(10),
                    CorrelationId::new(),
                    "root-egress-claim-1",
                    at,
                )
                .expect("claim egress workflow");
        }
        let attempt = {
            let store = SqliteEventStore::open(config.database_path()).expect("read egress store");
            store
                .workflow_attempt(initial_attempt.attempt_id)
                .expect("read egress workflow")
                .expect("egress workflow exists")
        };
        let runtime = open(config).expect("open Root egress runtime");
        let context = ContextManifest {
            id: ContextManifestId::new(),
            attempt_id: attempt.attempt_id,
            selected_sources: vec![ContextSelection {
                source: root_invocation_source("private-project-context"),
                perspective: None,
                influence: root_invocation_payload("test.root_egress_influence.v1", "selected"),
                transformations: Vec::new(),
                placement: 0,
                approximate_tokens: 7,
            }],
            selection_policy: root_invocation_payload("test.root_egress_selection.v1", "bounded"),
            expansion_steps: Vec::new(),
            approximate_tokens: 7,
        };
        let context_hash = canonical_contract_hash(
            ContextManifest::TYPE_ID,
            "MetaCog.ContextManifest",
            &context,
        )
        .expect("context hash");
        let route = ModelEgressRoute {
            provider_id: connection.id.clone(),
            model_id: "fixture-model".into(),
            destination: connection.base_url.clone(),
            destination_class: ModelDestinationClass::ThirdPartyRemote,
        };
        let request = ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: "fixture-model".into(),
            client_request_id: Some("governed-provider-request-1".into()),
            instructions: Some("Use only the disclosed project context.".into()),
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Return the deterministic fixture result.".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: Some(0.0),
            top_p: None,
            max_output_tokens: Some(64),
            metadata: serde_json::Value::Null,
        };
        let request_artifact =
            model_provider_request_artifact(&request).expect("canonical provider request artifact");
        let proposal = ModelInvocationProposal {
            invocation_id: InvocationId::new(),
            attempt_id: attempt.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            runtime_actor,
            runtime_identity: runtime_identity.clone(),
            role: InvocationRole::Work,
            task_contract: attempt.task_contract.clone(),
            request_artifact: request_artifact.clone(),
            request_hash: request_artifact.content_hash,
            route: route.clone(),
            data_classification: ModelDataClassification::Confidential,
            disclosure: ModelDisclosureManifest {
                context_manifest: context_hash,
                disclosed_sources: vec![root_invocation_source("private-project-context")],
                approximate_input_tokens: 7,
            },
            requested_limits: ModelBudget {
                max_input_tokens: 128,
                max_output_tokens: 64,
                max_cost_microusd: 25_000,
            },
            deadline: at + chrono::Duration::minutes(5),
        };
        let policy = ModelEgressPolicy {
            allowed_routes: vec![route],
            minimum_data_classification: ModelDataClassification::Internal,
            maximum_data_classification: ModelDataClassification::Confidential,
            maximum_limits: ModelBudget {
                max_input_tokens: 256,
                max_output_tokens: 128,
                max_cost_microusd: 50_000,
            },
            maximum_permit_lifetime_seconds: 600,
        };
        let receipt = authorize_runtime_model_egress(
            &runtime,
            &context,
            &proposal,
            &runtime_identity,
            &policy,
            at,
        )
        .expect("Root egress authorization");
        assert_eq!(receipt.permit.invocation_id, proposal.invocation_id);
        assert_eq!(receipt.permit.request_hash, proposal.request_hash);
        assert_eq!(receipt.permit.secret_handles.len(), 1);
        assert_eq!(receipt.secret_handles.len(), 1);
        assert_eq!(
            runtime
                .resolve_model_secret_handle_binding(
                    receipt.permit.id,
                    receipt.permit.secret_handles[0],
                    at,
                )
                .expect("resolve opaque binding"),
            receipt.secret_handles[0]
        );
        assert_eq!(
            authorize_runtime_model_egress(
                &runtime,
                &context,
                &proposal,
                &runtime_identity,
                &policy,
                at,
            )
            .expect("exact egress replay"),
            receipt
        );
        let encoded = serde_json::to_string(&receipt).expect("serialize egress receipt");
        assert!(!encoded.contains("secret-value"));

        let runtime_build = runtime_identity.build_hash.clone();
        let manifest = InvocationManifest {
            id: proposal.invocation_id,
            role: proposal.role,
            attempt_id: proposal.attempt_id,
            task_contract: proposal.task_contract.clone(),
            runtime_identity: runtime_identity.clone(),
            cognitive_configuration: workflow_hash("governed-provider-cognitive-config"),
            role_manifest: workflow_hash("governed-provider-work-role"),
            prompt_bundle: workflow_hash("governed-provider-prompt-bundle"),
            context_manifest: proposal.disclosure.context_manifest.clone(),
            model_route: mcg_storage::model_egress_route_payload(&proposal.route)
                .expect("governed provider route payload"),
            resolved_provider: receipt.permit.resolved_provider.clone(),
            resolved_model: receipt.permit.resolved_model.clone(),
            observed_model_fingerprint: Some("fixture-model-v1".into()),
            sampling: root_invocation_payload("test.root_sampling.v1", "zero"),
            reasoning: None,
            available_tool_manifests: Vec::new(),
            selected_skill_manifests: Vec::new(),
            authority_envelope: root_invocation_source("root-provider-authority"),
            egress_permit: Some(receipt.permit_ref.clone()),
            repository_revision: Some(workflow_hash("governed-provider-revision")),
            environment: root_invocation_source("governed-provider-environment"),
            runtime_build,
            request_hash: proposal.request_hash.clone(),
            response_hash: None,
        };
        let gateway = DeterministicModelGateway {
            calls: Mutex::new(0),
            response: ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "fixture-response-1".into(),
                model: "fixture-model".into(),
                status: Some("completed".into()),
                output: vec![OutputItem::Message {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: "private deterministic result".into(),
                    }],
                }],
                usage: Usage {
                    input_tokens: 7,
                    cached_input_tokens: 0,
                    output_tokens: 4,
                    reasoning_tokens: 0,
                    total_tokens: 11,
                },
                provider_metadata: serde_json::json!({
                    "fixture": true,
                    "system_fingerprint": "fixture-provider-revision-sensitive"
                }),
            },
        };
        let expected_fingerprint = observed_model_fingerprint(&receipt, &gateway.response)
            .expect("observed provider fingerprint");
        assert!(!expected_fingerprint.contains("fixture-provider-revision-sensitive"));
        let mut changed_response = gateway.response.clone();
        changed_response.provider_metadata["system_fingerprint"] =
            serde_json::json!("different-provider-revision");
        assert_ne!(
            observed_model_fingerprint(&receipt, &changed_response)
                .expect("changed observed provider fingerprint"),
            expected_fingerprint
        );
        let mut content_changed_response = gateway.response.clone();
        content_changed_response.id = "different-response-id".into();
        content_changed_response.output.clear();
        content_changed_response.provider_metadata["fixture"] = serde_json::json!(false);
        assert_eq!(
            observed_model_fingerprint(&receipt, &content_changed_response)
                .expect("content-minimized observed provider fingerprint"),
            expected_fingerprint,
            "response content and unapproved metadata must not enter identity evidence"
        );
        let model_run_id = ModelRunId::new();
        let mut substituted_request = request.clone();
        substituted_request.model = "substituted-model".into();
        assert!(
            execute_authorized_model_invocation(
                &runtime,
                &gateway,
                &manifest,
                model_run_id,
                &substituted_request,
                at,
            )
            .await
            .is_err()
        );
        assert_eq!(*gateway.calls.lock().expect("model gateway calls"), 0);

        let dispatch = execute_authorized_model_invocation(
            &runtime,
            &gateway,
            &manifest,
            model_run_id,
            &request,
            at,
        )
        .await
        .expect("governed deterministic provider dispatch");
        assert_eq!(*gateway.calls.lock().expect("model gateway calls"), 1);
        assert_eq!(dispatch.response.id, "fixture-response-1");
        assert_eq!(dispatch.completed_run.run.state, ModelRunState::Completed);
        assert_eq!(
            dispatch
                .completed_run
                .run
                .usage
                .as_ref()
                .expect("provider usage")
                .output_tokens,
            4
        );
        assert!(dispatch.completed_run.run.response_artifact.is_some());
        assert_eq!(
            dispatch
                .completed_run
                .run
                .observed_model_fingerprint
                .as_deref(),
            Some(expected_fingerprint.as_str())
        );
        assert!(
            runtime
                .runtime_response(model_run_id)
                .expect("provider response lookup")
                .is_none(),
            "generic durable state must not retain raw provider response content"
        );

        let mut over_budget_proposal = proposal.clone();
        over_budget_proposal.invocation_id = InvocationId::new();
        over_budget_proposal.requested_limits.max_cost_microusd = 1;
        let over_budget_receipt = authorize_runtime_model_egress(
            &runtime,
            &context,
            &over_budget_proposal,
            &runtime_identity,
            &policy,
            at,
        )
        .expect("authorize exact cost-budget fixture");
        let mut over_budget_manifest = manifest.clone();
        over_budget_manifest.id = over_budget_proposal.invocation_id;
        over_budget_manifest.egress_permit = Some(over_budget_receipt.permit_ref);
        over_budget_manifest.resolved_provider = over_budget_receipt.permit.resolved_provider;
        over_budget_manifest.resolved_model = over_budget_receipt.permit.resolved_model;
        let over_budget_run_id = ModelRunId::new();
        assert!(
            execute_authorized_model_invocation(
                &runtime,
                &gateway,
                &over_budget_manifest,
                over_budget_run_id,
                &request,
                at,
            )
            .await
            .is_err(),
            "known provider cost above the exact permit ceiling must fail closed"
        );
        let over_budget_run = runtime
            .governed_provider_model_run(over_budget_proposal.invocation_id, over_budget_run_id)
            .expect("cost-budget model run lookup")
            .expect("cost-budget model run");
        assert_eq!(over_budget_run.run.state, ModelRunState::OutcomeUnknown);
        assert!(
            over_budget_run
                .run
                .outcome_evidence
                .as_ref()
                .is_some_and(|evidence| {
                    evidence.actual_cost_microusd.is_none()
                        && evidence.error_class == Some(ModelRunErrorClass::OutcomeUnknown)
                })
        );

        let mut stale_proposal = proposal.clone();
        stale_proposal.invocation_id = InvocationId::new();
        let stale_receipt = authorize_runtime_model_egress(
            &runtime,
            &context,
            &stale_proposal,
            &runtime_identity,
            &policy,
            at,
        )
        .expect("authorize stale-result fixture invocation");
        let mut stale_manifest = manifest.clone();
        stale_manifest.id = stale_proposal.invocation_id;
        stale_manifest.egress_permit = Some(stale_receipt.permit_ref);
        stale_manifest.resolved_provider = stale_receipt.permit.resolved_provider;
        stale_manifest.resolved_model = stale_receipt.permit.resolved_model;
        let stale_result_run_id = ModelRunId::new();
        let lease_advancing_gateway = LeaseAdvancingModelGateway {
            runtime: &runtime,
            attempt_id: attempt.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            response: gateway.response.clone(),
            at: at + chrono::Duration::seconds(1),
        };
        let stale_result = execute_authorized_model_invocation(
            &runtime,
            &lease_advancing_gateway,
            &stale_manifest,
            stale_result_run_id,
            &request,
            at,
        )
        .await;
        assert!(
            stale_result.is_err(),
            "a result arriving after the workflow revision advances must be rejected"
        );
        let stale_run = runtime
            .governed_provider_model_run(stale_proposal.invocation_id, stale_result_run_id)
            .unwrap_or_else(|error| {
                panic!("stale provider run lookup after {stale_result:?}: {error}")
            })
            .unwrap_or_else(|| panic!("stale provider model run missing after {stale_result:?}"));
        assert_eq!(stale_run.run.state, ModelRunState::OutcomeUnknown);

        let mut substituted_route = proposal.clone();
        substituted_route.route.destination = "https://other.example/v1".into();
        assert!(
            authorize_runtime_model_egress(
                &runtime,
                &context,
                &substituted_route,
                &runtime_identity,
                &policy,
                at,
            )
            .is_err()
        );
        let mut substituted_identity = runtime_identity.clone();
        substituted_identity.instance_id = RuntimeInstanceId::new();
        assert!(
            authorize_runtime_model_egress(
                &runtime,
                &context,
                &proposal,
                &substituted_identity,
                &policy,
                at,
            )
            .is_err()
        );
        let mut under_disclosed = proposal;
        under_disclosed.invocation_id = InvocationId::new();
        under_disclosed.disclosure.disclosed_sources.clear();
        assert!(
            authorize_runtime_model_egress(
                &runtime,
                &context,
                &under_disclosed,
                &runtime_identity,
                &policy,
                at,
            )
            .is_err()
        );
    }

    #[test]
    fn root_tool_capability_kernel_intersects_manifest_policy_and_host_guarantees() {
        use mcg_contracts::{
            ExecutionPlatform, FilesystemAccess, FilesystemCapability, IsolationGuarantee,
            NetworkCapability, SandboxCapabilityProfile, ToolCapabilitySet, ToolInvocationProposal,
            ToolManifest, ToolManifestId, ToolPolicy, ToolResourceLimits,
        };

        let at = Utc::now();
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: workflow_hash("tool-kernel-runtime"),
            protocol_version: PROTOCOL_V1,
            started_at: at - chrono::Duration::seconds(1),
        };
        let required_capabilities = ToolCapabilitySet {
            filesystem: vec![FilesystemCapability {
                root: "workspace".into(),
                access: FilesystemAccess::Read,
            }],
            network: vec![NetworkCapability {
                destination: "https://tool.example.test".into(),
            }],
            secret_slots: vec![
                StableTypeId::parse("mcg.tool_secret.fixture.v1").expect("secret slot"),
            ],
            process_spawn: false,
        };
        let required_sandbox = SandboxCapabilityProfile {
            filesystem_scope: IsolationGuarantee::Hard,
            network: IsolationGuarantee::Hard,
            process_tree: IsolationGuarantee::BestEffort,
            secrets: IsolationGuarantee::Hard,
            environment: IsolationGuarantee::Hard,
            resources: IsolationGuarantee::BestEffort,
            protected_paths: IsolationGuarantee::Hard,
        };
        let limits = ToolResourceLimits {
            timeout_ms: 30_000,
            max_output_bytes: 1_048_576,
            max_memory_bytes: 256 * 1_048_576,
            max_child_processes: 1,
        };
        let manifest = ToolManifest {
            id: ToolManifestId::new(),
            version: 1,
            name: "fixture.read".into(),
            summary: "Read one governed resource".into(),
            runner: root_invocation_source("tool-runner"),
            input_schema: root_invocation_source("tool-input-schema"),
            output_schema: root_invocation_source("tool-output-schema"),
            maximum_effect_class: EffectClass::Reversible,
            required_capabilities: required_capabilities.clone(),
            resource_ceiling: limits,
            sandbox_requirement: required_sandbox,
            platform_support: vec![ExecutionPlatform::Windows, ExecutionPlatform::Linux],
            receipt_semantics: StableTypeId::parse("mcg.tool_receipt.fixture.v1")
                .expect("receipt type"),
            reconciler: Some(root_invocation_source("tool-reconciler")),
        };
        let argument_hash = workflow_hash("tool-arguments");
        let proposal = ToolInvocationProposal {
            action_id: ActionId::new(),
            attempt_id: AttemptId::new(),
            attempt_revision: 4,
            lease_generation: 2,
            runtime_actor: RuntimeActorId::new(),
            runtime_identity: runtime_identity.clone(),
            task_contract: VersionedRef {
                id: TaskContractId::new(),
                version: 1,
                content_hash: workflow_hash("tool-task-contract"),
            },
            tool_manifest: manifest.versioned_ref().expect("manifest ref"),
            arguments: ArtifactRef {
                content_hash: argument_hash.clone(),
                media_type: "application/json".into(),
                byte_length: 2,
            },
            argument_hash,
            requested_effect_class: EffectClass::Observational,
            requested_capabilities: required_capabilities.clone(),
            requested_limits: limits,
            required_sandbox,
            platform: ExecutionPlatform::Windows,
            deadline: at + chrono::Duration::minutes(2),
        };
        let policy = ToolPolicy {
            allowed_manifests: vec![manifest.versioned_ref().expect("allowed manifest")],
            maximum_effect_class: EffectClass::Consequential,
            maximum_capabilities: ToolCapabilitySet {
                filesystem: vec![FilesystemCapability {
                    root: "workspace".into(),
                    access: FilesystemAccess::Write,
                }],
                network: required_capabilities.network.clone(),
                secret_slots: required_capabilities.secret_slots.clone(),
                process_spawn: false,
            },
            maximum_limits: ToolResourceLimits {
                timeout_ms: 60_000,
                max_output_bytes: 2 * 1_048_576,
                max_memory_bytes: 512 * 1_048_576,
                max_child_processes: 2,
            },
            maximum_permit_lifetime_seconds: 300,
        };
        let available = SandboxCapabilityProfile {
            filesystem_scope: IsolationGuarantee::Hard,
            network: IsolationGuarantee::Hard,
            process_tree: IsolationGuarantee::Hard,
            secrets: IsolationGuarantee::Hard,
            environment: IsolationGuarantee::Hard,
            resources: IsolationGuarantee::Hard,
            protected_paths: IsolationGuarantee::Hard,
        };
        let permit = authorize_runtime_tool_capability(
            &proposal,
            &runtime_identity,
            &manifest,
            &policy,
            available,
            ExecutionPlatform::Windows,
            at,
        )
        .expect("Root tool permit");
        assert_eq!(permit.action_id, proposal.action_id);
        assert_eq!(permit.tool_manifest, proposal.tool_manifest);
        assert_eq!(permit.capabilities, required_capabilities);
        assert_eq!(permit.secret_handles.len(), 1);
        assert_eq!(permit.sandbox, available);

        let mut weak_host = available;
        weak_host.network = IsolationGuarantee::BestEffort;
        assert!(
            authorize_runtime_tool_capability(
                &proposal,
                &runtime_identity,
                &manifest,
                &policy,
                weak_host,
                ExecutionPlatform::Windows,
                at,
            )
            .is_err(),
            "a host below any required sandbox dimension must fail closed"
        );

        let mut substituted = proposal.clone();
        substituted.tool_manifest.content_hash = workflow_hash("substituted-tool-manifest");
        assert!(
            authorize_runtime_tool_capability(
                &substituted,
                &runtime_identity,
                &manifest,
                &policy,
                available,
                ExecutionPlatform::Windows,
                at,
            )
            .is_err()
        );
        assert!(
            authorize_runtime_tool_capability(
                &proposal,
                &runtime_identity,
                &manifest,
                &policy,
                available,
                ExecutionPlatform::Macos,
                at,
            )
            .is_err()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn root_tool_capability_authority_persists_and_replays_through_runtime() {
        use mcg_contracts::{
            ExecutionPlatform, FilesystemAccess, FilesystemCapability, IsolationGuarantee,
            SandboxCapabilityProfile, ToolCapabilitySet, ToolInvocationProposal, ToolManifest,
            ToolManifestId, ToolPolicy, ToolResourceLimits,
        };

        let (temporary, config, runtime, execution, actor, at) = authorized_filesystem_fixture();
        let (action, _) = runtime
            .canonical_action_for_execution(execution.id)
            .expect("load canonical Tool action")
            .expect("canonical Tool action exists");
        let attempt = runtime
            .canonical_workflows()
            .expect("load canonical workflows")
            .into_iter()
            .find(|attempt| attempt.attempt_id == action.attempt_id)
            .expect("Tool workflow attempt");
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: workflow_hash("persisted-tool-runtime"),
            protocol_version: PROTOCOL_V1,
            started_at: at - chrono::Duration::seconds(1),
        };
        let capabilities = ToolCapabilitySet {
            filesystem: vec![FilesystemCapability {
                root: "capability-root".into(),
                access: FilesystemAccess::Write,
            }],
            network: Vec::new(),
            secret_slots: vec![
                StableTypeId::parse("mcg.tool_secret.persisted_fixture.v1").expect("secret slot"),
            ],
            process_spawn: false,
        };
        let limits = ToolResourceLimits {
            timeout_ms: 30_000,
            max_output_bytes: 1_048_576,
            max_memory_bytes: 256 * 1_048_576,
            max_child_processes: 1,
        };
        let required_sandbox = SandboxCapabilityProfile {
            filesystem_scope: IsolationGuarantee::Hard,
            network: IsolationGuarantee::BestEffort,
            process_tree: IsolationGuarantee::Hard,
            secrets: IsolationGuarantee::Hard,
            environment: IsolationGuarantee::Hard,
            resources: IsolationGuarantee::BestEffort,
            protected_paths: IsolationGuarantee::Hard,
        };
        let manifest = ToolManifest {
            id: ToolManifestId::new(),
            version: 1,
            name: "fixture.persisted-write".into(),
            summary: "Persist one exact governed Tool authorization".into(),
            runner: root_invocation_source("persisted-tool-runner"),
            input_schema: root_invocation_source("persisted-tool-input"),
            output_schema: root_invocation_source("persisted-tool-output"),
            maximum_effect_class: EffectClass::Reversible,
            required_capabilities: capabilities.clone(),
            resource_ceiling: limits,
            sandbox_requirement: required_sandbox,
            platform_support: vec![ExecutionPlatform::Windows],
            receipt_semantics: StableTypeId::parse("mcg.tool_receipt.persisted_fixture.v1")
                .expect("receipt type"),
            reconciler: Some(root_invocation_source("persisted-tool-reconciler")),
        };
        runtime
            .register_tool_manifest(&manifest, at)
            .expect("register exact Tool manifest");
        let proposal = ToolInvocationProposal {
            action_id: action.id,
            attempt_id: action.attempt_id,
            attempt_revision: attempt.revision,
            lease_generation: attempt.lease_generation,
            runtime_actor: actor,
            runtime_identity: runtime_identity.clone(),
            task_contract: action.task_contract.clone(),
            tool_manifest: manifest.versioned_ref().expect("Tool reference"),
            arguments: ArtifactRef {
                content_hash: action.operation_hash.clone(),
                media_type: "application/cbor".into(),
                byte_length: u64::try_from(action.operation.bytes.len()).expect("argument size"),
            },
            argument_hash: action.operation_hash.clone(),
            requested_effect_class: action.effect_class,
            requested_capabilities: capabilities.clone(),
            requested_limits: limits,
            required_sandbox,
            platform: ExecutionPlatform::Windows,
            deadline: at + chrono::Duration::minutes(2),
        };
        let policy = ToolPolicy {
            allowed_manifests: vec![manifest.versioned_ref().expect("allowed Tool")],
            maximum_effect_class: EffectClass::Consequential,
            maximum_capabilities: capabilities,
            maximum_limits: limits,
            maximum_permit_lifetime_seconds: 300,
        };
        let available = SandboxCapabilityProfile {
            filesystem_scope: IsolationGuarantee::Hard,
            network: IsolationGuarantee::Hard,
            process_tree: IsolationGuarantee::Hard,
            secrets: IsolationGuarantee::Hard,
            environment: IsolationGuarantee::Hard,
            resources: IsolationGuarantee::Hard,
            protected_paths: IsolationGuarantee::Hard,
        };
        let receipt = authorize_and_persist_runtime_tool_capability(
            &runtime,
            &proposal,
            &runtime_identity,
            &manifest,
            &policy,
            available,
            ExecutionPlatform::Windows,
            at,
        )
        .expect("persist Root Tool authorization");
        assert_eq!(receipt.proposal, proposal);
        assert_eq!(receipt.permit.platform, ExecutionPlatform::Windows);
        assert_eq!(receipt.secret_handles.len(), 1);
        assert_eq!(
            authorize_and_persist_runtime_tool_capability(
                &runtime,
                &proposal,
                &runtime_identity,
                &manifest,
                &policy,
                available,
                ExecutionPlatform::Windows,
                at,
            )
            .expect("exact Root Tool replay"),
            receipt
        );
        assert_eq!(
            runtime
                .resolve_tool_secret_handle_binding(
                    receipt.permit.id,
                    receipt.permit.secret_handles[0],
                    at,
                )
                .expect("resolve exact opaque Tool handle"),
            receipt.secret_handles[0]
        );

        drop(runtime);
        let reopened = open(config).expect("reopen Root with durable Tool authority");
        assert_eq!(
            reopened
                .active_tool_capability_authorization(action.id, at)
                .expect("reload active Tool authority"),
            Some(receipt)
        );
        drop(reopened);
        drop(temporary);
    }

    #[test]
    fn protected_plugin_session_rejects_substituted_and_replayed_capability_evidence() {
        let invocation_id = PluginInvocationId::new();
        let sessions = Mutex::new(BTreeMap::new());
        let initial = PluginHostTurn {
            schema_version: PLUGIN_SCHEMA_VERSION,
            original_input: serde_json::json!({"exact": "input"}),
            outcomes: Vec::new(),
        };
        assert_eq!(
            validate_protected_plugin_host_turn(
                &sessions,
                invocation_id,
                &serde_json::to_value(&initial).expect("initial host turn"),
            )
            .expect("track initial host turn"),
            initial.original_input
        );
        let exact_outcome = PluginCapabilityOutcome::Succeeded {
            request_id: "read-proof".into(),
            outcome: serde_json::json!({"stdout": "protected"}),
        };
        {
            let mut tracked = sessions.lock().expect("tracked host turns");
            tracked
                .get_mut(&invocation_id)
                .expect("tracked invocation")
                .outcomes
                .push(exact_outcome);
        }
        let exact = sessions
            .lock()
            .expect("tracked host turns")
            .get(&invocation_id)
            .expect("tracked invocation")
            .clone();
        assert!(!plugin_capability_request_is_fresh(&exact, "read-proof"));
        assert!(plugin_capability_request_is_fresh(&exact, "read-other"));
        validate_protected_plugin_host_turn(
            &sessions,
            invocation_id,
            &serde_json::to_value(&exact).expect("exact continuation"),
        )
        .expect("exact Root continuation");

        let mut substituted = exact;
        substituted.outcomes[0] = PluginCapabilityOutcome::Succeeded {
            request_id: "read-proof".into(),
            outcome: serde_json::json!({"stdout": "fabricated"}),
        };
        assert!(matches!(
            validate_protected_plugin_host_turn(
                &sessions,
                invocation_id,
                &serde_json::to_value(substituted).expect("substituted continuation"),
            ),
            Err(PluginRunnerClientError::Denied(_))
        ));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn root_plugin_dispatch_issues_exact_permit_and_settles_observational_action() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = Arc::new(AppRuntime::open(config).expect("runtime"));
        let identity = current_runtime_identity().expect("Root identity");
        runtime
            .initialize_root_ownership(
                identity.clone(),
                RootMigrationReceipt {
                    schema_version: PROTOCOL_V1,
                    from_storage_schema: STORAGE_SCHEMA_VERSION,
                    to_storage_schema: STORAGE_SCHEMA_VERSION,
                    backup_path: None,
                    backup_sha256: None,
                    migrated_at: Utc::now(),
                },
            )
            .expect("Root ownership");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runner = SourceRef {
            source_type: StableTypeId::parse("mcg.plugin_runner_executable.v1")
                .expect("runner type"),
            locator: if cfg!(windows) {
                "mcg-plugin-runner.exe".into()
            } else {
                "mcg-plugin-runner".into()
            },
            content_hash: ContentHash {
                algorithm: HashAlgorithm::Sha256,
                domain: "MetaCog.PluginRunnerExecutable".into(),
                digest_hex: "c".repeat(64),
            },
        };
        runtime
            .install_protected_plugin_dispatcher(Arc::new(RootPluginDispatcher {
                runtime: Arc::downgrade(&runtime),
                root_executable: PathBuf::from("unused-in-injected-test"),
                runtime_actor: RuntimeActorId(identity.instance_id.0),
                runtime_identity: identity,
                capability_sessions: Mutex::new(BTreeMap::new()),
                test_gateway: Some(Arc::new(RecordingPluginGateway {
                    runner,
                    requests: requests.clone(),
                })),
                test_action_gateway: None,
            }))
            .expect("install Root plugin dispatcher");
        let module = br#"(module (func (export "_start")))"#;
        let revision = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.root-governed".into(),
                display_name: "Root governed plugin fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let installation = runtime
            .install_sandboxed_plugin(&revision, module)
            .expect("install plugin");
        runtime
            .set_plugin_enabled(&revision.manifest.id, installation.revision, true)
            .expect("enable plugin");
        let invocation_id = PluginInvocationId::new();
        let result = runtime
            .invoke_sandboxed_plugin(
                invocation_id,
                &revision.manifest.id,
                None,
                serde_json::json!({"governed": true}),
            )
            .await
            .expect("invoke through Root");
        assert_eq!(
            result.invocation.state,
            mcg_domain::PluginInvocationState::Succeeded
        );
        let workflows = runtime.canonical_workflows().expect("plugin workflows");
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].state, WorkflowState::Succeeded);
        let authorization = {
            let captured = requests.lock().expect("captured requests");
            assert_eq!(captured.len(), 1);
            captured[0]
                .authorization
                .clone()
                .expect("Root authorization")
        };
        assert_eq!(authorization.invocation_id, invocation_id.to_string());
        assert_eq!(authorization.plugin_revision_id, revision.id.to_string());
        assert!(authorization.permit.secret_handles.is_empty());
        assert!(authorization.permit.capabilities.filesystem.is_empty());
        assert!(authorization.permit.capabilities.network.is_empty());
        assert!(!authorization.permit.capabilities.process_spawn);
        let actions = runtime.canonical_actions(10).expect("actions");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].state, ActionExecutionState::NoEffectConfirmed);
        assert_eq!(actions[0].outcome, Some(EffectOutcome::NoEffectConfirmed));
        let capabilities = runtime
            .tool_authorizations(10, Utc::now())
            .expect("Tool authority");
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].action_id, actions[0].action_id);

        let mut capability_revision = revision.clone();
        capability_revision.id = PluginRevisionId::new();
        capability_revision.manifest.id = "example.root-denied-capability".into();
        capability_revision.manifest.display_name = "Denied capability fixture".into();
        capability_revision.manifest.requested_capabilities =
            vec![PluginCapability::FilesystemRead];
        let capability_installation = runtime
            .install_sandboxed_plugin(&capability_revision, module)
            .expect("install capability-bearing plugin");
        let capability_installation = runtime
            .set_plugin_capability_grants(
                &capability_revision.manifest.id,
                capability_installation.revision,
                vec![PluginCapabilityGrant {
                    schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemRead,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
            )
            .expect("grant compatibility capability");
        runtime
            .set_plugin_enabled(
                &capability_revision.manifest.id,
                capability_installation.revision,
                true,
            )
            .expect("enable capability-bearing plugin");
        let denied = runtime
            .invoke_sandboxed_plugin(
                PluginInvocationId::new(),
                &capability_revision.manifest.id,
                None,
                serde_json::json!({"must_not_launch": true}),
            )
            .await
            .expect("fail-closed plugin result");
        assert_eq!(
            denied.invocation.state,
            mcg_domain::PluginInvocationState::Failed
        );
        assert_eq!(
            denied.invocation.error_code.as_deref(),
            Some("authorization_denied")
        );
        assert_eq!(
            requests.lock().expect("captured requests").len(),
            1,
            "capability-bearing plugins must be denied before the runner is contacted"
        );
        let workflows = runtime.canonical_workflows().expect("plugin workflows");
        assert_eq!(workflows.len(), 1);
        assert_eq!(
            workflows
                .iter()
                .filter(|workflow| workflow.state == WorkflowState::Failed)
                .count(),
            0,
            "a capability invocation without a task/project is denied before canonical authority"
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn root_executes_plugin_project_read_with_a_separate_exact_permit() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let project_root = tempfile::tempdir().expect("plugin project root");
        fs::write(
            project_root.path().join("proof.txt"),
            b"protected broker proof",
        )
        .expect("plugin read fixture");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = Arc::new(AppRuntime::open(config).expect("runtime"));
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover plugin project");
        let mut task = TaskEnvelope::new("exercise a protected plugin read");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("create plugin task");
        let identity = current_runtime_identity().expect("Root identity");
        runtime
            .initialize_root_ownership(
                identity.clone(),
                RootMigrationReceipt {
                    schema_version: PROTOCOL_V1,
                    from_storage_schema: STORAGE_SCHEMA_VERSION,
                    to_storage_schema: STORAGE_SCHEMA_VERSION,
                    backup_path: None,
                    backup_sha256: None,
                    migrated_at: Utc::now(),
                },
            )
            .expect("Root ownership");
        let plugin_requests = Arc::new(Mutex::new(Vec::new()));
        let action_requests = Arc::new(Mutex::new(Vec::new()));
        let runner = SourceRef {
            source_type: StableTypeId::parse("mcg.plugin_runner_executable.v1")
                .expect("runner type"),
            locator: "mcg-plugin-runner".into(),
            content_hash: ContentHash {
                algorithm: HashAlgorithm::Sha256,
                domain: "MetaCog.PluginRunnerExecutable".into(),
                digest_hex: "d".repeat(64),
            },
        };
        runtime
            .install_protected_plugin_dispatcher(Arc::new(RootPluginDispatcher {
                runtime: Arc::downgrade(&runtime),
                root_executable: PathBuf::from("unused-in-injected-test"),
                runtime_actor: RuntimeActorId(identity.instance_id.0),
                runtime_identity: identity,
                capability_sessions: Mutex::new(BTreeMap::new()),
                test_gateway: Some(Arc::new(ScriptedProtectedPluginGateway {
                    runner,
                    requests: plugin_requests.clone(),
                    turns: Mutex::new(VecDeque::from([
                        PluginGuestTurn::CapabilityRequest {
                            request: PluginCapabilityRequest {
                                schema_version: PLUGIN_SCHEMA_VERSION,
                                id: "read-proof".into(),
                                operation: PluginCapabilityOperation::FilesystemRead {
                                    path: "proof.txt".into(),
                                    max_bytes: 1_024,
                                },
                            },
                        },
                        PluginGuestTurn::Completed {
                            output: serde_json::json!({"observed": true}),
                        },
                    ])),
                })),
                test_action_gateway: Some(Arc::new(PluginReadActionGateway {
                    calls: action_requests.clone(),
                })),
            }))
            .expect("install protected plugin dispatcher");
        let module = br#"(module (func (export "_start")))"#;
        let revision = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.protected-read".into(),
                display_name: "Protected read fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: vec![PluginCapability::FilesystemRead],
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let installation = runtime
            .install_sandboxed_plugin(&revision, module)
            .expect("install protected read plugin");
        let installation = runtime
            .set_plugin_capability_grants(
                &revision.manifest.id,
                installation.revision,
                vec![PluginCapabilityGrant {
                    schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemRead,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
            )
            .expect("grant protected project read");
        runtime
            .set_plugin_enabled(&revision.manifest.id, installation.revision, true)
            .expect("enable protected read plugin");

        let result = runtime
            .invoke_sandboxed_plugin(
                PluginInvocationId::new(),
                &revision.manifest.id,
                Some(task.id),
                serde_json::json!({"goal": "read proof"}),
            )
            .await
            .expect("invoke protected read plugin");
        assert_eq!(
            result.invocation.state,
            mcg_domain::PluginInvocationState::Succeeded
        );
        assert_eq!(
            result.result.expect("plugin result").output,
            serde_json::json!({"observed": true})
        );
        let captured = plugin_requests.lock().expect("plugin requests");
        assert_eq!(captured.len(), 2);
        let second: PluginHostTurn =
            serde_json::from_value(captured[1].input.clone()).expect("protected host turn");
        let PluginCapabilityOutcome::Succeeded { outcome, .. } = &second.outcomes[0] else {
            panic!("protected read must return exact success evidence")
        };
        assert_eq!(outcome["stdout"], "protected broker proof");
        drop(captured);
        let action_requests = action_requests.lock().expect("action requests");
        assert_eq!(action_requests.len(), 1);
        assert_eq!(
            action_requests[0].capability_root,
            fs::canonicalize(project_root.path()).expect("canonical project root")
        );
        drop(action_requests);
        let actions = runtime.canonical_actions(10).expect("plugin actions");
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().all(|action| {
            action.state == ActionExecutionState::NoEffectConfirmed
                && action.outcome == Some(EffectOutcome::NoEffectConfirmed)
        }));
        assert_eq!(
            runtime
                .canonical_workflows()
                .expect("plugin workflow")
                .into_iter()
                .next()
                .expect("canonical plugin workflow")
                .state,
            WorkflowState::Succeeded
        );
        let capabilities = runtime
            .tool_authorizations(10, Utc::now())
            .expect("plugin Tool permits");
        assert_eq!(capabilities.len(), 3);
        assert_eq!(
            capabilities
                .iter()
                .filter(|capability| capability.filesystem_scope_count > 0)
                .count(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn root_creates_plugin_project_file_with_reconciliation_and_compensation_authority() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let project_root = tempfile::tempdir().expect("plugin project root");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = Arc::new(AppRuntime::open(config).expect("runtime"));
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover plugin project");
        let mut task = TaskEnvelope::new("exercise a protected plugin create");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("create plugin task");
        let identity = current_runtime_identity().expect("Root identity");
        runtime
            .initialize_root_ownership(
                identity.clone(),
                RootMigrationReceipt {
                    schema_version: PROTOCOL_V1,
                    from_storage_schema: STORAGE_SCHEMA_VERSION,
                    to_storage_schema: STORAGE_SCHEMA_VERSION,
                    backup_path: None,
                    backup_sha256: None,
                    migrated_at: Utc::now(),
                },
            )
            .expect("Root ownership");
        let plugin_requests = Arc::new(Mutex::new(Vec::new()));
        let action_requests = Arc::new(Mutex::new(Vec::new()));
        let runner = SourceRef {
            source_type: StableTypeId::parse("mcg.plugin_runner_executable.v1")
                .expect("runner type"),
            locator: "mcg-plugin-runner".into(),
            content_hash: ContentHash {
                algorithm: HashAlgorithm::Sha256,
                domain: "MetaCog.PluginRunnerExecutable".into(),
                digest_hex: "e".repeat(64),
            },
        };
        runtime
            .install_protected_plugin_dispatcher(Arc::new(RootPluginDispatcher {
                runtime: Arc::downgrade(&runtime),
                root_executable: PathBuf::from("unused-in-injected-test"),
                runtime_actor: RuntimeActorId(identity.instance_id.0),
                runtime_identity: identity,
                capability_sessions: Mutex::new(BTreeMap::new()),
                test_gateway: Some(Arc::new(ScriptedProtectedPluginGateway {
                    runner,
                    requests: plugin_requests.clone(),
                    turns: Mutex::new(VecDeque::from([
                        PluginGuestTurn::CapabilityRequest {
                            request: PluginCapabilityRequest {
                                schema_version: PLUGIN_SCHEMA_VERSION,
                                id: "create-proof".into(),
                                operation: PluginCapabilityOperation::FilesystemWrite {
                                    path: "created.txt".into(),
                                    content: "protected create proof".into(),
                                    create_new: true,
                                },
                            },
                        },
                        PluginGuestTurn::Completed {
                            output: serde_json::json!({"created": true}),
                        },
                    ])),
                })),
                test_action_gateway: Some(Arc::new(PluginCreateActionGateway {
                    calls: action_requests.clone(),
                })),
            }))
            .expect("install protected plugin dispatcher");
        let module = br#"(module (func (export "_start")))"#;
        let revision = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.protected-create".into(),
                display_name: "Protected create fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: vec![PluginCapability::FilesystemWrite],
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let installation = runtime
            .install_sandboxed_plugin(&revision, module)
            .expect("install protected create plugin");
        let installation = runtime
            .set_plugin_capability_grants(
                &revision.manifest.id,
                installation.revision,
                vec![PluginCapabilityGrant {
                    schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemWrite,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
            )
            .expect("grant protected project write");
        runtime
            .set_plugin_enabled(&revision.manifest.id, installation.revision, true)
            .expect("enable protected create plugin");

        let result = runtime
            .invoke_sandboxed_plugin(
                PluginInvocationId::new(),
                &revision.manifest.id,
                Some(task.id),
                serde_json::json!({"goal": "create proof"}),
            )
            .await
            .expect("invoke protected create plugin");

        assert_eq!(
            result.invocation.state,
            mcg_domain::PluginInvocationState::Succeeded
        );
        assert_eq!(
            result.result.expect("plugin result").output,
            serde_json::json!({"created": true})
        );
        assert_eq!(
            fs::read_to_string(project_root.path().join("created.txt"))
                .expect("created project file"),
            "protected create proof"
        );
        let captured = plugin_requests.lock().expect("plugin requests");
        assert_eq!(captured.len(), 2);
        let second: PluginHostTurn =
            serde_json::from_value(captured[1].input.clone()).expect("protected host turn");
        assert!(matches!(
            second.outcomes[0],
            PluginCapabilityOutcome::Succeeded { .. }
        ));
        drop(captured);
        let action_requests = action_requests.lock().expect("action requests");
        assert_eq!(action_requests.len(), 1);
        assert!(action_requests[0].envelope.compensation.is_some());
        assert!(action_requests[0].envelope.reconciler.is_some());
        assert_eq!(
            action_requests[0].capability_root,
            fs::canonicalize(project_root.path()).expect("canonical project root")
        );
        drop(action_requests);
        let actions = runtime.canonical_actions(10).expect("plugin actions");
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions
                .iter()
                .filter(|action| action.state == ActionExecutionState::Committed)
                .count(),
            1
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| action.state == ActionExecutionState::NoEffectConfirmed)
                .count(),
            2
        );
        assert_eq!(
            runtime
                .canonical_workflows()
                .expect("plugin workflow")
                .into_iter()
                .next()
                .expect("canonical plugin workflow")
                .state,
            WorkflowState::Succeeded
        );
        let capabilities = runtime
            .tool_authorizations(10, Utc::now())
            .expect("plugin Tool permits");
        assert_eq!(capabilities.len(), 3);
        assert_eq!(
            capabilities
                .iter()
                .filter(|capability| capability.filesystem_scope_count > 0)
                .count(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn lost_plugin_create_response_waits_for_reconciliation_without_redispatch() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let project_root = tempfile::tempdir().expect("plugin project root");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = Arc::new(AppRuntime::open(config.clone()).expect("runtime"));
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover plugin project");
        let mut task = TaskEnvelope::new("exercise an uncertain protected plugin create");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("create plugin task");
        let identity = current_runtime_identity().expect("Root identity");
        runtime
            .initialize_root_ownership(
                identity.clone(),
                RootMigrationReceipt {
                    schema_version: PROTOCOL_V1,
                    from_storage_schema: STORAGE_SCHEMA_VERSION,
                    to_storage_schema: STORAGE_SCHEMA_VERSION,
                    backup_path: None,
                    backup_sha256: None,
                    migrated_at: Utc::now(),
                },
            )
            .expect("Root ownership");
        let plugin_requests = Arc::new(Mutex::new(Vec::new()));
        let action_requests = Arc::new(Mutex::new(Vec::new()));
        runtime
            .install_protected_plugin_dispatcher(Arc::new(RootPluginDispatcher {
                runtime: Arc::downgrade(&runtime),
                root_executable: PathBuf::from("unused-in-injected-test"),
                runtime_actor: RuntimeActorId(identity.instance_id.0),
                runtime_identity: identity,
                capability_sessions: Mutex::new(BTreeMap::new()),
                test_gateway: Some(Arc::new(ScriptedProtectedPluginGateway {
                    runner: SourceRef {
                        source_type: StableTypeId::parse("mcg.plugin_runner_executable.v1")
                            .expect("runner type"),
                        locator: "mcg-plugin-runner".into(),
                        content_hash: ContentHash {
                            algorithm: HashAlgorithm::Sha256,
                            domain: "MetaCog.PluginRunnerExecutable".into(),
                            digest_hex: "f".repeat(64),
                        },
                    },
                    requests: plugin_requests,
                    turns: Mutex::new(VecDeque::from([PluginGuestTurn::CapabilityRequest {
                        request: PluginCapabilityRequest {
                            schema_version: PLUGIN_SCHEMA_VERSION,
                            id: "lost-create-proof".into(),
                            operation: PluginCapabilityOperation::FilesystemWrite {
                                path: "uncertain.txt".into(),
                                content: "effect survives response loss".into(),
                                create_new: true,
                            },
                        },
                    }])),
                })),
                test_action_gateway: Some(Arc::new(PluginLostCreateResponseGateway {
                    calls: action_requests.clone(),
                })),
            }))
            .expect("install protected plugin dispatcher");
        let module = br#"(module (func (export "_start")))"#;
        let revision = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.uncertain-create".into(),
                display_name: "Uncertain create fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: vec![PluginCapability::FilesystemWrite],
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let installation = runtime
            .install_sandboxed_plugin(&revision, module)
            .expect("install uncertain create plugin");
        let installation = runtime
            .set_plugin_capability_grants(
                &revision.manifest.id,
                installation.revision,
                vec![PluginCapabilityGrant {
                    schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemWrite,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
            )
            .expect("grant protected project write");
        runtime
            .set_plugin_enabled(&revision.manifest.id, installation.revision, true)
            .expect("enable uncertain create plugin");

        let result = runtime
            .invoke_sandboxed_plugin(
                PluginInvocationId::new(),
                &revision.manifest.id,
                Some(task.id),
                serde_json::json!({"goal": "lose the response"}),
            )
            .await
            .expect("settle uncertain plugin invocation");

        assert_eq!(
            result.invocation.state,
            mcg_domain::PluginInvocationState::Uncertain
        );
        assert_eq!(
            result.invocation.error_code.as_deref(),
            Some("capability_execution_uncertain")
        );
        assert_eq!(
            fs::read_to_string(project_root.path().join("uncertain.txt"))
                .expect("uncertain effect remains inspectable"),
            "effect survives response loss"
        );
        assert_eq!(action_requests.lock().expect("action requests").len(), 1);
        let workflow = runtime
            .canonical_workflows()
            .expect("plugin workflow")
            .into_iter()
            .next()
            .expect("canonical plugin workflow");
        assert_eq!(
            workflow.state,
            WorkflowState::Waiting(WaitReason::Reconciliation)
        );
        let actions = runtime.canonical_actions(10).expect("plugin actions");
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions
                .iter()
                .filter(|action| action.state == ActionExecutionState::OutcomeUnknown)
                .count(),
            1
        );
        let jobs = runtime
            .due_canonical_reconciliations(Utc::now(), 10)
            .expect("due plugin reconciliation");
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            decode_governed_filesystem_operation(&jobs[0].adapter)
                .expect("exact inspection reconciler"),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "uncertain.txt".into(),
                max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
            }
        );
        let job = jobs[0].clone();
        let target = runtime
            .canonical_action_for_execution(job.execution_id)
            .expect("uncertain target query")
            .expect("uncertain target")
            .1;
        drop(runtime);

        let runtime = Arc::new(AppRuntime::open(config).expect("reopen runtime"));
        let worker = RuntimeActorId::new();
        let reconciled = reconcile_filesystem_action(
            &runtime,
            &PluginLostCreateResponseGateway {
                calls: action_requests.clone(),
            },
            job.id,
            job.revision,
            job.lease_generation,
            target.revision,
            target.lease_generation,
            worker,
            std::cmp::max(Utc::now(), job.next_attempt_at),
        )
        .await
        .expect("reconcile plugin create after runtime reopen");
        assert_eq!(
            reconciled.resolution.job.state,
            mcg_contracts::ReconciliationJobState::Resolved
        );
        commit_reconciled_plugin_filesystem_effect(
            &runtime,
            &reconciled.resolution,
            worker,
            Utc::now(),
        )
        .expect("commit independently reconciled plugin effect");
        settle_reconciled_plugin_action_workflow(
            &runtime,
            reconciled.resolution.job.action_id,
            Utc::now(),
        )
        .expect("settle reconciled plugin workflow");
        assert_eq!(
            runtime
                .canonical_workflows()
                .expect("settled plugin workflow")
                .into_iter()
                .next()
                .expect("canonical plugin workflow")
                .state,
            WorkflowState::Failed
        );
        let actions = runtime
            .canonical_actions(10)
            .expect("reconciled plugin actions");
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions
                .iter()
                .filter(|action| action.state == ActionExecutionState::Committed)
                .count(),
            1
        );
        assert_eq!(
            actions
                .iter()
                .filter(|action| action.state == ActionExecutionState::NoEffectConfirmed)
                .count(),
            2
        );
        let requests = action_requests.lock().expect("action requests");
        assert_eq!(requests.len(), 2);
        assert_ne!(requests[0].action_id, requests[1].action_id);
        assert_eq!(
            requests[0].execution_mode,
            mcg_action_runner::ActionRunnerExecutionMode::Effect
        );
        assert_eq!(
            requests[1].execution_mode,
            mcg_action_runner::ActionRunnerExecutionMode::Effect
        );
        assert_eq!(requests[0].envelope.effect_class, EffectClass::Reversible);
        assert_eq!(
            requests[1].envelope.effect_class,
            EffectClass::Observational
        );
        assert!(matches!(
            decode_governed_filesystem_operation(&requests[1].operation)
                .expect("exact reconciliation probe"),
            GovernedFilesystemOperation::InspectFile { .. }
        ));
        drop(requests);
        assert_eq!(
            runtime
                .tool_authorizations(10, Utc::now())
                .expect("reconciliation Tool permits")
                .len(),
            3
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
            let capability = runtime
                .active_tool_capability_authorization(receipt.execution.action_id, now)
                .expect("load active filesystem Tool permit")
                .expect("filesystem Tool permit exists");
            assert_eq!(capability.permit.action_id, receipt.execution.action_id);
            assert_eq!(
                capability.proposal.argument_hash,
                capability.permit.argument_hash
            );
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

    #[tokio::test]
    async fn authenticated_local_ipc_projects_bounded_tool_authority_without_secret_handles() {
        let (temporary, _config, runtime, execution, _actor, now) = authorized_filesystem_fixture();
        execute_authorized_filesystem_action(
            &runtime,
            &ScriptedActionGateway(GatewayOutcome::LaunchFailed),
            execution.id,
            execution.revision,
            execution.lease_generation,
            RuntimeActorId::new(),
            now,
        )
        .await
        .expect("persist Tool authority before launch failure");
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let summaries = query_root_capabilities(temporary.path(), 10)
            .await
            .expect("Tool authorization list");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].action_id, execution.action_id);
        let inspection = query_root_capability(temporary.path(), execution.action_id)
            .await
            .expect("Tool authorization show")
            .expect("Tool authorization inspection");
        assert_eq!(inspection.summary, summaries[0]);
        assert_eq!(inspection.scopes.filesystem.len(), 1);
        assert_eq!(inspection.scopes.network.len(), 0);
        assert!(!inspection.scopes.process_spawn);
        let serialized = serde_json::to_string(&inspection).expect("serialize inspection");
        assert!(!serialized.contains("secret_handles"));
        assert!(!serialized.contains("arguments"));
        assert!(query_root_capabilities(temporary.path(), 0).await.is_err());

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }
}
