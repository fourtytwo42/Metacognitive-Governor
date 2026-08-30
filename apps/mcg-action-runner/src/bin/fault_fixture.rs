//! Process-failure fixture for integration tests.
//!
//! This binary is deliberately not a supported sidecar. Release packaging copies only the
//! fixed `mcg-action-runner` executable. Tests copy this executable into an isolated temporary
//! directory under that fixed name so the production client validates and supervises it.

use std::io::Read as _;
use std::process::ExitCode;
use std::time::Duration;

use mcg_action_runner::{ACTION_RUNNER_REQUEST_MAX_BYTES, ActionRunnerRequest};

fn main() -> ExitCode {
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
        _ => ExitCode::from(2),
    }
}
