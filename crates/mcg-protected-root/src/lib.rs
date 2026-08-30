//! Protected Root process bootstrap and authoritative storage cutover.
//!
//! Production executables enter the application through this crate so the
//! existing compatibility API and every `SQLite` mutation share one Root owner.

use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::Utc;
use mcg_config::AppConfig;
use mcg_contracts::{
    AuthenticatedRpcRequest, ContentHash, ContractError, HashAlgorithm, PROTOCOL_V1, RequestId,
    RootStatus, RpcEnvelope, RuntimeIdentity, RuntimeInstanceId, StableTypeId, VersionedPayload,
};
use mcg_platform::{
    PlatformError, ROOT_STATUS_REQUEST_TYPE, ROOT_STATUS_RESPONSE_TYPE, RootIpcSession,
    create_root_ipc_session, read_canonical_frame, remove_root_ipc_session, write_canonical_frame,
};
use mcg_runtime::{AppRuntime, RuntimeError};
use mcg_storage::{
    RootMigrationReceipt, STORAGE_SCHEMA_VERSION, StorageError, acquire_data_directory_lock,
    create_backup_bundle_from_snapshot, inspect_database_before_root,
    snapshot_database_before_root,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Semaphore,
    task::JoinHandle,
};
use tracing::{info, warn};
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum ProtectedRootError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("configured Root API address is invalid: {0}")]
    Address(#[from] std::net::AddrParseError),
}

/// Opens the sole authoritative runtime after a verified ownership-migration backup.
///
/// # Errors
/// Returns a protected-root error when locking, backup, migration, identity
/// initialization, recovery, or executable hashing fails.
pub fn open(config: AppConfig) -> Result<Arc<AppRuntime>, ProtectedRootError> {
    fs::create_dir_all(&config.data_directory)?;
    let data_lock = acquire_data_directory_lock(&config.data_directory)?;
    let migration_receipt = prepare_root_migration(&config)?;
    let runtime = Arc::new(AppRuntime::open_with_data_directory_lock(
        config, data_lock,
    )?);
    let identity = current_runtime_identity()?;
    runtime.initialize_root_ownership(identity, migration_receipt)?;
    Ok(runtime)
}

/// Runs the Root-owned compatibility API until shutdown.
///
/// # Errors
/// Returns startup, migration, recovery, or server failures.
pub async fn serve(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = config.bind_address.parse()?;
    let data_directory = config.data_directory.clone();
    let runtime = open(config)?;

    // Internal role threads are recoverable implementation details, not user-owned tasks.
    match runtime.archive_owned_codex_threads_from_recents().await {
        Ok(archived) if archived > 0 => info!(archived, "archived idle MCG threads"),
        Ok(_) => {}
        Err(error) => warn!(%error, "could not archive idle MCG threads"),
    }

    let session = create_root_ipc_session(&data_directory)?;
    let session_guard = RootSessionGuard {
        data_directory,
        session_id: session.session_id,
    };
    let ipc = start_root_ipc(runtime.clone(), session)?;
    info!(%address, "starting MetaCog Protected Root compatibility API");
    let api_result = mcg_api::serve(address, runtime).await;
    ipc.abort();
    drop(session_guard);
    api_result?;
    info!("MetaCog Protected Root stopped cleanly");
    Ok(())
}

const ROOT_IPC_CONNECTION_LIMIT: usize = 32;
const ROOT_WRITER_QUEUE_CAPACITY: u32 = 64;
const ROOT_REPLAY_WINDOW: usize = 4096;

struct RootSessionGuard {
    data_directory: PathBuf,
    session_id: mcg_contracts::SessionId,
}

impl Drop for RootSessionGuard {
    fn drop(&mut self) {
        remove_root_ipc_session(&self.data_directory, self.session_id);
        #[cfg(unix)]
        {
            let _ = fs::remove_file(mcg_platform::root_ipc_endpoint(&self.data_directory));
        }
    }
}

#[derive(Default)]
struct ReplayWindow {
    order: VecDeque<RequestId>,
    seen: BTreeSet<RequestId>,
}

impl ReplayWindow {
    fn insert(&mut self, request_id: RequestId) -> bool {
        if !self.seen.insert(request_id) {
            return false;
        }
        self.order.push_back(request_id);
        if self.order.len() > ROOT_REPLAY_WINDOW
            && let Some(expired) = self.order.pop_front()
        {
            self.seen.remove(&expired);
        }
        true
    }
}

#[cfg(windows)]
fn start_root_ipc(
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
) -> Result<JoinHandle<()>, ProtectedRootError> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&session.endpoint)?;
    let replay = Arc::new(Mutex::new(ReplayWindow::default()));
    let permits = Arc::new(Semaphore::new(ROOT_IPC_CONNECTION_LIMIT));
    Ok(tokio::spawn(async move {
        let mut next = first;
        loop {
            if let Err(error) = next.connect().await {
                warn!(%error, "Protected Root named-pipe accept failed");
                break;
            }
            let connected = next;
            next = match ServerOptions::new()
                .reject_remote_clients(true)
                .create(&session.endpoint)
            {
                Ok(server) => server,
                Err(error) => {
                    warn!(%error, "Protected Root named-pipe instance creation failed");
                    break;
                }
            };
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let runtime = runtime.clone();
            let session = session.clone();
            let replay = replay.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = handle_root_ipc(connected, runtime, session, replay).await {
                    warn!(%error, "Protected Root IPC request rejected");
                }
            });
        }
    }))
}

#[cfg(unix)]
fn start_root_ipc(
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
) -> Result<JoinHandle<()>, ProtectedRootError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let endpoint = PathBuf::from(&session.endpoint);
    let _ = fs::remove_file(&endpoint);
    let listener = tokio::net::UnixListener::bind(&endpoint)?;
    fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
    let owner_uid = fs::metadata(endpoint.parent().unwrap_or(Path::new("/")))?.uid();
    let replay = Arc::new(Mutex::new(ReplayWindow::default()));
    let permits = Arc::new(Semaphore::new(ROOT_IPC_CONNECTION_LIMIT));
    Ok(tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(error) => {
                    warn!(%error, "Protected Root Unix-socket accept failed");
                    break;
                }
            };
            match stream.peer_cred() {
                Ok(credential) if credential.uid() == owner_uid => {}
                _ => continue,
            }
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let runtime = runtime.clone();
            let session = session.clone();
            let replay = replay.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = handle_root_ipc(stream, runtime, session, replay).await {
                    warn!(%error, "Protected Root IPC request rejected");
                }
            });
        }
    }))
}

async fn handle_root_ipc<S>(
    mut stream: S,
    runtime: Arc<AppRuntime>,
    session: RootIpcSession,
    replay: Arc<Mutex<ReplayWindow>>,
) -> Result<(), ProtectedRootError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut request: AuthenticatedRpcRequest = read_canonical_frame(&mut stream).await?;
    request.envelope.validate()?;
    if !session.verifies(&request.session_capability)
        || request.envelope.session_id != session.session_id
        || request.envelope.message_type.as_str() != ROOT_STATUS_REQUEST_TYPE
        || request.envelope.payload.type_id.as_str() != ROOT_STATUS_REQUEST_TYPE
        || request
            .envelope
            .expires_at
            .is_none_or(|deadline| deadline < Utc::now())
    {
        return Err(PlatformError::InvalidSession.into());
    }
    request.session_capability.zeroize();
    if !replay
        .lock()
        .map_err(|_| PlatformError::InvalidSession)?
        .insert(request.envelope.request_id)
    {
        return Err(PlatformError::InvalidSession.into());
    }
    let ownership = runtime
        .root_ownership()?
        .ok_or_else(|| StorageError::InvalidData("Root ownership is not initialized".into()))?;
    let status = RootStatus {
        schema_version: 1,
        service: "protected-root".to_owned(),
        process_id: std::process::id(),
        protocol_version: PROTOCOL_V1,
        session_id: session.session_id,
        runtime_identity: ownership.runtime_identity.clone(),
        storage_schema_version: STORAGE_SCHEMA_VERSION,
        writer_queue_capacity: ROOT_WRITER_QUEUE_CAPACITY,
        migration_receipt: ownership.migration_receipt,
    };
    let response = RpcEnvelope {
        protocol_version: PROTOCOL_V1,
        request_id: request.envelope.request_id,
        correlation_id: request.envelope.correlation_id,
        session_id: session.session_id,
        caller_instance: ownership.runtime_identity.instance_id,
        message_type: StableTypeId::parse(ROOT_STATUS_RESPONSE_TYPE)?,
        expires_at: Some(Utc::now() + chrono::Duration::seconds(5)),
        payload: VersionedPayload::canonical_v1(
            StableTypeId::parse(ROOT_STATUS_RESPONSE_TYPE)?,
            &serde_json::to_value(status)?,
        )?,
    };
    write_canonical_frame(&mut stream, &response).await?;
    Ok(())
}

fn prepare_root_migration(config: &AppConfig) -> Result<RootMigrationReceipt, ProtectedRootError> {
    let database = config.database_path();
    let now = Utc::now();
    if !database.exists() {
        return Ok(RootMigrationReceipt {
            schema_version: 1,
            from_storage_schema: 0,
            to_storage_schema: STORAGE_SCHEMA_VERSION,
            backup_path: None,
            backup_sha256: None,
            migrated_at: now,
        });
    }

    let report = inspect_database_before_root(&database)?;
    if report.has_root_ownership {
        return Ok(RootMigrationReceipt {
            schema_version: 1,
            from_storage_schema: report.schema_version,
            to_storage_schema: STORAGE_SCHEMA_VERSION,
            backup_path: None,
            backup_sha256: None,
            migrated_at: now,
        });
    }

    let backup_directory = config.data_directory.join("backups");
    fs::create_dir_all(&backup_directory)?;
    let nonce = Uuid::new_v4().simple();
    let snapshot = backup_directory.join(format!(".pre-root-{nonce}.sqlite3"));
    let backup = backup_directory.join(format!(
        "pre-root-schema-{}-{}-{nonce}.mcgb",
        report.schema_version,
        now.format("%Y%m%dT%H%M%SZ")
    ));
    let result = (|| {
        snapshot_database_before_root(&database, &snapshot)?;
        create_backup_bundle_from_snapshot(
            &snapshot,
            &config.data_directory,
            &backup,
            env!("CARGO_PKG_VERSION"),
            now,
        )
    })();
    remove_snapshot_files(&snapshot);
    let summary = result?;
    #[cfg(debug_assertions)]
    if std::env::var_os("MCG_TEST_INTERRUPT_AFTER_ROOT_BACKUP").is_some() {
        return Err(ProtectedRootError::Io(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "test interruption after verified Root migration backup",
        )));
    }
    Ok(RootMigrationReceipt {
        schema_version: 1,
        from_storage_schema: report.schema_version,
        to_storage_schema: STORAGE_SCHEMA_VERSION,
        backup_path: Some(path_relative_to(&config.data_directory, &summary.path)),
        backup_sha256: Some(summary.sha256),
        migrated_at: now,
    })
}

fn current_runtime_identity() -> Result<RuntimeIdentity, ProtectedRootError> {
    let executable = std::env::current_exe()?;
    let bytes = fs::read(executable)?;
    let digest = Sha256::digest(bytes);
    Ok(RuntimeIdentity {
        instance_id: RuntimeInstanceId::new(),
        build_hash: ContentHash {
            algorithm: HashAlgorithm::Sha256,
            domain: "RootExecutable".to_owned(),
            digest_hex: format!("{digest:x}"),
        },
        protocol_version: PROTOCOL_V1,
        started_at: Utc::now(),
    })
}

fn remove_snapshot_files(snapshot: &Path) {
    let mut wal = snapshot.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = snapshot.as_os_str().to_os_string();
    shm.push("-shm");
    for path in [snapshot.to_path_buf(), wal.into(), shm.into()] {
        let _ = fs::remove_file(path);
    }
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcg_contracts::{CorrelationId, SessionId};
    use mcg_platform::{exchange_root_rpc, query_root_status, root_ipc_session_path};
    use mcg_storage::{SqliteEventStore, verify_backup_bundle};

    fn status_request(session: &RootIpcSession) -> AuthenticatedRpcRequest {
        AuthenticatedRpcRequest {
            envelope: RpcEnvelope {
                protocol_version: PROTOCOL_V1,
                request_id: RequestId::new(),
                correlation_id: CorrelationId::new(),
                session_id: session.session_id,
                caller_instance: RuntimeInstanceId::new(),
                message_type: StableTypeId::parse(ROOT_STATUS_REQUEST_TYPE)
                    .expect("status request type"),
                expires_at: Some(Utc::now() + chrono::Duration::seconds(5)),
                payload: VersionedPayload::canonical_v1(
                    StableTypeId::parse(ROOT_STATUS_REQUEST_TYPE).expect("status request type"),
                    &serde_json::json!({}),
                )
                .expect("status payload"),
            },
            session_capability: session.capability.clone(),
        }
    }

    #[test]
    fn existing_database_is_backed_up_and_root_identity_is_restart_stable() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        drop(SqliteEventStore::open(config.database_path()).expect("existing database"));

        let first = open(config.clone()).expect("first Protected Root open");
        let first_ownership = first
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        let backup = first_ownership
            .migration_receipt
            .backup_path
            .as_deref()
            .map(|path| temporary.path().join(path))
            .expect("verified pre-Root backup path");
        let backup_summary = verify_backup_bundle(&backup).expect("verified pre-Root backup");
        assert_eq!(
            first_ownership.migration_receipt.backup_sha256.as_deref(),
            Some(backup_summary.sha256.as_str())
        );
        assert!(
            open(config.clone()).is_err(),
            "a competing Root must not acquire authoritative write ownership"
        );
        let identity = first_ownership.runtime_identity.instance_id;
        drop(first);

        let second = open(config).expect("restarted Protected Root");
        let second_ownership = second
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        assert_eq!(second_ownership.runtime_identity.instance_id, identity);
        assert_eq!(
            second_ownership.migration_receipt,
            first_ownership.migration_receipt
        );
        assert_eq!(
            fs::read_dir(temporary.path().join("backups"))
                .expect("backup directory")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "mcgb"))
                .count(),
            1
        );
    }

    #[test]
    fn interrupted_after_backup_retries_without_mutating_the_source() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        drop(SqliteEventStore::open(config.database_path()).expect("existing database"));

        let interrupted = prepare_root_migration(&config).expect("pre-migration backup");
        let interrupted_backup = temporary.path().join(
            interrupted
                .backup_path
                .as_deref()
                .expect("interrupted migration backup"),
        );
        verify_backup_bundle(&interrupted_backup).expect("verified interrupted backup");
        let before_retry = inspect_database_before_root(&config.database_path())
            .expect("source remains inspectable");
        assert!(!before_retry.has_root_ownership);

        let runtime = open(config).expect("retry Protected Root migration");
        let ownership = runtime
            .root_ownership()
            .expect("ownership query")
            .expect("retry ownership");
        let retry_backup = temporary.path().join(
            ownership
                .migration_receipt
                .backup_path
                .as_deref()
                .expect("retry backup"),
        );
        assert_ne!(retry_backup, interrupted_backup);
        verify_backup_bundle(&retry_backup).expect("verified retry backup");
    }

    #[tokio::test]
    async fn authenticated_local_ipc_reports_the_same_root_identity() {
        let temporary = tempfile::tempdir().expect("temporary Root data directory");
        let config = AppConfig::default().with_data_directory(temporary.path());
        let runtime = open(config).expect("Protected Root open");
        let ownership = runtime
            .root_ownership()
            .expect("ownership query")
            .expect("ownership record");
        let session = create_root_ipc_session(temporary.path()).expect("IPC session");
        let server = start_root_ipc(runtime, session.clone()).expect("IPC server");

        let status = query_root_status(temporary.path())
            .await
            .expect("authenticated status");
        assert_eq!(status.service, "protected-root");
        assert_eq!(status.session_id, session.session_id);
        assert_eq!(
            status.runtime_identity.instance_id,
            ownership.runtime_identity.instance_id
        );
        assert_eq!(status.storage_schema_version, STORAGE_SCHEMA_VERSION);

        let replayed = status_request(&session);
        exchange_root_rpc(&session, &replayed)
            .await
            .expect("first request with unique request ID");
        assert!(exchange_root_rpc(&session, &replayed).await.is_err());

        let mut expired = status_request(&session);
        expired.envelope.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(exchange_root_rpc(&session, &expired).await.is_err());

        let mut unsupported_protocol = status_request(&session);
        unsupported_protocol.envelope.protocol_version = PROTOCOL_V1 + 1;
        assert!(
            exchange_root_rpc(&session, &unsupported_protocol)
                .await
                .is_err()
        );

        let mut stale_session = status_request(&session);
        stale_session.envelope.session_id = SessionId::new();
        assert!(exchange_root_rpc(&session, &stale_session).await.is_err());

        let mut forged = session.clone();
        forged.capability = "0".repeat(64);
        fs::write(
            root_ipc_session_path(temporary.path()),
            serde_json::to_vec(&forged).expect("forged session JSON"),
        )
        .expect("replace client session");
        assert!(query_root_status(temporary.path()).await.is_err());

        server.abort();
        remove_root_ipc_session(temporary.path(), session.session_id);
    }
}
