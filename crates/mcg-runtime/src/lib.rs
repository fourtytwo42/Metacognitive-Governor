//! Dependency wiring, durable task orchestration, and local authentication.

mod oauth;

pub use oauth::*;

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentModelGateway,
    AgentRun, AgentRunState, AgentState, AgentToolGateway, AgentTurnCheckpoint,
    AgentTurnCheckpointSink, AgentTurnEngine, AgentTurnError, AgentTurnRequest, AgentTurnResult,
    AgentUsageRecord, ConditionResult, ConfidenceEstimate, EvaluationEvidence, EvaluationOutcome,
    EvaluationReport, ExperimentBenchmark, ExperimentDecision, ExperimentDefinition,
    ExperimentMode, ExperimentState, ExternalToolBinding, LeaseBudget, MeasuredCandidateDesigner,
    OrganizationGenome, OrganizationalMetrics, RoleKind, RunSupervisionSnapshot,
    ShadowExecutionReceipt, SuccessCondition, SupervisionAnalyzer, SupervisionObservation,
    SupervisionSignal, WorkPlan,
};
use mcg_config::{AppConfig, AutonomyPreset};
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, ApprovalId, ComputerActionBatch, ComputerActionBatchId,
    ComputerActionBatchState, ComputerUseCapability, ComputerUseTarget, ExperimentId, LeaseId,
    MemoryClaimId, NotificationSeverity, OrganizationId, OrganizationRevisionId,
    PLATFORM_SCHEMA_VERSION, PlatformCapability, PlatformCapabilityAvailability,
    PlatformCapabilityKind, PlatformCapabilitySnapshot, PlatformCapabilitySnapshotId,
    PlatformSession, PlatformSessionEvent, PlatformSessionId, PlatformSessionKind,
    PlatformSessionOwner, PlatformSessionState, ProjectId, RiskLevel, RunId, SITE_SCHEMA_VERSION,
    SSH_SCHEMA_VERSION, ScheduleDefinition, ScheduleId, ScheduleOccurrence, ScheduleOccurrenceId,
    ScheduleOccurrenceState, ScheduleState, ScheduleTrigger, SchedulerNotification,
    SchedulerNotificationId, Site, SiteDeployment, SiteDeploymentCommand, SiteDeploymentId,
    SiteDeploymentState, SiteEnvironment, SiteHealthCheck, SiteHealthCheckId, SiteHealthState,
    SiteId, SiteMonitor, SiteRevision, SiteRevisionId, SiteState, SshAuthentication, SshHost,
    SshHostId, SshHostPlatform, SshHostRevision, SshHostRevisionId, SshHostState, SshOperation,
    SshOperationId, SshOperationKind, SshOperationState, SupervisionObservationId, TaskBudget,
    TaskEnvelope, TaskId, TaskState, ToolExecutionId, ToolRequestId, WorkItemId,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{
    LEGACY_MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSION, McpAuthMode, McpClient, McpConnection,
    McpCredential, McpCredentialStore, McpError, McpProtocolMode, McpToolCatalog,
    MemoryMcpCredentialStore, OsMcpCredentialStore,
};
use mcg_policy::{CapabilityPolicy, OwnerConstitution};
use mcg_projects::{
    ClaimProvenance, ContextAssembler, ContextAssembly, ContextCategory, ContextItem, MemoryClaim,
    MemoryError, MemoryLifecycle, MemoryLifecycleAction, ProjectDiscoverer, ProjectDiscoveryError,
    ProjectModel, SnapshotInvalidation, render_managed_memory,
};
use mcg_providers::{
    BrowserAuthorizationConfig, DeviceAuthorizationConfig, MemoryProviderCredentialStore,
    ModelProbeReport, ModelRequest, ModelResponse, OAuthTokenSet, OsProviderCredentialStore,
    ProviderConnection, ProviderCredential, ProviderCredentialStore, ProviderError,
    ProviderModelCatalog, ProviderModelRef, ProviderRegistry, RouteCandidate, RoutingPolicy,
    ToolDefinition, clear_provider_credentials, save_provider_credentials,
};
use mcg_storage::{
    AgentRecoveryReport, AgentTurnDisposition, ConsumerEvaluationCheckpoint,
    ConsumerEvaluationCheckpointState, RecoveryReport, SqliteEventStore, StorageError,
};
pub use mcg_storage::{
    AgentTurnCommit, AutomaticExperimentDisposition, AutomaticExperimentOutcome,
    AutomaticSupervisionMutation, CandidateRevisionMutation, ExperimentMutation, ScheduleClaim,
    ScheduleWakeMutation,
};
use mcg_tools::{
    ApprovalRepository, ApprovalRequest, ApprovalResolution, ApprovalState, ArtifactStore,
    BrokerControls, BrowserExecutor, BrowserWorkflowDocument, BrowserWorkflowStep,
    ComputerUseAction, ComputerUseBatchResult, ComputerUseFrame, ComputerUseManager,
    ComputerUseObservation, ExecutionLease, ExecutionLeaseRepository, FilesystemExecutor,
    GitExecutor, HostExecutor, HttpExecutor, InMemoryMemoryRepository, LeaseState, MemoryExecutor,
    OutputRedactor, PersistentBrowserManager, PersistentBrowserMode, PersistentBrowserObservation,
    ProcessExecutor, ProcessOptions, ProcessStream, RejectingCredentialResolver,
    ResearchEvidenceDocument, ResearchExecutor, SshExecutor, SystemExecutor, TOOL_SCHEMA_VERSION,
    ToolAuditRecord, ToolAuditSink, ToolBroker, ToolError, ToolExecutor, ToolOutcome,
    ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
    browser_runtime_available, computer_use_runtime_available,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
                )
                | Self::AgentTurn(AgentTurnError::Invalid(_))
                | Self::Tool(ToolError::Invalid(_))
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
pub struct AgentRunView {
    pub run: AgentRun,
    pub plan: WorkPlan,
    pub instances: Vec<AgentInstance>,
    pub leases: Vec<AgentLease>,
    pub usage: Vec<AgentUsageRecord>,
    pub evaluations: Vec<EvaluationReport>,
    pub results: Vec<AgentTurnResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceAgentRun {
    pub task: TaskEnvelope,
    pub view: AgentRunView,
    pub replayed: bool,
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
        self.registry
            .read()
            .await
            .stream_response(target, request)
            .await
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

pub struct AppRuntime {
    config: AppConfig,
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
    platform_capabilities: PlatformCapabilitySnapshot,
    browser_sessions: Arc<PersistentBrowserManager>,
    computer_sessions: Arc<ComputerUseManager>,
    provider_credentials: Arc<dyn ProviderCredentialStore>,
    mcp_credentials: Arc<dyn McpCredentialStore>,
    provider_registry: Arc<tokio::sync::RwLock<ProviderRegistry>>,
    oauth_state: OAuthRuntimeState,
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
        let platform_capabilities = detect_platform_capabilities(&config, startup_now);
        let platform_capabilities =
            store.record_platform_capability_snapshot(&platform_capabilities)?;
        let browser_sessions = Arc::new(PersistentBrowserManager::new(
            config.browser_executable.clone(),
            config.data_directory.join("browser-sessions"),
        )?);
        let computer_sessions = Arc::new(ComputerUseManager::new());
        let provider_credentials: Arc<dyn ProviderCredentialStore> =
            Arc::new(OsProviderCredentialStore);
        let mcp_credentials: Arc<dyn McpCredentialStore> = Arc::new(OsMcpCredentialStore);
        Ok(Self::assemble(
            config,
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
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            BrowserAuthorizationConfig::default(),
            DeviceAuthorizationConfig::default(),
        ))
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
        config: AppConfig,
        token: String,
        browser_config: BrowserAuthorizationConfig,
        device_config: DeviceAuthorizationConfig,
    ) -> Result<Self, RuntimeError> {
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
        let platform_capabilities = detect_platform_capabilities(&config, startup_now);
        let platform_capabilities =
            store.record_platform_capability_snapshot(&platform_capabilities)?;
        let browser_sessions = Arc::new(PersistentBrowserManager::new(
            config.browser_executable.clone(),
            std::env::temp_dir().join(format!("mcg-browser-sessions-{}", Uuid::new_v4())),
        )?);
        let computer_sessions = Arc::new(ComputerUseManager::new());
        let auth = LocalAuth::from_token(token)?;
        let provider_credentials: Arc<dyn ProviderCredentialStore> =
            Arc::new(MemoryProviderCredentialStore::default());
        let mcp_credentials: Arc<dyn McpCredentialStore> =
            Arc::new(MemoryMcpCredentialStore::default());
        Ok(Self::assemble(
            config,
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
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            browser_config,
            device_config,
        ))
    }

    #[allow(clippy::too_many_arguments)] // Centralizes all recovered state and injected secure boundaries.
    fn assemble(
        config: AppConfig,
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
        platform_capabilities: PlatformCapabilitySnapshot,
        browser_sessions: Arc<PersistentBrowserManager>,
        computer_sessions: Arc<ComputerUseManager>,
        provider_credentials: Arc<dyn ProviderCredentialStore>,
        mcp_credentials: Arc<dyn McpCredentialStore>,
        browser_config: BrowserAuthorizationConfig,
        device_config: DeviceAuthorizationConfig,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        Self {
            config,
            store: Arc::new(Mutex::new(store)),
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
            platform_capabilities,
            browser_sessions,
            computer_sessions,
            provider_credentials,
            mcp_credentials,
            provider_registry: Arc::new(tokio::sync::RwLock::new(ProviderRegistry::default())),
            oauth_state: OAuthRuntimeState::new(browser_config, device_config),
        }
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
        RuntimeStatus {
            service: "mcg-daemon".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            state: RuntimeState::Ready,
            advanced_mode: self.config.advanced_mode,
            shadow_experiments: self.config.shadow_experiments,
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
        }
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
                preset: self.config.autonomy,
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
                preset: self.config.autonomy,
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
                preset: self.config.autonomy,
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
                preset: self.config.autonomy,
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
        }
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
        if !self.config.shadow_experiments {
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
        if self.config.autonomy == AutonomyPreset::Guided
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
            | SuccessCondition::PageRenders { url }
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
        }
        let control = self.tool_control();
        let broker_preset =
            if self.config.autonomy == AutonomyPreset::Guided && checkpoint.batch_approved {
                AutonomyPreset::Balanced
            } else {
                self.config.autonomy
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
            } else if let SuccessCondition::PageRenders { url } = condition {
                ToolRequestKind::BrowserRender {
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

    /// Atomically reserves and starts an idempotent task/run requested through
    /// an external interface, then returns its complete durable view.
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
        task: &TaskEnvelope,
        genome: &OrganizationGenome,
        run: &AgentRun,
        plan: &WorkPlan,
    ) -> Result<InterfaceAgentRun, RuntimeError> {
        let receipt = self.store()?.create_interface_agent_run(
            interface,
            request_key,
            request_hash,
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
        let project = self
            .project(spec.project_id)?
            .ok_or(StorageError::ProjectNotFound(spec.project_id))?;
        let context = self.assemble_project_context(
            spec.project_id,
            Vec::new(),
            spec.context_token_budget,
            spec.target_path.as_deref(),
        )?;
        let context_json = serde_json::to_vec(&context)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let context_fingerprint = format!("sha256:{:x}", Sha256::digest(context_json));
        let (tools, external_tools) = self.resolve_agent_tools_for_authority(
            role.kind,
            &spec.requested_tools,
            evaluation_only,
        )?;
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
        let (running, events) = self.store()?.dispatch_agent_work(&instance, &lease, now)?;
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
                definitions.push(tool_definition(requested_name)?);
            }
        }
        Ok((definitions, bindings))
    }

    fn resolve_agent_tools_for_authority(
        &self,
        role: RoleKind,
        requested: &[String],
        evaluation_only: bool,
    ) -> Result<(Vec<ToolDefinition>, Vec<ExternalToolBinding>), RuntimeError> {
        if !evaluation_only {
            return self.resolve_agent_tools(role, requested);
        }
        if !requested.is_empty() {
            return Err(StorageError::Conflict(
                "inactive candidate runs cannot receive host or MCP tools".into(),
            )
            .into());
        }
        Ok((Vec::new(), Vec::new()))
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
        let prepared = self.prepare_agent_turn(spec).await?;
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
                preset: self.config.autonomy,
                constitution: OwnerConstitution::secure_defaults(),
                capabilities,
                leases: Arc::new(control),
                redactor: OutputRedactor::default(),
            },
        );
        let confidence = ConfidenceEstimate::new(5_000, 0)
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        self.execute_agent_turn(
            prepared.request,
            &self.provider_gateway(),
            &broker,
            confidence,
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
                preset: self.config.autonomy,
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
            &broker,
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
    Ok(format!(
        "{:x}",
        Sha256::digest(
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
            .map_err(StorageError::from)?,
        )
    ))
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
        .map(|name| tool_definition(name))
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

#[allow(clippy::too_many_lines)] // One exhaustive schema registry is easier to audit than scattered schemas.
fn tool_definition(name: &str) -> Result<ToolDefinition, StorageError> {
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
        _ => {
            return Err(StorageError::InvalidData(format!(
                "tool {name} has no runtime schema"
            )));
        }
    };
    Ok(ToolDefinition {
        name: name.to_owned(),
        description: Some(format!("Governed {name} operation")),
        parameters,
        strict: Some(true),
    })
}

fn runtime_tool_error(error: ToolError) -> RuntimeError {
    AgentTurnError::Tool(error).into()
}

fn objective_command(condition: &SuccessCondition) -> Option<&str> {
    match condition {
        SuccessCondition::Build { command }
        | SuccessCondition::Test { command }
        | SuccessCondition::StaticAnalysis { command } => Some(command),
        SuccessCondition::Custom { description } => objective_custom_command(description),
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
        } else if let SuccessCondition::PageRenders { url } = condition {
            Some(format!("Page renders at {url}"))
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
        SuccessCondition::HttpResponds { .. } => "governed_http_status",
        SuccessCondition::PageRenders { .. } => "governed_browser_render",
        SuccessCondition::ElementExists { .. } => "governed_browser_element",
        SuccessCondition::BrowserWorkflow { .. } => "governed_browser_workflow",
        SuccessCondition::ScreenshotMatches { .. } => "governed_screenshot_comparison",
        SuccessCondition::FileExists { .. } => "governed_file_artifact",
        SuccessCondition::ResearchSources { .. } => "governed_research_sources",
        _ => "governed_process_exit",
    }
}

fn objective_outcome_passed(condition: &SuccessCondition, outcome: &ToolOutcome) -> bool {
    if let SuccessCondition::HttpResponds { status, .. } = condition {
        return observed_http_status(outcome) == Some(*status);
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
    outcome.status == ToolOutcomeStatus::Succeeded
}

fn objective_evidence_artifact<'a>(
    condition: &SuccessCondition,
    outcome: &'a ToolOutcome,
) -> Option<&'a mcg_tools::ArtifactReference> {
    if matches!(condition, SuccessCondition::BrowserWorkflow { .. }) {
        return outcome.artifacts.get(2);
    }
    if matches!(
        condition,
        SuccessCondition::PageRenders { .. }
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
    if matches!(
        condition,
        SuccessCondition::PageRenders { .. }
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
    if matches!(executable, "npm" | "npx" | "pnpm" | "yarn" | "bun") {
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
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentTurnRequest, ExecutivePlanner, LeaseBudget, PlannedWork,
        PlanningRequest, RoleKind, SuccessCondition, TaskComplexity,
    };
    use mcg_domain::{AgentInstanceId, AgentLeaseId, EvaluationId, RunId, TaskBudget};
    use mcg_policy::{CapabilityPolicy, OwnerConstitution};
    use mcg_projects::{ContextAssembly, ContextCategory, ContextItem};
    use mcg_providers::{
        CONTRACT_VERSION, ContentPart, MessageRole, ModelCapabilities, ModelDescriptor,
        ModelResponse, OutputItem, ProviderError, ProviderModelRef, RouteCandidate, RoutingPolicy,
        ToolDefinition, Usage,
    };
    use mcg_tools::{
        BrokerControls, OutputRedactor, TOOL_SCHEMA_VERSION, ToolBroker, ToolExecutor, ToolOutcome,
        ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
    };

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn local_auth_verifies_without_exposing_token_in_debug() {
        let auth = LocalAuth::from_token(TEST_TOKEN.to_owned()).expect("valid token");
        assert!(auth.verifies(TEST_TOKEN));
        assert!(!auth.verifies("wrong-token"));
        assert!(!format!("{auth:?}").contains(TEST_TOKEN));
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
    fn committed_task_is_immediately_readable() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime should open");
        let task = TaskEnvelope::new("persist through runtime");
        runtime.create_task(&task).expect("task should persist");

        assert_eq!(runtime.task(task.id).expect("task should read"), Some(task));
    }

    #[test]
    fn inactive_candidate_authority_never_resolves_host_or_mcp_tools() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime");
        let (tools, external) = runtime
            .resolve_agent_tools_for_authority(RoleKind::Coder, &[], true)
            .expect("evaluation-only empty tool plane");
        assert!(tools.is_empty());
        assert!(external.is_empty());
        assert!(
            runtime
                .resolve_agent_tools_for_authority(RoleKind::Coder, &["fs.read".into()], true)
                .is_err()
        );
        assert!(
            !runtime
                .resolve_agent_tools_for_authority(RoleKind::Coder, &[], false)
                .expect("active role defaults")
                .0
                .is_empty()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One contiguous restart/exactly-once evidence scenario.
    fn pending_evaluations_are_supervised_once_after_restart_including_empty_findings() {
        let directory = tempfile::tempdir().expect("data directory");
        let config = AppConfig::default().with_data_directory(directory.path());
        let now = Utc::now();
        let run_id = RunId::new();
        let baseline_revision_id = OrganizationRevisionId::new();
        let failed_id = EvaluationId::new();
        let passed_id = EvaluationId::new();
        {
            let runtime = AppRuntime::open(config.clone()).expect("runtime");
            let task = TaskEnvelope::new("exercise automatic supervision");
            runtime.create_task(&task).expect("task");
            let genome = OrganizationGenome::consumer_default(now);
            let mut genome = genome;
            genome.revision_id = baseline_revision_id;
            runtime
                .record_organization_revision(&genome)
                .expect("organization");
            let condition = SuccessCondition::Custom {
                description: "objective evidence exists".into(),
            };
            let planning = PlanningRequest {
                run_id,
                task: task.clone(),
                complexity: TaskComplexity::Atomic,
                work: vec![PlannedWork::atomic(
                    "exercise supervision",
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
                conditions: vec![ConditionResult {
                    condition: condition.clone(),
                    passed: outcome == EvaluationOutcome::Passed,
                    detail: "durable fixture evidence".into(),
                }],
                outcome,
                confidence: ConfidenceEstimate::new(confidence, 1).expect("confidence"),
                regressions: if outcome == EvaluationOutcome::Failed {
                    vec!["seeded accessibility regression".into()]
                } else {
                    Vec::new()
                },
                evidence: vec![EvaluationEvidence {
                    method: "fixture".into(),
                    artifact_sha256: Some("a".repeat(64)),
                    event_sequence: Some(1),
                    detail: "objective fixture".into(),
                }],
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

        let reopened = AppRuntime::open(config).expect("second reopen");
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
            ExperimentState::Ready
        );
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
    #[allow(clippy::too_many_lines)] // Full runtime/provider/broker/storage boundary fixture.
    async fn completed_agent_turn_commits_usage_result_and_terminal_authority_atomically() {
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
        let store = runtime.store().expect("store");
        assert_eq!(
            store.agent_turn_result(lease.id).expect("result"),
            Some(commit.result.clone())
        );
        assert_eq!(store.agent_usage(run.id).expect("usage").len(), 3);
        drop(store);
        assert!(
            runtime
                .task_events(task.id)
                .expect("events")
                .iter()
                .any(|event| event.kind == mcg_events::EventKind::AgentTurnCompleted)
        );
    }
}
