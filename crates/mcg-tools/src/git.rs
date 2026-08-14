use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use mcg_domain::ToolExecutionId;

use crate::{
    ArtifactStore, ProcessExecutor, ProcessOptions, ProcessStream, TOOL_SCHEMA_VERSION, ToolError,
    ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest, ToolRequestKind,
    VerificationEvidence,
};

pub struct GitExecutor {
    process: Arc<ProcessExecutor>,
    roots: Vec<PathBuf>,
    artifacts: ArtifactStore,
}

impl GitExecutor {
    /// Creates a Git executor constrained to project and worktree roots.
    ///
    /// # Errors
    ///
    /// Returns an error when no root is supplied or one cannot be canonicalized.
    pub fn new(
        process: Arc<ProcessExecutor>,
        roots: impl IntoIterator<Item = PathBuf>,
        artifact_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let roots = roots
            .into_iter()
            .map(|root| {
                root.canonicalize().map_err(|error| {
                    ToolError::Execution(format!("resolve Git capability root: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if roots.is_empty() {
            return Err(ToolError::Invalid(
                "at least one Git capability root is required".into(),
            ));
        }
        Ok(Self {
            process,
            roots,
            artifacts: ArtifactStore::open(artifact_root)?,
        })
    }

    fn repository(&self, value: &str) -> Result<PathBuf, ToolError> {
        let canonical = canonical_scoped_path(value, &self.roots, true)?;
        if !canonical.join(".git").exists() {
            return Err(ToolError::Invalid("path is not a Git worktree".into()));
        }
        Ok(canonical)
    }

    fn worktree_destination(&self, value: &str) -> Result<PathBuf, ToolError> {
        canonical_scoped_path(value, &self.roots, false)
    }

    async fn git(
        &self,
        outer: &ToolRequest,
        repository: &Path,
        argv: Vec<String>,
        stdin_data: Option<String>,
    ) -> Result<ToolOutcome, ToolError> {
        let internal = ToolRequest {
            schema_version: TOOL_SCHEMA_VERSION,
            id: mcg_domain::ToolRequestId::new(),
            task_id: outer.task_id,
            namespace: outer.namespace,
            summary: format!("internal git for {}", outer.kind.stable_name()),
            timeout_seconds: outer.timeout_seconds,
            kind: ToolRequestKind::ProcessExec {
                options: ProcessOptions {
                    executable: "git".into(),
                    argv,
                    cwd: repository.to_string_lossy().into(),
                    env: Vec::new(),
                    stdin: if stdin_data.is_some() {
                        ProcessStream::Piped
                    } else {
                        ProcessStream::Null
                    },
                    stdin_data,
                    stdout: ProcessStream::Piped,
                    stderr: ProcessStream::Piped,
                },
            },
            created_at: Utc::now(),
        };
        let outcome = self
            .process
            .execute(ToolExecutionId::new(), &internal)
            .await?;
        if outcome.status != ToolOutcomeStatus::Succeeded {
            return Err(ToolError::Execution(format!(
                "git command failed: {}",
                outcome.stderr.unwrap_or(outcome.summary)
            )));
        }
        Ok(outcome)
    }

    async fn output_command(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        repository: &str,
        argv: Vec<String>,
        summary: &str,
    ) -> Result<ToolOutcome, ToolError> {
        let repository = self.repository(repository)?;
        let command = self.git(request, &repository, argv, None).await?;
        let output = command.stdout.unwrap_or_default();
        let artifact = self.artifacts.capture(output.as_bytes(), "text/plain")?;
        Ok(git_outcome(
            execution_id,
            request,
            summary,
            Some(output),
            vec![artifact],
            "git.command_succeeded",
        ))
    }

    async fn commit(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        repository: &str,
        message: &str,
        paths: &[String],
    ) -> Result<ToolOutcome, ToolError> {
        let repository = self.repository(repository)?;
        if message.trim().is_empty() || message.contains('\0') {
            return Err(ToolError::Invalid("commit message is invalid".into()));
        }
        if paths.is_empty() {
            return Err(ToolError::Invalid(
                "git.commit requires explicit project-relative paths".into(),
            ));
        }
        validate_pathspecs(paths)?;
        let mut add = vec!["add".into(), "--".into()];
        add.extend(paths.iter().cloned());
        self.git(request, &repository, add, None).await?;
        let mut commit = vec![
            "commit".into(),
            "--only".into(),
            "-m".into(),
            message.into(),
            "--".into(),
        ];
        commit.extend(paths.iter().cloned());
        let committed = self.git(request, &repository, commit, None).await?;
        let head = self
            .git(
                request,
                &repository,
                vec!["rev-parse".into(), "HEAD".into()],
                None,
            )
            .await?
            .stdout
            .unwrap_or_default()
            .trim()
            .to_owned();
        Ok(git_outcome(
            execution_id,
            request,
            "committed explicit paths",
            committed.stdout,
            Vec::new(),
            &format!("git.head={head}"),
        ))
    }

    async fn checkout(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        repository: &str,
        reference: &str,
        create: bool,
    ) -> Result<ToolOutcome, ToolError> {
        let repository = self.repository(repository)?;
        validate_reference(reference)?;
        let argv = if create {
            vec!["switch".into(), "-c".into(), reference.into()]
        } else {
            vec!["switch".into(), reference.into()]
        };
        self.git(request, &repository, argv, None).await?;
        let observed = self
            .git(
                request,
                &repository,
                vec!["branch".into(), "--show-current".into()],
                None,
            )
            .await?
            .stdout
            .unwrap_or_default();
        Ok(git_outcome(
            execution_id,
            request,
            "switched Git reference",
            Some(observed.trim().into()),
            Vec::new(),
            "git.reference_observed",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn worktree_create(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        repository: &str,
        worktree: &str,
        branch: &str,
        base_reference: &str,
    ) -> Result<ToolOutcome, ToolError> {
        let repository = self.repository(repository)?;
        let destination = self.worktree_destination(worktree)?;
        if destination.exists() {
            return Err(ToolError::Execution(
                "worktree destination already exists".into(),
            ));
        }
        validate_reference(branch)?;
        validate_reference(base_reference)?;
        self.git(
            request,
            &repository,
            vec![
                "worktree".into(),
                "add".into(),
                "-b".into(),
                branch.into(),
                "--".into(),
                external_path(&destination),
                base_reference.into(),
            ],
            None,
        )
        .await?;
        let observed = destination.join(".git").is_file() || destination.join(".git").is_dir();
        if !observed {
            return Err(ToolError::Execution(
                "Git reported success but worktree metadata is absent".into(),
            ));
        }
        Ok(git_outcome(
            execution_id,
            request,
            "created isolated Git worktree",
            Some(destination.to_string_lossy().into()),
            Vec::new(),
            "git.worktree_metadata_observed",
        ))
    }

    async fn worktree_remove(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        repository: &str,
        worktree: &str,
        force: bool,
    ) -> Result<ToolOutcome, ToolError> {
        let repository = self.repository(repository)?;
        let worktree = self.worktree_destination(worktree)?;
        let mut argv = vec!["worktree".into(), "remove".into()];
        if force {
            argv.push("--force".into());
        }
        argv.extend(["--".into(), external_path(&worktree)]);
        self.git(request, &repository, argv, None).await?;
        Ok(git_outcome(
            execution_id,
            request,
            "removed isolated Git worktree",
            None,
            Vec::new(),
            if worktree.exists() {
                "git.worktree_removal_unverified"
            } else {
                "git.worktree_absent"
            },
        ))
    }

    async fn apply_patch(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: ApplyPatchParameters<'_>,
    ) -> Result<ToolOutcome, ToolError> {
        if parameters.patch.is_empty() || parameters.patch.len() > 16 * 1_048_576 {
            return Err(ToolError::Invalid(
                "Git patch must be between 1 byte and 16 MiB".into(),
            ));
        }
        let repository = self.repository(parameters.repository)?;
        self.git(
            request,
            &repository,
            vec![
                "apply".into(),
                "--check".into(),
                "--whitespace=error-all".into(),
                "-".into(),
            ],
            Some(parameters.patch.into()),
        )
        .await?;
        if parameters.check_only {
            return Ok(git_outcome(
                execution_id,
                request,
                "patch applies cleanly",
                None,
                Vec::new(),
                "git.apply_check",
            ));
        }
        let mut argv = vec!["apply".into(), "--whitespace=error-all".into()];
        if parameters.three_way {
            argv.push("--3way".into());
        }
        argv.push("-".into());
        self.git(request, &repository, argv, Some(parameters.patch.into()))
            .await?;
        let diff = self
            .git(
                request,
                &repository,
                vec!["diff".into(), "--no-ext-diff".into(), "--binary".into()],
                None,
            )
            .await?
            .stdout
            .unwrap_or_default();
        let artifact = self.artifacts.capture(diff.as_bytes(), "text/x-diff")?;
        Ok(git_outcome(
            execution_id,
            request,
            "applied conflict-checked Git patch",
            Some(diff),
            vec![artifact],
            "git.diff_observed",
        ))
    }
}

#[async_trait]
impl ToolExecutor for GitExecutor {
    #[allow(clippy::too_many_lines)]
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::GitStatus { repository } => {
                self.output_command(
                    execution_id,
                    request,
                    repository,
                    vec!["status".into(), "--porcelain=v2".into(), "--branch".into()],
                    "captured Git status",
                )
                .await
            }
            ToolRequestKind::GitDiff { repository, staged } => {
                let mut argv = vec!["diff".into(), "--no-ext-diff".into(), "--binary".into()];
                if *staged {
                    argv.push("--cached".into());
                }
                self.output_command(execution_id, request, repository, argv, "captured Git diff")
                    .await
            }
            ToolRequestKind::GitCommit {
                repository,
                message,
                paths,
            } => {
                self.commit(execution_id, request, repository, message, paths)
                    .await
            }
            ToolRequestKind::GitCheckout {
                repository,
                reference,
                create,
            } => {
                self.checkout(execution_id, request, repository, reference, *create)
                    .await
            }
            ToolRequestKind::GitWorktreeCreate {
                repository,
                worktree,
                branch,
                base_reference,
            } => {
                self.worktree_create(
                    execution_id,
                    request,
                    repository,
                    worktree,
                    branch,
                    base_reference,
                )
                .await
            }
            ToolRequestKind::GitWorktreeRemove {
                repository,
                worktree,
                force,
            } => {
                self.worktree_remove(execution_id, request, repository, worktree, *force)
                    .await
            }
            ToolRequestKind::GitApplyPatch {
                repository,
                patch,
                three_way,
                check_only,
            } => {
                self.apply_patch(
                    execution_id,
                    request,
                    ApplyPatchParameters {
                        repository,
                        patch,
                        three_way: *three_way,
                        check_only: *check_only,
                    },
                )
                .await
            }
            _ => Err(ToolError::Invalid(
                "Git executor received an unsupported tool".into(),
            )),
        }
    }
}

struct ApplyPatchParameters<'a> {
    repository: &'a str,
    patch: &'a str,
    three_way: bool,
    check_only: bool,
}

fn canonical_scoped_path(
    value: &str,
    roots: &[PathBuf],
    must_exist: bool,
) -> Result<PathBuf, ToolError> {
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(ToolError::Denied(
            "Git paths must be absolute and normalized".into(),
        ));
    }
    let canonical = if must_exist || path.exists() {
        path.canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve Git path: {error}")))?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| ToolError::Invalid("Git path has no parent".into()))?
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve Git path parent: {error}")))?;
        parent.join(
            path.file_name()
                .ok_or_else(|| ToolError::Invalid("Git path has no file name".into()))?,
        )
    };
    if !roots.iter().any(|root| canonical.starts_with(root)) {
        return Err(ToolError::Denied(
            "Git path is outside capability roots".into(),
        ));
    }
    Ok(canonical)
}

fn validate_pathspecs(paths: &[String]) -> Result<(), ToolError> {
    for value in paths {
        let path = Path::new(value);
        if value.is_empty()
            || value.starts_with('-')
            || path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(ToolError::Invalid(format!(
                "Git pathspec must be normalized and project-relative: {value}"
            )));
        }
    }
    Ok(())
}

fn validate_reference(value: &str) -> Result<(), ToolError> {
    if value.is_empty()
        || value.starts_with('-')
        || value.contains(['\0', ' ', '~', '^', ':', '?', '*', '[', '\\'])
        || value.contains("..")
        || value.contains("@{")
        || value.ends_with(['.', '/'])
    {
        return Err(ToolError::Invalid("Git reference is invalid".into()));
    }
    Ok(())
}

fn external_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(local) = value.strip_prefix(r"\\?\") {
            return local.into();
        }
    }
    value.into_owned()
}

fn git_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    summary: &str,
    stdout: Option<String>,
    artifacts: Vec<crate::ArtifactReference>,
    verification: &str,
) -> ToolOutcome {
    let now = Utc::now();
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: summary.into(),
        exit_code: Some(0),
        stdout,
        stderr: None,
        artifacts,
        verification: vec![VerificationEvidence {
            method: verification.into(),
            passed: !verification.ends_with("unverified"),
            detail: verification.into(),
        }],
        started_at: now,
        finished_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use tempfile::tempdir;

    use super::*;
    use crate::RejectingCredentialResolver;

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, GitExecutor) {
        let root = tempdir().expect("root");
        let artifacts = tempdir().expect("artifacts");
        let repository = root.path().join("repository");
        std::fs::create_dir(&repository).expect("repo dir");
        for arguments in [
            vec!["init"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "Fixture"],
        ] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(&repository)
                    .status()
                    .expect("git")
                    .success()
            );
        }
        std::fs::write(repository.join("file.txt"), "first\n").expect("file");
        assert!(
            Command::new("git")
                .args(["add", "file.txt"])
                .current_dir(&repository)
                .status()
                .expect("add")
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-m", "initial"])
                .current_dir(&repository)
                .status()
                .expect("commit")
                .success()
        );
        let process = Arc::new(
            ProcessExecutor::new(
                [root.path().to_path_buf()],
                Arc::new(RejectingCredentialResolver),
            )
            .expect("process"),
        );
        let git = GitExecutor::new(process, [root.path().to_path_buf()], artifacts.path())
            .expect("git executor");
        (root, artifacts, git)
    }

    #[tokio::test]
    async fn stale_patch_is_rejected_without_modifying_worktree() {
        let (root, _artifacts, git) = fixture();
        let repository = root.path().join("repository");
        let patch = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-missing\n+second\n";
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "patch",
            ToolRequestKind::GitApplyPatch {
                repository: repository.to_string_lossy().into(),
                patch: patch.into(),
                three_way: false,
                check_only: false,
            },
        );
        assert!(git.execute(ToolExecutionId::new(), &request).await.is_err());
        assert_eq!(
            std::fs::read_to_string(repository.join("file.txt")).expect("read"),
            "first\n"
        );
    }

    #[tokio::test]
    async fn creates_and_removes_isolated_worktree() {
        let (root, _artifacts, git) = fixture();
        let repository = root.path().join("repository");
        let worktree = root.path().join("isolated");
        let create = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "worktree",
            ToolRequestKind::GitWorktreeCreate {
                repository: repository.to_string_lossy().into(),
                worktree: worktree.to_string_lossy().into(),
                branch: "fixture-branch".into(),
                base_reference: "HEAD".into(),
            },
        );
        git.execute(ToolExecutionId::new(), &create)
            .await
            .expect("create");
        assert!(worktree.join(".git").exists());
        let remove = ToolRequest::new(
            create.task_id,
            "remove",
            ToolRequestKind::GitWorktreeRemove {
                repository: repository.to_string_lossy().into(),
                worktree: worktree.to_string_lossy().into(),
                force: false,
            },
        );
        git.execute(ToolExecutionId::new(), &remove)
            .await
            .expect("remove");
        assert!(!worktree.exists());
    }
}
