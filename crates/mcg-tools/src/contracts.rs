use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mcg_domain::{
    ApprovalId, ArtifactId, ComputerActionBatchId, LeaseId, ManagedProcessId, PlatformSessionId,
    SiteDeploymentId, SiteId, TaskId, ToolExecutionId, ToolRequestId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::computer_use::ComputerUseAction;

pub const TOOL_SCHEMA_VERSION: u16 = 1;
pub const RESEARCH_EVIDENCE_SCHEMA_VERSION: u16 = 1;
pub const BROWSER_WORKFLOW_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserWorkflowDocument {
    pub schema_version: u16,
    pub name: String,
    pub start_url: String,
    pub steps: Vec<BrowserWorkflowStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserWorkflowStep {
    AssertElement { selector: String },
    Click { selector: String },
    Type { selector: String, value: String },
    Press { selector: String, key: String },
    AssertText { selector: String, contains: String },
    AssertUrl { url: String },
}

impl BrowserWorkflowDocument {
    /// Validates the bounded, same-origin, non-script workflow language.
    ///
    /// # Errors
    ///
    /// Returns a stable explanation for malformed or unsafe workflow input.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BROWSER_WORKFLOW_SCHEMA_VERSION
            || self.name.trim().is_empty()
            || self.name.chars().count() > 128
            || self.steps.is_empty()
            || self.steps.len() > 64
        {
            return Err(
                "browser workflow requires schema version 1, a bounded name, and 1 to 64 steps"
                    .into(),
            );
        }
        let start = workflow_url(&self.start_url)?;
        let origin = start.origin().ascii_serialization();
        for step in &self.steps {
            match step {
                BrowserWorkflowStep::AssertElement { selector }
                | BrowserWorkflowStep::Click { selector }
                | BrowserWorkflowStep::Type { selector, .. }
                | BrowserWorkflowStep::Press { selector, .. }
                | BrowserWorkflowStep::AssertText { selector, .. } => {
                    validate_workflow_selector(selector)?;
                }
                BrowserWorkflowStep::AssertUrl { url } => {
                    if workflow_url(url)?.origin().ascii_serialization() != origin {
                        return Err(
                            "browser workflow URL assertions must remain on the start origin"
                                .into(),
                        );
                    }
                }
            }
            match step {
                BrowserWorkflowStep::Type { value, .. }
                    if value.chars().count() > 4_096 || value.contains('\0') =>
                {
                    return Err(
                        "browser workflow typed values must be at most 4096 characters".into(),
                    );
                }
                BrowserWorkflowStep::Press { key, .. }
                    if !matches!(
                        key.as_str(),
                        "Enter"
                            | "Tab"
                            | "Escape"
                            | "Space"
                            | "ArrowUp"
                            | "ArrowDown"
                            | "ArrowLeft"
                            | "ArrowRight"
                    ) =>
                {
                    return Err("browser workflow key is not in the safe key allowlist".into());
                }
                BrowserWorkflowStep::AssertText { contains, .. }
                    if contains.trim().is_empty() || contains.chars().count() > 1_000 =>
                {
                    return Err(
                        "browser workflow text assertions must contain 1 to 1000 characters".into(),
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn workflow_url(value: &str) -> Result<Url, String> {
    let url =
        Url::parse(value).map_err(|error| format!("invalid browser workflow URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("browser workflow URLs must use credential-free HTTP(S)".into());
    }
    Ok(url)
}

fn validate_workflow_selector(selector: &str) -> Result<(), String> {
    if selector.trim().is_empty()
        || selector.chars().count() > 512
        || selector.chars().any(char::is_control)
    {
        return Err("browser selector must contain 1 to 512 printable characters".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchEvidenceDocument {
    pub schema_version: u16,
    pub scope: String,
    pub claims: Vec<ResearchClaimEvidence>,
    #[serde(default)]
    pub conflicts: Vec<ResearchConflictEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchClaimEvidence {
    pub id: String,
    pub statement: String,
    pub citations: Vec<ResearchCitationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchCitationEvidence {
    pub url: String,
    pub quote: String,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchConflictEvidence {
    pub claim_ids: Vec<String>,
    pub summary: String,
}

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
    TcpConnect {
        endpoint: String,
    },
    BrowserRender {
        url: String,
        viewport_width: u32,
        viewport_height: u32,
    },
    BrowserElementExists {
        url: String,
        selector: String,
        viewport_width: u32,
        viewport_height: u32,
    },
    BrowserAccessibility {
        url: String,
        viewport_width: u32,
        viewport_height: u32,
    },
    BrowserWorkflow {
        workflow_path: String,
        expected_sha256: String,
        workflow_artifact: ArtifactReference,
        workflow_json: String,
        document: BrowserWorkflowDocument,
        viewport_width: u32,
        viewport_height: u32,
    },
    BrowserScreenshotMatches {
        url: String,
        baseline_path: String,
        expected_baseline_sha256: String,
        viewport_width: u32,
        viewport_height: u32,
        max_changed_basis_points: u16,
        per_channel_threshold: u8,
    },
    ResearchVerify {
        evidence_path: String,
        evidence_artifact: ArtifactReference,
        evidence_json: String,
        document: ResearchEvidenceDocument,
        required_scope: String,
        minimum_sources: u16,
        max_age_days: Option<u32>,
        max_response_bytes: u64,
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
    ComputerActions {
        batch_id: ComputerActionBatchId,
        session_id: PlatformSessionId,
        expected_frame_sha256: String,
        actions: Vec<ComputerUseAction>,
    },
    SiteDeploy {
        deployment_id: SiteDeploymentId,
        site_id: SiteId,
        site_revision: u32,
        task_id: Option<TaskId>,
        options: ProcessOptions,
    },
    SshOperation {
        operation_id: mcg_domain::SshOperationId,
        host_id: mcg_domain::SshHostId,
        host_revision: u32,
        hostname: String,
        port: u16,
        username: String,
        known_hosts_entry: String,
        host_key_sha256: String,
        kind: mcg_domain::SshOperationKind,
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
            Self::TcpConnect { .. } => "tcp.connect".into(),
            Self::BrowserRender { .. } => "browser.render".into(),
            Self::BrowserElementExists { .. } => "browser.element_exists".into(),
            Self::BrowserAccessibility { .. } => "browser.accessibility".into(),
            Self::BrowserWorkflow { .. } => "browser.workflow".into(),
            Self::BrowserScreenshotMatches { .. } => "browser.screenshot_matches".into(),
            Self::ResearchVerify { .. } => "research.verify".into(),
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
            Self::ComputerActions { .. } => "computer.actions".into(),
            Self::SiteDeploy { .. } => "sites.deploy".into(),
            Self::SshOperation { kind, .. } => match kind {
                mcg_domain::SshOperationKind::Inspect => "ssh.inspect".into(),
                mcg_domain::SshOperationKind::Execute { .. } => "ssh.execute".into(),
                mcg_domain::SshOperationKind::Upload { .. } => "ssh.upload".into(),
                mcg_domain::SshOperationKind::Download { .. } => "ssh.download".into(),
            },
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
                | Self::ComputerActions { .. }
                | Self::SiteDeploy { .. }
                | Self::SshOperation {
                    kind: mcg_domain::SshOperationKind::Execute { .. }
                        | mcg_domain::SshOperationKind::Upload { .. },
                    ..
                }
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

    #[test]
    fn browser_workflow_language_is_bounded_and_same_origin() {
        let mut workflow = BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "checkout".into(),
            start_url: "https://example.test/start".into(),
            steps: vec![
                BrowserWorkflowStep::Type {
                    selector: "#name".into(),
                    value: "Ada".into(),
                },
                BrowserWorkflowStep::Press {
                    selector: "#name".into(),
                    key: "Enter".into(),
                },
                BrowserWorkflowStep::AssertUrl {
                    url: "https://example.test/done".into(),
                },
            ],
        };
        workflow.validate().expect("valid workflow");
        workflow.steps[1] = BrowserWorkflowStep::Press {
            selector: "#name".into(),
            key: "Control+Alt+Delete".into(),
        };
        assert!(
            workflow
                .validate()
                .expect_err("unsafe key")
                .contains("allowlist")
        );
        workflow.steps[1] = BrowserWorkflowStep::AssertUrl {
            url: "https://other.example.test/done".into(),
        };
        assert!(
            workflow
                .validate()
                .expect_err("cross-origin assertion")
                .contains("start origin")
        );
    }
}
