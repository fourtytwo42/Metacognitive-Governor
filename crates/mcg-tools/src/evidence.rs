use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use mcg_domain::{ArtifactId, LeaseId, ToolExecutionId, ToolRequestId};
use sha2::{Digest, Sha256};

use crate::{ArtifactReference, ExecutionLease, LeaseState, TOOL_SCHEMA_VERSION, ToolError};

pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    /// Creates a content-addressed artifact store.
    ///
    /// # Errors
    ///
    /// Returns an error if the root cannot be created or resolved.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ToolError> {
        fs::create_dir_all(root.as_ref())
            .map_err(|error| ToolError::Execution(format!("create artifact store: {error}")))?;
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|error| ToolError::Execution(format!("resolve artifact store: {error}")))?;
        Ok(Self { root })
    }

    /// Captures immutable bytes under their SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the artifact cannot be written atomically.
    pub fn capture(
        &self,
        bytes: &[u8],
        media_type: impl Into<String>,
    ) -> Result<ArtifactReference, ToolError> {
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        let shard = &sha256[..2];
        let directory = self.root.join(shard);
        fs::create_dir_all(&directory)
            .map_err(|error| ToolError::Execution(format!("create artifact shard: {error}")))?;
        let destination = directory.join(&sha256);
        if !destination.exists() {
            Self::write_new(&destination, bytes, &sha256)?;
        }
        Ok(ArtifactReference {
            id: ArtifactId::new(),
            sha256: sha256.clone(),
            media_type: media_type.into(),
            size_bytes: u64::try_from(bytes.len())
                .map_err(|_| ToolError::Execution("artifact is too large".into()))?,
            relative_path: format!("{shard}/{sha256}"),
        })
    }

    fn write_new(destination: &Path, bytes: &[u8], digest: &str) -> Result<(), ToolError> {
        let parent = destination
            .parent()
            .ok_or_else(|| ToolError::Execution("artifact destination has no parent".into()))?;
        let temporary = parent.join(format!(".{digest}.{}.tmp", ArtifactId::new()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, destination)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            if !destination.exists() {
                return Err(ToolError::Execution(format!("capture artifact: {error}")));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct LeaseRegistry {
    leases: Arc<Mutex<Vec<ExecutionLease>>>,
}

#[async_trait]
pub trait ExecutionLeaseRepository: Send + Sync {
    async fn acquire(
        &self,
        request_id: ToolRequestId,
        execution_id: ToolExecutionId,
        duration: Duration,
    ) -> Result<ExecutionLease, ToolError>;
    async fn finish(&self, lease_id: LeaseId, state: LeaseState) -> Result<(), ToolError>;
}

impl LeaseRegistry {
    /// Acquires one active execution lease for a request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid duration or a duplicate active lease.
    pub fn acquire(
        &self,
        request_id: ToolRequestId,
        execution_id: ToolExecutionId,
        duration: Duration,
    ) -> Result<ExecutionLease, ToolError> {
        if duration <= Duration::zero() {
            return Err(ToolError::Lease("lease duration must be positive".into()));
        }
        let now = Utc::now();
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| ToolError::Lease("lease registry lock poisoned".into()))?;
        for lease in leases
            .iter_mut()
            .filter(|lease| lease.tool_request_id == request_id)
        {
            if lease.state == LeaseState::Active && lease.expires_at > now {
                return Err(ToolError::Lease(
                    "request already has an active lease".into(),
                ));
            }
            if lease.state == LeaseState::Active {
                lease.state = LeaseState::Expired;
            }
        }
        let lease = ExecutionLease {
            schema_version: TOOL_SCHEMA_VERSION,
            id: LeaseId::new(),
            execution_id,
            tool_request_id: request_id,
            state: LeaseState::Active,
            acquired_at: now,
            expires_at: now + duration,
            heartbeat_at: now,
        };
        leases.push(lease.clone());
        Ok(lease)
    }

    /// Transitions an active lease to a terminal state.
    ///
    /// # Errors
    ///
    /// Returns an error when the lease is absent, expired, or already terminal.
    pub fn finish(&self, lease_id: LeaseId, state: LeaseState) -> Result<(), ToolError> {
        if state == LeaseState::Active {
            return Err(ToolError::Lease("finish requires a terminal state".into()));
        }
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| ToolError::Lease("lease registry lock poisoned".into()))?;
        let lease = leases
            .iter_mut()
            .find(|lease| lease.id == lease_id)
            .ok_or_else(|| ToolError::Lease("lease was not found".into()))?;
        if lease.state != LeaseState::Active || lease.expires_at <= Utc::now() {
            return Err(ToolError::Lease("lease is not active".into()));
        }
        lease.state = state;
        lease.heartbeat_at = Utc::now();
        Ok(())
    }
}

#[async_trait]
impl ExecutionLeaseRepository for LeaseRegistry {
    async fn acquire(
        &self,
        request_id: ToolRequestId,
        execution_id: ToolExecutionId,
        duration: Duration,
    ) -> Result<ExecutionLease, ToolError> {
        Self::acquire(self, request_id, execution_id, duration)
    }

    async fn finish(&self, lease_id: LeaseId, state: LeaseState) -> Result<(), ToolError> {
        Self::finish(self, lease_id, state)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn artifact_capture_is_content_addressed_and_idempotent() {
        let root = tempdir().expect("tempdir");
        let store = ArtifactStore::open(root.path()).expect("store");
        let first = store.capture(b"evidence", "text/plain").expect("first");
        let second = store.capture(b"evidence", "text/plain").expect("second");
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.relative_path, second.relative_path);
        assert!(root.path().join(first.relative_path).is_file());
    }

    #[test]
    fn duplicate_active_lease_is_rejected() {
        let registry = LeaseRegistry::default();
        let request_id = ToolRequestId::new();
        registry
            .acquire(request_id, ToolExecutionId::new(), Duration::seconds(30))
            .expect("first lease");
        assert!(
            registry
                .acquire(request_id, ToolExecutionId::new(), Duration::seconds(30))
                .is_err()
        );
    }
}
