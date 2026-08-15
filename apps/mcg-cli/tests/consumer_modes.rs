use std::{
    io::Write,
    process::{Command, Output, Stdio},
    sync::Arc,
};

use mcg_config::AppConfig;
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
