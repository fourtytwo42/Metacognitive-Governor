//! Dependency-isolated cognition worker for Root-governed Runtime execution.

use std::{
    collections::{BTreeSet, VecDeque},
    fs::File,
    io::Read as _,
    path::Path,
};

use chrono::Utc;
use mcg_contracts::{
    AuthenticatedRpcRequest, CompletionVerdict, ContentHash, ContractError, CriterionResult,
    DeterministicVerifyInput, HashAlgorithm, InvocationRole, PROTOCOL_V1,
    RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN, RUNTIME_WORKER_ACK_MESSAGE_TYPE,
    RUNTIME_WORKER_CANCEL_MESSAGE_TYPE, RUNTIME_WORKER_HELLO_MESSAGE_TYPE,
    RUNTIME_WORKER_INVOKE_MESSAGE_TYPE, RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
    RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE, RequestId, RpcEnvelope, RuntimeInstanceId,
    RuntimeInvocationRequest, RuntimeInvocationResult, RuntimeWorkerControl, RuntimeWorkerHello,
    RuntimeWorkerRole, StableTypeId, VersionedPayload, canonical_contract_hash,
    decode_canonical_cbor_v1,
};
use mcg_platform::{
    PlatformError, RuntimeWorkerSession, connect_runtime_worker, read_canonical_frame,
    write_canonical_frame,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroize;

const REPLAY_WINDOW: usize = 1_024;
const EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.RuntimeWorkerExecutable";
const CRITERION_HASH_DOMAIN: &str = "MetaCog.DeterministicCriterionEvidence";
const WORK_RESPONSE_TYPE_ID: &str = "mcg.deterministic_work_response.v1";
const VERIFY_RESPONSE_TYPE_ID: &str = "mcg.deterministic_verify_response.v1";

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("runtime worker I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("runtime worker platform boundary failed: {0}")]
    Platform(#[from] PlatformError),
    #[error("runtime worker contract failed: {0}")]
    Contract(#[from] ContractError),
    #[error("runtime worker serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("runtime worker session or request was rejected")]
    InvalidSession,
}

#[derive(Default)]
struct ReplayWindow {
    order: VecDeque<RequestId>,
    seen: BTreeSet<RequestId>,
}

impl ReplayWindow {
    fn insert(&mut self, request_id: RequestId) -> bool {
        if !self.seen.insert(request_id) {
            return false;
        }
        self.order.push_back(request_id);
        if self.order.len() > REPLAY_WINDOW
            && let Some(expired) = self.order.pop_front()
        {
            self.seen.remove(&expired);
        }
        true
    }
}

/// Reads the private launch session from inherited stdin and serves one Root connection.
///
/// # Errors
/// Returns an error for malformed bootstrap data, executable substitution, authentication,
/// transport, protocol, or deterministic execution failure.
pub async fn run_from_stdin() -> Result<(), WorkerError> {
    let mut input = tokio::io::stdin();
    let session: RuntimeWorkerSession = read_canonical_frame(&mut input).await?;
    run(session).await
}

/// Connects to the Root-owned endpoint, authenticates the exact process generation, and serves
/// bounded invocation/control requests until explicit shutdown or channel closure.
///
/// # Errors
/// Returns an error for an invalid launch session, executable mismatch, protocol violation,
/// replay, malformed invocation, or transport failure.
pub async fn run(mut session: RuntimeWorkerSession) -> Result<(), WorkerError> {
    session.validate(Utc::now())?;
    let executable_hash = executable_hash(std::env::current_exe()?)?;
    if executable_hash != session.expected_executable_hash {
        return Err(WorkerError::InvalidSession);
    }
    let runtime_identity = mcg_contracts::RuntimeIdentity {
        instance_id: RuntimeInstanceId::new(),
        build_hash: executable_hash.clone(),
        protocol_version: PROTOCOL_V1,
        started_at: Utc::now(),
    };
    let hello = RuntimeWorkerHello {
        schema_version: RuntimeWorkerHello::SCHEMA_VERSION,
        role: RuntimeWorkerRole::Cognition,
        worker_generation: session.worker_generation,
        process_id: std::process::id(),
        runtime_identity: runtime_identity.clone(),
        executable_hash,
    };
    hello.validate()?;
    let mut stream = connect_runtime_worker(&session.endpoint, session.expires_at).await?;
    let hello_request = authenticated_request(
        &session,
        runtime_identity.instance_id,
        RUNTIME_WORKER_HELLO_MESSAGE_TYPE,
        &hello,
    )?;
    write_canonical_frame(&mut stream, &hello_request).await?;

    let mut replay = ReplayWindow::default();
    loop {
        let mut request: AuthenticatedRpcRequest = match read_canonical_frame(&mut stream).await {
            Ok(request) => request,
            Err(PlatformError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        validate_root_request(&session, &request, &mut replay)?;
        request.session_capability.zeroize();
        let request_type = request.envelope.message_type.as_str();
        let request_value = decode_canonical_cbor_v1(&request.envelope.payload.bytes)?;
        let (response_type, response_value, shutdown) = match request_type {
            RUNTIME_WORKER_INVOKE_MESSAGE_TYPE => {
                let invocation: RuntimeInvocationRequest = serde_json::from_value(request_value)?;
                invocation.validate()?;
                validate_invocation_identity(&session, &runtime_identity, &invocation)?;
                let result = execute_deterministic(&invocation)?;
                (
                    RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
                    serde_json::to_value(result)?,
                    false,
                )
            }
            RUNTIME_WORKER_CANCEL_MESSAGE_TYPE | RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE => {
                let control: RuntimeWorkerControl = serde_json::from_value(request_value)?;
                control.validate()?;
                if control.worker_generation != session.worker_generation {
                    return Err(WorkerError::InvalidSession);
                }
                (
                    RUNTIME_WORKER_ACK_MESSAGE_TYPE,
                    serde_json::to_value(control)?,
                    request_type == RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE,
                )
            }
            _ => return Err(WorkerError::InvalidSession),
        };
        let response = response_envelope(
            &session,
            &runtime_identity,
            &request.envelope,
            response_type,
            &response_value,
        )?;
        write_canonical_frame(&mut stream, &response).await?;
        if shutdown {
            session.capability.zeroize();
            return Ok(());
        }
    }
}

/// Computes the exact executable hash Root binds into one launch session.
///
/// # Errors
/// Returns an error if the executable cannot be read.
pub fn executable_hash(path: impl AsRef<Path>) -> Result<ContentHash, WorkerError> {
    let mut file = File::open(path)?;
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
        algorithm: HashAlgorithm::Sha256,
        domain: EXECUTABLE_HASH_DOMAIN.to_owned(),
        digest_hex: format!("{:x}", hasher.finalize()),
    })
}

fn authenticated_request<T: serde::Serialize>(
    session: &RuntimeWorkerSession,
    caller_instance: RuntimeInstanceId,
    message_type: &str,
    value: &T,
) -> Result<AuthenticatedRpcRequest, WorkerError> {
    let message_type_id = StableTypeId::parse(message_type)?;
    Ok(AuthenticatedRpcRequest {
        envelope: RpcEnvelope {
            protocol_version: PROTOCOL_V1,
            request_id: RequestId::new(),
            correlation_id: mcg_contracts::CorrelationId::new(),
            session_id: session.session_id,
            caller_instance,
            message_type: message_type_id.clone(),
            expires_at: Some((Utc::now() + chrono::Duration::seconds(5)).min(session.expires_at)),
            payload: VersionedPayload::canonical_v1(
                message_type_id,
                &serde_json::to_value(value)?,
            )?,
        },
        session_capability: session.capability.clone(),
    })
}

fn validate_root_request(
    session: &RuntimeWorkerSession,
    request: &AuthenticatedRpcRequest,
    replay: &mut ReplayWindow,
) -> Result<(), WorkerError> {
    request.envelope.validate()?;
    let request_type = request.envelope.message_type.as_str();
    let supported = matches!(
        request_type,
        RUNTIME_WORKER_INVOKE_MESSAGE_TYPE
            | RUNTIME_WORKER_CANCEL_MESSAGE_TYPE
            | RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE
    );
    if !session.verifies(&request.session_capability)
        || request.envelope.session_id != session.session_id
        || request.envelope.payload.type_id.as_str() != request_type
        || request
            .envelope
            .expires_at
            .is_none_or(|expires_at| expires_at < Utc::now() || expires_at > session.expires_at)
        || !supported
        || !replay.insert(request.envelope.request_id)
    {
        return Err(WorkerError::InvalidSession);
    }
    Ok(())
}

fn validate_invocation_identity(
    session: &RuntimeWorkerSession,
    runtime_identity: &mcg_contracts::RuntimeIdentity,
    request: &RuntimeInvocationRequest,
) -> Result<(), WorkerError> {
    if request.worker_generation != session.worker_generation
        || request.invocation_manifest.runtime_identity != *runtime_identity
        || request.invocation_manifest.runtime_build != runtime_identity.build_hash
    {
        return Err(WorkerError::InvalidSession);
    }
    Ok(())
}

fn execute_deterministic(
    request: &RuntimeInvocationRequest,
) -> Result<RuntimeInvocationResult, WorkerError> {
    let (response, criterion_results, verdict) = match request.invocation_manifest.role {
        InvocationRole::Work => (
            VersionedPayload::canonical_v1(
                StableTypeId::parse(WORK_RESPONSE_TYPE_ID)?,
                &serde_json::json!({
                    "input_hash": request.invocation_manifest.request_hash,
                    "invocation_id": request.invocation_manifest.id,
                    "outcome": "deterministic_work_complete",
                }),
            )?,
            Vec::new(),
            None,
        ),
        InvocationRole::Verify => {
            if request.input.type_id.as_str() != DeterministicVerifyInput::TYPE_ID {
                return Err(WorkerError::InvalidSession);
            }
            let input: DeterministicVerifyInput =
                serde_json::from_value(decode_canonical_cbor_v1(&request.input.bytes)?)?;
            input.validate()?;
            let criterion_results = input
                .criteria
                .iter()
                .map(|criterion| {
                    Ok(CriterionResult {
                        clause_id: criterion.clause_id.clone(),
                        passed: criterion.passed,
                        evidence_refs: criterion.evidence_refs.clone(),
                        detail_hash: canonical_contract_hash(
                            "mcg.deterministic_criterion_evidence.v1",
                            CRITERION_HASH_DOMAIN,
                            criterion,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, ContractError>>()?;
            let verdict = if criterion_results.iter().all(|criterion| criterion.passed) {
                CompletionVerdict::Satisfied
            } else {
                CompletionVerdict::NotSatisfied
            };
            (
                VersionedPayload::canonical_v1(
                    StableTypeId::parse(VERIFY_RESPONSE_TYPE_ID)?,
                    &serde_json::json!({
                        "criteria": criterion_results,
                        "verdict": verdict,
                        "work_response_hash": input.work_response_hash,
                    }),
                )?,
                criterion_results,
                Some(verdict),
            )
        }
    };
    let response_hash = response.content_hash(RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)?;
    let result = RuntimeInvocationResult {
        worker_generation: request.worker_generation,
        invocation_id: request.invocation_manifest.id,
        model_run_id: request.model_run_id,
        role: request.invocation_manifest.role,
        request_hash: request.invocation_manifest.request_hash.clone(),
        response,
        response_hash,
        criterion_results,
        verdict,
    };
    result.validate()?;
    Ok(result)
}

fn response_envelope<T: serde::Serialize>(
    session: &RuntimeWorkerSession,
    runtime_identity: &mcg_contracts::RuntimeIdentity,
    request: &RpcEnvelope,
    response_type: &str,
    value: &T,
) -> Result<RpcEnvelope, WorkerError> {
    let response_type = StableTypeId::parse(response_type)?;
    Ok(RpcEnvelope {
        protocol_version: PROTOCOL_V1,
        request_id: request.request_id,
        correlation_id: request.correlation_id,
        session_id: session.session_id,
        caller_instance: runtime_identity.instance_id,
        message_type: response_type.clone(),
        expires_at: Some((Utc::now() + chrono::Duration::seconds(5)).min(session.expires_at)),
        payload: VersionedPayload::canonical_v1(response_type, &serde_json::to_value(value)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_manifest_has_no_protected_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in [
            "mcg-storage",
            "mcg-protected-root",
            "mcg-runtime",
            "mcg-providers",
            "mcg-tools",
            "rusqlite",
            "reqwest",
        ] {
            assert!(
                !manifest
                    .lines()
                    .any(|line| line.trim_start().starts_with(&format!("{forbidden} "))),
                "forbidden dependency: {forbidden}"
            );
        }
    }

    #[test]
    fn executable_hash_is_exact_and_domain_separated() {
        let hash = executable_hash(std::env::current_exe().unwrap()).unwrap();
        hash.validate().unwrap();
        assert_eq!(hash.algorithm, HashAlgorithm::Sha256);
        assert_eq!(hash.domain, EXECUTABLE_HASH_DOMAIN);
    }
}
