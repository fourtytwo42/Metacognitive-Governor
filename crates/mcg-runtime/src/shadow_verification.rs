#[cfg(any(target_os = "linux", test))]
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};
use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use mcg_agents::SuccessCondition;
#[cfg(any(target_os = "linux", test))]
use mcg_domain::{TaskId, ToolExecutionId};
use mcg_tools::{ArtifactReference, ArtifactStore};
#[cfg(any(target_os = "linux", test))]
use mcg_tools::{
    ProcessExecutor, ProcessOptions, ProcessStream, RejectingCredentialResolver, ToolError,
    ToolExecutor, ToolRequest, ToolRequestKind,
};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "linux", test))]
use uuid::Uuid;

pub(crate) const SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION: u16 = 1;
#[cfg(any(target_os = "linux", test))]
const BWRAP_PROOF: &[u8] = b"mcg-shadow-bwrap-v1\n";
#[cfg(any(target_os = "linux", test))]
const BWRAP_TIMEOUT_SECONDS: u64 = 600;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_SOURCE_FILES: u64 = 50_000;
#[cfg(any(target_os = "linux", test))]
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
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_ARGUMENTS: usize = 128;
#[cfg(any(target_os = "linux", test))]
const BWRAP_MAX_COMMAND_BYTES: usize = 32 * 1024;

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

#[cfg(any(target_os = "linux", test))]
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

#[cfg(any(target_os = "linux", test))]
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

#[cfg(any(target_os = "linux", test))]
fn verify_workspace_bounds(root: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| ToolError::Execution(format!("inspect shadow workspace: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
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
            if metadata.file_type().is_symlink() {
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
