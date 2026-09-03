//! One-shot process contract for sandboxed plugin execution.

use std::{
    fmt::Write as _,
    future::Future,
    io::Read as _,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

#[cfg(feature = "executor")]
use base64::{Engine as _, engine::general_purpose::STANDARD};
#[cfg(feature = "executor")]
use chrono::{DateTime, Utc};
use mcg_contracts::{
    ArtifactRef, ContentHash, EffectClass, ExecutionPlatform, HashAlgorithm, IsolationGuarantee,
    SandboxCapabilityProfile, SourceRef, StableTypeId, ToolCapabilitySet, ToolInvocationPermit,
    ToolInvocationProposal, ToolManifest, ToolManifestId, ToolPolicy, ToolResourceLimits,
    VersionedPayload,
};
#[cfg(feature = "executor")]
use mcg_contracts::{PolicyBinding, tool_invocation_permit_hash, tool_invocation_proposal_hash};
use mcg_plugin_protocol::{PLUGIN_SCHEMA_VERSION, PluginManifest, PluginResult, WasiPluginLimits};
#[cfg(feature = "executor")]
use mcg_plugins::WasiPluginExecutor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _},
    process::{Child, Command},
    time::Instant,
};
use uuid::Uuid;

pub const RUNNER_REQUEST_MAX_BYTES: usize = 32 * 1_024 * 1_024;
const RUNNER_DETAIL_MAX_BYTES: usize = 2_048;
const RUNNER_RESPONSE_MAX_BYTES: usize = 2 * 1_024 * 1_024;
const RUNNER_STDERR_MAX_BYTES: usize = 4 * 1_024;
pub const PLUGIN_RUNNER_EXECUTABLE_SOURCE_TYPE: &str = "mcg.plugin_runner_executable.v1";
pub const PLUGIN_RUNNER_EXECUTABLE_HASH_DOMAIN: &str = "MetaCog.PluginRunnerExecutable";
pub const PLUGIN_RUNNER_INVOCATION_TYPE: &str = "mcg.plugin_runner_invocation.v1";
pub const PLUGIN_RUNNER_INVOCATION_HASH_DOMAIN: &str = "MetaCog.PluginRunnerInvocation";
pub const PLUGIN_RUNNER_INPUT_SCHEMA_TYPE: &str = "mcg.plugin_runner_input_schema.v1";
pub const PLUGIN_RUNNER_OUTPUT_SCHEMA_TYPE: &str = "mcg.plugin_runner_output_schema.v1";
pub const PLUGIN_RUNNER_RECEIPT_TYPE: &str = "mcg.plugin_runner_receipt.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRunnerAuthorization {
    pub invocation_id: String,
    pub plugin_revision_id: String,
    pub manifest: ToolManifest,
    pub proposal: ToolInvocationProposal,
    pub proposal_hash: ContentHash,
    pub policy: ToolPolicy,
    pub permit: ToolInvocationPermit,
    pub permit_hash: ContentHash,
    pub permit_ref: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PluginRunnerInvocationBinding<'a> {
    schema_version: u32,
    invocation_id: &'a str,
    plugin_revision_id: &'a str,
    plugin_manifest: &'a PluginManifest,
    module_sha256: &'a str,
    input_sha256: &'a str,
    limits: &'a WasiPluginLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRunnerRequest {
    pub schema_version: u16,
    pub manifest: PluginManifest,
    pub module_sha256: String,
    pub module_base64: String,
    pub input: Value,
    #[serde(default)]
    pub limits: WasiPluginLimits,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<PluginRunnerAuthorization>,
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
    #[error("plugin runner executable identity changed")]
    ExecutableChanged,
    #[error("plugin execution was denied: {0}")]
    Denied(String),
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
    #[error("plugin runner was cancelled and reaped")]
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PluginRunnerClient {
    executable: PathBuf,
    outer_timeout: Duration,
    runner_source: SourceRef,
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
            || outer_timeout > Duration::from_mins(5)
        {
            return Err(PluginRunnerClientError::Unavailable);
        }
        let metadata = std::fs::symlink_metadata(executable)
            .map_err(|_| PluginRunnerClientError::Unavailable)?;
        if !metadata.file_type().is_file() || is_link_or_reparse(&metadata) {
            return Err(PluginRunnerClientError::Unavailable);
        }
        Ok(Self {
            executable: executable.to_path_buf(),
            outer_timeout,
            runner_source: plugin_runner_executable_source(executable)?,
        })
    }

    /// Returns the exact content-addressed runner identity pinned by this client.
    #[must_use]
    pub fn runner_source(&self) -> &SourceRef {
        &self.runner_source
    }

    /// Executes exactly one request in a cleared-environment child process.
    ///
    /// # Errors
    /// Returns bounded launch, pipe, timeout, exit, or response-contract failures.
    pub async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
        self.invoke_with_cancellation(request, std::future::pending())
            .await
    }

    /// Executes exactly one request and explicitly terminates/reaps the child when cancellation
    /// becomes authoritative.
    ///
    /// # Errors
    /// Returns bounded launch, pipe, cancellation, timeout, exit, or response-contract failures.
    pub async fn invoke_with_cancellation<C>(
        &self,
        request: &PluginRunnerRequest,
        cancellation: C,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError>
    where
        C: Future<Output = ()>,
    {
        let (expected_plugin_id, request) = self.encode_request(request)?;
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
        tokio::pin!(cancellation);
        let write_result = {
            let write = tokio::time::timeout_at(deadline, async {
                stdin.write_all(&request).await?;
                stdin.shutdown().await
            });
            tokio::pin!(write);
            tokio::select! {
                result = &mut write => Some(result),
                () = &mut cancellation => None,
            }
        };
        let Some(write_result) = write_result else {
            drop(stdin);
            stop_runner_child(&mut child, &stdout_task, &stderr_task).await;
            return Err(PluginRunnerClientError::Cancelled);
        };
        match write_result {
            Ok(Ok(())) => {}
            write_result => {
                drop(stdin);
                stop_runner_child(&mut child, &stdout_task, &stderr_task).await;
                return Err(if write_result.is_err() {
                    PluginRunnerClientError::Timeout
                } else {
                    PluginRunnerClientError::Process
                });
            }
        }
        drop(stdin);
        let wait_result = {
            let wait = tokio::time::timeout_at(deadline, child.wait());
            tokio::pin!(wait);
            tokio::select! {
                result = &mut wait => Some(result),
                () = &mut cancellation => None,
            }
        };
        let Some(wait_result) = wait_result else {
            stop_runner_child(&mut child, &stdout_task, &stderr_task).await;
            return Err(PluginRunnerClientError::Cancelled);
        };
        let status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => {
                stop_runner_child(&mut child, &stdout_task, &stderr_task).await;
                return Err(PluginRunnerClientError::Process);
            }
            Err(_) => {
                stop_runner_child(&mut child, &stdout_task, &stderr_task).await;
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

    fn encode_request(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<(String, Vec<u8>), PluginRunnerClientError> {
        if plugin_runner_executable_source(&self.executable)
            .ok()
            .as_ref()
            != Some(&self.runner_source)
        {
            return Err(PluginRunnerClientError::ExecutableChanged);
        }
        let expected_plugin_id = request.manifest.id.clone();
        let request =
            serde_json::to_vec(request).map_err(|_| PluginRunnerClientError::InvalidResponse)?;
        if request.len() > RUNNER_REQUEST_MAX_BYTES {
            return Err(PluginRunnerClientError::RequestTooLarge);
        }
        Ok((expected_plugin_id, request))
    }
}

async fn stop_runner_child(
    child: &mut Child,
    stdout_task: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stderr_task: &tokio::task::JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout_task.abort();
    stderr_task.abort();
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
#[cfg(feature = "executor")]
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
    if let Some(authorization) = &request.authorization
        && let Err(error) =
            validate_plugin_runner_authorization(&request, authorization, Utc::now())
    {
        return PluginRunnerResponse::failed(plugin_id, "tool_authority_invalid", error);
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

/// Returns the exact platform on which the plugin runner is executing.
#[must_use]
pub const fn current_execution_platform() -> ExecutionPlatform {
    if cfg!(windows) {
        ExecutionPlatform::Windows
    } else if cfg!(target_os = "macos") {
        ExecutionPlatform::Macos
    } else {
        ExecutionPlatform::Linux
    }
}

/// Reports the closed-ambient `WASIp1` runner boundary.
///
/// The guest receives no preopened directories, sockets, environment variables,
/// arguments, terminal, secrets, or process-spawn imports. Wasmtime enforces the
/// supplied fuel, memory, table, instance, I/O, and wall-clock ceilings.
#[must_use]
pub const fn plugin_runner_sandbox_profile() -> SandboxCapabilityProfile {
    SandboxCapabilityProfile {
        filesystem_scope: IsolationGuarantee::Hard,
        network: IsolationGuarantee::Hard,
        process_tree: IsolationGuarantee::Hard,
        secrets: IsolationGuarantee::Hard,
        environment: IsolationGuarantee::Hard,
        resources: IsolationGuarantee::Hard,
        protected_paths: IsolationGuarantee::Hard,
    }
}

/// Projects the WASI execution limits into the common Tool resource vocabulary.
///
/// # Errors
/// Returns an error if a host-sized limit cannot be represented by the stable contract.
pub fn plugin_runner_resource_limits(
    limits: &WasiPluginLimits,
) -> Result<ToolResourceLimits, String> {
    limits.validate().map_err(|error| error.to_string())?;
    Ok(ToolResourceLimits {
        timeout_ms: limits.maximum_wall_time_ms,
        max_output_bytes: u64::try_from(limits.maximum_output_bytes)
            .map_err(|_| "plugin output limit is not representable".to_owned())?,
        max_memory_bytes: u64::try_from(limits.maximum_memory_bytes)
            .map_err(|_| "plugin memory limit is not representable".to_owned())?,
        max_child_processes: 0,
    })
}

/// Builds the exact immutable Tool manifest for one plugin revision and module.
///
/// The manifest grants the WASI guest no host capability. Plugin capability requests
/// are separate broker operations and are not implied by the guest manifest.
///
/// # Errors
/// Returns an error for malformed identities, digests, limits, or runner evidence.
pub fn governed_plugin_tool_manifest(
    runner: &SourceRef,
    plugin_revision_id: &str,
    plugin_manifest: &PluginManifest,
    module_sha256: &str,
    limits: &WasiPluginLimits,
    platform: ExecutionPlatform,
) -> Result<ToolManifest, String> {
    runner.validate().map_err(|error| error.to_string())?;
    plugin_manifest
        .validate()
        .map_err(|error| error.to_string())?;
    validate_hex_sha256(module_sha256, "plugin module digest")?;
    if plugin_revision_id.is_empty() || plugin_revision_id.len() > 128 {
        return Err("plugin revision identity is invalid".into());
    }
    let resources = plugin_runner_resource_limits(limits)?;
    let sandbox = plugin_runner_sandbox_profile();
    let identity_material = serde_json::to_vec(&serde_json::json!({
        "runner": runner,
        "plugin_revision_id": plugin_revision_id,
        "plugin_manifest": plugin_manifest,
        "module_sha256": module_sha256.to_ascii_lowercase(),
        "limits": limits,
        "platform": platform,
        "sandbox": sandbox,
    }))
    .map_err(|error| error.to_string())?;
    let digest = Sha256::digest(identity_material);
    let mut uuid_bytes = [0_u8; 16];
    uuid_bytes.copy_from_slice(&digest[..16]);
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
    let manifest = ToolManifest {
        id: ToolManifestId(Uuid::from_bytes(uuid_bytes)),
        version: 1,
        name: format!("plugin.{}", plugin_manifest.id),
        summary: format!(
            "Closed-ambient WASIp1 execution of plugin {} revision {}",
            plugin_manifest.id, plugin_revision_id
        ),
        runner: runner.clone(),
        input_schema: static_source_ref(
            PLUGIN_RUNNER_INPUT_SCHEMA_TYPE,
            PLUGIN_RUNNER_INVOCATION_TYPE,
            "MetaCog.PluginRunnerInputSchema",
        ),
        output_schema: static_source_ref(
            PLUGIN_RUNNER_OUTPUT_SCHEMA_TYPE,
            "mcg.plugin_result.v1",
            "MetaCog.PluginRunnerOutputSchema",
        ),
        maximum_effect_class: EffectClass::Observational,
        required_capabilities: ToolCapabilitySet::default(),
        resource_ceiling: resources,
        sandbox_requirement: sandbox,
        platform_support: vec![platform],
        receipt_semantics: StableTypeId::parse(PLUGIN_RUNNER_RECEIPT_TYPE)
            .map_err(|error| error.to_string())?,
        reconciler: None,
    };
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Builds the exact argument artifact bound by a plugin Tool proposal and permit.
///
/// # Errors
/// Returns an error for malformed identities, digests, limits, or canonical encoding.
pub fn plugin_runner_invocation_artifact(
    invocation_id: &str,
    plugin_revision_id: &str,
    plugin_manifest: &PluginManifest,
    module_sha256: &str,
    input: &Value,
    limits: &WasiPluginLimits,
) -> Result<ArtifactRef, String> {
    let payload = plugin_runner_invocation_payload(
        invocation_id,
        plugin_revision_id,
        plugin_manifest,
        module_sha256,
        input,
        limits,
    )?;
    let byte_length = u64::try_from(payload.bytes.len())
        .map_err(|_| "plugin invocation binding is not representable".to_owned())?;
    let content_hash = payload
        .content_hash(PLUGIN_RUNNER_INVOCATION_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(ArtifactRef {
        content_hash,
        media_type: "application/vnd.metacog.plugin-runner-invocation+cbor".into(),
        byte_length,
    })
}

/// Builds the canonical compact descriptor used as the action operation and Tool argument.
/// Raw module and input bytes remain outside the authority envelope while their exact digests,
/// identities, manifest, and limits remain bound.
///
/// # Errors
/// Returns an error for malformed identities, digests, limits, or canonical encoding.
pub fn plugin_runner_invocation_payload(
    invocation_id: &str,
    plugin_revision_id: &str,
    plugin_manifest: &PluginManifest,
    module_sha256: &str,
    input: &Value,
    limits: &WasiPluginLimits,
) -> Result<VersionedPayload, String> {
    if invocation_id.is_empty() || invocation_id.len() > 128 {
        return Err("plugin invocation identity is invalid".into());
    }
    if plugin_revision_id.is_empty() || plugin_revision_id.len() > 128 {
        return Err("plugin revision identity is invalid".into());
    }
    plugin_manifest
        .validate()
        .map_err(|error| error.to_string())?;
    validate_hex_sha256(module_sha256, "plugin module digest")?;
    limits.validate().map_err(|error| error.to_string())?;
    let input_bytes = serde_json::to_vec(input).map_err(|error| error.to_string())?;
    let input_sha256 = sha256_hex(&input_bytes);
    let binding = PluginRunnerInvocationBinding {
        schema_version: 1,
        invocation_id,
        plugin_revision_id,
        plugin_manifest,
        module_sha256,
        input_sha256: &input_sha256,
        limits,
    };
    VersionedPayload::canonical_v1(
        StableTypeId::parse(PLUGIN_RUNNER_INVOCATION_TYPE).map_err(|error| error.to_string())?,
        &serde_json::to_value(binding).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Builds the least-authority Root policy for one exact plugin Tool manifest.
///
/// # Errors
/// Returns an error when the manifest cannot produce an immutable reference.
pub fn governed_plugin_tool_policy(manifest: &ToolManifest) -> Result<ToolPolicy, String> {
    Ok(ToolPolicy {
        allowed_manifests: vec![
            manifest
                .versioned_ref()
                .map_err(|error| error.to_string())?,
        ],
        maximum_effect_class: EffectClass::Observational,
        maximum_capabilities: ToolCapabilitySet::default(),
        maximum_limits: manifest.resource_ceiling,
        maximum_permit_lifetime_seconds: 5 * 60,
    })
}

#[cfg(feature = "executor")]
#[allow(clippy::too_many_lines)]
fn validate_plugin_runner_authorization(
    request: &PluginRunnerRequest,
    authorization: &PluginRunnerAuthorization,
    now: DateTime<Utc>,
) -> Result<(), String> {
    authorization
        .manifest
        .validate()
        .map_err(|error| error.to_string())?;
    authorization
        .proposal
        .validate()
        .map_err(|error| error.to_string())?;
    authorization
        .policy
        .validate()
        .map_err(|error| error.to_string())?;
    authorization
        .permit
        .validate()
        .map_err(|error| error.to_string())?;
    authorization
        .permit_ref
        .validate()
        .map_err(|error| error.to_string())?;
    let runner = plugin_runner_executable_source(
        &std::env::current_exe().map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let platform = current_execution_platform();
    let manifest = governed_plugin_tool_manifest(
        &runner,
        &authorization.plugin_revision_id,
        &request.manifest,
        &request.module_sha256,
        &request.limits,
        platform,
    )?;
    let manifest_ref = manifest
        .versioned_ref()
        .map_err(|error| error.to_string())?;
    let arguments = plugin_runner_invocation_artifact(
        &authorization.invocation_id,
        &authorization.plugin_revision_id,
        &request.manifest,
        &request.module_sha256,
        &request.input,
        &request.limits,
    )?;
    let proposal_hash = tool_invocation_proposal_hash(&authorization.proposal)
        .map_err(|error| error.to_string())?;
    let permit_hash =
        tool_invocation_permit_hash(&authorization.permit).map_err(|error| error.to_string())?;
    let policy_binding = tool_policy_binding(&authorization.policy)?;
    let resources = plugin_runner_resource_limits(&request.limits)?;
    let sandbox = plugin_runner_sandbox_profile();
    if authorization.manifest != manifest
        || authorization.proposal.tool_manifest != manifest_ref
        || authorization.permit.tool_manifest != manifest_ref
        || authorization.proposal.arguments != arguments
        || authorization.proposal.argument_hash != arguments.content_hash
        || authorization.permit.argument_hash != arguments.content_hash
        || authorization.proposal_hash != proposal_hash
        || authorization.permit.proposal_hash != proposal_hash
        || authorization.permit_hash != permit_hash
        || authorization.proposal.action_id != authorization.permit.action_id
        || authorization.proposal.attempt_id != authorization.permit.attempt_id
        || authorization.proposal.attempt_revision != authorization.permit.attempt_revision
        || authorization.proposal.lease_generation != authorization.permit.lease_generation
        || authorization.proposal.runtime_actor != authorization.permit.runtime_actor
        || authorization.proposal.runtime_identity != authorization.permit.runtime_identity
        || authorization.proposal.task_contract != authorization.permit.task_contract
        || authorization.proposal.requested_effect_class != EffectClass::Observational
        || authorization.permit.allowed_effect_class != EffectClass::Observational
        || authorization.proposal.requested_capabilities != ToolCapabilitySet::default()
        || authorization.permit.capabilities != ToolCapabilitySet::default()
        || authorization.proposal.requested_limits != resources
        || authorization.permit.limits != resources
        || authorization.proposal.required_sandbox != sandbox
        || !authorization.permit.sandbox.satisfies(sandbox)
        || authorization.proposal.platform != platform
        || authorization.permit.platform != platform
        || authorization.permit.policy != policy_binding
        || authorization.proposal.deadline != authorization.permit.expires_at
        || authorization.permit.issued_at > now
        || authorization.permit.expires_at <= now
        || !authorization.permit.secret_handles.is_empty()
        || authorization.permit_ref.source_type.as_str() != ToolInvocationPermit::TYPE_ID
        || authorization.permit_ref.locator != authorization.permit.id.to_string()
        || authorization.permit_ref.content_hash != permit_hash
    {
        return Err("request differs from the exact Root-issued plugin Tool authority".into());
    }
    Ok(())
}

#[cfg(feature = "executor")]
fn tool_policy_binding(policy: &ToolPolicy) -> Result<PolicyBinding, String> {
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse(ToolPolicy::TYPE_ID).map_err(|error| error.to_string())?,
        &serde_json::to_value(policy).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let content_hash = payload
        .content_hash(mcg_contracts::TOOL_POLICY_HASH_DOMAIN)
        .map_err(|error| error.to_string())?;
    Ok(PolicyBinding {
        payload,
        content_hash,
    })
}

fn validate_hex_sha256(value: &str, label: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(format!("{label} is not lowercase SHA-256"))
}

fn static_source_ref(source_type: &str, locator: &str, domain: &str) -> SourceRef {
    let digest = Sha256::digest(format!("{source_type}\0{locator}").as_bytes());
    SourceRef {
        source_type: StableTypeId::parse(source_type).expect("fixed plugin-runner source type"),
        locator: locator.into(),
        content_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: domain.into(),
            digest_hex: format!("{digest:x}"),
        },
    }
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
    let result_matches = response.result.as_ref().is_some_and(|result| {
        result.schema_version == PLUGIN_SCHEMA_VERSION && result.plugin_id == expected_plugin_id
    });
    if status_success != response_succeeded
        || response.schema_version != PLUGIN_SCHEMA_VERSION
        || response.plugin_id.as_deref() != Some(expected_plugin_id)
        || (response_succeeded
            && (!result_matches || response.error_code.is_some() || response.detail.is_some()))
        || (!response_succeeded
            && (response.result.is_some()
                || !response.error_code.as_deref().is_some_and(valid_error_code)
                || response
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.len() > RUNNER_DETAIL_MAX_BYTES)))
    {
        return Err(PluginRunnerClientError::InvalidResponse);
    }
    Ok(response)
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Hashes one exact real plugin-runner executable into a stable source reference.
///
/// # Errors
/// Returns unavailable when the path is not a regular non-link file or cannot be read.
pub fn plugin_runner_executable_source(path: &Path) -> Result<SourceRef, PluginRunnerClientError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| PluginRunnerClientError::Unavailable)?;
    if !metadata.file_type().is_file() || is_link_or_reparse(&metadata) {
        return Err(PluginRunnerClientError::Unavailable);
    }
    let mut file = std::fs::File::open(path).map_err(|_| PluginRunnerClientError::Unavailable)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1_024].into_boxed_slice();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| PluginRunnerClientError::Unavailable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(SourceRef {
        source_type: StableTypeId::parse(PLUGIN_RUNNER_EXECUTABLE_SOURCE_TYPE)
            .map_err(|_| PluginRunnerClientError::Unavailable)?,
        locator: runner_file_name().into(),
        content_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: PLUGIN_RUNNER_EXECUTABLE_HASH_DOMAIN.into(),
            digest_hex: format!("{:x}", hasher.finalize()),
        },
    })
}

const fn runner_file_name() -> &'static str {
    if cfg!(windows) {
        "mcg-plugin-runner.exe"
    } else {
        "mcg-plugin-runner"
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;
    use mcg_contracts::{
        ActionId, AttemptId, PROTOCOL_V1, PermitId, RuntimeActorId, RuntimeIdentity,
        RuntimeInstanceId, TaskContractId, VersionedRef,
    };
    use serde_json::json;

    use mcg_plugin_protocol::{PluginClass, PluginManifest};

    use super::*;

    #[cfg(feature = "executor")]
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
            authorization: None,
        }
    }

    #[cfg(feature = "executor")]
    fn authorize(request: &PluginRunnerRequest, now: DateTime<Utc>) -> PluginRunnerAuthorization {
        let invocation_id = "019-plugin-invocation".to_owned();
        let plugin_revision_id = "019-plugin-revision".to_owned();
        let runner = plugin_runner_executable_source(&std::env::current_exe().expect("test exe"))
            .expect("runner source");
        let manifest = governed_plugin_tool_manifest(
            &runner,
            &plugin_revision_id,
            &request.manifest,
            &request.module_sha256,
            &request.limits,
            current_execution_platform(),
        )
        .expect("manifest");
        let policy = governed_plugin_tool_policy(&manifest).expect("policy");
        let arguments = plugin_runner_invocation_artifact(
            &invocation_id,
            &plugin_revision_id,
            &request.manifest,
            &request.module_sha256,
            &request.input,
            &request.limits,
        )
        .expect("arguments");
        let runtime_identity = RuntimeIdentity {
            instance_id: RuntimeInstanceId::new(),
            build_hash: runner.content_hash.clone(),
            protocol_version: PROTOCOL_V1,
            started_at: now - ChronoDuration::seconds(1),
        };
        let action_id = ActionId::new();
        let proposal = ToolInvocationProposal {
            action_id,
            attempt_id: AttemptId::new(),
            attempt_revision: 1,
            lease_generation: 1,
            runtime_actor: RuntimeActorId::new(),
            runtime_identity: runtime_identity.clone(),
            task_contract: VersionedRef {
                id: TaskContractId::new(),
                version: 1,
                content_hash: static_source_ref(
                    "mcg.test_task_contract.v1",
                    "plugin-runner-test",
                    "MetaCog.TestTaskContract",
                )
                .content_hash,
            },
            tool_manifest: manifest.versioned_ref().expect("manifest ref"),
            arguments: arguments.clone(),
            argument_hash: arguments.content_hash,
            requested_effect_class: EffectClass::Observational,
            requested_capabilities: ToolCapabilitySet::default(),
            requested_limits: manifest.resource_ceiling,
            required_sandbox: manifest.sandbox_requirement,
            platform: current_execution_platform(),
            deadline: now + ChronoDuration::minutes(2),
        };
        let proposal_hash = tool_invocation_proposal_hash(&proposal).expect("proposal hash");
        let permit = ToolInvocationPermit {
            id: PermitId::new(),
            action_id,
            attempt_id: proposal.attempt_id,
            attempt_revision: proposal.attempt_revision,
            lease_generation: proposal.lease_generation,
            runtime_actor: proposal.runtime_actor,
            runtime_identity,
            task_contract: proposal.task_contract.clone(),
            tool_manifest: proposal.tool_manifest.clone(),
            proposal_hash: proposal_hash.clone(),
            argument_hash: proposal.argument_hash.clone(),
            allowed_effect_class: EffectClass::Observational,
            capabilities: ToolCapabilitySet::default(),
            limits: proposal.requested_limits,
            sandbox: plugin_runner_sandbox_profile(),
            platform: current_execution_platform(),
            policy: tool_policy_binding(&policy).expect("policy binding"),
            expires_at: proposal.deadline,
            secret_handles: Vec::new(),
            issued_at: now,
        };
        let permit_hash = tool_invocation_permit_hash(&permit).expect("permit hash");
        PluginRunnerAuthorization {
            invocation_id,
            plugin_revision_id,
            manifest,
            proposal,
            proposal_hash,
            policy,
            permit: permit.clone(),
            permit_hash: permit_hash.clone(),
            permit_ref: SourceRef {
                source_type: StableTypeId::parse(ToolInvocationPermit::TYPE_ID)
                    .expect("permit type"),
                locator: permit.id.to_string(),
                content_hash: permit_hash,
            },
        }
    }

    #[cfg(feature = "executor")]
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

    #[cfg(feature = "executor")]
    #[tokio::test]
    async fn malformed_and_oversized_requests_are_bounded() {
        let malformed = execute_request(b"not-json").await;
        assert_eq!(malformed.error_code.as_deref(), Some("invalid_request"));
        let oversized = execute_request(&vec![b'x'; RUNNER_REQUEST_MAX_BYTES + 1]).await;
        assert_eq!(oversized.error_code.as_deref(), Some("request_too_large"));
    }

    #[cfg(feature = "executor")]
    #[test]
    fn exact_plugin_tool_authority_binds_input_module_revision_and_limits() {
        let now = Utc::now();
        let module = br#"(module (func (export "_start")))"#;
        let mut input_substituted = request(module);
        let authorization = authorize(&input_substituted, now);
        validate_plugin_runner_authorization(&input_substituted, &authorization, now)
            .expect("exact authority");

        input_substituted.input = json!({"substituted": true});
        let error = validate_plugin_runner_authorization(&input_substituted, &authorization, now)
            .expect_err("input substitution must fail");
        assert!(error.contains("exact Root-issued"));

        let revision_request = request(module);
        let mut authorization = authorize(&revision_request, now);
        authorization.plugin_revision_id.push_str("-changed");
        assert!(
            validate_plugin_runner_authorization(&revision_request, &authorization, now).is_err()
        );

        let mut limits_substituted = request(module);
        let authorization = authorize(&limits_substituted, now);
        limits_substituted.limits.maximum_output_bytes /= 2;
        assert!(
            validate_plugin_runner_authorization(&limits_substituted, &authorization, now).is_err()
        );
    }

    #[cfg(feature = "executor")]
    #[tokio::test]
    async fn executor_rejects_argument_substitution_before_guest_execution() {
        let module = br#"(module (func (export "_start")))"#;
        let mut request = request(module);
        request.authorization = Some(authorize(&request, Utc::now()));
        request.input = json!({"substituted": true});

        let response = execute_request(&serde_json::to_vec(&request).expect("request")).await;

        assert_eq!(response.state, PluginRunnerState::Failed);
        assert_eq!(
            response.error_code.as_deref(),
            Some("tool_authority_invalid")
        );
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

        let mut mismatched_result = valid.clone();
        mismatched_result.result.as_mut().expect("result").plugin_id = "another.plugin".into();
        assert!(matches!(
            validate_response(
                true,
                "example.runner",
                &serde_json::to_vec(&mismatched_result).expect("response")
            ),
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
