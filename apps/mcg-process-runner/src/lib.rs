use std::{
    collections::BTreeSet,
    io::Read as _,
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId,
    ArtifactRef, ContentHash, EffectClass, ExecutionPlatform, FilesystemAccess,
    FilesystemCapability, HashAlgorithm, IsolationGuarantee, PROTOCOL_V1, PermitId, PolicyBinding,
    RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, SandboxCapabilityProfile, SourceRef,
    StableTypeId, TOOL_POLICY_HASH_DOMAIN, TaskContractId, ToolCapabilitySet, ToolInvocationPermit,
    ToolInvocationProposal, ToolManifest, ToolManifestId, ToolPolicy, ToolResourceLimits,
    VersionedPayload, VersionedRef, decode_canonical_cbor_v1, tool_invocation_permit_hash,
    tool_invocation_proposal_hash,
};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _},
    process::Command,
    time::Instant,
};
use uuid::Uuid;

pub const PROCESS_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const PROCESS_RUNNER_REQUEST_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const PROCESS_RUNNER_RESPONSE_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const PROCESS_RUNNER_STREAM_MAX_BYTES: u64 = 1024 * 1024;
pub const GOVERNED_ACTION_OPERATION_HASH_DOMAIN: &str = "MetaCog.ActionOperation";
pub const GOVERNED_PROCESS_AUTHORITY_HASH_DOMAIN: &str = "MetaCog.ProcessAuthority";
pub const GOVERNED_PROCESS_AUTHORITY_TYPE: &str = "mcg.governed_process_authority.v1";
pub const GOVERNED_PROCESS_OPERATION_TYPE: &str = "mcg.governed_process_operation.v1";
pub const GOVERNED_PROCESS_OBSERVATION_TYPE: &str = "mcg.governed_process_observation.v1";
pub const GOVERNED_PROCESS_RECONCILER_TYPE: &str = "mcg.governed_process_reconciler.v1";
pub const PROCESS_RUNNER_EXECUTABLE_SOURCE_TYPE: &str = "mcg.process_runner_executable.v1";
pub const PROCESS_RUNNER_EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.ProcessRunnerExecutable";
pub const PROCESS_RUNNER_INPUT_SCHEMA_TYPE: &str = "mcg.process_runner_input_schema.v1";
pub const PROCESS_RUNNER_OUTPUT_SCHEMA_TYPE: &str = "mcg.process_runner_output_schema.v1";
pub const PROCESS_RUNNER_RECONCILER_SOURCE_TYPE: &str = "mcg.process_runner_reconciler.v1";
pub const PROCESS_RUNNER_RECEIPT_TYPE: &str = "mcg.process_runner_receipt.v1";

const STDERR_MAX_BYTES: usize = 4 * 1024;
const DETAIL_MAX_BYTES: usize = 4 * 1024;
const MAX_ARGUMENTS: usize = 128;
const MAX_ENVIRONMENT: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 8 * 1024;
const MAX_ENVIRONMENT_VALUE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessRunnerAuthorization {
    pub manifest: ToolManifest,
    pub proposal: ToolInvocationProposal,
    pub proposal_hash: ContentHash,
    pub policy: ToolPolicy,
    pub permit: ToolInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRunnerCapabilityEvidence {
    pub runner: SourceRef,
    pub platform: ExecutionPlatform,
    pub available_sandbox: SandboxCapabilityProfile,
    pub resource_ceiling: ToolResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub envelope: ActionEnvelope,
    pub authorization: ProcessRunnerAuthorization,
    pub project_root: PathBuf,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedProcessEnvironmentVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedProcessOperation {
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub argv: Vec<String>,
    pub cwd_relative: String,
    pub environment: Vec<GovernedProcessEnvironmentVariable>,
    pub stdin_base64: Option<String>,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedProcessAuthority {
    pub action_id: ActionId,
    pub actor: RuntimeActorId,
    pub project_root: PathBuf,
    pub operation_hash: ContentHash,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedProcessReconciler {
    pub executable_sha256: String,
    pub disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedProcessObservation {
    pub executable_sha256: String,
    pub exit_code: Option<i32>,
    pub stdout_base64: String,
    pub stderr_base64: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessRunnerOutcome {
    NoEffectConfirmed,
    EffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: ProcessRunnerOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessRunnerProtocolError {
    #[error("governed process action is invalid: {0}")]
    Invalid(&'static str),
    #[error("governed process canonical payload is invalid")]
    Canonical,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRunnerClientError {
    #[error("process runner is unavailable")]
    Unavailable,
    #[error("process runner executable identity changed")]
    ExecutableChanged,
    #[error("process runner request is too large")]
    RequestTooLarge,
    #[error("process runner could not be launched")]
    Launch,
    #[error("process runner request delivery failed")]
    Delivery,
    #[error("process runner exceeded its deadline")]
    Timeout,
    #[error("process runner transport failed")]
    Process,
    #[error("process runner output exceeded its bound")]
    ResponseTooLarge,
    #[error("process runner emitted diagnostic output")]
    DiagnosticOutput,
    #[error("process runner returned an invalid response")]
    InvalidResponse,
}

impl ProcessRunnerClientError {
    #[must_use]
    pub const fn effect_may_have_occurred(self) -> bool {
        !matches!(
            self,
            Self::Unavailable | Self::ExecutableChanged | Self::RequestTooLarge | Self::Launch
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProcessRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
    capability_evidence: ProcessRunnerCapabilityEvidence,
}

impl ProcessRunnerClient {
    /// Selects only the fixed process runner adjacent to a trusted application executable.
    ///
    /// # Errors
    /// Returns unavailable when the adjacent path or fixed executable identity is invalid.
    pub fn adjacent_to(application: impl AsRef<Path>) -> Result<Self, ProcessRunnerClientError> {
        let parent = application
            .as_ref()
            .parent()
            .ok_or(ProcessRunnerClientError::Unavailable)?;
        Self::with_executable(parent.join(runner_file_name()), Duration::from_secs(35))
    }

    /// Creates a fixed-identity client for package bootstrap and process-contract tests.
    ///
    /// # Errors
    /// Returns unavailable when the path, filename, file identity, or timeout is invalid.
    pub fn with_executable(
        executable: impl AsRef<Path>,
        outer_timeout: Duration,
    ) -> Result<Self, ProcessRunnerClientError> {
        let executable = executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name()))
            || outer_timeout.is_zero()
            || outer_timeout > Duration::from_mins(2)
        {
            return Err(ProcessRunnerClientError::Unavailable);
        }
        require_regular_unlinked(executable)?;
        let timeout_ms = u64::try_from(outer_timeout.as_millis())
            .map_err(|_| ProcessRunnerClientError::Unavailable)?;
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
            capability_evidence: ProcessRunnerCapabilityEvidence {
                runner: runner_executable_source(executable)?,
                platform: current_execution_platform(),
                available_sandbox: process_runner_sandbox_profile(),
                resource_ceiling: ToolResourceLimits {
                    timeout_ms,
                    max_output_bytes: PROCESS_RUNNER_STREAM_MAX_BYTES,
                    max_memory_bytes: 0,
                    max_child_processes: 64,
                },
            },
        })
    }

    #[must_use]
    pub fn capability_evidence(&self) -> &ProcessRunnerCapabilityEvidence {
        &self.capability_evidence
    }

    /// Dispatches one request through a cleared-environment, kill-on-drop process tree.
    ///
    /// # Errors
    /// Returns a categorical client error for prelaunch identity failure, bounded transport
    /// failure, timeout, diagnostic output, or an invalid child response.
    pub async fn invoke(
        &self,
        request: &ProcessRunnerRequest,
    ) -> Result<ProcessRunnerResponse, ProcessRunnerClientError> {
        self.validate_executable_identity()?;
        let bytes =
            serde_json::to_vec(request).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
        if bytes.len() > PROCESS_RUNNER_REQUEST_MAX_BYTES {
            return Err(ProcessRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command
            .env_clear()
            .current_dir(
                self.executable
                    .parent()
                    .ok_or(ProcessRunnerClientError::Unavailable)?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(JobObject);
        let mut child = command
            .spawn()
            .map_err(|_| ProcessRunnerClientError::Launch)?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (
            child.stdin().take(),
            child.stdout().take(),
            child.stderr().take(),
        ) else {
            let _ = Box::into_pin(child.kill()).await;
            let _ = child.wait().await;
            return Err(ProcessRunnerClientError::Launch);
        };
        let mut stdout_task = tokio::spawn(read_bounded(stdout, PROCESS_RUNNER_RESPONSE_MAX_BYTES));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        if tokio::time::timeout_at(deadline, async {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await
        })
        .await
        .is_err()
        {
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(ProcessRunnerClientError::Timeout);
        }
        drop(stdin);
        let status = match tokio::time::timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Process);
            }
            Err(_) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Timeout);
            }
        };
        let output = tokio::time::timeout_at(deadline, async {
            Ok::<_, ProcessRunnerClientError>((
                join_reader(&mut stdout_task).await?,
                join_reader(&mut stderr_task).await?,
            ))
        })
        .await
        .map_err(|_| ProcessRunnerClientError::Timeout)??;
        if output.0.1 || output.1.1 {
            return Err(ProcessRunnerClientError::ResponseTooLarge);
        }
        if !output.1.0.is_empty() {
            return Err(ProcessRunnerClientError::DiagnosticOutput);
        }
        validate_client_response(status.success(), request, &output.0.0)
    }

    fn validate_executable_identity(&self) -> Result<(), ProcessRunnerClientError> {
        require_regular_unlinked(&self.executable)?;
        if runner_executable_source(&self.executable)? != self.capability_evidence.runner {
            return Err(ProcessRunnerClientError::ExecutableChanged);
        }
        Ok(())
    }
}

async fn read_bounded<R: AsyncRead + Unpin>(
    reader: R,
    limit: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .await?;
    let overflow = bytes.len() > limit;
    if overflow {
        bytes.truncate(limit);
    }
    Ok((bytes, overflow))
}

async fn join_reader(
    task: &mut tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
) -> Result<(Vec<u8>, bool), ProcessRunnerClientError> {
    task.await
        .map_err(|_| ProcessRunnerClientError::Process)?
        .map_err(|_| ProcessRunnerClientError::Process)
}

async fn stop_child(
    child: &mut dyn ChildWrapper,
    stdout: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stderr: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
) {
    let _ = Box::into_pin(child.kill()).await;
    let _ = child.wait().await;
    stdout.abort();
    stderr.abort();
}

fn validate_client_response(
    process_succeeded: bool,
    request: &ProcessRunnerRequest,
    bytes: &[u8],
) -> Result<ProcessRunnerResponse, ProcessRunnerClientError> {
    let response: ProcessRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
    let response_succeeded = response.outcome == ProcessRunnerOutcome::EffectConfirmed;
    if response.protocol_version != PROCESS_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.action_id != Some(request.action_id)
        || response.execution_id != Some(request.execution_id)
        || process_succeeded != response_succeeded
        || response.detail.is_empty()
        || response.detail.len() > DETAIL_MAX_BYTES
        || response.started_at > response.finished_at
    {
        return Err(ProcessRunnerClientError::InvalidResponse);
    }
    match response.outcome {
        ProcessRunnerOutcome::EffectConfirmed => {
            let Some(observation) = &response.observation else {
                return Err(ProcessRunnerClientError::InvalidResponse);
            };
            if response.error_code.is_some()
                || observation.type_id.as_str() != GOVERNED_PROCESS_OBSERVATION_TYPE
                || decode_canonical_cbor_v1(&observation.bytes).is_err()
            {
                return Err(ProcessRunnerClientError::InvalidResponse);
            }
        }
        _ if response.observation.is_some()
            || response
                .error_code
                .as_deref()
                .is_none_or(|code| !valid_code(code)) =>
        {
            return Err(ProcessRunnerClientError::InvalidResponse);
        }
        _ => {}
    }
    Ok(response)
}

/// Canonically encodes one exact process invocation in the governed action hash domain.
///
/// # Errors
/// Returns an error when the operation is invalid or cannot be canonically encoded.
pub fn governed_process_operation_payload(
    operation: &GovernedProcessOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    validate_operation(operation).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_PROCESS_OPERATION_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(operation).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Canonically binds one action to one exact project root, operation, actor, and deadline.
///
/// # Errors
/// Returns an error when the authority is invalid or cannot be canonically encoded.
pub fn governed_process_authority_binding(
    authority: &GovernedProcessAuthority,
) -> Result<PolicyBinding, String> {
    if !authority.project_root.is_absolute() || authority.expires_at <= Utc::now() {
        return Err("governed process authority is invalid".into());
    }
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_PROCESS_AUTHORITY_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(authority).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(GOVERNED_PROCESS_AUTHORITY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Creates the mandatory manual-only reconciliation descriptor for an arbitrary process effect.
///
/// # Errors
/// Returns an error when the executable digest or canonical payload is invalid.
pub fn governed_process_reconciler(executable_sha256: &str) -> Result<VersionedPayload, String> {
    if !valid_sha256(executable_sha256) {
        return Err("process executable digest is invalid".into());
    }
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_PROCESS_RECONCILER_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(GovernedProcessReconciler {
            executable_sha256: executable_sha256.into(),
            disposition: "manual_review_no_redispatch".into(),
        })
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Validates the immutable process action and its mandatory irreversible/manual-review shape.
///
/// # Errors
/// Returns a protocol error for malformed, expired, substituted, or under-governed action state.
pub fn validate_governed_process_envelope(
    envelope: &ActionEnvelope,
    now: DateTime<Utc>,
) -> Result<GovernedProcessAuthority, ProcessRunnerProtocolError> {
    if envelope.operation.type_id.as_str() != GOVERNED_PROCESS_OPERATION_TYPE
        || envelope.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN
        || envelope
            .operation
            .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
            .map_err(|_| ProcessRunnerProtocolError::Canonical)?
            != envelope.operation_hash
    {
        return Err(ProcessRunnerProtocolError::Invalid("operation binding"));
    }
    if envelope.authority.payload.type_id.as_str() != GOVERNED_PROCESS_AUTHORITY_TYPE
        || envelope.authority.content_hash.domain != GOVERNED_PROCESS_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_PROCESS_AUTHORITY_HASH_DOMAIN)
            .map_err(|_| ProcessRunnerProtocolError::Canonical)?
            != envelope.authority.content_hash
    {
        return Err(ProcessRunnerProtocolError::Invalid("authority binding"));
    }
    let authority: GovernedProcessAuthority = decode_payload(&envelope.authority.payload)?;
    let operation: GovernedProcessOperation = decode_payload(&envelope.operation)?;
    validate_operation(&operation)?;
    let Some(reconciler) = &envelope.reconciler else {
        return Err(ProcessRunnerProtocolError::Invalid("process reconciler"));
    };
    let expected_reconciler = governed_process_reconciler(&operation.executable_sha256)
        .map_err(|_| ProcessRunnerProtocolError::Canonical)?;
    if authority.action_id != envelope.id
        || authority.actor != envelope.actor
        || authority.operation_hash != envelope.operation_hash
        || !authority.project_root.is_absolute()
        || authority.expires_at <= now
        || envelope.effect_class != EffectClass::Irreversible
        || envelope.elevated_authority.is_none()
        || envelope.compensation.is_some()
        || envelope.compensates_action.is_some()
        || reconciler != &expected_reconciler
        || envelope
            .reconciliation_deadline
            .is_none_or(|deadline| deadline <= now)
    {
        return Err(ProcessRunnerProtocolError::Invalid(
            "process authority scope",
        ));
    }
    Ok(authority)
}

/// Builds a one-shot child request from exact dispatched immutable action state.
///
/// # Errors
/// Returns a protocol error when the action, execution, or authority binding is invalid.
pub fn request_from_authorized_action(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    authorization: ProcessRunnerAuthorization,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<ProcessRunnerRequest, ProcessRunnerProtocolError> {
    let authority = validate_governed_process_envelope(envelope, now)?;
    if execution.action_id != envelope.id
        || execution.state != ActionExecutionState::Executing
        || execution.executor.is_none()
    {
        return Err(ProcessRunnerProtocolError::Invalid("execution binding"));
    }
    Ok(ProcessRunnerRequest {
        protocol_version: PROCESS_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        envelope: envelope.clone(),
        authorization,
        project_root: authority.project_root,
        operation: envelope.operation.clone(),
        operation_hash: envelope.operation_hash.clone(),
    })
}

/// Decodes one canonical process observation.
///
/// # Errors
/// Returns a protocol error when the type or canonical payload is invalid.
pub fn decode_governed_process_observation(
    payload: &VersionedPayload,
) -> Result<GovernedProcessObservation, ProcessRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_PROCESS_OBSERVATION_TYPE {
        return Err(ProcessRunnerProtocolError::Invalid(
            "process observation type",
        ));
    }
    decode_payload(payload)
}

fn decode_payload<T: serde::de::DeserializeOwned>(
    payload: &VersionedPayload,
) -> Result<T, ProcessRunnerProtocolError> {
    let value = decode_canonical_cbor_v1(&payload.bytes)
        .map_err(|_| ProcessRunnerProtocolError::Canonical)?;
    serde_json::from_value(value).map_err(|_| ProcessRunnerProtocolError::Canonical)
}

fn validate_operation(
    operation: &GovernedProcessOperation,
) -> Result<(), ProcessRunnerProtocolError> {
    if !operation.executable.is_absolute()
        || operation
            .executable
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || !valid_sha256(&operation.executable_sha256)
        || !normalized_relative_path(&operation.cwd_relative)
        || operation.argv.len() > MAX_ARGUMENTS
        || operation.environment.len() > MAX_ENVIRONMENT
        || operation.timeout_ms == 0
        || operation.timeout_ms > 120_000
        || operation.max_output_bytes == 0
        || operation.max_output_bytes > PROCESS_RUNNER_STREAM_MAX_BYTES
        || operation
            .argv
            .iter()
            .any(|value| value.len() > MAX_ARGUMENT_BYTES || value.contains('\0'))
    {
        return Err(ProcessRunnerProtocolError::Invalid(
            "process operation shape",
        ));
    }
    let mut names = BTreeSet::new();
    for variable in &operation.environment {
        let normalized = variable.name.to_ascii_uppercase();
        if !valid_environment_name(&variable.name)
            || variable.value.len() > MAX_ENVIRONMENT_VALUE_BYTES
            || variable.value.contains('\0')
            || !names.insert(normalized.clone())
            || reserved_environment_name(&normalized)
        {
            return Err(ProcessRunnerProtocolError::Invalid(
                "process environment shape",
            ));
        }
    }
    if operation.stdin_base64.as_ref().is_some_and(|value| {
        decode_canonical_base64(value, PROCESS_RUNNER_STREAM_MAX_BYTES).is_err()
    }) {
        return Err(ProcessRunnerProtocolError::Invalid("process stdin"));
    }
    Ok(())
}

fn normalized_relative_path(value: &str) -> bool {
    if value == "." {
        return true;
    }
    if value.is_empty() || value.len() > 4_096 || value.contains(['\\', '\0']) {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path.components().all(|component| match component {
            Component::Normal(value) => !value.is_empty(),
            _ => false,
        })
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
        })
}

fn reserved_environment_name(value: &str) -> bool {
    matches!(
        value,
        "PATH"
            | "PATHEXT"
            | "COMSPEC"
            | "SYSTEMROOT"
            | "WINDIR"
            | "LD_PRELOAD"
            | "LD_LIBRARY_PATH"
            | "DYLD_INSERT_LIBRARIES"
            | "DYLD_LIBRARY_PATH"
            | "DYLD_FRAMEWORK_PATH"
            | "PYTHONPATH"
            | "NODE_OPTIONS"
            | "RUSTC_WRAPPER"
            | "SHELL"
    ) || value.starts_with("DYLD_")
}

fn decode_canonical_base64(value: &str, limit: u64) -> Result<Vec<u8>, ()> {
    let bytes = STANDARD.decode(value).map_err(|_| ())?;
    if u64::try_from(bytes.len()).map_err(|_| ())? > limit || STANDARD.encode(&bytes) != value {
        return Err(());
    }
    Ok(bytes)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[must_use]
pub const fn current_execution_platform() -> ExecutionPlatform {
    if cfg!(windows) {
        ExecutionPlatform::Windows
    } else if cfg!(target_os = "macos") {
        ExecutionPlatform::Macos
    } else {
        ExecutionPlatform::Linux
    }
}

/// Reports only the confinement physically enforced by this adjacent runner.
#[must_use]
pub const fn process_runner_sandbox_profile() -> SandboxCapabilityProfile {
    SandboxCapabilityProfile {
        // Exact cwd and a cleared environment do not confine an arbitrary executable's host
        // filesystem or sockets. Report those dimensions unavailable until a platform backend
        // supplies an independently qualified ACL/AppContainer/sandbox/namespace boundary.
        filesystem_scope: IsolationGuarantee::Unavailable,
        network: IsolationGuarantee::Unavailable,
        process_tree: if cfg!(windows) {
            IsolationGuarantee::Hard
        } else {
            IsolationGuarantee::BestEffort
        },
        secrets: IsolationGuarantee::Unavailable,
        environment: IsolationGuarantee::Hard,
        resources: IsolationGuarantee::BestEffort,
        protected_paths: IsolationGuarantee::Unavailable,
    }
}

/// Measures the fixed regular, unlinked runner executable into a content-addressed source.
///
/// # Errors
/// Returns unavailable when the path is linked, irregular, unreadable, or cannot be hashed.
pub fn runner_executable_source(path: &Path) -> Result<SourceRef, ProcessRunnerClientError> {
    require_regular_unlinked(path)?;
    let digest = sha256_file(path).map_err(|_| ProcessRunnerClientError::Unavailable)?;
    Ok(SourceRef {
        source_type: StableTypeId::parse(PROCESS_RUNNER_EXECUTABLE_SOURCE_TYPE)
            .map_err(|_| ProcessRunnerClientError::Unavailable)?,
        locator: runner_file_name().into(),
        content_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: PROCESS_RUNNER_EXECUTABLE_HASH_DOMAIN.into(),
            digest_hex: digest,
        },
    })
}

/// Measures one exact regular, unlinked executable file for Root authorization.
///
/// # Errors
/// Returns unavailable when the path is linked, irregular, unreadable, or cannot be hashed.
pub fn executable_sha256(path: &Path) -> Result<String, ProcessRunnerClientError> {
    require_regular_unlinked(path)?;
    sha256_file(path).map_err(|_| ProcessRunnerClientError::Unavailable)
}

#[must_use]
pub fn process_runner_input_schema_source() -> SourceRef {
    static_source_ref(
        PROCESS_RUNNER_INPUT_SCHEMA_TYPE,
        GOVERNED_PROCESS_OPERATION_TYPE,
        "MetaCog.ProcessRunnerInputSchema",
    )
}

#[must_use]
pub fn process_runner_output_schema_source() -> SourceRef {
    static_source_ref(
        PROCESS_RUNNER_OUTPUT_SCHEMA_TYPE,
        GOVERNED_PROCESS_OBSERVATION_TYPE,
        "MetaCog.ProcessRunnerOutputSchema",
    )
}

#[must_use]
pub fn process_runner_reconciler_source() -> SourceRef {
    static_source_ref(
        PROCESS_RUNNER_RECONCILER_SOURCE_TYPE,
        GOVERNED_PROCESS_RECONCILER_TYPE,
        "MetaCog.ProcessRunnerReconciler",
    )
}

/// Produces a deterministic manifest for one executable/project/limit authority tuple.
/// Derives the deterministic Tool manifest for one exact runner, project, executable, and limit.
///
/// # Errors
/// Returns an error when paths, identity, limits, or the derived manifest are invalid.
pub fn governed_process_tool_manifest(
    evidence: &ProcessRunnerCapabilityEvidence,
    project_root: &Path,
    executable: &Path,
    executable_sha256: &str,
    limits: ToolResourceLimits,
) -> Result<ToolManifest, String> {
    if !project_root.is_absolute() || !executable.is_absolute() || !valid_sha256(executable_sha256)
    {
        return Err("process Tool paths or executable identity are invalid".into());
    }
    limits.validate().map_err(|error| error.to_string())?;
    if !limits.fits_within(evidence.resource_ceiling) {
        return Err("process Tool limits exceed runner capacity".into());
    }
    let root = project_root
        .to_str()
        .ok_or("process Tool project root is not Unicode")?
        .to_owned();
    let executable_text = executable
        .to_str()
        .ok_or("process Tool executable is not Unicode")?;
    let material = serde_json::to_vec(&serde_json::json!({
        "runner": evidence.runner,
        "platform": evidence.platform,
        "project_root": root,
        "executable": executable_text,
        "executable_sha256": executable_sha256,
        "sandbox": evidence.available_sandbox,
        "limits": limits,
    }))
    .map_err(|error| error.to_string())?;
    let digest = Sha256::digest(material);
    let mut uuid_bytes = [0_u8; 16];
    uuid_bytes.copy_from_slice(&digest[..16]);
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
    let manifest = ToolManifest {
        id: ToolManifestId(Uuid::from_bytes(uuid_bytes)),
        version: 1,
        name: "process.exec".into(),
        summary: "Governed one-shot execution of one exact executable identity".into(),
        runner: evidence.runner.clone(),
        input_schema: process_runner_input_schema_source(),
        output_schema: process_runner_output_schema_source(),
        maximum_effect_class: EffectClass::Irreversible,
        required_capabilities: ToolCapabilitySet {
            filesystem: vec![FilesystemCapability {
                root,
                access: FilesystemAccess::Read,
            }],
            network: Vec::new(),
            secret_slots: Vec::new(),
            process_spawn: true,
        },
        resource_ceiling: limits,
        sandbox_requirement: evidence.available_sandbox,
        platform_support: vec![evidence.platform],
        receipt_semantics: StableTypeId::parse(PROCESS_RUNNER_RECEIPT_TYPE)
            .map_err(|error| error.to_string())?,
        reconciler: Some(process_runner_reconciler_source()),
    };
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Derives the exact Root policy for one governed process manifest.
///
/// # Errors
/// Returns an error when the manifest reference or resulting policy is invalid.
pub fn governed_process_tool_policy(manifest: &ToolManifest) -> Result<ToolPolicy, String> {
    Ok(ToolPolicy {
        allowed_manifests: vec![
            manifest
                .versioned_ref()
                .map_err(|error| error.to_string())?,
        ],
        maximum_effect_class: EffectClass::Irreversible,
        maximum_capabilities: manifest.required_capabilities.clone(),
        maximum_limits: manifest.resource_ceiling,
        maximum_permit_lifetime_seconds: 24 * 60 * 60,
    })
}

fn static_source_ref(source_type: &str, locator: &str, domain: &str) -> SourceRef {
    let digest = Sha256::digest(format!("{source_type}\0{locator}").as_bytes());
    SourceRef {
        source_type: StableTypeId::parse(source_type).expect("fixed process-runner source type"),
        locator: locator.into(),
        content_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: domain.into(),
            digest_hex: format!("{digest:x}"),
        },
    }
}

fn tool_policy_binding(policy: &ToolPolicy) -> Result<PolicyBinding, String> {
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(ToolPolicy::TYPE_ID).map_err(|error| error.to_string())?,
        &serde_json::to_value(policy).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(TOOL_POLICY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

fn require_regular_unlinked(path: &Path) -> Result<(), ProcessRunnerClientError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| ProcessRunnerClientError::Unavailable)?;
    if !metadata.file_type().is_file() || is_link_or_reparse(&metadata) {
        return Err(ProcessRunnerClientError::Unavailable);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

const fn runner_file_name() -> &'static str {
    if cfg!(windows) {
        "mcg-process-runner.exe"
    } else {
        "mcg-process-runner"
    }
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[derive(Debug)]
struct RunnerFailure {
    outcome: ProcessRunnerOutcome,
    code: &'static str,
    detail: String,
}

impl RunnerFailure {
    fn denied(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: ProcessRunnerOutcome::Denied,
            code,
            detail: detail.into(),
        }
    }

    fn no_effect(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: ProcessRunnerOutcome::NoEffectConfirmed,
            code,
            detail: detail.into(),
        }
    }

    fn unknown(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: ProcessRunnerOutcome::OutcomeUnknown,
            code,
            detail: detail.into(),
        }
    }
}

/// Parses, independently validates, and executes one one-shot process request.
pub async fn execute_request(bytes: &[u8]) -> ProcessRunnerResponse {
    let started_at = Utc::now();
    if bytes.is_empty() || bytes.len() > PROCESS_RUNNER_REQUEST_MAX_BYTES {
        return failure_response(
            None,
            &RunnerFailure::denied("request_size_invalid", "process request size is invalid"),
            started_at,
        );
    }
    let Ok(request) = serde_json::from_slice::<ProcessRunnerRequest>(bytes) else {
        return failure_response(
            None,
            &RunnerFailure::denied("request_invalid", "process request is not valid JSON"),
            started_at,
        );
    };
    match execute(&request).await {
        Ok(observation) => ProcessRunnerResponse {
            protocol_version: PROCESS_RUNNER_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            action_id: Some(request.action_id),
            execution_id: Some(request.execution_id),
            outcome: ProcessRunnerOutcome::EffectConfirmed,
            observation: Some(observation),
            error_code: None,
            detail: "exact governed process exited and its bounded output was captured".into(),
            started_at,
            finished_at: Utc::now(),
        },
        Err(error) => failure_response(Some(&request), &error, started_at),
    }
}

fn failure_response(
    request: Option<&ProcessRunnerRequest>,
    failure: &RunnerFailure,
    started_at: DateTime<Utc>,
) -> ProcessRunnerResponse {
    ProcessRunnerResponse {
        protocol_version: PROCESS_RUNNER_PROTOCOL_VERSION,
        request_id: request.map(|request| request.request_id),
        action_id: request.map(|request| request.action_id),
        execution_id: request.map(|request| request.execution_id),
        outcome: failure.outcome,
        observation: None,
        error_code: Some(failure.code.into()),
        detail: failure.detail.chars().take(DETAIL_MAX_BYTES).collect(),
        started_at,
        finished_at: Utc::now(),
    }
}

#[allow(clippy::too_many_lines)] // Keep the post-launch ambiguity boundary contiguous and auditable.
async fn execute(request: &ProcessRunnerRequest) -> Result<VersionedPayload, RunnerFailure> {
    if request.protocol_version != PROCESS_RUNNER_PROTOCOL_VERSION {
        return Err(RunnerFailure::denied(
            "protocol_unsupported",
            "process runner protocol version is unsupported",
        ));
    }
    validate_tool_authorization(request, Utc::now())?;
    let operation: GovernedProcessOperation = decode_payload(&request.operation)
        .map_err(|error| RunnerFailure::denied("operation_invalid", error.to_string()))?;
    let root = canonical_regular_directory(&request.project_root, "project_root_invalid")?;
    let cwd = root.join(&operation.cwd_relative);
    let cwd = canonical_regular_directory(&cwd, "working_directory_invalid")?;
    if !cwd.starts_with(&root) {
        return Err(RunnerFailure::denied(
            "working_directory_outside_project",
            "process working directory escapes the exact project root",
        ));
    }
    require_regular_unlinked(&operation.executable).map_err(|_| {
        RunnerFailure::no_effect(
            "executable_unavailable",
            "authorized executable is unavailable or linked",
        )
    })?;
    let executable_digest = sha256_file(&operation.executable).map_err(|_| {
        RunnerFailure::no_effect(
            "executable_unavailable",
            "authorized executable cannot be measured",
        )
    })?;
    if executable_digest != operation.executable_sha256 {
        return Err(RunnerFailure::no_effect(
            "executable_identity_changed",
            "authorized executable content changed before launch",
        ));
    }
    let stdin = operation
        .stdin_base64
        .as_deref()
        .map(|value| decode_canonical_base64(value, PROCESS_RUNNER_STREAM_MAX_BYTES))
        .transpose()
        .map_err(|()| RunnerFailure::denied("stdin_invalid", "stdin is not canonical base64"))?;

    let mut command = Command::new(&operation.executable);
    command
        .args(&operation.argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for variable in &operation.environment {
        command.env(&variable.name, &variable.value);
    }
    let mut command = CommandWrap::from(command);
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(JobObject);
    let mut child = command.spawn().map_err(|error| {
        RunnerFailure::no_effect("process_launch_failed", format!("launch process: {error}"))
    })?;
    let (child_stdin, Some(stdout), Some(stderr)) = (
        child.stdin().take(),
        child.stdout().take(),
        child.stderr().take(),
    ) else {
        let _ = Box::into_pin(child.kill()).await;
        let _ = child.wait().await;
        return Err(RunnerFailure::unknown(
            "process_pipe_unavailable",
            "process launched without its exact pipe contract",
        ));
    };
    let limit = usize::try_from(operation.max_output_bytes).map_err(|_| {
        RunnerFailure::unknown("output_bound_invalid", "process output bound is invalid")
    })?;
    let mut stdout_task = tokio::spawn(read_bounded(stdout, limit));
    let mut stderr_task = tokio::spawn(read_bounded(stderr, limit));
    let deadline = Instant::now() + Duration::from_millis(operation.timeout_ms);
    if let (Some(mut child_stdin), Some(input)) = (child_stdin, stdin) {
        let delivered = tokio::time::timeout_at(deadline, async {
            child_stdin.write_all(&input).await?;
            child_stdin.shutdown().await
        })
        .await;
        if !matches!(delivered, Ok(Ok(()))) {
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(RunnerFailure::unknown(
                "stdin_delivery_unknown",
                "process launched but stdin delivery did not complete",
            ));
        }
    }
    let status = match tokio::time::timeout_at(deadline, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => {
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(RunnerFailure::unknown(
                "process_wait_failed",
                "process launched but its final state is unavailable",
            ));
        }
        Err(_) => {
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(RunnerFailure::unknown(
                "process_timeout",
                "process exceeded its deadline and its supervised tree was terminated",
            ));
        }
    };
    let Ok(Ok(((stdout, stdout_truncated), (stderr, stderr_truncated)))) =
        tokio::time::timeout_at(deadline, async {
            Ok::<_, ProcessRunnerClientError>((
                join_reader(&mut stdout_task).await?,
                join_reader(&mut stderr_task).await?,
            ))
        })
        .await
    else {
        stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
        return Err(RunnerFailure::unknown(
            "process_output_unknown",
            "process exited but bounded output collection did not complete",
        ));
    };
    let observation = GovernedProcessObservation {
        executable_sha256: executable_digest,
        exit_code: status.code(),
        stdout_base64: STANDARD.encode(stdout),
        stderr_base64: STANDARD.encode(stderr),
        stdout_truncated,
        stderr_truncated,
    };
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_PROCESS_OBSERVATION_TYPE)
            .map_err(|error| RunnerFailure::unknown("observation_invalid", error.to_string()))?,
        &serde_json::to_value(observation)
            .map_err(|error| RunnerFailure::unknown("observation_invalid", error.to_string()))?,
    )
    .map_err(|error| RunnerFailure::unknown("observation_invalid", error.to_string()))
}

fn canonical_regular_directory(path: &Path, code: &'static str) -> Result<PathBuf, RunnerFailure> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| RunnerFailure::denied(code, error.to_string()))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(RunnerFailure::denied(code, "path is not a real directory"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| RunnerFailure::denied(code, error.to_string()))?;
    if canonical != path {
        return Err(RunnerFailure::denied(
            code,
            "path is not the exact canonical directory",
        ));
    }
    Ok(canonical)
}

#[allow(clippy::too_many_lines)]
fn validate_tool_authorization(
    request: &ProcessRunnerRequest,
    now: DateTime<Utc>,
) -> Result<(), RunnerFailure> {
    let authorization = &request.authorization;
    authorization
        .manifest
        .validate()
        .map_err(|error| RunnerFailure::denied("tool_manifest_invalid", error.to_string()))?;
    authorization
        .proposal
        .validate()
        .map_err(|error| RunnerFailure::denied("tool_proposal_invalid", error.to_string()))?;
    authorization
        .policy
        .validate()
        .map_err(|error| RunnerFailure::denied("tool_policy_invalid", error.to_string()))?;
    authorization
        .permit
        .validate()
        .map_err(|error| RunnerFailure::denied("tool_permit_invalid", error.to_string()))?;
    authorization
        .permit_ref
        .validate()
        .map_err(|error| RunnerFailure::denied("tool_permit_ref_invalid", error.to_string()))?;
    let authority = validate_governed_process_envelope(&request.envelope, now)
        .map_err(|error| RunnerFailure::denied("action_envelope_invalid", error.to_string()))?;
    let operation: GovernedProcessOperation = decode_payload(&request.operation)
        .map_err(|error| RunnerFailure::denied("operation_invalid", error.to_string()))?;
    let manifest_ref = authorization
        .manifest
        .versioned_ref()
        .map_err(|error| RunnerFailure::denied("tool_manifest_invalid", error.to_string()))?;
    let proposal_hash = tool_invocation_proposal_hash(&authorization.proposal)
        .map_err(|error| RunnerFailure::denied("tool_proposal_invalid", error.to_string()))?;
    let permit_hash = tool_invocation_permit_hash(&authorization.permit)
        .map_err(|error| RunnerFailure::denied("tool_permit_invalid", error.to_string()))?;
    let policy_binding = tool_policy_binding(&authorization.policy)
        .map_err(|error| RunnerFailure::denied("tool_policy_invalid", error))?;
    let expected_runner = runner_executable_source(&std::env::current_exe().map_err(|error| {
        RunnerFailure::denied("runner_identity_unavailable", error.to_string())
    })?)
    .map_err(|_| {
        RunnerFailure::denied(
            "runner_identity_unavailable",
            "process runner identity cannot be measured",
        )
    })?;
    let root = request.project_root.to_str().ok_or_else(|| {
        RunnerFailure::denied("project_root_invalid", "project root is not Unicode")
    })?;
    let expected_capabilities = &authorization.manifest.required_capabilities;
    let operation_hash = request
        .operation
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| RunnerFailure::denied("operation_invalid", error.to_string()))?;
    let expected_limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_output_bytes,
        max_memory_bytes: 0,
        max_child_processes: 64,
    };

    if request.action_id != request.envelope.id
        || request.operation != request.envelope.operation
        || request.operation_hash != request.envelope.operation_hash
        || request.operation_hash != operation_hash
        || request.project_root != authority.project_root
        || authorization.proposal.action_id != request.envelope.id
        || authorization.permit.action_id != request.envelope.id
        || authorization.proposal.attempt_id != request.envelope.attempt_id
        || authorization.proposal.runtime_actor != request.envelope.actor
        || authorization.proposal.task_contract != request.envelope.task_contract
        || authorization.proposal.argument_hash != request.envelope.operation_hash
        || authorization.proposal.requested_effect_class != EffectClass::Irreversible
        || authorization.proposal.tool_manifest != manifest_ref
        || authorization.permit.tool_manifest != manifest_ref
        || authorization.proposal_hash != proposal_hash
        || authorization.permit.proposal_hash != proposal_hash
        || authorization.permit_hash != permit_hash
        || authorization.permit.argument_hash != request.envelope.operation_hash
        || authorization.permit.allowed_effect_class != EffectClass::Irreversible
        || authorization.permit.capabilities != *expected_capabilities
        || authorization.proposal.requested_capabilities != *expected_capabilities
        || authorization.permit.limits != expected_limits
        || authorization.proposal.requested_limits != expected_limits
        || authorization.manifest.resource_ceiling != expected_limits
        || authorization.proposal.required_sandbox != authorization.manifest.sandbox_requirement
        || authorization.permit.policy != policy_binding
        || authorization.permit.platform != current_execution_platform()
        || authorization.proposal.platform != current_execution_platform()
        || !authorization
            .manifest
            .platform_support
            .contains(&current_execution_platform())
        || !process_runner_sandbox_profile().satisfies(authorization.manifest.sandbox_requirement)
        || !authorization
            .permit
            .sandbox
            .satisfies(authorization.manifest.sandbox_requirement)
        || authorization.permit.issued_at > now
        || authorization.permit.expires_at <= now
        || authorization.proposal.deadline != authorization.permit.expires_at
        || !authorization.permit.secret_handles.is_empty()
        || expected_capabilities.filesystem.as_slice()
            != [FilesystemCapability {
                root: root.into(),
                access: FilesystemAccess::Read,
            }]
        || !expected_capabilities.network.is_empty()
        || !expected_capabilities.secret_slots.is_empty()
        || !expected_capabilities.process_spawn
        || authorization.manifest.runner != expected_runner
        || authorization.manifest.reconciler.as_ref() != Some(&process_runner_reconciler_source())
        || authorization.permit_ref.source_type.as_str() != ToolInvocationPermit::TYPE_ID
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != permit_hash
    {
        return Err(RunnerFailure::denied(
            "tool_authority_mismatch",
            "request differs from the exact Root-issued process Tool authority",
        ));
    }
    Ok(())
}

/// Builds a self-contained exact-authority request for runner process qualification.
/// Production dispatch must instead use Root-persisted authorization.
///
/// # Errors
/// Returns an error when package paths, canonical authority, or derived contracts are invalid.
#[allow(clippy::too_many_lines)] // Keep the self-contained package proof explicit and reviewable.
pub fn qualification_request_with_evidence(
    root: &Path,
    operation: &GovernedProcessOperation,
    evidence: &ProcessRunnerCapabilityEvidence,
) -> Result<ProcessRunnerRequest, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let now = Utc::now();
    let deadline = now
        + chrono::Duration::milliseconds(
            i64::try_from(operation.timeout_ms.saturating_add(5_000))
                .map_err(|error| error.to_string())?,
        );
    let action_id = ActionId::new();
    let actor = RuntimeActorId::new();
    let (payload, operation_hash) = governed_process_operation_payload(operation)?;
    let limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_output_bytes,
        max_memory_bytes: 0,
        max_child_processes: 64,
    };
    let manifest = governed_process_tool_manifest(
        evidence,
        &root,
        &operation.executable,
        &operation.executable_sha256,
        limits,
    )?;
    let policy = governed_process_tool_policy(&manifest)?;
    let authority = governed_process_authority_binding(&GovernedProcessAuthority {
        action_id,
        actor,
        project_root: root.clone(),
        operation_hash: operation_hash.clone(),
        expires_at: deadline,
    })?;
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: mcg_contracts::WorkflowId::new(),
        attempt_id: mcg_contracts::AttemptId::new(),
        task_contract: VersionedRef {
            id: TaskContractId::new(),
            version: 1,
            content_hash: operation_hash.clone(),
        },
        actor,
        origin: None,
        authority,
        elevated_authority: Some(tool_policy_binding(&policy)?),
        expected_versions: Vec::new(),
        operation: payload,
        operation_hash,
        effect_class: EffectClass::Irreversible,
        preconditions: Vec::new(),
        budget_reservation: tool_policy_binding(&policy)?,
        idempotency_key: format!("process-runner-qualification-{action_id}"),
        compensation: None,
        compensates_action: None,
        reconciler: Some(governed_process_reconciler(&operation.executable_sha256)?),
        reconciliation_deadline: Some(deadline),
        created_at: now,
    };
    let execution = ActionExecution {
        id: ActionExecutionId::new(),
        action_id,
        attempt_number: 1,
        state: ActionExecutionState::Executing,
        revision: 2,
        lease_generation: 1,
        executor: Some(actor),
        outcome: None,
        created_at: now,
        updated_at: now,
    };
    let runtime_identity = RuntimeIdentity {
        instance_id: RuntimeInstanceId::new(),
        build_hash: evidence.runner.content_hash.clone(),
        protocol_version: PROTOCOL_V1,
        started_at: now - chrono::Duration::seconds(1),
    };
    let proposal = ToolInvocationProposal {
        action_id,
        attempt_id: envelope.attempt_id,
        attempt_revision: 1,
        lease_generation: 1,
        runtime_actor: actor,
        runtime_identity: runtime_identity.clone(),
        task_contract: envelope.task_contract.clone(),
        tool_manifest: manifest
            .versioned_ref()
            .map_err(|error| error.to_string())?,
        arguments: ArtifactRef {
            content_hash: envelope.operation_hash.clone(),
            media_type: "application/vnd.metacog.governed-process-operation+cbor".into(),
            byte_length: u64::try_from(envelope.operation.bytes.len())
                .map_err(|error| error.to_string())?,
        },
        argument_hash: envelope.operation_hash.clone(),
        requested_effect_class: EffectClass::Irreversible,
        requested_capabilities: manifest.required_capabilities.clone(),
        requested_limits: limits,
        required_sandbox: manifest.sandbox_requirement,
        platform: evidence.platform,
        deadline,
    };
    let proposal_hash =
        tool_invocation_proposal_hash(&proposal).map_err(|error| error.to_string())?;
    let permit = ToolInvocationPermit {
        id: PermitId::new(),
        action_id,
        attempt_id: proposal.attempt_id,
        attempt_revision: proposal.attempt_revision,
        lease_generation: proposal.lease_generation,
        runtime_actor: actor,
        runtime_identity,
        task_contract: proposal.task_contract.clone(),
        tool_manifest: proposal.tool_manifest.clone(),
        proposal_hash: proposal_hash.clone(),
        argument_hash: proposal.argument_hash.clone(),
        allowed_effect_class: EffectClass::Irreversible,
        capabilities: proposal.requested_capabilities.clone(),
        limits,
        sandbox: evidence.available_sandbox,
        platform: evidence.platform,
        policy: tool_policy_binding(&policy)?,
        expires_at: deadline,
        secret_handles: Vec::new(),
        issued_at: now,
    };
    let permit_hash = tool_invocation_permit_hash(&permit).map_err(|error| error.to_string())?;
    let authorization = ProcessRunnerAuthorization {
        manifest,
        proposal,
        proposal_hash,
        policy,
        permit: permit.clone(),
        permit_hash: permit_hash.clone(),
        permit_ref: SourceRef {
            source_type: StableTypeId::parse(ToolInvocationPermit::TYPE_ID)
                .map_err(|error| error.to_string())?,
            locator: permit.id.to_string(),
            content_hash: permit_hash,
        },
    };
    request_from_authorized_action(&envelope, &execution, authorization, Uuid::new_v4(), now)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_executable() -> (PathBuf, Vec<String>) {
        #[cfg(windows)]
        {
            let executable = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            (
                executable,
                vec!["/d".into(), "/c".into(), "echo governed".into()],
            )
        }
        #[cfg(unix)]
        {
            let executable = PathBuf::from("/bin/sh");
            (executable, vec!["-c".into(), "printf governed".into()])
        }
    }

    fn fixture_request(root: &Path) -> ProcessRunnerRequest {
        let current = std::env::current_exe().expect("test executable");
        let evidence = ProcessRunnerCapabilityEvidence {
            runner: runner_executable_source(&current).expect("runner evidence"),
            platform: current_execution_platform(),
            available_sandbox: process_runner_sandbox_profile(),
            resource_ceiling: ToolResourceLimits {
                timeout_ms: 10_000,
                max_output_bytes: 64 * 1024,
                max_memory_bytes: 0,
                max_child_processes: 64,
            },
        };
        let (executable, argv) = fixture_executable();
        let operation = GovernedProcessOperation {
            executable: executable.clone(),
            executable_sha256: sha256_file(&executable).expect("executable digest"),
            argv,
            cwd_relative: "work".into(),
            environment: vec![GovernedProcessEnvironmentVariable {
                name: "MCG_PROCESS_PROOF".into(),
                value: "literal-only".into(),
            }],
            stdin_base64: None,
            timeout_ms: 10_000,
            max_output_bytes: 64 * 1024,
        };
        qualification_request_with_evidence(root, &operation, &evidence)
            .expect("qualification request")
    }

    #[tokio::test]
    async fn exact_authority_executes_and_captures_output() {
        let root = tempfile::tempdir().expect("root");
        std::fs::create_dir(root.path().join("work")).expect("work directory");
        let request = fixture_request(root.path());
        let response = execute_request(&serde_json::to_vec(&request).expect("request JSON")).await;
        assert_eq!(
            response.outcome,
            ProcessRunnerOutcome::EffectConfirmed,
            "{}: {}",
            response.error_code.as_deref().unwrap_or("no_code"),
            response.detail
        );
        let observation = decode_governed_process_observation(
            response.observation.as_ref().expect("observation"),
        )
        .expect("decode observation");
        let stdout = STANDARD
            .decode(observation.stdout_base64)
            .expect("decode stdout");
        assert!(String::from_utf8_lossy(&stdout).contains("governed"));
    }

    #[tokio::test]
    async fn substituted_argument_is_denied_before_launch() {
        let root = tempfile::tempdir().expect("root");
        std::fs::create_dir(root.path().join("work")).expect("work directory");
        let mut request = fixture_request(root.path());
        request.operation_hash.digest_hex = "0".repeat(64);
        let response = execute_request(&serde_json::to_vec(&request).expect("request JSON")).await;
        assert_eq!(response.outcome, ProcessRunnerOutcome::Denied);
        assert_eq!(
            response.error_code.as_deref(),
            Some("tool_authority_mismatch")
        );
    }

    #[test]
    fn loader_environment_and_noncanonical_stdin_are_rejected() {
        let (executable, argv) = fixture_executable();
        let mut operation = GovernedProcessOperation {
            executable: executable.clone(),
            executable_sha256: sha256_file(&executable).expect("digest"),
            argv,
            cwd_relative: "work".into(),
            environment: vec![GovernedProcessEnvironmentVariable {
                name: "PATH".into(),
                value: "ambient".into(),
            }],
            stdin_base64: None,
            timeout_ms: 1_000,
            max_output_bytes: 1_024,
        };
        assert!(validate_operation(&operation).is_err());
        operation.environment.clear();
        operation.stdin_base64 = Some("not base64".into());
        assert!(validate_operation(&operation).is_err());
    }
}
