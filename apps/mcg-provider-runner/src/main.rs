use std::process::ExitCode;

use futures_util::StreamExt as _;
use mcg_provider_runner::{
    PROVIDER_RUNNER_PROTOCOL_VERSION, PROVIDER_RUNNER_REQUEST_MAX_BYTES,
    PreparedProviderRunnerStream, ProviderRunnerExecution, ProviderRunnerOutcome,
    ProviderRunnerRequest, ProviderRunnerResponse, ProviderRunnerStreamFrame, execute_request,
    execute_stream_request, write_stream_frame,
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
            "streaming": "length_prefixed_correlated_frames",
            "stream_cancellation": "kill_on_drop_process_tree",
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
    let request = if read.is_ok() { request } else { Vec::new() };
    let execution = serde_json::from_slice::<ProviderRunnerRequest>(&request)
        .map(|request| request.execution)
        .unwrap_or_default();
    if execution == ProviderRunnerExecution::Stream {
        return execute_stream(&request).await;
    }
    let response = execute_request(&request).await;
    let succeeded = response.outcome == ProviderRunnerOutcome::Completed;
    let output = serde_json::to_vec(&response).unwrap_or_else(|_| {
        format!(r#"{{"protocol_version":{PROVIDER_RUNNER_PROTOCOL_VERSION},"request_id":null,"invocation_id":null,"permit_id":null,"outcome":"denied","response":null,"error_code":"response_serialization_failed","detail":"provider runner response serialization failed"}}"#).into_bytes()
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

async fn execute_stream(request: &[u8]) -> ExitCode {
    let mut stdout = tokio::io::stdout();
    let mut total_bytes = 0_usize;
    let prepared = match execute_stream_request(request).await {
        Ok(prepared) => prepared,
        Err(response) => {
            return write_failed_stream_terminal(&mut stdout, response, &mut total_bytes).await;
        }
    };
    run_prepared_stream(&mut stdout, prepared, &mut total_bytes).await
}

async fn run_prepared_stream<W: tokio::io::AsyncWrite + Unpin>(
    stdout: &mut W,
    mut prepared: PreparedProviderRunnerStream,
    total_bytes: &mut usize,
) -> ExitCode {
    let mut sequence = 0_u64;
    while let Some(event) = prepared.stream.next().await {
        if let Ok(event) = event {
            let completed = matches!(event, mcg_providers::ModelStreamEvent::Completed { .. });
            let frame = ProviderRunnerStreamFrame::Event {
                protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
                request_id: prepared.request_id,
                invocation_id: prepared.invocation_id,
                permit_id: prepared.permit_id,
                sequence,
                event,
            };
            if write_stream_frame(stdout, &frame, total_bytes)
                .await
                .is_err()
            {
                return ExitCode::FAILURE;
            }
            sequence = sequence.saturating_add(1);
            if completed {
                let terminal = ProviderRunnerStreamFrame::Terminal {
                    protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
                    request_id: prepared.request_id,
                    invocation_id: prepared.invocation_id,
                    permit_id: prepared.permit_id,
                    sequence,
                    outcome: ProviderRunnerOutcome::Completed,
                    error_code: None,
                    detail: "provider stream completed".into(),
                };
                return if write_stream_frame(stdout, &terminal, total_bytes)
                    .await
                    .is_ok()
                {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
            }
            continue;
        }
        let terminal = ProviderRunnerStreamFrame::Terminal {
            protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
            request_id: prepared.request_id,
            invocation_id: prepared.invocation_id,
            permit_id: prepared.permit_id,
            sequence,
            outcome: ProviderRunnerOutcome::OutcomeUnknown,
            error_code: Some("provider_stream_failed".into()),
            detail: "provider stream failed after dispatch".into(),
        };
        let _ = write_stream_frame(stdout, &terminal, total_bytes).await;
        return ExitCode::FAILURE;
    }
    let terminal = ProviderRunnerStreamFrame::Terminal {
        protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
        request_id: prepared.request_id,
        invocation_id: prepared.invocation_id,
        permit_id: prepared.permit_id,
        sequence,
        outcome: ProviderRunnerOutcome::OutcomeUnknown,
        error_code: Some("provider_stream_incomplete".into()),
        detail: "provider stream ended without completion".into(),
    };
    let _ = write_stream_frame(stdout, &terminal, total_bytes).await;
    ExitCode::FAILURE
}

async fn write_failed_stream_terminal<W: tokio::io::AsyncWrite + Unpin>(
    stdout: &mut W,
    response: ProviderRunnerResponse,
    total_bytes: &mut usize,
) -> ExitCode {
    let (Some(request_id), Some(invocation_id), Some(permit_id)) = (
        response.request_id,
        response.invocation_id,
        response.permit_id,
    ) else {
        return ExitCode::FAILURE;
    };
    let terminal = ProviderRunnerStreamFrame::Terminal {
        protocol_version: PROVIDER_RUNNER_PROTOCOL_VERSION,
        request_id,
        invocation_id,
        permit_id,
        sequence: 0,
        outcome: response.outcome,
        error_code: response.error_code,
        detail: response.detail,
    };
    let _ = write_stream_frame(stdout, &terminal, total_bytes).await;
    ExitCode::FAILURE
}
