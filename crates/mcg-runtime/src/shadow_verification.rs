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
const BWRAP_MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_PROCESSES: u32 = 256;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_OPEN_FILES: u32 = 1024;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_ARGUMENTS: usize = 128;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
const BWRAP_MAX_COMMAND_BYTES: usize = 32 * 1024;
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
}

/// Selects a backend only after its immutable executable boundary passes local
/// safety checks. Unsupported hosts retain the no-execution backend.
pub(crate) fn default_shadow_command_verifier() -> Arc<dyn ShadowCommandVerifier> {
    #[cfg(target_os = "linux")]
    if let Some(executable) = trusted_bubblewrap_executable() {
        return Arc::new(LinuxBubblewrapVerifier { executable });
    }
    #[cfg(target_os = "windows")]
    if let Some((executable, runner)) = trusted_windows_sandbox_components() {
        return Arc::new(WindowsSandboxVerifier {
            executable,
            runner,
            lifecycle: tokio::sync::Mutex::new(()),
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
        let proof = EphemeralProof::create(&workspace_root)?;
        let argv =
            bubblewrap_arguments(&workspace_root, &proof.path, executable, &command_arguments)?;
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
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug)]
struct WindowsSandboxVerifier {
    executable: PathBuf,
    runner: PathBuf,
    lifecycle: tokio::sync::Mutex<()>,
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
}

#[cfg(any(target_os = "windows", test))]
impl WindowsSandboxVerifier {
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
        let executable = windows_command_name(executable);
        let harness = WindowsSandboxHarness::create(
            &workspace_root,
            &self.runner,
            &executable,
            &command_arguments,
        )?;
        let configuration = windows_sandbox_configuration(&workspace_root, &harness.path)?;
        let process = ProcessExecutor::new(
            [harness.path.clone()],
            Arc::new(RejectingCredentialResolver),
        )?;

        let (exit_code, timed_out) = self
            .execute_session(&process, &harness.path, &configuration)
            .await?;
        let transcript = ShadowCommandTranscript {
            schema_version: SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION,
            condition: condition.clone(),
            exit_code,
            timed_out,
            stdout: None,
            stderr: None,
        };
        let bytes = serde_json::to_vec(&transcript)
            .map_err(|error| ToolError::Execution(format!("serialize transcript: {error}")))?;
        let transcript =
            artifacts.capture(&bytes, "application/vnd.mcg.shadow-command+json;version=1")?;
        Ok(ShadowCommandVerification::Completed { transcript })
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
            WSB_START_TIMEOUT_SECONDS,
            "start Windows Sandbox shadow session",
        )
        .await;
        if let Err(error) = started
            .and_then(|outcome| require_success(&outcome, "start Windows Sandbox shadow session"))
        {
            stop_windows_sandbox(process, &self.executable, harness_root, &session_id).await;
            return Err(error);
        }

        let probe = windows_sandbox_exec_arguments(&session_id, "--probe");
        let preflight = run_windows_sandbox_cli(
            process,
            &self.executable,
            harness_root,
            probe.clone(),
            WSB_START_TIMEOUT_SECONDS,
            "probe Windows Sandbox shadow session",
        )
        .await;
        if preflight.as_ref().ok().and_then(|value| value.exit_code) != Some(0) {
            stop_windows_sandbox(process, &self.executable, harness_root, &session_id).await;
            return Err(ToolError::Unavailable(
                "Windows Sandbox runner preflight failed".into(),
            ));
        }

        let command = run_windows_sandbox_cli(
            process,
            &self.executable,
            harness_root,
            windows_sandbox_exec_arguments(&session_id, r"C:\mcg-harness\request.json"),
            WSB_COMMAND_TIMEOUT_SECONDS,
            "execute Windows Sandbox shadow command",
        )
        .await;
        let disposition = match command {
            Err(ToolError::TimedOut) => Ok((None, true)),
            Err(error) => Err(error),
            Ok(outcome) => {
                let postflight = run_windows_sandbox_cli(
                    process,
                    &self.executable,
                    harness_root,
                    probe,
                    WSB_START_TIMEOUT_SECONDS,
                    "postflight Windows Sandbox shadow session",
                )
                .await?;
                if postflight.exit_code == Some(0) {
                    match outcome.exit_code {
                        Some(0) => Ok((Some(0), false)),
                        Some(1) => Ok((Some(1), false)),
                        Some(WSB_SETUP_FAILURE_EXIT_CODE) => Err(ToolError::Unavailable(
                            "Windows Sandbox runner rejected its setup".into(),
                        )),
                        _ => Err(ToolError::Unavailable(
                            "Windows Sandbox returned an unrecognized command disposition".into(),
                        )),
                    }
                } else {
                    Err(ToolError::Unavailable(
                        "Windows Sandbox runner postflight failed".into(),
                    ))
                }
            }
        };
        stop_windows_sandbox(process, &self.executable, harness_root, &session_id).await;
        disposition
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
                schema_version: 1,
                source_root: r"C:\mcg-source",
                workspace_root: r"C:\mcg-workspace",
                executable,
                argv,
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
fn windows_sandbox_configuration(source: &Path, harness: &Path) -> Result<String, ToolError> {
    let source = xml_escape_path(source)?;
    let harness = xml_escape_path(harness)?;
    Ok(format!(
        "<Configuration><vGPU>Disable</vGPU><Networking>Disable</Networking><AudioInput>Disable</AudioInput><VideoInput>Disable</VideoInput><PrinterRedirection>Disable</PrinterRedirection><ClipboardRedirection>Disable</ClipboardRedirection><ProtectedClient>Enable</ProtectedClient><MemoryInMB>8192</MemoryInMB><MappedFolders><MappedFolder><HostFolder>{source}</HostFolder><SandboxFolder>C:\\mcg-source</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder><MappedFolder><HostFolder>{harness}</HostFolder><SandboxFolder>C:\\mcg-harness</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder></MappedFolders></Configuration>"
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
fn windows_sandbox_exec_arguments(session_id: &str, argument: &str) -> Vec<String> {
    vec![
        "exec".into(),
        "--raw".into(),
        "--id".into(),
        session_id.into(),
        "--command".into(),
        r"C:\mcg-harness\mcg-shadow-runner.exe".into(),
        "--run-as".into(),
        "System".into(),
        "--".into(),
        argument.into(),
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
        "npm" | "npx" | "pnpm" | "yarn" | "bun" => format!("{executable}.cmd"),
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
fn require_success(outcome: &ToolOutcome, purpose: &str) -> Result<(), ToolError> {
    if outcome.exit_code == Some(0) {
        Ok(())
    } else {
        Err(ToolError::Unavailable(format!(
            "{purpose} failed with {:?}",
            outcome.exit_code
        )))
    }
}

#[cfg(any(target_os = "windows", test))]
async fn stop_windows_sandbox(
    process: &ProcessExecutor,
    executable: &Path,
    cwd: &Path,
    session_id: &str,
) {
    let _ = run_windows_sandbox_cli(
        process,
        executable,
        cwd,
        windows_sandbox_stop_arguments(session_id),
        WSB_STOP_TIMEOUT_SECONDS,
        "stop Windows Sandbox shadow session",
    )
    .await;
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn condition_command(condition: &SuccessCondition) -> Option<&str> {
    match condition {
        SuccessCondition::Build { command }
        | SuccessCondition::Test { command }
        | SuccessCondition::StaticAnalysis { command } => Some(command),
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

#[cfg(any(target_os = "linux", test))]
fn bubblewrap_arguments(
    workspace_root: &Path,
    proof_path: &Path,
    executable: &str,
    command_arguments: &[String],
) -> Result<Vec<String>, ToolError> {
    let workspace = workspace_root
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow workspace path is not Unicode".into()))?;
    let proof = proof_path
        .to_str()
        .ok_or_else(|| ToolError::Invalid("shadow proof path is not Unicode".into()))?;
    let mut argv = vec![
        "--unshare-all".into(),
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
        "/usr/bin:/bin".into(),
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
    ];
    argv.extend(command_arguments.iter().cloned());
    Ok(argv)
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

    [Path::new("/usr/bin/bwrap"), Path::new("/bin/bwrap")]
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
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let windows = std::env::var_os("WINDIR").map(PathBuf::from)?;
    let expected = windows.join("System32").join("wsb.exe");
    let executable = expected.canonicalize().ok()?;
    if !executable
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        return None;
    }
    let executable_metadata = fs::symlink_metadata(&executable).ok()?;
    if !executable_metadata.is_file()
        || executable_metadata.file_type().is_symlink()
        || executable_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return None;
    }

    let runner = std::env::current_exe()
        .ok()?
        .parent()?
        .join("mcg-shadow-runner.exe");
    let runner_metadata = fs::symlink_metadata(&runner).ok()?;
    if !runner_metadata.is_file()
        || runner_metadata.file_type().is_symlink()
        || runner_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return None;
    }
    Some((executable, runner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubblewrap_profile_is_networkless_read_only_and_argument_vector_safe() {
        let workspace = tempfile::tempdir().expect("workspace");
        let proof = EphemeralProof::create(workspace.path()).expect("proof");
        let argv = bubblewrap_arguments(
            workspace.path(),
            &proof.path,
            "npm",
            &["test;touch".into(), "/outside".into()],
        )
        .expect("profile");
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["--unshare-all", "--disable-userns"])
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

    #[tokio::test]
    async fn missing_bubblewrap_produces_no_completed_evidence() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let verifier = LinuxBubblewrapVerifier {
            executable: workspace.path().join("missing-bwrap"),
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

    #[test]
    fn windows_sandbox_profile_is_locked_down_read_only_and_xml_safe() {
        let source = Path::new(r"C:\source & fixtures");
        let harness = Path::new(r"C:\harness <private>");
        let configuration =
            windows_sandbox_configuration(source, harness).expect("Windows Sandbox profile");
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
    }

    #[test]
    fn windows_sandbox_cli_uses_system_context_and_never_writable_shares() {
        let configuration =
            windows_sandbox_configuration(Path::new(r"C:\source"), Path::new(r"C:\harness"))
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

        let exec = windows_sandbox_exec_arguments("session", r"C:\mcg-harness\request.json");
        assert!(exec.windows(2).any(|pair| pair == ["--run-as", "System"]));
        assert!(
            exec.windows(2)
                .any(|pair| pair == ["--command", r"C:\mcg-harness\mcg-shadow-runner.exe"])
        );
        assert_eq!(
            exec.last().map(String::as_str),
            Some(r"C:\mcg-harness\request.json")
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

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn installed_bubblewrap_executes_ephemerally_and_hides_host_etc() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let Some(executable) = trusted_bubblewrap_executable() else {
            return;
        };
        let workspace = tempfile::tempdir().expect("workspace");
        fs::write(workspace.path().join("input.txt"), b"input").expect("input");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifacts");
        let verifier = LinuxBubblewrapVerifier { executable };

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
