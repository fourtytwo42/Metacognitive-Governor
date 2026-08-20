//! Dependency wiring, durable task orchestration, and local authentication.

mod oauth;
mod shadow_verification;

pub use oauth::*;

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
    time::Instant,
};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentModelGateway,
    AgentRun, AgentRunState, AgentState, AgentToolGateway, AgentTurnCheckpoint,
    AgentTurnCheckpointSink, AgentTurnEngine, AgentTurnError, AgentTurnRequest, AgentTurnResult,
    AgentUsageRecord, BenchmarkArmResult, ConditionResult, ConfidenceEstimate,
    EXPERIMENT_SCHEMA_VERSION, EvaluationEvidence, EvaluationOutcome, EvaluationReport,
    ExperimentBenchmark, ExperimentDecision, ExperimentDecisionKind, ExperimentDefinition,
    ExperimentMode, ExperimentState, ExternalToolBinding, LeaseBudget, MeasuredCandidateDesigner,
    OrganizationGenome, OrganizationalMetrics, PlannedWork, PlanningRequest,
    PortableOrganizationComparison, PortableOrganizationGenome, ProspectiveAnalysis,
    ProviderProxyEnsembleCheckpoint, ProviderProxyTurnCheckpoint, RoleKind, RunSupervisionSnapshot,
    SHADOW_WORKSPACE_SCHEMA_VERSION, ShadowAuthority, ShadowExecutionReceipt, ShadowWorkspace,
    ShadowWorkspaceState, SuccessCondition, SupervisionAnalyzer, SupervisionObservation,
    SupervisionSignal, TaskComplexity, WorkPlan,
};
use mcg_config::{AppConfig, AutonomyPreset, UserSettings, UserSettingsRevision};
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, ApprovalId, ComputerActionBatch, ComputerActionBatchId,
    ComputerActionBatchState, ComputerUseCapability, ComputerUseTarget, EvaluationId,
    ExperimentBenchmarkId, ExperimentDecisionId, ExperimentId, LeaseId, MemoryClaimId,
    MemoryTransferId, NotificationSeverity, OrganizationId, OrganizationImportId,
    OrganizationRevisionId, PLATFORM_SCHEMA_VERSION, PlatformCapability,
    PlatformCapabilityAvailability, PlatformCapabilityKind, PlatformCapabilitySnapshot,
    PlatformCapabilitySnapshotId, PlatformSession, PlatformSessionEvent, PlatformSessionId,
    PlatformSessionKind, PlatformSessionOwner, PlatformSessionState, PluginCapabilityGrant,
    PluginCapabilityScope, PluginInstallation, PluginInvocationId, PluginInvocationRecord,
    PluginInvocationState, PluginRevision, ProjectId, ProxyCallId, ProxyConversationId, RiskLevel,
    RunId, SITE_SCHEMA_VERSION, SSH_SCHEMA_VERSION, SSH_WORKFLOW_SCHEMA_VERSION,
    ScheduleDefinition, ScheduleId, ScheduleOccurrence, ScheduleOccurrenceId,
    ScheduleOccurrenceState, ScheduleState, ScheduleTrigger, SchedulerNotification,
    SchedulerNotificationId, Site, SiteDeployment, SiteDeploymentCommand, SiteDeploymentId,
    SiteDeploymentState, SiteEnvironment, SiteHealthCheck, SiteHealthCheckId, SiteHealthState,
    SiteId, SiteMonitor, SiteRevision, SiteRevisionId, SiteState, SshAuthentication, SshHost,
    SshHostId, SshHostPlatform, SshHostRevision, SshHostRevisionId, SshHostState, SshOperation,
    SshOperationId, SshOperationKind, SshOperationState, SshWorkflow, SshWorkflowId,
    SshWorkflowState, SshWorkflowStep, SshWorkflowStepId, SshWorkflowStepState,
    SupervisionObservationId, TaskBudget, TaskEnvelope, TaskId, TaskState, ToolExecutionId,
    ToolRequestId, WorkItemId,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{
    LEGACY_MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSION, McpAuthMode, McpClient, McpConnection,
    McpCredential, McpCredentialStore, McpError, McpProtocolMode, McpToolCatalog,
    MemoryMcpCredentialStore, OsMcpCredentialStore,
};
use mcg_plugin_protocol::{
    PLUGIN_CAPABILITY_TURN_LIMIT, PLUGIN_SCHEMA_VERSION, PluginCapability,
    PluginCapabilityOperation, PluginCapabilityOutcome, PluginCapabilityRequest, PluginGuestTurn,
    PluginHostTurn, PluginResult, PluginValueSource, WasiPluginLimits,
};
use mcg_plugin_runner::{
    PluginRunnerClient, PluginRunnerClientError, PluginRunnerRequest, PluginRunnerResponse,
    PluginRunnerState,
};
use mcg_plugin_trust::{
    PluginSignatureProof, PluginTrustError, PluginTrustStore, TrustedPluginSigner,
};
use mcg_policy::{CapabilityPolicy, OwnerConstitution};
use mcg_projects::{
    ClaimProvenance, ContextAssembler, ContextAssembly, ContextCategory, ContextItem, MemoryClaim,
    MemoryError, MemoryLifecycle, MemoryLifecycleAction, MemoryState, ProjectDiscoverer,
    ProjectDiscoveryError, ProjectMemoryTransfer, ProjectModel, ProvenanceKind,
    SnapshotInvalidation, prepare_project_memory_transfer, render_managed_memory,
};
use mcg_providers::{
    BrowserAuthorizationConfig, DeviceAuthorizationConfig, MemoryProviderCredentialStore,
    ModelProbeReport, ModelRequest, ModelResponse, OAuthTokenSet, OsProviderCredentialStore,
    PROXY_LEDGER_SCHEMA_VERSION, ProviderConnection, ProviderCredential, ProviderCredentialStore,
    ProviderDispatchBegin, ProviderDispatchFailure, ProviderDispatchRecord, ProviderError,
    ProviderModelCatalog, ProviderModelRef, ProviderRegistry, ProviderSecretSlot, ProxyCallRecord,
    ProxyConversation, ProxyConversationView, ProxyUsageSummary, RouteCandidate, RoutingPolicy,
    ToolDefinition, clear_provider_credentials, save_provider_credentials,
};
use mcg_storage::{
    AgentRecoveryReport, AgentTurnDisposition, BackupMcpSecret, BackupProviderSecret,
    BackupSecretSnapshot, DataDirectoryLock, RecoveryReport, STORAGE_SCHEMA_VERSION,
    SqliteEventStore, StorageError, acquire_data_directory_lock,
    create_encrypted_backup_bundle_from_snapshot, decrypt_backup_bundle_secrets,
};
pub use mcg_storage::{
    AgentTurnCommit, AutomaticExperimentDisposition, AutomaticExperimentOutcome,
    AutomaticSupervisionMutation, BackupBundleManifest, BackupBundleSummary, BackupPassphrase,
    CandidateRevisionMutation, ConsumerEvaluationCheckpoint, ConsumerEvaluationCheckpointState,
    ExperimentMutation, ScheduleClaim, ScheduleWakeMutation, ShadowWorkspaceMutation,
    create_backup_bundle_from_snapshot, verify_backup_bundle,
};
use mcg_tools::{
    ApprovalRepository, ApprovalRequest, ApprovalResolution, ApprovalState, ArtifactStore,
    BrokerControls, BrowserExecutor, BrowserWorkflowDocument, BrowserWorkflowStep,
    ComputerUseAction, ComputerUseBatchResult, ComputerUseFrame, ComputerUseManager,
    ComputerUseObservation, CredentialReference, CredentialResolver, ExecutionLease,
    ExecutionLeaseRepository, FilesystemExecutor, GitExecutor, HeaderValue, HostExecutor,
    HttpExecutor, InMemoryMemoryRepository, LeaseState, MemoryExecutor, OutputRedactor,
    PersistentBrowserManager, PersistentBrowserMode, PersistentBrowserObservation,
    ProcessEnvironmentVariable, ProcessExecutor, ProcessOptions, ProcessStream,
    RejectingCredentialResolver, ResearchEvidenceDocument, ResearchExecutor, SecretMaterial,
    SshExecutor, SystemExecutor, TOOL_SCHEMA_VERSION, ToolAuditPhase, ToolAuditRecord,
    ToolAuditSink, ToolBroker, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus,
    ToolRequest, ToolRequestKind, VerificationEvidence, browser_runtime_available,
    computer_use_runtime_available,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use shadow_verification::UnavailableShadowCommandVerifier;
use shadow_verification::{
    SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION, ShadowCommandTranscript, ShadowCommandVerification,
    ShadowCommandVerifier, default_shadow_command_verifier,
};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub service: String,
    pub version: String,
    pub state: RuntimeState,
    pub advanced_mode: bool,
    pub autonomy: AutonomyPreset,
    pub shadow_experiments: bool,
    pub recovered_events: u64,
    pub recovered_tasks: u64,
    pub recovered_tool_leases: u64,
    pub recovered_agent_runs: u64,
    pub recovered_agent_instances: u64,
    pub recovered_agent_leases: u64,
    pub recovered_schedule_occurrences: u64,
    pub recovered_platform_sessions: u64,
    pub expired_platform_sessions: u64,
    pub recovered_computer_action_batches: u64,
    pub recovered_site_deployments: u64,
    pub recovered_ssh_operations: u64,
    pub recovered_ssh_workflows: u64,
    pub recovered_shadow_workspaces: u64,
    pub recovered_plugin_invocations: u64,
}

pub const DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Pass,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub code: String,
    pub label: String,
    pub severity: DiagnosticSeverity,
    pub detail: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCounts {
    pub events: u64,
    pub tasks: u64,
    pub artifacts: u64,
    pub backups: u64,
    pub providers: u64,
    pub models: u64,
    pub mcp_connections: u64,
    pub mcp_tools: u64,
    pub schedules: u64,
    pub pending_approvals: u64,
    pub plugins: u64,
    pub plugin_invocations: u64,
    #[serde(default)]
    pub provider_dispatch_started: u64,
    #[serde(default)]
    pub provider_dispatch_completed: u64,
    #[serde(default)]
    pub provider_dispatch_failed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub schema_version: u16,
    pub generated_at: DateTime<Utc>,
    pub service: String,
    pub version: String,
    pub os: String,
    pub architecture: String,
    pub overall: DiagnosticSeverity,
    pub checks: Vec<DiagnosticCheck>,
    pub counts: DiagnosticCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformSessionRequest {
    pub id: PlatformSessionId,
    pub kind: PlatformSessionKind,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub display_name: String,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub computer_capabilities: Vec<ComputerUseCapability>,
    #[serde(default)]
    pub computer_target: Option<ComputerUseTarget>,
    pub lifetime_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteRegistrationRequest {
    pub id: SiteId,
    pub project_id: ProjectId,
    pub display_name: String,
    pub environment: SiteEnvironment,
    pub public_url: String,
    pub health_url: String,
    pub hosting_provider: String,
    pub deployment_command: Option<SiteDeploymentCommand>,
    pub credential_reference: Option<String>,
    pub state: SiteState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteUpdateRequest {
    pub expected_revision: u32,
    pub display_name: String,
    pub environment: SiteEnvironment,
    pub public_url: String,
    pub health_url: String,
    pub hosting_provider: String,
    pub deployment_command: Option<SiteDeploymentCommand>,
    pub credential_reference: Option<String>,
    pub state: SiteState,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteMutation {
    pub site: Site,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteHealthMutation {
    pub check: SiteHealthCheck,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteDeploymentMutation {
    pub deployment: SiteDeployment,
    pub replayed: bool,
    pub outcome: Option<ToolOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteMonitorRequest {
    pub schedule_id: ScheduleId,
    pub interval_seconds: u64,
    pub expected_status: u16,
    pub first_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteMonitorMutation {
    pub monitor: SiteMonitor,
    pub schedule: ScheduleDefinition,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHostRegistrationRequest {
    pub id: SshHostId,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHostUpdateRequest {
    pub expected_revision: u32,
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
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHostMutation {
    pub host: SshHost,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshOperationMutation {
    pub operation: SshOperation,
    pub replayed: bool,
    pub outcome: Option<ToolOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshWorkflowStepRequest {
    pub id: SshWorkflowStepId,
    pub name: String,
    #[serde(default)]
    pub host_id: Option<SshHostId>,
    #[serde(default)]
    pub host_revision: Option<u32>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<SshWorkflowStepId>,
    pub operation: SshOperationKind,
    #[serde(default = "default_ssh_workflow_timeout")]
    pub timeout_seconds: u64,
}

const fn default_ssh_workflow_timeout() -> u64 {
    60
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshWorkflowCreateRequest {
    pub id: SshWorkflowId,
    pub display_name: String,
    pub task_id: Option<TaskId>,
    pub steps: Vec<SshWorkflowStepRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshWorkflowMutation {
    pub workflow: SshWorkflow,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformSessionCreateResult {
    pub session: PlatformSession,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformBrowserMutation {
    pub session: PlatformSession,
    pub browser: PersistentBrowserObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformComputerMutation {
    pub session: PlatformSession,
    pub computer: ComputerUseObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformComputerActionMutation {
    pub batch: ComputerActionBatch,
    pub replayed: bool,
    pub result: Option<ComputerUseBatchResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConsumerEvaluationProgress {
    AwaitingApproval { approval_id: ApprovalId },
    Settled(EvaluationReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Starting,
    Ready,
    Degraded,
    Stopping,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    ProjectDiscovery(#[from] ProjectDiscoveryError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    AgentTurn(#[from] AgentTurnError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Mcp(#[from] McpError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error(transparent)]
    PluginTrust(#[from] PluginTrustError),
    #[error("runtime storage lock was poisoned")]
    StorageLock,
    #[error("local authentication token is invalid")]
    InvalidAuthToken,
}

impl RuntimeError {
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::Storage(
                StorageError::TaskNotFound(_)
                    | StorageError::ProjectNotFound(_)
                    | StorageError::MemoryClaimNotFound(_)
                    | StorageError::OrganizationRevisionNotFound(_)
                    | StorageError::AgentRunNotFound(_)
                    | StorageError::AgentInstanceNotFound(_)
                    | StorageError::AgentLeaseNotFound(_)
                    | StorageError::ScheduleNotFound(_)
                    | StorageError::PlatformSessionNotFound(_)
                    | StorageError::SiteNotFound(_)
                    | StorageError::SshHostNotFound(_)
                    | StorageError::SshWorkflowNotFound(_)
                    | StorageError::BackupNotFound(_)
                    | StorageError::PluginNotFound(_)
                    | StorageError::PluginRevisionNotFound(_)
            )
        )
    }

    #[must_use]
    pub const fn is_invalid_request(&self) -> bool {
        matches!(
            self,
            Self::Storage(StorageError::InvalidTransition(_) | StorageError::InvalidData(_))
                | Self::ProjectDiscovery(_)
                | Self::Memory(
                    MemoryError::SecretNamespace
                        | MemoryError::InvalidConfidence
                        | MemoryError::EmptyKey
                        | MemoryError::InvalidTransfer(_)
                )
                | Self::AgentTurn(AgentTurnError::Invalid(_))
                | Self::Tool(ToolError::Invalid(_))
                | Self::PluginTrust(_)
        )
    }

    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::Storage(StorageError::Conflict(_))
                | Self::Memory(MemoryError::InvalidTransition(_, _, _))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMutation {
    pub project: ProjectModel,
    pub event: EventEnvelope,
    pub invalidation: Option<SnapshotInvalidationDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisionMutation {
    pub observation: SupervisionObservation,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceMutation<T> {
    pub evidence: T,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInvocationMutation {
    pub invocation: PluginInvocationRecord,
    pub result: Option<PluginResult>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInvalidationDto {
    pub changed_inputs: Vec<String>,
    pub invalidate_instructions: bool,
    pub invalidate_commands: bool,
    pub invalidate_world_model: bool,
}

impl From<SnapshotInvalidation> for SnapshotInvalidationDto {
    fn from(value: SnapshotInvalidation) -> Self {
        Self {
            changed_inputs: value.changed_inputs,
            invalidate_instructions: value.invalidate_instructions,
            invalidate_commands: value.invalidate_commands,
            invalidate_world_model: value.invalidate_world_model,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryClaimMutation {
    pub claim: MemoryClaim,
    pub previous: Option<MemoryClaim>,
    pub event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMemoryTransferMutation {
    pub transfer: ProjectMemoryTransfer,
    pub claim: MemoryClaim,
    pub replayed: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMemoryTransferPreview {
    pub claim: MemoryClaim,
    pub source_claim_sha256: String,
}

pub const MEMORY_CONSOLIDATION_BATCH_LIMIT: usize = 64;
pub const MEMORY_DECAY_GRACE_DAYS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryConsolidationReport {
    pub examined: usize,
    pub decayed: usize,
    pub retracted: usize,
    pub conflicts: usize,
    pub mutations: Vec<MemoryClaimMutation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortableOrganizationImportMutation {
    pub organization: OrganizationGenome,
    pub portable_sha256: String,
    pub replayed: bool,
    pub event: Option<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRunView {
    pub run: AgentRun,
    pub plan: WorkPlan,
    pub instances: Vec<AgentInstance>,
    pub leases: Vec<AgentLease>,
    pub usage: Vec<AgentUsageRecord>,
    pub evaluations: Vec<EvaluationReport>,
    pub results: Vec<AgentTurnResult>,
}

pub const RUN_REPLAY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReplayEvidence {
    pub task: TaskEnvelope,
    pub organization: OrganizationGenome,
    pub view: AgentRunView,
    pub events: Vec<EventEnvelope>,
    pub tool_audit: Vec<ToolAuditRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReplayCounts {
    pub events: u64,
    pub tool_audit_records: u64,
    pub work_items: u64,
    pub instances: u64,
    pub leases: u64,
    pub usage_records: u64,
    pub evaluations: u64,
    pub turn_results: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReplayInvariant {
    pub code: String,
    pub passed: bool,
    pub detail: String,
}

/// A deterministic, observation-only reconstruction of one durable agent run.
///
/// The digest covers `evidence` exactly. No wall-clock value is included, so
/// identical durable state produces an identical report across process restarts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReplayReport {
    pub schema_version: u16,
    pub run_id: RunId,
    pub task_id: TaskId,
    pub organization_revision_id: OrganizationRevisionId,
    pub terminal: bool,
    pub side_effect_free: bool,
    pub evidence_sha256: String,
    pub counts: RunReplayCounts,
    pub invariants: Vec<RunReplayInvariant>,
    pub evidence: RunReplayEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceAgentRun {
    pub task: TaskEnvelope,
    pub view: AgentRunView,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticShadowLaunch {
    pub experiment: ExperimentDefinition,
    pub run: AgentRun,
    pub source_run_id: RunId,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticShadowExecution {
    pub launch: AutomaticShadowLaunch,
    pub receipt: ShadowExecutionReceipt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticShadowAssessment {
    pub evaluation: EvaluationReport,
    pub benchmark: ExperimentBenchmark,
    pub experiment: ExperimentDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMutation {
    pub connection: ProviderConnection,
    pub catalog: ProviderModelCatalog,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpMutation {
    pub connection: McpConnection,
    pub catalog: McpToolCatalog,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDispatchSpec {
    pub run_id: RunId,
    pub work_item_id: WorkItemId,
    pub project_id: ProjectId,
    #[serde(default)]
    pub target_path: Option<String>,
    pub context_token_budget: u32,
    #[serde(default)]
    pub requested_tools: Vec<String>,
    pub estimated_output_tokens: u64,
    pub max_model_calls: u16,
}

pub struct PreparedAgentTurn {
    pub request: AgentTurnRequest,
    pub events: Vec<EventEnvelope>,
    pub project_root: std::path::PathBuf,
}

#[derive(Debug, Clone)]
struct ShadowExecutionAuthority {
    experiment_id: ExperimentId,
    workspace_root: std::path::PathBuf,
}

#[derive(Clone)]
struct RuntimeMcpExecutor {
    store: Arc<Mutex<SqliteEventStore>>,
    credentials: Arc<dyn McpCredentialStore>,
}

#[async_trait]
impl ToolExecutor for RuntimeMcpExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let ToolRequestKind::McpCall {
            connection_id,
            tool_name,
            arguments,
            read_only,
            destructive,
            open_world,
        } = &request.kind
        else {
            return Err(ToolError::Invalid(
                "external MCP executor received a non-MCP request".into(),
            ));
        };
        let (mut connection, descriptor, protocol_version) = {
            let store = self
                .store
                .lock()
                .map_err(|_| ToolError::Execution("runtime storage lock was poisoned".into()))?;
            let connection = store
                .mcp_connection(connection_id)
                .map_err(|error| ToolError::Execution(error.to_string()))?
                .ok_or_else(|| ToolError::Invalid("MCP connection was not found".into()))?;
            let catalog = store
                .mcp_catalog(connection_id)
                .map_err(|error| ToolError::Execution(error.to_string()))?
                .ok_or_else(|| ToolError::Invalid("MCP tool catalog was not found".into()))?;
            let protocol_version = catalog.protocol_version.clone();
            let descriptor = catalog
                .tools
                .into_iter()
                .find(|tool| tool.name == *tool_name)
                .ok_or_else(|| ToolError::Invalid("MCP tool was not discovered".into()))?;
            (connection, descriptor, protocol_version)
        };
        if !connection.enabled {
            return Err(ToolError::Denied("MCP connection is disabled".into()));
        }
        let classification = mcp_tool_classification(&descriptor.annotations);
        if classification != (*read_only, *destructive, *open_world) {
            return Err(ToolError::Denied(
                "MCP tool classification changed after authority was issued".into(),
            ));
        }
        let credential = load_mcp_credential(self.credentials.as_ref(), &connection)
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        connection.protocol_mode = protocol_mode_for_catalog(&protocol_version)
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let client = McpClient::new(connection, credential)
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let started_at = Utc::now();
        let result = client
            .call_tool(tool_name, arguments.clone())
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let finished_at = Utc::now();
        let stdout = serde_json::to_string(&result)
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: if result.is_error {
                ToolOutcomeStatus::Failed
            } else {
                ToolOutcomeStatus::Succeeded
            },
            summary: if result.is_error {
                format!("MCP tool {tool_name} returned an error result")
            } else {
                format!("MCP tool {tool_name} completed")
            },
            exit_code: None,
            stdout: Some(stdout),
            stderr: None,
            artifacts: Vec::new(),
            verification: vec![VerificationEvidence {
                method: "mcp.tool_result".into(),
                passed: !result.is_error,
                detail: format!("connection={connection_id}; tool={tool_name}"),
            }],
            started_at,
            finished_at,
        })
    }
}

#[derive(Clone)]
pub struct RuntimeProviderGateway {
    registry: Arc<tokio::sync::RwLock<ProviderRegistry>>,
    oauth_refresh: Arc<OAuthRefreshCoordinator>,
}

enum ProtectedProviderCredentialSnapshot {
    None,
    Primary(ProviderCredential),
    OAuth(OAuthTokenSet),
}

impl RuntimeProviderGateway {
    /// Streams from one exact route selected by the caller-side router.
    ///
    /// # Errors
    ///
    /// Returns provider configuration, authentication, transport, or protocol errors.
    pub async fn stream_response(
        &self,
        target: &ProviderModelRef,
        request: ModelRequest,
    ) -> Result<mcg_providers::ProviderStream, ProviderError> {
        let initial = self
            .registry
            .read()
            .await
            .stream_response(target, request.clone())
            .await;
        if !matches!(initial, Err(ProviderError::Authentication(_))) {
            return initial;
        }
        if !self.refresh_after_authentication_failure(target).await? {
            return initial;
        }
        self.registry
            .read()
            .await
            .stream_response(target, request)
            .await
    }

    async fn refresh_after_authentication_failure(
        &self,
        target: &ProviderModelRef,
    ) -> Result<bool, ProviderError> {
        self.oauth_refresh
            .refresh_after_authentication_failure(&target.provider_id)
            .await
            .map_err(|error| match error {
                RuntimeError::Provider(error) => error,
                error => ProviderError::Authentication(format!(
                    "automatic OAuth refresh failed without changing billing mode: {error}"
                )),
            })
    }
}

impl std::fmt::Debug for RuntimeProviderGateway {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeProviderGateway")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl AgentModelGateway for RuntimeProviderGateway {
    async fn create_response(
        &self,
        target: &ProviderModelRef,
        request: ModelRequest,
    ) -> Result<ModelResponse, ProviderError> {
        let initial = self
            .registry
            .read()
            .await
            .create_response(target, request.clone())
            .await;
        if !matches!(initial, Err(ProviderError::Authentication(_))) {
            return initial;
        }
        if !self.refresh_after_authentication_failure(target).await? {
            return initial;
        }
        self.registry
            .read()
            .await
            .create_response(target, request)
            .await
    }
}

#[derive(Clone)]
pub struct LocalAuth {
    token: String,
}

impl std::fmt::Debug for LocalAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LocalAuth").finish_non_exhaustive()
    }
}

impl LocalAuth {
    const MINIMUM_TOKEN_LENGTH: usize = 32;

    /// Loads an existing local token or creates a private one.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable, invalid, or unwritable token files.
    pub fn load_or_create(path: &Path) -> Result<Self, RuntimeError> {
        if path.exists() {
            restrict_to_owner(path, 0o600)?;
            let mut token = String::new();
            OpenOptions::new()
                .read(true)
                .open(path)?
                .read_to_string(&mut token)?;
            return Self::from_token(token.trim().to_owned());
        }

        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
        Self::from_token(token)
    }

    /// Creates an in-memory authenticator for deterministic tests.
    ///
    /// # Errors
    ///
    /// Rejects tokens too short for local bearer authentication.
    pub fn from_token(token: String) -> Result<Self, RuntimeError> {
        if token.len() < Self::MINIMUM_TOKEN_LENGTH {
            return Err(RuntimeError::InvalidAuthToken);
        }
        Ok(Self { token })
    }

    #[must_use]
    pub fn verifies(&self, candidate: &str) -> bool {
        candidate.len() == self.token.len()
            && bool::from(candidate.as_bytes().ct_eq(self.token.as_bytes()))
    }

    #[must_use]
    pub fn token_for_local_client(&self) -> &str {
        &self.token
    }
}

#[async_trait]
trait PluginRunnerGateway: Send + Sync {
    async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError>;
}

struct ProcessPluginRunnerGateway(PluginRunnerClient);

#[async_trait]
impl PluginRunnerGateway for ProcessPluginRunnerGateway {
    async fn invoke(
        &self,
        request: &PluginRunnerRequest,
    ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
        self.0.invoke(request).await
    }
}

fn package_plugin_runner() -> Option<Arc<dyn PluginRunnerGateway>> {
    let application = std::env::current_exe().ok()?;
    let client = PluginRunnerClient::adjacent_to(application).ok()?;
    Some(Arc::new(ProcessPluginRunnerGateway(client)))
}

#[derive(Clone)]
struct ProviderToolCredentialResolver {
    store: Arc<dyn ProviderCredentialStore>,
}

#[async_trait]
impl CredentialResolver for ProviderToolCredentialResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Result<SecretMaterial, ToolError> {
        let slot = match reference.key.as_str() {
            "primary" => ProviderSecretSlot::Primary,
            "oauth_id_token" => ProviderSecretSlot::OAuthIdToken,
            "oauth_refresh_token" => ProviderSecretSlot::OAuthRefreshToken,
            _ => {
                return Err(ToolError::Denied(format!(
                    "credential key {}/{} is unsupported",
                    reference.provider, reference.key
                )));
            }
        };
        let credential = self
            .store
            .load(&reference.provider, slot)
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .ok_or_else(|| {
                ToolError::Denied(format!(
                    "credential reference {}/{} is unavailable",
                    reference.provider, reference.key
                ))
            })?;
        SecretMaterial::new(credential.expose_to_host_executor())
    }
}

fn validate_plugin_invocation_replay(
    invocation: &PluginInvocationRecord,
    plugin_id: &str,
    task_id: Option<TaskId>,
    input_sha256: &str,
) -> Result<(), StorageError> {
    if invocation.plugin_id != plugin_id
        || invocation.task_id != task_id
        || invocation.input_sha256 != input_sha256
    {
        return Err(StorageError::Conflict(format!(
            "plugin invocation {} was reused with different input",
            invocation.id
        )));
    }
    Ok(())
}

const fn plugin_runner_failure(
    error: &PluginRunnerClientError,
    capability_executed: bool,
) -> (PluginInvocationState, &'static str) {
    if capability_executed {
        return (PluginInvocationState::Uncertain, "runner_uncertain");
    }
    match error {
        PluginRunnerClientError::Unavailable | PluginRunnerClientError::RequestTooLarge => {
            (PluginInvocationState::Failed, "runner_unavailable")
        }
        PluginRunnerClientError::Timeout
        | PluginRunnerClientError::ResponseTooLarge
        | PluginRunnerClientError::DiagnosticOutput
        | PluginRunnerClientError::InvalidResponse
        | PluginRunnerClientError::Process => {
            (PluginInvocationState::Uncertain, "runner_uncertain")
        }
    }
}

fn plugin_tool_kind(
    operation: &PluginCapabilityOperation,
    grants: &[PluginCapabilityGrant],
    project_root: &Path,
) -> Result<ToolRequestKind, ToolError> {
    for capability in operation.required_capabilities() {
        if !grants.iter().any(|grant| grant.capability == capability) {
            return Err(ToolError::Denied(format!(
                "plugin capability {capability:?} is not granted"
            )));
        }
    }
    match operation {
        PluginCapabilityOperation::FilesystemRead { path, max_bytes } => {
            require_task_project_grant(grants, PluginCapability::FilesystemRead)?;
            Ok(ToolRequestKind::FsRead {
                path: plugin_project_path(project_root, path, false)?,
                max_bytes: *max_bytes,
            })
        }
        PluginCapabilityOperation::FilesystemWrite {
            path,
            content,
            create_new,
        } => {
            require_task_project_grant(grants, PluginCapability::FilesystemWrite)?;
            Ok(ToolRequestKind::FsWrite {
                path: plugin_project_path(project_root, path, false)?,
                content: content.clone(),
                create_new: *create_new,
            })
        }
        PluginCapabilityOperation::HttpRequest {
            method,
            url,
            headers,
            body,
            max_response_bytes,
        } => {
            let parsed = url::Url::parse(url)
                .map_err(|_| ToolError::Invalid("plugin HTTP URL is invalid".into()))?;
            let origin = parsed.origin().ascii_serialization();
            if !grant_scopes(grants, PluginCapability::Network).any(|scope| {
                matches!(scope, PluginCapabilityScope::NetworkOrigin { origin: allowed } if allowed == &origin)
            }) {
                return Err(ToolError::Denied(format!(
                    "plugin network origin {origin} is not granted"
                )));
            }
            Ok(ToolRequestKind::HttpRequest {
                method: method.clone(),
                url: url.clone(),
                headers: headers
                    .iter()
                    .map(|header| {
                        Ok(mcg_tools::RequestHeader {
                            name: header.name.clone(),
                            value: plugin_value_source(&header.value, grants)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ToolError>>()?,
                body: body.clone(),
                max_response_bytes: *max_response_bytes,
            })
        }
        PluginCapabilityOperation::ProcessExec {
            executable,
            argv,
            cwd,
            env,
            stdin,
        } => {
            if !grant_scopes(grants, PluginCapability::Process).any(|scope| {
                matches!(scope, PluginCapabilityScope::Executable { executable: allowed } if allowed.eq_ignore_ascii_case(executable))
            }) {
                return Err(ToolError::Denied(format!(
                    "plugin executable {executable} is not granted"
                )));
            }
            Ok(ToolRequestKind::ProcessExec {
                options: ProcessOptions {
                    executable: executable.clone(),
                    argv: argv.clone(),
                    cwd: plugin_project_path(project_root, cwd, true)?,
                    env: env
                        .iter()
                        .map(|variable| {
                            Ok(ProcessEnvironmentVariable {
                                name: variable.name.clone(),
                                value: plugin_value_source(&variable.value, grants)?,
                            })
                        })
                        .collect::<Result<Vec<_>, ToolError>>()?,
                    stdin: if stdin.is_some() {
                        ProcessStream::Piped
                    } else {
                        ProcessStream::Null
                    },
                    stdin_data: stdin.clone(),
                    stdout: ProcessStream::Piped,
                    stderr: ProcessStream::Piped,
                },
            })
        }
    }
}

fn grant_scopes(
    grants: &[PluginCapabilityGrant],
    capability: PluginCapability,
) -> impl Iterator<Item = &PluginCapabilityScope> {
    grants
        .iter()
        .filter(move |grant| grant.capability == capability)
        .flat_map(|grant| grant.scopes.iter())
}

fn require_task_project_grant(
    grants: &[PluginCapabilityGrant],
    capability: PluginCapability,
) -> Result<(), ToolError> {
    if grant_scopes(grants, capability)
        .any(|scope| matches!(scope, PluginCapabilityScope::TaskProject))
    {
        Ok(())
    } else {
        Err(ToolError::Denied(format!(
            "plugin {capability:?} task-project scope is not granted"
        )))
    }
}

fn plugin_value_source(
    source: &PluginValueSource,
    grants: &[PluginCapabilityGrant],
) -> Result<HeaderValue, ToolError> {
    match source {
        PluginValueSource::Literal(value) => Ok(HeaderValue::Literal(value.clone())),
        PluginValueSource::Credential(reference) => {
            if !grant_scopes(grants, PluginCapability::Credentials).any(|scope| {
                matches!(
                    scope,
                    PluginCapabilityScope::Credential { provider, key }
                    if provider == &reference.provider && key == &reference.key
                )
            }) {
                return Err(ToolError::Denied(format!(
                    "plugin credential reference {}/{} is not granted",
                    reference.provider, reference.key
                )));
            }
            Ok(HeaderValue::Credential(CredentialReference {
                provider: reference.provider.clone(),
                key: reference.key.clone(),
            }))
        }
    }
}

fn plugin_project_path(
    project_root: &Path,
    relative: &str,
    allow_root: bool,
) -> Result<String, ToolError> {
    let path = Path::new(relative);
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => normalized.push(value),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(ToolError::Denied(
                    "plugin project path escapes its task project".into(),
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() && !allow_root {
        return Err(ToolError::Invalid("plugin project path is empty".into()));
    }
    Ok(project_root.join(normalized).to_string_lossy().into_owned())
}

fn plugin_capability_policy(
    grants: &[PluginCapabilityGrant],
    project_root: &Path,
) -> CapabilityPolicy {
    let mut policy = CapabilityPolicy::project(project_root);
    policy.filesystem_read_roots.clear();
    policy.filesystem_write_roots.clear();
    policy.allowed_executables.clear();
    policy.allowed_shells.clear();
    policy.allowed_network_origins.clear();
    policy.allow_shell = false;
    policy.allow_git_mutation = false;
    policy.managed_process_control = mcg_policy::CapabilityAccess::Denied;
    policy.system_inspection = mcg_policy::CapabilityAccess::Denied;
    for grant in grants {
        for scope in &grant.scopes {
            match (grant.capability, scope) {
                (PluginCapability::FilesystemRead, PluginCapabilityScope::TaskProject) => {
                    policy
                        .filesystem_read_roots
                        .push(project_root.to_path_buf());
                }
                (PluginCapability::FilesystemWrite, PluginCapabilityScope::TaskProject) => {
                    policy
                        .filesystem_write_roots
                        .push(project_root.to_path_buf());
                }
                (PluginCapability::Network, PluginCapabilityScope::NetworkOrigin { origin }) => {
                    policy.allowed_network_origins.push(origin.clone());
                }
                (PluginCapability::Process, PluginCapabilityScope::Executable { executable }) => {
                    policy.allowed_executables.push(executable.clone());
                }
                _ => {}
            }
        }
    }
    policy
}

fn plugin_tool_outcome(outcome: &ToolOutcome) -> serde_json::Value {
    serde_json::json!({
        "status": outcome.status,
        "summary": bounded_plugin_output_text(&outcome.summary, 4_096),
        "exit_code": outcome.exit_code,
        "stdout": outcome.stdout.as_deref().map(|value| bounded_plugin_output_text(value, 48 * 1_024)),
        "stderr": outcome.stderr.as_deref().map(|value| bounded_plugin_output_text(value, 48 * 1_024)),
        "artifacts": outcome.artifacts.iter().take(16).collect::<Vec<_>>(),
        "verification": outcome.verification.iter().take(16).map(|evidence| serde_json::json!({
            "method": bounded_plugin_output_text(&evidence.method, 256),
            "passed": evidence.passed,
            "detail": bounded_plugin_output_text(&evidence.detail, 1_024),
        })).collect::<Vec<_>>(),
    })
}

fn bounded_plugin_output_text(value: &str, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value.to_owned();
    }
    let mut end = maximum_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}[TRUNCATED]", &value[..end])
}

pub struct AppRuntime {
    config: AppConfig,
    data_lock: Option<DataDirectoryLock>,
    settings: Arc<RwLock<UserSettings>>,
    store: Arc<Mutex<SqliteEventStore>>,
    event_sender: broadcast::Sender<EventEnvelope>,
    auth: LocalAuth,
    recovery: RecoveryReport,
    recovered_tool_leases: u64,
    agent_recovery: AgentRecoveryReport,
    recovered_schedule_occurrences: u64,
    recovered_platform_sessions: u64,
    expired_platform_sessions: u64,
    recovered_computer_action_batches: u64,
    recovered_site_deployments: u64,
    recovered_ssh_operations: u64,
    recovered_ssh_workflows: u64,
    recovered_shadow_workspaces: u64,
    recovered_plugin_invocations: u64,
    platform_capabilities: PlatformCapabilitySnapshot,
    browser_sessions: Arc<PersistentBrowserManager>,
    computer_sessions: Arc<ComputerUseManager>,
    provider_credentials: Arc<dyn ProviderCredentialStore>,
    mcp_credentials: Arc<dyn McpCredentialStore>,
    provider_registry: Arc<tokio::sync::RwLock<ProviderRegistry>>,
    oauth_state: OAuthRuntimeState,
    oauth_refresh: Arc<OAuthRefreshCoordinator>,
    shadow_command_verifier: Arc<dyn ShadowCommandVerifier>,
    plugin_runner: Option<Arc<dyn PluginRunnerGateway>>,
    plugin_trust: Arc<PluginTrustStore>,
    _ephemeral_data_directory: Option<tempfile::TempDir>,
}

impl std::fmt::Debug for AppRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppRuntime")
            .field("config", &self.config)
            .field("auth", &self.auth)
            .field("recovery", &self.recovery)
            .field("recovered_tool_leases", &self.recovered_tool_leases)
            .field("agent_recovery", &self.agent_recovery)
            .field("provider_registry", &self.provider_registry)
            .finish_non_exhaustive()
    }
}

impl AppRuntime {
    /// Opens the durable runtime, migrates storage, and rebuilds projections.
    ///
    /// # Errors
    ///
    /// Returns an error when its private data directory, authentication, database,
    /// migration, or recovery cannot be initialized safely.
    pub fn open(config: AppConfig) -> Result<Self, RuntimeError> {
        fs::create_dir_all(&config.data_directory)?;
        restrict_to_owner(&config.data_directory, 0o700)?;
        let data_lock = acquire_data_directory_lock(&config.data_directory)?;
        let auth = LocalAuth::load_or_create(&config.auth_token_path())?;
        let mut store = SqliteEventStore::open(config.database_path())?;
        restrict_to_owner(&config.database_path(), 0o600)?;
        let recovery = store.recover_projections()?;
        let recovered_tool_leases = store.recover_tool_leases()?;
        let agent_recovery = store.recover_agent_work(Utc::now())?;
        let recovered_schedule_occurrences = store.recover_schedule_claims(Utc::now())?;
        let startup_now = Utc::now();
        let expired_platform_sessions = store.expire_platform_sessions(startup_now)?;
        let recovered_platform_sessions = store.recover_platform_sessions(startup_now)?;
        let recovered_computer_action_batches =
            store.recover_computer_action_batches(startup_now)?;
        let recovered_site_deployments = store.recover_site_deployments(startup_now)?;
        let recovered_ssh_operations = store.recover_ssh_operations(startup_now)?;
        let (recovered_ssh_workflows, _) = store.recover_ssh_workflows(startup_now)?;
        let recovered_shadow_workspaces = store.recover_shadow_workspaces(startup_now)?;
        let recovered_plugin_invocations = store
            .recover_plugin_invocations(startup_now)?
            .invocations_marked_uncertain;
        let platform_capabilities = detect_platform_capabilities(&config, startup_now);
        let platform_capabilities =
            store.record_platform_capability_snapshot(&platform_capabilities)?;
        let settings = store.initialize_user_settings(&config, startup_now)?;
        let browser_sessions = Arc::new(PersistentBrowserManager::new(
            config.browser_executable.clone(),
            config.data_directory.join("browser-sessions"),
        )?);
        let computer_sessions = Arc::new(ComputerUseManager::new());
        let provider_credentials: Arc<dyn ProviderCredentialStore> =
            Arc::new(OsProviderCredentialStore);
        let mcp_credentials: Arc<dyn McpCredentialStore> = Arc::new(OsMcpCredentialStore);
        Self::assemble(
            config,
            Some(data_lock),
            settings,
            store,
            auth,
            recovery,
            recovered_tool_leases,
            agent_recovery,
            recovered_schedule_occurrences,
            recovered_platform_sessions,
            expired_platform_sessions,
            recovered_computer_action_batches,
            recovered_site_deployments,
            recovered_ssh_operations,
            recovered_ssh_workflows,
            recovered_shadow_workspaces,
            recovered_plugin_invocations,
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            BrowserAuthorizationConfig::default(),
            DeviceAuthorizationConfig::default(),
            None,
        )
    }

    /// Creates an isolated runtime for tests and embedded deterministic fixtures.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or the supplied token is invalid.
    pub fn open_in_memory(config: AppConfig, token: String) -> Result<Self, RuntimeError> {
        Self::open_in_memory_with_oauth_configs(
            config,
            token,
            BrowserAuthorizationConfig::default(),
            DeviceAuthorizationConfig::default(),
        )
    }

    /// Creates an isolated runtime with explicit OAuth endpoints for deterministic
    /// loopback integration tests. Production callers should use [`Self::open`].
    ///
    /// # Errors
    ///
    /// Returns an error when storage or the supplied token is invalid.
    pub fn open_in_memory_with_oauth_configs(
        mut config: AppConfig,
        token: String,
        browser_config: BrowserAuthorizationConfig,
        device_config: DeviceAuthorizationConfig,
    ) -> Result<Self, RuntimeError> {
        let default_data_directory = AppConfig::default().data_directory;
        let ephemeral_data_directory = if config.data_directory == default_data_directory {
            let temporary = tempfile::Builder::new()
                .prefix("mcg-in-memory-")
                .tempdir()?;
            config.data_directory = temporary.path().join("data");
            Some(temporary)
        } else {
            None
        };
        fs::create_dir_all(&config.data_directory)?;
        let mut store = SqliteEventStore::open_in_memory()?;
        let recovery = store.recover_projections()?;
        let recovered_tool_leases = store.recover_tool_leases()?;
        let agent_recovery = store.recover_agent_work(Utc::now())?;
        let recovered_schedule_occurrences = store.recover_schedule_claims(Utc::now())?;
        let startup_now = Utc::now();
        let expired_platform_sessions = store.expire_platform_sessions(startup_now)?;
        let recovered_platform_sessions = store.recover_platform_sessions(startup_now)?;
        let recovered_computer_action_batches =
            store.recover_computer_action_batches(startup_now)?;
        let recovered_site_deployments = store.recover_site_deployments(startup_now)?;
        let recovered_ssh_operations = store.recover_ssh_operations(startup_now)?;
        let (recovered_ssh_workflows, _) = store.recover_ssh_workflows(startup_now)?;
        let recovered_shadow_workspaces = store.recover_shadow_workspaces(startup_now)?;
        let recovered_plugin_invocations = store
            .recover_plugin_invocations(startup_now)?
            .invocations_marked_uncertain;
        let platform_capabilities = detect_platform_capabilities(&config, startup_now);
        let platform_capabilities =
            store.record_platform_capability_snapshot(&platform_capabilities)?;
        let settings = store.initialize_user_settings(&config, startup_now)?;
        let browser_sessions = Arc::new(PersistentBrowserManager::new(
            config.browser_executable.clone(),
            config.data_directory.join("browser-sessions"),
        )?);
        let computer_sessions = Arc::new(ComputerUseManager::new());
        let auth = LocalAuth::from_token(token)?;
        let provider_credentials: Arc<dyn ProviderCredentialStore> =
            Arc::new(MemoryProviderCredentialStore::default());
        let mcp_credentials: Arc<dyn McpCredentialStore> =
            Arc::new(MemoryMcpCredentialStore::default());
        Self::assemble(
            config,
            None,
            settings,
            store,
            auth,
            recovery,
            recovered_tool_leases,
            agent_recovery,
            recovered_schedule_occurrences,
            recovered_platform_sessions,
            expired_platform_sessions,
            recovered_computer_action_batches,
            recovered_site_deployments,
            recovered_ssh_operations,
            recovered_ssh_workflows,
            recovered_shadow_workspaces,
            recovered_plugin_invocations,
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            browser_config,
            device_config,
            ephemeral_data_directory,
        )
    }

    #[allow(clippy::too_many_arguments)] // Centralizes all recovered state and injected secure boundaries.
    fn assemble(
        config: AppConfig,
        data_lock: Option<DataDirectoryLock>,
        settings: UserSettings,
        store: SqliteEventStore,
        auth: LocalAuth,
        recovery: RecoveryReport,
        recovered_tool_leases: u64,
        agent_recovery: AgentRecoveryReport,
        recovered_schedule_occurrences: u64,
        recovered_platform_sessions: u64,
        expired_platform_sessions: u64,
        recovered_computer_action_batches: u64,
        recovered_site_deployments: u64,
        recovered_ssh_operations: u64,
        recovered_ssh_workflows: u64,
        recovered_shadow_workspaces: u64,
        recovered_plugin_invocations: u64,
        platform_capabilities: PlatformCapabilitySnapshot,
        browser_sessions: Arc<PersistentBrowserManager>,
        computer_sessions: Arc<ComputerUseManager>,
        provider_credentials: Arc<dyn ProviderCredentialStore>,
        mcp_credentials: Arc<dyn McpCredentialStore>,
        browser_config: BrowserAuthorizationConfig,
        device_config: DeviceAuthorizationConfig,
        ephemeral_data_directory: Option<tempfile::TempDir>,
    ) -> Result<Self, RuntimeError> {
        let plugin_trust =
            PluginTrustStore::new(config.trusted_plugin_signers.iter().map(|signer| {
                TrustedPluginSigner {
                    id: signer.id.clone(),
                    public_key_base64: signer.public_key_base64.clone(),
                    registries: signer.registries.clone(),
                }
            }))?;
        let (event_sender, _) = broadcast::channel(256);
        let store = Arc::new(Mutex::new(store));
        let provider_registry = Arc::new(tokio::sync::RwLock::new(ProviderRegistry::default()));
        let oauth_refresh = Arc::new(OAuthRefreshCoordinator::new(
            store.clone(),
            provider_credentials.clone(),
            provider_registry.clone(),
            device_config.clone(),
            event_sender.clone(),
        ));
        Ok(Self {
            config,
            data_lock,
            settings: Arc::new(RwLock::new(settings)),
            store,
            event_sender,
            auth,
            recovery,
            recovered_tool_leases,
            agent_recovery,
            recovered_schedule_occurrences,
            recovered_platform_sessions,
            expired_platform_sessions,
            recovered_computer_action_batches,
            recovered_site_deployments,
            recovered_ssh_operations,
            recovered_ssh_workflows,
            recovered_shadow_workspaces,
            recovered_plugin_invocations,
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            provider_registry,
            oauth_state: OAuthRuntimeState::new(browser_config, device_config),
            oauth_refresh,
            shadow_command_verifier: default_shadow_command_verifier(),
            plugin_runner: package_plugin_runner(),
            plugin_trust: Arc::new(plugin_trust),
            _ephemeral_data_directory: ephemeral_data_directory,
        })
    }

    fn store(&self) -> Result<MutexGuard<'_, SqliteEventStore>, RuntimeError> {
        self.store.lock().map_err(|_| RuntimeError::StorageLock)
    }

    #[must_use]
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    #[must_use]
    pub fn auth(&self) -> &LocalAuth {
        &self.auth
    }

    #[must_use]
    pub fn status(&self) -> RuntimeStatus {
        let settings = self.user_settings();
        RuntimeStatus {
            service: "mcg-daemon".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            state: RuntimeState::Ready,
            advanced_mode: settings.advanced_mode,
            autonomy: settings.autonomy,
            shadow_experiments: settings.shadow_experiments,
            recovered_events: self.recovery.events_replayed,
            recovered_tasks: self.recovery.tasks_rebuilt,
            recovered_tool_leases: self.recovered_tool_leases,
            recovered_agent_runs: self.agent_recovery.runs_marked_recovering,
            recovered_agent_instances: self.agent_recovery.instances_expired,
            recovered_agent_leases: self.agent_recovery.leases_expired,
            recovered_schedule_occurrences: self.recovered_schedule_occurrences,
            recovered_platform_sessions: self.recovered_platform_sessions,
            expired_platform_sessions: self.expired_platform_sessions,
            recovered_computer_action_batches: self.recovered_computer_action_batches,
            recovered_site_deployments: self.recovered_site_deployments,
            recovered_ssh_operations: self.recovered_ssh_operations,
            recovered_ssh_workflows: self.recovered_ssh_workflows,
            recovered_shadow_workspaces: self.recovered_shadow_workspaces,
            recovered_plugin_invocations: self.recovered_plugin_invocations,
        }
    }

    /// Captures and registers one digest-bound sandboxed plugin revision in the
    /// local content-addressed store. New revisions are always disabled.
    ///
    /// # Errors
    /// Returns validation, digest, artifact, conflict, or storage failures. A
    /// module file captured before a later database conflict is an inert
    /// content-addressed orphan and is never routable.
    pub fn install_sandboxed_plugin(
        &self,
        revision: &PluginRevision,
        module: &[u8],
    ) -> Result<PluginInstallation, RuntimeError> {
        if revision.source != mcg_domain::PluginInstallSource::LocalImport
            || revision.signature.state != mcg_domain::PluginSignatureState::Unsigned
        {
            return Err(StorageError::InvalidData(
                "this install path accepts only explicit unsigned local imports".into(),
            )
            .into());
        }
        self.capture_plugin_module(revision, module)?;
        self.store()?
            .install_unsigned_local_plugin_revision(revision)
            .map_err(Into::into)
    }

    /// Verifies, captures, and registers one signed registry package. The
    /// revision remains disabled until a separate explicit enable mutation.
    ///
    /// # Errors
    /// Rejects untrusted signers/registries, malformed or mismatched proofs,
    /// changed module bytes, artifact failures, and durable catalog conflicts.
    pub fn install_verified_sandboxed_plugin(
        &self,
        revision: &PluginRevision,
        module: &[u8],
        proof: &PluginSignatureProof,
    ) -> Result<PluginInstallation, RuntimeError> {
        let verified = self
            .plugin_trust
            .verify_registry_revision(revision, proof)?;
        self.capture_plugin_module(verified.revision(), module)?;
        self.store()?
            .install_verified_plugin_revision(&verified)
            .map_err(Into::into)
    }

    fn capture_plugin_module(
        &self,
        revision: &PluginRevision,
        module: &[u8],
    ) -> Result<(), RuntimeError> {
        let module_size = u64::try_from(module.len())
            .map_err(|_| StorageError::InvalidData("plugin module is too large".into()))?;
        let module_sha256 = format!("{:x}", Sha256::digest(module));
        if module_size != revision.module_size_bytes || module_sha256 != revision.module_sha256 {
            return Err(StorageError::InvalidData(
                "plugin module bytes do not match the revision identity".into(),
            )
            .into());
        }
        let artifact_store = ArtifactStore::open(self.config.data_directory.join("artifacts"))?;
        let artifact = artifact_store.capture(module, "application/wasm")?;
        artifact_store.verify_reference(&artifact, revision.module_size_bytes)?;
        if artifact.sha256 != revision.module_sha256
            || artifact.size_bytes != revision.module_size_bytes
        {
            return Err(StorageError::Conflict(
                "captured plugin module changed before catalog commit".into(),
            )
            .into());
        }
        Ok(())
    }

    /// Lists stable plugin installation heads without module or invocation content.
    ///
    /// # Errors
    /// Returns storage failures.
    pub fn plugin_installations(&self) -> Result<Vec<PluginInstallation>, RuntimeError> {
        self.store()?.plugin_installations().map_err(Into::into)
    }

    /// Loads one immutable plugin revision without module bytes.
    ///
    /// # Errors
    /// Returns storage or decoding failures.
    pub fn plugin_revision(
        &self,
        revision_id: mcg_domain::PluginRevisionId,
    ) -> Result<Option<PluginRevision>, RuntimeError> {
        self.store()?
            .plugin_revision(revision_id)
            .map_err(Into::into)
    }

    /// Loads one durable plugin invocation projection without result content.
    ///
    /// # Errors
    /// Returns storage or decoding failures.
    pub fn plugin_invocation(
        &self,
        invocation_id: PluginInvocationId,
    ) -> Result<Option<PluginInvocationRecord>, RuntimeError> {
        self.store()?
            .plugin_invocation(invocation_id)
            .map_err(Into::into)
    }

    /// Loads immutable lifecycle snapshots for one plugin invocation.
    ///
    /// # Errors
    /// Returns storage or decoding failures.
    pub fn plugin_invocation_history(
        &self,
        invocation_id: PluginInvocationId,
    ) -> Result<Vec<PluginInvocationRecord>, RuntimeError> {
        self.store()?
            .plugin_invocation_history(invocation_id)
            .map_err(Into::into)
    }

    /// Replaces one plugin's complete owner-issued capability grant set. Every
    /// change disables the plugin until a separate explicit enable mutation.
    ///
    /// # Errors
    /// Returns not-found, stale-revision, scope validation, or storage failures.
    pub fn set_plugin_capability_grants(
        &self,
        plugin_id: &str,
        expected_revision: u64,
        grants: Vec<PluginCapabilityGrant>,
    ) -> Result<PluginInstallation, RuntimeError> {
        self.store()?
            .set_plugin_capability_grants(plugin_id, expected_revision, grants, Utc::now())
            .map_err(Into::into)
    }

    /// Enables or disables one plugin head with optimistic revision control.
    ///
    /// # Errors
    /// Returns not-found, stale-revision, or storage failures.
    pub fn set_plugin_enabled(
        &self,
        plugin_id: &str,
        expected_revision: u64,
        enabled: bool,
    ) -> Result<PluginInstallation, RuntimeError> {
        self.store()?
            .set_plugin_enabled(plugin_id, expected_revision, enabled, Utc::now())
            .map_err(Into::into)
    }

    /// Invokes one active sandboxed plugin through the isolated package-local
    /// runner and durably settles hash-only evidence.
    ///
    /// Exact terminal identities replay without executing. Process/contract
    /// failures after launch settle as `uncertain` and are never retried.
    ///
    /// # Errors
    /// Returns invalid input, missing/tampered artifact, catalog, or durable
    /// settlement failures. Guest failures are returned as successful mutations
    /// whose invocation state is `failed` or `uncertain`.
    pub async fn invoke_sandboxed_plugin(
        &self,
        invocation_id: PluginInvocationId,
        plugin_id: &str,
        task_id: Option<TaskId>,
        input: serde_json::Value,
    ) -> Result<PluginInvocationMutation, RuntimeError> {
        let limits = WasiPluginLimits::default();
        let input_bytes = serde_json::to_vec(&input).map_err(StorageError::from)?;
        if input_bytes.len() > limits.maximum_input_bytes {
            return Err(StorageError::InvalidData("plugin input is too large".into()).into());
        }
        let input_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
        if let Some(existing) = self.store()?.plugin_invocation(invocation_id)? {
            validate_plugin_invocation_replay(&existing, plugin_id, task_id, &input_sha256)?;
            return self.replay_plugin_invocation(existing);
        }
        let (installation, revision) = {
            let store = self.store()?;
            let installation = store
                .plugin_installation(plugin_id)?
                .ok_or_else(|| StorageError::PluginNotFound(plugin_id.into()))?;
            let revision = store
                .plugin_revision(installation.active_revision_id)?
                .ok_or(StorageError::PluginRevisionNotFound(
                    installation.active_revision_id,
                ))?;
            (installation, revision)
        };
        if !installation.enabled {
            return Err(StorageError::Conflict("plugin is disabled".into()).into());
        }
        let started_at = Utc::now();
        let running = PluginInvocationRecord {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: invocation_id,
            plugin_id: plugin_id.into(),
            plugin_revision_id: revision.id,
            task_id,
            input_sha256,
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at,
            completed_at: None,
        };
        if let Err(error) = self.store()?.begin_plugin_invocation(&running) {
            if matches!(error, StorageError::Conflict(_))
                && let Some(existing) = self.store()?.plugin_invocation(invocation_id)?
            {
                validate_plugin_invocation_replay(
                    &existing,
                    plugin_id,
                    task_id,
                    &running.input_sha256,
                )?;
                return self.replay_plugin_invocation(existing);
            }
            return Err(error.into());
        }

        self.execute_plugin_invocation(running, revision, input, limits)
            .await
    }

    #[allow(clippy::too_many_lines)] // Keeps the bounded guest/broker settlement state machine contiguous.
    async fn execute_plugin_invocation(
        &self,
        running: PluginInvocationRecord,
        revision: PluginRevision,
        input: serde_json::Value,
        limits: WasiPluginLimits,
    ) -> Result<PluginInvocationMutation, RuntimeError> {
        let artifact_store = ArtifactStore::open(self.config.data_directory.join("artifacts"))?;
        let module = match artifact_store
            .read_verified_digest(&revision.module_sha256, revision.module_size_bytes)
        {
            Ok(module) if u64::try_from(module.len()).ok() == Some(revision.module_size_bytes) => {
                module
            }
            _ => {
                return self.settle_plugin_invocation(
                    running,
                    PluginInvocationState::Failed,
                    None,
                    Some("module_unavailable"),
                );
            }
        };
        let Some(runner) = self.plugin_runner.clone() else {
            return self.settle_plugin_invocation(
                running,
                PluginInvocationState::Failed,
                None,
                Some("runner_unavailable"),
            );
        };
        let capability_aware = !revision.manifest.requested_capabilities.is_empty();
        let mut host_turn = PluginHostTurn {
            schema_version: PLUGIN_SCHEMA_VERSION,
            original_input: input.clone(),
            outcomes: Vec::new(),
        };
        let mut request_ids = BTreeSet::new();
        for _ in 0..=PLUGIN_CAPABILITY_TURN_LIMIT {
            let runner_input = if capability_aware {
                serde_json::to_value(&host_turn).map_err(StorageError::from)?
            } else {
                input.clone()
            };
            let request = PluginRunnerRequest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                manifest: revision.manifest.clone(),
                module_sha256: revision.module_sha256.clone(),
                module_base64: STANDARD.encode(&module),
                input: runner_input,
                limits: limits.clone(),
            };
            let response = match runner.invoke(&request).await {
                Ok(response) if response.state == PluginRunnerState::Succeeded => response,
                Ok(response) => {
                    return self.settle_plugin_invocation(
                        running,
                        PluginInvocationState::Failed,
                        None,
                        response.error_code.as_deref().or(Some("runner_failed")),
                    );
                }
                Err(error) => {
                    let (state, code) =
                        plugin_runner_failure(&error, !host_turn.outcomes.is_empty());
                    return self.settle_plugin_invocation(running, state, None, Some(code));
                }
            };
            let Some(mut result) = response.result else {
                return self.settle_plugin_invocation(
                    running,
                    if host_turn.outcomes.is_empty() {
                        PluginInvocationState::Failed
                    } else {
                        PluginInvocationState::Uncertain
                    },
                    None,
                    Some("runner_result_missing"),
                );
            };
            if !capability_aware {
                return self.persist_plugin_result(&artifact_store, running, result);
            }
            let guest_turn: PluginGuestTurn = match serde_json::from_value(result.output.clone()) {
                Ok(turn) => turn,
                Err(_) => {
                    return self.settle_plugin_invocation(
                        running,
                        if host_turn.outcomes.is_empty() {
                            PluginInvocationState::Failed
                        } else {
                            PluginInvocationState::Uncertain
                        },
                        None,
                        Some("guest_contract_invalid"),
                    );
                }
            };
            match guest_turn {
                PluginGuestTurn::Completed { output } => {
                    result.output = output;
                    return self.persist_plugin_result(&artifact_store, running, result);
                }
                PluginGuestTurn::CapabilityRequest { request } => {
                    if host_turn.outcomes.len() == PLUGIN_CAPABILITY_TURN_LIMIT
                        || request.validate(&revision.manifest).is_err()
                        || !request_ids.insert(request.id.clone())
                    {
                        return self.settle_plugin_invocation(
                            running,
                            if host_turn.outcomes.is_empty() {
                                PluginInvocationState::Failed
                            } else {
                                PluginInvocationState::Uncertain
                            },
                            None,
                            Some("capability_contract_invalid"),
                        );
                    }
                    match self
                        .execute_plugin_capability(&running, &revision, &request)
                        .await
                    {
                        Ok(outcome) if outcome.status == ToolOutcomeStatus::Succeeded => {
                            let outcome = plugin_tool_outcome(&outcome);
                            host_turn.outcomes.push(PluginCapabilityOutcome::Succeeded {
                                request_id: request.id,
                                outcome,
                            });
                        }
                        Ok(_) => host_turn.outcomes.push(PluginCapabilityOutcome::Failed {
                            request_id: request.id,
                            error_code: "tool_failed".into(),
                        }),
                        Err(ToolError::ApprovalRequired { .. }) => {
                            return self.settle_plugin_invocation(
                                running,
                                PluginInvocationState::Failed,
                                None,
                                Some("capability_approval_required"),
                            );
                        }
                        Err(ToolError::Denied(_) | ToolError::Invalid(_)) => {
                            return self.settle_plugin_invocation(
                                running,
                                PluginInvocationState::Failed,
                                None,
                                Some("capability_denied"),
                            );
                        }
                        Err(_) => {
                            return self.settle_plugin_invocation(
                                running,
                                PluginInvocationState::Uncertain,
                                None,
                                Some("capability_execution_uncertain"),
                            );
                        }
                    }
                }
            }
        }
        self.settle_plugin_invocation(
            running,
            PluginInvocationState::Uncertain,
            None,
            Some("capability_turn_limit"),
        )
    }

    #[allow(clippy::too_many_lines)] // Constructs one complete least-authority broker boundary.
    async fn execute_plugin_capability(
        &self,
        running: &PluginInvocationRecord,
        revision: &PluginRevision,
        capability_request: &PluginCapabilityRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let task_id = running
            .task_id
            .ok_or_else(|| ToolError::Denied("plugin capabilities require a task".into()))?;
        let task = self
            .task(task_id)
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .ok_or_else(|| ToolError::Denied("plugin capability task was not found".into()))?;
        let project_id = task.project_id.ok_or_else(|| {
            ToolError::Denied("plugin capabilities require a task project".into())
        })?;
        let project = self
            .project(project_id)
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .ok_or_else(|| ToolError::Denied("plugin capability project was not found".into()))?;
        let installation = self
            .store()
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .plugin_installation(&running.plugin_id)
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .ok_or_else(|| ToolError::Denied("plugin installation was not found".into()))?;
        if !installation.enabled || installation.active_revision_id != revision.id {
            return Err(ToolError::Denied(
                "plugin authority changed during invocation".into(),
            ));
        }

        let root = std::path::PathBuf::from(&project.root);
        let kind = plugin_tool_kind(
            &capability_request.operation,
            &installation.capability_grants,
            &root,
        )?;
        let mut request = ToolRequest::new(
            task_id,
            format!(
                "Plugin {} capability {}",
                running.plugin_id, capability_request.id
            ),
            kind,
        );
        let operation = serde_json::to_vec(capability_request)
            .map_err(|error| ToolError::Invalid(error.to_string()))?;
        let digest = Sha256::digest(
            [
                b"mcg.plugin.capability.v1".as_slice(),
                revision.id.to_string().as_bytes(),
                task_id.to_string().as_bytes(),
                running.input_sha256.as_bytes(),
                &operation,
            ]
            .concat(),
        );
        let mut stable = [0_u8; 16];
        stable.copy_from_slice(&digest[..16]);
        request.id = ToolRequestId::from_stable_bytes(stable);

        let artifact_root = self.config.data_directory.join("artifacts");
        let credentials: Arc<dyn CredentialResolver> = Arc::new(ProviderToolCredentialResolver {
            store: self.provider_credentials.clone(),
        });
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifact_root)
                .map_err(|error| ToolError::Execution(error.to_string()))?,
        );
        let process = Arc::new(
            ProcessExecutor::new([root.clone()], credentials.clone())
                .map_err(|error| ToolError::Execution(error.to_string()))?,
        );
        let http = Arc::new(
            HttpExecutor::new(credentials, filesystem.clone(), &artifact_root)
                .map_err(|error| ToolError::Execution(error.to_string()))?,
        );
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: http.clone(),
            browser: Arc::new(
                BrowserExecutor::with_filesystem(
                    self.config.browser_executable.clone(),
                    &artifact_root,
                    filesystem,
                )
                .map_err(|error| ToolError::Execution(error.to_string()))?,
            ),
            research: Arc::new(ResearchExecutor::new(http)),
            system: Arc::new(SystemExecutor),
            git: Arc::new(
                GitExecutor::new(process, [root.clone()], &artifact_root)
                    .map_err(|error| ToolError::Execution(error.to_string()))?,
            ),
            memory: Arc::new(MemoryExecutor::new(InMemoryMemoryRepository::default())),
            external: Arc::new(RuntimeMcpExecutor {
                store: self.store.clone(),
                credentials: self.mcp_credentials.clone(),
            }),
        };
        let capabilities = plugin_capability_policy(&installation.capability_grants, &root);
        let control = self.tool_control();
        ToolBroker::new(
            host,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        )
        .execute(&request)
        .await
    }

    fn persist_plugin_result(
        &self,
        artifact_store: &ArtifactStore,
        running: PluginInvocationRecord,
        result: PluginResult,
    ) -> Result<PluginInvocationMutation, RuntimeError> {
        let result_bytes = serde_json::to_vec(&result).map_err(StorageError::from)?;
        match artifact_store.capture(&result_bytes, "application/vnd.mcg.plugin-result+json") {
            Ok(artifact) => self.settle_plugin_invocation(
                running,
                PluginInvocationState::Succeeded,
                Some((artifact.sha256, result)),
                None,
            ),
            Err(_) => self.settle_plugin_invocation(
                running,
                PluginInvocationState::Uncertain,
                None,
                Some("result_persistence_failed"),
            ),
        }
    }

    fn settle_plugin_invocation(
        &self,
        mut invocation: PluginInvocationRecord,
        state: PluginInvocationState,
        result: Option<(String, PluginResult)>,
        error_code: Option<&str>,
    ) -> Result<PluginInvocationMutation, RuntimeError> {
        invocation.state = state;
        invocation.result_sha256 = result.as_ref().map(|(sha256, _)| sha256.clone());
        invocation.error_code = error_code.map(str::to_owned);
        invocation.completed_at = Some(Utc::now().max(invocation.started_at));
        let invocation = self.store()?.complete_plugin_invocation(&invocation)?;
        Ok(PluginInvocationMutation {
            invocation,
            result: result.map(|(_, result)| result),
            replayed: false,
        })
    }

    fn replay_plugin_invocation(
        &self,
        invocation: PluginInvocationRecord,
    ) -> Result<PluginInvocationMutation, RuntimeError> {
        let result = if invocation.state == PluginInvocationState::Succeeded {
            let digest = invocation.result_sha256.as_deref().ok_or_else(|| {
                StorageError::InvalidData("successful plugin invocation lacks result hash".into())
            })?;
            let artifacts = ArtifactStore::open(self.config.data_directory.join("artifacts"))?;
            let bytes = artifacts.read_verified_digest(digest, 8 * 1_024 * 1_024)?;
            let result: PluginResult =
                serde_json::from_slice(&bytes).map_err(StorageError::from)?;
            if result.schema_version != PLUGIN_SCHEMA_VERSION
                || result.plugin_id != invocation.plugin_id
            {
                return Err(StorageError::InvalidData(
                    "stored plugin result does not match its invocation".into(),
                )
                .into());
            }
            Some(result)
        } else {
            None
        };
        Ok(PluginInvocationMutation {
            invocation,
            result,
            replayed: true,
        })
    }

    /// Produces a versioned, secret-free, read-only support report.
    ///
    /// This method never probes providers, runs tools, or mutates durable state.
    ///
    /// # Errors
    /// Returns storage failures when the local inventory cannot be read safely.
    #[allow(clippy::too_many_lines)] // Keeps the complete ordered support contract auditable.
    pub fn diagnostics(&self, now: DateTime<Utc>) -> Result<DiagnosticReport, RuntimeError> {
        let storage = self.store()?.diagnostics()?;
        let mut checks = Vec::new();
        let mut counts = DiagnosticCounts {
            events: storage.event_count,
            tasks: storage.task_count,
            artifacts: storage.artifact_count,
            plugins: storage.plugin_count,
            plugin_invocations: storage.plugin_invocation_count,
            provider_dispatch_started: storage.provider_dispatch_started_count,
            provider_dispatch_completed: storage.provider_dispatch_completed_count,
            provider_dispatch_failed: storage.provider_dispatch_failed_count,
            ..DiagnosticCounts::default()
        };
        let mut add_check = |code: &str,
                             label: &str,
                             severity: DiagnosticSeverity,
                             detail: String,
                             remediation: Option<&str>| {
            checks.push(DiagnosticCheck {
                code: code.to_owned(),
                label: label.to_owned(),
                severity,
                detail,
                remediation: remediation.map(str::to_owned),
            });
        };

        let integrity_ok = storage.integrity.eq_ignore_ascii_case("ok");
        add_check(
            "database_integrity",
            "Database integrity",
            if integrity_ok {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Error
            },
            if integrity_ok {
                "SQLite quick-check completed successfully.".to_owned()
            } else {
                "SQLite reported an integrity problem.".to_owned()
            },
            (!integrity_ok).then_some(
                "Stop the daemon, preserve the data directory, and restore a verified backup.",
            ),
        );
        let schema_ok = storage.schema_version == STORAGE_SCHEMA_VERSION;
        add_check(
            "schema_compatibility",
            "Schema compatibility",
            if schema_ok {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Error
            },
            format!(
                "Database schema {} of {} supported.",
                storage.schema_version, STORAGE_SCHEMA_VERSION
            ),
            (!schema_ok).then_some("Install a compatible MCG version before continuing."),
        );

        let dispatches_clear = counts.provider_dispatch_started == 0;
        add_check(
            "provider_dispatch_recovery",
            "Provider dispatch recovery",
            if dispatches_clear {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Warning
            },
            format!(
                "{} physical provider request(s) are active or indeterminate; {} validated request(s) completed; {} definite request(s) failed.",
                counts.provider_dispatch_started,
                counts.provider_dispatch_completed,
                counts.provider_dispatch_failed
            ),
            (!dispatches_clear).then_some(
                "Review the affected task and provider billing/logs, then retry with a new owner-authorized call identity; MCG will not resend an indeterminate request automatically.",
            ),
        );

        let durable = self.data_lock.is_some();
        let auth_is_regular = if durable {
            fs::symlink_metadata(self.config.auth_token_path()).is_ok_and(|metadata| {
                metadata.file_type().is_file() && !metadata.file_type().is_symlink()
            })
        } else {
            true
        };
        add_check(
            "loopback_authentication",
            "Loopback authentication",
            if auth_is_regular {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Error
            },
            if durable {
                if auth_is_regular {
                    "Bearer authentication is active and its token is stored in a regular private file."
                } else {
                    "Bearer authentication is active, but its token file is not a safe regular file."
                }
            } else {
                "Bearer authentication is active in an isolated test runtime."
            }
            .to_owned(),
            (!auth_is_regular).then_some(
                "Stop the daemon, remove the unsafe token link, and restart MCG to create a private token file.",
            ),
        );

        let backup_result = if durable {
            self.backups()
        } else {
            Ok(Vec::new())
        };
        match backup_result {
            Ok(backups) => {
                counts.backups = backups.len().try_into().unwrap_or(u64::MAX);
                add_check(
                    "portable_backups",
                    "Portable backups",
                    if backups.is_empty() {
                        DiagnosticSeverity::Warning
                    } else {
                        DiagnosticSeverity::Pass
                    },
                    if backups.is_empty() {
                        "No portable backup has been created yet.".to_owned()
                    } else {
                        format!("{} portable backup(s) passed verification.", backups.len())
                    },
                    backups.is_empty().then_some(
                        "Create and retain a portable backup before relying on the installation.",
                    ),
                );
            }
            Err(_) => add_check(
                "portable_backups",
                "Portable backups",
                DiagnosticSeverity::Error,
                "At least one portable backup could not be verified.".to_owned(),
                Some("Inspect backups individually and replace any invalid bundle."),
            ),
        }

        let providers = self.provider_connections()?;
        counts.providers = providers.len().try_into().unwrap_or(u64::MAX);
        for provider in &providers {
            if let Some(catalog) = self.provider_catalog(&provider.id)? {
                counts.models = counts
                    .models
                    .saturating_add(catalog.models.len().try_into().unwrap_or(u64::MAX));
            }
        }
        let providers_ready = counts.providers > 0 && counts.models > 0;
        add_check(
            "provider_readiness",
            "Model provider readiness",
            if providers_ready {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Warning
            },
            format!(
                "{} provider connection(s) with {} discovered model(s).",
                counts.providers, counts.models
            ),
            (!providers_ready)
                .then_some("Connect a provider and complete model discovery before running tasks."),
        );

        let mcp_connections = self.mcp_connections()?;
        counts.mcp_connections = mcp_connections.len().try_into().unwrap_or(u64::MAX);
        for connection in &mcp_connections {
            if let Some(catalog) = self.mcp_catalog(&connection.id)? {
                counts.mcp_tools = counts
                    .mcp_tools
                    .saturating_add(catalog.tools.len().try_into().unwrap_or(u64::MAX));
            }
        }
        add_check(
            "mcp_inventory",
            "MCP inventory",
            DiagnosticSeverity::Pass,
            format!(
                "{} optional MCP connection(s) with {} discovered tool(s).",
                counts.mcp_connections, counts.mcp_tools
            ),
            None,
        );

        counts.schedules = self.schedules()?.len().try_into().unwrap_or(u64::MAX);
        counts.pending_approvals = self
            .approvals(Some(ApprovalState::Pending))?
            .len()
            .try_into()
            .unwrap_or(u64::MAX);
        add_check(
            "durable_work_inventory",
            "Durable work inventory",
            DiagnosticSeverity::Pass,
            format!(
                "{} task(s), {} schedule(s), and {} pending approval(s).",
                counts.tasks, counts.schedules, counts.pending_approvals
            ),
            None,
        );

        let unavailable: Vec<_> = self
            .platform_capabilities
            .capabilities
            .iter()
            .filter(|capability| {
                capability.availability != PlatformCapabilityAvailability::Available
            })
            .collect();
        add_check(
            "platform_capabilities",
            "Platform capabilities",
            if unavailable.is_empty() {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Warning
            },
            format!(
                "{} of {} optional platform capability checks are unavailable or unsupported.",
                unavailable.len(),
                self.platform_capabilities.capabilities.len()
            ),
            (!unavailable.is_empty()).then_some(
                "Open the advanced Platform panel to review unavailable host integrations.",
            ),
        );

        let overall = checks
            .iter()
            .map(|check| check.severity)
            .max()
            .unwrap_or(DiagnosticSeverity::Pass);
        Ok(DiagnosticReport {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            generated_at: now,
            service: "mcg-daemon".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            os: self.platform_capabilities.os.clone(),
            architecture: self.platform_capabilities.architecture.clone(),
            overall,
            checks,
            counts,
        })
    }

    #[must_use]
    pub fn user_settings(&self) -> UserSettings {
        self.settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Atomically changes owner-controlled live behavior with revision protection.
    ///
    /// # Errors
    /// Returns stale-revision, validation, serialization, or storage failures.
    pub fn update_user_settings(
        &self,
        expected_revision: u64,
        advanced_mode: bool,
        autonomy: AutonomyPreset,
        shadow_experiments: bool,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<UserSettings, RuntimeError> {
        let settings = self.store()?.update_user_settings(
            expected_revision,
            advanced_mode,
            autonomy,
            shadow_experiments,
            reason,
            now,
        )?;
        *self
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = settings.clone();
        Ok(settings)
    }

    /// Lists immutable owner-settings revisions.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn user_settings_history(&self) -> Result<Vec<UserSettingsRevision>, RuntimeError> {
        Ok(self.store()?.user_settings_history()?)
    }

    /// Creates or verifies one caller-identified portable, secret-free backup.
    ///
    /// # Errors
    /// Returns storage, filesystem, snapshot, or integrity failures.
    pub fn create_backup(&self, backup_id: Uuid) -> Result<BackupBundleSummary, RuntimeError> {
        let directory = self.config.data_directory.join("backups");
        fs::create_dir_all(&directory)?;
        restrict_to_owner(&directory, 0o700)?;
        let destination = directory.join(format!("{backup_id}.mcgb"));
        if destination.exists() {
            let summary = verify_backup_bundle(&destination)?;
            if summary.manifest.secrets_included {
                return Err(StorageError::Conflict(
                    "backup identity already belongs to an encrypted credential backup".into(),
                )
                .into());
            }
            return Ok(summary);
        }
        let snapshot = directory.join(format!(".mcgb-{backup_id}-{}.sqlite3", Uuid::new_v4()));
        let result = self.store()?.snapshot_database(&snapshot).and_then(|()| {
            create_backup_bundle_from_snapshot(
                &snapshot,
                &self.config.data_directory,
                &destination,
                env!("CARGO_PKG_VERSION"),
                Utc::now(),
            )
        });
        let summary = finish_online_snapshot(&snapshot, result)?;
        restrict_to_owner(&destination, 0o600)?;
        Ok(summary)
    }

    /// Creates or authenticates one explicitly passphrase-encrypted credential backup.
    ///
    /// # Errors
    /// Returns protected-store, snapshot, KDF, encryption, identity-conflict, or integrity failures.
    #[allow(clippy::too_many_lines)] // Keeps connection locks, credential capture, SQLite snapshot, and cleanup in one audited scope.
    pub async fn create_encrypted_backup(
        &self,
        backup_id: Uuid,
        passphrase: &BackupPassphrase,
    ) -> Result<BackupBundleSummary, RuntimeError> {
        let directory = self.config.data_directory.join("backups");
        fs::create_dir_all(&directory)?;
        restrict_to_owner(&directory, 0o700)?;
        let destination = directory.join(format!("{backup_id}.mcgb"));
        if destination.exists() {
            let (summary, _) = decrypt_backup_bundle_secrets(&destination, passphrase)?;
            return Ok(summary);
        }

        let (initial_providers, initial_mcp) = {
            let store = self.store()?;
            (store.provider_connections()?, store.mcp_connections()?)
        };
        let mut provider_locks = Vec::with_capacity(initial_providers.len());
        for connection in &initial_providers {
            provider_locks.push(self.oauth_refresh.lock_connection(&connection.id).await);
        }
        let (locked_providers, locked_mcp) = {
            let store = self.store()?;
            (store.provider_connections()?, store.mcp_connections()?)
        };
        if initial_providers != locked_providers || initial_mcp != locked_mcp {
            return Err(StorageError::Conflict(
                "connections changed while encrypted backup authority was acquired".into(),
            )
            .into());
        }
        let provider_secrets = locked_providers
            .iter()
            .map(|connection| {
                Ok(match self.provider_credential_snapshot(connection)? {
                    ProtectedProviderCredentialSnapshot::None => BackupProviderSecret::None {
                        connection_id: connection.id.clone(),
                    },
                    ProtectedProviderCredentialSnapshot::Primary(credential) => {
                        BackupProviderSecret::Primary {
                            connection_id: connection.id.clone(),
                            auth_mode: connection.auth_mode,
                            credential,
                        }
                    }
                    ProtectedProviderCredentialSnapshot::OAuth(tokens) => {
                        BackupProviderSecret::OAuth {
                            connection_id: connection.id.clone(),
                            auth_mode: connection.auth_mode,
                            tokens,
                        }
                    }
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let mcp_secrets = locked_mcp
            .iter()
            .map(|connection| match connection.auth_mode {
                McpAuthMode::None => Ok(BackupMcpSecret::None {
                    connection_id: connection.id.clone(),
                }),
                McpAuthMode::BearerToken => self
                    .mcp_credentials
                    .load(&connection.id)?
                    .map(|credential| BackupMcpSecret::Bearer {
                        connection_id: connection.id.clone(),
                        credential,
                    })
                    .ok_or_else(|| {
                        McpError::Authentication(format!(
                            "MCP connection {} has no protected bearer credential",
                            connection.id
                        ))
                        .into()
                    }),
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let secrets = BackupSecretSnapshot::new(provider_secrets, mcp_secrets)?;
        let snapshot = directory.join(format!(".mcgb-{backup_id}-{}.sqlite3", Uuid::new_v4()));
        let result = {
            let store = self.store()?;
            if store.provider_connections()? != locked_providers
                || store.mcp_connections()? != locked_mcp
            {
                Err(StorageError::Conflict(
                    "connections changed while encrypted backup was being captured".into(),
                ))
            } else {
                store.snapshot_database(&snapshot)
            }
        }
        .and_then(|()| {
            create_encrypted_backup_bundle_from_snapshot(
                &snapshot,
                &self.config.data_directory,
                &destination,
                env!("CARGO_PKG_VERSION"),
                Utc::now(),
                &secrets,
                passphrase,
            )
        });
        let summary = finish_online_snapshot(&snapshot, result)?;
        restrict_to_owner(&destination, 0o600)?;
        drop(provider_locks);
        Ok(summary)
    }

    /// Lists and verifies every portable backup in deterministic name order.
    ///
    /// # Errors
    /// Returns filesystem or integrity failures. Invalid bundles never appear healthy.
    pub fn backups(&self) -> Result<Vec<BackupBundleSummary>, RuntimeError> {
        let directory = self.config.data_directory.join("backups");
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("mcgb") {
                paths.push(path);
            }
        }
        paths.sort();
        paths
            .iter()
            .map(|path| verify_backup_bundle(path).map_err(RuntimeError::from))
            .collect()
    }

    /// Verifies one caller-identified backup without changing runtime state.
    ///
    /// # Errors
    /// Returns missing-file, format, or integrity failures.
    pub fn verify_backup(&self, backup_id: Uuid) -> Result<BackupBundleSummary, RuntimeError> {
        let path = self
            .config
            .data_directory
            .join("backups")
            .join(format!("{backup_id}.mcgb"));
        if !path.exists() {
            return Err(StorageError::BackupNotFound(backup_id).into());
        }
        Ok(verify_backup_bundle(&path)?)
    }

    #[must_use]
    pub fn platform_capabilities(&self) -> PlatformCapabilitySnapshot {
        self.platform_capabilities.clone()
    }

    /// Requests a durable platform session reservation. A request remains
    /// `requested` until a concrete executor attaches and assumes authority.
    ///
    /// # Errors
    /// Returns an error for invalid input, unknown references, identity conflict, or storage failure.
    pub fn request_platform_session(
        &self,
        request: &PlatformSessionRequest,
        now: DateTime<Utc>,
    ) -> Result<PlatformSessionCreateResult, RuntimeError> {
        // Preserve the original browser definition byte-for-byte so a caller
        // retrying an identifier across an upgrade still receives the durable
        // reservation it created. Computer sessions use a distinct versioned
        // definition because their target and authority grants are immutable.
        let definition = match request.kind {
            PlatformSessionKind::Browser => serde_json::to_vec(&(
                PLATFORM_SCHEMA_VERSION,
                request.kind,
                request.project_id,
                request.task_id,
                request.display_name.trim(),
                &request.allowed_origins,
                request.lifetime_seconds,
            )),
            PlatformSessionKind::ComputerUse => serde_json::to_vec(&(
                PLATFORM_SCHEMA_VERSION,
                "computer_use_v1",
                request.kind,
                request.project_id,
                request.task_id,
                request.display_name.trim(),
                &request.computer_capabilities,
                &request.computer_target,
                request.lifetime_seconds,
            )),
        }
        .map_err(StorageError::from)?;
        let session = PlatformSession {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: request.id,
            definition_hash: format!("{:x}", Sha256::digest(definition)),
            kind: request.kind,
            state: PlatformSessionState::Requested,
            owner: PlatformSessionOwner::Mcg,
            project_id: request.project_id,
            task_id: request.task_id,
            display_name: request.display_name.trim().to_owned(),
            allowed_origins: request.allowed_origins.clone(),
            computer_capabilities: request.computer_capabilities.clone(),
            computer_target: request.computer_target.clone(),
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::seconds(i64::from(request.lifetime_seconds)),
            closed_at: None,
            recovery_count: 0,
        };
        let (session, created) = self
            .store()?
            .create_platform_session(&session, "local owner requested platform session")?;
        Ok(PlatformSessionCreateResult { session, created })
    }

    /// Lists durable platform session projections.
    ///
    /// # Errors
    /// Returns an error when runtime storage cannot be read.
    pub async fn platform_sessions(&self) -> Result<Vec<PlatformSession>, RuntimeError> {
        self.settle_expired_platform_sessions(Utc::now()).await?;
        Ok(self.store()?.platform_sessions()?)
    }

    /// Loads one durable platform session.
    ///
    /// # Errors
    /// Returns not-found for an unknown identity or an error when storage cannot be read.
    pub async fn platform_session(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<PlatformSession, RuntimeError> {
        self.settle_expired_platform_sessions(Utc::now()).await?;
        self.store()?
            .platform_session(session_id)?
            .ok_or_else(|| StorageError::PlatformSessionNotFound(session_id).into())
    }

    /// Loads immutable lifecycle history for one platform session.
    ///
    /// # Errors
    /// Returns not-found for an unknown identity or an error when storage cannot be read.
    pub async fn platform_session_history(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<Vec<PlatformSessionEvent>, RuntimeError> {
        self.settle_expired_platform_sessions(Utc::now()).await?;
        Ok(self.store()?.platform_session_history(session_id)?)
    }

    /// Launches an isolated persistent browser and then commits active authority.
    ///
    /// # Errors
    /// Returns an error for the wrong session kind/state/owner, browser failure, or storage failure.
    pub async fn attach_browser_session(
        &self,
        session_id: PlatformSessionId,
        mode: PersistentBrowserMode,
        now: DateTime<Utc>,
    ) -> Result<PlatformBrowserMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let requested = self.load_platform_session(session_id)?;
        require_browser_authority(
            &requested,
            PlatformSessionState::Requested,
            PlatformSessionOwner::Mcg,
        )?;
        let browser = self
            .browser_sessions
            .attach(session_id, &requested.allowed_origins, mode)
            .await?;
        let transition = {
            self.store()?.transition_platform_session(
                session_id,
                PlatformSessionState::Active,
                "isolated browser executor attached",
                now,
            )
        };
        match transition {
            Ok((session, _)) => Ok(PlatformBrowserMutation { session, browser }),
            Err(error) => {
                let _ = self.browser_sessions.close_if_attached(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Navigates an active MCG-owned browser inside its immutable origin grant.
    ///
    /// # Errors
    /// Returns an error for invalid authority, disallowed URLs, or browser failure.
    pub async fn navigate_browser_session(
        &self,
        session_id: PlatformSessionId,
        url: &str,
    ) -> Result<PersistentBrowserObservation, RuntimeError> {
        self.settle_expired_platform_sessions(Utc::now()).await?;
        let session = self.load_platform_session(session_id)?;
        require_browser_authority(
            &session,
            PlatformSessionState::Active,
            PlatformSessionOwner::Mcg,
        )?;
        Ok(self.browser_sessions.navigate(session_id, url).await?)
    }

    /// Hands a visible browser window to the local owner and commits that authority change.
    ///
    /// # Errors
    /// Returns an error for invalid authority, headless sessions, browser failure, or storage failure.
    pub async fn handoff_browser_session(
        &self,
        session_id: PlatformSessionId,
        now: DateTime<Utc>,
    ) -> Result<PlatformBrowserMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let session = self.load_platform_session(session_id)?;
        require_browser_authority(
            &session,
            PlatformSessionState::Active,
            PlatformSessionOwner::Mcg,
        )?;
        let browser = self.browser_sessions.handoff(session_id).await?;
        let transition = {
            self.store()?.transition_platform_session(
                session_id,
                PlatformSessionState::HandedOff,
                "visible browser handed to local owner",
                now,
            )
        };
        match transition {
            Ok((session, _)) => Ok(PlatformBrowserMutation { session, browser }),
            Err(error) => {
                let _ = self.browser_sessions.reclaim(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Blanks owner-visible content before returning browser authority to MCG.
    ///
    /// # Errors
    /// Returns an error for invalid authority, browser failure, or storage failure.
    pub async fn reclaim_browser_session(
        &self,
        session_id: PlatformSessionId,
        now: DateTime<Utc>,
    ) -> Result<PlatformBrowserMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let session = self.load_platform_session(session_id)?;
        require_browser_authority(
            &session,
            PlatformSessionState::HandedOff,
            PlatformSessionOwner::LocalOwner,
        )?;
        let browser = self.browser_sessions.reclaim(session_id).await?;
        let transition = {
            self.store()?.transition_platform_session(
                session_id,
                PlatformSessionState::Active,
                "local owner returned browser control after privacy blanking",
                now,
            )
        };
        match transition {
            Ok((session, _)) => Ok(PlatformBrowserMutation { session, browser }),
            Err(error) => {
                let _ = self.browser_sessions.handoff(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Binds the requested display region and then commits computer authority.
    ///
    /// # Errors
    /// Returns an error for invalid authority, an unavailable target, or storage failure.
    pub async fn attach_computer_session(
        &self,
        session_id: PlatformSessionId,
        now: DateTime<Utc>,
    ) -> Result<PlatformComputerMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let requested = self.load_platform_session(session_id)?;
        require_computer_authority(
            &requested,
            PlatformSessionState::Requested,
            PlatformSessionOwner::Mcg,
        )?;
        let target = requested
            .computer_target
            .as_ref()
            .ok_or_else(|| ToolError::Invalid("computer session has no immutable target".into()))?;
        let computer = self
            .computer_sessions
            .attach(session_id, target, &requested.computer_capabilities)
            .await?;
        let transition = self.store()?.transition_platform_session(
            session_id,
            PlatformSessionState::Active,
            "computer display executor attached",
            now,
        );
        match transition {
            Ok((session, _)) => Ok(PlatformComputerMutation { session, computer }),
            Err(error) => {
                let _ = self.computer_sessions.close_if_attached(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Captures the bound display region while MCG owns authority.
    ///
    /// # Errors
    /// Returns an error when authority, capture, or storage settlement fails.
    pub async fn capture_computer_session(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<ComputerUseFrame, RuntimeError> {
        self.settle_expired_platform_sessions(Utc::now()).await?;
        let session = self.load_platform_session(session_id)?;
        require_computer_authority(
            &session,
            PlatformSessionState::Active,
            PlatformSessionOwner::Mcg,
        )?;
        Ok(self.computer_sessions.capture(session_id).await?)
    }

    /// Applies a bounded typed batch only if the currently captured pixels still
    /// match the caller's observation digest.
    ///
    /// # Errors
    /// Returns an error for invalid authority, replay conflict, rejected input, or storage failure.
    pub async fn apply_computer_actions(
        &self,
        batch_id: ComputerActionBatchId,
        session_id: PlatformSessionId,
        expected_frame_sha256: &str,
        actions: &[ComputerUseAction],
    ) -> Result<PlatformComputerActionMutation, RuntimeError> {
        let now = Utc::now();
        self.settle_expired_platform_sessions(now).await?;
        let session = self.load_platform_session(session_id)?;
        let action_count = u16::try_from(actions.len())
            .map_err(|_| ToolError::Invalid("too many computer actions".into()))?;
        let request_hash =
            computer_action_request_hash(session_id, expected_frame_sha256, actions)?;
        if let Some(batch) = self.store()?.computer_action_batch(batch_id)? {
            if batch.session_id != session_id
                || batch.request_hash != request_hash
                || batch.action_count != action_count
            {
                return Err(StorageError::Conflict(
                    "computer action batch identity was reused with a different request".into(),
                )
                .into());
            }
            return settled_computer_action_replay(batch);
        }
        require_computer_authority(
            &session,
            PlatformSessionState::Active,
            PlatformSessionOwner::Mcg,
        )?;
        let task_id = session
            .task_id
            .unwrap_or_else(|| computer_authority_task_id(session_id));
        let tool_request_id = computer_action_tool_request_id(batch_id);
        let kind = ToolRequestKind::ComputerActions {
            batch_id,
            session_id,
            expected_frame_sha256: expected_frame_sha256.into(),
            actions: actions.to_vec(),
        };
        let mut request = ToolRequest::new(
            task_id,
            format!("Apply {action_count} bounded computer actions"),
            kind,
        );
        request.id = tool_request_id;
        let result = Arc::new(Mutex::new(None));
        let executor = ComputerActionExecutor {
            store: self.store.clone(),
            computer_sessions: self.computer_sessions.clone(),
            result: result.clone(),
        };
        let resource = computer_session_resource(session_id);
        let mut capabilities = CapabilityPolicy::project(&self.config.data_directory);
        capabilities.allowed_computer_sessions.push(resource);
        let control = self.tool_control();
        let broker = ToolBroker::new(
            executor,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        broker.execute(&request).await?;
        result
            .lock()
            .map_err(|_| RuntimeError::StorageLock)?
            .take()
            .ok_or_else(|| {
                ToolError::Execution("computer action executor returned no mutation".into()).into()
            })
    }

    /// Hands attached computer authority to the local owner.
    ///
    /// # Errors
    /// Returns an error when authority cannot be handed off or durably transitioned.
    pub async fn handoff_computer_session(
        &self,
        session_id: PlatformSessionId,
        now: DateTime<Utc>,
    ) -> Result<PlatformComputerMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let session = self.load_platform_session(session_id)?;
        require_computer_authority(
            &session,
            PlatformSessionState::Active,
            PlatformSessionOwner::Mcg,
        )?;
        let computer = self.computer_sessions.handoff(session_id).await?;
        let transition = self.store()?.transition_platform_session(
            session_id,
            PlatformSessionState::HandedOff,
            "computer control handed to local owner",
            now,
        );
        match transition {
            Ok((session, _)) => Ok(PlatformComputerMutation { session, computer }),
            Err(error) => {
                let _ = self.computer_sessions.reclaim(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Reclaims computer authority without reading the screen.
    ///
    /// # Errors
    /// Returns an error when authority cannot be reclaimed or durably transitioned.
    pub async fn reclaim_computer_session(
        &self,
        session_id: PlatformSessionId,
        now: DateTime<Utc>,
    ) -> Result<PlatformComputerMutation, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let session = self.load_platform_session(session_id)?;
        require_computer_authority(
            &session,
            PlatformSessionState::HandedOff,
            PlatformSessionOwner::LocalOwner,
        )?;
        let computer = self.computer_sessions.reclaim(session_id).await?;
        let transition = self.store()?.transition_platform_session(
            session_id,
            PlatformSessionState::Active,
            "local owner returned computer control; a fresh frame is required",
            now,
        );
        match transition {
            Ok((session, _)) => Ok(PlatformComputerMutation { session, computer }),
            Err(error) => {
                let _ = self.computer_sessions.handoff(session_id).await;
                Err(error.into())
            }
        }
    }

    /// Relinquishes a requested or attached session without inventing executor state.
    ///
    /// # Errors
    /// Returns an error for an unknown session, illegal transition, invalid reason, or storage failure.
    pub async fn close_platform_session(
        &self,
        session_id: PlatformSessionId,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<PlatformSession, RuntimeError> {
        self.settle_expired_platform_sessions(now).await?;
        let current = self.load_platform_session(session_id)?;
        if current.kind == PlatformSessionKind::ComputerUse {
            // The manager lock waits for an already-authorized input batch to
            // finish, then prevents any later input before durable closure.
            self.computer_sessions.close_if_attached(session_id).await?;
        }
        let (session, _) = self.store()?.transition_platform_session(
            session_id,
            PlatformSessionState::Closed,
            reason,
            now,
        )?;
        if current.kind == PlatformSessionKind::Browser {
            self.browser_sessions.close_if_attached(session_id).await?;
        }
        Ok(session)
    }

    async fn settle_expired_platform_sessions(
        &self,
        now: DateTime<Utc>,
    ) -> Result<(), RuntimeError> {
        let expired = self.store()?.expire_platform_session_ids(now)?;
        for session_id in expired {
            self.browser_sessions.close_if_attached(session_id).await?;
            self.computer_sessions.close_if_attached(session_id).await?;
        }
        Ok(())
    }

    fn load_platform_session(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<PlatformSession, RuntimeError> {
        self.store()?
            .platform_session(session_id)?
            .ok_or_else(|| StorageError::PlatformSessionNotFound(session_id).into())
    }

    /// Persists a task and publishes its committed event.
    ///
    /// # Errors
    ///
    /// Returns an error without publishing when persistence fails.
    pub fn create_task(&self, task: &TaskEnvelope) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.create_task(task)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Persists a task transition and publishes its committed event.
    ///
    /// # Errors
    ///
    /// Returns an error without publishing when validation or persistence fails.
    pub fn transition_task(
        &self,
        task_id: TaskId,
        state: TaskState,
    ) -> Result<TaskEnvelope, RuntimeError> {
        if state == TaskState::Cancelled
            && let Some((task, events)) = self.store()?.cancel_launched_task(task_id)?
        {
            for event in events {
                let _ = self.event_sender.send(event);
            }
            return Ok(task);
        }
        let (task, event) = self.store()?.transition_task(task_id, state)?;
        let _ = self.event_sender.send(event);
        Ok(task)
    }

    /// Loads a task projection.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn task(&self, task_id: TaskId) -> Result<Option<TaskEnvelope>, RuntimeError> {
        Ok(self.store()?.load_task(task_id)?)
    }

    /// Returns the durable consumer run associated with a task, when launched.
    ///
    /// # Errors
    ///
    /// Returns an error when launch persistence cannot be read.
    pub fn task_launch_run_id(&self, task_id: TaskId) -> Result<Option<RunId>, RuntimeError> {
        Ok(self.store()?.task_launch_run_id(task_id)?)
    }

    /// Lists current durable task projections in deterministic display order.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn tasks(&self) -> Result<Vec<TaskEnvelope>, RuntimeError> {
        Ok(self.store()?.tasks()?)
    }

    /// Persists a validated unattended-work schedule.
    ///
    /// # Errors
    /// Returns an error when durable storage rejects or cannot persist the definition.
    pub fn create_schedule(&self, schedule: &ScheduleDefinition) -> Result<(), RuntimeError> {
        self.store()?.create_schedule(schedule)?;
        Ok(())
    }

    /// Loads one durable schedule.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn schedule(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Option<ScheduleDefinition>, RuntimeError> {
        Ok(self.store()?.schedule(schedule_id)?)
    }

    /// Lists all durable schedules in stable creation order.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn schedules(&self) -> Result<Vec<ScheduleDefinition>, RuntimeError> {
        Ok(self.store()?.schedules()?)
    }

    /// Returns the greatest committed event cursor for new event-trigger definitions.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn latest_event_sequence(&self) -> Result<u64, RuntimeError> {
        Ok(self.store()?.latest_event_sequence()?)
    }

    /// Wakes event-trigger definitions for newly committed matching events.
    ///
    /// # Errors
    /// Returns an error when durable definitions or events cannot be reconciled.
    pub fn poll_event_schedules(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduleId>, RuntimeError> {
        Ok(self.store()?.poll_event_schedules(now)?)
    }

    /// Polls due project-confined file watches and persists changed fingerprints.
    ///
    /// # Errors
    /// Returns an error for invalid project/watch paths or unreadable durable state.
    pub fn poll_file_watches(&self, now: DateTime<Utc>) -> Result<Vec<ScheduleId>, RuntimeError> {
        let schedules = self.schedules()?;
        let mut changed = Vec::new();
        for schedule in schedules {
            let ScheduleTrigger::FileWatch {
                relative_path,
                poll_seconds,
            } = &schedule.trigger
            else {
                continue;
            };
            let poll_due = schedule.last_watch_at.is_none_or(|last| {
                i64::try_from(*poll_seconds)
                    .ok()
                    .and_then(|seconds| last.checked_add_signed(Duration::seconds(seconds)))
                    .is_some_and(|next| now >= next)
            });
            if schedule.state != ScheduleState::Enabled || !poll_due {
                continue;
            }
            let project = self
                .project(schedule.project_id)?
                .ok_or(StorageError::ProjectNotFound(schedule.project_id))?;
            let path = objective_project_file_path(&project.root, relative_path)?;
            let fingerprint = watch_file_fingerprint(Path::new(&project.root), Path::new(&path))?;
            if self
                .store()?
                .record_watch_observation(schedule.id, fingerprint, now)?
            {
                changed.push(schedule.id);
            }
        }
        Ok(changed)
    }

    /// Records a subsystem wake request without overriding an owner pause.
    ///
    /// # Errors
    /// Returns an error for missing schedules, empty reasons, or persistence failures.
    pub fn wake_schedule(
        &self,
        schedule_id: ScheduleId,
        severity: NotificationSeverity,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<ScheduleWakeMutation, RuntimeError> {
        Ok(self
            .store()?
            .wake_schedule(schedule_id, severity, reason, now)?)
    }

    /// Lists scheduler notifications.
    ///
    /// # Errors
    /// Returns an error when durable notifications cannot be read.
    pub fn scheduler_notifications(
        &self,
        include_read: bool,
    ) -> Result<Vec<SchedulerNotification>, RuntimeError> {
        Ok(self.store()?.scheduler_notifications(include_read)?)
    }

    /// Acknowledges one scheduler notification idempotently.
    ///
    /// # Errors
    /// Returns an error when the notification is missing or cannot be updated.
    pub fn acknowledge_scheduler_notification(
        &self,
        notification_id: SchedulerNotificationId,
        now: DateTime<Utc>,
    ) -> Result<SchedulerNotification, RuntimeError> {
        Ok(self
            .store()?
            .acknowledge_scheduler_notification(notification_id, now)?)
    }

    /// Lists the immutable execution history associated with one schedule.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn schedule_occurrences(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Vec<ScheduleOccurrence>, RuntimeError> {
        Ok(self.store()?.schedule_occurrences(schedule_id)?)
    }

    /// Changes whether a durable schedule may claim new work.
    ///
    /// # Errors
    /// Returns an error for invalid state changes or persistence failures.
    pub fn set_schedule_state(
        &self,
        schedule_id: ScheduleId,
        state: ScheduleState,
        now: DateTime<Utc>,
    ) -> Result<ScheduleDefinition, RuntimeError> {
        Ok(self.store()?.set_schedule_state(schedule_id, state, now)?)
    }

    /// Atomically claims either expired scheduled work or the next due occurrence.
    ///
    /// # Errors
    /// Returns an error for invalid lease bounds or persistence failures.
    pub fn claim_due_schedule(
        &self,
        now: DateTime<Utc>,
        lease_owner: &str,
        lease_seconds: i64,
    ) -> Result<Option<ScheduleClaim>, RuntimeError> {
        let claim = self
            .store()?
            .claim_due_schedule(now, lease_owner, lease_seconds)?;
        if let Some(event) = claim
            .as_ref()
            .and_then(|claim| claim.task_created_event.as_ref())
        {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(claim)
    }

    /// Settles approval-waiting scheduler history after its task reaches a terminal state.
    ///
    /// # Errors
    /// Returns an error when durable state cannot be read or updated atomically.
    pub fn reconcile_schedule_boundaries(&self, now: DateTime<Utc>) -> Result<u64, RuntimeError> {
        Ok(self.store()?.reconcile_schedule_boundaries(now)?)
    }

    /// Settles a claimed occurrence while enforcing its lease authority.
    ///
    /// # Errors
    /// Returns an error for lost authority, invalid state, or persistence failures.
    pub fn finish_schedule_occurrence(
        &self,
        occurrence_id: ScheduleOccurrenceId,
        lease_owner: &str,
        state: ScheduleOccurrenceState,
        detail: String,
        now: DateTime<Utc>,
    ) -> Result<ScheduleOccurrence, RuntimeError> {
        Ok(self.store()?.finish_schedule_occurrence(
            occurrence_id,
            lease_owner,
            state,
            detail,
            now,
        )?)
    }

    /// Settles a typed Site-monitor occurrence without a generic per-run notification.
    ///
    /// # Errors
    /// Returns an error for lost authority or invalid durable state.
    pub fn finish_site_monitor_occurrence(
        &self,
        occurrence_id: ScheduleOccurrenceId,
        lease_owner: &str,
        state: ScheduleOccurrenceState,
        detail: String,
        check: Option<&SiteHealthCheck>,
        now: DateTime<Utc>,
    ) -> Result<ScheduleOccurrence, RuntimeError> {
        Ok(self.store()?.finish_site_monitor_occurrence(
            occurrence_id,
            lease_owner,
            state,
            detail,
            check,
            now,
        )?)
    }

    /// Loads a task's ordered event history.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn task_events(&self, task_id: TaskId) -> Result<Vec<EventEnvelope>, RuntimeError> {
        Ok(self.store()?.events_for_task(task_id)?)
    }

    /// Loads durable events after a global sequence for stream catch-up.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn events_after(&self, sequence: u64) -> Result<Vec<EventEnvelope>, RuntimeError> {
        Ok(self.store()?.events_after(sequence)?)
    }

    /// Discovers a project from its canonical repository root and durably records
    /// the snapshot before publishing its event.
    ///
    /// # Errors
    ///
    /// Returns an error for inaccessible roots, invalid manifests, root identity
    /// conflicts, or failed persistence.
    pub fn discover_project(
        &self,
        root: &Path,
        project_id: ProjectId,
    ) -> Result<ProjectMutation, RuntimeError> {
        let previous = self.store()?.project(project_id)?;
        let project = ProjectDiscoverer::discover(root, project_id)?;
        let invalidation = previous
            .as_ref()
            .map(|previous| SnapshotInvalidation::between(previous, &project).into());
        let event = self.store()?.record_project_snapshot(&project)?;
        let _ = self.event_sender.send(event.clone());
        Ok(ProjectMutation {
            project,
            event,
            invalidation,
        })
    }

    /// Lists current durable project projections.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn projects(&self) -> Result<Vec<ProjectModel>, RuntimeError> {
        Ok(self.store()?.projects()?)
    }

    /// Loads one current durable project projection.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn project(&self, project_id: ProjectId) -> Result<Option<ProjectModel>, RuntimeError> {
        Ok(self.store()?.project(project_id)?)
    }

    /// Loads immutable discovery history for a project.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn project_history(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectModel>, RuntimeError> {
        Ok(self.store()?.project_history(project_id)?)
    }

    /// Records an immutable candidate organization revision without activating it.
    ///
    /// # Errors
    /// Returns an error for stale parents, invalid revisions, or identity conflicts.
    pub fn record_candidate_revision(
        &self,
        candidate: &OrganizationGenome,
    ) -> Result<CandidateRevisionMutation, RuntimeError> {
        let mutation = self
            .store()?
            .record_candidate_organization_revision(candidate)?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(mutation)
    }

    /// Records one immutable supervision observation.
    ///
    /// # Errors
    /// Returns an error for malformed evidence or conflicting identity reuse.
    pub fn record_supervision_observation(
        &self,
        observation: &SupervisionObservation,
    ) -> Result<SupervisionMutation, RuntimeError> {
        let mut store = self.store()?;
        let cursor = store.latest_event_sequence()?;
        let replayed = store.record_supervision_observation(observation)?;
        let events = if replayed {
            Vec::new()
        } else {
            store.events_after(cursor)?
        };
        drop(store);
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(SupervisionMutation {
            observation: observation.clone(),
            replayed,
            events,
        })
    }

    /// Lists immutable supervision observations.
    ///
    /// # Errors
    /// Returns an error when durable evidence cannot be decoded.
    pub fn supervision_observations(&self) -> Result<Vec<SupervisionObservation>, RuntimeError> {
        Ok(self.store()?.supervision_observations()?)
    }

    /// Creates an experiment over an inactive candidate revision.
    ///
    /// # Errors
    /// Returns an error for stale baselines, unknown evidence, or invalid policy.
    pub fn create_experiment(
        &self,
        experiment: &ExperimentDefinition,
    ) -> Result<ExperimentMutation, RuntimeError> {
        let mutation = self.store()?.create_experiment(experiment)?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(mutation)
    }

    /// Lists experiments for one organization.
    ///
    /// # Errors
    /// Returns an error when durable projections cannot be decoded.
    pub fn experiments(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<ExperimentDefinition>, RuntimeError> {
        Ok(self.store()?.experiments(organization_id)?)
    }

    /// Loads one experiment.
    ///
    /// # Errors
    /// Returns an error when durable projections cannot be decoded.
    pub fn experiment(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<Option<ExperimentDefinition>, RuntimeError> {
        Ok(self.store()?.experiment(experiment_id)?)
    }

    /// Applies one compare-and-set experiment transition.
    ///
    /// # Errors
    /// Returns an error for stale or illegal transitions.
    pub fn transition_experiment(
        &self,
        experiment_id: ExperimentId,
        expected: ExperimentState,
        next: ExperimentState,
    ) -> Result<ExperimentMutation, RuntimeError> {
        let mutation =
            self.store()?
                .transition_experiment(experiment_id, expected, next, Utc::now())?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(mutation)
    }

    /// Records evaluation-only shadow execution evidence.
    ///
    /// # Errors
    /// Returns an error unless the matching shadow experiment is running.
    pub fn record_shadow_execution(
        &self,
        receipt: &ShadowExecutionReceipt,
    ) -> Result<EvidenceMutation<ShadowExecutionReceipt>, RuntimeError> {
        let mut store = self.store()?;
        let cursor = store.latest_event_sequence()?;
        let replayed = store.record_shadow_execution(receipt)?;
        let events = if replayed {
            Vec::new()
        } else {
            store.events_after(cursor)?
        };
        drop(store);
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(EvidenceMutation {
            evidence: receipt.clone(),
            replayed,
            events,
        })
    }

    /// Lists evaluation-only shadow receipts for an experiment.
    ///
    /// # Errors
    /// Returns an error when durable evidence cannot be decoded.
    pub fn shadow_executions(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<Vec<ShadowExecutionReceipt>, RuntimeError> {
        Ok(self.store()?.shadow_executions(experiment_id)?)
    }

    /// Records an immutable paired benchmark.
    ///
    /// # Errors
    /// Returns an error unless both arms match the running experiment.
    pub fn record_experiment_benchmark(
        &self,
        benchmark: &ExperimentBenchmark,
    ) -> Result<EvidenceMutation<ExperimentBenchmark>, RuntimeError> {
        let mut store = self.store()?;
        let cursor = store.latest_event_sequence()?;
        let replayed = store.record_experiment_benchmark(benchmark)?;
        let events = if replayed {
            Vec::new()
        } else {
            store.events_after(cursor)?
        };
        drop(store);
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(EvidenceMutation {
            evidence: benchmark.clone(),
            replayed,
            events,
        })
    }

    /// Lists paired benchmark evidence for an experiment.
    ///
    /// # Errors
    /// Returns an error when durable evidence cannot be decoded.
    pub fn experiment_benchmarks(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<Vec<ExperimentBenchmark>, RuntimeError> {
        Ok(self.store()?.experiment_benchmarks(experiment_id)?)
    }

    /// Atomically records a measured experiment decision and any head change.
    ///
    /// # Errors
    /// Returns an error for stale direction, insufficient evidence, or policy failure.
    pub fn decide_experiment(
        &self,
        decision: &ExperimentDecision,
    ) -> Result<ExperimentMutation, RuntimeError> {
        let mutation = self.store()?.decide_experiment(decision)?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(mutation)
    }

    /// Lists immutable decisions for an experiment.
    ///
    /// # Errors
    /// Returns an error when durable evidence cannot be decoded.
    pub fn experiment_decisions(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<Vec<ExperimentDecision>, RuntimeError> {
        Ok(self.store()?.experiment_decisions(experiment_id)?)
    }

    /// Registers one durable project-bound Site with an immutable first revision.
    /// Exact caller identity replay returns the existing definition.
    ///
    /// # Errors
    /// Returns an error for missing projects, unsafe definitions, or identity conflicts.
    pub fn register_site(
        &self,
        request: &SiteRegistrationRequest,
    ) -> Result<SiteMutation, RuntimeError> {
        if request.state == SiteState::Archived {
            return Err(StorageError::InvalidData(
                "new Sites cannot be registered as archived".into(),
            )
            .into());
        }
        let now = Utc::now();
        let public_url = normalized_site_url(&request.public_url)?;
        let health_url = normalized_site_url(&request.health_url)?;
        let definition_hash = site_definition_hash(request, &public_url, &health_url)?;
        let site = Site {
            schema_version: SITE_SCHEMA_VERSION,
            id: request.id,
            definition_hash: definition_hash.clone(),
            revision: 1,
            project_id: request.project_id,
            display_name: request.display_name.trim().into(),
            environment: request.environment,
            public_url,
            health_url,
            hosting_provider: request.hosting_provider.trim().into(),
            deployment_command: request.deployment_command.clone(),
            credential_reference: request
                .credential_reference
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            state: request.state,
            created_at: now,
            updated_at: now,
        };
        let revision = SiteRevision {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteRevisionId::new(),
            site_id: site.id,
            revision: site.revision,
            definition_hash,
            site: site.clone(),
            reason: "owner registered site".into(),
            created_at: now,
        };
        let (site, created) = self.store()?.create_site(&site, &revision)?;
        Ok(SiteMutation {
            site,
            replayed: !created,
        })
    }

    /// Lists current Site projections.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn sites(&self) -> Result<Vec<Site>, RuntimeError> {
        Ok(self.store()?.sites()?)
    }

    /// Loads one current Site projection.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn site(&self, site_id: SiteId) -> Result<Option<Site>, RuntimeError> {
        Ok(self.store()?.site(site_id)?)
    }

    /// Atomically advances one Site definition and records an immutable revision.
    /// An exact retry after a lost response replays the already committed revision.
    ///
    /// # Errors
    /// Returns an error for stale revisions, unsafe definitions, or archived Sites.
    pub fn update_site(
        &self,
        site_id: SiteId,
        request: &SiteUpdateRequest,
    ) -> Result<SiteMutation, RuntimeError> {
        let current = self
            .site(site_id)?
            .ok_or(StorageError::SiteNotFound(site_id))?;
        if current.state == SiteState::Archived {
            return Err(
                StorageError::InvalidData("archived Sites cannot be changed".into()).into(),
            );
        }
        if request.expected_revision == 0 || request.reason.trim().is_empty() {
            return Err(StorageError::InvalidData(
                "Site updates require a positive expected revision and reason".into(),
            )
            .into());
        }
        let public_url = normalized_site_url(&request.public_url)?;
        let health_url = normalized_site_url(&request.health_url)?;
        let definition = SiteRegistrationRequest {
            id: site_id,
            project_id: current.project_id,
            display_name: request.display_name.clone(),
            environment: request.environment,
            public_url: public_url.clone(),
            health_url: health_url.clone(),
            hosting_provider: request.hosting_provider.clone(),
            deployment_command: request.deployment_command.clone(),
            credential_reference: request.credential_reference.clone(),
            state: request.state,
        };
        let definition_hash = site_definition_hash(&definition, &public_url, &health_url)?;
        if current.revision == request.expected_revision.saturating_add(1)
            && current.definition_hash == definition_hash
        {
            return Ok(SiteMutation {
                site: current,
                replayed: true,
            });
        }
        if current.revision != request.expected_revision {
            return Err(StorageError::Conflict(format!(
                "site {site_id} changed from expected revision {}",
                request.expected_revision
            ))
            .into());
        }
        let now = Utc::now();
        let site = Site {
            schema_version: SITE_SCHEMA_VERSION,
            id: site_id,
            definition_hash: definition_hash.clone(),
            revision: current.revision.saturating_add(1),
            project_id: current.project_id,
            display_name: request.display_name.trim().into(),
            environment: request.environment,
            public_url,
            health_url,
            hosting_provider: request.hosting_provider.trim().into(),
            deployment_command: request.deployment_command.clone(),
            credential_reference: request
                .credential_reference
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            state: request.state,
            created_at: current.created_at,
            updated_at: now,
        };
        let revision = SiteRevision {
            schema_version: SITE_SCHEMA_VERSION,
            id: SiteRevisionId::new(),
            site_id,
            revision: site.revision,
            definition_hash,
            site: site.clone(),
            reason: request.reason.trim().into(),
            created_at: now,
        };
        let site = self
            .store()?
            .update_site(request.expected_revision, &site, &revision)?;
        Ok(SiteMutation {
            site,
            replayed: false,
        })
    }

    /// Loads immutable definition history for one Site.
    ///
    /// # Errors
    /// Returns not-found for an unknown Site or an error when storage cannot be read.
    pub fn site_revisions(&self, site_id: SiteId) -> Result<Vec<SiteRevision>, RuntimeError> {
        Ok(self.store()?.site_revisions(site_id)?)
    }

    /// Runs and durably records one bounded same-origin Site health check.
    /// Exact check identity replay performs no network work.
    ///
    /// # Errors
    /// Returns an error for unknown/archived Sites, conflicting identity, or authority failure.
    #[allow(clippy::too_many_lines)] // Keeps one fail-closed health authority and evidence transaction.
    pub async fn check_site_health(
        &self,
        site_id: SiteId,
        check_id: SiteHealthCheckId,
    ) -> Result<SiteHealthMutation, RuntimeError> {
        let site = self
            .site(site_id)?
            .ok_or(StorageError::SiteNotFound(site_id))?;
        if site.state == SiteState::Archived {
            return Err(ToolError::Denied("archived sites cannot be checked".into()).into());
        }
        let request_hash = site_health_request_hash(&site)?;
        if let Some(existing) = self.store()?.site_health_check(check_id)? {
            if existing.site_id != site_id || existing.request_hash != request_hash {
                return Err(StorageError::Conflict(
                    "site health identity was reused with a different request".into(),
                )
                .into());
            }
            return Ok(SiteHealthMutation {
                check: existing,
                replayed: true,
            });
        }
        let project = self
            .project(site.project_id)?
            .ok_or(StorageError::ProjectNotFound(site.project_id))?;
        let root = std::path::PathBuf::from(project.root);
        let artifacts = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifacts).map_err(runtime_tool_error)?,
        );
        let http = HttpExecutor::new(
            Arc::new(RejectingCredentialResolver),
            filesystem,
            &artifacts,
        )
        .map_err(runtime_tool_error)?;
        let task_id = site_authority_task_id(site_id);
        let mut tool_request = ToolRequest::new(
            task_id,
            format!("Check Site health: {}", site.display_name),
            ToolRequestKind::HttpRequest {
                method: "GET".into(),
                url: site.health_url.clone(),
                headers: Vec::new(),
                body: None,
                max_response_bytes: 1_048_576,
            },
        );
        tool_request.id = site_health_tool_request_id(check_id);
        tool_request.timeout_seconds = 30;
        let mut capabilities = CapabilityPolicy::project(&root);
        capabilities
            .allowed_network_origins
            .push(objective_network_origin(&site.health_url)?);
        let control = self.tool_control();
        let broker = ToolBroker::new(
            http,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let started = Instant::now();
        let execution = broker.execute(&tool_request).await;
        let latency_milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let checked_at = Utc::now();
        let check = match execution {
            Ok(outcome) => {
                let status_code = outcome
                    .verification
                    .iter()
                    .find(|evidence| evidence.method == "http.status")
                    .and_then(|evidence| evidence.detail.parse::<u16>().ok());
                let state = if status_code.is_some_and(|status| (200..400).contains(&status)) {
                    SiteHealthState::Healthy
                } else {
                    SiteHealthState::Degraded
                };
                SiteHealthCheck {
                    schema_version: SITE_SCHEMA_VERSION,
                    id: check_id,
                    site_id,
                    request_hash,
                    state,
                    status_code,
                    latency_milliseconds,
                    detail: bounded_site_detail(&outcome.summary),
                    response_artifact_sha256: outcome
                        .artifacts
                        .first()
                        .map(|artifact| artifact.sha256.clone()),
                    checked_at,
                }
            }
            Err(error) => SiteHealthCheck {
                schema_version: SITE_SCHEMA_VERSION,
                id: check_id,
                site_id,
                request_hash,
                state: SiteHealthState::Unavailable,
                status_code: None,
                latency_milliseconds,
                detail: bounded_site_detail(&format!("health request failed: {error}")),
                response_artifact_sha256: None,
                checked_at,
            },
        };
        let (check, created) = self.store()?.record_site_health_check(&check)?;
        Ok(SiteHealthMutation {
            check,
            replayed: !created,
        })
    }

    /// Lists immutable health observations for one Site.
    ///
    /// # Errors
    /// Returns not-found for an unknown Site or an error when storage cannot be read.
    pub fn site_health_checks(
        &self,
        site_id: SiteId,
    ) -> Result<Vec<SiteHealthCheck>, RuntimeError> {
        Ok(self.store()?.site_health_checks(site_id)?)
    }

    /// Creates one durable scheduler-backed monitor for a Site.
    ///
    /// # Errors
    /// Returns an error for an archived Site, invalid interval/status, or identity conflict.
    pub fn create_site_monitor(
        &self,
        site_id: SiteId,
        request: &SiteMonitorRequest,
    ) -> Result<SiteMonitorMutation, RuntimeError> {
        let site = self
            .site(site_id)?
            .ok_or(StorageError::SiteNotFound(site_id))?;
        let now = Utc::now();
        let definition_hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(site_id, request)).map_err(StorageError::from)?,)
        );
        let schedule = ScheduleDefinition {
            schema_version: mcg_domain::SCHEDULE_SCHEMA_VERSION,
            id: request.schedule_id,
            definition_hash,
            name: format!("Monitor {}", site.display_name),
            project_id: site.project_id,
            goal: format!("Monitor Site {} health without model inference", site.id),
            constraints: vec!["Typed Site health executor only".into()],
            success_conditions: vec![format!(
                "HTTP {} {}",
                request.expected_status, site.health_url
            )],
            risk: RiskLevel::Low,
            budget: TaskBudget {
                max_tokens: None,
                max_cost_usd: None,
                max_duration_seconds: Some(60),
            },
            trigger: ScheduleTrigger::Interval {
                seconds: request.interval_seconds,
            },
            state: if site.state == SiteState::Active {
                ScheduleState::Enabled
            } else {
                ScheduleState::Paused
            },
            next_run_at: Some(request.first_run_at.unwrap_or(now)),
            target_path: None,
            context_token_budget: 1,
            requested_tools: vec!["http.request".into()],
            estimated_output_tokens: 0,
            max_model_calls: 1,
            watch_fingerprint: None,
            last_watch_at: None,
            last_event_sequence: 0,
            created_at: now,
            updated_at: now,
        };
        let monitor = SiteMonitor {
            schema_version: SITE_SCHEMA_VERSION,
            site_id,
            schedule_id: request.schedule_id,
            interval_seconds: request.interval_seconds,
            expected_status: request.expected_status,
            last_state: None,
            created_at: now,
            updated_at: now,
        };
        let (monitor, created) = self.store()?.create_site_monitor(&monitor, &schedule)?;
        let schedule = self
            .schedule(monitor.schedule_id)?
            .ok_or(StorageError::ScheduleNotFound(monitor.schedule_id))?;
        Ok(SiteMonitorMutation {
            monitor,
            schedule,
            replayed: !created,
        })
    }

    /// Loads the scheduler-backed monitor for one Site.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn site_monitor(&self, site_id: SiteId) -> Result<Option<SiteMonitor>, RuntimeError> {
        Ok(self.store()?.site_monitor(site_id)?)
    }

    /// Loads the Site monitor owning one schedule.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn site_monitor_by_schedule(
        &self,
        schedule_id: ScheduleId,
    ) -> Result<Option<SiteMonitor>, RuntimeError> {
        Ok(self.store()?.site_monitor_by_schedule(schedule_id)?)
    }

    /// Executes one exact revision-bound Site deployment through publication policy.
    /// Exact terminal retries are observation-only.
    ///
    /// # Errors
    /// Returns an error for missing authority, approval, unsupported credentials,
    /// conflicting identity, process failure, or uncertain prior execution.
    #[allow(clippy::too_many_lines)] // Keeps deployment preflight and broker construction together.
    pub async fn deploy_site(
        &self,
        site_id: SiteId,
        deployment_id: SiteDeploymentId,
        task_id: Option<TaskId>,
    ) -> Result<SiteDeploymentMutation, RuntimeError> {
        let site = self
            .site(site_id)?
            .ok_or(StorageError::SiteNotFound(site_id))?;
        let request_hash = site_deployment_request_hash(&site, task_id)?;
        if let Some(existing) = self.store()?.site_deployment(deployment_id)? {
            if existing.site_id != site_id
                || existing.site_revision != site.revision
                || existing.task_id != task_id
                || existing.request_hash != request_hash
            {
                return Err(StorageError::Conflict(
                    "site deployment identity was reused with a different request".into(),
                )
                .into());
            }
            return settled_site_deployment_replay(existing);
        }
        if site.state != SiteState::Active {
            return Err(ToolError::Denied("only active sites can be deployed".into()).into());
        }
        if site.credential_reference.is_some() {
            return Err(ToolError::Unavailable(
                "this Site credential binding has no installed deployment adapter".into(),
            )
            .into());
        }
        let command = site.deployment_command.as_ref().ok_or_else(|| {
            ToolError::Unavailable("the Site has no structured deployment command".into())
        })?;
        let project = self
            .project(site.project_id)?
            .ok_or(StorageError::ProjectNotFound(site.project_id))?;
        if let Some(task_id) = task_id {
            let task = self
                .task(task_id)?
                .ok_or(StorageError::TaskNotFound(task_id))?;
            if task.project_id != Some(site.project_id) {
                return Err(ToolError::Denied(
                    "deployment task is not bound to the Site project".into(),
                )
                .into());
            }
        }
        let root = std::path::PathBuf::from(&project.root);
        let cwd = if command.working_directory.is_empty() {
            root.clone()
        } else {
            root.join(&command.working_directory)
        };
        let options = ProcessOptions {
            executable: command.executable.clone(),
            argv: command.arguments.clone(),
            cwd: cwd.to_string_lossy().into_owned(),
            env: Vec::new(),
            stdin: ProcessStream::Null,
            stdin_data: None,
            stdout: ProcessStream::Piped,
            stderr: ProcessStream::Piped,
        };
        let authority_task_id = task_id.unwrap_or_else(|| site_authority_task_id(site_id));
        let mut request = ToolRequest::new(
            authority_task_id,
            format!(
                "Deploy {} {} revision {} using exact structured command",
                site.display_name,
                site_environment_label(site.environment),
                site.revision
            ),
            ToolRequestKind::SiteDeploy {
                deployment_id,
                site_id,
                site_revision: site.revision,
                task_id,
                options: options.clone(),
            },
        );
        request.id = site_deployment_tool_request_id(deployment_id);
        request.timeout_seconds = 1_800;
        let artifacts_path = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts_path)?;
        let result = Arc::new(Mutex::new(None));
        let executor = SiteDeploymentExecutor {
            store: self.store.clone(),
            process: Arc::new(
                ProcessExecutor::new([root.clone()], Arc::new(RejectingCredentialResolver))
                    .map_err(runtime_tool_error)?,
            ),
            artifacts: Arc::new(ArtifactStore::open(&artifacts_path).map_err(runtime_tool_error)?),
            result: result.clone(),
        };
        let site_resource = site_revision_resource(site_id, site.revision);
        let mut capabilities = CapabilityPolicy::project(&root);
        capabilities.allowed_sites.push(site_resource);
        let executable = platform_process_executable(&command.executable);
        if !capabilities
            .allowed_executables
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&executable))
        {
            capabilities.allowed_executables.push(executable);
        }
        let control = self.tool_control();
        let broker = ToolBroker::new(
            executor,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        broker.execute(&request).await?;
        result
            .lock()
            .map_err(|_| RuntimeError::StorageLock)?
            .take()
            .ok_or_else(|| {
                ToolError::Execution("Site deployment executor returned no mutation".into()).into()
            })
    }

    /// Lists durable deployment history for one Site.
    ///
    /// # Errors
    /// Returns not-found for an unknown Site or an error when storage cannot be read.
    pub fn site_deployments(&self, site_id: SiteId) -> Result<Vec<SiteDeployment>, RuntimeError> {
        Ok(self.store()?.site_deployments(site_id)?)
    }

    /// Registers one host with explicit pinned OpenSSH host-key material.
    /// Exact caller identity replay returns the existing definition.
    ///
    /// # Errors
    /// Returns an error for invalid definitions or identity conflicts.
    pub fn register_ssh_host(
        &self,
        request: &SshHostRegistrationRequest,
    ) -> Result<SshHostMutation, RuntimeError> {
        if request.state == SshHostState::Archived {
            return Err(StorageError::InvalidData(
                "new SSH hosts cannot be registered as archived".into(),
            )
            .into());
        }
        let now = Utc::now();
        let capabilities = normalize_ssh_capabilities(&request.capabilities)?;
        let definition_hash = ssh_host_definition_hash(request)?;
        let host = SshHost {
            schema_version: SSH_SCHEMA_VERSION,
            id: request.id,
            definition_hash: definition_hash.clone(),
            revision: 1,
            display_name: request.display_name.trim().into(),
            hostname: request.hostname.trim().into(),
            port: request.port,
            username: request.username.trim().into(),
            platform: request.platform,
            authentication: request.authentication,
            known_hosts_entry: request.known_hosts_entry.trim().into(),
            host_key_sha256: request.host_key_sha256.trim().into(),
            capabilities,
            state: request.state,
            created_at: now,
            updated_at: now,
        };
        let revision = SshHostRevision {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostRevisionId::new(),
            host_id: host.id,
            revision: 1,
            definition_hash,
            host: host.clone(),
            reason: "owner registered SSH host".into(),
            created_at: now,
        };
        let (host, created) = self.store()?.create_ssh_host(&host, &revision)?;
        Ok(SshHostMutation {
            host,
            replayed: !created,
        })
    }

    /// Lists current SSH host projections.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_hosts(&self) -> Result<Vec<SshHost>, RuntimeError> {
        Ok(self.store()?.ssh_hosts()?)
    }

    /// Loads one current SSH host projection.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_host(&self, host_id: SshHostId) -> Result<Option<SshHost>, RuntimeError> {
        Ok(self.store()?.ssh_host(host_id)?)
    }

    /// Advances a pinned host definition with optimistic revision authority.
    /// Exact lost-response retry is observation-only; archive is terminal.
    ///
    /// # Errors
    /// Returns an error for stale revisions, invalid keys, or archived hosts.
    pub fn update_ssh_host(
        &self,
        host_id: SshHostId,
        request: &SshHostUpdateRequest,
    ) -> Result<SshHostMutation, RuntimeError> {
        let current = self
            .ssh_host(host_id)?
            .ok_or(StorageError::SshHostNotFound(host_id))?;
        if current.state == SshHostState::Archived
            || request.expected_revision == 0
            || request.reason.trim().is_empty()
        {
            return Err(StorageError::InvalidData(
                "SSH host update requires live authority, a revision, and a reason".into(),
            )
            .into());
        }
        let definition = SshHostRegistrationRequest {
            id: host_id,
            display_name: request.display_name.clone(),
            hostname: request.hostname.clone(),
            port: request.port,
            username: request.username.clone(),
            platform: request.platform,
            authentication: request.authentication,
            known_hosts_entry: request.known_hosts_entry.clone(),
            host_key_sha256: request.host_key_sha256.clone(),
            capabilities: request.capabilities.clone(),
            state: request.state,
        };
        let definition_hash = ssh_host_definition_hash(&definition)?;
        if current.revision == request.expected_revision.saturating_add(1)
            && current.definition_hash == definition_hash
        {
            return Ok(SshHostMutation {
                host: current,
                replayed: true,
            });
        }
        if current.revision != request.expected_revision {
            return Err(StorageError::Conflict(format!(
                "SSH host {host_id} changed from expected revision {}",
                request.expected_revision
            ))
            .into());
        }
        let now = Utc::now();
        let capabilities = normalize_ssh_capabilities(&request.capabilities)?;
        let host = SshHost {
            schema_version: SSH_SCHEMA_VERSION,
            id: host_id,
            definition_hash: definition_hash.clone(),
            revision: current.revision.saturating_add(1),
            display_name: request.display_name.trim().into(),
            hostname: request.hostname.trim().into(),
            port: request.port,
            username: request.username.trim().into(),
            platform: request.platform,
            authentication: request.authentication,
            known_hosts_entry: request.known_hosts_entry.trim().into(),
            host_key_sha256: request.host_key_sha256.trim().into(),
            capabilities,
            state: request.state,
            created_at: current.created_at,
            updated_at: now,
        };
        let revision = SshHostRevision {
            schema_version: SSH_SCHEMA_VERSION,
            id: SshHostRevisionId::new(),
            host_id,
            revision: host.revision,
            definition_hash,
            host: host.clone(),
            reason: request.reason.trim().into(),
            created_at: now,
        };
        let host = self
            .store()?
            .update_ssh_host(request.expected_revision, &host, &revision)?;
        Ok(SshHostMutation {
            host,
            replayed: false,
        })
    }

    /// Loads immutable definition history for one SSH host.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_host_revisions(
        &self,
        host_id: SshHostId,
    ) -> Result<Vec<SshHostRevision>, RuntimeError> {
        Ok(self.store()?.ssh_host_revisions(host_id)?)
    }

    /// Executes one caller-owned typed SSH operation through policy, approval, lease, and audit.
    /// Exact terminal retries are observation-only; uncertain work is never repeated.
    ///
    /// # Errors
    /// Returns an error for missing authority, approval, unavailable OpenSSH, or execution failure.
    #[allow(clippy::too_many_lines)] // Keeps remote authority, local roots, and broker setup together.
    pub async fn run_ssh_operation(
        &self,
        host_id: SshHostId,
        operation_id: SshOperationId,
        task_id: Option<TaskId>,
        kind: SshOperationKind,
        timeout_seconds: u64,
    ) -> Result<SshOperationMutation, RuntimeError> {
        let host = self
            .ssh_host(host_id)?
            .ok_or(StorageError::SshHostNotFound(host_id))?;
        let request_hash = ssh_operation_request_hash(&host, task_id, &kind, timeout_seconds)?;
        if let Some(existing) = self.store()?.ssh_operation(operation_id)? {
            if existing.host_id != host_id
                || existing.host_revision != host.revision
                || existing.task_id != task_id
                || existing.request_hash != request_hash
            {
                return Err(StorageError::Conflict(
                    "SSH operation identity was reused with a different request".into(),
                )
                .into());
            }
            return settled_ssh_operation_replay(existing);
        }
        if host.state != SshHostState::Active {
            return Err(ToolError::Denied("only active SSH hosts can execute work".into()).into());
        }
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(ToolError::Invalid("SSH timeout must be 1 to 3600 seconds".into()).into());
        }
        let task = if let Some(id) = task_id {
            Some(self.task(id)?.ok_or(StorageError::TaskNotFound(id))?)
        } else {
            None
        };
        let project_root = task
            .as_ref()
            .and_then(|task| task.project_id)
            .map(|project_id| self.project(project_id))
            .transpose()?
            .flatten()
            .map(|project| std::path::PathBuf::from(project.root));
        if matches!(
            kind,
            SshOperationKind::Upload { .. } | SshOperationKind::Download { .. }
        ) && project_root.is_none()
        {
            return Err(ToolError::Denied(
                "SSH file transfer requires a task bound to a durable project".into(),
            )
            .into());
        }
        let local_root = project_root
            .clone()
            .unwrap_or_else(|| self.config.data_directory.clone());
        let ssh = find_path_executable("ssh")
            .ok_or_else(|| ToolError::Unavailable("OpenSSH ssh executable was not found".into()))?;
        let scp = find_path_executable("scp")
            .ok_or_else(|| ToolError::Unavailable("OpenSSH scp executable was not found".into()))?;
        let scratch = self.config.data_directory.join("ssh-scratch");
        let artifacts_path = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts_path)?;
        let result = Arc::new(Mutex::new(None));
        let executor = SshOperationExecutor {
            store: self.store.clone(),
            ssh: Arc::new(
                SshExecutor::new(ssh, scp, scratch, [local_root.clone()])
                    .map_err(runtime_tool_error)?,
            ),
            artifacts: Arc::new(ArtifactStore::open(&artifacts_path).map_err(runtime_tool_error)?),
            request_hash,
            task_id,
            result: result.clone(),
        };
        let authority_task_id = task_id.unwrap_or_else(|| ssh_authority_task_id(host_id));
        let mut request = ToolRequest::new(
            authority_task_id,
            format!(
                "{} on pinned SSH host {}",
                ssh_kind_label(&kind),
                host.display_name
            ),
            ToolRequestKind::SshOperation {
                operation_id,
                host_id,
                host_revision: host.revision,
                hostname: host.hostname.clone(),
                port: host.port,
                username: host.username.clone(),
                known_hosts_entry: host.known_hosts_entry.clone(),
                host_key_sha256: host.host_key_sha256.clone(),
                kind: kind.clone(),
            },
        );
        request.id = ssh_operation_tool_request_id(operation_id);
        request.timeout_seconds = timeout_seconds;
        let mut capabilities = CapabilityPolicy::project(&local_root);
        capabilities
            .allowed_ssh_hosts
            .push(ssh_revision_resource(host_id, host.revision));
        let control = self.tool_control();
        let broker = ToolBroker::new(
            executor,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        broker.execute(&request).await?;
        result
            .lock()
            .map_err(|_| RuntimeError::StorageLock)?
            .take()
            .ok_or_else(|| {
                ToolError::Execution("SSH executor returned no durable mutation".into()).into()
            })
    }

    /// Lists retained SSH operation history for one host.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_operations(&self, host_id: SshHostId) -> Result<Vec<SshOperation>, RuntimeError> {
        Ok(self.store()?.ssh_operations(host_id)?)
    }

    /// Creates one immutable, dependency-ordered workflow over pinned SSH host revisions.
    /// Exact caller identity replay is observation-only.
    ///
    /// # Errors
    /// Returns an error for missing tasks/hosts, stale host pins, invalid graphs, or conflicts.
    #[allow(clippy::too_many_lines)] // Keeps canonical replay, routing, and durable graph creation together.
    pub fn create_ssh_workflow(
        &self,
        request: &SshWorkflowCreateRequest,
    ) -> Result<SshWorkflowMutation, RuntimeError> {
        let now = Utc::now();
        let definition_hash = ssh_workflow_definition_hash(request)?;
        if let Some(existing) = self.ssh_workflow(request.id)? {
            if existing.definition_hash == definition_hash {
                return Ok(SshWorkflowMutation {
                    workflow: existing,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(StorageError::Conflict(format!(
                "SSH workflow {} already exists with a different definition",
                request.id
            ))
            .into());
        }
        let hosts = self.ssh_hosts()?;
        let mut steps = Vec::with_capacity(request.steps.len());
        for step in &request.steps {
            let required_capabilities = normalize_ssh_capabilities(&step.required_capabilities)?;
            let (host, routed_automatically) = match (step.host_id, step.host_revision) {
                (Some(host_id), Some(host_revision)) => {
                    let host = hosts
                        .iter()
                        .find(|host| host.id == host_id && host.revision == host_revision)
                        .ok_or(StorageError::Conflict(format!(
                            "SSH workflow step {} pins an unavailable host revision",
                            step.id
                        )))?;
                    (host, false)
                }
                (None, None) if !required_capabilities.is_empty() => {
                    let host = hosts
                        .iter()
                        .filter(|host| host.state == SshHostState::Active)
                        .filter(|host| {
                            required_capabilities
                                .iter()
                                .all(|required| host.capabilities.binary_search(required).is_ok())
                        })
                        .min_by_key(|host| host.id)
                        .ok_or_else(|| {
                            StorageError::Conflict(format!(
                                "no active SSH host provides every capability required by step {}",
                                step.id
                            ))
                        })?;
                    (host, true)
                }
                _ => {
                    return Err(StorageError::InvalidData(format!(
                        "SSH workflow step {} must provide both host fields or capability requirements",
                        step.id
                    ))
                    .into());
                }
            };
            if host.state != SshHostState::Active
                || required_capabilities
                    .iter()
                    .any(|required| host.capabilities.binary_search(required).is_err())
            {
                return Err(StorageError::Conflict(format!(
                    "SSH workflow step {} cannot use a host missing required capabilities",
                    step.id
                ))
                .into());
            }
            steps.push(SshWorkflowStep {
                id: step.id,
                name: step.name.trim().into(),
                host_id: host.id,
                host_revision: host.revision,
                required_capabilities,
                routed_automatically,
                dependencies: step.dependencies.clone(),
                operation_id: ssh_workflow_operation_id(request.id, step.id),
                operation: step.operation.clone(),
                timeout_seconds: step.timeout_seconds,
                state: SshWorkflowStepState::Pending,
                detail: String::new(),
            });
        }
        let workflow = SshWorkflow {
            schema_version: SSH_WORKFLOW_SCHEMA_VERSION,
            id: request.id,
            definition_hash,
            display_name: request.display_name.trim().into(),
            task_id: request.task_id,
            state: SshWorkflowState::Planned,
            steps,
            created_at: now,
            updated_at: now,
        };
        let (workflow, created, events) = self.store()?.create_ssh_workflow(&workflow)?;
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(SshWorkflowMutation {
            workflow,
            replayed: !created,
            events,
        })
    }

    /// Lists all retained multi-host SSH workflows.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_workflows(&self) -> Result<Vec<SshWorkflow>, RuntimeError> {
        Ok(self.store()?.ssh_workflows()?)
    }

    /// Loads one retained multi-host SSH workflow.
    ///
    /// # Errors
    /// Returns an error when durable storage cannot be read.
    pub fn ssh_workflow(
        &self,
        workflow_id: SshWorkflowId,
    ) -> Result<Option<SshWorkflow>, RuntimeError> {
        Ok(self.store()?.ssh_workflow(workflow_id)?)
    }

    /// Advances every dependency-ready step through the ordinary typed SSH broker.
    /// The first approval/setup boundary pauses the workflow; failed or uncertain
    /// operations stop it without executing dependents.
    ///
    /// # Errors
    /// Returns an error for missing workflows or invalid/stale durable authority.
    #[allow(clippy::too_many_lines)] // Keeps the ordered durable workflow state machine auditable.
    pub async fn run_ssh_workflow(
        &self,
        workflow_id: SshWorkflowId,
    ) -> Result<SshWorkflowMutation, RuntimeError> {
        let mut workflow = self
            .ssh_workflow(workflow_id)?
            .ok_or(StorageError::SshWorkflowNotFound(workflow_id))?;
        if workflow.state.is_terminal() {
            return Ok(SshWorkflowMutation {
                workflow,
                replayed: true,
                events: Vec::new(),
            });
        }
        let mut events = Vec::new();
        if workflow.state != SshWorkflowState::Running {
            let previous = workflow.state;
            workflow.state = SshWorkflowState::Running;
            workflow.updated_at = Utc::now();
            events.push(self.commit_ssh_workflow(previous, &workflow)?);
        }

        loop {
            let Some(index) = workflow.steps.iter().position(|step| {
                step.state == SshWorkflowStepState::Pending
                    && step.dependencies.iter().all(|dependency| {
                        workflow.steps.iter().any(|candidate| {
                            candidate.id == *dependency
                                && candidate.state == SshWorkflowStepState::Succeeded
                        })
                    })
            }) else {
                workflow.state = if workflow
                    .steps
                    .iter()
                    .all(|step| step.state == SshWorkflowStepState::Succeeded)
                {
                    SshWorkflowState::Succeeded
                } else {
                    SshWorkflowState::Failed
                };
                let previous = SshWorkflowState::Running;
                workflow.updated_at = Utc::now();
                events.push(self.commit_ssh_workflow(previous, &workflow)?);
                return Ok(SshWorkflowMutation {
                    workflow,
                    replayed: false,
                    events,
                });
            };

            let step = workflow.steps[index].clone();
            let host = self
                .ssh_host(step.host_id)?
                .ok_or(StorageError::SshHostNotFound(step.host_id))?;
            if host.state != SshHostState::Active || host.revision != step.host_revision {
                workflow.steps[index].state = SshWorkflowStepState::Failed;
                workflow.steps[index].detail =
                    "pinned SSH host was disabled, archived, or revised".into();
                skip_ssh_workflow_dependents(&mut workflow, step.id);
                workflow.state = SshWorkflowState::Failed;
                workflow.updated_at = Utc::now();
                events.push(self.commit_ssh_workflow(SshWorkflowState::Running, &workflow)?);
                return Ok(SshWorkflowMutation {
                    workflow,
                    replayed: false,
                    events,
                });
            }

            match self
                .run_ssh_operation(
                    step.host_id,
                    step.operation_id,
                    workflow.task_id,
                    step.operation,
                    step.timeout_seconds,
                )
                .await
            {
                Ok(mutation) => {
                    workflow.steps[index].detail = mutation.operation.detail;
                    workflow.steps[index].state = match mutation.operation.state {
                        SshOperationState::Succeeded => SshWorkflowStepState::Succeeded,
                        SshOperationState::Failed => SshWorkflowStepState::Failed,
                        SshOperationState::Uncertain | SshOperationState::Executing => {
                            SshWorkflowStepState::Uncertain
                        }
                    };
                }
                Err(RuntimeError::Tool(ToolError::ApprovalRequired { .. })) => {
                    workflow.state = SshWorkflowState::Waiting;
                    workflow.steps[index].detail = "owner approval is required".into();
                    workflow.updated_at = Utc::now();
                    events.push(self.commit_ssh_workflow(SshWorkflowState::Running, &workflow)?);
                    return Ok(SshWorkflowMutation {
                        workflow,
                        replayed: false,
                        events,
                    });
                }
                Err(error) => {
                    let operation = self.store()?.ssh_operation(step.operation_id)?;
                    match operation {
                        Some(operation)
                            if matches!(
                                operation.state,
                                SshOperationState::Uncertain | SshOperationState::Executing
                            ) =>
                        {
                            workflow.steps[index].state = SshWorkflowStepState::Uncertain;
                            workflow.state = SshWorkflowState::Uncertain;
                            workflow.steps[index].detail = operation.detail;
                        }
                        Some(operation) => {
                            workflow.steps[index].detail = operation.detail;
                            workflow.steps[index].state =
                                if operation.state == SshOperationState::Succeeded {
                                    SshWorkflowStepState::Succeeded
                                } else {
                                    SshWorkflowStepState::Failed
                                };
                            workflow.state = if operation.state == SshOperationState::Succeeded {
                                SshWorkflowState::Running
                            } else {
                                SshWorkflowState::Failed
                            };
                        }
                        None if matches!(&error, RuntimeError::Tool(ToolError::Unavailable(_))) => {
                            workflow.state = SshWorkflowState::Waiting;
                            workflow.steps[index].detail = ssh_workflow_error_detail(&error);
                        }
                        None => {
                            workflow.steps[index].state = SshWorkflowStepState::Failed;
                            workflow.state = SshWorkflowState::Failed;
                            workflow.steps[index].detail = ssh_workflow_error_detail(&error);
                        }
                    }
                }
            }

            if matches!(
                workflow.steps[index].state,
                SshWorkflowStepState::Failed | SshWorkflowStepState::Uncertain
            ) {
                skip_ssh_workflow_dependents(&mut workflow, step.id);
            }
            workflow.updated_at = Utc::now();
            let next = workflow.state;
            events.push(self.commit_ssh_workflow(SshWorkflowState::Running, &workflow)?);
            if next != SshWorkflowState::Running {
                return Ok(SshWorkflowMutation {
                    workflow,
                    replayed: false,
                    events,
                });
            }
        }
    }

    /// Cancels a workflow that has no currently executing remote operation.
    ///
    /// # Errors
    /// Returns an error when the workflow is missing or remote authority is in flight.
    pub fn cancel_ssh_workflow(
        &self,
        workflow_id: SshWorkflowId,
    ) -> Result<SshWorkflowMutation, RuntimeError> {
        let mut workflow = self
            .ssh_workflow(workflow_id)?
            .ok_or(StorageError::SshWorkflowNotFound(workflow_id))?;
        if workflow.state.is_terminal() {
            return Ok(SshWorkflowMutation {
                workflow,
                replayed: true,
                events: Vec::new(),
            });
        }
        for step in &workflow.steps {
            if self
                .store()?
                .ssh_operation(step.operation_id)?
                .is_some_and(|operation| operation.state == SshOperationState::Executing)
            {
                return Err(StorageError::Conflict(
                    "cannot cancel while an SSH operation may still be executing".into(),
                )
                .into());
            }
        }
        let previous = workflow.state;
        for step in &mut workflow.steps {
            if step.state == SshWorkflowStepState::Pending {
                step.state = SshWorkflowStepState::Skipped;
                step.detail = "workflow cancelled by owner".into();
            }
        }
        workflow.state = SshWorkflowState::Cancelled;
        workflow.updated_at = Utc::now();
        let event = self.commit_ssh_workflow(previous, &workflow)?;
        Ok(SshWorkflowMutation {
            workflow,
            replayed: false,
            events: vec![event],
        })
    }

    fn commit_ssh_workflow(
        &self,
        expected_state: SshWorkflowState,
        workflow: &SshWorkflow,
    ) -> Result<EventEnvelope, RuntimeError> {
        let (_, event) = self
            .store()?
            .update_ssh_workflow(expected_state, workflow)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Creates and durably records a validated project-scoped memory claim.
    ///
    /// # Errors
    ///
    /// Invalid/secret scopes and missing projects fail without an event.
    #[allow(clippy::too_many_arguments)]
    pub fn create_memory_claim(
        &self,
        project_id: ProjectId,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: serde_json::Value,
        confidence: f64,
        provenance: ClaimProvenance,
        fresh_until: Option<DateTime<Utc>>,
    ) -> Result<MemoryClaimMutation, RuntimeError> {
        let claim = MemoryLifecycle::create(
            project_id,
            namespace,
            key,
            value,
            confidence,
            provenance,
            fresh_until,
        )?;
        let event = self.store()?.create_memory_claim(&claim)?;
        let _ = self.event_sender.send(event.clone());
        Ok(MemoryClaimMutation {
            claim,
            previous: None,
            event,
        })
    }

    /// Transfers one exact reviewed durable claim into another project.
    ///
    /// The caller owns the transfer identity and exact source digest. Replay is
    /// observation-only; changed identity reuse, stale evidence, target key
    /// collisions, and implicit/same-project transfer fail closed.
    ///
    /// # Errors
    ///
    /// Returns not-found, invalid-request, conflict, or storage failures without
    /// creating partial target memory.
    pub fn transfer_project_memory(
        &self,
        transfer_id: MemoryTransferId,
        source_project_id: ProjectId,
        source_claim_id: MemoryClaimId,
        source_claim_sha256: &str,
        target_project_id: ProjectId,
        reason: &str,
    ) -> Result<ProjectMemoryTransferMutation, RuntimeError> {
        let (transfer, claim, event, replayed) = {
            let mut store = self.store()?;
            let source = store
                .memory_claim(source_claim_id)?
                .ok_or(StorageError::MemoryClaimNotFound(source_claim_id))?;
            if source.project_id != source_project_id {
                return Err(StorageError::MemoryClaimNotFound(source_claim_id).into());
            }
            let (requested, target_claim) = prepare_project_memory_transfer(
                transfer_id,
                &source,
                source_claim_sha256,
                target_project_id,
                reason,
            )?;
            store.create_project_memory_transfer(&requested, &target_claim)?
        };
        let mut events = Vec::new();
        if let Some(event) = event {
            let _ = self.event_sender.send(event.clone());
            events.push(event);
        }
        Ok(ProjectMemoryTransferMutation {
            transfer,
            claim,
            replayed,
            events,
        })
    }

    /// Loads one exact source claim and returns its canonical transfer digest.
    ///
    /// This read-only review step admits only a fresh durable source. The
    /// returned digest must be supplied unchanged to the later mutation.
    ///
    /// # Errors
    ///
    /// Returns not-found for a claim outside the source project and rejects a
    /// source that is stale, non-durable, or oversized.
    pub fn project_memory_transfer_preview(
        &self,
        source_project_id: ProjectId,
        source_claim_id: MemoryClaimId,
    ) -> Result<ProjectMemoryTransferPreview, RuntimeError> {
        let claim = self
            .store()?
            .memory_claim(source_claim_id)?
            .filter(|claim| claim.project_id == source_project_id)
            .ok_or(StorageError::MemoryClaimNotFound(source_claim_id))?;
        let source_claim_sha256 = mcg_projects::transferable_memory_claim_sha256(&claim)?;
        Ok(ProjectMemoryTransferPreview {
            claim,
            source_claim_sha256,
        })
    }

    /// Lists immutable inbound and outbound transfer evidence for one project.
    ///
    /// # Errors
    ///
    /// Returns not-found for an unknown project or a storage read failure.
    pub fn project_memory_transfers(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectMemoryTransfer>, RuntimeError> {
        let store = self.store()?;
        if store.project(project_id)?.is_none() {
            return Err(StorageError::ProjectNotFound(project_id).into());
        }
        Ok(store.project_memory_transfers(project_id)?)
    }

    /// Corrects the current claim version under one storage lock and records the
    /// superseding version plus event atomically.
    ///
    /// # Errors
    ///
    /// Missing/stale versions and invalid corrected values fail without mutation.
    pub fn correct_memory_claim(
        &self,
        project_id: ProjectId,
        claim_id: MemoryClaimId,
        value: serde_json::Value,
        confidence: f64,
        provenance: ClaimProvenance,
        fresh_until: Option<DateTime<Utc>>,
    ) -> Result<MemoryClaimMutation, RuntimeError> {
        let (claim, previous, event) = {
            let mut store = self.store()?;
            let mut previous = store
                .memory_claim(claim_id)?
                .ok_or(StorageError::MemoryClaimNotFound(claim_id))?;
            if previous.project_id != project_id {
                return Err(StorageError::MemoryClaimNotFound(claim_id).into());
            }
            let claim = MemoryLifecycle::correct(
                &mut previous,
                value,
                confidence,
                provenance,
                fresh_until,
            )?;
            let (stored_previous, event) = store.correct_memory_claim(&claim)?;
            (claim, stored_previous, event)
        };
        let _ = self.event_sender.send(event.clone());
        Ok(MemoryClaimMutation {
            claim,
            previous: Some(previous),
            event,
        })
    }

    /// Reinforces, decays, or retracts a claim by recording a successor version.
    ///
    /// # Errors
    ///
    /// Missing/stale versions and illegal lifecycle transitions fail atomically.
    pub fn transition_memory_claim(
        &self,
        project_id: ProjectId,
        claim_id: MemoryClaimId,
        action: MemoryLifecycleAction,
        provenance: ClaimProvenance,
    ) -> Result<MemoryClaimMutation, RuntimeError> {
        let (claim, previous, event) = {
            let mut store = self.store()?;
            let mut previous = store
                .memory_claim(claim_id)?
                .ok_or(StorageError::MemoryClaimNotFound(claim_id))?;
            if previous.project_id != project_id {
                return Err(StorageError::MemoryClaimNotFound(claim_id).into());
            }
            let claim = match action {
                MemoryLifecycleAction::Reinforce => {
                    MemoryLifecycle::reinforced(&mut previous, provenance)?
                }
                MemoryLifecycleAction::Decay => {
                    MemoryLifecycle::decayed(&mut previous, provenance)?
                }
                MemoryLifecycleAction::Retract => {
                    MemoryLifecycle::retracted(&mut previous, provenance)?
                }
            };
            let (stored_previous, event) = store.revise_memory_claim(&claim)?;
            (claim, stored_previous, event)
        };
        let _ = self.event_sender.send(event.clone());
        Ok(MemoryClaimMutation {
            claim,
            previous: Some(previous),
            event,
        })
    }

    /// Lists current project memory heads with optional namespace/freshness filters.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn memory_claims(
        &self,
        project_id: ProjectId,
        namespace: Option<&str>,
        fresh_only: bool,
    ) -> Result<Vec<MemoryClaim>, RuntimeError> {
        Ok(self
            .store()?
            .memory_claims(project_id, namespace, fresh_only)?)
    }

    /// Loads every version for one scoped memory key.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn memory_claim_history(
        &self,
        project_id: ProjectId,
        namespace: &str,
        key: &str,
    ) -> Result<Vec<MemoryClaim>, RuntimeError> {
        Ok(self
            .store()?
            .memory_claim_history(project_id, namespace, key)?)
    }

    /// Assembles bounded project context from discovery facts, current fresh
    /// memory, and caller-supplied task context.
    ///
    /// # Errors
    ///
    /// Returns not-found for unknown projects or a storage read failure.
    pub fn assemble_project_context(
        &self,
        project_id: ProjectId,
        mut supplied: Vec<ContextItem>,
        token_budget: u32,
        relative_target: Option<&str>,
    ) -> Result<ContextAssembly, RuntimeError> {
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        supplied.extend(ContextAssembler::from_project_for_target(
            &project,
            relative_target.unwrap_or(""),
        ));
        supplied.extend(
            self.memory_claims(project_id, None, true)?
                .into_iter()
                .map(|claim| ContextItem {
                    category: ContextCategory::Memory,
                    content: format!("{}.{} = {}", claim.namespace, claim.key, claim.value),
                    source: claim.provenance.source,
                    priority: if claim.state == mcg_projects::MemoryState::Durable {
                        800
                    } else {
                        500
                    },
                    estimated_tokens: 0,
                }),
        );
        Ok(ContextAssembler::assemble(supplied, token_budget))
    }

    /// Renders current managed memory into stable human-readable Markdown.
    ///
    /// # Errors
    ///
    /// Returns an error when memory cannot be read.
    pub fn managed_memory(&self, project_id: ProjectId) -> Result<String, RuntimeError> {
        let claims = self.memory_claims(project_id, None, false)?;
        Ok(render_managed_memory(project_id, &claims))
    }

    /// Applies one bounded deterministic freshness-consolidation batch.
    ///
    /// Claims with an elapsed explicit freshness deadline enter decay, including
    /// previously durable claims. A decaying successor is retained for seven
    /// days before a later batch retracts it. Every change creates an immutable
    /// successor and ordinary memory event; concurrent owner edits win and are
    /// counted rather than overwritten.
    ///
    /// # Errors
    ///
    /// Storage or invariant failures other than a concurrent-head conflict stop
    /// the batch and are returned.
    pub fn consolidate_memory_at(
        &self,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<MemoryConsolidationReport, RuntimeError> {
        let decay_cutoff = now - Duration::days(MEMORY_DECAY_GRACE_DAYS);
        let candidates = self
            .store()?
            .memory_consolidation_candidates(now, decay_cutoff, limit)?;
        let mut report = MemoryConsolidationReport {
            examined: candidates.len(),
            decayed: 0,
            retracted: 0,
            conflicts: 0,
            mutations: Vec::with_capacity(candidates.len()),
        };
        for mut previous in candidates {
            let provenance = ClaimProvenance {
                source_kind: ProvenanceKind::Derived,
                source: "automatic freshness consolidation".into(),
                task_id: None,
                artifact_sha256: None,
            };
            let mut claim = match previous.state {
                MemoryState::New | MemoryState::Working | MemoryState::Durable
                    if previous.fresh_until.is_some_and(|deadline| deadline <= now) =>
                {
                    MemoryLifecycle::expired(&mut previous, provenance)?
                }
                MemoryState::Decaying if previous.observed_at <= decay_cutoff => {
                    MemoryLifecycle::retracted(&mut previous, provenance)?
                }
                _ => continue,
            };
            claim.observed_at = now;
            let stored = self.store()?.revise_memory_claim(&claim);
            let (stored_previous, event) = match stored {
                Ok(value) => value,
                Err(StorageError::Conflict(_)) => {
                    report.conflicts += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            match claim.state {
                MemoryState::Decaying => report.decayed += 1,
                MemoryState::Retracted => report.retracted += 1,
                _ => unreachable!("consolidation only decays or retracts"),
            }
            let _ = self.event_sender.send(event.clone());
            report.mutations.push(MemoryClaimMutation {
                claim,
                previous: Some(stored_previous),
                event,
            });
        }
        Ok(report)
    }

    /// Lists persisted secret-free provider connections.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn provider_connections(&self) -> Result<Vec<ProviderConnection>, RuntimeError> {
        Ok(self.store()?.provider_connections()?)
    }

    /// Loads the latest durable discovery snapshot for a provider.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn provider_catalog(
        &self,
        connection_id: &str,
    ) -> Result<Option<ProviderModelCatalog>, RuntimeError> {
        Ok(self.store()?.provider_catalog(connection_id)?)
    }

    /// Validates optional governed context before a proxy call can incur provider work.
    /// Existing conversation context is immutable; a task-bound conversation inherits
    /// the task's project when the caller omits it.
    ///
    /// # Errors
    /// Returns missing-context, immutable-context conflict, or storage failures.
    pub fn prepare_proxy_conversation(
        &self,
        id: ProxyConversationId,
        mut project_id: Option<ProjectId>,
        task_id: Option<TaskId>,
        now: DateTime<Utc>,
    ) -> Result<ProxyConversation, RuntimeError> {
        if let Some(task_id) = task_id {
            let task = self
                .task(task_id)?
                .ok_or(StorageError::TaskNotFound(task_id))?;
            if let Some(requested) = project_id
                && task.project_id != Some(requested)
            {
                return Err(StorageError::Conflict(
                    "provider proxy task does not belong to the requested project".into(),
                )
                .into());
            }
            project_id = project_id.or(task.project_id);
        }
        if let Some(project_id) = project_id
            && self.project(project_id)?.is_none()
        {
            return Err(StorageError::ProjectNotFound(project_id).into());
        }
        if let Some(existing) = self.store()?.proxy_conversation(id)? {
            if existing.conversation.project_id != project_id
                || existing.conversation.task_id != task_id
            {
                return Err(StorageError::Conflict(
                    "provider proxy conversation context is immutable".into(),
                )
                .into());
            }
            return Ok(ProxyConversation {
                updated_at: now,
                ..existing.conversation
            });
        }
        Ok(ProxyConversation {
            schema_version: PROXY_LEDGER_SCHEMA_VERSION,
            id,
            project_id,
            task_id,
            created_at: now,
            updated_at: now,
        })
    }

    /// Persists one completed proxy call and advances its conversation projection.
    ///
    /// # Errors
    /// Returns validation, context, conflict, serialization, or storage failures.
    pub fn record_proxy_call(
        &self,
        conversation: &ProxyConversation,
        call: &ProxyCallRecord,
    ) -> Result<(), RuntimeError> {
        self.store()?.record_proxy_call(conversation, call)?;
        Ok(())
    }

    /// Durably claims one content-free physical provider call before network I/O.
    ///
    /// # Errors
    /// Returns validation, identity-conflict, serialization, or storage failures.
    pub fn begin_provider_dispatch(
        &self,
        dispatch: &ProviderDispatchRecord,
    ) -> Result<ProviderDispatchBegin, RuntimeError> {
        Ok(self.store()?.begin_provider_dispatch(dispatch)?)
    }

    /// Marks one claimed physical request as having returned a validated response.
    ///
    /// # Errors
    /// Returns missing-dispatch, response-identity, validation, or storage failures.
    pub fn complete_provider_dispatch(
        &self,
        client_request_id: &str,
        response_id: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<ProviderDispatchRecord, RuntimeError> {
        Ok(self.store()?.complete_provider_dispatch(
            client_request_id,
            response_id,
            completed_at,
        )?)
    }

    /// Marks one claimed physical request as a definite classified failure.
    ///
    /// # Errors
    /// Returns missing-dispatch, classification, validation, or storage failures.
    pub fn fail_provider_dispatch(
        &self,
        client_request_id: &str,
        failure: ProviderDispatchFailure,
        failed_at: DateTime<Utc>,
    ) -> Result<ProviderDispatchRecord, RuntimeError> {
        Ok(self
            .store()?
            .fail_provider_dispatch(client_request_id, failure, failed_at)?)
    }

    /// Lists content-free physical dispatch evidence for one logical proxy call.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn provider_dispatches_for_call(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Vec<ProviderDispatchRecord>, RuntimeError> {
        Ok(self.store()?.provider_dispatches_for_call(call_id)?)
    }

    /// Saves private continuation state for an MCG-managed provider-proxy turn.
    ///
    /// # Errors
    /// Returns validation, stale-progress, context, or storage failures.
    pub fn save_provider_proxy_turn_checkpoint(
        &self,
        checkpoint: &ProviderProxyTurnCheckpoint,
    ) -> Result<(), RuntimeError> {
        self.store()?
            .save_provider_proxy_turn_checkpoint(checkpoint)?;
        Ok(())
    }

    /// Loads private continuation state for one managed provider-proxy call.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn provider_proxy_turn_checkpoint(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Option<ProviderProxyTurnCheckpoint>, RuntimeError> {
        Ok(self.store()?.provider_proxy_turn_checkpoint(call_id)?)
    }

    /// Saves private, restart-safe member/synthesis state for one ensemble call.
    ///
    /// # Errors
    /// Returns validation, stale-progress, context, or storage failures.
    pub fn save_provider_proxy_ensemble_checkpoint(
        &self,
        checkpoint: &ProviderProxyEnsembleCheckpoint,
    ) -> Result<(), RuntimeError> {
        self.store()?
            .save_provider_proxy_ensemble_checkpoint(checkpoint)?;
        Ok(())
    }

    /// Loads private member/synthesis state for one ensemble call.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn provider_proxy_ensemble_checkpoint(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Option<ProviderProxyEnsembleCheckpoint>, RuntimeError> {
        Ok(self.store()?.provider_proxy_ensemble_checkpoint(call_id)?)
    }

    /// Loads one completed provider-proxy call by its global caller identity.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn proxy_call(
        &self,
        call_id: ProxyCallId,
    ) -> Result<Option<ProxyCallRecord>, RuntimeError> {
        Ok(self.store()?.proxy_call(call_id)?)
    }

    /// Lists immutable content-free provider-proxy calls for evidence-based
    /// local routing. Prompts and generated output are never present.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn proxy_calls(&self) -> Result<Vec<ProxyCallRecord>, RuntimeError> {
        Ok(self.store()?.proxy_calls()?)
    }

    /// Atomically records final proxy usage and removes the exact private
    /// managed-turn continuation checkpoint.
    ///
    /// # Errors
    /// Returns validation, identity, checkpoint, or storage failures.
    pub fn complete_provider_proxy_turn(
        &self,
        checkpoint: &ProviderProxyTurnCheckpoint,
        call: &ProxyCallRecord,
    ) -> Result<(), RuntimeError> {
        self.store()?
            .complete_provider_proxy_turn(checkpoint, call)?;
        Ok(())
    }

    /// Atomically records final ensemble usage and removes its exact private
    /// member/synthesis continuation checkpoint.
    ///
    /// # Errors
    /// Returns validation, identity, checkpoint, or storage failures.
    pub fn complete_provider_proxy_ensemble(
        &self,
        checkpoint: &ProviderProxyEnsembleCheckpoint,
        call: &ProxyCallRecord,
    ) -> Result<(), RuntimeError> {
        self.store()?
            .complete_provider_proxy_ensemble(checkpoint, call)?;
        Ok(())
    }

    /// Executes one canonical `host::` provider tool through the same
    /// project-confined policy, approval, lease, redaction, artifact, and audit
    /// plane used by native agent turns. The caller-owned proxy call and model
    /// tool-call identities derive a stable tool request id for approval retry.
    ///
    /// # Errors
    /// Returns missing task/project, malformed or unsupported tool, policy,
    /// approval, executor, or durable audit failures.
    #[allow(clippy::too_many_lines)] // Keep the fail-closed decode-to-broker sequence visible in one boundary.
    pub async fn execute_provider_host_tool(
        &self,
        task_id: TaskId,
        call_id: ProxyCallId,
        model_tool_call_id: &str,
        qualified_name: &str,
        arguments: &str,
    ) -> Result<ToolOutcome, RuntimeError> {
        if model_tool_call_id.trim().is_empty()
            || model_tool_call_id.len() > 256
            || model_tool_call_id.chars().any(char::is_control)
        {
            return Err(StorageError::InvalidData(
                "provider model tool-call identity is invalid".into(),
            )
            .into());
        }
        let name = qualified_name.strip_prefix("host::").ok_or_else(|| {
            StorageError::InvalidData("provider-managed host tool has no host:: namespace".into())
        })?;
        if canonical_host_tool_definition(name).is_none() {
            return Err(StorageError::InvalidData(format!(
                "provider-managed host tool {qualified_name} is unsupported"
            ))
            .into());
        }
        let mut object = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
            arguments,
        )
        .map_err(|error| {
            StorageError::InvalidData(format!(
                "invalid provider-managed {qualified_name} arguments: {error}"
            ))
        })?;
        if object.contains_key("tool") {
            return Err(StorageError::InvalidData(
                "provider-managed arguments cannot override the typed tool discriminator".into(),
            )
            .into());
        }
        let canonical_arguments =
            serde_json::to_string(&object).map_err(StorageError::Serialization)?;
        object.insert(
            "tool".into(),
            serde_json::Value::String(name.replace('.', "_")),
        );
        let mut kind: ToolRequestKind = serde_json::from_value(serde_json::Value::Object(object))
            .map_err(|error| {
            StorageError::InvalidData(format!(
                "invalid provider-managed {qualified_name} arguments: {error}"
            ))
        })?;
        if kind.stable_name() != name {
            return Err(StorageError::InvalidData(
                "provider-managed tool arguments decoded to a different primitive".into(),
            )
            .into());
        }

        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("provider-managed task has no project".into())
        })?;
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        let root = PathBuf::from(project.root)
            .canonicalize()
            .map_err(StorageError::Io)?;
        resolve_project_relative_tool_paths(&mut kind, &root);
        let artifacts = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let credentials = Arc::new(RejectingCredentialResolver);
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifacts).map_err(runtime_tool_error)?,
        );
        let process = Arc::new(
            ProcessExecutor::new([root.clone()], credentials.clone())
                .map_err(runtime_tool_error)?,
        );
        let http = Arc::new(
            HttpExecutor::new(credentials, filesystem.clone(), &artifacts)
                .map_err(runtime_tool_error)?,
        );
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: http.clone(),
            browser: Arc::new(
                BrowserExecutor::new(self.config.browser_executable.clone(), &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            research: Arc::new(ResearchExecutor::new(http)),
            system: Arc::new(SystemExecutor),
            git: Arc::new(
                GitExecutor::new(process, [root.clone()], &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            memory: Arc::new(MemoryExecutor::new(InMemoryMemoryRepository::default())),
            external: Arc::new(RuntimeMcpExecutor {
                store: self.store.clone(),
                credentials: self.mcp_credentials.clone(),
            }),
        };
        let control = self.tool_control();
        let broker = ToolBroker::new(
            host,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: CapabilityPolicy::project(root),
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let mut request = ToolRequest::new(
            task_id,
            format!("Provider-managed {qualified_name} operation"),
            kind,
        );
        request.id = provider_managed_tool_request_id(
            call_id,
            model_tool_call_id,
            qualified_name,
            &canonical_arguments,
        );
        request.namespace = mcg_tools::ToolNamespace::Host;
        broker.execute(&request).await.map_err(RuntimeError::from)
    }

    /// Executes one canonical read-only `mcg::` provider tool against the exact
    /// task/project authority while retaining content-free decision/completion
    /// audit and a durable execution lease.
    ///
    /// # Errors
    /// Returns missing context, malformed/unsupported arguments, storage,
    /// lease, audit, serialization, or bounded-output failures.
    #[allow(clippy::too_many_lines)] // Keeps validation, bounded execution, lease, and audit ordering explicit.
    pub async fn execute_provider_mcg_tool(
        &self,
        task_id: TaskId,
        call_id: ProxyCallId,
        model_tool_call_id: &str,
        qualified_name: &str,
        arguments: &str,
    ) -> Result<ToolOutcome, RuntimeError> {
        if model_tool_call_id.trim().is_empty()
            || model_tool_call_id.len() > 256
            || model_tool_call_id.chars().any(char::is_control)
        {
            return Err(StorageError::InvalidData(
                "provider model tool-call identity is invalid".into(),
            )
            .into());
        }
        let mut object = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
            arguments,
        )
        .map_err(|error| {
            StorageError::InvalidData(format!(
                "invalid provider-managed {qualified_name} arguments: {error}"
            ))
        })?;
        let canonical_arguments =
            serde_json::to_string(&object).map_err(StorageError::Serialization)?;
        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("provider-managed task has no project".into())
        })?;
        let output = match qualified_name {
            "mcg::task.get" => {
                require_provider_mcg_keys(&object, &[])?;
                serde_json::to_value(&task).map_err(StorageError::Serialization)?
            }
            "mcg::project.context" => {
                require_provider_mcg_keys(&object, &["relative_target", "token_budget"])?;
                let relative_target =
                    take_bounded_optional_string(&mut object, "relative_target", 1_024)?
                        .map(|value| validate_provider_mcg_relative_target(&value))
                        .transpose()?;
                let token_budget =
                    take_bounded_optional_u64(&mut object, "token_budget", 64, 4_096)?
                        .unwrap_or(4_096);
                serde_json::to_value(self.assemble_project_context(
                    project_id,
                    Vec::new(),
                    u32::try_from(token_budget).unwrap_or(4_096),
                    relative_target.as_deref(),
                )?)
                .map_err(StorageError::Serialization)?
            }
            "mcg::memory.search" => {
                require_provider_mcg_keys(&object, &["query", "namespace", "limit"])?;
                let query = take_bounded_required_string(&mut object, "query", 512)?;
                let namespace = take_bounded_optional_string(&mut object, "namespace", 128)?;
                let limit = take_bounded_optional_u64(&mut object, "limit", 1, 20)?.unwrap_or(10);
                let query = query.to_lowercase();
                let claims = self
                    .memory_claims(project_id, namespace.as_deref(), true)?
                    .into_iter()
                    .filter(|claim| {
                        claim.namespace.to_lowercase().contains(&query)
                            || claim.key.to_lowercase().contains(&query)
                            || claim.value.to_string().to_lowercase().contains(&query)
                            || claim.provenance.source.to_lowercase().contains(&query)
                    })
                    .take(usize::try_from(limit).unwrap_or(20))
                    .collect::<Vec<_>>();
                serde_json::json!({"claims":claims})
            }
            _ => {
                return Err(StorageError::InvalidData(format!(
                    "provider-managed MCG tool {qualified_name} is unsupported"
                ))
                .into());
            }
        };
        let output = serde_json::to_string(&output).map_err(StorageError::Serialization)?;
        if output.len() > 256 * 1_024 {
            return Err(StorageError::InvalidData(
                "provider-managed MCG tool output exceeds 256 KiB".into(),
            )
            .into());
        }
        let request_id = provider_managed_tool_request_id(
            call_id,
            model_tool_call_id,
            qualified_name,
            &canonical_arguments,
        );
        let execution_id = ToolExecutionId::new();
        let resource = format!("project:{project_id}");
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                format!("{task_id}\0{call_id}\0{model_tool_call_id}\0{qualified_name}\0{canonical_arguments}")
                    .as_bytes()
            )
        );
        let control = self.tool_control();
        control
            .record(ToolAuditRecord {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id,
                task_id,
                tool: qualified_name.into(),
                resource: resource.clone(),
                request_fingerprint: fingerprint.clone(),
                phase: ToolAuditPhase::Decision,
                decision: "allow: canonical read-only MCG task/project authority".into(),
                result: None,
                artifacts: Vec::new(),
                verification: Vec::new(),
                recorded_at: Utc::now(),
            })
            .await?;
        let lease = control
            .acquire(request_id, execution_id, Duration::seconds(30))
            .await?;
        let started_at = Utc::now();
        let finished_at = Utc::now().max(started_at);
        let outcome = ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id,
            status: ToolOutcomeStatus::Succeeded,
            summary: format!("Completed governed {qualified_name} read"),
            exit_code: None,
            stdout: Some(output),
            stderr: None,
            artifacts: Vec::new(),
            verification: Vec::new(),
            started_at,
            finished_at,
        };
        control.finish(lease.id, LeaseState::Completed).await?;
        control
            .record(ToolAuditRecord {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id,
                task_id,
                tool: qualified_name.into(),
                resource,
                request_fingerprint: fingerprint,
                phase: ToolAuditPhase::Completion,
                decision: "completed: canonical read-only MCG operation".into(),
                result: Some(ToolOutcomeStatus::Succeeded),
                artifacts: Vec::new(),
                verification: Vec::new(),
                recorded_at: finished_at,
            })
            .await?;
        Ok(outcome)
    }

    /// Lists privacy-preserving proxy conversation projections.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn proxy_conversations(&self) -> Result<Vec<ProxyConversation>, RuntimeError> {
        Ok(self.store()?.proxy_conversations()?)
    }

    /// Loads one proxy conversation with immutable call history.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn proxy_conversation(
        &self,
        id: ProxyConversationId,
    ) -> Result<Option<ProxyConversationView>, RuntimeError> {
        Ok(self.store()?.proxy_conversation(id)?)
    }

    /// Aggregates proxy token usage and known costs.
    ///
    /// # Errors
    /// Returns decoding or storage failures.
    pub fn proxy_usage_summary(&self) -> Result<ProxyUsageSummary, RuntimeError> {
        Ok(self.store()?.proxy_usage_summary()?)
    }

    /// Lists persisted secret-free MCP client connections.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read.
    pub fn mcp_connections(&self) -> Result<Vec<McpConnection>, RuntimeError> {
        Ok(self.store()?.mcp_connections()?)
    }

    /// Loads the latest durable MCP tool discovery snapshot.
    ///
    /// # Errors
    /// Returns an error when storage cannot be read.
    pub fn mcp_catalog(&self, connection_id: &str) -> Result<Option<McpToolCatalog>, RuntimeError> {
        Ok(self.store()?.mcp_catalog(connection_id)?)
    }

    /// Creates an HTTP or stdio MCP connection, stores any bearer credential only
    /// in the protected operating-system store, discovers tools, and persists
    /// secret-free evidence atomically.
    ///
    /// # Errors
    /// Returns duplicate, validation, credential-store, transport, negotiation,
    /// remote-response, or persistence failures. Failed setup removes credentials.
    pub async fn configure_mcp(
        &self,
        connection: McpConnection,
        credential: Option<&McpCredential>,
    ) -> Result<McpMutation, RuntimeError> {
        if self.store()?.mcp_connection(&connection.id)?.is_some() {
            return Err(StorageError::Conflict(format!(
                "MCP connection {} already exists",
                connection.id
            ))
            .into());
        }
        save_mcp_credential(self.mcp_credentials.as_ref(), &connection, credential)?;
        let client_credential = load_mcp_credential(self.mcp_credentials.as_ref(), &connection)?;
        let client = match McpClient::new(connection.clone(), client_credential) {
            Ok(client) => client,
            Err(error) => {
                let _ = self.mcp_credentials.delete(&connection.id);
                return Err(error.into());
            }
        };
        let catalog = match client.discover().await {
            Ok(catalog) => catalog,
            Err(error) => {
                let _ = self.mcp_credentials.delete(&connection.id);
                return Err(error.into());
            }
        };
        let events = match self
            .store()?
            .record_mcp_configuration(&connection, &catalog, Utc::now())
        {
            Ok(events) => events,
            Err(error) => {
                let _ = self.mcp_credentials.delete(&connection.id);
                return Err(error.into());
            }
        };
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(McpMutation {
            connection,
            catalog,
            events,
        })
    }

    /// Rediscovers tools for one durable HTTP or stdio MCP connection.
    ///
    /// # Errors
    /// Returns not-found, credential-store, transport, negotiation, response, or
    /// persistence failures without replacing the previous durable catalog.
    pub async fn discover_mcp(&self, connection_id: &str) -> Result<McpToolCatalog, RuntimeError> {
        let connection = self
            .store()?
            .mcp_connection(connection_id)?
            .ok_or_else(|| {
                StorageError::InvalidData(format!("MCP connection {connection_id} was not found"))
            })?;
        let credential = load_mcp_credential(self.mcp_credentials.as_ref(), &connection)?;
        let catalog = McpClient::new(connection, credential)?.discover().await?;
        let event = self.store()?.record_mcp_catalog(&catalog)?;
        let _ = self.event_sender.send(event);
        Ok(catalog)
    }

    #[must_use]
    pub fn provider_gateway(&self) -> RuntimeProviderGateway {
        RuntimeProviderGateway {
            registry: self.provider_registry.clone(),
            oauth_refresh: self.oauth_refresh.clone(),
        }
    }

    /// Disconnects one provider, removes every protected credential slot, and
    /// atomically removes its active connection/catalog projections while retaining
    /// secret-free immutable audit and usage history.
    ///
    /// # Errors
    /// Returns not-found, protected-store, registry, or persistence failures. When
    /// persistence fails after credential deletion, the prior credential set and
    /// live registry entry are restored best-effort before returning the error.
    pub async fn disconnect_provider(
        &self,
        connection_id: &str,
    ) -> Result<EventEnvelope, RuntimeError> {
        let _operation = self.oauth_refresh.lock_connection(connection_id).await;
        let connection = self
            .store()?
            .provider_connection(connection_id)?
            .ok_or_else(|| {
                StorageError::InvalidData(format!(
                    "provider connection {connection_id} was not found"
                ))
            })?;
        let credentials = self.provider_credential_snapshot(&connection)?;
        let mut registry = self.provider_registry.write().await;
        registry.unregister(connection_id);
        if let Err(error) =
            Self::clear_connection_credentials(self.provider_credentials.as_ref(), &connection)
        {
            if self
                .restore_provider_registration(&mut registry, &connection, &credentials)
                .is_err()
            {
                return Err(ProviderError::Authentication(
                    "provider credential deletion failed and the prior live registration could not be restored"
                        .into(),
                )
                .into());
            }
            return Err(error.into());
        }
        let event = match self
            .store()?
            .record_provider_connection_removed(connection_id, Utc::now())
        {
            Ok(event) => event,
            Err(error) => {
                if self
                    .restore_provider_registration(&mut registry, &connection, &credentials)
                    .is_err()
                {
                    return Err(ProviderError::Authentication(
                        "provider disconnect persistence failed and the prior live registration could not be restored"
                            .into(),
                    )
                    .into());
                }
                return Err(error.into());
            }
        };
        drop(registry);
        self.oauth_refresh.forget_connection(connection_id).await;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    fn clear_connection_credentials(
        store: &dyn ProviderCredentialStore,
        connection: &ProviderConnection,
    ) -> Result<(), ProviderError> {
        if connection.auth_mode == mcg_providers::ProviderAuthMode::None {
            return Ok(());
        }
        clear_provider_credentials(store, &connection.id)
    }

    fn provider_credential_snapshot(
        &self,
        connection: &ProviderConnection,
    ) -> Result<ProtectedProviderCredentialSnapshot, RuntimeError> {
        match connection.auth_mode {
            mcg_providers::ProviderAuthMode::None => Ok(ProtectedProviderCredentialSnapshot::None),
            mcg_providers::ProviderAuthMode::ApiKey
            | mcg_providers::ProviderAuthMode::BearerToken => self
                .provider_credentials
                .load(&connection.id, ProviderSecretSlot::Primary)?
                .map(ProtectedProviderCredentialSnapshot::Primary)
                .ok_or_else(|| {
                    ProviderError::Authentication(format!(
                        "provider connection {} has no protected primary credential",
                        connection.id
                    ))
                    .into()
                }),
            mcg_providers::ProviderAuthMode::ChatGptBrowserOauth
            | mcg_providers::ProviderAuthMode::ChatGptDeviceCode => Ok(
                ProtectedProviderCredentialSnapshot::OAuth(mcg_providers::load_oauth_credentials(
                    self.provider_credentials.as_ref(),
                    &connection.id,
                    connection.auth_mode,
                )?),
            ),
        }
    }

    fn restore_provider_credentials(
        &self,
        connection: &ProviderConnection,
        credentials: &ProtectedProviderCredentialSnapshot,
    ) -> Result<(), ProviderError> {
        match credentials {
            ProtectedProviderCredentialSnapshot::None => save_provider_credentials(
                self.provider_credentials.as_ref(),
                &connection.id,
                connection.auth_mode,
                None,
                None,
            ),
            ProtectedProviderCredentialSnapshot::Primary(primary) => save_provider_credentials(
                self.provider_credentials.as_ref(),
                &connection.id,
                connection.auth_mode,
                Some(primary),
                None,
            ),
            ProtectedProviderCredentialSnapshot::OAuth(oauth) => save_provider_credentials(
                self.provider_credentials.as_ref(),
                &connection.id,
                connection.auth_mode,
                None,
                Some(oauth),
            ),
        }
    }

    fn restore_provider_registration(
        &self,
        registry: &mut ProviderRegistry,
        connection: &ProviderConnection,
        credentials: &ProtectedProviderCredentialSnapshot,
    ) -> Result<(), ProviderError> {
        self.restore_provider_credentials(connection, credentials)?;
        registry.register_connection(connection.clone(), self.provider_credentials.as_ref())
    }

    /// Returns discovered routes, lazily restoring durable provider metadata and
    /// catalogs after daemon restart without issuing a discovery/model request.
    ///
    /// # Errors
    ///
    /// Returns a protected-credential, provider-validation, or storage error.
    pub async fn provider_route_candidates(&self) -> Result<Vec<RouteCandidate>, RuntimeError> {
        let durable = {
            let store = self.store()?;
            store
                .provider_connections()?
                .into_iter()
                .map(|connection| {
                    let catalog = store.provider_catalog(&connection.id)?;
                    Ok((connection, catalog))
                })
                .collect::<Result<Vec<_>, StorageError>>()?
        };
        let mut registry = self.provider_registry.write().await;
        let mut first_error = None;
        for (connection, catalog) in durable {
            if !registry
                .provider_ids()
                .iter()
                .any(|id| id == &connection.id)
                && let Err(error) = registry
                    .register_connection(connection.clone(), self.provider_credentials.as_ref())
            {
                first_error.get_or_insert(error);
                continue;
            }
            if registry.models(&connection.id).is_none()
                && let Some(catalog) = catalog
                && let Err(error) = registry.restore_catalog(&connection.id, catalog.models)
            {
                first_error.get_or_insert(error);
            }
        }
        let routes = registry.route_candidates();
        if routes.is_empty()
            && let Some(error) = first_error
        {
            return Err(error.into());
        }
        Ok(routes)
    }

    /// Creates a provider connection, stores credentials only in the protected
    /// operating-system store, discovers models, and persists secret-free evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate/invalid metadata, mismatched credentials,
    /// protected-store failure, discovery failure, or durable persistence failure.
    pub async fn configure_provider(
        &self,
        connection: ProviderConnection,
        primary: Option<&ProviderCredential>,
        oauth: Option<&OAuthTokenSet>,
    ) -> Result<ProviderMutation, RuntimeError> {
        if self.store()?.provider_connection(&connection.id)?.is_some() {
            return Err(StorageError::Conflict(format!(
                "provider connection {} already exists",
                connection.id
            ))
            .into());
        }
        save_provider_credentials(
            self.provider_credentials.as_ref(),
            &connection.id,
            connection.auth_mode,
            primary,
            oauth,
        )?;
        {
            let mut registry = self.provider_registry.write().await;
            if let Err(error) =
                registry.register_connection(connection.clone(), self.provider_credentials.as_ref())
            {
                let _ =
                    clear_provider_credentials(self.provider_credentials.as_ref(), &connection.id);
                return Err(error.into());
            }
        }
        let discovery_result = self
            .provider_registry
            .write()
            .await
            .discover(&connection.id)
            .await;
        let models = match discovery_result {
            Ok(models) => models,
            Err(error) => {
                self.provider_registry
                    .write()
                    .await
                    .unregister(&connection.id);
                let _ =
                    clear_provider_credentials(self.provider_credentials.as_ref(), &connection.id);
                return Err(error.into());
            }
        };
        let catalog = ProviderModelCatalog {
            connection_id: connection.id.clone(),
            models,
            discovered_at: Utc::now(),
        };
        let persistence_result = {
            let mut store = self.store()?;
            store.record_provider_configuration(&connection, &catalog, Utc::now())
        };
        let events = match persistence_result {
            Ok(events) => events,
            Err(error) => {
                self.provider_registry
                    .write()
                    .await
                    .unregister(&connection.id);
                let _ =
                    clear_provider_credentials(self.provider_credentials.as_ref(), &connection.id);
                return Err(error.into());
            }
        };
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(ProviderMutation {
            connection,
            catalog,
            events,
        })
    }

    /// Re-registers a durable provider if needed, performs live discovery, and
    /// replaces its durable secret-free catalog snapshot.
    ///
    /// # Errors
    ///
    /// Returns not-found, credential-store, provider, or persistence errors.
    pub async fn discover_provider(
        &self,
        connection_id: &str,
    ) -> Result<ProviderModelCatalog, RuntimeError> {
        let connection = self
            .store()?
            .provider_connection(connection_id)?
            .ok_or_else(|| {
                StorageError::InvalidData(format!(
                    "provider connection {connection_id} was not found"
                ))
            })?;
        let models = {
            let mut registry = self.provider_registry.write().await;
            if !registry.provider_ids().iter().any(|id| id == connection_id) {
                registry.register_connection(connection, self.provider_credentials.as_ref())?;
            }
            registry.discover(connection_id).await?
        };
        let catalog = ProviderModelCatalog {
            connection_id: connection_id.to_owned(),
            models,
            discovered_at: Utc::now(),
        };
        let event = self.store()?.record_provider_catalog(&catalog)?;
        let _ = self.event_sender.send(event);
        Ok(catalog)
    }

    /// Explicitly probes one model's live Responses, streaming, and tool-call
    /// behavior and persists the observed capability descriptor.
    ///
    /// # Errors
    ///
    /// Returns not-found, credential-store, provider, probe, or persistence errors.
    pub async fn probe_provider_model(
        &self,
        connection_id: &str,
        model_id: &str,
    ) -> Result<ModelProbeReport, RuntimeError> {
        let connection = self
            .store()?
            .provider_connection(connection_id)?
            .ok_or_else(|| {
                StorageError::InvalidData(format!(
                    "provider connection {connection_id} was not found"
                ))
            })?;
        let (report, models) = {
            let mut registry = self.provider_registry.write().await;
            if !registry.provider_ids().iter().any(|id| id == connection_id) {
                registry.register_connection(connection, self.provider_credentials.as_ref())?;
            }
            if registry.models(connection_id).is_none() {
                registry.discover(connection_id).await?;
            }
            let report = registry.probe_model(connection_id, model_id).await?;
            let models = registry.models(connection_id).ok_or_else(|| {
                ProviderError::Configuration("provider probe produced no catalog".into())
            })?;
            (report, models)
        };
        let catalog = ProviderModelCatalog {
            connection_id: connection_id.to_owned(),
            models,
            discovered_at: Utc::now(),
        };
        let event = self.store()?.record_provider_catalog(&catalog)?;
        let _ = self.event_sender.send(event);
        Ok(report)
    }

    /// Records a validated immutable organization revision and publishes it.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or non-head revisions and failed persistence.
    pub fn record_organization_revision(
        &self,
        genome: &OrganizationGenome,
    ) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.record_organization_revision(genome)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Loads a current organization projection.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn organization(
        &self,
        organization_id: mcg_domain::OrganizationId,
    ) -> Result<Option<OrganizationGenome>, RuntimeError> {
        Ok(self.store()?.organization(organization_id)?)
    }

    /// Lists current organization projections.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn organizations(&self) -> Result<Vec<OrganizationGenome>, RuntimeError> {
        Ok(self.store()?.organizations()?)
    }

    /// Loads one immutable organization revision.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn organization_revision(
        &self,
        revision_id: mcg_domain::OrganizationRevisionId,
    ) -> Result<Option<OrganizationGenome>, RuntimeError> {
        Ok(self.store()?.organization_revision(revision_id)?)
    }

    /// Loads every immutable revision for one organization in revision order.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn organization_history(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<OrganizationGenome>, RuntimeError> {
        Ok(self.store()?.organization_history(organization_id)?)
    }

    /// Exports one immutable organization revision as a bounded, content-addressed,
    /// identity-independent portable definition.
    ///
    /// # Errors
    ///
    /// Returns not-found or portable-contract validation errors.
    pub fn export_organization(
        &self,
        revision_id: OrganizationRevisionId,
    ) -> Result<PortableOrganizationGenome, RuntimeError> {
        let genome = self
            .organization_revision(revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(revision_id))?;
        PortableOrganizationGenome::export(&genome, Utc::now())
            .map_err(|error| StorageError::InvalidData(error.to_string()).into())
    }

    /// Imports a portable definition as a fresh root organization. A caller-owned
    /// import id deterministically binds all local identities, making exact retries
    /// observational and conflicting retries fail closed.
    ///
    /// # Errors
    ///
    /// Returns validation, conflict, or persistence errors.
    pub fn import_organization(
        &self,
        import_id: OrganizationImportId,
        portable: &PortableOrganizationGenome,
        name: Option<&str>,
    ) -> Result<PortableOrganizationImportMutation, RuntimeError> {
        let candidate = portable
            .materialize_for_import(import_id, name, Utc::now())
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if let Some(existing) = self.organization(candidate.organization_id)? {
            let mut replay_candidate = candidate;
            replay_candidate.created_at = existing.created_at;
            if replay_candidate != existing {
                return Err(StorageError::Conflict(
                    "organization import id was already used for different materialized content"
                        .into(),
                )
                .into());
            }
            return Ok(PortableOrganizationImportMutation {
                organization: existing,
                portable_sha256: portable.content_sha256.clone(),
                replayed: true,
                event: None,
            });
        }
        let event = self.record_organization_revision(&candidate)?;
        Ok(PortableOrganizationImportMutation {
            organization: candidate,
            portable_sha256: portable.content_sha256.clone(),
            replayed: false,
            event: Some(event),
        })
    }

    /// Compares two immutable revisions through their identity-independent
    /// portable definitions.
    ///
    /// # Errors
    ///
    /// Returns not-found or portable-contract validation errors.
    pub fn compare_organizations(
        &self,
        left: OrganizationRevisionId,
        right: OrganizationRevisionId,
    ) -> Result<PortableOrganizationComparison, RuntimeError> {
        let left = self.export_organization(left)?;
        let right = self.export_organization(right)?;
        left.validate()
            .and_then(|()| right.validate())
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        Ok(left.compare(&right))
    }

    /// Loads the full inspectable durable view of one agent run.
    ///
    /// # Errors
    ///
    /// Returns not-found for unknown runs or invalid persisted evidence.
    pub fn agent_run_view(&self, run_id: mcg_domain::RunId) -> Result<AgentRunView, RuntimeError> {
        let store = self.store()?;
        let run = store
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let plan = store
            .work_plan(run_id)?
            .ok_or_else(|| StorageError::InvalidData("agent run has no work plan".into()))?;
        let instances = store.agent_instances(run_id)?;
        let leases = store.agent_leases_for_run(run_id)?;
        let usage = store.agent_usage(run_id)?;
        let evaluations = store.evaluations(run_id)?;
        let results = leases
            .iter()
            .map(|lease| store.agent_turn_result(lease.id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(AgentRunView {
            run,
            plan,
            instances,
            leases,
            usage,
            evaluations,
            results,
        })
    }

    /// Reconstructs and verifies one run exclusively from durable evidence.
    ///
    /// This operation is observation-only: it never dispatches a provider,
    /// executes a tool, reads mutable project files, or appends runtime state.
    /// The returned digest is stable across repeated reads and daemon restarts
    /// while the durable evidence is unchanged.
    ///
    /// # Errors
    ///
    /// Returns not-found for an unknown run and invalid-data when persisted
    /// identity, ordering, or graph invariants do not form one coherent run.
    pub fn replay_agent_run(&self, run_id: RunId) -> Result<RunReplayReport, RuntimeError> {
        let store = self.store()?;
        let run = store
            .agent_run(run_id)?
            .ok_or(StorageError::AgentRunNotFound(run_id))?;
        let plan = store
            .work_plan(run_id)?
            .ok_or_else(|| StorageError::InvalidData("agent run has no work plan".into()))?;
        let instances = store.agent_instances(run_id)?;
        let leases = store.agent_leases_for_run(run_id)?;
        let usage = store.agent_usage(run_id)?;
        let evaluations = store.evaluations(run_id)?;
        let results = leases
            .iter()
            .map(|lease| store.agent_turn_result(lease.id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let task = store
            .load_task(run.task_id)?
            .ok_or(StorageError::TaskNotFound(run.task_id))?;
        let organization = store
            .organization_revision(run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                run.organization_revision_id,
            ))?;
        let events = store.events_for_task(run.task_id)?;
        let tool_audit = store.tool_audit_for_task(run.task_id)?;
        let evidence = RunReplayEvidence {
            task,
            organization,
            view: AgentRunView {
                run,
                plan,
                instances,
                leases,
                usage,
                evaluations,
                results,
            },
            events,
            tool_audit,
        };
        let invariants = validate_run_replay_evidence(&evidence)?;
        let encoded = serde_json::to_vec(&evidence).map_err(StorageError::from)?;
        let counts = RunReplayCounts {
            events: replay_count(evidence.events.len())?,
            tool_audit_records: replay_count(evidence.tool_audit.len())?,
            work_items: replay_count(evidence.view.plan.items.len())?,
            instances: replay_count(evidence.view.instances.len())?,
            leases: replay_count(evidence.view.leases.len())?,
            usage_records: replay_count(evidence.view.usage.len())?,
            evaluations: replay_count(evidence.view.evaluations.len())?,
            turn_results: replay_count(evidence.view.results.len())?,
        };
        Ok(RunReplayReport {
            schema_version: RUN_REPLAY_SCHEMA_VERSION,
            run_id,
            task_id: evidence.task.id,
            organization_revision_id: evidence.organization.revision_id,
            terminal: evidence.view.run.state.is_terminal(),
            side_effect_free: true,
            evidence_sha256: format!("{:x}", Sha256::digest(encoded)),
            counts,
            invariants,
            evidence,
        })
    }

    /// Records immutable objective evaluation evidence and publishes its event.
    ///
    /// # Errors
    ///
    /// Returns an error when the report does not cover its run's success
    /// conditions or violates the organization's evaluator/evidence policy.
    pub fn record_evaluation(
        &self,
        report: &EvaluationReport,
    ) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.record_evaluation(report)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Deterministically derives and atomically records supervision findings for one evaluation.
    ///
    /// Exact retries, including evaluations with no findings, are observation-only.
    ///
    /// # Errors
    ///
    /// Returns an error when the evaluation/run cannot be loaded or durable evidence conflicts.
    pub fn supervise_evaluation(
        &self,
        evaluation_id: mcg_domain::EvaluationId,
    ) -> Result<AutomaticSupervisionMutation, RuntimeError> {
        if let Some(mutation) = self.store()?.automatic_supervision(evaluation_id)? {
            return Ok(mutation);
        }
        let evaluation = self
            .store()?
            .evaluation(evaluation_id)?
            .ok_or_else(|| StorageError::Conflict("evaluation was not found".into()))?;
        let view = self.agent_run_view(evaluation.run_id)?;
        let failed_instances = u32::try_from(
            view.instances
                .iter()
                .filter(|instance| instance.state == AgentState::Failed)
                .count(),
        )
        .map_err(|_| StorageError::InvalidData("failed instance count exceeds u32".into()))?;
        let exhausted_leases = u32::try_from(
            view.leases
                .iter()
                .filter(|lease| lease.budget.is_exhausted())
                .count(),
        )
        .map_err(|_| StorageError::InvalidData("exhausted lease count exceeds u32".into()))?;
        let snapshot = RunSupervisionSnapshot {
            task_id: evaluation.task_id,
            run_id: evaluation.run_id,
            organization_revision_id: view.run.organization_revision_id,
            run_state: view.run.state,
            metrics: view.run.metrics,
            failed_instances,
            exhausted_leases,
            evaluation: evaluation.clone(),
        };
        let findings = SupervisionAnalyzer::analyze(&snapshot);
        let analysis_bytes =
            serde_json::to_vec(&(&snapshot, &findings)).map_err(StorageError::from)?;
        let analysis_sha256 = format!("{:x}", Sha256::digest(&analysis_bytes));
        let observations = findings
            .into_iter()
            .enumerate()
            .map(|(index, finding)| {
                let finding_bytes =
                    serde_json::to_vec(&(evaluation.id, &analysis_sha256, index, &finding))?;
                let digest = Sha256::digest(finding_bytes);
                let mut stable_bytes = [0_u8; 16];
                stable_bytes.copy_from_slice(&digest[..16]);
                Ok(SupervisionObservation {
                    schema_version: mcg_agents::SUPERVISION_SCHEMA_VERSION,
                    id: SupervisionObservationId::from_stable_bytes(stable_bytes),
                    loop_kind: finding.loop_kind,
                    signal: finding.signal,
                    severity_basis_points: finding.severity_basis_points,
                    recommended_intervention: finding.recommended_intervention,
                    task_id: Some(evaluation.task_id),
                    run_id: Some(evaluation.run_id),
                    organization_revision_id: Some(view.run.organization_revision_id),
                    summary: finding.summary,
                    evidence: finding.evidence,
                    observed_at: evaluation.evaluated_at,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let mutation = self.store()?.record_automatic_supervision(
            evaluation.id,
            &analysis_sha256,
            &observations,
            Utc::now(),
        )?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(mutation)
    }

    /// Processes a bounded backlog of evaluations not yet marked as supervised.
    ///
    /// # Errors
    ///
    /// Returns an error when durable evaluation or supervision state is invalid.
    pub fn supervise_pending_evaluations(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomaticSupervisionMutation>, RuntimeError> {
        let evaluation_ids = self.store()?.unsupervised_evaluation_ids(limit)?;
        evaluation_ids
            .into_iter()
            .map(|evaluation_id| self.supervise_evaluation(evaluation_id))
            .collect()
    }

    /// Converts eligible slow-loop observations into inactive, ready shadow experiments.
    ///
    /// Consumer release builds require explicit shadow enablement; development and tests enable
    /// this by default. Existing active experiments defer later observations without discarding
    /// them, and stale-baseline observations receive an immutable terminal disposition.
    ///
    /// # Errors
    ///
    /// Returns an error when durable evidence, candidate identity, or experiment state conflicts.
    #[allow(clippy::too_many_lines)] // Keeps the restart-safe propose/create/ready/mark sequence auditable.
    pub fn orchestrate_pending_candidates(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomaticExperimentOutcome>, RuntimeError> {
        if !self.user_settings().shadow_experiments {
            return Ok(Vec::new());
        }
        let observation_ids = self.store()?.pending_candidate_observation_ids(limit)?;
        let mut outcomes = Vec::new();
        for observation_id in observation_ids {
            if let Some(outcome) = self.store()?.automatic_experiment_outcome(observation_id)? {
                outcomes.push(outcome);
                continue;
            }
            let observation = self
                .store()?
                .supervision_observation(observation_id)?
                .ok_or_else(|| {
                    StorageError::Conflict("supervision observation was not found".into())
                })?;
            let Some(baseline_revision_id) = observation.organization_revision_id else {
                outcomes.push(self.record_stale_candidate_outcome(
                    observation_id,
                    "slow-loop observation has no organization revision",
                )?);
                continue;
            };
            let Some(baseline) = self.organization_revision(baseline_revision_id)? else {
                outcomes.push(self.record_stale_candidate_outcome(
                    observation_id,
                    "observed organization revision no longer exists",
                )?);
                continue;
            };
            let Some(current) = self.organization(baseline.organization_id)? else {
                outcomes.push(self.record_stale_candidate_outcome(
                    observation_id,
                    "observed organization no longer exists",
                )?);
                continue;
            };
            if current.revision_id != baseline.revision_id {
                outcomes.push(self.record_stale_candidate_outcome(
                    observation_id,
                    "organization head changed before candidate orchestration",
                )?);
                continue;
            }
            let candidate_revision_id = OrganizationRevisionId::from_stable_bytes(
                automatic_experiment_id_bytes("candidate", observation.id),
            );
            let experiment_id = ExperimentId::from_stable_bytes(automatic_experiment_id_bytes(
                "experiment",
                observation.id,
            ));
            let existing_experiment = self.experiment(experiment_id)?;
            let experiments = self.experiments(baseline.organization_id)?;
            if experiments.iter().any(|experiment| {
                experiment.id != experiment_id
                    && matches!(
                        experiment.state,
                        ExperimentState::Draft
                            | ExperimentState::Ready
                            | ExperimentState::Running
                            | ExperimentState::AwaitingDecision
                            | ExperimentState::Promoted
                    )
            }) {
                continue;
            }
            let candidate =
                if let Some(candidate) = self.organization_revision(candidate_revision_id)? {
                    if candidate.organization_id != baseline.organization_id
                        || candidate.parent_revision_id != Some(baseline.revision_id)
                    {
                        return Err(StorageError::Conflict(
                            "recovered automatic candidate does not match its baseline".into(),
                        )
                        .into());
                    }
                    candidate
                } else {
                    let history = self
                        .store()?
                        .organization_history(baseline.organization_id)?;
                    let revision = history
                        .iter()
                        .map(|genome| genome.revision)
                        .max()
                        .unwrap_or(baseline.revision)
                        .saturating_add(1);
                    MeasuredCandidateDesigner::design(
                        &baseline,
                        &observation,
                        candidate_revision_id,
                        revision,
                        observation.observed_at,
                    )
                    .map_err(|error| StorageError::InvalidData(error.to_string()))?
                };
            self.record_candidate_revision(&candidate)?;
            let mut experiment = if let Some(experiment) = existing_experiment {
                experiment
            } else {
                self.create_experiment(&automatic_experiment_definition(
                    &baseline,
                    &observation,
                    candidate_revision_id,
                    experiment_id,
                )?)?
                .experiment
            };
            if experiment.state == ExperimentState::Draft {
                experiment = self
                    .transition_experiment(
                        experiment.id,
                        ExperimentState::Draft,
                        ExperimentState::Ready,
                    )?
                    .experiment;
            }
            if experiment.state != ExperimentState::Ready {
                return Err(StorageError::Conflict(
                    "recovered automatic experiment advanced before its ready marker".into(),
                )
                .into());
            }
            let outcome = AutomaticExperimentOutcome {
                observation_id: observation.id,
                disposition: AutomaticExperimentDisposition::Ready,
                candidate_revision_id: Some(candidate.revision_id),
                experiment_id: Some(experiment.id),
                detail: "bounded candidate is ready for evaluation-only shadow execution".into(),
                processed_at: Utc::now(),
                replayed: false,
            };
            outcomes.push(
                self.store()?
                    .record_automatic_experiment_outcome(&outcome)?,
            );
        }
        Ok(outcomes)
    }

    fn record_stale_candidate_outcome(
        &self,
        observation_id: SupervisionObservationId,
        detail: &str,
    ) -> Result<AutomaticExperimentOutcome, RuntimeError> {
        Ok(self
            .store()?
            .record_automatic_experiment_outcome(&AutomaticExperimentOutcome {
                observation_id,
                disposition: AutomaticExperimentDisposition::Stale,
                candidate_revision_id: None,
                experiment_id: None,
                detail: detail.into(),
                processed_at: Utc::now(),
                replayed: false,
            })?)
    }

    /// Creates or resumes stable evaluation-only runs for automatic shadow experiments.
    ///
    /// This stage creates no host authority and makes no provider call. A separate executor may
    /// dispatch the returned running plan under the inactive-candidate tool restrictions.
    ///
    /// # Errors
    /// Returns an error when source evidence, stable run identity, or experiment state conflicts.
    pub fn start_ready_shadow_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomaticShadowLaunch>, RuntimeError> {
        if !self.user_settings().shadow_experiments {
            return Ok(Vec::new());
        }
        let experiment_ids = self.store()?.executable_automatic_experiment_ids(limit)?;
        let mut launches = Vec::with_capacity(experiment_ids.len());
        for experiment_id in experiment_ids {
            let mut experiment = self.experiment(experiment_id)?.ok_or_else(|| {
                StorageError::Conflict("automatic experiment was not found".into())
            })?;
            let observation_id = *experiment.observation_ids.first().ok_or_else(|| {
                StorageError::InvalidData("automatic experiment has no source observation".into())
            })?;
            let observation = self
                .store()?
                .supervision_observation(observation_id)?
                .ok_or_else(|| StorageError::Conflict("source observation was not found".into()))?;
            let source_run_id = observation.run_id.ok_or_else(|| {
                StorageError::InvalidData("automatic experiment has no source run".into())
            })?;
            let source = self.agent_run_view(source_run_id)?;
            if source.run.organization_revision_id != experiment.baseline_revision_id {
                return Err(StorageError::Conflict(
                    "automatic experiment source run does not use its baseline".into(),
                )
                .into());
            }
            let (mut run, replayed) =
                self.load_or_create_automatic_shadow_run(&experiment, &source)?;
            if matches!(
                run.state,
                AgentRunState::Planned | AgentRunState::Recovering
            ) {
                run =
                    self.transition_agent_run(run.id, AgentRunState::Running, run.metrics.clone())?;
            }
            if run.state != AgentRunState::Running {
                return Err(
                    StorageError::Conflict("automatic shadow run is not resumable".into()).into(),
                );
            }
            if experiment.state == ExperimentState::Ready {
                experiment = self
                    .transition_experiment(
                        experiment.id,
                        ExperimentState::Ready,
                        ExperimentState::Running,
                    )?
                    .experiment;
            }
            if experiment.state != ExperimentState::Running {
                return Err(StorageError::Conflict(
                    "automatic shadow experiment is not running".into(),
                )
                .into());
            }
            launches.push(AutomaticShadowLaunch {
                experiment,
                run,
                source_run_id,
                replayed,
            });
        }
        Ok(launches)
    }

    /// Creates one bounded, private, content-verified duplicate of the source project.
    ///
    /// The durable `preparing` claim commits before filesystem writes. A copy or verification
    /// failure becomes `uncertain`; only a complete manifest match may become `ready`.
    ///
    /// # Errors
    /// Returns an error for missing scope, unsafe source entries, exceeded bounds, or stale state.
    pub fn prepare_shadow_workspace(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<ShadowWorkspaceMutation, RuntimeError> {
        if let Some(existing) = self.store()?.shadow_workspace(experiment_id)? {
            if existing.state == ShadowWorkspaceState::Ready {
                return Ok(ShadowWorkspaceMutation {
                    workspace: existing,
                    replayed: true,
                    events: Vec::new(),
                });
            }
            return Err(
                StorageError::Conflict("shadow workspace is not safely reusable".into()).into(),
            );
        }
        let (experiment, project, source_root, data_root) =
            self.shadow_workspace_source(experiment_id)?;
        let project_id = project.project_id;
        let manifest = scan_shadow_workspace_source(&source_root, &data_root)?;
        let relative_path = format!("shadow-workspaces/{experiment_id}");
        let definition_hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    SHADOW_WORKSPACE_SCHEMA_VERSION,
                    experiment.id,
                    project.id,
                    project.fingerprint,
                    &manifest.sha256,
                    &relative_path,
                    SHADOW_MAX_FILES,
                    SHADOW_MAX_TOTAL_BYTES,
                    SHADOW_MAX_FILE_BYTES,
                    SHADOW_MAX_DEPTH,
                ))
                .map_err(StorageError::from)?
            )
        );
        let now = Utc::now();
        let preparing = ShadowWorkspace {
            schema_version: SHADOW_WORKSPACE_SCHEMA_VERSION,
            experiment_id,
            project_id,
            state: ShadowWorkspaceState::Preparing,
            definition_hash: definition_hash.clone(),
            source_fingerprint: manifest.sha256.clone(),
            relative_path: relative_path.clone(),
            file_count: 0,
            total_bytes: 0,
            content_sha256: None,
            created_at: now,
            updated_at: now,
        };
        let mutation = self.store()?.create_shadow_workspace(&preparing)?;
        for event in &mutation.events {
            let _ = self.event_sender.send(event.clone());
        }
        if mutation.replayed {
            return Err(StorageError::Conflict(
                "shadow workspace preparation is already in progress".into(),
            )
            .into());
        }
        let destination = data_root.join(&relative_path);
        let copy_result = copy_shadow_workspace(&source_root, &destination, &manifest);
        let copied = match copy_result {
            Ok(copied) => copied,
            Err(error) => {
                let failed = self.store()?.fail_shadow_workspace(
                    experiment_id,
                    "bounded private project duplication failed",
                    Utc::now(),
                )?;
                for event in &failed.events {
                    let _ = self.event_sender.send(event.clone());
                }
                return Err(error);
            }
        };
        let completed = self.store()?.complete_shadow_workspace(
            experiment_id,
            &definition_hash,
            copied.file_count,
            copied.total_bytes,
            copied.sha256,
            Utc::now(),
        )?;
        for event in &completed.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(completed)
    }

    fn shadow_workspace_source(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<
        (
            ExperimentDefinition,
            ProjectModel,
            std::path::PathBuf,
            std::path::PathBuf,
        ),
        RuntimeError,
    > {
        let experiment = self
            .experiment(experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow experiment was not found".into()))?;
        if experiment.state != ExperimentState::Running {
            return Err(StorageError::Conflict(
                "shadow workspace requires a running experiment".into(),
            )
            .into());
        }
        let observation_id = *experiment.observation_ids.first().ok_or_else(|| {
            StorageError::InvalidData("shadow experiment has no source observation".into())
        })?;
        let observation = self
            .store()?
            .supervision_observation(observation_id)?
            .ok_or_else(|| StorageError::Conflict("source observation was not found".into()))?;
        let task_id = observation.task_id.ok_or_else(|| {
            StorageError::InvalidData("shadow observation has no source task".into())
        })?;
        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("shadow source task has no selected project".into())
        })?;
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        let source_root = std::path::PathBuf::from(&project.root)
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let data_root = self
            .config
            .data_directory
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        Ok((experiment, project, source_root, data_root))
    }

    /// Executes stable automatic shadow plans with private-copy filesystem authority.
    ///
    /// A work item that already has a non-completed lease is never retried automatically. This
    /// makes a crash-interrupted or failed paid turn an explicit review boundary. Successful full
    /// plan output is captured in the local content-addressed artifact store before its immutable
    /// evaluation-only receipt is recorded.
    ///
    /// # Errors
    /// Returns an error for conflicting durable authority, provider failure, or artifact failure.
    pub async fn execute_automatic_shadow_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomaticShadowExecution>, RuntimeError> {
        let launches = self.start_ready_shadow_runs(limit)?;
        if launches.is_empty() || self.provider_route_candidates().await?.is_empty() {
            return Ok(Vec::new());
        }
        let mut executions = Vec::new();
        for launch in launches {
            self.prepare_shadow_workspace(launch.experiment.id)?;
            if let Some(receipt) = self.execute_automatic_shadow_launch(&launch).await? {
                executions.push(AutomaticShadowExecution { launch, receipt });
            }
        }
        Ok(executions)
    }

    async fn execute_automatic_shadow_launch(
        &self,
        launch: &AutomaticShadowLaunch,
    ) -> Result<Option<ShadowExecutionReceipt>, RuntimeError> {
        let task = self
            .task(launch.run.task_id)?
            .ok_or(StorageError::TaskNotFound(launch.run.task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("automatic shadow task has no selected project".into())
        })?;
        let workspace = self
            .store()?
            .shadow_workspace(launch.experiment.id)?
            .ok_or_else(|| StorageError::Conflict("shadow workspace was not found".into()))?;
        let shadow_authority = ShadowExecutionAuthority {
            experiment_id: launch.experiment.id,
            workspace_root: self.config.data_directory.join(&workspace.relative_path),
        };
        let items = self.agent_run_view(launch.run.id)?.plan.items;
        for item in items {
            let view = self.agent_run_view(launch.run.id)?;
            let completed = view
                .leases
                .iter()
                .filter(|lease| lease.state == AgentLeaseState::Completed)
                .map(|lease| lease.work_item_id)
                .collect::<BTreeSet<_>>();
            if completed.contains(&item.id) {
                continue;
            }
            if view
                .leases
                .iter()
                .any(|lease| lease.work_item_id == item.id)
                || item
                    .dependencies
                    .iter()
                    .any(|dependency| !completed.contains(dependency))
            {
                return Ok(None);
            }
            self.execute_agent_work_with_authority(
                &AgentDispatchSpec {
                    run_id: launch.run.id,
                    work_item_id: item.id,
                    project_id,
                    target_path: None,
                    context_token_budget: 8_192,
                    requested_tools: Vec::new(),
                    estimated_output_tokens: 2_048,
                    max_model_calls: 4,
                },
                Some(shadow_authority.clone()),
                false,
            )
            .await?;
        }
        let view = self.agent_run_view(launch.run.id)?;
        if view.plan.items.iter().any(|item| {
            !view.leases.iter().any(|lease| {
                lease.work_item_id == item.id && lease.state == AgentLeaseState::Completed
            })
        }) {
            return Ok(None);
        }
        let bytes = serde_json::to_vec(&view.results).map_err(StorageError::from)?;
        let artifacts = self.config.data_directory.join("artifacts");
        let artifact = ArtifactStore::open(&artifacts)
            .map_err(runtime_tool_error)?
            .capture(&bytes, "application/vnd.mcg.shadow-results+json")
            .map_err(runtime_tool_error)?;
        let receipt = ShadowExecutionReceipt {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            experiment_id: launch.experiment.id,
            source_task_id: launch.run.task_id,
            candidate_run_id: launch.run.id,
            authority: ShadowAuthority::EvaluationOnly,
            output_artifact_sha256: artifact.sha256,
            completed_at: Utc::now(),
        };
        self.record_shadow_execution(&receipt)?;
        Ok(Some(receipt))
    }

    /// Converts completed automatic shadow output into paired evidence and a durable decision.
    ///
    /// Conditions without an objective-specific isolated verifier are scored honestly as
    /// inconclusive. They therefore produce a benchmark regression and automatic rejection while
    /// retaining the provider, tool, workspace, and evaluation evidence for review.
    ///
    /// # Errors
    /// Returns an error for missing source evidence, invalid independent authority, or conflicts.
    pub async fn assess_automatic_shadow_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomaticShadowAssessment>, RuntimeError> {
        let experiment_ids = self.store()?.assessable_automatic_experiment_ids(limit)?;
        let mut assessments = Vec::with_capacity(experiment_ids.len());
        for experiment_id in experiment_ids {
            assessments.push(self.assess_automatic_shadow_run(experiment_id).await?);
        }
        self.decide_automatic_experiments(limit)?;
        Ok(assessments)
    }

    async fn assess_automatic_shadow_run(
        &self,
        experiment_id: ExperimentId,
    ) -> Result<AutomaticShadowAssessment, RuntimeError> {
        let experiment = self
            .experiment(experiment_id)?
            .ok_or_else(|| StorageError::Conflict("automatic experiment was not found".into()))?;
        let receipt = self
            .shadow_executions(experiment.id)?
            .into_iter()
            .next()
            .ok_or_else(|| StorageError::Conflict("shadow receipt was not found".into()))?;
        let observation_id = *experiment.observation_ids.first().ok_or_else(|| {
            StorageError::InvalidData("automatic experiment has no source observation".into())
        })?;
        let observation = self
            .store()?
            .supervision_observation(observation_id)?
            .ok_or_else(|| StorageError::Conflict("source observation was not found".into()))?;
        let source_run_id = observation.run_id.ok_or_else(|| {
            StorageError::InvalidData("automatic observation has no source run".into())
        })?;
        let source = self.agent_run_view(source_run_id)?;
        let baseline = self.source_evaluation_for_observation(&source, observation_id)?;
        let candidate = self.agent_run_view(receipt.candidate_run_id)?;
        let evaluation = self
            .isolated_shadow_evaluation(&candidate, &receipt, &baseline)
            .await?;
        let benchmark = self.automatic_shadow_benchmark(
            &experiment,
            &source,
            &baseline,
            &candidate,
            &evaluation,
            evaluation.evaluated_at,
        )?;
        self.record_experiment_benchmark(&benchmark)?;
        Ok(AutomaticShadowAssessment {
            evaluation,
            benchmark,
            experiment,
        })
    }

    fn source_evaluation_for_observation(
        &self,
        source: &AgentRunView,
        observation_id: SupervisionObservationId,
    ) -> Result<EvaluationReport, RuntimeError> {
        for evaluation in &source.evaluations {
            if self
                .store()?
                .automatic_supervision(evaluation.id)?
                .is_some_and(|mutation| {
                    mutation
                        .observations
                        .iter()
                        .any(|observation| observation.id == observation_id)
                })
            {
                return Ok(evaluation.clone());
            }
        }
        Err(StorageError::Conflict(
            "automatic observation has no immutable source evaluation".into(),
        )
        .into())
    }

    fn baseline_shadow_service_command(
        &self,
        candidate: &AgentRunView,
        receipt: &ShadowExecutionReceipt,
        baseline: &EvaluationReport,
        condition: &SuccessCondition,
    ) -> Result<Option<String>, RuntimeError> {
        if candidate.run.task_id != receipt.source_task_id
            || baseline.task_id != receipt.source_task_id
            || baseline
                .conditions
                .iter()
                .filter(|result| result.condition == *condition && result.passed)
                .count()
                != 1
        {
            return Ok(None);
        }
        let Some(checkpoint) = self
            .store()?
            .consumer_evaluation_checkpoint(receipt.source_task_id)?
        else {
            return Ok(None);
        };
        if checkpoint.state != ConsumerEvaluationCheckpointState::Completed
            || checkpoint.run_id != baseline.run_id
        {
            return Ok(None);
        }
        let Some(task) = self.task(receipt.source_task_id)? else {
            return Ok(None);
        };
        let Some(project_id) = task.project_id else {
            return Ok(None);
        };
        let Some(snapshot) = self
            .store()?
            .project_snapshot(checkpoint.project_snapshot_id)?
        else {
            return Ok(None);
        };
        if snapshot.project_id != project_id {
            return Ok(None);
        }
        Ok(baseline_service_command(&snapshot).map(str::to_owned))
    }

    #[allow(clippy::too_many_lines)] // Keep the complete independent evidence derivation auditable.
    async fn isolated_shadow_evaluation(
        &self,
        candidate: &AgentRunView,
        receipt: &ShadowExecutionReceipt,
        baseline: &EvaluationReport,
    ) -> Result<EvaluationReport, RuntimeError> {
        let evaluation_id = EvaluationId::from_stable_bytes(automatic_shadow_evidence_id_bytes(
            "evaluation",
            receipt.experiment_id,
        ));
        if let Some(existing) = candidate
            .evaluations
            .iter()
            .find(|evaluation| evaluation.id == evaluation_id)
        {
            return Ok(existing.clone());
        }
        let organization = self
            .organization_revision(candidate.run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                candidate.run.organization_revision_id,
            ))?;
        let evaluator_roles = organization
            .roles
            .iter()
            .filter(|role| role.kind == RoleKind::Evaluator)
            .map(|role| role.id)
            .collect::<BTreeSet<_>>();
        let evaluator_instance_id = candidate
            .instances
            .iter()
            .find(|instance| {
                instance.state == AgentState::Completed
                    && evaluator_roles.contains(&instance.role_id)
            })
            .map(|instance| instance.id)
            .ok_or_else(|| {
                StorageError::Conflict("shadow run has no completed independent evaluator".into())
            })?;
        let workspace = self
            .store()?
            .shadow_workspace(receipt.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow workspace was not found".into()))?;
        if workspace.state != ShadowWorkspaceState::Retained
            || workspace.experiment_id != receipt.experiment_id
        {
            return Err(StorageError::Conflict(
                "shadow evaluation requires a retained private workspace".into(),
            )
            .into());
        }
        let data_root = self
            .config
            .data_directory
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let workspace_root = data_root
            .join(&workspace.relative_path)
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if !workspace_root.starts_with(&data_root) {
            return Err(StorageError::Conflict(
                "retained shadow workspace escaped the private data directory".into(),
            )
            .into());
        }
        let artifact_root = self.config.data_directory.join("artifacts");
        let verifier = Arc::new(
            FilesystemExecutor::open([workspace_root.clone()], &artifact_root)
                .map_err(runtime_tool_error)?,
        );
        let artifacts = ArtifactStore::open(&artifact_root).map_err(runtime_tool_error)?;
        let research = ResearchExecutor::new(Arc::new(
            HttpExecutor::new(
                Arc::new(RejectingCredentialResolver),
                verifier.clone(),
                &artifact_root,
            )
            .map_err(runtime_tool_error)?,
        ));
        let mut conditions = Vec::new();
        let mut evidence = vec![EvaluationEvidence {
            method: "isolated_shadow_execution".into(),
            artifact_sha256: Some(receipt.output_artifact_sha256.clone()),
            event_sequence: None,
            detail: "Provider and bounded filesystem-tool evidence was retained under durable private-workspace authority.".into(),
        }];
        let mut condition_support = Vec::new();
        let mut supported = 0_u32;
        for condition in candidate
            .plan
            .items
            .iter()
            .flat_map(|item| item.success_conditions.iter())
        {
            if !conditions
                .iter()
                .any(|result: &ConditionResult| result.condition == *condition)
            {
                let condition_is_supported = if matches!(
                    condition,
                    SuccessCondition::FileExists { .. }
                ) {
                    let (result, verification) = verify_isolated_file_condition(
                        verifier.as_ref(),
                        candidate.run.task_id,
                        &workspace_root,
                        condition,
                    )
                    .await?;
                    conditions.push(result);
                    evidence.push(verification);
                    true
                } else if objective_command(condition).is_some() {
                    let (result, verification, is_supported) = verify_isolated_command_condition(
                        self.shadow_command_verifier.as_ref(),
                        &artifacts,
                        &workspace_root,
                        condition,
                    )
                    .await;
                    conditions.push(result);
                    evidence.push(verification);
                    is_supported
                } else if matches!(condition, SuccessCondition::ResearchSources { .. }) {
                    let baseline_urls =
                        baseline_research_source_urls(&artifacts, baseline, condition);
                    let (result, verification, is_supported) = verify_isolated_research_condition(
                        verifier.as_ref(),
                        &research,
                        candidate.run.task_id,
                        &workspace_root,
                        condition,
                        baseline_urls.as_ref(),
                    )
                    .await?;
                    conditions.push(result);
                    evidence.push(verification);
                    is_supported
                } else if matches!(
                    condition,
                    SuccessCondition::HttpResponds { .. }
                        | SuccessCondition::ServiceHealthy { .. }
                        | SuccessCondition::PortListening { .. }
                        | SuccessCondition::PageRenders { .. }
                        | SuccessCondition::Accessibility { .. }
                        | SuccessCondition::ElementExists { .. }
                        | SuccessCondition::BrowserWorkflow { .. }
                        | SuccessCondition::ScreenshotMatches { .. }
                ) {
                    let command = self
                        .baseline_shadow_service_command(candidate, receipt, baseline, condition)?;
                    let (result, verification, is_supported) = verify_isolated_service_condition(
                        self.shadow_command_verifier.as_ref(),
                        &artifacts,
                        &workspace_root,
                        command.as_deref(),
                        condition,
                    )
                    .await;
                    conditions.push(result);
                    evidence.push(verification);
                    is_supported
                } else {
                    conditions.push(ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: "This condition has no condition-specific verifier inside the private shadow workspace.".into(),
                    });
                    false
                };
                if condition_is_supported {
                    supported = supported.saturating_add(1);
                }
                condition_support.push(condition_is_supported);
            }
        }
        let outcome = isolated_evaluation_outcome(&conditions, &condition_support);
        let regressions = isolated_evaluation_regressions(&conditions, &condition_support);
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: evaluation_id,
            run_id: candidate.run.id,
            task_id: candidate.run.task_id,
            evaluator_instance_id: Some(evaluator_instance_id),
            conditions,
            outcome,
            confidence: ConfidenceEstimate::new(10_000, supported.max(1))
                .map_err(|error| StorageError::InvalidData(error.to_string()))?,
            regressions,
            evidence,
            evaluated_at: Utc::now(),
        };
        self.record_evaluation(&report)?;
        Ok(report)
    }

    fn automatic_shadow_benchmark(
        &self,
        experiment: &ExperimentDefinition,
        source: &AgentRunView,
        baseline: &EvaluationReport,
        candidate: &AgentRunView,
        evaluation: &EvaluationReport,
        measured_at: DateTime<Utc>,
    ) -> Result<ExperimentBenchmark, RuntimeError> {
        let artifacts = ArtifactStore::open(self.config.data_directory.join("artifacts"))
            .map_err(runtime_tool_error)?;
        let baseline_bytes = serde_json::to_vec(baseline).map_err(StorageError::from)?;
        let candidate_bytes = serde_json::to_vec(evaluation).map_err(StorageError::from)?;
        let baseline_artifact = artifacts
            .capture(&baseline_bytes, "application/vnd.mcg.evaluation+json")
            .map_err(runtime_tool_error)?;
        let candidate_artifact = artifacts
            .capture(&candidate_bytes, "application/vnd.mcg.evaluation+json")
            .map_err(runtime_tool_error)?;
        Ok(ExperimentBenchmark {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            id: ExperimentBenchmarkId::from_stable_bytes(automatic_shadow_evidence_id_bytes(
                "benchmark",
                experiment.id,
            )),
            experiment_id: experiment.id,
            scenario_key: format!("automatic/task/{}", source.run.task_id),
            baseline: BenchmarkArmResult {
                organization_revision_id: source.run.organization_revision_id,
                run_id: source.run.id,
                evaluation_id: baseline.id,
                outcome: baseline.outcome,
                confidence: baseline.confidence,
                metrics: source.run.metrics.clone(),
                evidence_sha256: vec![baseline_artifact.sha256],
            },
            candidate: BenchmarkArmResult {
                organization_revision_id: candidate.run.organization_revision_id,
                run_id: candidate.run.id,
                evaluation_id: evaluation.id,
                outcome: evaluation.outcome,
                confidence: evaluation.confidence,
                metrics: candidate.run.metrics.clone(),
                evidence_sha256: vec![candidate_artifact.sha256],
            },
            regressions: evaluation.regressions.clone(),
            measured_at,
        })
    }

    fn decide_automatic_experiments(&self, limit: usize) -> Result<(), RuntimeError> {
        let experiment_ids = self.store()?.decidable_automatic_experiment_ids(limit)?;
        for experiment_id in experiment_ids {
            let mut experiment = self.experiment(experiment_id)?.ok_or_else(|| {
                StorageError::Conflict("automatic experiment was not found".into())
            })?;
            let benchmarks = self.experiment_benchmarks(experiment.id)?;
            if experiment.state == ExperimentState::Running {
                experiment = self
                    .transition_experiment(
                        experiment.id,
                        ExperimentState::Running,
                        ExperimentState::AwaitingDecision,
                    )?
                    .experiment;
            }
            let regression_count = benchmarks
                .iter()
                .map(|benchmark| benchmark.regressions.len())
                .sum::<usize>();
            let evidence_matches = benchmarks
                .iter()
                .map(|benchmark| {
                    Ok(
                        self.benchmark_arm_matches_durable_evidence(&benchmark.baseline)?
                            && self.benchmark_arm_matches_durable_evidence(&benchmark.candidate)?,
                    )
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?
                .into_iter()
                .all(|matches| matches);
            let promotable = benchmarks.len() >= usize::from(experiment.required_benchmarks)
                && regression_count <= usize::from(experiment.maximum_regressions)
                && evidence_matches
                && benchmarks.iter().all(|benchmark| {
                    benchmark.candidate.outcome == EvaluationOutcome::Passed
                        && benchmark.candidate.confidence.basis_points
                            >= experiment.minimum_confidence.basis_points
                });
            let kind = if promotable {
                ExperimentDecisionKind::Promote
            } else {
                ExperimentDecisionKind::Reject
            };
            let (from_revision_id, to_revision_id) = if promotable {
                (
                    experiment.baseline_revision_id,
                    experiment.candidate_revision_id,
                )
            } else {
                (
                    experiment.candidate_revision_id,
                    experiment.baseline_revision_id,
                )
            };
            self.decide_experiment(&ExperimentDecision {
                schema_version: EXPERIMENT_SCHEMA_VERSION,
                id: ExperimentDecisionId::from_stable_bytes(automatic_shadow_evidence_id_bytes(
                    "decision",
                    experiment.id,
                )),
                experiment_id: experiment.id,
                kind,
                from_revision_id,
                to_revision_id,
                benchmark_ids: benchmarks.iter().map(|benchmark| benchmark.id).collect(),
                trigger_evaluation_ids: Vec::new(),
                prospective_analysis: ProspectiveAnalysis {
                    expected_benefits: vec![experiment.hypothesis.clone()],
                    expected_risks: vec![
                        "Candidate changes remain inactive until paired evidence satisfies policy."
                            .into(),
                    ],
                    historical_evidence: experiment
                        .observation_ids
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                },
                reason: if promotable {
                    "Automatic paired evidence satisfies every configured promotion gate.".into()
                } else {
                    "Automatic paired evidence does not satisfy the configured promotion gates."
                        .into()
                },
                decided_at: benchmarks
                    .iter()
                    .map(|benchmark| benchmark.measured_at)
                    .max()
                    .unwrap_or(experiment.updated_at),
            })?;
        }
        Ok(())
    }

    fn benchmark_arm_matches_durable_evidence(
        &self,
        arm: &BenchmarkArmResult,
    ) -> Result<bool, RuntimeError> {
        let Some(evaluation) = self.store()?.evaluation(arm.evaluation_id)? else {
            return Ok(false);
        };
        let Some(run) = self.store()?.agent_run(arm.run_id)? else {
            return Ok(false);
        };
        Ok(evaluation.run_id == arm.run_id
            && evaluation.outcome == arm.outcome
            && evaluation.confidence == arm.confidence
            && run.organization_revision_id == arm.organization_revision_id
            && run.metrics == arm.metrics)
    }

    /// Reverts promoted automatic candidates after a durable post-promotion regression.
    ///
    /// # Errors
    /// Returns an error when regression provenance, revision direction, or decision state conflicts.
    pub fn revert_regressing_automatic_experiments(
        &self,
        limit: usize,
    ) -> Result<Vec<ExperimentMutation>, RuntimeError> {
        let candidates = self.store()?.automatic_reversion_candidates(limit)?;
        let mut mutations = Vec::with_capacity(candidates.len());
        for (experiment_id, evaluation_id) in candidates {
            let experiment = self.experiment(experiment_id)?.ok_or_else(|| {
                StorageError::Conflict("automatic experiment was not found".into())
            })?;
            let evaluation = self.store()?.evaluation(evaluation_id)?.ok_or_else(|| {
                StorageError::Conflict("reversion evaluation was not found".into())
            })?;
            let benchmarks = self.experiment_benchmarks(experiment.id)?;
            mutations.push(self.decide_experiment(&ExperimentDecision {
                schema_version: EXPERIMENT_SCHEMA_VERSION,
                id: ExperimentDecisionId::from_stable_bytes(automatic_shadow_evidence_id_bytes(
                    "reversion",
                    experiment.id,
                )),
                experiment_id: experiment.id,
                kind: ExperimentDecisionKind::Revert,
                from_revision_id: experiment.candidate_revision_id,
                to_revision_id: experiment.baseline_revision_id,
                benchmark_ids: benchmarks.iter().map(|benchmark| benchmark.id).collect(),
                trigger_evaluation_ids: vec![evaluation.id],
                prospective_analysis: ProspectiveAnalysis {
                    expected_benefits: vec![
                        "Restore the last measured baseline after a candidate regression.".into(),
                    ],
                    expected_risks: vec![
                        "The candidate's intended improvement is disabled pending new evidence."
                            .into(),
                    ],
                    historical_evidence: vec![evaluation.id.to_string()],
                },
                reason: "A durable post-promotion evaluation measured candidate regression.".into(),
                decided_at: evaluation.evaluated_at,
            })?);
        }
        Ok(mutations)
    }

    fn load_or_create_automatic_shadow_run(
        &self,
        experiment: &ExperimentDefinition,
        source: &AgentRunView,
    ) -> Result<(AgentRun, bool), RuntimeError> {
        let candidate_run_id =
            RunId::from_stable_bytes(automatic_shadow_run_id_bytes(experiment.id));
        if let Some(run) = self.store()?.agent_run(candidate_run_id)? {
            if run.task_id != source.run.task_id
                || run.organization_revision_id != experiment.candidate_revision_id
            {
                return Err(StorageError::Conflict(
                    "stable shadow run identity was reused with different authority".into(),
                )
                .into());
            }
            return Ok((run, true));
        }
        let candidate = self
            .organization_revision(experiment.candidate_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                experiment.candidate_revision_id,
            ))?;
        let task = self
            .task(source.run.task_id)?
            .ok_or(StorageError::TaskNotFound(source.run.task_id))?;
        let work = source
            .plan
            .items
            .iter()
            .map(|item| {
                let role = candidate
                    .roles
                    .iter()
                    .find(|role| role.id == item.assigned_role)
                    .ok_or_else(|| {
                        StorageError::InvalidData(
                            "shadow source role is absent from candidate".into(),
                        )
                    })?;
                Ok(PlannedWork {
                    id: item.id,
                    goal: item.goal.clone(),
                    role: role.kind,
                    dependencies: item.dependencies.clone(),
                    success_conditions: item.success_conditions.clone(),
                    risk_notes: item.risk_notes.clone(),
                    budget_weight: 1,
                    estimated_useful_seconds: 60,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let plan = mcg_agents::ExecutivePlanner::default()
            .plan(
                &candidate,
                &PlanningRequest {
                    run_id: candidate_run_id,
                    task: task.clone(),
                    complexity: if work.len() == 1 {
                        TaskComplexity::Atomic
                    } else {
                        TaskComplexity::Compound
                    },
                    work,
                    require_independent_evaluation: true,
                },
                experiment.updated_at,
            )
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: candidate_run_id,
            task_id: source.run.task_id,
            organization_revision_id: experiment.candidate_revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: experiment.updated_at,
            updated_at: experiment.updated_at,
            completed_at: None,
        };
        self.create_agent_run(&run, &plan)?;
        Ok((run, false))
    }

    /// Loads a consumer evaluation checkpoint when objective execution has
    /// started or is waiting for approval.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn consumer_evaluation_checkpoint(
        &self,
        task_id: TaskId,
    ) -> Result<Option<ConsumerEvaluationCheckpoint>, RuntimeError> {
        Ok(self.store()?.consumer_evaluation_checkpoint(task_id)?)
    }

    /// Restores an owner-approved objective evaluation checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error unless the matching stable request is approved.
    pub fn resume_consumer_evaluation(
        &self,
        task_id: TaskId,
    ) -> Result<ConsumerEvaluationCheckpoint, RuntimeError> {
        let (checkpoint, events) = self
            .store()?
            .resume_consumer_evaluation(task_id, Utc::now())?;
        for event in events {
            let _ = self.event_sender.send(event);
        }
        Ok(checkpoint)
    }

    /// Starts a fresh objective request identity after denial/cancellation.
    ///
    /// Returns `None` when the task is blocked for ordinary agent work.
    ///
    /// # Errors
    ///
    /// Returns an error for stale checkpoint/task/run state.
    pub fn retry_consumer_evaluation(
        &self,
        task_id: TaskId,
    ) -> Result<Option<TaskEnvelope>, RuntimeError> {
        let Some((task, events)) = self
            .store()?
            .retry_consumer_evaluation(task_id, Utc::now())?
        else {
            return Ok(None);
        };
        for event in events {
            let _ = self.event_sender.send(event);
        }
        Ok(Some(task))
    }

    /// Executes supported consumer success conditions through the governed
    /// tool broker, records objective evidence, and atomically settles the
    /// launched task/run from the resulting independent evaluation report.
    ///
    /// # Errors
    ///
    /// Returns an error when durable authority is incomplete or inconsistent,
    /// a discovered command drifts, or governed process execution cannot run.
    #[allow(clippy::too_many_lines)] // Keep evidence construction adjacent to governed execution.
    pub async fn evaluate_and_settle_consumer_run(
        &self,
        task_id: TaskId,
    ) -> Result<ConsumerEvaluationProgress, RuntimeError> {
        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("consumer evaluation requires a selected project".into())
        })?;
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        let run_id = self
            .task_launch_run_id(task_id)?
            .ok_or_else(|| StorageError::Conflict("task has no consumer launch".into()))?;
        let view = self.agent_run_view(run_id)?;
        let organization = self
            .organization_revision(view.run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                view.run.organization_revision_id,
            ))?;
        let evaluator_roles = organization
            .roles
            .iter()
            .filter(|role| role.kind == RoleKind::Evaluator)
            .map(|role| role.id)
            .collect::<Vec<_>>();
        let evaluator_instance_id = view
            .instances
            .iter()
            .find(|instance| {
                instance.state == AgentState::Completed
                    && evaluator_roles.contains(&instance.role_id)
            })
            .map(|instance| instance.id)
            .ok_or_else(|| {
                StorageError::Conflict(
                    "consumer evaluation requires a completed independent evaluator".into(),
                )
            })?;
        let mut conditions = Vec::new();
        for item in &view.plan.items {
            for condition in &item.success_conditions {
                if !conditions.contains(condition) {
                    conditions.push(condition.clone());
                }
            }
        }
        if conditions.is_empty() {
            return Err(StorageError::InvalidData(
                "consumer run has no success conditions to evaluate".into(),
            )
            .into());
        }
        let existing_checkpoint = self.store()?.consumer_evaluation_checkpoint(task_id)?;
        let mut checkpoint = existing_checkpoint
            .clone()
            .unwrap_or(ConsumerEvaluationCheckpoint {
                schema_version: AGENT_RUN_SCHEMA_VERSION,
                task_id,
                run_id,
                project_snapshot_id: project.id,
                state: ConsumerEvaluationCheckpointState::Running,
                next_condition_index: 0,
                attempt: 0,
                request_id: None,
                batch_approved: false,
                results: Vec::new(),
                evidence: Vec::new(),
                regressions: Vec::new(),
                objective_failures: 0,
                unsupported: 0,
                executed: 0,
                updated_at: Utc::now(),
            });
        if checkpoint.run_id != run_id
            || checkpoint.project_snapshot_id != project.id
            || checkpoint.state != ConsumerEvaluationCheckpointState::Running
            || checkpoint.next_condition_index > conditions.len()
        {
            return Err(StorageError::Conflict(
                "consumer evaluation checkpoint is stale or not resumable".into(),
            )
            .into());
        }
        if existing_checkpoint.is_none() {
            let event = self
                .store()?
                .save_consumer_evaluation_checkpoint(&checkpoint)?;
            let _ = self.event_sender.send(event);
        }
        let autonomy = self.user_settings().autonomy;
        if autonomy == AutonomyPreset::Guided
            && !checkpoint.batch_approved
            && conditions
                .iter()
                .any(|condition| objective_condition_label(condition).is_some())
        {
            let request_id = objective_batch_request_id(run_id, checkpoint.attempt, &conditions)?;
            let control = self.tool_control();
            let approval = control
                .find_for(request_id)
                .await
                .map_err(runtime_tool_error)?;
            let approval_id = if let Some(approval) = approval {
                if approval.state != ApprovalState::Pending {
                    return Err(StorageError::Conflict(
                        "guided evaluation batch approval is not pending".into(),
                    )
                    .into());
                }
                approval.id
            } else {
                let approval_id = ApprovalId::new();
                let checks = conditions
                    .iter()
                    .filter_map(objective_condition_label)
                    .collect::<Vec<_>>()
                    .join(", ");
                control
                    .create(ApprovalRequest {
                        schema_version: TOOL_SCHEMA_VERSION,
                        id: approval_id,
                        tool_request_id: request_id,
                        task_id,
                        action: "evaluation.run_discovered_checks".into(),
                        resource: project.root.clone(),
                        reason: format!(
                            "Guided mode requires approval to run these exact objective checks: {checks}"
                        ),
                        state: ApprovalState::Pending,
                        requested_at: Utc::now(),
                        expires_at: Utc::now() + Duration::hours(24),
                    })
                    .await
                    .map_err(runtime_tool_error)?;
                approval_id
            };
            checkpoint.state = ConsumerEvaluationCheckpointState::WaitingForApproval;
            checkpoint.request_id = Some(request_id);
            checkpoint.updated_at = Utc::now();
            let (_task, events) = self
                .store()?
                .wait_consumer_evaluation_approval(&checkpoint)?;
            for event in events {
                let _ = self.event_sender.send(event);
            }
            return Ok(ConsumerEvaluationProgress::AwaitingApproval { approval_id });
        }
        let root = std::path::PathBuf::from(&project.root);
        let artifacts = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let credentials = Arc::new(RejectingCredentialResolver);
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifacts).map_err(runtime_tool_error)?,
        );
        let process = Arc::new(
            ProcessExecutor::new([root.clone()], credentials.clone())
                .map_err(runtime_tool_error)?,
        );
        let http = Arc::new(
            HttpExecutor::new(credentials, filesystem.clone(), &artifacts)
                .map_err(runtime_tool_error)?,
        );
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: http.clone(),
            browser: Arc::new(
                BrowserExecutor::with_filesystem(
                    self.config.browser_executable.clone(),
                    &artifacts,
                    filesystem.clone(),
                )
                .map_err(runtime_tool_error)?,
            ),
            research: Arc::new(ResearchExecutor::new(http)),
            system: Arc::new(SystemExecutor),
            git: Arc::new(
                GitExecutor::new(process, [root.clone()], &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            memory: Arc::new(MemoryExecutor::new(InMemoryMemoryRepository::default())),
            external: Arc::new(RuntimeMcpExecutor {
                store: self.store.clone(),
                credentials: self.mcp_credentials.clone(),
            }),
        };
        let mut capabilities = CapabilityPolicy::project(&root);
        for command in &project.commands {
            if let Some(executable) = command
                .command
                .split_ascii_whitespace()
                .next()
                .map(platform_process_executable)
                && !capabilities
                    .allowed_executables
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(&executable))
            {
                capabilities.allowed_executables.push(executable);
            }
        }
        for condition in &conditions {
            if let SuccessCondition::HttpResponds { url, .. }
            | SuccessCondition::ServiceHealthy { endpoint: url }
            | SuccessCondition::PageRenders { url }
            | SuccessCondition::Accessibility { url }
            | SuccessCondition::ElementExists { url, .. }
            | SuccessCondition::ScreenshotMatches { url, .. } = condition
            {
                let origin = objective_network_origin(url)?;
                if !capabilities
                    .allowed_network_origins
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(&origin))
                {
                    capabilities.allowed_network_origins.push(origin);
                }
            }
            if let SuccessCondition::PortListening { endpoint } = condition {
                let origin = objective_tcp_origin(endpoint)?;
                if !capabilities
                    .allowed_network_origins
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(&origin))
                {
                    capabilities.allowed_network_origins.push(origin);
                }
            }
        }
        let control = self.tool_control();
        let broker_preset = if autonomy == AutonomyPreset::Guided && checkpoint.batch_approved {
            AutonomyPreset::Balanced
        } else {
            autonomy
        };
        let broker = ToolBroker::new(
            host.clone(),
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: broker_preset,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: capabilities.clone(),
                leases: Arc::new(control.clone()),
                redactor: OutputRedactor::default(),
            },
        );
        for (index, condition) in conditions
            .iter()
            .enumerate()
            .skip(checkpoint.next_condition_index)
        {
            let mut preparation_error = None;
            let request_kind = if let Some(command) = objective_command(condition) {
                if !project
                    .commands
                    .iter()
                    .any(|discovered| discovered.command == command)
                {
                    return Err(StorageError::Conflict(
                        "planned objective command no longer matches project discovery".into(),
                    )
                    .into());
                }
                ToolRequestKind::ProcessExec {
                    options: objective_process_options(command, &project.root)?,
                }
            } else if let SuccessCondition::HttpResponds { url, .. } = condition {
                ToolRequestKind::HttpRequest {
                    method: "GET".into(),
                    url: url.clone(),
                    headers: Vec::new(),
                    body: None,
                    max_response_bytes: 1_048_576,
                }
            } else if let SuccessCondition::ServiceHealthy { endpoint } = condition {
                ToolRequestKind::HttpRequest {
                    method: "GET".into(),
                    url: endpoint.clone(),
                    headers: Vec::new(),
                    body: None,
                    max_response_bytes: 1_048_576,
                }
            } else if let SuccessCondition::PortListening { endpoint } = condition {
                ToolRequestKind::TcpConnect {
                    endpoint: endpoint.clone(),
                }
            } else if let SuccessCondition::PageRenders { url } = condition {
                ToolRequestKind::BrowserRender {
                    url: url.clone(),
                    viewport_width: 1_280,
                    viewport_height: 720,
                }
            } else if let SuccessCondition::Accessibility { url } = condition {
                ToolRequestKind::BrowserAccessibility {
                    url: url.clone(),
                    viewport_width: 1_280,
                    viewport_height: 720,
                }
            } else if let SuccessCondition::ElementExists { url, selector } = condition {
                ToolRequestKind::BrowserElementExists {
                    url: url.clone(),
                    selector: selector.clone(),
                    viewport_width: 1_280,
                    viewport_height: 720,
                }
            } else if let SuccessCondition::ScreenshotMatches {
                baseline,
                expected_baseline_sha256,
                url,
                max_changed_basis_points,
                per_channel_threshold,
            } = condition
            {
                ToolRequestKind::BrowserScreenshotMatches {
                    url: url.clone(),
                    baseline_path: objective_project_file_path(&project.root, baseline)?,
                    expected_baseline_sha256: expected_baseline_sha256.clone(),
                    viewport_width: 1_280,
                    viewport_height: 720,
                    max_changed_basis_points: *max_changed_basis_points,
                    per_channel_threshold: *per_channel_threshold,
                }
            } else if let SuccessCondition::FileExists { path, .. } = condition {
                ToolRequestKind::FsVerify {
                    path: objective_project_file_path(&project.root, path)?,
                    max_bytes: 1_073_741_824,
                }
            } else if let SuccessCondition::BrowserWorkflow {
                name,
                path,
                expected_sha256,
            } = condition
            {
                let absolute_path = objective_project_file_path(&project.root, path)?;
                let mut workflow_request = ToolRequest::new(
                    task_id,
                    format!("Read browser workflow: {path}"),
                    ToolRequestKind::FsRead {
                        path: absolute_path.clone(),
                        max_bytes: 1_048_576,
                    },
                );
                workflow_request.id = objective_stage_request_id(
                    run_id,
                    index,
                    checkpoint.attempt,
                    condition,
                    "browser-workflow",
                )?;
                workflow_request.timeout_seconds = task
                    .budget
                    .max_duration_seconds
                    .unwrap_or(1_800)
                    .clamp(1, 86_400);
                match broker.execute(&workflow_request).await {
                    Ok(outcome) if outcome.status == ToolOutcomeStatus::Succeeded => {
                        let workflow_json = outcome.stdout.clone().ok_or_else(|| {
                            ToolError::Execution(
                                "browser workflow did not contain UTF-8 JSON".into(),
                            )
                        });
                        let parsed = workflow_json
                            .as_ref()
                            .map_err(|error| ToolError::Execution(error.to_string()))
                            .and_then(|contents| {
                                serde_json::from_str::<BrowserWorkflowDocument>(contents).map_err(
                                    |error| {
                                        ToolError::Execution(format!(
                                            "browser workflow is not valid JSON: {error}"
                                        ))
                                    },
                                )
                            });
                        let artifact = outcome.artifacts.first().cloned().ok_or_else(|| {
                            ToolError::Execution(
                                "browser workflow read did not produce an immutable artifact"
                                    .into(),
                            )
                        });
                        match (workflow_json, parsed, artifact) {
                            (Ok(workflow_json), Ok(document), Ok(workflow_artifact)) => {
                                if document.name != *name {
                                    preparation_error = Some(ToolError::Execution(
                                        "browser workflow name does not match the bound condition"
                                            .into(),
                                    ));
                                }
                                if !workflow_artifact
                                    .sha256
                                    .eq_ignore_ascii_case(expected_sha256)
                                {
                                    preparation_error = Some(ToolError::Execution(
                                        "browser workflow changed after the objective was bound"
                                            .into(),
                                    ));
                                }
                                ToolRequestKind::BrowserWorkflow {
                                    workflow_path: absolute_path,
                                    expected_sha256: expected_sha256.clone(),
                                    workflow_artifact,
                                    workflow_json,
                                    document,
                                    viewport_width: 1_280,
                                    viewport_height: 720,
                                }
                            }
                            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                                preparation_error = Some(error);
                                ToolRequestKind::FsRead {
                                    path: absolute_path,
                                    max_bytes: 1_048_576,
                                }
                            }
                        }
                    }
                    Ok(outcome) => {
                        preparation_error = Some(ToolError::Execution(outcome.summary));
                        ToolRequestKind::FsRead {
                            path: absolute_path,
                            max_bytes: 1_048_576,
                        }
                    }
                    Err(error) => {
                        preparation_error = Some(error);
                        ToolRequestKind::FsRead {
                            path: absolute_path,
                            max_bytes: 1_048_576,
                        }
                    }
                }
            } else if let SuccessCondition::ResearchSources {
                evidence_path,
                minimum,
                max_age_days,
                required_scope,
            } = condition
            {
                let absolute_path = objective_project_file_path(&project.root, evidence_path)?;
                let mut evidence_request = ToolRequest::new(
                    task_id,
                    format!("Read research evidence: {evidence_path}"),
                    ToolRequestKind::FsRead {
                        path: absolute_path.clone(),
                        max_bytes: 1_048_576,
                    },
                );
                evidence_request.id = objective_stage_request_id(
                    run_id,
                    index,
                    checkpoint.attempt,
                    condition,
                    "research-evidence",
                )?;
                evidence_request.timeout_seconds = task
                    .budget
                    .max_duration_seconds
                    .unwrap_or(1_800)
                    .clamp(1, 86_400);
                match broker.execute(&evidence_request).await {
                    Ok(outcome) if outcome.status == ToolOutcomeStatus::Succeeded => {
                        let evidence_json = outcome.stdout.clone().ok_or_else(|| {
                            ToolError::Execution(
                                "research evidence file did not contain UTF-8 JSON".into(),
                            )
                        });
                        let parsed = evidence_json
                            .as_ref()
                            .map_err(|error| ToolError::Execution(error.to_string()))
                            .and_then(|contents| {
                                serde_json::from_str::<ResearchEvidenceDocument>(contents).map_err(
                                    |error| {
                                        ToolError::Execution(format!(
                                            "research evidence is not valid JSON: {error}"
                                        ))
                                    },
                                )
                            });
                        let artifact = outcome.artifacts.first().cloned().ok_or_else(|| {
                            ToolError::Execution(
                                "research evidence read did not produce an immutable artifact"
                                    .into(),
                            )
                        });
                        match (evidence_json, parsed, artifact) {
                            (Ok(evidence_json), Ok(document), Ok(evidence_artifact)) => {
                                ToolRequestKind::ResearchVerify {
                                    evidence_path: absolute_path,
                                    evidence_artifact,
                                    evidence_json,
                                    document,
                                    required_scope: required_scope.clone(),
                                    minimum_sources: *minimum,
                                    max_age_days: *max_age_days,
                                    max_response_bytes: 1_048_576,
                                }
                            }
                            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                                preparation_error = Some(error);
                                ToolRequestKind::FsRead {
                                    path: absolute_path,
                                    max_bytes: 1_048_576,
                                }
                            }
                        }
                    }
                    Ok(outcome) => {
                        preparation_error = Some(ToolError::Execution(outcome.summary));
                        ToolRequestKind::FsRead {
                            path: absolute_path,
                            max_bytes: 1_048_576,
                        }
                    }
                    Err(error) => {
                        preparation_error = Some(error);
                        ToolRequestKind::FsRead {
                            path: absolute_path,
                            max_bytes: 1_048_576,
                        }
                    }
                }
            } else {
                checkpoint.unsupported = checkpoint.unsupported.saturating_add(1);
                let detail = "No deterministic evaluator is implemented for this condition yet.";
                checkpoint.results.push(ConditionResult {
                    condition: condition.clone(),
                    passed: false,
                    detail: detail.into(),
                });
                checkpoint.evidence.push(EvaluationEvidence {
                    method: "unsupported_condition".into(),
                    artifact_sha256: None,
                    event_sequence: None,
                    detail: detail.into(),
                });
                checkpoint.next_condition_index = index.saturating_add(1);
                checkpoint.updated_at = Utc::now();
                let event = self
                    .store()?
                    .save_consumer_evaluation_checkpoint(&checkpoint)?;
                let _ = self.event_sender.send(event);
                continue;
            };
            let label = objective_condition_label(condition)
                .unwrap_or_else(|| "unsupported objective condition".into());
            let mut request = ToolRequest::new(
                task_id,
                format!("Objectively evaluate: {label}"),
                request_kind,
            );
            request.id = objective_request_id(run_id, index, checkpoint.attempt, condition)?;
            request.timeout_seconds = task
                .budget
                .max_duration_seconds
                .unwrap_or(1_800)
                .clamp(1, 86_400);
            let execution = if let Some(error) = preparation_error {
                Err(error)
            } else if let ToolRequestKind::BrowserWorkflow { document, .. } = &request.kind {
                let origins = objective_workflow_origins(document);
                match origins {
                    Err(error) => Err(error),
                    Ok(origins) => {
                        let mut workflow_capabilities = capabilities.clone();
                        for origin in origins {
                            if !workflow_capabilities
                                .allowed_network_origins
                                .contains(&origin)
                            {
                                workflow_capabilities.allowed_network_origins.push(origin);
                            }
                        }
                        ToolBroker::new(
                            host.clone(),
                            control.clone(),
                            control.clone(),
                            BrokerControls {
                                preset: broker_preset,
                                constitution: OwnerConstitution::secure_defaults(),
                                capabilities: workflow_capabilities,
                                leases: Arc::new(control.clone()),
                                redactor: OutputRedactor::default(),
                            },
                        )
                        .execute(&request)
                        .await
                    }
                }
            } else if let ToolRequestKind::ResearchVerify { document, .. } = &request.kind {
                let mut research_capabilities = capabilities.clone();
                let mut origins = BTreeSet::new();
                let mut source_urls = BTreeSet::new();
                let mut capability_error = None;
                for citation in document.claims.iter().flat_map(|claim| &claim.citations) {
                    source_urls.insert(citation.url.clone());
                    if source_urls.len() > 32 {
                        capability_error = Some(ToolError::Execution(
                            "research evidence exceeds 32 distinct source URLs".into(),
                        ));
                        break;
                    }
                    match objective_network_origin(&citation.url) {
                        Ok(origin) => {
                            origins.insert(origin);
                        }
                        Err(error) => {
                            capability_error = Some(ToolError::Execution(format!(
                                "research evidence contains an invalid source URL: {error}"
                            )));
                            break;
                        }
                    }
                }
                if let Some(error) = capability_error {
                    Err(error)
                } else {
                    for origin in origins {
                        if !research_capabilities
                            .allowed_network_origins
                            .contains(&origin)
                        {
                            research_capabilities.allowed_network_origins.push(origin);
                        }
                    }
                    ToolBroker::new(
                        host.clone(),
                        control.clone(),
                        control.clone(),
                        BrokerControls {
                            preset: broker_preset,
                            constitution: OwnerConstitution::secure_defaults(),
                            capabilities: research_capabilities,
                            leases: Arc::new(control.clone()),
                            redactor: OutputRedactor::default(),
                        },
                    )
                    .execute(&request)
                    .await
                }
            } else {
                broker.execute(&request).await
            };
            let outcome = match execution {
                Ok(outcome) => Some(outcome),
                Err(ToolError::ApprovalRequired { approval_id, .. }) => {
                    checkpoint.state = ConsumerEvaluationCheckpointState::WaitingForApproval;
                    checkpoint.request_id = Some(request.id);
                    checkpoint.updated_at = Utc::now();
                    let (_task, events) = self
                        .store()?
                        .wait_consumer_evaluation_approval(&checkpoint)?;
                    for event in events {
                        let _ = self.event_sender.send(event);
                    }
                    return Ok(ConsumerEvaluationProgress::AwaitingApproval { approval_id });
                }
                Err(ToolError::Execution(detail)) => {
                    checkpoint.executed = checkpoint.executed.saturating_add(1);
                    checkpoint.objective_failures = checkpoint.objective_failures.saturating_add(1);
                    let detail = format!("objective check could not execute: {detail}");
                    checkpoint.regressions.push(format!("{label}: {detail}"));
                    checkpoint.results.push(ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: detail.clone(),
                    });
                    checkpoint.evidence.push(EvaluationEvidence {
                        method: objective_evidence_method(condition).into(),
                        artifact_sha256: None,
                        event_sequence: None,
                        detail: format!("{label}: {detail}"),
                    });
                    None
                }
                Err(ToolError::TimedOut) => {
                    checkpoint.executed = checkpoint.executed.saturating_add(1);
                    checkpoint.objective_failures = checkpoint.objective_failures.saturating_add(1);
                    let detail = "objective check timed out".to_owned();
                    checkpoint.regressions.push(format!("{label}: {detail}"));
                    checkpoint.results.push(ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: detail.clone(),
                    });
                    checkpoint.evidence.push(EvaluationEvidence {
                        method: objective_evidence_method(condition).into(),
                        artifact_sha256: None,
                        event_sequence: None,
                        detail: format!("{label}: {detail}"),
                    });
                    None
                }
                Err(ToolError::Unavailable(detail)) => {
                    checkpoint.executed = checkpoint.executed.saturating_add(1);
                    checkpoint.unsupported = checkpoint.unsupported.saturating_add(1);
                    let detail = format!("objective evaluator is unavailable: {detail}");
                    checkpoint.results.push(ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: detail.clone(),
                    });
                    checkpoint.evidence.push(EvaluationEvidence {
                        method: objective_evidence_method(condition).into(),
                        artifact_sha256: None,
                        event_sequence: None,
                        detail: format!("{label}: {detail}"),
                    });
                    None
                }
                Err(error) => return Err(runtime_tool_error(error)),
            };
            if let Some(outcome) = outcome {
                checkpoint.executed = checkpoint.executed.saturating_add(1);
                let passed = objective_outcome_passed(condition, &outcome);
                if !passed {
                    checkpoint.objective_failures = checkpoint.objective_failures.saturating_add(1);
                }
                let detail = objective_condition_outcome_detail(condition, &outcome);
                if !passed {
                    checkpoint.regressions.push(format!("{label}: {detail}"));
                }
                checkpoint.results.push(ConditionResult {
                    condition: condition.clone(),
                    passed,
                    detail: detail.clone(),
                });
                checkpoint.evidence.push(EvaluationEvidence {
                    method: objective_evidence_method(condition).into(),
                    artifact_sha256: objective_evidence_artifact(condition, &outcome)
                        .map(|artifact| artifact.sha256.clone()),
                    event_sequence: None,
                    detail: format!("{label}: {detail}"),
                });
            }
            checkpoint.next_condition_index = index.saturating_add(1);
            checkpoint.attempt = 0;
            checkpoint.request_id = None;
            checkpoint.updated_at = Utc::now();
            let event = self
                .store()?
                .save_consumer_evaluation_checkpoint(&checkpoint)?;
            let _ = self.event_sender.send(event);
        }
        let outcome = if checkpoint.objective_failures > 0 {
            EvaluationOutcome::Failed
        } else if checkpoint.unsupported > 0 {
            EvaluationOutcome::Inconclusive
        } else {
            EvaluationOutcome::Passed
        };
        let confidence = ConfidenceEstimate::new(
            if outcome == EvaluationOutcome::Inconclusive {
                5_000
            } else {
                9_500
            },
            checkpoint.executed,
        )
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: mcg_domain::EvaluationId::new(),
            run_id,
            task_id,
            evaluator_instance_id: Some(evaluator_instance_id),
            conditions: checkpoint.results.clone(),
            outcome,
            confidence,
            regressions: checkpoint.regressions.clone(),
            evidence: checkpoint.evidence.clone(),
            evaluated_at: Utc::now(),
        };
        let events = self
            .store()?
            .settle_consumer_evaluation(&report, &checkpoint)?;
        for event in events {
            let _ = self.event_sender.send(event);
        }
        Ok(ConsumerEvaluationProgress::Settled(report))
    }

    /// Atomically records a planned run and immutable work plan.
    ///
    /// # Errors
    ///
    /// Returns an error when task, organization, plan, or budget invariants fail.
    pub fn create_agent_run(
        &self,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.create_agent_run(run, plan)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Resolves one durable external-interface request identity.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read or contains invalid identity data.
    pub fn interface_run_request(
        &self,
        interface: &str,
        request_key: &str,
    ) -> Result<Option<(TaskId, RunId)>, RuntimeError> {
        Ok(self
            .store()?
            .interface_run_request(interface, request_key)?)
    }

    /// Atomically reserves and starts an idempotent task/run requested through
    /// an external interface, then returns its complete durable view. When
    /// `consumer_launch` is true, the unique task-to-consumer-run identity is
    /// committed in the same transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the request key conflicts or any task,
    /// organization, plan, project, budget, or persistence invariant fails.
    #[allow(clippy::too_many_arguments)]
    pub fn create_interface_agent_run(
        &self,
        interface: &str,
        request_key: &str,
        request_hash: &str,
        consumer_launch: bool,
        task: &TaskEnvelope,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<InterfaceAgentRun, RuntimeError> {
        let receipt = self.store()?.create_interface_agent_run(
            interface,
            request_key,
            request_hash,
            consumer_launch,
            task,
            genome,
            run,
            plan,
        )?;
        for event in &receipt.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(InterfaceAgentRun {
            task: receipt.task,
            view: self.agent_run_view(receipt.run.id)?,
            replayed: receipt.replayed,
        })
    }

    /// Atomically starts the single consumer run associated with an existing
    /// durable task. A retry returns the original task/run view and never
    /// creates duplicate execution authority.
    ///
    /// # Errors
    ///
    /// Returns an error when task, organization, plan, project, budget, or
    /// persistence invariants fail.
    pub fn launch_task_agent_run(
        &self,
        task_id: TaskId,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<InterfaceAgentRun, RuntimeError> {
        let receipt = self
            .store()?
            .launch_task_agent_run(task_id, genome, run, plan)?;
        for event in &receipt.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(InterfaceAgentRun {
            task: receipt.task,
            view: self.agent_run_view(receipt.run.id)?,
            replayed: receipt.replayed,
        })
    }

    /// Transitions a durable agent run and publishes its event.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown runs, invalid metrics, or illegal transitions.
    pub fn transition_agent_run(
        &self,
        run_id: mcg_domain::RunId,
        next: AgentRunState,
        metrics: OrganizationalMetrics,
    ) -> Result<AgentRun, RuntimeError> {
        let (run, event) = self
            .store()?
            .transition_agent_run(run_id, next, metrics, Utc::now())?;
        let _ = self.event_sender.send(event);
        Ok(run)
    }

    /// Resolves one immutable work item into fresh bounded runtime authority.
    /// Project identity, role, dependencies, tool exposure, model routes, budgets,
    /// and organization concurrency are all derived or revalidated server-side.
    ///
    /// # Errors
    ///
    /// Returns an error for stale run state, project mismatch, unavailable routes,
    /// ungrantable tools, incomplete dependencies, or failed atomic persistence.
    #[allow(clippy::too_many_lines)] // Keep the complete server-side authority derivation visible together.
    pub async fn prepare_agent_turn(
        &self,
        spec: &AgentDispatchSpec,
    ) -> Result<PreparedAgentTurn, RuntimeError> {
        self.prepare_agent_turn_with_authority(spec, None, false)
            .await
    }

    #[allow(clippy::too_many_lines)] // Keep complete normal/shadow authority derivation together.
    async fn prepare_agent_turn_with_authority(
        &self,
        spec: &AgentDispatchSpec,
        shadow_authority: Option<&ShadowExecutionAuthority>,
        explicit_owner_retry: bool,
    ) -> Result<PreparedAgentTurn, RuntimeError> {
        if spec.context_token_budget < 64
            || spec.estimated_output_tokens == 0
            || spec.max_model_calls == 0
            || spec.max_model_calls > 32
        {
            return Err(
                StorageError::InvalidData("agent dispatch bounds are invalid".into()).into(),
            );
        }
        let view = self.agent_run_view(spec.run_id)?;
        if view.run.state != AgentRunState::Running {
            return Err(StorageError::Conflict(
                "agent work can only dispatch from a running run".into(),
            )
            .into());
        }
        let task = self
            .task(view.run.task_id)?
            .ok_or(StorageError::TaskNotFound(view.run.task_id))?;
        if task.project_id != Some(spec.project_id) {
            return Err(StorageError::InvalidData(
                "agent dispatch project does not match the task envelope".into(),
            )
            .into());
        }
        let item = view
            .plan
            .items
            .iter()
            .find(|item| item.id == spec.work_item_id)
            .cloned()
            .ok_or_else(|| StorageError::InvalidData("work item was not found".into()))?;
        let organization = self
            .organization_revision(view.run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                view.run.organization_revision_id,
            ))?;
        let role = organization
            .roles
            .iter()
            .find(|role| role.id == item.assigned_role)
            .cloned()
            .ok_or_else(|| StorageError::InvalidData("assigned role was not found".into()))?;
        let active_organization = self
            .organization(organization.organization_id)?
            .ok_or_else(|| StorageError::Conflict("active organization was not found".into()))?;
        let evaluation_only = active_organization.revision_id != organization.revision_id;
        let mut project = self
            .project(spec.project_id)?
            .ok_or(StorageError::ProjectNotFound(spec.project_id))?;
        if evaluation_only {
            let authority = shadow_authority.ok_or_else(|| {
                StorageError::Conflict(
                    "inactive candidate dispatch requires isolated shadow authority".into(),
                )
            })?;
            self.validate_shadow_execution_authority(authority, &view.run, spec.project_id)?;
            project.root = authority.workspace_root.to_string_lossy().into_owned();
        } else if shadow_authority.is_some() {
            return Err(StorageError::Conflict(
                "active organization runs cannot use shadow authority".into(),
            )
            .into());
        }
        let mut context_items = ContextAssembler::from_project_for_target(
            &project,
            spec.target_path.as_deref().unwrap_or(""),
        );
        context_items.extend(
            self.memory_claims(spec.project_id, None, true)?
                .into_iter()
                .map(|claim| ContextItem {
                    category: ContextCategory::Memory,
                    content: format!("{}.{} = {}", claim.namespace, claim.key, claim.value),
                    source: claim.provenance.source,
                    priority: if claim.state == mcg_projects::MemoryState::Durable {
                        800
                    } else {
                        500
                    },
                    estimated_tokens: 0,
                }),
        );
        let context = ContextAssembler::assemble(context_items, spec.context_token_budget);
        let context_json = serde_json::to_vec(&context)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let context_fingerprint = format!("sha256:{:x}", Sha256::digest(context_json));
        let (tools, external_tools) = if evaluation_only {
            Self::resolve_shadow_tools(role.kind, &spec.requested_tools)?
        } else {
            self.resolve_agent_tools(role.kind, &spec.requested_tools)?
        };
        let routes = self.provider_route_candidates().await?;
        if routes.is_empty() {
            return Err(ProviderError::Configuration(
                "no discovered provider models are available".into(),
            )
            .into());
        }
        let now = Utc::now();
        let lease_seconds = i64::try_from(organization.policy.default_lease_seconds)
            .map_err(|_| StorageError::InvalidData("default agent lease is too long".into()))?;
        let expires_at = now
            .checked_add_signed(Duration::seconds(lease_seconds))
            .ok_or_else(|| {
                StorageError::InvalidData("default agent lease overflows time".into())
            })?;
        let confidence = ConfidenceEstimate::new(5_000, 0)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: role.id,
            run_id: spec.run_id,
            task_id: view.run.task_id,
            state: AgentState::Created,
            model: None,
            context_fingerprint,
            confidence,
            created_at: now,
            updated_at: now,
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: item.id,
            goal: item.goal,
            budget: LeaseBudget {
                limit: item.budget,
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: tools.iter().map(|tool| tool.name.clone()).collect(),
            reporting_interval_seconds: 30,
            max_retries: 2,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now,
            expires_at,
            heartbeat_at: now,
        };
        let (running, events) = if explicit_owner_retry {
            self.store()?
                .dispatch_agent_work_for_explicit_retry(&instance, &lease, now)?
        } else {
            self.store()?.dispatch_agent_work(&instance, &lease, now)?
        };
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(PreparedAgentTurn {
            request: AgentTurnRequest {
                role,
                instance: running,
                lease,
                context,
                route_candidates: routes,
                routing_policy: RoutingPolicy::default(),
                tools,
                external_tools,
                estimated_output_tokens: spec.estimated_output_tokens,
                max_model_calls: spec.max_model_calls,
            },
            events,
            project_root: std::path::PathBuf::from(project.root),
        })
    }

    fn resolve_agent_tools(
        &self,
        role: RoleKind,
        requested: &[String],
    ) -> Result<(Vec<ToolDefinition>, Vec<ExternalToolBinding>), RuntimeError> {
        if requested.is_empty() {
            return Ok((resolve_role_tools(role)?, Vec::new()));
        }
        let allowed = role_tool_names(role);
        let external_allowed = !matches!(role, RoleKind::Executive | RoleKind::Conversation);
        let mut definitions = Vec::with_capacity(requested.len());
        let mut bindings = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for requested_name in requested {
            if !seen.insert(requested_name) {
                return Err(StorageError::InvalidData(format!(
                    "tool {requested_name} was requested more than once"
                ))
                .into());
            }
            if let Some((connection_id, remote_name)) = parse_mcp_tool_request(requested_name) {
                if !external_allowed {
                    return Err(StorageError::InvalidData(
                        "the selected role cannot receive external MCP tools".into(),
                    )
                    .into());
                }
                let connection = self
                    .store()?
                    .mcp_connection(connection_id)?
                    .ok_or_else(|| {
                        StorageError::InvalidData(format!(
                            "MCP connection {connection_id} was not found"
                        ))
                    })?;
                if !connection.enabled {
                    return Err(StorageError::InvalidData(format!(
                        "MCP connection {connection_id} is disabled"
                    ))
                    .into());
                }
                let catalog = self.store()?.mcp_catalog(connection_id)?.ok_or_else(|| {
                    StorageError::InvalidData(format!(
                        "MCP connection {connection_id} has no discovered catalog"
                    ))
                })?;
                let descriptor = catalog
                    .tools
                    .iter()
                    .find(|tool| tool.name == remote_name)
                    .ok_or_else(|| {
                        StorageError::InvalidData(format!(
                            "MCP tool {requested_name} was not discovered"
                        ))
                    })?;
                let exposed_name = exposed_mcp_tool_name(connection_id, remote_name);
                if definitions
                    .iter()
                    .any(|tool: &ToolDefinition| tool.name == exposed_name)
                {
                    return Err(StorageError::InvalidData(
                        "MCP tool exposure produced a duplicate identity".into(),
                    )
                    .into());
                }
                let (read_only, destructive, open_world) =
                    mcp_tool_classification(&descriptor.annotations);
                definitions.push(ToolDefinition {
                    name: exposed_name.clone(),
                    description: descriptor.description.clone(),
                    parameters: descriptor.input_schema.clone(),
                    strict: Some(true),
                });
                bindings.push(ExternalToolBinding {
                    exposed_name,
                    connection_id: connection_id.into(),
                    remote_name: remote_name.into(),
                    read_only,
                    destructive,
                    open_world,
                });
            } else {
                if !allowed.contains(&requested_name.as_str()) {
                    return Err(StorageError::InvalidData(format!(
                        "tool {requested_name} is not grantable to the selected role"
                    ))
                    .into());
                }
                definitions.push(canonical_host_tool_definition(requested_name).ok_or_else(
                    || {
                        StorageError::InvalidData(format!(
                            "tool {requested_name} has no runtime schema"
                        ))
                    },
                )?);
            }
        }
        Ok((definitions, bindings))
    }

    fn resolve_shadow_tools(
        role: RoleKind,
        requested: &[String],
    ) -> Result<(Vec<ToolDefinition>, Vec<ExternalToolBinding>), RuntimeError> {
        const SHADOW_SAFE_TOOLS: &[&str] =
            &["fs.read", "fs.list", "fs.write", "fs.patch", "fs.mkdir"];
        let allowed = role_tool_names(role);
        let names = if requested.is_empty() {
            allowed
                .iter()
                .copied()
                .filter(|name| SHADOW_SAFE_TOOLS.contains(name))
                .collect::<Vec<_>>()
        } else {
            let mut seen = BTreeSet::new();
            for name in requested {
                if !seen.insert(name.as_str())
                    || !allowed.contains(&name.as_str())
                    || !SHADOW_SAFE_TOOLS.contains(&name.as_str())
                {
                    return Err(StorageError::Conflict(format!(
                        "tool {name} is not grantable under isolated shadow authority"
                    ))
                    .into());
                }
            }
            requested.iter().map(String::as_str).collect()
        };
        Ok((
            names
                .into_iter()
                .map(|name| {
                    canonical_host_tool_definition(name).ok_or_else(|| {
                        StorageError::InvalidData(format!("tool {name} has no runtime schema"))
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?,
            Vec::new(),
        ))
    }

    fn validate_shadow_execution_authority(
        &self,
        authority: &ShadowExecutionAuthority,
        run: &AgentRun,
        project_id: ProjectId,
    ) -> Result<(), RuntimeError> {
        let experiment = self
            .experiment(authority.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow experiment was not found".into()))?;
        let workspace = self
            .store()?
            .shadow_workspace(authority.experiment_id)?
            .ok_or_else(|| StorageError::Conflict("shadow workspace was not found".into()))?;
        let expected_root = self
            .config
            .data_directory
            .join(&workspace.relative_path)
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let supplied_root = authority
            .workspace_root
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let data_root = self
            .config
            .data_directory
            .canonicalize()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        if experiment.state != ExperimentState::Running
            || experiment.candidate_revision_id != run.organization_revision_id
            || workspace.state != ShadowWorkspaceState::Ready
            || workspace.project_id != project_id
            || expected_root != supplied_root
            || !supplied_root.starts_with(&data_root)
        {
            return Err(StorageError::Conflict(
                "isolated shadow execution authority is stale or mismatched".into(),
            )
            .into());
        }
        Ok(())
    }

    /// Builds a project-confined host execution plane, atomically dispatches the
    /// selected work, executes its bounded provider/tool loop, and durably settles it.
    ///
    /// # Errors
    ///
    /// Returns an error when project capabilities cannot be opened, authority
    /// cannot be dispatched, or provider/tool execution cannot be settled.
    pub async fn execute_agent_work(
        &self,
        spec: &AgentDispatchSpec,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        self.execute_agent_work_with_authority(spec, None, false)
            .await
    }

    /// Executes a fresh attempt after an authenticated owner explicitly chose
    /// Retry. Only the automatic retry-count ceiling is overridden.
    ///
    /// # Errors
    ///
    /// Returns the normal dispatch, provider, tool, and settlement failures.
    pub async fn execute_agent_work_for_explicit_retry(
        &self,
        spec: &AgentDispatchSpec,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        self.execute_agent_work_with_authority(spec, None, true)
            .await
    }

    async fn execute_agent_work_with_authority(
        &self,
        spec: &AgentDispatchSpec,
        shadow_authority: Option<ShadowExecutionAuthority>,
        explicit_owner_retry: bool,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        let task_id = self.agent_run_view(spec.run_id)?.run.task_id;
        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        if task.project_id != Some(spec.project_id) {
            return Err(StorageError::InvalidData(
                "agent dispatch project does not match the task envelope".into(),
            )
            .into());
        }
        let project = self
            .project(spec.project_id)?
            .ok_or(StorageError::ProjectNotFound(spec.project_id))?;
        let root = shadow_authority.as_ref().map_or_else(
            || std::path::PathBuf::from(&project.root),
            |value| value.workspace_root.clone(),
        );
        let artifacts = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let credentials = Arc::new(RejectingCredentialResolver);
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifacts).map_err(runtime_tool_error)?,
        );
        let process = Arc::new(
            ProcessExecutor::new([root.clone()], credentials.clone())
                .map_err(runtime_tool_error)?,
        );
        let http = Arc::new(
            HttpExecutor::new(credentials, filesystem.clone(), &artifacts)
                .map_err(runtime_tool_error)?,
        );
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: http.clone(),
            browser: Arc::new(
                BrowserExecutor::new(self.config.browser_executable.clone(), &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            research: Arc::new(ResearchExecutor::new(http)),
            system: Arc::new(SystemExecutor),
            git: Arc::new(
                GitExecutor::new(process, [root.clone()], &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            memory: Arc::new(MemoryExecutor::new(InMemoryMemoryRepository::default())),
            external: Arc::new(RuntimeMcpExecutor {
                store: self.store.clone(),
                credentials: self.mcp_credentials.clone(),
            }),
        };
        let prepared = self
            .prepare_agent_turn_with_authority(
                spec,
                shadow_authority.as_ref(),
                explicit_owner_retry,
            )
            .await?;
        let mut capabilities = CapabilityPolicy::project(&root);
        capabilities.allowed_network_origins.extend(
            prepared
                .request
                .external_tools
                .iter()
                .map(|binding| format!("mcp://{}/{}", binding.connection_id, binding.remote_name)),
        );
        let control = self.tool_control();
        let broker = ToolBroker::new(
            host,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let confidence = ConfidenceEstimate::new(5_000, 0)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let scoped_tools = ProjectScopedAgentTools::new(&broker, root);
        self.execute_agent_turn_with_authority(
            prepared.request,
            &self.provider_gateway(),
            &scoped_tools,
            confidence,
            shadow_authority.as_ref(),
        )
        .await
    }

    /// Resumes one approval-waiting local-tool turn after explicit owner
    /// approval, revalidating project context, tools, routes, and live lease
    /// before restoring durable running authority.
    ///
    /// # Errors
    ///
    /// Returns an error when approval is absent, authority is ambiguous/stale,
    /// context changed, an external-tool checkpoint cannot be reconstructed, or
    /// provider/tool execution fails.
    #[allow(clippy::too_many_lines)]
    pub async fn resume_approved_agent_work(
        &self,
        task_id: TaskId,
        context_token_budget: u32,
        estimated_output_tokens: u64,
        max_model_calls: u16,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        if context_token_budget < 64
            || estimated_output_tokens == 0
            || max_model_calls == 0
            || max_model_calls > 32
        {
            return Err(StorageError::InvalidData("agent resume bounds are invalid".into()).into());
        }
        let run_id = self
            .task_launch_run_id(task_id)?
            .ok_or_else(|| StorageError::Conflict("task has no consumer launch".into()))?;
        let view = self.agent_run_view(run_id)?;
        let waiting = view
            .instances
            .iter()
            .filter(|instance| instance.state == AgentState::Waiting)
            .collect::<Vec<_>>();
        if waiting.len() != 1 {
            return Err(StorageError::Conflict(
                "consumer task does not have one waiting agent".into(),
            )
            .into());
        }
        let waiting = waiting[0];
        let lease = view
            .leases
            .iter()
            .find(|lease| lease.instance_id == waiting.id && lease.state == AgentLeaseState::Active)
            .ok_or_else(|| StorageError::Conflict("waiting agent has no active lease".into()))?;
        let checkpoint = self
            .store()?
            .agent_turn_checkpoint(lease.id)?
            .ok_or_else(|| {
                StorageError::Conflict(
                    "approval-waiting turn has no durable provider checkpoint".into(),
                )
            })?;
        let pending_request_id = checkpoint.pending_tool_request_id.ok_or_else(|| {
            StorageError::Conflict("approval-waiting checkpoint has no pending tool request".into())
        })?;
        if !self
            .approvals(Some(ApprovalState::Approved))?
            .iter()
            .any(|approval| {
                approval.task_id == task_id && approval.tool_request_id == pending_request_id
            })
        {
            return Err(StorageError::Conflict(
                "checkpointed tool request has not been approved".into(),
            )
            .into());
        }
        let task = self
            .task(task_id)?
            .ok_or(StorageError::TaskNotFound(task_id))?;
        let project_id = task.project_id.ok_or_else(|| {
            StorageError::InvalidData("consumer task has no selected project".into())
        })?;
        let project = self
            .project(project_id)?
            .ok_or(StorageError::ProjectNotFound(project_id))?;
        let organization = self
            .organization_revision(view.run.organization_revision_id)?
            .ok_or(StorageError::OrganizationRevisionNotFound(
                view.run.organization_revision_id,
            ))?;
        let role = organization
            .roles
            .iter()
            .find(|role| role.id == waiting.role_id)
            .cloned()
            .ok_or_else(|| StorageError::InvalidData("waiting role was not found".into()))?;
        let context =
            self.assemble_project_context(project_id, Vec::new(), context_token_budget, None)?;
        let context_json = serde_json::to_vec(&context)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let fingerprint = format!("sha256:{:x}", Sha256::digest(context_json));
        if fingerprint != waiting.context_fingerprint {
            return Err(StorageError::Conflict(
                "project context changed while approval was pending; replan is required".into(),
            )
            .into());
        }
        let requested_tools = lease
            .tool_permissions
            .iter()
            .map(|permission| {
                checkpoint
                    .request
                    .external_tools
                    .iter()
                    .find(|binding| binding.exposed_name == *permission)
                    .map_or_else(
                        || permission.clone(),
                        |binding| {
                            format!("mcp::{}::{}", binding.connection_id, binding.remote_name)
                        },
                    )
            })
            .collect::<Vec<_>>();
        let (tools, external_tools) = self.resolve_agent_tools(role.kind, &requested_tools)?;
        let routes = self.provider_route_candidates().await?;
        if routes.is_empty() {
            return Err(ProviderError::Configuration(
                "no discovered provider models are available".into(),
            )
            .into());
        }
        let root = std::path::PathBuf::from(project.root);
        let artifacts = self.config.data_directory.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let credentials = Arc::new(RejectingCredentialResolver);
        let filesystem = Arc::new(
            FilesystemExecutor::open([root.clone()], &artifacts).map_err(runtime_tool_error)?,
        );
        let process = Arc::new(
            ProcessExecutor::new([root.clone()], credentials.clone())
                .map_err(runtime_tool_error)?,
        );
        let http = Arc::new(
            HttpExecutor::new(credentials, filesystem.clone(), &artifacts)
                .map_err(runtime_tool_error)?,
        );
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: http.clone(),
            browser: Arc::new(
                BrowserExecutor::new(self.config.browser_executable.clone(), &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            research: Arc::new(ResearchExecutor::new(http)),
            system: Arc::new(SystemExecutor),
            git: Arc::new(
                GitExecutor::new(process, [root.clone()], &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
            memory: Arc::new(MemoryExecutor::new(InMemoryMemoryRepository::default())),
            external: Arc::new(RuntimeMcpExecutor {
                store: self.store.clone(),
                credentials: self.mcp_credentials.clone(),
            }),
        };
        let mut capabilities = CapabilityPolicy::project(&root);
        capabilities.allowed_network_origins.extend(
            external_tools
                .iter()
                .map(|binding| format!("mcp://{}/{}", binding.connection_id, binding.remote_name)),
        );
        let control = self.tool_control();
        let broker = ToolBroker::new(
            host,
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: self.user_settings().autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let resumed = self
            .store()?
            .resume_launched_agent_turn(task_id, Utc::now())?;
        for event in resumed.events {
            let _ = self.event_sender.send(event);
        }
        let confidence = ConfidenceEstimate::new(5_000, 0)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let scoped_tools = ProjectScopedAgentTools::new(&broker, root);
        self.execute_agent_turn(
            AgentTurnRequest {
                role,
                instance: resumed.instance,
                lease: resumed.lease,
                context,
                route_candidates: routes,
                routing_policy: RoutingPolicy::default(),
                tools,
                external_tools,
                estimated_output_tokens: checkpoint.request.estimated_output_tokens,
                max_model_calls: checkpoint.request.max_model_calls,
            },
            &self.provider_gateway(),
            &scoped_tools,
            confidence,
        )
        .await
    }

    /// Records a selected ephemeral agent instance and publishes its event.
    ///
    /// # Errors
    ///
    /// Returns an error when the instance does not match its run and selected role.
    pub fn record_agent_instance(
        &self,
        instance: &AgentInstance,
    ) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.record_agent_instance(instance)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Transitions a current agent instance with measured confidence.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown instances or illegal transitions.
    pub fn transition_agent_instance(
        &self,
        instance_id: mcg_domain::AgentInstanceId,
        next: AgentState,
        confidence: ConfidenceEstimate,
    ) -> Result<AgentInstance, RuntimeError> {
        let (instance, event) =
            self.store()?
                .transition_agent_instance(instance_id, next, confidence, Utc::now())?;
        let _ = self.event_sender.send(event);
        Ok(instance)
    }

    /// Records a bounded active agent lease and publishes its event.
    ///
    /// # Errors
    ///
    /// Returns an error when the lease does not match active selected work.
    pub fn record_agent_lease(&self, lease: &AgentLease) -> Result<EventEnvelope, RuntimeError> {
        let event = self.store()?.record_agent_lease(lease)?;
        let _ = self.event_sender.send(event.clone());
        Ok(event)
    }

    /// Executes one bounded provider/tool turn and atomically commits its result,
    /// usage, run metrics, completed instance, and completed lease.
    ///
    /// # Errors
    ///
    /// Stale runtime authority, route/provider errors, policy/tool errors, and
    /// failed atomic persistence are returned without fabricating completion.
    pub async fn execute_agent_turn<M: AgentModelGateway, T: AgentToolGateway>(
        &self,
        request: AgentTurnRequest,
        models: &M,
        tools: &T,
        confidence: ConfidenceEstimate,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        self.execute_agent_turn_with_authority(request, models, tools, confidence, None)
            .await
    }

    async fn execute_agent_turn_with_authority<M: AgentModelGateway, T: AgentToolGateway>(
        &self,
        request: AgentTurnRequest,
        models: &M,
        tools: &T,
        confidence: ConfidenceEstimate,
        shadow_authority: Option<&ShadowExecutionAuthority>,
    ) -> Result<AgentTurnCommit, RuntimeError> {
        {
            let store = self.store()?;
            let stored_instance = store
                .agent_instance(request.instance.id)?
                .ok_or(StorageError::AgentInstanceNotFound(request.instance.id))?;
            let stored_lease = store
                .agent_lease(request.lease.id)?
                .ok_or(StorageError::AgentLeaseNotFound(request.lease.id))?;
            let run = store
                .agent_run(request.instance.run_id)?
                .ok_or(StorageError::AgentRunNotFound(request.instance.run_id))?;
            let genome = store
                .organization_revision(run.organization_revision_id)?
                .ok_or(StorageError::OrganizationRevisionNotFound(
                    run.organization_revision_id,
                ))?;
            let role_matches = genome
                .roles
                .iter()
                .any(|role| role == &request.role && role.id == stored_instance.role_id);
            let active = store.organization(genome.organization_id)?.ok_or_else(|| {
                StorageError::Conflict("active organization was not found".into())
            })?;
            let evaluation_only = active.revision_id != genome.revision_id;
            if stored_instance != request.instance
                || stored_lease != request.lease
                || run.state != AgentRunState::Running
                || !role_matches
            {
                return Err(StorageError::Conflict(
                    "agent turn request uses stale or mismatched durable authority".into(),
                )
                .into());
            }
            if evaluation_only {
                let authority = shadow_authority.ok_or_else(|| {
                    StorageError::Conflict(
                        "inactive candidate execution requires isolated shadow authority".into(),
                    )
                })?;
                drop(store);
                self.validate_shadow_execution_authority(
                    authority,
                    &run,
                    self.task(run.task_id)?
                        .and_then(|task| task.project_id)
                        .ok_or_else(|| {
                            StorageError::Conflict("shadow task has no selected project".into())
                        })?,
                )?;
            } else if shadow_authority.is_some() {
                return Err(StorageError::Conflict(
                    "active organization execution cannot use shadow authority".into(),
                )
                .into());
            }
        }
        let checkpoint = self.store()?.agent_turn_checkpoint(request.lease.id)?;
        let checkpoint_sink = DurableAgentTurnCheckpointSink {
            store: self.store.clone(),
        };
        let result = match AgentTurnEngine
            .execute_resumable(request.clone(), checkpoint, models, tools, &checkpoint_sink)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let (disposition, reason_code) = classify_agent_turn_error(&error);
                let events = self.store()?.settle_agent_turn_error(
                    request.instance.id,
                    request.lease.id,
                    disposition,
                    reason_code,
                    Utc::now(),
                )?;
                for event in events {
                    let _ = self.event_sender.send(event);
                }
                return Err(error.into());
            }
        };
        let commit = self.store()?.commit_agent_turn(
            request.instance.id,
            request.lease.id,
            &result,
            confidence,
            shadow_authority.map(|authority| authority.experiment_id),
            Utc::now(),
        )?;
        for event in &commit.events {
            let _ = self.event_sender.send(event.clone());
        }
        Ok(commit)
    }

    /// Lists durable approvals, optionally filtered by lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn approvals(
        &self,
        state: Option<ApprovalState>,
    ) -> Result<Vec<ApprovalRequest>, RuntimeError> {
        Ok(self.store()?.approvals(state)?)
    }

    /// Loads one durable approval.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn approval(
        &self,
        approval_id: ApprovalId,
    ) -> Result<Option<ApprovalRequest>, RuntimeError> {
        Ok(self.store()?.approval(approval_id)?)
    }

    /// Resolves an approval and publishes its committed event.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lifecycle transitions or failed persistence.
    pub fn resolve_approval(
        &self,
        resolution: &ApprovalResolution,
    ) -> Result<ApprovalRequest, RuntimeError> {
        let mut store = self.store()?;
        if matches!(
            resolution.state,
            ApprovalState::Denied | ApprovalState::Cancelled
        ) && let Some((approval, events)) =
            store.reject_consumer_evaluation_approval(resolution)?
        {
            drop(store);
            for event in events {
                let _ = self.event_sender.send(event);
            }
            return Ok(approval);
        }
        if matches!(
            resolution.state,
            ApprovalState::Denied | ApprovalState::Cancelled
        ) && let Some((approval, events)) = store.reject_waiting_agent_approval(resolution)?
        {
            drop(store);
            for event in events {
                let _ = self.event_sender.send(event);
            }
            return Ok(approval);
        }
        let (approval, event) = store.resolve_approval(resolution)?;
        drop(store);
        let _ = self.event_sender.send(event);
        Ok(approval)
    }

    /// Loads immutable tool audit history for one task.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be read.
    pub fn tool_audit(&self, task_id: TaskId) -> Result<Vec<ToolAuditRecord>, RuntimeError> {
        Ok(self.store()?.tool_audit_for_task(task_id)?)
    }

    /// Reports whether a lease currently retains a resumable provider-turn
    /// checkpoint without exposing the private transcript itself.
    ///
    /// # Errors
    ///
    /// Returns an error when durable storage cannot be read or decoded.
    pub fn has_agent_turn_checkpoint(&self, lease_id: AgentLeaseId) -> Result<bool, RuntimeError> {
        Ok(self.store()?.agent_turn_checkpoint(lease_id)?.is_some())
    }

    #[must_use]
    pub fn tool_control(&self) -> DurableToolControl {
        DurableToolControl {
            store: self.store.clone(),
            event_sender: self.event_sender.clone(),
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_sender.subscribe()
    }
}

fn replay_count(value: usize) -> Result<u64, StorageError> {
    u64::try_from(value)
        .map_err(|_| StorageError::InvalidData("run replay evidence count overflowed".into()))
}

fn replay_invariant(code: &str, detail: &str) -> RunReplayInvariant {
    RunReplayInvariant {
        code: code.into(),
        passed: true,
        detail: detail.into(),
    }
}

fn replay_invalid(detail: impl Into<String>) -> StorageError {
    StorageError::InvalidData(format!("run replay invariant failed: {}", detail.into()))
}

#[allow(clippy::too_many_lines)] // Keeps the complete fail-closed replay contract auditable in one ordered pass.
fn validate_run_replay_evidence(
    evidence: &RunReplayEvidence,
) -> Result<Vec<RunReplayInvariant>, StorageError> {
    let run = &evidence.view.run;
    let plan = &evidence.view.plan;
    if evidence.task.id != run.task_id || plan.task_id != run.task_id || plan.run_id != run.id {
        return Err(replay_invalid("run, plan, and task identities differ"));
    }
    if evidence.organization.revision_id != run.organization_revision_id
        || plan.organization_revision_id != run.organization_revision_id
    {
        return Err(replay_invalid(
            "run, plan, and organization revision identities differ",
        ));
    }
    plan.validate(&evidence.organization)
        .map_err(|error| replay_invalid(format!("the persisted work plan is invalid: {error}")))?;
    let mut invariants = vec![replay_invariant(
        "identity_coherent",
        "task, run, plan, and organization revision identities agree",
    )];

    let role_ids = evidence
        .organization
        .roles
        .iter()
        .map(|role| role.id)
        .collect::<BTreeSet<_>>();
    if role_ids.len() != evidence.organization.roles.len()
        || plan
            .selected_roles
            .iter()
            .any(|role_id| !role_ids.contains(role_id))
    {
        return Err(replay_invalid(
            "the plan selects an absent or duplicate organization role",
        ));
    }
    let work_item_ids = plan
        .items
        .iter()
        .map(|item| item.id)
        .collect::<BTreeSet<_>>();
    if work_item_ids.len() != plan.items.len()
        || plan.items.iter().any(|item| {
            !role_ids.contains(&item.assigned_role)
                || item
                    .dependencies
                    .iter()
                    .any(|dependency| *dependency == item.id || !work_item_ids.contains(dependency))
        })
    {
        return Err(replay_invalid(
            "the work graph has duplicate, self, unknown dependency, or role references",
        ));
    }
    invariants.push(replay_invariant(
        "plan_graph_coherent",
        "selected roles and work-item dependencies resolve inside the pinned organization and plan",
    ));

    let instance_ids = evidence
        .view
        .instances
        .iter()
        .map(|instance| instance.id)
        .collect::<BTreeSet<_>>();
    if instance_ids.len() != evidence.view.instances.len()
        || evidence.view.instances.iter().any(|instance| {
            instance.run_id != run.id
                || instance.task_id != run.task_id
                || !role_ids.contains(&instance.role_id)
        })
    {
        return Err(replay_invalid(
            "an agent instance has a duplicate identity or escapes the run, task, or organization",
        ));
    }
    let lease_ids = evidence
        .view
        .leases
        .iter()
        .map(|lease| lease.id)
        .collect::<BTreeSet<_>>();
    if lease_ids.len() != evidence.view.leases.len()
        || evidence.view.leases.iter().any(|lease| {
            !instance_ids.contains(&lease.instance_id)
                || !work_item_ids.contains(&lease.work_item_id)
        })
        || evidence.view.results.len() > evidence.view.leases.len()
    {
        return Err(replay_invalid(
            "a lease or turn result cannot be resolved inside the run graph",
        ));
    }
    invariants.push(replay_invariant(
        "authority_graph_coherent",
        "instances, leases, and retained turn results remain inside the pinned run graph",
    ));

    if evidence.view.usage.iter().any(|usage| {
        usage.run_id != run.id
            || usage.task_id != run.task_id
            || usage
                .instance_id
                .is_some_and(|instance_id| !instance_ids.contains(&instance_id))
    }) {
        return Err(replay_invalid(
            "a usage record escapes the run, task, or known instances",
        ));
    }
    if evidence.view.evaluations.iter().any(|evaluation| {
        evaluation.run_id != run.id
            || evaluation.task_id != run.task_id
            || evaluation
                .evaluator_instance_id
                .is_some_and(|instance_id| !instance_ids.contains(&instance_id))
    }) {
        return Err(replay_invalid(
            "an evaluation escapes the run, task, or known evaluator instances",
        ));
    }
    invariants.push(replay_invariant(
        "accounting_coherent",
        "usage and evaluations are bound to the reconstructed run and task",
    ));

    let mut previous_sequence = None;
    for event in &evidence.events {
        if event.task_id != Some(run.task_id)
            || event.sequence == 0
            || previous_sequence.is_some_and(|sequence| event.sequence <= sequence)
        {
            return Err(replay_invalid(
                "task events are unbound, unsequenced, duplicated, or out of order",
            ));
        }
        previous_sequence = Some(event.sequence);
    }
    if evidence
        .tool_audit
        .iter()
        .any(|record| record.task_id != run.task_id)
    {
        return Err(replay_invalid("tool audit evidence escapes the run task"));
    }
    invariants.push(replay_invariant(
        "history_coherent",
        "events are strictly ordered and all tool-audit records are task-bound",
    ));
    invariants.push(replay_invariant(
        "observation_only",
        "replay was reconstructed from durable records without provider or tool execution",
    ));
    Ok(invariants)
}

fn require_browser_authority(
    session: &PlatformSession,
    expected_state: PlatformSessionState,
    expected_owner: PlatformSessionOwner,
) -> Result<(), RuntimeError> {
    if session.kind != PlatformSessionKind::Browser {
        return Err(ToolError::Invalid("platform session is not a browser session".into()).into());
    }
    if session.state != expected_state || session.owner != expected_owner {
        return Err(ToolError::Invalid(format!(
            "browser authority requires {expected_state:?}/{expected_owner:?}, found {:?}/{:?}",
            session.state, session.owner
        ))
        .into());
    }
    Ok(())
}

fn require_computer_authority(
    session: &PlatformSession,
    expected_state: PlatformSessionState,
    expected_owner: PlatformSessionOwner,
) -> Result<(), RuntimeError> {
    if session.kind != PlatformSessionKind::ComputerUse {
        return Err(
            ToolError::Invalid("platform session is not a computer-use session".into()).into(),
        );
    }
    if session.state != expected_state || session.owner != expected_owner {
        return Err(ToolError::Invalid(format!(
            "computer authority requires {expected_state:?}/{expected_owner:?}, found {:?}/{:?}",
            session.state, session.owner
        ))
        .into());
    }
    Ok(())
}

fn computer_action_request_hash(
    session_id: PlatformSessionId,
    expected_frame_sha256: &str,
    actions: &[ComputerUseAction],
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(session_id, expected_frame_sha256, actions))
                .map_err(StorageError::from)?,
        )
    ))
}

fn computer_session_resource(session_id: PlatformSessionId) -> String {
    format!("computer://session/{session_id}")
}

fn computer_authority_task_id(session_id: PlatformSessionId) -> TaskId {
    TaskId::from_stable_bytes(stable_authority_bytes(
        "mcg:computer-session-authority:v1",
        session_id,
    ))
}

fn computer_action_tool_request_id(batch_id: ComputerActionBatchId) -> ToolRequestId {
    ToolRequestId::from_stable_bytes(stable_authority_bytes(
        "mcg:computer-action-request:v1",
        batch_id,
    ))
}

fn site_authority_task_id(site_id: SiteId) -> TaskId {
    TaskId::from_stable_bytes(stable_authority_bytes("mcg:site-authority:v1", site_id))
}

fn site_health_tool_request_id(check_id: SiteHealthCheckId) -> ToolRequestId {
    ToolRequestId::from_stable_bytes(stable_authority_bytes(
        "mcg:site-health-request:v1",
        check_id,
    ))
}

fn site_deployment_tool_request_id(deployment_id: SiteDeploymentId) -> ToolRequestId {
    ToolRequestId::from_stable_bytes(stable_authority_bytes(
        "mcg:site-deployment-request:v1",
        deployment_id,
    ))
}

fn site_revision_resource(site_id: SiteId, revision: u32) -> String {
    format!("site://{site_id}/revision/{revision}")
}

fn ssh_authority_task_id(host_id: SshHostId) -> TaskId {
    TaskId::from_stable_bytes(stable_authority_bytes("mcg:ssh-host-authority:v1", host_id))
}

fn ssh_operation_tool_request_id(operation_id: SshOperationId) -> ToolRequestId {
    ToolRequestId::from_stable_bytes(stable_authority_bytes(
        "mcg:ssh-operation-request:v1",
        operation_id,
    ))
}

fn ssh_revision_resource(host_id: SshHostId, revision: u32) -> String {
    format!("ssh://host/{host_id}/revision/{revision}")
}

const fn ssh_kind_label(kind: &SshOperationKind) -> &'static str {
    match kind {
        SshOperationKind::Inspect => "Inspect",
        SshOperationKind::Execute { .. } => "Execute",
        SshOperationKind::Upload { .. } => "Upload",
        SshOperationKind::Download { .. } => "Download",
    }
}

fn ssh_host_definition_hash(request: &SshHostRegistrationRequest) -> Result<String, RuntimeError> {
    let capabilities = normalize_ssh_capabilities(&request.capabilities)?;
    let encoded = if capabilities.is_empty() {
        serde_json::to_vec(&(
            SSH_SCHEMA_VERSION,
            request.id,
            request.display_name.trim(),
            request.hostname.trim(),
            request.port,
            request.username.trim(),
            request.platform,
            request.authentication,
            request.known_hosts_entry.trim(),
            request.host_key_sha256.trim(),
            request.state,
        ))
    } else {
        serde_json::to_vec(&(
            SSH_SCHEMA_VERSION,
            request.id,
            request.display_name.trim(),
            request.hostname.trim(),
            request.port,
            request.username.trim(),
            request.platform,
            request.authentication,
            request.known_hosts_entry.trim(),
            request.host_key_sha256.trim(),
            capabilities,
            request.state,
        ))
    }
    .map_err(StorageError::from)?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn normalize_ssh_capabilities(values: &[String]) -> Result<Vec<String>, RuntimeError> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if normalized.len() > 64
        || normalized.iter().any(|value| {
            value.is_empty()
                || value.chars().count() > 96
                || !value.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, ':' | '.' | '_' | '-')
                })
        })
    {
        return Err(StorageError::InvalidData(
            "SSH capabilities must be bounded lowercase namespaced atoms".into(),
        )
        .into());
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn ssh_operation_request_hash(
    host: &SshHost,
    task_id: Option<TaskId>,
    kind: &SshOperationKind,
    timeout_seconds: u64,
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                SSH_SCHEMA_VERSION,
                host.id,
                host.revision,
                &host.definition_hash,
                task_id,
                kind,
                timeout_seconds,
            ))
            .map_err(StorageError::from)?,
        )
    ))
}

fn ssh_workflow_definition_hash(
    request: &SshWorkflowCreateRequest,
) -> Result<String, RuntimeError> {
    #[derive(Serialize)]
    struct LegacyStep<'a> {
        id: SshWorkflowStepId,
        name: &'a str,
        host_id: SshHostId,
        host_revision: u32,
        dependencies: &'a [SshWorkflowStepId],
        operation: &'a SshOperationKind,
        timeout_seconds: u64,
    }

    let legacy_steps = request
        .steps
        .iter()
        .map(|step| {
            let host_id = step.host_id?;
            let host_revision = step.host_revision?;
            if !step.required_capabilities.is_empty() {
                return None;
            }
            Some(LegacyStep {
                id: step.id,
                name: &step.name,
                host_id,
                host_revision,
                dependencies: &step.dependencies,
                operation: &step.operation,
                timeout_seconds: step.timeout_seconds,
            })
        })
        .collect::<Option<Vec<_>>>();
    if let Some(steps) = legacy_steps {
        let encoded = serde_json::to_vec(&(
            SSH_WORKFLOW_SCHEMA_VERSION,
            request.id,
            request.display_name.trim(),
            request.task_id,
            steps,
        ))
        .map_err(StorageError::from)?;
        return Ok(format!("{:x}", Sha256::digest(encoded)));
    }

    let mut steps = Vec::with_capacity(request.steps.len());
    for step in &request.steps {
        steps.push((
            step.id,
            step.name.trim().to_owned(),
            step.host_id,
            step.host_revision,
            normalize_ssh_capabilities(&step.required_capabilities)?,
            step.dependencies.clone(),
            step.operation.clone(),
            step.timeout_seconds,
        ));
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                SSH_WORKFLOW_SCHEMA_VERSION,
                request.id,
                request.display_name.trim(),
                request.task_id,
                steps,
            ))
            .map_err(StorageError::from)?,
        )
    ))
}

fn ssh_workflow_operation_id(
    workflow_id: SshWorkflowId,
    step_id: SshWorkflowStepId,
) -> SshOperationId {
    let digest = Sha256::digest(format!(
        "mcg:ssh-workflow-operation:v1:{workflow_id}:{step_id}"
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    SshOperationId::from_stable_bytes(bytes)
}

fn skip_ssh_workflow_dependents(workflow: &mut SshWorkflow, failed: SshWorkflowStepId) {
    let mut blocked = BTreeSet::from([failed]);
    loop {
        let additions = workflow
            .steps
            .iter()
            .filter(|step| {
                step.state == SshWorkflowStepState::Pending
                    && step
                        .dependencies
                        .iter()
                        .any(|dependency| blocked.contains(dependency))
            })
            .map(|step| step.id)
            .collect::<Vec<_>>();
        if additions.is_empty() {
            break;
        }
        for id in additions {
            blocked.insert(id);
            if let Some(step) = workflow.steps.iter_mut().find(|step| step.id == id) {
                step.state = SshWorkflowStepState::Skipped;
                step.detail = "dependency did not complete successfully".into();
            }
        }
    }
}

fn ssh_workflow_error_detail(error: &RuntimeError) -> String {
    match error {
        RuntimeError::Tool(ToolError::Unavailable(_)) => {
            "OpenSSH is unavailable on the controller".into()
        }
        RuntimeError::Tool(
            ToolError::Denied(detail) | ToolError::Invalid(detail) | ToolError::Execution(detail),
        ) => detail.chars().take(1_024).collect(),
        RuntimeError::Tool(ToolError::TimedOut) => "SSH workflow step timed out".into(),
        RuntimeError::Tool(ToolError::ApprovalRequired { .. }) => {
            "owner approval is required".into()
        }
        _ => "SSH workflow step could not be completed safely".into(),
    }
}

const fn site_environment_label(environment: SiteEnvironment) -> &'static str {
    match environment {
        SiteEnvironment::Development => "development",
        SiteEnvironment::Preview => "preview",
        SiteEnvironment::Production => "production",
    }
}

const fn supervision_signal_label(signal: SupervisionSignal) -> &'static str {
    match signal {
        SupervisionSignal::RepeatedFailure => "repeated-failure",
        SupervisionSignal::AgentDisagreement => "agent-disagreement",
        SupervisionSignal::RunawayLoop => "runaway-loop",
        SupervisionSignal::Overconfidence => "overconfidence",
        SupervisionSignal::Underconfidence => "underconfidence",
        SupervisionSignal::ToolFailure => "tool-failure",
        SupervisionSignal::BudgetPressure => "budget-pressure",
        SupervisionSignal::ExcessiveOverhead => "excessive-overhead",
        SupervisionSignal::UserCorrection => "user-correction",
        SupervisionSignal::Regression => "regression",
    }
}

fn automatic_experiment_id_bytes(
    namespace: &str,
    observation_id: SupervisionObservationId,
) -> [u8; 16] {
    let digest = Sha256::digest(format!(
        "mcg-auto-experiment\0{namespace}\0{observation_id}"
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

fn automatic_shadow_run_id_bytes(experiment_id: ExperimentId) -> [u8; 16] {
    let digest = Sha256::digest(format!("mcg-auto-shadow-run\0{experiment_id}"));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

fn automatic_shadow_evidence_id_bytes(namespace: &str, experiment_id: ExperimentId) -> [u8; 16] {
    let digest = Sha256::digest(format!(
        "mcg-auto-shadow-evidence\0{namespace}\0{experiment_id}"
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

const SHADOW_MAX_FILES: u64 = 50_000;
const SHADOW_MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const SHADOW_MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const SHADOW_MAX_COMMAND_TRANSCRIPT_BYTES: u64 = 1024 * 1024;
const SHADOW_MAX_DEPTH: usize = 32;

struct ShadowWorkspaceManifest {
    files: Vec<std::path::PathBuf>,
    file_count: u64,
    total_bytes: u64,
    sha256: String,
}

fn scan_shadow_workspace_source(
    source_root: &Path,
    data_root: &Path,
) -> Result<ShadowWorkspaceManifest, RuntimeError> {
    if source_root == data_root {
        return Err(StorageError::InvalidData(
            "shadow source cannot be the private data directory".into(),
        )
        .into());
    }
    let mut directories = vec![(source_root.to_path_buf(), 0_usize)];
    let mut files = Vec::new();
    while let Some((directory, depth)) = directories.pop() {
        if depth > SHADOW_MAX_DEPTH {
            return Err(StorageError::InvalidData(
                "shadow source exceeds the maximum directory depth".into(),
            )
            .into());
        }
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            if path == data_root {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(StorageError::InvalidData(
                    "shadow source contains a symbolic link or junction".into(),
                )
                .into());
            }
            if metadata.is_dir() {
                if entry.file_name() == ".git" {
                    continue;
                }
                directories.push((path, depth + 1));
            } else if metadata.is_file() {
                if metadata.len() > SHADOW_MAX_FILE_BYTES {
                    return Err(StorageError::InvalidData(
                        "shadow source contains an oversized file".into(),
                    )
                    .into());
                }
                files.push(path);
                if u64::try_from(files.len()).unwrap_or(u64::MAX) > SHADOW_MAX_FILES {
                    return Err(StorageError::InvalidData(
                        "shadow source exceeds the file-count limit".into(),
                    )
                    .into());
                }
            } else {
                return Err(StorageError::InvalidData(
                    "shadow source contains a non-regular filesystem entry".into(),
                )
                .into());
            }
        }
    }
    files.sort();
    hash_shadow_files(source_root, files)
}

fn hash_shadow_files(
    source_root: &Path,
    files: Vec<std::path::PathBuf>,
) -> Result<ShadowWorkspaceManifest, RuntimeError> {
    let mut digest = Sha256::new();
    let mut total_bytes = 0_u64;
    for path in &files {
        let relative = shadow_relative_path(source_root, path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::InvalidData(
                "shadow source changed during bounded scan".into(),
            )
            .into());
        }
        total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
            StorageError::InvalidData("shadow source byte count overflowed".into())
        })?;
        if total_bytes > SHADOW_MAX_TOTAL_BYTES {
            return Err(StorageError::InvalidData(
                "shadow source exceeds the total byte limit".into(),
            )
            .into());
        }
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_le_bytes());
        digest.update([0]);
        let mut file = std::fs::File::open(path)?;
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        digest.update([0]);
    }
    Ok(ShadowWorkspaceManifest {
        file_count: u64::try_from(files.len())
            .map_err(|_| StorageError::InvalidData("too many shadow files".into()))?,
        files,
        total_bytes,
        sha256: format!("{:x}", digest.finalize()),
    })
}

fn copy_shadow_workspace(
    source_root: &Path,
    destination: &Path,
    expected: &ShadowWorkspaceManifest,
) -> Result<ShadowWorkspaceManifest, RuntimeError> {
    if destination.exists() {
        return Err(
            StorageError::Conflict("shadow workspace destination already exists".into()).into(),
        );
    }
    let parent = destination.parent().ok_or_else(|| {
        StorageError::InvalidData("shadow workspace destination has no parent".into())
    })?;
    fs::create_dir_all(parent)?;
    fs::create_dir(destination)?;
    let mut digest = Sha256::new();
    let mut total_bytes = 0_u64;
    for source in &expected.files {
        let relative = shadow_relative_path(source_root, source)?;
        let target = destination.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let metadata = fs::symlink_metadata(source)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > SHADOW_MAX_FILE_BYTES
        {
            return Err(
                StorageError::InvalidData("shadow source changed before copy".into()).into(),
            );
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut input = std::fs::File::open(source)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(metadata.len().to_le_bytes());
        digest.update([0]);
        let mut observed = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
            digest.update(&buffer[..read]);
            observed = observed
                .checked_add(
                    u64::try_from(read).map_err(|_| {
                        StorageError::InvalidData("shadow file size overflowed".into())
                    })?,
                )
                .ok_or_else(|| StorageError::InvalidData("shadow file size overflowed".into()))?;
        }
        if observed != metadata.len() {
            return Err(
                StorageError::Conflict("shadow source changed while copying".into()).into(),
            );
        }
        fs::set_permissions(&target, metadata.permissions())?;
        output.sync_all()?;
        digest.update([0]);
        total_bytes = total_bytes
            .checked_add(observed)
            .ok_or_else(|| StorageError::InvalidData("shadow byte count overflowed".into()))?;
    }
    let copied = ShadowWorkspaceManifest {
        files: expected.files.clone(),
        file_count: expected.file_count,
        total_bytes,
        sha256: format!("{:x}", digest.finalize()),
    };
    if copied.file_count != expected.file_count
        || copied.total_bytes != expected.total_bytes
        || copied.sha256 != expected.sha256
    {
        return Err(StorageError::Conflict(
            "shadow workspace content does not match the claimed source manifest".into(),
        )
        .into());
    }
    Ok(copied)
}

fn shadow_relative_path(source_root: &Path, path: &Path) -> Result<String, RuntimeError> {
    let relative = path
        .strip_prefix(source_root)
        .map_err(|_| StorageError::InvalidData("shadow source escaped its root".into()))?;
    let value = relative.to_str().ok_or_else(|| {
        StorageError::InvalidData("shadow source path is not valid Unicode".into())
    })?;
    if value.is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(StorageError::InvalidData(
            "shadow source path is not a safe relative path".into(),
        )
        .into());
    }
    Ok(value.replace('\\', "/"))
}

fn automatic_experiment_definition(
    baseline: &OrganizationGenome,
    observation: &SupervisionObservation,
    candidate_revision_id: OrganizationRevisionId,
    experiment_id: ExperimentId,
) -> Result<ExperimentDefinition, StorageError> {
    Ok(ExperimentDefinition {
        schema_version: mcg_agents::EXPERIMENT_SCHEMA_VERSION,
        id: experiment_id,
        organization_id: baseline.organization_id,
        baseline_revision_id: baseline.revision_id,
        candidate_revision_id,
        mode: ExperimentMode::Shadow,
        state: ExperimentState::Draft,
        hypothesis: format!(
            "Measured candidate addresses {}: {}",
            supervision_signal_label(observation.signal),
            observation.summary
        ),
        task_classes: vec![format!(
            "supervision/{}",
            supervision_signal_label(observation.signal)
        )],
        required_benchmarks: 1,
        maximum_regressions: 0,
        minimum_confidence: ConfidenceEstimate::new(8_000, 1)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
        observation_ids: vec![observation.id],
        created_at: observation.observed_at,
        updated_at: observation.observed_at,
    })
}

fn normalized_site_url(value: &str) -> Result<String, RuntimeError> {
    let url = url::Url::parse(value.trim())
        .map_err(|error| ToolError::Invalid(format!("invalid Site URL: {error}")))?;
    Ok(url.to_string())
}

fn site_definition_hash(
    request: &SiteRegistrationRequest,
    public_url: &str,
    health_url: &str,
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                SITE_SCHEMA_VERSION,
                request.project_id,
                request.display_name.trim(),
                request.environment,
                public_url,
                health_url,
                request.hosting_provider.trim(),
                &request.deployment_command,
                request
                    .credential_reference
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
                request.state,
            ))
            .map_err(StorageError::from)?,
        )
    ))
}

fn site_health_request_hash(site: &Site) -> Result<String, RuntimeError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                SITE_SCHEMA_VERSION,
                site.id,
                site.revision,
                &site.definition_hash,
                &site.health_url,
            ))
            .map_err(StorageError::from)?,
        )
    ))
}

fn site_deployment_request_hash(
    site: &Site,
    task_id: Option<TaskId>,
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                SITE_SCHEMA_VERSION,
                site.id,
                site.revision,
                &site.definition_hash,
                task_id,
                &site.deployment_command,
            ))
            .map_err(StorageError::from)?,
        )
    ))
}

fn bounded_site_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "Site operation produced no detail".into();
    }
    if value.len() <= 512 {
        return value.into();
    }
    let mut boundary = 512;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value[..boundary].into()
}

fn stable_authority_bytes(namespace: &str, id: impl std::fmt::Display) -> [u8; 16] {
    let digest = Sha256::digest(format!("{namespace}\0{id}").as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

fn settled_computer_action_replay(
    batch: ComputerActionBatch,
) -> Result<PlatformComputerActionMutation, RuntimeError> {
    match batch.state {
        ComputerActionBatchState::Succeeded => Ok(PlatformComputerActionMutation {
            batch,
            replayed: true,
            result: None,
        }),
        ComputerActionBatchState::Executing => {
            Err(ToolError::Denied("computer input batch is already executing".into()).into())
        }
        ComputerActionBatchState::Rejected | ComputerActionBatchState::Uncertain => {
            Err(ToolError::Denied(format!(
                "computer input batch will not be replayed: {}",
                batch.detail
            ))
            .into())
        }
    }
}

#[derive(Clone)]
struct ComputerActionExecutor {
    store: Arc<Mutex<SqliteEventStore>>,
    computer_sessions: Arc<ComputerUseManager>,
    result: Arc<Mutex<Option<PlatformComputerActionMutation>>>,
}

impl ComputerActionExecutor {
    fn store(&self) -> Result<MutexGuard<'_, SqliteEventStore>, ToolError> {
        self.store
            .lock()
            .map_err(|_| ToolError::Execution("runtime storage lock was poisoned".into()))
    }

    fn save_result(&self, result: PlatformComputerActionMutation) -> Result<(), ToolError> {
        *self.result.lock().map_err(|_| {
            ToolError::Execution("computer action result lock was poisoned".into())
        })? = Some(result);
        Ok(())
    }
}

#[async_trait]
impl ToolExecutor for ComputerActionExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let ToolRequestKind::ComputerActions {
            batch_id,
            session_id,
            expected_frame_sha256,
            actions,
        } = &request.kind
        else {
            return Err(ToolError::Invalid(
                "computer executor received a different tool kind".into(),
            ));
        };
        let started_at = Utc::now();
        let action_count = u16::try_from(actions.len())
            .map_err(|_| ToolError::Invalid("too many computer actions".into()))?;
        let request_hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(*session_id, expected_frame_sha256, actions))
                    .map_err(|error| ToolError::Invalid(error.to_string()))?,
            )
        );
        let claimed = ComputerActionBatch {
            schema_version: PLATFORM_SCHEMA_VERSION,
            id: *batch_id,
            session_id: *session_id,
            request_hash,
            state: ComputerActionBatchState::Executing,
            action_count,
            result_frame_sha256: None,
            detail: String::new(),
            created_at: started_at,
            updated_at: started_at,
        };
        let (batch, created) = self
            .store()?
            .claim_computer_action_batch(&claimed)
            .map_err(tool_storage_error)?;
        if !created {
            let replay = settled_computer_action_replay(batch)
                .map_err(|error| ToolError::Denied(error.to_string()))?;
            self.save_result(replay)?;
            return Ok(computer_action_tool_outcome(
                execution_id,
                request,
                started_at,
                true,
            ));
        }
        let execution = self
            .computer_sessions
            .apply(*session_id, expected_frame_sha256, actions)
            .await;
        match execution {
            Ok(execution) => {
                let batch = self
                    .store()?
                    .finish_computer_action_batch(
                        *batch_id,
                        ComputerActionBatchState::Succeeded,
                        Some(execution.frame.sha256.clone()),
                        "computer input batch completed",
                        Utc::now(),
                    )
                    .map_err(tool_storage_error)?;
                self.save_result(PlatformComputerActionMutation {
                    batch,
                    replayed: false,
                    result: Some(execution),
                })?;
                Ok(computer_action_tool_outcome(
                    execution_id,
                    request,
                    started_at,
                    false,
                ))
            }
            Err(error) => {
                let (state, detail) = match &error {
                    ToolError::Invalid(_) | ToolError::Denied(_) | ToolError::Unavailable(_) => (
                        ComputerActionBatchState::Rejected,
                        "computer input batch was rejected before host input",
                    ),
                    _ => (
                        ComputerActionBatchState::Uncertain,
                        "computer input outcome is uncertain and will not be replayed",
                    ),
                };
                self.store()?
                    .finish_computer_action_batch(*batch_id, state, None, detail, Utc::now())
                    .map_err(tool_storage_error)?;
                Err(error)
            }
        }
    }
}

fn computer_action_tool_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    started_at: DateTime<Utc>,
    replayed: bool,
) -> ToolOutcome {
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: if replayed {
            "computer action batch was already durably settled".into()
        } else {
            "bounded computer action batch completed".into()
        },
        exit_code: None,
        stdout: None,
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: "exact_frame_digest".into(),
            passed: true,
            detail: "captured pixels matched immediately before bounded input".into(),
        }],
        started_at,
        finished_at: Utc::now(),
    }
}

fn settled_site_deployment_replay(
    deployment: SiteDeployment,
) -> Result<SiteDeploymentMutation, RuntimeError> {
    match deployment.state {
        SiteDeploymentState::Succeeded => Ok(SiteDeploymentMutation {
            deployment,
            replayed: true,
            outcome: None,
        }),
        SiteDeploymentState::Executing => {
            Err(ToolError::Denied("Site deployment is already executing".into()).into())
        }
        SiteDeploymentState::Failed | SiteDeploymentState::Uncertain => {
            Err(ToolError::Denied(format!(
                "Site deployment will not be replayed: {}",
                deployment.detail
            ))
            .into())
        }
    }
}

#[derive(Clone)]
struct SiteDeploymentExecutor {
    store: Arc<Mutex<SqliteEventStore>>,
    process: Arc<ProcessExecutor>,
    artifacts: Arc<ArtifactStore>,
    result: Arc<Mutex<Option<SiteDeploymentMutation>>>,
}

impl SiteDeploymentExecutor {
    fn store(&self) -> Result<MutexGuard<'_, SqliteEventStore>, ToolError> {
        self.store
            .lock()
            .map_err(|_| ToolError::Execution("runtime storage lock was poisoned".into()))
    }

    fn save_result(&self, result: SiteDeploymentMutation) -> Result<(), ToolError> {
        *self.result.lock().map_err(|_| {
            ToolError::Execution("Site deployment result lock was poisoned".into())
        })? = Some(result);
        Ok(())
    }
}

#[async_trait]
impl ToolExecutor for SiteDeploymentExecutor {
    #[allow(clippy::too_many_lines)] // Claim, process execution, evidence, and settlement are ordered.
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let ToolRequestKind::SiteDeploy {
            deployment_id,
            site_id,
            site_revision,
            task_id,
            options,
        } = &request.kind
        else {
            return Err(ToolError::Invalid(
                "Site executor received a different tool kind".into(),
            ));
        };
        let site = self
            .store()?
            .site(*site_id)
            .map_err(tool_storage_error)?
            .ok_or_else(|| ToolError::Invalid("Site was not found".into()))?;
        let command = site
            .deployment_command
            .as_ref()
            .ok_or_else(|| ToolError::Unavailable("Site deployment command is absent".into()))?;
        if site.state != SiteState::Active
            || site.revision != *site_revision
            || command.executable != options.executable
            || command.arguments != options.argv
            || !options.env.is_empty()
        {
            return Err(ToolError::Denied(
                "Site definition or command changed after authority was issued".into(),
            ));
        }
        let started_at = Utc::now();
        let request_hash = site_deployment_request_hash(&site, *task_id)
            .map_err(|error| ToolError::Invalid(error.to_string()))?;
        let claimed = SiteDeployment {
            schema_version: SITE_SCHEMA_VERSION,
            id: *deployment_id,
            site_id: *site_id,
            site_revision: *site_revision,
            task_id: *task_id,
            request_hash,
            state: SiteDeploymentState::Executing,
            detail: String::new(),
            log_artifact_sha256: Vec::new(),
            created_at: started_at,
            updated_at: started_at,
        };
        let (deployment, created) = self
            .store()?
            .claim_site_deployment(&claimed)
            .map_err(tool_storage_error)?;
        if !created {
            let replay = settled_site_deployment_replay(deployment)
                .map_err(|error| ToolError::Denied(error.to_string()))?;
            self.save_result(replay)?;
            return Ok(site_deployment_replay_outcome(
                execution_id,
                request,
                started_at,
            ));
        }
        let mut process_request = request.clone();
        process_request.kind = ToolRequestKind::ProcessExec {
            options: options.clone(),
        };
        let execution = self.process.execute(execution_id, &process_request).await;
        match execution {
            Ok(mut outcome) => {
                let log_artifact = capture_site_process_log(self.artifacts.as_ref(), &outcome)?;
                let log_hashes = log_artifact
                    .as_ref()
                    .map(|artifact| vec![artifact.sha256.clone()])
                    .unwrap_or_default();
                if let Some(artifact) = log_artifact {
                    outcome.artifacts.push(artifact);
                }
                let deployment_state = if outcome.status == ToolOutcomeStatus::Succeeded {
                    SiteDeploymentState::Succeeded
                } else {
                    SiteDeploymentState::Failed
                };
                let deployment = self
                    .store()?
                    .finish_site_deployment(
                        *deployment_id,
                        deployment_state,
                        &bounded_site_detail(&outcome.summary),
                        log_hashes,
                        Utc::now(),
                    )
                    .map_err(tool_storage_error)?;
                self.save_result(SiteDeploymentMutation {
                    deployment,
                    replayed: false,
                    outcome: Some(outcome.clone()),
                })?;
                Ok(outcome)
            }
            Err(error) => {
                self.store()?
                    .finish_site_deployment(
                        *deployment_id,
                        SiteDeploymentState::Failed,
                        "Site deployment process failed before verified completion",
                        Vec::new(),
                        Utc::now(),
                    )
                    .map_err(tool_storage_error)?;
                Err(error)
            }
        }
    }
}

fn capture_site_process_log(
    artifacts: &ArtifactStore,
    outcome: &ToolOutcome,
) -> Result<Option<mcg_tools::ArtifactReference>, ToolError> {
    if outcome.stdout.is_none() && outcome.stderr.is_none() {
        return Ok(None);
    }
    let mut log = String::new();
    if let Some(stdout) = &outcome.stdout {
        log.push_str("stdout:\n");
        log.push_str(stdout);
        log.push('\n');
    }
    if let Some(stderr) = &outcome.stderr {
        log.push_str("stderr:\n");
        log.push_str(stderr);
        log.push('\n');
    }
    artifacts
        .capture(log.as_bytes(), "text/plain; charset=utf-8")
        .map(Some)
}

fn site_deployment_replay_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    started_at: DateTime<Utc>,
) -> ToolOutcome {
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: "Site deployment was already durably settled".into(),
        exit_code: None,
        stdout: None,
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: "site.deployment_replay".into(),
            passed: true,
            detail: "no deployment process was repeated".into(),
        }],
        started_at,
        finished_at: Utc::now(),
    }
}

fn settled_ssh_operation_replay(
    operation: SshOperation,
) -> Result<SshOperationMutation, RuntimeError> {
    match operation.state {
        SshOperationState::Succeeded => Ok(SshOperationMutation {
            operation,
            replayed: true,
            outcome: None,
        }),
        SshOperationState::Executing => {
            Err(ToolError::Denied("SSH operation is already executing".into()).into())
        }
        SshOperationState::Failed | SshOperationState::Uncertain => Err(ToolError::Denied(
            format!("SSH operation will not be replayed: {}", operation.detail),
        )
        .into()),
    }
}

#[derive(Clone)]
struct SshOperationExecutor {
    store: Arc<Mutex<SqliteEventStore>>,
    ssh: Arc<SshExecutor>,
    artifacts: Arc<ArtifactStore>,
    request_hash: String,
    task_id: Option<TaskId>,
    result: Arc<Mutex<Option<SshOperationMutation>>>,
}

impl SshOperationExecutor {
    fn store(&self) -> Result<MutexGuard<'_, SqliteEventStore>, ToolError> {
        self.store
            .lock()
            .map_err(|_| ToolError::Execution("runtime storage lock was poisoned".into()))
    }

    fn save_result(&self, result: SshOperationMutation) -> Result<(), ToolError> {
        *self
            .result
            .lock()
            .map_err(|_| ToolError::Execution("SSH operation result lock was poisoned".into()))? =
            Some(result);
        Ok(())
    }
}

#[async_trait]
impl ToolExecutor for SshOperationExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        let ToolRequestKind::SshOperation {
            operation_id,
            host_id,
            host_revision,
            kind,
            ..
        } = &request.kind
        else {
            return Err(ToolError::Invalid(
                "SSH operation executor received a different tool kind".into(),
            ));
        };
        let started_at = Utc::now();
        let claimed = SshOperation {
            schema_version: SSH_SCHEMA_VERSION,
            id: *operation_id,
            host_id: *host_id,
            host_revision: *host_revision,
            task_id: self.task_id,
            request_hash: self.request_hash.clone(),
            kind: kind.clone(),
            state: SshOperationState::Executing,
            detail: String::new(),
            exit_code: None,
            artifact_sha256: Vec::new(),
            created_at: started_at,
            updated_at: started_at,
        };
        let (operation, created) = self
            .store()?
            .claim_ssh_operation(&claimed)
            .map_err(tool_storage_error)?;
        if !created {
            let replay = settled_ssh_operation_replay(operation)
                .map_err(|error| ToolError::Denied(error.to_string()))?;
            self.save_result(replay)?;
            return Ok(ssh_operation_replay_outcome(
                execution_id,
                request,
                started_at,
            ));
        }

        match self.ssh.execute(execution_id, request).await {
            Ok(mut outcome) => {
                let mut evidence = capture_ssh_evidence(self.artifacts.as_ref(), kind, &outcome)?;
                let artifact_sha256 = evidence
                    .iter()
                    .map(|artifact| artifact.sha256.clone())
                    .collect();
                outcome.artifacts.append(&mut evidence);
                let state = if outcome.status == ToolOutcomeStatus::Succeeded {
                    SshOperationState::Succeeded
                } else {
                    SshOperationState::Failed
                };
                let operation = self
                    .store()?
                    .finish_ssh_operation(
                        *operation_id,
                        state,
                        &bounded_site_detail(&outcome.summary),
                        outcome.exit_code,
                        artifact_sha256,
                        Utc::now(),
                    )
                    .map_err(tool_storage_error)?;
                self.save_result(SshOperationMutation {
                    operation,
                    replayed: false,
                    outcome: Some(outcome.clone()),
                })?;
                Ok(outcome)
            }
            Err(error) => {
                self.store()?
                    .finish_ssh_operation(
                        *operation_id,
                        SshOperationState::Failed,
                        "OpenSSH failed before verified completion",
                        None,
                        Vec::new(),
                        Utc::now(),
                    )
                    .map_err(tool_storage_error)?;
                Err(error)
            }
        }
    }
}

fn capture_ssh_evidence(
    artifacts: &ArtifactStore,
    kind: &SshOperationKind,
    outcome: &ToolOutcome,
) -> Result<Vec<mcg_tools::ArtifactReference>, ToolError> {
    let mut evidence = Vec::new();
    if let Some(log) = capture_site_process_log(artifacts, outcome)? {
        evidence.push(log);
    }
    let local_path = match kind {
        SshOperationKind::Upload { local_path, .. } => Some(local_path),
        SshOperationKind::Download { local_path, .. }
            if outcome.status == ToolOutcomeStatus::Succeeded =>
        {
            Some(local_path)
        }
        SshOperationKind::Inspect
        | SshOperationKind::Execute { .. }
        | SshOperationKind::Download { .. } => None,
    };
    if let Some(local_path) = local_path {
        let file = fs::File::open(local_path)
            .map_err(|error| ToolError::Execution(format!("open SSH file evidence: {error}")))?;
        evidence.push(artifacts.capture_reader(
            file,
            1024 * 1024 * 1024,
            "application/octet-stream",
        )?);
    }
    Ok(evidence)
}

fn ssh_operation_replay_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    started_at: DateTime<Utc>,
) -> ToolOutcome {
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary: "SSH operation was already durably settled".into(),
        exit_code: None,
        stdout: None,
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: "ssh.operation_replay".into(),
            passed: true,
            detail: "no SSH process or transfer was repeated".into(),
        }],
        started_at,
        finished_at: Utc::now(),
    }
}

#[derive(Clone)]
pub struct DurableToolControl {
    store: Arc<Mutex<SqliteEventStore>>,
    event_sender: broadcast::Sender<EventEnvelope>,
}

#[derive(Clone)]
struct DurableAgentTurnCheckpointSink {
    store: Arc<Mutex<SqliteEventStore>>,
}

/// Resolves ordinary model-facing project-relative paths into the exact root
/// already granted to the broker. Traversal and absolute paths are deliberately
/// left untouched so the existing capability and executor checks still deny
/// unsafe requests and retain their normal audit evidence.
struct ProjectScopedAgentTools<'a, T> {
    inner: &'a T,
    root: PathBuf,
}

impl<'a, T> ProjectScopedAgentTools<'a, T> {
    fn new(inner: &'a T, root: PathBuf) -> Self {
        Self { inner, root }
    }
}

#[async_trait]
impl<T: AgentToolGateway> AgentToolGateway for ProjectScopedAgentTools<'_, T> {
    async fn execute(&self, request: &ToolRequest) -> Result<ToolOutcome, ToolError> {
        let mut scoped = request.clone();
        resolve_project_relative_tool_paths(&mut scoped.kind, &self.root);
        self.inner.execute(&scoped).await
    }
}

fn resolve_project_relative_tool_paths(kind: &mut ToolRequestKind, root: &Path) {
    let resolve = |value: &mut String| {
        let supplied = Path::new(value);
        if supplied.is_absolute()
            || supplied.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return;
        }
        let mut resolved = root.to_path_buf();
        for component in supplied.components() {
            if let Component::Normal(segment) = component {
                resolved.push(segment);
            }
        }
        *value = resolved.to_string_lossy().into_owned();
    };
    match kind {
        ToolRequestKind::FsRead { path, .. }
        | ToolRequestKind::FsVerify { path, .. }
        | ToolRequestKind::FsWrite { path, .. }
        | ToolRequestKind::FsPatch { path, .. }
        | ToolRequestKind::FsList { path, .. }
        | ToolRequestKind::FsMkdir { path, .. }
        | ToolRequestKind::FsDelete { path, .. } => resolve(path),
        ToolRequestKind::FsMove { from, to } | ToolRequestKind::FsCopy { from, to } => {
            resolve(from);
            resolve(to);
        }
        ToolRequestKind::ProcessExec { options }
        | ToolRequestKind::ProcessSpawn { options }
        | ToolRequestKind::SiteDeploy { options, .. } => resolve(&mut options.cwd),
        ToolRequestKind::ShellExec { cwd, .. } => resolve(cwd),
        ToolRequestKind::HttpDownload { destination, .. } => resolve(destination),
        ToolRequestKind::BrowserWorkflow { workflow_path, .. } => resolve(workflow_path),
        ToolRequestKind::BrowserScreenshotMatches { baseline_path, .. } => {
            resolve(baseline_path);
        }
        ToolRequestKind::ResearchVerify { evidence_path, .. } => resolve(evidence_path),
        ToolRequestKind::GitStatus { repository }
        | ToolRequestKind::GitDiff { repository, .. }
        | ToolRequestKind::GitCommit { repository, .. }
        | ToolRequestKind::GitCheckout { repository, .. }
        | ToolRequestKind::GitApplyPatch { repository, .. } => resolve(repository),
        ToolRequestKind::GitWorktreeCreate {
            repository,
            worktree,
            ..
        }
        | ToolRequestKind::GitWorktreeRemove {
            repository,
            worktree,
            ..
        } => {
            resolve(repository);
            resolve(worktree);
        }
        ToolRequestKind::ProcessStatus { .. }
        | ToolRequestKind::ProcessStdin { .. }
        | ToolRequestKind::ProcessKill { .. }
        | ToolRequestKind::HttpRequest { .. }
        | ToolRequestKind::TcpConnect { .. }
        | ToolRequestKind::BrowserRender { .. }
        | ToolRequestKind::BrowserElementExists { .. }
        | ToolRequestKind::BrowserAccessibility { .. }
        | ToolRequestKind::SystemInfo
        | ToolRequestKind::SystemEnv { .. }
        | ToolRequestKind::SystemProcesses
        | ToolRequestKind::MemorySearch { .. }
        | ToolRequestKind::MemoryRead { .. }
        | ToolRequestKind::MemoryWrite { .. }
        | ToolRequestKind::ComputerActions { .. }
        | ToolRequestKind::SshOperation { .. }
        | ToolRequestKind::McpCall { .. } => {}
    }
}

impl AgentTurnCheckpointSink for DurableAgentTurnCheckpointSink {
    fn save(&self, checkpoint: &AgentTurnCheckpoint) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|_| "runtime storage lock was poisoned".to_owned())?
            .save_agent_turn_checkpoint(checkpoint)
            .map_err(|error| error.to_string())
    }
}

impl DurableToolControl {
    fn store(&self) -> Result<MutexGuard<'_, SqliteEventStore>, ToolError> {
        self.store
            .lock()
            .map_err(|_| ToolError::Execution("runtime storage lock was poisoned".into()))
    }
}

#[async_trait]
impl ApprovalRepository for DurableToolControl {
    async fn find_for(
        &self,
        request_id: ToolRequestId,
    ) -> Result<Option<ApprovalRequest>, ToolError> {
        self.store()?
            .approval_for_request(request_id)
            .map_err(tool_storage_error)
    }

    async fn create(&self, request: ApprovalRequest) -> Result<(), ToolError> {
        let event = self
            .store()?
            .create_approval(&request)
            .map_err(tool_storage_error)?;
        let _ = self.event_sender.send(event);
        Ok(())
    }
}

#[async_trait]
impl ToolAuditSink for DurableToolControl {
    async fn record(&self, record: ToolAuditRecord) -> Result<(), ToolError> {
        let event = self
            .store()?
            .record_tool_audit(&record)
            .map_err(tool_storage_error)?;
        let _ = self.event_sender.send(event);
        Ok(())
    }
}

#[async_trait]
impl ExecutionLeaseRepository for DurableToolControl {
    async fn acquire(
        &self,
        request_id: ToolRequestId,
        execution_id: ToolExecutionId,
        duration: Duration,
    ) -> Result<ExecutionLease, ToolError> {
        self.store()?
            .acquire_tool_lease(request_id, execution_id, duration)
            .map_err(|error| ToolError::Lease(error.to_string()))
    }

    async fn finish(&self, lease_id: LeaseId, state: LeaseState) -> Result<(), ToolError> {
        self.store()?
            .finish_tool_lease(lease_id, state)
            .map_err(|error| ToolError::Lease(error.to_string()))
    }
}

fn role_tool_names(role: RoleKind) -> &'static [&'static str] {
    match role {
        RoleKind::Coder => &[
            "fs.read",
            "fs.list",
            "fs.write",
            "fs.patch",
            "fs.mkdir",
            "process.exec",
            "git.status",
            "git.diff",
            "git.apply_patch",
        ],
        RoleKind::Tester | RoleKind::Evaluator => &[
            "fs.read",
            "fs.list",
            "process.exec",
            "git.status",
            "git.diff",
        ],
        RoleKind::Researcher => &["http.request"],
        RoleKind::ToolWorker => &["fs.read", "fs.list", "process.exec", "system.info"],
        RoleKind::TemporarySpecialist => &["fs.read", "fs.list"],
        RoleKind::Executive | RoleKind::Conversation => &[],
    }
}

fn resolve_role_tools(role: RoleKind) -> Result<Vec<ToolDefinition>, StorageError> {
    role_tool_names(role)
        .iter()
        .map(|name| {
            canonical_host_tool_definition(name).ok_or_else(|| {
                StorageError::InvalidData(format!("tool {name} has no runtime schema"))
            })
        })
        .collect()
}

fn parse_mcp_tool_request(value: &str) -> Option<(&str, &str)> {
    let mut parts = value.splitn(3, "::");
    match (parts.next(), parts.next(), parts.next()) {
        (Some("mcp"), Some(connection_id), Some(tool_name))
            if !connection_id.is_empty() && !tool_name.is_empty() =>
        {
            Some((connection_id, tool_name))
        }
        _ => None,
    }
}

fn exposed_mcp_tool_name(connection_id: &str, remote_name: &str) -> String {
    let slug = |value: &str, max: usize| {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
            .take(max)
            .collect::<String>()
    };
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{connection_id}\0{remote_name}"))
    );
    format!(
        "mcp_{}_{}_{}",
        slug(connection_id, 16),
        slug(remote_name, 20),
        &digest[..16]
    )
}

fn mcp_tool_classification(annotations: &serde_json::Value) -> (bool, bool, bool) {
    let value = |name: &str, default| {
        annotations
            .get(name)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    };
    let read_only = value("readOnlyHint", false);
    let destructive = annotations
        .get("destructiveHint")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(!read_only);
    (read_only, destructive, value("openWorldHint", true))
}

fn protocol_mode_for_catalog(protocol_version: &str) -> Result<McpProtocolMode, McpError> {
    match protocol_version {
        MCP_PROTOCOL_VERSION => Ok(McpProtocolMode::Modern20260728),
        LEGACY_MCP_PROTOCOL_VERSION => Ok(McpProtocolMode::Legacy),
        other => Err(McpError::Negotiation(format!(
            "stored MCP catalog uses unsupported protocol version {other}"
        ))),
    }
}

/// Returns MCG's canonical model-facing JSON schema for one host tool.
/// Caller-supplied descriptions and schemas must never replace this authority
/// for an MCG-owned execution path.
#[must_use]
#[allow(clippy::too_many_lines)] // One exhaustive schema registry is easier to audit than scattered schemas.
pub fn canonical_host_tool_definition(name: &str) -> Option<ToolDefinition> {
    let properties = |fields: serde_json::Value, required: &[&str]| {
        serde_json::json!({
            "type": "object",
            "properties": fields,
            "required": required,
            "additionalProperties": false
        })
    };
    let parameters = match name {
        "fs.read" => properties(
            serde_json::json!({
                "path": {"type": "string"},
                "max_bytes": {"type": "integer", "minimum": 1}
            }),
            &["path", "max_bytes"],
        ),
        "fs.list" => properties(
            serde_json::json!({
                "path": {"type": "string"},
                "recursive": {"type": "boolean"},
                "max_entries": {"type": "integer", "minimum": 1}
            }),
            &["path", "recursive", "max_entries"],
        ),
        "fs.write" => properties(
            serde_json::json!({
                "path": {"type": "string"},
                "content": {"type": "string"},
                "create_new": {"type": "boolean"}
            }),
            &["path", "content", "create_new"],
        ),
        "fs.patch" => properties(
            serde_json::json!({
                "path": {"type": "string"},
                "expected_sha256": {"type": "string"},
                "replacement": {"type": "string"}
            }),
            &["path", "expected_sha256", "replacement"],
        ),
        "fs.mkdir" => properties(
            serde_json::json!({
                "path": {"type": "string"},
                "recursive": {"type": "boolean"}
            }),
            &["path", "recursive"],
        ),
        "process.exec" => properties(
            serde_json::json!({
                "options": {
                    "type": "object",
                    "properties": {
                        "executable": {"type": "string"},
                        "argv": {"type": "array", "items": {"type": "string"}},
                        "cwd": {"type": "string"},
                        "env": {"type": "array", "maxItems": 0},
                        "stdin": {"type": "string", "enum": ["null", "piped", "inherit"]},
                        "stdin_data": {"type": ["string", "null"]},
                        "stdout": {"type": "string", "enum": ["null", "piped", "inherit"]},
                        "stderr": {"type": "string", "enum": ["null", "piped", "inherit"]}
                    },
                    "required": ["executable", "argv", "cwd", "env", "stdin", "stdin_data", "stdout", "stderr"],
                    "additionalProperties": false
                }
            }),
            &["options"],
        ),
        "git.status" => properties(
            serde_json::json!({"repository": {"type": "string"}}),
            &["repository"],
        ),
        "git.diff" => properties(
            serde_json::json!({
                "repository": {"type": "string"},
                "staged": {"type": "boolean"}
            }),
            &["repository", "staged"],
        ),
        "git.apply_patch" => properties(
            serde_json::json!({
                "repository": {"type": "string"},
                "patch": {"type": "string"},
                "three_way": {"type": "boolean"},
                "check_only": {"type": "boolean"}
            }),
            &["repository", "patch", "three_way", "check_only"],
        ),
        "http.request" => properties(
            serde_json::json!({
                "method": {"type": "string"},
                "url": {"type": "string"},
                "headers": {"type": "array", "maxItems": 0},
                "body": {"type": ["string", "null"]},
                "max_response_bytes": {"type": "integer", "minimum": 1}
            }),
            &["method", "url", "headers", "body", "max_response_bytes"],
        ),
        "system.info" => properties(serde_json::json!({}), &[]),
        _ => return None,
    };
    Some(ToolDefinition {
        name: name.to_owned(),
        description: Some(format!("Governed {name} operation")),
        parameters,
        strict: Some(true),
    })
}

/// Derives the stable governed request identity for one canonical provider tool call.
#[must_use]
pub fn provider_managed_tool_request_id(
    call_id: ProxyCallId,
    model_tool_call_id: &str,
    qualified_name: &str,
    canonical_arguments: &str,
) -> ToolRequestId {
    let mut hash = Sha256::new();
    hash.update(b"mcg-provider-managed-tool-v1");
    hash.update([0]);
    hash.update(call_id.to_string().as_bytes());
    hash.update([0]);
    hash.update(model_tool_call_id.as_bytes());
    hash.update([0]);
    hash.update(qualified_name.as_bytes());
    hash.update([0]);
    hash.update(canonical_arguments.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    ToolRequestId::from_stable_bytes(bytes)
}

fn require_provider_mcg_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    allowed: &[&str],
) -> Result<(), RuntimeError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(StorageError::InvalidData(
            "provider-managed MCG arguments contain an unknown field".into(),
        )
        .into());
    }
    Ok(())
}

fn take_bounded_required_string(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    max_len: usize,
) -> Result<String, RuntimeError> {
    take_bounded_optional_string(object, key, max_len)?.ok_or_else(|| {
        StorageError::InvalidData(format!("provider-managed MCG argument {key} is required")).into()
    })
}

fn take_bounded_optional_string(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    max_len: usize,
) -> Result<Option<String>, RuntimeError> {
    object
        .remove(key)
        .map(|value| {
            value
                .as_str()
                .filter(|value| {
                    !value.trim().is_empty()
                        && value.len() <= max_len
                        && !value.chars().any(char::is_control)
                })
                .map(str::to_owned)
                .ok_or_else(|| {
                    StorageError::InvalidData(format!(
                        "provider-managed MCG argument {key} is invalid"
                    ))
                    .into()
                })
        })
        .transpose()
}

fn take_bounded_optional_u64(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
) -> Result<Option<u64>, RuntimeError> {
    object
        .remove(key)
        .map(|value| {
            value
                .as_u64()
                .filter(|value| (*value >= minimum) && (*value <= maximum))
                .ok_or_else(|| {
                    StorageError::InvalidData(format!(
                        "provider-managed MCG argument {key} is out of bounds"
                    ))
                    .into()
                })
        })
        .transpose()
}

fn validate_provider_mcg_relative_target(value: &str) -> Result<String, RuntimeError> {
    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized.as_bytes().get(1) == Some(&b':')
        || normalized
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(StorageError::InvalidData(
            "provider-managed MCG relative target is not normalized and project-relative".into(),
        )
        .into());
    }
    Ok(normalized)
}

fn runtime_tool_error(error: ToolError) -> RuntimeError {
    AgentTurnError::Tool(error).into()
}

fn objective_command(condition: &SuccessCondition) -> Option<&str> {
    match condition {
        SuccessCondition::Build { command }
        | SuccessCondition::Test { command }
        | SuccessCondition::StaticAnalysis { command }
        | SuccessCondition::ConfigurationValid { command } => Some(command),
        SuccessCondition::Custom { description } => objective_custom_command(description),
        _ => None,
    }
}

fn baseline_service_command(project: &ProjectModel) -> Option<&str> {
    let starts = project
        .commands
        .iter()
        .filter(|command| command.name == "start")
        .collect::<Vec<_>>();
    match starts.as_slice() {
        [command] if !command.command.trim().is_empty() => return Some(command.command.as_str()),
        [] => {}
        _ => return None,
    }
    let development = project
        .commands
        .iter()
        .filter(|command| command.name == "dev")
        .collect::<Vec<_>>();
    match development.as_slice() {
        [command] if !command.command.trim().is_empty() => Some(command.command.as_str()),
        _ => None,
    }
}

fn objective_custom_command(description: &str) -> Option<&str> {
    description
        .strip_prefix("COMMAND SUCCEEDS ")
        .map(str::trim)
        .filter(|command| !command.is_empty())
}

fn objective_condition_label(condition: &SuccessCondition) -> Option<String> {
    objective_command(condition).map(str::to_owned).or_else(|| {
        if let SuccessCondition::HttpResponds { url, status } = condition {
            Some(format!("HTTP GET {url} returns {status}"))
        } else if let SuccessCondition::ServiceHealthy { endpoint } = condition {
            Some(format!("Service is healthy at {endpoint}"))
        } else if let SuccessCondition::PortListening { endpoint } = condition {
            Some(format!("TCP port accepts connections at {endpoint}"))
        } else if let SuccessCondition::PageRenders { url } = condition {
            Some(format!("Page renders at {url}"))
        } else if let SuccessCondition::Accessibility { url } = condition {
            Some(format!("Accessibility baseline passes at {url}"))
        } else if let SuccessCondition::ElementExists { url, selector } = condition {
            Some(format!("Element {selector} exists at {url}"))
        } else if let SuccessCondition::BrowserWorkflow { name, path, .. } = condition {
            Some(format!("Browser workflow {name} from {path}"))
        } else if let SuccessCondition::ScreenshotMatches { baseline, url, .. } = condition {
            Some(format!("Screenshot at {url} matches {baseline}"))
        } else if let SuccessCondition::FileExists { path, .. } = condition {
            Some(format!("File exists: {path}"))
        } else if let SuccessCondition::ResearchSources {
            evidence_path,
            minimum,
            ..
        } = condition
        {
            Some(format!(
                "Research evidence in {evidence_path} uses at least {minimum} sources"
            ))
        } else {
            None
        }
    })
}

fn objective_project_file_path(project_root: &str, relative: &str) -> Result<String, RuntimeError> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::CurDir
                    | std::path::Component::ParentDir
            )
        })
    {
        return Err(StorageError::InvalidData(
            "file objective path must be normalized and project-relative".into(),
        )
        .into());
    }
    Ok(Path::new(project_root)
        .join(relative)
        .to_string_lossy()
        .into_owned())
}

fn watch_file_fingerprint(project_root: &Path, target: &Path) -> Result<String, RuntimeError> {
    const MAX_WATCH_BYTES: u64 = 16 * 1024 * 1024;
    let relative = target.strip_prefix(project_root).map_err(|_| {
        StorageError::InvalidData("file watch escaped its durable project root".into())
    })?;
    let mut current = project_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(StorageError::InvalidData(
                    "file watches reject symbolic-link traversal".into(),
                )
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok("missing".into());
            }
            Err(error) => return Err(error.into()),
        }
    }
    let metadata = fs::metadata(target)?;
    if !metadata.is_file() || metadata.len() > MAX_WATCH_BYTES {
        return Err(StorageError::InvalidData(
            "file watches require a regular file no larger than 16 MiB".into(),
        )
        .into());
    }
    let mut file = std::fs::File::open(target)?;
    let mut hash = Sha256::new();
    hash.update(b"mcg-file-watch-v1\0");
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn objective_network_origin(value: &str) -> Result<String, RuntimeError> {
    let url = url::Url::parse(value)
        .map_err(|error| StorageError::InvalidData(format!("invalid objective URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(StorageError::InvalidData(
            "objective HTTP checks require a credential-free HTTP(S) URL".into(),
        )
        .into());
    }
    Ok(url.origin().ascii_serialization())
}

fn objective_tcp_origin(value: &str) -> Result<String, RuntimeError> {
    let url = url::Url::parse(value).map_err(|error| {
        StorageError::InvalidData(format!("invalid objective TCP endpoint: {error}"))
    })?;
    if url.scheme() != "tcp"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.port().is_none()
        || !url.path().is_empty()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(StorageError::InvalidData(
            "objective port checks require tcp://host:port without credentials, path, query, or fragment"
                .into(),
        )
        .into());
    }
    Ok(url.to_string())
}

fn objective_workflow_origins(
    document: &BrowserWorkflowDocument,
) -> Result<BTreeSet<String>, ToolError> {
    let start = objective_network_origin(&document.start_url)
        .map_err(|error| ToolError::Execution(format!("invalid workflow start URL: {error}")))?;
    let mut origins = BTreeSet::from([start.clone()]);
    for step in &document.steps {
        if let BrowserWorkflowStep::AssertUrl { url } = step {
            let origin = objective_network_origin(url).map_err(|error| {
                ToolError::Execution(format!("invalid workflow URL assertion: {error}"))
            })?;
            if origin != start {
                return Err(ToolError::Denied(
                    "browser workflows cannot authorize cross-origin navigation".into(),
                ));
            }
            origins.insert(origin);
        }
    }
    Ok(origins)
}

fn objective_evidence_method(condition: &SuccessCondition) -> &'static str {
    match condition {
        SuccessCondition::ConfigurationValid { .. } => "governed_configuration_validation",
        SuccessCondition::HttpResponds { .. } => "governed_http_status",
        SuccessCondition::ServiceHealthy { .. } => "governed_service_health",
        SuccessCondition::PortListening { .. } => "governed_tcp_port",
        SuccessCondition::PageRenders { .. } => "governed_browser_render",
        SuccessCondition::Accessibility { .. } => "governed_browser_accessibility",
        SuccessCondition::ElementExists { .. } => "governed_browser_element",
        SuccessCondition::BrowserWorkflow { .. } => "governed_browser_workflow",
        SuccessCondition::ScreenshotMatches { .. } => "governed_screenshot_comparison",
        SuccessCondition::FileExists { .. } => "governed_file_artifact",
        SuccessCondition::ResearchSources { .. } => "governed_research_sources",
        _ => "governed_process_exit",
    }
}

async fn verify_isolated_file_condition(
    verifier: &FilesystemExecutor,
    task_id: TaskId,
    workspace_root: &Path,
    condition: &SuccessCondition,
) -> Result<(ConditionResult, EvaluationEvidence), RuntimeError> {
    let SuccessCondition::FileExists { path, .. } = condition else {
        return Err(StorageError::InvalidData(
            "isolated file verifier received a different condition kind".into(),
        )
        .into());
    };
    let absolute = objective_project_file_path(workspace_root.to_string_lossy().as_ref(), path)?;
    let request = ToolRequest::new(
        task_id,
        "verify isolated shadow file objective",
        ToolRequestKind::FsVerify {
            path: absolute,
            max_bytes: SHADOW_MAX_FILE_BYTES,
        },
    );
    match verifier.execute(ToolExecutionId::new(), &request).await {
        Ok(outcome) => {
            let passed = objective_outcome_passed(condition, &outcome);
            Ok((
                ConditionResult {
                    condition: condition.clone(),
                    passed,
                    detail: objective_condition_outcome_detail(condition, &outcome),
                },
                EvaluationEvidence {
                    method: "isolated_file_verification".into(),
                    artifact_sha256: objective_evidence_artifact(condition, &outcome)
                        .map(|artifact| artifact.sha256.clone()),
                    event_sequence: None,
                    detail: if passed {
                        "A regular file in the retained private workspace satisfied its size and digest requirements.".into()
                    } else {
                        "A regular file was captured from the retained private workspace but did not satisfy its declared size or digest requirements.".into()
                    },
                },
            ))
        }
        Err(_) => Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "Private-workspace file verification failed closed; no qualifying regular-file artifact was captured.".into(),
            },
            EvaluationEvidence {
                method: "isolated_file_verification".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "The declared file could not be captured safely from the retained private workspace.".into(),
            },
        )),
    }
}

fn baseline_research_source_urls(
    artifacts: &ArtifactStore,
    baseline: &EvaluationReport,
    condition: &SuccessCondition,
) -> Option<BTreeSet<String>> {
    const MAX_EVIDENCE_BYTES: u64 = 1_048_576;
    let SuccessCondition::ResearchSources { required_scope, .. } = condition else {
        return None;
    };
    if !baseline
        .conditions
        .iter()
        .any(|result| result.passed && result.condition == *condition)
    {
        return None;
    }
    let mut matches = baseline
        .evidence
        .iter()
        .filter(|evidence| evidence.method == "governed_research_sources")
        .filter_map(|evidence| evidence.artifact_sha256.as_deref())
        .filter_map(|digest| {
            artifacts
                .read_verified_digest(digest, MAX_EVIDENCE_BYTES)
                .ok()
        })
        .filter_map(|bytes| serde_json::from_slice::<ResearchEvidenceDocument>(&bytes).ok())
        .filter(|document| document.scope.trim() == required_scope.trim())
        .map(|document| {
            document
                .claims
                .into_iter()
                .flat_map(|claim| claim.citations)
                .map(|citation| citation.url)
                .collect::<BTreeSet<_>>()
        });
    let urls = matches.next()?;
    if urls.is_empty() || matches.next().is_some() {
        None
    } else {
        Some(urls)
    }
}

#[allow(clippy::too_many_lines)] // Keep baseline binding, artifact capture, and fetch settlement auditable together.
async fn verify_isolated_research_condition(
    filesystem: &FilesystemExecutor,
    research: &ResearchExecutor,
    task_id: TaskId,
    workspace_root: &Path,
    condition: &SuccessCondition,
    baseline_urls: Option<&BTreeSet<String>>,
) -> Result<(ConditionResult, EvaluationEvidence, bool), RuntimeError> {
    const MAX_EVIDENCE_BYTES: u64 = 1_048_576;
    let SuccessCondition::ResearchSources {
        evidence_path,
        minimum,
        max_age_days,
        required_scope,
    } = condition
    else {
        return Err(StorageError::InvalidData(
            "isolated research verifier received a different condition kind".into(),
        )
        .into());
    };
    let Some(baseline_urls) = baseline_urls else {
        return Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "The source evaluation has no unique, passing immutable research-source binding; automatic verification remains inconclusive.".into(),
            },
            EvaluationEvidence {
                method: "isolated_research_verification".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "No candidate file was trusted and no source was fetched because baseline authorization could not be re-established from immutable evidence.".into(),
            },
            false,
        ));
    };
    let absolute =
        objective_project_file_path(workspace_root.to_string_lossy().as_ref(), evidence_path)?;
    let read_request = ToolRequest::new(
        task_id,
        "read isolated shadow research evidence",
        ToolRequestKind::FsRead {
            path: absolute.clone(),
            max_bytes: MAX_EVIDENCE_BYTES,
        },
    );
    let read = match filesystem
        .execute(ToolExecutionId::new(), &read_request)
        .await
    {
        Ok(outcome) if outcome.status == ToolOutcomeStatus::Succeeded => outcome,
        _ => {
            return Ok((
                ConditionResult {
                    condition: condition.clone(),
                    passed: false,
                    detail: "Private-workspace research evidence could not be read as one bounded regular UTF-8 file.".into(),
                },
                EvaluationEvidence {
                    method: "isolated_research_verification".into(),
                    artifact_sha256: None,
                    event_sequence: None,
                    detail: "No research source was fetched because the declared evidence file failed closed before verification.".into(),
                },
                true,
            ));
        }
    };
    let Some(evidence_json) = read.stdout else {
        return Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "Private-workspace research evidence was not valid UTF-8 JSON.".into(),
            },
            EvaluationEvidence {
                method: "isolated_research_verification".into(),
                artifact_sha256: read.artifacts.first().map(|artifact| artifact.sha256.clone()),
                event_sequence: None,
                detail: "The bounded evidence artifact was retained, but no source was fetched because it could not be parsed.".into(),
            },
            true,
        ));
    };
    let evidence_artifact = read.artifacts.first().cloned().ok_or_else(|| {
        StorageError::InvalidData(
            "isolated research evidence read omitted its immutable artifact".into(),
        )
    })?;
    let Ok(document) = serde_json::from_str::<ResearchEvidenceDocument>(&evidence_json) else {
        return Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "Private-workspace research evidence was not valid schema-bound JSON."
                    .into(),
            },
            EvaluationEvidence {
                method: "isolated_research_verification".into(),
                artifact_sha256: Some(evidence_artifact.sha256),
                event_sequence: None,
                detail: "The bounded evidence artifact was retained, but no source was fetched because JSON decoding failed.".into(),
            },
            true,
        ));
    };
    let candidate_urls = document
        .claims
        .iter()
        .flat_map(|claim| &claim.citations)
        .map(|citation| citation.url.clone())
        .collect::<BTreeSet<_>>();
    if candidate_urls != *baseline_urls {
        return Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "The candidate changed the exact source URL set bound by the immutable baseline evaluation.".into(),
            },
            EvaluationEvidence {
                method: "isolated_research_verification".into(),
                artifact_sha256: Some(evidence_artifact.sha256),
                event_sequence: None,
                detail: "No source was fetched because automatic shadow assessment never expands or changes baseline network authority.".into(),
            },
            true,
        ));
    }
    let mut request = ToolRequest::new(
        task_id,
        "verify isolated shadow research sources",
        ToolRequestKind::ResearchVerify {
            evidence_path: absolute,
            evidence_artifact: evidence_artifact.clone(),
            evidence_json,
            document,
            required_scope: required_scope.clone(),
            minimum_sources: *minimum,
            max_age_days: *max_age_days,
            max_response_bytes: MAX_EVIDENCE_BYTES,
        },
    );
    request.timeout_seconds = 120;
    let Ok(outcome) = research.execute(ToolExecutionId::new(), &request).await else {
        return Ok((
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "Credential-free research-source verification failed closed before producing a complete outcome.".into(),
            },
            EvaluationEvidence {
                method: "isolated_research_verification".into(),
                artifact_sha256: Some(evidence_artifact.sha256),
                event_sequence: None,
                detail: "The immutable evidence file was retained; source fetching or validation did not complete, so the candidate was not accepted.".into(),
            },
            true,
        ));
    };
    let passed = outcome.status == ToolOutcomeStatus::Succeeded;
    let source_count = outcome
        .verification
        .iter()
        .filter(|item| item.method == "research.source" && item.passed)
        .count();
    Ok((
        ConditionResult {
            condition: condition.clone(),
            passed,
            detail: objective_condition_outcome_detail(condition, &outcome),
        },
        EvaluationEvidence {
            method: "isolated_research_verification".into(),
            artifact_sha256: Some(evidence_artifact.sha256),
            event_sequence: None,
            detail: if passed {
                format!(
                    "The immutable private-workspace report passed independent scope, structure, freshness, conflict, and quote-grounding checks against {source_count} credential-free sources."
                )
            } else {
                "The immutable private-workspace report or one of its declared sources failed independent research verification.".into()
            },
        },
        true,
    ))
}

async fn verify_isolated_command_condition(
    verifier: &dyn ShadowCommandVerifier,
    artifacts: &ArtifactStore,
    workspace_root: &Path,
    condition: &SuccessCondition,
) -> (ConditionResult, EvaluationEvidence, bool) {
    if objective_command(condition).is_none() {
        return (
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "The isolated command verifier rejected a non-command condition.".into(),
            },
            EvaluationEvidence {
                method: "isolated_command_sandbox".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "No command was executed because the condition kind was invalid.".into(),
            },
            false,
        );
    }
    match verifier.verify(workspace_root, condition, artifacts).await {
        ShadowCommandVerification::Unavailable => (
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "No qualified operating-system command sandbox is available; the condition remains inconclusive.".into(),
            },
            EvaluationEvidence {
                method: "isolated_command_sandbox".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "The verifier failed closed without launching a host process. A working-directory boundary alone is not accepted as isolation.".into(),
            },
            false,
        ),
        ShadowCommandVerification::Completed { transcript } => {
            let parsed = artifacts
                .read_verified_reference(&transcript, SHADOW_MAX_COMMAND_TRANSCRIPT_BYTES)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ShadowCommandTranscript>(&bytes).ok())
                .filter(|record| {
                    record.schema_version == SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION
                        && record.condition == *condition
                        && valid_optional_artifact(artifacts, record.stdout.as_ref())
                        && valid_optional_artifact(artifacts, record.stderr.as_ref())
                });
            let Some(record) = parsed else {
                return (
                    ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: "The command sandbox returned invalid immutable evidence; the condition remains inconclusive.".into(),
                    },
                    EvaluationEvidence {
                        method: "isolated_command_sandbox".into(),
                        artifact_sha256: None,
                        event_sequence: None,
                        detail: "Malformed command-transcript evidence was rejected.".into(),
                    },
                    false,
                );
            };
            let condition_passed = !record.timed_out && record.exit_code == Some(0);
            (
                ConditionResult {
                    condition: condition.clone(),
                    passed: condition_passed,
                    detail: if condition_passed {
                        "The command passed inside a qualified operating-system shadow sandbox."
                    } else {
                        "The command failed inside a qualified operating-system shadow sandbox."
                    }
                    .into(),
                },
                EvaluationEvidence {
                    method: "isolated_command_sandbox".into(),
                    artifact_sha256: Some(transcript.sha256),
                    event_sequence: None,
                    detail: "A bounded command transcript was captured without granting the candidate host process, credential, or network authority.".into(),
                },
                true,
            )
        }
    }
}

#[allow(clippy::too_many_lines)] // Keep fail-closed service evidence validation auditable in one boundary.
async fn verify_isolated_service_condition(
    verifier: &dyn ShadowCommandVerifier,
    artifacts: &ArtifactStore,
    workspace_root: &Path,
    service_command: Option<&str>,
    condition: &SuccessCondition,
) -> (ConditionResult, EvaluationEvidence, bool) {
    if !matches!(
        condition,
        SuccessCondition::HttpResponds { .. }
            | SuccessCondition::ServiceHealthy { .. }
            | SuccessCondition::PortListening { .. }
            | SuccessCondition::PageRenders { .. }
            | SuccessCondition::Accessibility { .. }
            | SuccessCondition::ElementExists { .. }
            | SuccessCondition::BrowserWorkflow { .. }
            | SuccessCondition::ScreenshotMatches { .. }
    ) {
        return (
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "The isolated service verifier rejected a non-service condition.".into(),
            },
            EvaluationEvidence {
                method: "isolated_service_sandbox".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "No service was started because the condition kind was invalid.".into(),
            },
            false,
        );
    }
    let Some(service_command) = service_command else {
        return (
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "No unique immutable baseline start or dev command authorizes this service probe; the condition remains inconclusive.".into(),
            },
            EvaluationEvidence {
                method: "isolated_service_sandbox".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "The verifier failed closed before launching a process because baseline project authority was missing or ambiguous.".into(),
            },
            false,
        );
    };
    match verifier
        .verify_service(workspace_root, service_command, condition, artifacts)
        .await
    {
        ShadowCommandVerification::Unavailable => (
            ConditionResult {
                condition: condition.clone(),
                passed: false,
                detail: "No qualified operating-system service sandbox is available; the condition remains inconclusive.".into(),
            },
            EvaluationEvidence {
                method: "isolated_service_sandbox".into(),
                artifact_sha256: None,
                event_sequence: None,
                detail: "The verifier failed closed without starting a host service or probing through host networking.".into(),
            },
            false,
        ),
        ShadowCommandVerification::Completed { transcript } => {
            let parsed_transcript = artifacts
                .read_verified_reference(&transcript, SHADOW_MAX_COMMAND_TRANSCRIPT_BYTES)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ShadowCommandTranscript>(&bytes).ok())
                .filter(|record| {
                    record.schema_version == SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION
                        && record.condition == *condition
                        && valid_optional_artifact(artifacts, record.stdout.as_ref())
                        && valid_optional_artifact(artifacts, record.stderr.as_ref())
                });
            let Some(record) = parsed_transcript else {
                return (
                    ConditionResult {
                        condition: condition.clone(),
                        passed: false,
                        detail: "The service sandbox returned invalid immutable evidence; the condition remains inconclusive.".into(),
                    },
                    EvaluationEvidence {
                        method: "isolated_service_sandbox".into(),
                        artifact_sha256: None,
                        event_sequence: None,
                        detail: "Malformed service-transcript evidence was rejected.".into(),
                    },
                    false,
                );
            };
            let condition_passed = !record.timed_out && record.exit_code == Some(0);
            (
                ConditionResult {
                    condition: condition.clone(),
                    passed: condition_passed,
                    detail: if condition_passed {
                        "The baseline-bound service passed its loopback HTTP/TCP/browser probe inside one qualified operating-system shadow sandbox."
                    } else {
                        "The baseline-bound service failed its loopback HTTP/TCP/browser probe inside one qualified operating-system shadow sandbox."
                    }
                    .into(),
                },
                EvaluationEvidence {
                    method: "isolated_service_sandbox".into(),
                    artifact_sha256: Some(transcript.sha256),
                    event_sequence: None,
                    detail: "Service startup and the bounded loopback HTTP/TCP/browser probe shared the same network-isolated sandbox; no host-network fallback was permitted.".into(),
                },
                true,
            )
        }
    }
}

fn valid_optional_artifact(
    artifacts: &ArtifactStore,
    artifact: Option<&mcg_tools::ArtifactReference>,
) -> bool {
    artifact.is_none_or(|artifact| {
        artifacts
            .verify_reference(artifact, SHADOW_MAX_COMMAND_TRANSCRIPT_BYTES)
            .is_ok()
    })
}

fn isolated_evaluation_outcome(
    conditions: &[ConditionResult],
    condition_support: &[bool],
) -> EvaluationOutcome {
    if conditions.is_empty()
        || conditions.len() != condition_support.len()
        || condition_support.iter().any(|supported| !supported)
    {
        EvaluationOutcome::Inconclusive
    } else if conditions.iter().all(|condition| condition.passed) {
        EvaluationOutcome::Passed
    } else {
        EvaluationOutcome::Failed
    }
}

fn isolated_evaluation_regressions(
    conditions: &[ConditionResult],
    condition_support: &[bool],
) -> Vec<String> {
    conditions
        .iter()
        .enumerate()
        .filter(|(_, condition)| !condition.passed)
        .map(|(index, condition)| {
            let supported = condition_support.get(index).copied().unwrap_or(false);
            if !supported {
                "candidate condition lacks an isolated verifier".into()
            } else if objective_command(&condition.condition).is_some() {
                "candidate failed an isolated command objective".into()
            } else if matches!(
                condition.condition,
                SuccessCondition::ResearchSources { .. }
            ) {
                "candidate failed isolated research-source verification".into()
            } else if matches!(
                condition.condition,
                SuccessCondition::HttpResponds { .. }
                    | SuccessCondition::ServiceHealthy { .. }
                    | SuccessCondition::PortListening { .. }
                    | SuccessCondition::PageRenders { .. }
                    | SuccessCondition::Accessibility { .. }
                    | SuccessCondition::ElementExists { .. }
                    | SuccessCondition::BrowserWorkflow { .. }
                    | SuccessCondition::ScreenshotMatches { .. }
            ) {
                "candidate failed an isolated service objective".into()
            } else {
                "candidate failed an isolated file objective".into()
            }
        })
        .collect()
}

fn objective_outcome_passed(condition: &SuccessCondition, outcome: &ToolOutcome) -> bool {
    if let SuccessCondition::HttpResponds { status, .. } = condition {
        return observed_http_status(outcome) == Some(*status);
    }
    if matches!(condition, SuccessCondition::ServiceHealthy { .. }) {
        return observed_http_status(outcome).is_some_and(|status| (200..300).contains(&status));
    }
    if matches!(condition, SuccessCondition::PortListening { .. }) {
        return outcome.status == ToolOutcomeStatus::Succeeded
            && outcome
                .verification
                .iter()
                .any(|evidence| evidence.method == "tcp.connect" && evidence.passed);
    }
    if let SuccessCondition::FileExists {
        min_bytes,
        expected_sha256,
        ..
    } = condition
    {
        return outcome.status == ToolOutcomeStatus::Succeeded
            && outcome.artifacts.first().is_some_and(|artifact| {
                artifact.size_bytes >= *min_bytes
                    && expected_sha256
                        .as_ref()
                        .is_none_or(|expected| artifact.sha256.eq_ignore_ascii_case(expected))
            });
    }
    if matches!(condition, SuccessCondition::ScreenshotMatches { .. }) {
        return outcome.status == ToolOutcomeStatus::Succeeded
            && outcome.verification.iter().any(|evidence| {
                evidence.method == "browser.screenshot_pixel_diff" && evidence.passed
            });
    }
    if matches!(condition, SuccessCondition::Accessibility { .. }) {
        return outcome.status == ToolOutcomeStatus::Succeeded
            && outcome
                .verification
                .iter()
                .any(|evidence| evidence.method == "browser.accessibility" && evidence.passed);
    }
    outcome.status == ToolOutcomeStatus::Succeeded
}

fn objective_evidence_artifact<'a>(
    condition: &SuccessCondition,
    outcome: &'a ToolOutcome,
) -> Option<&'a mcg_tools::ArtifactReference> {
    if matches!(condition, SuccessCondition::BrowserWorkflow { .. }) {
        return outcome.artifacts.get(2);
    }
    if matches!(condition, SuccessCondition::Accessibility { .. }) {
        return outcome.artifacts.get(2);
    }
    if matches!(
        condition,
        SuccessCondition::PageRenders { .. }
            | SuccessCondition::Accessibility { .. }
            | SuccessCondition::ElementExists { .. }
            | SuccessCondition::ScreenshotMatches { .. }
    ) {
        outcome.artifacts.get(1)
    } else {
        outcome.artifacts.first()
    }
}

fn observed_http_status(outcome: &ToolOutcome) -> Option<u16> {
    outcome
        .verification
        .iter()
        .find(|evidence| evidence.method == "http.status")
        .and_then(|evidence| evidence.detail.parse().ok())
}

fn objective_condition_outcome_detail(
    condition: &SuccessCondition,
    outcome: &ToolOutcome,
) -> String {
    if let SuccessCondition::HttpResponds { status, .. } = condition {
        return format!(
            "{}; expected_status={status}; observed_status={}",
            outcome.summary,
            observed_http_status(outcome)
                .map_or_else(|| "unknown".into(), |value| value.to_string())
        );
    }
    if matches!(condition, SuccessCondition::ServiceHealthy { .. }) {
        return format!(
            "{}; expected_status=200-299; observed_status={}",
            outcome.summary,
            observed_http_status(outcome)
                .map_or_else(|| "unknown".into(), |value| value.to_string())
        );
    }
    if matches!(condition, SuccessCondition::PortListening { .. }) {
        let observed = outcome
            .verification
            .iter()
            .any(|evidence| evidence.method == "tcp.connect" && evidence.passed);
        return format!("{}; tcp_connect={observed}", outcome.summary);
    }
    if matches!(
        condition,
        SuccessCondition::PageRenders { .. }
            | SuccessCondition::Accessibility { .. }
            | SuccessCondition::ElementExists { .. }
            | SuccessCondition::BrowserWorkflow { .. }
            | SuccessCondition::ScreenshotMatches { .. }
            | SuccessCondition::ResearchSources { .. }
    ) {
        let verification = outcome
            .verification
            .iter()
            .map(|evidence| {
                format!(
                    "{}={}:{}",
                    evidence.method, evidence.passed, evidence.detail
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return format!("{}; {verification}", outcome.summary);
    }
    if let SuccessCondition::FileExists {
        min_bytes,
        expected_sha256,
        ..
    } = condition
    {
        return outcome.artifacts.first().map_or_else(
            || format!("{}; no file artifact was captured", outcome.summary),
            |artifact| {
                format!(
                    "{}; observed_size={}; minimum_size={min_bytes}; observed_sha256={}; expected_sha256={}",
                    outcome.summary,
                    artifact.size_bytes,
                    artifact.sha256,
                    expected_sha256.as_deref().unwrap_or("any")
                )
            },
        );
    }
    objective_outcome_detail(outcome)
}

fn objective_process_options(
    command: &str,
    project_root: &str,
) -> Result<ProcessOptions, RuntimeError> {
    let mut parts = command.split_ascii_whitespace();
    let executable = parts
        .next()
        .ok_or_else(|| StorageError::InvalidData("objective command has no executable".into()))?;
    Ok(ProcessOptions {
        executable: platform_process_executable(executable),
        argv: parts.map(str::to_owned).collect(),
        cwd: project_root.into(),
        env: Vec::new(),
        stdin: ProcessStream::Null,
        stdin_data: None,
        stdout: ProcessStream::Piped,
        stderr: ProcessStream::Piped,
    })
}

fn platform_process_executable(executable: &str) -> String {
    #[cfg(target_os = "windows")]
    if matches!(
        executable,
        "npm" | "npx" | "pnpm" | "pnpx" | "yarn" | "yarnpkg" | "corepack"
    ) {
        return format!("{executable}.cmd");
    }
    executable.into()
}

fn objective_request_id(
    run_id: RunId,
    index: usize,
    attempt: u16,
    condition: &SuccessCondition,
) -> Result<ToolRequestId, RuntimeError> {
    let mut hash = Sha256::new();
    hash.update(run_id.to_string().as_bytes());
    hash.update([0]);
    hash.update(index.to_le_bytes());
    hash.update([0]);
    hash.update(attempt.to_le_bytes());
    hash.update([0]);
    hash.update(
        serde_json::to_vec(condition)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    );
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Ok(ToolRequestId::from_stable_bytes(bytes))
}

fn objective_stage_request_id(
    run_id: RunId,
    index: usize,
    attempt: u16,
    condition: &SuccessCondition,
    stage: &str,
) -> Result<ToolRequestId, RuntimeError> {
    let mut hash = Sha256::new();
    hash.update(run_id.to_string().as_bytes());
    hash.update(b"\0objective-stage\0");
    hash.update(index.to_le_bytes());
    hash.update([0]);
    hash.update(attempt.to_le_bytes());
    hash.update([0]);
    hash.update(stage.as_bytes());
    hash.update([0]);
    hash.update(
        serde_json::to_vec(condition)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    );
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Ok(ToolRequestId::from_stable_bytes(bytes))
}

fn objective_batch_request_id(
    run_id: RunId,
    attempt: u16,
    conditions: &[SuccessCondition],
) -> Result<ToolRequestId, RuntimeError> {
    let mut hash = Sha256::new();
    hash.update(run_id.to_string().as_bytes());
    hash.update(b"\0guided-evaluation-batch\0");
    hash.update(attempt.to_le_bytes());
    hash.update([0]);
    hash.update(
        serde_json::to_vec(conditions)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?,
    );
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Ok(ToolRequestId::from_stable_bytes(bytes))
}

fn objective_outcome_detail(outcome: &ToolOutcome) -> String {
    let mut detail = format!(
        "{}; exit_code={}",
        outcome.summary,
        outcome
            .exit_code
            .map_or_else(|| "none".into(), |code| code.to_string())
    );
    if let Some(stdout) = &outcome.stdout {
        detail.push_str("; stdout=");
        detail.extend(stdout.chars().take(1_000));
    }
    if let Some(stderr) = &outcome.stderr {
        detail.push_str("; stderr=");
        detail.extend(stderr.chars().take(1_000));
    }
    detail
}

fn save_mcp_credential(
    store: &dyn McpCredentialStore,
    connection: &McpConnection,
    credential: Option<&McpCredential>,
) -> Result<(), McpError> {
    match (connection.auth_mode, credential) {
        (McpAuthMode::None, None) => Ok(()),
        (McpAuthMode::BearerToken, Some(credential)) => store.save(&connection.id, credential),
        _ => Err(McpError::Configuration(
            "MCP credential shape does not match the explicit authentication mode".into(),
        )),
    }
}

fn load_mcp_credential(
    store: &dyn McpCredentialStore,
    connection: &McpConnection,
) -> Result<Option<McpCredential>, McpError> {
    match connection.auth_mode {
        McpAuthMode::None => Ok(None),
        McpAuthMode::BearerToken => store.load(&connection.id)?.map_or_else(
            || {
                Err(McpError::Authentication(format!(
                    "MCP connection {} has no protected credential",
                    connection.id
                )))
            },
            |credential| Ok(Some(credential)),
        ),
    }
}

#[allow(clippy::needless_pass_by_value)] // `Result::map_err` adapter owns its source error.
fn tool_storage_error(error: StorageError) -> ToolError {
    ToolError::Execution(format!("durable tool control failed: {error}"))
}

fn classify_agent_turn_error(error: &AgentTurnError) -> (AgentTurnDisposition, &'static str) {
    match error {
        AgentTurnError::Tool(ToolError::ApprovalRequired { .. }) => {
            (AgentTurnDisposition::Waiting, "approval_required")
        }
        AgentTurnError::Invalid(_) => (AgentTurnDisposition::Failed, "invalid_turn"),
        AgentTurnError::Checkpoint(_) => (AgentTurnDisposition::Failed, "checkpoint_failure"),
        AgentTurnError::IterationLimit => (AgentTurnDisposition::Failed, "iteration_limit"),
        AgentTurnError::Provider(mcg_providers::ProviderError::Authentication(_)) => {
            (AgentTurnDisposition::Failed, "provider_authentication")
        }
        AgentTurnError::Provider(mcg_providers::ProviderError::RateLimited { .. }) => {
            (AgentTurnDisposition::Failed, "provider_rate_limited")
        }
        AgentTurnError::Provider(_) => (AgentTurnDisposition::Failed, "provider_failure"),
        AgentTurnError::Tool(ToolError::Denied(_)) => (AgentTurnDisposition::Failed, "tool_denied"),
        AgentTurnError::Tool(ToolError::TimedOut) => {
            (AgentTurnDisposition::Failed, "tool_timed_out")
        }
        AgentTurnError::Tool(_) => (AgentTurnDisposition::Failed, "tool_failure"),
    }
}

fn detect_platform_capabilities(
    config: &AppConfig,
    observed_at: DateTime<Utc>,
) -> PlatformCapabilitySnapshot {
    let browser_available = browser_runtime_available(config.browser_executable.as_deref());
    let computer_available = computer_use_runtime_available();
    let ssh_available =
        find_path_executable("ssh").is_some() && find_path_executable("scp").is_some();
    let desktop_supported = matches!(std::env::consts::OS, "windows" | "linux");
    PlatformCapabilitySnapshot {
        schema_version: PLATFORM_SCHEMA_VERSION,
        id: PlatformCapabilitySnapshotId::new(),
        os: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        capabilities: vec![
            PlatformCapability {
                kind: PlatformCapabilityKind::BrowserAutomation,
                availability: if browser_available {
                    PlatformCapabilityAvailability::Available
                } else {
                    PlatformCapabilityAvailability::Unavailable
                },
                detail: if browser_available {
                    "local browser runtime configuration verified".into()
                } else {
                    "no local browser runtime configuration has been verified".into()
                },
            },
            PlatformCapability {
                kind: PlatformCapabilityKind::ComputerUse,
                availability: if computer_available {
                    PlatformCapabilityAvailability::Available
                } else if desktop_supported {
                    PlatformCapabilityAvailability::Unavailable
                } else {
                    PlatformCapabilityAvailability::Unsupported
                },
                detail: if computer_available {
                    "screen capture and bounded pointer/keyboard control verified".into()
                } else if std::env::consts::OS == "linux" {
                    "no supported X11 display is attached; Wayland control is not yet qualified"
                        .into()
                } else {
                    "computer-use capture/input is unavailable on this host".into()
                },
            },
            PlatformCapability {
                kind: PlatformCapabilityKind::NativeDesktop,
                availability: if desktop_supported {
                    PlatformCapabilityAvailability::Available
                } else {
                    PlatformCapabilityAvailability::Unsupported
                },
                detail: if desktop_supported {
                    "native desktop target is supported on this operating system".into()
                } else {
                    "native desktop target is not supported on this operating system".into()
                },
            },
            PlatformCapability {
                kind: PlatformCapabilityKind::SshClient,
                availability: if ssh_available {
                    PlatformCapabilityAvailability::Available
                } else {
                    PlatformCapabilityAvailability::Unavailable
                },
                detail: if ssh_available {
                    "OpenSSH command and transfer clients verified".into()
                } else {
                    "OpenSSH ssh/scp executables were not found".into()
                },
            },
            PlatformCapability {
                kind: PlatformCapabilityKind::Sites,
                availability: PlatformCapabilityAvailability::Available,
                detail: "durable Sites registry, deployment, and monitoring are available".into(),
            },
        ],
        observed_at,
    }
}

fn find_path_executable(name: &str) -> Option<std::path::PathBuf> {
    if cfg!(windows)
        && matches!(name, "ssh" | "scp")
        && let Some(system_root) = std::env::var_os("SystemRoot")
    {
        let native = Path::new(&system_root)
            .join("System32")
            .join("OpenSSH")
            .join(format!("{name}.exe"));
        if let Ok(native) = native.canonicalize()
            && native.is_file()
        {
            return Some(native);
        }
    }
    let path = std::env::var_os("PATH")?;
    let candidates = if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_owned()]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(&path)
        .flat_map(|directory| {
            candidates
                .iter()
                .map(move |candidate| directory.join(candidate))
        })
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path, mode: u32) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn restrict_to_owner(_path: &Path, _mode: u32) -> Result<(), std::io::Error> {
    Ok(())
}

fn finish_online_snapshot<T>(
    snapshot: &Path,
    result: Result<T, StorageError>,
) -> Result<T, StorageError> {
    let cleanup = remove_online_snapshot_files(snapshot);
    match result {
        Ok(value) => {
            cleanup?;
            Ok(value)
        }
        Err(error) => {
            let _ = cleanup;
            Err(error)
        }
    }
}

fn remove_online_snapshot_files(snapshot: &Path) -> Result<(), StorageError> {
    let mut wal = snapshot.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = snapshot.as_os_str().to_os_string();
    shm.push("-shm");
    for path in [
        snapshot.to_path_buf(),
        PathBuf::from(wal),
        PathBuf::from(shm),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;
    use axum::{
        Json, Router,
        response::Html,
        routing::{get, post},
    };
    use ed25519_dalek::{Signer as _, SigningKey};
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentTurnRequest, ExecutivePlanner, LeaseBudget, PlannedWork,
        PlanningRequest, RoleKind, SuccessCondition, TaskComplexity,
    };
    use mcg_config::TrustedPluginSignerConfig;
    use mcg_domain::{AgentInstanceId, AgentLeaseId, EvaluationId, RunId, TaskBudget};
    use mcg_plugin_trust::{
        PLUGIN_SIGNATURE_SCHEME, PLUGIN_TRUST_SCHEMA_VERSION, plugin_signing_payload,
    };
    use mcg_policy::{CapabilityPolicy, OwnerConstitution};
    use mcg_projects::{ContextAssembly, ContextCategory, ContextItem};
    use mcg_providers::{
        CONTRACT_VERSION, ContentPart, MessageRole, ModelCapabilities, ModelDescriptor,
        ModelResponse, OutputItem, ProviderAuthMode, ProviderError, ProviderKind, ProviderModelRef,
        RouteCandidate, RoutingPolicy, ToolDefinition, Usage,
    };
    use mcg_tools::{
        BrokerControls, OutputRedactor, RESEARCH_EVIDENCE_SCHEMA_VERSION, ResearchCitationEvidence,
        ResearchClaimEvidence, TOOL_SCHEMA_VERSION, ToolBroker, ToolExecutor, ToolOutcome,
        ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
    };

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn runtime_online_backup_cleanup_removes_snapshot_wal_and_shm_on_every_outcome() {
        let directory = tempfile::tempdir().expect("snapshot directory");
        let snapshot = directory.path().join(".mcgb-runtime.sqlite3");
        let paths = [
            snapshot.clone(),
            PathBuf::from(format!("{}-wal", snapshot.display())),
            PathBuf::from(format!("{}-shm", snapshot.display())),
        ];
        let create_snapshot_files = || {
            for path in &paths {
                fs::write(path, b"snapshot fixture").expect("snapshot fixture");
            }
        };

        create_snapshot_files();
        assert_eq!(
            finish_online_snapshot(&snapshot, Ok::<_, StorageError>(7))
                .expect("successful cleanup"),
            7
        );
        assert!(paths.iter().all(|path| !path.exists()));

        create_snapshot_files();
        let error = finish_online_snapshot::<()>(
            &snapshot,
            Err(StorageError::InvalidData("fixture failure".into())),
        )
        .expect_err("primary failure must survive cleanup");
        assert!(error.to_string().contains("fixture failure"));
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[test]
    fn automatic_memory_consolidation_is_bounded_restart_safe_and_idempotent() {
        let data = tempfile::tempdir().expect("runtime data");
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]")
            .expect("project manifest");
        let config = AppConfig::default().with_data_directory(data.path());
        let project_id = ProjectId::new();
        let baseline = Utc::now();
        let working_id = {
            let runtime = AppRuntime::open(config.clone()).expect("runtime");
            runtime
                .discover_project(project_root.path(), project_id)
                .expect("project");
            let created = runtime
                .create_memory_claim(
                    project_id,
                    "project",
                    "deployment_target",
                    serde_json::json!("staging"),
                    0.9,
                    ClaimProvenance {
                        source_kind: ProvenanceKind::User,
                        source: "owner fixture".into(),
                        task_id: None,
                        artifact_sha256: None,
                    },
                    Some(baseline + Duration::hours(1)),
                )
                .expect("memory");
            let working = runtime
                .transition_memory_claim(
                    project_id,
                    created.claim.id,
                    MemoryLifecycleAction::Reinforce,
                    ClaimProvenance {
                        source_kind: ProvenanceKind::ToolObservation,
                        source: "successful deployment".into(),
                        task_id: None,
                        artifact_sha256: None,
                    },
                )
                .expect("reinforcement");
            assert!(
                runtime
                    .managed_memory(project_id)
                    .expect("managed memory")
                    .contains("staging")
            );

            let first = runtime
                .consolidate_memory_at(baseline + Duration::hours(2), 8)
                .expect("first consolidation");
            assert_eq!((first.examined, first.decayed, first.retracted), (1, 1, 0));
            assert_eq!(first.mutations[0].claim.state, MemoryState::Decaying);
            assert!(
                !runtime
                    .managed_memory(project_id)
                    .expect("managed memory")
                    .contains("staging")
            );
            working.claim.id
        };

        let reopened = AppRuntime::open(config).expect("reopened runtime");
        let second = reopened
            .consolidate_memory_at(baseline + Duration::days(8) + Duration::hours(3), 8)
            .expect("second consolidation");
        assert_eq!(
            (second.examined, second.decayed, second.retracted),
            (1, 0, 1)
        );
        assert_eq!(second.mutations[0].claim.state, MemoryState::Retracted);
        let replay = reopened
            .consolidate_memory_at(baseline + Duration::days(9), 8)
            .expect("idempotent consolidation");
        assert_eq!(
            (replay.examined, replay.decayed, replay.retracted),
            (0, 0, 0)
        );
        let history = reopened
            .memory_claim_history(project_id, "project", "deployment_target")
            .expect("history");
        assert_eq!(history.len(), 4);
        assert!(history.iter().any(|claim| claim.id == working_id));
        assert_eq!(
            history.last().map(|claim| claim.state),
            Some(MemoryState::Retracted)
        );
    }

    struct RejectingProviderCredentialStore(AtomicUsize);

    impl ProviderCredentialStore for RejectingProviderCredentialStore {
        fn load(
            &self,
            _connection_id: &str,
            _slot: ProviderSecretSlot,
        ) -> Result<Option<ProviderCredential>, ProviderError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ProviderError::Authentication(
                "credential store must not be touched".into(),
            ))
        }

        fn save(
            &self,
            _connection_id: &str,
            _slot: ProviderSecretSlot,
            _credential: &ProviderCredential,
        ) -> Result<(), ProviderError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ProviderError::Authentication(
                "credential store must not be touched".into(),
            ))
        }

        fn delete(
            &self,
            _connection_id: &str,
            _slot: ProviderSecretSlot,
        ) -> Result<(), ProviderError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(ProviderError::Authentication(
                "credential store must not be touched".into(),
            ))
        }
    }

    #[test]
    fn anonymous_provider_disconnect_does_not_touch_the_os_credential_store() {
        let store = RejectingProviderCredentialStore(AtomicUsize::new(0));
        let connection = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "anonymous-local".into(),
            display_name: "Anonymous local".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://127.0.0.1:1234/v1".into(),
            auth_mode: ProviderAuthMode::None,
            organization: None,
            project: None,
        };

        AppRuntime::clear_connection_credentials(&store, &connection)
            .expect("anonymous provider has no credentials to clear");
        assert_eq!(store.0.load(Ordering::SeqCst), 0);

        let authenticated = ProviderConnection {
            auth_mode: ProviderAuthMode::ApiKey,
            ..connection
        };
        assert!(AppRuntime::clear_connection_credentials(&store, &authenticated).is_err());
        assert_eq!(store.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn configuration_validation_retains_typed_command_and_evidence_semantics() {
        let condition = SuccessCondition::ConfigurationValid {
            command: "npm run check:config".into(),
        };
        assert_eq!(objective_command(&condition), Some("npm run check:config"));
        assert_eq!(
            objective_evidence_method(&condition),
            "governed_configuration_validation"
        );
    }

    #[test]
    fn portable_organization_import_is_restart_safe_idempotent_and_comparable() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let source = OrganizationGenome::consumer_default(Utc::now());
        let import_id = OrganizationImportId::new();
        let runtime = AppRuntime::open(config.clone()).expect("runtime");
        runtime
            .record_organization_revision(&source)
            .expect("source organization");
        let portable = runtime
            .export_organization(source.revision_id)
            .expect("portable export");
        let imported = runtime
            .import_organization(import_id, &portable, None)
            .expect("portable import");
        assert!(!imported.replayed);
        assert!(imported.event.is_some());
        assert_ne!(
            imported.organization.organization_id,
            source.organization_id
        );
        assert!(
            runtime
                .compare_organizations(source.revision_id, imported.organization.revision_id)
                .expect("comparison")
                .equivalent
        );
        let event_count = runtime
            .store()
            .expect("store")
            .event_count()
            .expect("events");

        drop(runtime);
        let reopened = AppRuntime::open(config).expect("reopened runtime");
        let replay = reopened
            .import_organization(import_id, &portable, None)
            .expect("replayed import");
        assert!(replay.replayed);
        assert!(replay.event.is_none());
        assert_eq!(replay.organization, imported.organization);
        assert_eq!(
            reopened
                .store()
                .expect("store")
                .event_count()
                .expect("events"),
            event_count
        );

        let mut changed_source = source;
        changed_source.name = "Conflicting import".into();
        let changed = PortableOrganizationGenome::export(&changed_source, Utc::now())
            .expect("changed portable");
        assert!(matches!(
            reopened.import_organization(import_id, &changed, None),
            Err(RuntimeError::Storage(StorageError::Conflict(_)))
        ));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One fixture proves discovery, routing, durable planning, and evaluator context together.
    async fn evaluator_turn_receives_bounded_readable_project_topology() {
        let project_root = tempfile::tempdir().expect("project");
        std::fs::write(
            project_root.path().join("compose.yaml"),
            "services:\n  api:\n    image: example/api:1\n    depends_on:\n      - db\n  db:\n    image: postgres:17\n",
        )
        .expect("compose");
        let data = tempfile::tempdir().expect("data");
        let runtime = AppRuntime::open_in_memory(
            AppConfig::default().with_data_directory(data.path()),
            TEST_TOKEN.to_owned(),
        )
        .expect("runtime");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover project");

        let provider = Router::new().route(
            "/v1/models",
            get(|| async {
                Json(serde_json::json!({
                    "object": "list",
                    "data": [{
                        "id": "topology-evaluator",
                        "object": "model",
                        "owned_by": "fixture",
                        "context_window": 8_192,
                        "supported_parameters": ["tools"]
                    }]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("provider listener");
        let address = listener.local_addr().expect("provider address");
        let server = tokio::spawn(async move {
            axum::serve(listener, provider).await.expect("provider");
        });
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "topology_fixture".into(),
                    display_name: "Topology fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: format!("http://{address}/v1"),
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("configure provider");

        let mut task = TaskEnvelope::new("evaluate the service topology");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("task");
        let now = Utc::now();
        let genome = OrganizationGenome::consumer_default(now);
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "evaluate declared dependencies",
                RoleKind::Evaluator,
                vec![SuccessCondition::Custom {
                    description: "dependency topology is consistent".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &planning, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        runtime
            .launch_task_agent_run(task.id, &genome, &run, &plan)
            .expect("launch");
        let prepared = runtime
            .prepare_agent_turn(&AgentDispatchSpec {
                run_id: run.id,
                work_item_id: plan.items[0].id,
                project_id,
                target_path: None,
                context_token_budget: 96,
                requested_tools: Vec::new(),
                estimated_output_tokens: 128,
                max_model_calls: 1,
            })
            .await
            .expect("prepare evaluator");
        assert_eq!(prepared.request.role.kind, RoleKind::Evaluator);
        assert!(prepared.request.context.estimated_tokens <= 96);
        assert!(prepared.request.context.items.iter().any(|item| {
            item.content == "World relationship: api -> uses database -> db (PostgreSQL)"
                && item.source == "compose.yaml#services.api.depends_on"
        }));
        assert!(prepared.request.context.items.iter().all(|item| {
            !item.content.starts_with("World relationship:")
                || (!item.content.contains("service:") && !item.content.contains("database:"))
        }));
        server.abort();
    }

    #[test]
    fn model_tool_paths_resolve_inside_the_granted_project_root() {
        let project = tempfile::tempdir().expect("project");
        let root = project.path().canonicalize().expect("canonical project");

        let mut list = ToolRequestKind::FsList {
            path: ".".into(),
            recursive: false,
            max_entries: 32,
        };
        resolve_project_relative_tool_paths(&mut list, &root);
        let ToolRequestKind::FsList { path, .. } = list else {
            unreachable!();
        };
        assert_eq!(PathBuf::from(path), root);

        let mut read = ToolRequestKind::FsRead {
            path: "src/./index.html".into(),
            max_bytes: 1024,
        };
        resolve_project_relative_tool_paths(&mut read, &root);
        let ToolRequestKind::FsRead { path, .. } = read else {
            unreachable!();
        };
        assert_eq!(PathBuf::from(path), root.join("src").join("index.html"));

        let mut traversal = ToolRequestKind::FsRead {
            path: "../secret".into(),
            max_bytes: 1024,
        };
        resolve_project_relative_tool_paths(&mut traversal, &root);
        let ToolRequestKind::FsRead { path, .. } = traversal else {
            unreachable!();
        };
        assert_eq!(path, "../secret");

        let outside = root.parent().expect("project parent").join("outside");
        let mut absolute = ToolRequestKind::FsRead {
            path: outside.to_string_lossy().into_owned(),
            max_bytes: 1024,
        };
        resolve_project_relative_tool_paths(&mut absolute, &root);
        let ToolRequestKind::FsRead { path, .. } = absolute else {
            unreachable!();
        };
        assert_eq!(PathBuf::from(path), outside);
    }

    #[derive(Clone, Copy)]
    enum FixtureShadowCommandResult {
        Pass,
        Fail,
        MismatchedCondition,
        InvalidOutputEvidence,
        InvalidEvidence,
    }

    struct FixtureShadowCommandVerifier(FixtureShadowCommandResult);

    struct SuccessfulPluginRunner(Arc<AtomicUsize>);

    #[async_trait]
    impl PluginRunnerGateway for SuccessfulPluginRunner {
        async fn invoke(
            &self,
            request: &PluginRunnerRequest,
        ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(PluginRunnerResponse {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: Some(request.manifest.id.clone()),
                state: PluginRunnerState::Succeeded,
                result: Some(PluginResult {
                    schema_version: PLUGIN_SCHEMA_VERSION,
                    plugin_id: request.manifest.id.clone(),
                    output: request.input.clone(),
                }),
                error_code: None,
                detail: None,
            })
        }
    }

    struct TimeoutPluginRunner;

    #[async_trait]
    impl PluginRunnerGateway for TimeoutPluginRunner {
        async fn invoke(
            &self,
            _request: &PluginRunnerRequest,
        ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
            Err(PluginRunnerClientError::Timeout)
        }
    }

    struct ScriptedPluginRunner {
        turns: Mutex<VecDeque<PluginGuestTurn>>,
        inputs: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    #[async_trait]
    impl PluginRunnerGateway for ScriptedPluginRunner {
        async fn invoke(
            &self,
            request: &PluginRunnerRequest,
        ) -> Result<PluginRunnerResponse, PluginRunnerClientError> {
            self.inputs
                .lock()
                .expect("plugin input fixture")
                .push(request.input.clone());
            let turn = self
                .turns
                .lock()
                .expect("plugin turn fixture")
                .pop_front()
                .expect("scripted plugin turn");
            Ok(PluginRunnerResponse {
                schema_version: PLUGIN_SCHEMA_VERSION,
                plugin_id: Some(request.manifest.id.clone()),
                state: PluginRunnerState::Succeeded,
                result: Some(PluginResult {
                    schema_version: PLUGIN_SCHEMA_VERSION,
                    plugin_id: request.manifest.id.clone(),
                    output: serde_json::to_value(turn).expect("serialize plugin guest turn"),
                }),
                error_code: None,
                detail: None,
            })
        }
    }

    #[async_trait]
    impl ShadowCommandVerifier for FixtureShadowCommandVerifier {
        async fn verify(
            &self,
            workspace_root: &Path,
            condition: &SuccessCondition,
            artifacts: &ArtifactStore,
        ) -> ShadowCommandVerification {
            assert!(workspace_root.is_dir());
            assert!(objective_command(condition).is_some());
            if matches!(self.0, FixtureShadowCommandResult::InvalidEvidence) {
                return ShadowCommandVerification::Completed {
                    transcript: mcg_tools::ArtifactReference {
                        id: mcg_domain::ArtifactId::new(),
                        sha256: "not-a-digest".into(),
                        media_type: "application/vnd.mcg.shadow-command+json".into(),
                        size_bytes: 0,
                        relative_path: "outside".into(),
                    },
                };
            }
            let passed = matches!(self.0, FixtureShadowCommandResult::Pass);
            let transcript_bytes = serde_json::to_vec(&ShadowCommandTranscript {
                schema_version: SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION,
                condition: if matches!(self.0, FixtureShadowCommandResult::MismatchedCondition) {
                    SuccessCondition::Build {
                        command: "different command".into(),
                    }
                } else {
                    condition.clone()
                },
                exit_code: Some(i32::from(!passed)),
                timed_out: false,
                stdout: matches!(self.0, FixtureShadowCommandResult::InvalidOutputEvidence).then(
                    || mcg_tools::ArtifactReference {
                        id: mcg_domain::ArtifactId::new(),
                        sha256: "f".repeat(64),
                        media_type: "text/plain".into(),
                        size_bytes: 1,
                        relative_path: format!("ff/{}", "f".repeat(64)),
                    },
                ),
                stderr: None,
            })
            .expect("serialize fixture transcript");
            let transcript = artifacts
                .capture(&transcript_bytes, "application/vnd.mcg.shadow-command+json")
                .expect("capture fixture transcript");
            ShadowCommandVerification::Completed { transcript }
        }

        async fn verify_service(
            &self,
            workspace_root: &Path,
            service_command: &str,
            condition: &SuccessCondition,
            artifacts: &ArtifactStore,
        ) -> ShadowCommandVerification {
            assert!(workspace_root.is_dir());
            assert!(!service_command.trim().is_empty());
            assert!(matches!(
                condition,
                SuccessCondition::HttpResponds { .. }
                    | SuccessCondition::ServiceHealthy { .. }
                    | SuccessCondition::PortListening { .. }
                    | SuccessCondition::PageRenders { .. }
                    | SuccessCondition::Accessibility { .. }
                    | SuccessCondition::ElementExists { .. }
                    | SuccessCondition::BrowserWorkflow { .. }
                    | SuccessCondition::ScreenshotMatches { .. }
            ));
            if matches!(self.0, FixtureShadowCommandResult::InvalidEvidence) {
                return ShadowCommandVerification::Completed {
                    transcript: mcg_tools::ArtifactReference {
                        id: mcg_domain::ArtifactId::new(),
                        sha256: "not-a-digest".into(),
                        media_type: "application/vnd.mcg.shadow-command+json".into(),
                        size_bytes: 0,
                        relative_path: "outside".into(),
                    },
                };
            }
            let passed = matches!(self.0, FixtureShadowCommandResult::Pass);
            let transcript_bytes = serde_json::to_vec(&ShadowCommandTranscript {
                schema_version: SHADOW_COMMAND_TRANSCRIPT_SCHEMA_VERSION,
                condition: if matches!(self.0, FixtureShadowCommandResult::MismatchedCondition) {
                    SuccessCondition::ServiceHealthy {
                        endpoint: "http://127.0.0.1:65535/different".into(),
                    }
                } else {
                    condition.clone()
                },
                exit_code: Some(i32::from(!passed)),
                timed_out: false,
                stdout: matches!(self.0, FixtureShadowCommandResult::InvalidOutputEvidence).then(
                    || mcg_tools::ArtifactReference {
                        id: mcg_domain::ArtifactId::new(),
                        sha256: "f".repeat(64),
                        media_type: "text/plain".into(),
                        size_bytes: 1,
                        relative_path: format!("ff/{}", "f".repeat(64)),
                    },
                ),
                stderr: None,
            })
            .expect("serialize fixture service transcript");
            let transcript = artifacts
                .capture(&transcript_bytes, "application/vnd.mcg.shadow-command+json")
                .expect("capture fixture service transcript");
            ShadowCommandVerification::Completed { transcript }
        }
    }

    #[test]
    fn default_in_memory_runtime_owns_and_removes_an_isolated_data_directory() {
        let consumer_data_directory = AppConfig::default().data_directory;
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("in-memory runtime");
        let isolated = runtime.config().data_directory.clone();
        assert_ne!(isolated, consumer_data_directory);
        assert!(isolated.is_dir());
        drop(runtime);
        assert!(!isolated.exists());
    }

    #[test]
    fn local_auth_verifies_without_exposing_token_in_debug() {
        let auth = LocalAuth::from_token(TEST_TOKEN.to_owned()).expect("valid token");
        assert!(auth.verifies(TEST_TOKEN));
        assert!(!auth.verifies("wrong-token"));
        assert!(!format!("{auth:?}").contains(TEST_TOKEN));
    }

    #[test]
    fn interrupted_plugin_invocation_becomes_uncertain_on_runtime_restart() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let config = AppConfig::default().with_data_directory(directory.path());
        let installed_at = Utc::now();
        let revision = mcg_domain::PluginRevision {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: mcg_domain::PluginRevisionId::new(),
            manifest: mcg_plugin_protocol::PluginManifest {
                schema_version: mcg_plugin_protocol::PLUGIN_SCHEMA_VERSION,
                id: "example.runtime-recovery".into(),
                display_name: "Runtime recovery fixture".into(),
                version: "1.0.0".into(),
                class: mcg_plugin_protocol::PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: "a".repeat(64),
            module_size_bytes: 128,
            source: mcg_domain::PluginInstallSource::LocalImport,
            signature: mcg_domain::PluginSignature {
                state: mcg_domain::PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at,
        };
        let invocation = mcg_domain::PluginInvocationRecord {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: mcg_domain::PluginInvocationId::new(),
            plugin_id: revision.manifest.id.clone(),
            plugin_revision_id: revision.id,
            task_id: None,
            input_sha256: "b".repeat(64),
            state: mcg_domain::PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: installed_at + Duration::seconds(1),
            completed_at: None,
        };
        {
            let runtime = AppRuntime::open(config.clone()).expect("runtime");
            let mut store = runtime.store().expect("store");
            let installation = store
                .install_unsigned_local_plugin_revision(&revision)
                .expect("install plugin");
            store
                .set_plugin_enabled(
                    &revision.manifest.id,
                    installation.revision,
                    true,
                    installed_at,
                )
                .expect("enable plugin");
            store
                .begin_plugin_invocation(&invocation)
                .expect("begin invocation");
        }

        let reopened = AppRuntime::open(config).expect("reopen runtime");

        assert_eq!(reopened.status().recovered_plugin_invocations, 1);
        let recovered = reopened
            .store()
            .expect("store")
            .plugin_invocation(invocation.id)
            .expect("invocation")
            .expect("stored invocation");
        assert_eq!(
            recovered.state,
            mcg_domain::PluginInvocationState::Uncertain
        );
        assert_eq!(recovered.error_code.as_deref(), Some("runner_interrupted"));
    }

    #[test]
    fn signed_registry_install_requires_exact_configured_trust() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let module = br#"(module (func (export "_start")))"#;
        let signing_key = SigningKey::from_bytes(&[23; 32]);
        let mut config = AppConfig::default().with_data_directory(directory.path());
        config.trusted_plugin_signers = vec![TrustedPluginSignerConfig {
            id: "publisher.example".into(),
            public_key_base64: STANDARD.encode(signing_key.verifying_key().to_bytes()),
            registries: vec!["https://plugins.example/".into()],
        }];
        let revision = PluginRevision {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: mcg_domain::PluginRevisionId::new(),
            manifest: mcg_plugin_protocol::PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.signed-runtime".into(),
                display_name: "Signed runtime fixture".into(),
                version: "1.0.0".into(),
                class: mcg_plugin_protocol::PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: mcg_domain::PluginInstallSource::Registry {
                registry: "https://plugins.example/".into(),
                package: "example.signed-runtime".into(),
            },
            signature: mcg_domain::PluginSignature {
                state: mcg_domain::PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let payload =
            plugin_signing_payload(&revision, "publisher.example").expect("signing payload");
        let mut proof = PluginSignatureProof {
            schema_version: PLUGIN_TRUST_SCHEMA_VERSION,
            scheme: PLUGIN_SIGNATURE_SCHEME.into(),
            signer: "publisher.example".into(),
            signature_base64: STANDARD
                .encode(SigningKey::from_bytes(&[29; 32]).sign(&payload).to_bytes()),
        };
        let runtime = AppRuntime::open(config).expect("runtime");
        assert!(matches!(
            runtime.install_verified_sandboxed_plugin(&revision, module, &proof),
            Err(RuntimeError::PluginTrust(
                PluginTrustError::VerificationFailed
            ))
        ));
        assert!(runtime.plugin_installations().expect("catalog").is_empty());
        assert!(!directory.path().join("artifacts").exists());

        proof.signature_base64 = STANDARD.encode(signing_key.sign(&payload).to_bytes());
        let installed = runtime
            .install_verified_sandboxed_plugin(&revision, module, &proof)
            .expect("verified install");
        assert!(!installed.enabled);
        let stored = runtime
            .store()
            .expect("store")
            .plugin_revision(revision.id)
            .expect("revision lookup")
            .expect("stored revision");
        assert_eq!(
            stored.signature.state,
            mcg_domain::PluginSignatureState::Verified
        );
        assert_eq!(
            stored.signature.signer.as_deref(),
            Some("publisher.example")
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Proves install, opt-in, execution, replay, restart, and uncertainty together.
    async fn plugin_runtime_is_content_addressed_opt_in_and_replay_safe() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let config = AppConfig::default().with_data_directory(directory.path());
        let module = br#"(module (func (export "_start")))"#;
        let installed_at = Utc::now();
        let revision = PluginRevision {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: mcg_domain::PluginRevisionId::new(),
            manifest: mcg_plugin_protocol::PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.runtime".into(),
                display_name: "Runtime fixture".into(),
                version: "1.0.0".into(),
                class: mcg_plugin_protocol::PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: mcg_domain::PluginInstallSource::LocalImport,
            signature: mcg_domain::PluginSignature {
                state: mcg_domain::PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at,
        };
        let invocation_id = PluginInvocationId::new();
        let timeout_id = PluginInvocationId::new();
        let calls = Arc::new(AtomicUsize::new(0));
        {
            let mut runtime = AppRuntime::open(config.clone()).expect("runtime");
            runtime.plugin_runner = Some(Arc::new(SuccessfulPluginRunner(calls.clone())));
            let installed = runtime
                .install_sandboxed_plugin(&revision, module)
                .expect("install module");
            assert!(!installed.enabled);
            assert!(matches!(
                runtime
                    .invoke_sandboxed_plugin(
                        invocation_id,
                        &revision.manifest.id,
                        None,
                        serde_json::json!({"message": "hello"}),
                    )
                    .await,
                Err(RuntimeError::Storage(StorageError::Conflict(_)))
            ));
            let enabled = runtime
                .set_plugin_enabled(&revision.manifest.id, installed.revision, true)
                .expect("explicit enable");
            assert!(enabled.enabled);

            let invoked = runtime
                .invoke_sandboxed_plugin(
                    invocation_id,
                    &revision.manifest.id,
                    None,
                    serde_json::json!({"message": "hello"}),
                )
                .await
                .expect("invoke");
            assert_eq!(invoked.invocation.state, PluginInvocationState::Succeeded);
            assert_eq!(
                invoked.result.expect("result").output,
                serde_json::json!({"message": "hello"})
            );
            assert!(!invoked.replayed);
            let replay = runtime
                .invoke_sandboxed_plugin(
                    invocation_id,
                    &revision.manifest.id,
                    None,
                    serde_json::json!({"message": "hello"}),
                )
                .await
                .expect("replay");
            assert!(replay.replayed);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(matches!(
                runtime
                    .invoke_sandboxed_plugin(
                        invocation_id,
                        &revision.manifest.id,
                        None,
                        serde_json::json!({"message": "changed"}),
                    )
                    .await,
                Err(RuntimeError::Storage(StorageError::Conflict(_)))
            ));

            runtime.plugin_runner = Some(Arc::new(TimeoutPluginRunner));
            let uncertain = runtime
                .invoke_sandboxed_plugin(
                    timeout_id,
                    &revision.manifest.id,
                    None,
                    serde_json::json!({"message": "timeout"}),
                )
                .await
                .expect("uncertain outcome");
            assert_eq!(uncertain.invocation.state, PluginInvocationState::Uncertain);
            assert_eq!(
                uncertain.invocation.error_code.as_deref(),
                Some("runner_uncertain")
            );
        }

        let reopened = AppRuntime::open(config).expect("reopen runtime");
        let replay = reopened
            .invoke_sandboxed_plugin(
                invocation_id,
                &revision.manifest.id,
                None,
                serde_json::json!({"message": "hello"}),
            )
            .await
            .expect("restart replay");
        assert!(replay.replayed);
        assert_eq!(
            replay.result.expect("stored result").output,
            serde_json::json!({"message": "hello"})
        );
        let uncertain = reopened
            .invoke_sandboxed_plugin(
                timeout_id,
                &revision.manifest.id,
                None,
                serde_json::json!({"message": "timeout"}),
            )
            .await
            .expect("uncertain replay");
        assert!(uncertain.replayed);
        assert_eq!(uncertain.invocation.state, PluginInvocationState::Uncertain);
        let diagnostics = reopened.diagnostics(Utc::now()).expect("diagnostics");
        assert_eq!(diagnostics.counts.plugins, 1);
        assert_eq!(diagnostics.counts.plugin_invocations, 2);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // End-to-end authority evidence is clearest as one fixture.
    async fn plugin_capability_turn_reads_only_its_task_project_through_the_broker() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("proof.txt"), "brokered proof")
            .expect("project fixture");
        let data = tempfile::tempdir().expect("data directory");
        let config = AppConfig::default().with_data_directory(data.path());
        let mut runtime =
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover project");
        let mut task = TaskEnvelope::new("exercise a brokered plugin read");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("create task");

        let module = br#"(module (func (export "_start")))"#;
        let revision = PluginRevision {
            schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
            id: mcg_domain::PluginRevisionId::new(),
            manifest: mcg_plugin_protocol::PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.brokered-read".into(),
                display_name: "Brokered read fixture".into(),
                version: "1.0.0".into(),
                class: mcg_plugin_protocol::PluginClass::SandboxedWasi,
                requested_capabilities: vec![PluginCapability::FilesystemRead],
            },
            module_sha256: format!("{:x}", Sha256::digest(module)),
            module_size_bytes: u64::try_from(module.len()).expect("module size"),
            source: mcg_domain::PluginInstallSource::LocalImport,
            signature: mcg_domain::PluginSignature {
                state: mcg_domain::PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        let installed = runtime
            .install_sandboxed_plugin(&revision, module)
            .expect("install plugin");
        let granted = runtime
            .set_plugin_capability_grants(
                &revision.manifest.id,
                installed.revision,
                vec![PluginCapabilityGrant {
                    schema_version: mcg_domain::PLUGIN_CATALOG_SCHEMA_VERSION,
                    capability: PluginCapability::FilesystemRead,
                    scopes: vec![PluginCapabilityScope::TaskProject],
                }],
            )
            .expect("grant task project read");
        runtime
            .set_plugin_enabled(&revision.manifest.id, granted.revision, true)
            .expect("enable plugin");

        let inputs = Arc::new(Mutex::new(Vec::new()));
        runtime.plugin_runner = Some(Arc::new(ScriptedPluginRunner {
            turns: Mutex::new(VecDeque::from([
                PluginGuestTurn::CapabilityRequest {
                    request: PluginCapabilityRequest {
                        schema_version: PLUGIN_SCHEMA_VERSION,
                        id: "read-proof".into(),
                        operation: PluginCapabilityOperation::FilesystemRead {
                            path: "proof.txt".into(),
                            max_bytes: 1_024,
                        },
                    },
                },
                PluginGuestTurn::Completed {
                    output: serde_json::json!({"observed": true}),
                },
            ])),
            inputs: inputs.clone(),
        }));
        let mutation = runtime
            .invoke_sandboxed_plugin(
                PluginInvocationId::new(),
                &revision.manifest.id,
                Some(task.id),
                serde_json::json!({"goal": "read proof"}),
            )
            .await
            .expect("invoke brokered plugin");
        assert_eq!(mutation.invocation.state, PluginInvocationState::Succeeded);
        assert_eq!(
            mutation.result.expect("result").output,
            serde_json::json!({"observed": true})
        );
        let inputs = inputs.lock().expect("recorded inputs");
        assert_eq!(inputs.len(), 2);
        let second: PluginHostTurn =
            serde_json::from_value(inputs[1].clone()).expect("second host turn");
        let PluginCapabilityOutcome::Succeeded { outcome, .. } = &second.outcomes[0] else {
            panic!("brokered read must succeed")
        };
        assert!(
            outcome["stdout"]
                .as_str()
                .is_some_and(|stdout| stdout.contains("brokered proof"))
        );
        let audit = runtime.tool_audit(task.id).expect("tool audit");
        assert_eq!(audit.len(), 2);
        assert!(audit.iter().all(|record| record.tool == "fs.read"));
    }

    #[test]
    fn file_objective_requires_minimum_size_and_expected_digest() {
        let outcome = ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id: ToolExecutionId::new(),
            request_id: ToolRequestId::new(),
            status: ToolOutcomeStatus::Succeeded,
            summary: "verified regular file (4 bytes)".into(),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts: vec![mcg_tools::ArtifactReference {
                id: mcg_domain::ArtifactId::new(),
                sha256: "a".repeat(64),
                media_type: "application/octet-stream".into(),
                size_bytes: 4,
                relative_path: "aa/fixture".into(),
            }],
            verification: Vec::new(),
            started_at: Utc::now(),
            finished_at: Utc::now(),
        };
        let condition = SuccessCondition::FileExists {
            path: "dist/result.bin".into(),
            min_bytes: 4,
            expected_sha256: Some("A".repeat(64)),
        };
        assert!(objective_outcome_passed(&condition, &outcome));
        assert!(!objective_outcome_passed(
            &SuccessCondition::FileExists {
                path: "dist/result.bin".into(),
                min_bytes: 5,
                expected_sha256: Some("a".repeat(64)),
            },
            &outcome
        ));
        assert!(!objective_outcome_passed(
            &SuccessCondition::FileExists {
                path: "dist/result.bin".into(),
                min_bytes: 0,
                expected_sha256: Some("b".repeat(64)),
            },
            &outcome
        ));
    }

    #[test]
    fn isolated_condition_outcomes_fail_closed_for_failure_and_unsupported_kinds() {
        let file = SuccessCondition::FileExists {
            path: "dist/result.bin".into(),
            min_bytes: 1,
            expected_sha256: None,
        };
        let passing = vec![ConditionResult {
            condition: file.clone(),
            passed: true,
            detail: "verified".into(),
        }];
        assert_eq!(
            isolated_evaluation_outcome(&passing, &[true]),
            EvaluationOutcome::Passed
        );
        let failing = vec![ConditionResult {
            condition: file,
            passed: false,
            detail: "missing".into(),
        }];
        assert_eq!(
            isolated_evaluation_outcome(&failing, &[true]),
            EvaluationOutcome::Failed
        );
        assert_eq!(
            isolated_evaluation_regressions(&failing, &[true]),
            vec!["candidate failed an isolated file objective"]
        );
        let unsupported = vec![ConditionResult {
            condition: SuccessCondition::ServiceHealthy {
                endpoint: "local-service".into(),
            },
            passed: false,
            detail: "unsupported".into(),
        }];
        assert_eq!(
            isolated_evaluation_outcome(&unsupported, &[false]),
            EvaluationOutcome::Inconclusive
        );
        assert_eq!(
            isolated_evaluation_regressions(&unsupported, &[false]),
            vec!["candidate condition lacks an isolated verifier"]
        );
    }

    #[tokio::test]
    async fn isolated_file_verifier_captures_exact_evidence_and_rejects_unsafe_paths() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifacts = tempfile::tempdir().expect("artifacts");
        std::fs::write(workspace.path().join("proof.bin"), b"proof").expect("proof file");
        let verifier = FilesystemExecutor::open([workspace.path().to_path_buf()], artifacts.path())
            .expect("filesystem verifier");
        let digest = format!("{:x}", Sha256::digest(b"proof"));
        let passing = SuccessCondition::FileExists {
            path: "proof.bin".into(),
            min_bytes: 5,
            expected_sha256: Some(digest.clone()),
        };
        let (result, evidence) =
            verify_isolated_file_condition(&verifier, TaskId::new(), workspace.path(), &passing)
                .await
                .expect("passing verification");
        assert!(result.passed);
        assert_eq!(evidence.artifact_sha256.as_deref(), Some(digest.as_str()));

        let mismatch = SuccessCondition::FileExists {
            path: "proof.bin".into(),
            min_bytes: 5,
            expected_sha256: Some("0".repeat(64)),
        };
        let (result, evidence) =
            verify_isolated_file_condition(&verifier, TaskId::new(), workspace.path(), &mismatch)
                .await
                .expect("mismatch is measured");
        assert!(!result.passed);
        assert_eq!(evidence.artifact_sha256.as_deref(), Some(digest.as_str()));

        let missing = SuccessCondition::FileExists {
            path: "missing.bin".into(),
            min_bytes: 5,
            expected_sha256: None,
        };
        let (result, evidence) =
            verify_isolated_file_condition(&verifier, TaskId::new(), workspace.path(), &missing)
                .await
                .expect("missing file fails closed");
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());

        let traversal = SuccessCondition::FileExists {
            path: "../outside.bin".into(),
            min_bytes: 5,
            expected_sha256: None,
        };
        assert!(
            verify_isolated_file_condition(&verifier, TaskId::new(), workspace.path(), &traversal,)
                .await
                .is_err()
        );

        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret.bin"), b"outside").expect("outside file");
        let link = workspace.path().join("escape.bin");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path().join("secret.bin"), &link).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(outside.path().join("secret.bin"), &link).is_ok();
        if linked {
            let escape = SuccessCondition::FileExists {
                path: "escape.bin".into(),
                min_bytes: 1,
                expected_sha256: None,
            };
            let (result, evidence) =
                verify_isolated_file_condition(&verifier, TaskId::new(), workspace.path(), &escape)
                    .await
                    .expect("symlink escape fails as a condition");
            assert!(!result.passed);
            assert!(evidence.artifact_sha256.is_none());
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One test proves every fetch/no-fetch authorization branch.
    async fn isolated_research_verifier_rebinds_and_grounds_private_workspace_evidence() {
        let fetches = Arc::new(AtomicUsize::new(0));
        let served_fetches = fetches.clone();
        let app = Router::new().route(
            "/source",
            get(move || {
                let served_fetches = served_fetches.clone();
                async move {
                    served_fetches.fetch_add(1, Ordering::SeqCst);
                    "The independently observed result is reproducible."
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("research listener");
        let address = listener.local_addr().expect("research address");
        let server = tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let filesystem = Arc::new(
            FilesystemExecutor::open([workspace.path().to_path_buf()], artifact_root.path())
                .expect("filesystem verifier"),
        );
        let http = Arc::new(
            HttpExecutor::new(
                Arc::new(RejectingCredentialResolver),
                filesystem.clone(),
                artifact_root.path(),
            )
            .expect("HTTP verifier"),
        );
        let research = ResearchExecutor::new(http);
        let mut document = ResearchEvidenceDocument {
            schema_version: RESEARCH_EVIDENCE_SCHEMA_VERSION,
            scope: "compare the reproducibility evidence".into(),
            claims: vec![ResearchClaimEvidence {
                id: "claim-one".into(),
                statement: "the result is reproducible".into(),
                citations: vec![ResearchCitationEvidence {
                    url: format!("http://{address}/source"),
                    quote: "The independently observed result is reproducible.".into(),
                    published_at: Some(Utc::now()),
                }],
            }],
            conflicts: Vec::new(),
        };
        let evidence_path = workspace.path().join("research.json");
        std::fs::write(
            &evidence_path,
            serde_json::to_vec(&document).expect("research JSON"),
        )
        .expect("write research evidence");
        let condition = SuccessCondition::ResearchSources {
            evidence_path: "research.json".into(),
            minimum: 1,
            max_age_days: Some(30),
            required_scope: "compare the reproducibility evidence".into(),
        };
        let baseline_urls = BTreeSet::from([format!("http://{address}/source")]);
        let (result, evidence, supported) = verify_isolated_research_condition(
            filesystem.as_ref(),
            &research,
            TaskId::new(),
            workspace.path(),
            &condition,
            Some(&baseline_urls),
        )
        .await
        .expect("grounded research verification");
        assert!(result.passed);
        assert!(supported);
        assert_eq!(evidence.method, "isolated_research_verification");
        assert!(evidence.artifact_sha256.is_some());
        assert!(evidence.detail.contains("1 credential-free sources"));
        assert_eq!(fetches.load(Ordering::SeqCst), 1);

        document.scope = "a candidate-controlled replacement scope".into();
        std::fs::write(
            &evidence_path,
            serde_json::to_vec(&document).expect("changed research JSON"),
        )
        .expect("rewrite research evidence");
        let (result, evidence, supported) = verify_isolated_research_condition(
            filesystem.as_ref(),
            &research,
            TaskId::new(),
            workspace.path(),
            &condition,
            Some(&baseline_urls),
        )
        .await
        .expect("scope mismatch is a measured failure");
        assert!(!result.passed);
        assert!(supported);
        assert!(evidence.artifact_sha256.is_some());
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert_eq!(
            isolated_evaluation_regressions(&[result], &[true]),
            vec!["candidate failed isolated research-source verification"]
        );

        document.scope = "compare the reproducibility evidence".into();
        document.claims[0].citations[0].url = "http://127.0.0.1:1/private".into();
        std::fs::write(
            &evidence_path,
            serde_json::to_vec(&document).expect("changed URL research JSON"),
        )
        .expect("rewrite changed URL evidence");
        let (result, evidence, supported) = verify_isolated_research_condition(
            filesystem.as_ref(),
            &research,
            TaskId::new(),
            workspace.path(),
            &condition,
            Some(&baseline_urls),
        )
        .await
        .expect("changed source set is rejected without expanded authority");
        assert!(!result.passed);
        assert!(supported);
        assert!(evidence.detail.contains("No source was fetched"));
        assert_eq!(fetches.load(Ordering::SeqCst), 1);

        std::fs::write(&evidence_path, b"not JSON").expect("malformed research evidence");
        let (result, evidence, supported) = verify_isolated_research_condition(
            filesystem.as_ref(),
            &research,
            TaskId::new(),
            workspace.path(),
            &condition,
            Some(&baseline_urls),
        )
        .await
        .expect("malformed evidence fails closed");
        assert!(!result.passed);
        assert!(supported);
        assert!(evidence.artifact_sha256.is_some());

        let (result, evidence, supported) = verify_isolated_research_condition(
            filesystem.as_ref(),
            &research,
            TaskId::new(),
            workspace.path(),
            &condition,
            None,
        )
        .await
        .expect("missing baseline remains inconclusive");
        assert!(!result.passed);
        assert!(!supported);
        assert!(evidence.artifact_sha256.is_none());
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn isolated_command_verification_distinguishes_pass_failure_and_unavailability() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifact store");
        let condition = SuccessCondition::Test {
            command: "cargo test --workspace".into(),
        };

        let (result, evidence, supported) = verify_isolated_command_condition(
            &UnavailableShadowCommandVerifier,
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());
        assert!(result.detail.contains("inconclusive"));

        let (result, evidence, supported) = verify_isolated_command_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::Pass),
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(supported);
        assert!(result.passed);
        assert!(evidence.artifact_sha256.is_some());

        let (result, evidence, supported) = verify_isolated_command_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::MismatchedCondition),
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());

        let (result, evidence, supported) = verify_isolated_command_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::InvalidOutputEvidence),
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());

        let (result, evidence, supported) = verify_isolated_command_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::Fail),
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_some());

        let (result, evidence, supported) = verify_isolated_command_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::InvalidEvidence),
            &artifacts,
            workspace.path(),
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());
    }

    #[test]
    fn baseline_service_command_prefers_one_start_then_one_dev_and_rejects_ambiguity() {
        let root = tempfile::tempdir().expect("project");
        std::fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"start":"node server.js","dev":"node server.js"}}"#,
        )
        .expect("package fixture");
        let mut project =
            ProjectDiscoverer::discover(root.path(), ProjectId::new()).expect("discover project");
        assert_eq!(baseline_service_command(&project), Some("npm run start"));

        project.commands.retain(|command| command.name != "start");
        assert_eq!(baseline_service_command(&project), Some("npm run dev"));
        let duplicate = project
            .commands
            .iter()
            .find(|command| command.name == "dev")
            .expect("dev command")
            .clone();
        project.commands.push(duplicate);
        assert_eq!(baseline_service_command(&project), None);
    }

    #[test]
    fn durable_run_replay_is_deterministic_restart_safe_and_observation_only() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let now = Utc::now();
        let task = TaskEnvelope::new("replay one governed run");
        let organization = OrganizationGenome::consumer_default(now);
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "verify durable replay",
                RoleKind::Evaluator,
                vec![SuccessCondition::Custom {
                    description: "digest remains stable".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&organization, &planning, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: organization.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        let runtime = AppRuntime::open(config.clone()).expect("runtime");
        runtime.create_task(&task).expect("task");
        runtime
            .record_organization_revision(&organization)
            .expect("organization");
        runtime.create_agent_run(&run, &plan).expect("run");
        let first = runtime.replay_agent_run(run.id).expect("first replay");
        let second = runtime.replay_agent_run(run.id).expect("second replay");
        assert_eq!(first, second);
        assert!(first.side_effect_free);
        assert!(first.invariants.iter().all(|invariant| invariant.passed));
        assert_eq!(
            runtime.task_events(task.id).expect("events"),
            first.evidence.events
        );
        assert_eq!(
            runtime.tool_audit(task.id).expect("audit"),
            first.evidence.tool_audit
        );

        let mut tampered = first.evidence.clone();
        tampered.view.plan.task_id = TaskId::new();
        assert!(validate_run_replay_evidence(&tampered).is_err());

        drop(runtime);
        let reopened = AppRuntime::open(config).expect("reopened runtime");
        let after_restart = reopened.replay_agent_run(run.id).expect("restart replay");
        assert_eq!(after_restart, first);
        assert_eq!(
            reopened.task_events(task.id).expect("events"),
            first.evidence.events
        );
        assert_eq!(
            reopened.tool_audit(task.id).expect("audit"),
            first.evidence.tool_audit
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // The durable source-run/snapshot binding is the behavior under test.
    fn shadow_service_command_uses_the_completed_baseline_snapshot_not_latest_discovery() {
        let root = tempfile::tempdir().expect("project");
        std::fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"start":"node server.js"}}"#,
        )
        .expect("package fixture");
        let data = tempfile::tempdir().expect("data");
        let runtime = AppRuntime::open_in_memory(
            AppConfig::default().with_data_directory(data.path()),
            TEST_TOKEN.to_owned(),
        )
        .expect("runtime");
        let project_id = ProjectId::new();
        let baseline_project = runtime
            .discover_project(root.path(), project_id)
            .expect("baseline project");
        assert_eq!(
            baseline_service_command(&baseline_project.project),
            Some("npm run start")
        );

        let mut task = TaskEnvelope::new("qualify the local service");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("task");
        let now = Utc::now();
        let genome = OrganizationGenome::consumer_default(now);
        let condition = SuccessCondition::HttpResponds {
            url: "http://127.0.0.1:43123/health".into(),
            status: 204,
        };
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "qualify service",
                RoleKind::Coder,
                vec![condition.clone()],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &planning, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        runtime
            .launch_task_agent_run(task.id, &genome, &run, &plan)
            .expect("launch");
        let instance = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: plan.items[0].assigned_role,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:service-binding-fixture".into(),
            confidence: ConfidenceEstimate::new(8_000, 0).expect("confidence"),
            created_at: now,
            updated_at: now,
        };
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: Vec::new(),
            reporting_interval_seconds: 30,
            max_retries: 0,
            escalation_conditions: Vec::new(),
            state: AgentLeaseState::Active,
            acquired_at: now,
            expires_at: now + Duration::minutes(5),
            heartbeat_at: now,
        };
        runtime
            .store()
            .expect("store")
            .dispatch_agent_work(&instance, &lease, now)
            .expect("dispatch");
        runtime
            .store()
            .expect("store")
            .finish_agent_lease(lease.id, AgentLeaseState::Completed, 0, 0.0, 1, now)
            .expect("finish lease");
        let result = ConditionResult {
            condition: condition.clone(),
            passed: true,
            detail: "baseline service passed".into(),
        };
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: EvaluationId::new(),
            run_id: run.id,
            task_id: task.id,
            evaluator_instance_id: None,
            conditions: vec![result.clone()],
            outcome: EvaluationOutcome::Passed,
            confidence: ConfidenceEstimate::new(9_000, 1).expect("confidence"),
            regressions: Vec::new(),
            evidence: vec![EvaluationEvidence {
                method: "governed_http_status".into(),
                artifact_sha256: Some("b".repeat(64)),
                event_sequence: None,
                detail: "baseline loopback service evidence".into(),
            }],
            evaluated_at: now,
        };
        let checkpoint = ConsumerEvaluationCheckpoint {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            task_id: task.id,
            run_id: run.id,
            project_snapshot_id: baseline_project.project.id,
            state: ConsumerEvaluationCheckpointState::Running,
            next_condition_index: 1,
            attempt: 0,
            request_id: None,
            batch_approved: false,
            results: vec![result],
            evidence: Vec::new(),
            regressions: Vec::new(),
            objective_failures: 0,
            unsupported: 0,
            executed: 1,
            updated_at: now,
        };
        runtime
            .store()
            .expect("store")
            .save_consumer_evaluation_checkpoint(&checkpoint)
            .expect("checkpoint");
        runtime
            .store()
            .expect("store")
            .settle_consumer_evaluation(&report, &checkpoint)
            .expect("settle");

        std::fs::write(root.path().join("pnpm-lock.yaml"), "lockfileVersion: '9.0'")
            .expect("new lockfile");
        let latest = runtime
            .discover_project(root.path(), project_id)
            .expect("latest project");
        assert_eq!(
            baseline_service_command(&latest.project),
            Some("pnpm run start")
        );
        let candidate = runtime.agent_run_view(run.id).expect("candidate view");
        let receipt = ShadowExecutionReceipt {
            schema_version: EXPERIMENT_SCHEMA_VERSION,
            experiment_id: ExperimentId::new(),
            source_task_id: task.id,
            candidate_run_id: run.id,
            authority: ShadowAuthority::EvaluationOnly,
            output_artifact_sha256: "a".repeat(64),
            completed_at: now,
        };
        assert_eq!(
            runtime
                .baseline_shadow_service_command(&candidate, &receipt, &report, &condition)
                .expect("baseline binding"),
            Some("npm run start".into())
        );
        let mut failed_baseline = report.clone();
        failed_baseline.conditions[0].passed = false;
        assert_eq!(
            runtime
                .baseline_shadow_service_command(
                    &candidate,
                    &receipt,
                    &failed_baseline,
                    &condition,
                )
                .expect("failed baseline"),
            None
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One table-driven test covers every service verifier disposition and condition kind.
    async fn isolated_service_verification_requires_baseline_authority_and_valid_evidence() {
        let workspace = tempfile::tempdir().expect("workspace");
        let artifact_root = tempfile::tempdir().expect("artifacts");
        let artifacts = ArtifactStore::open(artifact_root.path()).expect("artifact store");
        let condition = SuccessCondition::HttpResponds {
            url: "http://127.0.0.1:43123/health".into(),
            status: 204,
        };

        let (result, evidence, supported) = verify_isolated_service_condition(
            &FixtureShadowCommandVerifier(FixtureShadowCommandResult::Pass),
            &artifacts,
            workspace.path(),
            None,
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());

        let (result, evidence, supported) = verify_isolated_service_condition(
            &UnavailableShadowCommandVerifier,
            &artifacts,
            workspace.path(),
            Some("npm run start"),
            &condition,
        )
        .await;
        assert!(!supported);
        assert!(!result.passed);
        assert!(evidence.artifact_sha256.is_none());

        for (fixture, expected_supported, expected_passed, expected_artifact) in [
            (FixtureShadowCommandResult::Pass, true, true, true),
            (FixtureShadowCommandResult::Fail, true, false, true),
            (
                FixtureShadowCommandResult::MismatchedCondition,
                false,
                false,
                false,
            ),
            (
                FixtureShadowCommandResult::InvalidOutputEvidence,
                false,
                false,
                false,
            ),
            (
                FixtureShadowCommandResult::InvalidEvidence,
                false,
                false,
                false,
            ),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &condition,
            )
            .await;
            assert_eq!(supported, expected_supported);
            assert_eq!(result.passed, expected_passed);
            assert_eq!(evidence.artifact_sha256.is_some(), expected_artifact);
        }

        let tcp_condition = SuccessCondition::PortListening {
            endpoint: "tcp://127.0.0.1:43123".into(),
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &tcp_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("HTTP/TCP"));
        }

        let page_condition = SuccessCondition::PageRenders {
            url: "http://127.0.0.1:43123/app".into(),
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &page_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("browser"));
        }

        let element_condition = SuccessCondition::ElementExists {
            url: "http://127.0.0.1:43123/app".into(),
            selector: "main#app[data-state='ready']".into(),
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &element_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("browser"));
        }

        let accessibility_condition = SuccessCondition::Accessibility {
            url: "http://127.0.0.1:43123/app".into(),
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &accessibility_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("browser"));
        }

        let workflow_condition = SuccessCondition::BrowserWorkflow {
            name: "consumer flow".into(),
            path: "tests/browser-flow.json".into(),
            expected_sha256: "a".repeat(64),
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &workflow_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("browser"));
        }

        let screenshot_condition = SuccessCondition::ScreenshotMatches {
            baseline: "tests/baseline.png".into(),
            expected_baseline_sha256: "b".repeat(64),
            url: "http://127.0.0.1:43123/visual".into(),
            max_changed_basis_points: 100,
            per_channel_threshold: 8,
        };
        for (fixture, expected_passed) in [
            (FixtureShadowCommandResult::Pass, true),
            (FixtureShadowCommandResult::Fail, false),
        ] {
            let (result, evidence, supported) = verify_isolated_service_condition(
                &FixtureShadowCommandVerifier(fixture),
                &artifacts,
                workspace.path(),
                Some("npm run start"),
                &screenshot_condition,
            )
            .await;
            assert!(supported);
            assert_eq!(result.passed, expected_passed);
            assert!(evidence.artifact_sha256.is_some());
            assert!(result.detail.contains("browser"));
        }
    }

    #[test]
    fn committed_task_is_immediately_readable() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime should open");
        let task = TaskEnvelope::new("persist through runtime");
        runtime.create_task(&task).expect("task should persist");

        assert_eq!(runtime.task(task.id).expect("task should read"), Some(task));
    }

    #[test]
    fn isolated_candidate_authority_only_resolves_workspace_filesystem_tools() {
        let (tools, external) = AppRuntime::resolve_shadow_tools(RoleKind::Coder, &[])
            .expect("isolated filesystem tool plane");
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["fs.read", "fs.list", "fs.write", "fs.patch", "fs.mkdir"]
        );
        assert!(external.is_empty());
        assert!(
            AppRuntime::resolve_shadow_tools(RoleKind::Coder, &["process.exec".into()]).is_err()
        );
        assert!(
            AppRuntime::resolve_shadow_tools(RoleKind::Coder, &["mcp::connection::tool".into()])
                .is_err()
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One contiguous restart/exactly-once evidence scenario.
    async fn pending_evaluations_are_supervised_once_after_restart_including_empty_findings() {
        let directory = tempfile::tempdir().expect("data directory");
        let config = AppConfig::default().with_data_directory(directory.path());
        let project_root = tempfile::tempdir().expect("project directory");
        std::fs::write(project_root.path().join("package.json"), "{}").expect("manifest");
        let research_app = Router::new().route(
            "/source",
            get(|| async { "The automatic candidate evidence is independently reproducible." }),
        );
        let research_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("research listener");
        let research_address = research_listener.local_addr().expect("research address");
        tokio::spawn(async move {
            axum::serve(research_listener, research_app)
                .await
                .expect("research source");
        });
        let research_document = ResearchEvidenceDocument {
            schema_version: RESEARCH_EVIDENCE_SCHEMA_VERSION,
            scope: "exercise automatic supervision".into(),
            claims: vec![ResearchClaimEvidence {
                id: "reproducible".into(),
                statement: "candidate evidence is reproducible".into(),
                citations: vec![ResearchCitationEvidence {
                    url: format!("http://{research_address}/source"),
                    quote: "The automatic candidate evidence is independently reproducible.".into(),
                    published_at: Some(Utc::now()),
                }],
            }],
            conflicts: Vec::new(),
        };
        let research_bytes =
            serde_json::to_vec(&research_document).expect("research evidence JSON");
        std::fs::write(project_root.path().join("research.json"), &research_bytes)
            .expect("research evidence");
        let baseline_research_artifact =
            ArtifactStore::open(config.data_directory.join("artifacts"))
                .expect("baseline artifact store")
                .capture(&research_bytes, "application/json")
                .expect("baseline research artifact");
        let research_condition = SuccessCondition::ResearchSources {
            evidence_path: "research.json".into(),
            minimum: 1,
            max_age_days: Some(30),
            required_scope: "exercise automatic supervision".into(),
        };
        let project_id = ProjectId::new();
        let now = Utc::now();
        let run_id = RunId::new();
        let baseline_revision_id = OrganizationRevisionId::new();
        let failed_id = EvaluationId::new();
        let passed_id = EvaluationId::new();
        {
            let runtime = AppRuntime::open(config.clone()).expect("runtime");
            runtime
                .discover_project(project_root.path(), project_id)
                .expect("project");
            let mut task = TaskEnvelope::new("exercise automatic supervision");
            task.project_id = Some(project_id);
            runtime.create_task(&task).expect("task");
            let genome = OrganizationGenome::consumer_default(now);
            let mut genome = genome;
            genome.revision_id = baseline_revision_id;
            runtime
                .record_organization_revision(&genome)
                .expect("organization");
            let condition = SuccessCondition::FileExists {
                path: "package.json".into(),
                min_bytes: 2,
                expected_sha256: Some(format!("{:x}", Sha256::digest(b"{}"))),
            };
            let command_condition = SuccessCondition::Test {
                command: "npm test".into(),
            };
            let planning = PlanningRequest {
                run_id,
                task: task.clone(),
                complexity: TaskComplexity::Atomic,
                work: vec![PlannedWork::atomic(
                    "exercise supervision",
                    RoleKind::Coder,
                    vec![
                        condition.clone(),
                        command_condition.clone(),
                        research_condition.clone(),
                    ],
                )],
                require_independent_evaluation: false,
            };
            let plan = ExecutivePlanner::default()
                .plan(&genome, &planning, now)
                .expect("plan");
            let run = AgentRun {
                schema_version: AGENT_RUN_SCHEMA_VERSION,
                id: planning.run_id,
                task_id: task.id,
                organization_revision_id: genome.revision_id,
                state: AgentRunState::Planned,
                metrics: OrganizationalMetrics::default(),
                created_at: now,
                updated_at: now,
                completed_at: None,
            };
            runtime.create_agent_run(&run, &plan).expect("run");
            runtime
                .transition_agent_run(
                    run.id,
                    AgentRunState::Running,
                    OrganizationalMetrics::default(),
                )
                .expect("running");
            let evaluation = |id, outcome, confidence| EvaluationReport {
                schema_version: AGENT_RUN_SCHEMA_VERSION,
                id,
                run_id: run.id,
                task_id: task.id,
                evaluator_instance_id: None,
                conditions: vec![
                    ConditionResult {
                        condition: condition.clone(),
                        passed: outcome == EvaluationOutcome::Passed,
                        detail: "durable fixture evidence".into(),
                    },
                    ConditionResult {
                        condition: command_condition.clone(),
                        passed: outcome == EvaluationOutcome::Passed,
                        detail: "durable command fixture evidence".into(),
                    },
                    ConditionResult {
                        condition: research_condition.clone(),
                        passed: true,
                        detail: "durable research fixture evidence".into(),
                    },
                ],
                outcome,
                confidence: ConfidenceEstimate::new(confidence, 1).expect("confidence"),
                regressions: if outcome == EvaluationOutcome::Failed {
                    vec!["seeded accessibility regression".into()]
                } else {
                    Vec::new()
                },
                evidence: vec![
                    EvaluationEvidence {
                        method: "fixture".into(),
                        artifact_sha256: Some("a".repeat(64)),
                        event_sequence: Some(1),
                        detail: "objective fixture".into(),
                    },
                    EvaluationEvidence {
                        method: "governed_research_sources".into(),
                        artifact_sha256: Some(baseline_research_artifact.sha256.clone()),
                        event_sequence: Some(2),
                        detail: "owner-governed baseline research binding".into(),
                    },
                ],
                evaluated_at: now,
            };
            runtime
                .record_evaluation(&evaluation(failed_id, EvaluationOutcome::Failed, 9_000))
                .expect("failed evaluation");
            runtime
                .record_evaluation(&evaluation(passed_id, EvaluationOutcome::Passed, 9_000))
                .expect("passed evaluation");
            assert!(
                runtime
                    .supervision_observations()
                    .expect("observations")
                    .is_empty()
            );
        }

        let runtime = AppRuntime::open(config.clone()).expect("reopened runtime");
        let mutations = runtime
            .supervise_pending_evaluations(32)
            .expect("supervise backlog");
        assert_eq!(mutations.len(), 2);
        assert!(mutations.iter().all(|mutation| !mutation.replayed));
        assert_eq!(
            mutations
                .iter()
                .map(|mutation| mutation.observations.len())
                .sum::<usize>(),
            3
        );
        let observations = runtime
            .supervision_observations()
            .expect("durable observations");
        assert_eq!(observations.len(), 3);
        assert!(
            observations
                .iter()
                .all(|observation| observation.task_id == observations[0].task_id)
        );
        let slow_observation = observations
            .iter()
            .find(|observation| observation.loop_kind == mcg_agents::SupervisionLoop::Slow)
            .expect("slow observation");
        let baseline = runtime
            .organization_revision(baseline_revision_id)
            .expect("baseline")
            .expect("present");
        let recovered_candidate_id = OrganizationRevisionId::from_stable_bytes(
            automatic_experiment_id_bytes("candidate", slow_observation.id),
        );
        let recovered_experiment_id = ExperimentId::from_stable_bytes(
            automatic_experiment_id_bytes("experiment", slow_observation.id),
        );
        let recovered_candidate = MeasuredCandidateDesigner::design(
            &baseline,
            slow_observation,
            recovered_candidate_id,
            2,
            slow_observation.observed_at,
        )
        .expect("partial candidate");
        runtime
            .record_candidate_revision(&recovered_candidate)
            .expect("candidate committed before simulated crash");
        runtime
            .create_experiment(
                &automatic_experiment_definition(
                    &baseline,
                    slow_observation,
                    recovered_candidate_id,
                    recovered_experiment_id,
                )
                .expect("experiment definition"),
            )
            .expect("draft committed before simulated crash");
        drop(runtime);
        let runtime = AppRuntime::open(config.clone()).expect("reopen partial orchestration");
        let candidate_outcomes = runtime
            .orchestrate_pending_candidates(8)
            .expect("orchestrate slow-loop candidate");
        assert_eq!(candidate_outcomes.len(), 1);
        let candidate_outcome = &candidate_outcomes[0];
        assert_eq!(
            candidate_outcome.disposition,
            AutomaticExperimentDisposition::Ready
        );
        let experiment = runtime
            .experiment(candidate_outcome.experiment_id.expect("experiment id"))
            .expect("experiment")
            .expect("present");
        assert_eq!(experiment.state, ExperimentState::Ready);
        assert_eq!(experiment.baseline_revision_id, baseline_revision_id);
        assert_eq!(
            runtime
                .organization(experiment.organization_id)
                .expect("active organization")
                .expect("present")
                .revision_id,
            baseline_revision_id
        );
        let candidate = runtime
            .organization_revision(
                candidate_outcome
                    .candidate_revision_id
                    .expect("candidate revision"),
            )
            .expect("candidate")
            .expect("present");
        assert!(candidate.evaluation.require_independent_evaluation);
        assert_eq!(candidate.parent_revision_id, Some(baseline_revision_id));
        let shadow_launches = runtime
            .start_ready_shadow_runs(2)
            .expect("start evaluation-only shadow");
        assert_eq!(shadow_launches.len(), 1);
        assert!(!shadow_launches[0].replayed);
        assert_eq!(
            shadow_launches[0].run.organization_revision_id,
            candidate.revision_id
        );
        assert_eq!(
            shadow_launches[0].experiment.state,
            ExperimentState::Running
        );
        assert_ne!(shadow_launches[0].run.id, run_id);
        assert!(
            runtime
                .execute_automatic_shadow_runs(2)
                .await
                .expect("no configured route is a safe no-op")
                .is_empty()
        );
        assert!(
            runtime
                .agent_run_view(shadow_launches[0].run.id)
                .expect("shadow view")
                .leases
                .is_empty()
        );
        assert!(
            runtime
                .orchestrate_pending_candidates(8)
                .expect("candidate marker replay")
                .is_empty()
        );
        runtime
            .transition_agent_run(
                run_id,
                AgentRunState::Failed,
                OrganizationalMetrics {
                    coordination_seconds: 90,
                    ..OrganizationalMetrics::default()
                },
            )
            .expect("run changes after immutable analysis");
        assert!(
            runtime
                .supervise_evaluation(failed_id)
                .expect("replay after run projection changed")
                .replayed
        );
        assert!(
            runtime
                .supervise_pending_evaluations(32)
                .expect("empty backlog")
                .is_empty()
        );
        assert!(
            runtime
                .supervise_evaluation(passed_id)
                .expect("empty-finding replay")
                .replayed
        );
        drop(runtime);

        let mut reopened = AppRuntime::open(config.clone()).expect("second reopen");
        assert_eq!(
            reopened
                .supervision_observations()
                .expect("reopened observations"),
            observations
        );
        assert!(
            reopened
                .supervise_pending_evaluations(32)
                .expect("persisted markers")
                .is_empty()
        );
        assert!(
            reopened
                .orchestrate_pending_candidates(8)
                .expect("persisted candidate outcome")
                .is_empty()
        );
        assert_eq!(
            reopened
                .experiment(candidate_outcome.experiment_id.expect("experiment id"))
                .expect("reopened experiment")
                .expect("present")
                .state,
            ExperimentState::Running
        );
        let resumed_shadow = reopened
            .start_ready_shadow_runs(2)
            .expect("resume interrupted shadow");
        assert_eq!(resumed_shadow.len(), 1);
        assert!(resumed_shadow[0].replayed);
        assert_eq!(resumed_shadow[0].run.state, AgentRunState::Running);
        let calls = Arc::new(AtomicUsize::new(0));
        let fixture_calls = calls.clone();
        let experiment_id = candidate_outcome.experiment_id.expect("experiment id");
        let shadow_manifest = config
            .data_directory
            .join("shadow-workspaces")
            .join(experiment_id.to_string())
            .join("package.json")
            .to_string_lossy()
            .into_owned();
        let provider = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "shadow-model",
                            "object": "model",
                            "owned_by": "fixture",
                            "context_window": 32_768,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route(
                "/v1/responses",
                post(move || {
                    let calls = fixture_calls.clone();
                    let shadow_manifest = shadow_manifest.clone();
                    async move {
                        let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
                        let output = if call % 2 == 1 {
                            serde_json::json!([{
                                "type": "function_call",
                                "call_id": format!("shadow-read-{call}"),
                                "name": "fs.read",
                                "arguments": serde_json::json!({
                                    "path": shadow_manifest,
                                    "max_bytes": 128
                                }).to_string(),
                                "status": "completed"
                            }])
                        } else {
                            serde_json::json!([{
                                "type": "message",
                                "role": "assistant",
                                "content": [{
                                    "type": "output_text",
                                    "text": "Isolated candidate inspected its copied manifest."
                                }]
                            }])
                        };
                        Json(serde_json::json!({
                            "id": format!("shadow-response-{call}"),
                            "model": "shadow-model",
                            "status": "completed",
                            "output": output,
                            "usage": {
                                "input_tokens": 24,
                                "output_tokens": 8,
                                "total_tokens": 32
                            }
                        }))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("shadow provider listener");
        let address = listener.local_addr().expect("provider address");
        tokio::spawn(async move {
            axum::serve(listener, provider)
                .await
                .expect("shadow provider");
        });
        reopened
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "shadow_fixture".into(),
                    display_name: "Shadow Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: format!("http://{address}/v1"),
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("configure provider");
        let executions = reopened
            .execute_automatic_shadow_runs(2)
            .await
            .expect("execute candidate output");
        assert_eq!(executions.len(), 1);
        let receipt = &executions[0].receipt;
        let shadow_view = reopened
            .agent_run_view(receipt.candidate_run_id)
            .expect("completed shadow turns");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            shadow_view.plan.items.len() * 2
        );
        assert!(
            shadow_view
                .leases
                .iter()
                .all(|lease| lease.state == AgentLeaseState::Completed)
        );
        assert!(shadow_view.results.iter().all(|result| {
            result.tool_outcomes.len() == 1
                && result.tool_outcomes[0].status == ToolOutcomeStatus::Succeeded
        }));
        let workspace = reopened
            .store()
            .expect("store")
            .shadow_workspace(experiment_id)
            .expect("workspace query")
            .expect("workspace");
        assert_eq!(workspace.state, ShadowWorkspaceState::Retained);
        assert_eq!(workspace.file_count, 2);
        assert_eq!(
            std::fs::read(
                config
                    .data_directory
                    .join(&workspace.relative_path)
                    .join("package.json")
            )
            .expect("copied manifest"),
            b"{}"
        );
        let workspace_root = config.data_directory.join(&workspace.relative_path);
        let executor =
            FilesystemExecutor::open([workspace_root], config.data_directory.join("artifacts"))
                .expect("isolated filesystem executor");
        let outside_request = ToolRequest::new(
            receipt.source_task_id,
            "attempt original project read",
            ToolRequestKind::FsRead {
                path: project_root
                    .path()
                    .join("package.json")
                    .to_string_lossy()
                    .into_owned(),
                max_bytes: 128,
            },
        );
        assert!(
            executor
                .execute(ToolExecutionId::new(), &outside_request)
                .await
                .is_err()
        );
        let artifact = config
            .data_directory
            .join("artifacts")
            .join(&receipt.output_artifact_sha256[..2])
            .join(&receipt.output_artifact_sha256);
        assert!(artifact.is_file());
        assert!(
            reopened
                .execute_automatic_shadow_runs(2)
                .await
                .expect("receipt replay")
                .is_empty()
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            shadow_view.plan.items.len() * 2
        );
        reopened.shadow_command_verifier = Arc::new(FixtureShadowCommandVerifier(
            FixtureShadowCommandResult::Pass,
        ));
        let assessments = [reopened
            .assess_automatic_shadow_run(executions[0].launch.experiment.id)
            .await
            .expect("assess retained shadow output")];
        assert_eq!(assessments[0].evaluation.outcome, EvaluationOutcome::Passed);
        assert!(assessments[0].benchmark.regressions.is_empty());
        assert!(
            assessments[0]
                .evaluation
                .conditions
                .iter()
                .all(|condition| condition.passed)
        );
        assert!(assessments[0].evaluation.evidence.iter().any(|evidence| {
            evidence.method == "isolated_file_verification"
                && evidence.artifact_sha256.as_deref()
                    == Some(format!("{:x}", Sha256::digest(b"{}")).as_str())
        }));
        assert!(assessments[0].evaluation.evidence.iter().any(|evidence| {
            evidence.method == "isolated_command_sandbox" && evidence.artifact_sha256.is_some()
        }));
        assert!(assessments[0].evaluation.evidence.iter().any(|evidence| {
            evidence.method == "isolated_research_verification"
                && evidence.artifact_sha256.is_some()
        }));
        assert_eq!(
            reopened
                .experiment(assessments[0].experiment.id)
                .expect("pre-decision experiment")
                .expect("present")
                .state,
            ExperimentState::Running
        );
        drop(reopened);
        let reopened = AppRuntime::open(config.clone()).expect("reopen after benchmark crash");
        assert!(
            reopened
                .assess_automatic_shadow_runs(2)
                .await
                .expect("resume automatic decision")
                .is_empty()
        );
        let promoted = reopened
            .experiment(assessments[0].experiment.id)
            .expect("promoted experiment")
            .expect("present");
        assert_eq!(promoted.state, ExperimentState::Promoted);
        assert_eq!(
            reopened
                .organization(promoted.organization_id)
                .expect("promoted organization")
                .expect("present")
                .revision_id,
            candidate.revision_id
        );
        let decisions = reopened
            .experiment_decisions(promoted.id)
            .expect("automatic decision");
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].kind, ExperimentDecisionKind::Promote);
        assert!(
            reopened
                .assess_automatic_shadow_runs(2)
                .await
                .expect("assessment replay")
                .is_empty()
        );
        drop(reopened);
        let final_reopen = AppRuntime::open(config).expect("reopen promoted experiment");
        assert_eq!(
            final_reopen
                .experiment(promoted.id)
                .expect("durable promotion")
                .expect("present")
                .state,
            ExperimentState::Promoted
        );
        assert!(
            final_reopen
                .assess_automatic_shadow_runs(2)
                .await
                .expect("durable replay")
                .is_empty()
        );
        let mut post_promotion_regression = assessments[0].evaluation.clone();
        post_promotion_regression.id = EvaluationId::new();
        post_promotion_regression.outcome = EvaluationOutcome::Failed;
        post_promotion_regression.regressions = vec!["seeded promoted regression".into()];
        post_promotion_regression.conditions[0].passed = false;
        post_promotion_regression.conditions[0].detail = "seeded promoted regression".into();
        post_promotion_regression.evaluated_at = Utc::now() + Duration::seconds(1);
        final_reopen
            .record_evaluation(&post_promotion_regression)
            .expect("post-promotion regression");
        let reversions = final_reopen
            .revert_regressing_automatic_experiments(8)
            .expect("automatic reversion");
        assert_eq!(reversions.len(), 1);
        assert_eq!(reversions[0].experiment.state, ExperimentState::Reverted);
        assert_eq!(
            final_reopen
                .organization(candidate.organization_id)
                .expect("restored organization")
                .expect("present")
                .revision_id,
            baseline_revision_id
        );
        assert!(
            final_reopen
                .revert_regressing_automatic_experiments(8)
                .expect("reversion replay")
                .is_empty()
        );
    }

    #[test]
    fn ssh_capability_extensions_preserve_legacy_definition_hashes() {
        #[derive(Serialize)]
        struct LegacyStep<'a> {
            id: SshWorkflowStepId,
            name: &'a str,
            host_id: SshHostId,
            host_revision: u32,
            dependencies: &'a [SshWorkflowStepId],
            operation: &'a SshOperationKind,
            timeout_seconds: u64,
        }

        let host_id = SshHostId::new();
        let host = SshHostRegistrationRequest {
            id: host_id,
            display_name: " Legacy host ".into(),
            hostname: "legacy.example.test".into(),
            port: 22,
            username: "builder".into(),
            platform: SshHostPlatform::Posix,
            authentication: SshAuthentication::Agent,
            known_hosts_entry: "legacy.example.test ssh-ed25519 AAAAC3NzaLegacy".into(),
            host_key_sha256: "SHA256:legacy".into(),
            capabilities: Vec::new(),
            state: SshHostState::Active,
        };
        let expected_host_hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    SSH_SCHEMA_VERSION,
                    host.id,
                    host.display_name.trim(),
                    host.hostname.trim(),
                    host.port,
                    host.username.trim(),
                    host.platform,
                    host.authentication,
                    host.known_hosts_entry.trim(),
                    host.host_key_sha256.trim(),
                    host.state,
                ))
                .expect("legacy host encoding")
            )
        );
        assert_eq!(
            ssh_host_definition_hash(&host).expect("host hash"),
            expected_host_hash
        );

        let step_id = SshWorkflowStepId::new();
        let operation = SshOperationKind::Inspect;
        let workflow = SshWorkflowCreateRequest {
            id: SshWorkflowId::new(),
            display_name: " Legacy workflow ".into(),
            task_id: None,
            steps: vec![SshWorkflowStepRequest {
                id: step_id,
                name: " inspect ".into(),
                host_id: Some(host_id),
                host_revision: Some(1),
                required_capabilities: Vec::new(),
                dependencies: Vec::new(),
                operation: operation.clone(),
                timeout_seconds: 60,
            }],
        };
        let legacy_steps = vec![LegacyStep {
            id: step_id,
            name: " inspect ",
            host_id,
            host_revision: 1,
            dependencies: &[],
            operation: &operation,
            timeout_seconds: 60,
        }];
        let expected_workflow_hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    SSH_WORKFLOW_SCHEMA_VERSION,
                    workflow.id,
                    workflow.display_name.trim(),
                    workflow.task_id,
                    legacy_steps,
                ))
                .expect("legacy workflow encoding")
            )
        );
        assert_eq!(
            ssh_workflow_definition_hash(&workflow).expect("workflow hash"),
            expected_workflow_hash
        );
    }

    #[test]
    fn ssh_workflow_creation_replay_and_host_pins_fail_closed() {
        let runtime =
            AppRuntime::open_in_memory(AppConfig::default(), "a".repeat(32)).expect("runtime");
        let host_id = SshHostId::new();
        runtime
            .register_ssh_host(&SshHostRegistrationRequest {
                id: host_id,
                display_name: "Workflow host".into(),
                hostname: "runtime-workflow.example.test".into(),
                port: 22,
                username: "builder".into(),
                platform: SshHostPlatform::Posix,
                authentication: SshAuthentication::Agent,
                known_hosts_entry:
                    "runtime-workflow.example.test ssh-ed25519 AAAAC3NzaRuntimeWorkflow".into(),
                host_key_sha256: "SHA256:runtimeWorkflow".into(),
                capabilities: vec!["arch:x86_64".into(), "os:linux".into()],
                state: SshHostState::Active,
            })
            .expect("host");
        let request = SshWorkflowCreateRequest {
            id: SshWorkflowId::new(),
            display_name: "Runtime workflow".into(),
            task_id: None,
            steps: vec![SshWorkflowStepRequest {
                id: SshWorkflowStepId::new(),
                name: "inspect".into(),
                host_id: Some(host_id),
                host_revision: Some(1),
                required_capabilities: Vec::new(),
                dependencies: Vec::new(),
                operation: SshOperationKind::Inspect,
                timeout_seconds: 60,
            }],
        };
        let created = runtime.create_ssh_workflow(&request).expect("create");
        assert!(!created.replayed);
        let replay = runtime.create_ssh_workflow(&request).expect("replay");
        assert!(replay.replayed);
        assert_eq!(
            replay.workflow.definition_hash,
            created.workflow.definition_hash
        );

        let mut conflict = request.clone();
        conflict.display_name = "Different authority".into();
        assert!(matches!(
            runtime.create_ssh_workflow(&conflict),
            Err(RuntimeError::Storage(StorageError::Conflict(_)))
        ));
        let mut stale = request;
        stale.id = SshWorkflowId::new();
        stale.steps[0].host_revision = Some(2);
        assert!(matches!(
            runtime.create_ssh_workflow(&stale),
            Err(RuntimeError::Storage(StorageError::Conflict(_)))
        ));
        let routed_request = SshWorkflowCreateRequest {
            id: SshWorkflowId::new(),
            display_name: "Capability-routed workflow".into(),
            task_id: None,
            steps: vec![SshWorkflowStepRequest {
                id: SshWorkflowStepId::new(),
                name: "route to Linux builder".into(),
                host_id: None,
                host_revision: None,
                required_capabilities: vec!["os:linux".into(), "arch:x86_64".into()],
                dependencies: Vec::new(),
                operation: SshOperationKind::Inspect,
                timeout_seconds: 60,
            }],
        };
        let routed = runtime
            .create_ssh_workflow(&routed_request)
            .expect("capability route");
        assert_eq!(routed.workflow.steps[0].host_id, host_id);
        assert_eq!(routed.workflow.steps[0].host_revision, 1);
        assert!(routed.workflow.steps[0].routed_automatically);
        assert_eq!(
            routed.workflow.steps[0].required_capabilities,
            ["arch:x86_64", "os:linux"]
        );
        let mut routed_replay = routed_request.clone();
        routed_replay.steps[0].required_capabilities =
            vec!["OS:LINUX".into(), "arch:x86_64".into()];
        assert!(
            runtime
                .create_ssh_workflow(&routed_replay)
                .expect("normalized route replay")
                .replayed
        );
        let mut unavailable = routed_request;
        unavailable.id = SshWorkflowId::new();
        unavailable.steps[0].required_capabilities = vec!["gpu:nvidia".into()];
        assert!(matches!(
            runtime.create_ssh_workflow(&unavailable),
            Err(RuntimeError::Storage(StorageError::Conflict(_)))
        ));
        let cancelled = runtime
            .cancel_ssh_workflow(created.workflow.id)
            .expect("cancel");
        assert_eq!(cancelled.workflow.state, SshWorkflowState::Cancelled);
    }

    #[tokio::test]
    async fn live_ssh_inspect_survives_runtime_restart_without_reexecution() {
        if std::env::var("MCG_LIVE_SSH").as_deref() != Ok("1") {
            eprintln!(
                "skipped: set MCG_LIVE_SSH=1 with the documented non-secret host metadata and an SSH agent"
            );
            return;
        }
        let hostname = std::env::var("MCG_LIVE_SSH_HOST").expect("live SSH hostname");
        let username = std::env::var("MCG_LIVE_SSH_USER").expect("live SSH username");
        let known_hosts_entry =
            std::env::var("MCG_LIVE_SSH_KNOWN_HOSTS").expect("pinned host line");
        let host_key_sha256 = std::env::var("MCG_LIVE_SSH_FINGERPRINT").expect("host fingerprint");
        let directory = tempfile::tempdir().expect("temporary runtime");
        let config = AppConfig::default().with_data_directory(directory.path());
        let host_id = SshHostId::new();
        let operation_id = SshOperationId::new();
        {
            let runtime = AppRuntime::open(config.clone()).expect("durable runtime");
            runtime
                .register_ssh_host(&SshHostRegistrationRequest {
                    id: host_id,
                    display_name: "Live Ubuntu qualification".into(),
                    hostname,
                    port: 22,
                    username,
                    platform: SshHostPlatform::Posix,
                    authentication: SshAuthentication::Agent,
                    known_hosts_entry,
                    host_key_sha256,
                    capabilities: vec!["os:linux".into()],
                    state: SshHostState::Active,
                })
                .expect("register pinned live host");
            let mutation = runtime
                .run_ssh_operation(host_id, operation_id, None, SshOperationKind::Inspect, 30)
                .await
                .expect("live inspect");
            assert_eq!(
                mutation.operation.state,
                SshOperationState::Succeeded,
                "live SSH outcome: {:?}",
                mutation.outcome
            );
            assert!(
                mutation
                    .outcome
                    .as_ref()
                    .and_then(|outcome| outcome.stdout.as_deref())
                    .is_some_and(|stdout| stdout.contains("Linux"))
            );
        }
        let reopened = AppRuntime::open(config).expect("reopen runtime");
        let replay = reopened
            .run_ssh_operation(host_id, operation_id, None, SshOperationKind::Inspect, 30)
            .await
            .expect("settled replay");
        assert!(replay.replayed);
        assert!(replay.outcome.is_none());
        assert_eq!(reopened.status().recovered_ssh_operations, 0);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Proves registry, health, approval, deployment, and replay together.
    async fn site_registration_and_health_are_idempotent_governed_and_artifact_backed() {
        let calls = Arc::new(AtomicUsize::new(0));
        let fixture_calls = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let app = Router::new().route(
            "/health",
            get(move || {
                let calls = fixture_calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (axum::http::StatusCode::NO_CONTENT, "")
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fixture server");
        });
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("package.json"), "{}").expect("manifest");
        let data = tempfile::tempdir().expect("data");
        let config = AppConfig::default().with_data_directory(data.path());
        let runtime = AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover project");
        let site_id = SiteId::new();
        let request = SiteRegistrationRequest {
            id: site_id,
            project_id,
            display_name: "  Fixture Site  ".into(),
            environment: SiteEnvironment::Production,
            public_url: format!("http://{address}"),
            health_url: format!("http://{address}/health"),
            hosting_provider: "fixture".into(),
            deployment_command: Some(SiteDeploymentCommand {
                executable: "node".into(),
                arguments: vec![
                    "-e".into(),
                    "console.log('deploy-ok');require('fs').appendFileSync('deploy-count.txt','x')"
                        .into(),
                ],
                working_directory: String::new(),
            }),
            credential_reference: None,
            state: SiteState::Active,
        };
        let created = runtime.register_site(&request).expect("register");
        assert!(!created.replayed);
        assert_eq!(created.site.display_name, "Fixture Site");
        assert!(runtime.register_site(&request).expect("replay").replayed);
        assert_eq!(runtime.site_revisions(site_id).expect("revisions").len(), 1);

        let check_id = SiteHealthCheckId::new();
        let checked = runtime
            .check_site_health(site_id, check_id)
            .await
            .expect("check");
        assert!(!checked.replayed);
        assert_eq!(checked.check.state, SiteHealthState::Healthy);
        assert_eq!(checked.check.status_code, Some(204));
        assert!(checked.check.response_artifact_sha256.is_some());
        assert!(
            runtime
                .check_site_health(site_id, check_id)
                .await
                .expect("check replay")
                .replayed
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            runtime.site_health_checks(site_id).expect("history").len(),
            1
        );
        let audit = runtime
            .tool_audit(site_authority_task_id(site_id))
            .expect("audit");
        assert_eq!(audit.len(), 2);
        assert!(audit.iter().all(|record| record.tool == "http.request"));

        let deployment_id = SiteDeploymentId::new();
        assert!(matches!(
            runtime.deploy_site(site_id, deployment_id, None).await,
            Err(RuntimeError::Tool(ToolError::ApprovalRequired { .. }))
        ));
        assert!(
            runtime
                .site_deployments(site_id)
                .expect("deployments")
                .is_empty()
        );
        let pending = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("pending approval");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].action, "sites.deploy");
        assert!(pending[0].resource.contains("deploy-count.txt"));
        runtime
            .resolve_approval(&ApprovalResolution {
                approval_id: pending[0].id,
                state: ApprovalState::Approved,
                resolved_by: "fixture owner".into(),
                reason: Some("exercise exact production deployment".into()),
                resolved_at: Utc::now(),
            })
            .expect("approve");
        let deployed = runtime
            .deploy_site(site_id, deployment_id, None)
            .await
            .expect("deploy");
        assert_eq!(deployed.deployment.state, SiteDeploymentState::Succeeded);
        assert_eq!(deployed.deployment.log_artifact_sha256.len(), 1);
        assert_eq!(
            std::fs::read_to_string(project_root.path().join("deploy-count.txt"))
                .expect("deployment marker"),
            "x"
        );
        assert!(
            runtime
                .deploy_site(site_id, deployment_id, None)
                .await
                .expect("deployment replay")
                .replayed
        );
        assert_eq!(
            std::fs::read_to_string(project_root.path().join("deploy-count.txt"))
                .expect("unchanged marker"),
            "x"
        );
        server.abort();
    }

    #[test]
    fn browser_session_definition_remains_retry_compatible() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime should open");
        let now = Utc::now();
        let request = PlatformSessionRequest {
            id: PlatformSessionId::new(),
            kind: PlatformSessionKind::Browser,
            project_id: None,
            task_id: None,
            display_name: "  compatible browser  ".into(),
            allowed_origins: vec!["https://example.com".into()],
            computer_capabilities: Vec::new(),
            computer_target: None,
            lifetime_seconds: 300,
        };
        let legacy_definition = serde_json::to_vec(&(
            PLATFORM_SCHEMA_VERSION,
            request.kind,
            request.project_id,
            request.task_id,
            request.display_name.trim(),
            &request.allowed_origins,
            request.lifetime_seconds,
        ))
        .expect("legacy definition");
        let expected = format!("{:x}", Sha256::digest(legacy_definition));

        let created = runtime
            .request_platform_session(&request, now)
            .expect("request");
        assert_eq!(created.session.definition_hash, expected);
        assert!(created.created);
        assert!(
            !runtime
                .request_platform_session(&request, now)
                .expect("exact retry")
                .created
        );
    }

    #[tokio::test]
    async fn persistent_browser_authority_and_history_follow_real_executor_outcomes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let app = Router::new()
            .route("/one", get(|| async { Html("<title>One</title>") }))
            .route("/two", get(|| async { Html("<title>Two</title>") }));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fixture server");
        });
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        let now = Utc::now();
        let session_id = PlatformSessionId::new();
        let origin = format!("http://{address}");
        runtime
            .request_platform_session(
                &PlatformSessionRequest {
                    id: session_id,
                    kind: PlatformSessionKind::Browser,
                    project_id: None,
                    task_id: None,
                    display_name: "real browser fixture".into(),
                    allowed_origins: vec![origin.clone()],
                    computer_capabilities: Vec::new(),
                    computer_target: None,
                    lifetime_seconds: 300,
                },
                now,
            )
            .expect("request");
        let attached = runtime
            .attach_browser_session(session_id, PersistentBrowserMode::Headless, now)
            .await
            .expect("attach");
        assert_eq!(attached.session.state, PlatformSessionState::Active);
        assert_eq!(attached.browser.safe_url, "about:blank");
        let one = runtime
            .navigate_browser_session(session_id, &format!("{origin}/one?secret=hidden"))
            .await
            .expect("first navigation");
        assert_eq!(one.title.as_deref(), Some("One"));
        assert!(!one.safe_url.contains("secret"));
        let two = runtime
            .navigate_browser_session(session_id, &format!("{origin}/two"))
            .await
            .expect("second navigation");
        assert_eq!(two.title.as_deref(), Some("Two"));
        assert!(
            runtime
                .navigate_browser_session(session_id, "https://example.com/")
                .await
                .expect_err("cross-origin navigation must fail")
                .is_invalid_request()
        );
        assert_eq!(
            runtime
                .platform_session(session_id)
                .await
                .expect("session")
                .state,
            PlatformSessionState::Active
        );
        assert!(
            runtime
                .handoff_browser_session(session_id, Utc::now())
                .await
                .expect_err("headless handoff must fail")
                .is_invalid_request()
        );
        runtime
            .settle_expired_platform_sessions(now + Duration::seconds(301))
            .await
            .expect("expire and clean executor");
        assert_eq!(
            runtime
                .load_platform_session(session_id)
                .expect("closed")
                .state,
            PlatformSessionState::Closed
        );
        let history = runtime
            .platform_session_history(session_id)
            .await
            .expect("history");
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].to_state, PlatformSessionState::Requested);
        assert_eq!(history[1].to_state, PlatformSessionState::Active);
        assert_eq!(history[2].to_state, PlatformSessionState::Closed);
        server.abort();
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Covers discovery, namespace binding, and live governed execution.
    async fn discovered_mcp_tool_is_namespaced_and_executes_through_runtime_boundary() {
        async fn fixture(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
            let result = match body["method"].as_str().expect("method") {
                "server/discover" => serde_json::json!({
                    "supportedVersions": [mcg_mcp::MCP_PROTOCOL_VERSION],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "runtime-fixture"}
                }),
                "tools/list" => serde_json::json!({
                    "tools": [{
                        "name": "lookup",
                        "description": "Read fixture data",
                        "inputSchema": {"type": "object"},
                        "annotations": {
                            "readOnlyHint": true,
                            "destructiveHint": false,
                            "openWorldHint": false
                        }
                    }],
                    "serverInfo": {"name": "runtime-fixture"}
                }),
                "tools/call" => serde_json::json!({
                    "isError": false,
                    "content": [{"type": "text", "text": "fixture result"}],
                    "structuredContent": {"value": 42},
                    "serverInfo": {"name": "runtime-fixture"}
                }),
                _ => unreachable!(),
            };
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": body["id"].clone(),
                "result": result
            }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/mcp", post(fixture)))
                .await
                .expect("serve");
        });
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        runtime
            .configure_mcp(
                McpConnection {
                    contract_version: mcg_mcp::MCP_CONNECTION_CONTRACT_VERSION,
                    id: "fixture".into(),
                    display_name: "Fixture".into(),
                    transport: mcg_mcp::McpTransport::StreamableHttp {
                        url: format!("http://{address}/mcp"),
                    },
                    auth_mode: McpAuthMode::None,
                    protocol_mode: mcg_mcp::McpProtocolMode::Auto,
                    enabled: true,
                },
                None,
            )
            .await
            .expect("configure MCP");
        let (definitions, bindings) = runtime
            .resolve_agent_tools(RoleKind::Researcher, &["mcp::fixture::lookup".into()])
            .expect("resolve tools");
        assert_eq!(definitions.len(), 1);
        assert_eq!(bindings.len(), 1);
        assert!(definitions[0].name.starts_with("mcp_fixture_lookup_"));
        let request = ToolRequest {
            schema_version: TOOL_SCHEMA_VERSION,
            id: ToolRequestId::new(),
            task_id: TaskId::new(),
            namespace: mcg_tools::ToolNamespace::ExternalMcp,
            summary: "Read fixture data".into(),
            timeout_seconds: 30,
            kind: ToolRequestKind::McpCall {
                connection_id: bindings[0].connection_id.clone(),
                tool_name: bindings[0].remote_name.clone(),
                arguments: serde_json::json!({"query": "fixture"}),
                read_only: bindings[0].read_only,
                destructive: bindings[0].destructive,
                open_world: bindings[0].open_world,
            },
            created_at: Utc::now(),
        };
        let outcome = RuntimeMcpExecutor {
            store: runtime.store.clone(),
            credentials: runtime.mcp_credentials.clone(),
        }
        .execute(ToolExecutionId::new(), &request)
        .await
        .expect("execute MCP");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(outcome.stdout.expect("stdout").contains("fixture result"));
    }

    struct CountingExecutor(Arc<AtomicUsize>);

    #[async_trait]
    impl ToolExecutor for CountingExecutor {
        async fn execute(
            &self,
            execution_id: ToolExecutionId,
            request: &ToolRequest,
        ) -> Result<ToolOutcome, ToolError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutcome {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id: request.id,
                status: ToolOutcomeStatus::Succeeded,
                summary: "fixture executed".into(),
                exit_code: None,
                stdout: None,
                stderr: None,
                artifacts: Vec::new(),
                verification: vec![VerificationEvidence {
                    method: "fixture".into(),
                    passed: true,
                    detail: "true".into(),
                }],
                started_at: chrono::Utc::now(),
                finished_at: chrono::Utc::now(),
            })
        }
    }

    #[tokio::test]
    async fn durable_broker_waits_for_owner_then_records_completion() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        let task = TaskEnvelope::new("durable broker");
        runtime.create_task(&task).expect("task");
        let root = tempfile::tempdir().expect("root");
        let count = Arc::new(AtomicUsize::new(0));
        let control = runtime.tool_control();
        let broker = ToolBroker::new(
            CountingExecutor(count.clone()),
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: mcg_config::AutonomyPreset::Autonomous,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: CapabilityPolicy::project(root.path()),
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let request = ToolRequest::new(
            task.id,
            "delete generated output",
            ToolRequestKind::FsDelete {
                path: root.path().join("generated").to_string_lossy().into(),
                recursive: true,
            },
        );
        assert!(matches!(
            broker.execute(&request).await,
            Err(ToolError::ApprovalRequired { .. })
        ));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let approval = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("approvals")
            .into_iter()
            .next()
            .expect("pending approval");
        runtime
            .resolve_approval(&ApprovalResolution {
                approval_id: approval.id,
                state: ApprovalState::Approved,
                resolved_by: "test_owner".into(),
                reason: None,
                resolved_at: chrono::Utc::now(),
            })
            .expect("resolve");
        broker.execute(&request).await.expect("execute approved");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let audit = runtime.tool_audit(task.id).expect("audit");
        assert_eq!(audit.len(), 3);
        assert_eq!(
            audit.last().and_then(|record| record.result),
            Some(ToolOutcomeStatus::Succeeded)
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One end-to-end safety scenario covers read, approval, retry, and confinement.
    async fn provider_host_tool_is_project_confined_stable_and_approval_resumable() {
        let data_directory = tempfile::tempdir().expect("data directory");
        let config = AppConfig {
            data_directory: data_directory.path().to_path_buf(),
            ..AppConfig::default()
        };
        let runtime = AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime");
        let current = runtime.user_settings();
        runtime
            .update_user_settings(
                current.revision,
                current.advanced_mode,
                AutonomyPreset::Guided,
                current.shadow_experiments,
                "provider host tool test",
                Utc::now(),
            )
            .expect("guided settings");
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]")
            .expect("project manifest");
        std::fs::write(project_root.path().join("fixture.txt"), "fixture").expect("fixture file");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("project");
        let mut task = TaskEnvelope::new("provider host tool boundary");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("task");
        let call_id = ProxyCallId::new();

        let read = runtime
            .execute_provider_host_tool(
                task.id,
                call_id,
                "model-call-read",
                "host::fs.read",
                r#"{"path":"fixture.txt","max_bytes":1024}"#,
            )
            .await
            .expect("project read");
        assert_eq!(read.status, ToolOutcomeStatus::Succeeded);
        assert!(read.stdout.is_some_and(|stdout| stdout.contains("fixture")));

        let write_arguments = r#"{"path":"generated.txt","content":"generated","create_new":true}"#;
        let approval_id = match runtime
            .execute_provider_host_tool(
                task.id,
                call_id,
                "model-call-write",
                "host::fs.write",
                write_arguments,
            )
            .await
        {
            Err(RuntimeError::Tool(ToolError::ApprovalRequired { approval_id, .. })) => approval_id,
            other => panic!("guided provider write should require approval: {other:?}"),
        };
        runtime
            .resolve_approval(&ApprovalResolution {
                approval_id,
                state: ApprovalState::Approved,
                resolved_by: "test_owner".into(),
                reason: None,
                resolved_at: Utc::now(),
            })
            .expect("approve write");
        let written = runtime
            .execute_provider_host_tool(
                task.id,
                call_id,
                "model-call-write",
                "host::fs.write",
                write_arguments,
            )
            .await
            .expect("approved write");
        assert_eq!(written.status, ToolOutcomeStatus::Succeeded);
        assert_eq!(
            std::fs::read_to_string(project_root.path().join("generated.txt"))
                .expect("generated file"),
            "generated"
        );
        assert!(
            runtime
                .execute_provider_host_tool(
                    task.id,
                    call_id,
                    "model-call-escape",
                    "host::fs.read",
                    r#"{"path":"../outside.txt","max_bytes":1024}"#,
                )
                .await
                .is_err()
        );
        assert!(
            runtime
                .execute_provider_host_tool(
                    task.id,
                    call_id,
                    "model-call-override",
                    "host::fs.read",
                    r#"{"tool":"fs_delete","path":"fixture.txt","max_bytes":1024}"#,
                )
                .await
                .is_err()
        );
        let audits = runtime.tool_audit(task.id).expect("tool audit");
        assert!(audits.iter().any(|record| {
            record.request_id == written.request_id
                && record.result == Some(ToolOutcomeStatus::Succeeded)
        }));
    }

    #[tokio::test]
    async fn provider_mcg_tools_are_bounded_project_scoped_and_durably_audited() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(project_root.path().join("Cargo.toml"), "[workspace]")
            .expect("project manifest");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("project");
        let mut task = TaskEnvelope::new("inspect durable MCG context");
        task.project_id = Some(project_id);
        runtime.create_task(&task).expect("task");
        runtime
            .create_memory_claim(
                project_id,
                "fixture",
                "package_manager",
                serde_json::json!("cargo"),
                1.0,
                ClaimProvenance {
                    source_kind: mcg_projects::ProvenanceKind::User,
                    source: "provider-mcg-test".into(),
                    task_id: Some(task.id.to_string()),
                    artifact_sha256: None,
                },
                None,
            )
            .expect("memory");
        let call_id = ProxyCallId::new();
        let task_outcome = runtime
            .execute_provider_mcg_tool(task.id, call_id, "task-call", "mcg::task.get", "{}")
            .await
            .expect("task get");
        assert_eq!(task_outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(
            task_outcome
                .stdout
                .as_deref()
                .is_some_and(|output| output.contains("inspect durable MCG context"))
        );
        let memory_outcome = runtime
            .execute_provider_mcg_tool(
                task.id,
                call_id,
                "memory-call",
                "mcg::memory.search",
                r#"{"query":"cargo","limit":5}"#,
            )
            .await
            .expect("memory search");
        assert!(
            memory_outcome
                .stdout
                .as_deref()
                .is_some_and(|output| output.contains("package_manager"))
        );
        assert!(
            runtime
                .execute_provider_mcg_tool(
                    task.id,
                    call_id,
                    "traversal-call",
                    "mcg::project.context",
                    r#"{"relative_target":"../outside","token_budget":256}"#,
                )
                .await
                .is_err()
        );
        assert!(
            runtime
                .execute_provider_mcg_tool(
                    task.id,
                    call_id,
                    "unknown-call",
                    "mcg::task.get",
                    r#"{"untrusted":true}"#,
                )
                .await
                .is_err()
        );
        let audits = runtime.tool_audit(task.id).expect("MCG audits");
        assert_eq!(audits.len(), 4);
        assert!(audits.iter().all(|record| {
            record.tool.starts_with("mcg::")
                && !record.request_fingerprint.contains("cargo")
                && !record.decision.contains("cargo")
        }));
    }

    struct TurnModels {
        responses: Mutex<VecDeque<ModelResponse>>,
    }

    #[async_trait]
    impl AgentModelGateway for TurnModels {
        async fn create_response(
            &self,
            _target: &ProviderModelRef,
            _request: mcg_providers::ModelRequest,
        ) -> Result<ModelResponse, ProviderError> {
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or_else(|| ProviderError::Incompatible("missing fixture response".into()))
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Full success/failure runtime/provider/broker/storage boundary fixture.
    async fn completed_and_failed_agent_turns_settle_usage_and_authority_atomically() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        let root = tempfile::tempdir().expect("project root");
        let fixture_path = root.path().join("fixture.txt");
        std::fs::write(&fixture_path, "fixture").expect("fixture file");
        let mut task = TaskEnvelope::new("Read the fixture through the agent organization");
        task.budget = TaskBudget {
            max_tokens: Some(10_000),
            max_cost_usd: Some(2.0),
            max_duration_seconds: Some(600),
        };
        runtime.create_task(&task).expect("task");
        let now = Utc::now();
        let genome = OrganizationGenome::consumer_default(now);
        runtime
            .record_organization_revision(&genome)
            .expect("organization");
        let planned = PlannedWork::atomic(
            "Read the fixture",
            RoleKind::Coder,
            vec![SuccessCondition::Custom {
                description: "fixture content returned".into(),
            }],
        );
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![planned],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&genome, &planning, now)
            .expect("plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: genome.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        runtime.create_agent_run(&run, &plan).expect("run");
        runtime
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
            )
            .expect("start run");
        let role = genome
            .roles
            .iter()
            .find(|role| role.kind == RoleKind::Coder)
            .expect("coder role")
            .clone();
        let created = AgentInstance {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentInstanceId::new(),
            role_id: role.id,
            run_id: run.id,
            task_id: task.id,
            state: AgentState::Created,
            model: None,
            context_fingerprint: "sha256:runtime-fixture".into(),
            confidence: ConfidenceEstimate::new(8_000, 1).expect("confidence"),
            created_at: now,
            updated_at: now,
        };
        runtime.record_agent_instance(&created).expect("instance");
        let instance = runtime
            .transition_agent_instance(created.id, AgentState::Running, created.confidence)
            .expect("start instance");
        let lease = AgentLease {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: AgentLeaseId::new(),
            instance_id: instance.id,
            work_item_id: plan.items[0].id,
            goal: plan.items[0].goal.clone(),
            budget: LeaseBudget {
                limit: plan.items[0].budget.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            tool_permissions: vec!["fs.read".into()],
            reporting_interval_seconds: 30,
            max_retries: 1,
            escalation_conditions: Vec::new(),
            state: mcg_agents::AgentLeaseState::Active,
            acquired_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(5),
            heartbeat_at: Utc::now(),
        };
        runtime.record_agent_lease(&lease).expect("lease");

        let arguments = serde_json::json!({
            "path": fixture_path.to_string_lossy(),
            "max_bytes": 4096
        })
        .to_string();
        let models = TurnModels {
            responses: Mutex::new(VecDeque::from([
                ModelResponse {
                    contract_version: CONTRACT_VERSION,
                    id: "response-tool".into(),
                    model: "fixture-model".into(),
                    status: Some("completed".into()),
                    output: vec![OutputItem::ToolCall {
                        id: "call-read".into(),
                        name: "fs.read".into(),
                        arguments,
                    }],
                    usage: Usage {
                        input_tokens: 50,
                        output_tokens: 10,
                        total_tokens: 60,
                        ..Usage::default()
                    },
                    provider_metadata: serde_json::Value::Null,
                },
                ModelResponse {
                    contract_version: CONTRACT_VERSION,
                    id: "response-final".into(),
                    model: "fixture-model".into(),
                    status: Some("completed".into()),
                    output: vec![OutputItem::Message {
                        role: MessageRole::Assistant,
                        content: vec![ContentPart::Text {
                            text: "The fixture was read.".into(),
                        }],
                    }],
                    usage: Usage {
                        input_tokens: 80,
                        output_tokens: 8,
                        total_tokens: 88,
                        ..Usage::default()
                    },
                    provider_metadata: serde_json::Value::Null,
                },
            ])),
        };
        let control = runtime.tool_control();
        let broker = ToolBroker::new(
            CountingExecutor(Arc::new(AtomicUsize::new(0))),
            control.clone(),
            control.clone(),
            BrokerControls {
                preset: mcg_config::AutonomyPreset::Autonomous,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities: CapabilityPolicy::project(root.path()),
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let turn = AgentTurnRequest {
            role,
            instance,
            lease: lease.clone(),
            context: ContextAssembly {
                items: vec![ContextItem {
                    category: ContextCategory::Goal,
                    content: task.goal.clone(),
                    source: "task".into(),
                    priority: 1_000,
                    estimated_tokens: 12,
                }],
                omitted_items: 0,
                estimated_tokens: 12,
                compression_round: 0,
            },
            route_candidates: vec![RouteCandidate {
                target: ProviderModelRef {
                    provider_id: "fixture-provider".into(),
                    model_id: "fixture-model".into(),
                },
                model: ModelDescriptor {
                    id: "fixture-model".into(),
                    display_name: "Fixture".into(),
                    owned_by: None,
                    context_window: Some(8_192),
                    capabilities: ModelCapabilities {
                        tools: Some(true),
                        ..ModelCapabilities::default()
                    },
                    price: None,
                    metadata: serde_json::Value::Null,
                },
                preference: 0,
            }],
            routing_policy: RoutingPolicy::default(),
            tools: vec![ToolDefinition {
                name: "fs.read".into(),
                description: Some("Read a bounded project file".into()),
                parameters: serde_json::json!({"type":"object"}),
                strict: Some(true),
            }],
            external_tools: Vec::new(),
            estimated_output_tokens: 200,
            max_model_calls: 4,
        };
        let failed_turn_template = turn.clone();
        let commit = runtime
            .execute_agent_turn(
                turn,
                &models,
                &broker,
                ConfidenceEstimate::new(9_000, 1).expect("confidence"),
            )
            .await
            .expect("execute and commit turn");
        assert_eq!(commit.instance.state, AgentState::Completed);
        assert_eq!(commit.lease.state, mcg_agents::AgentLeaseState::Completed);
        assert_eq!(commit.usage.len(), 3);
        assert_eq!(commit.run.metrics.model_calls, 2);
        assert_eq!(commit.run.metrics.tool_calls, 1);
        {
            let store = runtime.store().expect("store");
            assert_eq!(
                store.agent_turn_result(lease.id).expect("result"),
                Some(commit.result.clone())
            );
            assert_eq!(store.agent_usage(run.id).expect("usage").len(), 3);
        }
        assert!(
            runtime
                .task_events(task.id)
                .expect("events")
                .iter()
                .any(|event| event.kind == mcg_events::EventKind::AgentTurnCompleted)
        );

        let failed_created = AgentInstance {
            id: AgentInstanceId::new(),
            state: AgentState::Created,
            model: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            ..failed_turn_template.instance.clone()
        };
        runtime
            .record_agent_instance(&failed_created)
            .expect("failed-turn instance");
        let failed_instance = runtime
            .transition_agent_instance(
                failed_created.id,
                AgentState::Running,
                failed_created.confidence,
            )
            .expect("start failed-turn instance");
        let failed_lease = AgentLease {
            id: AgentLeaseId::new(),
            instance_id: failed_instance.id,
            budget: LeaseBudget {
                limit: failed_turn_template.lease.budget.limit.clone(),
                consumed_tokens: 0,
                consumed_cost_usd: 0.0,
                elapsed_seconds: 0,
            },
            state: mcg_agents::AgentLeaseState::Active,
            acquired_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(5),
            heartbeat_at: Utc::now(),
            ..failed_turn_template.lease.clone()
        };
        runtime
            .record_agent_lease(&failed_lease)
            .expect("failed-turn lease");
        let failed_arguments = serde_json::json!({
            "path": fixture_path.to_string_lossy(),
            "max_bytes": 4096
        })
        .to_string();
        let failed_models = TurnModels {
            responses: Mutex::new(VecDeque::from([ModelResponse {
                contract_version: CONTRACT_VERSION,
                id: "response-before-provider-failure".into(),
                model: "fixture-model".into(),
                status: Some("completed".into()),
                output: vec![OutputItem::ToolCall {
                    id: "call-before-provider-failure".into(),
                    name: "fs.read".into(),
                    arguments: failed_arguments,
                }],
                usage: Usage {
                    input_tokens: 50,
                    output_tokens: 10,
                    total_tokens: 60,
                    ..Usage::default()
                },
                provider_metadata: serde_json::Value::Null,
            }])),
        };
        let failed_turn = AgentTurnRequest {
            instance: failed_instance,
            lease: failed_lease.clone(),
            ..failed_turn_template
        };
        assert!(matches!(
            runtime
                .execute_agent_turn(
                    failed_turn,
                    &failed_models,
                    &broker,
                    ConfidenceEstimate::new(9_000, 1).expect("confidence"),
                )
                .await,
            Err(RuntimeError::AgentTurn(AgentTurnError::Provider(
                ProviderError::Incompatible(_)
            )))
        ));
        let store = runtime.store().expect("store after failed turn");
        let failed_instance = store
            .agent_instance(failed_created.id)
            .expect("failed instance")
            .expect("present");
        assert_eq!(failed_instance.state, AgentState::Failed);
        assert_eq!(
            failed_instance.model.as_ref().map(|model| (
                model.provider_connection_id.as_str(),
                model.model_id.as_str()
            )),
            Some(("fixture-provider", "fixture-model"))
        );
        let failed_lease = store
            .agent_lease(failed_lease.id)
            .expect("failed lease")
            .expect("present");
        assert_eq!(failed_lease.state, mcg_agents::AgentLeaseState::Failed);
        assert_eq!(failed_lease.budget.consumed_tokens, 60);
        let usage = store.agent_usage(run.id).expect("all usage");
        let failed_usage = usage
            .iter()
            .filter(|usage| usage.instance_id == Some(failed_created.id))
            .collect::<Vec<_>>();
        assert_eq!(failed_usage.len(), 2);
        assert_eq!(
            failed_usage
                .iter()
                .filter(|usage| usage.kind == mcg_agents::UsageKind::ProviderModel)
                .count(),
            1
        );
        assert_eq!(
            failed_usage
                .iter()
                .filter(|usage| usage.kind == mcg_agents::UsageKind::Tool)
                .count(),
            1
        );
        let settled_run = store.agent_run(run.id).expect("run").expect("present");
        assert_eq!(settled_run.metrics.model_calls, 3);
        assert_eq!(settled_run.metrics.tool_calls, 2);
        assert_eq!(settled_run.metrics.tokens, 208);
        assert_eq!(
            store
                .agent_turn_checkpoint(failed_lease.id)
                .expect("checkpoint"),
            None
        );
        assert_eq!(
            store
                .agent_turn_result(failed_lease.id)
                .expect("failed result"),
            None
        );
        drop(store);
        let events = runtime.task_events(task.id).expect("events after failure");
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == mcg_events::EventKind::AgentUsageRecorded)
                .count(),
            5
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == mcg_events::EventKind::AgentTurnFailed)
        );
    }
}
