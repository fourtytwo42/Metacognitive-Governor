use std::{fs, path::PathBuf, time::Duration};

use mcg_process_runner::{
    GovernedProcessOperation, ProcessRunnerClient, ProcessRunnerClientError, ProcessRunnerOutcome,
    decode_governed_process_observation, executable_sha256, qualification_request_with_evidence,
};

fn command(exits_late: bool) -> (PathBuf, Vec<String>) {
    #[cfg(windows)]
    {
        if exits_late {
            let executable = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            return (
                executable,
                vec![
                    "/d".into(),
                    "/c".into(),
                    r"C:\Windows\System32\ping.exe -n 3 127.0.0.1 >nul & echo late>late.txt".into(),
                ],
            );
        }
        (
            PathBuf::from(r"C:\Windows\System32\cmd.exe"),
            vec!["/d".into(), "/c".into(), "echo governed".into()],
        )
    }
    #[cfg(unix)]
    {
        if exits_late {
            return (
                PathBuf::from("/bin/sh"),
                vec!["-c".into(), "sleep 0.8; printf late > late.txt".into()],
            );
        }
        (
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), "printf governed".into()],
        )
    }
}

fn operation(exits_late: bool, timeout_ms: u64) -> GovernedProcessOperation {
    let (executable, argv) = command(exits_late);
    GovernedProcessOperation {
        executable: executable.clone(),
        executable_sha256: executable_sha256(&executable).expect("measure command"),
        argv,
        cwd_relative: ".".into(),
        environment: Vec::new(),
        stdin_base64: None,
        timeout_ms,
        max_output_bytes: 64 * 1024,
    }
}

#[tokio::test]
async fn real_client_executes_one_exact_request() {
    let root = tempfile::tempdir().expect("project root");
    let runner = PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client = ProcessRunnerClient::with_executable(&runner, Duration::from_secs(10))
        .expect("process runner client");
    let request = qualification_request_with_evidence(
        root.path(),
        &operation(false, 5_000),
        client.capability_evidence(),
    )
    .expect("qualification request");
    let response = client.invoke(&request).await.expect("runner response");
    assert_eq!(response.outcome, ProcessRunnerOutcome::EffectConfirmed);
    let observation = decode_governed_process_observation(
        response.observation.as_ref().expect("process observation"),
    )
    .expect("decode observation");
    assert_eq!(observation.exit_code, Some(0));
    let stdout = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        observation.stdout_base64,
    )
    .expect("decode stdout");
    assert!(String::from_utf8_lossy(&stdout).contains("governed"));
}

#[tokio::test]
async fn timeout_terminates_late_work_and_is_never_reported_as_no_effect() {
    let root = tempfile::tempdir().expect("project root");
    let runner = PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client = ProcessRunnerClient::with_executable(&runner, Duration::from_secs(10))
        .expect("process runner client");
    let request = qualification_request_with_evidence(
        root.path(),
        &operation(true, 100),
        client.capability_evidence(),
    )
    .expect("qualification request");
    let response = client
        .invoke(&request)
        .await
        .expect("bounded timeout response");
    assert_eq!(response.outcome, ProcessRunnerOutcome::OutcomeUnknown);
    assert_eq!(response.error_code.as_deref(), Some("process_timeout"));
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    assert!(
        !root.path().join("late.txt").exists(),
        "terminated process work must not escape after the runner responds"
    );
}

#[tokio::test]
async fn changed_runner_identity_is_rejected_before_process_launch() {
    let root = tempfile::tempdir().expect("project root");
    let runner_root = tempfile::tempdir().expect("runner root");
    let packaged_runner = PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let copied_runner = runner_root.path().join(
        packaged_runner
            .file_name()
            .expect("packaged process runner filename"),
    );
    fs::copy(&packaged_runner, &copied_runner).expect("copy process runner");
    let client = ProcessRunnerClient::with_executable(&copied_runner, Duration::from_secs(10))
        .expect("process runner client");
    let request = qualification_request_with_evidence(
        root.path(),
        &operation(false, 5_000),
        client.capability_evidence(),
    )
    .expect("qualification request");
    fs::write(&copied_runner, b"substituted runner").expect("substitute process runner");

    assert_eq!(
        client.invoke(&request).await,
        Err(ProcessRunnerClientError::ExecutableChanged)
    );
}
