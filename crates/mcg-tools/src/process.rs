use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use mcg_domain::{ManagedProcessId, ToolExecutionId};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use crate::{
    CredentialResolver, HeaderValue, OutputRedactor, ProcessOptions, ProcessStream, SecretMaterial,
    TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest,
    ToolRequestKind, VerificationEvidence,
};

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

#[derive(Default)]
struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

struct ManagedProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Arc<Mutex<CapturedOutput>>,
    stderr: Arc<Mutex<CapturedOutput>>,
    started_at: chrono::DateTime<Utc>,
    redactor: OutputRedactor,
}

#[derive(Serialize)]
struct ProcessObservation<'a> {
    process_id: ManagedProcessId,
    running: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
    started_at: chrono::DateTime<Utc>,
    observed_at: chrono::DateTime<Utc>,
    marker: &'a str,
}

pub struct ProcessExecutor {
    working_roots: Vec<PathBuf>,
    credentials: Arc<dyn CredentialResolver>,
    processes: Mutex<HashMap<ManagedProcessId, ManagedProcess>>,
    max_output_bytes: usize,
}

impl ProcessExecutor {
    /// Creates a supervisor constrained to existing working-directory roots.
    ///
    /// # Errors
    ///
    /// Returns an error when no root is supplied or a root cannot be resolved.
    pub fn new(
        roots: impl IntoIterator<Item = PathBuf>,
        credentials: Arc<dyn CredentialResolver>,
    ) -> Result<Self, ToolError> {
        let mut working_roots = roots
            .into_iter()
            .map(|root| {
                root.canonicalize().map_err(|error| {
                    ToolError::Execution(format!("resolve process working root: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if working_roots.is_empty() {
            return Err(ToolError::Invalid(
                "at least one process working root is required".into(),
            ));
        }
        working_roots.sort_by_key(|right| std::cmp::Reverse(right.components().count()));
        Ok(Self {
            working_roots,
            credentials,
            processes: Mutex::new(HashMap::new()),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        })
    }

    fn resolve_cwd(&self, value: &str) -> Result<PathBuf, ToolError> {
        let path = Path::new(value);
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(ToolError::Denied(
                "process working directory must be absolute and normalized".into(),
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve working directory: {error}")))?;
        if !self
            .working_roots
            .iter()
            .any(|root| canonical.starts_with(root))
        {
            return Err(ToolError::Denied(
                "process working directory is outside capability roots".into(),
            ));
        }
        Ok(canonical)
    }

    async fn prepare_command(
        &self,
        options: &ProcessOptions,
    ) -> Result<(Command, Vec<SecretMaterial>), ToolError> {
        validate_executable(&options.executable)?;
        if options.stdin_data.is_some() && options.stdin != ProcessStream::Piped {
            return Err(ToolError::Invalid(
                "stdin_data requires a piped stdin stream".into(),
            ));
        }
        let executable = platform_process_executable(&options.executable);
        let mut command = Command::new(executable);
        command.args(&options.argv);
        command.current_dir(self.resolve_cwd(&options.cwd)?);
        command.kill_on_drop(true);
        command.env_clear();
        preserve_runtime_environment(&mut command);
        let mut secrets = Vec::new();
        for variable in &options.env {
            validate_environment_name(&variable.name)?;
            match &variable.value {
                HeaderValue::Literal(value) => {
                    command.env(&variable.name, value);
                    if !value.is_empty() {
                        secrets.push(SecretMaterial::new(value.clone())?);
                    }
                }
                HeaderValue::Credential(reference) => {
                    let secret = self.credentials.resolve(reference).await?;
                    command.env(&variable.name, secret.expose_to_executor());
                    secrets.push(secret);
                }
            }
        }
        command.stdin(to_stdio(options.stdin));
        command.stdout(to_stdio(options.stdout));
        command.stderr(to_stdio(options.stderr));
        Ok((command, secrets))
    }

    async fn exec(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        options: &ProcessOptions,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let (mut command, secrets) = self.prepare_command(options).await?;
        let mut child = command
            .spawn()
            .map_err(|error| ToolError::Execution(format!("spawn process: {error}")))?;
        if let Some(mut stdin) = child.stdin.take()
            && let Some(data) = &options.stdin_data
        {
            stdin
                .write_all(data.as_bytes())
                .await
                .map_err(|error| ToolError::Execution(format!("write process stdin: {error}")))?;
        }
        let stdout = spawn_capture(child.stdout.take(), self.max_output_bytes);
        let stderr = spawn_capture(child.stderr.take(), self.max_output_bytes);
        let status = if let Ok(result) = tokio::time::timeout(
            std::time::Duration::from_secs(request.timeout_seconds),
            child.wait(),
        )
        .await
        {
            result.map_err(|error| ToolError::Execution(format!("wait for process: {error}")))?
        } else {
            child.kill().await.map_err(|error| {
                ToolError::Execution(format!("kill timed-out process: {error}"))
            })?;
            let _ = child.wait().await;
            return Err(ToolError::TimedOut);
        };
        let stdout = join_capture(stdout).await?;
        let stderr = join_capture(stderr).await?;
        let redactor = OutputRedactor::new(secrets);
        let stdout_text = redactor.redact(&String::from_utf8_lossy(&stdout.bytes));
        let stderr_text = redactor.redact(&String::from_utf8_lossy(&stderr.bytes));
        let exit_code = status.code();
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: if status.success() {
                ToolOutcomeStatus::Succeeded
            } else {
                ToolOutcomeStatus::Failed
            },
            summary: format!(
                "process exited with {}{}",
                exit_code.map_or_else(|| "no code".into(), |code| code.to_string()),
                if stdout.truncated || stderr.truncated {
                    "; output truncated"
                } else {
                    ""
                }
            ),
            exit_code,
            stdout: (!stdout_text.is_empty()).then_some(stdout_text),
            stderr: (!stderr_text.is_empty()).then_some(stderr_text),
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "process.exit_status".into(),
                passed: status.success(),
                detail: exit_code.map_or_else(|| "terminated".into(), |code| code.to_string()),
            }],
            started_at,
            finished_at: Utc::now(),
        })
    }

    async fn spawn(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        options: &ProcessOptions,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let (mut command, secrets) = self.prepare_command(options).await?;
        let mut child = command
            .spawn()
            .map_err(|error| ToolError::Execution(format!("spawn process: {error}")))?;
        if let (Some(stdin), Some(data)) = (child.stdin.as_mut(), &options.stdin_data) {
            stdin
                .write_all(data.as_bytes())
                .await
                .map_err(|error| ToolError::Execution(format!("write initial stdin: {error}")))?;
            stdin
                .flush()
                .await
                .map_err(|error| ToolError::Execution(format!("flush initial stdin: {error}")))?;
        }
        let stdout = Arc::new(Mutex::new(CapturedOutput::default()));
        let stderr = Arc::new(Mutex::new(CapturedOutput::default()));
        drain_background(child.stdout.take(), stdout.clone(), self.max_output_bytes);
        drain_background(child.stderr.take(), stderr.clone(), self.max_output_bytes);
        let process_id = ManagedProcessId::new();
        self.processes.lock().await.insert(
            process_id,
            ManagedProcess {
                stdin: child.stdin.take(),
                child,
                stdout,
                stderr,
                started_at,
                redactor: OutputRedactor::new(secrets),
            },
        );
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: ToolOutcomeStatus::Succeeded,
            summary: "managed process started".into(),
            exit_code: None,
            stdout: Some(serde_json::json!({"process_id": process_id}).to_string()),
            stderr: None,
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "process.spawned".into(),
                passed: true,
                detail: process_id.to_string(),
            }],
            started_at,
            finished_at: Utc::now(),
        })
    }

    async fn status(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        process_id: ManagedProcessId,
    ) -> Result<ToolOutcome, ToolError> {
        let mut processes = self.processes.lock().await;
        let process = processes
            .get_mut(&process_id)
            .ok_or_else(|| ToolError::Execution("managed process was not found".into()))?;
        let status = process
            .child
            .try_wait()
            .map_err(|error| ToolError::Execution(format!("inspect process: {error}")))?;
        let stdout = process.stdout.lock().await;
        let stderr = process.stderr.lock().await;
        let exit_code = status.and_then(|value| value.code());
        let observation = ProcessObservation {
            process_id,
            running: status.is_none(),
            exit_code,
            stdout: process
                .redactor
                .redact(&String::from_utf8_lossy(&stdout.bytes)),
            stderr: process
                .redactor
                .redact(&String::from_utf8_lossy(&stderr.bytes)),
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            started_at: process.started_at,
            observed_at: Utc::now(),
            marker: "managed_process_status",
        };
        let encoded = serde_json::to_string(&observation)
            .map_err(|error| ToolError::Execution(format!("serialize process status: {error}")))?;
        Ok(success_outcome(
            execution_id,
            request,
            "managed process inspected",
            Some(encoded),
            exit_code,
            "process.observed",
        ))
    }

    async fn write_stdin(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        process_id: ManagedProcessId,
        data: &str,
    ) -> Result<ToolOutcome, ToolError> {
        let mut processes = self.processes.lock().await;
        let process = processes
            .get_mut(&process_id)
            .ok_or_else(|| ToolError::Execution("managed process was not found".into()))?;
        let stdin = process
            .stdin
            .as_mut()
            .ok_or_else(|| ToolError::Execution("managed process stdin is not piped".into()))?;
        stdin
            .write_all(data.as_bytes())
            .await
            .map_err(|error| ToolError::Execution(format!("write process stdin: {error}")))?;
        stdin
            .flush()
            .await
            .map_err(|error| ToolError::Execution(format!("flush process stdin: {error}")))?;
        Ok(success_outcome(
            execution_id,
            request,
            "wrote managed process stdin",
            None,
            None,
            "process.stdin_flushed",
        ))
    }

    async fn kill(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        process_id: ManagedProcessId,
    ) -> Result<ToolOutcome, ToolError> {
        let mut processes = self.processes.lock().await;
        let process = processes
            .get_mut(&process_id)
            .ok_or_else(|| ToolError::Execution("managed process was not found".into()))?;
        if process
            .child
            .try_wait()
            .map_err(|error| ToolError::Execution(format!("inspect process: {error}")))?
            .is_none()
        {
            process
                .child
                .kill()
                .await
                .map_err(|error| ToolError::Execution(format!("kill process: {error}")))?;
            let _ = process.child.wait().await;
        }
        process.stdin = None;
        Ok(success_outcome(
            execution_id,
            request,
            "managed process stopped",
            None,
            None,
            "process.not_running",
        ))
    }

    async fn shell(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        shell: &str,
        command_text: &str,
        cwd: &str,
    ) -> Result<ToolOutcome, ToolError> {
        let (executable, argv) = shell_arguments(shell, command_text)?;
        self.exec(
            execution_id,
            request,
            &ProcessOptions {
                executable,
                argv,
                cwd: cwd.into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            },
        )
        .await
    }
}

#[async_trait]
impl ToolExecutor for ProcessExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::ProcessExec { options } => {
                self.exec(execution_id, request, options).await
            }
            ToolRequestKind::ProcessSpawn { options } => {
                self.spawn(execution_id, request, options).await
            }
            ToolRequestKind::ProcessStatus { process_id } => {
                self.status(execution_id, request, *process_id).await
            }
            ToolRequestKind::ProcessStdin { process_id, data } => {
                self.write_stdin(execution_id, request, *process_id, data)
                    .await
            }
            ToolRequestKind::ProcessKill { process_id } => {
                self.kill(execution_id, request, *process_id).await
            }
            ToolRequestKind::ShellExec {
                shell,
                command,
                cwd,
            } => self.shell(execution_id, request, shell, command, cwd).await,
            _ => Err(ToolError::Invalid(
                "process executor received an unsupported tool".into(),
            )),
        }
    }
}

fn validate_executable(value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(ToolError::Invalid("executable is invalid".into()));
    }
    Ok(())
}

fn platform_process_executable(executable: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        let path = Path::new(executable);
        if path.components().count() == 1 && path.extension().is_none() {
            let command = executable.to_ascii_lowercase();
            if matches!(
                command.as_str(),
                "npm" | "npx" | "pnpm" | "pnpx" | "yarn" | "yarnpkg" | "corepack"
            ) {
                return format!("{executable}.cmd");
            }
        }
    }
    executable.into()
}

fn validate_environment_name(value: &str) -> Result<(), ToolError> {
    if value.is_empty() || value.contains(['=', '\0']) {
        return Err(ToolError::Invalid(
            "environment variable name is invalid".into(),
        ));
    }
    Ok(())
}

fn preserve_runtime_environment(command: &mut Command) {
    for name in [
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "TEMP",
        "TMP",
        "HOME",
        "USERPROFILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn to_stdio(stream: ProcessStream) -> Stdio {
    match stream {
        ProcessStream::Null => Stdio::null(),
        ProcessStream::Piped => Stdio::piped(),
        ProcessStream::Inherit => Stdio::inherit(),
    }
}

fn shell_arguments(shell: &str, command: &str) -> Result<(String, Vec<String>), ToolError> {
    let name = Path::new(shell)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();
    let arguments = match name.as_str() {
        "powershell" | "pwsh" => vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            command.into(),
        ],
        "cmd" => vec!["/D".into(), "/S".into(), "/C".into(), command.into()],
        "sh" | "bash" => vec!["-c".into(), command.into()],
        _ => {
            return Err(ToolError::Denied(
                "shell must be powershell, pwsh, cmd, sh, or bash".into(),
            ));
        }
    };
    Ok((shell.into(), arguments))
}

fn spawn_capture<R>(
    reader: Option<R>,
    max_bytes: usize,
) -> Option<tokio::task::JoinHandle<Result<CapturedOutput, std::io::Error>>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    reader.map(|reader| tokio::spawn(read_bounded(reader, max_bytes)))
}

async fn join_capture(
    task: Option<tokio::task::JoinHandle<Result<CapturedOutput, std::io::Error>>>,
) -> Result<CapturedOutput, ToolError> {
    match task {
        Some(task) => task
            .await
            .map_err(|error| ToolError::Execution(format!("join output reader: {error}")))?
            .map_err(|error| ToolError::Execution(format!("read process output: {error}"))),
        None => Ok(CapturedOutput::default()),
    }
}

fn drain_background<R>(reader: Option<R>, output: Arc<Mutex<CapturedOutput>>, max_bytes: usize)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    if let Some(reader) = reader {
        tokio::spawn(async move {
            let _ = read_shared(reader, output, max_bytes).await;
        });
    }
}

async fn read_shared<R>(
    mut reader: R,
    output: Arc<Mutex<CapturedOutput>>,
    max_bytes: usize,
) -> Result<(), std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 8_192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        let mut captured = output.lock().await;
        let remaining = max_bytes.saturating_sub(captured.bytes.len());
        let retained = remaining.min(read);
        captured.bytes.extend_from_slice(&buffer[..retained]);
        if retained < read {
            captured.truncated = true;
        }
    }
}

async fn read_bounded<R>(mut reader: R, max_bytes: usize) -> Result<CapturedOutput, std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut captured = CapturedOutput::default();
    let mut buffer = [0_u8; 8_192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(captured.bytes.len());
        let retained = remaining.min(read);
        captured.bytes.extend_from_slice(&buffer[..retained]);
        if retained < read {
            captured.truncated = true;
        }
    }
    Ok(captured)
}

fn success_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    summary: &str,
    stdout: Option<String>,
    exit_code: Option<i32>,
    method: &str,
) -> ToolOutcome {
    let now = Utc::now();
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: summary.into(),
        exit_code,
        stdout,
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: method.into(),
            passed: true,
            detail: "true".into(),
        }],
        started_at: now,
        finished_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CredentialReference, ProcessEnvironmentVariable, RejectingCredentialResolver};
    use tempfile::tempdir;

    fn executor(root: &Path) -> ProcessExecutor {
        ProcessExecutor::new([root.to_path_buf()], Arc::new(RejectingCredentialResolver))
            .expect("executor")
    }

    fn echo_options(root: &Path, text: &str) -> ProcessOptions {
        if cfg!(windows) {
            ProcessOptions {
                executable: "cmd".into(),
                argv: vec!["/D".into(), "/C".into(), format!("echo {text}")],
                cwd: root.to_string_lossy().into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            }
        } else {
            ProcessOptions {
                executable: "sh".into(),
                argv: vec!["-c".into(), format!("printf '%s' '{text}'")],
                cwd: root.to_string_lossy().into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            }
        }
    }

    fn sleep_options(root: &Path, seconds: u64) -> ProcessOptions {
        if cfg!(windows) {
            ProcessOptions {
                executable: "powershell".into(),
                argv: vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    format!("Write-Output ready; Start-Sleep -Seconds {seconds}"),
                ],
                cwd: root.to_string_lossy().into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            }
        } else {
            ProcessOptions {
                executable: "sh".into(),
                argv: vec!["-c".into(), format!("printf ready; sleep {seconds}")],
                cwd: root.to_string_lossy().into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            }
        }
    }

    #[tokio::test]
    async fn structured_exec_captures_exit_and_output() {
        let root = tempdir().expect("root");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "echo",
            ToolRequestKind::ProcessExec {
                options: echo_options(root.path(), "hello"),
            },
        );
        let outcome = executor(root.path())
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("execute");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|value| value.contains("hello"))
        );
        assert_eq!(outcome.exit_code, Some(0));
    }

    #[tokio::test]
    async fn working_directory_escape_is_denied() {
        let root = tempdir().expect("root");
        let mut options = echo_options(root.path(), "no");
        options.cwd = root.path().join("..").to_string_lossy().into();
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "escape",
            ToolRequestKind::ProcessExec { options },
        );
        assert!(matches!(
            executor(root.path())
                .execute(ToolExecutionId::new(), &request)
                .await,
            Err(ToolError::Denied(_))
        ));
    }

    #[tokio::test]
    async fn timeout_kills_the_child_and_returns_a_typed_error() {
        let root = tempdir().expect("root");
        let mut request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "timeout",
            ToolRequestKind::ProcessExec {
                options: sleep_options(root.path(), 5),
            },
        );
        request.timeout_seconds = 1;
        assert!(matches!(
            executor(root.path())
                .execute(ToolExecutionId::new(), &request)
                .await,
            Err(ToolError::TimedOut)
        ));
    }

    #[tokio::test]
    async fn spawned_process_can_be_observed_and_stopped_by_typed_id() {
        let root = tempdir().expect("root");
        let executor = executor(root.path());
        let spawn_request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "spawn",
            ToolRequestKind::ProcessSpawn {
                options: sleep_options(root.path(), 5),
            },
        );
        let spawned = executor
            .execute(ToolExecutionId::new(), &spawn_request)
            .await
            .expect("spawn");
        let value: serde_json::Value =
            serde_json::from_str(spawned.stdout.as_deref().expect("spawn output")).expect("json");
        let process_id: ManagedProcessId =
            serde_json::from_value(value.get("process_id").cloned().expect("process id"))
                .expect("typed process id");
        let status_request = ToolRequest::new(
            spawn_request.task_id,
            "status",
            ToolRequestKind::ProcessStatus { process_id },
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut observed_ready = false;
        while tokio::time::Instant::now() < deadline {
            let status = executor
                .execute(ToolExecutionId::new(), &status_request)
                .await
                .expect("status");
            if status
                .stdout
                .as_deref()
                .is_some_and(|value| value.contains("ready"))
            {
                observed_ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let kill_request = ToolRequest::new(
            spawn_request.task_id,
            "kill",
            ToolRequestKind::ProcessKill { process_id },
        );
        assert!(
            executor
                .execute(ToolExecutionId::new(), &kill_request)
                .await
                .is_ok()
        );
        assert!(observed_ready, "spawned process never reported ready");
    }

    #[tokio::test]
    async fn output_is_bounded_while_the_pipe_is_fully_drained() {
        let root = tempdir().expect("root");
        let mut executor = executor(root.path());
        executor.max_output_bytes = 32;
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "bounded output",
            ToolRequestKind::ProcessExec {
                options: echo_options(root.path(), &"x".repeat(512)),
            },
        );
        let outcome = executor
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("execute");
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|value| value.len() <= 32)
        );
        assert!(outcome.summary.contains("output truncated"));
    }

    struct FixtureCredentialResolver;

    #[async_trait]
    impl CredentialResolver for FixtureCredentialResolver {
        async fn resolve(
            &self,
            _reference: &CredentialReference,
        ) -> Result<SecretMaterial, ToolError> {
            SecretMaterial::new("credential-fixture-secret")
        }
    }

    #[tokio::test]
    async fn credential_environment_echo_is_redacted() {
        let root = tempdir().expect("root");
        let executor = ProcessExecutor::new(
            [root.path().to_path_buf()],
            Arc::new(FixtureCredentialResolver),
        )
        .expect("executor");
        let mut options = if cfg!(windows) {
            echo_options(root.path(), "%FIXTURE_SECRET%")
        } else {
            ProcessOptions {
                executable: "sh".into(),
                argv: vec!["-c".into(), "printf '%s' \"$FIXTURE_SECRET\"".into()],
                cwd: root.path().to_string_lossy().into(),
                env: Vec::new(),
                stdin: ProcessStream::Null,
                stdin_data: None,
                stdout: ProcessStream::Piped,
                stderr: ProcessStream::Piped,
            }
        };
        options.env.push(ProcessEnvironmentVariable {
            name: "FIXTURE_SECRET".into(),
            value: HeaderValue::Credential(CredentialReference {
                provider: "fixture".into(),
                key: "test".into(),
            }),
        });
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "secret echo",
            ToolRequestKind::ProcessExec { options },
        );
        let outcome = executor
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("execute");
        let stdout = outcome.stdout.as_deref().expect("stdout");
        assert!(stdout.contains("[REDACTED]"));
        assert!(!stdout.contains("credential-fixture-secret"));
    }

    #[test]
    fn environment_names_reject_command_injection_syntax() {
        assert!(validate_environment_name("GOOD_NAME").is_ok());
        assert!(validate_environment_name("BAD=VALUE").is_err());
        let _ = ProcessEnvironmentVariable {
            name: "FIXTURE".into(),
            value: HeaderValue::Literal("value".into()),
        };
    }

    #[test]
    fn platform_process_launcher_preserves_structured_execution() {
        if cfg!(windows) {
            assert_eq!(platform_process_executable("npm"), "npm.cmd");
            assert_eq!(platform_process_executable("NPM"), "NPM.cmd");
            assert_eq!(platform_process_executable("corepack"), "corepack.cmd");
            assert_eq!(platform_process_executable("bun"), "bun");
            assert_eq!(platform_process_executable("npm.cmd"), "npm.cmd");
            assert_eq!(
                platform_process_executable(r"C:\\tools\\npm"),
                r"C:\\tools\\npm"
            );
        } else {
            assert_eq!(platform_process_executable("npm"), "npm");
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn windows_javascript_runner_executes_cmd_wrapper_without_shell_request() {
        let root = tempdir().expect("root");
        std::fs::write(
            root.path().join("npm.cmd"),
            "@echo off\r\necho structured-wrapper-ok\r\n",
        )
        .expect("fixture npm wrapper");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "run JavaScript package command",
            ToolRequestKind::ProcessExec {
                options: ProcessOptions {
                    executable: "npm".into(),
                    argv: vec!["run".into(), "test".into()],
                    cwd: root.path().to_string_lossy().into_owned(),
                    env: vec![ProcessEnvironmentVariable {
                        name: "PATH".into(),
                        value: HeaderValue::Literal(root.path().to_string_lossy().into_owned()),
                    }],
                    stdin: ProcessStream::Null,
                    stdin_data: None,
                    stdout: ProcessStream::Piped,
                    stderr: ProcessStream::Piped,
                },
            },
        );
        let outcome = executor(root.path())
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("execute npm.cmd wrapper");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|stdout| stdout.contains("structured-wrapper-ok"))
        );
    }
}
