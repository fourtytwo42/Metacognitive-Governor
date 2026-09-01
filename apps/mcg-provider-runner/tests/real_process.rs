use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use mcg_contracts::{
    AttemptId, ContentHash, HashAlgorithm, InvocationId, InvocationRole, ModelBudget,
    ModelDataClassification, ModelDestinationClass, ModelDisclosureManifest, ModelEgressRoute,
    ModelInvocationPermit, ModelInvocationProposal, PROTOCOL_V1, PermitId, PolicyBinding,
    RuntimeActorId, RuntimeIdentity, RuntimeInstanceId, SourceRef, StableTypeId, TaskContractId,
    VersionedPayload, VersionedRef, canonical_contract_hash,
};
use mcg_provider_runner::{
    PROVIDER_RUNNER_PROTOCOL_VERSION, ProviderRunnerAuthorization, ProviderRunnerClient,
    ProviderRunnerRequest, model_provider_request_artifact,
};
use mcg_providers::{
    CONTRACT_VERSION, ContentPart, Message, MessageRole, ModelCapabilities, ModelDescriptor,
    ModelRequest, ProviderAuthMode, ProviderConnection, ProviderKind,
};
use sha2::{Digest as _, Sha256};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::oneshot,
};
use uuid::Uuid;

fn hash(domain: &str, seed: &str) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    ContentHash {
        algorithm: HashAlgorithm::Sha256,
        domain: domain.into(),
        digest_hex: format!("{:x}", hasher.finalize()),
    }
}

fn source_ref<T: serde::Serialize>(
    source_type: &str,
    locator: &str,
    domain: &str,
    value: &T,
) -> SourceRef {
    SourceRef {
        source_type: StableTypeId::parse(source_type).expect("source type"),
        locator: locator.into(),
        content_hash: canonical_contract_hash(source_type, domain, value).expect("source hash"),
    }
}

#[allow(clippy::too_many_lines)]
fn authorization(
    connection: &ProviderConnection,
    model: &ModelDescriptor,
    request: &ModelRequest,
) -> ProviderRunnerAuthorization {
    let at = Utc::now();
    let artifact = model_provider_request_artifact(request).expect("request artifact");
    let policy_payload = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.real_provider_runner_policy.v1").expect("policy type"),
        &serde_json::json!({"network": "loopback"}),
    )
    .expect("policy payload");
    let proposal = ModelInvocationProposal {
        invocation_id: InvocationId::new(),
        attempt_id: AttemptId::new(),
        attempt_revision: 1,
        lease_generation: 1,
        runtime_actor: RuntimeActorId::new(),
        runtime_identity: RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: hash("Test.RuntimeBuild", "real-process"),
            protocol_version: PROTOCOL_V1,
            started_at: at - ChronoDuration::seconds(1),
        },
        role: InvocationRole::Work,
        task_contract: VersionedRef {
            id: TaskContractId::new(),
            version: 1,
            content_hash: hash("Test.TaskContract", "real-process"),
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
            context_manifest: hash("Test.ContextManifest", "real-process"),
            disclosed_sources: Vec::new(),
            approximate_input_tokens: 3,
        },
        requested_limits: ModelBudget {
            max_input_tokens: 64,
            max_output_tokens: 32,
            max_cost_microusd: 0,
        },
        deadline: at + ChronoDuration::minutes(1),
    };
    let proposal_hash = canonical_contract_hash(
        ModelInvocationProposal::TYPE_ID,
        "MetaCog.ModelInvocationProposal",
        &proposal,
    )
    .expect("proposal hash");
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
        resolved_provider: source_ref(
            "mcg.provider_connection.v1",
            &connection.id,
            "MetaCog.ProviderConnection",
            connection,
        ),
        resolved_model: source_ref(
            "mcg.provider_model.v1",
            &format!("{}/{}", connection.id, model.id),
            "MetaCog.ProviderModel",
            model,
        ),
        destination: source_ref(
            "mcg.provider_destination.v1",
            &connection.base_url,
            "MetaCog.ProviderDestination",
            &connection.base_url,
        ),
        destination_class: proposal.route.destination_class,
        allowed_data_classification: proposal.data_classification,
        disclosure_hash: canonical_contract_hash(
            ModelDisclosureManifest::TYPE_ID,
            "MetaCog.ModelDisclosureManifest",
            &proposal.disclosure,
        )
        .expect("disclosure hash"),
        limits: proposal.requested_limits,
        policy: PolicyBinding {
            content_hash: policy_payload
                .content_hash("Test.RealProviderRunnerPolicy")
                .expect("policy hash"),
            payload: policy_payload,
        },
        expires_at: proposal.deadline,
        secret_handles: Vec::new(),
        issued_at: at,
    };
    let permit_hash = canonical_contract_hash(
        ModelInvocationPermit::TYPE_ID,
        "MetaCog.ModelInvocationPermit",
        &permit,
    )
    .expect("permit hash");
    ProviderRunnerAuthorization {
        proposal,
        proposal_hash,
        permit_ref: SourceRef {
            source_type: StableTypeId::parse("mcg.model_invocation_permit.v2")
                .expect("permit type"),
            locator: permit.id.to_string(),
            content_hash: permit_hash.clone(),
        },
        permit,
        permit_hash,
        secret_handles: Vec::new(),
    }
}

#[tokio::test]
async fn fixed_child_executes_one_real_loopback_provider_request() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("provider listener");
    let address = listener.local_addr().expect("provider address");
    let provider = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("provider connection");
        let mut request = vec![0_u8; 64 * 1024];
        let read = stream.read(&mut request).await.expect("provider request");
        request.truncate(read);
        let body = serde_json::json!({
            "id": "real-child-response",
            "model": "real-child-model",
            "status": "completed",
            "output": [{"type":"message","role":"assistant","content":[{"type":"output_text","text":"child boundary"}]}],
            "usage": {"input_tokens":3,"output_tokens":2,"total_tokens":5}
        })
        .to_string();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("provider response");
        String::from_utf8(request).expect("provider request text")
    });
    let connection = ProviderConnection {
        contract_version: CONTRACT_VERSION,
        id: "real-child-provider".into(),
        display_name: "Real Child Provider".into(),
        kind: ProviderKind::OpenAiCompatible,
        base_url: format!("http://{address}/v1"),
        auth_mode: ProviderAuthMode::None,
        organization: None,
        project: None,
    };
    let model = ModelDescriptor {
        id: "real-child-model".into(),
        display_name: "Real Child Model".into(),
        owned_by: Some("fixture".into()),
        context_window: Some(1_024),
        capabilities: ModelCapabilities {
            responses: Some(true),
            ..ModelCapabilities::default()
        },
        price: None,
        metadata: serde_json::Value::Null,
    };
    let model_request = ModelRequest {
        contract_version: CONTRACT_VERSION,
        model: model.id.clone(),
        client_request_id: Some("real-child-client-request".into()),
        instructions: None,
        messages: vec![Message {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "execute in child".into(),
            }],
            name: None,
        }],
        tools: Vec::new(),
        tool_choice: None,
        structured_output: None,
        reasoning: None,
        temperature: Some(0.0),
        top_p: None,
        max_output_tokens: Some(32),
        metadata: serde_json::Value::Null,
    };
    let request = ProviderRunnerRequest {
        protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
        request_id: Uuid::new_v4(),
        authorization: authorization(&connection, &model, &model_request),
        connection,
        model,
        request: model_request,
    };
    let client = ProviderRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-provider-runner"),
        Duration::from_secs(10),
    )
    .expect("provider runner client");
    let response = client
        .invoke(&request)
        .await
        .expect("child provider response");
    assert_eq!(response.id, "real-child-response");
    assert_eq!(response.usage.total_tokens, 5);
    let observed = provider.await.expect("provider fixture");
    assert!(observed.contains("POST /v1/responses HTTP/1.1"));
    assert!(!observed.to_ascii_lowercase().contains("authorization:"));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn dropping_an_inflight_call_kills_the_child_and_closes_provider_transport() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("cancellation provider listener");
    let address = listener
        .local_addr()
        .expect("cancellation provider address");
    let (contacted_sender, contacted_receiver) = oneshot::channel();
    let provider = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("cancellation provider connection");
        let mut request = vec![0_u8; 64 * 1024];
        let read = stream
            .read(&mut request)
            .await
            .expect("cancellation provider request");
        assert!(read > 0);
        contacted_sender.send(()).expect("report provider contact");
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match stream.read(&mut request).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::BrokenPipe
                        ) =>
                    {
                        break;
                    }
                    Err(error) => panic!("provider transport close failed: {error}"),
                }
            }
        })
        .await
        .expect("cancelled child closes provider transport");
    });
    let connection = ProviderConnection {
        contract_version: CONTRACT_VERSION,
        id: "cancelled-real-child-provider".into(),
        display_name: "Cancelled real child provider".into(),
        kind: ProviderKind::OpenAiCompatible,
        base_url: format!("http://{address}/v1"),
        auth_mode: ProviderAuthMode::None,
        organization: None,
        project: None,
    };
    let model = ModelDescriptor {
        id: "cancelled-real-child-model".into(),
        display_name: "Cancelled real child model".into(),
        owned_by: Some("fixture".into()),
        context_window: Some(1_024),
        capabilities: ModelCapabilities {
            responses: Some(true),
            ..ModelCapabilities::default()
        },
        price: None,
        metadata: serde_json::Value::Null,
    };
    let model_request = ModelRequest {
        contract_version: CONTRACT_VERSION,
        model: model.id.clone(),
        client_request_id: Some("cancelled-real-child-request".into()),
        instructions: None,
        messages: vec![Message {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "wait until cancelled".into(),
            }],
            name: None,
        }],
        tools: Vec::new(),
        tool_choice: None,
        structured_output: None,
        reasoning: None,
        temperature: Some(0.0),
        top_p: None,
        max_output_tokens: Some(32),
        metadata: serde_json::Value::Null,
    };
    let request = ProviderRunnerRequest {
        protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
        request_id: Uuid::new_v4(),
        authorization: authorization(&connection, &model, &model_request),
        connection,
        model,
        request: model_request,
    };
    let client = ProviderRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-provider-runner"),
        Duration::from_secs(30),
    )
    .expect("cancellable provider runner client");
    let invocation = tokio::spawn(async move { client.invoke(&request).await });
    tokio::time::timeout(Duration::from_secs(5), contacted_receiver)
        .await
        .expect("provider contact deadline")
        .expect("provider contact signal");
    invocation.abort();
    assert!(
        invocation
            .await
            .expect_err("cancelled invocation task")
            .is_cancelled()
    );
    provider.await.expect("cancelled provider fixture");
}
