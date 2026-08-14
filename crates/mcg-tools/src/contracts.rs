use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mcg_domain::{
    ApprovalId, ArtifactId, LeaseId, ManagedProcessId, TaskId, ToolExecutionId, ToolRequestId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const TOOL_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolNamespace {
    Caller,
    Host,
    Mcg,
    ExternalMcp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialReference {
    pub provider: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestHeader {
    pub name: String,
    #[serde(flatten)]
    pub value: HeaderValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum HeaderValue {
    Literal(String),
    Credential(CredentialReference),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStream {
    Null,
    Piped,
    Inherit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessOptions {
    pub executable: String,
    #[serde(default)]
    pub argv: Vec<String>,
    pub cwd: String,
    #[serde(default)]
    pub env: Vec<ProcessEnvironmentVariable>,
    pub stdin: ProcessStream,
    #[serde(default)]
    pub stdin_data: Option<String>,
    pub stdout: ProcessStream,
    pub stderr: ProcessStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEnvironmentVariable {
    pub name: String,
    #[serde(flatten)]
    pub value: HeaderValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum ToolRequestKind {
    FsRead {
        path: String,
        max_bytes: u64,
    },
    FsVerify {
        path: String,
        max_bytes: u64,
    },
    FsWrite {
        path: String,
        content: String,
        create_new: bool,
    },
    FsPatch {
        path: String,
        expected_sha256: String,
        replacement: String,
    },
    FsList {
        path: String,
        recursive: bool,
        max_entries: u64,
    },
    FsMkdir {
        path: String,
        recursive: bool,
    },
    FsMove {
        from: String,
        to: String,
    },
    FsCopy {
        from: String,
        to: String,
    },
    FsDelete {
        path: String,
        recursive: bool,
    },
    ProcessExec {
        options: ProcessOptions,
    },
    ProcessSpawn {
        options: ProcessOptions,
    },
    ProcessStatus {
        process_id: ManagedProcessId,
    },
    ProcessStdin {
        process_id: ManagedProcessId,
        data: String,
    },
    ProcessKill {
        process_id: ManagedProcessId,
    },
    ShellExec {
        shell: String,
        command: String,
        cwd: String,
    },
    HttpRequest {
        method: String,
        url: String,
        #[serde(default)]
        headers: Vec<RequestHeader>,
        body: Option<String>,
        max_response_bytes: u64,
    },
    HttpDownload {
        url: String,
        destination: String,
        #[serde(default)]
        headers: Vec<RequestHeader>,
        max_bytes: u64,
        expected_sha256: Option<String>,
    },
    BrowserRender {
        url: String,
        viewport_width: u32,
        viewport_height: u32,
    },
    SystemInfo,
    SystemEnv {
        names: Vec<String>,
    },
    SystemProcesses,
    GitStatus {
        repository: String,
    },
    GitDiff {
        repository: String,
        staged: bool,
    },
    GitCommit {
        repository: String,
        message: String,
        paths: Vec<String>,
    },
    GitCheckout {
        repository: String,
        reference: String,
        create: bool,
    },
    GitWorktreeCreate {
        repository: String,
        worktree: String,
        branch: String,
        base_reference: String,
    },
    GitWorktreeRemove {
        repository: String,
        worktree: String,
        force: bool,
    },
    GitApplyPatch {
        repository: String,
        patch: String,
        three_way: bool,
        check_only: bool,
    },
    MemorySearch {
        query: String,
        limit: u64,
    },
    MemoryRead {
        key: String,
    },
    MemoryWrite {
        key: String,
        value: Value,
    },
    McpCall {
        connection_id: String,
        tool_name: String,
        arguments: Value,
        read_only: bool,
        destructive: bool,
        open_world: bool,
    },
}

impl ToolRequestKind {
    #[must_use]
    pub fn stable_name(&self) -> String {
        match self {
            Self::FsRead { .. } => "fs.read".into(),
            Self::FsVerify { .. } => "fs.verify".into(),
            Self::FsWrite { .. } => "fs.write".into(),
            Self::FsPatch { .. } => "fs.patch".into(),
            Self::FsList { .. } => "fs.list".into(),
            Self::FsMkdir { .. } => "fs.mkdir".into(),
            Self::FsMove { .. } => "fs.move".into(),
            Self::FsCopy { .. } => "fs.copy".into(),
            Self::FsDelete { .. } => "fs.delete".into(),
            Self::ProcessExec { .. } => "process.exec".into(),
            Self::ProcessSpawn { .. } => "process.spawn".into(),
            Self::ProcessStatus { .. } => "process.status".into(),
            Self::ProcessStdin { .. } => "process.stdin".into(),
            Self::ProcessKill { .. } => "process.kill".into(),
            Self::ShellExec { .. } => "shell.exec".into(),
            Self::HttpRequest { .. } => "http.request".into(),
            Self::HttpDownload { .. } => "http.download".into(),
            Self::BrowserRender { .. } => "browser.render".into(),
            Self::SystemInfo => "system.info".into(),
            Self::SystemEnv { .. } => "system.env".into(),
            Self::SystemProcesses => "system.processes".into(),
            Self::GitStatus { .. } => "git.status".into(),
            Self::GitDiff { .. } => "git.diff".into(),
            Self::GitCommit { .. } => "git.commit".into(),
            Self::GitCheckout { .. } => "git.checkout".into(),
            Self::GitWorktreeCreate { .. } => "git.worktree_create".into(),
            Self::GitWorktreeRemove { .. } => "git.worktree_remove".into(),
            Self::GitApplyPatch { .. } => "git.apply_patch".into(),
            Self::MemorySearch { .. } => "memory.search".into(),
            Self::MemoryRead { .. } => "memory.read".into(),
            Self::MemoryWrite { .. } => "memory.write".into(),
            Self::McpCall {
                connection_id,
                tool_name,
                ..
            } => format!("mcp::{connection_id}::{tool_name}"),
        }
    }

    #[must_use]
    pub const fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::FsDelete { .. }
                | Self::ProcessKill { .. }
                | Self::GitCheckout { .. }
                | Self::GitWorktreeRemove { .. }
                | Self::McpCall {
                    destructive: true,
                    ..
                }
        )
    }

    #[must_use]
    pub const fn mutates_host(&self) -> bool {
        matches!(
            self,
            Self::FsWrite { .. }
                | Self::FsPatch { .. }
                | Self::FsMkdir { .. }
                | Self::FsMove { .. }
                | Self::FsCopy { .. }
                | Self::FsDelete { .. }
                | Self::ProcessExec { .. }
                | Self::ProcessSpawn { .. }
                | Self::ProcessStdin { .. }
                | Self::ProcessKill { .. }
                | Self::ShellExec { .. }
                | Self::HttpDownload { .. }
                | Self::GitCommit { .. }
                | Self::GitCheckout { .. }
                | Self::GitWorktreeCreate { .. }
                | Self::GitWorktreeRemove { .. }
                | Self::GitApplyPatch {
                    check_only: false,
                    ..
                }
                | Self::MemoryWrite { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRequest {
    pub schema_version: u16,
    pub id: ToolRequestId,
    pub task_id: TaskId,
    pub namespace: ToolNamespace,
    pub summary: String,
    pub timeout_seconds: u64,
    pub kind: ToolRequestKind,
    pub created_at: DateTime<Utc>,
}

impl ToolRequest {
    #[must_use]
    pub fn new(task_id: TaskId, summary: impl Into<String>, kind: ToolRequestKind) -> Self {
        Self {
            schema_version: TOOL_SCHEMA_VERSION,
            id: ToolRequestId::new(),
            task_id,
            namespace: ToolNamespace::Host,
            summary: summary.into(),
            timeout_seconds: 60,
            kind,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub schema_version: u16,
    pub id: ApprovalId,
    pub tool_request_id: ToolRequestId,
    pub task_id: TaskId,
    pub action: String,
    pub resource: String,
    pub reason: String,
    pub state: ApprovalState,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalResolution {
    pub approval_id: ApprovalId,
    pub state: ApprovalState,
    pub resolved_by: String,
    pub reason: Option<String>,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    Active,
    Completed,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLease {
    pub schema_version: u16,
    pub id: LeaseId,
    pub execution_id: ToolExecutionId,
    pub tool_request_id: ToolRequestId,
    pub state: LeaseState,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub id: ArtifactId,
    pub sha256: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub method: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutcomeStatus {
    Succeeded,
    Failed,
    Denied,
    ApprovalRequired,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub schema_version: u16,
    pub execution_id: ToolExecutionId,
    pub request_id: ToolRequestId,
    pub status: ToolOutcomeStatus,
    pub summary: String,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactReference>,
    #[serde(default)]
    pub verification: Vec<VerificationEvidence>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolAuditRecord {
    pub schema_version: u16,
    pub execution_id: ToolExecutionId,
    pub request_id: ToolRequestId,
    pub task_id: TaskId,
    pub tool: String,
    pub resource: String,
    pub request_fingerprint: String,
    pub phase: ToolAuditPhase,
    pub decision: String,
    pub result: Option<ToolOutcomeStatus>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactReference>,
    #[serde(default)]
    pub verification: Vec<VerificationEvidence>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAuditPhase {
    Decision,
    Completion,
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("tool request is invalid: {0}")]
    Invalid(String),
    #[error("tool request was denied: {0}")]
    Denied(String),
    #[error("tool request requires approval {approval_id}: {reason}")]
    ApprovalRequired {
        approval_id: ApprovalId,
        reason: String,
    },
    #[error("tool execution timed out")]
    TimedOut,
    #[error("tool execution failed: {0}")]
    Execution(String),
    #[error("tool is unavailable: {0}")]
    Unavailable(String),
    #[error("tool lease is not active: {0}")]
    Lease(String),
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_primitive_has_a_stable_external_name() {
        let request = ToolRequestKind::FsRead {
            path: "project/README.md".into(),
            max_bytes: 1_024,
        };
        assert_eq!(request.stable_name(), "fs.read");
        assert!(!request.mutates_host());
        assert!(
            ToolRequestKind::FsDelete {
                path: "project/tmp".into(),
                recursive: true,
            }
            .is_destructive()
        );
    }

    #[test]
    fn request_schema_round_trips_without_losing_tool_identity() {
        let request = ToolRequest::new(
            TaskId::new(),
            "read project file",
            ToolRequestKind::FsRead {
                path: "README.md".into(),
                max_bytes: 10_000,
            },
        );
        let encoded = serde_json::to_string(&request).expect("serialize");
        let decoded: ToolRequest = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, request);
        assert_eq!(decoded.schema_version, TOOL_SCHEMA_VERSION);
    }
}
