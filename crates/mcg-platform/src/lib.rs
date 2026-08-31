//! Safe cross-platform primitives for Protected Root local IPC.

use std::{
    fmt, fs,
    io::{ErrorKind, Read as _, Write as _},
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::fmt::Write as _;

use chrono::{DateTime, Duration, Utc};
use mcg_contracts::{
    ActionCompensationReceipt, ActionId, ActionInspection, ActionSummary, AuthenticatedRpcRequest,
    ContentHash, ContractError, CorrelationId, InvocationId, InvocationInspection,
    InvocationSummary, OwnerActionCompensationRequest, PROTOCOL_V1, RequestId, RootStatus,
    RpcEnvelope, RuntimeInstanceId, RuntimeWorkerRole, SessionId, StableTypeId, VersionedPayload,
    WorkflowAttempt, WorkflowCancelRequest, WorkflowId, WorkflowInspection, canonical_cbor_v1,
    decode_canonical_cbor_v1,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub const MAX_ROOT_IPC_FRAME_BYTES: usize = 1024 * 1024;
pub const ROOT_IPC_SESSION_SCHEMA_VERSION: u16 = 1;
pub const RUNTIME_WORKER_SESSION_SCHEMA_VERSION: u16 = 1;
pub const RUNTIME_WORKER_EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.RuntimeWorkerExecutable";
pub const MAX_ROOT_INVOCATION_SUMMARIES: usize = 200;
pub const MAX_ROOT_INVOCATION_MODEL_RUNS: usize = 100;
pub const ROOT_STATUS_REQUEST_TYPE: &str = "metacog.root.status.request.v1";
pub const ROOT_STATUS_RESPONSE_TYPE: &str = "metacog.root.status.response.v1";
pub const ROOT_WORKFLOW_LIST_REQUEST_TYPE: &str = "metacog.root.workflow_list.request.v1";
pub const ROOT_WORKFLOW_LIST_RESPONSE_TYPE: &str = "metacog.root.workflow_list.response.v1";
pub const ROOT_WORKFLOW_SHOW_REQUEST_TYPE: &str = "metacog.root.workflow_show.request.v1";
pub const ROOT_WORKFLOW_SHOW_RESPONSE_TYPE: &str = "metacog.root.workflow_show.response.v1";
pub const ROOT_WORKFLOW_CANCEL_REQUEST_TYPE: &str = "metacog.root.workflow_cancel.request.v1";
pub const ROOT_WORKFLOW_CANCEL_RESPONSE_TYPE: &str = "metacog.root.workflow_cancel.response.v1";
pub const ROOT_ACTION_LIST_REQUEST_TYPE: &str = "metacog.root.action_list.request.v1";
pub const ROOT_ACTION_LIST_RESPONSE_TYPE: &str = "metacog.root.action_list.response.v1";
pub const ROOT_ACTION_SHOW_REQUEST_TYPE: &str = "metacog.root.action_show.request.v1";
pub const ROOT_ACTION_SHOW_RESPONSE_TYPE: &str = "metacog.root.action_show.response.v1";
pub const ROOT_ACTION_COMPENSATE_REQUEST_TYPE: &str = "metacog.root.action_compensate.request.v1";
pub const ROOT_ACTION_COMPENSATE_RESPONSE_TYPE: &str = "metacog.root.action_compensate.response.v1";
pub const ROOT_INVOCATION_LIST_REQUEST_TYPE: &str = "metacog.root.invocation_list.request.v1";
pub const ROOT_INVOCATION_LIST_RESPONSE_TYPE: &str = "metacog.root.invocation_list.response.v1";
pub const ROOT_INVOCATION_SHOW_REQUEST_TYPE: &str = "metacog.root.invocation_show.request.v1";
pub const ROOT_INVOCATION_SHOW_RESPONSE_TYPE: &str = "metacog.root.invocation_show.response.v1";
const ROOT_SESSION_FILE: &str = "root-ipc-session.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct RootIpcSession {
    pub schema_version: u16,
    pub session_id: SessionId,
    pub endpoint: String,
    pub capability: String,
    pub root_process_id: u32,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for RootIpcSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootIpcSession")
            .field("schema_version", &self.schema_version)
            .field("session_id", &self.session_id)
            .field("endpoint", &self.endpoint)
            .field("root_process_id", &self.root_process_id)
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

impl Drop for RootIpcSession {
    fn drop(&mut self) {
        self.capability.zeroize();
    }
}

impl RootIpcSession {
    #[must_use]
    pub fn verifies(&self, candidate: &str) -> bool {
        candidate.len() == self.capability.len()
            && bool::from(candidate.as_bytes().ct_eq(self.capability.as_bytes()))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeWorkerSession {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub endpoint: String,
    pub capability: String,
    pub role: RuntimeWorkerRole,
    pub worker_generation: u64,
    pub expected_executable_hash: ContentHash,
    pub root_process_id: u32,
    pub expires_at: DateTime<Utc>,
}

impl fmt::Debug for RuntimeWorkerSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeWorkerSession")
            .field("schema_version", &self.schema_version)
            .field("protocol_version", &self.protocol_version)
            .field("session_id", &self.session_id)
            .field("endpoint", &self.endpoint)
            .field("role", &self.role)
            .field("worker_generation", &self.worker_generation)
            .field("expected_executable_hash", &self.expected_executable_hash)
            .field("root_process_id", &self.root_process_id)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl Drop for RuntimeWorkerSession {
    fn drop(&mut self) {
        self.capability.zeroize();
    }
}

impl RuntimeWorkerSession {
    #[must_use]
    pub fn verifies(&self, candidate: &str) -> bool {
        candidate.len() == self.capability.len()
            && bool::from(candidate.as_bytes().ct_eq(self.capability.as_bytes()))
    }

    /// Validates one short-lived, executable- and generation-bound launch session.
    ///
    /// # Errors
    /// Returns an error for a malformed, expired, weak, or unsupported session.
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), PlatformError> {
        if self.schema_version != RUNTIME_WORKER_SESSION_SCHEMA_VERSION
            || self.protocol_version != PROTOCOL_V1
            || self.worker_generation == 0
            || self.root_process_id == 0
            || self.capability.len() < 64
            || self.endpoint.is_empty()
            || self.expires_at <= now
            || self.expected_executable_hash.validate().is_err()
        {
            return Err(PlatformError::InvalidSession);
        }
        Ok(())
    }
}

pub trait LocalIpcIo: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> LocalIpcIo for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub type LocalIpcStream = Box<dyn LocalIpcIo>;

pub struct RuntimeWorkerListener {
    endpoint: String,
    #[cfg(windows)]
    next: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
    #[cfg(unix)]
    owner_uid: u32,
}

impl RuntimeWorkerListener {
    /// Binds an owner-local Runtime endpoint before the child is launched.
    ///
    /// # Errors
    /// Returns an error when the endpoint cannot be exclusively bound or protected.
    pub fn bind(endpoint: &str) -> Result<Self, PlatformError> {
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;

            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(endpoint)?;
            Ok(Self {
                endpoint: endpoint.to_owned(),
                next: Some(next),
            })
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};

            let path = PathBuf::from(endpoint);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let listener = tokio::net::UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            let parent = path.parent().ok_or(PlatformError::InvalidSession)?;
            let owner_uid = fs::metadata(parent)?.uid();
            Ok(Self {
                endpoint: endpoint.to_owned(),
                listener,
                owner_uid,
            })
        }
    }

    /// Accepts one same-owner worker connection.
    ///
    /// # Errors
    /// Returns an error for transport failure or a peer-credential mismatch.
    pub async fn accept(&mut self) -> Result<LocalIpcStream, PlatformError> {
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;

            let server = self.next.take().ok_or(PlatformError::InvalidSession)?;
            server.connect().await?;
            self.next = Some(
                ServerOptions::new()
                    .reject_remote_clients(true)
                    .create(&self.endpoint)?,
            );
            Ok(Box::new(server))
        }
        #[cfg(unix)]
        {
            let (stream, _) = self.listener.accept().await?;
            let credential = stream.peer_cred()?;
            if credential.uid() != self.owner_uid {
                return Err(PlatformError::InvalidSession);
            }
            Ok(Box::new(stream))
        }
    }
}

#[cfg(unix)]
impl Drop for RuntimeWorkerListener {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.endpoint);
    }
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("local IPC I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("local IPC contract failed: {0}")]
    Contract(#[from] ContractError),
    #[error("local IPC serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("local IPC frame exceeds its size limit")]
    FrameTooLarge,
    #[error("local IPC frame is empty")]
    EmptyFrame,
    #[error("local IPC session file is invalid")]
    InvalidSession,
    #[error("local IPC inspection bound is invalid")]
    InvalidInspectionBound,
}

/// Derives the per-profile local endpoint without including raw user-controlled paths.
#[must_use]
pub fn root_ipc_endpoint(data_directory: &Path) -> String {
    #[cfg(windows)]
    {
        let digest = Sha256::digest(data_directory.to_string_lossy().to_lowercase().as_bytes());
        let suffix = digest[..12]
            .iter()
            .fold(String::with_capacity(24), |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to String cannot fail");
                output
            });
        format!(r"\\.\pipe\metacog-root-{suffix}")
    }
    #[cfg(not(windows))]
    {
        data_directory
            .join("root.sock")
            .to_string_lossy()
            .into_owned()
    }
}

/// Derives a unique local endpoint for one Root-launched Runtime generation.
#[must_use]
pub fn runtime_worker_ipc_endpoint(data_directory: &Path, session_id: SessionId) -> String {
    #[cfg(windows)]
    {
        let material = format!(
            "{}:{}",
            data_directory.to_string_lossy().to_lowercase(),
            session_id
        );
        let digest = Sha256::digest(material.as_bytes());
        let suffix = digest[..12]
            .iter()
            .fold(String::with_capacity(24), |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to String cannot fail");
                output
            });
        format!(r"\\.\pipe\metacog-runtime-{suffix}")
    }
    #[cfg(not(windows))]
    {
        data_directory
            .join(format!("runtime-{}.sock", session_id.0.simple()))
            .to_string_lossy()
            .into_owned()
    }
}

/// Creates an in-memory, launch-scoped Runtime capability. It is never published to disk.
///
/// # Errors
/// Returns an error for a zero generation or invalid executable identity.
pub fn create_runtime_worker_session(
    data_directory: &Path,
    worker_generation: u64,
    expected_executable_hash: ContentHash,
    expires_at: DateTime<Utc>,
) -> Result<RuntimeWorkerSession, PlatformError> {
    let session_id = SessionId::new();
    let session = RuntimeWorkerSession {
        schema_version: RUNTIME_WORKER_SESSION_SCHEMA_VERSION,
        protocol_version: PROTOCOL_V1,
        session_id,
        endpoint: runtime_worker_ipc_endpoint(data_directory, session_id),
        capability: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        role: RuntimeWorkerRole::Cognition,
        worker_generation,
        expected_executable_hash,
        root_process_id: std::process::id(),
        expires_at,
    };
    session.validate(Utc::now())?;
    Ok(session)
}

/// Computes the exact executable identity bound into a Runtime worker launch session.
///
/// # Errors
/// Returns an error when the executable cannot be opened or read.
pub fn runtime_worker_executable_hash(
    path: impl AsRef<Path>,
) -> Result<ContentHash, PlatformError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(ContentHash {
        algorithm: mcg_contracts::HashAlgorithm::Sha256,
        domain: RUNTIME_WORKER_EXECUTABLE_HASH_DOMAIN.to_owned(),
        digest_hex: format!("{:x}", hasher.finalize()),
    })
}

/// Connects a worker to the Root-bound endpoint before the launch deadline.
///
/// # Errors
/// Returns an error when the endpoint remains unavailable or the deadline elapses.
pub async fn connect_runtime_worker(
    endpoint: &str,
    deadline: DateTime<Utc>,
) -> Result<LocalIpcStream, PlatformError> {
    loop {
        if Utc::now() >= deadline {
            return Err(PlatformError::InvalidSession);
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;

            if let Ok(pipe) = ClientOptions::new().open(endpoint) {
                return Ok(Box::new(pipe));
            }
        }
        #[cfg(unix)]
        {
            if let Ok(stream) = tokio::net::UnixStream::connect(endpoint).await {
                return Ok(Box::new(stream));
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[must_use]
pub fn root_ipc_session_path(data_directory: &Path) -> PathBuf {
    data_directory.join(ROOT_SESSION_FILE)
}

/// Creates and durably publishes a fresh launch-scoped Root IPC capability.
///
/// # Errors
/// Returns an error when the private session file cannot be created.
pub fn create_root_ipc_session(data_directory: &Path) -> Result<RootIpcSession, PlatformError> {
    let session = RootIpcSession {
        schema_version: ROOT_IPC_SESSION_SCHEMA_VERSION,
        session_id: SessionId::new(),
        endpoint: root_ipc_endpoint(data_directory),
        capability: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        root_process_id: std::process::id(),
        created_at: Utc::now(),
    };
    let path = root_ipc_session_path(data_directory);
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let bytes = Zeroizing::new(serde_json::to_vec(&session)?);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(session)
}

/// Loads the current launch-scoped Root IPC session from the owner profile.
///
/// # Errors
/// Returns an error for missing, malformed, stale-shape, or weak session data.
pub fn load_root_ipc_session(data_directory: &Path) -> Result<RootIpcSession, PlatformError> {
    let bytes = Zeroizing::new(fs::read(root_ipc_session_path(data_directory))?);
    let session: RootIpcSession = serde_json::from_slice(&bytes)?;
    if session.schema_version != ROOT_IPC_SESSION_SCHEMA_VERSION
        || session.capability.len() < 64
        || session.root_process_id == 0
        || session.endpoint != root_ipc_endpoint(data_directory)
    {
        return Err(PlatformError::InvalidSession);
    }
    Ok(session)
}

/// Removes a session file only when it still names the launch being shut down.
pub fn remove_root_ipc_session(data_directory: &Path, session_id: SessionId) {
    let path = root_ipc_session_path(data_directory);
    if load_root_ipc_session(data_directory).is_ok_and(|session| session.session_id == session_id) {
        let _ = fs::remove_file(path);
    }
}

/// Writes one bounded, canonical, length-framed IPC value.
///
/// # Errors
/// Returns an error for unsupported values, oversize frames, or stream failures.
pub async fn write_canonical_frame<T, W>(writer: &mut W, value: &T) -> Result<(), PlatformError>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let value = serde_json::to_value(value)?;
    let bytes = canonical_cbor_v1(&value)?;
    if bytes.is_empty() || bytes.len() > MAX_ROOT_IPC_FRAME_BYTES {
        return Err(PlatformError::FrameTooLarge);
    }
    let length = u32::try_from(bytes.len()).map_err(|_| PlatformError::FrameTooLarge)?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one bounded canonical frame without allocating from an untrusted length.
///
/// # Errors
/// Returns an error for partial, empty, oversized, malformed, or noncanonical frames.
pub async fn read_canonical_frame<T, R>(reader: &mut R) -> Result<T, PlatformError>
where
    T: DeserializeOwned,
    R: AsyncRead + Unpin,
{
    let length = reader.read_u32().await? as usize;
    if length == 0 {
        return Err(PlatformError::EmptyFrame);
    }
    if length > MAX_ROOT_IPC_FRAME_BYTES {
        return Err(PlatformError::FrameTooLarge);
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes).await?;
    let value = decode_canonical_cbor_v1(&bytes)?;
    Ok(serde_json::from_value(value)?)
}

/// Queries the current Protected Root over its launch-scoped local IPC session.
///
/// # Errors
/// Returns an error for unavailable transport, invalid session proof, malformed
/// response, correlation mismatch, or unsupported protocol data.
pub async fn query_root_status(data_directory: &Path) -> Result<RootStatus, PlatformError> {
    query_root(
        data_directory,
        ROOT_STATUS_REQUEST_TYPE,
        ROOT_STATUS_RESPONSE_TYPE,
        &serde_json::json!({}),
    )
    .await
}

/// Lists the latest attempt for every canonical workflow over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport or invalid/correlated protocol data.
pub async fn query_root_workflows(
    data_directory: &Path,
) -> Result<Vec<WorkflowAttempt>, PlatformError> {
    query_root(
        data_directory,
        ROOT_WORKFLOW_LIST_REQUEST_TYPE,
        ROOT_WORKFLOW_LIST_RESPONSE_TYPE,
        &serde_json::json!({}),
    )
    .await
}

/// Loads one canonical workflow and its ordered evidence over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport or invalid/correlated protocol data.
pub async fn query_root_workflow(
    data_directory: &Path,
    workflow_id: WorkflowId,
) -> Result<Option<WorkflowInspection>, PlatformError> {
    query_root(
        data_directory,
        ROOT_WORKFLOW_SHOW_REQUEST_TYPE,
        ROOT_WORKFLOW_SHOW_RESPONSE_TYPE,
        &serde_json::json!({"workflow_id": workflow_id}),
    )
    .await
}

/// Requests durable, generation-fenced cancellation over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport, invalid protocol data, or rejected cancellation.
pub async fn cancel_root_workflow(
    data_directory: &Path,
    request: &WorkflowCancelRequest,
) -> Result<WorkflowAttempt, PlatformError> {
    query_root(
        data_directory,
        ROOT_WORKFLOW_CANCEL_REQUEST_TYPE,
        ROOT_WORKFLOW_CANCEL_RESPONSE_TYPE,
        request,
    )
    .await
}

/// Lists a bounded newest-first projection of canonical actions over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport, invalid bounds, or correlated protocol failure.
pub async fn query_root_actions(
    data_directory: &Path,
    limit: usize,
) -> Result<Vec<ActionSummary>, PlatformError> {
    query_root(
        data_directory,
        ROOT_ACTION_LIST_REQUEST_TYPE,
        ROOT_ACTION_LIST_RESPONSE_TYPE,
        &serde_json::json!({"limit": limit}),
    )
    .await
}

/// Loads one canonical action and bounded ordered evidence over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport, invalid bounds, or correlated protocol failure.
pub async fn query_root_action(
    data_directory: &Path,
    action_id: ActionId,
    event_limit: usize,
) -> Result<Option<ActionInspection>, PlatformError> {
    query_root(
        data_directory,
        ROOT_ACTION_SHOW_REQUEST_TYPE,
        ROOT_ACTION_SHOW_RESPONSE_TYPE,
        &serde_json::json!({"action_id": action_id, "event_limit": event_limit}),
    )
    .await
}

/// Initiates or exactly replays one owner-confirmed causal compensation through Protected Root.
///
/// # Errors
/// Returns an error for unavailable transport, stale identities, invalid owner confirmation, or
/// any rejected Root authority or action-runner boundary.
pub async fn compensate_root_action(
    data_directory: &Path,
    request: &OwnerActionCompensationRequest,
) -> Result<ActionCompensationReceipt, PlatformError> {
    request.validate()?;
    query_root(
        data_directory,
        ROOT_ACTION_COMPENSATE_REQUEST_TYPE,
        ROOT_ACTION_COMPENSATE_RESPONSE_TYPE,
        request,
    )
    .await
}

/// Lists bounded newest-first reconstructable invocations over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport, invalid bounds, or correlated protocol failure.
pub async fn query_root_invocations(
    data_directory: &Path,
    limit: usize,
) -> Result<Vec<InvocationSummary>, PlatformError> {
    if limit == 0 || limit > MAX_ROOT_INVOCATION_SUMMARIES {
        return Err(PlatformError::InvalidInspectionBound);
    }
    query_root(
        data_directory,
        ROOT_INVOCATION_LIST_REQUEST_TYPE,
        ROOT_INVOCATION_LIST_RESPONSE_TYPE,
        &serde_json::json!({"limit": limit}),
    )
    .await
}

/// Loads one canonical bounded invocation reconstruction over authenticated local IPC.
///
/// # Errors
/// Returns an error for unavailable transport, invalid bounds, or correlated protocol failure.
pub async fn query_root_invocation(
    data_directory: &Path,
    invocation_id: InvocationId,
    model_run_limit: usize,
) -> Result<Option<InvocationInspection>, PlatformError> {
    if model_run_limit == 0 || model_run_limit > MAX_ROOT_INVOCATION_MODEL_RUNS {
        return Err(PlatformError::InvalidInspectionBound);
    }
    query_root(
        data_directory,
        ROOT_INVOCATION_SHOW_REQUEST_TYPE,
        ROOT_INVOCATION_SHOW_RESPONSE_TYPE,
        &serde_json::json!({
            "invocation_id": invocation_id,
            "model_run_limit": model_run_limit
        }),
    )
    .await
}

async fn query_root<TRequest, TResponse>(
    data_directory: &Path,
    request_type: &str,
    response_type: &str,
    payload: &TRequest,
) -> Result<TResponse, PlatformError>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    let session = load_root_ipc_session(data_directory)?;
    let request_id = RequestId::new();
    let correlation_id = CorrelationId::new();
    let payload_value = serde_json::to_value(payload)?;
    let request = AuthenticatedRpcRequest {
        envelope: RpcEnvelope {
            protocol_version: PROTOCOL_V1,
            request_id,
            correlation_id,
            session_id: session.session_id,
            caller_instance: RuntimeInstanceId::new(),
            message_type: StableTypeId::parse(request_type)?,
            expires_at: Some(Utc::now() + Duration::seconds(5)),
            payload: VersionedPayload::canonical_v1(
                StableTypeId::parse(request_type)?,
                &payload_value,
            )?,
        },
        session_capability: session.capability.clone(),
    };
    let response = exchange_root_rpc(&session, &request).await?;
    response.validate()?;
    if response.request_id != request_id
        || response.correlation_id != correlation_id
        || response.session_id != session.session_id
        || response.message_type.as_str() != response_type
    {
        return Err(PlatformError::InvalidSession);
    }
    let value = decode_canonical_cbor_v1(&response.payload.bytes)?;
    Ok(serde_json::from_value(value)?)
}

/// Exchanges one already-constructed authenticated Root RPC request.
///
/// This low-level function exists for compatibility clients and adversarial
/// protocol tests. Callers must still validate response correlation and type.
///
/// # Errors
/// Returns transport, framing, canonicalization, or decoding errors.
pub async fn exchange_root_rpc(
    session: &RootIpcSession,
    request: &AuthenticatedRpcRequest,
) -> Result<RpcEnvelope, PlatformError> {
    root_round_trip(&session.endpoint, request).await
}

#[cfg(windows)]
async fn root_round_trip<T: Serialize>(
    endpoint: &str,
    request: &T,
) -> Result<RpcEnvelope, PlatformError> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut pipe = ClientOptions::new().open(endpoint)?;
    write_canonical_frame(&mut pipe, request).await?;
    read_canonical_frame(&mut pipe).await
}

#[cfg(unix)]
async fn root_round_trip<T: Serialize>(
    endpoint: &str,
    request: &T,
) -> Result<RpcEnvelope, PlatformError> {
    let mut stream = tokio::net::UnixStream::connect(endpoint).await?;
    write_canonical_frame(&mut stream, request).await?;
    read_canonical_frame(&mut stream).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn canonical_frames_round_trip_and_reject_oversize_before_allocation() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let write = tokio::spawn(async move {
            write_canonical_frame(&mut client, &json!({"message": "é", "count": 3}))
                .await
                .expect("write frame");
        });
        let value: serde_json::Value = read_canonical_frame(&mut server).await.expect("read frame");
        write.await.expect("writer task");
        assert_eq!(value, json!({"message": "é", "count": 3}));

        let oversized = (u32::try_from(MAX_ROOT_IPC_FRAME_BYTES).unwrap() + 1).to_be_bytes();
        let mut input = oversized.as_slice();
        assert!(matches!(
            read_canonical_frame::<serde_json::Value, _>(&mut input).await,
            Err(PlatformError::FrameTooLarge)
        ));
    }

    #[test]
    fn session_capability_is_launch_scoped_and_private() {
        let temporary = tempfile::tempdir().expect("temporary data directory");
        let session = create_root_ipc_session(temporary.path()).expect("create session");
        let loaded = load_root_ipc_session(temporary.path()).expect("load session");
        assert_eq!(loaded.session_id, session.session_id);
        assert!(loaded.verifies(&session.capability));
        assert!(!loaded.verifies("wrong"));
        remove_root_ipc_session(temporary.path(), session.session_id);
        assert!(!root_ipc_session_path(temporary.path()).exists());
    }

    #[test]
    fn runtime_worker_sessions_are_unique_private_and_executable_bound() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable_hash = VersionedPayload::canonical_v1(
            StableTypeId::parse("test.runtime_executable.v1").unwrap(),
            &json!({"binary": "worker"}),
        )
        .unwrap()
        .content_hash("TestRuntimeExecutable")
        .unwrap();
        let first = create_runtime_worker_session(
            directory.path(),
            1,
            executable_hash.clone(),
            Utc::now() + Duration::seconds(30),
        )
        .unwrap();
        let second = create_runtime_worker_session(
            directory.path(),
            2,
            executable_hash,
            Utc::now() + Duration::seconds(30),
        )
        .unwrap();
        assert_ne!(first.session_id, second.session_id);
        assert_ne!(first.endpoint, second.endpoint);
        assert_ne!(first.capability, second.capability);
        assert!(!format!("{first:?}").contains(&first.capability));
        assert!(first.verifies(&first.capability));
        assert!(!first.verifies(&second.capability));
        assert_eq!(first.worker_generation, 1);
        assert_eq!(second.worker_generation, 2);
    }
}
