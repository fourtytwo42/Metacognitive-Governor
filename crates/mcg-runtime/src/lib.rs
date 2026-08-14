//! Dependency wiring, durable task orchestration, and local authentication.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Mutex, MutexGuard},
};

use mcg_config::AppConfig;
use mcg_domain::{TaskEnvelope, TaskId, TaskState};
use mcg_events::EventEnvelope;
use mcg_storage::{RecoveryReport, SqliteEventStore, StorageError};
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
    store: Mutex<SqliteEventStore>,
    event_sender: broadcast::Sender<EventEnvelope>,
    auth: LocalAuth,
    recovery: RecoveryReport,
}

impl std::fmt::Debug for AppRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppRuntime")
            .field("config", &self.config)
            .field("auth", &self.auth)
            .field("recovery", &self.recovery)
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
        Ok(Self::assemble(config, store, auth, recovery))
    }

    /// Creates an isolated runtime for tests and embedded deterministic fixtures.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or the supplied token is invalid.
    pub fn open_in_memory(config: AppConfig, token: String) -> Result<Self, RuntimeError> {
        let mut store = SqliteEventStore::open_in_memory()?;
        let recovery = store.recover_projections()?;
        let auth = LocalAuth::from_token(token)?;
        Ok(Self::assemble(config, store, auth, recovery))
    }

    fn assemble(
        config: AppConfig,
        store: SqliteEventStore,
        auth: LocalAuth,
        recovery: RecoveryReport,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        Self {
            config,
            store: Mutex::new(store),
            event_sender,
            auth,
            recovery,
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

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_sender.subscribe()
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
    use super::*;

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
}
