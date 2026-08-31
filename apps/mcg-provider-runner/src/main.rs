use std::process::ExitCode;

use mcg_provider_runner::{
    PROVIDER_RUNNER_REQUEST_MAX_BYTES, ProviderRunnerOutcome, execute_request,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 1 && arguments[0] == "--qualify-provider-runner" {
        let report = serde_json::json!({
            "protocol_version": mcg_provider_runner::PROVIDER_RUNNER_PROTOCOL_VERSION,
            "role": "provider_adapter",
            "storage_linked": false,
            "credential_store": "operating_system",
            "physical_retry_attempts": 1,
        });
        println!("{report}");
        return ExitCode::SUCCESS;
    }
    if !arguments.is_empty() {
        return ExitCode::FAILURE;
    }
    let mut request = Vec::new();
    let read = tokio::io::stdin()
        .take(u64::try_from(PROVIDER_RUNNER_REQUEST_MAX_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut request)
        .await;
    let response = match read {
        Ok(_) => execute_request(&request).await,
        Err(_) => execute_request(b"").await,
    };
    let succeeded = response.outcome == ProviderRunnerOutcome::Completed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"protocol_version":1,"request_id":null,"invocation_id":null,"permit_id":null,"outcome":"denied","response":null,"error_code":"response_serialization_failed","detail":"provider runner response serialization failed"}"#.to_vec()
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
