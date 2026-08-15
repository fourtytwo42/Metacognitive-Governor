//! Stable, versioned domain types shared by every MCG interface.

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
domain_id!(TaskId);
domain_id!(RunId);
domain_id!(ToolRequestId);
domain_id!(ToolExecutionId);
domain_id!(ApprovalId);
domain_id!(LeaseId);
domain_id!(ArtifactId);
domain_id!(ManagedProcessId);
domain_id!(ProjectSnapshotId);
domain_id!(MemoryClaimId);
domain_id!(OrganizationId);
domain_id!(OrganizationRevisionId);
domain_id!(RoleId);
domain_id!(AgentInstanceId);
domain_id!(AgentLeaseId);
domain_id!(WorkItemId);
domain_id!(EvaluationId);
domain_id!(UsageRecordId);
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
domain_id!(SupervisionObservationId);
domain_id!(ExperimentId);
domain_id!(ExperimentBenchmarkId);
domain_id!(ExperimentDecisionId);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}
