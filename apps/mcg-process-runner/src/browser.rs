use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use chromiumoxide::{
    Browser, BrowserConfig, Element, Page,
    cdp::browser_protocol::{
        emulation::SetDeviceMetricsOverrideParams,
        fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
        network::ErrorReason,
    },
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use mcg_browser_contracts::{BrowserWorkflowDocument, BrowserWorkflowStep};
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId,
    ArtifactRef, ContentHash, EffectClass, ExecutionPlatform, FilesystemAccess,
    FilesystemCapability, IsolationGuarantee, NetworkCapability, PROTOCOL_V1, PermitId,
    PolicyBinding, RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, SandboxCapabilityProfile,
    SourceRef, StableTypeId, TaskContractId, ToolCapabilitySet, ToolInvocationPermit,
    ToolInvocationProposal, ToolManifest, ToolManifestId, ToolPolicy, ToolResourceLimits,
    VersionedPayload, VersionedRef, decode_canonical_cbor_v1, tool_invocation_permit_hash,
    tool_invocation_proposal_hash,
};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::{io::AsyncWriteExt as _, process::Command, time::Instant};
use url::Url;
use uuid::Uuid;

use super::{
    GOVERNED_ACTION_OPERATION_HASH_DOMAIN, ProcessRunnerClientError, current_execution_platform,
    join_reader, preserve_platform_runtime_environment, process_runner_sandbox_profile,
    read_bounded, require_regular_unlinked, runner_executable_source, runner_file_name,
    static_source_ref, stop_child, tool_policy_binding, valid_code,
};

pub const BROWSER_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const BROWSER_RUNNER_REQUEST_MAX_BYTES: usize = 1024 * 1024;
pub const BROWSER_RUNNER_RESPONSE_MAX_BYTES: usize = 64 * 1024;
pub const GOVERNED_BROWSER_AUTHORITY_HASH_DOMAIN: &str = "MetaCog.BrowserAuthority";
pub const GOVERNED_BROWSER_AUTHORITY_TYPE: &str = "mcg.governed_browser_authority.v1";
pub const GOVERNED_BROWSER_OPERATION_TYPE: &str = "mcg.governed_browser_operation.v1";
pub const GOVERNED_BROWSER_OBSERVATION_TYPE: &str = "mcg.governed_browser_observation.v1";
pub const GOVERNED_BROWSER_RECONCILER_TYPE: &str = "mcg.governed_browser_reconciler.v1";
pub const BROWSER_RUNNER_INPUT_SCHEMA_TYPE: &str = "mcg.browser_runner_input_schema.v1";
pub const BROWSER_RUNNER_OUTPUT_SCHEMA_TYPE: &str = "mcg.browser_runner_output_schema.v1";
pub const BROWSER_RUNNER_RECONCILER_SOURCE_TYPE: &str = "mcg.browser_runner_reconciler.v1";
pub const BROWSER_RUNNER_RECEIPT_TYPE: &str = "mcg.browser_runner_receipt.v1";
pub const BROWSER_EXECUTABLE_SOURCE_TYPE: &str = "mcg.browser_executable.v1";
pub const BROWSER_EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.BrowserExecutable";

const DETAIL_MAX_BYTES: usize = 4 * 1024;
const STDERR_MAX_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserRunnerAuthorization {
    pub manifest: ToolManifest,
    pub proposal: ToolInvocationProposal,
    pub proposal_hash: ContentHash,
    pub policy: ToolPolicy,
    pub permit: ToolInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRunnerCapabilityEvidence {
    pub runner: SourceRef,
    pub browser: SourceRef,
    pub platform: ExecutionPlatform,
    pub available_sandbox: SandboxCapabilityProfile,
    pub resource_ceiling: ToolResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub envelope: ActionEnvelope,
    pub authorization: BrowserRunnerAuthorization,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedBrowserOperation {
    pub browser_executable: PathBuf,
    pub browser_sha256: String,
    pub profile_root: PathBuf,
    pub origin: String,
    pub workflow: BrowserWorkflowDocument,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedBrowserAuthority {
    pub action_id: ActionId,
    pub actor: RuntimeActorId,
    pub operation_hash: ContentHash,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedBrowserReconciler {
    pub origin: String,
    pub browser_sha256: String,
    pub disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedBrowserObservation {
    pub browser_sha256: String,
    pub workflow_sha256: String,
    pub final_url: String,
    pub steps_completed: u16,
    pub total_steps: u16,
    pub denied_requests: u64,
    pub workflow_passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserRunnerOutcome {
    Completed,
    NoEffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: BrowserRunnerOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserRunnerProtocolError {
    #[error("governed browser action is invalid: {0}")]
    Invalid(&'static str),
    #[error("governed browser canonical payload is invalid")]
    Canonical,
}

#[derive(Debug, Clone)]
pub struct BrowserRunnerClient {
    executable: PathBuf,
    browser_executable: PathBuf,
    outer_timeout: Duration,
    capability_evidence: BrowserRunnerCapabilityEvidence,
}

impl BrowserRunnerClient {
    /// Selects the fixed capability runner adjacent to Root and one exact browser executable.
    ///
    /// # Errors
    /// Returns unavailable when either executable is absent, linked, irregular, or unmeasurable.
    pub fn adjacent_to(
        application: impl AsRef<Path>,
        browser_executable: impl AsRef<Path>,
    ) -> Result<Self, ProcessRunnerClientError> {
        let parent = application
            .as_ref()
            .parent()
            .ok_or(ProcessRunnerClientError::Unavailable)?;
        Self::with_executables(
            parent.join(runner_file_name()),
            browser_executable,
            Duration::from_secs(125),
        )
    }

    /// Creates a browser client with exact runner and browser identities for qualification.
    ///
    /// # Errors
    /// Returns unavailable for unsafe paths or an invalid timeout.
    pub fn with_executables(
        executable: impl AsRef<Path>,
        browser_executable: impl AsRef<Path>,
        outer_timeout: Duration,
    ) -> Result<Self, ProcessRunnerClientError> {
        let executable = executable.as_ref();
        let browser_executable = browser_executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name()))
            || outer_timeout.is_zero()
            || outer_timeout > Duration::from_mins(3)
            || !browser_executable.is_absolute()
        {
            return Err(ProcessRunnerClientError::Unavailable);
        }
        require_regular_unlinked(executable)?;
        require_regular_unlinked(browser_executable)?;
        let browser = browser_executable_source(browser_executable)?;
        Ok(Self {
            executable: executable.to_path_buf(),
            browser_executable: browser_executable.to_path_buf(),
            outer_timeout,
            capability_evidence: BrowserRunnerCapabilityEvidence {
                runner: runner_executable_source(executable)?,
                browser,
                platform: current_execution_platform(),
                available_sandbox: browser_runner_sandbox_profile(),
                resource_ceiling: ToolResourceLimits {
                    timeout_ms: 120_000,
                    max_output_bytes: BROWSER_RUNNER_RESPONSE_MAX_BYTES as u64,
                    max_memory_bytes: 0,
                    max_child_processes: 64,
                },
            },
        })
    }

    #[must_use]
    pub fn capability_evidence(&self) -> &BrowserRunnerCapabilityEvidence {
        &self.capability_evidence
    }

    /// Dispatches one exact browser workflow through the fixed cleared-environment child.
    ///
    /// # Errors
    /// Returns a categorical transport error on identity, launch, delivery, timeout, or response failure.
    pub async fn invoke(
        &self,
        request: &BrowserRunnerRequest,
    ) -> Result<BrowserRunnerResponse, ProcessRunnerClientError> {
        require_regular_unlinked(&self.executable)?;
        require_regular_unlinked(&self.browser_executable)?;
        if runner_executable_source(&self.executable)? != self.capability_evidence.runner
            || browser_executable_source(&self.browser_executable)?
                != self.capability_evidence.browser
        {
            return Err(ProcessRunnerClientError::ExecutableChanged);
        }
        let bytes =
            serde_json::to_vec(request).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
        if bytes.len() > BROWSER_RUNNER_REQUEST_MAX_BYTES {
            return Err(ProcessRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command
            .arg("--governed-browser")
            .env_clear()
            .current_dir(
                self.executable
                    .parent()
                    .ok_or(ProcessRunnerClientError::Unavailable)?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        preserve_platform_runtime_environment(&mut command);
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
        let mut stdout_task = tokio::spawn(read_bounded(stdout, BROWSER_RUNNER_RESPONSE_MAX_BYTES));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        match tokio::time::timeout_at(deadline, async {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Delivery);
            }
            Err(_) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Timeout);
            }
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
        validate_browser_client_response(status.success(), request, &output.0.0)
    }
}

fn validate_browser_client_response(
    status_succeeded: bool,
    request: &BrowserRunnerRequest,
    bytes: &[u8],
) -> Result<BrowserRunnerResponse, ProcessRunnerClientError> {
    let response: BrowserRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
    if response.protocol_version != BROWSER_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.action_id != Some(request.action_id)
        || response.execution_id != Some(request.execution_id)
        || status_succeeded != (response.outcome == BrowserRunnerOutcome::Completed)
        || response.detail.len() > DETAIL_MAX_BYTES
    {
        return Err(ProcessRunnerClientError::InvalidResponse);
    }
    match response.outcome {
        BrowserRunnerOutcome::Completed => {
            let Some(observation) = &response.observation else {
                return Err(ProcessRunnerClientError::InvalidResponse);
            };
            if response.error_code.is_some()
                || observation.type_id.as_str() != GOVERNED_BROWSER_OBSERVATION_TYPE
                || decode_canonical_cbor_v1(&observation.bytes).is_err()
            {
                return Err(ProcessRunnerClientError::InvalidResponse);
            }
            let operation: GovernedBrowserOperation = decode_browser_payload(&request.operation)
                .map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
            let observation = decode_governed_browser_observation(observation)
                .map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
            validate_governed_browser_observation(&operation, &observation)
                .map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
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

/// Canonically encodes and hashes one exact governed browser workflow.
///
/// # Errors
/// Returns an error for malformed executable identity, origin, workflow, viewport, or timeout.
pub fn governed_browser_operation_payload(
    operation: &GovernedBrowserOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    validate_browser_operation(operation).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_BROWSER_OPERATION_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(operation).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Binds one exact browser action, actor, operation, and expiry.
///
/// # Errors
/// Returns an error for expired or malformed authority.
pub fn governed_browser_authority_binding(
    authority: &GovernedBrowserAuthority,
) -> Result<PolicyBinding, String> {
    if authority.expires_at <= Utc::now() {
        return Err("governed browser authority is expired".into());
    }
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_BROWSER_AUTHORITY_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(authority).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(GOVERNED_BROWSER_AUTHORITY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Creates the mandatory manual/no-redispatch descriptor for browser effects.
///
/// # Errors
/// Returns an error for invalid operation evidence.
pub fn governed_browser_reconciler(
    operation: &GovernedBrowserOperation,
) -> Result<VersionedPayload, String> {
    validate_browser_operation(operation).map_err(|error| error.to_string())?;
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_BROWSER_RECONCILER_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(GovernedBrowserReconciler {
            origin: operation.origin.clone(),
            browser_sha256: operation.browser_sha256.clone(),
            disposition: "manual_review_no_redispatch".into(),
        })
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Independently validates the immutable browser action and conservative effect shape.
///
/// # Errors
/// Returns a protocol error for substitution, expiry, or missing reconciliation authority.
pub fn validate_governed_browser_envelope(
    envelope: &ActionEnvelope,
    now: DateTime<Utc>,
) -> Result<GovernedBrowserAuthority, BrowserRunnerProtocolError> {
    if envelope.operation.type_id.as_str() != GOVERNED_BROWSER_OPERATION_TYPE
        || envelope.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN
        || envelope
            .operation
            .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
            .map_err(|_| BrowserRunnerProtocolError::Canonical)?
            != envelope.operation_hash
        || envelope.authority.payload.type_id.as_str() != GOVERNED_BROWSER_AUTHORITY_TYPE
        || envelope.authority.content_hash.domain != GOVERNED_BROWSER_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_BROWSER_AUTHORITY_HASH_DOMAIN)
            .map_err(|_| BrowserRunnerProtocolError::Canonical)?
            != envelope.authority.content_hash
    {
        return Err(BrowserRunnerProtocolError::Invalid(
            "operation or authority binding",
        ));
    }
    let authority: GovernedBrowserAuthority = decode_browser_payload(&envelope.authority.payload)?;
    let operation: GovernedBrowserOperation = decode_browser_payload(&envelope.operation)?;
    validate_browser_operation(&operation)?;
    if authority.action_id != envelope.id
        || authority.actor != envelope.actor
        || authority.operation_hash != envelope.operation_hash
        || authority.expires_at <= now
        || envelope.effect_class != EffectClass::Irreversible
        || envelope.elevated_authority.is_none()
        || envelope.compensation.is_some()
        || envelope.compensates_action.is_some()
        || envelope.reconciler.as_ref()
            != Some(
                &governed_browser_reconciler(&operation)
                    .map_err(|_| BrowserRunnerProtocolError::Canonical)?,
            )
        || envelope
            .reconciliation_deadline
            .is_none_or(|deadline| deadline <= now)
    {
        return Err(BrowserRunnerProtocolError::Invalid(
            "browser authority scope",
        ));
    }
    Ok(authority)
}

/// Creates one child request from exact executing action state.
///
/// # Errors
/// Returns a protocol error when the action or execution is not exact and active.
pub fn browser_request_from_authorized_action(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    authorization: BrowserRunnerAuthorization,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<BrowserRunnerRequest, BrowserRunnerProtocolError> {
    validate_governed_browser_envelope(envelope, now)?;
    if execution.action_id != envelope.id
        || execution.state != ActionExecutionState::Executing
        || execution.executor.is_none()
    {
        return Err(BrowserRunnerProtocolError::Invalid("execution binding"));
    }
    Ok(BrowserRunnerRequest {
        protocol_version: BROWSER_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        envelope: envelope.clone(),
        authorization,
        operation: envelope.operation.clone(),
        operation_hash: envelope.operation_hash.clone(),
    })
}

/// Decodes one content-minimized browser observation.
///
/// # Errors
/// Returns a protocol error for wrong type or malformed canonical bytes.
pub fn decode_governed_browser_observation(
    payload: &VersionedPayload,
) -> Result<GovernedBrowserObservation, BrowserRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_BROWSER_OBSERVATION_TYPE {
        return Err(BrowserRunnerProtocolError::Invalid(
            "browser observation type",
        ));
    }
    decode_browser_payload(payload)
}

/// Verifies that bounded browser evidence belongs to the exact authorized operation.
///
/// # Errors
/// Returns a protocol error for substituted executable/workflow/origin or inconsistent steps.
pub fn validate_governed_browser_observation(
    operation: &GovernedBrowserOperation,
    observation: &GovernedBrowserObservation,
) -> Result<(), BrowserRunnerProtocolError> {
    validate_browser_operation(operation)?;
    let workflow_sha256 = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&operation.workflow)
                .map_err(|_| BrowserRunnerProtocolError::Canonical)?
        )
    );
    let final_url = Url::parse(&observation.final_url)
        .map_err(|_| BrowserRunnerProtocolError::Invalid("browser observation URL"))?;
    let total_steps = u16::try_from(operation.workflow.steps.len())
        .map_err(|_| BrowserRunnerProtocolError::Invalid("browser observation steps"))?;
    if observation.browser_sha256 != operation.browser_sha256
        || observation.workflow_sha256 != workflow_sha256
        || final_url.origin().ascii_serialization() != operation.origin
        || final_url.query().is_some()
        || final_url.fragment().is_some()
        || observation.total_steps != total_steps
        || observation.steps_completed > total_steps
        || observation.workflow_passed && observation.steps_completed != total_steps
    {
        return Err(BrowserRunnerProtocolError::Invalid(
            "browser observation binding",
        ));
    }
    Ok(())
}

fn decode_browser_payload<T: serde::de::DeserializeOwned>(
    payload: &VersionedPayload,
) -> Result<T, BrowserRunnerProtocolError> {
    let value = decode_canonical_cbor_v1(&payload.bytes)
        .map_err(|_| BrowserRunnerProtocolError::Canonical)?;
    serde_json::from_value(value).map_err(|_| BrowserRunnerProtocolError::Canonical)
}

fn validate_browser_operation(
    operation: &GovernedBrowserOperation,
) -> Result<(), BrowserRunnerProtocolError> {
    let start = Url::parse(&operation.workflow.start_url)
        .map_err(|_| BrowserRunnerProtocolError::Invalid("browser start URL"))?;
    if !operation.browser_executable.is_absolute()
        || operation
            .browser_executable
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || !operation.profile_root.is_absolute()
        || operation
            .profile_root
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || !valid_sha256(&operation.browser_sha256)
        || operation.workflow.validate().is_err()
        || start.origin().ascii_serialization() != operation.origin
        || canonical_origin(&operation.origin).as_deref() != Some(operation.origin.as_str())
        || !(320..=7_680).contains(&operation.viewport_width)
        || !(240..=4_320).contains(&operation.viewport_height)
        || operation.timeout_ms == 0
        || operation.timeout_ms > 120_000
    {
        return Err(BrowserRunnerProtocolError::Invalid(
            "browser operation shape",
        ));
    }
    Ok(())
}

fn canonical_origin(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Returns the actual fixed browser executable identity.
///
/// # Errors
/// Returns unavailable when the path is not an exact regular unlinked file.
pub fn browser_executable_source(path: &Path) -> Result<SourceRef, ProcessRunnerClientError> {
    require_regular_unlinked(path)?;
    let locator = path
        .to_str()
        .ok_or(ProcessRunnerClientError::Unavailable)?
        .to_owned();
    let digest = super::executable_sha256(path)?;
    Ok(SourceRef {
        source_type: StableTypeId::parse(BROWSER_EXECUTABLE_SOURCE_TYPE)
            .map_err(|_| ProcessRunnerClientError::Unavailable)?,
        locator,
        content_hash: ContentHash {
            algorithm: mcg_contracts::HashAlgorithm::Sha256,
            domain: BROWSER_EXECUTABLE_HASH_DOMAIN.into(),
            digest_hex: digest,
        },
    })
}

#[must_use]
pub const fn browser_runner_sandbox_profile() -> SandboxCapabilityProfile {
    let mut profile = process_runner_sandbox_profile();
    profile.network = IsolationGuarantee::BestEffort;
    profile.process_tree = if cfg!(windows) {
        IsolationGuarantee::Hard
    } else {
        IsolationGuarantee::BestEffort
    };
    profile
}

#[must_use]
pub fn browser_runner_input_schema_source() -> SourceRef {
    static_source_ref(
        BROWSER_RUNNER_INPUT_SCHEMA_TYPE,
        GOVERNED_BROWSER_OPERATION_TYPE,
        "MetaCog.BrowserRunnerInputSchema",
    )
}

#[must_use]
pub fn browser_runner_output_schema_source() -> SourceRef {
    static_source_ref(
        BROWSER_RUNNER_OUTPUT_SCHEMA_TYPE,
        GOVERNED_BROWSER_OBSERVATION_TYPE,
        "MetaCog.BrowserRunnerOutputSchema",
    )
}

#[must_use]
pub fn browser_runner_reconciler_source() -> SourceRef {
    static_source_ref(
        BROWSER_RUNNER_RECONCILER_SOURCE_TYPE,
        GOVERNED_BROWSER_RECONCILER_TYPE,
        "MetaCog.BrowserRunnerReconciler",
    )
}

/// Derives the deterministic Tool manifest for one exact browser/origin/limit tuple.
///
/// # Errors
/// Returns an error for inconsistent evidence, operation, or limits.
pub fn governed_browser_tool_manifest(
    evidence: &BrowserRunnerCapabilityEvidence,
    operation: &GovernedBrowserOperation,
    limits: ToolResourceLimits,
) -> Result<ToolManifest, String> {
    validate_browser_operation(operation).map_err(|error| error.to_string())?;
    limits.validate().map_err(|error| error.to_string())?;
    if !limits.fits_within(evidence.resource_ceiling)
        || evidence.browser.content_hash.digest_hex != operation.browser_sha256
        || evidence.browser.locator != operation.browser_executable.to_string_lossy()
    {
        return Err("browser Tool evidence or limits differ from the operation".into());
    }
    let material = serde_json::to_vec(&serde_json::json!({
        "runner": evidence.runner,
        "browser": evidence.browser,
        "platform": evidence.platform,
        "profile_root": operation.profile_root,
        "origin": operation.origin,
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
        name: "browser.workflow".into(),
        summary: "Governed ephemeral same-origin browser workflow".into(),
        runner: evidence.runner.clone(),
        input_schema: browser_runner_input_schema_source(),
        output_schema: browser_runner_output_schema_source(),
        maximum_effect_class: EffectClass::Irreversible,
        required_capabilities: ToolCapabilitySet {
            filesystem: vec![FilesystemCapability {
                root: operation
                    .profile_root
                    .to_str()
                    .ok_or("browser profile root is not Unicode")?
                    .to_owned(),
                access: FilesystemAccess::Write,
            }],
            network: vec![NetworkCapability {
                destination: operation.origin.clone(),
            }],
            secret_slots: Vec::new(),
            process_spawn: true,
        },
        resource_ceiling: limits,
        sandbox_requirement: evidence.available_sandbox,
        platform_support: vec![evidence.platform],
        receipt_semantics: StableTypeId::parse(BROWSER_RUNNER_RECEIPT_TYPE)
            .map_err(|error| error.to_string())?,
        reconciler: Some(browser_runner_reconciler_source()),
    };
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Derives exact Root policy for one browser manifest.
///
/// # Errors
/// Returns an error when the manifest reference is invalid.
pub fn governed_browser_tool_policy(manifest: &ToolManifest) -> Result<ToolPolicy, String> {
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

#[derive(Debug)]
struct BrowserFailure {
    outcome: BrowserRunnerOutcome,
    code: &'static str,
    detail: String,
}

impl BrowserFailure {
    fn denied(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: BrowserRunnerOutcome::Denied,
            code,
            detail: detail.into(),
        }
    }

    fn unknown(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: BrowserRunnerOutcome::OutcomeUnknown,
            code,
            detail: detail.into(),
        }
    }
}

/// Executes one browser runner request after independently validating all Root authority.
#[allow(clippy::too_many_lines)]
pub async fn execute_browser_request(bytes: &[u8]) -> BrowserRunnerResponse {
    let started_at = Utc::now();
    let Ok(request) = serde_json::from_slice::<BrowserRunnerRequest>(bytes) else {
        return browser_error_response(
            None,
            started_at,
            BrowserFailure::denied("request_invalid", "browser request is invalid"),
        );
    };
    if bytes.len() > BROWSER_RUNNER_REQUEST_MAX_BYTES {
        return browser_error_response(
            Some(&request),
            started_at,
            BrowserFailure::denied("request_too_large", "browser request exceeds its bound"),
        );
    }
    match execute_browser(&request).await {
        Ok(observation) => BrowserRunnerResponse {
            protocol_version: BROWSER_RUNNER_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            action_id: Some(request.action_id),
            execution_id: Some(request.execution_id),
            outcome: BrowserRunnerOutcome::Completed,
            observation: Some(observation),
            error_code: None,
            detail: "browser workflow completed with bounded evidence".into(),
            started_at,
            finished_at: Utc::now(),
        },
        Err(error) => browser_error_response(Some(&request), started_at, error),
    }
}

fn browser_error_response(
    request: Option<&BrowserRunnerRequest>,
    started_at: DateTime<Utc>,
    error: BrowserFailure,
) -> BrowserRunnerResponse {
    let mut detail = error.detail;
    detail.truncate(DETAIL_MAX_BYTES);
    BrowserRunnerResponse {
        protocol_version: BROWSER_RUNNER_PROTOCOL_VERSION,
        request_id: request.map(|request| request.request_id),
        action_id: request.map(|request| request.action_id),
        execution_id: request.map(|request| request.execution_id),
        outcome: error.outcome,
        observation: None,
        error_code: Some(error.code.into()),
        detail,
        started_at,
        finished_at: Utc::now(),
    }
}

#[allow(clippy::too_many_lines)]
async fn execute_browser(
    request: &BrowserRunnerRequest,
) -> Result<VersionedPayload, BrowserFailure> {
    validate_browser_tool_authorization(request, Utc::now())?;
    let operation: GovernedBrowserOperation = decode_browser_payload(&request.operation)
        .map_err(|error| BrowserFailure::denied("operation_invalid", error.to_string()))?;
    require_regular_unlinked(&operation.browser_executable).map_err(|_| {
        BrowserFailure::denied(
            "browser_identity_unavailable",
            "browser executable is unavailable",
        )
    })?;
    let measured = browser_executable_source(&operation.browser_executable).map_err(|_| {
        BrowserFailure::denied(
            "browser_identity_unavailable",
            "browser executable cannot be measured",
        )
    })?;
    if measured.content_hash.digest_hex != operation.browser_sha256 {
        return Err(BrowserFailure::denied(
            "browser_identity_changed",
            "browser executable identity changed",
        ));
    }
    let profile = tempfile::Builder::new()
        .prefix("mcg-governed-browser-")
        .tempdir_in(&operation.profile_root)
        .map_err(|error| BrowserFailure::denied("profile_unavailable", error.to_string()))?;
    let config = BrowserConfig::builder()
        .chrome_executable(&operation.browser_executable)
        .enable_request_intercept()
        .disable_cache()
        .user_data_dir(profile.path())
        .window_size(operation.viewport_width, operation.viewport_height)
        .request_timeout(Duration::from_millis(operation.timeout_ms))
        .launch_timeout(Duration::from_millis(operation.timeout_ms.min(30_000)))
        .build()
        .map_err(|error| BrowserFailure::denied("browser_config_invalid", error.clone()))?;
    let launched = Browser::launch(config).await;
    let (mut browser, mut handler) = launched
        .map_err(|error| BrowserFailure::denied("browser_launch_failed", error.to_string()))?;
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });
    let result = tokio::time::timeout(
        Duration::from_millis(operation.timeout_ms),
        run_browser_workflow(&mut browser, &operation),
    )
    .await;
    let shutdown = tokio::time::timeout(Duration::from_secs(5), async {
        browser.close().await.map_err(|_| ())?;
        browser.wait().await.map_err(|_| ())?;
        Ok::<_, ()>(())
    })
    .await;
    if !matches!(shutdown, Ok(Ok(()))) {
        let killed = tokio::time::timeout(Duration::from_secs(5), browser.kill()).await;
        handler_task.abort();
        if !matches!(killed, Ok(Some(Ok(())))) {
            return Err(BrowserFailure::unknown(
                "browser_shutdown_timeout",
                "browser shutdown and forced reap were not confirmed",
            ));
        }
        return Err(BrowserFailure::unknown(
            "browser_shutdown_timeout",
            "browser required forced shutdown after workflow completion",
        ));
    }
    handler_task.abort();
    profile.close().map_err(|error| {
        BrowserFailure::unknown(
            "profile_cleanup_failed",
            format!("isolated browser profile cleanup failed: {error}"),
        )
    })?;
    let observation = match result {
        Ok(Ok(observation)) => observation,
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            return Err(BrowserFailure::unknown(
                "browser_timeout",
                "browser workflow timed out after launch",
            ));
        }
    };
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_BROWSER_OBSERVATION_TYPE)
            .map_err(|error| BrowserFailure::unknown("observation_invalid", error.to_string()))?,
        &serde_json::to_value(observation)
            .map_err(|error| BrowserFailure::unknown("observation_invalid", error.to_string()))?,
    )
    .map_err(|error| BrowserFailure::unknown("observation_invalid", error.to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractionTarget {
    href: Option<String>,
    target: Option<String>,
    download: bool,
    has_form: bool,
    form_action: Option<String>,
    form_target: Option<String>,
    form_method: Option<String>,
}

async fn run_browser_workflow(
    browser: &mut Browser,
    operation: &GovernedBrowserOperation,
) -> Result<GovernedBrowserObservation, BrowserFailure> {
    let page = Arc::new(
        browser
            .new_page("about:blank")
            .await
            .map_err(|error| BrowserFailure::unknown("page_open_failed", error.to_string()))?,
    );
    page.execute(SetDeviceMetricsOverrideParams::new(
        operation.viewport_width,
        operation.viewport_height,
        1.0,
        false,
    ))
    .await
    .map_err(|error| BrowserFailure::unknown("viewport_failed", error.to_string()))?;
    let mut requests = page
        .event_listener::<EventRequestPaused>()
        .await
        .map_err(|error| BrowserFailure::unknown("interception_failed", error.to_string()))?;
    let intercepted_page = page.clone();
    let intercepted_origin = operation.origin.clone();
    let denied_requests = Arc::new(AtomicU64::new(0));
    let denied_for_task = denied_requests.clone();
    let intercept_task = tokio::spawn(async move {
        while let Some(event) = requests.next().await {
            let allowed = browser_request_allowed(
                &event.request.url,
                &event.request.method,
                &intercepted_origin,
            );
            let result = if allowed {
                intercepted_page
                    .execute(ContinueRequestParams::new(event.request_id.clone()))
                    .await
                    .map(|_| ())
            } else {
                denied_for_task.fetch_add(1, Ordering::SeqCst);
                intercepted_page
                    .execute(
                        FailRequestParams::builder()
                            .request_id(event.request_id.clone())
                            .error_reason(ErrorReason::BlockedByClient)
                            .build()
                            .expect("complete browser denial"),
                    )
                    .await
                    .map(|_| ())
            };
            if result.is_err() {
                break;
            }
        }
    });
    let result = async {
        page.goto(&operation.workflow.start_url)
            .await
            .map_err(|error| BrowserFailure::unknown("navigation_failed", error.to_string()))?;
        require_current_origin(&page, &operation.origin).await?;
        let mut completed = 0_u16;
        let mut passed = true;
        for step in &operation.workflow.steps {
            if execute_workflow_step(&page, step, &operation.origin)
                .await
                .is_ok()
            {
                completed = completed.saturating_add(1);
            } else {
                passed = false;
                break;
            }
            if matches!(
                step,
                BrowserWorkflowStep::Click { .. }
                    | BrowserWorkflowStep::Type { .. }
                    | BrowserWorkflowStep::Press { .. }
            ) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            require_current_origin(&page, &operation.origin).await?;
        }
        let final_url = current_browser_url(&page).await?;
        let workflow_bytes = serde_json::to_vec(&operation.workflow)
            .map_err(|error| BrowserFailure::unknown("workflow_hash_failed", error.to_string()))?;
        Ok(GovernedBrowserObservation {
            browser_sha256: operation.browser_sha256.clone(),
            workflow_sha256: format!("{:x}", Sha256::digest(workflow_bytes)),
            final_url: safe_browser_url(&final_url),
            steps_completed: completed,
            total_steps: u16::try_from(operation.workflow.steps.len()).unwrap_or(u16::MAX),
            denied_requests: denied_requests.load(Ordering::SeqCst),
            workflow_passed: passed,
        })
    }
    .await;
    intercept_task.abort();
    result
}

async fn execute_workflow_step(
    page: &Page,
    step: &BrowserWorkflowStep,
    origin: &str,
) -> Result<(), BrowserFailure> {
    match step {
        BrowserWorkflowStep::AssertElement { selector } => page
            .find_elements(selector)
            .await
            .map_err(|error| BrowserFailure::unknown("selector_failed", error.to_string()))
            .and_then(|elements| {
                if elements.is_empty() {
                    Err(BrowserFailure::unknown(
                        "assertion_failed",
                        "expected element was absent",
                    ))
                } else {
                    Ok(())
                }
            }),
        BrowserWorkflowStep::Click { selector } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| BrowserFailure::unknown("selector_failed", error.to_string()))?;
            validate_interaction_target(&element, origin).await?;
            element
                .click()
                .await
                .map_err(|error| BrowserFailure::unknown("click_failed", error.to_string()))?;
            Ok(())
        }
        BrowserWorkflowStep::Type { selector, value } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| BrowserFailure::unknown("selector_failed", error.to_string()))?;
            element
                .click()
                .await
                .map_err(|error| BrowserFailure::unknown("type_failed", error.to_string()))?;
            element
                .type_str(value)
                .await
                .map_err(|error| BrowserFailure::unknown("type_failed", error.to_string()))?;
            Ok(())
        }
        BrowserWorkflowStep::Press { selector, key } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| BrowserFailure::unknown("selector_failed", error.to_string()))?;
            validate_interaction_target(&element, origin).await?;
            element
                .focus()
                .await
                .map_err(|error| BrowserFailure::unknown("press_failed", error.to_string()))?;
            element
                .press_key(key)
                .await
                .map_err(|error| BrowserFailure::unknown("press_failed", error.to_string()))?;
            Ok(())
        }
        BrowserWorkflowStep::AssertText { selector, contains } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| BrowserFailure::unknown("selector_failed", error.to_string()))?;
            let text = element
                .inner_text()
                .await
                .map_err(|error| BrowserFailure::unknown("assertion_failed", error.to_string()))?
                .unwrap_or_default();
            if text.contains(contains) {
                Ok(())
            } else {
                Err(BrowserFailure::unknown(
                    "assertion_failed",
                    "expected text was absent",
                ))
            }
        }
        BrowserWorkflowStep::AssertUrl { url } => {
            let observed = current_browser_url(page).await?;
            if observed.as_str() == url {
                Ok(())
            } else {
                Err(BrowserFailure::unknown(
                    "assertion_failed",
                    "expected URL did not match",
                ))
            }
        }
    }
}

async fn validate_interaction_target(
    element: &Element,
    origin: &str,
) -> Result<(), BrowserFailure> {
    let response = element.call_js_fn(
        "function() { const form = this.form || this.closest?.('form') || null; return JSON.stringify({ href: this.href || null, target: this.target || null, download: this.hasAttribute?.('download') || false, hasForm: !!form, formAction: form ? (this.formAction || form.action || null) : null, formTarget: form ? (this.formTarget || form.target || null) : null, formMethod: form ? (this.formMethod || form.method || 'get') : null }); }",
        false,
    ).await.map_err(|error| BrowserFailure::unknown("interaction_inspection_failed", error.to_string()))?;
    let value = response
        .result
        .value
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| {
            BrowserFailure::unknown(
                "interaction_inspection_failed",
                "interaction metadata was absent",
            )
        })?;
    let target: InteractionTarget = serde_json::from_str(&value).map_err(|error| {
        BrowserFailure::unknown("interaction_inspection_failed", error.to_string())
    })?;
    if target.download
        || target
            .target
            .as_deref()
            .is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("_self"))
        || target
            .form_target
            .as_deref()
            .is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("_self"))
    {
        return Err(BrowserFailure::denied(
            "interaction_denied",
            "interaction opens a new context or download",
        ));
    }
    for destination in [target.href.as_deref(), target.form_action.as_deref()]
        .into_iter()
        .flatten()
    {
        let url = Url::parse(destination).map_err(|_| {
            BrowserFailure::denied("interaction_denied", "interaction destination is invalid")
        })?;
        if !matches!(url.scheme(), "http" | "https") || url.origin().ascii_serialization() != origin
        {
            return Err(BrowserFailure::denied(
                "interaction_denied",
                "interaction destination is outside the exact origin",
            ));
        }
    }
    if target.has_form
        && !target
            .form_method
            .as_deref()
            .unwrap_or("get")
            .eq_ignore_ascii_case("get")
    {
        return Err(BrowserFailure::denied(
            "interaction_denied",
            "mutating form submission is denied",
        ));
    }
    Ok(())
}

async fn current_browser_url(page: &Page) -> Result<Url, BrowserFailure> {
    let value = page
        .url()
        .await
        .map_err(|error| BrowserFailure::unknown("url_read_failed", error.to_string()))?
        .ok_or_else(|| BrowserFailure::unknown("url_read_failed", "browser URL was absent"))?;
    Url::parse(&value)
        .map_err(|error| BrowserFailure::unknown("url_read_failed", error.to_string()))
}

async fn require_current_origin(page: &Page, origin: &str) -> Result<(), BrowserFailure> {
    if current_browser_url(page)
        .await?
        .origin()
        .ascii_serialization()
        == origin
    {
        Ok(())
    } else {
        Err(BrowserFailure::unknown(
            "origin_escape",
            "browser left the exact origin after launch",
        ))
    }
}

fn browser_request_allowed(value: &str, method: &str, origin: &str) -> bool {
    if !matches!(method, "GET" | "HEAD") {
        return false;
    }
    if value.starts_with("data:") || value.starts_with("blob:") || value == "about:blank" {
        return true;
    }
    Url::parse(value).is_ok_and(|url| url.origin().ascii_serialization() == origin)
}

fn safe_browser_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

#[allow(clippy::too_many_lines)]
fn validate_browser_tool_authorization(
    request: &BrowserRunnerRequest,
    now: DateTime<Utc>,
) -> Result<(), BrowserFailure> {
    let authorization = &request.authorization;
    authorization
        .manifest
        .validate()
        .map_err(|error| BrowserFailure::denied("tool_manifest_invalid", error.to_string()))?;
    authorization
        .proposal
        .validate()
        .map_err(|error| BrowserFailure::denied("tool_proposal_invalid", error.to_string()))?;
    authorization
        .policy
        .validate()
        .map_err(|error| BrowserFailure::denied("tool_policy_invalid", error.to_string()))?;
    authorization
        .permit
        .validate()
        .map_err(|error| BrowserFailure::denied("tool_permit_invalid", error.to_string()))?;
    authorization
        .permit_ref
        .validate()
        .map_err(|error| BrowserFailure::denied("tool_permit_ref_invalid", error.to_string()))?;
    validate_governed_browser_envelope(&request.envelope, now)
        .map_err(|error| BrowserFailure::denied("action_envelope_invalid", error.to_string()))?;
    let operation: GovernedBrowserOperation = decode_browser_payload(&request.operation)
        .map_err(|error| BrowserFailure::denied("operation_invalid", error.to_string()))?;
    let manifest_ref = authorization
        .manifest
        .versioned_ref()
        .map_err(|error| BrowserFailure::denied("tool_manifest_invalid", error.to_string()))?;
    let proposal_hash = tool_invocation_proposal_hash(&authorization.proposal)
        .map_err(|error| BrowserFailure::denied("tool_proposal_invalid", error.to_string()))?;
    let permit_hash = tool_invocation_permit_hash(&authorization.permit)
        .map_err(|error| BrowserFailure::denied("tool_permit_invalid", error.to_string()))?;
    let expected_policy = tool_policy_binding(&authorization.policy)
        .map_err(|error| BrowserFailure::denied("tool_policy_invalid", error))?;
    let expected_runner = runner_executable_source(&std::env::current_exe().map_err(|error| {
        BrowserFailure::denied("runner_identity_unavailable", error.to_string())
    })?)
    .map_err(|_| {
        BrowserFailure::denied(
            "runner_identity_unavailable",
            "runner identity cannot be measured",
        )
    })?;
    let expected_browser =
        browser_executable_source(&operation.browser_executable).map_err(|_| {
            BrowserFailure::denied(
                "browser_identity_unavailable",
                "browser identity cannot be measured",
            )
        })?;
    let expected_limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: BROWSER_RUNNER_RESPONSE_MAX_BYTES as u64,
        max_memory_bytes: 0,
        max_child_processes: 64,
    };
    let operation_hash = request
        .operation
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| BrowserFailure::denied("operation_invalid", error.to_string()))?;
    if request.protocol_version != BROWSER_RUNNER_PROTOCOL_VERSION
        || request.action_id != request.envelope.id
        || request.operation != request.envelope.operation
        || request.operation_hash != request.envelope.operation_hash
        || request.operation_hash != operation_hash
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
        || authorization.permit.capabilities != authorization.manifest.required_capabilities
        || authorization.proposal.requested_capabilities
            != authorization.manifest.required_capabilities
        || authorization.permit.limits != expected_limits
        || authorization.proposal.requested_limits != expected_limits
        || authorization.manifest.resource_ceiling != expected_limits
        || authorization.proposal.required_sandbox != authorization.manifest.sandbox_requirement
        || authorization.permit.policy != expected_policy
        || authorization.permit.platform != current_execution_platform()
        || authorization.proposal.platform != current_execution_platform()
        || !authorization
            .manifest
            .platform_support
            .contains(&current_execution_platform())
        || !browser_runner_sandbox_profile().satisfies(authorization.manifest.sandbox_requirement)
        || !authorization
            .permit
            .sandbox
            .satisfies(authorization.manifest.sandbox_requirement)
        || authorization.permit.issued_at > now
        || authorization.permit.expires_at <= now
        || authorization.proposal.deadline != authorization.permit.expires_at
        || authorization
            .manifest
            .required_capabilities
            .filesystem
            .as_slice()
            != [FilesystemCapability {
                root: operation.profile_root.to_string_lossy().into_owned(),
                access: FilesystemAccess::Write,
            }]
        || authorization
            .manifest
            .required_capabilities
            .network
            .as_slice()
            != [NetworkCapability {
                destination: operation.origin.clone(),
            }]
        || !authorization
            .manifest
            .required_capabilities
            .secret_slots
            .is_empty()
        || !authorization.manifest.required_capabilities.process_spawn
        || authorization.manifest.runner != expected_runner
        || authorization.manifest.reconciler != Some(browser_runner_reconciler_source())
        || authorization.permit_ref.source_type.as_str() != ToolInvocationPermit::TYPE_ID
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != permit_hash
        || !authorization.permit.secret_handles.is_empty()
        || expected_browser.content_hash.digest_hex != operation.browser_sha256
        || expected_browser.locator != operation.browser_executable.to_string_lossy()
    {
        return Err(BrowserFailure::denied(
            "tool_authority_mismatch",
            "request differs from exact Root-issued browser Tool authority",
        ));
    }
    Ok(())
}

/// Builds one exact self-contained request for child/package qualification.
/// Production dispatch must use Protected Root-persisted authority.
///
/// # Errors
/// Returns an error when any operation, manifest, policy, proposal, or permit binding is invalid.
#[allow(clippy::too_many_lines)]
pub fn browser_qualification_request_with_evidence(
    operation: &GovernedBrowserOperation,
    evidence: &BrowserRunnerCapabilityEvidence,
) -> Result<BrowserRunnerRequest, String> {
    let now = Utc::now();
    let deadline = now
        + chrono::Duration::milliseconds(
            i64::try_from(operation.timeout_ms.saturating_add(5_000))
                .map_err(|error| error.to_string())?,
        );
    let action_id = ActionId::new();
    let actor = RuntimeActorId::new();
    let (payload, operation_hash) = governed_browser_operation_payload(operation)?;
    let limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: BROWSER_RUNNER_RESPONSE_MAX_BYTES as u64,
        max_memory_bytes: 0,
        max_child_processes: 64,
    };
    let manifest = governed_browser_tool_manifest(evidence, operation, limits)?;
    let policy = governed_browser_tool_policy(&manifest)?;
    let authority = governed_browser_authority_binding(&GovernedBrowserAuthority {
        action_id,
        actor,
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
        idempotency_key: format!("browser-runner-qualification-{action_id}"),
        compensation: None,
        compensates_action: None,
        reconciler: Some(governed_browser_reconciler(operation)?),
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
            media_type: "application/vnd.metacog.governed-browser-operation+cbor".into(),
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
    let authorization = BrowserRunnerAuthorization {
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
    browser_request_from_authorized_action(
        &envelope,
        &execution,
        authorization,
        Uuid::new_v4(),
        now,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_operation_rejects_origin_and_executable_substitution() {
        let mut operation = GovernedBrowserOperation {
            browser_executable: if cfg!(windows) {
                PathBuf::from(r"C:\Browser\chrome.exe")
            } else {
                PathBuf::from("/browser/chrome")
            },
            browser_sha256: "a".repeat(64),
            profile_root: if cfg!(windows) {
                PathBuf::from(r"C:\MetaCog\browser-profiles")
            } else {
                PathBuf::from("/metacog/browser-profiles")
            },
            origin: "https://example.com".into(),
            workflow: BrowserWorkflowDocument {
                schema_version: 1,
                name: "safe".into(),
                start_url: "https://example.com/start".into(),
                steps: vec![BrowserWorkflowStep::AssertElement {
                    selector: "main".into(),
                }],
            },
            viewport_width: 1280,
            viewport_height: 720,
            timeout_ms: 30_000,
        };
        assert!(governed_browser_operation_payload(&operation).is_ok());
        operation.origin = "https://other.example".into();
        assert!(governed_browser_operation_payload(&operation).is_err());
        operation.origin = "https://example.com".into();
        operation.browser_executable = PathBuf::from("relative-browser");
        assert!(governed_browser_operation_payload(&operation).is_err());
    }

    #[test]
    fn request_filter_is_same_origin_read_only() {
        assert!(browser_request_allowed(
            "https://example.com/app.js",
            "GET",
            "https://example.com"
        ));
        assert!(!browser_request_allowed(
            "https://example.com/save",
            "POST",
            "https://example.com"
        ));
        assert!(!browser_request_allowed(
            "https://tracker.example/pixel",
            "GET",
            "https://example.com"
        ));
    }

    #[test]
    fn browser_observation_is_exactly_operation_bound_and_query_free() {
        let operation = GovernedBrowserOperation {
            browser_executable: if cfg!(windows) {
                PathBuf::from(r"C:\Browser\chrome.exe")
            } else {
                PathBuf::from("/browser/chrome")
            },
            browser_sha256: "a".repeat(64),
            profile_root: if cfg!(windows) {
                PathBuf::from(r"C:\MetaCog\browser-profiles")
            } else {
                PathBuf::from("/metacog/browser-profiles")
            },
            origin: "https://example.com".into(),
            workflow: BrowserWorkflowDocument {
                schema_version: 1,
                name: "evidence".into(),
                start_url: "https://example.com/start?private=true".into(),
                steps: vec![BrowserWorkflowStep::AssertElement {
                    selector: "main".into(),
                }],
            },
            viewport_width: 1280,
            viewport_height: 720,
            timeout_ms: 30_000,
        };
        let mut observation = GovernedBrowserObservation {
            browser_sha256: operation.browser_sha256.clone(),
            workflow_sha256: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&operation.workflow).expect("workflow"))
            ),
            final_url: "https://example.com/done".into(),
            steps_completed: 1,
            total_steps: 1,
            denied_requests: 2,
            workflow_passed: true,
        };
        assert!(validate_governed_browser_observation(&operation, &observation).is_ok());
        observation.final_url = "https://example.com/done?leak=true".into();
        assert!(validate_governed_browser_observation(&operation, &observation).is_err());
        observation.final_url = "https://example.com/done".into();
        observation.workflow_sha256 = "b".repeat(64);
        assert!(validate_governed_browser_observation(&operation, &observation).is_err());
    }

    #[tokio::test]
    async fn substituted_workflow_is_denied_before_browser_launch() {
        let current = std::env::current_exe().expect("test executable");
        let profiles = tempfile::tempdir().expect("profile root");
        let evidence = BrowserRunnerCapabilityEvidence {
            runner: runner_executable_source(&current).expect("runner source"),
            browser: browser_executable_source(&current).expect("browser source"),
            platform: current_execution_platform(),
            available_sandbox: browser_runner_sandbox_profile(),
            resource_ceiling: ToolResourceLimits {
                timeout_ms: 120_000,
                max_output_bytes: BROWSER_RUNNER_RESPONSE_MAX_BYTES as u64,
                max_memory_bytes: 0,
                max_child_processes: 64,
            },
        };
        let operation = GovernedBrowserOperation {
            browser_executable: current.clone(),
            browser_sha256: crate::executable_sha256(&current).expect("browser hash"),
            profile_root: profiles.path().to_path_buf(),
            origin: "https://example.com".into(),
            workflow: BrowserWorkflowDocument {
                schema_version: 1,
                name: "original".into(),
                start_url: "https://example.com/start".into(),
                steps: vec![BrowserWorkflowStep::AssertElement {
                    selector: "main".into(),
                }],
            },
            viewport_width: 1280,
            viewport_height: 720,
            timeout_ms: 30_000,
        };
        let mut request = browser_qualification_request_with_evidence(&operation, &evidence)
            .expect("qualification request");
        let mut substituted = operation;
        substituted.workflow.name = "substituted".into();
        let (payload, hash) =
            governed_browser_operation_payload(&substituted).expect("substituted operation");
        request.operation = payload;
        request.operation_hash = hash;
        let response = execute_browser_request(
            &serde_json::to_vec(&request).expect("serialize substituted request"),
        )
        .await;
        assert_eq!(response.outcome, BrowserRunnerOutcome::Denied);
        assert_eq!(
            response.error_code.as_deref(),
            Some("tool_authority_mismatch")
        );
        assert!(response.observation.is_none());
    }
}
