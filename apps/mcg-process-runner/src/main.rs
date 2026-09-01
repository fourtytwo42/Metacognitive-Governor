use std::{path::PathBuf, process::ExitCode, time::Duration};

use chrono::Utc;
use mcg_browser_contracts::{
    BROWSER_WORKFLOW_SCHEMA_VERSION, BrowserWorkflowDocument, BrowserWorkflowStep,
};
use mcg_mcp::{
    LEGACY_MCP_PROTOCOL_VERSION, MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpConnection,
    McpProtocolMode, McpServerInfo, McpToolCatalog, McpToolDescriptor, McpTransport,
};
use mcg_process_runner::{
    BROWSER_RUNNER_REQUEST_MAX_BYTES, BrowserRunnerClient, BrowserRunnerOutcome,
    BrowserRunnerResponse, GovernedBrowserOperation, GovernedHttpOperation,
    GovernedMcpSessionOperation, GovernedProcessOperation, HTTP_RUNNER_REQUEST_MAX_BYTES,
    HttpRunnerClient, HttpRunnerOutcome, HttpRunnerResponse, MCP_SESSION_RUNNER_REQUEST_MAX_BYTES,
    McpSessionRunnerClient, McpSessionRunnerOutcome, McpSessionRunnerResponse,
    PROCESS_RUNNER_REQUEST_MAX_BYTES, ProcessRunnerClient, ProcessRunnerOutcome,
    browser_qualification_request_with_evidence, executable_sha256, execute_browser_request,
    execute_http_request, execute_mcp_session_request, execute_request,
    http_qualification_request_with_evidence, mcp_session_qualification_request_with_evidence,
    qualification_request_with_evidence,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.as_slice() == [std::ffi::OsString::from("--governed-http")] {
        let mut request = Vec::new();
        let read = tokio::io::stdin()
            .take(u64::try_from(HTTP_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut request)
            .await;
        let response = match read {
            Ok(_) => execute_http_request(&request).await,
            Err(_) => execute_http_request(b"").await,
        };
        return emit_http(response).await;
    }
    if arguments.as_slice() == [std::ffi::OsString::from("--governed-browser")] {
        let mut request = Vec::new();
        let read = tokio::io::stdin()
            .take(u64::try_from(BROWSER_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut request)
            .await;
        let response = match read {
            Ok(_) => execute_browser_request(&request).await,
            Err(_) => execute_browser_request(b"").await,
        };
        return emit_browser(response).await;
    }
    if arguments.as_slice() == [std::ffi::OsString::from("--governed-mcp-session")] {
        let mut request = Vec::new();
        let read = tokio::io::stdin()
            .take(u64::try_from(MCP_SESSION_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut request)
            .await;
        let response = match read {
            Ok(_) => execute_mcp_session_request(&request).await,
            Err(_) => execute_mcp_session_request(b"").await,
        };
        return emit_mcp_session(response).await;
    }
    if arguments.as_slice() == [std::ffi::OsString::from("--qualify-http-policy")] {
        return qualify_http_policy().await;
    }
    if arguments.as_slice() == [std::ffi::OsString::from("--qualify-mcp-session-policy")] {
        return qualify_mcp_session_policy().await;
    }
    if arguments.len() == 3 && arguments[0] == "--qualify-browser-policy" {
        return qualify_browser_policy(PathBuf::from(&arguments[1]), PathBuf::from(&arguments[2]))
            .await;
    }
    if !arguments.is_empty() {
        return qualify(&arguments).await;
    }
    let mut request = Vec::new();
    let read = tokio::io::stdin()
        .take(u64::try_from(PROCESS_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut request)
        .await;
    let response = match read {
        Ok(_) => execute_request(&request).await,
        Err(_) => execute_request(b"").await,
    };
    emit(response).await
}

async fn emit_mcp_session(response: McpSessionRunnerResponse) -> ExitCode {
    let succeeded = response.outcome == McpSessionRunnerOutcome::Completed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"protocol_version":1,"request_id":null,"action_id":null,"execution_id":null,"outcome":"denied","observation":null,"error_code":"response_serialization_failed","detail":"could not encode MCP session runner response","started_at":"1970-01-01T00:00:00Z","finished_at":"1970-01-01T00:00:00Z"}"#.to_vec()
    });
    let mut stdout = tokio::io::stdout();
    if stdout.write_all(&output).await.is_err() || stdout.write_all(b"\n").await.is_err() {
        return ExitCode::FAILURE;
    }
    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn qualify_browser_policy(browser: PathBuf, profile_root: PathBuf) -> ExitCode {
    let Ok(current) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    if std::fs::create_dir_all(&profile_root).is_err() {
        return ExitCode::FAILURE;
    }
    let Ok(profile_root) = std::fs::canonicalize(profile_root) else {
        return ExitCode::FAILURE;
    };
    let Ok(client) =
        BrowserRunnerClient::with_executables(&current, &browser, Duration::from_secs(10))
    else {
        return ExitCode::FAILURE;
    };
    let operation = GovernedBrowserOperation {
        browser_executable: browser.clone(),
        browser_sha256: match executable_sha256(&browser) {
            Ok(digest) => digest,
            Err(_) => return ExitCode::FAILURE,
        },
        profile_root,
        origin: "https://example.com".into(),
        workflow: BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "browser package policy qualification".into(),
            start_url: "https://example.com/qualification".into(),
            steps: vec![BrowserWorkflowStep::AssertElement {
                selector: "main".into(),
            }],
        },
        viewport_width: 1280,
        viewport_height: 720,
        timeout_ms: 5_000,
    };
    let Ok(mut request) =
        browser_qualification_request_with_evidence(&operation, client.capability_evidence())
    else {
        return ExitCode::FAILURE;
    };
    request.operation_hash.digest_hex = "0".repeat(64);
    let Ok(response) = client.invoke(&request).await else {
        return ExitCode::FAILURE;
    };
    let qualified = response.outcome == BrowserRunnerOutcome::Denied
        && response.error_code.as_deref() == Some("tool_authority_mismatch")
        && response.observation.is_none();
    let output = serde_json::to_vec(&response).unwrap_or_default();
    let mut stdout = tokio::io::stdout();
    if output.is_empty()
        || stdout.write_all(&output).await.is_err()
        || stdout.write_all(b"\n").await.is_err()
    {
        return ExitCode::FAILURE;
    }
    if qualified {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn emit_browser(response: BrowserRunnerResponse) -> ExitCode {
    let succeeded = response.outcome == BrowserRunnerOutcome::Completed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"protocol_version":1,"request_id":null,"action_id":null,"execution_id":null,"outcome":"denied","observation":null,"error_code":"response_serialization_failed","detail":"could not encode browser runner response","started_at":"1970-01-01T00:00:00Z","finished_at":"1970-01-01T00:00:00Z"}"#.to_vec()
    });
    let mut stdout = tokio::io::stdout();
    if stdout.write_all(&output).await.is_err() || stdout.write_all(b"\n").await.is_err() {
        return ExitCode::FAILURE;
    }
    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn qualify_http_policy() -> ExitCode {
    let Ok(current) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    let Ok(client) = HttpRunnerClient::with_executable(&current, Duration::from_secs(10)) else {
        return ExitCode::FAILURE;
    };
    let operation = GovernedHttpOperation {
        method: "GET".into(),
        url: "http://localhost:9/qualification".into(),
        origin: "http://localhost:9".into(),
        headers: Vec::new(),
        body_base64: None,
        timeout_ms: 5_000,
        max_response_bytes: 64 * 1024,
        max_redirects: 0,
    };
    let Ok(request) =
        http_qualification_request_with_evidence(&operation, client.capability_evidence())
    else {
        return ExitCode::FAILURE;
    };
    let Ok(bytes) = serde_json::to_vec(&request) else {
        return ExitCode::FAILURE;
    };
    let response = execute_http_request(&bytes).await;
    let qualified = response.outcome == HttpRunnerOutcome::Denied
        && response.error_code.as_deref() == Some("dns_scope_denied")
        && response.observation.is_none();
    let output = serde_json::to_vec(&response).unwrap_or_default();
    let mut stdout = tokio::io::stdout();
    if output.is_empty()
        || stdout.write_all(&output).await.is_err()
        || stdout.write_all(b"\n").await.is_err()
    {
        return ExitCode::FAILURE;
    }
    if qualified {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn qualify_mcp_session_policy() -> ExitCode {
    let Ok(current) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    let now = Utc::now();
    let connection = McpConnection {
        contract_version: MCP_CONNECTION_CONTRACT_VERSION,
        id: "package-policy-qualification".into(),
        display_name: "Package policy qualification".into(),
        transport: McpTransport::StreamableHttp {
            url: "http://localhost:9/mcp".into(),
        },
        auth_mode: McpAuthMode::None,
        protocol_mode: McpProtocolMode::Legacy,
        enabled: true,
    };
    let operation = GovernedMcpSessionOperation {
        connection: connection.clone(),
        catalog: McpToolCatalog {
            connection_id: connection.id.clone(),
            protocol_version: LEGACY_MCP_PROTOCOL_VERSION.into(),
            server_info: McpServerInfo {
                name: "package-policy-qualification".into(),
                version: Some("1.0.0".into()),
            },
            capabilities: serde_json::json!({"tools": {}}),
            tools: vec![McpToolDescriptor {
                name: "qualify".into(),
                title: Some("Qualify".into()),
                description: Some("Package policy qualification".into()),
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: None,
                annotations: serde_json::json!({"readOnlyHint": true}),
            }],
            discovered_at: now,
            expires_at: now + chrono::Duration::minutes(5),
        },
        tool_name: "qualify".into(),
        arguments: serde_json::json!({}),
        stdio_executable_sha256: None,
        timeout_ms: 5_000,
        max_response_bytes: 64 * 1024,
    };
    let Ok(client) = McpSessionRunnerClient::with_operation(&current, &operation) else {
        return ExitCode::FAILURE;
    };
    let Ok(mut request) =
        mcp_session_qualification_request_with_evidence(&operation, client.capability_evidence())
    else {
        return ExitCode::FAILURE;
    };
    request.operation_hash.digest_hex = "0".repeat(64);
    let Ok(response) = client.invoke(&request).await else {
        return ExitCode::FAILURE;
    };
    let qualified = response.outcome == McpSessionRunnerOutcome::Denied
        && response.error_code.as_deref() == Some("tool_authority_mismatch")
        && response.observation.is_none();
    let output = serde_json::to_vec(&response).unwrap_or_default();
    let mut stdout = tokio::io::stdout();
    if output.is_empty()
        || stdout.write_all(&output).await.is_err()
        || stdout.write_all(b"\n").await.is_err()
    {
        return ExitCode::FAILURE;
    }
    if qualified {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn emit_http(response: HttpRunnerResponse) -> ExitCode {
    let succeeded = response.outcome == HttpRunnerOutcome::Completed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"protocol_version":1,"request_id":null,"action_id":null,"execution_id":null,"outcome":"denied","observation":null,"error_code":"response_serialization_failed","detail":"could not encode HTTP runner response","started_at":"1970-01-01T00:00:00Z","finished_at":"1970-01-01T00:00:00Z"}"#.to_vec()
    });
    let mut stdout = tokio::io::stdout();
    if stdout.write_all(&output).await.is_err() || stdout.write_all(b"\n").await.is_err() {
        return ExitCode::FAILURE;
    }
    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn qualify(arguments: &[std::ffi::OsString]) -> ExitCode {
    if arguments.len() != 2 || arguments[0] != "--qualify-process-root" {
        return ExitCode::FAILURE;
    }
    let root = PathBuf::from(&arguments[1]);
    let Ok(current) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    let Ok(client) = ProcessRunnerClient::with_executable(&current, Duration::from_secs(10)) else {
        return ExitCode::FAILURE;
    };
    #[cfg(windows)]
    let (executable, argv) = (
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        vec![
            "/d".into(),
            "/c".into(),
            "echo governed-process-qualified".into(),
        ],
    );
    #[cfg(unix)]
    let (executable, argv) = (
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), "printf governed-process-qualified".into()],
    );
    let operation = GovernedProcessOperation {
        executable: executable.clone(),
        executable_sha256: match executable_sha256(&executable) {
            Ok(digest) => digest,
            Err(_) => return ExitCode::FAILURE,
        },
        argv,
        cwd_relative: ".".into(),
        environment: Vec::new(),
        stdin_base64: None,
        timeout_ms: 5_000,
        max_output_bytes: 64 * 1024,
    };
    let Ok(request) =
        qualification_request_with_evidence(&root, &operation, client.capability_evidence())
    else {
        return ExitCode::FAILURE;
    };
    let Ok(bytes) = serde_json::to_vec(&request) else {
        return ExitCode::FAILURE;
    };
    emit(execute_request(&bytes).await).await
}

async fn emit(response: mcg_process_runner::ProcessRunnerResponse) -> ExitCode {
    let succeeded = response.outcome == ProcessRunnerOutcome::EffectConfirmed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"protocol_version":1,"request_id":null,"action_id":null,"execution_id":null,"outcome":"denied","observation":null,"error_code":"response_serialization_failed","detail":"could not encode runner response","started_at":"1970-01-01T00:00:00Z","finished_at":"1970-01-01T00:00:00Z"}"#.to_vec()
    });
    let mut stdout = tokio::io::stdout();
    if stdout.write_all(&output).await.is_err() || stdout.write_all(b"\n").await.is_err() {
        return ExitCode::FAILURE;
    }
    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
