#![cfg(feature = "executor")]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine as _;
use mcg_action_runner::{
    ActionRunnerClient, ActionRunnerClientError, ActionRunnerRequest, ActionRunnerResponse,
    GovernedFilesystemOperation, RunnerEffectOutcome, qualification_request_with_evidence,
};

fn request(root: &Path, operation: &GovernedFilesystemOperation) -> ActionRunnerRequest {
    let client = ActionRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-action-runner"),
        Duration::from_secs(5),
    )
    .unwrap();
    request_for(&client, root, operation)
}

fn request_for(
    client: &ActionRunnerClient,
    root: &Path,
    operation: &GovernedFilesystemOperation,
) -> ActionRunnerRequest {
    qualification_request_with_evidence(root, operation, client.capability_evidence()).unwrap()
}

fn invoke(request: &[u8]) -> (std::process::ExitStatus, ActionRunnerResponse) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcg-action-runner"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(request).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.stderr.is_empty());
    let response = serde_json::from_slice(&output.stdout).unwrap();
    (output.status, response)
}

fn fault_client(root: &Path, timeout: Duration) -> ActionRunnerClient {
    let runner = root.join(if cfg!(windows) {
        "mcg-action-runner.exe"
    } else {
        "mcg-action-runner"
    });
    std::fs::copy(
        env!("CARGO_BIN_EXE_mcg-action-runner-fault-fixture"),
        &runner,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(&runner).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&runner, permissions).unwrap();
    }
    ActionRunnerClient::with_executable(runner, timeout).unwrap()
}

fn fixture_request(client: &ActionRunnerClient, root: &Path) -> ActionRunnerRequest {
    request_for(
        client,
        root,
        &GovernedFilesystemOperation::InspectFile {
            relative_path: "probe.txt".into(),
            max_bytes: 100,
        },
    )
}

fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        let output = Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    }
    #[cfg(unix)]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .unwrap();
        output.status.success() && !output.stdout.is_empty()
    }
}

#[test]
fn separate_processes_create_once_then_reconcile_without_repeating_effect() {
    let root = tempfile::tempdir().unwrap();
    let create = request(
        root.path(),
        &GovernedFilesystemOperation::CreateFile {
            relative_path: "process.txt".into(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(b"one effect"),
        },
    );
    let (status, created) = invoke(&serde_json::to_vec(&create).unwrap());
    assert!(status.success());
    assert_eq!(created.outcome, RunnerEffectOutcome::EffectConfirmed);

    let inspect = request(
        root.path(),
        &GovernedFilesystemOperation::InspectFile {
            relative_path: "process.txt".into(),
            max_bytes: 100,
        },
    );
    let (status, observed) = invoke(&serde_json::to_vec(&inspect).unwrap());
    assert!(status.success());
    assert_eq!(observed.outcome, RunnerEffectOutcome::Observational);
    assert_eq!(
        std::fs::read_to_string(root.path().join("process.txt")).unwrap(),
        "one effect"
    );

    let (status, duplicate) = invoke(&serde_json::to_vec(&create).unwrap());
    assert!(status.success());
    assert_eq!(duplicate.outcome, RunnerEffectOutcome::NoEffectConfirmed);
    assert_eq!(
        std::fs::read_to_string(root.path().join("process.txt")).unwrap(),
        "one effect"
    );
}

#[test]
fn malformed_request_fails_closed_without_output_noise() {
    let (status, response) = invoke(br#"{"not":"a request"}"#);
    assert!(!status.success());
    assert_eq!(response.outcome, RunnerEffectOutcome::Denied);
    assert_eq!(response.error_code.as_deref(), Some("request_invalid"));
}

#[test]
fn packaged_qualification_mode_exercises_and_cleans_the_runner() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcg-action-runner"))
        .arg("--qualify-filesystem-root")
        .arg(root.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["created"], "effect_confirmed");
    assert_eq!(report["reconciled"], "observational");
    assert_eq!(report["duplicate"], "no_effect_confirmed");
    assert_eq!(report["compensated"], "effect_confirmed");
    assert_eq!(report["final_absent"], true);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn bounded_parent_client_validates_the_exact_child_response() {
    let root = tempfile::tempdir().unwrap();
    let client = ActionRunnerClient::with_executable(
        env!("CARGO_BIN_EXE_mcg-action-runner"),
        Duration::from_secs(5),
    )
    .unwrap();
    let create = request_for(
        &client,
        root.path(),
        &GovernedFilesystemOperation::CreateFile {
            relative_path: "client.txt".into(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(b"client effect"),
        },
    );
    let response = client.invoke(&create).await.unwrap();
    assert_eq!(response.request_id, Some(create.request_id));
    assert_eq!(response.action_id, Some(create.action_id));
    assert_eq!(response.execution_id, Some(create.execution_id));
    assert_eq!(response.outcome, RunnerEffectOutcome::EffectConfirmed);
    assert_eq!(
        std::fs::read(root.path().join("client.txt")).unwrap(),
        b"client effect"
    );

    assert!(!ActionRunnerClientError::Launch.effect_may_have_occurred());
    assert!(ActionRunnerClientError::Timeout.effect_may_have_occurred());
}

#[tokio::test]
async fn parent_refuses_runner_replacement_after_identity_measurement() {
    let root = tempfile::tempdir().unwrap();
    let runner = root.path().join(if cfg!(windows) {
        "mcg-action-runner.exe"
    } else {
        "mcg-action-runner"
    });
    std::fs::copy(env!("CARGO_BIN_EXE_mcg-action-runner"), &runner).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(&runner).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&runner, permissions).unwrap();
    }
    let client = ActionRunnerClient::with_executable(&runner, Duration::from_secs(5)).unwrap();
    let request = fixture_request(&client, root.path());
    std::fs::copy(
        env!("CARGO_BIN_EXE_mcg-action-runner-fault-fixture"),
        &runner,
    )
    .unwrap();

    let error = client.invoke(&request).await.unwrap_err();
    assert!(matches!(error, ActionRunnerClientError::Unavailable));
    assert!(!error.effect_may_have_occurred());
    assert!(!root.path().join("effect.txt").exists());
}

#[tokio::test]
async fn child_exit_before_effect_is_unknown_but_leaves_no_effect() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fault-mode.txt"), "exit-before-effect").unwrap();
    let client = fault_client(root.path(), Duration::from_secs(2));

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::InvalidResponse));
    assert!(error.effect_may_have_occurred());
    assert!(!root.path().join("effect.txt").exists());
}

#[tokio::test]
async fn lost_response_after_effect_preserves_unknown_outcome_evidence() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fault-mode.txt"), "effect-then-exit").unwrap();
    let client = fault_client(root.path(), Duration::from_secs(2));

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::InvalidResponse));
    assert!(error.effect_may_have_occurred());
    assert_eq!(
        std::fs::read(root.path().join("effect.txt")).unwrap(),
        b"effect occurred"
    );
}

#[tokio::test]
async fn timeout_kills_reaps_and_rejects_late_effects() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fault-mode.txt"), "timeout-before-effect").unwrap();
    let client = fault_client(root.path(), Duration::from_millis(100));
    let request = fixture_request(&client, root.path());
    let started = Instant::now();

    let error = client.invoke(&request).await.unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(3));
    let pid = std::fs::read_to_string(root.path().join("fixture-pid.txt"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(!process_exists(pid), "timed-out child must be reaped");
    std::thread::sleep(Duration::from_millis(150));
    assert!(!root.path().join("effect.txt").exists());
    assert!(!root.path().join("late-effect.txt").exists());
}

#[tokio::test]
async fn timeout_after_effect_is_unknown_and_still_stops_late_work() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fault-mode.txt"), "effect-then-timeout").unwrap();
    let client = fault_client(root.path(), Duration::from_millis(100));

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::Timeout));
    assert!(error.effect_may_have_occurred());
    assert_eq!(
        std::fs::read(root.path().join("effect.txt")).unwrap(),
        b"effect occurred"
    );
    let pid = std::fs::read_to_string(root.path().join("fixture-pid.txt"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(!process_exists(pid), "timed-out child must be reaped");
    std::thread::sleep(Duration::from_millis(150));
    assert!(!root.path().join("late-effect.txt").exists());
}

#[tokio::test]
async fn bounded_parent_rejects_oversized_or_diagnostic_child_output() {
    for (mode, expected) in [
        (
            "oversized-response",
            ActionRunnerClientError::ResponseTooLarge,
        ),
        (
            "oversized-diagnostic",
            ActionRunnerClientError::ResponseTooLarge,
        ),
        (
            "diagnostic-output",
            ActionRunnerClientError::DiagnosticOutput,
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("fault-mode.txt"), mode).unwrap();
        let client = fault_client(root.path(), Duration::from_secs(2));

        let error = client
            .invoke(&fixture_request(&client, root.path()))
            .await
            .unwrap_err();

        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&expected)
        );
        assert!(error.effect_may_have_occurred());
    }
}

#[tokio::test]
async fn bounded_parent_rejects_malformed_real_child_response() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fault-mode.txt"), "malformed-response").unwrap();
    let client = fault_client(root.path(), Duration::from_secs(2));

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::InvalidResponse));
    assert!(error.effect_may_have_occurred());
}

#[tokio::test]
async fn timeout_terminates_the_entire_child_process_tree() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("fault-mode.txt"),
        "descendant-then-timeout",
    )
    .unwrap();
    let client = fault_client(root.path(), Duration::from_millis(250));

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::Timeout));
    let pid = std::fs::read_to_string(root.path().join("descendant-pid.txt"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(
        !process_exists(pid),
        "descendant must leave with its process group"
    );
    std::thread::sleep(Duration::from_millis(150));
    assert!(!root.path().join("descendant-late-effect.txt").exists());
}

#[tokio::test]
async fn deadline_still_applies_after_the_process_leader_exits() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("fault-mode.txt"),
        "descendant-outlives-leader",
    )
    .unwrap();
    let client = fault_client(root.path(), Duration::from_millis(250));
    let started = Instant::now();

    let error = client
        .invoke(&fixture_request(&client, root.path()))
        .await
        .unwrap_err();

    assert!(matches!(error, ActionRunnerClientError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(3));
    let pid = std::fs::read_to_string(root.path().join("descendant-pid.txt"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(!process_exists(pid), "group wait must include descendants");
    std::thread::sleep(Duration::from_millis(150));
    assert!(!root.path().join("descendant-late-effect.txt").exists());
}
