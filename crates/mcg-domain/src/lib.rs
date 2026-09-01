//! Stable, versioned domain types shared by every MCG interface.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const TASK_SCHEMA_VERSION: u16 = 1;

macro_rules! domain_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

domain_id!(ProjectId);
domain_id!(ThreadId);
domain_id!(ConversationId);
domain_id!(ConversationMessageId);
domain_id!(ConversationTurnId);
domain_id!(ConversationAgentSessionId);
domain_id!(AgentDelegationId);
domain_id!(AgentMessageId);
domain_id!(BlackboardEntryId);
domain_id!(AgentContextCheckpointId);
domain_id!(CoordinationBatchId);
domain_id!(AgentWakeId);
domain_id!(ConversationPreviewId);
domain_id!(ManagedWorktreeId);
domain_id!(IntegrationJournalId);
domain_id!(CloudDiffApplyId);
domain_id!(AgentCredentialAuditId);
domain_id!(TaskId);
domain_id!(RunId);
domain_id!(ToolRequestId);
domain_id!(ToolExecutionId);
domain_id!(ApprovalId);
domain_id!(LeaseId);
domain_id!(ArtifactId);
domain_id!(ManagedProcessId);
domain_id!(ProjectSnapshotId);
domain_id!(ReviewedTopologyPlanId);
domain_id!(ReviewedTopologyStepId);
domain_id!(MemoryClaimId);
domain_id!(MemoryTransferId);
domain_id!(OrganizationId);
domain_id!(OrganizationImportId);
domain_id!(OrganizationShareReviewId);
domain_id!(OrganizationRevisionId);
domain_id!(RoleId);
domain_id!(AgentInstanceId);
domain_id!(AgentLeaseId);
domain_id!(WorkItemId);
domain_id!(EvaluationId);
domain_id!(UsageRecordId);
domain_id!(ProxyConversationId);
domain_id!(ProxyCallId);
domain_id!(ScheduleId);
domain_id!(ScheduleOccurrenceId);
domain_id!(WakeRequestId);
domain_id!(SchedulerNotificationId);
domain_id!(PlatformCapabilitySnapshotId);
domain_id!(PlatformSessionId);
domain_id!(PlatformSessionEventId);
domain_id!(ComputerActionBatchId);
domain_id!(SiteId);
domain_id!(SiteRevisionId);
domain_id!(SiteDeploymentId);
domain_id!(SiteHealthCheckId);
domain_id!(SshHostId);
domain_id!(SshHostRevisionId);
domain_id!(SshOperationId);
domain_id!(SshWorkflowId);
domain_id!(SshWorkflowStepId);
domain_id!(SupervisionObservationId);
domain_id!(ExperimentId);
domain_id!(ExperimentBenchmarkId);
domain_id!(ExperimentDecisionId);
domain_id!(GovernorProposalId);
domain_id!(PluginRevisionId);
domain_id!(PluginInvocationId);

impl ToolRequestId {
    /// Creates a stable request identity from a caller-provided 128-bit digest.
    /// The UUID variant/version bits are normalized so serialized identities
    /// remain standard while retries can address the same approval record.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl TaskId {
    /// Creates a stable synthetic task identity from a caller-provided digest.
    ///
    /// This is reserved for durable authority subjects that are task-shaped in
    /// the current storage schema but must not collide with caller-owned UUIDs.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl SiteId {
    /// Creates a stable imported-Site identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl SshOperationId {
    /// Creates a stable per-step operation identity for durable workflow retries.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl SupervisionObservationId {
    /// Creates a stable automatic-observation identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl OrganizationRevisionId {
    /// Creates a stable automatic candidate-revision identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl OrganizationId {
    /// Creates a stable organization identity for an idempotent portable import.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl RoleId {
    /// Creates a stable role identity within an idempotent portable import.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl ExperimentId {
    /// Creates a stable automatic experiment identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl RunId {
    /// Creates a stable evaluation-only shadow-run identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl EvaluationId {
    /// Creates a stable automatic shadow-evaluation identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl ExperimentBenchmarkId {
    /// Creates a stable automatic paired-benchmark identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

impl ExperimentDecisionId {
    /// Creates a stable automatic experiment-decision identity from a 128-bit digest.
    #[must_use]
    pub fn from_stable_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(Uuid::from_bytes(bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Active,
    WaitingForApproval,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl TaskState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Active | Self::Cancelled)
                | (
                    Self::Active,
                    Self::WaitingForApproval
                        | Self::Blocked
                        | Self::Completed
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::WaitingForApproval,
                    Self::Active | Self::Blocked | Self::Failed | Self::Cancelled
                )
                | (Self::Blocked, Self::Active | Self::Failed | Self::Cancelled)
        )
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        formatter.write_str(value.as_str().ok_or(std::fmt::Error)?)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid task transition from {from} to {to}")]
pub struct InvalidTaskTransition {
    pub from: TaskState,
    pub to: TaskState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskBudget {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub max_duration_seconds: Option<u64>,
}

pub const PLATFORM_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformCapabilityKind {
    BrowserAutomation,
    ComputerUse,
    NativeDesktop,
    SshClient,
    Sites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformCapabilityAvailability {
    Available,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapability {
    pub kind: PlatformCapabilityKind,
    pub availability: PlatformCapabilityAvailability,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilitySnapshot {
    pub schema_version: u16,
    pub id: PlatformCapabilitySnapshotId,
    pub os: String,
    pub architecture: String,
    pub capabilities: Vec<PlatformCapability>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformSessionKind {
    Browser,
    ComputerUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformSessionState {
    Requested,
    Active,
    HandedOff,
    Interrupted,
    Closed,
    Failed,
}

impl PlatformSessionState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Requested, Self::Active | Self::Closed | Self::Failed)
                | (
                    Self::Active,
                    Self::HandedOff | Self::Interrupted | Self::Closed | Self::Failed
                )
                | (
                    Self::HandedOff,
                    Self::Active | Self::Interrupted | Self::Closed | Self::Failed
                )
                | (Self::Interrupted, Self::Closed)
        )
    }

    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Requested | Self::Active | Self::HandedOff)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformSessionOwner {
    Mcg,
    LocalOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseCapability {
    ScreenRead,
    Mouse,
    Keyboard,
    Clipboard,
    WindowList,
    ProcessInspect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseTarget {
    /// Platform display identity. `None` resolves to the primary display at attach time.
    #[serde(default)]
    pub display_id: Option<u32>,
    /// Display-relative bounds. `None` binds the complete selected display.
    #[serde(default)]
    pub region: Option<ComputerUseRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerActionBatchState {
    Executing,
    Succeeded,
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerActionBatch {
    pub schema_version: u16,
    pub id: ComputerActionBatchId,
    pub session_id: PlatformSessionId,
    pub request_hash: String,
    pub state: ComputerActionBatchState,
    pub action_count: u16,
    pub result_frame_sha256: Option<String>,
    pub detail: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformSession {
    pub schema_version: u16,
    pub id: PlatformSessionId,
    pub definition_hash: String,
    pub kind: PlatformSessionKind,
    pub state: PlatformSessionState,
    pub owner: PlatformSessionOwner,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub display_name: String,
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub computer_capabilities: Vec<ComputerUseCapability>,
    #[serde(default)]
    pub computer_target: Option<ComputerUseTarget>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub recovery_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformSessionEvent {
    pub schema_version: u16,
    pub id: PlatformSessionEventId,
    pub session_id: PlatformSessionId,
    pub from_state: Option<PlatformSessionState>,
    pub to_state: PlatformSessionState,
    pub owner: PlatformSessionOwner,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
}

pub const SCHEDULE_SCHEMA_VERSION: u16 = 1;

pub const SITE_SCHEMA_VERSION: u16 = 1;
pub const SSH_SCHEMA_VERSION: u16 = 1;
pub const SSH_WORKFLOW_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteEnvironment {
    Development,
    Preview,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteState {
    Active,
    MonitoringPaused,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteDeploymentCommand {
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Project-relative working directory. An empty string means project root.
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    pub schema_version: u16,
    pub id: SiteId,
    pub definition_hash: String,
    pub revision: u32,
    pub project_id: ProjectId,
    pub display_name: String,
    pub environment: SiteEnvironment,
    pub public_url: String,
    pub health_url: String,
    pub hosting_provider: String,
    pub deployment_command: Option<SiteDeploymentCommand>,
    pub credential_reference: Option<String>,
    pub state: SiteState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteRevision {
    pub schema_version: u16,
    pub id: SiteRevisionId,
    pub site_id: SiteId,
    pub revision: u32,
    pub definition_hash: String,
    pub site: Site,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteDeploymentState {
    Executing,
    Succeeded,
    Failed,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteDeployment {
    pub schema_version: u16,
    pub id: SiteDeploymentId,
    pub site_id: SiteId,
    pub site_revision: u32,
    pub task_id: Option<TaskId>,
    pub request_hash: String,
    pub state: SiteDeploymentState,
    pub detail: String,
    #[serde(default)]
    pub log_artifact_sha256: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteHealthState {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteHealthCheck {
    pub schema_version: u16,
    pub id: SiteHealthCheckId,
    pub site_id: SiteId,
    pub request_hash: String,
    pub state: SiteHealthState,
    pub status_code: Option<u16>,
    pub latency_milliseconds: u64,
    pub detail: String,
    pub response_artifact_sha256: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteMonitor {
    pub schema_version: u16,
    pub site_id: SiteId,
    pub schedule_id: ScheduleId,
    pub interval_seconds: u64,
    pub expected_status: u16,
    pub last_state: Option<SiteHealthState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshHostPlatform {
    Posix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthentication {
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshHostState {
    Active,
    Disabled,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHost {
    pub schema_version: u16,
    pub id: SshHostId,
    pub definition_hash: String,
    pub revision: u32,
    pub display_name: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub platform: SshHostPlatform,
    pub authentication: SshAuthentication,
    pub known_hosts_entry: String,
    pub host_key_sha256: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub state: SshHostState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHostRevision {
    pub schema_version: u16,
    pub id: SshHostRevisionId,
    pub host_id: SshHostId,
    pub revision: u32,
    pub definition_hash: String,
    pub host: SshHost,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SshOperationKind {
    Inspect,
    Execute {
        executable: String,
        #[serde(default)]
        arguments: Vec<String>,
        working_directory: Option<String>,
    },
    Upload {
        local_path: String,
        remote_path: String,
        overwrite: bool,
        expected_sha256: String,
    },
    Download {
        remote_path: String,
        local_path: String,
        overwrite: bool,
        expected_sha256: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshOperationState {
    Executing,
    Succeeded,
    Failed,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshOperation {
    pub schema_version: u16,
    pub id: SshOperationId,
    pub host_id: SshHostId,
    pub host_revision: u32,
    pub task_id: Option<TaskId>,
    pub request_hash: String,
    pub kind: SshOperationKind,
    pub state: SshOperationState,
    pub detail: String,
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub artifact_sha256: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshWorkflowState {
    Planned,
    Running,
    Waiting,
    Succeeded,
    Failed,
    Uncertain,
    Cancelled,
}

impl SshWorkflowState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Uncertain | Self::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshWorkflowStepState {
    Pending,
    Succeeded,
    Failed,
    Uncertain,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshWorkflowStep {
    pub id: SshWorkflowStepId,
    pub name: String,
    pub host_id: SshHostId,
    pub host_revision: u32,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub routed_automatically: bool,
    #[serde(default)]
    pub dependencies: Vec<SshWorkflowStepId>,
    pub operation_id: SshOperationId,
    pub operation: SshOperationKind,
    pub timeout_seconds: u64,
    pub state: SshWorkflowStepState,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshWorkflow {
    pub schema_version: u16,
    pub id: SshWorkflowId,
    pub definition_hash: String,
    pub display_name: String,
    pub task_id: Option<TaskId>,
    pub state: SshWorkflowState,
    pub steps: Vec<SshWorkflowStep>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleState {
    Enabled,
    Paused,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ScheduleTrigger {
    Once {
        at: DateTime<Utc>,
    },
    Interval {
        seconds: u64,
    },
    Cron {
        expression: String,
    },
    FileWatch {
        relative_path: String,
        poll_seconds: u64,
    },
    Event {
        event_kind: String,
    },
    Heartbeat {
        idle_seconds: u64,
        busy_seconds: u64,
    },
}

impl ScheduleTrigger {
    /// Computes the next UTC time trigger. Event and file-watch triggers wait
    /// for an external durable observation and therefore return `None`.
    ///
    /// # Errors
    /// Returns an error for malformed cron expressions or invalid intervals.
    pub fn next_after(
        &self,
        after: DateTime<Utc>,
        system_busy: bool,
    ) -> Result<Option<DateTime<Utc>>, String> {
        match self {
            Self::Once { at } => Ok((*at > after).then_some(*at)),
            Self::Interval { seconds } => duration_after(after, *seconds).map(Some),
            Self::Cron { expression } => next_cron_time(expression, after).map(Some),
            Self::FileWatch { .. } | Self::Event { .. } => Ok(None),
            Self::Heartbeat {
                idle_seconds,
                busy_seconds,
            } => duration_after(
                after,
                if system_busy {
                    *busy_seconds
                } else {
                    *idle_seconds
                },
            )
            .map(Some),
        }
    }
}

fn duration_after(after: DateTime<Utc>, seconds: u64) -> Result<DateTime<Utc>, String> {
    if seconds == 0 {
        return Err("schedule duration must be positive".into());
    }
    let seconds = i64::try_from(seconds).map_err(|_| "schedule duration is too large")?;
    after
        .checked_add_signed(Duration::seconds(seconds))
        .ok_or_else(|| "schedule duration exceeds the supported calendar".into())
}

#[derive(Debug)]
struct CronSpec {
    minute: Vec<u32>,
    hour: Vec<u32>,
    day: Vec<u32>,
    month: Vec<u32>,
    weekday: Vec<u32>,
    day_wildcard: bool,
    weekday_wildcard: bool,
}

fn next_cron_time(expression: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err("cron requires five UTC fields: minute hour day month weekday".into());
    }
    let spec = CronSpec {
        minute: parse_cron_field(fields[0], 0, 59, false)?,
        hour: parse_cron_field(fields[1], 0, 23, false)?,
        day: parse_cron_field(fields[2], 1, 31, false)?,
        month: parse_cron_field(fields[3], 1, 12, false)?,
        weekday: parse_cron_field(fields[4], 0, 7, true)?,
        day_wildcard: fields[2] == "*",
        weekday_wildcard: fields[4] == "*",
    };
    let mut candidate = after
        .checked_add_signed(Duration::minutes(1))
        .ok_or_else(|| "cron start time exceeds the supported calendar".to_owned())?
        .with_second(0)
        .and_then(|value| value.with_nanosecond(0))
        .ok_or_else(|| "cron start time is invalid".to_owned())?;
    let limit = after
        .checked_add_signed(Duration::days(366 * 5))
        .ok_or_else(|| "cron horizon exceeds the supported calendar".to_owned())?;
    while candidate <= limit {
        let day_match = spec.day.contains(&candidate.day());
        let weekday = candidate.weekday().num_days_from_sunday();
        let weekday_match = spec.weekday.contains(&weekday);
        let calendar_match = if spec.day_wildcard || spec.weekday_wildcard {
            day_match && weekday_match
        } else {
            day_match || weekday_match
        };
        if spec.minute.contains(&candidate.minute())
            && spec.hour.contains(&candidate.hour())
            && spec.month.contains(&candidate.month())
            && calendar_match
        {
            return Ok(candidate);
        }
        candidate = candidate
            .checked_add_signed(Duration::minutes(1))
            .ok_or_else(|| "cron search exceeds the supported calendar".to_owned())?;
    }
    Err("cron has no occurrence within five years".into())
}

fn parse_cron_field(
    field: &str,
    minimum: u32,
    maximum: u32,
    normalize_sunday: bool,
) -> Result<Vec<u32>, String> {
    let mut values = Vec::new();
    for part in field.split(',') {
        let (base, step, stepped) = part
            .split_once('/')
            .map_or((part, 1, false), |(base, step)| {
                (base, step.parse::<u32>().unwrap_or(0), true)
            });
        if step == 0 {
            return Err("cron step must be a positive integer".into());
        }
        let (start, end) = if base == "*" {
            (minimum, maximum)
        } else if let Some((start, end)) = base.split_once('-') {
            (
                parse_cron_number(start, minimum, maximum)?,
                parse_cron_number(end, minimum, maximum)?,
            )
        } else if stepped {
            let value = parse_cron_number(base, minimum, maximum)?;
            (value, maximum)
        } else {
            let value = parse_cron_number(base, minimum, maximum)?;
            (value, value)
        };
        if start > end {
            return Err("cron ranges must be ascending".into());
        }
        let step = usize::try_from(step).map_err(|_| "cron step is too large")?;
        for value in (start..=end).step_by(step) {
            let value = if normalize_sunday && value == 7 {
                0
            } else {
                value
            };
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    if values.is_empty() {
        return Err("cron fields cannot be empty".into());
    }
    values.sort_unstable();
    Ok(values)
}

fn parse_cron_number(value: &str, minimum: u32, maximum: u32) -> Result<u32, String> {
    let value = value
        .parse::<u32>()
        .map_err(|_| "cron fields must contain numbers, ranges, lists, or steps")?;
    if !(minimum..=maximum).contains(&value) {
        return Err("cron field value is outside its allowed range".into());
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleDefinition {
    pub schema_version: u16,
    pub id: ScheduleId,
    #[serde(default)]
    pub definition_hash: String,
    pub name: String,
    pub project_id: ProjectId,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_conditions: Vec<String>,
    pub risk: RiskLevel,
    pub budget: TaskBudget,
    pub trigger: ScheduleTrigger,
    pub state: ScheduleState,
    pub next_run_at: Option<DateTime<Utc>>,
    pub target_path: Option<String>,
    pub context_token_budget: u32,
    pub requested_tools: Vec<String>,
    pub estimated_output_tokens: u64,
    pub max_model_calls: u16,
    #[serde(default)]
    pub watch_fingerprint: Option<String>,
    #[serde(default)]
    pub last_watch_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_event_sequence: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Info,
    Warning,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeRequest {
    pub schema_version: u16,
    pub id: WakeRequestId,
    pub schedule_id: ScheduleId,
    pub severity: NotificationSeverity,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerNotification {
    pub schema_version: u16,
    pub id: SchedulerNotificationId,
    pub schedule_id: ScheduleId,
    pub occurrence_id: Option<ScheduleOccurrenceId>,
    pub severity: NotificationSeverity,
    pub title: String,
    pub detail: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleOccurrenceState {
    Running,
    Recovering,
    WaitingForApproval,
    Completed,
    Failed,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleOccurrence {
    pub schema_version: u16,
    pub id: ScheduleOccurrenceId,
    pub schedule_id: ScheduleId,
    pub task_id: Option<TaskId>,
    pub due_at: DateTime<Utc>,
    pub state: ScheduleOccurrenceState,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub attempt: u32,
    pub detail: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub const PLUGIN_CATALOG_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginInstallSource {
    LocalImport,
    Registry { registry: String, package: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSignatureState {
    Unsigned,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSignature {
    pub state: PluginSignatureState,
    pub scheme: Option<String>,
    pub signer: Option<String>,
    pub signature_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRevision {
    pub schema_version: u16,
    pub id: PluginRevisionId,
    pub manifest: mcg_plugin_protocol::PluginManifest,
    pub module_sha256: String,
    pub module_size_bytes: u64,
    pub source: PluginInstallSource,
    pub signature: PluginSignature,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginCapabilityScope {
    TaskProject,
    NetworkOrigin {
        origin: String,
    },
    Executable {
        executable: String,
    },
    Credential {
        provider: String,
        key: String,
    },
    McpTool {
        connection_id: String,
        tool_name: String,
    },
    BrowserOrigin {
        origin: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapabilityGrant {
    pub schema_version: u16,
    pub capability: mcg_plugin_protocol::PluginCapability,
    pub scopes: Vec<PluginCapabilityScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstallation {
    pub schema_version: u16,
    pub plugin_id: String,
    pub active_revision_id: PluginRevisionId,
    pub enabled: bool,
    #[serde(default)]
    pub capability_grants: Vec<PluginCapabilityGrant>,
    pub revision: u64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInvocationState {
    Running,
    Succeeded,
    Failed,
    Uncertain,
}

impl PluginInvocationState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Uncertain)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInvocationRecord {
    pub schema_version: u16,
    pub id: PluginInvocationId,
    pub plugin_id: String,
    pub plugin_revision_id: PluginRevisionId,
    pub task_id: Option<TaskId>,
    pub input_sha256: String,
    pub state: PluginInvocationState,
    pub result_sha256: Option<String>,
    pub error_code: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub const CONVERSATION_SCHEMA_VERSION: u16 = 2;
pub const CONVERSATION_TEAM_SCHEMA_VERSION: u16 = 8;
pub const GLOBAL_AGENT_SESSION_SUMMARY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationAgentKind {
    Chat,
    Coordinator,
    Coder,
    Tester,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationAgentSessionState {
    Starting,
    Ready,
    Working,
    Waiting,
    Failed,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationAgentSession {
    pub schema_version: u16,
    pub id: ConversationAgentSessionId,
    pub conversation_id: ConversationId,
    pub role_id: RoleId,
    pub organization_revision_id: OrganizationRevisionId,
    pub kind: ConversationAgentKind,
    #[serde(default)]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<ConversationAgentSessionId>,
    pub state: ConversationAgentSessionState,
    pub codex_thread_id: Option<String>,
    pub model: Option<String>,
    pub prompt_revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Content-minimized cross-conversation projection for the global agent browser.
///
/// This deliberately excludes prompts, messages, credentials, tool payloads, and private
/// reasoning. Clients use the stable conversation and session identities to request an
/// explicitly scoped view only after the owner selects a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalAgentSessionSummary {
    pub schema_version: u16,
    pub session_id: ConversationAgentSessionId,
    pub conversation_id: ConversationId,
    pub conversation_title: String,
    pub conversation_state: ConversationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    pub role_id: RoleId,
    pub organization_revision_id: OrganizationRevisionId,
    pub kind: ConversationAgentKind,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<ConversationAgentSessionId>,
    pub state: ConversationAgentSessionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDelegationState {
    Requested,
    Preparing,
    Running,
    Waiting,
    Blocked,
    Reported,
    Integrating,
    Completed,
    Failed,
    Cancelled,
}

impl AgentDelegationState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingDelegationBrief {
    pub objective: String,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub success_conditions: Vec<String>,
    #[serde(default)]
    pub context_references: Vec<String>,
}

/// Deterministic, runtime-enforced completion requirements compiled before a Coder starts.
/// Prompt text may explain this contract, but cannot relax it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DelegationCompletionContract {
    #[serde(default)]
    pub objective_sha256: String,
    #[serde(default)]
    pub allowed_scopes: Vec<String>,
    #[serde(default)]
    pub required_checks: Vec<String>,
    #[serde(default)]
    pub required_evidence: Vec<String>,
    #[serde(default)]
    pub require_integration_ready: bool,
    #[serde(default)]
    pub context_token_budget: u64,
    #[serde(default)]
    pub max_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DelegationFailureClass {
    #[default]
    None,
    Precondition,
    PolicyDenied,
    TransientTool,
    AgentOmission,
    Verification,
    Conflict,
    Delivery,
    Cancelled,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DelegationRecoveryDirective {
    #[default]
    None,
    RetrySameThread,
    RetryNewDelegation,
    Replan,
    AwaitOwner,
    RetainForReview,
    DoNotRetry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DelegationPreflightReceipt {
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentContextReceipt {
    #[serde(default)]
    pub source_count: u32,
    #[serde(default)]
    pub approximate_tokens: u64,
    #[serde(default)]
    pub token_budget: u64,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub source_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DelegationOutcomeMetrics {
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub model_turns: u32,
    #[serde(default)]
    pub approximate_tokens: u64,
    #[serde(default)]
    pub tool_calls: u32,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub owner_interventions: u32,
    #[serde(default)]
    pub changed_file_count: u32,
    #[serde(default)]
    pub check_count: u32,
    #[serde(default)]
    pub successful: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DelegationGovernance {
    #[serde(default)]
    pub contract: DelegationCompletionContract,
    #[serde(default)]
    pub preflight: DelegationPreflightReceipt,
    #[serde(default)]
    pub context_receipts: Vec<AgentContextReceipt>,
    #[serde(default)]
    pub outcome: DelegationOutcomeMetrics,
    #[serde(default)]
    pub failure_class: DelegationFailureClass,
    #[serde(default)]
    pub recovery: DelegationRecoveryDirective,
    #[serde(default)]
    pub dependencies: Vec<AgentDelegationId>,
    #[serde(default)]
    pub reserved_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDelegation {
    pub schema_version: u16,
    pub id: AgentDelegationId,
    pub conversation_id: ConversationId,
    pub source_session_id: ConversationAgentSessionId,
    pub coder_session_id: Option<ConversationAgentSessionId>,
    pub project_id: ProjectId,
    pub organization_revision_id: OrganizationRevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordination_batch_id: Option<CoordinationBatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_turn_id: Option<ConversationTurnId>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub prompt_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<ApprovalMode>,
    pub state: AgentDelegationState,
    pub brief: CodingDelegationBrief,
    #[serde(default)]
    pub governance: DelegationGovernance,
    pub task_id: Option<TaskId>,
    pub run_id: Option<RunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_instance_id: Option<AgentInstanceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_lease_id: Option<AgentLeaseId>,
    pub worktree_id: Option<ManagedWorktreeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_of_id: Option<AgentDelegationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub unresolved_issues: Vec<String>,
    #[serde(default)]
    pub integration_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator_decision: Option<CoordinatorReviewDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator_rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator_reviewed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_codex_item_id: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Returns the authoritative leaf attempts in a delegation retry graph.
///
/// Failed or cancelled attempts remain durable evidence, but once another delegation names an
/// attempt through `retry_of_id`, that ancestor is no longer an effective batch outcome. Multiple
/// independent retry branches remain visible as separate leaves and must each settle.
pub fn effective_delegation_attempt_ids(
    attempts: impl IntoIterator<Item = (AgentDelegationId, Option<AgentDelegationId>)>,
) -> BTreeSet<AgentDelegationId> {
    let attempts = attempts.into_iter().collect::<Vec<_>>();
    let superseded = attempts
        .iter()
        .filter_map(|(_, retry_of_id)| *retry_of_id)
        .collect::<BTreeSet<_>>();
    attempts
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !superseded.contains(id))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageKind {
    Joined,
    Left,
    Delegation,
    Update,
    Question,
    Answer,
    Amendment,
    Report,
    Cancellation,
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentRoomKind {
    Frontstage,
    #[default]
    Backstage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub schema_version: u16,
    pub id: AgentMessageId,
    pub conversation_id: ConversationId,
    pub delegation_id: Option<AgentDelegationId>,
    pub from_session_id: ConversationAgentSessionId,
    pub to_session_id: ConversationAgentSessionId,
    pub kind: AgentMessageKind,
    #[serde(default)]
    pub room: AgentRoomKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContextCheckpoint {
    pub schema_version: u16,
    pub id: AgentContextCheckpointId,
    pub conversation_id: ConversationId,
    pub session_id: ConversationAgentSessionId,
    pub room: AgentRoomKind,
    pub revision: u64,
    pub through_message_id: Option<AgentMessageId>,
    #[serde(default)]
    pub cause: AgentContextCheckpointCause,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub source_references: Vec<String>,
    #[serde(default)]
    pub approximate_tokens: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentContextCheckpointCause {
    #[default]
    Legacy,
    DelegationSealed,
    Question,
    Blocker,
    FinalReview,
    SizeThreshold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorReviewDecision {
    Accept,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationBatchState {
    Open,
    Running,
    ReviewPending,
    Reviewing,
    ChatPending,
    Completed,
    Failed,
    RecoveryRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConversationDeliveryTarget {
    CodexSites,
    #[default]
    CustomProject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinationBatch {
    pub schema_version: u16,
    pub id: CoordinationBatchId,
    pub conversation_id: ConversationId,
    pub origin_turn_id: ConversationTurnId,
    pub coordinator_session_id: ConversationAgentSessionId,
    #[serde(default)]
    pub delivery_target: ConversationDeliveryTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_owner_session_id: Option<ConversationAgentSessionId>,
    pub state: CoordinationBatchState,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub delegation_ids: Vec<AgentDelegationId>,
    #[serde(default)]
    pub coordinator_turn_count: u32,
    #[serde(default)]
    pub coordinator_approximate_tokens: u64,
    /// True when the deterministic runtime can safely route a bounded single-Coder request
    /// without spending a Coordinator model turn. The Coordinator remains the authority owner.
    #[serde(default)]
    pub coordinator_bypassed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_rationale: Option<String>,
    #[serde(default)]
    pub owner_intervention_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationPreviewStrategy {
    CodexSites,
    StaticFiles,
    ManifestCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationPreviewState {
    PreparingSite,
    Building,
    Validating,
    PublishingPrivate,
    ApprovalRequired,
    Available,
    Running,
    Stopped,
    Failed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationPreview {
    pub schema_version: u16,
    pub id: ConversationPreviewId,
    pub conversation_id: ConversationId,
    pub coordination_batch_id: CoordinationBatchId,
    pub delegation_id: AgentDelegationId,
    pub project_id: ProjectId,
    pub delivery_target: ConversationDeliveryTarget,
    pub strategy: ConversationPreviewStrategy,
    pub state: ConversationPreviewState,
    pub source_revision: u64,
    pub source_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWakeTarget {
    Coordinator,
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWakeKind {
    InitialRequest,
    Question,
    Blocker,
    FinalReview,
    ChatCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWakeState {
    Pending,
    Claimed,
    Completed,
    Failed,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWake {
    pub schema_version: u16,
    pub id: AgentWakeId,
    pub idempotency_key: String,
    pub conversation_id: ConversationId,
    pub coordination_batch_id: CoordinationBatchId,
    pub delegation_id: Option<AgentDelegationId>,
    pub source_message_id: Option<AgentMessageId>,
    pub target_session_id: ConversationAgentSessionId,
    pub target: AgentWakeTarget,
    pub kind: AgentWakeKind,
    pub state: AgentWakeState,
    #[serde(default)]
    pub attempt_count: u32,
    pub claimed_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub delegation_revisions: Vec<(AgentDelegationId, u64)>,
    pub payload: String,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlackboardEntryKind {
    Goal,
    Context,
    Decision,
    Status,
    Evidence,
    Question,
    Answer,
    Result,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub schema_version: u16,
    pub id: BlackboardEntryId,
    pub conversation_id: ConversationId,
    pub delegation_id: Option<AgentDelegationId>,
    pub author_session_id: ConversationAgentSessionId,
    pub kind: BlackboardEntryKind,
    pub content: String,
    #[serde(default)]
    pub source_references: Vec<String>,
    pub supersedes_id: Option<BlackboardEntryId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedWorktreeState {
    Preparing,
    Ready,
    Dirty,
    Integrating,
    Integrated,
    Conflict,
    Retained,
    Removed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ManagedWorkspaceMode {
    #[default]
    GitWorktree,
    DirectWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BaselineConfidence {
    #[default]
    Verified,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineSourceKind {
    Tracked,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeBaselineEntry {
    pub normalized_path: String,
    pub source_kind: BaselineSourceKind,
    pub size: u64,
    pub mode: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeBaselineExclusion {
    pub normalized_path: String,
    pub reason: String,
    #[serde(default)]
    pub review_blocker: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationJournalState {
    Preparing,
    DryRunPassed,
    Applying,
    Applied,
    RollingBack,
    RolledBack,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationTouchedPath {
    pub normalized_path: String,
    pub live_sha256: Option<String>,
    pub preimage_sha256: Option<String>,
    pub preimage_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationJournal {
    pub schema_version: u16,
    pub id: IntegrationJournalId,
    pub delegation_id: AgentDelegationId,
    pub worktree_id: ManagedWorktreeId,
    pub state: IntegrationJournalState,
    pub live_head: Option<String>,
    #[serde(default)]
    pub touched_paths: Vec<IntegrationTouchedPath>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedWorktree {
    pub schema_version: u16,
    pub id: ManagedWorktreeId,
    pub conversation_id: ConversationId,
    pub delegation_id: AgentDelegationId,
    pub project_id: ProjectId,
    pub root: String,
    pub branch: String,
    pub base_head: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_head: Option<String>,
    pub baseline_sha256: String,
    #[serde(default)]
    pub workspace_mode: ManagedWorkspaceMode,
    #[serde(default)]
    pub baseline_confidence: BaselineConfidence,
    #[serde(default)]
    pub baseline_entries: Vec<WorktreeBaselineEntry>,
    #[serde(default)]
    pub baseline_exclusions: Vec<WorktreeBaselineExclusion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_journal: Option<IntegrationJournal>,
    pub state: ManagedWorktreeState,
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_reason: Option<String>,
    #[serde(default)]
    pub storage_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Durable state for applying the latest diff produced by a Codex cloud task.
///
/// The native Codex client is allowed to mutate only an isolated shadow worktree. `MetaCog` then
/// integrates the resulting bounded patch into the owner workspace under the same preimage and
/// rollback guarantees used by governed coding work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudDiffApplyState {
    Preparing,
    Ready,
    Applying,
    Applied,
    Conflict,
    RolledBack,
    Failed,
    Retained,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudDiffTouchedPath {
    pub normalized_path: String,
    pub live_content_id: Option<String>,
    pub expected_content_id: Option<String>,
    pub preimage_sha256: Option<String>,
    pub preimage_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudDiffApply {
    pub schema_version: u16,
    pub id: CloudDiffApplyId,
    pub project_id: ProjectId,
    pub task_id: String,
    pub state: CloudDiffApplyState,
    pub root: String,
    pub branch: String,
    pub base_head: String,
    pub baseline_head: String,
    pub baseline_sha256: String,
    #[serde(default)]
    pub baseline_entries: Vec<WorktreeBaselineEntry>,
    #[serde(default)]
    pub baseline_exclusions: Vec<WorktreeBaselineExclusion>,
    #[serde(default)]
    pub changed_paths: Vec<String>,
    #[serde(default)]
    pub touched_paths: Vec<CloudDiffTouchedPath>,
    pub patch_sha256: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCredentialAuditKind {
    BootstrapIssued,
    BootstrapExchanged,
    CredentialRenewed,
    CredentialRevoked,
    CredentialExpired,
    ToolAllowed,
    ToolDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCredentialAudit {
    pub schema_version: u16,
    pub id: AgentCredentialAuditId,
    pub conversation_id: ConversationId,
    pub session_id: ConversationAgentSessionId,
    pub delegation_id: Option<AgentDelegationId>,
    pub project_id: ProjectId,
    pub organization_revision_id: OrganizationRevisionId,
    pub role: ConversationAgentKind,
    pub kind: AgentCredentialAuditKind,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    pub nonce_sha256: String,
    pub tool_name: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConversationTeamState {
    #[serde(default)]
    pub sessions: Vec<ConversationAgentSession>,
    #[serde(default)]
    pub delegations: Vec<AgentDelegation>,
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    #[serde(default)]
    pub blackboard: Vec<BlackboardEntry>,
    #[serde(default)]
    pub worktrees: Vec<ManagedWorktree>,
    #[serde(default)]
    pub context_checkpoints: Vec<AgentContextCheckpoint>,
    #[serde(default)]
    pub coordination_batches: Vec<CoordinationBatch>,
    #[serde(default)]
    pub wakes: Vec<AgentWake>,
    #[serde(default)]
    pub previews: Vec<ConversationPreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedContextSourceKind {
    ConversationMessage,
    AgentMessage,
    Blackboard,
    ProjectContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedContextHit {
    pub source_id: String,
    pub conversation_id: ConversationId,
    pub delegation_id: Option<AgentDelegationId>,
    pub source_kind: SharedContextSourceKind,
    pub author_role: String,
    pub created_at: DateTime<Utc>,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationState {
    Active,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTurnState {
    Pending,
    Streaming,
    WaitingForApproval,
    Recovering,
    Indeterminate,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConversationMode {
    #[default]
    Default,
    Plan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationGoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: ConversationGoalStatus,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub is_other: bool,
    pub is_secret: bool,
    pub options: Option<Vec<ConversationInputOption>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInputRequest {
    pub request_id: serde_json::Value,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ConversationInputQuestion>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationActivityKind {
    Working,
    Waiting,
    ReasoningSummary,
    Plan,
    Command,
    FileChange,
    McpCall,
    CollaborationCall,
    WebSearch,
    ImageView,
    Approval,
    Elicitation,
    ContextCompaction,
    Hook,
    Usage,
    Recovery,
    Interruption,
    Failure,
    Completion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationActivityState {
    Running,
    Completed,
    Failed,
    Declined,
}

/// A durable, user-visible projection of Codex work performed during a chat turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationActivity {
    pub schema_version: u16,
    pub conversation_id: ConversationId,
    pub turn_id: ConversationTurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session_id: Option<ConversationAgentSessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_id: Option<AgentDelegationId>,
    pub codex_item_id: String,
    pub kind: ConversationActivityKind,
    pub state: ConversationActivityState,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub output: Option<String>,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<u64>,
    /// The validated app-server item or notification payload used to build this
    /// presentation projection. Older activities omit it; consumers must keep
    /// using the normalized fields when it is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_payload: Option<serde_json::Value>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    #[default]
    Ask,
    AutoReview,
    ReadOnly,
    FullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionApprovalPolicy {
    Untrusted,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TeamPreset {
    #[default]
    Automatic,
    Fast,
    Balanced,
    Thorough,
}

impl TeamPreset {
    /// Returns the owner-selected parallel specialist ceiling. Automatic and
    /// Thorough defer to the pinned organization's policy.
    #[must_use]
    pub const fn parallel_specialist_limit(self) -> Option<u16> {
        match self {
            Self::Automatic | Self::Thorough => None,
            Self::Fast => Some(1),
            Self::Balanced => Some(3),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CodexConfigOverride {
    /// TOML-compatible dotted configuration path, retained for audit and UI editing.
    pub key: String,
    /// Value after Codex-compatible TOML parsing (or literal-string fallback).
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExecutionPreferences {
    /// Optional Codex model-provider override. Only dedicated, validated
    /// provider selection may populate this value; raw configuration cannot
    /// replace `MetaCog`'s provider or authentication boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    pub approval_mode: ApprovalMode,
    /// Optional exact Codex approval-policy override. Legacy callers continue
    /// to derive this from `approval_mode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ExecutionApprovalPolicy>,
    /// Optional exact Codex sandbox override. Project-free and role-scoped
    /// execution can still reduce this authority at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<ExecutionSandboxMode>,
    /// Optional named Codex permission profile selected by the owner. The
    /// runtime re-resolves it against the pinned Codex catalog before use and
    /// never permits it to expand `MetaCog` role or organization authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<String>,
    /// Optional Codex v2 configuration profile selected with `--profile`/`-p`.
    /// The runtime resolves and validates this file through the pinned Codex
    /// loader before persistence, then stores only a bounded, governance-safe
    /// snapshot for deterministic execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_profile: Option<String>,
    /// Governance-safe profile snapshot resolved by the runtime. API callers
    /// cannot expand authority by supplying this value because it is always
    /// replaced during originating-turn validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_profile_config: Option<serde_json::Value>,
    /// SHA-256 of the exact profile file used to create the snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_profile_digest: Option<String>,
    /// Profile paths intentionally removed because `MetaCog` governs them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codex_profile_ignored_keys: Vec<String>,
    /// Owner-selected, canonical local directories that a workspace-write turn may also mutate.
    /// Runtime validation re-resolves these paths before every originating turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_writable_roots: Vec<String>,
    /// Per-run Codex feature overrides. These are validated against `MetaCog`'s
    /// authority boundary and never mutate the shared Codex configuration.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub codex_feature_overrides: BTreeMap<String, bool>,
    /// Ordered per-run Codex configuration overrides. Later entries win, matching
    /// repeatable `-c key=value` CLI behavior. Runtime validation prevents these
    /// values from expanding MetaCog-governed authority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codex_config_overrides: Vec<CodexConfigOverride>,
    /// Require Codex's own strict configuration loader to accept the current
    /// base and project configuration before this run is persisted or sent.
    #[serde(default)]
    pub strict_config: bool,
    /// Optional JSON Schema constraining the final owner-visible response.
    /// The runtime validates and bounds this value before persistence, and
    /// applies it only to the final Chat response rather than internal routing
    /// or specialist coordination turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub team_preset: TeamPreset,
    /// Optional owner-selected response style. This never expands authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionKind {
    File,
    Folder,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MentionTarget {
    ProjectPath {
        project_id: ProjectId,
        relative_path: String,
    },
    ExternalPath {
        canonical_path: String,
        artifact_sha256: Option<String>,
        artifact_relative_path: Option<String>,
    },
    Plugin {
        plugin_id: String,
        plugin_revision_id: PluginRevisionId,
        installation_revision: u64,
        grants_sha256: String,
    },
}

/// One untrusted composer reference submitted by a first-party client. Byte
/// offsets address the UTF-8 message buffer and are revalidated by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerMentionInput {
    pub kind: MentionKind,
    pub label: String,
    pub resource: String,
    pub byte_start: u32,
    pub byte_end: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_revision_id: Option<PluginRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMention {
    pub id: String,
    pub kind: MentionKind,
    pub label: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub target: MentionTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_modified_unix_milliseconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionCandidate {
    pub kind: MentionKind,
    pub label: String,
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_revision_id: Option<PluginRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub schema_version: u16,
    pub id: ConversationId,
    pub title: String,
    pub project_id: Option<ProjectId>,
    pub state: ConversationState,
    pub execution_preferences: ExecutionPreferences,
    #[serde(default)]
    pub mode: ConversationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<ConversationGoal>,
    #[serde(default)]
    pub pending_input_requests: Vec<ConversationInputRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_conversation_id: Option<ConversationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_origin_turn_id: Option<ConversationTurnId>,
    #[serde(default)]
    pub visibility: ConversationVisibility,
    #[serde(default)]
    pub interaction_kind: ConversationInteractionKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    #[must_use]
    pub fn new(title: impl Into<String>, project_id: Option<ProjectId>) -> Self {
        let now = Utc::now();
        Self {
            schema_version: CONVERSATION_SCHEMA_VERSION,
            id: ConversationId::new(),
            title: title.into(),
            project_id,
            state: ConversationState::Active,
            execution_preferences: ExecutionPreferences::default(),
            mode: ConversationMode::Default,
            goal: None,
            pending_input_requests: Vec::new(),
            parent_conversation_id: None,
            fork_origin_turn_id: None,
            visibility: ConversationVisibility::Normal,
            interaction_kind: ConversationInteractionKind::Interactive,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationInteractionKind {
    #[default]
    Interactive,
    NonInteractive,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationVisibility {
    #[default]
    Normal,
    Ephemeral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub schema_version: u16,
    pub id: ConversationMessageId,
    pub conversation_id: ConversationId,
    pub turn_id: ConversationTurnId,
    pub role: ConversationRole,
    pub content: String,
    #[serde(default)]
    pub mentions: Vec<ConversationMention>,
    pub codex_item_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub schema_version: u16,
    pub id: ConversationTurnId,
    pub conversation_id: ConversationId,
    pub state: ConversationTurnState,
    pub execution_preferences: ExecutionPreferences,
    pub codex_thread_id: Option<String>,
    pub codex_turn_id: Option<String>,
    pub task_id: Option<TaskId>,
    pub run_id: Option<RunId>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationView {
    pub conversation: Conversation,
    pub turns: Vec<ConversationTurn>,
    pub messages: Vec<ConversationMessage>,
    #[serde(default)]
    pub activities: Vec<ConversationActivity>,
    #[serde(default)]
    pub team: ConversationTeamState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexThreadBinding {
    pub conversation_id: Option<ConversationId>,
    pub conversation_turn_id: Option<ConversationTurnId>,
    pub run_id: Option<RunId>,
    pub agent_instance_id: Option<AgentInstanceId>,
    #[serde(default)]
    pub agent_session_id: Option<ConversationAgentSessionId>,
    #[serde(default)]
    pub delegation_id: Option<AgentDelegationId>,
    #[serde(default)]
    pub role_id: Option<RoleId>,
    #[serde(default)]
    pub organization_revision_id: Option<OrganizationRevisionId>,
    #[serde(default)]
    pub workspace_id: Option<ManagedWorktreeId>,
    pub codex_thread_id: String,
    pub codex_turn_id: Option<String>,
    pub codex_item_id: Option<String>,
    pub codex_session_id: Option<String>,
    pub binary_version: String,
    pub protocol_version: String,
    #[serde(default)]
    pub ownership_source: Option<String>,
    #[serde(default)]
    pub manageable: bool,
    #[serde(default)]
    pub last_reconciled_item_id: Option<String>,
    #[serde(default)]
    pub recovery_state: CodexRecoveryState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodexRecoveryState {
    #[default]
    Healthy,
    Retryable,
    Recovering,
    Indeterminate,
    Resumed,
    Forked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexOwnedThread {
    pub conversation_id: ConversationId,
    pub codex_thread_id: String,
    pub ownership_source: String,
    pub manageable: bool,
    pub recovery_state: CodexRecoveryState,
    pub last_reconciled_item_id: Option<String>,
    pub binary_version: String,
    pub protocol_version: String,
}

impl TryFrom<CodexThreadBinding> for CodexOwnedThread {
    type Error = &'static str;

    fn try_from(binding: CodexThreadBinding) -> Result<Self, Self::Error> {
        let conversation_id = binding
            .conversation_id
            .ok_or("binding is not associated with a conversation")?;
        let ownership_source = binding
            .ownership_source
            .ok_or("binding ownership cannot be proven")?;
        Ok(Self {
            conversation_id,
            codex_thread_id: binding.codex_thread_id,
            ownership_source,
            manageable: binding.manageable,
            recovery_state: binding.recovery_state,
            last_reconciled_item_id: binding.last_reconciled_item_id,
            binary_version: binding.binary_version,
            protocol_version: binding.protocol_version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexItemEvent {
    pub conversation_id: ConversationId,
    pub conversation_turn_id: ConversationTurnId,
    pub codex_thread_id: String,
    pub codex_turn_id: Option<String>,
    pub codex_item_id: String,
    pub event_kind: String,
    pub sequence: u64,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexPendingRequestRecord {
    pub request_id: String,
    pub method: String,
    pub supported: bool,
    pub state: String,
    pub received_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub schema_version: u16,
    pub id: TaskId,
    pub project_id: Option<ProjectId>,
    pub thread_id: Option<ThreadId>,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_conditions: Vec<String>,
    pub risk: RiskLevel,
    pub budget: TaskBudget,
    pub state: TaskState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskEnvelope {
    #[must_use]
    pub fn new(goal: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            schema_version: TASK_SCHEMA_VERSION,
            id: TaskId::new(),
            project_id: None,
            thread_id: None,
            goal: goal.into(),
            constraints: Vec::new(),
            success_conditions: Vec::new(),
            risk: RiskLevel::Medium,
            budget: TaskBudget::default(),
            state: TaskState::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    /// Applies a legal state transition.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidTaskTransition`] when the state machine forbids the edge.
    pub fn transition_to(&mut self, next: TaskState) -> Result<(), InvalidTaskTransition> {
        if !self.state.can_transition_to(next) {
            return Err(InvalidTaskTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_envelope_round_trips_through_json() {
        let task = TaskEnvelope::new("Build a verified website");
        let json = serde_json::to_string(&task).expect("task should serialize");
        let decoded: TaskEnvelope = serde_json::from_str(&json).expect("task should deserialize");

        assert_eq!(decoded, task);
        assert_eq!(decoded.schema_version, TASK_SCHEMA_VERSION);
    }

    #[test]
    fn legacy_conversation_defaults_to_normal_mode_without_goal_state() {
        let conversation = Conversation::new("Legacy chat", None);
        let mut document =
            serde_json::to_value(conversation).expect("conversation should serialize");
        let object = document.as_object_mut().expect("conversation object");
        object
            .get_mut("execution_preferences")
            .and_then(serde_json::Value::as_object_mut)
            .expect("execution preferences")
            .remove("personality");
        let execution_preferences = object
            .get_mut("execution_preferences")
            .and_then(serde_json::Value::as_object_mut)
            .expect("execution preferences");
        execution_preferences.remove("approval_policy");
        execution_preferences.remove("sandbox_mode");
        execution_preferences.remove("additional_writable_roots");
        execution_preferences.remove("strict_config");
        execution_preferences.remove("codex_profile");
        execution_preferences.remove("codex_profile_config");
        execution_preferences.remove("codex_profile_digest");
        execution_preferences.remove("codex_profile_ignored_keys");
        execution_preferences.remove("output_schema");
        object.remove("mode");
        object.remove("goal");
        object.remove("pending_input_requests");
        object.remove("interaction_kind");

        let decoded: Conversation = serde_json::from_value(document).expect("legacy conversation");
        assert_eq!(decoded.mode, ConversationMode::Default);
        assert!(decoded.execution_preferences.personality.is_none());
        assert!(decoded.execution_preferences.approval_policy.is_none());
        assert!(decoded.execution_preferences.sandbox_mode.is_none());
        assert!(!decoded.execution_preferences.strict_config);
        assert_eq!(decoded.execution_preferences.codex_profile, None);
        assert_eq!(decoded.execution_preferences.codex_profile_config, None);
        assert_eq!(decoded.execution_preferences.codex_profile_digest, None);
        assert_eq!(decoded.execution_preferences.output_schema, None);
        assert!(
            decoded
                .execution_preferences
                .codex_profile_ignored_keys
                .is_empty()
        );
        assert!(
            decoded
                .execution_preferences
                .additional_writable_roots
                .is_empty()
        );
        assert!(decoded.goal.is_none());
        assert!(decoded.pending_input_requests.is_empty());
        assert_eq!(
            decoded.interaction_kind,
            ConversationInteractionKind::Interactive
        );
    }

    #[test]
    fn cron_and_adaptive_heartbeat_compute_deterministic_utc_wakes() {
        let after = "2026-08-14T23:58:45Z"
            .parse::<DateTime<Utc>>()
            .expect("timestamp");
        let cron = ScheduleTrigger::Cron {
            expression: "*/5 0 * * *".into(),
        };
        assert_eq!(
            cron.next_after(after, false).expect("valid cron"),
            Some(
                "2026-08-15T00:00:00Z"
                    .parse::<DateTime<Utc>>()
                    .expect("timestamp")
            )
        );
        assert_eq!(
            ScheduleTrigger::Cron {
                expression: "5/10 * * * *".into(),
            }
            .next_after(
                "2026-08-14T12:04:00Z"
                    .parse::<DateTime<Utc>>()
                    .expect("timestamp"),
                false,
            )
            .expect("stepped cron"),
            Some(
                "2026-08-14T12:05:00Z"
                    .parse::<DateTime<Utc>>()
                    .expect("timestamp")
            )
        );
        let heartbeat = ScheduleTrigger::Heartbeat {
            idle_seconds: 300,
            busy_seconds: 30,
        };
        assert_eq!(
            heartbeat.next_after(after, false).expect("idle wake"),
            Some(after + Duration::seconds(300))
        );
        assert_eq!(
            heartbeat.next_after(after, true).expect("busy wake"),
            Some(after + Duration::seconds(30))
        );
        assert!(
            ScheduleTrigger::Cron {
                expression: "70 * * * *".into()
            }
            .next_after(after, false)
            .is_err()
        );
    }

    #[test]
    fn terminal_task_states_cannot_restart() {
        assert!(!TaskState::Completed.can_transition_to(TaskState::Active));
        assert!(TaskState::Pending.can_transition_to(TaskState::Active));
        assert!(TaskState::WaitingForApproval.can_transition_to(TaskState::Blocked));
    }

    #[test]
    fn platform_session_state_machine_has_only_explicit_authority_edges() {
        assert!(PlatformSessionState::Requested.can_transition_to(PlatformSessionState::Active));
        assert!(PlatformSessionState::Active.can_transition_to(PlatformSessionState::HandedOff));
        assert!(PlatformSessionState::HandedOff.can_transition_to(PlatformSessionState::Active));
        assert!(PlatformSessionState::Active.can_transition_to(PlatformSessionState::Interrupted));
        assert!(PlatformSessionState::Interrupted.can_transition_to(PlatformSessionState::Closed));
        assert!(
            !PlatformSessionState::Requested.can_transition_to(PlatformSessionState::HandedOff)
        );
        assert!(!PlatformSessionState::Interrupted.can_transition_to(PlatformSessionState::Active));
        assert!(!PlatformSessionState::Closed.can_transition_to(PlatformSessionState::Active));
        assert!(PlatformSessionState::Requested.is_live());
        assert!(PlatformSessionState::HandedOff.is_live());
        assert!(!PlatformSessionState::Failed.is_live());
    }

    #[test]
    fn legacy_plugin_installation_defaults_to_no_capability_grants() {
        let revision_id = PluginRevisionId::new();
        let document = serde_json::json!({
            "schema_version": PLUGIN_CATALOG_SCHEMA_VERSION,
            "plugin_id": "example.legacy",
            "active_revision_id": revision_id,
            "enabled": false,
            "revision": 1,
            "updated_at": Utc::now(),
        });
        let installation: PluginInstallation =
            serde_json::from_value(document).expect("legacy installation");
        assert!(installation.capability_grants.is_empty());
    }

    #[test]
    fn retry_ancestors_are_not_effective_batch_outcomes() {
        let first = AgentDelegationId::new();
        let second = AgentDelegationId::new();
        let third = AgentDelegationId::new();
        let independent = AgentDelegationId::new();

        let leaves = effective_delegation_attempt_ids([
            (first, None),
            (second, Some(first)),
            (third, Some(second)),
            (independent, None),
        ]);

        assert_eq!(leaves, BTreeSet::from([third, independent]));
    }
}
