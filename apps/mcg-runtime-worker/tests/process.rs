use std::process::Stdio;

use chrono::{Duration, Utc};
use mcg_contracts::{
    AuthenticatedRpcRequest, CompletionVerdict, ContentHash, ContextManifest, ContextManifestId,
    ContextSelection, CorrelationId, DeterministicCriterionEvidence, DeterministicVerifyInput,
    InvocationId, InvocationManifest, InvocationRole, ModelRunId, PROTOCOL_V1,
    RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN, RUNTIME_WORKER_ACK_MESSAGE_TYPE,
    RUNTIME_WORKER_HELLO_MESSAGE_TYPE, RUNTIME_WORKER_INVOKE_MESSAGE_TYPE,
    RUNTIME_WORKER_RESULT_MESSAGE_TYPE, RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE, RequestId,
    RpcEnvelope, RuntimeInstanceId, RuntimeInvocationRequest, RuntimeInvocationResult,
    RuntimeWorkerControl, RuntimeWorkerHello, SourceRef, StableTypeId, TaskContractId,
    VersionedPayload, VersionedRef, canonical_contract_hash, decode_canonical_cbor_v1,
};
use mcg_platform::{
    LocalIpcStream, RuntimeWorkerListener, RuntimeWorkerSession, create_runtime_worker_session,
    read_canonical_frame, write_canonical_frame,
};
use mcg_runtime_worker::executable_hash;
use tempfile::TempDir;
use tokio::{process::Child, time::timeout};

struct Harness {
    _directory: TempDir,
    child: Child,
    session: RuntimeWorkerSession,
    stream: LocalIpcStream,
    hello: RuntimeWorkerHello,
    root_instance: RuntimeInstanceId,
}

async fn launch() -> Harness {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let session = create_runtime_worker_session(
        directory.path(),
        1,
        executable_hash(executable).unwrap(),
        Utc::now() + Duration::seconds(30),
    )
    .unwrap();
    let mut listener = RuntimeWorkerListener::bind(&session.endpoint).unwrap();
    let mut child = tokio::process::Command::new(executable)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    write_canonical_frame(&mut stdin, &session).await.unwrap();
    drop(stdin);
    let mut stream = timeout(std::time::Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let hello_request: AuthenticatedRpcRequest = timeout(
        std::time::Duration::from_secs(5),
        read_canonical_frame(&mut stream),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        hello_request.envelope.message_type.as_str(),
        RUNTIME_WORKER_HELLO_MESSAGE_TYPE
    );
    assert!(session.verifies(&hello_request.session_capability));
    assert_eq!(hello_request.envelope.session_id, session.session_id);
    let hello: RuntimeWorkerHello = serde_json::from_value(
        decode_canonical_cbor_v1(&hello_request.envelope.payload.bytes).unwrap(),
    )
    .unwrap();
    hello.validate().unwrap();
    assert_eq!(hello.process_id, child.id().unwrap());
    assert_eq!(hello.worker_generation, session.worker_generation);
    assert_eq!(hello.executable_hash, session.expected_executable_hash);
    Harness {
        _directory: directory,
        child,
        session,
        stream,
        hello,
        root_instance: RuntimeInstanceId::new(),
    }
}

fn hash(label: &str) -> ContentHash {
    VersionedPayload::canonical_v1(
        StableTypeId::parse("test.runtime_ref.v1").unwrap(),
        &serde_json::json!({"label": label}),
    )
    .unwrap()
    .content_hash("TestRuntimeRef")
    .unwrap()
}

fn source(label: &str) -> SourceRef {
    SourceRef {
        source_type: StableTypeId::parse("test.runtime_source.v1").unwrap(),
        locator: format!("artifact://{label}"),
        content_hash: hash(label),
    }
}

fn context(attempt_id: mcg_contracts::AttemptId, label: &str) -> ContextManifest {
    ContextManifest {
        id: ContextManifestId::new(),
        attempt_id,
        selected_sources: vec![ContextSelection {
            source: source(label),
            perspective: Some(source("owner")),
            influence: VersionedPayload::canonical_v1(
                StableTypeId::parse("test.influence.v1").unwrap(),
                &serde_json::json!({"trust": "root-authorized"}),
            )
            .unwrap(),
            transformations: Vec::new(),
            placement: 0,
            approximate_tokens: 4,
        }],
        selection_policy: VersionedPayload::canonical_v1(
            StableTypeId::parse("test.context_policy.v1").unwrap(),
            &serde_json::json!({"mode": "deterministic"}),
        )
        .unwrap(),
        expansion_steps: Vec::new(),
        approximate_tokens: 4,
    }
}

fn invocation_request(
    harness: &Harness,
    role: InvocationRole,
    input: VersionedPayload,
) -> RuntimeInvocationRequest {
    let attempt_id = mcg_contracts::AttemptId::new();
    let context = context(
        attempt_id,
        match role {
            InvocationRole::Work => "work-input",
            InvocationRole::Verify => "work-output",
        },
    );
    let context_hash = canonical_contract_hash(
        ContextManifest::TYPE_ID,
        "MetaCog.ContextManifest",
        &context,
    )
    .unwrap();
    let request_hash = input
        .content_hash(RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN)
        .unwrap();
    let invocation = InvocationManifest {
        id: InvocationId::new(),
        role,
        attempt_id,
        task_contract: VersionedRef {
            id: TaskContractId::new(),
            version: 1,
            content_hash: hash("task-contract"),
        },
        runtime_identity: harness.hello.runtime_identity.clone(),
        cognitive_configuration: hash("configuration"),
        role_manifest: hash(match role {
            InvocationRole::Work => "work-role",
            InvocationRole::Verify => "verify-role",
        }),
        prompt_bundle: hash("prompt"),
        context_manifest: context_hash,
        model_route: VersionedPayload::canonical_v1(
            StableTypeId::parse("test.model_route.v1").unwrap(),
            &serde_json::json!({"provider": "deterministic-fake"}),
        )
        .unwrap(),
        resolved_provider: source("fake-provider"),
        resolved_model: source("fake-model"),
        observed_model_fingerprint: Some("fake-v1".to_owned()),
        sampling: VersionedPayload::canonical_v1(
            StableTypeId::parse("test.sampling.v1").unwrap(),
            &serde_json::json!({"temperature": 0}),
        )
        .unwrap(),
        reasoning: None,
        available_tool_manifests: Vec::new(),
        selected_skill_manifests: Vec::new(),
        authority_envelope: source("authority"),
        egress_permit: None,
        repository_revision: Some(hash("revision")),
        environment: source("environment"),
        runtime_build: harness.hello.runtime_identity.build_hash.clone(),
        request_hash,
        response_hash: None,
    };
    RuntimeInvocationRequest {
        worker_generation: harness.session.worker_generation,
        attempt_revision: 1,
        lease_generation: 0,
        context_manifest: context,
        invocation_manifest: invocation,
        model_run_id: ModelRunId::new(),
        input,
    }
}

fn root_request<T: serde::Serialize>(
    harness: &Harness,
    message_type: &str,
    value: &T,
) -> AuthenticatedRpcRequest {
    let message_type = StableTypeId::parse(message_type).unwrap();
    AuthenticatedRpcRequest {
        envelope: RpcEnvelope {
            protocol_version: PROTOCOL_V1,
            request_id: RequestId::new(),
            correlation_id: CorrelationId::new(),
            session_id: harness.session.session_id,
            caller_instance: harness.root_instance,
            message_type: message_type.clone(),
            expires_at: Some(Utc::now() + Duration::seconds(5)),
            payload: VersionedPayload::canonical_v1(
                message_type,
                &serde_json::to_value(value).unwrap(),
            )
            .unwrap(),
        },
        session_capability: harness.session.capability.clone(),
    }
}

async fn exchange<T: serde::Serialize>(
    harness: &mut Harness,
    message_type: &str,
    response_type: &str,
    value: &T,
) -> RpcEnvelope {
    let request = root_request(harness, message_type, value);
    write_canonical_frame(&mut harness.stream, &request)
        .await
        .unwrap();
    let response: RpcEnvelope = timeout(
        std::time::Duration::from_secs(5),
        read_canonical_frame(&mut harness.stream),
    )
    .await
    .unwrap()
    .unwrap();
    response.validate().unwrap();
    assert_eq!(response.request_id, request.envelope.request_id);
    assert_eq!(response.correlation_id, request.envelope.correlation_id);
    assert_eq!(response.session_id, harness.session.session_id);
    assert_eq!(
        response.caller_instance,
        harness.hello.runtime_identity.instance_id
    );
    assert_eq!(response.message_type.as_str(), response_type);
    response
}

fn response_value<T: serde::de::DeserializeOwned>(response: &RpcEnvelope) -> T {
    serde_json::from_value(decode_canonical_cbor_v1(&response.payload.bytes).unwrap()).unwrap()
}

#[tokio::test]
async fn separate_worker_authenticates_and_runs_distinct_work_and_verify() {
    let mut harness = launch().await;
    let work_input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"objective": "deterministic result"}),
    )
    .unwrap();
    let work_request = invocation_request(&harness, InvocationRole::Work, work_input);
    let response = exchange(
        &mut harness,
        RUNTIME_WORKER_INVOKE_MESSAGE_TYPE,
        RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
        &work_request,
    )
    .await;
    let work: RuntimeInvocationResult = response_value(&response);
    work.validate().unwrap();
    assert_eq!(work.role, InvocationRole::Work);
    assert!(work.verdict.is_none());

    let verify_input = DeterministicVerifyInput {
        work_response_hash: work.response_hash.clone(),
        criteria: vec![DeterministicCriterionEvidence {
            clause_id: "criterion-1".to_owned(),
            passed: true,
            evidence_refs: vec![source("independent-test")],
        }],
    };
    let verify_payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(DeterministicVerifyInput::TYPE_ID).unwrap(),
        &serde_json::to_value(verify_input).unwrap(),
    )
    .unwrap();
    let verify_request = invocation_request(&harness, InvocationRole::Verify, verify_payload);
    let response = exchange(
        &mut harness,
        RUNTIME_WORKER_INVOKE_MESSAGE_TYPE,
        RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
        &verify_request,
    )
    .await;
    let verify: RuntimeInvocationResult = response_value(&response);
    verify.validate().unwrap();
    assert_eq!(verify.role, InvocationRole::Verify);
    assert_eq!(verify.verdict, Some(CompletionVerdict::Satisfied));
    assert_eq!(verify.criterion_results.len(), 1);

    let control = RuntimeWorkerControl {
        worker_generation: harness.session.worker_generation,
    };
    exchange(
        &mut harness,
        RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE,
        RUNTIME_WORKER_ACK_MESSAGE_TYPE,
        &control,
    )
    .await;
    assert!(
        timeout(std::time::Duration::from_secs(5), harness.child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn wrong_capability_is_rejected_without_execution() {
    let mut harness = launch().await;
    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"objective": "must not execute"}),
    )
    .unwrap();
    let invocation = invocation_request(&harness, InvocationRole::Work, input);
    let mut request = root_request(&harness, RUNTIME_WORKER_INVOKE_MESSAGE_TYPE, &invocation);
    request.session_capability = "0".repeat(64);
    write_canonical_frame(&mut harness.stream, &request)
        .await
        .unwrap();
    assert!(
        !timeout(std::time::Duration::from_secs(5), harness.child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn executable_substitution_fails_before_connecting() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut wrong_hash = executable_hash(executable).unwrap();
    wrong_hash.digest_hex = "0".repeat(64);
    let session = create_runtime_worker_session(
        directory.path(),
        1,
        wrong_hash,
        Utc::now() + Duration::seconds(5),
    )
    .unwrap();
    let _listener = RuntimeWorkerListener::bind(&session.endpoint).unwrap();
    let mut child = tokio::process::Command::new(executable)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    write_canonical_frame(&mut stdin, &session).await.unwrap();
    drop(stdin);
    assert!(
        !timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[test]
fn runtime_worker_dependency_tree_excludes_protected_implementations() {
    let metadata = std::process::Command::new(env!("CARGO"))
        .args(["tree", "-p", "mcg-runtime-worker", "--prefix", "none"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let tree = String::from_utf8(metadata.stdout).unwrap();
    for forbidden in [
        "mcg-storage",
        "mcg-protected-root",
        "mcg-runtime ",
        "mcg-providers",
        "mcg-tools",
        "rusqlite",
        "reqwest",
    ] {
        assert!(
            !tree.contains(forbidden),
            "forbidden dependency: {forbidden}"
        );
    }
}
