use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use chrono::Utc;
use mcg_contracts::{
    ArtifactRef, ContentHash, HashAlgorithm, InvocationId, ModelInvocationPermit,
    ModelInvocationProposal, PermitId, SecretHandleId, SourceRef, StableTypeId,
    canonical_contract_hash,
};
use mcg_providers::{
    ModelDescriptor, ModelProvider as _, ModelRequest, ModelResponse,
    OpenAiCompatibleProviderBuilder, ProviderAuthMode, ProviderConnection, ProviderCredential,
    ProviderCredentialStore, ProviderError, ProviderSecretSlot, RetryPolicy,
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

pub const PROVIDER_RUNNER_PROTOCOL_VERSION: u32 = 1;
pub const PROVIDER_RUNNER_REQUEST_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const PROVIDER_RUNNER_RESPONSE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MODEL_PROVIDER_REQUEST_TYPE: &str = "mcg.model_provider_request.v1";
pub const MODEL_PROVIDER_REQUEST_HASH_DOMAIN: &str = "MetaCog.ModelProviderRequest";
pub const MODEL_PROVIDER_RESPONSE_TYPE: &str = "mcg.model_provider_response.v1";
pub const MODEL_PROVIDER_RESPONSE_HASH_DOMAIN: &str = "MetaCog.ModelProviderResponse";
const PROVIDER_RUNNER_STDERR_MAX_BYTES: usize = 4 * 1024;
const PROVIDER_RUNNER_DETAIL_MAX_BYTES: usize = 1024;
const MODEL_INVOCATION_PROPOSAL_HASH_DOMAIN: &str = "MetaCog.ModelInvocationProposal";
const MODEL_INVOCATION_PERMIT_HASH_DOMAIN: &str = "MetaCog.ModelInvocationPermit";
const MODEL_DISCLOSURE_HASH_DOMAIN: &str = "MetaCog.ModelDisclosureManifest";
const MODEL_PERMIT_SOURCE_TYPE: &str = "mcg.model_invocation_permit.v2";
const PROVIDER_CONNECTION_TYPE: &str = "mcg.provider_connection.v1";
const PROVIDER_CONNECTION_HASH_DOMAIN: &str = "MetaCog.ProviderConnection";
const PROVIDER_MODEL_TYPE: &str = "mcg.provider_model.v1";
const PROVIDER_MODEL_HASH_DOMAIN: &str = "MetaCog.ProviderModel";
const PROVIDER_DESTINATION_TYPE: &str = "mcg.provider_destination.v1";
const PROVIDER_DESTINATION_HASH_DOMAIN: &str = "MetaCog.ProviderDestination";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerSecretSlot {
    Primary,
    OAuthIdToken,
    OAuthRefreshToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerSecretHandleBinding {
    pub handle_id: SecretHandleId,
    pub permit_id: PermitId,
    pub connection_id: String,
    pub slot: RunnerSecretSlot,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRunnerAuthorization {
    pub proposal: ModelInvocationProposal,
    pub proposal_hash: ContentHash,
    pub permit: ModelInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
    pub secret_handles: Vec<RunnerSecretHandleBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRunnerRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub authorization: ProviderRunnerAuthorization,
    pub connection: ProviderConnection,
    pub model: ModelDescriptor,
    pub request: ModelRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRunnerOutcome {
    Completed,
    OutcomeUnknown,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRunnerResponse {
    pub protocol_version: u32,
    pub request_id: Option<Uuid>,
    pub invocation_id: Option<InvocationId>,
    pub permit_id: Option<PermitId>,
    pub outcome: ProviderRunnerOutcome,
    pub response: Option<ModelResponse>,
    pub error_code: Option<String>,
    pub detail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderRunnerClientError {
    #[error("provider runner is unavailable")]
    Unavailable,
    #[error("provider runner request is too large")]
    RequestTooLarge,
    #[error("provider runner process could not be launched")]
    Launch,
    #[error("provider runner request delivery failed")]
    Delivery,
    #[error("provider runner timed out")]
    Timeout,
    #[error("provider runner response exceeded its bound")]
    ResponseTooLarge,
    #[error("provider runner wrote to its diagnostic channel")]
    DiagnosticOutput,
    #[error("provider runner returned an invalid response")]
    InvalidResponse,
    #[error("provider runner process failed")]
    Process,
    #[error("provider execution failed after dispatch: {0}")]
    Provider(String),
}

#[derive(Debug, Clone)]
pub struct ProviderRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
}

impl ProviderRunnerClient {
    /// Selects only the fixed provider runner adjacent to the trusted Root executable.
    ///
    /// # Errors
    /// Returns unavailable when the adjacent fixed-name runner is missing or unsafe.
    pub fn adjacent_to(
        application_executable: impl AsRef<Path>,
    ) -> Result<Self, ProviderRunnerClientError> {
        let parent = application_executable
            .as_ref()
            .parent()
            .ok_or(ProviderRunnerClientError::Unavailable)?;
        Self::with_executable(parent.join(runner_file_name()), Duration::from_mins(2))
    }

    /// Creates a bounded client for a fixed-name, regular, non-link executable.
    ///
    /// # Errors
    /// Returns unavailable for a wrong name, unsafe file type, or invalid deadline.
    pub fn with_executable(
        executable: impl AsRef<Path>,
        outer_timeout: Duration,
    ) -> Result<Self, ProviderRunnerClientError> {
        let executable = executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name()))
            || outer_timeout.is_zero()
            || outer_timeout > Duration::from_mins(10)
        {
            return Err(ProviderRunnerClientError::Unavailable);
        }
        let metadata = std::fs::symlink_metadata(executable)
            .map_err(|_| ProviderRunnerClientError::Unavailable)?;
        if !metadata.file_type().is_file() || is_link_or_reparse(&metadata) {
            return Err(ProviderRunnerClientError::Unavailable);
        }
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
        })
    }

    /// Executes one request in a cleared-environment child and validates the correlated response.
    ///
    /// # Errors
    /// Returns a bounded launch, delivery, timeout, output, process, or provider error.
    pub async fn invoke(
        &self,
        request: &ProviderRunnerRequest,
    ) -> Result<ModelResponse, ProviderRunnerClientError> {
        let request_bytes =
            serde_json::to_vec(request).map_err(|_| ProviderRunnerClientError::InvalidResponse)?;
        if request_bytes.len() > PROVIDER_RUNNER_REQUEST_MAX_BYTES {
            return Err(ProviderRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command.env_clear();
        preserve_platform_runtime_environment(&mut command);
        command
            .current_dir(
                self.executable
                    .parent()
                    .ok_or(ProviderRunnerClientError::Unavailable)?,
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
            .map_err(|_| ProviderRunnerClientError::Launch)?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) = (
            child.stdin().take(),
            child.stdout().take(),
            child.stderr().take(),
        ) else {
            let _ = Box::into_pin(child.kill()).await;
            let _ = child.wait().await;
            return Err(ProviderRunnerClientError::Launch);
        };
        let mut stdout_task =
            tokio::spawn(read_bounded(stdout, PROVIDER_RUNNER_RESPONSE_MAX_BYTES));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, PROVIDER_RUNNER_STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        match tokio::time::timeout_at(deadline, async {
            stdin.write_all(&request_bytes).await?;
            stdin.shutdown().await
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProviderRunnerClientError::Delivery);
            }
            Err(_) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProviderRunnerClientError::Timeout);
            }
        }
        drop(stdin);
        let status = match tokio::time::timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProviderRunnerClientError::Process);
            }
            Err(_) => {
                stop_child(child.as_mut(), &stdout_task, &stderr_task).await;
                return Err(ProviderRunnerClientError::Timeout);
            }
        };
        let ((stdout, stdout_overflow), (stderr, stderr_overflow)) =
            tokio::time::timeout_at(deadline, async {
                Ok::<_, ProviderRunnerClientError>((
                    join_reader(&mut stdout_task).await?,
                    join_reader(&mut stderr_task).await?,
                ))
            })
            .await
            .map_err(|_| ProviderRunnerClientError::Timeout)??;
        if stdout_overflow || stderr_overflow {
            return Err(ProviderRunnerClientError::ResponseTooLarge);
        }
        if !stderr.is_empty() {
            return Err(ProviderRunnerClientError::DiagnosticOutput);
        }
        validate_client_response(status.success(), request, &stdout)
    }
}

/// Runs one request using the operating-system protected provider credential store.
pub async fn execute_request(bytes: &[u8]) -> ProviderRunnerResponse {
    execute_request_with_store(bytes, &mcg_providers::OsProviderCredentialStore).await
}

/// Runs one request with a supplied protected store for deterministic qualification.
pub async fn execute_request_with_store<S: ProviderCredentialStore + ?Sized>(
    bytes: &[u8],
    store: &S,
) -> ProviderRunnerResponse {
    let started = Utc::now();
    let request: ProviderRunnerRequest = match serde_json::from_slice(bytes) {
        Ok(request) if bytes.len() <= PROVIDER_RUNNER_REQUEST_MAX_BYTES => request,
        _ => {
            return denied(
                None,
                "invalid_request",
                "provider runner request is invalid",
            );
        }
    };
    let identity = (
        Some(request.request_id),
        Some(request.authorization.proposal.invocation_id),
        Some(request.authorization.permit.id),
    );
    if let Err(code) = validate_runner_request(&request, started) {
        return denied(
            Some(identity),
            code,
            "provider runner authority validation failed",
        );
    }
    let scoped = ScopedCredentialStore {
        inner: store,
        bindings: &request.authorization.secret_handles,
        connection_id: &request.connection.id,
    };
    let Ok(provider) = build_provider(&request.connection, &scoped) else {
        return denied(
            Some(identity),
            "credential_unavailable",
            "provider credential resolution failed",
        );
    };
    match provider.create_response(request.request).await {
        Ok(response) => ProviderRunnerResponse {
            protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
            request_id: identity.0,
            invocation_id: identity.1,
            permit_id: identity.2,
            outcome: ProviderRunnerOutcome::Completed,
            response: Some(response),
            error_code: None,
            detail: "provider request completed".into(),
        },
        Err(error) => ProviderRunnerResponse {
            protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
            request_id: identity.0,
            invocation_id: identity.1,
            permit_id: identity.2,
            outcome: ProviderRunnerOutcome::OutcomeUnknown,
            response: None,
            error_code: Some(provider_error_code(&error).into()),
            detail: "provider request failed after dispatch".into(),
        },
    }
}

fn validate_runner_request(
    request: &ProviderRunnerRequest,
    now: chrono::DateTime<Utc>,
) -> Result<(), &'static str> {
    let authorization = &request.authorization;
    authorization
        .proposal
        .validate()
        .map_err(|_| "invalid_proposal")?;
    authorization
        .permit
        .validate()
        .map_err(|_| "invalid_permit")?;
    request
        .connection
        .validate()
        .map_err(|_| "invalid_connection")?;
    let request_artifact = model_provider_request_artifact(&request.request)
        .map_err(|_| "invalid_request_artifact")?;
    let disclosure_hash = contract_hash(
        mcg_contracts::ModelDisclosureManifest::TYPE_ID,
        MODEL_DISCLOSURE_HASH_DOMAIN,
        &authorization.proposal.disclosure,
    )
    .map_err(|_| "invalid_disclosure_hash")?;
    if request.protocol_version != PROVIDER_RUNNER_PROTOCOL_VERSION
        || now < authorization.permit.issued_at
        || now >= authorization.permit.expires_at
        || authorization.proposal_hash
            != model_proposal_hash(&authorization.proposal).map_err(|_| "invalid_proposal_hash")?
        || authorization.permit_hash
            != model_permit_hash(&authorization.permit).map_err(|_| "invalid_permit_hash")?
        || authorization.proposal.invocation_id != authorization.permit.invocation_id
        || authorization.proposal.attempt_id != authorization.permit.attempt_id
        || authorization.proposal.attempt_revision != authorization.permit.attempt_revision
        || authorization.proposal.lease_generation != authorization.permit.lease_generation
        || authorization.proposal.runtime_actor != authorization.permit.runtime_actor
        || authorization.proposal.runtime_identity != authorization.permit.runtime_identity
        || authorization.proposal.role != authorization.permit.role
        || authorization.proposal.task_contract != authorization.permit.task_contract
        || authorization.permit.proposal_hash != authorization.proposal_hash
        || authorization.permit.disclosure_hash != disclosure_hash
        || authorization.permit.destination_class != authorization.proposal.route.destination_class
        || authorization.permit.limits != authorization.proposal.requested_limits
        || authorization.permit.expires_at != authorization.proposal.deadline
        || authorization.permit_ref.source_type.as_str() != MODEL_PERMIT_SOURCE_TYPE
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != authorization.permit_hash
        || authorization.proposal.request_artifact != request_artifact
        || authorization.proposal.request_hash != request_artifact.content_hash
        || authorization.permit.request_hash != request_artifact.content_hash
        || authorization.proposal.route.provider_id != request.connection.id
        || authorization.proposal.route.model_id != request.model.id
        || authorization.proposal.route.destination != request.connection.base_url
        || authorization.permit.resolved_provider
            != model_provider_source_ref(&request.connection)
                .map_err(|_| "invalid_provider_hash")?
        || authorization.permit.resolved_model
            != model_descriptor_source_ref(&request.connection.id, &request.model)
                .map_err(|_| "invalid_model_hash")?
        || authorization.permit.destination
            != model_destination_source_ref(&request.connection.base_url)
                .map_err(|_| "invalid_destination_hash")?
        || request.request.model != request.model.id
        || request
            .request
            .client_request_id
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err("authority_mismatch");
    }
    validate_handle_bindings(authorization, &request.connection, now)
}

fn validate_handle_bindings(
    authorization: &ProviderRunnerAuthorization,
    connection: &ProviderConnection,
    now: chrono::DateTime<Utc>,
) -> Result<(), &'static str> {
    let expected_slots: &[RunnerSecretSlot] = match connection.auth_mode {
        ProviderAuthMode::None => &[],
        ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken => &[RunnerSecretSlot::Primary],
        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode => {
            return Err("unsupported_oauth");
        }
    };
    if authorization.secret_handles.len() != expected_slots.len()
        || authorization.permit.secret_handles.len() != expected_slots.len()
    {
        return Err("secret_handle_mismatch");
    }
    for (index, slot) in expected_slots.iter().enumerate() {
        let binding = &authorization.secret_handles[index];
        if binding.handle_id != authorization.permit.secret_handles[index]
            || binding.permit_id != authorization.permit.id
            || binding.connection_id != connection.id
            || binding.slot != *slot
            || binding.expires_at != authorization.permit.expires_at
            || now >= binding.expires_at
        {
            return Err("secret_handle_mismatch");
        }
    }
    Ok(())
}

fn build_provider<S: ProviderCredentialStore + ?Sized>(
    connection: &ProviderConnection,
    store: &S,
) -> Result<mcg_providers::OpenAiCompatibleProvider, ProviderError> {
    let mut builder =
        OpenAiCompatibleProviderBuilder::new(connection.clone()).retry_policy(RetryPolicy {
            max_attempts: 1,
            initial_backoff: Duration::ZERO,
            max_backoff: Duration::ZERO,
        });
    match connection.auth_mode {
        ProviderAuthMode::None => {}
        ProviderAuthMode::ApiKey | ProviderAuthMode::BearerToken => {
            let credential = store
                .load(&connection.id, ProviderSecretSlot::Primary)?
                .ok_or_else(|| {
                    ProviderError::Authentication(
                        "authorized provider credential is unavailable".into(),
                    )
                })?;
            builder = builder.credential(credential);
        }
        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode => {
            return Err(ProviderError::Configuration(
                "isolated OAuth provider execution is not yet supported".into(),
            ));
        }
    }
    builder.build()
}

struct ScopedCredentialStore<'a, S: ?Sized> {
    inner: &'a S,
    bindings: &'a [RunnerSecretHandleBinding],
    connection_id: &'a str,
}

impl<S: ProviderCredentialStore + ?Sized> ProviderCredentialStore for ScopedCredentialStore<'_, S> {
    fn load(
        &self,
        connection_id: &str,
        slot: ProviderSecretSlot,
    ) -> Result<Option<ProviderCredential>, ProviderError> {
        let model_slot = match slot {
            ProviderSecretSlot::Primary => RunnerSecretSlot::Primary,
            ProviderSecretSlot::OAuthIdToken => RunnerSecretSlot::OAuthIdToken,
            ProviderSecretSlot::OAuthRefreshToken => RunnerSecretSlot::OAuthRefreshToken,
        };
        if connection_id != self.connection_id
            || !self
                .bindings
                .iter()
                .any(|binding| binding.connection_id == connection_id && binding.slot == model_slot)
        {
            return Err(ProviderError::Authentication(
                "credential slot is outside the Root permit".into(),
            ));
        }
        self.inner.load(connection_id, slot)
    }

    fn save(
        &self,
        _connection_id: &str,
        _slot: ProviderSecretSlot,
        _credential: &ProviderCredential,
    ) -> Result<(), ProviderError> {
        Err(ProviderError::Authentication(
            "provider runner has no credential write authority".into(),
        ))
    }

    fn delete(&self, _connection_id: &str, _slot: ProviderSecretSlot) -> Result<(), ProviderError> {
        Err(ProviderError::Authentication(
            "provider runner has no credential delete authority".into(),
        ))
    }
}

/// Hashes the exact bounded JSON provider request using the stable gateway domain.
///
/// # Errors
/// Returns an error for non-finite sampling values or invalid/oversized JSON.
pub fn model_provider_request_artifact(
    request: &ModelRequest,
) -> Result<ArtifactRef, serde_json::Error> {
    if request.temperature.is_some_and(|value| !value.is_finite())
        || request.top_p.is_some_and(|value| !value.is_finite())
    {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "non-finite sampling value",
        )));
    }
    governed_provider_artifact(
        MODEL_PROVIDER_REQUEST_TYPE,
        MODEL_PROVIDER_REQUEST_HASH_DOMAIN,
        "application/vnd.metacog.model-provider-request+json",
        request,
    )
}

/// Hashes the exact bounded JSON provider response using the stable gateway domain.
///
/// # Errors
/// Returns an error for invalid or oversized JSON.
pub fn model_provider_response_artifact(
    response: &ModelResponse,
) -> Result<ArtifactRef, serde_json::Error> {
    governed_provider_artifact(
        MODEL_PROVIDER_RESPONSE_TYPE,
        MODEL_PROVIDER_RESPONSE_HASH_DOMAIN,
        "application/vnd.metacog.model-provider-response+json",
        response,
    )
}

fn governed_provider_artifact<T: Serialize>(
    type_id: &str,
    hash_domain: &str,
    media_type: &str,
    value: &T,
) -> Result<ArtifactRef, serde_json::Error> {
    let _ = StableTypeId::parse(type_id).map_err(|error| {
        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() || bytes.len() > PROVIDER_RUNNER_RESPONSE_MAX_BYTES {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "governed provider payload size",
        )));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"MetaCog.GovernedProviderArtifact:v1\0");
    hasher.update(type_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(hash_domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(&bytes);
    Ok(ArtifactRef {
        content_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: hash_domain.into(),
            digest_hex: format!("{:x}", hasher.finalize()),
        },
        media_type: media_type.into(),
        byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

fn model_proposal_hash(
    proposal: &ModelInvocationProposal,
) -> Result<ContentHash, serde_json::Error> {
    contract_hash(
        ModelInvocationProposal::TYPE_ID,
        MODEL_INVOCATION_PROPOSAL_HASH_DOMAIN,
        proposal,
    )
}

fn model_permit_hash(permit: &ModelInvocationPermit) -> Result<ContentHash, serde_json::Error> {
    contract_hash(
        ModelInvocationPermit::TYPE_ID,
        MODEL_INVOCATION_PERMIT_HASH_DOMAIN,
        permit,
    )
}

fn model_provider_source_ref(
    connection: &ProviderConnection,
) -> Result<SourceRef, serde_json::Error> {
    source_ref(
        PROVIDER_CONNECTION_TYPE,
        &connection.id,
        PROVIDER_CONNECTION_HASH_DOMAIN,
        connection,
    )
}

fn model_descriptor_source_ref(
    provider_id: &str,
    model: &ModelDescriptor,
) -> Result<SourceRef, serde_json::Error> {
    source_ref(
        PROVIDER_MODEL_TYPE,
        &format!("{provider_id}/{}", model.id),
        PROVIDER_MODEL_HASH_DOMAIN,
        model,
    )
}

fn model_destination_source_ref(destination: &str) -> Result<SourceRef, serde_json::Error> {
    source_ref(
        PROVIDER_DESTINATION_TYPE,
        destination,
        PROVIDER_DESTINATION_HASH_DOMAIN,
        &destination,
    )
}

fn source_ref<T: Serialize>(
    source_type: &str,
    locator: &str,
    domain: &str,
    value: &T,
) -> Result<SourceRef, serde_json::Error> {
    let source = SourceRef {
        source_type: StableTypeId::parse(source_type).map_err(contract_serialization_error)?,
        locator: locator.into(),
        content_hash: contract_hash(source_type, domain, value)?,
    };
    source.validate().map_err(contract_serialization_error)?;
    Ok(source)
}

fn contract_hash<T: Serialize>(
    type_id: &str,
    domain: &str,
    value: &T,
) -> Result<ContentHash, serde_json::Error> {
    canonical_contract_hash(type_id, domain, value).map_err(contract_serialization_error)
}

fn contract_serialization_error(error: impl std::fmt::Display) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        error.to_string(),
    ))
}

fn denied(
    identity: Option<(Option<Uuid>, Option<InvocationId>, Option<PermitId>)>,
    code: &str,
    detail: &str,
) -> ProviderRunnerResponse {
    let (request_id, invocation_id, permit_id) = identity.unwrap_or_default();
    ProviderRunnerResponse {
        protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
        request_id,
        invocation_id,
        permit_id,
        outcome: ProviderRunnerOutcome::Denied,
        response: None,
        error_code: Some(code.into()),
        detail: detail.into(),
    }
}

fn validate_client_response(
    process_succeeded: bool,
    request: &ProviderRunnerRequest,
    bytes: &[u8],
) -> Result<ModelResponse, ProviderRunnerClientError> {
    let response: ProviderRunnerResponse =
        serde_json::from_slice(bytes).map_err(|_| ProviderRunnerClientError::InvalidResponse)?;
    if response.protocol_version != PROVIDER_RUNNER_PROTOCOL_VERSION
        || response.request_id != Some(request.request_id)
        || response.invocation_id != Some(request.authorization.proposal.invocation_id)
        || response.permit_id != Some(request.authorization.permit.id)
        || process_succeeded != (response.outcome == ProviderRunnerOutcome::Completed)
        || response.detail.is_empty()
        || response.detail.len() > PROVIDER_RUNNER_DETAIL_MAX_BYTES
    {
        return Err(ProviderRunnerClientError::InvalidResponse);
    }
    match response.outcome {
        ProviderRunnerOutcome::Completed if response.error_code.is_none() => response
            .response
            .ok_or(ProviderRunnerClientError::InvalidResponse),
        ProviderRunnerOutcome::OutcomeUnknown
            if response.response.is_none()
                && response.error_code.as_deref().is_some_and(valid_code) =>
        {
            Err(ProviderRunnerClientError::Provider(
                response
                    .error_code
                    .unwrap_or_else(|| "provider_failed".into()),
            ))
        }
        ProviderRunnerOutcome::Denied
            if response.response.is_none()
                && response.error_code.as_deref().is_some_and(valid_code) =>
        {
            Err(ProviderRunnerClientError::InvalidResponse)
        }
        _ => Err(ProviderRunnerClientError::InvalidResponse),
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
) -> Result<(Vec<u8>, bool), ProviderRunnerClientError> {
    task.await
        .map_err(|_| ProviderRunnerClientError::Process)?
        .map_err(|_| ProviderRunnerClientError::Process)
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

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(windows)]
fn preserve_platform_runtime_environment(command: &mut Command) {
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

#[cfg(not(windows))]
const fn preserve_platform_runtime_environment(_command: &mut Command) {}

const fn provider_error_code(error: &ProviderError) -> &'static str {
    match error {
        ProviderError::Configuration(_) => "provider_configuration",
        ProviderError::Authentication(_) => "provider_authentication",
        ProviderError::RateLimited { .. } => "provider_rate_limited",
        ProviderError::Request { .. } => "provider_request",
        ProviderError::Http { .. } => "provider_http",
        ProviderError::Incompatible(_) => "provider_incompatible",
    }
}

const fn runner_file_name() -> &'static str {
    if cfg!(windows) {
        "mcg-provider-runner.exe"
    } else {
        "mcg-provider-runner"
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

#[cfg(not(any(unix, windows)))]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;
    use mcg_contracts::{
        AttemptId, HashAlgorithm, InvocationRole, ModelBudget, ModelDataClassification,
        ModelDestinationClass, ModelDisclosureManifest, ModelEgressRoute, PROTOCOL_V1,
        PolicyBinding, RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, TaskContractId,
        VersionedPayload, VersionedRef,
    };
    use mcg_providers::{
        ContentPart, MemoryProviderCredentialStore, Message, MessageRole, ModelCapabilities,
        ProviderKind,
    };
    use tokio::{io::AsyncWriteExt as _, net::TcpListener};

    use super::*;

    fn fixture_hash(domain: &str, seed: &str) -> ContentHash {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: domain.into(),
            digest_hex: format!("{:x}", hasher.finalize()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn fixture_authorization(
        connection: &ProviderConnection,
        model: &ModelDescriptor,
        request: &ModelRequest,
        at: chrono::DateTime<Utc>,
    ) -> ProviderRunnerAuthorization {
        let artifact = model_provider_request_artifact(request).expect("request artifact");
        let handle = SecretHandleId::new();
        let policy_payload = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.provider_runner_policy.v1").expect("policy type"),
            &serde_json::json!({"allow": true}),
        )
        .expect("policy payload");
        let policy = PolicyBinding {
            content_hash: policy_payload
                .content_hash("Test.ProviderRunnerPolicy")
                .expect("policy hash"),
            payload: policy_payload,
        };
        let proposal = ModelInvocationProposal {
            invocation_id: InvocationId::new(),
            attempt_id: AttemptId::new(),
            attempt_revision: 2,
            lease_generation: 3,
            runtime_actor: RuntimeActorId::new(),
            runtime_identity: RuntimeIdentity {
                instance_id: RuntimeInstanceId::new(),
                build_hash: fixture_hash("Test.RuntimeBuild", "runtime"),
                protocol_version: PROTOCOL_V1,
                started_at: at - ChronoDuration::seconds(5),
            },
            role: InvocationRole::Work,
            task_contract: VersionedRef {
                id: TaskContractId::new(),
                version: 1,
                content_hash: fixture_hash("Test.TaskContract", "task"),
            },
            request_artifact: artifact.clone(),
            request_hash: artifact.content_hash,
            route: ModelEgressRoute {
                provider_id: connection.id.clone(),
                model_id: model.id.clone(),
                destination: connection.base_url.clone(),
                destination_class: ModelDestinationClass::LocalLoopback,
            },
            data_classification: ModelDataClassification::Internal,
            disclosure: ModelDisclosureManifest {
                context_manifest: fixture_hash("Test.ContextManifest", "context"),
                disclosed_sources: Vec::new(),
                approximate_input_tokens: 5,
            },
            requested_limits: ModelBudget {
                max_input_tokens: 100,
                max_output_tokens: 50,
                max_cost_microusd: 1_000,
            },
            deadline: at + ChronoDuration::minutes(2),
        };
        let proposal_hash = model_proposal_hash(&proposal).expect("proposal hash");
        let disclosure_hash = contract_hash(
            ModelDisclosureManifest::TYPE_ID,
            MODEL_DISCLOSURE_HASH_DOMAIN,
            &proposal.disclosure,
        )
        .expect("disclosure hash");
        let permit = ModelInvocationPermit {
            id: PermitId::new(),
            invocation_id: proposal.invocation_id,
            attempt_id: proposal.attempt_id,
            attempt_revision: proposal.attempt_revision,
            lease_generation: proposal.lease_generation,
            runtime_actor: proposal.runtime_actor,
            runtime_identity: proposal.runtime_identity.clone(),
            role: proposal.role,
            task_contract: proposal.task_contract.clone(),
            proposal_hash: proposal_hash.clone(),
            request_hash: proposal.request_hash.clone(),
            resolved_provider: model_provider_source_ref(connection).expect("provider ref"),
            resolved_model: model_descriptor_source_ref(&connection.id, model).expect("model ref"),
            destination: model_destination_source_ref(&connection.base_url)
                .expect("destination ref"),
            destination_class: proposal.route.destination_class,
            allowed_data_classification: proposal.data_classification,
            disclosure_hash,
            limits: proposal.requested_limits,
            policy,
            expires_at: proposal.deadline,
            secret_handles: vec![handle],
            issued_at: at,
        };
        let permit_hash = model_permit_hash(&permit).expect("permit hash");
        ProviderRunnerAuthorization {
            proposal,
            proposal_hash,
            permit_ref: SourceRef {
                source_type: StableTypeId::parse(MODEL_PERMIT_SOURCE_TYPE).expect("permit type"),
                locator: permit.id.to_string(),
                content_hash: permit_hash.clone(),
            },
            secret_handles: vec![RunnerSecretHandleBinding {
                handle_id: handle,
                permit_id: permit.id,
                connection_id: connection.id.clone(),
                slot: RunnerSecretSlot::Primary,
                expires_at: permit.expires_at,
            }],
            permit,
            permit_hash,
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn authorized_adapter_resolves_one_secret_and_calls_one_real_http_provider() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture listener");
        let address = listener.local_addr().expect("fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("provider connection");
            let mut request_bytes = vec![0_u8; 64 * 1024];
            let read = stream
                .read(&mut request_bytes)
                .await
                .expect("provider request");
            request_bytes.truncate(read);
            let body = serde_json::json!({
                "id": "isolated-response-1",
                "model": "fixture-model",
                "status": "completed",
                "output": [{"type":"message","role":"assistant","content":[{"type":"output_text","text":"isolated result"}]}],
                "usage": {"input_tokens":5,"output_tokens":3,"total_tokens":8}
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("provider response");
            request_bytes
        });
        let connection = ProviderConnection {
            contract_version: mcg_providers::CONTRACT_VERSION,
            id: "isolated-fixture".into(),
            display_name: "Isolated Fixture".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: format!("http://{address}/v1"),
            auth_mode: ProviderAuthMode::ApiKey,
            organization: None,
            project: None,
        };
        let model = ModelDescriptor {
            id: "fixture-model".into(),
            display_name: "Fixture Model".into(),
            owned_by: Some("fixture".into()),
            context_window: Some(1_024),
            capabilities: ModelCapabilities {
                responses: Some(true),
                ..ModelCapabilities::default()
            },
            price: None,
            metadata: serde_json::Value::Null,
        };
        let request = ModelRequest {
            contract_version: mcg_providers::CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: Some("isolated-client-request-1".into()),
            instructions: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: Some(0.0),
            top_p: None,
            max_output_tokens: Some(50),
            metadata: serde_json::Value::Null,
        };
        let at = Utc::now();
        let authorization = fixture_authorization(&connection, &model, &request, at);
        let store = MemoryProviderCredentialStore::default();
        store
            .save(
                &connection.id,
                ProviderSecretSlot::Primary,
                &ProviderCredential::new("runner-fixture-secret").expect("credential"),
            )
            .expect("save credential");
        let runner_request = ProviderRunnerRequest {
            protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            authorization,
            connection,
            model,
            request,
        };
        let encoded = serde_json::to_vec(&runner_request).expect("runner request");
        assert!(!String::from_utf8_lossy(&encoded).contains("runner-fixture-secret"));
        let response = execute_request_with_store(&encoded, &store).await;
        assert_eq!(response.outcome, ProviderRunnerOutcome::Completed);
        assert_eq!(
            response.response.as_ref().expect("response").id,
            "isolated-response-1"
        );
        let observed_request =
            String::from_utf8(server.await.expect("server result")).expect("http request text");
        assert!(observed_request.contains("authorization: Bearer runner-fixture-secret"));
        assert!(observed_request.contains("POST /v1/responses HTTP/1.1"));
    }

    #[tokio::test]
    async fn adapter_rejects_substitution_before_credential_or_network_access() {
        let connection = ProviderConnection {
            contract_version: mcg_providers::CONTRACT_VERSION,
            id: "substitution-fixture".into(),
            display_name: "Substitution Fixture".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://127.0.0.1:9/v1".into(),
            auth_mode: ProviderAuthMode::ApiKey,
            organization: None,
            project: None,
        };
        let model = ModelDescriptor {
            id: "fixture-model".into(),
            display_name: "Fixture Model".into(),
            owned_by: None,
            context_window: None,
            capabilities: ModelCapabilities::default(),
            price: None,
            metadata: serde_json::Value::Null,
        };
        let request = ModelRequest {
            contract_version: mcg_providers::CONTRACT_VERSION,
            model: model.id.clone(),
            client_request_id: Some("substitution-request".into()),
            instructions: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".into(),
                }],
                name: None,
            }],
            tools: Vec::new(),
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: Some(10),
            metadata: serde_json::Value::Null,
        };
        let mut runner_request = ProviderRunnerRequest {
            protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            authorization: fixture_authorization(&connection, &model, &request, Utc::now()),
            connection,
            model,
            request,
        };
        runner_request.request.model = "substituted-model".into();
        let response = execute_request_with_store(
            &serde_json::to_vec(&runner_request).expect("request"),
            &MemoryProviderCredentialStore::default(),
        )
        .await;
        assert_eq!(response.outcome, ProviderRunnerOutcome::Denied);
        assert_eq!(response.error_code.as_deref(), Some("authority_mismatch"));
    }
}
