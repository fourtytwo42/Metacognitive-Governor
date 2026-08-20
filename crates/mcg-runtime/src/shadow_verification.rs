#[cfg(target_os = "windows")]
use std::path::Path as StdPath;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};
use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use mcg_agents::SuccessCondition;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use mcg_domain::{TaskId, ToolExecutionId};
#[cfg(any(target_os = "windows", test))]
use mcg_tools::ToolOutcome;
use mcg_tools::{ArtifactReference, ArtifactStore};
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use mcg_tools::{
    ProcessExecutor, ProcessOptions, ProcessStream, RejectingCredentialResolver, ToolError,
    ToolExecutor, ToolRequest, ToolRequestKind,
};
use serde::{Deserialize, Serialize};
use url::{Host, Url};
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use uuid::Uuid;

pub(crate) const SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION: u16 = 1;
#[cfg(any(target_os = "linux", test))]
const BWRAP_PROOF: &[u8] = b"mcg-shadow-bwrap-v1\n";
#[cfg(any(target_os = "linux", test))]
const BWRAP_TIMEOUT_SECONDS: u64 = 600;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_SOURCE_FILES: u64 = 50_000;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_SOURCE_DEPTH: usize = 32;
#[cfg(any(target_os = "linux", test))]
const BWRAP_TMPFS_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_ADDRESS_SPACE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_BROWSER_ADDRESS_SPACE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_PROCESSES: u32 = 256;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_OPEN_FILES: u32 = 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_ARGUMENTS: usize = 128;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_COMMAND_BYTES: usize = 32 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const NODE_TOOLCHAIN_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const NODE_TOOLCHAIN_MAX_FILES: u64 = 100_000;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const NODE_TOOLCHAIN_MAX_DEPTH: usize = 24;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BROWSER_RUNTIME_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BROWSER_RUNTIME_MAX_FILES: u64 = 100_000;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BROWSER_RUNTIME_MAX_DEPTH: usize = 16;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const SHADOW_MAX_SELECTOR_BYTES: usize = 4 * 1024;
#[cfg(any(target_os = "windows", test))]
const WSB_START_TIMEOUT_SECONDS: u64 = 180;
#[cfg(any(target_os = "windows", test))]
const WSB_COMMAND_TIMEOUT_SECONDS: u64 = 600;
#[cfg(any(target_os = "windows", test))]
const WSB_STOP_TIMEOUT_SECONDS: u64 = 60;
#[cfg(any(target_os = "windows", test))]
const WSB_SETUP_FAILURE_EXIT_CODE: i32 = 125;

/// Credential-free, bounded command result interpreted by the trusted runtime.
/// Output bodies belong in separate bounded artifacts; this record binds the
/// exact condition to the process disposition used for pass/fail derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ShadowCommandTranscript {
    pub schema_version: u16,
    pub condition: SuccessCondition,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: Option<ArtifactReference>,
    pub stderr: Option<ArtifactReference>,
}

/// Outcome returned by an operating-system-level shadow command sandbox.
///
/// A completed result is trusted only when it carries an immutable transcript
/// artifact digest. `Unavailable` is deliberately distinct from command
/// failure: it keeps the objective inconclusive instead of pretending that a
/// host process launched with a restricted working directory was isolated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShadowCommandVerification {
    Unavailable,
    #[allow(
        dead_code,
        reason = "constructed by the Linux backend and deterministic test verifiers"
    )]
    Completed {
        transcript: ArtifactReference,
    },
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeToolchain {
    host_root: PathBuf,
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserRuntime {
    host_root: PathBuf,
    executable_name: String,
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ShadowServiceProbe {
    Http {
        url: String,
        expected_status: Option<u16>,
    },
    Tcp {
        endpoint: String,
    },
    PageRender {
        url: String,
    },
    ElementExists {
        url: String,
        selector: String,
    },
}

#[cfg(any(target_os = "linux", test))]
impl ShadowServiceProbe {
    fn endpoint(&self) -> &str {
        match self {
            Self::Tcp { endpoint } => endpoint,
            Self::Http { url, .. } | Self::PageRender { url } | Self::ElementExists { url, .. } => {
                url
            }
        }
    }

    const fn expected_status(&self) -> Option<u16> {
        match self {
            Self::Http {
                expected_status, ..
            } => *expected_status,
            Self::Tcp { .. } | Self::PageRender { .. } | Self::ElementExists { .. } => None,
        }
    }

    const fn kind(&self) -> &'static str {
        match self {
            Self::Http { .. } => "http",
            Self::Tcp { .. } => "tcp",
            Self::PageRender { .. } => "page_render",
            Self::ElementExists { .. } => "element_exists",
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
impl ShadowServiceProbe {
    const fn requires_browser(&self) -> bool {
        matches!(self, Self::PageRender { .. } | Self::ElementExists { .. })
    }
}

/// Capability boundary for build, test, and static-analysis verification in a
/// retained shadow workspace.
///
/// Implementations must provide operating-system enforcement for filesystem,
/// process, credential, and network isolation. A working-directory check alone
/// does not satisfy this contract.
#[async_trait]
pub(crate) trait ShadowCommandVerifier: Send + Sync {
    async fn verify(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification;

    async fn verify_service(
        &self,
        _workspace_root: &Path,
        _service_command: &str,
        _condition: &SuccessCondition,
        _artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        ShadowCommandVerification::Unavailable
    }
}

/// Selects a backend only after its immutable executable boundary passes local
/// safety checks. Unsupported hosts retain the no-execution backend.
pub(crate) fn default_shadow_command_verifier() -> Arc<dyn ShadowCommandVerifier> {
    #[cfg(target_os = "linux")]
    if let Some(executable) = trusted_bubblewrap_executable() {
        return Arc::new(LinuxBubblewrapVerifier {
            executable,
            runner: trusted_linux_shadow_runner(),
        });
    }
    #[cfg(target_os = "windows")]
    if let Some((executable, runner)) = trusted_windows_sandbox_components() {
        return Arc::new(WindowsSandboxVerifier {
            executable,
            runner,
            lifecycle: tokio::sync::Mutex::new(()),
            timeouts: WindowsSandboxTimeouts::default(),
        });
    }
    Arc::new(UnavailableShadowCommandVerifier)
}

/// Production-safe fallback when no qualified host sandbox is present.
#[derive(Debug, Default)]
pub(crate) struct UnavailableShadowCommandVerifier;

#[async_trait]
impl ShadowCommandVerifier for UnavailableShadowCommandVerifier {
    async fn verify(
        &self,
        _workspace_root: &Path,
        _condition: &SuccessCondition,
        _artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        ShadowCommandVerification::Unavailable
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug)]
struct LinuxBubblewrapVerifier {
    executable: PathBuf,
    runner: Option<PathBuf>,
}

#[cfg(any(target_os = "linux", test))]
#[async_trait]
impl ShadowCommandVerifier for LinuxBubblewrapVerifier {
    async fn verify(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        match self
            .verify_inner(workspace_root, condition, artifacts)
            .await
        {
            Ok(result) => result,
            Err(_) => ShadowCommandVerification::Unavailable,
        }
    }

    async fn verify_service(
        &self,
        workspace_root: &Path,
        service_command: &str,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        match self
            .verify_service_inner(workspace_root, service_command, condition, artifacts)
            .await
        {
            Ok(result) => result,
            Err(_) => ShadowCommandVerification::Unavailable,
        }
    }
}

#[cfg(any(target_os = "linux", test))]
impl LinuxBubblewrapVerifier {
    async fn verify_inner(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> Result<ShadowCommandVerification, ToolError> {
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve shadow workspace: {error}")))?;
        verify_workspace_bounds(&workspace_root)?;
        let command = condition_command(condition)
            .ok_or_else(|| ToolError::Invalid("shadow verifier requires a command".into()))?;
        let (executable, command_arguments) = split_command(command)?;
        let toolchain = discover_node_toolchain(executable)?;
        let proof = EphemeralProof::create(&workspace_root)?;
        let argv = bubblewrap_arguments(
            &workspace_root,
            &proof.path,
            executable,
            &command_arguments,
            toolchain.as_ref(),
        )?;
        let process = ProcessExecutor::new(
            [workspace_root.clone()],
            Arc::new(RejectingCredentialResolver),
        )?;
        let options = ProcessOptions {
            executable: self.executable.to_string_lossy().into_owned(),
            argv,
            cwd: workspace_root.to_string_lossy().into_owned(),
            env: Vec::new(),
            stdin: ProcessStream::Null,
            stdin_data: None,
            stdout: ProcessStream::Piped,
            stderr: ProcessStream::Piped,
        };
        let mut request = ToolRequest::new(
            TaskId::new(),
            "verify command in a Linux bubblewrap shadow sandbox",
            ToolRequestKind::ProcessExec { options },
        );
        request.timeout_seconds = BWRAP_TIMEOUT_SECONDS;
        let execution = process.execute(ToolExecutionId::new(), &request).await;
        let proof_valid = fs::read(&proof.path).is_ok_and(|bytes| bytes == BWRAP_PROOF);
        if !proof_valid {
            return Err(ToolError::Unavailable(
                "bubblewrap did not prove entry into the configured sandbox".into(),
            ));
        }
        let (exit_code, timed_out, stdout, stderr) = match execution {
            Ok(outcome) => (
                outcome.exit_code,
                false,
                capture_optional_output(artifacts, outcome.stdout.as_deref())?,
                capture_optional_output(artifacts, outcome.stderr.as_deref())?,
            ),
            Err(ToolError::TimedOut) => (None, true, None, None),
            Err(error) => return Err(error),
        };
        capture_shadow_transcript(artifacts, condition, exit_code, timed_out, stdout, stderr)
    }

    #[allow(clippy::too_many_lines)] // Keep the complete same-sandbox browser mapping and process profile auditable.
    async fn verify_service_inner(
        &self,
        workspace_root: &Path,
        service_command: &str,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> Result<ShadowCommandVerification, ToolError> {
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve shadow workspace: {error}")))?;
        verify_workspace_bounds(&workspace_root)?;
        let probe = shadow_service_probe(condition)?;
        let browser = probe
            .requires_browser()
            .then(discover_browser_runtime)
            .transpose()?
            .flatten();
        if probe.requires_browser() && browser.is_none() {
            return Err(ToolError::Unavailable(
                "same-sandbox browser verification requires a configured browser runtime".into(),
            ));
        }
        let probe_program_available = match probe {
            ShadowServiceProbe::Tcp { .. } => Path::new("/bin/bash").is_file(),
            ShadowServiceProbe::Http { .. } => Path::new("/usr/bin/curl").is_file(),
            ShadowServiceProbe::PageRender { .. } | ShadowServiceProbe::ElementExists { .. } => {
                true
            }
        };
        let sleep_available = probe.requires_browser() || Path::new("/usr/bin/sleep").is_file();
        if !probe_program_available || !sleep_available {
            return Err(ToolError::Unavailable(
                "Linux shadow service verification requires its trusted probe utility and sleep"
                    .into(),
            ));
        }
        let (executable, command_arguments) = split_command(service_command)?;
        let toolchain = discover_node_toolchain(executable)?;
        let proof = EphemeralProof::create(&workspace_root)?;
        let runner_request = if probe.requires_browser() {
            self.runner.as_deref().ok_or_else(|| {
                ToolError::Unavailable(
                    "same-sandbox browser verification requires the packaged shadow runner".into(),
                )
            })?;
            Some(LinuxRunnerRequestFile::create(
                &workspace_root,
                executable,
                &command_arguments,
                toolchain.as_ref(),
                browser.as_ref().expect("browser checked above"),
                &probe,
            )?)
        } else {
            None
        };
        let argv = bubblewrap_service_arguments(
            &workspace_root,
            &proof.path,
            executable,
            &command_arguments,
            &probe,
            toolchain.as_ref(),
            browser.as_ref(),
            self.runner.as_deref(),
            runner_request
                .as_ref()
                .map(|request| request.path.as_path()),
        )?;
        let process = ProcessExecutor::new(
            [workspace_root.clone()],
            Arc::new(RejectingCredentialResolver),
        )?;
        let options = ProcessOptions {
            executable: self.executable.to_string_lossy().into_owned(),
            argv,
            cwd: workspace_root.to_string_lossy().into_owned(),
            env: Vec::new(),
            stdin: ProcessStream::Null,
            stdin_data: None,
            stdout: ProcessStream::Piped,
            stderr: ProcessStream::Piped,
        };
        let mut request = ToolRequest::new(
            TaskId::new(),
            "verify a service beside its HTTP probe in one Linux shadow sandbox",
            ToolRequestKind::ProcessExec { options },
        );
        request.timeout_seconds = BWRAP_TIMEOUT_SECONDS;
        let execution = process.execute(ToolExecutionId::new(), &request).await;
        let proof_valid = fs::read(&proof.path).is_ok_and(|bytes| bytes == BWRAP_PROOF);
        if !proof_valid {
            return Err(ToolError::Unavailable(
                "bubblewrap did not prove entry into the configured service sandbox".into(),
            ));
        }
        let (exit_code, timed_out, stdout, stderr) = match execution {
            Ok(outcome) => (
                outcome.exit_code,
                false,
                capture_optional_output(artifacts, outcome.stdout.as_deref())?,
                capture_optional_output(artifacts, outcome.stderr.as_deref())?,
            ),
            Err(ToolError::TimedOut) => (None, true, None, None),
            Err(error) => return Err(error),
        };
        capture_shadow_transcript(artifacts, condition, exit_code, timed_out, stdout, stderr)
    }
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug)]
struct WindowsSandboxVerifier {
    executable: PathBuf,
    runner: PathBuf,
    lifecycle: tokio::sync::Mutex<()>,
    timeouts: WindowsSandboxTimeouts,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, Copy)]
struct WindowsSandboxTimeouts {
    start: u64,
    command: u64,
    stop: u64,
}

#[cfg(any(target_os = "windows", test))]
impl Default for WindowsSandboxTimeouts {
    fn default() -> Self {
        Self {
            start: WSB_START_TIMEOUT_SECONDS,
            command: WSB_COMMAND_TIMEOUT_SECONDS,
            stop: WSB_STOP_TIMEOUT_SECONDS,
        }
    }
}

#[cfg(any(target_os = "windows", test))]
#[async_trait]
impl ShadowCommandVerifier for WindowsSandboxVerifier {
    async fn verify(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        let _lifecycle = self.lifecycle.lock().await;
        match self
            .verify_inner(workspace_root, condition, artifacts)
            .await
        {
            Ok(result) => result,
            Err(_) => ShadowCommandVerification::Unavailable,
        }
    }

    async fn verify_service(
        &self,
        workspace_root: &Path,
        service_command: &str,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> ShadowCommandVerification {
        let _lifecycle = self.lifecycle.lock().await;
        match self
            .verify_service_inner(workspace_root, service_command, condition, artifacts)
            .await
        {
            Ok(result) => result,
            Err(_) => ShadowCommandVerification::Unavailable,
        }
    }
}

#[cfg(any(target_os = "windows", test))]
impl WindowsSandboxVerifier {
    async fn stop_session(&self, process: &ProcessExecutor, harness_root: &Path, session_id: &str) {
        stop_windows_sandbox(
            process,
            &self.executable,
            harness_root,
            session_id,
            self.timeouts.stop,
        )
        .await;
    }

    async fn verify_inner(
        &self,
        workspace_root: &Path,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> Result<ShadowCommandVerification, ToolError> {
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve shadow workspace: {error}")))?;
        verify_workspace_bounds(&workspace_root)?;
        let command = condition_command(condition)
            .ok_or_else(|| ToolError::Invalid("shadow verifier requires a command".into()))?;
        let (executable, command_arguments) = split_command(command)?;
        let toolchain = discover_node_toolchain(executable)?;
        let executable = windows_command_name(executable);
        let harness = WindowsSandboxHarness::create(
            &workspace_root,
            &self.runner,
            &executable,
            &command_arguments,
            toolchain.as_ref(),
            None,
            None,
        )?;
        let configuration = windows_sandbox_configuration(
            &workspace_root,
            &harness.path,
            toolchain.as_ref(),
            None,
        )?;
        let process = ProcessExecutor::new(
            [harness.path.clone()],
            Arc::new(RejectingCredentialResolver),
        )?;

        let (exit_code, timed_out) = self
            .execute_session(&process, &harness.path, &configuration)
            .await?;
        capture_shadow_transcript(artifacts, condition, exit_code, timed_out, None, None)
    }

    async fn verify_service_inner(
        &self,
        workspace_root: &Path,
        service_command: &str,
        condition: &SuccessCondition,
        artifacts: &ArtifactStore,
    ) -> Result<ShadowCommandVerification, ToolError> {
        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve shadow workspace: {error}")))?;
        verify_workspace_bounds(&workspace_root)?;
        let probe = shadow_service_probe(condition)?;
        let browser = probe
            .requires_browser()
            .then(discover_browser_runtime)
            .transpose()?
            .flatten();
        if probe.requires_browser() && browser.is_none() {
            return Err(ToolError::Unavailable(
                "same-sandbox browser verification requires a configured browser runtime".into(),
            ));
        }
        let (executable, command_arguments) = split_command(service_command)?;
        let toolchain = discover_node_toolchain(executable)?;
        let executable = windows_command_name(executable);
        let harness = WindowsSandboxHarness::create(
            &workspace_root,
            &self.runner,
            &executable,
            &command_arguments,
            toolchain.as_ref(),
            browser.as_ref(),
            Some(&probe),
        )?;
        let configuration = windows_sandbox_configuration(
            &workspace_root,
            &harness.path,
            toolchain.as_ref(),
            browser.as_ref(),
        )?;
        let process = ProcessExecutor::new(
            [harness.path.clone()],
            Arc::new(RejectingCredentialResolver),
        )?;
        let (exit_code, timed_out) = self
            .execute_session(&process, &harness.path, &configuration)
            .await?;
        capture_shadow_transcript(artifacts, condition, exit_code, timed_out, None, None)
    }

    async fn execute_session(
        &self,
        process: &ProcessExecutor,
        harness_root: &Path,
        configuration: &str,
    ) -> Result<(Option<i32>, bool), ToolError> {
        let session_id = Uuid::new_v4().to_string();
        let start = windows_sandbox_start_arguments(&session_id, configuration);
        let started = run_windows_sandbox_cli(
            process,
            &self.executable,
            harness_root,
            start,
            self.timeouts.start,
            "start Windows Sandbox shadow session",
        )
        .await;
        if let Err(error) = started
            .and_then(|outcome| require_success(&outcome, "start Windows Sandbox shadow session"))
        {
            self.stop_session(process, harness_root, &session_id).await;
            return Err(error);
        }

        let probe =
            windows_sandbox_exec_arguments(&session_id, WindowsSandboxRunnerInvocation::Probe);
        let preflight = run_windows_sandbox_cli(
            process,
            &self.executable,
            harness_root,
            probe.clone(),
            self.timeouts.start,
            "probe Windows Sandbox shadow session",
        )
        .await
        .and_then(|outcome| {
            let exit_code =
                windows_sandbox_exec_exit_code(&outcome, "probe Windows Sandbox shadow session")?;
            Ok((outcome, exit_code))
        });
        if preflight.as_ref().ok().map(|(_, exit_code)| *exit_code) != Some(0) {
            self.stop_session(process, harness_root, &session_id).await;
            let detail = preflight.map_or_else(
                |error| error.to_string(),
                |(outcome, exit_code)| {
                    format!(
                        "nested exit={exit_code}; {}",
                        windows_sandbox_outcome_detail(&outcome)
                    )
                },
            );
            return Err(ToolError::Unavailable(format!(
                "Windows Sandbox runner preflight failed: {detail}"
            )));
        }

        let disposition = self
            .execute_command_and_postflight(process, harness_root, &session_id, probe)
            .await;
        self.stop_session(process, harness_root, &session_id).await;
        disposition
    }

    async fn execute_command_and_postflight(
        &self,
        process: &ProcessExecutor,
        harness_root: &Path,
        session_id: &str,
        probe: Vec<String>,
    ) -> Result<(Option<i32>, bool), ToolError> {
        let command = run_windows_sandbox_cli(
            process,
            &self.executable,
            harness_root,
            windows_sandbox_exec_arguments(session_id, WindowsSandboxRunnerInvocation::Request),
            self.timeouts.command,
            "execute Windows Sandbox shadow command",
        )
        .await;
        match command {
            Err(ToolError::TimedOut) => Ok((None, true)),
            Err(error) => Err(error),
            Ok(outcome) => match windows_sandbox_exec_exit_code(
                &outcome,
                "execute Windows Sandbox shadow command",
            ) {
                Err(error) => Err(error),
                Ok(command_exit_code) => {
                    let postflight = run_windows_sandbox_cli(
                        process,
                        &self.executable,
                        harness_root,
                        probe,
                        self.timeouts.start,
                        "postflight Windows Sandbox shadow session",
                    )
                    .await;
                    match postflight.and_then(|postflight| {
                        windows_sandbox_exec_exit_code(
                            &postflight,
                            "postflight Windows Sandbox shadow session",
                        )
                    }) {
                        Err(error) => Err(error),
                        Ok(postflight_exit_code) if postflight_exit_code != 0 => {
                            Err(ToolError::Unavailable(format!(
                                "Windows Sandbox runner postflight failed with nested exit {postflight_exit_code}"
                            )))
                        }
                        Ok(_) => match command_exit_code {
                            0 => Ok((Some(0), false)),
                            1 => Ok((Some(1), false)),
                            WSB_SETUP_FAILURE_EXIT_CODE => Err(ToolError::Unavailable(
                                "Windows Sandbox runner rejected its setup".into(),
                            )),
                            _ => Err(ToolError::Unavailable(format!(
                                "Windows Sandbox returned unrecognized nested exit {command_exit_code}"
                            ))),
                        },
                    }
                }
            },
        }
    }
}

#[cfg(any(target_os = "windows", test))]
#[derive(Serialize)]
struct WindowsRunnerRequest<'a> {
    schema_version: u16,
    source_root: &'a str,
    workspace_root: &'a str,
    executable: &'a str,
    argv: &'a [String],
    toolchain_root: Option<&'a str>,
    browser_executable: Option<String>,
    probe: Option<WindowsRunnerProbe<'a>>,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum WindowsRunnerProbe<'a> {
    HttpStatus {
        url: &'a str,
        expected_status: Option<u16>,
    },
    TcpConnect {
        endpoint: &'a str,
    },
    PageRender {
        url: &'a str,
    },
    ElementExists {
        url: &'a str,
        selector: &'a str,
    },
}

#[cfg(any(target_os = "windows", test))]
struct WindowsSandboxHarness {
    path: PathBuf,
}

#[cfg(any(target_os = "windows", test))]
impl WindowsSandboxHarness {
    fn create(
        workspace_root: &Path,
        runner: &Path,
        executable: &str,
        argv: &[String],
        toolchain: Option<&NodeToolchain>,
        browser: Option<&BrowserRuntime>,
        probe: Option<&ShadowServiceProbe>,
    ) -> Result<Self, ToolError> {
        let parent = workspace_root
            .parent()
            .ok_or_else(|| ToolError::Invalid("shadow workspace has no parent".into()))?;
        let path = parent.join(format!(".mcg-wsb-{}", Uuid::new_v4()));
        fs::create_dir(&path)
            .map_err(|error| ToolError::Execution(format!("create sandbox harness: {error}")))?;
        let result = (|| {
            fs::copy(runner, path.join("mcg-shadow-runner.exe")).map_err(|error| {
                ToolError::Execution(format!("copy Windows Sandbox runner: {error}"))
            })?;
            let request = WindowsRunnerRequest {
                schema_version: 5,
                source_root: r"C:\mcg-source",
                workspace_root: r"C:\mcg-workspace",
                executable,
                argv,
                toolchain_root: toolchain.map(|_| r"C:\mcg-toolchain"),
                browser_executable: browser
                    .map(|browser| format!(r"C:\mcg-browser\{}", browser.executable_name)),
                probe: probe.map(|probe| match probe {
                    ShadowServiceProbe::Http {
                        url,
                        expected_status,
                    } => WindowsRunnerProbe::HttpStatus {
                        url,
                        expected_status: *expected_status,
                    },
                    ShadowServiceProbe::Tcp { endpoint } => {
                        WindowsRunnerProbe::TcpConnect { endpoint }
                    }
                    ShadowServiceProbe::PageRender { url } => {
                        WindowsRunnerProbe::PageRender { url }
                    }
                    ShadowServiceProbe::ElementExists { url, selector } => {
                        WindowsRunnerProbe::ElementExists { url, selector }
                    }
                }),
            };
            let bytes = serde_json::to_vec(&request).map_err(|error| {
                ToolError::Execution(format!("serialize Windows Sandbox request: {error}"))
            })?;
            let request_path = path.join("request.json");
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            std::io::Write::write_all(
                &mut options.open(&request_path).map_err(|error| {
                    ToolError::Execution(format!("create Windows Sandbox request: {error}"))
                })?,
                &bytes,
            )
            .map_err(|error| {
                ToolError::Execution(format!("write Windows Sandbox request: {error}"))
            })?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&path);
            return Err(error);
        }
        Ok(Self { path })
    }
}

#[cfg(any(target_os = "windows", test))]
impl Drop for WindowsSandboxHarness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_configuration(
    source: &Path,
    harness: &Path,
    toolchain: Option<&NodeToolchain>,
    browser: Option<&BrowserRuntime>,
) -> Result<String, ToolError> {
    let source = xml_escape_path(source)?;
    let harness = xml_escape_path(harness)?;
    let toolchain_mapping = toolchain.map_or_else(
        || Ok(String::new()),
        |toolchain| {
            xml_escape_path(&toolchain.host_root).map(|root| {
                format!(
                    "<MappedFolder><HostFolder>{root}</HostFolder><SandboxFolder>C:\\mcg-toolchain</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>"
                )
            })
        },
    )?;
    let browser_mapping = browser.map_or_else(
        || Ok(String::new()),
        |browser| {
            xml_escape_path(&browser.host_root).map(|root| {
                format!(
                    "<MappedFolder><HostFolder>{root}</HostFolder><SandboxFolder>C:\\mcg-browser</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>"
                )
            })
        },
    )?;
    Ok(format!(
        "<Configuration><vGPU>Disable</vGPU><Networking>Disable</Networking><AudioInput>Disable</AudioInput><VideoInput>Disable</VideoInput><PrinterRedirection>Disable</PrinterRedirection><ClipboardRedirection>Disable</ClipboardRedirection><ProtectedClient>Enable</ProtectedClient><MemoryInMB>8192</MemoryInMB><MappedFolders><MappedFolder><HostFolder>{source}</HostFolder><SandboxFolder>C:\\mcg-source</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder><MappedFolder><HostFolder>{harness}</HostFolder><SandboxFolder>C:\\mcg-harness</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>{toolchain_mapping}{browser_mapping}</MappedFolders></Configuration>"
    ))
}

#[cfg(any(target_os = "windows", test))]
fn xml_escape_path(path: &Path) -> Result<String, ToolError> {
    let value = path
        .to_str()
        .ok_or_else(|| ToolError::Invalid("Windows Sandbox path is not Unicode".into()))?;
    if value.chars().any(char::is_control) {
        return Err(ToolError::Invalid(
            "Windows Sandbox path contains a control character".into(),
        ));
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_start_arguments(session_id: &str, configuration: &str) -> Vec<String> {
    vec![
        "start".into(),
        "--raw".into(),
        "--id".into(),
        session_id.into(),
        "--config".into(),
        configuration.into(),
    ]
}

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy)]
enum WindowsSandboxRunnerInvocation {
    Probe,
    Request,
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_exec_arguments(
    session_id: &str,
    invocation: WindowsSandboxRunnerInvocation,
) -> Vec<String> {
    let command = match invocation {
        WindowsSandboxRunnerInvocation::Probe => r"C:\mcg-harness\mcg-shadow-runner.exe --probe",
        WindowsSandboxRunnerInvocation::Request => {
            r"C:\mcg-harness\mcg-shadow-runner.exe C:\mcg-harness\request.json"
        }
    };
    vec![
        "exec".into(),
        "--raw".into(),
        "--id".into(),
        session_id.into(),
        "--command".into(),
        command.into(),
        "--run-as".into(),
        "System".into(),
    ]
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_stop_arguments(session_id: &str) -> Vec<String> {
    vec![
        "stop".into(),
        "--raw".into(),
        "--id".into(),
        session_id.into(),
    ]
}

#[cfg(any(target_os = "windows", test))]
fn windows_command_name(executable: &str) -> String {
    match executable.to_ascii_lowercase().as_str() {
        "npm" | "npx" | "pnpm" | "pnpx" | "yarn" | "yarnpkg" | "corepack" => {
            format!("{executable}.cmd")
        }
        _ => executable.to_owned(),
    }
}

#[cfg(any(target_os = "windows", test))]
async fn run_windows_sandbox_cli(
    process: &ProcessExecutor,
    executable: &Path,
    cwd: &Path,
    argv: Vec<String>,
    timeout_seconds: u64,
    purpose: &str,
) -> Result<ToolOutcome, ToolError> {
    let options = ProcessOptions {
        executable: executable.to_string_lossy().into_owned(),
        argv,
        cwd: cwd.to_string_lossy().into_owned(),
        env: Vec::new(),
        stdin: ProcessStream::Null,
        stdin_data: None,
        stdout: ProcessStream::Piped,
        stderr: ProcessStream::Piped,
    };
    let mut request = ToolRequest::new(
        TaskId::new(),
        purpose,
        ToolRequestKind::ProcessExec { options },
    );
    request.timeout_seconds = timeout_seconds;
    process.execute(ToolExecutionId::new(), &request).await
}

#[cfg(any(target_os = "windows", test))]
#[derive(Deserialize)]
struct WindowsSandboxExecEnvelope {
    #[serde(rename = "ExitCode")]
    exit_code: i32,
}

#[cfg(any(target_os = "windows", test))]
fn parse_windows_sandbox_exec_exit_code(raw: &str) -> Option<i32> {
    if raw.len() > 512 {
        return None;
    }
    serde_json::from_str::<WindowsSandboxExecEnvelope>(raw)
        .ok()
        .map(|envelope| envelope.exit_code)
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_exec_exit_code(outcome: &ToolOutcome, purpose: &str) -> Result<i32, ToolError> {
    require_success(outcome, purpose)?;
    outcome
        .stdout
        .as_deref()
        .and_then(parse_windows_sandbox_exec_exit_code)
        .ok_or_else(|| {
            ToolError::Unavailable(format!(
                "{purpose} returned no valid nested exit-code envelope"
            ))
        })
}

#[cfg(any(target_os = "windows", test))]
fn require_success(outcome: &ToolOutcome, purpose: &str) -> Result<(), ToolError> {
    if outcome.exit_code == Some(0) {
        Ok(())
    } else {
        Err(ToolError::Unavailable(format!(
            "{purpose} failed: {}",
            windows_sandbox_outcome_detail(outcome)
        )))
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_sandbox_outcome_detail(outcome: &ToolOutcome) -> String {
    const MAX_FIELD_CHARS: usize = 1_024;

    fn excerpt(value: Option<&str>) -> String {
        value
            .map(|value| value.chars().take(MAX_FIELD_CHARS).collect())
            .unwrap_or_default()
    }

    format!(
        "exit={:?}; stdout={:?}; stderr={:?}",
        outcome.exit_code,
        excerpt(outcome.stdout.as_deref()),
        excerpt(outcome.stderr.as_deref())
    )
}

#[cfg(any(target_os = "windows", test))]
async fn stop_windows_sandbox(
    process: &ProcessExecutor,
    executable: &Path,
    cwd: &Path,
    session_id: &str,
    timeout_seconds: u64,
) {
    let _ = run_windows_sandbox_cli(
        process,
        executable,
        cwd,
        windows_sandbox_stop_arguments(session_id),
        timeout_seconds,
        "stop Windows Sandbox shadow session",
    )
    .await;
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn condition_command(condition: &SuccessCondition) -> Option<&str> {
    match condition {
        SuccessCondition::Build { command }
        | SuccessCondition::Test { command }
        | SuccessCondition::StaticAnalysis { command }
        | SuccessCondition::ConfigurationValid { command } => Some(command),
        SuccessCondition::Custom { description } => description
            .strip_prefix("COMMAND SUCCEEDS ")
            .map(str::trim)
            .filter(|command| !command.is_empty()),
        _ => None,
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn split_command(command: &str) -> Result<(&str, Vec<String>), ToolError> {
    if command.len() > BWRAP_MAX_COMMAND_BYTES || command.contains('\0') {
        return Err(ToolError::Invalid(
            "shadow command exceeds its structured argument boundary".into(),
        ));
    }
    let mut parts = command.split_ascii_whitespace();
    let executable = parts
        .next()
        .ok_or_else(|| ToolError::Invalid("shadow command has no executable".into()))?;
    let arguments = parts.map(str::to_owned).collect::<Vec<_>>();
    if arguments.len() > BWRAP_MAX_ARGUMENTS {
        return Err(ToolError::Invalid(
            "shadow command has too many arguments".into(),
        ));
    }
    Ok((executable, arguments))
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn shadow_service_probe(condition: &SuccessCondition) -> Result<ShadowServiceProbe, ToolError> {
    let (value, expected_status, scheme) = match condition {
        SuccessCondition::HttpResponds { url, status } => (url, Some(*status), "http"),
        SuccessCondition::ServiceHealthy { endpoint } => (endpoint, None, "http"),
        SuccessCondition::PortListening { endpoint } => (endpoint, None, "tcp"),
        SuccessCondition::PageRenders { url } | SuccessCondition::ElementExists { url, .. } => {
            (url, None, "http")
        }
        _ => {
            return Err(ToolError::Invalid(
                "shadow service verifier requires an HTTP, service-health, TCP-port, page-render, or element condition".into(),
            ));
        }
    };
    if value.len() > 2 * 1024 || value.chars().any(char::is_control) {
        return Err(ToolError::Invalid(
            "shadow service URL exceeds its boundary".into(),
        ));
    }
    let url = Url::parse(value)
        .map_err(|error| ToolError::Invalid(format!("invalid shadow service URL: {error}")))?;
    let loopback = match url.host() {
        Some(Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
        }
        Some(Host::Ipv4(host)) => host == std::net::Ipv4Addr::LOCALHOST,
        Some(Host::Ipv6(host)) => host == std::net::Ipv6Addr::LOCALHOST,
        None => false,
    };
    if url.scheme() != scheme
        || !loopback
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (scheme == "tcp" && (!url.path().is_empty() || url.query().is_some()))
        || expected_status.is_some_and(|status| !(100..=599).contains(&status))
    {
        return Err(ToolError::Denied(
            "shadow service probes require credential-free loopback HTTP or TCP with an explicit port"
                .into(),
        ));
    }
    if let SuccessCondition::ElementExists { selector, .. } = condition {
        if selector.trim().is_empty()
            || selector.len() > SHADOW_MAX_SELECTOR_BYTES
            || selector.contains('\0')
            || selector.chars().any(char::is_control)
        {
            return Err(ToolError::Invalid(
                "shadow element selector exceeds its bounded CSS-selector contract".into(),
            ));
        }
        Ok(ShadowServiceProbe::ElementExists {
            url: value.clone(),
            selector: selector.clone(),
        })
    } else if matches!(condition, SuccessCondition::PageRenders { .. }) {
        Ok(ShadowServiceProbe::PageRender { url: value.clone() })
    } else if scheme == "tcp" {
        Ok(ShadowServiceProbe::Tcp {
            endpoint: value.clone(),
        })
    } else {
        Ok(ShadowServiceProbe::Http {
            url: value.clone(),
            expected_status,
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn discover_node_toolchain(executable: &str) -> Result<Option<NodeToolchain>, ToolError> {
    let name = Path::new(executable)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        "node" | "npm" | "npx" | "pnpm" | "pnpx" | "yarn" | "yarnpkg" | "corepack"
    ) {
        return Ok(None);
    }
    let Some(requested_candidate) = resolve_path_executable(executable) else {
        return Ok(None);
    };
    let runtime_candidate = if name == "node" {
        requested_candidate.clone()
    } else {
        resolve_path_executable("node").ok_or_else(|| {
            ToolError::Unavailable(
                "Node package-manager command has no runtime in the same environment".into(),
            )
        })?
    };
    let runtime_candidate = runtime_candidate
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve Node runtime: {error}")))?;
    #[cfg(not(target_os = "windows"))]
    if runtime_candidate.starts_with("/usr/bin") || runtime_candidate.starts_with("/bin") {
        return Ok(None);
    }
    let toolchain = node_toolchain_from_candidate(&runtime_candidate)?;
    let requested_candidate = requested_candidate
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve Node command: {error}")))?;
    if !requested_candidate.starts_with(&toolchain.host_root) {
        return Err(ToolError::Denied(
            "Node-family command resolves outside its runtime distribution".into(),
        ));
    }
    Ok(Some(toolchain))
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn node_toolchain_from_candidate(candidate: &Path) -> Result<NodeToolchain, ToolError> {
    let candidate = candidate
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve Node command: {error}")))?;
    let parent = candidate
        .parent()
        .ok_or_else(|| ToolError::Unavailable("Node executable has no parent directory".into()))?;
    let root = if parent.file_name().is_some_and(|value| value == "bin") {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    }
    .canonicalize()
    .map_err(|error| ToolError::Unavailable(format!("resolve Node toolchain: {error}")))?;
    let node = if cfg!(target_os = "windows") {
        root.join("node.exe")
    } else {
        root.join("bin/node")
    };
    let node_metadata = fs::symlink_metadata(&node)
        .map_err(|error| ToolError::Unavailable(format!("inspect Node runtime: {error}")))?;
    if metadata_is_link_or_reparse(&node_metadata) || !node_metadata.is_file() {
        return Err(ToolError::Unavailable(
            "Node toolchain does not contain one regular runtime executable".into(),
        ));
    }
    if !candidate.starts_with(&root) {
        return Err(ToolError::Denied(
            "Node command resolves outside its toolchain root".into(),
        ));
    }
    verify_node_toolchain_bounds(&root)?;
    Ok(NodeToolchain { host_root: root })
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn discover_browser_runtime() -> Result<Option<BrowserRuntime>, ToolError> {
    let Some(candidate) = std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from) else {
        return Ok(None);
    };
    let candidate = candidate
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve browser runtime: {error}")))?;
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| ToolError::Unavailable(format!("inspect browser runtime: {error}")))?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(ToolError::Denied(
            "browser runtime must be one regular executable".into(),
        ));
    }
    let host_root = candidate
        .parent()
        .ok_or_else(|| ToolError::Unavailable("browser executable has no parent".into()))?
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve browser root: {error}")))?;
    if !candidate.starts_with(&host_root) {
        return Err(ToolError::Denied(
            "browser executable escaped its distribution root".into(),
        ));
    }
    let executable_name = candidate
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\', '\0']))
        .ok_or_else(|| ToolError::Unavailable("browser executable name is invalid".into()))?
        .to_owned();
    verify_browser_runtime_bounds(&host_root)?;
    Ok(Some(BrowserRuntime {
        host_root,
        executable_name,
    }))
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn verify_browser_runtime_bounds(root: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| ToolError::Unavailable(format!("inspect browser root: {error}")))?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ToolError::Denied(
            "browser runtime root must be a real directory".into(),
        ));
    }
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > BROWSER_RUNTIME_MAX_DEPTH {
            return Err(ToolError::Unavailable(
                "browser runtime exceeds the depth limit".into(),
            ));
        }
        for entry in fs::read_dir(&directory)
            .map_err(|error| ToolError::Unavailable(format!("read browser runtime: {error}")))?
        {
            let entry = entry.map_err(|error| ToolError::Unavailable(error.to_string()))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ToolError::Unavailable(error.to_string()))?;
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path)
                    .map_err(|error| ToolError::Unavailable(error.to_string()))?;
                if target.is_absolute()
                    || path
                        .canonicalize()
                        .ok()
                        .is_none_or(|resolved| !resolved.starts_with(root))
                {
                    return Err(ToolError::Denied(
                        "browser runtime contains an escaping symbolic link".into(),
                    ));
                }
                files = files.saturating_add(1);
            } else if metadata_is_link_or_reparse(&metadata) {
                return Err(ToolError::Denied(
                    "browser runtime contains a reparse point".into(),
                ));
            } else if metadata.is_dir() {
                pending.push((path, depth.saturating_add(1)));
            } else if metadata.is_file() {
                files = files.saturating_add(1);
                bytes = bytes.checked_add(metadata.len()).ok_or_else(|| {
                    ToolError::Unavailable("browser runtime size overflow".into())
                })?;
            } else {
                return Err(ToolError::Denied(
                    "browser runtime contains a non-regular entry".into(),
                ));
            }
            if files > BROWSER_RUNTIME_MAX_FILES || bytes > BROWSER_RUNTIME_MAX_BYTES {
                return Err(ToolError::Unavailable(
                    "browser runtime exceeds its file or byte limit".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn resolve_path_executable(executable: &str) -> Option<PathBuf> {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        return path.is_file().then(|| path.to_path_buf());
    }
    let path_value = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_value) {
        #[cfg(target_os = "windows")]
        let names = if path.extension().is_some() {
            vec![executable.to_owned()]
        } else {
            vec![
                executable.to_owned(),
                format!("{executable}.exe"),
                format!("{executable}.cmd"),
                format!("{executable}.bat"),
            ]
        };
        #[cfg(not(target_os = "windows"))]
        let names = vec![executable.to_owned()];
        for name in names {
            let candidate = directory.join(name);
            if fs::symlink_metadata(&candidate)
                .ok()
                .is_some_and(|metadata| metadata.is_file() || metadata.file_type().is_symlink())
            {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn verify_node_toolchain_bounds(root: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| ToolError::Unavailable(format!("inspect Node toolchain: {error}")))?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ToolError::Denied(
            "Node toolchain root must be a real directory".into(),
        ));
    }
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > NODE_TOOLCHAIN_MAX_DEPTH {
            return Err(ToolError::Unavailable(
                "Node toolchain exceeds the depth limit".into(),
            ));
        }
        for entry in fs::read_dir(&directory)
            .map_err(|error| ToolError::Unavailable(format!("read Node toolchain: {error}")))?
        {
            let entry = entry.map_err(|error| ToolError::Unavailable(error.to_string()))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ToolError::Unavailable(error.to_string()))?;
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path)
                    .map_err(|error| ToolError::Unavailable(error.to_string()))?;
                if target.is_absolute()
                    || path
                        .canonicalize()
                        .ok()
                        .is_none_or(|resolved| !resolved.starts_with(root))
                {
                    return Err(ToolError::Denied(
                        "Node toolchain contains an escaping symbolic link".into(),
                    ));
                }
                files = files.saturating_add(1);
            } else if metadata_is_link_or_reparse(&metadata) {
                return Err(ToolError::Denied(
                    "Node toolchain contains a reparse point".into(),
                ));
            } else if metadata.is_dir() {
                pending.push((path, depth.saturating_add(1)));
            } else if metadata.is_file() {
                files = files.saturating_add(1);
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| ToolError::Unavailable("Node toolchain size overflow".into()))?;
            } else {
                return Err(ToolError::Denied(
                    "Node toolchain contains a non-regular entry".into(),
                ));
            }
            if files > NODE_TOOLCHAIN_MAX_FILES || bytes > NODE_TOOLCHAIN_MAX_BYTES {
                return Err(ToolError::Unavailable(
                    "Node toolchain exceeds its file or byte limit".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
#[allow(clippy::too_many_lines)] // Keep the complete mount, namespace, limit, and toolchain profile auditable.
fn bubblewrap_arguments(
    workspace_root: &Path,
    proof_path: &Path,
    executable: &str,
    command_arguments: &[String],
    toolchain: Option<&NodeToolchain>,
) -> Result<Vec<String>, ToolError> {
    let workspace = workspace_root
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow workspace path is not Unicode".into()))?;
    let proof = proof_path
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow proof path is not Unicode".into()))?;
    let sandbox_path = if toolchain.is_some() {
        "/mcg-toolchain/bin:/usr/bin:/bin"
    } else {
        "/usr/bin:/bin"
    };
    let mut argv = vec![
        "--unshare-all".into(),
        "--unshare-user".into(),
        "--disable-userns".into(),
        "--die-with-parent".into(),
        "--new-session".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--hostname".into(),
        "mcg-shadow".into(),
        "--clearenv".into(),
        "--setenv".into(),
        "PATH".into(),
        sandbox_path.into(),
        "--setenv".into(),
        "HOME".into(),
        "/nonexistent".into(),
        "--setenv".into(),
        "TMPDIR".into(),
        "/tmp".into(),
        "--ro-bind".into(),
        "/usr".into(),
        "/usr".into(),
        "--ro-bind-try".into(),
        "/bin".into(),
        "/bin".into(),
        "--ro-bind-try".into(),
        "/lib".into(),
        "/lib".into(),
        "--ro-bind-try".into(),
        "/lib64".into(),
        "/lib64".into(),
        "--dir".into(),
        "/etc".into(),
        "--ro-bind-try".into(),
        "/etc/ld.so.cache".into(),
        "/etc/ld.so.cache".into(),
        "--ro-bind-try".into(),
        "/etc/alternatives".into(),
        "/etc/alternatives".into(),
        "--ro-bind-try".into(),
        "/etc/ssl/certs".into(),
        "/etc/ssl/certs".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
        "--dir".into(),
        "/home".into(),
        "--dir".into(),
        "/run".into(),
        "--ro-bind".into(),
        workspace.into(),
        "/source".into(),
        "--size".into(),
        BWRAP_TMPFS_BYTES.to_string(),
        "--tmpfs".into(),
        "/workspace".into(),
    ];
    if let Some(toolchain) = toolchain {
        let host_root = toolchain
            .host_root
            .to_str()
            .ok_or_else(|| ToolError::Invalid("Node toolchain path is not Unicode".into()))?;
        argv.extend([
            "--ro-bind".into(),
            host_root.into(),
            "/mcg-toolchain".into(),
        ]);
    }
    argv.extend([
        "--bind".into(),
        proof.into(),
        "/mcg-proof".into(),
        "--chdir".into(),
        "/workspace".into(),
        "--".into(),
        "/usr/bin/prlimit".into(),
        format!("--cpu={BWRAP_TIMEOUT_SECONDS}"),
        format!("--as={BWRAP_ADDRESS_SPACE_BYTES}"),
        format!("--fsize={BWRAP_MAX_FILE_BYTES}"),
        format!("--nproc={BWRAP_MAX_PROCESSES}"),
        format!("--nofile={BWRAP_MAX_OPEN_FILES}"),
        "--".into(),
        "/bin/sh".into(),
        "-c".into(),
        "umask 077; /bin/cp -R /source/. /workspace/ || exit 125; printf 'mcg-shadow-bwrap-v1\\n' > /mcg-proof || exit 125; cd /workspace || exit 125; exec \"$@\"".into(),
        "mcg-shadow".into(),
        executable.into(),
    ]);
    argv.extend(command_arguments.iter().cloned());
    Ok(argv)
}

#[cfg(any(target_os = "linux", test))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "keep the complete same-sandbox service and probe profile auditable"
)]
fn bubblewrap_service_arguments(
    workspace_root: &Path,
    proof_path: &Path,
    executable: &str,
    command_arguments: &[String],
    probe: &ShadowServiceProbe,
    toolchain: Option<&NodeToolchain>,
    browser: Option<&BrowserRuntime>,
    runner: Option<&Path>,
    runner_request: Option<&Path>,
) -> Result<Vec<String>, ToolError> {
    let workspace = workspace_root
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow workspace path is not Unicode".into()))?;
    let proof = proof_path
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow proof path is not Unicode".into()))?;
    let sandbox_path = if toolchain.is_some() {
        "/mcg-toolchain/bin:/usr/bin:/bin"
    } else {
        "/usr/bin:/bin"
    };
    let use_runner = probe.requires_browser();
    if use_runner != (runner.is_some() && runner_request.is_some()) {
        return Err(ToolError::Invalid(
            "browser sandbox runner mappings are incomplete".into(),
        ));
    }
    let sandbox_source = if use_runner { "/mcg-source" } else { "/source" };
    let address_space_bytes = if probe.requires_browser() {
        BWRAP_BROWSER_ADDRESS_SPACE_BYTES
    } else {
        BWRAP_ADDRESS_SPACE_BYTES
    };
    let mut argv = vec![
        "--unshare-all".into(),
        "--unshare-user".into(),
        "--disable-userns".into(),
        "--die-with-parent".into(),
        "--new-session".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--hostname".into(),
        "mcg-shadow".into(),
        "--clearenv".into(),
        "--setenv".into(),
        "PATH".into(),
        sandbox_path.into(),
        "--setenv".into(),
        "HOME".into(),
        "/nonexistent".into(),
        "--setenv".into(),
        "TMPDIR".into(),
        "/tmp".into(),
        "--ro-bind".into(),
        "/usr".into(),
        "/usr".into(),
        "--ro-bind-try".into(),
        "/bin".into(),
        "/bin".into(),
        "--ro-bind-try".into(),
        "/lib".into(),
        "/lib".into(),
        "--ro-bind-try".into(),
        "/lib64".into(),
        "/lib64".into(),
        "--dir".into(),
        "/etc".into(),
        "--ro-bind-try".into(),
        "/etc/ld.so.cache".into(),
        "/etc/ld.so.cache".into(),
        "--ro-bind-try".into(),
        "/etc/alternatives".into(),
        "/etc/alternatives".into(),
        "--ro-bind-try".into(),
        "/etc/ssl/certs".into(),
        "/etc/ssl/certs".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
        "--dir".into(),
        "/home".into(),
        "--dir".into(),
        "/run".into(),
        "--ro-bind".into(),
        workspace.into(),
        sandbox_source.into(),
        "--size".into(),
        BWRAP_TMPFS_BYTES.to_string(),
        "--tmpfs".into(),
        "/workspace".into(),
    ];
    if let Some(toolchain) = toolchain {
        let host_root = toolchain
            .host_root
            .to_str()
            .ok_or_else(|| ToolError::Invalid("Node toolchain path is not Unicode".into()))?;
        argv.extend([
            "--ro-bind".into(),
            host_root.into(),
            "/mcg-toolchain".into(),
        ]);
    }
    if let Some(browser) = browser {
        let host_root = browser
            .host_root
            .to_str()
            .ok_or_else(|| ToolError::Invalid("browser runtime path is not Unicode".into()))?;
        argv.extend(["--ro-bind".into(), host_root.into(), "/mcg-browser".into()]);
    }
    if use_runner {
        let runner = runner
            .and_then(Path::to_str)
            .ok_or_else(|| ToolError::Invalid("shadow runner path is not Unicode".into()))?;
        let runner_request = runner_request.and_then(Path::to_str).ok_or_else(|| {
            ToolError::Invalid("shadow runner request path is not Unicode".into())
        })?;
        argv.extend([
            "--ro-bind".into(),
            runner.into(),
            "/mcg-shadow-runner".into(),
            "--ro-bind".into(),
            runner_request.into(),
            "/mcg-request.json".into(),
            "--bind".into(),
            proof.into(),
            "/mcg-proof".into(),
            "--chdir".into(),
            "/tmp".into(),
            "--".into(),
            "/usr/bin/prlimit".into(),
            format!("--cpu={BWRAP_TIMEOUT_SECONDS}"),
            format!("--as={address_space_bytes}"),
            format!("--fsize={BWRAP_MAX_FILE_BYTES}"),
            format!("--nproc={BWRAP_MAX_PROCESSES}"),
            format!("--nofile={BWRAP_MAX_OPEN_FILES}"),
            "--".into(),
            "/bin/sh".into(),
            "-c".into(),
            "printf 'mcg-shadow-bwrap-v1\\n' > /mcg-proof || exit 125; exec /mcg-shadow-runner /mcg-request.json".into(),
        ]);
        return Ok(argv);
    }
    argv.extend([
        "--bind".into(),
        proof.into(),
        "/mcg-proof".into(),
        "--chdir".into(),
        "/workspace".into(),
        "--".into(),
        "/usr/bin/prlimit".into(),
        format!("--cpu={BWRAP_TIMEOUT_SECONDS}"),
        format!("--as={address_space_bytes}"),
        format!("--fsize={BWRAP_MAX_FILE_BYTES}"),
        format!("--nproc={BWRAP_MAX_PROCESSES}"),
        format!("--nofile={BWRAP_MAX_OPEN_FILES}"),
        "--".into(),
        "/bin/sh".into(),
        "-c".into(),
        "umask 077; /bin/cp -R /source/. /workspace/ || exit 125; printf 'mcg-shadow-bwrap-v1\\n' > /mcg-proof || exit 125; cd /workspace || exit 125; probe_kind=$1; probe_value=$2; expected=$3; host=$4; port=$5; shift 5; HOST=\"$host\" PORT=\"$port\" \"$@\" >/dev/null 2>/dev/null & service_pid=$!; cleanup_service() { kill -TERM \"$service_pid\" 2>/dev/null || true; wait \"$service_pid\" 2>/dev/null || true; }; trap cleanup_service EXIT INT TERM; attempt=0; while [ \"$attempt\" -lt 300 ]; do if [ \"$probe_kind\" = tcp ]; then /bin/bash -c 'exec 3<>/dev/tcp/$1/$2' mcg-tcp \"$host\" \"$port\" 2>/dev/null && exit 0; else status=$(/usr/bin/curl --silent --output /dev/null --write-out '%{http_code}' --max-time 1 --max-filesize 1048576 -- \"$probe_value\") || status=''; if [ -n \"$status\" ]; then if [ \"$expected\" = 2xx ]; then case \"$status\" in 2??) exit 0;; *) exit 1;; esac; elif [ \"$status\" = \"$expected\" ]; then exit 0; else exit 1; fi; fi; fi; kill -0 \"$service_pid\" 2>/dev/null || exit 1; attempt=$((attempt + 1)); /usr/bin/sleep 0.1; done; exit 1".into(),
        "mcg-shadow-service".into(),
        probe.kind().into(),
        probe.endpoint().into(),
        probe
            .expected_status()
            .map_or_else(|| "2xx".into(), |status| status.to_string()),
        probe_host(probe.endpoint())?.into(),
        probe_port(probe.endpoint())?.to_string(),
        executable.into(),
    ]);
    argv.extend(command_arguments.iter().cloned());
    Ok(argv)
}

#[cfg(any(target_os = "linux", test))]
fn probe_port(value: &str) -> Result<u16, ToolError> {
    Url::parse(value)
        .ok()
        .and_then(|url| url.port())
        .ok_or_else(|| ToolError::Invalid("shadow service URL has no explicit port".into()))
}

#[cfg(any(target_os = "linux", test))]
fn probe_host(value: &str) -> Result<&'static str, ToolError> {
    let url = Url::parse(value)
        .map_err(|error| ToolError::Invalid(format!("invalid shadow service URL: {error}")))?;
    match url.host() {
        Some(Host::Ipv6(host)) if host == std::net::Ipv6Addr::LOCALHOST => Ok("::1"),
        Some(Host::Domain(host))
            if host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" =>
        {
            Ok("127.0.0.1")
        }
        Some(Host::Domain("::1")) => Ok("::1"),
        Some(Host::Ipv4(host)) if host == std::net::Ipv4Addr::LOCALHOST => Ok("127.0.0.1"),
        _ => Err(ToolError::Denied(
            "shadow service probe host is not exact loopback".into(),
        )),
    }
}

#[cfg(any(target_os = "linux", test))]
fn capture_optional_output(
    artifacts: &ArtifactStore,
    output: Option<&str>,
) -> Result<Option<ArtifactReference>, ToolError> {
    output
        .filter(|output| !output.is_empty())
        .map(|output| artifacts.capture(output.as_bytes(), "text/plain; charset=utf-8"))
        .transpose()
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn capture_shadow_transcript(
    artifacts: &ArtifactStore,
    condition: &SuccessCondition,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: Option<ArtifactReference>,
    stderr: Option<ArtifactReference>,
) -> Result<ShadowCommandVerification, ToolError> {
    let transcript = ShadowCommandTranscript {
        schema_version: SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION,
        condition: condition.clone(),
        exit_code,
        timed_out,
        stdout,
        stderr,
    };
    let bytes = serde_json::to_vec(&transcript)
        .map_err(|error| ToolError::Execution(format!("serialize transcript: {error}")))?;
    let transcript =
        artifacts.capture(&bytes, "application/vnd.mcg.shadow-command+json;version=1")?;
    Ok(ShadowCommandVerification::Completed { transcript })
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn verify_workspace_bounds(root: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| ToolError::Execution(format!("inspect shadow workspace: {error}")))?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ToolError::Denied(
            "shadow command workspace must be a real directory".into(),
        ));
    }
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > BWRAP_MAX_SOURCE_DEPTH {
            return Err(ToolError::Unavailable(
                "shadow command workspace exceeds the depth limit".into(),
            ));
        }
        for entry in fs::read_dir(&directory)
            .map_err(|error| ToolError::Execution(format!("read shadow workspace: {error}")))?
        {
            let entry = entry.map_err(|error| ToolError::Execution(error.to_string()))?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(ToolError::Denied(
                    "shadow command workspace cannot contain symbolic links".into(),
                ));
            }
            if metadata.is_dir() {
                pending.push((entry.path(), depth.saturating_add(1)));
            } else if metadata.is_file() {
                files = files.saturating_add(1);
                bytes = bytes.saturating_add(metadata.len());
                if files > BWRAP_MAX_SOURCE_FILES || bytes > BWRAP_MAX_SOURCE_BYTES {
                    return Err(ToolError::Unavailable(
                        "shadow command workspace exceeds its file or byte limit".into(),
                    ));
                }
            } else {
                return Err(ToolError::Denied(
                    "shadow command workspace contains a non-regular entry".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

#[cfg(any(target_os = "linux", test))]
struct EphemeralProof {
    path: PathBuf,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Serialize)]
struct LinuxRunnerRequest<'a> {
    schema_version: u16,
    source_root: &'static str,
    workspace_root: &'static str,
    executable: &'a str,
    argv: &'a [String],
    toolchain_root: Option<&'static str>,
    browser_executable: String,
    probe: LinuxRunnerProbe<'a>,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum LinuxRunnerProbe<'a> {
    PageRender { url: &'a str },
    ElementExists { url: &'a str, selector: &'a str },
}

#[cfg(any(target_os = "linux", test))]
struct LinuxRunnerRequestFile {
    path: PathBuf,
}

#[cfg(any(target_os = "linux", test))]
impl LinuxRunnerRequestFile {
    fn create(
        workspace_root: &Path,
        executable: &str,
        argv: &[String],
        toolchain: Option<&NodeToolchain>,
        browser: &BrowserRuntime,
        probe: &ShadowServiceProbe,
    ) -> Result<Self, ToolError> {
        let probe = match probe {
            ShadowServiceProbe::PageRender { url } => LinuxRunnerProbe::PageRender { url },
            ShadowServiceProbe::ElementExists { url, selector } => {
                LinuxRunnerProbe::ElementExists { url, selector }
            }
            _ => {
                return Err(ToolError::Invalid(
                    "Linux runner request requires a browser probe".into(),
                ));
            }
        };
        let parent = workspace_root
            .parent()
            .ok_or_else(|| ToolError::Invalid("shadow workspace has no parent".into()))?;
        let path = parent.join(format!(".mcg-shadow-request-{}.json", Uuid::new_v4()));
        let request = LinuxRunnerRequest {
            schema_version: 5,
            source_root: "/mcg-source",
            workspace_root: "/tmp/mcg-workspace",
            executable,
            argv,
            toolchain_root: toolchain.map(|_| "/mcg-toolchain"),
            browser_executable: format!("/mcg-browser/{}", browser.executable_name),
            probe,
        };
        let bytes = serde_json::to_vec(&request).map_err(|error| {
            ToolError::Execution(format!("serialize Linux shadow-runner request: {error}"))
        })?;
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|error| {
            ToolError::Execution(format!("create Linux shadow-runner request: {error}"))
        })?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|error| {
            ToolError::Execution(format!("write Linux shadow-runner request: {error}"))
        })?;
        file.sync_all().map_err(|error| {
            ToolError::Execution(format!("sync Linux shadow-runner request: {error}"))
        })?;
        Ok(Self { path })
    }
}

#[cfg(any(target_os = "linux", test))]
impl Drop for LinuxRunnerRequestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(any(target_os = "linux", test))]
impl EphemeralProof {
    fn create(workspace_root: &Path) -> Result<Self, ToolError> {
        let parent = workspace_root
            .parent()
            .ok_or_else(|| ToolError::Invalid("shadow workspace has no parent".into()))?;
        let path = parent.join(format!(".mcg-shadow-proof-{}", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.create_new(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&path)
            .map_err(|error| ToolError::Execution(format!("create shadow proof: {error}")))?;
        Ok(Self { path })
    }
}

#[cfg(any(target_os = "linux", test))]
impl Drop for EphemeralProof {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(target_os = "linux")]
fn trusted_bubblewrap_executable() -> Option<PathBuf> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    [
        Path::new("/usr/lib/MCG/mcg-bwrap"),
        Path::new("/usr/bin/bwrap"),
        Path::new("/bin/bwrap"),
    ]
    .into_iter()
    .filter_map(|path| path.canonicalize().ok())
    .find(|path| {
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        metadata.is_file()
            && metadata.uid() == 0
            && metadata.permissions().mode() & 0o6022 == 0
            && Path::new("/usr/bin/prlimit").is_file()
            && Path::new("/bin/sh").is_file()
            && Path::new("/bin/cp").is_file()
            && installed_bubblewrap_version_is_supported(path)
    })
}

#[cfg(target_os = "linux")]
fn trusted_linux_shadow_runner() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let expected = std::env::current_exe()
        .ok()?
        .parent()?
        .join("mcg-shadow-runner");
    let metadata = fs::symlink_metadata(&expected).ok()?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o111 == 0
    {
        return None;
    }
    expected.canonicalize().ok()
}

#[cfg(target_os = "linux")]
fn installed_bubblewrap_version_is_supported(executable: &Path) -> bool {
    std::process::Command::new(executable)
        .arg("--version")
        .env_clear()
        .output()
        .ok()
        .filter(|output| output.status.success() && output.stdout.len() <= 128)
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| bubblewrap_version_is_supported(&output))
}

#[cfg(any(target_os = "linux", test))]
fn bubblewrap_version_is_supported(output: &str) -> bool {
    let Some(version) = output.split_ascii_whitespace().last() else {
        return false;
    };
    let mut components = version.split('.');
    let parsed = (
        components
            .next()
            .and_then(|value| value.parse::<u32>().ok()),
        components
            .next()
            .and_then(|value| value.parse::<u32>().ok()),
        components
            .next()
            .and_then(|value| value.parse::<u32>().ok()),
    );
    matches!(parsed, (Some(major), Some(minor), Some(patch)) if (major, minor, patch) >= (0, 11, 2) && components.next().is_none())
}

#[cfg(target_os = "windows")]
fn trusted_windows_sandbox_components() -> Option<(PathBuf, PathBuf)> {
    let windows = std::env::var_os("WINDIR").map(PathBuf::from)?;
    let system32 = windows.join("System32");
    let executable = trusted_windows_regular_file(&system32.join("wsb.exe"))
        .or_else(|| trusted_packaged_windows_sandbox_cli(&system32))?;

    let runner = std::env::current_exe()
        .ok()?
        .parent()?
        .join("mcg-shadow-runner.exe");
    let runner = trusted_windows_regular_file(&runner)?;
    Some((executable, runner))
}

#[cfg(target_os = "windows")]
fn trusted_packaged_windows_sandbox_cli(system32: &StdPath) -> Option<PathBuf> {
    const MAX_DISCOVERY_OUTPUT_BYTES: usize = 4 * 1024;
    const PACKAGE_FAMILY_PREFIX: &str = "MicrosoftWindows.WindowsSandbox_";
    const PACKAGE_PUBLISHER_SUFFIX: &str = "__cw5n1h2txyewy";

    let powershell =
        trusted_windows_regular_file(&system32.join("WindowsPowerShell/v1.0/powershell.exe"))?;
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$packages = @(Appx\\Get-AppxPackage -Name 'MicrosoftWindows.WindowsSandbox' -ErrorAction SilentlyContinue); if ($packages.Count -eq 1) { [Console]::Out.Write(($packages[0].InstallLocation)) }",
        ])
        .env_clear()
        .env("SystemRoot", system32.parent()?)
        .env("WINDIR", system32.parent()?)
        .env(
            "PSModulePath",
            system32.join("WindowsPowerShell/v1.0/Modules"),
        )
        .output()
        .ok()?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.stdout.len() > MAX_DISCOVERY_OUTPUT_BYTES
        || output.stderr.len() > MAX_DISCOVERY_OUTPUT_BYTES
    {
        return None;
    }
    let package_location = std::str::from_utf8(&output.stdout).ok()?.trim();
    if package_location.is_empty() || package_location.lines().count() != 1 {
        return None;
    }
    let package_location = PathBuf::from(package_location);
    let package_name = package_location.file_name()?.to_str()?;
    if !package_name.starts_with(PACKAGE_FAMILY_PREFIX)
        || !package_name.ends_with(PACKAGE_PUBLISHER_SUFFIX)
    {
        return None;
    }

    if !package_location.is_absolute()
        || package_location.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return None;
    }
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from)?;
    let windows_apps = program_files.join("WindowsApps");
    if !windows_paths_equal(package_location.parent()?, &windows_apps) {
        return None;
    }
    if !trusted_windows_directory(&package_location) {
        return None;
    }
    trusted_windows_package_file(&package_location.join("wsb.exe"))
}

#[cfg(target_os = "windows")]
fn trusted_windows_regular_file(expected: &StdPath) -> Option<PathBuf> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let executable = expected.canonicalize().ok()?;
    if !windows_paths_equal(&executable, expected) {
        return None;
    }
    let metadata = fs::symlink_metadata(&executable).ok()?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return None;
    }
    Some(executable)
}

#[cfg(target_os = "windows")]
fn windows_paths_equal(left: &StdPath, right: &StdPath) -> bool {
    use std::os::windows::ffi::OsStrExt;

    fn normalized(path: &StdPath) -> Vec<u16> {
        const VERBATIM_PREFIX: &[u16] = &[92, 92, 63, 92];

        let mut units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if units.starts_with(VERBATIM_PREFIX) {
            units.drain(..VERBATIM_PREFIX.len());
        }
        for unit in &mut units {
            if *unit == u16::from(b'/') {
                *unit = u16::from(b'\\');
            } else if let Ok(ascii) = u8::try_from(*unit) {
                *unit = u16::from(ascii.to_ascii_lowercase());
            }
        }
        units
    }

    normalized(left) == normalized(right)
}

#[cfg(target_os = "windows")]
fn trusted_windows_directory(path: &StdPath) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    fs::symlink_metadata(path).ok().is_some_and(|metadata| {
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
    })
}

#[cfg(target_os = "windows")]
fn trusted_windows_package_file(path: &StdPath) -> Option<PathBuf> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return None;
    }
    Some(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "windows")]
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[test]
    fn bubblewrap_profile_is_networkless_read_only_and_argument_vector_safe() {
        let workspace = tempfile::tempdir().expect("workspace");
        let proof = EphemeralProof::create(workspace.path()).expect("proof");
        let argv = bubblewrap_arguments(
            workspace.path(),
            &proof.path,
            "npm",
            &["test;touch".into(), "/outside".into()],
            None,
        )
        .expect("profile");
        assert!(
            argv.windows(3)
                .any(|arguments| arguments
                    == ["--unshare-all", "--unshare-user", "--disable-userns"])
        );
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["--new-session", "--cap-drop"])
        );
        assert!(!argv.iter().any(|argument| argument == "--share-net"));
        assert!(argv.windows(2).any(|pair| pair
            == [
                "--ro-bind",
                workspace.path().to_str().expect("unicode workspace")
            ]));
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["--tmpfs", "/workspace"])
        );
        assert_eq!(argv[argv.len() - 3..], ["npm", "test;touch", "/outside"]);
        assert_eq!(fs::read(&proof.path).expect("empty proof"), b"");
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One auditable profile covers URL, namespace, toolchain, and browser boundaries.
    fn service_probe_and_bubblewrap_profile_are_loopback_only_and_argument_safe() {
        let accepted = [
            "http://127.0.0.1:43123/health",
            "http://localhost:43123/health?ready=1",
            "http://[::1]:43123/health",
        ];
        for url in accepted {
            let probe = shadow_service_probe(&SuccessCondition::HttpResponds {
                url: url.into(),
                status: 204,
            })
            .expect("exact loopback URL");
            assert_eq!(probe.expected_status(), Some(204));
        }
        for endpoint in [
            "tcp://127.0.0.1:43123",
            "tcp://localhost:43123",
            "tcp://[::1]:43123",
        ] {
            assert!(matches!(
                shadow_service_probe(&SuccessCondition::PortListening {
                    endpoint: endpoint.into(),
                })
                .unwrap_or_else(|error| panic!("exact loopback TCP endpoint {endpoint}: {error}")),
                ShadowServiceProbe::Tcp { .. }
            ));
        }
        assert!(matches!(
            shadow_service_probe(&SuccessCondition::PageRenders {
                url: "http://127.0.0.1:43123/app".into(),
            })
            .expect("exact loopback page URL"),
            ShadowServiceProbe::PageRender { .. }
        ));
        assert!(matches!(
            shadow_service_probe(&SuccessCondition::ElementExists {
                url: "http://127.0.0.1:43123/app".into(),
                selector: "main#app[data-state='ready']".into(),
            })
            .expect("exact loopback element assertion"),
            ShadowServiceProbe::ElementExists { .. }
        ));
        for selector in [
            String::new(),
            " \t".to_owned(),
            "x".repeat(SHADOW_MAX_SELECTOR_BYTES + 1),
        ] {
            assert!(
                shadow_service_probe(&SuccessCondition::ElementExists {
                    url: "http://127.0.0.1:43123/app".into(),
                    selector,
                })
                .is_err()
            );
        }
        for url in [
            "https://127.0.0.1:43123/health",
            "http://127.0.0.1/health",
            "http://127.0.0.2:43123/health",
            "http://example.com:43123/health",
            "http://user@127.0.0.1:43123/health",
            "http://127.0.0.1:43123/health#fragment",
            "http://127.0.0.1:43123/health\r\nX-Test: bad",
        ] {
            assert!(
                shadow_service_probe(&SuccessCondition::ServiceHealthy {
                    endpoint: url.into(),
                })
                .is_err(),
                "unsafe URL accepted: {url:?}"
            );
        }

        let workspace = tempfile::tempdir().expect("workspace");
        let proof = EphemeralProof::create(workspace.path()).expect("proof");
        let probe = ShadowServiceProbe::Http {
            url: "http://127.0.0.1:43123/health?value=literal;touch".into(),
            expected_status: Some(204),
        };
        let argv = bubblewrap_service_arguments(
            workspace.path(),
            &proof.path,
            "node",
            &["server.js;touch".into(), "literal argument".into()],
            &probe,
            Some(&NodeToolchain {
                host_root: workspace.path().to_path_buf(),
            }),
            None,
            None,
            None,
        )
        .expect("service profile");
        assert_eq!(
            argv.iter()
                .filter(|argument| *argument == "--unshare-all")
                .count(),
            1
        );
        assert!(
            argv.windows(3)
                .any(|arguments| arguments
                    == ["--unshare-all", "--unshare-user", "--disable-userns"])
        );
        assert!(!argv.iter().any(|argument| argument == "--share-net"));
        assert!(argv.windows(2).any(|pair| pair
            == [
                "--ro-bind",
                workspace.path().to_str().expect("workspace path")
            ]));
        assert!(argv.windows(2).any(|pair| pair
            == [
                "--ro-bind",
                workspace.path().to_str().expect("toolchain path")
            ]));
        let shell_index = argv
            .iter()
            .position(|argument| argument == "mcg-shadow-service")
            .expect("shell argument boundary");
        assert!(!argv[shell_index - 1].contains(probe.endpoint()));
        assert_eq!(
            &argv[shell_index + 1..],
            [
                "http",
                probe.endpoint(),
                "204",
                "127.0.0.1",
                "43123",
                "node",
                "server.js;touch",
                "literal argument",
            ]
        );

        let browser_root = tempfile::tempdir().expect("browser root");
        let browser = BrowserRuntime {
            host_root: browser_root.path().to_path_buf(),
            executable_name: "chrome".into(),
        };
        let page_probe = ShadowServiceProbe::PageRender {
            url: "http://127.0.0.1:43123/app".into(),
        };
        let runner = workspace.path().join("mcg-shadow-runner");
        let request = workspace.path().join("request.json");
        fs::write(&runner, b"runner").expect("runner fixture");
        fs::write(&request, b"{}").expect("request fixture");
        let page_argv = bubblewrap_service_arguments(
            workspace.path(),
            &proof.path,
            "node",
            &["server.js".into()],
            &page_probe,
            None,
            Some(&browser),
            Some(&runner),
            Some(&request),
        )
        .expect("page-render profile");
        assert!(page_argv.windows(3).any(|items| items
            == [
                "--ro-bind",
                browser_root.path().to_str().expect("browser root"),
                "/mcg-browser"
            ]));
        assert!(page_argv.windows(3).any(|items| items
            == [
                "--ro-bind",
                runner.to_str().expect("runner path"),
                "/mcg-shadow-runner"
            ]));
        assert!(page_argv.windows(3).any(|items| items
            == [
                "--ro-bind",
                request.to_str().expect("request path"),
                "/mcg-request.json"
            ]));
        assert!(
            page_argv
                .last()
                .is_some_and(|script| script.contains("exec /mcg-shadow-runner /mcg-request.json"))
        );
        assert!(
            !page_argv
                .iter()
                .any(|argument| argument.contains("--dump-dom"))
        );
    }

    #[test]
    fn command_and_workspace_boundaries_reject_ambiguous_or_unsafe_input() {
        assert!(bubblewrap_version_is_supported("bubblewrap 0.11.2\n"));
        assert!(bubblewrap_version_is_supported("bubblewrap 1.0.0\n"));
        assert!(!bubblewrap_version_is_supported("bubblewrap 0.11.1\n"));
        assert!(!bubblewrap_version_is_supported("unexpected"));
        let (executable, arguments) = split_command("cargo test --workspace").expect("command");
        assert_eq!(executable, "cargo");
        assert_eq!(arguments, ["test", "--workspace"]);
        assert!(split_command("").is_err());
        assert!(split_command(&"x".repeat(BWRAP_MAX_COMMAND_BYTES + 1)).is_err());
        assert!(
            condition_command(&SuccessCondition::Custom {
                description: "COMMAND SUCCEEDS npm test".into(),
            })
            .is_some()
        );
        assert!(
            condition_command(&SuccessCondition::Custom {
                description: "please run npm test".into(),
            })
            .is_none()
        );
        assert_eq!(
            condition_command(&SuccessCondition::ConfigurationValid {
                command: "npm run check:config".into(),
            }),
            Some("npm run check:config")
        );

        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("regular"), b"safe").expect("file");
        verify_workspace_bounds(workspace.path()).expect("bounded workspace");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), b"outside").expect("outside file");
        let link = workspace.path().join("escape");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path().join("secret"), &link).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(outside.path().join("secret"), &link).is_ok();
        if linked {
            assert!(verify_workspace_bounds(workspace.path()).is_err());
            fs::remove_file(&link).expect("remove link fixture");
        }

        let oversized = workspace.path().join("oversized");
        let oversized_file = fs::File::create(&oversized).expect("oversized fixture");
        oversized_file
            .set_len(BWRAP_MAX_SOURCE_BYTES + 1)
            .expect("sparse length");
        drop(oversized_file);
        assert!(verify_workspace_bounds(workspace.path()).is_err());
        fs::remove_file(oversized).expect("remove oversized fixture");

        let mut deep = workspace.path().to_path_buf();
        for _ in 0..=BWRAP_MAX_SOURCE_DEPTH {
            deep.push("d");
        }
        fs::create_dir_all(&deep).expect("deep fixture");
        assert!(verify_workspace_bounds(workspace.path()).is_err());
    }

    #[test]
    fn node_toolchain_mapping_is_read_only_bounded_and_rejects_link_escape() {
        let root = tempfile::tempdir().expect("toolchain");
        #[cfg(target_os = "windows")]
        let candidate = {
            fs::write(root.path().join("node.exe"), b"node").expect("node runtime");
            fs::write(root.path().join("npm.cmd"), b"npm").expect("npm command");
            root.path().join("npm.cmd")
        };
        #[cfg(not(target_os = "windows"))]
        let candidate = {
            fs::create_dir_all(root.path().join("bin")).expect("bin");
            fs::write(root.path().join("bin/node"), b"node").expect("node runtime");
            fs::write(root.path().join("bin/npm"), b"npm").expect("npm command");
            root.path().join("bin/npm")
        };
        let toolchain = node_toolchain_from_candidate(&candidate).expect("bounded toolchain");
        assert_eq!(
            toolchain.host_root,
            root.path().canonicalize().expect("root")
        );
        #[cfg(unix)]
        {
            let launchers = tempfile::tempdir().expect("launcher directory");
            let launcher = launchers.path().join("node");
            std::os::unix::fs::symlink(root.path().join("bin/node"), &launcher)
                .expect("owner-local launcher symlink");
            assert_eq!(
                node_toolchain_from_candidate(&launcher)
                    .expect("canonical launcher")
                    .host_root,
                toolchain.host_root
            );
        }

        let workspace = tempfile::tempdir().expect("workspace");
        let proof = EphemeralProof::create(workspace.path()).expect("proof");
        let argv = bubblewrap_arguments(
            workspace.path(),
            &proof.path,
            "npm",
            &["test".into()],
            Some(&toolchain),
        )
        .expect("Node profile");
        assert!(argv.windows(3).any(|values| {
            values
                == [
                    "--ro-bind",
                    toolchain.host_root.to_str().expect("Unicode toolchain"),
                    "/mcg-toolchain",
                ]
        }));
        assert!(
            argv.windows(2)
                .any(|values| { values == ["PATH", "/mcg-toolchain/bin:/usr/bin:/bin"] })
        );

        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), b"secret").expect("secret");
        let link = root.path().join("escape");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path().join("secret"), &link).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(outside.path().join("secret"), &link).is_ok();
        if linked {
            assert!(verify_node_toolchain_bounds(root.path()).is_err());
        }
    }

    #[tokio::test]
    async fn missing_bubblewrap_produces_no_completed_evidence() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let verifier = LinuxBubblewrapVerifier {
            executable: workspace.path().join("missing-bwrap"),
            runner: None,
        };
        let result = verifier
            .verify(
                workspace.path(),
                &SuccessCondition::Test {
                    command: "touch should-not-exist".into(),
                },
                &ArtifactStore::open(artifact_root.path()).expect("artifacts"),
            )
            .await;
        assert_eq!(result, ShadowCommandVerification::Unavailable);
        assert!(!workspace.path().join("should-not-exist").exists());
        assert_eq!(
            fs::read_dir(workspace.path())
                .expect("workspace entries")
                .count(),
            0
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn trusted_windows_path_comparison_accepts_only_equivalent_fixed_paths() {
        assert!(windows_paths_equal(
            Path::new(r"\\?\C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            Path::new(r"c:/windows/system32/windowspowershell/v1.0/POWERSHELL.EXE"),
        ));
        assert!(!windows_paths_equal(
            Path::new(r"\\?\C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            Path::new(r"C:\Windows\Temp\powershell.exe"),
        ));

        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .expect("WINDIR");
        assert!(
            trusted_windows_regular_file(
                &windows.join("System32/WindowsPowerShell/v1.0/powershell.exe")
            )
            .is_some(),
            "the fixed System32 Windows PowerShell binary must survive canonicalization"
        );
    }

    #[test]
    fn windows_sandbox_profile_is_locked_down_read_only_and_xml_safe() {
        let source = Path::new(r"C:\source & fixtures");
        let harness = Path::new(r"C:\harness <private>");
        let configuration = windows_sandbox_configuration(source, harness, None, None)
            .expect("Windows Sandbox profile");
        for setting in [
            "<vGPU>Disable</vGPU>",
            "<Networking>Disable</Networking>",
            "<AudioInput>Disable</AudioInput>",
            "<VideoInput>Disable</VideoInput>",
            "<PrinterRedirection>Disable</PrinterRedirection>",
            "<ClipboardRedirection>Disable</ClipboardRedirection>",
            "<ProtectedClient>Enable</ProtectedClient>",
        ] {
            assert!(configuration.contains(setting), "missing {setting}");
        }
        assert_eq!(configuration.matches("<MappedFolder>").count(), 2);
        assert_eq!(
            configuration.matches("<ReadOnly>true</ReadOnly>").count(),
            2
        );
        assert!(configuration.contains(r"C:\source &amp; fixtures"));
        assert!(configuration.contains(r"C:\harness &lt;private&gt;"));
        assert!(!configuration.contains("<LogonCommand>"));

        let toolchain = NodeToolchain {
            host_root: PathBuf::from(r"C:\node & tools"),
        };
        let configuration = windows_sandbox_configuration(source, harness, Some(&toolchain), None)
            .expect("Windows Sandbox Node profile");
        assert_eq!(configuration.matches("<MappedFolder>").count(), 3);
        assert_eq!(
            configuration.matches("<ReadOnly>true</ReadOnly>").count(),
            3
        );
        assert!(configuration.contains(r"C:\node &amp; tools"));
        assert!(configuration.contains(r"C:\mcg-toolchain"));

        let browser = BrowserRuntime {
            host_root: PathBuf::from(r"C:\browser & runtime"),
            executable_name: "chrome.exe".into(),
        };
        let configuration = windows_sandbox_configuration(source, harness, None, Some(&browser))
            .expect("Windows Sandbox browser profile");
        assert_eq!(configuration.matches("<MappedFolder>").count(), 3);
        assert_eq!(
            configuration.matches("<ReadOnly>true</ReadOnly>").count(),
            3
        );
        assert!(configuration.contains(r"C:\browser &amp; runtime"));
        assert!(configuration.contains(r"C:\mcg-browser"));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture keeps every versioned Windows probe variant auditable together.
    fn windows_service_request_is_schema_bound_and_probe_tagged() {
        let parent = tempfile::tempdir().expect("parent");
        let workspace = parent.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let runner = parent.path().join("mcg-shadow-runner.exe");
        fs::write(&runner, b"runner fixture").expect("runner");
        let probe = ShadowServiceProbe::Http {
            url: "http://127.0.0.1:43123/health".into(),
            expected_status: Some(204),
        };
        let harness = WindowsSandboxHarness::create(
            &workspace,
            &runner,
            "npm.cmd",
            &["run".into(), "start".into()],
            None,
            None,
            Some(&probe),
        )
        .expect("service harness");
        let request: serde_json::Value =
            serde_json::from_slice(&fs::read(harness.path.join("request.json")).expect("request"))
                .expect("request JSON");
        assert_eq!(request["schema_version"], 5);
        assert_eq!(request["source_root"], r"C:\mcg-source");
        assert_eq!(request["workspace_root"], r"C:\mcg-workspace");
        assert_eq!(request["executable"], "npm.cmd");
        assert_eq!(request["argv"], serde_json::json!(["run", "start"]));
        assert_eq!(request["toolchain_root"], serde_json::Value::Null);
        assert_eq!(request["browser_executable"], serde_json::Value::Null);
        assert_eq!(request["probe"]["kind"], "http_status");
        assert_eq!(request["probe"]["url"], probe.endpoint());
        assert_eq!(request["probe"]["expected_status"], 204);

        let tcp_probe = ShadowServiceProbe::Tcp {
            endpoint: "tcp://127.0.0.1:43123".into(),
        };
        let tcp_harness = WindowsSandboxHarness::create(
            &workspace,
            &runner,
            "npm.cmd",
            &["run".into(), "start".into()],
            None,
            None,
            Some(&tcp_probe),
        )
        .expect("TCP service harness");
        let tcp_request: serde_json::Value = serde_json::from_slice(
            &fs::read(tcp_harness.path.join("request.json")).expect("TCP request"),
        )
        .expect("TCP request JSON");
        assert_eq!(tcp_request["probe"]["kind"], "tcp_connect");
        assert_eq!(tcp_request["probe"]["endpoint"], tcp_probe.endpoint());

        let browser_root = parent.path().join("browser");
        fs::create_dir(&browser_root).expect("browser root");
        let browser = BrowserRuntime {
            host_root: browser_root,
            executable_name: "chrome.exe".into(),
        };
        let page_probe = ShadowServiceProbe::PageRender {
            url: "http://127.0.0.1:43123/app".into(),
        };
        let page_harness = WindowsSandboxHarness::create(
            &workspace,
            &runner,
            "npm.cmd",
            &["run".into(), "start".into()],
            None,
            Some(&browser),
            Some(&page_probe),
        )
        .expect("page-render harness");
        let page_request: serde_json::Value = serde_json::from_slice(
            &fs::read(page_harness.path.join("request.json")).expect("page request"),
        )
        .expect("page request JSON");
        assert_eq!(page_request["probe"]["kind"], "page_render");
        assert_eq!(page_request["probe"]["url"], page_probe.endpoint());
        assert_eq!(
            page_request["browser_executable"],
            r"C:\mcg-browser\chrome.exe"
        );

        let element_probe = ShadowServiceProbe::ElementExists {
            url: "http://127.0.0.1:43123/app".into(),
            selector: "main#app[data-state='ready']".into(),
        };
        let element_harness = WindowsSandboxHarness::create(
            &workspace,
            &runner,
            "npm.cmd",
            &["run".into(), "start".into()],
            None,
            Some(&browser),
            Some(&element_probe),
        )
        .expect("element harness");
        let element_request: serde_json::Value = serde_json::from_slice(
            &fs::read(element_harness.path.join("request.json")).expect("element request"),
        )
        .expect("element request JSON");
        assert_eq!(element_request["probe"]["kind"], "element_exists");
        assert_eq!(element_request["probe"]["url"], element_probe.endpoint());
        assert_eq!(
            element_request["probe"]["selector"],
            "main#app[data-state='ready']"
        );
    }

    #[test]
    fn windows_sandbox_raw_exec_requires_the_nested_exit_code() {
        assert_eq!(
            parse_windows_sandbox_exec_exit_code("{\"ExitCode\":0}"),
            Some(0)
        );
        assert_eq!(
            parse_windows_sandbox_exec_exit_code("{\r\n  \"ExitCode\": -1073741515\r\n}"),
            Some(-1_073_741_515)
        );
        assert_eq!(parse_windows_sandbox_exec_exit_code("{}"), None);
        assert_eq!(
            parse_windows_sandbox_exec_exit_code("{\"ExitCode\":\"0\"}"),
            None
        );
        assert_eq!(parse_windows_sandbox_exec_exit_code("not-json"), None);
        assert_eq!(parse_windows_sandbox_exec_exit_code(&" ".repeat(513)), None);
    }

    #[test]
    fn windows_sandbox_cli_uses_system_context_and_never_writable_shares() {
        let configuration = windows_sandbox_configuration(
            Path::new(r"C:\source"),
            Path::new(r"C:\harness"),
            None,
            None,
        )
        .expect("profile");
        let start = windows_sandbox_start_arguments("session", &configuration);
        assert_eq!(
            &start[..5],
            ["start", "--raw", "--id", "session", "--config"]
        );
        assert!(
            !start
                .iter()
                .any(|value| value == "share" || value == "--allow-write")
        );

        let exec =
            windows_sandbox_exec_arguments("session", WindowsSandboxRunnerInvocation::Request);
        assert!(exec.windows(2).any(|pair| pair == ["--run-as", "System"]));
        assert!(exec.windows(2).any(|pair| pair
            == [
                "--command",
                r"C:\mcg-harness\mcg-shadow-runner.exe C:\mcg-harness\request.json"
            ]));
        assert!(!exec.iter().any(|argument| argument == "--"));

        let probe =
            windows_sandbox_exec_arguments("session", WindowsSandboxRunnerInvocation::Probe);
        assert!(
            probe
                .windows(2)
                .any(|pair| pair == ["--command", r"C:\mcg-harness\mcg-shadow-runner.exe --probe"])
        );
        assert_eq!(windows_command_name("npm"), "npm.cmd");
        assert_eq!(windows_command_name("cargo"), "cargo");
    }

    #[tokio::test]
    async fn missing_windows_sandbox_produces_no_completed_evidence() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let fixture_root = tempfile::tempdir().expect("fixture root");
        let runner = fixture_root.path().join("mcg-shadow-runner.exe");
        fs::write(&runner, b"not executed").expect("runner fixture");
        let verifier = WindowsSandboxVerifier {
            executable: fixture_root.path().join("missing-wsb.exe"),
            runner,
            lifecycle: tokio::sync::Mutex::new(()),
            timeouts: WindowsSandboxTimeouts::default(),
        };
        let result = verifier
            .verify(
                workspace.path(),
                &SuccessCondition::Test {
                    command: "cmd.exe /C echo escaped>should-not-exist".into(),
                },
                &ArtifactStore::open(artifact_root.path()).expect("artifacts"),
            )
            .await;
        assert_eq!(result, ShadowCommandVerification::Unavailable);
        assert!(!workspace.path().join("should-not-exist").exists());
        assert_eq!(
            fs::read_dir(workspace.path())
                .expect("workspace entries")
                .count(),
            0
        );
    }

    #[cfg(target_os = "windows")]
    fn installed_windows_sandbox_verifier() -> WindowsSandboxVerifier {
        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .expect("WINDIR");
        let executable = trusted_windows_regular_file(&windows.join("System32/wsb.exe"))
            .or_else(|| trusted_packaged_windows_sandbox_cli(&windows.join("System32")))
            .expect("trusted Windows Sandbox CLI");
        let runner = std::env::var_os("MCG_WINDOWS_SANDBOX_RUNNER")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()?
                    .parent()?
                    .parent()
                    .map(|target| target.join("mcg-shadow-runner.exe"))
            })
            .and_then(|path| trusted_windows_regular_file(&path))
            .expect("built mcg-shadow-runner.exe");
        WindowsSandboxVerifier {
            executable,
            runner,
            lifecycle: tokio::sync::Mutex::new(()),
            timeouts: WindowsSandboxTimeouts::default(),
        }
    }

    #[cfg(target_os = "windows")]
    fn windows_sandbox_live_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    #[cfg(target_os = "windows")]
    fn encoded_powershell_condition(script: &str) -> SuccessCondition {
        let mut bytes = Vec::with_capacity(script.len().saturating_mul(2));
        for unit in script.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        SuccessCondition::Test {
            command: format!(
                "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
                STANDARD.encode(bytes)
            ),
        }
    }

    #[cfg(target_os = "windows")]
    async fn assert_no_windows_sandbox_sessions(verifier: &WindowsSandboxVerifier, cwd: &Path) {
        let process =
            ProcessExecutor::new([cwd.to_path_buf()], Arc::new(RejectingCredentialResolver))
                .expect("list process boundary");
        let outcome = run_windows_sandbox_cli(
            &process,
            &verifier.executable,
            cwd,
            vec!["list".into(), "--raw".into()],
            verifier.timeouts.stop,
            "list Windows Sandbox environments",
        )
        .await
        .expect("list Windows Sandbox environments");
        assert_eq!(outcome.exit_code, Some(0));
        let value: serde_json::Value = serde_json::from_str(
            outcome
                .stdout
                .as_deref()
                .expect("Windows Sandbox list output"),
        )
        .expect("Windows Sandbox list JSON");
        assert_eq!(
            value
                .get("WindowsSandboxEnvironments")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "every live qualification session must be stopped"
        );
    }

    #[cfg(target_os = "windows")]
    #[allow(clippy::too_many_lines)] // One live fixture proves HTTP, TCP, browser, and host-network isolation together.
    async fn assert_windows_node_service_isolated(
        verifier: &WindowsSandboxVerifier,
        workspace: &Path,
        artifacts: &ArtifactStore,
    ) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::AsyncWriteExt as _;

        fs::write(
            workspace.join("server.js"),
            r#"const http = require('http');
const host = process.env.HOST || '127.0.0.1';
const port = Number(process.env.PORT);
http.createServer((request, response) => {
  if (request.url === '/health') {
    response.writeHead(204);
    response.end();
  } else if (request.url === '/app') {
    response.writeHead(200, {'content-type': 'text/html; charset=utf-8'});
    response.end('<!doctype html><html><body><main id="app"></main><script>document.getElementById("app").textContent="hydrated";</script></body></html>');
  } else {
    response.writeHead(404);
    response.end();
  }
}).listen(port, host);"#,
        )
        .expect("service fixture");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("host collision listener");
        let address = listener.local_addr().expect("host collision address");
        let service_hits = Arc::new(AtomicUsize::new(0));
        let fixture_hits = service_hits.clone();
        let host_service = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                fixture_hits.fetch_add(1, Ordering::SeqCst);
                let _ = stream
                    .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                    .await;
            }
        });

        for (expected_status, expected_exit_code) in [(204, 0), (201, 1)] {
            let condition = SuccessCondition::HttpResponds {
                url: format!("http://{address}/health"),
                status: expected_status,
            };
            let ShadowCommandVerification::Completed { transcript } = verifier
                .verify_service(workspace, "node server.js", &condition, artifacts)
                .await
            else {
                panic!("same-sandbox Windows service probe must produce measured evidence");
            };
            let bytes = artifacts
                .read_verified_reference(&transcript, 1024 * 1024)
                .expect("service transcript");
            let record: ShadowCommandTranscript =
                serde_json::from_slice(&bytes).expect("service record");
            assert_eq!(record.exit_code, Some(expected_exit_code));
            assert!(!record.timed_out);
            assert_no_windows_sandbox_sessions(verifier, workspace).await;
        }
        let tcp_condition = SuccessCondition::PortListening {
            endpoint: format!("tcp://{address}"),
        };
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify_service(workspace, "node server.js", &tcp_condition, artifacts)
            .await
        else {
            panic!("same-sandbox Windows TCP probe must produce measured evidence");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("TCP service transcript");
        let record: ShadowCommandTranscript =
            serde_json::from_slice(&bytes).expect("TCP service record");
        assert_eq!(record.exit_code, Some(0));
        assert!(!record.timed_out);
        assert_no_windows_sandbox_sessions(verifier, workspace).await;
        if discover_browser_runtime()
            .expect("inspect browser runtime")
            .is_some()
        {
            let page_condition = SuccessCondition::PageRenders {
                url: format!("http://{address}/app"),
            };
            let ShadowCommandVerification::Completed { transcript } = verifier
                .verify_service(workspace, "node server.js", &page_condition, artifacts)
                .await
            else {
                panic!("same-sandbox Windows browser probe must produce measured evidence");
            };
            let bytes = artifacts
                .read_verified_reference(&transcript, 1024 * 1024)
                .expect("browser service transcript");
            let record: ShadowCommandTranscript =
                serde_json::from_slice(&bytes).expect("browser service record");
            assert_eq!(record.exit_code, Some(0));
            assert!(!record.timed_out);
            assert_no_windows_sandbox_sessions(verifier, workspace).await;

            for (selector, expected_exit_code) in [("main#app", 0), ("#missing", 1)] {
                let element_condition = SuccessCondition::ElementExists {
                    url: format!("http://{address}/app"),
                    selector: selector.into(),
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify_service(workspace, "node server.js", &element_condition, artifacts)
                    .await
                else {
                    panic!("same-sandbox Windows element probe must produce measured evidence");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("element service transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("element service record");
                assert_eq!(record.exit_code, Some(expected_exit_code));
                assert!(!record.timed_out);
                assert_no_windows_sandbox_sessions(verifier, workspace).await;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(service_hits.load(Ordering::SeqCst), 0);
        host_service.abort();
        eprintln!(
            "qualified same-sandbox Windows Node HTTP/TCP/browser service pass/fail with zero host-network hits"
        );
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "requires the current Microsoft Windows Sandbox package and nested virtualization"]
    async fn installed_windows_sandbox_executes_ephemerally_and_keeps_host_mappings_read_only() {
        let _live_test = windows_sandbox_live_test_lock().lock().await;
        let verifier = installed_windows_sandbox_verifier();
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("input.txt"), b"input").expect("input");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifacts");

        let ephemeral = encoded_powershell_condition(
            "if(Test-Path -LiteralPath 'input.txt'){Set-Content -LiteralPath 'sandbox-only.txt' -Value 'sandbox-only'; exit 0}; exit 7",
        );
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify_inner(workspace.path(), &ephemeral, &artifacts)
            .await
            .expect("installed Windows Sandbox must pass its runner probe")
        else {
            panic!("installed Windows Sandbox returned no completed evidence");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert_eq!(record.exit_code, Some(0));
        assert!(!record.timed_out);
        assert!(!workspace.path().join("sandbox-only.txt").exists());

        if discover_node_toolchain("node")
            .expect("inspect Node toolchain")
            .is_some()
        {
            eprintln!("qualified owner-local Node mapping inside Windows Sandbox");
            for command in ["node --version", "npm --version"] {
                let condition = SuccessCondition::Test {
                    command: command.into(),
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify_inner(workspace.path(), &condition, &artifacts)
                    .await
                    .expect("mapped Node toolchain must enter Windows Sandbox")
                else {
                    panic!("mapped Node toolchain returned no evidence for {command}");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("Node transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("Node record");
                assert_eq!(record.exit_code, Some(0), "mapped command: {command}");
                assert!(!record.timed_out);
                assert_no_windows_sandbox_sessions(&verifier, workspace.path()).await;
            }
            assert_windows_node_service_isolated(&verifier, workspace.path(), &artifacts).await;
        }

        let host_write = encoded_powershell_condition(
            "try { Set-Content -LiteralPath 'C:\\mcg-source\\escaped.txt' -Value 'escaped' -ErrorAction Stop; exit 0 } catch { exit 9 }",
        );
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify_inner(workspace.path(), &host_write, &artifacts)
            .await
            .expect("read-only host mapping check must run")
        else {
            panic!("read-only host mapping check returned no completed evidence");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("host-write transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert!(matches!(record.exit_code, Some(0 | 1)));
        assert!(!record.timed_out);
        // Windows Sandbox may satisfy a write through its disposable view even when the
        // mapped host folder is read-only. Host non-persistence is the security invariant.
        assert!(!workspace.path().join("escaped.txt").exists());
        assert_no_windows_sandbox_sessions(&verifier, workspace.path()).await;
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "requires the current Microsoft Windows Sandbox package and nested virtualization"]
    async fn installed_windows_sandbox_blocks_network_stops_timeouts_and_rejects_bad_setup() {
        let _live_test = windows_sandbox_live_test_lock().lock().await;
        let mut verifier = installed_windows_sandbox_verifier();
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("input.txt"), b"input").expect("input");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifacts");

        let network = encoded_powershell_condition(
            "$routes=@(Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue); $ips=@(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -ne '127.0.0.1' }); if($routes.Count -eq 0 -and $ips.Count -eq 0){exit 0}; exit 9",
        );
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify_inner(workspace.path(), &network, &artifacts)
            .await
            .expect("network-disabled sandbox command")
        else {
            panic!("network-disabled sandbox returned no completed evidence");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("network transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert_eq!(record.exit_code, Some(0));
        assert!(!record.timed_out);
        assert_no_windows_sandbox_sessions(&verifier, workspace.path()).await;

        verifier.timeouts.command = 2;
        let timeout = encoded_powershell_condition("Start-Sleep -Seconds 30; exit 9");
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify_inner(workspace.path(), &timeout, &artifacts)
            .await
            .expect("timed-out sandbox command")
        else {
            panic!("timed-out sandbox returned no completed evidence");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("timeout transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert_eq!(record.exit_code, None);
        assert!(record.timed_out);
        assert_no_windows_sandbox_sessions(&verifier, workspace.path()).await;

        let arguments = vec!["/D".into(), "/C".into(), "exit 0".into()];
        let harness = WindowsSandboxHarness::create(
            workspace.path(),
            &verifier.runner,
            "cmd.exe",
            &arguments,
            None,
            None,
            None,
        )
        .expect("setup-failure harness");
        fs::write(
            harness.path.join("request.json"),
            br#"{"schema_version":999}"#,
        )
        .expect("corrupt request");
        let configuration =
            windows_sandbox_configuration(workspace.path(), &harness.path, None, None)
                .expect("setup-failure profile");
        let process = ProcessExecutor::new(
            [harness.path.clone()],
            Arc::new(RejectingCredentialResolver),
        )
        .expect("setup-failure process boundary");
        let error = verifier
            .execute_session(&process, &harness.path, &configuration)
            .await
            .expect_err("bad runner setup must remain unavailable");
        assert!(matches!(
            error,
            ToolError::Unavailable(message)
                if message.contains("Windows Sandbox runner rejected its setup")
        ));
        assert_no_windows_sandbox_sessions(&verifier, workspace.path()).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Keep the complete live isolation gate auditable in one fixture.
    async fn installed_bubblewrap_executes_ephemerally_and_hides_host_etc() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::AsyncWriteExt as _;

        let Some(executable) = trusted_bubblewrap_executable() else {
            return;
        };
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("input.txt"), b"input").expect("input");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifacts");
        let runner = trusted_linux_shadow_runner().or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()?
                .parent()?
                .join("mcg-shadow-runner")
                .canonicalize()
                .ok()
        });
        let verifier = LinuxBubblewrapVerifier { executable, runner };

        let condition = SuccessCondition::Test {
            command: "touch sandbox-only.txt".into(),
        };
        let ShadowCommandVerification::Completed { transcript } = verifier
            .verify(workspace.path(), &condition, &artifacts)
            .await
        else {
            panic!("installed bubblewrap must pass its sandbox probe");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert_eq!(record.exit_code, Some(0));
        assert!(!record.timed_out);
        assert!(!workspace.path().join("sandbox-only.txt").exists());

        let escape = SuccessCondition::Test {
            command: "cat /etc/passwd".into(),
        };
        let ShadowCommandVerification::Completed { transcript } =
            verifier.verify(workspace.path(), &escape, &artifacts).await
        else {
            panic!("sandbox escape check must produce measured failure");
        };
        let bytes = artifacts
            .read_verified_reference(&transcript, 1024 * 1024)
            .expect("escape transcript");
        let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
        assert_ne!(record.exit_code, Some(0));

        let node_toolchain = discover_node_toolchain("node").expect("inspect Node toolchain");
        if std::env::var_os("MCG_REQUIRE_NODE_TOOLCHAIN").is_some() {
            assert!(
                node_toolchain.is_some(),
                "required owner-local Node distribution was not discovered; resolved={:?}",
                resolve_path_executable("node")
            );
        }
        if node_toolchain.is_some() {
            eprintln!("qualified owner-local Node mapping inside bubblewrap");
            for command in ["node --version", "npm --version"] {
                let condition = SuccessCondition::Test {
                    command: command.into(),
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify(workspace.path(), &condition, &artifacts)
                    .await
                else {
                    panic!("mapped Node toolchain must produce measured evidence for {command}");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("Node transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("Node record");
                assert_eq!(record.exit_code, Some(0), "mapped command: {command}");
                assert!(!record.timed_out);
            }

            if Path::new("/usr/bin/curl").is_file() && Path::new("/usr/bin/sleep").is_file() {
                fs::write(
                    workspace.path().join("server.js"),
                    r#"const http = require('http');
const host = process.env.HOST || '127.0.0.1';
const port = Number(process.env.PORT);
http.createServer((request, response) => {
  if (request.url === '/health') {
    response.writeHead(204);
    response.end();
  } else if (request.url === '/app') {
    response.writeHead(200, {'content-type': 'text/html; charset=utf-8'});
    response.end('<!doctype html><html><body><main id="app"></main><script>document.getElementById("app").textContent="hydrated";</script></body></html>');
  } else {
    response.writeHead(404);
    response.end();
  }
}).listen(port, host);"#,
                )
                .expect("service fixture");
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("host collision listener");
                let address = listener.local_addr().expect("host collision address");
                let service_hits = Arc::new(AtomicUsize::new(0));
                let fixture_hits = service_hits.clone();
                let host_service = tokio::spawn(async move {
                    loop {
                        let Ok((mut stream, _)) = listener.accept().await else {
                            break;
                        };
                        fixture_hits.fetch_add(1, Ordering::SeqCst);
                        let _ = stream
                            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                            .await;
                    }
                });
                let condition = SuccessCondition::HttpResponds {
                    url: format!("http://{address}/health"),
                    status: 204,
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify_service(workspace.path(), "node server.js", &condition, &artifacts)
                    .await
                else {
                    panic!("same-sandbox service probe must produce measured evidence");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("service transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("service record");
                assert_eq!(record.exit_code, Some(0));
                assert!(!record.timed_out);

                if verifier.runner.is_some()
                    && discover_browser_runtime()
                        .expect("inspect browser runtime")
                        .is_some()
                {
                    let page_condition = SuccessCondition::PageRenders {
                        url: format!("http://{address}/app"),
                    };
                    let ShadowCommandVerification::Completed { transcript } = verifier
                        .verify_service(
                            workspace.path(),
                            "node server.js",
                            &page_condition,
                            &artifacts,
                        )
                        .await
                    else {
                        panic!("same-sandbox browser probe must produce measured evidence");
                    };
                    let bytes = artifacts
                        .read_verified_reference(&transcript, 1024 * 1024)
                        .expect("browser service transcript");
                    let record: ShadowCommandTranscript =
                        serde_json::from_slice(&bytes).expect("browser service record");
                    assert_eq!(record.exit_code, Some(0));
                    assert!(!record.timed_out);

                    for (selector, expected_exit_code) in [("main#app", 0), ("#missing", 1)] {
                        let element_condition = SuccessCondition::ElementExists {
                            url: format!("http://{address}/app"),
                            selector: selector.into(),
                        };
                        let ShadowCommandVerification::Completed { transcript } = verifier
                            .verify_service(
                                workspace.path(),
                                "node server.js",
                                &element_condition,
                                &artifacts,
                            )
                            .await
                        else {
                            panic!("same-sandbox element probe must produce measured evidence");
                        };
                        let bytes = artifacts
                            .read_verified_reference(&transcript, 1024 * 1024)
                            .expect("element service transcript");
                        let record: ShadowCommandTranscript =
                            serde_json::from_slice(&bytes).expect("element service record");
                        assert_eq!(record.exit_code, Some(expected_exit_code));
                        assert!(!record.timed_out);
                    }
                }

                let tcp_condition = SuccessCondition::PortListening {
                    endpoint: format!("tcp://{address}"),
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify_service(
                        workspace.path(),
                        "node server.js",
                        &tcp_condition,
                        &artifacts,
                    )
                    .await
                else {
                    panic!("same-sandbox TCP probe must produce measured evidence");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("TCP service transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("TCP service record");
                assert_eq!(record.exit_code, Some(0));
                assert!(!record.timed_out);

                let wrong_status = SuccessCondition::HttpResponds {
                    url: format!("http://{address}/health"),
                    status: 201,
                };
                let ShadowCommandVerification::Completed { transcript } = verifier
                    .verify_service(
                        workspace.path(),
                        "node server.js",
                        &wrong_status,
                        &artifacts,
                    )
                    .await
                else {
                    panic!("wrong-status service probe must produce measured evidence");
                };
                let bytes = artifacts
                    .read_verified_reference(&transcript, 1024 * 1024)
                    .expect("wrong-status transcript");
                let record: ShadowCommandTranscript =
                    serde_json::from_slice(&bytes).expect("wrong-status record");
                assert_eq!(record.exit_code, Some(1));
                assert!(!record.timed_out);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                assert_eq!(service_hits.load(Ordering::SeqCst), 0);
                host_service.abort();
                eprintln!(
                    "qualified same-sandbox Node HTTP/TCP/browser service pass/fail with zero host-network hits"
                );
            }
        }

        if Path::new("/usr/bin/curl").is_file() {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("listener");
            let address = listener.local_addr().expect("address");
            let hits = Arc::new(AtomicUsize::new(0));
            let fixture_hits = hits.clone();
            let server = tokio::spawn(async move {
                if let Ok((_stream, _peer)) = listener.accept().await {
                    fixture_hits.fetch_add(1, Ordering::SeqCst);
                }
            });
            let network = SuccessCondition::Test {
                command: format!("curl -fsS http://{address}/"),
            };
            let ShadowCommandVerification::Completed { transcript } = verifier
                .verify(workspace.path(), &network, &artifacts)
                .await
            else {
                panic!("network isolation check must produce measured failure");
            };
            let bytes = artifacts
                .read_verified_reference(&transcript, 1024 * 1024)
                .expect("network transcript");
            let record: ShadowCommandTranscript = serde_json::from_slice(&bytes).expect("record");
            assert_ne!(record.exit_code, Some(0));
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            assert_eq!(hits.load(Ordering::SeqCst), 0);
            server.abort();
        }
    }
}
