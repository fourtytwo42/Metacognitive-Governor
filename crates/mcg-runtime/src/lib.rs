//! Dependency wiring, durable task orchestration, and local authentication.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::Duration;
use mcg_config::AppConfig;
use mcg_domain::{
    ApprovalId, LeaseId, TaskEnvelope, TaskId, TaskState, ToolExecutionId, ToolRequestId,
};
use mcg_events::EventEnvelope;
use mcg_storage::{RecoveryReport, SqliteEventStore, StorageError};
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
    #[error("runtime storage lock was poisoned")]
    StorageLock,
    #[error("local authentication token is invalid")]
    InvalidAuthToken,
}

impl RuntimeError {
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::Storage(StorageError::TaskNotFound(_)))
    }

    #[must_use]
    pub const fn is_invalid_request(&self) -> bool {
        matches!(self, Self::Storage(StorageError::InvalidTransition(_)))
    }

    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(self, Self::Storage(StorageError::Conflict(_)))
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
}

impl std::fmt::Debug for AppRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppRuntime")
            .field("config", &self.config)
            .field("auth", &self.auth)
            .field("recovery", &self.recovery)
            .field("recovered_tool_leases", &self.recovered_tool_leases)
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
        Ok(Self::assemble(
            config,
            store,
            auth,
            recovery,
            recovered_tool_leases,
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
        let auth = LocalAuth::from_token(token)?;
        Ok(Self::assemble(
            config,
            store,
            auth,
            recovery,
            recovered_tool_leases,
        ))
    }

    fn assemble(
        config: AppConfig,
        store: SqliteEventStore,
        auth: LocalAuth,
        recovery: RecoveryReport,
        recovered_tool_leases: u64,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        Self {
            config,
            store: Arc::new(Mutex::new(store)),
            event_sender,
            auth,
            recovery,
            recovered_tool_leases,
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use mcg_policy::{CapabilityPolicy, OwnerConstitution};
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
}
