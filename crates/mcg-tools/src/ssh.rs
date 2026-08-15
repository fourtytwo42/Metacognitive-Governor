use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use chrono::Utc;
use mcg_domain::{SshOperationKind, ToolExecutionId};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::{
    TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest,
    ToolRequestKind, VerificationEvidence,
};

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    truncated: bool,
}

pub struct SshExecutor {
    ssh_executable: PathBuf,
    scp_executable: PathBuf,
    scratch_root: PathBuf,
    empty_config: PathBuf,
    local_roots: Vec<PathBuf>,
    max_output_bytes: usize,
}

impl SshExecutor {
    /// Creates an OpenSSH executor with isolated host-key material and local path roots.
    ///
    /// # Errors
    /// Returns an error when executables, scratch storage, or roots cannot be resolved.
    pub fn new(
        ssh_executable: impl AsRef<Path>,
        scp_executable: impl AsRef<Path>,
        scratch_root: impl AsRef<Path>,
        local_roots: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, ToolError> {
        let ssh_executable = resolve_regular_file(ssh_executable.as_ref(), "SSH executable")?;
        let scp_executable = resolve_regular_file(scp_executable.as_ref(), "SCP executable")?;
        fs::create_dir_all(scratch_root.as_ref())
            .map_err(|error| ToolError::Execution(format!("create SSH scratch root: {error}")))?;
        let scratch_root = scratch_root
            .as_ref()
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve SSH scratch root: {error}")))?;
        let empty_config = scratch_root.join("empty-openssh-config");
        if !empty_config.exists() {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&empty_config)
                .map_err(|error| {
                    ToolError::Execution(format!("create empty OpenSSH config: {error}"))
                })?;
        }
        let empty_config = resolve_regular_file(&empty_config, "empty OpenSSH config")?;
        let local_roots = local_roots
            .into_iter()
            .map(|root| {
                root.canonicalize().map_err(|error| {
                    ToolError::Execution(format!("resolve SSH local root: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if local_roots.is_empty() {
            return Err(ToolError::Invalid(
                "at least one SSH local path root is required".into(),
            ));
        }
        Ok(Self {
            ssh_executable,
            scp_executable,
            scratch_root,
            empty_config,
            local_roots,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        })
    }

    fn known_hosts_path(
        &self,
        execution_id: ToolExecutionId,
        entry: &str,
    ) -> Result<PathBuf, ToolError> {
        validate_known_hosts_entry(entry)?;
        let path = self
            .scratch_root
            .join(format!("known-hosts-{execution_id}.txt"));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| ToolError::Execution(format!("create pinned host file: {error}")))?;
        file.write_all(entry.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|error| ToolError::Execution(format!("write pinned host file: {error}")))?;
        Ok(path)
    }

    fn common_arguments(
        empty_config: &Path,
        known_hosts: &Path,
        port: u16,
        transfer: bool,
    ) -> Result<Vec<String>, ToolError> {
        let known_hosts = known_hosts
            .to_str()
            .ok_or_else(|| ToolError::Unavailable("SSH scratch path is not UTF-8".into()))?;
        let empty_config = empty_config
            .to_str()
            .ok_or_else(|| ToolError::Unavailable("SSH config path is not UTF-8".into()))?;
        let mut arguments = vec![
            "-F".into(),
            empty_config.into(),
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "PasswordAuthentication=no".into(),
            "-o".into(),
            "KbdInteractiveAuthentication=no".into(),
            "-o".into(),
            "NumberOfPasswordPrompts=0".into(),
            "-o".into(),
            "ForwardAgent=no".into(),
            "-o".into(),
            "ClearAllForwardings=yes".into(),
            "-o".into(),
            "StrictHostKeyChecking=yes".into(),
            "-o".into(),
            format!("UserKnownHostsFile={known_hosts}"),
            "-o".into(),
            format!("GlobalKnownHostsFile={empty_config}"),
            "-o".into(),
            "ConnectTimeout=10".into(),
        ];
        arguments.push(if transfer { "-P" } else { "-p" }.into());
        arguments.push(port.to_string());
        Ok(arguments)
    }

    fn resolve_local_read(&self, value: &str) -> Result<PathBuf, ToolError> {
        let canonical = Path::new(value)
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve upload source: {error}")))?;
        if !canonical.is_file() || !self.within_local_roots(&canonical) {
            return Err(ToolError::Denied(
                "upload source must be a regular file inside a local capability root".into(),
            ));
        }
        Ok(canonical)
    }

    fn resolve_local_write(&self, value: &str, overwrite: bool) -> Result<PathBuf, ToolError> {
        let path = Path::new(value);
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(ToolError::Denied(
                "download destination must be absolute and normalized".into(),
            ));
        }
        if path.exists() {
            let canonical = path.canonicalize().map_err(|error| {
                ToolError::Execution(format!("resolve download destination: {error}"))
            })?;
            if !overwrite {
                return Err(ToolError::Denied(
                    "download destination exists and overwrite is disabled".into(),
                ));
            }
            if !canonical.is_file() || !self.within_local_roots(&canonical) {
                return Err(ToolError::Denied(
                    "download destination is outside a local capability root".into(),
                ));
            }
            return Ok(canonical);
        }
        let parent = path.parent().ok_or_else(|| {
            ToolError::Denied("download destination has no parent directory".into())
        })?;
        let parent = parent.canonicalize().map_err(|error| {
            ToolError::Execution(format!("resolve download parent directory: {error}"))
        })?;
        if !self.within_local_roots(&parent) {
            return Err(ToolError::Denied(
                "download destination is outside a local capability root".into(),
            ));
        }
        let name = path
            .file_name()
            .ok_or_else(|| ToolError::Denied("download destination requires a file name".into()))?;
        Ok(parent.join(name))
    }

    fn within_local_roots(&self, path: &Path) -> bool {
        self.local_roots.iter().any(|root| path.starts_with(root))
    }

    async fn run(
        &self,
        executable: &Path,
        arguments: &[String],
        timeout_seconds: u64,
    ) -> Result<CommandOutput, ToolError> {
        let mut command = Command::new(executable);
        command.args(arguments);
        command.current_dir(&self.scratch_root);
        command.kill_on_drop(true);
        command.env_clear();
        preserve_ssh_environment(&mut command);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| ToolError::Execution(format!("start OpenSSH: {error}")))?;
        let stdout = drain_bounded(child.stdout.take(), self.max_output_bytes);
        let stderr = drain_bounded(child.stderr.take(), self.max_output_bytes);
        let status = if let Ok(result) = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_seconds),
            child.wait(),
        )
        .await
        {
            result.map_err(|error| ToolError::Execution(format!("wait for OpenSSH: {error}")))?
        } else {
            child.kill().await.map_err(|error| {
                ToolError::Execution(format!("stop timed-out OpenSSH: {error}"))
            })?;
            let _ = child.wait().await;
            return Err(ToolError::TimedOut);
        };
        let (stdout, stdout_truncated) = stdout
            .await
            .map_err(|error| ToolError::Execution(format!("join OpenSSH stdout: {error}")))?
            .map_err(|error| ToolError::Execution(format!("capture OpenSSH stdout: {error}")))?;
        let (stderr, stderr_truncated) = stderr
            .await
            .map_err(|error| ToolError::Execution(format!("join OpenSSH stderr: {error}")))?
            .map_err(|error| ToolError::Execution(format!("capture OpenSSH stderr: {error}")))?;
        Ok(CommandOutput {
            success: status.success(),
            exit_code: status.code(),
            stdout,
            stderr,
            truncated: stdout_truncated || stderr_truncated,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_operation(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        hostname: &str,
        port: u16,
        username: &str,
        known_hosts_entry: &str,
        host_key_sha256: &str,
        kind: &SshOperationKind,
    ) -> Result<ToolOutcome, ToolError> {
        validate_connection(hostname, port, username, known_hosts_entry, host_key_sha256)?;
        let known_hosts = self.known_hosts_path(execution_id, known_hosts_entry)?;
        let started_at = Utc::now();
        let result = self
            .execute_with_host_file(request, hostname, port, username, &known_hosts, kind)
            .await;
        let _ = fs::remove_file(&known_hosts);
        let output = result?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: if output.success {
                ToolOutcomeStatus::Succeeded
            } else {
                ToolOutcomeStatus::Failed
            },
            summary: format!(
                "OpenSSH operation exited with {}{}",
                output
                    .exit_code
                    .map_or_else(|| "no code".into(), |code| code.to_string()),
                if output.truncated {
                    "; output truncated"
                } else {
                    ""
                }
            ),
            exit_code: output.exit_code,
            stdout: (!stdout.is_empty()).then_some(stdout),
            stderr: (!stderr.is_empty()).then_some(stderr),
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "ssh.exit_status".into(),
                passed: output.success,
                detail: format!(
                    "host key {host_key_sha256}; exit {}",
                    output
                        .exit_code
                        .map_or_else(|| "none".into(), |code| code.to_string())
                ),
            }],
            started_at,
            finished_at: Utc::now(),
        })
    }

    #[allow(clippy::too_many_lines)] // Keeps all four typed operation argument vectors auditable.
    async fn execute_with_host_file(
        &self,
        request: &ToolRequest,
        hostname: &str,
        port: u16,
        username: &str,
        known_hosts: &Path,
        kind: &SshOperationKind,
    ) -> Result<CommandOutput, ToolError> {
        let target = ssh_target(username, hostname)?;
        match kind {
            SshOperationKind::Inspect => {
                let mut arguments =
                    Self::common_arguments(&self.empty_config, known_hosts, port, false)?;
                arguments.extend(["-n".into(), "-T".into(), "--".into(), target]);
                arguments.push("uname -srm && id -u".into());
                self.run(&self.ssh_executable, &arguments, request.timeout_seconds)
                    .await
            }
            SshOperationKind::Execute {
                executable,
                arguments: remote_arguments,
                working_directory,
            } => {
                let remote = encode_posix_command(
                    executable,
                    remote_arguments,
                    working_directory.as_deref(),
                )?;
                let mut arguments =
                    Self::common_arguments(&self.empty_config, known_hosts, port, false)?;
                arguments.extend(["-n".into(), "-T".into(), "--".into(), target, remote]);
                self.run(&self.ssh_executable, &arguments, request.timeout_seconds)
                    .await
            }
            SshOperationKind::Upload {
                local_path,
                remote_path,
                overwrite,
                expected_sha256,
            } => {
                validate_remote_path(remote_path)?;
                validate_sha256(expected_sha256)?;
                let local = self.resolve_local_read(local_path)?;
                if file_sha256(&local)? != *expected_sha256 {
                    return Err(ToolError::Denied(
                        "upload source changed after authority was issued".into(),
                    ));
                }
                if !overwrite {
                    self.require_remote_absent(
                        request,
                        hostname,
                        port,
                        username,
                        known_hosts,
                        remote_path,
                    )
                    .await?;
                }
                let mut arguments =
                    Self::common_arguments(&self.empty_config, known_hosts, port, true)?;
                arguments.extend([
                    "--".into(),
                    path_text(&local)?,
                    scp_remote(&target, remote_path)?,
                ]);
                let copied = self
                    .run(&self.scp_executable, &arguments, request.timeout_seconds)
                    .await?;
                if !copied.success {
                    return Ok(copied);
                }
                let observed = self
                    .remote_sha256(request, hostname, port, username, known_hosts, remote_path)
                    .await?;
                if observed != *expected_sha256 {
                    return Err(ToolError::Execution(
                        "uploaded file hash did not match local authority".into(),
                    ));
                }
                Ok(copied)
            }
            SshOperationKind::Download {
                remote_path,
                local_path,
                overwrite,
                expected_sha256,
            } => {
                validate_remote_path(remote_path)?;
                if let Some(expected) = expected_sha256 {
                    validate_sha256(expected)?;
                }
                let local = self.resolve_local_write(local_path, *overwrite)?;
                let mut arguments =
                    Self::common_arguments(&self.empty_config, known_hosts, port, true)?;
                arguments.extend([
                    "--".into(),
                    scp_remote(&target, remote_path)?,
                    path_text(&local)?,
                ]);
                let copied = self
                    .run(&self.scp_executable, &arguments, request.timeout_seconds)
                    .await?;
                if copied.success {
                    let observed = file_sha256(&local)?;
                    if expected_sha256
                        .as_ref()
                        .is_some_and(|expected| expected != &observed)
                    {
                        let _ = fs::remove_file(&local);
                        return Err(ToolError::Execution(
                            "downloaded file hash did not match expected evidence".into(),
                        ));
                    }
                }
                Ok(copied)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn require_remote_absent(
        &self,
        request: &ToolRequest,
        hostname: &str,
        port: u16,
        username: &str,
        known_hosts: &Path,
        remote_path: &str,
    ) -> Result<(), ToolError> {
        let target = ssh_target(username, hostname)?;
        let mut arguments = Self::common_arguments(&self.empty_config, known_hosts, port, false)?;
        arguments.extend([
            "-n".into(),
            "-T".into(),
            "--".into(),
            target,
            format!("test ! -e {}", quote_posix(remote_path)?),
        ]);
        let result = self
            .run(&self.ssh_executable, &arguments, request.timeout_seconds)
            .await?;
        if result.success {
            Ok(())
        } else {
            Err(ToolError::Denied(
                "remote destination exists or could not be checked".into(),
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn remote_sha256(
        &self,
        request: &ToolRequest,
        hostname: &str,
        port: u16,
        username: &str,
        known_hosts: &Path,
        remote_path: &str,
    ) -> Result<String, ToolError> {
        let target = ssh_target(username, hostname)?;
        let mut arguments = Self::common_arguments(&self.empty_config, known_hosts, port, false)?;
        arguments.extend([
            "-n".into(),
            "-T".into(),
            "--".into(),
            target,
            format!("sha256sum -- {}", quote_posix(remote_path)?),
        ]);
        let result = self
            .run(&self.ssh_executable, &arguments, request.timeout_seconds)
            .await?;
        if !result.success {
            return Err(ToolError::Execution(
                "remote SHA-256 verification failed".into(),
            ));
        }
        let value = String::from_utf8_lossy(&result.stdout)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned();
        validate_sha256(&value)?;
        Ok(value)
    }
}

#[async_trait]
impl ToolExecutor for SshExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let ToolRequestKind::SshOperation {
            hostname,
            port,
            username,
            known_hosts_entry,
            host_key_sha256,
            kind,
            ..
        } = &request.kind
        else {
            return Err(ToolError::Invalid(
                "SSH executor received a different tool kind".into(),
            ));
        };
        self.execute_operation(
            execution_id,
            request,
            hostname,
            *port,
            username,
            known_hosts_entry,
            host_key_sha256,
            kind,
        )
        .await
    }
}

fn encode_posix_command(
    executable: &str,
    arguments: &[String],
    working_directory: Option<&str>,
) -> Result<String, ToolError> {
    if executable.is_empty()
        || executable.starts_with('-')
        || executable.chars().count() > 512
        || executable.chars().any(char::is_control)
    {
        return Err(ToolError::Invalid("invalid remote executable".into()));
    }
    if arguments.len() > 256 {
        return Err(ToolError::Invalid(
            "remote command has too many arguments".into(),
        ));
    }
    let command = std::iter::once(executable)
        .chain(arguments.iter().map(String::as_str))
        .map(quote_posix)
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    if let Some(directory) = working_directory {
        validate_remote_path(directory)?;
        Ok(format!(
            "cd -- {} && exec {command}",
            quote_posix(directory)?
        ))
    } else {
        Ok(format!("exec {command}"))
    }
}

fn quote_posix(value: &str) -> Result<String, ToolError> {
    if value.chars().count() > 8_192 || value.chars().any(char::is_control) {
        return Err(ToolError::Invalid(
            "remote argument contains control characters or is too long".into(),
        ));
    }
    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}

fn validate_connection(
    hostname: &str,
    port: u16,
    username: &str,
    known_hosts_entry: &str,
    host_key_sha256: &str,
) -> Result<(), ToolError> {
    if !valid_atom(hostname, 253, ".-:")
        || port == 0
        || !valid_atom(username, 128, "._-")
        || !host_key_sha256.starts_with("SHA256:")
        || host_key_sha256.chars().count() > 96
    {
        return Err(ToolError::Invalid(
            "invalid SSH connection authority".into(),
        ));
    }
    validate_known_hosts_entry(known_hosts_entry)?;
    let pinned_target = known_hosts_entry
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let expected_target = if port == 22 {
        hostname.to_owned()
    } else {
        format!("[{hostname}]:{port}")
    };
    if !pinned_target
        .split(',')
        .any(|target| target == expected_target)
    {
        return Err(ToolError::Denied(
            "pinned known_hosts entry does not authorize the requested host and port".into(),
        ));
    }
    Ok(())
}

fn validate_known_hosts_entry(entry: &str) -> Result<(), ToolError> {
    let mut fields = entry.split_whitespace();
    let targets = fields.next().unwrap_or_default();
    let algorithm = fields.next().unwrap_or_default();
    let key = fields.next().unwrap_or_default();
    if targets.is_empty()
        || !matches!(
            algorithm,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
        )
        || key.is_empty()
        || fields.next().is_some()
        || entry.chars().count() > 8_192
        || entry.chars().any(char::is_control)
    {
        return Err(ToolError::Invalid(
            "invalid pinned known_hosts entry".into(),
        ));
    }
    Ok(())
}

fn validate_remote_path(value: &str) -> Result<(), ToolError> {
    if !value.starts_with('/')
        || value.chars().count() > 4_096
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|component| matches!(component, "." | ".."))
    {
        return Err(ToolError::Invalid(
            "remote path must be absolute and normalized".into(),
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), ToolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ToolError::Invalid("invalid SHA-256 digest".into()));
    }
    Ok(())
}

fn valid_atom(value: &str, max: usize, punctuation: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= max
        && !value.starts_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || punctuation.contains(character))
}

fn ssh_target(username: &str, hostname: &str) -> Result<String, ToolError> {
    if !valid_atom(hostname, 253, ".-:") || !valid_atom(username, 128, "._-") {
        return Err(ToolError::Invalid("invalid SSH target".into()));
    }
    if hostname.contains(':') {
        Ok(format!("{username}@[{hostname}]"))
    } else {
        Ok(format!("{username}@{hostname}"))
    }
}

fn scp_remote(target: &str, remote_path: &str) -> Result<String, ToolError> {
    validate_remote_path(remote_path)?;
    Ok(format!("{target}:{remote_path}"))
}

fn resolve_regular_file(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let resolved = path
        .canonicalize()
        .map_err(|error| ToolError::Unavailable(format!("resolve {label}: {error}")))?;
    if !resolved.is_file() {
        return Err(ToolError::Unavailable(format!("{label} is not a file")));
    }
    Ok(resolved)
}

fn path_text(path: &Path) -> Result<String, ToolError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ToolError::Unavailable("local path is not UTF-8".into()))
}

fn file_sha256(path: &Path) -> Result<String, ToolError> {
    let bytes = fs::read(path)
        .map_err(|error| ToolError::Execution(format!("read file for SHA-256: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn preserve_ssh_environment(command: &mut Command) {
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "HOME",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "SSH_AUTH_SOCK",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn drain_bounded(
    reader: Option<impl AsyncRead + Unpin + Send + 'static>,
    limit: usize,
) -> tokio::task::JoinHandle<Result<(Vec<u8>, bool), std::io::Error>> {
    tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return Ok((Vec::new(), false));
        };
        let mut retained = Vec::new();
        let mut truncated = false;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let read = reader.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            let remaining = limit.saturating_sub(retained.len());
            let keep = remaining.min(read);
            retained.extend_from_slice(&buffer[..keep]);
            truncated |= keep < read;
        }
        Ok((retained, truncated))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_arguments_are_literal_and_controls_fail_closed() {
        assert_eq!(
            encode_posix_command(
                "printf",
                &["%s".into(), "hello; touch /tmp/pwned's".into()],
                Some("/tmp/build dir"),
            )
            .expect("encode"),
            "cd -- '/tmp/build dir' && exec 'printf' '%s' 'hello; touch /tmp/pwned'\"'\"'s'"
        );
        assert!(encode_posix_command("printf", &["bad\narg".into()], None).is_err());
        assert!(encode_posix_command("-oProxyCommand=evil", &[], None).is_err());
    }

    #[test]
    fn targets_paths_and_digests_reject_option_and_traversal_injection() {
        assert!(ssh_target("-oProxyCommand=evil", "example.com").is_err());
        assert!(ssh_target("user", "-example.com").is_err());
        assert!(validate_remote_path("/srv/../secret").is_err());
        assert!(validate_remote_path("relative/file").is_err());
        assert!(validate_sha256(&"a".repeat(64)).is_ok());
    }
}
