use std::{
    io::Write,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use mcg_config::AppConfig;
use mcg_providers::{CONTRACT_VERSION, ProviderAuthMode, ProviderConnection, ProviderKind};
use mcg_runtime::AppRuntime;
use tokio::{net::TcpListener, task::JoinHandle};

const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

async fn start_server(runtime: Arc<AppRuntime>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        mcg_api::serve_listener(listener, runtime, std::future::pending())
            .await
            .expect("fixture server");
    });
    (format!("http://{address}"), server)
}

async fn start_provider(calls: Arc<AtomicUsize>) -> (String, JoinHandle<()>) {
    async fn response(
        State(calls): State<Arc<AtomicUsize>>,
        Json(_body): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        calls.fetch_add(1, Ordering::SeqCst);
        Json(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "model": "consumer-cli-model",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "provider-backed CLI result"}]
            }],
            "usage": {"input_tokens": 24, "output_tokens": 4, "total_tokens": 28}
        }))
    }

    let app = Router::new()
        .route(
            "/v1/models",
            get(|| async {
                Json(serde_json::json!({
                    "object": "list",
                    "data": [{
                        "id": "consumer-cli-model",
                        "object": "model",
                        "owned_by": "fixture",
                        "context_window": 32768,
                        "supported_parameters": ["tools"]
                    }]
                }))
            }),
        )
        .route("/v1/responses", post(response))
        .with_state(calls);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("provider listener");
    let address = listener.local_addr().expect("provider address");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("provider fixture server");
    });
    (format!("http://{address}/v1"), server)
}

fn run_cli(host: &str, project: &std::path::Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcg"))
        .args(["--host", host, "-C"])
        .arg(project)
        .args(arguments)
        .env("MCG_AUTH_TOKEN", TEST_TOKEN)
        .output()
        .expect("CLI process")
}

fn run_cli_with_stdin(
    host: &str,
    project: &std::path::Path,
    arguments: &[&str],
    stdin: &str,
) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcg"))
        .args(["--host", host, "-C"])
        .arg(project)
        .args(arguments)
        .env("MCG_AUTH_TOKEN", TEST_TOKEN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI process");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin.as_bytes())
        .expect("write child stdin");
    child.wait_with_output().expect("CLI output")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consumer_cli_supports_bare_piped_json_and_streaming_workflows() {
    let project = tempfile::tempdir().expect("project directory");
    std::fs::write(
        project.path().join("package.json"),
        r#"{"scripts":{"test":"node --test"}}"#,
    )
    .expect("project manifest");
    let runtime = Arc::new(
        AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned()).expect("runtime"),
    );
    let (host, server) = start_server(runtime.clone()).await;

    let bare = run_cli_with_stdin(
        &host,
        project.path(),
        &["review this diff"],
        "diff --git a/file.txt b/file.txt\n+private fixture content\n",
    );
    assert!(!bare.status.success());
    assert!(String::from_utf8_lossy(&bare.stderr).contains("requires attention"));
    let bare_stdout = String::from_utf8(bare.stdout).expect("bare UTF-8 output");
    assert!(bare_stdout.contains("Status: blocked"));
    assert!(!bare_stdout.contains("private fixture content"));

    let tasks = runtime.tasks().expect("tasks after bare submission");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].goal, "review this diff");
    assert!(tasks[0].constraints[0].contains("private fixture content"));

    let json = run_cli(
        &host,
        project.path(),
        &["run", "--json", "fix", "the", "tests"],
    );
    assert!(!json.status.success());
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("one JSON result");
    assert_eq!(json["task"]["goal"], "fix the tests");
    assert_eq!(json["task"]["state"], "blocked");

    let stream = run_cli(
        &host,
        project.path(),
        &["run", "--stream-json", "inspect", "the", "project"],
    );
    assert!(!stream.status.success());
    let records = String::from_utf8(stream.stdout)
        .expect("stream UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("NDJSON record"))
        .collect::<Vec<_>>();
    assert_eq!(
        records.first().and_then(|record| record["type"].as_str()),
        Some("task")
    );
    assert_eq!(
        records.last().and_then(|record| record["type"].as_str()),
        Some("result")
    );
    assert!(records.iter().any(|record| record["type"] == "event"));
    assert!(records.iter().all(|record| {
        !record
            .to_string()
            .contains("0123456789abcdef0123456789abcdef")
    }));

    let piped = run_cli_with_stdin(&host, project.path(), &[], "summarize this project\n");
    assert!(!piped.status.success());
    assert!(String::from_utf8_lossy(&piped.stdout).contains("Status: blocked"));

    let oversized = run_cli_with_stdin(
        &host,
        project.path(),
        &["review oversized input"],
        &"x".repeat(1_048_577),
    );
    assert!(!oversized.status.success());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("1048576-byte limit"));

    assert_eq!(runtime.tasks().expect("all task projections").len(), 4);
    assert_eq!(
        runtime.projects().expect("reused project projection").len(),
        1
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)] // Human, JSON, NDJSON, provider, evaluator, and project-command evidence form one CLI journey contract.
async fn consumer_cli_completes_provider_backed_human_json_and_streaming_workflows() {
    let project = tempfile::tempdir().expect("provider-backed project directory");
    std::fs::write(
        project.path().join("package.json"),
        r#"{"scripts":{"test":"node --test"}}"#,
    )
    .expect("provider-backed project manifest");
    let runtime = Arc::new(
        AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned()).expect("runtime"),
    );
    let provider_calls = Arc::new(AtomicUsize::new(0));
    let (provider_url, provider_server) = start_provider(provider_calls.clone()).await;
    runtime
        .configure_provider(
            ProviderConnection {
                contract_version: CONTRACT_VERSION,
                id: "consumer_cli_fixture".into(),
                display_name: "Consumer CLI Fixture".into(),
                kind: ProviderKind::OpenAiCompatible,
                base_url: provider_url,
                auth_mode: ProviderAuthMode::None,
                organization: None,
                project: None,
            },
            None,
            None,
        )
        .await
        .expect("configure provider-backed CLI fixture");
    let (host, daemon_server) = start_server(runtime.clone()).await;

    let human = run_cli_with_stdin(
        &host,
        project.path(),
        &["complete a provider-backed CLI task"],
        "bounded owner context that must not be printed\n",
    );
    assert!(
        human.status.success(),
        "human stderr: {}",
        String::from_utf8_lossy(&human.stderr)
    );
    let human_stdout = String::from_utf8(human.stdout).expect("human UTF-8 output");
    assert!(human_stdout.contains("Status: completed"));
    assert!(human_stdout.contains("provider-backed CLI result"));
    assert!(!human_stdout.contains("bounded owner context"));
    assert!(!human_stdout.contains(TEST_TOKEN));

    let json = run_cli(
        &host,
        project.path(),
        &["run", "--json", "complete", "the", "JSON", "journey"],
    );
    assert!(
        json.status.success(),
        "JSON stderr: {}",
        String::from_utf8_lossy(&json.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("success JSON");
    assert_eq!(json["task"]["state"], "completed");
    assert_eq!(json["run"]["run"]["state"], "completed");
    assert_eq!(json["execution"]["state"], "completed");
    assert_eq!(
        json["run"]["evaluations"][0]["outcome"],
        serde_json::json!("passed")
    );
    assert_eq!(json["run"]["results"].as_array().map(Vec::len), Some(2));
    assert!(!json.to_string().contains(TEST_TOKEN));
    let run_id = json["run"]["run"]["id"]
        .as_str()
        .expect("completed run id")
        .to_owned();
    let calls_before_replay = provider_calls.load(Ordering::SeqCst);
    let replay = run_cli(&host, project.path(), &["run", "replay", &run_id, "--json"]);
    assert!(
        replay.status.success(),
        "replay stderr: {}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay: serde_json::Value = serde_json::from_slice(&replay.stdout).expect("replay JSON");
    assert_eq!(replay["run_id"], run_id);
    assert_eq!(replay["side_effect_free"], true);
    assert!(
        replay["invariants"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["passed"] == true))
    );
    assert_eq!(provider_calls.load(Ordering::SeqCst), calls_before_replay);

    let stream = run_cli(
        &host,
        project.path(),
        &[
            "run",
            "--stream-json",
            "complete",
            "the",
            "streaming",
            "journey",
        ],
    );
    assert!(
        stream.status.success(),
        "stream stderr: {}",
        String::from_utf8_lossy(&stream.stderr)
    );
    let records = String::from_utf8(stream.stdout)
        .expect("success stream UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("success NDJSON"))
        .collect::<Vec<_>>();
    assert_eq!(
        records.first().and_then(|record| record["type"].as_str()),
        Some("task")
    );
    assert_eq!(
        records.last().and_then(|record| record["type"].as_str()),
        Some("result")
    );
    assert_eq!(
        records
            .last()
            .and_then(|record| record.pointer("/result/task/state")),
        Some(&serde_json::json!("completed"))
    );
    assert!(records.iter().any(|record| record["type"] == "event"));
    assert!(
        records
            .iter()
            .all(|record| !record.to_string().contains(TEST_TOKEN))
    );

    assert_eq!(provider_calls.load(Ordering::SeqCst), 6);
    let tasks = runtime.tasks().expect("provider-backed tasks");
    assert_eq!(tasks.len(), 3);
    assert!(
        tasks
            .iter()
            .all(|task| task.state.to_string() == "completed")
    );
    assert_eq!(
        runtime.projects().expect("provider-backed projects").len(),
        1
    );
    for task in tasks {
        assert!(
            runtime
                .tool_audit(task.id)
                .expect("provider-backed task audit")
                .iter()
                .any(|record| record.tool == "process.exec"),
            "each CLI journey must independently run the discovered project test"
        );
    }

    daemon_server.abort();
    provider_server.abort();
}
