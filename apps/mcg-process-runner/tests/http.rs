use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mcg_mcp::{McpCredential, McpCredentialStore, MemoryMcpCredentialStore};
use mcg_process_runner::{
    GovernedHttpHeader, GovernedHttpHeaderValue, GovernedHttpOperation,
    HttpRunnerCapabilityEvidence, HttpRunnerClient, HttpRunnerOutcome, ProcessRunnerClientError,
    decode_governed_http_observation, execute_http_request_with_store,
    execute_http_request_with_stores, http_qualification_request_with_evidence,
    http_runner_sandbox_profile, runner_executable_source,
};
use mcg_providers::{
    MemoryProviderCredentialStore, ProviderCredential, ProviderCredentialStore, ProviderSecretSlot,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
};

async fn one_shot_server(response: Option<Vec<u8>>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture");
    let address = listener.local_addr().expect("fixture address");
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        observed.fetch_add(1, Ordering::SeqCst);
        let mut bytes = vec![0_u8; 32 * 1024];
        let _ = stream.read(&mut bytes).await;
        if let Some(response) = response {
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        }
    });
    (format!("http://{address}"), calls)
}

async fn delayed_response_server(delay: Duration, response: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind delayed fixture");
    let address = listener.local_addr().expect("delayed fixture address");
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        observed.fetch_add(1, Ordering::SeqCst);
        let mut bytes = vec![0_u8; 32 * 1024];
        let _ = stream.read(&mut bytes).await;
        tokio::time::sleep(delay).await;
        let _ = stream.write_all(&response).await;
        let _ = stream.shutdown().await;
    });
    (format!("http://{address}"), calls)
}

async fn capturing_server(response: Vec<u8>) -> (String, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind capturing fixture");
    let address = listener.local_addr().expect("capturing fixture address");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let observed = captured.clone();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut bytes = vec![0_u8; 32 * 1024];
        let count = stream.read(&mut bytes).await.unwrap_or_default();
        observed
            .lock()
            .expect("capture request")
            .extend_from_slice(&bytes[..count]);
        let _ = stream.write_all(&response).await;
        let _ = stream.shutdown().await;
    });
    (format!("http://{address}"), captured)
}

fn operation(method: &str, url: &str) -> GovernedHttpOperation {
    let parsed = url::Url::parse(url).expect("fixture URL");
    GovernedHttpOperation {
        method: method.into(),
        url: parsed.to_string(),
        origin: parsed.origin().ascii_serialization(),
        headers: Vec::new(),
        body_base64: (method == "POST").then(|| STANDARD.encode(b"fixture")),
        timeout_ms: 2_000,
        max_response_bytes: 64 * 1024,
        max_redirects: if method == "GET" { 5 } else { 0 },
    }
}

fn packaged_client() -> HttpRunnerClient {
    HttpRunnerClient::with_executable(
        PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner")),
        Duration::from_secs(10),
    )
    .expect("HTTP runner client")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_executes_exact_pinned_get() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 8\r\nConnection: close\r\n\r\ngoverned".to_vec();
    let (origin, calls) = one_shot_server(Some(response)).await;
    let client = packaged_client();
    let request = http_qualification_request_with_evidence(
        &operation("GET", &format!("{origin}/data?private=omitted")),
        client.capability_evidence(),
    )
    .expect("qualification request");
    let response = client.invoke(&request).await.expect("HTTP runner response");
    assert_eq!(response.outcome, HttpRunnerOutcome::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let observation =
        decode_governed_http_observation(response.observation.as_ref().expect("HTTP observation"))
            .expect("decode HTTP observation");
    assert_eq!(observation.status_code, 200);
    assert!(!observation.final_url.contains("private"));
    assert_eq!(
        STANDARD.decode(observation.body_base64).expect("body"),
        b"governed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn substituted_authority_is_denied_before_network_contact() {
    let (origin, calls) = one_shot_server(Some(
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
    ))
    .await;
    let client = packaged_client();
    let mut request = http_qualification_request_with_evidence(
        &operation("GET", &format!("{origin}/data")),
        client.capability_evidence(),
    )
    .expect("qualification request");
    request.operation_hash.digest_hex = "0".repeat(64);
    let response = client.invoke(&request).await.expect("denial response");
    assert_eq!(response.outcome, HttpRunnerOutcome::Denied);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_origin_redirect_is_denied_without_following() {
    let response = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/escape\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (origin, calls) = one_shot_server(Some(response)).await;
    let client = packaged_client();
    let request = http_qualification_request_with_evidence(
        &operation("GET", &format!("{origin}/redirect")),
        client.capability_evidence(),
    )
    .expect("qualification request");
    let response = client.invoke(&request).await.expect("redirect response");
    assert_eq!(response.outcome, HttpRunnerOutcome::NoEffectConfirmed);
    assert_eq!(
        response.error_code.as_deref(),
        Some("redirect_scope_denied")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_mutating_response_is_outcome_unknown() {
    let (origin, calls) = one_shot_server(None).await;
    let client = packaged_client();
    let request = http_qualification_request_with_evidence(
        &operation("POST", &format!("{origin}/mutate")),
        client.capability_evidence(),
    )
    .expect("qualification request");
    let response = client.invoke(&request).await.expect("unknown response");
    assert_eq!(response.outcome, HttpRunnerOutcome::OutcomeUnknown);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mutating_request_timeout_after_contact_is_outcome_unknown() {
    let response =
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (origin, calls) = delayed_response_server(Duration::from_millis(500), response).await;
    let client = packaged_client();
    let mut operation = operation("POST", &format!("{origin}/mutate"));
    operation.timeout_ms = 75;
    let request =
        http_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualification request");
    let response = client.invoke(&request).await.expect("timeout response");
    assert_eq!(response.outcome, HttpRunnerOutcome::OutcomeUnknown);
    assert_eq!(
        response.error_code.as_deref(),
        Some("request_timeout_unknown")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authoritative_cancellation_reaps_contacted_http_runner() {
    let response =
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (origin, calls) = delayed_response_server(Duration::from_secs(2), response).await;
    let client = packaged_client();
    let mut operation = operation("POST", &format!("{origin}/mutate"));
    operation.timeout_ms = 5_000;
    let request =
        http_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualification request");
    let observed_calls = calls.clone();

    let result = client
        .invoke_with_cancellation(&request, async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                while observed_calls.load(Ordering::SeqCst) == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("HTTP runner did not contact the fixture");
        })
        .await;

    assert_eq!(result, Err(ProcessRunnerClientError::Cancelled));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn opaque_credential_is_resolved_only_in_runner_and_redacted_from_output() {
    let secret = "fixture-private-token";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{secret}",
        secret.len()
    )
    .into_bytes();
    let (origin, calls) = one_shot_server(Some(response)).await;
    let executable = std::env::current_exe().expect("test executable");
    let evidence = HttpRunnerCapabilityEvidence {
        runner: runner_executable_source(&executable).expect("test runner source"),
        platform: mcg_process_runner::current_execution_platform(),
        available_sandbox: http_runner_sandbox_profile(),
        resource_ceiling: mcg_contracts::ToolResourceLimits {
            timeout_ms: 120_000,
            max_output_bytes: mcg_process_runner::HTTP_RUNNER_BODY_MAX_BYTES,
            max_memory_bytes: 0,
            max_child_processes: 0,
        },
    };
    let mut operation = operation("GET", &format!("{origin}/secret"));
    operation.headers.push(GovernedHttpHeader {
        name: "authorization".into(),
        value: GovernedHttpHeaderValue::Credential {
            provider: "fixture-provider".into(),
            key: "primary".into(),
        },
    });
    let request = http_qualification_request_with_evidence(&operation, &evidence)
        .expect("credential qualification request");
    let serialized = serde_json::to_string(&request).expect("serialize request");
    assert!(!serialized.contains(secret));
    let store = MemoryProviderCredentialStore::default();
    store
        .save(
            "fixture-provider",
            ProviderSecretSlot::Primary,
            &ProviderCredential::new(secret).expect("credential"),
        )
        .expect("save credential");
    let response = execute_http_request_with_store(
        &serde_json::to_vec(&request).expect("request bytes"),
        &store,
    )
    .await;
    assert_eq!(response.outcome, HttpRunnerOutcome::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let observation =
        decode_governed_http_observation(response.observation.as_ref().expect("observation"))
            .expect("decode observation");
    let body = STANDARD.decode(observation.body_base64).expect("body");
    assert_eq!(body, b"[REDACTED]");
}

#[tokio::test]
async fn opaque_mcp_bearer_is_exactly_bound_resolved_and_redacted() {
    let secret = "fixture-mcp-private-token";
    let body = format!("{{\"echo\":\"{secret}\"}}");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes();
    let (origin, captured) = capturing_server(response).await;
    let executable = std::env::current_exe().expect("test executable");
    let evidence = HttpRunnerCapabilityEvidence {
        runner: runner_executable_source(&executable).expect("test runner source"),
        platform: mcg_process_runner::current_execution_platform(),
        available_sandbox: http_runner_sandbox_profile(),
        resource_ceiling: mcg_contracts::ToolResourceLimits {
            timeout_ms: 120_000,
            max_output_bytes: mcg_process_runner::HTTP_RUNNER_BODY_MAX_BYTES,
            max_memory_bytes: 0,
            max_child_processes: 0,
        },
    };
    let mut operation = operation("POST", &format!("{origin}/mcp"));
    operation.body_base64 = Some(STANDARD.encode(b"{\"jsonrpc\":\"2.0\"}"));
    operation.headers.push(GovernedHttpHeader {
        name: "authorization".into(),
        value: GovernedHttpHeaderValue::McpBearer {
            connection_id: "fixture-mcp".into(),
        },
    });
    let request = http_qualification_request_with_evidence(&operation, &evidence)
        .expect("MCP bearer qualification request");
    let serialized = serde_json::to_string(&request).expect("serialize MCP request");
    assert!(!serialized.contains(secret));
    let mcp_store = MemoryMcpCredentialStore::default();
    mcp_store
        .save(
            "fixture-mcp",
            &McpCredential::new(secret).expect("MCP credential"),
        )
        .expect("save MCP credential");
    let response = execute_http_request_with_stores(
        &serde_json::to_vec(&request).expect("request bytes"),
        &MemoryProviderCredentialStore::default(),
        &mcp_store,
    )
    .await;
    assert_eq!(response.outcome, HttpRunnerOutcome::Completed);
    let captured = captured.lock().expect("captured request");
    let outbound = String::from_utf8_lossy(&captured);
    assert!(outbound.contains(&format!("authorization: Bearer {secret}")));
    let observation =
        decode_governed_http_observation(response.observation.as_ref().expect("observation"))
            .expect("decode observation");
    let body = STANDARD.decode(observation.body_base64).expect("body");
    assert!(!String::from_utf8_lossy(&body).contains(secret));
}

#[tokio::test]
async fn hostname_resolution_to_loopback_is_denied_before_contact() {
    let response = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec();
    let (origin, calls) = one_shot_server(Some(response)).await;
    let port = url::Url::parse(&origin)
        .expect("fixture origin")
        .port()
        .expect("fixture port");
    let executable = std::env::current_exe().expect("test executable");
    let evidence = HttpRunnerCapabilityEvidence {
        runner: runner_executable_source(&executable).expect("test runner source"),
        platform: mcg_process_runner::current_execution_platform(),
        available_sandbox: http_runner_sandbox_profile(),
        resource_ceiling: mcg_contracts::ToolResourceLimits {
            timeout_ms: 120_000,
            max_output_bytes: mcg_process_runner::HTTP_RUNNER_BODY_MAX_BYTES,
            max_memory_bytes: 0,
            max_child_processes: 0,
        },
    };
    let request = http_qualification_request_with_evidence(
        &operation("GET", &format!("http://localhost:{port}/rebind")),
        &evidence,
    )
    .expect("hostname qualification request");
    let response = execute_http_request_with_store(
        &serde_json::to_vec(&request).expect("request bytes"),
        &MemoryProviderCredentialStore::default(),
    )
    .await;
    assert_eq!(response.outcome, HttpRunnerOutcome::Denied);
    assert_eq!(response.error_code.as_deref(), Some("dns_scope_denied"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
