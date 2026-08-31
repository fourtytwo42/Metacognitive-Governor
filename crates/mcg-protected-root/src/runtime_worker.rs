//! Root-owned supervision for the dependency-isolated cognition worker.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration as StdDuration,
};

use chrono::{Duration, Utc};
use mcg_contracts::{
    AuthenticatedRpcRequest, ContractError, CorrelationId, PROTOCOL_V1,
    RUNTIME_WORKER_ACK_MESSAGE_TYPE, RUNTIME_WORKER_CANCEL_MESSAGE_TYPE,
    RUNTIME_WORKER_HELLO_MESSAGE_TYPE, RUNTIME_WORKER_INVOKE_MESSAGE_TYPE,
    RUNTIME_WORKER_RESULT_MESSAGE_TYPE, RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE, RequestId,
    RpcEnvelope, RuntimeInstanceId, RuntimeInvocationRequest, RuntimeInvocationResult,
    RuntimeWorkerControl, RuntimeWorkerHello, RuntimeWorkerRole, StableTypeId, VersionedPayload,
    decode_canonical_cbor_v1,
};
use mcg_platform::{
    LocalIpcStream, PlatformError, RuntimeWorkerListener, RuntimeWorkerSession,
    create_runtime_worker_session, read_canonical_frame, runtime_worker_executable_hash,
    write_canonical_frame,
};
use thiserror::Error;
use tokio::{
    process::{Child, Command},
    time::timeout,
};
use zeroize::Zeroize;

const DEFAULT_HANDSHAKE_TIMEOUT: StdDuration = StdDuration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: StdDuration = StdDuration::from_secs(10);
const DEFAULT_SESSION_LIFETIME: StdDuration = StdDuration::from_secs(60 * 60);

#[derive(Debug, Error)]
pub enum RuntimeWorkerClientError {
    #[error("Runtime worker is unavailable")]
    Unavailable,
    #[error("Runtime worker process could not be launched: {0}")]
    Launch(#[source] std::io::Error),
    #[error("Runtime worker platform boundary failed: {0}")]
    Platform(#[from] PlatformError),
    #[error("Runtime worker contract failed: {0}")]
    Contract(#[from] ContractError),
    #[error("Runtime worker serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Runtime worker did not respond before its deadline")]
    Timeout,
    #[error("Runtime worker identity or response was rejected")]
    InvalidResponse,
}

/// One authenticated Root-owned Runtime process generation.
pub struct RuntimeWorkerClient {
    child: Child,
    stream: LocalIpcStream,
    session: RuntimeWorkerSession,
    hello: RuntimeWorkerHello,
    root_instance: RuntimeInstanceId,
    request_timeout: StdDuration,
    usable: bool,
}

impl std::fmt::Debug for RuntimeWorkerClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeWorkerClient")
            .field("process_id", &self.hello.process_id)
            .field("worker_generation", &self.hello.worker_generation)
            .field("runtime_identity", &self.hello.runtime_identity)
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl Drop for RuntimeWorkerClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl RuntimeWorkerClient {
    /// Launches the fixed worker adjacent to the Root executable.
    ///
    /// # Errors
    /// Returns an error when the adjacent executable is absent or fails authentication.
    pub async fn adjacent_to_root(
        root_executable: impl AsRef<Path>,
        data_directory: impl AsRef<Path>,
        worker_generation: u64,
    ) -> Result<Self, RuntimeWorkerClientError> {
        let root_executable = root_executable.as_ref();
        let parent = root_executable
            .parent()
            .ok_or(RuntimeWorkerClientError::Unavailable)?;
        let worker_name = if cfg!(windows) {
            "mcg-runtime-worker.exe"
        } else {
            "mcg-runtime-worker"
        };
        Self::launch(
            parent.join(worker_name),
            data_directory,
            worker_generation,
            DEFAULT_HANDSHAKE_TIMEOUT,
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_SESSION_LIFETIME,
        )
        .await
    }

    /// Launches and authenticates one explicit worker executable. This exists for deterministic
    /// package and process tests; production uses [`Self::adjacent_to_root`].
    ///
    /// # Errors
    /// Returns an error for launch, bootstrap, timeout, or identity mismatch.
    pub async fn launch(
        executable: impl Into<PathBuf>,
        data_directory: impl AsRef<Path>,
        worker_generation: u64,
        handshake_timeout: StdDuration,
        request_timeout: StdDuration,
        session_lifetime: StdDuration,
    ) -> Result<Self, RuntimeWorkerClientError> {
        let executable = executable.into();
        if !executable.is_file() {
            return Err(RuntimeWorkerClientError::Unavailable);
        }
        let executable_hash = runtime_worker_executable_hash(&executable)?;
        let lifetime = Duration::from_std(session_lifetime)
            .map_err(|_| RuntimeWorkerClientError::InvalidResponse)?;
        let session = create_runtime_worker_session(
            data_directory.as_ref(),
            worker_generation,
            executable_hash,
            Utc::now() + lifetime,
        )?;
        let mut listener = RuntimeWorkerListener::bind(&session.endpoint)?;
        let mut child = Command::new(&executable)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(RuntimeWorkerClientError::Launch)?;
        let process_id = child
            .id()
            .ok_or(RuntimeWorkerClientError::InvalidResponse)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(RuntimeWorkerClientError::InvalidResponse)?;
        timeout(
            handshake_timeout,
            write_canonical_frame(&mut stdin, &session),
        )
        .await
        .map_err(|_| RuntimeWorkerClientError::Timeout)??;
        drop(stdin);
        let mut stream = timeout(handshake_timeout, listener.accept())
            .await
            .map_err(|_| RuntimeWorkerClientError::Timeout)??;
        let mut hello_request: AuthenticatedRpcRequest =
            timeout(handshake_timeout, read_canonical_frame(&mut stream))
                .await
                .map_err(|_| RuntimeWorkerClientError::Timeout)??;
        let hello = validate_hello(&session, process_id, &hello_request)?;
        hello_request.session_capability.zeroize();
        Ok(Self {
            child,
            stream,
            session,
            hello,
            root_instance: RuntimeInstanceId::new(),
            request_timeout,
            usable: true,
        })
    }

    #[must_use]
    pub fn hello(&self) -> &RuntimeWorkerHello {
        &self.hello
    }

    #[must_use]
    pub fn worker_generation(&self) -> u64 {
        self.session.worker_generation
    }

    /// Dispatches one already-persisted invocation and returns only an authenticated, exactly
    /// correlated proposal from this process generation.
    ///
    /// # Errors
    /// Returns an error for substituted identity, transport failure, deadline, or malformed data.
    pub async fn invoke(
        &mut self,
        request: &RuntimeInvocationRequest,
    ) -> Result<RuntimeInvocationResult, RuntimeWorkerClientError> {
        request.validate()?;
        if request.worker_generation != self.session.worker_generation
            || request.invocation_manifest.runtime_identity != self.hello.runtime_identity
            || request.invocation_manifest.runtime_build != self.hello.executable_hash
        {
            return Err(RuntimeWorkerClientError::InvalidResponse);
        }
        let response = self
            .exchange(
                RUNTIME_WORKER_INVOKE_MESSAGE_TYPE,
                RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
                request,
            )
            .await?;
        let result: RuntimeInvocationResult = match serde_json::from_value(response) {
            Ok(result) => result,
            Err(error) => {
                self.poison();
                return Err(error.into());
            }
        };
        if let Err(error) = result.validate() {
            self.poison();
            return Err(error.into());
        }
        if result.worker_generation != request.worker_generation
            || result.invocation_id != request.invocation_manifest.id
            || result.model_run_id != request.model_run_id
            || result.role != request.invocation_manifest.role
            || result.request_hash != request.invocation_manifest.request_hash
        {
            self.poison();
            return Err(RuntimeWorkerClientError::InvalidResponse);
        }
        Ok(result)
    }

    /// Requests generation-fenced cancellation of worker-local activity.
    ///
    /// # Errors
    /// Returns an error when the channel is unavailable or the acknowledgement is invalid.
    pub async fn cancel(&mut self) -> Result<(), RuntimeWorkerClientError> {
        self.control(RUNTIME_WORKER_CANCEL_MESSAGE_TYPE).await
    }

    /// Forcefully terminates and reaps this exact worker generation. Root uses this after a
    /// deadline or when replacing a compromised/broken channel; no invocation result is accepted.
    ///
    /// # Errors
    /// Returns an error when the exact child cannot be killed or reaped.
    pub async fn terminate(&mut self) -> Result<(), RuntimeWorkerClientError> {
        if self
            .child
            .try_wait()
            .map_err(RuntimeWorkerClientError::Launch)?
            .is_none()
        {
            self.child
                .kill()
                .await
                .map_err(RuntimeWorkerClientError::Launch)?;
            self.child
                .wait()
                .await
                .map_err(RuntimeWorkerClientError::Launch)?;
        }
        self.usable = false;
        Ok(())
    }

    /// Requests clean shutdown and waits for the exact child to exit.
    ///
    /// # Errors
    /// Returns an error when shutdown is not acknowledged or the child cannot be reaped.
    pub async fn shutdown(mut self) -> Result<(), RuntimeWorkerClientError> {
        self.control(RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE).await?;
        timeout(self.request_timeout, self.child.wait())
            .await
            .map_err(|_| RuntimeWorkerClientError::Timeout)?
            .map_err(RuntimeWorkerClientError::Launch)?;
        Ok(())
    }

    async fn control(&mut self, message_type: &str) -> Result<(), RuntimeWorkerClientError> {
        let control = RuntimeWorkerControl {
            worker_generation: self.session.worker_generation,
        };
        let value = self
            .exchange(message_type, RUNTIME_WORKER_ACK_MESSAGE_TYPE, &control)
            .await?;
        let response: RuntimeWorkerControl = serde_json::from_value(value)?;
        if response != control {
            self.poison();
            return Err(RuntimeWorkerClientError::InvalidResponse);
        }
        Ok(())
    }

    async fn exchange<T: serde::Serialize>(
        &mut self,
        request_type: &str,
        response_type: &str,
        value: &T,
    ) -> Result<serde_json::Value, RuntimeWorkerClientError> {
        if !self.usable {
            return Err(RuntimeWorkerClientError::Unavailable);
        }
        let request_id = RequestId::new();
        let correlation_id = CorrelationId::new();
        let type_id = StableTypeId::parse(request_type)?;
        let expires_at = (Utc::now()
            + Duration::from_std(self.request_timeout)
                .map_err(|_| RuntimeWorkerClientError::InvalidResponse)?)
        .min(self.session.expires_at);
        let request = AuthenticatedRpcRequest {
            envelope: RpcEnvelope {
                protocol_version: PROTOCOL_V1,
                request_id,
                correlation_id,
                session_id: self.session.session_id,
                caller_instance: self.root_instance,
                message_type: type_id.clone(),
                expires_at: Some(expires_at),
                payload: VersionedPayload::canonical_v1(type_id, &serde_json::to_value(value)?)?,
            },
            session_capability: self.session.capability.clone(),
        };
        let delivered = timeout(
            self.request_timeout,
            write_canonical_frame(&mut self.stream, &request),
        )
        .await;
        match delivered {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                self.poison();
                return Err(error.into());
            }
            Err(_) => {
                self.poison();
                return Err(RuntimeWorkerClientError::Timeout);
            }
        }
        let received = timeout(self.request_timeout, read_canonical_frame(&mut self.stream)).await;
        let response: RpcEnvelope = match received {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                self.poison();
                return Err(error.into());
            }
            Err(_) => {
                self.poison();
                return Err(RuntimeWorkerClientError::Timeout);
            }
        };
        if let Err(error) = validate_response(
            &self.session,
            &self.hello,
            &response,
            request_id,
            correlation_id,
            response_type,
        ) {
            self.poison();
            return Err(error);
        }
        match decode_canonical_cbor_v1(&response.payload.bytes) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.poison();
                Err(error.into())
            }
        }
    }

    fn poison(&mut self) {
        self.usable = false;
        let _ = self.child.start_kill();
    }
}

fn validate_hello(
    session: &RuntimeWorkerSession,
    process_id: u32,
    request: &AuthenticatedRpcRequest,
) -> Result<RuntimeWorkerHello, RuntimeWorkerClientError> {
    request.envelope.validate()?;
    let envelope = &request.envelope;
    if !session.verifies(&request.session_capability)
        || envelope.protocol_version != PROTOCOL_V1
        || envelope.session_id != session.session_id
        || envelope.message_type.as_str() != RUNTIME_WORKER_HELLO_MESSAGE_TYPE
        || envelope.payload.type_id.as_str() != RUNTIME_WORKER_HELLO_MESSAGE_TYPE
        || envelope
            .expires_at
            .is_none_or(|expiry| expiry < Utc::now() || expiry > session.expires_at)
    {
        return Err(RuntimeWorkerClientError::InvalidResponse);
    }
    let hello: RuntimeWorkerHello =
        serde_json::from_value(decode_canonical_cbor_v1(&envelope.payload.bytes)?)?;
    hello.validate()?;
    if hello.role != RuntimeWorkerRole::Cognition
        || hello.worker_generation != session.worker_generation
        || hello.process_id != process_id
        || hello.executable_hash != session.expected_executable_hash
        || hello.runtime_identity.instance_id != envelope.caller_instance
    {
        return Err(RuntimeWorkerClientError::InvalidResponse);
    }
    Ok(hello)
}

fn validate_response(
    session: &RuntimeWorkerSession,
    hello: &RuntimeWorkerHello,
    response: &RpcEnvelope,
    request_id: RequestId,
    correlation_id: CorrelationId,
    response_type: &str,
) -> Result<(), RuntimeWorkerClientError> {
    response.validate()?;
    if response.protocol_version != PROTOCOL_V1
        || response.request_id != request_id
        || response.correlation_id != correlation_id
        || response.session_id != session.session_id
        || response.caller_instance != hello.runtime_identity.instance_id
        || response.message_type.as_str() != response_type
        || response.payload.type_id.as_str() != response_type
        || response
            .expires_at
            .is_none_or(|expiry| expiry < Utc::now() || expiry > session.expires_at)
    {
        return Err(RuntimeWorkerClientError::InvalidResponse);
    }
    Ok(())
}
