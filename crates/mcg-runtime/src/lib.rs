//! Dependency wiring, durable task orchestration, and local authentication.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use mcg_agents::{
    AgentInstance, AgentLease, AgentModelGateway, AgentRun, AgentRunState, AgentState,
    AgentToolGateway, AgentTurnEngine, AgentTurnError, AgentTurnRequest, AgentTurnResult,
    AgentUsageRecord, ConfidenceEstimate, EvaluationReport, OrganizationGenome,
    OrganizationalMetrics, WorkPlan,
};
use mcg_config::AppConfig;
use mcg_domain::{
    ApprovalId, LeaseId, MemoryClaimId, ProjectId, TaskEnvelope, TaskId, TaskState,
    ToolExecutionId, ToolRequestId,
};
use mcg_events::EventEnvelope;
use mcg_projects::{
    ClaimProvenance, ContextAssembler, ContextAssembly, ContextCategory, ContextItem, MemoryClaim,
    MemoryError, MemoryLifecycle, MemoryLifecycleAction, ProjectDiscoverer, ProjectDiscoveryError,
    ProjectModel, SnapshotInvalidation, render_managed_memory,
};
use mcg_storage::{
    AgentRecoveryReport, AgentTurnCommit, AgentTurnDisposition, RecoveryReport, SqliteEventStore,
    StorageError,
};
use mcg_tools::{
    ApprovalRepository, ApprovalRequest, ApprovalResolution, ApprovalState, ExecutionLease,
    ExecutionLeaseRepository, LeaseState, ToolAuditRecord, ToolAuditSink, ToolError,
};
use serde::{Deserialize, Serialize};
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
        Ok(Self::assemble(
            config,
            store,
            auth,
            recovery,
            recovered_tool_leases,
            agent_recovery,
        ))
    }

    /// Creates an isolated runtime for tests and embedded deterministic fixtures.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or the supplied token is invalid.
    pub fn open_in_memory(config: AppConfig, token: String) -> Result<Self, RuntimeError> {
        let mut store = SqliteEventStore::open_in_memory()?;
        let recovery = store.recover_projections()?;
        let recovered_tool_leases = store.recover_tool_leases()?;
        let agent_recovery = store.recover_agent_work(Utc::now())?;
        let auth = LocalAuth::from_token(token)?;
        Ok(Self::assemble(
            config,
            store,
            auth,
            recovery,
            recovered_tool_leases,
            agent_recovery,
        ))
    }

    fn assemble(
        config: AppConfig,
        store: SqliteEventStore,
        auth: LocalAuth,
        recovery: RecoveryReport,
        recovered_tool_leases: u64,
        agent_recovery: AgentRecoveryReport,
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
