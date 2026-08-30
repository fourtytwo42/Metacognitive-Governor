//! Unpackaged child-process fixture for Protected Root integration tests.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;
use std::time::Duration;

use chrono::Utc;
use mcg_action_runner::{
    ACTION_RUNNER_PROTOCOL_VERSION, ACTION_RUNNER_REQUEST_MAX_BYTES, ActionRunnerRequest,
    ActionRunnerResponse, GOVERNED_FILESYSTEM_OBSERVATION_TYPE, GovernedFilesystemOperation,
    RunnerEffectOutcome, decode_governed_filesystem_operation,
};
use mcg_contracts::{StableTypeId, VersionedPayload};
use sha2::{Digest as _, Sha256};

fn main() -> ExitCode {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(u64::try_from(ACTION_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let Ok(request) = serde_json::from_slice::<ActionRunnerRequest>(&bytes) else {
        return ExitCode::FAILURE;
    };
    let mode =
        std::fs::read_to_string(request.capability_root.join("fault-mode.txt")).unwrap_or_default();
    match mode.trim() {
        "exit-before-effect" => ExitCode::FAILURE,
        "effect-then-exit" => {
            if apply_expected_create(&request).is_err() {
                return ExitCode::from(2);
            }
            ExitCode::FAILURE
        }
        "timeout-before-effect" => {
            std::thread::sleep(Duration::from_secs(2));
            ExitCode::SUCCESS
        }
        "effect-then-timeout" => {
            if apply_expected_create(&request).is_err() {
                return ExitCode::from(2);
            }
            std::thread::sleep(Duration::from_secs(2));
            ExitCode::SUCCESS
        }
        "inspect" => inspect(&request),
        _ => ExitCode::from(2),
    }
}

fn apply_expected_create(request: &ActionRunnerRequest) -> Result<(), ()> {
    let operation = decode_governed_filesystem_operation(&request.operation).map_err(|_| ())?;
    let GovernedFilesystemOperation::CreateFile {
        relative_path,
        content_base64,
    } = operation
    else {
        return Err(());
    };
    if content_base64 != "YXV0aG9yaXplZA==" || relative_path != "authorized.txt" {
        return Err(());
    }
    std::fs::write(request.capability_root.join(relative_path), b"authorized").map_err(|_| ())
}

fn inspect(request: &ActionRunnerRequest) -> ExitCode {
    let Ok(GovernedFilesystemOperation::InspectFile { relative_path, .. }) =
        decode_governed_filesystem_operation(&request.operation)
    else {
        return ExitCode::from(2);
    };
    let value = std::fs::read(request.capability_root.join(&relative_path)).ok();
    let Ok(observation) = VersionedPayload::canonical_v1(
        StableTypeId::parse(GOVERNED_FILESYSTEM_OBSERVATION_TYPE).expect("fixed observation type"),
        &serde_json::json!({
            "relative_path": relative_path,
            "exists": value.is_some(),
            "sha256": value.as_ref().map(|bytes| format!("{:x}", Sha256::digest(bytes))),
            "size_bytes": value.as_ref().map(Vec::len),
            "content_base64": null,
        }),
    ) else {
        return ExitCode::from(2);
    };
    let now = Utc::now();
    let response = ActionRunnerResponse {
        protocol_version: ACTION_RUNNER_PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        action_id: Some(request.action_id),
        execution_id: Some(request.execution_id),
        outcome: RunnerEffectOutcome::Observational,
        observation: Some(observation),
        error_code: None,
        detail: "fault fixture inspection".into(),
        started_at: now,
        finished_at: now,
    };
    let Ok(bytes) = serde_json::to_vec(&response) else {
        return ExitCode::from(2);
    };
    if std::io::stdout().write_all(&bytes).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
