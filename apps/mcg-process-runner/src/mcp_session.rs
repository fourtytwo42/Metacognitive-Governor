use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use chrono::{DateTime, Utc};
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
use mcg_mcp::{
    LEGACY_MCP_PROTOCOL_VERSION, LegacyHttpMcpClient, MCP_PROTOCOL_VERSION, McpAuthMode,
    McpConnection, McpCredentialStore, McpProtocolMode, McpToolCatalog, McpToolResult,
    McpTransport, OsMcpCredentialStore, StdioMcpClient,
};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::{
    io::AsyncWriteExt as _,
    net::lookup_host,
    process::Command,
    time::{Instant, timeout},
};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{
    GOVERNED_ACTION_OPERATION_HASH_DOMAIN, HttpRunnerAuthorization, MCP_BEARER_SECRET_PROVIDER,
    ProcessRunnerClientError, current_execution_platform, executable_sha256, join_reader,
    mcp_bearer_secret_slot, preserve_platform_runtime_environment, process_runner_sandbox_profile,
    read_bounded, require_regular_unlinked, runner_executable_source, runner_file_name,
    static_source_ref, stop_child, tool_policy_binding, valid_code,
};

pub const MCP_SESSION_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const MCP_SESSION_RUNNER_REQUEST_MAX_BYTES: usize = 12 * 1024 * 1024;
pub const MCP_SESSION_RUNNER_RESPONSE_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const MCP_SESSION_RESULT_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const GOVERNED_MCP_SESSION_AUTHORITY_HASH_DOMAIN: &str = "MetaCog.McpSessionAuthority";
pub const GOVERNED_MCP_SESSION_AUTHORITY_TYPE: &str = "mcg.governed_mcp_session_authority.v1";
pub const GOVERNED_MCP_SESSION_OPERATION_TYPE: &str = "mcg.governed_mcp_session_operation.v1";
pub const GOVERNED_MCP_SESSION_OBSERVATION_TYPE: &str = "mcg.governed_mcp_session_observation.v1";
pub const GOVERNED_MCP_SESSION_RECONCILER_TYPE: &str = "mcg.governed_mcp_session_reconciler.v1";
pub const MCP_SESSION_RUNNER_INPUT_SCHEMA_TYPE: &str = "mcg.mcp_session_runner_input_schema.v1";
pub const MCP_SESSION_RUNNER_OUTPUT_SCHEMA_TYPE: &str = "mcg.mcp_session_runner_output_schema.v1";
pub const MCP_SESSION_RUNNER_RECONCILER_SOURCE_TYPE: &str = "mcg.mcp_session_runner_reconciler.v1";
pub const MCP_SESSION_RUNNER_RECEIPT_TYPE: &str = "mcg.mcp_session_runner_receipt.v1";
pub const MCP_STDIO_EXECUTABLE_SOURCE_TYPE: &str = "mcg.mcp_stdio_executable.v1";
pub const MCP_STDIO_EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.McpStdioExecutable";

const ARGUMENT_MAX_BYTES: usize = 1024 * 1024;
const DETAIL_MAX_BYTES: usize = 4 * 1024;
const STDERR_MAX_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone)]
pub struct McpSessionRunnerClient {
    executable: PathBuf,
    stdio_executable: Option<PathBuf>,
    outer_timeout: Duration,
    capability_evidence: McpSessionRunnerCapabilityEvidence,
}

impl McpSessionRunnerClient {
    /// Selects the fixed capability runner adjacent to Root and measures the exact session
    /// transport identity.
    ///
    /// # Errors
    /// Returns unavailable when the runner or configured stdio executable is unsafe.
    pub fn adjacent_to(
        application: impl AsRef<Path>,
        operation: &GovernedMcpSessionOperation,
    ) -> Result<Self, ProcessRunnerClientError> {
        let parent = application
            .as_ref()
            .parent()
            .ok_or(ProcessRunnerClientError::Unavailable)?;
        Self::with_operation(parent.join(runner_file_name()), operation)
    }

    /// Creates an exact-identity session client for qualification and package injection.
    ///
    /// # Errors
    /// Returns unavailable for malformed operation or executable identity.
    pub fn with_operation(
        executable: impl AsRef<Path>,
        operation: &GovernedMcpSessionOperation,
    ) -> Result<Self, ProcessRunnerClientError> {
        validate_mcp_session_operation(operation, Utc::now())
            .map_err(|_| ProcessRunnerClientError::Unavailable)?;
        let executable = executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name())) {
            return Err(ProcessRunnerClientError::Unavailable);
        }
        require_regular_unlinked(executable)?;
        let stdio_executable = match &operation.connection.transport {
            McpTransport::Stdio { command, .. } => {
                let path = PathBuf::from(command);
                require_regular_unlinked(&path)?;
                Some(path)
            }
            McpTransport::StreamableHttp { .. } => None,
        };
        let stdio_source = stdio_executable
            .as_deref()
            .map(mcp_stdio_executable_source)
            .transpose()?;
        Ok(Self {
            executable: executable.to_path_buf(),
            stdio_executable,
            outer_timeout: Duration::from_millis(operation.timeout_ms.saturating_add(5_000)),
            capability_evidence: McpSessionRunnerCapabilityEvidence {
                runner: runner_executable_source(executable)?,
                stdio_executable: stdio_source,
                platform: current_execution_platform(),
                available_sandbox: mcp_session_runner_sandbox_profile(operation),
                resource_ceiling: ToolResourceLimits {
                    timeout_ms: operation.timeout_ms,
                    max_output_bytes: MCP_SESSION_RESULT_MAX_BYTES,
                    max_memory_bytes: 0,
                    max_child_processes: if matches!(
                        operation.connection.transport,
                        McpTransport::Stdio { .. }
                    ) {
                        64
                    } else {
                        0
                    },
                },
            },
        })
    }

    #[must_use]
    pub fn capability_evidence(&self) -> &McpSessionRunnerCapabilityEvidence {
        &self.capability_evidence
    }

    /// Dispatches one exact session request through a cleared-environment supervised child tree.
    ///
    /// # Errors
    /// Returns a categorical transport error for identity, launch, timeout, output, or contract
    /// failure.
    pub async fn invoke(
        &self,
        request: &McpSessionRunnerRequest,
    ) -> Result<McpSessionRunnerResponse, ProcessRunnerClientError> {
        require_regular_unlinked(&self.executable)?;
        validate_mcp_session_runner_identity(&self.capability_evidence, &self.executable)?;
        if let Some(path) = &self.stdio_executable {
            require_regular_unlinked(path)?;
            if self.capability_evidence.stdio_executable.as_ref()
                != Some(&mcp_stdio_executable_source(path)?)
            {
                return Err(ProcessRunnerClientError::ExecutableChanged);
            }
        }
        let bytes =
            serde_json::to_vec(request).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
        if bytes.len() > MCP_SESSION_RUNNER_REQUEST_MAX_BYTES {
            return Err(ProcessRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command.env_clear();
        preserve_platform_runtime_environment(&mut command);
        command
            .arg("--governed-mcp-session")
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
        let mut stdout_task =
            tokio::spawn(read_bounded(stdout, MCP_SESSION_RUNNER_RESPONSE_MAX_BYTES));
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
        validate_mcp_session_client_response(status.success(), request, &output.0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSessionRunnerCapabilityEvidence {
    pub runner: SourceRef,
    pub stdio_executable: Option<SourceRef>,
    pub platform: ExecutionPlatform,
    pub available_sandbox: SandboxCapabilityProfile,
    pub resource_ceiling: ToolResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSessionRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub envelope: ActionEnvelope,
    pub authorization: HttpRunnerAuthorization,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GovernedMcpSessionOperation {
    pub connection: McpConnection,
    pub catalog: McpToolCatalog,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdio_executable_sha256: Option<String>,
    pub timeout_ms: u64,
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedMcpSessionAuthority {
    pub action_id: ActionId,
    pub actor: RuntimeActorId,
    pub operation_hash: ContentHash,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedMcpSessionReconciler {
    pub connection_id: String,
    pub protocol_version: String,
    pub tool_name: String,
    pub disposition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GovernedMcpSessionObservation {
    pub operation_sha256: String,
    pub connection_id: String,
    pub protocol_version: String,
    pub tool_name: String,
    pub result: McpToolResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpSessionRunnerOutcome {
    Completed,
    NoEffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpSessionRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: McpSessionRunnerOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum McpSessionRunnerProtocolError {
    #[error("governed MCP session action is invalid: {0}")]
    Invalid(&'static str),
    #[error("governed MCP session canonical payload is invalid")]
    Canonical,
}

/// Canonically encodes and hashes one exact catalog-bound MCP session call.
///
/// # Errors
/// Returns an error for malformed, expired, unsupported, or oversized session authority.
pub fn governed_mcp_session_operation_payload(
    operation: &GovernedMcpSessionOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    validate_mcp_session_operation(operation, Utc::now()).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_MCP_SESSION_OPERATION_TYPE)
            .map_err(|error| error.to_string())?,
        &serde_json::to_value(operation).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Binds one exact session action, actor, operation, and expiry.
///
/// # Errors
/// Returns an error for expired or malformed authority.
pub fn governed_mcp_session_authority_binding(
    authority: &GovernedMcpSessionAuthority,
) -> Result<PolicyBinding, String> {
    if authority.expires_at <= Utc::now() {
        return Err("governed MCP session authority is expired".into());
    }
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_MCP_SESSION_AUTHORITY_TYPE)
            .map_err(|error| error.to_string())?,
        &serde_json::to_value(authority).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(GOVERNED_MCP_SESSION_AUTHORITY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Creates the mandatory manual/no-redispatch descriptor for an MCP session effect.
///
/// # Errors
/// Returns an error for malformed operation evidence.
pub fn governed_mcp_session_reconciler(
    operation: &GovernedMcpSessionOperation,
) -> Result<VersionedPayload, String> {
    validate_mcp_session_operation(operation, Utc::now()).map_err(|error| error.to_string())?;
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_MCP_SESSION_RECONCILER_TYPE)
            .map_err(|error| error.to_string())?,
        &serde_json::to_value(GovernedMcpSessionReconciler {
            connection_id: operation.connection.id.clone(),
            protocol_version: operation.catalog.protocol_version.clone(),
            tool_name: operation.tool_name.clone(),
            disposition: "manual_review_no_redispatch".into(),
        })
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Independently validates the immutable session action and conservative effect shape.
///
/// # Errors
/// Returns a protocol error for substitution, expiry, or missing reconciliation authority.
pub fn validate_governed_mcp_session_envelope(
    envelope: &ActionEnvelope,
    now: DateTime<Utc>,
) -> Result<GovernedMcpSessionAuthority, McpSessionRunnerProtocolError> {
    if envelope.operation.type_id.as_str() != GOVERNED_MCP_SESSION_OPERATION_TYPE
        || envelope.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN
        || envelope
            .operation
            .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
            .map_err(|_| McpSessionRunnerProtocolError::Canonical)?
            != envelope.operation_hash
        || envelope.authority.payload.type_id.as_str() != GOVERNED_MCP_SESSION_AUTHORITY_TYPE
        || envelope.authority.content_hash.domain != GOVERNED_MCP_SESSION_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_MCP_SESSION_AUTHORITY_HASH_DOMAIN)
            .map_err(|_| McpSessionRunnerProtocolError::Canonical)?
            != envelope.authority.content_hash
    {
        return Err(McpSessionRunnerProtocolError::Invalid(
            "operation or authority binding",
        ));
    }
    let authority: GovernedMcpSessionAuthority =
        decode_mcp_session_payload(&envelope.authority.payload)?;
    let operation: GovernedMcpSessionOperation = decode_mcp_session_payload(&envelope.operation)?;
    validate_mcp_session_operation(&operation, now)?;
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
                &governed_mcp_session_reconciler(&operation)
                    .map_err(|_| McpSessionRunnerProtocolError::Canonical)?,
            )
        || envelope
            .reconciliation_deadline
            .is_none_or(|deadline| deadline <= now)
    {
        return Err(McpSessionRunnerProtocolError::Invalid(
            "MCP session authority scope",
        ));
    }
    Ok(authority)
}

/// Creates one child request from exact executing action state.
///
/// # Errors
/// Returns a protocol error when the action or execution is not exact and active.
pub fn mcp_session_request_from_authorized_action(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    authorization: HttpRunnerAuthorization,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<McpSessionRunnerRequest, McpSessionRunnerProtocolError> {
    validate_governed_mcp_session_envelope(envelope, now)?;
    if execution.action_id != envelope.id
        || execution.state != ActionExecutionState::Executing
        || execution.executor.is_none()
    {
        return Err(McpSessionRunnerProtocolError::Invalid("execution binding"));
    }
    Ok(McpSessionRunnerRequest {
        protocol_version: MCP_SESSION_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        envelope: envelope.clone(),
        authorization,
        operation: envelope.operation.clone(),
        operation_hash: envelope.operation_hash.clone(),
    })
}

/// Decodes one bounded MCP session observation.
///
/// # Errors
/// Returns a protocol error for wrong type or malformed canonical bytes.
pub fn decode_governed_mcp_session_observation(
    payload: &VersionedPayload,
) -> Result<GovernedMcpSessionObservation, McpSessionRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_MCP_SESSION_OBSERVATION_TYPE {
        return Err(McpSessionRunnerProtocolError::Invalid(
            "MCP session observation type",
        ));
    }
    decode_mcp_session_payload(payload)
}

/// Verifies that one bounded result belongs to the exact authorized session operation.
///
/// # Errors
/// Returns a protocol error for substituted operation, connection, protocol, tool, or size.
pub fn validate_governed_mcp_session_observation(
    operation: &GovernedMcpSessionOperation,
    observation: &GovernedMcpSessionObservation,
) -> Result<(), McpSessionRunnerProtocolError> {
    validate_mcp_session_operation(operation, Utc::now())?;
    let operation_sha256 = operation_sha256(operation)?;
    let result_bytes = serde_json::to_vec(&observation.result)
        .map_err(|_| McpSessionRunnerProtocolError::Canonical)?;
    if observation.operation_sha256 != operation_sha256
        || observation.connection_id != operation.connection.id
        || observation.protocol_version != operation.catalog.protocol_version
        || observation.tool_name != operation.tool_name
        || u64::try_from(result_bytes.len()).unwrap_or(u64::MAX) > operation.max_response_bytes
    {
        return Err(McpSessionRunnerProtocolError::Invalid(
            "MCP session observation binding",
        ));
    }
    Ok(())
}

fn operation_sha256(
    operation: &GovernedMcpSessionOperation,
) -> Result<String, McpSessionRunnerProtocolError> {
    let bytes =
        serde_json::to_vec(operation).map_err(|_| McpSessionRunnerProtocolError::Canonical)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn decode_mcp_session_payload<T: serde::de::DeserializeOwned>(
    payload: &VersionedPayload,
) -> Result<T, McpSessionRunnerProtocolError> {
    let value = decode_canonical_cbor_v1(&payload.bytes)
        .map_err(|_| McpSessionRunnerProtocolError::Canonical)?;
    serde_json::from_value(value).map_err(|_| McpSessionRunnerProtocolError::Canonical)
}

fn validate_mcp_session_operation(
    operation: &GovernedMcpSessionOperation,
    now: DateTime<Utc>,
) -> Result<(), McpSessionRunnerProtocolError> {
    operation
        .connection
        .validate()
        .map_err(|_| McpSessionRunnerProtocolError::Invalid("MCP connection"))?;
    operation
        .catalog
        .validate()
        .map_err(|_| McpSessionRunnerProtocolError::Invalid("MCP catalog"))?;
    let argument_bytes = serde_json::to_vec(&operation.arguments)
        .map_err(|_| McpSessionRunnerProtocolError::Canonical)?;
    if !operation.connection.enabled
        || operation.catalog.connection_id != operation.connection.id
        || operation.catalog.discovered_at > now
        || operation.catalog.expires_at <= now
        || operation.tool_name.is_empty()
        || operation.tool_name.len() > 1024
        || !operation.arguments.is_object()
        || argument_bytes.len() > ARGUMENT_MAX_BYTES
        || operation.timeout_ms == 0
        || operation.timeout_ms > 120_000
        || operation.max_response_bytes == 0
        || operation.max_response_bytes > MCP_SESSION_RESULT_MAX_BYTES
        || !operation
            .catalog
            .tools
            .iter()
            .any(|tool| tool.name == operation.tool_name)
    {
        return Err(McpSessionRunnerProtocolError::Invalid(
            "MCP session operation shape",
        ));
    }
    match &operation.connection.transport {
        McpTransport::StreamableHttp { url } => {
            let parsed = Url::parse(url)
                .map_err(|_| McpSessionRunnerProtocolError::Invalid("MCP HTTP endpoint"))?;
            if operation.stdio_executable_sha256.is_some()
                || operation.catalog.protocol_version != LEGACY_MCP_PROTOCOL_VERSION
                || operation.connection.protocol_mode == McpProtocolMode::Modern20260728
                || !matches!(parsed.scheme(), "http" | "https")
                || parsed.fragment().is_some()
            {
                return Err(McpSessionRunnerProtocolError::Invalid(
                    "legacy MCP HTTP session shape",
                ));
            }
        }
        McpTransport::Stdio { command, .. } => {
            let executable = Path::new(command);
            let protocol_allowed = match operation.connection.protocol_mode {
                McpProtocolMode::Auto => matches!(
                    operation.catalog.protocol_version.as_str(),
                    MCP_PROTOCOL_VERSION | LEGACY_MCP_PROTOCOL_VERSION
                ),
                McpProtocolMode::Modern20260728 => {
                    operation.catalog.protocol_version == MCP_PROTOCOL_VERSION
                }
                McpProtocolMode::Legacy => {
                    operation.catalog.protocol_version == LEGACY_MCP_PROTOCOL_VERSION
                }
            };
            if !executable.is_absolute()
                || executable
                    .components()
                    .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
                || operation
                    .stdio_executable_sha256
                    .as_deref()
                    .is_none_or(|digest| !valid_sha256(digest))
                || operation.connection.auth_mode != McpAuthMode::None
                || !protocol_allowed
            {
                return Err(McpSessionRunnerProtocolError::Invalid(
                    "MCP stdio session shape",
                ));
            }
        }
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Measures one exact regular, unlinked stdio server executable.
///
/// # Errors
/// Returns unavailable when the path is unsafe, unreadable, or unmeasurable.
pub fn mcp_stdio_executable_source(path: &Path) -> Result<SourceRef, ProcessRunnerClientError> {
    require_regular_unlinked(path)?;
    Ok(SourceRef {
        source_type: StableTypeId::parse(MCP_STDIO_EXECUTABLE_SOURCE_TYPE)
            .map_err(|_| ProcessRunnerClientError::Unavailable)?,
        locator: path
            .to_str()
            .ok_or(ProcessRunnerClientError::Unavailable)?
            .to_owned(),
        content_hash: ContentHash {
            algorithm: mcg_contracts::HashAlgorithm::Sha256,
            domain: MCP_STDIO_EXECUTABLE_HASH_DOMAIN.into(),
            digest_hex: executable_sha256(path)?,
        },
    })
}

#[must_use]
pub fn mcp_session_runner_sandbox_profile(
    operation: &GovernedMcpSessionOperation,
) -> SandboxCapabilityProfile {
    let mut profile = process_runner_sandbox_profile();
    profile.process_tree = if cfg!(windows) {
        IsolationGuarantee::Hard
    } else {
        IsolationGuarantee::BestEffort
    };
    if matches!(
        operation.connection.transport,
        McpTransport::StreamableHttp { .. }
    ) {
        profile.network = IsolationGuarantee::BestEffort;
        profile.secrets = if operation.connection.auth_mode == McpAuthMode::BearerToken {
            IsolationGuarantee::BestEffort
        } else {
            IsolationGuarantee::Unavailable
        };
    }
    profile
}

#[must_use]
pub fn mcp_session_runner_input_schema_source() -> SourceRef {
    static_source_ref(
        MCP_SESSION_RUNNER_INPUT_SCHEMA_TYPE,
        GOVERNED_MCP_SESSION_OPERATION_TYPE,
        "MetaCog.McpSessionRunnerInputSchema",
    )
}

#[must_use]
pub fn mcp_session_runner_output_schema_source() -> SourceRef {
    static_source_ref(
        MCP_SESSION_RUNNER_OUTPUT_SCHEMA_TYPE,
        GOVERNED_MCP_SESSION_OBSERVATION_TYPE,
        "MetaCog.McpSessionRunnerOutputSchema",
    )
}

#[must_use]
pub fn mcp_session_runner_reconciler_source() -> SourceRef {
    static_source_ref(
        MCP_SESSION_RUNNER_RECONCILER_SOURCE_TYPE,
        GOVERNED_MCP_SESSION_RECONCILER_TYPE,
        "MetaCog.McpSessionRunnerReconciler",
    )
}

/// Derives one deterministic Tool manifest for an exact MCP transport and catalog snapshot.
///
/// # Errors
/// Returns an error for inconsistent evidence, operation, or resource limits.
pub fn governed_mcp_session_tool_manifest(
    evidence: &McpSessionRunnerCapabilityEvidence,
    operation: &GovernedMcpSessionOperation,
    limits: ToolResourceLimits,
) -> Result<ToolManifest, String> {
    validate_mcp_session_operation(operation, Utc::now()).map_err(|error| error.to_string())?;
    limits.validate().map_err(|error| error.to_string())?;
    if !limits.fits_within(evidence.resource_ceiling)
        || evidence.available_sandbox != mcp_session_runner_sandbox_profile(operation)
    {
        return Err("MCP session Tool evidence or limits differ from the operation".into());
    }
    let (filesystem, network, secret_slots, process_spawn) = match &operation.connection.transport {
        McpTransport::StreamableHttp { url } => {
            if evidence.stdio_executable.is_some() {
                return Err(
                    "HTTP MCP session unexpectedly carries stdio executable evidence".into(),
                );
            }
            let parsed = Url::parse(url).map_err(|error| error.to_string())?;
            let slots = if operation.connection.auth_mode == McpAuthMode::BearerToken {
                vec![mcp_bearer_secret_slot(&operation.connection.id)?]
            } else {
                Vec::new()
            };
            (
                Vec::new(),
                vec![NetworkCapability {
                    destination: parsed.origin().ascii_serialization(),
                }],
                slots,
                false,
            )
        }
        McpTransport::Stdio { command, .. } => {
            let source = evidence
                .stdio_executable
                .as_ref()
                .ok_or("stdio MCP session has no executable evidence")?;
            let expected_digest = operation
                .stdio_executable_sha256
                .as_deref()
                .ok_or("stdio MCP session has no executable digest")?;
            if source.locator != *command || source.content_hash.digest_hex != expected_digest {
                return Err("stdio MCP executable evidence differs from the operation".into());
            }
            let parent = Path::new(command)
                .parent()
                .and_then(Path::to_str)
                .ok_or("stdio MCP executable parent is invalid")?
                .to_owned();
            (
                vec![FilesystemCapability {
                    root: parent,
                    access: FilesystemAccess::Read,
                }],
                Vec::new(),
                Vec::new(),
                true,
            )
        }
    };
    let material = serde_json::to_vec(&serde_json::json!({
        "runner": evidence.runner,
        "stdio_executable": evidence.stdio_executable,
        "platform": evidence.platform,
        "connection": operation.connection,
        "catalog": operation.catalog,
        "tool_name": operation.tool_name,
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
        name: "mcp.session.call".into(),
        summary: "Governed catalog-bound MCP session and one exact tool call".into(),
        runner: evidence.runner.clone(),
        input_schema: mcp_session_runner_input_schema_source(),
        output_schema: mcp_session_runner_output_schema_source(),
        maximum_effect_class: EffectClass::Irreversible,
        required_capabilities: ToolCapabilitySet {
            filesystem,
            network,
            secret_slots,
            process_spawn,
        },
        resource_ceiling: limits,
        sandbox_requirement: evidence.available_sandbox,
        platform_support: vec![evidence.platform],
        receipt_semantics: StableTypeId::parse(MCP_SESSION_RUNNER_RECEIPT_TYPE)
            .map_err(|error| error.to_string())?,
        reconciler: Some(mcp_session_runner_reconciler_source()),
    };
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Derives exact Root policy for one MCP session manifest.
///
/// # Errors
/// Returns an error when the manifest reference is invalid.
pub fn governed_mcp_session_tool_policy(manifest: &ToolManifest) -> Result<ToolPolicy, String> {
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

/// Confirms the fixed adjacent runner identity represented by capability evidence.
///
/// # Errors
/// Returns unavailable if the current runner cannot be measured exactly.
pub fn validate_mcp_session_runner_identity(
    evidence: &McpSessionRunnerCapabilityEvidence,
    runner: &Path,
) -> Result<(), ProcessRunnerClientError> {
    if evidence.runner != runner_executable_source(runner)?
        || evidence.platform != current_execution_platform()
    {
        return Err(ProcessRunnerClientError::ExecutableChanged);
    }
    Ok(())
}

fn validate_mcp_session_client_response(
    process_succeeded: bool,
    request: &McpSessionRunnerRequest,
    bytes: &[u8],
) -> Result<McpSessionRunnerResponse, ProcessRunnerClientError> {
    let response: McpSessionRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
    if response.protocol_version != MCP_SESSION_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.action_id != Some(request.action_id)
        || response.execution_id != Some(request.execution_id)
        || process_succeeded != (response.outcome == McpSessionRunnerOutcome::Completed)
        || response.detail.is_empty()
        || response.detail.len() > DETAIL_MAX_BYTES
        || response.started_at > response.finished_at
    {
        return Err(ProcessRunnerClientError::InvalidResponse);
    }
    match response.outcome {
        McpSessionRunnerOutcome::Completed => {
            let Some(observation) = &response.observation else {
                return Err(ProcessRunnerClientError::InvalidResponse);
            };
            let operation: GovernedMcpSessionOperation =
                decode_mcp_session_payload(&request.operation)
                    .map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
            let observation = decode_governed_mcp_session_observation(observation)
                .map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
            if response.error_code.is_some()
                || validate_governed_mcp_session_observation(&operation, &observation).is_err()
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

#[derive(Debug)]
struct McpSessionFailure {
    outcome: McpSessionRunnerOutcome,
    code: &'static str,
    detail: String,
}

impl McpSessionFailure {
    fn denied(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: McpSessionRunnerOutcome::Denied,
            code,
            detail: detail.into(),
        }
    }

    fn no_effect(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: McpSessionRunnerOutcome::NoEffectConfirmed,
            code,
            detail: detail.into(),
        }
    }

    fn unknown(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: McpSessionRunnerOutcome::OutcomeUnknown,
            code,
            detail: detail.into(),
        }
    }
}

/// Executes one MCP session request using the operating-system credential store.
pub async fn execute_mcp_session_request(bytes: &[u8]) -> McpSessionRunnerResponse {
    execute_mcp_session_request_with_store(bytes, &OsMcpCredentialStore).await
}

/// Executes one MCP session request with an injected credential store for deterministic tests.
pub async fn execute_mcp_session_request_with_store<S: McpCredentialStore + ?Sized>(
    bytes: &[u8],
    store: &S,
) -> McpSessionRunnerResponse {
    let started_at = Utc::now();
    let Ok(request) = serde_json::from_slice::<McpSessionRunnerRequest>(bytes) else {
        return mcp_session_error_response(
            None,
            started_at,
            McpSessionFailure::denied("request_invalid", "MCP session request is invalid"),
        );
    };
    if bytes.len() > MCP_SESSION_RUNNER_REQUEST_MAX_BYTES {
        return mcp_session_error_response(
            Some(&request),
            started_at,
            McpSessionFailure::denied("request_too_large", "MCP session request exceeds its bound"),
        );
    }
    match execute_mcp_session(&request, store).await {
        Ok(observation) => McpSessionRunnerResponse {
            protocol_version: MCP_SESSION_RUNNER_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            action_id: Some(request.action_id),
            execution_id: Some(request.execution_id),
            outcome: McpSessionRunnerOutcome::Completed,
            observation: Some(observation),
            error_code: None,
            detail: "catalog-bound MCP session completed with bounded evidence".into(),
            started_at,
            finished_at: Utc::now(),
        },
        Err(error) => mcp_session_error_response(Some(&request), started_at, error),
    }
}

fn mcp_session_error_response(
    request: Option<&McpSessionRunnerRequest>,
    started_at: DateTime<Utc>,
    error: McpSessionFailure,
) -> McpSessionRunnerResponse {
    let mut detail = error.detail;
    detail.truncate(DETAIL_MAX_BYTES);
    McpSessionRunnerResponse {
        protocol_version: MCP_SESSION_RUNNER_PROTOCOL_VERSION,
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

async fn execute_mcp_session<S: McpCredentialStore + ?Sized>(
    request: &McpSessionRunnerRequest,
    store: &S,
) -> Result<VersionedPayload, McpSessionFailure> {
    validate_mcp_session_tool_authorization(request, Utc::now())?;
    let operation: GovernedMcpSessionOperation = decode_mcp_session_payload(&request.operation)
        .map_err(|error| McpSessionFailure::denied("operation_invalid", error.to_string()))?;
    let (result, secrets) = execute_catalog_bound_call(&operation, store).await?;
    let result = redact_mcp_result(result, &secrets).map_err(|_| {
        McpSessionFailure::unknown(
            "result_redaction_failed",
            "MCP result could not be safely redacted after possible tool contact",
        )
    })?;
    encode_mcp_session_observation(&operation, result)
}

async fn execute_catalog_bound_call<S: McpCredentialStore + ?Sized>(
    operation: &GovernedMcpSessionOperation,
    store: &S,
) -> Result<(McpToolResult, Vec<Zeroizing<String>>), McpSessionFailure> {
    let result = match &operation.connection.transport {
        McpTransport::StreamableHttp { url } => {
            let (result, secret) = execute_legacy_http_session(operation, url, store).await?;
            return Ok((result, secret.into_iter().collect()));
        }
        McpTransport::Stdio { command, .. } => execute_stdio_session(operation, command).await?,
    };
    Ok((result, Vec::new()))
}

async fn execute_legacy_http_session<S: McpCredentialStore + ?Sized>(
    operation: &GovernedMcpSessionOperation,
    url: &str,
    store: &S,
) -> Result<(McpToolResult, Option<Zeroizing<String>>), McpSessionFailure> {
    let parsed = Url::parse(url)
        .map_err(|_| McpSessionFailure::denied("endpoint_invalid", "MCP endpoint is invalid"))?;
    let client = confined_http_client(&parsed).await?;
    let credential = if operation.connection.auth_mode == McpAuthMode::BearerToken {
        Some(
            store
                .load(&operation.connection.id)
                .map_err(|_| {
                    McpSessionFailure::denied(
                        "credential_unavailable",
                        "authorized MCP credential store is unavailable",
                    )
                })?
                .ok_or_else(|| {
                    McpSessionFailure::denied(
                        "credential_unavailable",
                        "authorized MCP credential is unavailable",
                    )
                })?,
        )
    } else {
        None
    };
    let secret = credential
        .as_ref()
        .map(|credential| Zeroizing::new(credential.expose_to_host_executor().to_owned()));
    let client = LegacyHttpMcpClient::new_with_http(
        operation.connection.clone(),
        credential,
        client,
        usize::try_from(operation.max_response_bytes).map_err(|_| {
            McpSessionFailure::denied("response_limit_invalid", "response limit is invalid")
        })?,
    )
    .map_err(|_| {
        McpSessionFailure::no_effect(
            "client_unavailable",
            "legacy MCP client could not be constructed",
        )
    })?;
    let result = timeout(
        Duration::from_millis(operation.timeout_ms),
        client.call_tool_against_catalog(
            &operation.catalog,
            &operation.tool_name,
            operation.arguments.clone(),
        ),
    )
    .await
    .map_err(|_| {
        McpSessionFailure::unknown(
            "session_timeout_unknown",
            "MCP HTTP session timed out after possible tool contact",
        )
    })?
    .map_err(|_| {
        McpSessionFailure::unknown(
            "session_outcome_unknown",
            "MCP HTTP session failed after possible tool contact",
        )
    })?;
    Ok((result, secret))
}

async fn execute_stdio_session(
    operation: &GovernedMcpSessionOperation,
    command: &str,
) -> Result<McpToolResult, McpSessionFailure> {
    let executable = Path::new(command);
    require_regular_unlinked(executable).map_err(|_| {
        McpSessionFailure::denied(
            "stdio_identity_unavailable",
            "MCP stdio executable is unavailable",
        )
    })?;
    let measured = mcp_stdio_executable_source(executable).map_err(|_| {
        McpSessionFailure::denied(
            "stdio_identity_unavailable",
            "MCP stdio executable cannot be measured",
        )
    })?;
    if operation.stdio_executable_sha256.as_deref()
        != Some(measured.content_hash.digest_hex.as_str())
    {
        return Err(McpSessionFailure::denied(
            "stdio_identity_changed",
            "MCP stdio executable identity changed before launch",
        ));
    }
    let client = StdioMcpClient::new(operation.connection.clone()).map_err(|_| {
        McpSessionFailure::no_effect(
            "client_unavailable",
            "stdio MCP client could not be constructed",
        )
    })?;
    timeout(
        Duration::from_millis(operation.timeout_ms),
        client.call_tool_against_catalog(
            &operation.catalog,
            &operation.tool_name,
            operation.arguments.clone(),
        ),
    )
    .await
    .map_err(|_| {
        McpSessionFailure::unknown(
            "session_timeout_unknown",
            "MCP stdio session timed out after possible tool contact",
        )
    })?
    .map_err(|_| {
        McpSessionFailure::unknown(
            "session_outcome_unknown",
            "MCP stdio session failed after possible tool contact",
        )
    })
}

fn encode_mcp_session_observation(
    operation: &GovernedMcpSessionOperation,
    result: McpToolResult,
) -> Result<VersionedPayload, McpSessionFailure> {
    let observation = GovernedMcpSessionObservation {
        operation_sha256: operation_sha256(operation)
            .map_err(|error| McpSessionFailure::unknown("result_invalid", error.to_string()))?,
        connection_id: operation.connection.id.clone(),
        protocol_version: operation.catalog.protocol_version.clone(),
        tool_name: operation.tool_name.clone(),
        result,
    };
    validate_governed_mcp_session_observation(operation, &observation)
        .map_err(|error| McpSessionFailure::unknown("result_invalid", error.to_string()))?;
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_MCP_SESSION_OBSERVATION_TYPE)
            .map_err(|error| McpSessionFailure::unknown("result_invalid", error.to_string()))?,
        &serde_json::to_value(observation)
            .map_err(|error| McpSessionFailure::unknown("result_invalid", error.to_string()))?,
    )
    .map_err(|error| McpSessionFailure::unknown("result_invalid", error.to_string()))
}

fn redact_mcp_result(
    result: McpToolResult,
    secrets: &[Zeroizing<String>],
) -> Result<McpToolResult, serde_json::Error> {
    let mut value = serde_json::to_value(result)?;
    redact_json_strings(&mut value, secrets);
    serde_json::from_value(value)
}

fn redact_json_strings(value: &mut serde_json::Value, secrets: &[Zeroizing<String>]) {
    match value {
        serde_json::Value::String(text) => {
            for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
                let secret = secret.as_str();
                *text = text.replace(&format!("Bearer {secret}"), "[REDACTED]");
                *text = text.replace(secret, "[REDACTED]");
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_json_strings(value, secrets);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                redact_json_strings(value, secrets);
            }
        }
        _ => {}
    }
}

async fn confined_http_client(url: &Url) -> Result<reqwest::Client, McpSessionFailure> {
    let resolved = resolve_and_validate_destination(url).await?;
    let host = url
        .host_str()
        .ok_or_else(|| McpSessionFailure::denied("endpoint_invalid", "MCP endpoint has no host"))?;
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve(host, resolved)
        .build()
        .map_err(|_| {
            McpSessionFailure::no_effect(
                "client_unavailable",
                "confined MCP HTTP client could not be constructed",
            )
        })
}

async fn resolve_and_validate_destination(url: &Url) -> Result<SocketAddr, McpSessionFailure> {
    let host = url
        .host_str()
        .ok_or_else(|| McpSessionFailure::denied("endpoint_invalid", "MCP endpoint has no host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| McpSessionFailure::denied("endpoint_invalid", "MCP endpoint has no port"))?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_unroutable(ip) {
            return Err(McpSessionFailure::denied(
                "destination_denied",
                "MCP destination is unspecified or multicast",
            ));
        }
        return Ok(SocketAddr::new(ip, port));
    }
    let addresses = lookup_host((host, port))
        .await
        .map_err(|_| {
            McpSessionFailure::no_effect("dns_failed", "MCP destination DNS lookup failed")
        })?
        .collect::<BTreeSet<_>>();
    if addresses.is_empty() {
        return Err(McpSessionFailure::no_effect(
            "dns_empty",
            "MCP destination DNS lookup returned no addresses",
        ));
    }
    if addresses.iter().any(|address| is_non_public(address.ip())) {
        return Err(McpSessionFailure::denied(
            "dns_scope_denied",
            "MCP hostname resolved to a non-public destination; use an explicitly granted IP endpoint for local access",
        ));
    }
    addresses.into_iter().next().ok_or_else(|| {
        McpSessionFailure::no_effect(
            "dns_empty",
            "MCP destination DNS lookup returned no address",
        )
    })
}

fn is_unroutable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_unspecified() || ip.is_multicast() || ip == Ipv4Addr::BROADCAST,
        IpAddr::V6(ip) => ip.is_unspecified() || ip.is_multicast(),
    }
}

fn is_non_public(ip: IpAddr) -> bool {
    if is_unroutable(ip) {
        return true;
    }
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || octets[0] == 0
                || octets[0] == 100 && (64..=127).contains(&octets[1])
                || octets[0] == 169 && octets[1] == 254
                || octets[0] == 192 && octets[1] == 0 && octets[2] == 0
                || octets[0] == 192 && octets[1] == 0 && octets[2] == 2
                || octets[0] == 192 && octets[1] == 88 && octets[2] == 99
                || octets[0] == 198 && (18..=19).contains(&octets[1])
                || octets[0] == 198 && octets[1] == 51 && octets[2] == 100
                || octets[0] == 203 && octets[1] == 0 && octets[2] == 113
                || octets[0] >= 224
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_non_public(IpAddr::V4(mapped)))
        }
    }
}

#[allow(clippy::too_many_lines)]
fn validate_mcp_session_tool_authorization(
    request: &McpSessionRunnerRequest,
    now: DateTime<Utc>,
) -> Result<(), McpSessionFailure> {
    let authorization = &request.authorization;
    authorization
        .manifest
        .validate()
        .map_err(|error| McpSessionFailure::denied("tool_manifest_invalid", error.to_string()))?;
    authorization
        .proposal
        .validate()
        .map_err(|error| McpSessionFailure::denied("tool_proposal_invalid", error.to_string()))?;
    authorization
        .policy
        .validate()
        .map_err(|error| McpSessionFailure::denied("tool_policy_invalid", error.to_string()))?;
    authorization
        .permit
        .validate()
        .map_err(|error| McpSessionFailure::denied("tool_permit_invalid", error.to_string()))?;
    authorization
        .permit_ref
        .validate()
        .map_err(|error| McpSessionFailure::denied("tool_permit_ref_invalid", error.to_string()))?;
    validate_governed_mcp_session_envelope(&request.envelope, now)
        .map_err(|error| McpSessionFailure::denied("action_envelope_invalid", error.to_string()))?;
    let operation: GovernedMcpSessionOperation = decode_mcp_session_payload(&request.operation)
        .map_err(|error| McpSessionFailure::denied("operation_invalid", error.to_string()))?;
    let current_runner = std::env::current_exe().map_err(|error| {
        McpSessionFailure::denied("runner_identity_unavailable", error.to_string())
    })?;
    let stdio_executable = match &operation.connection.transport {
        McpTransport::Stdio { command, .. } => Some(
            mcp_stdio_executable_source(Path::new(command)).map_err(|_| {
                McpSessionFailure::denied(
                    "stdio_identity_unavailable",
                    "MCP stdio executable cannot be measured",
                )
            })?,
        ),
        McpTransport::StreamableHttp { .. } => None,
    };
    let evidence = McpSessionRunnerCapabilityEvidence {
        runner: runner_executable_source(&current_runner).map_err(|_| {
            McpSessionFailure::denied(
                "runner_identity_unavailable",
                "MCP session runner cannot be measured",
            )
        })?,
        stdio_executable,
        platform: current_execution_platform(),
        available_sandbox: mcp_session_runner_sandbox_profile(&operation),
        resource_ceiling: ToolResourceLimits {
            timeout_ms: operation.timeout_ms,
            max_output_bytes: MCP_SESSION_RESULT_MAX_BYTES,
            max_memory_bytes: 0,
            max_child_processes: if matches!(
                operation.connection.transport,
                McpTransport::Stdio { .. }
            ) {
                64
            } else {
                0
            },
        },
    };
    let expected_limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_response_bytes,
        max_memory_bytes: 0,
        max_child_processes: evidence.resource_ceiling.max_child_processes,
    };
    let expected_manifest =
        governed_mcp_session_tool_manifest(&evidence, &operation, expected_limits)
            .map_err(|error| McpSessionFailure::denied("tool_manifest_invalid", error))?;
    let expected_policy = governed_mcp_session_tool_policy(&expected_manifest)
        .map_err(|error| McpSessionFailure::denied("tool_policy_invalid", error))?;
    let expected_policy_binding = tool_policy_binding(&expected_policy)
        .map_err(|error| McpSessionFailure::denied("tool_policy_invalid", error))?;
    let manifest_ref = expected_manifest
        .versioned_ref()
        .map_err(|error| McpSessionFailure::denied("tool_manifest_invalid", error.to_string()))?;
    let proposal_hash = tool_invocation_proposal_hash(&authorization.proposal)
        .map_err(|error| McpSessionFailure::denied("tool_proposal_invalid", error.to_string()))?;
    let permit_hash = tool_invocation_permit_hash(&authorization.permit)
        .map_err(|error| McpSessionFailure::denied("tool_permit_invalid", error.to_string()))?;
    let operation_hash = request
        .operation
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| McpSessionFailure::denied("operation_invalid", error.to_string()))?;
    let expected_slots = if operation.connection.auth_mode == McpAuthMode::BearerToken {
        vec![
            mcp_bearer_secret_slot(&operation.connection.id)
                .map_err(|error| McpSessionFailure::denied("credential_invalid", error))?,
        ]
    } else {
        Vec::new()
    };
    if request.protocol_version != MCP_SESSION_RUNNER_PROTOCOL_VERSION
        || request.action_id != request.envelope.id
        || request.operation != request.envelope.operation
        || request.operation_hash != request.envelope.operation_hash
        || request.operation_hash != operation_hash
        || authorization.manifest != expected_manifest
        || authorization.policy != expected_policy
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
        || authorization.permit.capabilities != expected_manifest.required_capabilities
        || authorization.proposal.requested_capabilities != expected_manifest.required_capabilities
        || authorization.permit.limits != expected_limits
        || authorization.proposal.requested_limits != expected_limits
        || authorization.proposal.required_sandbox != expected_manifest.sandbox_requirement
        || authorization.permit.policy != expected_policy_binding
        || authorization.permit.platform != current_execution_platform()
        || authorization.proposal.platform != current_execution_platform()
        || authorization.permit.issued_at > now
        || authorization.permit.expires_at <= now
        || authorization.proposal.deadline != authorization.permit.expires_at
        || authorization.permit_ref.source_type.as_str() != ToolInvocationPermit::TYPE_ID
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != permit_hash
        || authorization.permit.secret_handles.len() != expected_slots.len()
        || authorization.secret_handles.len() != expected_slots.len()
    {
        return Err(McpSessionFailure::denied(
            "tool_authority_mismatch",
            "request differs from exact Root-issued MCP session Tool authority",
        ));
    }
    for (index, slot) in expected_slots.iter().enumerate() {
        let binding = &authorization.secret_handles[index];
        if binding.handle_id != authorization.permit.secret_handles[index]
            || binding.permit_id != authorization.permit.id
            || binding.provider != MCP_BEARER_SECRET_PROVIDER
            || binding.key != operation.connection.id
            || &binding.slot != slot
            || binding.expires_at != authorization.permit.expires_at
            || binding.expires_at <= now
        {
            return Err(McpSessionFailure::denied(
                "credential_handle_mismatch",
                "MCP credential handle binding is substituted",
            ));
        }
    }
    Ok(())
}

/// Builds one self-contained request for no-credential runner and package qualification.
/// Production dispatch must use Protected Root-persisted authority and handle bindings.
///
/// # Errors
/// Returns an error for invalid operation, credential-bearing qualification, or inconsistent
/// manifest/proposal/permit evidence.
#[allow(clippy::too_many_lines)]
pub fn mcp_session_qualification_request_with_evidence(
    operation: &GovernedMcpSessionOperation,
    evidence: &McpSessionRunnerCapabilityEvidence,
) -> Result<McpSessionRunnerRequest, String> {
    if operation.connection.auth_mode != McpAuthMode::None {
        return Err("credential-bearing MCP sessions require Root-issued opaque handles".into());
    }
    let now = Utc::now();
    let deadline = now
        + chrono::Duration::milliseconds(
            i64::try_from(operation.timeout_ms.saturating_add(5_000))
                .map_err(|error| error.to_string())?,
        );
    let action_id = ActionId::new();
    let actor = RuntimeActorId::new();
    let (payload, operation_hash) = governed_mcp_session_operation_payload(operation)?;
    let limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_response_bytes,
        max_memory_bytes: 0,
        max_child_processes: evidence.resource_ceiling.max_child_processes,
    };
    let manifest = governed_mcp_session_tool_manifest(evidence, operation, limits)?;
    let policy = governed_mcp_session_tool_policy(&manifest)?;
    let authority = governed_mcp_session_authority_binding(&GovernedMcpSessionAuthority {
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
        idempotency_key: format!("mcp-session-runner-qualification-{action_id}"),
        compensation: None,
        compensates_action: None,
        reconciler: Some(governed_mcp_session_reconciler(operation)?),
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
            media_type: "application/vnd.metacog.governed-mcp-session-operation+cbor".into(),
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
    let authorization = HttpRunnerAuthorization {
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
        secret_handles: Vec::new(),
    };
    mcp_session_request_from_authorized_action(
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
    use mcg_mcp::{MCP_CONNECTION_CONTRACT_VERSION, McpServerInfo, McpToolDescriptor};

    fn operation(transport: McpTransport, protocol: &str) -> GovernedMcpSessionOperation {
        let now = Utc::now();
        GovernedMcpSessionOperation {
            connection: McpConnection {
                contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                id: "fixture".into(),
                display_name: "Fixture".into(),
                auth_mode: McpAuthMode::None,
                protocol_mode: if protocol == MCP_PROTOCOL_VERSION {
                    McpProtocolMode::Modern20260728
                } else {
                    McpProtocolMode::Legacy
                },
                transport,
                enabled: true,
            },
            catalog: McpToolCatalog {
                connection_id: "fixture".into(),
                protocol_version: protocol.into(),
                server_info: McpServerInfo {
                    name: "fixture".into(),
                    version: Some("1".into()),
                },
                capabilities: serde_json::json!({"tools": {}}),
                tools: vec![McpToolDescriptor {
                    name: "echo".into(),
                    title: None,
                    description: None,
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: None,
                    annotations: serde_json::json!({"readOnlyHint": true}),
                }],
                discovered_at: now - chrono::Duration::seconds(1),
                expires_at: now + chrono::Duration::minutes(5),
            },
            tool_name: "echo".into(),
            arguments: serde_json::json!({"message": "hello"}),
            stdio_executable_sha256: None,
            timeout_ms: 30_000,
            max_response_bytes: 64 * 1024,
        }
    }

    #[test]
    fn http_sessions_require_legacy_catalogs_and_no_stdio_identity() {
        let mut operation = operation(
            McpTransport::StreamableHttp {
                url: "https://mcp.example.test/rpc".into(),
            },
            LEGACY_MCP_PROTOCOL_VERSION,
        );
        assert!(governed_mcp_session_operation_payload(&operation).is_ok());
        operation.catalog.protocol_version = MCP_PROTOCOL_VERSION.into();
        assert!(governed_mcp_session_operation_payload(&operation).is_err());
        operation.catalog.protocol_version = LEGACY_MCP_PROTOCOL_VERSION.into();
        operation.stdio_executable_sha256 = Some("0".repeat(64));
        assert!(governed_mcp_session_operation_payload(&operation).is_err());
    }

    #[test]
    fn stdio_sessions_require_absolute_exact_executable_identity() {
        let mut operation = operation(
            McpTransport::Stdio {
                command: if cfg!(windows) {
                    "C:\\fixture\\server.exe".into()
                } else {
                    "/fixture/server".into()
                },
                args: vec!["--stdio".into()],
            },
            MCP_PROTOCOL_VERSION,
        );
        assert!(governed_mcp_session_operation_payload(&operation).is_err());
        operation.stdio_executable_sha256 = Some("a".repeat(64));
        assert!(governed_mcp_session_operation_payload(&operation).is_ok());
        let McpTransport::Stdio { command, .. } = &mut operation.connection.transport else {
            unreachable!();
        };
        *command = "relative-server".into();
        assert!(governed_mcp_session_operation_payload(&operation).is_err());
    }

    #[test]
    fn observations_are_bound_to_the_exact_session_operation() {
        let operation = operation(
            McpTransport::StreamableHttp {
                url: "https://mcp.example.test/rpc".into(),
            },
            LEGACY_MCP_PROTOCOL_VERSION,
        );
        let mut observation = GovernedMcpSessionObservation {
            operation_sha256: operation_sha256(&operation).expect("operation hash"),
            connection_id: operation.connection.id.clone(),
            protocol_version: operation.catalog.protocol_version.clone(),
            tool_name: operation.tool_name.clone(),
            result: McpToolResult {
                is_error: false,
                content: Vec::new(),
                structured_content: Some(serde_json::json!({"ok": true})),
                server_info: None,
            },
        };
        assert!(validate_governed_mcp_session_observation(&operation, &observation).is_ok());
        observation.tool_name = "substituted".into();
        assert!(validate_governed_mcp_session_observation(&operation, &observation).is_err());
    }

    #[test]
    fn nested_results_redact_raw_and_bearer_credential_forms() {
        let secret = Zeroizing::new("super-secret-token".to_owned());
        let result = McpToolResult {
            is_error: false,
            content: vec![serde_json::json!({
                "text": "Bearer super-secret-token",
                "nested": ["prefix super-secret-token suffix"],
            })],
            structured_content: Some(serde_json::json!({
                "credential": "super-secret-token",
            })),
            server_info: None,
        };
        let redacted = redact_mcp_result(result, &[secret]).expect("redact MCP result");
        let encoded = serde_json::to_string(&redacted).expect("encode redacted result");
        assert!(!encoded.contains("super-secret-token"));
        assert!(encoded.matches("[REDACTED]").count() >= 3);
    }
}
