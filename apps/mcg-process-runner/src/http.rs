use std::{
    collections::{BTreeSet, HashSet},
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId,
    ArtifactRef, ContentHash, EffectClass, ExecutionPlatform, IsolationGuarantee,
    NetworkCapability, PROTOCOL_V1, PermitId, PolicyBinding, RuntimeActorId, RuntimeIdentity,
    RuntimeInstanceId, SandboxCapabilityProfile, SecretHandleId, SourceRef, StableTypeId,
    TaskContractId, ToolCapabilitySet, ToolInvocationPermit, ToolInvocationProposal, ToolManifest,
    ToolManifestId, ToolPolicy, ToolResourceLimits, VersionedPayload, VersionedRef,
    decode_canonical_cbor_v1, tool_invocation_permit_hash, tool_invocation_proposal_hash,
};
use mcg_mcp::{McpCredentialStore, MemoryMcpCredentialStore, OsMcpCredentialStore};
use mcg_providers::{OsProviderCredentialStore, ProviderCredentialStore, ProviderSecretSlot};
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use reqwest::{
    Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue as ReqwestHeaderValue, LOCATION},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::{io::AsyncWriteExt as _, net::lookup_host, process::Command, time::Instant};
use uuid::Uuid;
use zeroize::Zeroize;

use super::{
    GOVERNED_ACTION_OPERATION_HASH_DOMAIN, ProcessRunnerClientError, current_execution_platform,
    join_reader, preserve_platform_runtime_environment, process_runner_sandbox_profile,
    read_bounded, require_regular_unlinked, runner_executable_source, runner_file_name,
    static_source_ref, stop_child, tool_policy_binding, valid_code,
};

pub const HTTP_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const HTTP_RUNNER_REQUEST_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const HTTP_RUNNER_RESPONSE_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const HTTP_RUNNER_BODY_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const GOVERNED_HTTP_AUTHORITY_HASH_DOMAIN: &str = "MetaCog.HttpAuthority";
pub const GOVERNED_HTTP_AUTHORITY_TYPE: &str = "mcg.governed_http_authority.v1";
pub const GOVERNED_HTTP_OPERATION_TYPE: &str = "mcg.governed_http_operation.v1";
pub const GOVERNED_HTTP_OBSERVATION_TYPE: &str = "mcg.governed_http_observation.v1";
pub const GOVERNED_HTTP_RECONCILER_TYPE: &str = "mcg.governed_http_reconciler.v1";
pub const HTTP_RUNNER_INPUT_SCHEMA_TYPE: &str = "mcg.http_runner_input_schema.v1";
pub const HTTP_RUNNER_OUTPUT_SCHEMA_TYPE: &str = "mcg.http_runner_output_schema.v1";
pub const HTTP_RUNNER_RECONCILER_SOURCE_TYPE: &str = "mcg.http_runner_reconciler.v1";
pub const HTTP_RUNNER_RECEIPT_TYPE: &str = "mcg.http_runner_receipt.v1";
pub const MCP_BEARER_SECRET_PROVIDER: &str = "@mcp";

const DETAIL_MAX_BYTES: usize = 4 * 1024;
const STDERR_MAX_BYTES: usize = 4 * 1024;
const MAX_HEADERS: usize = 32;
const MAX_HEADER_VALUE_BYTES: usize = 64 * 1024;
const MAX_REDIRECTS: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRunnerAuthorization {
    pub manifest: ToolManifest,
    pub proposal: ToolInvocationProposal,
    pub proposal_hash: ContentHash,
    pub policy: ToolPolicy,
    pub permit: ToolInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
    #[serde(default)]
    pub secret_handles: Vec<HttpRunnerSecretHandleBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRunnerSecretHandleBinding {
    pub handle_id: SecretHandleId,
    pub permit_id: PermitId,
    pub provider: String,
    pub key: String,
    pub slot: StableTypeId,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRunnerCapabilityEvidence {
    pub runner: SourceRef,
    pub platform: ExecutionPlatform,
    pub available_sandbox: SandboxCapabilityProfile,
    pub resource_ceiling: ToolResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action_id: ActionId,
    pub execution_id: ActionExecutionId,
    pub envelope: ActionEnvelope,
    pub authorization: HttpRunnerAuthorization,
    pub operation: VersionedPayload,
    pub operation_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum GovernedHttpHeaderValue {
    Literal { value: String },
    Credential { provider: String, key: String },
    McpBearer { connection_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedHttpHeader {
    pub name: String,
    #[serde(flatten)]
    pub value: GovernedHttpHeaderValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedHttpOperation {
    pub method: String,
    pub url: String,
    pub origin: String,
    #[serde(default)]
    pub headers: Vec<GovernedHttpHeader>,
    pub body_base64: Option<String>,
    pub timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_redirects: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedHttpAuthority {
    pub action_id: ActionId,
    pub actor: RuntimeActorId,
    pub operation_hash: ContentHash,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedHttpReconciler {
    pub method: String,
    pub origin: String,
    pub disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedHttpObservation {
    pub status_code: u16,
    pub final_url: String,
    pub resolved_ip: String,
    pub content_type: String,
    pub body_base64: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpRunnerOutcome {
    Completed,
    NoEffectConfirmed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub action_id: Option<ActionId>,
    pub execution_id: Option<ActionExecutionId>,
    pub outcome: HttpRunnerOutcome,
    pub observation: Option<VersionedPayload>,
    pub error_code: Option<String>,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpRunnerProtocolError {
    #[error("governed HTTP action is invalid: {0}")]
    Invalid(&'static str),
    #[error("governed HTTP canonical payload is invalid")]
    Canonical,
}

#[derive(Debug, Clone)]
pub struct HttpRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
    capability_evidence: HttpRunnerCapabilityEvidence,
}

impl HttpRunnerClient {
    /// Selects the fixed governed-capability runner adjacent to a trusted application binary.
    ///
    /// # Errors
    /// Returns unavailable when the adjacent executable is absent or cannot be measured.
    pub fn adjacent_to(application: impl AsRef<Path>) -> Result<Self, ProcessRunnerClientError> {
        let parent = application
            .as_ref()
            .parent()
            .ok_or(ProcessRunnerClientError::Unavailable)?;
        Self::with_executable(parent.join(runner_file_name()), Duration::from_secs(35))
    }

    /// Creates a fixed-identity HTTP runner client for qualification and injected package paths.
    ///
    /// # Errors
    /// Returns unavailable for an invalid filename, timeout, link, or irregular executable.
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
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
            capability_evidence: HttpRunnerCapabilityEvidence {
                runner: runner_executable_source(executable)?,
                platform: current_execution_platform(),
                available_sandbox: http_runner_sandbox_profile(),
                resource_ceiling: ToolResourceLimits {
                    timeout_ms: 120_000,
                    max_output_bytes: HTTP_RUNNER_BODY_MAX_BYTES,
                    max_memory_bytes: 0,
                    max_child_processes: 0,
                },
            },
        })
    }

    #[must_use]
    pub fn capability_evidence(&self) -> &HttpRunnerCapabilityEvidence {
        &self.capability_evidence
    }

    /// Dispatches one exact request through a cleared-environment, kill-on-drop child boundary.
    ///
    /// # Errors
    /// Returns a categorical transport error for identity, launch, delivery, timeout, output, or
    /// response-contract failure.
    pub async fn invoke(
        &self,
        request: &HttpRunnerRequest,
    ) -> Result<HttpRunnerResponse, ProcessRunnerClientError> {
        self.invoke_with_cancellation(request, std::future::pending())
            .await
    }

    /// Dispatches one exact request and terminates/reaps its child when cancellation becomes
    /// authoritative.
    ///
    /// # Errors
    /// Returns a categorical transport error for identity, launch, delivery, cancellation,
    /// timeout, output, or response-contract failure.
    pub async fn invoke_with_cancellation<C>(
        &self,
        request: &HttpRunnerRequest,
        cancellation: C,
    ) -> Result<HttpRunnerResponse, ProcessRunnerClientError>
    where
        C: Future<Output = ()>,
    {
        let bytes = self.encode_request(request)?;
        let mut command = self.runner_command()?;
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
        let mut stdout_task = tokio::spawn(read_bounded(stdout, HTTP_RUNNER_RESPONSE_MAX_BYTES));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        tokio::pin!(cancellation);
        let write = tokio::time::timeout_at(deadline, async {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await
        });
        tokio::pin!(write);
        let write_result = tokio::select! {
            result = &mut write => Some(result),
            () = &mut cancellation => None,
        };
        let Some(write_result) = write_result else {
            drop(stdin);
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(ProcessRunnerClientError::Cancelled);
        };
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                drop(stdin);
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Delivery);
            }
            Err(_) => {
                drop(stdin);
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProcessRunnerClientError::Timeout);
            }
        }
        drop(stdin);
        let wait_result = {
            let wait = tokio::time::timeout_at(deadline, child.wait());
            tokio::pin!(wait);
            tokio::select! {
                result = &mut wait => Some(result),
                () = &mut cancellation => None,
            }
        };
        let Some(wait_result) = wait_result else {
            stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
            return Err(ProcessRunnerClientError::Cancelled);
        };
        let status = match wait_result {
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
        let output_result = {
            let output = tokio::time::timeout_at(deadline, async {
                Ok::<_, ProcessRunnerClientError>((
                    join_reader(&mut stdout_task).await?,
                    join_reader(&mut stderr_task).await?,
                ))
            });
            tokio::pin!(output);
            tokio::select! {
                result = &mut output => Some(result),
                () = &mut cancellation => None,
            }
        };
        let Some(output_result) = output_result else {
            stdout_task.abort();
            stderr_task.abort();
            return Err(ProcessRunnerClientError::Cancelled);
        };
        let output = output_result.map_err(|_| ProcessRunnerClientError::Timeout)??;
        if output.0.1 || output.1.1 {
            return Err(ProcessRunnerClientError::ResponseTooLarge);
        }
        if !output.1.0.is_empty() {
            return Err(ProcessRunnerClientError::DiagnosticOutput);
        }
        validate_http_client_response(status.success(), request, &output.0.0)
    }

    fn encode_request(
        &self,
        request: &HttpRunnerRequest,
    ) -> Result<Vec<u8>, ProcessRunnerClientError> {
        require_regular_unlinked(&self.executable)?;
        if runner_executable_source(&self.executable)? != self.capability_evidence.runner {
            return Err(ProcessRunnerClientError::ExecutableChanged);
        }
        let bytes =
            serde_json::to_vec(request).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
        if bytes.len() > HTTP_RUNNER_REQUEST_MAX_BYTES {
            return Err(ProcessRunnerClientError::RequestTooLarge);
        }
        Ok(bytes)
    }

    fn runner_command(&self) -> Result<CommandWrap, ProcessRunnerClientError> {
        let mut command = Command::new(&self.executable);
        command.env_clear();
        preserve_platform_runtime_environment(&mut command);
        command
            .arg("--governed-http")
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
        Ok(command)
    }
}

fn validate_http_client_response(
    process_succeeded: bool,
    request: &HttpRunnerRequest,
    bytes: &[u8],
) -> Result<HttpRunnerResponse, ProcessRunnerClientError> {
    let response: HttpRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ProcessRunnerClientError::InvalidResponse)?;
    if response.protocol_version != HTTP_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.action_id != Some(request.action_id)
        || response.execution_id != Some(request.execution_id)
        || process_succeeded != (response.outcome == HttpRunnerOutcome::Completed)
        || response.detail.is_empty()
        || response.detail.len() > DETAIL_MAX_BYTES
        || response.started_at > response.finished_at
    {
        return Err(ProcessRunnerClientError::InvalidResponse);
    }
    match response.outcome {
        HttpRunnerOutcome::Completed => {
            let Some(observation) = &response.observation else {
                return Err(ProcessRunnerClientError::InvalidResponse);
            };
            if response.error_code.is_some()
                || observation.type_id.as_str() != GOVERNED_HTTP_OBSERVATION_TYPE
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

/// Canonically encodes and hashes an exact governed HTTP operation.
///
/// # Errors
/// Returns an error for invalid request shape, URL, headers, body, limits, or encoding.
pub fn governed_http_operation_payload(
    operation: &GovernedHttpOperation,
) -> Result<(VersionedPayload, ContentHash), String> {
    validate_http_operation(operation).map_err(|error| error.to_string())?;
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_HTTP_OPERATION_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(operation).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let hash = payload
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok((payload, hash))
}

/// Binds one action, actor, operation, and deadline into canonical HTTP authority.
///
/// # Errors
/// Returns an error when the authority is expired or cannot be encoded.
pub fn governed_http_authority_binding(
    authority: &GovernedHttpAuthority,
) -> Result<PolicyBinding, String> {
    if authority.expires_at <= Utc::now() {
        return Err("governed HTTP authority is expired".into());
    }
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_HTTP_AUTHORITY_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(authority).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(GOVERNED_HTTP_AUTHORITY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

/// Creates the mandatory manual/no-redispatch reconciler for a mutating HTTP request.
///
/// # Errors
/// Returns an error for observational methods or invalid canonical evidence.
pub fn governed_http_reconciler(
    operation: &GovernedHttpOperation,
) -> Result<VersionedPayload, String> {
    if http_effect_class(&operation.method)? != EffectClass::Irreversible {
        return Err("observational HTTP requests do not use effect reconciliation".into());
    }
    VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_HTTP_RECONCILER_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(GovernedHttpReconciler {
            method: operation.method.clone(),
            origin: operation.origin.clone(),
            disposition: "manual_review_no_redispatch".into(),
        })
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Independently validates the exact action shape and authority for governed HTTP.
///
/// # Errors
/// Returns a protocol error for substituted, expired, malformed, or under-governed actions.
pub fn validate_governed_http_envelope(
    envelope: &ActionEnvelope,
    now: DateTime<Utc>,
) -> Result<GovernedHttpAuthority, HttpRunnerProtocolError> {
    if envelope.operation.type_id.as_str() != GOVERNED_HTTP_OPERATION_TYPE
        || envelope.operation_hash.domain != GOVERNED_ACTION_OPERATION_HASH_DOMAIN
        || envelope
            .operation
            .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
            .map_err(|_| HttpRunnerProtocolError::Canonical)?
            != envelope.operation_hash
        || envelope.authority.payload.type_id.as_str() != GOVERNED_HTTP_AUTHORITY_TYPE
        || envelope.authority.content_hash.domain != GOVERNED_HTTP_AUTHORITY_HASH_DOMAIN
        || envelope
            .authority
            .payload
            .content_hash(GOVERNED_HTTP_AUTHORITY_HASH_DOMAIN)
            .map_err(|_| HttpRunnerProtocolError::Canonical)?
            != envelope.authority.content_hash
    {
        return Err(HttpRunnerProtocolError::Invalid(
            "operation or authority binding",
        ));
    }
    let authority: GovernedHttpAuthority = decode_http_payload(&envelope.authority.payload)?;
    let operation: GovernedHttpOperation = decode_http_payload(&envelope.operation)?;
    validate_http_operation(&operation)?;
    let effect = http_effect_class(&operation.method)
        .map_err(|_| HttpRunnerProtocolError::Invalid("HTTP effect class"))?;
    let reconciliation_valid = if effect == EffectClass::Irreversible {
        envelope.reconciler.as_ref()
            == Some(
                &governed_http_reconciler(&operation)
                    .map_err(|_| HttpRunnerProtocolError::Canonical)?,
            )
            && envelope.elevated_authority.is_some()
            && envelope
                .reconciliation_deadline
                .is_some_and(|deadline| deadline > now)
    } else {
        envelope.reconciler.is_none()
            && envelope.elevated_authority.is_none()
            && envelope.reconciliation_deadline.is_none()
    };
    if authority.action_id != envelope.id
        || authority.actor != envelope.actor
        || authority.operation_hash != envelope.operation_hash
        || authority.expires_at <= now
        || envelope.effect_class != effect
        || envelope.compensation.is_some()
        || envelope.compensates_action.is_some()
        || !reconciliation_valid
    {
        return Err(HttpRunnerProtocolError::Invalid("HTTP authority scope"));
    }
    Ok(authority)
}

/// Creates a child request only from an exact executing action and persisted Tool authority.
///
/// # Errors
/// Returns a protocol error when the envelope or execution does not bind the exact request.
pub fn http_request_from_authorized_action(
    envelope: &ActionEnvelope,
    execution: &ActionExecution,
    authorization: HttpRunnerAuthorization,
    request_id: Uuid,
    now: DateTime<Utc>,
) -> Result<HttpRunnerRequest, HttpRunnerProtocolError> {
    validate_governed_http_envelope(envelope, now)?;
    if execution.action_id != envelope.id
        || execution.state != ActionExecutionState::Executing
        || execution.executor.is_none()
    {
        return Err(HttpRunnerProtocolError::Invalid("execution binding"));
    }
    Ok(HttpRunnerRequest {
        protocol_version: HTTP_RUNNER_PROTOCOL_VERSION,
        request_id,
        action_id: envelope.id,
        execution_id: execution.id,
        envelope: envelope.clone(),
        authorization,
        operation: envelope.operation.clone(),
        operation_hash: envelope.operation_hash.clone(),
    })
}

/// Decodes one canonical governed HTTP observation.
///
/// # Errors
/// Returns a protocol error for a wrong type or malformed canonical payload.
pub fn decode_governed_http_observation(
    payload: &VersionedPayload,
) -> Result<GovernedHttpObservation, HttpRunnerProtocolError> {
    if payload.type_id.as_str() != GOVERNED_HTTP_OBSERVATION_TYPE {
        return Err(HttpRunnerProtocolError::Invalid("HTTP observation type"));
    }
    decode_http_payload(payload)
}

fn decode_http_payload<T: serde::de::DeserializeOwned>(
    payload: &VersionedPayload,
) -> Result<T, HttpRunnerProtocolError> {
    let value =
        decode_canonical_cbor_v1(&payload.bytes).map_err(|_| HttpRunnerProtocolError::Canonical)?;
    serde_json::from_value(value).map_err(|_| HttpRunnerProtocolError::Canonical)
}

/// Maps the supported HTTP methods to the action ledger's conservative effect class.
///
/// # Errors
/// Returns an error for unsupported methods.
pub fn http_effect_class(method: &str) -> Result<EffectClass, String> {
    match method {
        "GET" | "HEAD" => Ok(EffectClass::Observational),
        "POST" | "PUT" | "PATCH" | "DELETE" => Ok(EffectClass::Irreversible),
        _ => Err("HTTP method is unsupported".into()),
    }
}

fn validate_http_operation(
    operation: &GovernedHttpOperation,
) -> Result<(), HttpRunnerProtocolError> {
    let url =
        Url::parse(&operation.url).map_err(|_| HttpRunnerProtocolError::Invalid("HTTP URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.fragment().is_some()
        || url.to_string() != operation.url
        || url.origin().ascii_serialization() != operation.origin
        || http_effect_class(&operation.method).is_err()
        || operation.headers.len() > MAX_HEADERS
        || operation.timeout_ms == 0
        || operation.timeout_ms > 120_000
        || operation.max_response_bytes == 0
        || operation.max_response_bytes > HTTP_RUNNER_BODY_MAX_BYTES
        || operation.max_redirects > MAX_REDIRECTS
        || !matches!(operation.method.as_str(), "GET" | "HEAD") && operation.max_redirects != 0
    {
        return Err(HttpRunnerProtocolError::Invalid("HTTP operation shape"));
    }
    let mut names = HashSet::new();
    for header in &operation.headers {
        let Ok(name) = HeaderName::from_bytes(header.name.as_bytes()) else {
            return Err(HttpRunnerProtocolError::Invalid("HTTP header name"));
        };
        if name.as_str() != header.name
            || !names.insert(header.name.clone())
            || matches!(
                header.name.as_str(),
                "connection" | "content-length" | "host" | "transfer-encoding" | "proxy-connection"
            )
        {
            return Err(HttpRunnerProtocolError::Invalid("HTTP header authority"));
        }
        match &header.value {
            GovernedHttpHeaderValue::Literal { value } => {
                if value.len() > MAX_HEADER_VALUE_BYTES
                    || ReqwestHeaderValue::from_str(value).is_err()
                    || matches!(
                        header.name.as_str(),
                        "authorization" | "cookie" | "proxy-authorization"
                    )
                {
                    return Err(HttpRunnerProtocolError::Invalid("HTTP literal header"));
                }
            }
            GovernedHttpHeaderValue::Credential { provider, key } => {
                if !valid_credential_reference(provider, key)
                    || !matches!(
                        header.name.as_str(),
                        "authorization" | "cookie" | "x-api-key"
                    )
                {
                    return Err(HttpRunnerProtocolError::Invalid("HTTP credential header"));
                }
            }
            GovernedHttpHeaderValue::McpBearer { connection_id } => {
                if header.name.as_str() != "authorization"
                    || mcp_bearer_secret_slot(connection_id).is_err()
                {
                    return Err(HttpRunnerProtocolError::Invalid("HTTP MCP bearer header"));
                }
            }
        }
    }
    if operation.body_base64.as_ref().is_some_and(|value| {
        STANDARD.decode(value).ok().is_none_or(|bytes| {
            u64::try_from(bytes.len()).unwrap_or(u64::MAX) > HTTP_RUNNER_BODY_MAX_BYTES
                || STANDARD.encode(bytes) != *value
        })
    }) {
        return Err(HttpRunnerProtocolError::Invalid("HTTP body"));
    }
    Ok(())
}

fn valid_credential_reference(provider: &str, key: &str) -> bool {
    !provider.is_empty()
        && provider.len() <= 256
        && provider
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && matches!(key, "primary" | "oauth_id_token")
}

/// Derives a content-minimized stable slot identifier for one supported credential reference.
///
/// # Errors
/// Returns an error for malformed, unknown, or renewable credential slots.
pub fn http_secret_slot(provider: &str, key: &str) -> Result<StableTypeId, String> {
    if !valid_credential_reference(provider, key) {
        return Err("HTTP credential reference is unsupported".into());
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{provider}\0{key}").as_bytes())
    );
    StableTypeId::parse(format!("mcg.plugin_http.secret_{}.v1", &digest[..32]))
        .map_err(|error| error.to_string())
}

/// Derives the opaque secret slot for one protected MCP bearer credential.
///
/// # Errors
/// Returns an error for a malformed MCP connection identity.
pub fn mcp_bearer_secret_slot(connection_id: &str) -> Result<StableTypeId, String> {
    if connection_id.is_empty()
        || connection_id.len() > 128
        || !connection_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("MCP connection identity is invalid".into());
    }
    let digest = format!("{:x}", Sha256::digest(connection_id.as_bytes()));
    StableTypeId::parse(format!("mcg.mcp_bearer.secret_{}.v1", &digest[..32]))
        .map_err(|error| error.to_string())
}

/// Returns the unique opaque secret slots in deterministic first-use order.
///
/// # Errors
/// Returns an error when any credential reference is unsupported.
pub fn http_secret_slots(operation: &GovernedHttpOperation) -> Result<Vec<StableTypeId>, String> {
    Ok(http_credential_references(operation)?
        .into_iter()
        .map(|(_, _, slot)| slot)
        .collect())
}

fn http_credential_references(
    operation: &GovernedHttpOperation,
) -> Result<Vec<(String, String, StableTypeId)>, String> {
    let mut slots = Vec::new();
    let mut unique = BTreeSet::new();
    for header in &operation.headers {
        let reference = match &header.value {
            GovernedHttpHeaderValue::Credential { provider, key } => (
                provider.clone(),
                key.clone(),
                http_secret_slot(provider, key)?,
            ),
            GovernedHttpHeaderValue::McpBearer { connection_id } => (
                MCP_BEARER_SECRET_PROVIDER.into(),
                connection_id.clone(),
                mcp_bearer_secret_slot(connection_id)?,
            ),
            GovernedHttpHeaderValue::Literal { .. } => continue,
        };
        if unique.insert(reference.2.clone()) {
            slots.push(reference);
        }
    }
    Ok(slots)
}

#[must_use]
pub const fn http_runner_sandbox_profile() -> SandboxCapabilityProfile {
    let mut profile = process_runner_sandbox_profile();
    profile.network = IsolationGuarantee::BestEffort;
    profile.secrets = IsolationGuarantee::BestEffort;
    profile.process_tree = if cfg!(windows) {
        IsolationGuarantee::Hard
    } else {
        IsolationGuarantee::BestEffort
    };
    profile
}

#[must_use]
pub fn http_runner_input_schema_source() -> SourceRef {
    static_source_ref(
        HTTP_RUNNER_INPUT_SCHEMA_TYPE,
        GOVERNED_HTTP_OPERATION_TYPE,
        "MetaCog.HttpRunnerInputSchema",
    )
}

#[must_use]
pub fn http_runner_output_schema_source() -> SourceRef {
    static_source_ref(
        HTTP_RUNNER_OUTPUT_SCHEMA_TYPE,
        GOVERNED_HTTP_OBSERVATION_TYPE,
        "MetaCog.HttpRunnerOutputSchema",
    )
}

#[must_use]
pub fn http_runner_reconciler_source() -> SourceRef {
    static_source_ref(
        HTTP_RUNNER_RECONCILER_SOURCE_TYPE,
        GOVERNED_HTTP_RECONCILER_TYPE,
        "MetaCog.HttpRunnerReconciler",
    )
}

/// Derives the exact deterministic Tool manifest for one origin, effect, slot set, and limit set.
///
/// # Errors
/// Returns an error for invalid operations, limits, evidence, or manifest contracts.
pub fn governed_http_tool_manifest(
    evidence: &HttpRunnerCapabilityEvidence,
    operation: &GovernedHttpOperation,
    limits: ToolResourceLimits,
) -> Result<ToolManifest, String> {
    validate_http_operation(operation).map_err(|error| error.to_string())?;
    limits.validate().map_err(|error| error.to_string())?;
    if !limits.fits_within(evidence.resource_ceiling) {
        return Err("HTTP Tool limits exceed runner capacity".into());
    }
    let effect = http_effect_class(&operation.method)?;
    let secret_slots = http_secret_slots(operation)?;
    let material = serde_json::to_vec(&serde_json::json!({
        "runner": evidence.runner,
        "platform": evidence.platform,
        "origin": operation.origin,
        "secret_slots": secret_slots,
        "sandbox": evidence.available_sandbox,
        "limits": limits,
        "effect": effect,
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
        name: "http.request".into(),
        summary: "Governed one-shot HTTP request to one exact origin".into(),
        runner: evidence.runner.clone(),
        input_schema: http_runner_input_schema_source(),
        output_schema: http_runner_output_schema_source(),
        maximum_effect_class: effect,
        required_capabilities: ToolCapabilitySet {
            filesystem: Vec::new(),
            network: vec![NetworkCapability {
                destination: operation.origin.clone(),
            }],
            secret_slots,
            process_spawn: false,
        },
        resource_ceiling: limits,
        sandbox_requirement: evidence.available_sandbox,
        platform_support: vec![evidence.platform],
        receipt_semantics: StableTypeId::parse(HTTP_RUNNER_RECEIPT_TYPE)
            .map_err(|error| error.to_string())?,
        reconciler: (effect == EffectClass::Irreversible).then(http_runner_reconciler_source),
    };
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Derives the exact Root policy for one governed HTTP manifest.
///
/// # Errors
/// Returns an error when the manifest reference cannot be constructed.
pub fn governed_http_tool_policy(manifest: &ToolManifest) -> Result<ToolPolicy, String> {
    Ok(ToolPolicy {
        allowed_manifests: vec![
            manifest
                .versioned_ref()
                .map_err(|error| error.to_string())?,
        ],
        maximum_effect_class: manifest.maximum_effect_class,
        maximum_capabilities: manifest.required_capabilities.clone(),
        maximum_limits: manifest.resource_ceiling,
        maximum_permit_lifetime_seconds: 24 * 60 * 60,
    })
}

#[derive(Debug)]
struct HttpFailure {
    outcome: HttpRunnerOutcome,
    code: &'static str,
    detail: String,
}

impl HttpFailure {
    fn denied(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: HttpRunnerOutcome::Denied,
            code,
            detail: detail.into(),
        }
    }

    fn no_effect(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: HttpRunnerOutcome::NoEffectConfirmed,
            code,
            detail: detail.into(),
        }
    }

    fn unknown(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            outcome: HttpRunnerOutcome::OutcomeUnknown,
            code,
            detail: detail.into(),
        }
    }
}

pub async fn execute_http_request(bytes: &[u8]) -> HttpRunnerResponse {
    execute_http_request_with_stores(bytes, &OsProviderCredentialStore, &OsMcpCredentialStore).await
}

pub async fn execute_http_request_with_store<S: ProviderCredentialStore + ?Sized>(
    bytes: &[u8],
    store: &S,
) -> HttpRunnerResponse {
    execute_http_request_with_stores(bytes, store, &MemoryMcpCredentialStore::default()).await
}

pub async fn execute_http_request_with_stores<
    P: ProviderCredentialStore + ?Sized,
    M: McpCredentialStore + ?Sized,
>(
    bytes: &[u8],
    provider_store: &P,
    mcp_store: &M,
) -> HttpRunnerResponse {
    let started_at = Utc::now();
    if bytes.is_empty() || bytes.len() > HTTP_RUNNER_REQUEST_MAX_BYTES {
        return http_failure_response(
            None,
            &HttpFailure::denied("request_size_invalid", "HTTP request size is invalid"),
            started_at,
        );
    }
    let Ok(request) = serde_json::from_slice::<HttpRunnerRequest>(bytes) else {
        return http_failure_response(
            None,
            &HttpFailure::denied("request_invalid", "HTTP request is not valid JSON"),
            started_at,
        );
    };
    match execute_http(&request, provider_store, mcp_store).await {
        Ok(observation) => HttpRunnerResponse {
            protocol_version: HTTP_RUNNER_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            action_id: Some(request.action_id),
            execution_id: Some(request.execution_id),
            outcome: HttpRunnerOutcome::Completed,
            observation: Some(observation),
            error_code: None,
            detail: "exact governed HTTP response was captured".into(),
            started_at,
            finished_at: Utc::now(),
        },
        Err(error) => http_failure_response(Some(&request), &error, started_at),
    }
}

fn http_failure_response(
    request: Option<&HttpRunnerRequest>,
    failure: &HttpFailure,
    started_at: DateTime<Utc>,
) -> HttpRunnerResponse {
    HttpRunnerResponse {
        protocol_version: HTTP_RUNNER_PROTOCOL_VERSION,
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

#[allow(clippy::too_many_lines)] // Keep the post-contact ambiguity boundary contiguous.
async fn execute_http<P: ProviderCredentialStore + ?Sized, M: McpCredentialStore + ?Sized>(
    request: &HttpRunnerRequest,
    provider_store: &P,
    mcp_store: &M,
) -> Result<VersionedPayload, HttpFailure> {
    if request.protocol_version != HTTP_RUNNER_PROTOCOL_VERSION {
        return Err(HttpFailure::denied(
            "protocol_unsupported",
            "HTTP runner protocol is unsupported",
        ));
    }
    validate_http_tool_authorization(request, Utc::now())?;
    let operation: GovernedHttpOperation = decode_http_payload(&request.operation)
        .map_err(|error| HttpFailure::denied("operation_invalid", error.to_string()))?;
    let method = Method::from_bytes(operation.method.as_bytes())
        .map_err(|_| HttpFailure::denied("method_invalid", "HTTP method is invalid"))?;
    let url = Url::parse(&operation.url)
        .map_err(|_| HttpFailure::denied("url_invalid", "HTTP URL is invalid"))?;
    let (mut headers, mut secrets) =
        resolve_http_headers(request, &operation, provider_store, mcp_store)?;
    let resolved = resolve_and_validate_destination(&url).await?;
    let host = url
        .host_str()
        .ok_or_else(|| HttpFailure::denied("url_invalid", "HTTP URL has no host"))?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve(host, resolved)
        .build()
        .map_err(|_| {
            HttpFailure::no_effect("client_unavailable", "HTTP client could not be constructed")
        })?;
    let body = operation.body_base64.as_deref().map(|value| {
        STANDARD
            .decode(value)
            .expect("validated canonical HTTP body")
    });
    let safe = matches!(operation.method.as_str(), "GET" | "HEAD");
    let deadline = Instant::now() + Duration::from_millis(operation.timeout_ms);
    let mut current = url;
    for redirect_count in 0..=operation.max_redirects {
        let mut builder = client
            .request(method.clone(), current.clone())
            .headers(headers.clone());
        if let Some(body) = &body {
            builder = builder.body(body.clone());
        }
        let response = match tokio::time::timeout_at(deadline, builder.send()).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                wipe_headers(&mut headers);
                secrets.zeroize();
                return Err(if safe {
                    HttpFailure::no_effect(
                        "request_failed",
                        "safe HTTP request failed after dispatch",
                    )
                } else {
                    HttpFailure::unknown(
                        "request_outcome_unknown",
                        "HTTP request failed after possible destination contact",
                    )
                });
            }
            Err(_) => {
                wipe_headers(&mut headers);
                secrets.zeroize();
                return Err(if safe {
                    HttpFailure::no_effect(
                        "request_timeout",
                        "safe HTTP request exceeded its deadline",
                    )
                } else {
                    HttpFailure::unknown(
                        "request_timeout_unknown",
                        "HTTP request timed out after possible destination contact",
                    )
                });
            }
        };
        if response.status().is_redirection() {
            if !safe {
                wipe_headers(&mut headers);
                secrets.zeroize();
                return Err(HttpFailure::unknown(
                    "mutating_redirect_unknown",
                    "mutating HTTP destination returned a redirect after contact",
                ));
            }
            if redirect_count == operation.max_redirects {
                wipe_headers(&mut headers);
                secrets.zeroize();
                return Err(HttpFailure::no_effect(
                    "redirect_limit",
                    "safe HTTP redirect limit was exceeded",
                ));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    HttpFailure::no_effect(
                        "redirect_invalid",
                        "safe HTTP redirect omitted a valid Location",
                    )
                })?;
            let next = current.join(location).map_err(|_| {
                HttpFailure::no_effect("redirect_invalid", "safe HTTP redirect URL is invalid")
            })?;
            if next.origin().ascii_serialization() != operation.origin || next.fragment().is_some()
            {
                wipe_headers(&mut headers);
                secrets.zeroize();
                return Err(HttpFailure::no_effect(
                    "redirect_scope_denied",
                    "safe HTTP redirect escaped the exact authorized origin",
                ));
            }
            current = next;
            continue;
        }
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .chars()
            .take(512)
            .collect::<String>();
        let limit = usize::try_from(operation.max_response_bytes).map_err(|_| {
            HttpFailure::unknown("response_limit_invalid", "HTTP response limit is invalid")
        })?;
        let mut output = Vec::new();
        let mut truncated = false;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::time::timeout_at(deadline, stream.next())
            .await
            .map_err(|_| {
                if safe {
                    HttpFailure::no_effect(
                        "response_timeout",
                        "safe HTTP response exceeded its deadline",
                    )
                } else {
                    HttpFailure::unknown(
                        "response_timeout_unknown",
                        "mutating HTTP response was lost after contact",
                    )
                }
            })?
        {
            let chunk = chunk.map_err(|_| {
                if safe {
                    HttpFailure::no_effect(
                        "response_failed",
                        "safe HTTP response could not be read",
                    )
                } else {
                    HttpFailure::unknown(
                        "response_failed_unknown",
                        "mutating HTTP response was lost after contact",
                    )
                }
            })?;
            let remaining = limit.saturating_sub(output.len());
            let retained = remaining.min(chunk.len());
            output.extend_from_slice(&chunk[..retained]);
            if retained < chunk.len() {
                truncated = true;
                break;
            }
            if output.len() == limit {
                truncated = match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(None) => false,
                    Ok(Some(Ok(_))) => true,
                    Ok(Some(Err(_))) | Err(_) => {
                        return Err(if safe {
                            HttpFailure::no_effect(
                                "response_failed",
                                "safe HTTP response boundary could not be verified",
                            )
                        } else {
                            HttpFailure::unknown(
                                "response_failed_unknown",
                                "mutating HTTP response boundary was lost after contact",
                            )
                        });
                    }
                };
                break;
            }
        }
        redact_secret_bytes(&mut output, &secrets);
        let mut content_type = content_type.into_bytes();
        redact_secret_bytes(&mut content_type, &secrets);
        let content_type = String::from_utf8_lossy(&content_type).into_owned();
        wipe_headers(&mut headers);
        secrets.zeroize();
        let mut final_url = current.clone();
        final_url.set_query(None);
        final_url.set_fragment(None);
        let observation = GovernedHttpObservation {
            status_code: status.as_u16(),
            final_url: final_url.to_string(),
            resolved_ip: resolved.ip().to_string(),
            content_type,
            body_base64: STANDARD.encode(output),
            truncated,
        };
        return VersionedPayload::canonical_v1(
            StableTypeId::parse(GOVERNED_HTTP_OBSERVATION_TYPE)
                .map_err(|error| HttpFailure::unknown("observation_invalid", error.to_string()))?,
            &serde_json::to_value(observation)
                .map_err(|error| HttpFailure::unknown("observation_invalid", error.to_string()))?,
        )
        .map_err(|error| HttpFailure::unknown("observation_invalid", error.to_string()));
    }
    Err(HttpFailure::no_effect(
        "redirect_state_invalid",
        "HTTP redirect state is invalid",
    ))
}

#[allow(clippy::too_many_lines)] // Keep secret lookup, validation, and zeroizable header material together.
fn resolve_http_headers<P: ProviderCredentialStore + ?Sized, M: McpCredentialStore + ?Sized>(
    request: &HttpRunnerRequest,
    operation: &GovernedHttpOperation,
    provider_store: &P,
    mcp_store: &M,
) -> Result<(HeaderMap, Vec<Vec<u8>>), HttpFailure> {
    let mut headers = HeaderMap::new();
    let mut secrets = Vec::new();
    for header in &operation.headers {
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|_| HttpFailure::denied("header_invalid", "HTTP header name is invalid"))?;
        let value = match &header.value {
            GovernedHttpHeaderValue::Literal { value } => ReqwestHeaderValue::from_str(value)
                .map_err(|_| {
                    HttpFailure::denied("header_invalid", "HTTP header value is invalid")
                })?,
            GovernedHttpHeaderValue::Credential { provider, key } => {
                let slot = provider_slot(key)?;
                let expected = http_secret_slot(provider, key)
                    .map_err(|error| HttpFailure::denied("credential_invalid", error))?;
                let binding = request
                    .authorization
                    .secret_handles
                    .iter()
                    .find(|binding| {
                        binding.provider == *provider
                            && binding.key == *key
                            && binding.slot == expected
                    })
                    .ok_or_else(|| {
                        HttpFailure::denied(
                            "credential_handle_mismatch",
                            "HTTP credential has no exact permit handle",
                        )
                    })?;
                if binding.permit_id != request.authorization.permit.id
                    || !request
                        .authorization
                        .permit
                        .secret_handles
                        .contains(&binding.handle_id)
                    || binding.expires_at != request.authorization.permit.expires_at
                    || binding.expires_at <= Utc::now()
                {
                    return Err(HttpFailure::denied(
                        "credential_handle_mismatch",
                        "HTTP credential handle is stale or substituted",
                    ));
                }
                let credential = provider_store
                    .load(provider, slot)
                    .map_err(|_| {
                        HttpFailure::no_effect(
                            "credential_unavailable",
                            "authorized HTTP credential store is unavailable",
                        )
                    })?
                    .ok_or_else(|| {
                        HttpFailure::no_effect(
                            "credential_unavailable",
                            "authorized HTTP credential is unavailable",
                        )
                    })?;
                let secret = credential.expose_to_host_executor().as_bytes().to_vec();
                let value = ReqwestHeaderValue::from_bytes(&secret).map_err(|_| {
                    HttpFailure::denied(
                        "credential_invalid",
                        "authorized HTTP credential is not a valid header value",
                    )
                })?;
                secrets.push(secret);
                value
            }
            GovernedHttpHeaderValue::McpBearer { connection_id } => {
                let expected = mcp_bearer_secret_slot(connection_id)
                    .map_err(|error| HttpFailure::denied("credential_invalid", error))?;
                let binding = request
                    .authorization
                    .secret_handles
                    .iter()
                    .find(|binding| {
                        binding.provider == MCP_BEARER_SECRET_PROVIDER
                            && binding.key == *connection_id
                            && binding.slot == expected
                    })
                    .ok_or_else(|| {
                        HttpFailure::denied(
                            "credential_handle_mismatch",
                            "MCP bearer has no exact permit handle",
                        )
                    })?;
                if binding.permit_id != request.authorization.permit.id
                    || !request
                        .authorization
                        .permit
                        .secret_handles
                        .contains(&binding.handle_id)
                    || binding.expires_at != request.authorization.permit.expires_at
                    || binding.expires_at <= Utc::now()
                {
                    return Err(HttpFailure::denied(
                        "credential_handle_mismatch",
                        "MCP bearer handle is stale or substituted",
                    ));
                }
                let credential = mcp_store
                    .load(connection_id)
                    .map_err(|_| {
                        HttpFailure::no_effect(
                            "credential_unavailable",
                            "authorized MCP credential store is unavailable",
                        )
                    })?
                    .ok_or_else(|| {
                        HttpFailure::no_effect(
                            "credential_unavailable",
                            "authorized MCP bearer is unavailable",
                        )
                    })?;
                let raw_secret = credential.expose_to_host_executor().as_bytes().to_vec();
                let mut authorization = b"Bearer ".to_vec();
                authorization.extend_from_slice(&raw_secret);
                let value = ReqwestHeaderValue::from_bytes(&authorization).map_err(|_| {
                    HttpFailure::denied(
                        "credential_invalid",
                        "authorized MCP bearer is not a valid header value",
                    )
                })?;
                secrets.push(raw_secret);
                secrets.push(authorization);
                value
            }
        };
        headers.insert(name, value);
    }
    Ok((headers, secrets))
}

fn provider_slot(key: &str) -> Result<ProviderSecretSlot, HttpFailure> {
    match key {
        "primary" => Ok(ProviderSecretSlot::Primary),
        "oauth_id_token" => Ok(ProviderSecretSlot::OAuthIdToken),
        _ => Err(HttpFailure::denied(
            "credential_invalid",
            "renewable or unknown HTTP credential slots are denied",
        )),
    }
}

async fn resolve_and_validate_destination(url: &Url) -> Result<SocketAddr, HttpFailure> {
    let host = url
        .host_str()
        .ok_or_else(|| HttpFailure::denied("url_invalid", "HTTP URL has no host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| HttpFailure::denied("url_invalid", "HTTP URL has no port"))?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_unroutable(ip) {
            return Err(HttpFailure::denied(
                "destination_denied",
                "HTTP destination is unspecified or multicast",
            ));
        }
        return Ok(SocketAddr::new(ip, port));
    }
    let addresses = lookup_host((host, port))
        .await
        .map_err(|_| HttpFailure::no_effect("dns_failed", "HTTP destination DNS lookup failed"))?
        .collect::<BTreeSet<_>>();
    if addresses.is_empty() {
        return Err(HttpFailure::no_effect(
            "dns_empty",
            "HTTP destination DNS lookup returned no addresses",
        ));
    }
    if addresses.iter().any(|address| is_non_public(address.ip())) {
        return Err(HttpFailure::denied(
            "dns_scope_denied",
            "hostname resolved to a non-public destination; use an explicitly granted IP origin for local access",
        ));
    }
    addresses.into_iter().next().ok_or_else(|| {
        HttpFailure::no_effect(
            "dns_empty",
            "HTTP destination DNS lookup returned no address",
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
                || ip.is_documentation()
                || octets[0] == 0
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 198 && (18..=19).contains(&octets[1]))
                || octets[0] >= 240
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_non_public(IpAddr::V4(mapped)))
        }
    }
}

fn redact_secret_bytes(output: &mut Vec<u8>, secrets: &[Vec<u8>]) {
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        let mut source_offset = 0;
        let mut redacted = Vec::with_capacity(output.len());
        while let Some(relative) = output[source_offset..]
            .windows(secret.len())
            .position(|window| window == secret)
        {
            let index = source_offset + relative;
            redacted.extend_from_slice(&output[source_offset..index]);
            redacted.extend_from_slice(b"[REDACTED]");
            source_offset = index + secret.len();
        }
        redacted.extend_from_slice(&output[source_offset..]);
        output.zeroize();
        *output = redacted;
    }
}

fn wipe_headers(headers: &mut HeaderMap) {
    headers.clear();
}

#[allow(clippy::too_many_lines)]
fn validate_http_tool_authorization(
    request: &HttpRunnerRequest,
    now: DateTime<Utc>,
) -> Result<(), HttpFailure> {
    let authorization = &request.authorization;
    authorization
        .manifest
        .validate()
        .map_err(|error| HttpFailure::denied("tool_manifest_invalid", error.to_string()))?;
    authorization
        .proposal
        .validate()
        .map_err(|error| HttpFailure::denied("tool_proposal_invalid", error.to_string()))?;
    authorization
        .policy
        .validate()
        .map_err(|error| HttpFailure::denied("tool_policy_invalid", error.to_string()))?;
    authorization
        .permit
        .validate()
        .map_err(|error| HttpFailure::denied("tool_permit_invalid", error.to_string()))?;
    authorization
        .permit_ref
        .validate()
        .map_err(|error| HttpFailure::denied("tool_permit_ref_invalid", error.to_string()))?;
    validate_governed_http_envelope(&request.envelope, now)
        .map_err(|error| HttpFailure::denied("action_envelope_invalid", error.to_string()))?;
    let operation: GovernedHttpOperation = decode_http_payload(&request.operation)
        .map_err(|error| HttpFailure::denied("operation_invalid", error.to_string()))?;
    let effect = http_effect_class(&operation.method)
        .map_err(|error| HttpFailure::denied("method_invalid", error))?;
    let manifest_ref = authorization
        .manifest
        .versioned_ref()
        .map_err(|error| HttpFailure::denied("tool_manifest_invalid", error.to_string()))?;
    let proposal_hash = tool_invocation_proposal_hash(&authorization.proposal)
        .map_err(|error| HttpFailure::denied("tool_proposal_invalid", error.to_string()))?;
    let permit_hash = tool_invocation_permit_hash(&authorization.permit)
        .map_err(|error| HttpFailure::denied("tool_permit_invalid", error.to_string()))?;
    let expected_policy = tool_policy_binding(&authorization.policy)
        .map_err(|error| HttpFailure::denied("tool_policy_invalid", error))?;
    let expected_runner =
        runner_executable_source(&std::env::current_exe().map_err(|error| {
            HttpFailure::denied("runner_identity_unavailable", error.to_string())
        })?)
        .map_err(|_| {
            HttpFailure::denied(
                "runner_identity_unavailable",
                "HTTP runner identity cannot be measured",
            )
        })?;
    let expected_limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_response_bytes,
        max_memory_bytes: 0,
        max_child_processes: 0,
    };
    let expected_slots = http_secret_slots(&operation)
        .map_err(|error| HttpFailure::denied("credential_invalid", error))?;
    let operation_hash = request
        .operation
        .content_hash(GOVERNED_ACTION_OPERATION_HASH_DOMAIN)
        .map_err(|error| HttpFailure::denied("operation_invalid", error.to_string()))?;
    let expected_reconciler =
        (effect == EffectClass::Irreversible).then(http_runner_reconciler_source);
    if request.action_id != request.envelope.id
        || request.operation != request.envelope.operation
        || request.operation_hash != request.envelope.operation_hash
        || request.operation_hash != operation_hash
        || authorization.proposal.action_id != request.envelope.id
        || authorization.permit.action_id != request.envelope.id
        || authorization.proposal.attempt_id != request.envelope.attempt_id
        || authorization.proposal.runtime_actor != request.envelope.actor
        || authorization.proposal.task_contract != request.envelope.task_contract
        || authorization.proposal.argument_hash != request.envelope.operation_hash
        || authorization.proposal.requested_effect_class != effect
        || authorization.proposal.tool_manifest != manifest_ref
        || authorization.permit.tool_manifest != manifest_ref
        || authorization.proposal_hash != proposal_hash
        || authorization.permit.proposal_hash != proposal_hash
        || authorization.permit_hash != permit_hash
        || authorization.permit.argument_hash != request.envelope.operation_hash
        || authorization.permit.allowed_effect_class != effect
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
        || !http_runner_sandbox_profile().satisfies(authorization.manifest.sandbox_requirement)
        || !authorization
            .permit
            .sandbox
            .satisfies(authorization.manifest.sandbox_requirement)
        || authorization.permit.issued_at > now
        || authorization.permit.expires_at <= now
        || authorization.proposal.deadline != authorization.permit.expires_at
        || !authorization
            .manifest
            .required_capabilities
            .filesystem
            .is_empty()
        || authorization
            .manifest
            .required_capabilities
            .network
            .as_slice()
            != [NetworkCapability {
                destination: operation.origin.clone(),
            }]
        || authorization.manifest.required_capabilities.secret_slots != expected_slots
        || authorization.manifest.required_capabilities.process_spawn
        || authorization.manifest.runner != expected_runner
        || authorization.manifest.reconciler != expected_reconciler
        || authorization.permit_ref.source_type.as_str() != ToolInvocationPermit::TYPE_ID
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != permit_hash
        || authorization.permit.secret_handles.len() != expected_slots.len()
        || authorization.secret_handles.len() != expected_slots.len()
    {
        return Err(HttpFailure::denied(
            "tool_authority_mismatch",
            "request differs from exact Root-issued HTTP Tool authority",
        ));
    }
    for (index, slot) in expected_slots.iter().enumerate() {
        let binding = &authorization.secret_handles[index];
        if binding.handle_id != authorization.permit.secret_handles[index]
            || binding.permit_id != authorization.permit.id
            || &binding.slot != slot
            || binding.expires_at != authorization.permit.expires_at
            || binding.expires_at <= now
            || binding_secret_slot(&binding.provider, &binding.key).as_ref() != Ok(slot)
        {
            return Err(HttpFailure::denied(
                "credential_handle_mismatch",
                "HTTP credential handle binding is substituted",
            ));
        }
    }
    Ok(())
}

fn binding_secret_slot(provider: &str, key: &str) -> Result<StableTypeId, String> {
    if provider == MCP_BEARER_SECRET_PROVIDER {
        mcp_bearer_secret_slot(key)
    } else {
        http_secret_slot(provider, key)
    }
}

/// Builds a self-contained exact request for runner qualification and tests.
/// Production dispatch must use Root-persisted authorization and opaque handle bindings.
///
/// # Errors
/// Returns an error when the operation, authority, manifest, proposal, or permit is invalid.
#[allow(clippy::too_many_lines)] // Qualification deliberately spells out every bound contract.
pub fn http_qualification_request_with_evidence(
    operation: &GovernedHttpOperation,
    evidence: &HttpRunnerCapabilityEvidence,
) -> Result<HttpRunnerRequest, String> {
    let credential_references = http_credential_references(operation)?;
    let now = Utc::now();
    let deadline = now
        + chrono::Duration::milliseconds(
            i64::try_from(operation.timeout_ms.saturating_add(5_000))
                .map_err(|error| error.to_string())?,
        );
    let effect = http_effect_class(&operation.method)?;
    let action_id = ActionId::new();
    let actor = RuntimeActorId::new();
    let (payload, operation_hash) = governed_http_operation_payload(operation)?;
    let limits = ToolResourceLimits {
        timeout_ms: operation.timeout_ms,
        max_output_bytes: operation.max_response_bytes,
        max_memory_bytes: 0,
        max_child_processes: 0,
    };
    let manifest = governed_http_tool_manifest(evidence, operation, limits)?;
    let policy = governed_http_tool_policy(&manifest)?;
    let authority = governed_http_authority_binding(&GovernedHttpAuthority {
        action_id,
        actor,
        operation_hash: operation_hash.clone(),
        expires_at: deadline,
    })?;
    let irreversible = effect == EffectClass::Irreversible;
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
        elevated_authority: irreversible
            .then(|| tool_policy_binding(&policy))
            .transpose()?,
        expected_versions: Vec::new(),
        operation: payload,
        operation_hash,
        effect_class: effect,
        preconditions: Vec::new(),
        budget_reservation: tool_policy_binding(&policy)?,
        idempotency_key: format!("http-runner-qualification-{action_id}"),
        compensation: None,
        compensates_action: None,
        reconciler: irreversible
            .then(|| governed_http_reconciler(operation))
            .transpose()?,
        reconciliation_deadline: irreversible.then_some(deadline),
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
            media_type: "application/vnd.metacog.governed-http-operation+cbor".into(),
            byte_length: u64::try_from(envelope.operation.bytes.len())
                .map_err(|error| error.to_string())?,
        },
        argument_hash: envelope.operation_hash.clone(),
        requested_effect_class: effect,
        requested_capabilities: manifest.required_capabilities.clone(),
        requested_limits: limits,
        required_sandbox: manifest.sandbox_requirement,
        platform: evidence.platform,
        deadline,
    };
    let proposal_hash =
        tool_invocation_proposal_hash(&proposal).map_err(|error| error.to_string())?;
    let handles = credential_references
        .iter()
        .map(|_| SecretHandleId::new())
        .collect::<Vec<_>>();
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
        allowed_effect_class: effect,
        capabilities: proposal.requested_capabilities.clone(),
        limits,
        sandbox: evidence.available_sandbox,
        platform: evidence.platform,
        policy: tool_policy_binding(&policy)?,
        expires_at: deadline,
        secret_handles: handles.clone(),
        issued_at: now,
    };
    let permit_hash = tool_invocation_permit_hash(&permit).map_err(|error| error.to_string())?;
    let secret_handles = handles
        .into_iter()
        .zip(credential_references)
        .map(
            |(handle_id, (provider, key, slot))| HttpRunnerSecretHandleBinding {
                handle_id,
                permit_id: permit.id,
                provider,
                key,
                slot,
                expires_at: deadline,
            },
        )
        .collect();
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
        secret_handles,
    };
    http_request_from_authorized_action(&envelope, &execution, authorization, Uuid::new_v4(), now)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_policy_denies_hostname_rebinding_but_allows_explicit_local_ip() {
        assert!(is_non_public(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!is_unroutable(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_unroutable(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    }

    #[test]
    fn credential_slots_are_stable_and_renewable_slots_are_denied() {
        assert_eq!(
            http_secret_slot("openai", "primary").expect("slot"),
            http_secret_slot("openai", "primary").expect("slot")
        );
        assert!(http_secret_slot("openai", "oauth_refresh_token").is_err());
    }
}
