use std::process::ExitCode;

use mcg_action_runner::{
    ACTION_RUNNER_REQUEST_MAX_BYTES, RunnerEffectOutcome, execute_request,
    qualify_filesystem_runner,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !arguments.is_empty() {
        return qualify(&arguments).await;
    }
    let mut request = Vec::new();
    let read = tokio::io::stdin()
        .take(u64::try_from(ACTION_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut request)
        .await;
    let response = match read {
        Ok(_) => execute_request(&request),
        Err(_) => execute_request(b""),
    };
    let succeeded = matches!(
        response.outcome,
        RunnerEffectOutcome::Observational
            | RunnerEffectOutcome::NoEffectConfirmed
            | RunnerEffectOutcome::EffectConfirmed
    );
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

async fn qualify(arguments: &[std::ffi::OsString]) -> ExitCode {
    let (succeeded, output) = if arguments.len() == 2 && arguments[0] == "--qualify-filesystem-root"
    {
        match qualify_filesystem_runner(std::path::Path::new(&arguments[1])) {
            Ok(report) => (
                true,
                serde_json::to_vec(&report).unwrap_or_else(|_| {
                    br#"{"qualified":false,"error":"response_serialization_failed"}"#.to_vec()
                }),
            ),
            Err(error) => (
                false,
                serde_json::to_vec(&serde_json::json!({
                    "qualified": false,
                    "error": error,
                }))
                .unwrap_or_else(|_| {
                    br#"{"qualified":false,"error":"response_serialization_failed"}"#.to_vec()
                }),
            ),
        }
    } else {
        (
            false,
            br#"{"qualified":false,"error":"usage: mcg-action-runner --qualify-filesystem-root <empty-absolute-directory>"}"#.to_vec(),
        )
    };
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
