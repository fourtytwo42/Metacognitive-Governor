use std::process::Stdio;

use chrono::{Duration, Utc};
use mcg_config::AppConfig;
use mcg_contracts::{
    AuthenticatedRpcRequest, CompletionCertificate, CompletionCertificateId, CompletionVerdict,
    ConfigurationId, ContentHash, ContextManifest, ContextManifestId, ContextSelection,
    ContractClause, CorrelationId, DeterministicCriterionEvidence, DeterministicVerifyInput,
    HashAlgorithm, InvocationId, InvocationManifest, InvocationRole, ModelRunId, PROTOCOL_V1,
    PolicyBinding, RUNTIME_INVOCATION_REQUEST_HASH_DOMAIN, RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN,
    RUNTIME_WORKER_ACK_MESSAGE_TYPE, RUNTIME_WORKER_HELLO_MESSAGE_TYPE,
    RUNTIME_WORKER_INVOKE_MESSAGE_TYPE, RUNTIME_WORKER_RESULT_MESSAGE_TYPE,
    RUNTIME_WORKER_SHUTDOWN_MESSAGE_TYPE, RequestId, RetryPolicy, RpcEnvelope, RuntimeActorId,
    RuntimeInstanceId, RuntimeInvocationRequest, RuntimeInvocationResult, RuntimeWorkerControl,
    RuntimeWorkerHello, SourceRef, StableTypeId, TaskContract, TaskContractId, TaskContractStatus,
    VersionedPayload, VersionedRef, WorkflowAttempt, WorkflowCancelRequest, WorkflowId,
    WorkflowState, canonical_contract_hash, decode_canonical_cbor_v1,
};
use mcg_platform::{
    LocalIpcStream, RuntimeWorkerListener, RuntimeWorkerSession, create_runtime_worker_session,
    read_canonical_frame, runtime_worker_executable_hash, write_canonical_frame,
};
use mcg_protected_root::{
    RuntimeInvocationExecutionReceipt, RuntimeWorkerClient, RuntimeWorkerReplacementReason,
    execute_runtime_invocation, execute_runtime_invocation_with_cancellation,
    launch_runtime_worker, open as open_protected_root, replace_idle_runtime_worker_if_needed,
};
use mcg_runtime::AppRuntime;
use mcg_storage::{SqliteEventStore, task_contract_content_hash};
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
        runtime_worker_executable_hash(executable).unwrap(),
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

fn policy(label: &str) -> PolicyBinding {
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.runtime_policy.v1").unwrap(),
        &serde_json::json!({"label": label}),
    )
    .unwrap();
    PolicyBinding {
        content_hash: payload.content_hash("TestRuntimePolicy").unwrap(),
        payload,
    }
}

fn workflow_fixture(now: chrono::DateTime<Utc>) -> (TaskContract, WorkflowAttempt) {
    let task_id = TaskContractId::new();
    let clause = ContractClause {
        clause_id: "criterion-1".into(),
        statement: "deterministic Runtime result is retained".into(),
        source_refs: vec![source("criterion")],
    };
    let contract = TaskContract {
        id: task_id,
        version: 1,
        previous_version: None,
        user_intent_ref: source("intent"),
        objective: "execute one Root-supervised deterministic invocation".into(),
        non_goals: Vec::new(),
        requirements: vec![clause.clone()],
        constraints: Vec::new(),
        assumptions: Vec::new(),
        acceptance_criteria: vec![clause],
        dependencies: Vec::new(),
        permitted_side_effects: policy("none"),
        authority: policy("root"),
        environment: policy("isolated"),
        risk: policy("low"),
        budgets: policy("bounded"),
        rollback: policy("none"),
        status: TaskContractStatus::Active,
        created_at: now,
    };
    let mut attempt = WorkflowAttempt {
        workflow_id: WorkflowId::new(),
        attempt_id: mcg_contracts::AttemptId::new(),
        task_contract: VersionedRef {
            id: task_id,
            version: 1,
            content_hash: ContentHash {
                algorithm: HashAlgorithm::Blake3_256,
                domain: "placeholder".into(),
                digest_hex: "0".repeat(64),
            },
        },
        parent_attempt_id: None,
        attempt_number: 1,
        state: WorkflowState::Ready,
        revision: 1,
        lease_owner: None,
        lease_generation: 0,
        lease_expires_at: None,
        heartbeat_at: None,
        checkpoint: None,
        environment_snapshot: hash("environment"),
        configuration: VersionedRef {
            id: ConfigurationId::new(),
            version: 1,
            content_hash: hash("configuration"),
        },
        idempotency_key: "runtime-workflow-create".into(),
        retry_policy: RetryPolicy {
            max_attempts: 2,
            initial_backoff_ms: 100,
            maximum_backoff_ms: 1_000,
        },
        compensation_policy: policy("none"),
        created_at: now,
        updated_at: now,
    };
    attempt.task_contract.content_hash = task_contract_content_hash(&contract).unwrap();
    (contract, attempt)
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
    hello: &RuntimeWorkerHello,
    worker_generation: u64,
    role: InvocationRole,
    input: VersionedPayload,
) -> RuntimeInvocationRequest {
    invocation_request_for(
        hello,
        worker_generation,
        role,
        input,
        mcg_contracts::AttemptId::new(),
        VersionedRef {
            id: TaskContractId::new(),
            version: 1,
            content_hash: hash("task-contract"),
        },
        1,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn invocation_request_for(
    hello: &RuntimeWorkerHello,
    worker_generation: u64,
    role: InvocationRole,
    input: VersionedPayload,
    attempt_id: mcg_contracts::AttemptId,
    task_contract: VersionedRef<TaskContractId>,
    attempt_revision: u64,
    lease_generation: u64,
) -> RuntimeInvocationRequest {
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
        task_contract,
        runtime_identity: hello.runtime_identity.clone(),
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
        runtime_build: hello.runtime_identity.build_hash.clone(),
        request_hash,
        response_hash: None,
    };
    RuntimeInvocationRequest {
        worker_generation,
        attempt_revision,
        lease_generation,
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
    let work_request = invocation_request(
        &harness.hello,
        harness.session.worker_generation,
        InvocationRole::Work,
        work_input,
    );
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
    let verify_request = invocation_request(
        &harness.hello,
        harness.session.worker_generation,
        InvocationRole::Verify,
        verify_payload,
    );
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
    let invocation = invocation_request(
        &harness.hello,
        harness.session.worker_generation,
        InvocationRole::Work,
        input,
    );
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
async fn duplicate_authenticated_request_is_rejected_as_replay() {
    let mut harness = launch().await;
    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"objective": "execute only once"}),
    )
    .unwrap();
    let invocation = invocation_request(
        &harness.hello,
        harness.session.worker_generation,
        InvocationRole::Work,
        input,
    );
    let request = root_request(&harness, RUNTIME_WORKER_INVOKE_MESSAGE_TYPE, &invocation);
    write_canonical_frame(&mut harness.stream, &request)
        .await
        .unwrap();
    let _: RpcEnvelope = read_canonical_frame(&mut harness.stream).await.unwrap();
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
    let mut wrong_hash = runtime_worker_executable_hash(executable).unwrap();
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

#[tokio::test]
async fn production_root_client_authenticates_invokes_and_shuts_down_exact_child() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut client = RuntimeWorkerClient::launch(
        executable,
        directory.path(),
        17,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert_eq!(client.hello().worker_generation, 17);
    assert_eq!(
        client.hello().executable_hash.domain,
        "MetaCog.RuntimeWorkerExecutable"
    );

    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "root-supervised"}),
    )
    .unwrap();
    let request = invocation_request(client.hello(), 17, InvocationRole::Work, input);
    let mut stale = request.clone();
    stale.worker_generation = 18;
    assert!(client.invoke(&stale).await.is_err());

    let result = client.invoke(&request).await.unwrap();
    assert_eq!(result.worker_generation, 17);
    assert_eq!(result.invocation_id, request.invocation_manifest.id);
    assert_eq!(result.model_run_id, request.model_run_id);
    assert_eq!(result.role, InvocationRole::Work);
    assert!(result.verdict.is_none());
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn production_root_client_enforces_one_bounded_request_deadline() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut client = RuntimeWorkerClient::launch(
        executable,
        directory.path(),
        19,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_nanos(1),
        std::time::Duration::from_secs(30),
    )
    .await
    .unwrap();
    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "deadline"}),
    )
    .unwrap();
    let request = invocation_request(client.hello(), 19, InvocationRole::Work, input);
    assert!(client.invoke(&request).await.is_err());
    assert!(matches!(
        client.invoke(&request).await,
        Err(mcg_protected_root::RuntimeWorkerClientError::Unavailable)
    ));
    client.terminate().await.unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One complete Work/Verify/certificate/restart proof.
async fn protected_root_commits_real_work_verify_completion_certificate() {
    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig {
        data_directory: directory.path().join("completion-profile"),
        ..AppConfig::default()
    };
    let restart_config = config.clone();
    std::fs::create_dir_all(&config.data_directory).unwrap();
    let now = Utc::now();
    let (contract, ready) = workflow_fixture(now);
    let running = {
        let mut store = SqliteEventStore::open(config.database_path()).unwrap();
        store
            .create_workflow(&contract, &ready, CorrelationId::new(), now)
            .unwrap();
        store
            .claim_workflow_attempt(
                ready.attempt_id,
                ready.revision,
                ready.lease_generation,
                RuntimeActorId::new(),
                now + Duration::minutes(5),
                CorrelationId::new(),
                "completion-workflow-claim",
                now,
            )
            .unwrap()
    };
    let runtime = AppRuntime::open(config).unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut client = launch_runtime_worker(&runtime, executable, directory.path())
        .await
        .unwrap();
    assert_eq!(client.worker_generation(), 1);

    let work_input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "work-then-independent-verify"}),
    )
    .unwrap();
    let work_request = invocation_request_for(
        client.hello(),
        client.worker_generation(),
        InvocationRole::Work,
        work_input,
        running.attempt_id,
        running.task_contract.clone(),
        running.revision,
        running.lease_generation,
    );
    let work = execute_runtime_invocation(&runtime, &mut client, &work_request, Utc::now())
        .await
        .unwrap();
    let verifying = runtime
        .begin_runtime_verification(
            running.attempt_id,
            running.revision,
            running.lease_generation,
            "begin-independent-verification",
            Utc::now(),
        )
        .unwrap();
    let verify_input = DeterministicVerifyInput {
        work_response_hash: work.result.response_hash.clone(),
        criteria: vec![DeterministicCriterionEvidence {
            clause_id: "criterion-1".into(),
            passed: true,
            evidence_refs: vec![source("work-output-evidence")],
        }],
    };
    let verify_payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(DeterministicVerifyInput::TYPE_ID).unwrap(),
        &serde_json::to_value(&verify_input).unwrap(),
    )
    .unwrap();
    let verify_request = invocation_request_for(
        client.hello(),
        client.worker_generation(),
        InvocationRole::Verify,
        verify_payload,
        verifying.attempt_id,
        verifying.task_contract.clone(),
        verifying.revision,
        verifying.lease_generation,
    );
    let verify = execute_runtime_invocation(&runtime, &mut client, &verify_request, Utc::now())
        .await
        .unwrap();
    assert_eq!(verify.result.verdict, Some(CompletionVerdict::Satisfied));
    let certificate = CompletionCertificate {
        id: CompletionCertificateId::new(),
        workflow_id: verifying.workflow_id,
        attempt_id: verifying.attempt_id,
        task_contract: verifying.task_contract.clone(),
        work_invocations: vec![work_request.invocation_manifest.id],
        verify_invocations: vec![verify_request.invocation_manifest.id],
        criterion_results: verify.result.criterion_results.clone(),
        evidence_refs: vec![source("completion-evidence")],
        tests_run: vec![source("deterministic-runtime-e2e")],
        artifacts: vec![
            work.completed_run.run.response_artifact.clone().unwrap(),
            verify.completed_run.run.response_artifact.clone().unwrap(),
        ],
        known_untested_paths: Vec::new(),
        unresolved_assumptions: Vec::new(),
        remaining_risks: Vec::new(),
        verdict: verify.result.verdict.unwrap(),
        issued_at: Utc::now(),
    };
    let (_, settled) = runtime
        .commit_runtime_completion(
            &certificate,
            verifying.revision,
            verifying.lease_generation,
            Utc::now(),
        )
        .unwrap();
    assert_eq!(settled.state, WorkflowState::Succeeded);
    client.shutdown().await.unwrap();
    drop(runtime);

    let restarted = AppRuntime::open(restart_config).unwrap();
    assert_eq!(
        restarted
            .canonical_workflow(settled.workflow_id)
            .unwrap()
            .unwrap()
            .attempt
            .state,
        WorkflowState::Succeeded
    );
    assert_eq!(
        restarted
            .runtime_invocation(verify_request.invocation_manifest.id, 10)
            .unwrap()
            .unwrap()
            .model_runs
            .last()
            .unwrap()
            .run
            .state,
        mcg_contracts::ModelRunState::Completed
    );
}

#[tokio::test]
async fn protected_root_replaces_dead_and_expired_idle_generations() {
    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig {
        data_directory: directory.path().join("replacement-profile"),
        ..AppConfig::default()
    };
    std::fs::create_dir_all(&config.data_directory).unwrap();
    let runtime = AppRuntime::open(config).unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut worker = Some(
        launch_runtime_worker(&runtime, executable, directory.path())
            .await
            .unwrap(),
    );
    let stale_hello = worker.as_ref().unwrap().hello().clone();
    assert!(
        replace_idle_runtime_worker_if_needed(
            &runtime,
            executable,
            directory.path(),
            &mut worker,
            Utc::now(),
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(worker.as_ref().unwrap().worker_generation(), 1);
    worker.as_mut().unwrap().terminate().await.unwrap();

    let dead_replacement = replace_idle_runtime_worker_if_needed(
        &runtime,
        executable,
        directory.path(),
        &mut worker,
        Utc::now(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(dead_replacement.previous_generation, 1);
    assert_eq!(dead_replacement.replacement_generation, 2);
    assert_eq!(
        dead_replacement.reason,
        RuntimeWorkerReplacementReason::Unusable
    );
    assert_eq!(worker.as_ref().unwrap().worker_generation(), 2);

    let stale_input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "stale-generation-must-not-run"}),
    )
    .unwrap();
    let stale_request = invocation_request(&stale_hello, 1, InvocationRole::Work, stale_input);
    assert!(
        execute_runtime_invocation(
            &runtime,
            worker.as_mut().unwrap(),
            &stale_request,
            Utc::now(),
        )
        .await
        .is_err()
    );
    assert!(
        runtime
            .runtime_invocation(stale_request.invocation_manifest.id, 10)
            .unwrap()
            .is_none()
    );

    worker.take().unwrap().shutdown().await.unwrap();
    let expiring_generation = runtime.allocate_runtime_worker_generation().unwrap();
    assert_eq!(expiring_generation, 3);
    worker = Some(
        RuntimeWorkerClient::launch(
            executable,
            directory.path(),
            expiring_generation,
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap(),
    );
    let expired_replacement = replace_idle_runtime_worker_if_needed(
        &runtime,
        executable,
        directory.path(),
        &mut worker,
        Utc::now() + Duration::seconds(2),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(expired_replacement.previous_generation, 3);
    assert_eq!(expired_replacement.replacement_generation, 4);
    assert_eq!(
        expired_replacement.reason,
        RuntimeWorkerReplacementReason::SessionExpired
    );
    assert_eq!(worker.as_ref().unwrap().worker_generation(), 4);
    worker.take().unwrap().shutdown().await.unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One real-process cancel/kill/late/reopen proof.
async fn active_invocation_cancellation_kills_generation_and_rejects_late_result() {
    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig {
        data_directory: directory.path().join("active-cancellation-profile"),
        ..AppConfig::default()
    };
    let restart_config = config.clone();
    std::fs::create_dir_all(&config.data_directory).unwrap();
    let now = Utc::now();
    let (contract, ready) = workflow_fixture(now);
    let running = {
        let mut store = SqliteEventStore::open(config.database_path()).unwrap();
        store
            .create_workflow(&contract, &ready, CorrelationId::new(), now)
            .unwrap();
        store
            .claim_workflow_attempt(
                ready.attempt_id,
                ready.revision,
                ready.lease_generation,
                RuntimeActorId::new(),
                now + Duration::minutes(5),
                CorrelationId::new(),
                "active-cancellation-claim",
                now,
            )
            .unwrap()
    };
    let runtime = AppRuntime::open(config).unwrap();
    let generation = runtime.allocate_runtime_worker_generation().unwrap();
    let fixture = env!("CARGO_BIN_EXE_mcg-runtime-worker-delayed-fixture");
    let mut client = RuntimeWorkerClient::launch(
        fixture,
        directory.path(),
        generation,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(35),
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "cancel-after-dispatch"}),
    )
    .unwrap();
    let request = invocation_request_for(
        client.hello(),
        client.worker_generation(),
        InvocationRole::Work,
        input,
        running.attempt_id,
        running.task_contract.clone(),
        running.revision,
        running.lease_generation,
    );
    let cancel = WorkflowCancelRequest {
        attempt_id: running.attempt_id,
        expected_revision: running.revision,
        expected_generation: running.lease_generation,
        idempotency_key: "cancel-active-runtime-invocation".into(),
    };
    let started = std::time::Instant::now();
    let receipt = execute_runtime_invocation_with_cancellation(
        &runtime,
        &mut client,
        &request,
        Utc::now(),
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            cancel
        },
    )
    .await
    .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let RuntimeInvocationExecutionReceipt::Cancelled(cancelled) = receipt else {
        panic!("delayed Runtime invocation completed instead of cancelling");
    };
    assert_eq!(cancelled.worker_generation, generation);
    assert_eq!(cancelled.workflow.state, WorkflowState::CancelRequested);
    assert_eq!(cancelled.workflow.revision, running.revision + 1);
    assert_eq!(
        cancelled.workflow.lease_generation,
        running.lease_generation + 1
    );
    assert_eq!(
        cancelled.model_run.run.state,
        mcg_contracts::ModelRunState::Cancelled
    );
    assert!(matches!(
        client.invoke(&request).await,
        Err(mcg_protected_root::RuntimeWorkerClientError::Unavailable)
    ));
    assert!(
        runtime
            .runtime_response(request.model_run_id)
            .unwrap()
            .is_none()
    );

    let response = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.late_runtime_response.v1").unwrap(),
        &serde_json::json!({"outcome": "must-not-commit"}),
    )
    .unwrap();
    let response_hash = response
        .content_hash(RUNTIME_INVOCATION_RESPONSE_HASH_DOMAIN)
        .unwrap();
    let late_result = RuntimeInvocationResult {
        worker_generation: generation,
        invocation_id: request.invocation_manifest.id,
        model_run_id: request.model_run_id,
        role: request.invocation_manifest.role,
        request_hash: request.invocation_manifest.request_hash.clone(),
        response,
        response_hash,
        criterion_results: Vec::new(),
        verdict: None,
    };
    late_result.validate().unwrap();
    assert!(
        runtime
            .complete_runtime_invocation(&request, &late_result, Utc::now())
            .is_err()
    );
    assert!(
        runtime
            .runtime_response(request.model_run_id)
            .unwrap()
            .is_none()
    );
    drop(client);
    drop(runtime);

    let reopened = open_protected_root(restart_config).unwrap();
    assert_eq!(
        reopened
            .canonical_workflow(running.workflow_id)
            .unwrap()
            .unwrap()
            .attempt
            .state,
        WorkflowState::Cancelled
    );
    let inspection = reopened
        .runtime_invocation(request.invocation_manifest.id, 10)
        .unwrap()
        .unwrap();
    assert_eq!(
        inspection
            .model_runs
            .iter()
            .map(|record| record.run.state)
            .collect::<Vec<_>>(),
        [
            mcg_contracts::ModelRunState::Planned,
            mcg_contracts::ModelRunState::Dispatched,
            mcg_contracts::ModelRunState::Cancelled,
        ]
    );
    assert!(
        reopened
            .runtime_response(request.model_run_id)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One ordered process/restart scenario retains causal evidence.
async fn protected_root_persists_real_worker_plan_dispatch_result_and_response() {
    let directory = tempfile::tempdir().unwrap();
    let config = AppConfig {
        data_directory: directory.path().join("profile"),
        ..AppConfig::default()
    };
    let restart_config = config.clone();
    std::fs::create_dir_all(&config.data_directory).unwrap();
    let now = Utc::now();
    let (contract, ready) = workflow_fixture(now);
    let running = {
        let mut store = SqliteEventStore::open(config.database_path()).unwrap();
        store
            .create_workflow(&contract, &ready, CorrelationId::new(), now)
            .unwrap();
        store
            .claim_workflow_attempt(
                ready.attempt_id,
                ready.revision,
                ready.lease_generation,
                RuntimeActorId::new(),
                now + Duration::minutes(5),
                CorrelationId::new(),
                "runtime-workflow-claim",
                now,
            )
            .unwrap()
    };
    let runtime = AppRuntime::open(config).unwrap();
    let executable = env!("CARGO_BIN_EXE_mcg-runtime-worker");
    let mut client = RuntimeWorkerClient::launch(
        executable,
        directory.path(),
        23,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(30),
    )
    .await
    .unwrap();
    let input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "persist-every-boundary"}),
    )
    .unwrap();
    let request = invocation_request_for(
        client.hello(),
        client.worker_generation(),
        InvocationRole::Work,
        input,
        running.attempt_id,
        running.task_contract.clone(),
        running.revision,
        running.lease_generation,
    );
    let receipt = execute_runtime_invocation(&runtime, &mut client, &request, Utc::now())
        .await
        .unwrap();
    assert_eq!(
        receipt.completed_run.run.state,
        mcg_contracts::ModelRunState::Completed
    );
    assert_eq!(
        receipt.response_artifact.response_hash,
        receipt.result.response_hash
    );
    let inspection = runtime
        .runtime_invocation(request.invocation_manifest.id, 10)
        .unwrap()
        .unwrap();
    assert_eq!(
        inspection
            .model_runs
            .iter()
            .map(|record| record.run.state)
            .collect::<Vec<_>>(),
        [
            mcg_contracts::ModelRunState::Planned,
            mcg_contracts::ModelRunState::Dispatched,
            mcg_contracts::ModelRunState::Completed,
        ]
    );
    assert_eq!(
        runtime.runtime_response(request.model_run_id).unwrap(),
        Some(receipt.response_artifact.clone())
    );

    let death_input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "worker-dies-before-dispatch-channel"}),
    )
    .unwrap();
    let death_request = invocation_request_for(
        client.hello(),
        client.worker_generation(),
        InvocationRole::Work,
        death_input,
        running.attempt_id,
        running.task_contract.clone(),
        running.revision,
        running.lease_generation,
    );
    client.terminate().await.unwrap();
    assert!(
        execute_runtime_invocation(&runtime, &mut client, &death_request, Utc::now())
            .await
            .is_err()
    );
    assert_eq!(
        runtime
            .runtime_invocation(death_request.invocation_manifest.id, 10)
            .unwrap()
            .unwrap()
            .model_runs
            .last()
            .unwrap()
            .run
            .state,
        mcg_contracts::ModelRunState::OutcomeUnknown
    );

    let mut late_client = RuntimeWorkerClient::launch(
        executable,
        directory.path(),
        24,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(30),
    )
    .await
    .unwrap();

    let late_input = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.work_input.v1").unwrap(),
        &serde_json::json!({"task": "must-not-commit-after-cancel"}),
    )
    .unwrap();
    let late_request = invocation_request_for(
        late_client.hello(),
        late_client.worker_generation(),
        InvocationRole::Work,
        late_input,
        running.attempt_id,
        running.task_contract,
        running.revision,
        running.lease_generation,
    );
    runtime
        .plan_runtime_invocation(&late_request, Utc::now())
        .unwrap();
    runtime
        .dispatch_runtime_invocation(&late_request, Utc::now())
        .unwrap();
    runtime
        .cancel_canonical_workflow(&WorkflowCancelRequest {
            attempt_id: running.attempt_id,
            expected_revision: running.revision,
            expected_generation: running.lease_generation,
            idempotency_key: "cancel-before-late-runtime-result".into(),
        })
        .unwrap();
    let late_result = late_client.invoke(&late_request).await.unwrap();
    assert!(
        runtime
            .complete_runtime_invocation(&late_request, &late_result, Utc::now())
            .is_err()
    );
    runtime
        .settle_runtime_invocation_without_result(
            &late_request,
            mcg_contracts::ModelRunState::OutcomeUnknown,
            Utc::now(),
        )
        .unwrap();
    assert_eq!(
        runtime
            .runtime_invocation(late_request.invocation_manifest.id, 10)
            .unwrap()
            .unwrap()
            .model_runs
            .last()
            .unwrap()
            .run
            .state,
        mcg_contracts::ModelRunState::OutcomeUnknown
    );
    late_client.shutdown().await.unwrap();
    drop(runtime);
    let restarted = AppRuntime::open(restart_config).unwrap();
    assert_eq!(
        restarted
            .runtime_invocation(death_request.invocation_manifest.id, 10)
            .unwrap()
            .unwrap()
            .model_runs
            .last()
            .unwrap()
            .run
            .state,
        mcg_contracts::ModelRunState::OutcomeUnknown
    );
    assert_eq!(
        restarted.runtime_response(request.model_run_id).unwrap(),
        Some(receipt.response_artifact)
    );
}

#[test]
fn runtime_worker_dependency_tree_excludes_protected_implementations() {
    let metadata = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "mcg-runtime-worker",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
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
