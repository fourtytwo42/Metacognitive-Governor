//! One-shot process contract for sandboxed plugin execution.

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mcg_plugins::{
    PLUGIN_SCHEMA_VERSION, PluginManifest, PluginResult, WasiPluginExecutor, WasiPluginLimits,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _},
    process::Command,
    time::Instant,
};

pub const RUNNER_REQUEST_MAX_BYTES: usize = 32 * 1_024 * 1_024;
const RUNNER_DETAIL_MAX_BYTES: usize = 2_048;
const RUNNER_RESPONSE_MAX_BYTES: usize = 2 * 1_024 * 1_024;
const RUNNER_STDERR_MAX_BYTES: usize = 4 * 1_024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRunnerRequest {
    pub schema_version: u16,
    pub manifest: PluginManifest,
    pub module_sha256: String,
    pub module_base64: String,
    pub input: Value,
    #[serde(default)]
    pub limits: WasiPluginLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRunnerState {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRunnerResponse {
    pub schema_version: u16,
    pub plugin_id: Option<String>,
    pub state: PluginRunnerState,
    pub result: Option<PluginResult>,
    pub error_code: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginRunnerClientError {
    #[error("plugin runner is unavailable")]
    Unavailable,
    #[error("plugin runner request is too large")]
    RequestTooLarge,
    #[error("plugin runner timed out")]
    Timeout,
    #[error("plugin runner response exceeded its bound")]
    ResponseTooLarge,
    #[error("plugin runner wrote to its diagnostic channel")]
    DiagnosticOutput,
    #[error("plugin runner returned an invalid response")]
    InvalidResponse,
    #[error("plugin runner process failed")]
    Process,
}

#[derive(Debug, Clone)]
pub struct PluginRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
}

impl PluginRunnerClient {
    /// Selects only the fixed runner name adjacent to a trusted application binary.
    ///
    /// # Errors
    /// Returns unavailable for a missing, linked, non-file, or wrongly named runner.
    pub fn adjacent_to(
        application_executable: impl AsRef<Path>,
    ) -> Result<Self, PluginRunnerClientError> {
        let parent = application_executable
            .as_ref()
            .parent()
            .ok_or(PluginRunnerClientError::Unavailable)?;
        Self::with_executable(parent.join(runner_file_name()), Duration::from_secs(130))
    }

    /// Creates a client for an exact runner path. Intended for a trusted package
    /// bootstrapper and process-contract tests, not user-selected executables.
    ///
    /// # Errors
    /// Returns unavailable unless the path is a real, non-symlink regular file
    /// with the fixed platform runner name and a bounded outer deadline.
    pub fn with_executable(
        executable: impl AsRef<Path>,
        outer_timeout: Duration,
    ) -> Result<Self, PluginRunnerClientError> {
        let executable = executable.as_ref();
        if executable.file_name() != Some(std::ffi::OsStr::new(runner_file_name()))
            || outer_timeout.is_zero()
            || outer_timeout > Duration::from_secs(300)
        {
            return Err(PluginRunnerClientError::Unavailable);
        }
        let metadata = std::fs::symlink_metadata(executable)
            .map_err(|_| PluginRunnerClientError::Unavailable)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(PluginRunnerClientError::Unavailable);
        }
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
        })
    }

    /// Executes exactly one request in a cleared-environment child process.
    ///
    /// # Errors
    /// Returns bounded launch, pipe, timeout, exit, or response-contract failures.
    pub async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
        let expected_plugin_id = request.manifest.id.clone();
        let request =
            serde_json::to_vec(request).map_err(|_| PluginRunnerClientError::InvalidResponse)?;
        if request.len() > RUNNER_REQUEST_MAX_BYTES {
            return Err(PluginRunnerClientError::RequestTooLarge);
        }
        let mut command = Command::new(&self.executable);
        command
            .env_clear()
            .current_dir(
                self.executable
                    .parent()
                    .ok_or(PluginRunnerClientError::Unavailable)?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|_| PluginRunnerClientError::Process)?;
        let (Some(mut stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(PluginRunnerClientError::Process);
        };
        let stdout_task = tokio::spawn(read_bounded(stdout, RUNNER_RESPONSE_MAX_BYTES));
        let stderr_task = tokio::spawn(read_bounded(stderr, RUNNER_STDERR_MAX_BYTES));
        let deadline = Instant::now() + self.outer_timeout;
        match tokio::time::timeout_at(deadline, async {
            stdin.write_all(&request).await?;
            stdin.shutdown().await
        })
        .await
        {
            Ok(Ok(())) => {}
            write_result => {
                drop(stdin);
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(if write_result.is_err() {
                    PluginRunnerClientError::Timeout
                } else {
                    PluginRunnerClientError::Process
                });
            }
        }
        drop(stdin);
        let status = match tokio::time::timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(PluginRunnerClientError::Process);
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(PluginRunnerClientError::Timeout);
            }
        };
        let (stdout, stdout_overflow) = stdout_task
            .await
            .map_err(|_| PluginRunnerClientError::Process)?
            .map_err(|_| PluginRunnerClientError::Process)?;
        let (stderr, stderr_overflow) = stderr_task
            .await
            .map_err(|_| PluginRunnerClientError::Process)?
            .map_err(|_| PluginRunnerClientError::Process)?;
        if stdout_overflow || stderr_overflow {
            return Err(PluginRunnerClientError::ResponseTooLarge);
        }
        if !stderr.is_empty() {
            return Err(PluginRunnerClientError::DiagnosticOutput);
        }
        validate_response(status.success(), &expected_plugin_id, &stdout)
    }
}

impl PluginRunnerResponse {
    #[must_use]
    pub fn failed(
        plugin_id: Option<String>,
        error_code: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id,
            state: PluginRunnerState::Failed,
            result: None,
            error_code: Some(error_code.into()),
            detail: Some(bounded_detail(detail.into())),
        }
    }
}

/// Parses, verifies, and executes one complete runner request.
///
/// # Errors
/// This function represents failures in the returned versioned response rather
/// than exposing an unstructured error channel.
pub async fn execute_request(request_bytes: &[u8]) -> PluginRunnerResponse {
    if request_bytes.len() > RUNNER_REQUEST_MAX_BYTES {
        return PluginRunnerResponse::failed(
            None,
            "request_too_large",
            "runner request is too large",
        );
    }
    let Ok(request) = serde_json::from_slice::<PluginRunnerRequest>(request_bytes) else {
        return PluginRunnerResponse::failed(
            None,
            "invalid_request",
            "runner request is not one valid JSON document",
        );
    };
    let plugin_id = Some(request.manifest.id.clone());
    if request.schema_version != PLUGIN_SCHEMA_VERSION {
        return PluginRunnerResponse::failed(
            plugin_id,
            "unsupported_schema",
            "runner request schema is unsupported",
        );
    }
    let Ok(module) = STANDARD.decode(request.module_base64.as_bytes()) else {
        return PluginRunnerResponse::failed(
            plugin_id,
            "invalid_module_encoding",
            "plugin module is not canonical base64",
        );
    };
    if STANDARD.encode(&module) != request.module_base64 {
        return PluginRunnerResponse::failed(
            plugin_id,
            "invalid_module_encoding",
            "plugin module is not canonical base64",
        );
    }
    if request.module_sha256.len() != 64
        || !request
            .module_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || sha256_hex(&module) != request.module_sha256.to_ascii_lowercase()
    {
        return PluginRunnerResponse::failed(
            plugin_id,
            "module_digest_mismatch",
            "plugin module digest does not match its request",
        );
    }
    let executor = match WasiPluginExecutor::new(request.limits) {
        Ok(executor) => executor,
        Err(error) => {
            return PluginRunnerResponse::failed(plugin_id, "invalid_limits", error.to_string());
        }
    };
    match executor
        .invoke_json(&request.manifest, &module, &request.input)
        .await
    {
        Ok(result) => PluginRunnerResponse {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id,
            state: PluginRunnerState::Succeeded,
            result: Some(result),
            error_code: None,
            detail: None,
        },
        Err(error) => {
            PluginRunnerResponse::failed(plugin_id, "plugin_execution_failed", error.to_string())
        }
    }
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("write to string");
    }
    encoded
}

fn bounded_detail(mut detail: String) -> String {
    if detail.len() <= RUNNER_DETAIL_MAX_BYTES {
        return detail;
    }
    let mut boundary = RUNNER_DETAIL_MAX_BYTES;
    while !detail.is_char_boundary(boundary) {
        boundary -= 1;
    }
    detail.truncate(boundary);
    detail
}

async fn read_bounded<R: AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::new();
    let mut overflow = false;
    let mut buffer = [0_u8; 8 * 1_024];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok((retained, overflow));
        }
        let remaining = limit.saturating_sub(retained.len());
        let keep = read.min(remaining);
        retained.extend_from_slice(&buffer[..keep]);
        overflow |= keep < read;
    }
}

fn validate_response(
    status_success: bool,
    expected_plugin_id: &str,
    stdout: &[u8],
) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
    let response: PluginRunnerResponse =
        serde_json::from_slice(stdout).map_err(|_| PluginRunnerClientError::InvalidResponse)?;
    let response_succeeded = response.state == PluginRunnerState::Succeeded;
    if status_success != response_succeeded
        || response.schema_version != PLUGIN_SCHEMA_VERSION
        || response.plugin_id.as_deref() != Some(expected_plugin_id)
        || (response_succeeded
            && (response.result.is_none()
                || response.error_code.is_some()
                || response.detail.is_some()))
        || (!response_succeeded
            && (response.result.is_some()
                || response.error_code.as_deref().is_none_or(str::is_empty)))
    {
        return Err(PluginRunnerClientError::InvalidResponse);
    }
    Ok(response)
}

const fn runner_file_name() -> &'static str {
    if cfg!(windows) {
        "mcg-plugin-runner.exe"
    } else {
        "mcg-plugin-runner"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use mcg_plugins::{PluginClass, PluginManifest};

    use super::*;

    fn request(module: &[u8]) -> PluginRunnerRequest {
        PluginRunnerRequest {
            schema_version: PLUGIN_SCHEMA_VERSION,
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.runner".into(),
                display_name: "Runner fixture".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: sha256_hex(module),
            module_base64: STANDARD.encode(module),
            input: json!({}),
            limits: WasiPluginLimits::default(),
        }
    }

    #[tokio::test]
    async fn digest_mismatch_fails_before_execution() {
        let module = br#"(module (func (export "_start")))"#;
        let mut request = request(module);
        request.module_sha256 = "0".repeat(64);
        let response = execute_request(&serde_json::to_vec(&request).expect("request")).await;
        assert_eq!(response.state, PluginRunnerState::Failed);
        assert_eq!(
            response.error_code.as_deref(),
            Some("module_digest_mismatch")
        );
    }

    #[tokio::test]
    async fn malformed_and_oversized_requests_are_bounded() {
        let malformed = execute_request(b"not-json").await;
        assert_eq!(malformed.error_code.as_deref(), Some("invalid_request"));
        let oversized = execute_request(&vec![b'x'; RUNNER_REQUEST_MAX_BYTES + 1]).await;
        assert_eq!(oversized.error_code.as_deref(), Some("request_too_large"));
    }

    #[test]
    fn parent_rejects_inconsistent_or_mismatched_responses() {
        let valid = PluginRunnerResponse {
            schema_version: PLUGIN_SCHEMA_VERSION,
            plugin_id: Some("example.runner".into()),
            state: PluginRunnerState::Succeeded,
            result: Some(PluginResult {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: "example.runner".into(),
                output: json!({"ok": true}),
            }),
            error_code: None,
            detail: None,
        };
        let encoded = serde_json::to_vec(&valid).expect("response");
        assert!(validate_response(true, "example.runner", &encoded).is_ok());
        assert!(matches!(
            validate_response(false, "example.runner", &encoded),
            Err(PluginRunnerClientError::InvalidResponse)
        ));
        assert!(matches!(
            validate_response(true, "another.plugin", &encoded),
            Err(PluginRunnerClientError::InvalidResponse)
        ));

        let mut inconsistent = valid;
        inconsistent.error_code = Some("impossible".into());
        assert!(matches!(
            validate_response(
                true,
                "example.runner",
                &serde_json::to_vec(&inconsistent).expect("response")
            ),
            Err(PluginRunnerClientError::InvalidResponse)
        ));
        assert!(matches!(
            validate_response(true, "example.runner", b"{}{}"),
            Err(PluginRunnerClientError::InvalidResponse)
        ));
    }
}
