use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use chrono::Utc;
use mcg_domain::ToolExecutionId;
use sha2::{Digest, Sha256};

use crate::{
    ArtifactStore, TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus,
    ToolRequest, ToolRequestKind, VerificationEvidence,
};

struct CapabilityRoot {
    scope: PathBuf,
    canonical: PathBuf,
    directory: Dir,
}

pub struct FilesystemExecutor {
    roots: Vec<CapabilityRoot>,
    artifacts: ArtifactStore,
}

impl FilesystemExecutor {
    /// Opens fixed directory capabilities used for all later file operations.
    ///
    /// # Errors
    ///
    /// Returns an error if no root is supplied or a root cannot be resolved/opened.
    pub fn open(
        roots: impl IntoIterator<Item = PathBuf>,
        artifact_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let mut capabilities = Vec::new();
        for root in roots {
            let scope = if root.is_absolute() {
                root.clone()
            } else {
                std::env::current_dir()
                    .map_err(|error| ToolError::Execution(format!("resolve current dir: {error}")))?
                    .join(&root)
            };
            let canonical = root
                .canonicalize()
                .map_err(|error| ToolError::Execution(format!("resolve file root: {error}")))?;
            let directory = Dir::open_ambient_dir(&canonical, ambient_authority())
                .map_err(|error| ToolError::Execution(format!("open file root: {error}")))?;
            capabilities.push(CapabilityRoot {
                scope,
                canonical,
                directory,
            });
        }
        if capabilities.is_empty() {
            return Err(ToolError::Invalid(
                "at least one filesystem capability root is required".into(),
            ));
        }
        capabilities.sort_by(|left, right| {
            right
                .canonical
                .components()
                .count()
                .cmp(&left.canonical.components().count())
        });
        Ok(Self {
            roots: capabilities,
            artifacts: ArtifactStore::open(artifact_root)?,
        })
    }

    fn resolve(&self, requested: &str) -> Result<(&Dir, PathBuf), ToolError> {
        let path = Path::new(requested);
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(ToolError::Denied(
                "filesystem paths must be absolute and normalized".into(),
            ));
        }
        self.roots
            .iter()
            .find_map(|root| {
                path.strip_prefix(&root.canonical)
                    .or_else(|_| path.strip_prefix(&root.scope))
                    .ok()
                    .map(|relative| (&root.directory, relative.to_path_buf()))
            })
            .ok_or_else(|| ToolError::Denied("path is outside filesystem capabilities".into()))
    }

    fn read(&self, path: &str, max_bytes: u64) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        let bytes = root
            .read(&relative)
            .map_err(|error| ToolError::Execution(format!("read file: {error}")))?;
        let size = u64::try_from(bytes.len())
            .map_err(|_| ToolError::Execution("file is too large".into()))?;
        if size > max_bytes {
            return Err(ToolError::Execution(format!(
                "file is {size} bytes, above the {max_bytes}-byte response limit"
            )));
        }
        let artifact = self.artifacts.capture(&bytes, "application/octet-stream")?;
        Ok(ExecutionResult {
            summary: format!("read {size} bytes"),
            stdout: Some(String::from_utf8_lossy(&bytes).into_owned()),
            artifacts: vec![artifact],
            verification: vec![VerificationEvidence {
                method: "sha256".into(),
                passed: true,
                detail: sha256(&bytes),
            }],
        })
    }

    fn verify(&self, path: &str, max_bytes: u64) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        let file = root.open(&relative).map_err(|error| {
            ToolError::Execution(format!("open file for verification: {error}"))
        })?;
        let metadata = file.metadata().map_err(|error| {
            ToolError::Execution(format!("inspect file for verification: {error}"))
        })?;
        if !metadata.is_file() {
            return Err(ToolError::Execution(
                "objective evidence path is not a regular file".into(),
            ));
        }
        let artifact =
            self.artifacts
                .capture_reader(file, max_bytes, "application/octet-stream")?;
        Ok(ExecutionResult {
            summary: format!("verified regular file ({} bytes)", artifact.size_bytes),
            stdout: None,
            verification: vec![
                VerificationEvidence {
                    method: "sha256".into(),
                    passed: true,
                    detail: artifact.sha256.clone(),
                },
                VerificationEvidence {
                    method: "file.size".into(),
                    passed: true,
                    detail: artifact.size_bytes.to_string(),
                },
            ],
            artifacts: vec![artifact],
        })
    }

    fn write(
        &self,
        path: &str,
        content: &str,
        create_new: bool,
    ) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        if create_new && root.symlink_metadata(&relative).is_ok() {
            return Err(ToolError::Execution("destination already exists".into()));
        }
        atomic_write(root, &relative, content.as_bytes())?;
        verify_file(root, &relative, content.as_bytes(), "wrote file")
    }

    pub(crate) fn write_download(
        &self,
        path: &str,
        bytes: &[u8],
    ) -> Result<VerificationEvidence, ToolError> {
        let (root, relative) = self.resolve(path)?;
        atomic_write(root, &relative, bytes)?;
        let observed = root
            .read(&relative)
            .map_err(|error| ToolError::Execution(format!("verify download: {error}")))?;
        let expected_sha = sha256(bytes);
        let observed_sha = sha256(&observed);
        if expected_sha != observed_sha {
            return Err(ToolError::Execution(
                "download verification hash mismatch after write".into(),
            ));
        }
        Ok(VerificationEvidence {
            method: "sha256.read_after_download".into(),
            passed: true,
            detail: observed_sha,
        })
    }

    fn list(
        &self,
        path: &str,
        recursive: bool,
        max_entries: u64,
    ) -> Result<ExecutionResult, ToolError> {
        if max_entries == 0 {
            return Err(ToolError::Invalid("max_entries must be positive".into()));
        }
        let (root, relative) = self.resolve(path)?;
        let mut pending = VecDeque::from([relative.clone()]);
        let mut entries = Vec::new();
        while let Some(directory) = pending.pop_front() {
            let read_dir = root
                .read_dir(&directory)
                .map_err(|error| ToolError::Execution(format!("list directory: {error}")))?;
            for entry in read_dir {
                let entry = entry.map_err(|error| {
                    ToolError::Execution(format!("read directory entry: {error}"))
                })?;
                let child = directory.join(entry.file_name());
                let file_type = entry.file_type().map_err(|error| {
                    ToolError::Execution(format!("inspect directory entry: {error}"))
                })?;
                entries.push(child.to_string_lossy().replace('\\', "/"));
                if u64::try_from(entries.len())
                    .map_err(|_| ToolError::Execution("entry count overflow".into()))?
                    >= max_entries
                {
                    let stdout = serde_json::to_string(&entries).map_err(|error| {
                        ToolError::Execution(format!("serialize directory listing: {error}"))
                    })?;
                    return Ok(ExecutionResult {
                        summary: format!("listed {} entries (limit reached)", entries.len()),
                        stdout: Some(stdout),
                        artifacts: Vec::new(),
                        verification: vec![VerificationEvidence {
                            method: "entry_limit".into(),
                            passed: true,
                            detail: max_entries.to_string(),
                        }],
                    });
                }
                if recursive && file_type.is_dir() && !file_type.is_symlink() {
                    pending.push_back(child);
                }
            }
        }
        let stdout = serde_json::to_string(&entries).map_err(|error| {
            ToolError::Execution(format!("serialize directory listing: {error}"))
        })?;
        Ok(ExecutionResult {
            summary: format!("listed {} entries", entries.len()),
            stdout: Some(stdout),
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "directory.enumeration".into(),
                passed: true,
                detail: entries.len().to_string(),
            }],
        })
    }

    fn patch(
        &self,
        path: &str,
        expected_sha256: &str,
        replacement: &str,
    ) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        let current = root
            .read(&relative)
            .map_err(|error| ToolError::Execution(format!("read patch target: {error}")))?;
        let actual = sha256(&current);
        if actual != expected_sha256.to_ascii_lowercase() {
            return Err(ToolError::Execution(format!(
                "patch conflict: expected {expected_sha256}, observed {actual}"
            )));
        }
        atomic_write(root, &relative, replacement.as_bytes())?;
        verify_file(root, &relative, replacement.as_bytes(), "patched file")
    }

    fn mkdir(&self, path: &str, recursive: bool) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        let result = if recursive {
            root.create_dir_all(&relative)
        } else {
            root.create_dir(&relative)
        };
        result.map_err(|error| ToolError::Execution(format!("create directory: {error}")))?;
        let observed = root
            .metadata(&relative)
            .map_err(|error| ToolError::Execution(format!("verify directory: {error}")))?
            .is_dir();
        Ok(simple_result(
            "created directory",
            "metadata.is_dir",
            observed,
        ))
    }

    fn copy_or_move(&self, from: &str, to: &str, copy: bool) -> Result<ExecutionResult, ToolError> {
        let (from_root, from_relative) = self.resolve(from)?;
        let (to_root, to_relative) = self.resolve(to)?;
        if copy {
            from_root
                .copy(&from_relative, to_root, &to_relative)
                .map_err(|error| ToolError::Execution(format!("copy file: {error}")))?;
        } else {
            from_root
                .rename(&from_relative, to_root, &to_relative)
                .map_err(|error| ToolError::Execution(format!("move file: {error}")))?;
        }
        let observed = to_root.metadata(&to_relative).is_ok();
        Ok(simple_result(
            if copy { "copied file" } else { "moved file" },
            "destination.exists",
            observed,
        ))
    }

    fn delete(&self, path: &str, recursive: bool) -> Result<ExecutionResult, ToolError> {
        let (root, relative) = self.resolve(path)?;
        let metadata = root
            .symlink_metadata(&relative)
            .map_err(|error| ToolError::Execution(format!("inspect delete target: {error}")))?;
        let result = if metadata.is_dir() {
            if recursive {
                root.remove_dir_all(&relative)
            } else {
                root.remove_dir(&relative)
            }
        } else {
            root.remove_file(&relative)
        };
        result.map_err(|error| ToolError::Execution(format!("delete path: {error}")))?;
        Ok(simple_result(
            "deleted path",
            "metadata.not_found",
            root.symlink_metadata(&relative).is_err(),
        ))
    }
}

#[async_trait]
impl ToolExecutor for FilesystemExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let result = match &request.kind {
            ToolRequestKind::FsRead { path, max_bytes } => self.read(path, *max_bytes),
            ToolRequestKind::FsVerify { path, max_bytes } => self.verify(path, *max_bytes),
            ToolRequestKind::FsWrite {
                path,
                content,
                create_new,
            } => self.write(path, content, *create_new),
            ToolRequestKind::FsPatch {
                path,
                expected_sha256,
                replacement,
            } => self.patch(path, expected_sha256, replacement),
            ToolRequestKind::FsMkdir { path, recursive } => self.mkdir(path, *recursive),
            ToolRequestKind::FsMove { from, to } => self.copy_or_move(from, to, false),
            ToolRequestKind::FsCopy { from, to } => self.copy_or_move(from, to, true),
            ToolRequestKind::FsDelete { path, recursive } => self.delete(path, *recursive),
            ToolRequestKind::FsList {
                path,
                recursive,
                max_entries,
            } => self.list(path, *recursive, *max_entries),
            _ => Err(ToolError::Invalid(
                "filesystem executor received a non-filesystem tool".into(),
            )),
        }?;
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: ToolOutcomeStatus::Succeeded,
            summary: result.summary,
            exit_code: None,
            stdout: result.stdout,
            stderr: None,
            artifacts: result.artifacts,
            verification: result.verification,
            started_at,
            finished_at: Utc::now(),
        })
    }
}

struct ExecutionResult {
    summary: String,
    stdout: Option<String>,
    artifacts: Vec<crate::ArtifactReference>,
    verification: Vec<VerificationEvidence>,
}

fn atomic_write(root: &Dir, relative: &Path, bytes: &[u8]) -> Result<(), ToolError> {
    let parent = relative
        .parent()
        .ok_or_else(|| ToolError::Invalid("file path has no parent".into()))?;
    let name = relative
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ToolError::Invalid("file name is invalid".into()))?;
    let temporary = parent.join(format!(".{name}.{}.tmp", mcg_domain::ArtifactId::new()));
    root.write(&temporary, bytes)
        .map_err(|error| ToolError::Execution(format!("write temporary file: {error}")))?;
    if let Err(error) = root.rename(&temporary, root, relative) {
        let _ = root.remove_file(&temporary);
        return Err(ToolError::Execution(format!(
            "replace file atomically: {error}"
        )));
    }
    Ok(())
}

fn verify_file(
    root: &Dir,
    relative: &Path,
    expected: &[u8],
    summary: &str,
) -> Result<ExecutionResult, ToolError> {
    let observed = root
        .read(relative)
        .map_err(|error| ToolError::Execution(format!("verify file: {error}")))?;
    let expected_sha = sha256(expected);
    let observed_sha = sha256(&observed);
    Ok(ExecutionResult {
        summary: summary.into(),
        stdout: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: "sha256.read_after_write".into(),
            passed: expected_sha == observed_sha,
            detail: observed_sha,
        }],
    })
}

fn simple_result(summary: &str, method: &str, passed: bool) -> ExecutionResult {
    ExecutionResult {
        summary: summary.into(),
        stdout: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: method.into(),
            passed,
            detail: passed.to_string(),
        }],
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::ToolRequest;

    fn executor() -> (tempfile::TempDir, FilesystemExecutor) {
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let executor = FilesystemExecutor::open([project.path().to_path_buf()], artifacts.path())
            .expect("executor");
        (project, executor)
    }

    #[tokio::test]
    async fn write_patch_and_read_produce_observable_hash_evidence() {
        let (project, executor) = executor();
        let path = project.path().join("result.txt");
        let write = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "write",
            ToolRequestKind::FsWrite {
                path: path.to_string_lossy().into(),
                content: "first".into(),
                create_new: true,
            },
        );
        executor
            .execute(ToolExecutionId::new(), &write)
            .await
            .expect("write");
        let patch_request = ToolRequest::new(
            write.task_id,
            "patch",
            ToolRequestKind::FsPatch {
                path: path.to_string_lossy().into(),
                expected_sha256: sha256(b"first"),
                replacement: "second".into(),
            },
        );
        let patched = executor
            .execute(ToolExecutionId::new(), &patch_request)
            .await
            .expect("patch");
        assert!(patched.verification[0].passed);
        let read = ToolRequest::new(
            write.task_id,
            "read",
            ToolRequestKind::FsRead {
                path: path.to_string_lossy().into(),
                max_bytes: 100,
            },
        );
        let observed = executor
            .execute(ToolExecutionId::new(), &read)
            .await
            .expect("read");
        assert_eq!(observed.stdout.as_deref(), Some("second"));
        assert_eq!(observed.artifacts.len(), 1);
    }

    #[tokio::test]
    async fn verify_streams_binary_evidence_without_stdout_and_enforces_limit() {
        let (project, executor) = executor();
        let path = project.path().join("result.bin");
        let bytes = vec![0xa5; 160_000];
        std::fs::write(&path, &bytes).expect("fixture");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "verify",
            ToolRequestKind::FsVerify {
                path: path.to_string_lossy().into(),
                max_bytes: 160_000,
            },
        );
        let observed = executor
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("verify");
        assert_eq!(observed.stdout, None);
        assert_eq!(observed.artifacts[0].sha256, sha256(&bytes));
        assert_eq!(observed.artifacts[0].size_bytes, 160_000);
        assert_eq!(observed.verification[1].detail, "160000");

        let too_small = ToolRequest::new(
            request.task_id,
            "bounded verify",
            ToolRequestKind::FsVerify {
                path: path.to_string_lossy().into(),
                max_bytes: 159_999,
            },
        );
        assert!(matches!(
            executor.execute(ToolExecutionId::new(), &too_small).await,
            Err(ToolError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn traversal_and_sibling_prefix_are_denied() {
        let (project, executor) = executor();
        for path in [
            project.path().join("..").join("escape.txt"),
            PathBuf::from(format!("{}-sibling/file", project.path().display())),
        ] {
            let request = ToolRequest::new(
                mcg_domain::TaskId::new(),
                "escape",
                ToolRequestKind::FsRead {
                    path: path.to_string_lossy().into(),
                    max_bytes: 100,
                },
            );
            assert!(matches!(
                executor.execute(ToolExecutionId::new(), &request).await,
                Err(ToolError::Denied(_))
            ));
        }
    }

    #[tokio::test]
    async fn stale_patch_hash_has_no_side_effect() {
        let (project, executor) = executor();
        let path = project.path().join("conflict.txt");
        std::fs::write(&path, "owner edit").expect("fixture");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "conflicting patch",
            ToolRequestKind::FsPatch {
                path: path.to_string_lossy().into(),
                expected_sha256: sha256(b"old value"),
                replacement: "agent edit".into(),
            },
        );
        assert!(
            executor
                .execute(ToolExecutionId::new(), &request)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(path).expect("read"), "owner edit");
    }
}
