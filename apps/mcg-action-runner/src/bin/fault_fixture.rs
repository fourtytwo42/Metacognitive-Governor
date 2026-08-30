//! Process-failure fixture for integration tests.
//!
//! This binary is deliberately not a supported sidecar. Release packaging copies only the
//! fixed `mcg-action-runner` executable. Tests copy this executable into an isolated temporary
//! directory under that fixed name so the production client validates and supervises it.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;
use std::time::Duration;

use mcg_action_runner::{
    ACTION_RUNNER_REQUEST_MAX_BYTES, ACTION_RUNNER_RESPONSE_MAX_BYTES, ActionRunnerRequest,
    execute_request,
};

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--descendant")) {
        let Some(root) = arguments.next() else {
            return ExitCode::from(2);
        };
        return descendant(std::path::Path::new(&root));
    }
    let mut request_bytes = Vec::new();
    if std::io::stdin()
        .take(u64::try_from(ACTION_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut request_bytes)
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let Ok(request) = serde_json::from_slice::<ActionRunnerRequest>(&request_bytes) else {
        return ExitCode::FAILURE;
    };
    let root = request.capability_root;
    let mode = std::fs::read_to_string(root.join("fault-mode.txt")).unwrap_or_default();
    let _ = std::fs::write(root.join("fixture-pid.txt"), std::process::id().to_string());

    match mode.trim() {
        "exit-before-effect" => ExitCode::FAILURE,
        "effect-then-exit" => {
            let _ = std::fs::write(root.join("effect.txt"), b"effect occurred");
            ExitCode::FAILURE
        }
        "timeout-before-effect" => {
            std::thread::sleep(Duration::from_secs(2));
            let _ = std::fs::write(root.join("late-effect.txt"), b"must never occur");
            ExitCode::SUCCESS
        }
        "effect-then-timeout" => {
            let _ = std::fs::write(root.join("effect.txt"), b"effect occurred");
            std::thread::sleep(Duration::from_secs(2));
            let _ = std::fs::write(root.join("late-effect.txt"), b"must never occur");
            ExitCode::SUCCESS
        }
        "oversized-response" => {
            let bytes = vec![b'x'; ACTION_RUNNER_RESPONSE_MAX_BYTES + 1];
            if std::io::stdout().write_all(&bytes).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "oversized-diagnostic" => {
            let diagnostic = vec![b'x'; 4 * 1024 + 1];
            if std::io::stderr().write_all(&diagnostic).is_err() {
                return ExitCode::FAILURE;
            }
            write_valid_response(&request_bytes)
        }
        "diagnostic-output" => {
            if std::io::stderr().write_all(b"redacted diagnostic").is_err() {
                return ExitCode::FAILURE;
            }
            write_valid_response(&request_bytes)
        }
        "malformed-response" => {
            if std::io::stdout().write_all(b"{}").is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "descendant-then-timeout" => {
            if spawn_descendant(&root, true).is_err() {
                return ExitCode::FAILURE;
            }
            std::thread::sleep(Duration::from_secs(2));
            ExitCode::SUCCESS
        }
        "descendant-outlives-leader" => {
            if spawn_descendant(&root, false).is_err() {
                return ExitCode::FAILURE;
            }
            if std::io::stdout().write_all(b"{}").is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        _ => ExitCode::from(2),
    }
}

fn spawn_descendant(root: &std::path::Path, isolate_stdio: bool) -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.arg("--descendant").arg(root);
    if isolate_stdio {
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    command.spawn().map(|_| ())
}

fn descendant(root: &std::path::Path) -> ExitCode {
    if std::fs::write(
        root.join("descendant-pid.txt"),
        std::process::id().to_string(),
    )
    .is_err()
    {
        return ExitCode::FAILURE;
    }
    std::thread::sleep(Duration::from_secs(5));
    if std::fs::write(root.join("descendant-late-effect.txt"), b"must never occur").is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn write_valid_response(request_bytes: &[u8]) -> ExitCode {
    let response = execute_request(request_bytes);
    let Ok(bytes) = serde_json::to_vec(&response) else {
        return ExitCode::FAILURE;
    };
    if std::io::stdout().write_all(&bytes).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
