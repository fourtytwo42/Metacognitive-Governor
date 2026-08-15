use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
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

    /// Streams immutable evidence into the content-addressed store.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or atomically storing the artifact fails, or
    /// if the stream exceeds `max_bytes`.
    pub fn capture_reader(
        &self,
        mut reader: impl Read,
        max_bytes: u64,
        media_type: impl Into<String>,
    ) -> Result<ArtifactReference, ToolError> {
        fs::create_dir_all(&self.root).map_err(|error| {
            ToolError::Execution(format!("restore artifact store root: {error}"))
        })?;
        let temporary = self
            .root
            .join(format!(".incoming.{}.tmp", ArtifactId::new()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            let mut digest = Sha256::new();
            let mut size_bytes = 0_u64;
            let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
            loop {
                let read = reader
                    .read(&mut buffer)
                    .map_err(|error| ToolError::Execution(error.to_string()))?;
                if read == 0 {
                    break;
                }
                size_bytes = size_bytes
                    .checked_add(
                        u64::try_from(read)
                            .map_err(|error| ToolError::Execution(error.to_string()))?,
                    )
                    .ok_or_else(|| ToolError::Execution("artifact size overflow".into()))?;
                if size_bytes > max_bytes {
                    return Err(ToolError::Unavailable(format!(
                        "file exceeds the {max_bytes}-byte artifact evidence limit"
                    )));
                }
                digest.update(&buffer[..read]);
                file.write_all(&buffer[..read])
                    .map_err(|error| ToolError::Execution(error.to_string()))?;
            }
            file.sync_all()
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            drop(file);

            let sha256 = format!("{:x}", digest.finalize());
            let shard = &sha256[..2];
            let directory = self.root.join(shard);
            fs::create_dir_all(&directory)
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            let destination = directory.join(&sha256);
            if destination.exists() {
                fs::remove_file(&temporary)
                    .map_err(|error| ToolError::Execution(error.to_string()))?;
            } else if let Err(error) = fs::rename(&temporary, &destination) {
                if destination.exists() {
                    fs::remove_file(&temporary)
                        .map_err(|error| ToolError::Execution(error.to_string()))?;
                } else {
                    return Err(ToolError::Execution(error.to_string()));
                }
            }
            Ok(ArtifactReference {
                id: ArtifactId::new(),
                sha256: sha256.clone(),
                media_type: media_type.into(),
                size_bytes,
                relative_path: format!("{shard}/{sha256}"),
            })
        })();
        match result {
            Ok(artifact) => Ok(artifact),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(match error {
                    ToolError::Unavailable(_) => error,
                    _ => ToolError::Execution(format!("capture streamed artifact: {error}")),
                })
            }
        }
    }

    /// Reopens and verifies one content-addressed artifact reference.
    ///
    /// # Errors
    ///
    /// Returns an error when the reference is malformed, escapes the store,
    /// traverses a symbolic link, exceeds `max_bytes`, or no longer matches its
    /// recorded size and SHA-256 digest.
    pub fn verify_reference(
        &self,
        artifact: &ArtifactReference,
        max_bytes: u64,
    ) -> Result<(), ToolError> {
        self.read_verified_reference(artifact, max_bytes).map(drop)
    }

    /// Reads one artifact only after validating its content-addressed reference.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::verify_reference`].
    pub fn read_verified_reference(
        &self,
        artifact: &ArtifactReference,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ToolError> {
        let digest = artifact.sha256.to_ascii_lowercase();
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ToolError::Invalid(
                "artifact digest must contain exactly 64 hexadecimal characters".into(),
            ));
        }
        let expected_relative = format!("{}/{digest}", &digest[..2]);
        if artifact.relative_path != expected_relative {
            return Err(ToolError::Invalid(
                "artifact path does not match its content digest".into(),
            ));
        }
        if artifact.size_bytes > max_bytes {
            return Err(ToolError::Unavailable(format!(
                "artifact exceeds the {max_bytes}-byte verification limit"
            )));
        }
        let shard = self.root.join(&digest[..2]);
        let shard_metadata = fs::symlink_metadata(&shard)
            .map_err(|error| ToolError::Execution(format!("inspect artifact shard: {error}")))?;
        if shard_metadata.file_type().is_symlink() || !shard_metadata.is_dir() {
            return Err(ToolError::Denied(
                "artifact shard must be a real directory".into(),
            ));
        }
        let path = shard.join(&digest);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| ToolError::Execution(format!("inspect artifact: {error}")))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ToolError::Denied(
                "artifact evidence must be a regular file".into(),
            ));
        }
        if metadata.len() != artifact.size_bytes || metadata.len() > max_bytes {
            return Err(ToolError::Execution(
                "artifact size no longer matches its immutable reference".into(),
            ));
        }
        let mut file = fs::File::open(&path)
            .map_err(|error| ToolError::Execution(format!("open artifact: {error}")))?;
        let mut observed = Sha256::new();
        let mut read_bytes = 0_u64;
        let mut bytes = Vec::with_capacity(
            usize::try_from(artifact.size_bytes)
                .map_err(|error| ToolError::Execution(error.to_string()))?,
        );
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| ToolError::Execution(format!("read artifact: {error}")))?;
            if read == 0 {
                break;
            }
            read_bytes = read_bytes
                .checked_add(
                    u64::try_from(read).map_err(|error| ToolError::Execution(error.to_string()))?,
                )
                .ok_or_else(|| ToolError::Execution("artifact size overflow".into()))?;
            if read_bytes > max_bytes {
                return Err(ToolError::Unavailable(format!(
                    "artifact exceeds the {max_bytes}-byte verification limit"
                )));
            }
            observed.update(&buffer[..read]);
            bytes.extend_from_slice(&buffer[..read]);
        }
        if format!("{:x}", observed.finalize()) != digest {
            return Err(ToolError::Execution(
                "artifact content no longer matches its immutable digest".into(),
            ));
        }
        Ok(bytes)
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
    fn artifact_reference_verification_rejects_forgery_tampering_and_size_overflow() {
        let root = tempdir().expect("tempdir");
        let store = ArtifactStore::open(root.path()).expect("store");
        let artifact = store
            .capture(b"immutable evidence", "text/plain")
            .expect("artifact");
        store
            .verify_reference(&artifact, artifact.size_bytes)
            .expect("valid reference");
        assert!(matches!(
            store.verify_reference(&artifact, artifact.size_bytes - 1),
            Err(ToolError::Unavailable(_))
        ));

        let mut forged = artifact.clone();
        forged.relative_path = "../outside".into();
        assert!(matches!(
            store.verify_reference(&forged, artifact.size_bytes),
            Err(ToolError::Invalid(_))
        ));

        std::fs::write(
            root.path().join(&artifact.relative_path),
            b"tampered evidence",
        )
        .expect("tamper fixture");
        assert!(matches!(
            store.verify_reference(&artifact, artifact.size_bytes),
            Err(ToolError::Execution(_))
        ));
    }

    #[test]
    fn streamed_artifact_is_bounded_and_deduplicated() {
        let root = tempdir().expect("tempdir");
        let store = ArtifactStore::open(root.path()).expect("store");
        let bytes = vec![0x5a; 160_000];
        let first = store
            .capture_reader(bytes.as_slice(), 160_000, "application/octet-stream")
            .expect("streamed artifact");
        let second = store
            .capture_reader(bytes.as_slice(), 160_000, "application/octet-stream")
            .expect("deduplicated artifact");
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.size_bytes, 160_000);
        assert_eq!(first.relative_path, second.relative_path);
        assert!(matches!(
            store.capture_reader(bytes.as_slice(), 159_999, "application/octet-stream"),
            Err(ToolError::Unavailable(_))
        ));
        assert_eq!(
            std::fs::read_dir(root.path())
                .expect("artifact root")
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains("incoming"))
                .count(),
            0
        );
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
