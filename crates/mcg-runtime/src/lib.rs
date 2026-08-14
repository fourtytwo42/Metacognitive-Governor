//! Dependency wiring, durable task orchestration, and local authentication.

mod oauth;

pub use oauth::*;

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentInstance, AgentLease, AgentLeaseState, AgentModelGateway,
    AgentRun, AgentRunState, AgentState, AgentToolGateway, AgentTurnEngine, AgentTurnError,
    AgentTurnRequest, AgentTurnResult, AgentUsageRecord, ConfidenceEstimate, EvaluationReport,
    ExternalToolBinding, LeaseBudget, OrganizationGenome, OrganizationalMetrics, RoleKind,
    WorkPlan,
};
use mcg_config::AppConfig;
use mcg_domain::{
    AgentInstanceId, AgentLeaseId, ApprovalId, LeaseId, MemoryClaimId, ProjectId, RunId,
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
pub use mcg_storage::AgentTurnCommit;
use mcg_storage::{
    AgentRecoveryReport, AgentTurnDisposition, RecoveryReport, SqliteEventStore, StorageError,
};
use mcg_tools::{
    ApprovalRepository, ApprovalRequest, ApprovalResolution, ApprovalState, BrokerControls,
    ExecutionLease, ExecutionLeaseRepository, FilesystemExecutor, GitExecutor, HostExecutor,
    HttpExecutor, InMemoryMemoryRepository, LeaseState, MemoryExecutor, OutputRedactor,
    ProcessExecutor, RejectingCredentialResolver, SystemExecutor, TOOL_SCHEMA_VERSION,
    ToolAuditRecord, ToolAuditSink, ToolBroker, ToolError, ToolExecutor, ToolOutcome,
    ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
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
    pub recovered_events: u64,
    pub recovered_tasks: u64,
    pub recovered_tool_leases: u64,
    pub recovered_agent_runs: u64,
    pub recovered_agent_instances: u64,
    pub recovered_agent_leases: u64,
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
            recovered_events: self.recovery.events_replayed,
            recovered_tasks: self.recovery.tasks_rebuilt,
            recovered_tool_leases: self.recovered_tool_leases,
            recovered_agent_runs: self.agent_recovery.runs_marked_recovering,
            recovered_agent_instances: self.agent_recovery.instances_expired,
            recovered_agent_leases: self.agent_recovery.leases_expired,
        }
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

    /// Returns the currently discovered in-memory routes available to agent turns.
    pub async fn provider_route_candidates(&self) -> Vec<RouteCandidate> {
        self.provider_registry.read().await.route_candidates()
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
        let (tools, external_tools) = self.resolve_agent_tools(role.kind, &spec.requested_tools)?;
        let routes = self.provider_route_candidates().await;
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
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: Arc::new(
                HttpExecutor::new(credentials, filesystem, &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
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
        if !self
            .approvals(Some(ApprovalState::Approved))?
            .iter()
            .any(|approval| approval.task_id == task_id)
        {
            return Err(StorageError::Conflict(
                "consumer task has no approved tool request".into(),
            )
            .into());
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
        let (tools, external_tools) =
            self.resolve_agent_tools(role.kind, &lease.tool_permissions)?;
        if !external_tools.is_empty() {
            return Err(StorageError::Conflict(
                "external MCP approval continuation requires a persisted turn checkpoint".into(),
            )
            .into());
        }
        let routes = self.provider_route_candidates().await;
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
        let host = HostExecutor {
            filesystem: filesystem.clone(),
            process: process.clone(),
            http: Arc::new(
                HttpExecutor::new(credentials, filesystem, &artifacts)
                    .map_err(runtime_tool_error)?,
            ),
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
                estimated_output_tokens,
                max_model_calls,
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
        let result = match AgentTurnEngine
            .execute(request.clone(), models, tools)
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
        let (approval, event) = self.store()?.resolve_approval(resolution)?;
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

#[derive(Clone)]
pub struct DurableToolControl {
    store: Arc<Mutex<SqliteEventStore>>,
    event_sender: broadcast::Sender<EventEnvelope>,
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
    use axum::{Json, Router, routing::post};
    use mcg_agents::{
        AGENT_RUN_SCHEMA_VERSION, AgentTurnRequest, ExecutivePlanner, LeaseBudget, PlannedWork,
        PlanningRequest, RoleKind, SuccessCondition, TaskComplexity,
    };
    use mcg_domain::{AgentInstanceId, AgentLeaseId, RunId, TaskBudget};
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
    fn committed_task_is_immediately_readable() {
        let runtime = AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
            .expect("runtime should open");
        let task = TaskEnvelope::new("persist through runtime");
        runtime.create_task(&task).expect("task should persist");

        assert_eq!(runtime.task(task.id).expect("task should read"), Some(task));
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
