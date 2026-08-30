#[cfg(feature = "executor")]
use std::io::{Read as _, Write as _};
#[cfg(feature = "executor")]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::{process::Stdio, time::Duration};

use base64::Engine as _;
#[cfg(feature = "executor")]
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt as _};
#[cfg(feature = "executor")]
use cap_std::ambient_authority;
#[cfg(feature = "executor")]
use cap_std::fs::{Dir, OpenOptions};
use chrono::{DateTime, Utc};
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId,
    ContentHash, EffectClass, PolicyBinding, ReconciliationJob, ReconciliationJobState,
    RuntimeActorId, StableTypeId, VersionedPayload, decode_canonical_cbor_v1,
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
use unicode_normalization::UnicodeNormalization as _;
use uuid::Uuid;

pub const ACTION_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const ACTION_RUNNER_REQUEST_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const ACTION_RUNNER_RESPONSE_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const ACTION_RUNNER_CONTENT_MAX_BYTES: u64 = 1024 * 1024;
pub const GOVERNED_ACTION_OPERATION_HASH_DOMAIN: &str = "MetaCog.ActionOperation";
pub const GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN: &str = "MetaCog.FilesystemAuthority";
pub const GOVERNED_FILESYSTEM_AUTHORITY_TYPE: &str = "mcg.governed_filesystem_authority.v1";
pub const GOVERNED_FILESYSTEM_OPERATION_TYPE: &str = "mcg.governed_filesystem_operation.v1";
pub const GOVERNED_FILESYSTEM_OBSERVATION_TYPE: &str = "mcg.governed_filesystem_observation.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub capability_root: PathBuf,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GovernedFilesystemOperation {
    ReadFile {
        relative_path: String,
        max_bytes: u64,
    },
    CreateFile {
        relative_path: String,
        content_base64: String,
    },
    RemoveCreatedFile {
        relative_path: String,
        expected_sha256: String,
    },
    InspectFile {
        relative_path: String,
        max_bytes: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedFilesystemAuthority {
    pub action_id: ActionId,
    pub actor: RuntimeActorId,
    pub capability_root: PathBuf,
    pub operation_hash: ContentHash,
    pub effect_class: EffectClass,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ActionRunnerProtocolError {
    #[error("governed filesystem action is invalid: {0}")]
    Invalid(&'static str),
    #[error("governed filesystem canonical payload is invalid")]
    Canonical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerEffectOutcome {
    Observational,
    NoEffectConfirmed,
    EffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedFilesystemObservation {
    pub relative_path: String,
    pub exists: bool,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub content_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: RunnerEffectOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

const ACTION_RUNNER_STDERR_MAX_BYTES: usize = 4 * 1024;
const ACTION_RUNNER_DETAIL_MAX_BYTES: usize = 4 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ActionRunnerClientError {
    #[error("action runner is unavailable")]
    Unavailable,
    #[error("action runner request is too large")]
    RequestTooLarge,
    #[error("action runner process could not be launched")]
    Launch,
    #[error("action runner request delivery failed")]
    Delivery,
    #[error("action runner timed out")]
    Timeout,
    #[error("action runner response exceeded its bound")]
    ResponseTooLarge,
    #[error("action runner wrote to its diagnostic channel")]
    DiagnosticOutput,
    #[error("action runner returned an invalid response")]
    InvalidResponse,
    #[error("action runner process failed")]
    Process,
}

impl ActionRunnerClientError {
    /// Returns whether physical execution may have started before the failure.
    #[must_use]
    pub const fn effect_may_have_occurred(&self) -> bool {
        !matches!(
            self,
            Self::Unavailable | Self::RequestTooLarge | Self::Launch
        )
    }
}

#[derive(Debug, Clone)]
pub struct ActionRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
}

impl ActionRunnerClient {
    /// Selects only the fixed runner name adjacent to a trusted application binary.
    ///
    /// # Errors
    ///
    /// Returns unavailable for a missing, linked, reparse, non-file, or wrongly named runner.
    pub fn adjacent_to(
        application_executable: impl AsRef<Path>,
    ) -> Result<Self, ActionRunnerClientError> {
        let parent = application_executable
            .as_ref()
            .parent()
            .ok_or(ActionRunnerClientError::Unavailable)?;
        Self::with_executable(parent.join(runner_file_name()), Duration::from_secs(30))
    }

    /// Creates a client for an exact runner path and bounded deadline.
    ///
    /// This constructor is intended for trusted package bootstrap and process-contract tests,
    /// never a user-selected executable.
    ///
    /// # Errors
    ///
    /// Returns unavailable unless the path is the fixed real adjacent runner filename and the
    /// outer timeout is between one millisecond and two minutes.
    pub fn with_executable(
        executable: impl AsRef<Path>,
        outer_timeout: Duration,
    ) -> Result<Self, ActionRunnerClientError> {
        let executable = executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name()))
            || outer_timeout.is_zero()
            || outer_timeout > Duration::from_mins(2)
        {
            return Err(ActionRunnerClientError::Unavailable);
        }
        let metadata = std::fs::symlink_metadata(executable)
            .map_err(|_| ActionRunnerClientError::Unavailable)?;
        if !metadata.file_type().is_file() || is_link_or_reparse(&metadata) {
            return Err(ActionRunnerClientError::Unavailable);
        }
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
        })
    }

    /// Executes exactly one request in a cleared-environment child and validates its identity.
    ///
    /// # Errors
    ///
    /// Returns a typed bounded launch, delivery, timeout, pipe, exit, or response-contract
    /// failure. Call [`ActionRunnerClientError::effect_may_have_occurred`] before classifying a
    /// failed dispatched action.
    pub async fn invoke(
        &self,
        request: &ActionRunnerRequest,
    ) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
        let request_bytes =
            serde_json::to_vec(request).map_err(|_| ActionRunnerClientError::InvalidResponse)?;
        if request_bytes.len() > ACTION_RUNNER_REQUEST_MAX_BYTES {
            return Err(ActionRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command
            .env_clear()
            .current_dir(
                self.executable
                    .parent()
                    .ok_or(ActionRunnerClientError::Unavailable)?,
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
            .map_err(|_| ActionRunnerClientError::Launch)?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (
            child.stdin().take(),
            child.stdout().take(),
            child.stderr().take(),
        ) else {
            let _ = Box::into_pin(child.kill()).await;
            let _ = child.wait().await;
            return Err(ActionRunnerClientError::Launch);
        };
        let mut stdout_task = tokio::spawn(read_bounded(stdout, ACTION_RUNNER_RESPONSE_MAX_BYTES));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, ACTION_RUNNER_STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        match tokio::time::timeout_at(deadline, async {
            stdin.write_all(&request_bytes).await?;
            stdin.shutdown().await
        })
        .await
        {
            Ok(Ok(())) => {}
            delivery => {
                drop(stdin);
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(if delivery.is_err() {
                    ActionRunnerClientError::Timeout
                } else {
                    ActionRunnerClientError::Delivery
                });
            }
        }
        drop(stdin);
        let status = match tokio::time::timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ActionRunnerClientError::Process);
            }
            Err(_) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ActionRunnerClientError::Timeout);
            }
        };
        let ((stdout, stdout_overflow), (stderr, stderr_overflow)) =
            match tokio::time::timeout_at(deadline, async {
                let stdout = join_reader(&mut stdout_task).await?;
                let stderr = join_reader(&mut stderr_task).await?;
                Ok::<_, ActionRunnerClientError>((stdout, stderr))
            })
            .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => {
                    stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                    return Err(error);
                }
                Err(_) => {
                    stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                    return Err(ActionRunnerClientError::Timeout);
                }
            };
        if stdout_overflow || stderr_overflow {
            return Err(ActionRunnerClientError::ResponseTooLarge);
        }
        if !stderr.is_empty() {
            return Err(ActionRunnerClientError::DiagnosticOutput);
        }
        validate_client_response(status.success(), request, &stdout)
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
) -> Result<(Vec<u8>, bool), ActionRunnerClientError> {
    task.await
        .map_err(|_| ActionRunnerClientError::Process)?
        .map_err(|_| ActionRunnerClientError::Process)
}

async fn stop_child(
    child: &mut dyn ChildWrapper,
    stdout_task: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stderr_task: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
) {
    let _ = Box::into_pin(child.kill()).await;
    let _ = child.wait().await;
    stdout_task.abort();
    stderr_task.abort();
}

fn validate_client_response(
    process_succeeded: bool,
    request: &ActionRunnerRequest,
    bytes: &[u8],
) -> Result<ActionRunnerResponse, ActionRunnerClientError> {
    let response: ActionRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ActionRunnerClientError::InvalidResponse)?;
    let outcome_succeeded = matches!(
        response.outcome,
        RunnerEffectOutcome::Observational
            | RunnerEffectOutcome::NoEffectConfirmed
            | RunnerEffectOutcome::EffectConfirmed
    );
    if response.protocol_version != ACTION_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.action_id != Some(request.action_id)
        || response.execution_id != Some(request.execution_id)
        || process_succeeded != outcome_succeeded
        || response.detail.is_empty()
        || response.detail.len() > ACTION_RUNNER_DETAIL_MAX_BYTES
        || response.started_at > response.finished_at
    {
        return Err(ActionRunnerClientError::InvalidResponse);
    }
    let contract_valid = match response.outcome {
        RunnerEffectOutcome::Observational | RunnerEffectOutcome::EffectConfirmed => {
            response.observation.is_some() && response.error_code.is_none()
        }
        RunnerEffectOutcome::NoEffectConfirmed
        | RunnerEffectOutcome::OutcomeUnknown
        | RunnerEffectOutcome::Denied => {
            response.observation.is_none() && response.error_code.as_deref().is_some_and(valid_code)
        }
    };
    if !contract_valid {
        return Err(ActionRunnerClientError::InvalidResponse);
    }
    if let Some(observation) = &response.observation
        && (observation.type_id.as_str() != GOVERNED_FILESYSTEM_OBSERVATION_TYPE
            || decode_canonical_cbor_v1(&observation.bytes).is_err())
    {
        return Err(ActionRunnerClientError::InvalidResponse);
    }
    Ok(response)
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

const fn runner_file_name() -> &'static str {
    if cfg!(windows) {
        "mcg-action-runner.exe"
    } else {
        "mcg-action-runner"
    }
}

#[cfg(feature = "executor")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemRunnerQualification {
    pub protocol_version: u32,
    pub created: RunnerEffectOutcome,
    pub reconciled: RunnerEffectOutcome,
    pub duplicate: RunnerEffectOutcome,
    pub compensated: RunnerEffectOutcome,
    pub final_absent: bool,
}

/// Canonically encodes a governed filesystem operation and binds its fixed hash domain.
///
/// # Errors
///
/// Returns an error when the operation cannot be serialized, canonically encoded, or hashed.
pub fn governed_filesystem_operation_payload(
    operation: &GovernedFilesystemOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    let value = serde_json::to_value(operation).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_OPERATION_TYPE)
            .map_err(|error| error.to_string())?,
        &value,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Canonically encodes one exact filesystem authority permit.
///
/// # Errors
///
/// Returns an error when the permit cannot be serialized, canonically encoded, or hashed.
pub fn governed_filesystem_authority_binding(
    authority: &GovernedFilesystemAuthority,
) -> Result<PolicyBinding, String> {
    let value = serde_json::to_value(authority).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_AUTHORITY_TYPE)
            .map_err(|error| error.to_string())?,
        &value,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Validates the exact filesystem authority, operation, reconciliation, and compensation shape.
///
/// # Errors
///
/// Returns an error when any canonical binding is missing, expired, substituted, or inconsistent
/// with the operation's effect class.
pub fn validate_governed_filesystem_envelope(
    envelope: &ActionEnvelope,
    now: DateTime<Utc>,
) -> Result<GovernedFilesystemAuthority, ActionRunnerProtocolError> {
    if envelope.operation.type_id.as_str() != GOVERNED_FILESYSTEM_OPERATION_TYPE
        || envelope.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN
        || envelope
            .operation
            .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
            .map_err(|_| ActionRunnerProtocolError::Canonical)?
            != envelope.operation_hash
    {
        return Err(ActionRunnerProtocolError::Invalid("operation binding"));
    }
    if envelope.authority.payload.type_id.as_str() != GOVERNED_FILESYSTEM_AUTHORITY_TYPE
        || envelope.authority.content_hash.domain != GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_FILESYSTEM_AUTHORITY_HASH_DOMAIN)
            .map_err(|_| ActionRunnerProtocolError::Canonical)?
            != envelope.authority.content_hash
    {
        return Err(ActionRunnerProtocolError::Invalid("authority binding"));
    }
    let authority: GovernedFilesystemAuthority = decode_payload(&envelope.authority.payload)?;
    if authority.action_id != envelope.id
        || authority.actor != envelope.actor
        || authority.operation_hash != envelope.operation_hash
        || authority.effect_class != envelope.effect_class
        || !authority.capability_root.is_absolute()
        || authority.expires_at <= now
    {
        return Err(ActionRunnerProtocolError::Invalid("authority scope"));
    }
    let operation: GovernedFilesystemOperation = decode_payload(&envelope.operation)?;
    validate_operation_plan(envelope, &operation)?;
    Ok(authority)
}

/// Builds one runner request exclusively from immutable authorized intent and dispatched state.
///
/// # Errors
///
/// Returns an error unless the action is currently executing and every authority, identity,
/// operation, effect, reconciliation, and compensation binding is exact.
pub fn request_from_authorized_action(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<ActionRunnerRequest, ActionRunnerProtocolError> {
    let authority = validate_governed_filesystem_envelope(envelope, now)?;
    if execution.action_id != envelope.id
        || execution.state != ActionExecutionState::Executing
        || execution.executor.is_none()
    {
        return Err(ActionRunnerProtocolError::Invalid("execution binding"));
    }
    Ok(ActionRunnerRequest {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        capability_root: authority.capability_root,
        operation: envelope.operation.clone(),
        operation_hash: envelope.operation_hash.clone(),
    })
}

/// Builds one observational request from an exact, actively leased reconciliation job.
///
/// The original effect operation is never reused. The request carries only the reconciler that
/// was committed in the immutable action envelope, and its operation hash is recomputed in the
/// fixed action-operation domain.
///
/// # Errors
///
/// Returns an error unless the action authority remains valid, the job and execution identities
/// and leases agree, and the job adapter is exactly the envelope's authorized reconciler.
pub fn request_from_reconciliation_job(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    job: &ReconciliationJob,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<ActionRunnerRequest, ActionRunnerProtocolError> {
    let authority = validate_governed_filesystem_envelope(envelope, now)?;
    let reconciler = envelope
        .reconciler
        .as_ref()
        .ok_or(ActionRunnerProtocolError::Invalid("filesystem reconciler"))?;
    if execution.action_id != envelope.id
        || execution.id != job.execution_id
        || job.action_id != envelope.id
        || job.adapter != *reconciler
        || job.state != ReconciliationJobState::Running
        || job.lease_owner.is_none()
        || job.lease_owner != execution.executor
        || job.lease_expires_at.is_none_or(|expiry| expiry <= now)
        || execution.state != ActionExecutionState::Reconciling
    {
        return Err(ActionRunnerProtocolError::Invalid("reconciliation binding"));
    }
    let operation = decode_filesystem_operation(reconciler)?;
    if !matches!(operation, GovernedFilesystemOperation::InspectFile { .. }) {
        return Err(ActionRunnerProtocolError::Invalid("filesystem reconciler"));
    }
    let operation_hash = reconciler
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|_| ActionRunnerProtocolError::Canonical)?;
    Ok(ActionRunnerRequest {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        capability_root: authority.capability_root,
        operation: reconciler.clone(),
        operation_hash,
    })
}

/// Decodes one canonical governed-filesystem operation.
///
/// # Errors
/// Returns an error for an unexpected type or noncanonical payload.
pub fn decode_governed_filesystem_operation(
    payload: &VersionedPayload,
) -> Result<GovernedFilesystemOperation, ActionRunnerProtocolError> {
    decode_filesystem_operation(payload)
}

/// Decodes one canonical governed-filesystem observation.
///
/// # Errors
/// Returns an error for an unexpected type or noncanonical payload.
pub fn decode_governed_filesystem_observation(
    payload: &VersionedPayload,
) -> Result<GovernedFilesystemObservation, ActionRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_FILESYSTEM_OBSERVATION_TYPE {
        return Err(ActionRunnerProtocolError::Invalid(
            "filesystem observation type",
        ));
    }
    decode_payload(payload)
}

fn decode_payload<T: serde::de::DeserializeOwned>(
    payload: &VersionedPayload,
) -> Result<T, ActionRunnerProtocolError> {
    let value = decode_canonical_cbor_v1(&payload.bytes)
        .map_err(|_| ActionRunnerProtocolError::Canonical)?;
    serde_json::from_value(value).map_err(|_| ActionRunnerProtocolError::Canonical)
}

fn validate_operation_plan(
    envelope: &ActionEnvelope,
    operation: &GovernedFilesystemOperation,
) -> Result<(), ActionRunnerProtocolError> {
    let relative_path = match operation {
        GovernedFilesystemOperation::ReadFile {
            relative_path,
            max_bytes,
        }
        | GovernedFilesystemOperation::InspectFile {
            relative_path,
            max_bytes,
        } => {
            if *max_bytes == 0 || *max_bytes > ACTION_RUNNER_CONTENT_MAX_BYTES {
                return Err(ActionRunnerProtocolError::Invalid("filesystem read bound"));
            }
            relative_path
        }
        GovernedFilesystemOperation::CreateFile { relative_path, .. }
        | GovernedFilesystemOperation::RemoveCreatedFile { relative_path, .. } => relative_path,
    };
    if !normalized_relative_path(relative_path) {
        return Err(ActionRunnerProtocolError::Invalid("filesystem path"));
    }
    match operation {
        GovernedFilesystemOperation::ReadFile { .. }
        | GovernedFilesystemOperation::InspectFile { .. } => {
            if envelope.effect_class != EffectClass::Observational
                || envelope.compensation.is_some()
                || envelope.compensates_action.is_some()
            {
                return Err(ActionRunnerProtocolError::Invalid(
                    "observational effect plan",
                ));
            }
        }
        GovernedFilesystemOperation::CreateFile {
            relative_path,
            content_base64,
        } => validate_create_plan(envelope, relative_path, content_base64)?,
        GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path,
            expected_sha256,
        } => {
            if envelope.effect_class != EffectClass::Reversible
                || envelope.compensates_action.is_none()
                || envelope.compensation.is_some()
                || !valid_sha256(expected_sha256)
            {
                return Err(ActionRunnerProtocolError::Invalid(
                    "compensation effect plan",
                ));
            }
            validate_reconciler(envelope, relative_path)?;
        }
    }
    Ok(())
}

fn normalized_relative_path(value: &str) -> bool {
    const MAX_PATH_BYTES: usize = 4_096;
    const MAX_COMPONENT_BYTES: usize = 255;

    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.nfc().ne(value.chars())
    {
        return false;
    }
    value.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component.len() <= MAX_COMPONENT_BYTES
            && !component.ends_with(['.', ' '])
            && !component.chars().any(|character| {
                character.is_ascii_control()
                    || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
            })
            && !windows_reserved_component(component)
    })
}

fn windows_reserved_component(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_create_plan(
    envelope: &ActionEnvelope,
    relative_path: &str,
    content_base64: &str,
) -> Result<(), ActionRunnerProtocolError> {
    if envelope.effect_class != EffectClass::Reversible || envelope.compensates_action.is_some() {
        return Err(ActionRunnerProtocolError::Invalid("create effect plan"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content_base64)
        .map_err(|_| ActionRunnerProtocolError::Invalid("create content"))?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != content_base64
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > ACTION_RUNNER_CONTENT_MAX_BYTES
    {
        return Err(ActionRunnerProtocolError::Invalid("create content"));
    }
    let compensation: GovernedFilesystemOperation = envelope
        .compensation
        .as_ref()
        .ok_or(ActionRunnerProtocolError::Invalid("create compensation"))
        .and_then(decode_filesystem_operation)?;
    if compensation
        != (GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path: relative_path.into(),
            expected_sha256: sha256(&bytes),
        })
    {
        return Err(ActionRunnerProtocolError::Invalid("create compensation"));
    }
    validate_reconciler(envelope, relative_path)
}

fn validate_reconciler(
    envelope: &ActionEnvelope,
    relative_path: &str,
) -> Result<(), ActionRunnerProtocolError> {
    let reconciler = envelope
        .reconciler
        .as_ref()
        .ok_or(ActionRunnerProtocolError::Invalid("filesystem reconciler"))
        .and_then(decode_filesystem_operation)?;
    if !matches!(
        reconciler,
        GovernedFilesystemOperation::InspectFile {
            relative_path: ref candidate,
            max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
        } if candidate == relative_path
    ) {
        return Err(ActionRunnerProtocolError::Invalid("filesystem reconciler"));
    }
    Ok(())
}

fn decode_filesystem_operation(
    payload: &VersionedPayload,
) -> Result<GovernedFilesystemOperation, ActionRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_FILESYSTEM_OPERATION_TYPE {
        return Err(ActionRunnerProtocolError::Invalid(
            "filesystem operation type",
        ));
    }
    decode_payload(payload)
}

/// Exercises the packaged runner in an explicitly supplied empty real directory.
///
/// The qualification creates one unique file, reconciles a deliberately discarded
/// response by inspection, proves duplicate dispatch has no effect, compensates the
/// exact bytes, and verifies that the directory is empty again.
///
/// # Errors
///
/// Returns an error when the root is not an empty real directory, any governed step
/// fails, an observation differs from the expected bytes, or cleanup cannot be proven.
#[cfg(feature = "executor")]
pub fn qualify_filesystem_runner(root: &Path) -> Result<FilesystemRunnerQualification, String> {
    CapabilityRoot::open(root).map_err(|error| error.to_string())?;
    let mut entries = std::fs::read_dir(root).map_err(|error| error.to_string())?;
    if entries
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("qualification root must be empty".into());
    }

    let relative_path = format!("mcg-action-runner-{}.txt", Uuid::new_v4());
    let content = b"MetaCog governed filesystem runner qualification\n";
    let expected_sha256 = sha256(content);
    let create = qualification_request(
        root,
        &GovernedFilesystemOperation::CreateFile {
            relative_path: relative_path.clone(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(content),
        },
    )?;
    let inspect = qualification_request(
        root,
        &GovernedFilesystemOperation::InspectFile {
            relative_path: relative_path.clone(),
            max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
        },
    )?;
    let compensate = qualification_request(
        root,
        &GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path: relative_path.clone(),
            expected_sha256: expected_sha256.clone(),
        },
    )?;

    let qualified = (|| {
        let created = invoke_request(&create)?;
        require_outcome(&created, RunnerEffectOutcome::EffectConfirmed, "create")?;

        let reconciled = invoke_request(&inspect)?;
        require_outcome(
            &reconciled,
            RunnerEffectOutcome::Observational,
            "reconciliation inspection",
        )?;
        let observed = decode_observation(&reconciled)?;
        if !observed.exists || observed.sha256.as_deref() != Some(expected_sha256.as_str()) {
            return Err(
                "qualification reconciliation did not observe the exact created file".into(),
            );
        }

        let duplicate = invoke_request(&create)?;
        require_outcome(
            &duplicate,
            RunnerEffectOutcome::NoEffectConfirmed,
            "duplicate create",
        )?;

        let compensated = invoke_request(&compensate)?;
        require_outcome(
            &compensated,
            RunnerEffectOutcome::EffectConfirmed,
            "compensation",
        )?;
        if decode_observation(&compensated)?.exists {
            return Err("qualification compensation did not leave the target absent".into());
        }

        Ok(FilesystemRunnerQualification {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            created: created.outcome,
            reconciled: reconciled.outcome,
            duplicate: duplicate.outcome,
            compensated: compensated.outcome,
            final_absent: true,
        })
    })();

    if qualified.is_err() && root.join(&relative_path).exists() {
        let _ = invoke_request(&compensate);
    }
    if root.join(&relative_path).exists() {
        return Err("qualification could not restore its empty root".into());
    }
    if std::fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("qualification root is not empty after cleanup".into());
    }
    qualified
}

#[cfg(feature = "executor")]
impl ActionRunnerResponse {
    fn malformed(started_at: DateTime<Utc>, code: &str, detail: impl Into<String>) -> Self {
        Self {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            request_id: None,
            action_id: None,
            execution_id: None,
            outcome: RunnerEffectOutcome::Denied,
            observation: None,
            error_code: Some(code.into()),
            detail: detail.into(),
            started_at,
            finished_at: Utc::now(),
        }
    }
}

#[cfg(feature = "executor")]
#[derive(Debug, thiserror::Error)]
enum RunnerFailure {
    #[error("{detail}")]
    Denied { code: &'static str, detail: String },
    #[error("{detail}")]
    NoEffect { code: &'static str, detail: String },
    #[error("{detail}")]
    Unknown { code: &'static str, detail: String },
}

#[cfg(feature = "executor")]
impl RunnerFailure {
    const fn outcome(&self) -> RunnerEffectOutcome {
        match self {
            Self::Denied { .. } => RunnerEffectOutcome::Denied,
            Self::NoEffect { .. } => RunnerEffectOutcome::NoEffectConfirmed,
            Self::Unknown { .. } => RunnerEffectOutcome::OutcomeUnknown,
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::Denied { code, .. }
            | Self::NoEffect { code, .. }
            | Self::Unknown { code, .. } => code,
        }
    }
}

#[cfg(feature = "executor")]
struct CapabilityRoot {
    canonical: PathBuf,
    directory: Dir,
}

/// Executes one bounded request without retaining authority between invocations.
#[must_use]
#[cfg(feature = "executor")]
pub fn execute_request(bytes: &[u8]) -> ActionRunnerResponse {
    let started_at = Utc::now();
    if bytes.len() > ACTION_RUNNER_REQUEST_MAX_BYTES {
        return ActionRunnerResponse::malformed(
            started_at,
            "request_too_large",
            "action runner request exceeds its byte limit",
        );
    }
    let request: ActionRunnerRequest = match serde_json::from_slice(bytes) {
        Ok(request) => request,
        Err(error) => {
            return ActionRunnerResponse::malformed(
                started_at,
                "request_invalid",
                format!("decode action runner request: {error}"),
            );
        }
    };
    let result = execute(&request);
    let (outcome, observation, error_code, detail) = match result {
        Ok((outcome, observation, detail)) => (outcome, observation, None, detail),
        Err(error) => (
            error.outcome(),
            None,
            Some(error.code().into()),
            error.to_string(),
        ),
    };
    ActionRunnerResponse {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        action_id: Some(request.action_id),
        execution_id: Some(request.execution_id),
        outcome,
        observation,
        error_code,
        detail,
        started_at,
        finished_at: Utc::now(),
    }
}

#[cfg(feature = "executor")]
fn execute(
    request: &ActionRunnerRequest,
) -> Result<(RunnerEffectOutcome, Option<VersionedPayload>, String), RunnerFailure> {
    if request.protocol_version != ACTION_RUNNER_PROTOCOL_VERSION {
        return Err(denied(
            "protocol_unsupported",
            "action runner protocol version is unsupported",
        ));
    }
    if request.operation.type_id.as_str() != GOVERNED_FILESYSTEM_OPERATION_TYPE {
        return Err(denied(
            "operation_type_denied",
            "operation is not a governed filesystem operation",
        ));
    }
    validate_operation_binding(request)?;
    let value = decode_canonical_cbor_v1(&request.operation.bytes)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    let operation: GovernedFilesystemOperation = serde_json::from_value(value)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    let root = CapabilityRoot::open(&request.capability_root)?;
    let (outcome, observation, detail) = execute_operation(&root, operation)?;
    encode_observation(outcome, &observation, detail)
}

#[cfg(feature = "executor")]
fn validate_operation_binding(request: &ActionRunnerRequest) -> Result<(), RunnerFailure> {
    if request.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN {
        return Err(denied(
            "operation_hash_domain_mismatch",
            "operation hash does not use the governed action domain",
        ));
    }
    let actual_hash = request
        .operation
        .content_hash(&request.operation_hash.domain)
        .map_err(|error| denied("operation_invalid", error.to_string()))?;
    if actual_hash != request.operation_hash {
        return Err(denied(
            "operation_hash_mismatch",
            "operation bytes do not match the authorized hash",
        ));
    }
    Ok(())
}

#[cfg(feature = "executor")]
fn execute_operation(
    root: &CapabilityRoot,
    operation: GovernedFilesystemOperation,
) -> Result<(RunnerEffectOutcome, GovernedFilesystemObservation, String), RunnerFailure> {
    Ok(match operation {
        GovernedFilesystemOperation::ReadFile {
            relative_path,
            max_bytes,
        } => {
            let observation = root.inspect(&relative_path, max_bytes, true)?;
            if !observation.exists {
                return Err(no_effect(
                    "file_not_found",
                    "governed read target does not exist",
                ));
            }
            (
                RunnerEffectOutcome::Observational,
                observation,
                "read bounded regular file".into(),
            )
        }
        GovernedFilesystemOperation::InspectFile {
            relative_path,
            max_bytes,
        } => (
            RunnerEffectOutcome::Observational,
            root.inspect(&relative_path, max_bytes, false)?,
            "inspected governed file state".into(),
        ),
        GovernedFilesystemOperation::CreateFile {
            relative_path,
            content_base64,
        } => {
            let content = base64::engine::general_purpose::STANDARD
                .decode(content_base64)
                .map_err(|error| denied("content_invalid", error.to_string()))?;
            if u64::try_from(content.len()).unwrap_or(u64::MAX) > ACTION_RUNNER_CONTENT_MAX_BYTES {
                return Err(denied(
                    "content_too_large",
                    "governed file content exceeds its byte limit",
                ));
            }
            let observation = root.create_new(&relative_path, &content)?;
            (
                RunnerEffectOutcome::EffectConfirmed,
                observation,
                "created governed file exactly once".into(),
            )
        }
        GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path,
            expected_sha256,
        } => {
            validate_sha256(&expected_sha256)?;
            let observation = root.remove_exact(&relative_path, &expected_sha256)?;
            (
                RunnerEffectOutcome::EffectConfirmed,
                observation,
                "removed the exact governed file as compensation".into(),
            )
        }
    })
}

#[cfg(feature = "executor")]
fn encode_observation(
    outcome: RunnerEffectOutcome,
    observation: &GovernedFilesystemObservation,
    detail: String,
) -> Result<(RunnerEffectOutcome, Option<VersionedPayload>, String), RunnerFailure> {
    let observation_value = serde_json::to_value(observation)
        .map_err(|error| denied("observation_invalid", error.to_string()))?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE)
            .map_err(|error| denied("observation_invalid", error.to_string()))?,
        &observation_value,
    )
    .map_err(|error| denied("observation_invalid", error.to_string()))?;
    Ok((outcome, Some(payload), detail))
}

#[cfg(feature = "executor")]
impl CapabilityRoot {
    fn open(path: &Path) -> Result<Self, RunnerFailure> {
        if !path.is_absolute() {
            return Err(denied(
                "capability_root_invalid",
                "capability root must be absolute",
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(denied(
                "capability_root_invalid",
                "capability root must be a real directory, not a link or reparse point",
            ));
        }
        let directory = Dir::open_ambient_dir(&canonical, ambient_authority())
            .map_err(|error| denied("capability_root_invalid", error.to_string()))?;
        Ok(Self {
            canonical,
            directory,
        })
    }

    fn relative(&self, value: &str) -> Result<PathBuf, RunnerFailure> {
        let path = Path::new(value);
        if !normalized_relative_path(value)
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(denied(
                "path_denied",
                "governed filesystem path must be normalized and relative",
            ));
        }
        let mut current = self.canonical.clone();
        let components = path.components().collect::<Vec<_>>();
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(component) = component else {
                unreachable!("validated path component");
            };
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if is_link_or_reparse(&metadata) {
                        return Err(denied(
                            "path_link_denied",
                            "governed path traverses a symlink or reparse point",
                        ));
                    }
                    if index + 1 < components.len() && !metadata.is_dir() {
                        return Err(denied(
                            "path_parent_invalid",
                            "governed path parent is not a directory",
                        ));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if index + 1 < components.len() {
                        return Err(denied(
                            "path_parent_missing",
                            "governed path parent does not exist",
                        ));
                    }
                }
                Err(error) => return Err(denied("path_inspection_failed", error.to_string())),
            }
        }
        Ok(path.to_path_buf())
    }

    fn inspect(
        &self,
        relative_path: &str,
        max_bytes: u64,
        include_content: bool,
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        if max_bytes == 0 || max_bytes > ACTION_RUNNER_CONTENT_MAX_BYTES {
            return Err(denied(
                "read_limit_invalid",
                "read limit must be within the governed content bound",
            ));
        }
        let relative = self.relative(relative_path)?;
        let metadata = match self.directory.symlink_metadata(&relative) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(GovernedFilesystemObservation {
                    relative_path: relative_path.into(),
                    exists: false,
                    sha256: None,
                    size_bytes: None,
                    content_base64: None,
                });
            }
            Err(error) => return Err(no_effect("file_inspection_failed", error.to_string())),
        };
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(denied(
                "file_type_denied",
                "governed target must be a regular non-link file",
            ));
        }
        if metadata.len() > max_bytes {
            return Err(denied(
                "file_too_large",
                "governed target exceeds the authorized read limit",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = self
            .directory
            .open_with(&relative, &options)
            .map_err(|error| no_effect("file_open_failed", error.to_string()))?;
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| no_effect("file_read_failed", error.to_string()))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
            return Err(denied(
                "file_too_large",
                "governed target changed beyond the authorized read limit",
            ));
        }
        Ok(observation(relative_path, Some(&bytes), include_content))
    }

    fn create_new(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        let relative = self.relative(relative_path)?;
        if self.directory.symlink_metadata(&relative).is_ok() {
            return Err(no_effect(
                "destination_exists",
                "create-new destination already exists",
            ));
        }
        let parent = relative
            .parent()
            .ok_or_else(|| denied("path_parent_invalid", "governed file has no parent"))?;
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| denied("path_name_invalid", "governed file name is invalid"))?;
        let temporary = parent.join(format!(".{name}.{}.mcg-action", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = self
            .directory
            .open_with(&temporary, &options)
            .map_err(|error| no_effect("temporary_create_failed", error.to_string()))?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = self.directory.remove_file(&temporary);
            return Err(no_effect("temporary_write_failed", error.to_string()));
        }
        drop(file);
        if let Err(error) = self
            .directory
            .hard_link(&temporary, &self.directory, &relative)
        {
            let _ = self.directory.remove_file(&temporary);
            return if self.directory.symlink_metadata(&relative).is_ok() {
                Err(no_effect(
                    "destination_exists",
                    "create-new destination appeared before commit",
                ))
            } else {
                Err(no_effect("file_publish_failed", error.to_string()))
            };
        }
        let _ = self.directory.remove_file(&temporary);
        match self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false) {
            Ok(observed) if observed.sha256.as_deref() == Some(&sha256(bytes)) => Ok(observed),
            Ok(_) => Err(unknown(
                "post_write_mismatch",
                "published file does not match authorized content",
            )),
            Err(error) => Err(unknown("post_write_unknown", error.to_string())),
        }
    }

    fn remove_exact(
        &self,
        relative_path: &str,
        expected_sha256: &str,
    ) -> Result<GovernedFilesystemObservation, RunnerFailure> {
        let relative = self.relative(relative_path)?;
        let before = self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false)?;
        if !before.exists {
            return Err(no_effect(
                "file_not_found",
                "compensation target is already absent",
            ));
        }
        if before.sha256.as_deref() != Some(expected_sha256) {
            return Err(no_effect(
                "precondition_failed",
                "compensation target hash changed",
            ));
        }
        let parent = relative
            .parent()
            .ok_or_else(|| denied("path_parent_invalid", "governed file has no parent"))?;
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| denied("path_name_invalid", "governed file name is invalid"))?;
        let quarantine = parent.join(format!(".{name}.{}.mcg-remove", Uuid::new_v4()));
        self.directory
            .rename(&relative, &self.directory, &quarantine)
            .map_err(|error| no_effect("compensation_rename_failed", error.to_string()))?;
        let quarantined = inspect_relative(
            &self.directory,
            &quarantine,
            ACTION_RUNNER_CONTENT_MAX_BYTES,
        );
        match quarantined {
            Ok(bytes) if sha256(&bytes) == expected_sha256 => {}
            Ok(_) => {
                if self
                    .directory
                    .rename(&quarantine, &self.directory, &relative)
                    .is_err()
                {
                    return Err(unknown(
                        "compensation_restore_failed",
                        "changed target was quarantined but could not be restored",
                    ));
                }
                return Err(no_effect(
                    "precondition_raced",
                    "compensation target changed before removal",
                ));
            }
            Err(error) => {
                let _ = self
                    .directory
                    .rename(&quarantine, &self.directory, &relative);
                return Err(unknown("compensation_inspection_failed", error.clone()));
            }
        }
        self.directory
            .remove_file(&quarantine)
            .map_err(|error| unknown("compensation_remove_failed", error.to_string()))?;
        let after = self.inspect(relative_path, ACTION_RUNNER_CONTENT_MAX_BYTES, false)?;
        if after.exists {
            return Err(unknown(
                "compensation_postcondition_unknown",
                "a file exists at the compensated path after exact removal",
            ));
        }
        Ok(after)
    }
}

#[cfg(feature = "executor")]
fn inspect_relative(directory: &Dir, relative: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = directory
        .open_with(relative, &options)
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err("governed file is not a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err("governed file exceeded its bound while reading".into());
    }
    Ok(bytes)
}

#[cfg(feature = "executor")]
fn observation(
    relative_path: &str,
    bytes: Option<&[u8]>,
    include_content: bool,
) -> GovernedFilesystemObservation {
    GovernedFilesystemObservation {
        relative_path: relative_path.into(),
        exists: bytes.is_some(),
        sha256: bytes.map(sha256),
        size_bytes: bytes.and_then(|bytes| u64::try_from(bytes.len()).ok()),
        content_base64: (include_content)
            .then(|| bytes.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)))
            .flatten(),
    }
}

#[cfg(feature = "executor")]
fn qualification_request(
    root: &Path,
    operation: &GovernedFilesystemOperation,
) -> Result<ActionRunnerRequest, String> {
    let (operation, operation_hash) = governed_filesystem_operation_payload(operation)?;
    Ok(ActionRunnerRequest {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id: Uuid::new_v4(),
        action_id: ActionId::new(),
        execution_id: ActionExecutionId::new(),
        capability_root: root.into(),
        operation,
        operation_hash,
    })
}

#[cfg(feature = "executor")]
fn invoke_request(request: &ActionRunnerRequest) -> Result<ActionRunnerResponse, String> {
    let bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    Ok(execute_request(&bytes))
}

#[cfg(feature = "executor")]
fn require_outcome(
    response: &ActionRunnerResponse,
    expected: RunnerEffectOutcome,
    step: &str,
) -> Result<(), String> {
    if response.outcome == expected {
        Ok(())
    } else {
        Err(format!(
            "qualification {step} returned {:?}: {}",
            response.outcome, response.detail
        ))
    }
}

#[cfg(feature = "executor")]
fn decode_observation(
    response: &ActionRunnerResponse,
) -> Result<GovernedFilesystemObservation, String> {
    let payload = response
        .observation
        .as_ref()
        .ok_or_else(|| "qualification response omitted its observation".to_string())?;
    let value = decode_canonical_cbor_v1(&payload.bytes).map_err(|error| error.to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

#[cfg(feature = "executor")]
fn validate_sha256(value: &str) -> Result<(), RunnerFailure> {
    if !valid_sha256(value) {
        return Err(denied(
            "sha256_invalid",
            "expected SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(feature = "executor")]
fn denied(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::Denied {
        code,
        detail: detail.into(),
    }
}

#[cfg(feature = "executor")]
fn no_effect(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::NoEffect {
        code,
        detail: detail.into(),
    }
}

#[cfg(feature = "executor")]
fn unknown(code: &'static str, detail: impl Into<String>) -> RunnerFailure {
    RunnerFailure::Unknown {
        code,
        detail: detail.into(),
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    false
}

#[cfg(all(test, feature = "executor"))]
mod tests {
    use super::*;

    fn policy_binding(type_id: &str, domain: &str) -> PolicyBinding {
        let payload = VersionedPayload::canonical_v1(
            StableTypeId::parse(type_id).unwrap(),
            &serde_json::json!({"bounded": true}),
        )
        .unwrap();
        let content_hash = payload.content_hash(domain).unwrap();
        PolicyBinding {
            payload,
            content_hash,
        }
    }

    fn authorized_create(
        root: &Path,
    ) -> (ActionEnvelope, ActionExecution, GovernedFilesystemOperation) {
        let now = Utc::now();
        let action_id = ActionId::new();
        let actor = RuntimeActorId::new();
        let operation = GovernedFilesystemOperation::CreateFile {
            relative_path: "authorized.txt".into(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(b"authorized"),
        };
        let (operation_payload, operation_hash) =
            governed_filesystem_operation_payload(&operation).unwrap();
        let (compensation, _) = governed_filesystem_operation_payload(
            &GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "authorized.txt".into(),
                expected_sha256: sha256(b"authorized"),
            },
        )
        .unwrap();
        let (reconciler, _) =
            governed_filesystem_operation_payload(&GovernedFilesystemOperation::InspectFile {
                relative_path: "authorized.txt".into(),
                max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
            })
            .unwrap();
        let authority = governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
            action_id,
            actor,
            capability_root: root.into(),
            operation_hash: operation_hash.clone(),
            effect_class: EffectClass::Reversible,
            expires_at: now + chrono::Duration::minutes(5),
        })
        .unwrap();
        let envelope = ActionEnvelope {
            id: action_id,
            workflow_id: mcg_contracts::WorkflowId::new(),
            attempt_id: mcg_contracts::AttemptId::new(),
            task_contract: mcg_contracts::VersionedRef {
                id: mcg_contracts::TaskContractId::new(),
                version: 1,
                content_hash: operation_hash.clone(),
            },
            actor,
            origin: None,
            authority,
            elevated_authority: None,
            expected_versions: vec![],
            operation: operation_payload,
            operation_hash,
            effect_class: EffectClass::Reversible,
            preconditions: vec![],
            budget_reservation: policy_binding("mcg.test_budget.v1", "TestBudget"),
            idempotency_key: "authorized-create".into(),
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
            state: ActionExecutionState::Executing,
            revision: 2,
            lease_generation: 1,
            executor: Some(RuntimeActorId::new()),
            outcome: None,
            created_at: now,
            updated_at: now,
        };
        (envelope, execution, operation)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn request(root: &Path, operation: GovernedFilesystemOperation) -> ActionRunnerRequest {
        let (operation, operation_hash) =
            governed_filesystem_operation_payload(&operation).unwrap();
        ActionRunnerRequest {
            protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            action_id: ActionId::new(),
            execution_id: ActionExecutionId::new(),
            capability_root: root.into(),
            operation,
            operation_hash,
        }
    }

    fn run(request: &ActionRunnerRequest) -> ActionRunnerResponse {
        execute_request(&serde_json::to_vec(request).unwrap())
    }

    fn decoded(response: &ActionRunnerResponse) -> GovernedFilesystemObservation {
        let payload = response.observation.as_ref().unwrap();
        serde_json::from_value(decode_canonical_cbor_v1(&payload.bytes).unwrap()).unwrap()
    }

    #[test]
    fn create_lost_response_reconcile_and_compensate_exactly_once() {
        let root = tempfile::tempdir().unwrap();
        let content = b"governed effect";
        let create = request(
            root.path(),
            GovernedFilesystemOperation::CreateFile {
                relative_path: "result.txt".into(),
                content_base64: base64::engine::general_purpose::STANDARD.encode(content),
            },
        );
        let created = run(&create);
        assert_eq!(created.outcome, RunnerEffectOutcome::EffectConfirmed);
        assert_eq!(
            std::fs::read(root.path().join("result.txt")).unwrap(),
            content
        );

        let inspect = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "result.txt".into(),
                max_bytes: 100,
            },
        );
        let reconciled = run(&inspect);
        assert_eq!(reconciled.outcome, RunnerEffectOutcome::Observational);
        let expected_sha256 = sha256(content);
        assert_eq!(
            decoded(&reconciled).sha256.as_deref(),
            Some(expected_sha256.as_str())
        );

        let duplicate = run(&create);
        assert_eq!(duplicate.outcome, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(duplicate.error_code.as_deref(), Some("destination_exists"));
        assert_eq!(
            std::fs::read(root.path().join("result.txt")).unwrap(),
            content
        );

        let compensate = request(
            root.path(),
            GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "result.txt".into(),
                expected_sha256: sha256(content),
            },
        );
        let removed = run(&compensate);
        assert_eq!(removed.outcome, RunnerEffectOutcome::EffectConfirmed);
        assert!(!decoded(&removed).exists);
        assert!(!root.path().join("result.txt").exists());
        let repeated = run(&compensate);
        assert_eq!(repeated.outcome, RunnerEffectOutcome::NoEffectConfirmed);
    }

    #[test]
    fn traversal_absolute_paths_hash_substitution_and_changed_compensation_are_denied() {
        let root = tempfile::tempdir().unwrap();
        for relative_path in [
            "../escape.txt",
            ".",
            "nested/../escape.txt",
            "nested\\escape.txt",
            "C:drive-relative.txt",
            "file.txt:stream",
            "CON.txt",
            "COM¹.log",
        ] {
            let response = run(&request(
                root.path(),
                GovernedFilesystemOperation::InspectFile {
                    relative_path: relative_path.into(),
                    max_bytes: 100,
                },
            ));
            assert_eq!(
                response.outcome,
                RunnerEffectOutcome::Denied,
                "semantic path must be denied: {relative_path:?}"
            );
        }
        let mut substituted = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "missing.txt".into(),
                max_bytes: 100,
            },
        );
        substituted.operation.bytes.push(0);
        assert_eq!(run(&substituted).outcome, RunnerEffectOutcome::Denied);
        let mut wrong_domain = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "missing.txt".into(),
                max_bytes: 100,
            },
        );
        wrong_domain.operation_hash = wrong_domain
            .operation
            .content_hash("AttackerSelectedDomain")
            .unwrap();
        let response = run(&wrong_domain);
        assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
        assert_eq!(
            response.error_code.as_deref(),
            Some("operation_hash_domain_mismatch")
        );

        std::fs::write(root.path().join("changed.txt"), "new owner content").unwrap();
        let response = run(&request(
            root.path(),
            GovernedFilesystemOperation::RemoveCreatedFile {
                relative_path: "changed.txt".into(),
                expected_sha256: sha256(b"old content"),
            },
        ));
        assert_eq!(response.outcome, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(
            std::fs::read_to_string(root.path().join("changed.txt")).unwrap(),
            "new owner content"
        );
    }

    #[test]
    fn semantic_paths_are_portable_bounded_and_unicode_canonical() {
        for valid in ["result.txt", "nested/result.txt", "café/receipt.json"] {
            assert!(normalized_relative_path(valid), "valid path {valid:?}");
        }
        for invalid in [
            "",
            ".",
            "..",
            "/absolute.txt",
            "nested/",
            "nested//file.txt",
            "nested\\file.txt",
            "C:drive-relative.txt",
            "file.txt:stream",
            "question?.txt",
            "control\u{001f}.txt",
            "trailing.",
            "trailing ",
            "CON",
            "con.txt",
            "NUL.json",
            "COM1.log",
            "COM¹.log",
            "LPT9",
            "cafe\u{301}.txt",
        ] {
            assert!(
                !normalized_relative_path(invalid),
                "invalid path {invalid:?}"
            );
        }
        assert!(!normalized_relative_path(&format!(
            "{}.txt",
            "a".repeat(256)
        )));
        assert!(!normalized_relative_path(&format!(
            "{}/{}",
            "a".repeat(255),
            "b".repeat(4_000)
        )));
    }

    #[test]
    fn package_qualification_restores_an_empty_root() {
        let root = tempfile::tempdir().unwrap();
        let report = qualify_filesystem_runner(root.path()).unwrap();
        assert_eq!(report.created, RunnerEffectOutcome::EffectConfirmed);
        assert_eq!(report.reconciled, RunnerEffectOutcome::Observational);
        assert_eq!(report.duplicate, RunnerEffectOutcome::NoEffectConfirmed);
        assert_eq!(report.compensated, RunnerEffectOutcome::EffectConfirmed);
        assert!(report.final_absent);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);

        std::fs::write(root.path().join("owner-file"), b"preserve").unwrap();
        assert!(qualify_filesystem_runner(root.path()).is_err());
        assert_eq!(
            std::fs::read(root.path().join("owner-file")).unwrap(),
            b"preserve"
        );
    }

    #[test]
    fn parent_contract_rejects_substituted_identity_and_exit_status() {
        let root = tempfile::tempdir().unwrap();
        let request = request(
            root.path(),
            GovernedFilesystemOperation::InspectFile {
                relative_path: "missing.txt".into(),
                max_bytes: 100,
            },
        );
        let response = run(&request);
        let encoded = serde_json::to_vec(&response).unwrap();
        assert!(validate_client_response(true, &request, &encoded).is_ok());
        assert!(validate_client_response(false, &request, &encoded).is_err());

        let mut substituted = response;
        substituted.execution_id = Some(ActionExecutionId::new());
        let encoded = serde_json::to_vec(&substituted).unwrap();
        assert!(validate_client_response(true, &request, &encoded).is_err());
    }

    #[test]
    fn authorized_request_is_derived_from_exact_cross_bound_intent() {
        let root = tempfile::tempdir().unwrap();
        let (envelope, execution, _) = authorized_create(root.path());
        let request = request_from_authorized_action(
            &envelope,
            &execution,
            Uuid::new_v4(),
            envelope.created_at,
        )
        .unwrap();
        assert_eq!(request.capability_root, root.path());
        assert_eq!(request.operation_hash, envelope.operation_hash);

        let mut substituted = envelope.clone();
        substituted.operation_hash.domain = "AttackerDomain".into();
        assert!(
            request_from_authorized_action(
                &substituted,
                &execution,
                Uuid::new_v4(),
                envelope.created_at,
            )
            .is_err()
        );

        let mut stale = execution.clone();
        stale.state = ActionExecutionState::Authorized;
        assert!(
            request_from_authorized_action(&envelope, &stale, Uuid::new_v4(), envelope.created_at,)
                .is_err()
        );

        let worker = RuntimeActorId::new();
        let mut reconciling = execution;
        reconciling.state = ActionExecutionState::Reconciling;
        reconciling.executor = Some(worker);
        let job = ReconciliationJob {
            id: mcg_contracts::ReconciliationJobId::new(),
            action_id: envelope.id,
            execution_id: reconciling.id,
            adapter: envelope.reconciler.clone().unwrap(),
            state: ReconciliationJobState::Running,
            revision: 2,
            lease_generation: 1,
            lease_owner: Some(worker),
            lease_expires_at: Some(envelope.created_at + chrono::Duration::minutes(1)),
            next_attempt_at: envelope.created_at,
            attempts: 1,
            deadline: envelope.created_at + chrono::Duration::minutes(5),
            last_hint: None,
            created_at: envelope.created_at,
            updated_at: envelope.created_at,
        };
        let reconciliation_request = request_from_reconciliation_job(
            &envelope,
            &reconciling,
            &job,
            Uuid::new_v4(),
            envelope.created_at,
        )
        .unwrap();
        assert_eq!(reconciliation_request.operation, job.adapter);
        assert_ne!(
            reconciliation_request.operation_hash,
            envelope.operation_hash
        );

        let mut substituted_job = job;
        substituted_job.adapter = envelope.operation.clone();
        assert!(
            request_from_reconciliation_job(
                &envelope,
                &reconciling,
                &substituted_job,
                Uuid::new_v4(),
                envelope.created_at,
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_targets_and_ancestors_are_denied() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();
        let response = run(&request(
            root.path(),
            GovernedFilesystemOperation::ReadFile {
                relative_path: "linked/secret.txt".into(),
                max_bytes: 100,
            },
        ));
        assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
        assert_eq!(response.error_code.as_deref(), Some("path_link_denied"));
    }
}
