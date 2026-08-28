use std::{
    io::Write,
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post, put},
};
use mcg_config::AppConfig;
use mcg_domain::ProjectId;
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

async fn start_feature_server() -> (String, JoinHandle<()>) {
    type FeatureState = Arc<Mutex<Vec<serde_json::Value>>>;

    async fn list(State(features): State<FeatureState>) -> Json<Vec<serde_json::Value>> {
        Json(features.lock().expect("feature state").clone())
    }

    async fn set(
        State(features): State<FeatureState>,
        Path(name): Path<String>,
        Json(request): Json<serde_json::Value>,
    ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
        let enabled = request.get("enabled").and_then(serde_json::Value::as_bool);
        let mut features = features.lock().expect("feature state");
        let Some(feature) = features.iter_mut().find(|feature| feature["name"] == name) else {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "unknown feature"})),
            );
        };
        let Some(enabled) = enabled else {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "enabled is required"})),
            );
        };
        feature["enabled"] = serde_json::json!(enabled);
        (
            axum::http::StatusCode::OK,
            Json(serde_json::Value::Array(features.clone())),
        )
    }

    let features = Arc::new(Mutex::new(vec![
        serde_json::json!({
            "name": "memories",
            "stage": "beta",
            "displayName": "Memories",
            "description": "Use durable memories.",
            "announcement": null,
            "enabled": false,
            "defaultEnabled": false
        }),
        serde_json::json!({
            "name": "shell_tool",
            "stage": "stable",
            "displayName": null,
            "description": null,
            "announcement": null,
            "enabled": true,
            "defaultEnabled": true
        }),
    ]));
    let app = Router::new()
        .route("/api/v1/codex/features", get(list))
        .route("/api/v1/codex/features/{name}", put(set))
        .with_state(features);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("feature listener");
    let address = listener.local_addr().expect("feature listener address");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("feature fixture server");
    });
    (format!("http://{address}"), server)
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

#[test]
fn codex_global_flags_route_to_compatible_commands_before_or_after_the_command() {
    for arguments in [
        vec![
            "--model",
            "gpt-5.6-terra",
            "--sandbox",
            "read-only",
            "--ask-for-approval",
            "untrusted",
            "--image",
            "screen.png",
            "--add-dir",
            ".",
            "--search",
            "--enable",
            "memories",
            "--disable",
            "sqlite",
            "exec",
            "--help",
        ],
        vec![
            "exec",
            "--model",
            "gpt-5.6-terra",
            "--sandbox",
            "read-only",
            "--ask-for-approval",
            "untrusted",
            "--image",
            "screen.png",
            "--add-dir",
            ".",
            "--search",
            "--enable",
            "memories",
            "--disable",
            "sqlite",
            "--help",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mcg"))
            .args(arguments)
            .output()
            .expect("Codex-compatible help process");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage: mcg"));
        assert!(stdout.contains("--ask-for-approval"));
        assert!(stdout.contains("--sandbox"));
        assert!(stdout.contains("--add-dir"));
        assert!(stdout.contains("--enable"));
        assert!(stdout.contains("--disable"));
    }

    let root_help = Command::new(env!("CARGO_BIN_EXE_mcg"))
        .arg("--help")
        .output()
        .expect("root help process");
    assert!(root_help.status.success());
    let root_help = String::from_utf8_lossy(&root_help.stdout);
    assert!(root_help.contains("--search"));
    assert!(root_help.contains("--no-alt-screen"));
}

#[cfg(windows)]
#[test]
fn app_handoff_is_daemon_free_and_reports_the_canonical_workspace() {
    let workspace = tempfile::tempdir().expect("workspace directory");
    let output = Command::new(env!("CARGO_BIN_EXE_mcg"))
        .arg("app")
        .arg(workspace.path())
        .output()
        .expect("desktop handoff process");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Open this workspace in MetaCog Desktop"));
    assert!(
        stdout.contains(
            std::fs::canonicalize(workspace.path())
                .expect("canonical workspace")
                .to_string_lossy()
                .as_ref()
        )
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_supports_structured_task_and_error_workflows() {
    let project = tempfile::tempdir().expect("project directory");
    std::fs::write(
        project.path().join("package.json"),
        r#"{"scripts":{"test":"node --test"}}"#,
    )
    .expect("project manifest");
    let runtime = Arc::new(
        AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned()).expect("runtime"),
    );
    let project_id = ProjectId::new();
    runtime
        .discover_project(project.path(), project_id)
        .expect("discover task project");
    let (host, server) = start_server(runtime.clone()).await;

    let created = run_cli(
        &host,
        project.path(),
        &[
            "task",
            "create",
            "review this diff",
            "--project-id",
            &project_id.to_string(),
            "--json",
        ],
    );
    assert!(
        created.status.success(),
        "create stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created: serde_json::Value =
        serde_json::from_slice(&created.stdout).expect("task JSON result");
    let task_id = created["task"]["id"].as_str().expect("task id");
    let launch = run_cli(
        &host,
        project.path(),
        &["task", "launch", task_id, "--json"],
    );
    assert!(launch.status.success());
    let launch: serde_json::Value =
        serde_json::from_slice(&launch.stdout).expect("launch JSON result");
    assert_eq!(launch["task"]["goal"], "review this diff");
    assert_eq!(launch["task"]["state"], "blocked");
    assert!(!launch.to_string().contains(TEST_TOKEN));

    let malformed = run_cli(
        &host,
        project.path(),
        &["task", "get", "not-a-uuid", "--json"],
    );
    assert!(!malformed.status.success());
    let error: serde_json::Value =
        serde_json::from_slice(&malformed.stderr).expect("structured CLI error");
    assert_eq!(error["error"], "cli_error");

    let oversized = run_cli_with_stdin(
        &host,
        project.path(),
        &["review oversized input"],
        &"x".repeat(1_048_577),
    );
    assert!(!oversized.status.success());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("1048576-byte limit"));

    assert_eq!(runtime.tasks().expect("all task projections").len(), 1);
    assert_eq!(
        runtime.projects().expect("reused project projection").len(),
        1
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)] // Provider, evaluator, replay, and project evidence form one CLI journey contract.
async fn cli_completes_explicit_provider_backed_task_workflows() {
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
    let project_id = ProjectId::new();
    runtime
        .discover_project(project.path(), project_id)
        .expect("discover provider-backed project");
    let (host, daemon_server) = start_server(runtime.clone()).await;

    let mut run_ids = Vec::new();
    for goal in [
        "complete a provider-backed CLI task",
        "complete the JSON journey",
        "complete the replay journey",
    ] {
        let created = run_cli(
            &host,
            project.path(),
            &[
                "task",
                "create",
                goal,
                "--project-id",
                &project_id.to_string(),
                "--json",
            ],
        );
        assert!(
            created.status.success(),
            "create stderr: {}",
            String::from_utf8_lossy(&created.stderr)
        );
        let created: serde_json::Value =
            serde_json::from_slice(&created.stdout).expect("created task JSON");
        let task_id = created["task"]["id"].as_str().expect("created task id");
        let launched = run_cli(
            &host,
            project.path(),
            &["task", "launch", task_id, "--json"],
        );
        assert!(
            launched.status.success(),
            "launch stderr: {}",
            String::from_utf8_lossy(&launched.stderr)
        );
        let launched: serde_json::Value =
            serde_json::from_slice(&launched.stdout).expect("launch JSON");
        assert_eq!(launched["task"]["state"], "completed");
        assert_eq!(launched["run"]["run"]["state"], "completed");
        assert_eq!(launched["execution"]["state"], "completed");
        assert_eq!(
            launched["run"]["evaluations"][0]["outcome"],
            serde_json::json!("passed")
        );
        assert_eq!(launched["run"]["results"].as_array().map(Vec::len), Some(2));
        assert!(!launched.to_string().contains(TEST_TOKEN));
        run_ids.push(
            launched["run"]["run"]["id"]
                .as_str()
                .expect("completed run id")
                .to_owned(),
        );
    }

    let calls_before_replay = provider_calls.load(Ordering::SeqCst);
    let replay = run_cli(
        &host,
        project.path(),
        &["run", "replay", &run_ids[0], "--json"],
    );
    assert!(replay.status.success());
    let replay: serde_json::Value = serde_json::from_slice(&replay.stdout).expect("replay JSON");
    assert_eq!(replay["run_id"], run_ids[0]);
    assert_eq!(replay["side_effect_free"], true);
    assert!(
        replay["invariants"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["passed"] == true))
    );
    assert_eq!(provider_calls.load(Ordering::SeqCst), calls_before_replay);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_compatible_session_commands_use_durable_metacog_conversations() {
    let project = tempfile::tempdir().expect("session project directory");
    std::fs::write(project.path().join("README.md"), "session fixture")
        .expect("session fixture file");
    let runtime = Arc::new(
        AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned()).expect("runtime"),
    );
    let project_id = ProjectId::new();
    runtime
        .discover_project(project.path(), project_id)
        .expect("discover session project");
    let (host, server) = start_server(runtime).await;

    let created = run_cli(
        &host,
        project.path(),
        &[
            "conversation",
            "create",
            "--title",
            "Codex compatibility session",
            "--project-id",
            &project_id.to_string(),
            "--json",
        ],
    );
    assert!(
        created.status.success(),
        "create stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created: serde_json::Value =
        serde_json::from_slice(&created.stdout).expect("created conversation JSON");
    let conversation_id = created["conversation"]["id"]
        .as_str()
        .expect("conversation id")
        .to_owned();

    let archived = run_cli(
        &host,
        project.path(),
        &["archive", "Codex compatibility session"],
    );
    assert!(
        archived.status.success(),
        "archive stderr: {}",
        String::from_utf8_lossy(&archived.stderr)
    );
    assert!(String::from_utf8_lossy(&archived.stdout).contains("Archived"));

    let restored = run_cli(&host, project.path(), &["unarchive", &conversation_id]);
    assert!(
        restored.status.success(),
        "unarchive stderr: {}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert!(String::from_utf8_lossy(&restored.stdout).contains("Restored"));

    let deleted = run_cli(
        &host,
        project.path(),
        &["delete", &conversation_id, "--force"],
    );
    assert!(
        deleted.status.success(),
        "delete stderr: {}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    assert!(String::from_utf8_lossy(&deleted.stdout).contains("Deleted 1 conversation"));

    let missing = run_cli(
        &host,
        project.path(),
        &["conversation", "show", &conversation_id, "--json"],
    );
    assert!(!missing.status.success());
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_compatible_feature_commands_list_and_mutate_pinned_flags() {
    let project = tempfile::tempdir().expect("feature project directory");
    let (host, server) = start_feature_server().await;

    let listed = run_cli(&host, project.path(), &["features", "list"]);
    assert!(
        listed.status.success(),
        "list stderr: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(listed.contains("memories"));
    assert!(listed.contains("beta"));
    assert!(listed.contains("false"));
    assert!(listed.contains("shell_tool"));

    let enabled = run_cli(&host, project.path(), &["features", "enable", "memories"]);
    assert!(
        enabled.status.success(),
        "enable stderr: {}",
        String::from_utf8_lossy(&enabled.stderr)
    );
    assert!(enabled.stdout.is_empty());
    let enabled_list = run_cli(&host, project.path(), &["features", "list"]);
    assert!(
        String::from_utf8_lossy(&enabled_list.stdout)
            .lines()
            .any(|line| line.starts_with("memories") && line.ends_with("true"))
    );

    let disabled = run_cli(&host, project.path(), &["features", "disable", "memories"]);
    assert!(disabled.status.success());
    assert!(disabled.stdout.is_empty());
    let disabled_list = run_cli(&host, project.path(), &["features", "list"]);
    assert!(
        String::from_utf8_lossy(&disabled_list.stdout)
            .lines()
            .any(|line| line.starts_with("memories") && line.ends_with("false"))
    );

    let invalid = run_cli(
        &host,
        project.path(),
        &["features", "enable", "Bad-Feature"],
    );
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("lowercase ASCII key"));

    server.abort();
}
