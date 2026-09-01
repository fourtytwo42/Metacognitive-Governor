use std::{
    io::{BufRead as _, Write as _},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse as _,
    routing::post,
};
use chrono::Utc;
use mcg_mcp::{
    MCP_CONNECTION_CONTRACT_VERSION, MCP_PROTOCOL_VERSION, McpAuthMode, McpConnection,
    McpProtocolMode, McpServerInfo, McpToolCatalog, McpToolDescriptor, McpTransport,
};
use mcg_process_runner::{
    GovernedMcpSessionOperation, McpSessionRunnerClient, McpSessionRunnerOutcome,
    ProcessRunnerClientError, decode_governed_mcp_session_observation, executable_sha256,
    mcp_session_qualification_request_with_evidence,
};
use serde_json::json;

fn copied_fixture_executable(directory: &std::path::Path) -> std::path::PathBuf {
    let destination = directory.join(if cfg!(windows) {
        "mcp-session-fixture.exe"
    } else {
        "mcp-session-fixture"
    });
    std::fs::copy(
        std::env::current_exe().expect("current test executable"),
        &destination,
    )
    .expect("copy MCP fixture executable");
    destination
}

fn operation(executable: &std::path::Path) -> GovernedMcpSessionOperation {
    let now = Utc::now();
    GovernedMcpSessionOperation {
        connection: McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "governed-stdio".into(),
            display_name: "Governed stdio fixture".into(),
            transport: McpTransport::Stdio {
                command: executable.to_string_lossy().into_owned(),
                args: vec![
                    "--ignored".into(),
                    "--exact".into(),
                    "stdio_server_fixture".into(),
                    "--nocapture".into(),
                ],
            },
            auth_mode: McpAuthMode::None,
            protocol_mode: McpProtocolMode::Modern20260728,
            enabled: true,
        },
        catalog: McpToolCatalog {
            connection_id: "governed-stdio".into(),
            protocol_version: MCP_PROTOCOL_VERSION.into(),
            server_info: McpServerInfo {
                name: "governed-fixture".into(),
                version: Some("1".into()),
            },
            capabilities: json!({"tools": {}}),
            tools: vec![McpToolDescriptor {
                name: "echo".into(),
                title: None,
                description: Some("Echo exact input".into()),
                input_schema: json!({"type": "object"}),
                output_schema: None,
                annotations: json!({"readOnlyHint": true}),
            }],
            discovered_at: now - chrono::Duration::seconds(1),
            expires_at: now + chrono::Duration::minutes(5),
        },
        tool_name: "echo".into(),
        arguments: json!({"message": "governed"}),
        stdio_executable_sha256: Some(executable_sha256(executable).expect("fixture identity")),
        timeout_ms: 30_000,
        max_response_bytes: 64 * 1024,
    }
}

fn legacy_http_operation(url: String) -> GovernedMcpSessionOperation {
    let now = Utc::now();
    GovernedMcpSessionOperation {
        connection: McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "governed-legacy-http".into(),
            display_name: "Governed legacy HTTP fixture".into(),
            transport: McpTransport::StreamableHttp { url },
            auth_mode: McpAuthMode::None,
            protocol_mode: McpProtocolMode::Legacy,
            enabled: true,
        },
        catalog: McpToolCatalog {
            connection_id: "governed-legacy-http".into(),
            protocol_version: mcg_mcp::LEGACY_MCP_PROTOCOL_VERSION.into(),
            server_info: McpServerInfo {
                name: "legacy-governed-fixture".into(),
                version: Some("1".into()),
            },
            capabilities: json!({"tools": {}}),
            tools: vec![McpToolDescriptor {
                name: "echo".into(),
                title: None,
                description: Some("Echo exact input".into()),
                input_schema: json!({"type": "object"}),
                output_schema: None,
                annotations: json!({"readOnlyHint": true}),
            }],
            discovered_at: now - chrono::Duration::seconds(1),
            expires_at: now + chrono::Duration::minutes(5),
        },
        tool_name: "echo".into(),
        arguments: json!({"message": "legacy-governed"}),
        stdio_executable_sha256: None,
        timeout_ms: 30_000,
        max_response_bytes: 64 * 1024,
    }
}

async fn legacy_http_fixture(
    State(methods): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> axum::response::Response {
    let method = request["method"].as_str().unwrap_or_default().to_owned();
    methods
        .lock()
        .expect("fixture methods")
        .push(method.clone());
    if method != "initialize" {
        assert_eq!(
            headers
                .get("mcp-session-id")
                .and_then(|value| value.to_str().ok()),
            Some("governed-session")
        );
    }
    if method == "notifications/initialized" {
        return StatusCode::ACCEPTED.into_response();
    }
    let result = match method.as_str() {
        "initialize" => json!({
            "protocolVersion": mcg_mcp::LEGACY_MCP_PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "legacy-governed-fixture", "version": "1"}
        }),
        "tools/list" => json!({"tools": [{
            "name": "echo",
            "description": "Echo exact input",
            "inputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": true}
        }]}),
        "tools/call" => json!({
            "isError": false,
            "content": [],
            "structuredContent": request["params"]["arguments"].clone()
        }),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let mut response = Json(json!({
        "jsonrpc": "2.0",
        "id": request["id"].clone(),
        "result": result
    }))
    .into_response();
    if method == "initialize" {
        response.headers_mut().insert(
            "mcp-session-id",
            HeaderValue::from_static("governed-session"),
        );
    }
    response
}

async fn legacy_http_server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind legacy fixture");
    let address = listener.local_addr().expect("legacy fixture address");
    let methods = Arc::new(Mutex::new(Vec::new()));
    let state = methods.clone();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/mcp", post(legacy_http_fixture))
                .with_state(state),
        )
        .await
        .expect("serve legacy fixture");
    });
    (format!("http://{address}/mcp"), methods)
}

#[tokio::test]
async fn real_child_executes_one_exact_catalog_bound_stdio_session() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let fixture = copied_fixture_executable(temporary.path());
    let operation = operation(&fixture);
    let runner = std::path::PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client =
        McpSessionRunnerClient::with_operation(&runner, &operation).expect("session client");
    let request =
        mcp_session_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualified session request");
    let response = client.invoke(&request).await.expect("session response");
    assert_eq!(response.outcome, McpSessionRunnerOutcome::Completed);
    let observation = decode_governed_mcp_session_observation(
        response.observation.as_ref().expect("session observation"),
    )
    .expect("decode session observation");
    assert_eq!(observation.connection_id, "governed-stdio");
    assert_eq!(observation.protocol_version, MCP_PROTOCOL_VERSION);
    assert_eq!(observation.tool_name, "echo");
    assert_eq!(
        observation.result.structured_content,
        Some(json!({"message": "governed"}))
    );

    let mut substituted = request;
    substituted.operation_hash.digest_hex = "0".repeat(64);
    let denied = client
        .invoke(&substituted)
        .await
        .expect("substitution denial response");
    assert_eq!(denied.outcome, McpSessionRunnerOutcome::Denied);
    assert_eq!(
        denied.error_code.as_deref(),
        Some("tool_authority_mismatch")
    );
}

#[tokio::test]
async fn changed_stdio_identity_is_rejected_before_session_runner_launch() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let fixture = copied_fixture_executable(temporary.path());
    let operation = operation(&fixture);
    let runner = std::path::PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client =
        McpSessionRunnerClient::with_operation(&runner, &operation).expect("session client");
    let request =
        mcp_session_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualified session request");
    std::fs::write(&fixture, b"substituted stdio executable").expect("replace fixture executable");
    assert_eq!(
        client.invoke(&request).await.expect_err("identity denial"),
        ProcessRunnerClientError::ExecutableChanged
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_child_executes_one_exact_catalog_bound_legacy_http_session() {
    let (url, methods) = legacy_http_server().await;
    let operation = legacy_http_operation(url);
    let runner = std::path::PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client =
        McpSessionRunnerClient::with_operation(&runner, &operation).expect("session client");
    let request =
        mcp_session_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualified session request");
    let response = client.invoke(&request).await.expect("session response");
    assert_eq!(response.outcome, McpSessionRunnerOutcome::Completed);
    let observation = decode_governed_mcp_session_observation(
        response.observation.as_ref().expect("session observation"),
    )
    .expect("decode session observation");
    assert_eq!(
        observation.result.structured_content,
        Some(json!({"message": "legacy-governed"}))
    );
    assert_eq!(
        *methods.lock().expect("fixture methods"),
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call"
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changed_live_legacy_catalog_blocks_the_tool_call() {
    let (url, methods) = legacy_http_server().await;
    let mut operation = legacy_http_operation(url);
    operation.catalog.tools[0].description = Some("substituted description".into());
    let runner = std::path::PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client =
        McpSessionRunnerClient::with_operation(&runner, &operation).expect("session client");
    let request =
        mcp_session_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("qualified session request");
    let response = client.invoke(&request).await.expect("session response");
    assert_eq!(response.outcome, McpSessionRunnerOutcome::OutcomeUnknown);
    assert_eq!(
        *methods.lock().expect("fixture methods"),
        ["initialize", "notifications/initialized", "tools/list"]
    );
}

#[test]
#[ignore = "spawned as an MCP stdio protocol fixture by the parent test"]
fn stdio_server_fixture() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(request) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let result = match request["method"].as_str().unwrap_or_default() {
            "server/discover" => json!({
                "supportedVersions": [MCP_PROTOCOL_VERSION],
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "governed-fixture", "version": "1"}
            }),
            "tools/list" => json!({"tools": [{
                "name": "echo",
                "description": "Echo exact input",
                "inputSchema": {"type": "object"},
                "annotations": {"readOnlyHint": true}
            }]}),
            "tools/call" => json!({
                "isError": false,
                "content": [],
                "structuredContent": request["params"]["arguments"].clone()
            }),
            _ => {
                writeln!(
                    stdout,
                    "{}",
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}})
                )
                .expect("write fixture error");
                stdout.flush().expect("flush fixture error");
                continue;
            }
        };
        writeln!(
            stdout,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "result": result})
        )
        .expect("write fixture response");
        stdout.flush().expect("flush fixture response");
    }
}
