use std::process::ExitCode;

use mcg_plugin_runner::{
    PluginRunnerResponse, PluginRunnerState, RUNNER_REQUEST_MAX_BYTES, execute_request,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[tokio::main]
async fn main() -> ExitCode {
    let mut request = Vec::new();
    let read_result = tokio::io::stdin()
        .take((RUNNER_REQUEST_MAX_BYTES + 1) as u64)
        .read_to_end(&mut request)
        .await;
    let response = match read_result {
        Ok(_) => execute_request(&request).await,
        Err(_) => PluginRunnerResponse::failed(
            None,
            "request_read_failed",
            "could not read runner request",
        ),
    };
    let succeeded = response.state == PluginRunnerState::Succeeded;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"schema_version":1,"plugin_id":null,"state":"failed","result":null,"error_code":"response_serialization_failed","detail":"could not encode runner response"}"#.to_vec()
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
