use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use mcg_domain::{
    PluginInstallation, PluginInvocationRecord, PluginInvocationState, PluginRevision,
};
use mcg_mcp::{McpAuthMode, McpConnection, McpCredential, McpCredentialStore};
use mcg_providers::{
    ProviderAuthMode, ProviderConnection, ProviderCredential, ProviderCredentialStore,
    ProviderSecretSlot, clear_provider_credentials, save_provider_credentials,
};
use rusqlite::{Connection, MAIN_DB, OpenFlags, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    STORAGE_SCHEMA_VERSION, SqliteEventStore, StorageError,
    backup_secrets::{
        BACKUP_SECRET_ENTRY, BackupMcpSecret, BackupPassphrase, BackupProviderSecret,
        BackupSecretEncryptionMetadata, BackupSecretSnapshot, MAX_SECRET_CIPHERTEXT_BYTES,
        decrypt_backup_secrets, encrypt_backup_secrets, validate_backup_secret_metadata,
    },
    plugins::{
        validate_plugin_backup_head, validate_plugin_invocation_backup_record,
        validate_plugin_revision,
    },
};

pub const BACKUP_BUNDLE_SCHEMA_VERSION: u16 = 2;
const MIN_BACKUP_BUNDLE_SCHEMA_VERSION: u16 = 1;
const MAGIC: &[u8; 8] = b"MCGB0001";
const DATABASE_ENTRY: &str = "state/mcg.sqlite3";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 100_001;
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 128 * 1024 * 1024 * 1024;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub struct DataDirectoryLock {
    _file: File,
}

/// Acquires the exclusive process lock protecting one MCG data directory.
///
/// # Errors
/// Returns an I/O error when the lock file cannot be opened, or a conflict when
/// another daemon or restore already owns the directory.
pub fn acquire_data_directory_lock(
    data_directory: &Path,
) -> Result<DataDirectoryLock, StorageError> {
    fs::create_dir_all(data_directory)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(data_directory.join(".mcg-data.lock"))?;
    file.try_lock_exclusive().map_err(|error| {
        StorageError::Conflict(format!(
            "MCG data directory is in use; stop the daemon before restore: {error}"
        ))
    })?;
    Ok(DataDirectoryLock { _file: file })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupBundleEntry {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupBundleManifest {
    pub schema_version: u16,
    pub application_version: String,
    pub created_at: DateTime<Utc>,
    pub secrets_included: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_encryption: Option<BackupSecretEncryptionMetadata>,
    pub entries: Vec<BackupBundleEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupBundleSummary {
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub manifest: BackupBundleManifest,
}

/// Creates and verifies a portable, secret-free backup from a live event store.
///
/// # Errors
/// Returns storage, filesystem, validation, or integrity failures.
pub fn create_backup_bundle(
    store: &SqliteEventStore,
    data_directory: &Path,
    destination: &Path,
    application_version: &str,
    now: DateTime<Utc>,
) -> Result<BackupBundleSummary, StorageError> {
    let parent = destination.parent().ok_or_else(|| {
        StorageError::InvalidData("backup destination has no parent directory".into())
    })?;
    fs::create_dir_all(parent)?;
    let snapshot = parent.join(format!(".mcgb-{}.sqlite3", Uuid::new_v4()));
    let result = (|| {
        store.snapshot_database(&snapshot)?;
        create_backup_bundle_from_snapshot(
            &snapshot,
            data_directory,
            destination,
            application_version,
            now,
        )
    })();
    let cleanup = remove_online_snapshot_files(&snapshot);
    match result {
        Ok(summary) => {
            cleanup?;
            Ok(summary)
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
            Err(error) => return Err(StorageError::Io(error)),
        }
    }
    Ok(())
}

impl SqliteEventStore {
    /// Writes one consistent online `SQLite` snapshot for a portable backup.
    ///
    /// # Errors
    /// Returns `SQLite` or destination I/O failures.
    pub fn snapshot_database(&self, destination: &Path) -> Result<(), StorageError> {
        self.connection.backup(MAIN_DB, destination, None)?;
        Ok(())
    }
}

/// Creates and verifies a portable bundle from an already-consistent snapshot.
///
/// # Errors
/// Returns filesystem, validation, or integrity failures.
pub fn create_backup_bundle_from_snapshot(
    snapshot: &Path,
    data_directory: &Path,
    destination: &Path,
    application_version: &str,
    now: DateTime<Utc>,
) -> Result<BackupBundleSummary, StorageError> {
    create_backup_bundle_from_snapshot_inner(
        snapshot,
        data_directory,
        destination,
        application_version,
        now,
        None,
    )
}

/// Creates an explicit passphrase-encrypted backup containing provider and MCP credentials.
///
/// # Errors
/// Returns filesystem, validation, protected-snapshot, KDF, encryption, or integrity failures.
pub fn create_encrypted_backup_bundle_from_snapshot(
    snapshot: &Path,
    data_directory: &Path,
    destination: &Path,
    application_version: &str,
    now: DateTime<Utc>,
    secrets: &BackupSecretSnapshot,
    passphrase: &BackupPassphrase,
) -> Result<BackupBundleSummary, StorageError> {
    validate_secret_snapshot_against_database(snapshot, secrets)?;
    create_backup_bundle_from_snapshot_inner(
        snapshot,
        data_directory,
        destination,
        application_version,
        now,
        Some((secrets, passphrase)),
    )
}

#[allow(clippy::too_many_lines)] // Keeps the audited create-new/write/verify/publish sequence in one cleanup scope.
fn create_backup_bundle_from_snapshot_inner(
    snapshot: &Path,
    data_directory: &Path,
    destination: &Path,
    application_version: &str,
    now: DateTime<Utc>,
    secret_input: Option<(&BackupSecretSnapshot, &BackupPassphrase)>,
) -> Result<BackupBundleSummary, StorageError> {
    validate_destination(destination)?;
    let destination_parent = destination.parent().ok_or_else(|| {
        StorageError::InvalidData("backup destination has no parent directory".into())
    })?;
    fs::create_dir_all(destination_parent)?;
    let data_root = data_directory.canonicalize()?;
    let destination_root = destination_parent.canonicalize()?;
    if !destination_root.starts_with(&data_root)
        || fs::symlink_metadata(destination_parent)?
            .file_type()
            .is_symlink()
    {
        return Err(StorageError::InvalidData(
            "backup destination must remain inside the MCG data directory".into(),
        ));
    }
    let nonce = Uuid::new_v4();
    let temporary = destination_parent.join(format!(".mcgb-{nonce}.tmp"));
    let result = (|| {
        let mut sources = vec![(DATABASE_ENTRY.to_owned(), snapshot.to_path_buf())];
        sources.extend(artifact_sources(&data_directory.join("artifacts"))?);
        if sources
            .len()
            .saturating_add(usize::from(secret_input.is_some()))
            > MAX_ENTRY_COUNT
        {
            return Err(StorageError::InvalidData(
                "backup contains too many entries".into(),
            ));
        }
        let mut entries = Vec::with_capacity(sources.len());
        for (path, source) in &sources {
            entries.push(inspect_source(path, source)?);
        }
        validate_plugin_catalog_backup(snapshot, &entries)?;
        let (secret_encryption, secret_ciphertext) =
            if let Some((secrets, passphrase)) = secret_input {
                let associated_data = secret_binding_bytes(application_version, now, &entries)?;
                let (metadata, ciphertext) =
                    encrypt_backup_secrets(secrets, passphrase, &associated_data)?;
                entries.push(inspect_bytes(BACKUP_SECRET_ENTRY, &ciphertext)?);
                (Some(metadata), Some(ciphertext))
            } else {
                (None, None)
            };
        let manifest = BackupBundleManifest {
            schema_version: BACKUP_BUNDLE_SCHEMA_VERSION,
            application_version: application_version.to_owned(),
            created_at: now,
            secrets_included: secret_encryption.is_some(),
            secret_encryption,
            entries,
        };
        validate_manifest(&manifest)?;
        let manifest_bytes = serde_json::to_vec(&manifest)?;
        let manifest_len = u32::try_from(manifest_bytes.len()).map_err(|_| {
            StorageError::InvalidData("backup manifest exceeds format bounds".into())
        })?;
        let entry_bytes = manifest.entries.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.size_bytes)
                .ok_or_else(|| StorageError::InvalidData("backup size overflow".into()))
        })?;
        let encoded_bytes = 12_u64
            .checked_add(u64::from(manifest_len))
            .and_then(|value| value.checked_add(entry_bytes))
            .ok_or_else(|| StorageError::InvalidData("backup size overflow".into()))?;
        if encoded_bytes > MAX_BUNDLE_BYTES {
            return Err(StorageError::InvalidData("backup is too large".into()));
        }
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        output.write_all(MAGIC)?;
        output.write_all(&manifest_len.to_be_bytes())?;
        output.write_all(&manifest_bytes)?;
        for ((_, source), entry) in sources.iter().zip(&manifest.entries) {
            copy_verified_source(source, entry, &mut output)?;
        }
        if let Some(ciphertext) = secret_ciphertext.as_deref() {
            let entry = manifest.entries.last().ok_or_else(|| {
                StorageError::InvalidData("encrypted backup entry is missing".into())
            })?;
            copy_verified_bytes(ciphertext, entry, &mut output)?;
        }
        output.sync_all()?;
        drop(output);
        let mut summary = verify_backup_bundle(&temporary)?;
        fs::hard_link(&temporary, destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                StorageError::Conflict(format!(
                    "backup destination already exists: {}",
                    destination.display()
                ))
            } else {
                StorageError::Io(error)
            }
        })?;
        fs::remove_file(&temporary)?;
        sync_parent(destination_parent)?;
        summary.path = destination.to_path_buf();
        Ok(summary)
    })();
    let _ = fs::remove_file(&temporary);
    result
}

/// Fully validates a portable backup without changing application state.
///
/// # Errors
/// Returns filesystem, format, bounds, or digest validation failures.
pub fn verify_backup_bundle(path: &Path) -> Result<BackupBundleSummary, StorageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StorageError::InvalidData(
            "backup bundle must be a regular file".into(),
        ));
    }
    let mut input = File::open(path)?;
    let size_bytes = metadata.len();
    if size_bytes > MAX_BUNDLE_BYTES {
        return Err(StorageError::InvalidData(
            "backup bundle is too large".into(),
        ));
    }
    let (manifest, header_bytes) = read_header(&mut input)?;
    let mut database = tempfile::NamedTempFile::new()?;
    let mut bundle_digest = Sha256::new();
    bundle_digest.update(MAGIC);
    bundle_digest.update(header_bytes);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    for entry in &manifest.entries {
        let mut remaining = entry.size_bytes;
        let mut entry_digest = Sha256::new();
        while remaining > 0 {
            let wanted = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
                .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
            input.read_exact(&mut buffer[..wanted])?;
            entry_digest.update(&buffer[..wanted]);
            bundle_digest.update(&buffer[..wanted]);
            if entry.path == DATABASE_ENTRY {
                database.as_file_mut().write_all(&buffer[..wanted])?;
            }
            remaining -= u64::try_from(wanted)
                .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
        }
        require_digest(entry, &entry_digest.finalize())?;
    }
    let mut trailing = [0_u8; 1];
    if input.read(&mut trailing)? != 0 {
        return Err(StorageError::InvalidData(
            "backup bundle contains trailing bytes".into(),
        ));
    }
    database.as_file_mut().flush()?;
    validate_restored_database(database.path())?;
    validate_plugin_catalog_backup(database.path(), &manifest.entries)?;
    Ok(BackupBundleSummary {
        path: path.to_path_buf(),
        sha256: format!("{:x}", bundle_digest.finalize()),
        size_bytes,
        manifest,
    })
}

/// Verifies, authenticates, and decrypts the explicit credential entry in one bundle.
///
/// # Errors
/// Returns format, integrity, passphrase, authentication, or credential-shape failures.
pub fn decrypt_backup_bundle_secrets(
    path: &Path,
    passphrase: &BackupPassphrase,
) -> Result<(BackupBundleSummary, BackupSecretSnapshot), StorageError> {
    let summary = verify_backup_bundle(path)?;
    let metadata = summary.manifest.secret_encryption.as_ref().ok_or_else(|| {
        StorageError::InvalidData("backup contains no encrypted credentials".into())
    })?;
    let secret_entry = summary.manifest.entries.last().ok_or_else(|| {
        StorageError::InvalidData("backup encrypted credential entry is missing".into())
    })?;
    let base_entries = &summary.manifest.entries[..summary.manifest.entries.len() - 1];
    let associated_data = secret_binding_bytes(
        &summary.manifest.application_version,
        summary.manifest.created_at,
        base_entries,
    )?;
    let mut input = File::open(path)?;
    let (reopened_manifest, _) = read_header(&mut input)?;
    if reopened_manifest != summary.manifest {
        return Err(StorageError::Conflict(
            "backup changed while encrypted credentials were being authenticated".into(),
        ));
    }
    let base_bytes = base_entries.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.size_bytes)
            .ok_or_else(|| StorageError::InvalidData("backup size overflow".into()))
    })?;
    let offset = i64::try_from(base_bytes)
        .map_err(|_| StorageError::InvalidData("backup size overflow".into()))?;
    input.seek(SeekFrom::Current(offset))?;
    let ciphertext_len = usize::try_from(secret_entry.size_bytes)
        .map_err(|_| StorageError::InvalidData("encrypted credential entry is too large".into()))?;
    let mut ciphertext = vec![0_u8; ciphertext_len];
    input.read_exact(&mut ciphertext)?;
    let secrets = decrypt_backup_secrets(metadata, &ciphertext, passphrase, &associated_data)?;
    Ok((summary, secrets))
}

/// Restores a verified bundle while retaining the previous database for rollback.
///
/// # Errors
/// Returns a conflict while the data directory is active, or filesystem,
/// database, schema, format, and integrity failures.
pub fn restore_backup_bundle(
    bundle: &Path,
    data_directory: &Path,
    now: DateTime<Utc>,
) -> Result<BackupBundleSummary, StorageError> {
    let _data_lock = acquire_data_directory_lock(data_directory)?;
    let summary = verify_backup_bundle(bundle)?;
    if summary.manifest.secrets_included {
        return Err(StorageError::InvalidData(
            "encrypted credentials require explicit passphrase restore".into(),
        ));
    }
    fs::create_dir_all(data_directory)?;
    let nonce = Uuid::new_v4();
    let staging = data_directory.join(format!(".restore-{nonce}"));
    fs::create_dir(&staging)?;
    let result = (|| {
        extract_bundle(bundle, &staging, &summary.manifest)?;
        let restored_database =
            staging.join(DATABASE_ENTRY.replace('/', std::path::MAIN_SEPARATOR_STR));
        validate_restored_database(&restored_database)?;
        disable_restored_plugins(&restored_database, now)?;
        merge_restored_artifacts(
            &staging.join("artifacts"),
            &data_directory.join("artifacts"),
        )?;
        install_restored_database(&restored_database, data_directory, now)?;
        Ok(summary)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Restores one explicitly passphrase-encrypted bundle and its protected credentials.
///
/// The daemon must be stopped. The passphrase and complete bundle are authenticated before any
/// destination state changes. Credential identities and authentication modes must match the
/// staged database exactly. Protected-store writes are rolled back if any credential, artifact,
/// plugin-safety, or database installation step fails.
///
/// # Errors
/// Returns a conflict while the data directory is active, or authentication, protected-store,
/// filesystem, database, schema, format, integrity, or rollback failures.
pub fn restore_encrypted_backup_bundle<P, M>(
    bundle: &Path,
    data_directory: &Path,
    now: DateTime<Utc>,
    passphrase: &BackupPassphrase,
    provider_store: &P,
    mcp_store: &M,
) -> Result<BackupBundleSummary, StorageError>
where
    P: ProviderCredentialStore + ?Sized,
    M: McpCredentialStore + ?Sized,
{
    let _data_lock = acquire_data_directory_lock(data_directory)?;
    let (summary, secrets) = decrypt_backup_bundle_secrets(bundle, passphrase)?;
    fs::create_dir_all(data_directory)?;
    let nonce = Uuid::new_v4();
    let staging = data_directory.join(format!(".restore-{nonce}"));
    fs::create_dir(&staging)?;
    let result = (|| {
        extract_bundle(bundle, &staging, &summary.manifest)?;
        let restored_database =
            staging.join(DATABASE_ENTRY.replace('/', std::path::MAIN_SEPARATOR_STR));
        validate_restored_database(&restored_database)?;
        validate_plugin_catalog_backup(&restored_database, &summary.manifest.entries)?;
        validate_secret_snapshot_against_database(&restored_database, &secrets)?;
        disable_restored_plugins(&restored_database, now)?;

        let rollback = capture_credential_rollback(&secrets, provider_store, mcp_store)?;
        if let Err(error) = install_secret_snapshot(&secrets, provider_store, mcp_store) {
            return match restore_credential_rollback(&rollback, provider_store, mcp_store) {
                Ok(()) => Err(error),
                Err(()) => Err(StorageError::Conflict(
                    "credential import failed and protected-store rollback also failed".into(),
                )),
            };
        }

        let install_result = (|| {
            merge_restored_artifacts(
                &staging.join("artifacts"),
                &data_directory.join("artifacts"),
            )?;
            install_restored_database(&restored_database, data_directory, now)?;
            Ok(summary)
        })();
        match install_result {
            Ok(summary) => Ok(summary),
            Err(error) => match restore_credential_rollback(&rollback, provider_store, mcp_store) {
                Ok(()) => Err(error),
                Err(()) => Err(StorageError::Conflict(
                    "backup restore failed and protected-store rollback also failed".into(),
                )),
            },
        }
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

struct ProviderCredentialRollback {
    connection_id: String,
    primary: Option<ProviderCredential>,
    oauth_id_token: Option<ProviderCredential>,
    oauth_refresh_token: Option<ProviderCredential>,
}

struct McpCredentialRollback {
    connection_id: String,
    bearer: Option<McpCredential>,
}

struct CredentialRollback {
    providers: Vec<ProviderCredentialRollback>,
    mcp_connections: Vec<McpCredentialRollback>,
}

fn capture_credential_rollback<P, M>(
    secrets: &BackupSecretSnapshot,
    provider_store: &P,
    mcp_store: &M,
) -> Result<CredentialRollback, StorageError>
where
    P: ProviderCredentialStore + ?Sized,
    M: McpCredentialStore + ?Sized,
{
    let providers = secrets
        .providers
        .iter()
        .map(|secret| {
            let connection_id = provider_secret_connection_id(secret);
            Ok(ProviderCredentialRollback {
                connection_id: connection_id.to_owned(),
                primary: provider_store
                    .load(connection_id, ProviderSecretSlot::Primary)
                    .map_err(protected_store_error)?,
                oauth_id_token: provider_store
                    .load(connection_id, ProviderSecretSlot::OAuthIdToken)
                    .map_err(protected_store_error)?,
                oauth_refresh_token: provider_store
                    .load(connection_id, ProviderSecretSlot::OAuthRefreshToken)
                    .map_err(protected_store_error)?,
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    let mcp_connections = secrets
        .mcp_connections
        .iter()
        .map(|secret| {
            let connection_id = mcp_secret_connection_id(secret);
            Ok(McpCredentialRollback {
                connection_id: connection_id.to_owned(),
                bearer: mcp_store
                    .load(connection_id)
                    .map_err(protected_store_error)?,
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    Ok(CredentialRollback {
        providers,
        mcp_connections,
    })
}

fn install_secret_snapshot<P, M>(
    secrets: &BackupSecretSnapshot,
    provider_store: &P,
    mcp_store: &M,
) -> Result<(), StorageError>
where
    P: ProviderCredentialStore + ?Sized,
    M: McpCredentialStore + ?Sized,
{
    for secret in &secrets.providers {
        let connection_id = provider_secret_connection_id(secret);
        clear_provider_credentials(provider_store, connection_id).map_err(protected_store_error)?;
        match secret {
            BackupProviderSecret::None { .. } => {}
            BackupProviderSecret::Primary {
                auth_mode,
                credential,
                ..
            } => save_provider_credentials(
                provider_store,
                connection_id,
                *auth_mode,
                Some(credential),
                None,
            )
            .map_err(protected_store_error)?,
            BackupProviderSecret::OAuth {
                auth_mode, tokens, ..
            } => save_provider_credentials(
                provider_store,
                connection_id,
                *auth_mode,
                None,
                Some(tokens),
            )
            .map_err(protected_store_error)?,
        }
    }
    for secret in &secrets.mcp_connections {
        let connection_id = mcp_secret_connection_id(secret);
        mcp_store
            .delete(connection_id)
            .map_err(protected_store_error)?;
        if let BackupMcpSecret::Bearer { credential, .. } = secret {
            mcp_store
                .save(connection_id, credential)
                .map_err(protected_store_error)?;
        }
    }
    Ok(())
}

fn restore_credential_rollback<P, M>(
    rollback: &CredentialRollback,
    provider_store: &P,
    mcp_store: &M,
) -> Result<(), ()>
where
    P: ProviderCredentialStore + ?Sized,
    M: McpCredentialStore + ?Sized,
{
    let mut failed = false;
    for provider in &rollback.providers {
        for (slot, credential) in [
            (ProviderSecretSlot::Primary, provider.primary.as_ref()),
            (
                ProviderSecretSlot::OAuthIdToken,
                provider.oauth_id_token.as_ref(),
            ),
            (
                ProviderSecretSlot::OAuthRefreshToken,
                provider.oauth_refresh_token.as_ref(),
            ),
        ] {
            let result = credential.map_or_else(
                || provider_store.delete(&provider.connection_id, slot),
                |credential| provider_store.save(&provider.connection_id, slot, credential),
            );
            failed |= result.is_err();
        }
    }
    for connection in &rollback.mcp_connections {
        let result = connection.bearer.as_ref().map_or_else(
            || mcp_store.delete(&connection.connection_id),
            |credential| mcp_store.save(&connection.connection_id, credential),
        );
        failed |= result.is_err();
    }
    if failed { Err(()) } else { Ok(()) }
}

fn validate_secret_snapshot_against_database(
    database: &Path,
    secrets: &BackupSecretSnapshot,
) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if !table_exists(&connection, "provider_connections")?
        || !table_exists(&connection, "mcp_connections")?
    {
        return Err(StorageError::InvalidData(
            "encrypted credential backup database lacks connection projections".into(),
        ));
    }
    let providers = read_backup_connections::<ProviderConnection>(
        &connection,
        "SELECT connection_id, connection_json FROM provider_connections ORDER BY connection_id",
    )?;
    let mcp_connections = read_backup_connections::<McpConnection>(
        &connection,
        "SELECT connection_id, connection_json FROM mcp_connections ORDER BY connection_id",
    )?;
    if providers.len() != secrets.providers.len()
        || mcp_connections.len() != secrets.mcp_connections.len()
    {
        return Err(StorageError::InvalidData(
            "encrypted credentials do not cover the database connection catalog exactly".into(),
        ));
    }
    for (connection, secret) in providers.iter().zip(&secrets.providers) {
        connection
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let matches = match secret {
            BackupProviderSecret::None { connection_id } => {
                connection.id == *connection_id && connection.auth_mode == ProviderAuthMode::None
            }
            BackupProviderSecret::Primary {
                connection_id,
                auth_mode,
                ..
            }
            | BackupProviderSecret::OAuth {
                connection_id,
                auth_mode,
                ..
            } => connection.id == *connection_id && connection.auth_mode == *auth_mode,
        };
        if !matches {
            return Err(StorageError::InvalidData(
                "encrypted provider credential does not match its database connection".into(),
            ));
        }
    }
    for (connection, secret) in mcp_connections.iter().zip(&secrets.mcp_connections) {
        connection
            .validate()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let matches = match secret {
            BackupMcpSecret::None { connection_id } => {
                connection.id == *connection_id && connection.auth_mode == McpAuthMode::None
            }
            BackupMcpSecret::Bearer { connection_id, .. } => {
                connection.id == *connection_id && connection.auth_mode == McpAuthMode::BearerToken
            }
        };
        if !matches {
            return Err(StorageError::InvalidData(
                "encrypted MCP credential does not match its database connection".into(),
            ));
        }
    }
    Ok(())
}

fn read_backup_connections<T>(connection: &Connection, query: &str) -> Result<Vec<T>, StorageError>
where
    T: for<'de> Deserialize<'de>,
{
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let (stored_id, json) = row?;
        let value: serde_json::Value = serde_json::from_str(&json)?;
        if value.get("id").and_then(serde_json::Value::as_str) != Some(stored_id.as_str()) {
            return Err(StorageError::InvalidData(
                "connection projection identity is inconsistent".into(),
            ));
        }
        result.push(serde_json::from_value(value)?);
    }
    Ok(result)
}

fn provider_secret_connection_id(secret: &BackupProviderSecret) -> &str {
    match secret {
        BackupProviderSecret::None { connection_id }
        | BackupProviderSecret::Primary { connection_id, .. }
        | BackupProviderSecret::OAuth { connection_id, .. } => connection_id,
    }
}

fn mcp_secret_connection_id(secret: &BackupMcpSecret) -> &str {
    match secret {
        BackupMcpSecret::None { connection_id } | BackupMcpSecret::Bearer { connection_id, .. } => {
            connection_id
        }
    }
}

fn protected_store_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::ProtectedStore(error.to_string())
}

fn validate_destination(destination: &Path) -> Result<(), StorageError> {
    if destination.extension().and_then(|value| value.to_str()) != Some("mcgb") {
        return Err(StorageError::InvalidData(
            "backup destination must use the .mcgb extension".into(),
        ));
    }
    Ok(())
}

fn artifact_sources(root: &Path) -> Result<Vec<(String, PathBuf)>, StorageError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(StorageError::InvalidData(
            "artifact store root cannot be a symbolic link".into(),
        ));
    }
    let mut sources = Vec::new();
    for shard in fs::read_dir(root)? {
        let shard = shard?;
        let shard_type = shard.file_type()?;
        if shard_type.is_symlink() || !shard_type.is_dir() {
            return Err(StorageError::InvalidData(
                "artifact store contains an unsafe entry".into(),
            ));
        }
        let shard_name = shard.file_name().to_string_lossy().into_owned();
        if shard_name.len() != 2 || !is_lower_hex(&shard_name) {
            return Err(StorageError::InvalidData(
                "artifact shard name is invalid".into(),
            ));
        }
        for artifact in fs::read_dir(shard.path())? {
            let artifact = artifact?;
            let artifact_type = artifact.file_type()?;
            if artifact_type.is_symlink() || !artifact_type.is_file() {
                return Err(StorageError::InvalidData(
                    "artifact store contains an unsafe file".into(),
                ));
            }
            let digest = artifact.file_name().to_string_lossy().into_owned();
            if digest.len() != 64 || !is_lower_hex(&digest) || !digest.starts_with(&shard_name) {
                return Err(StorageError::InvalidData(
                    "artifact path does not match a SHA-256 digest".into(),
                ));
            }
            sources.push((format!("artifacts/{shard_name}/{digest}"), artifact.path()));
        }
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(sources)
}

fn inspect_source(path: &str, source: &Path) -> Result<BackupBundleEntry, StorageError> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StorageError::InvalidData(format!(
            "backup source is not a regular file: {path}"
        )));
    }
    if metadata.len() > MAX_ENTRY_BYTES {
        return Err(StorageError::InvalidData(format!(
            "backup source exceeds the per-entry bound: {path}"
        )));
    }
    let mut input = File::open(source)?;
    let sha256 = hash_reader(&mut input, metadata.len())?;
    Ok(BackupBundleEntry {
        path: path.to_owned(),
        size_bytes: metadata.len(),
        sha256,
    })
}

fn inspect_bytes(path: &str, bytes: &[u8]) -> Result<BackupBundleEntry, StorageError> {
    let size_bytes = u64::try_from(bytes.len())
        .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
    if size_bytes > MAX_ENTRY_BYTES {
        return Err(StorageError::InvalidData(format!(
            "backup source exceeds the per-entry bound: {path}"
        )));
    }
    Ok(BackupBundleEntry {
        path: path.to_owned(),
        size_bytes,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

#[derive(Serialize)]
struct BackupSecretBinding<'a> {
    domain: &'static str,
    bundle_schema_version: u16,
    application_version: &'a str,
    created_at: DateTime<Utc>,
    entries: &'a [BackupBundleEntry],
}

fn secret_binding_bytes(
    application_version: &str,
    created_at: DateTime<Utc>,
    entries: &[BackupBundleEntry],
) -> Result<Vec<u8>, StorageError> {
    Ok(serde_json::to_vec(&BackupSecretBinding {
        domain: "metacognitive-governor.encrypted-backup-credentials.v1",
        bundle_schema_version: BACKUP_BUNDLE_SCHEMA_VERSION,
        application_version,
        created_at,
        entries,
    })?)
}

fn copy_verified_source(
    source: &Path,
    entry: &BackupBundleEntry,
    output: &mut File,
) -> Result<(), StorageError> {
    let mut input = File::open(source)?;
    let mut digest = Sha256::new();
    let mut remaining = entry.size_bytes;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
            .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
        let read = input.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(StorageError::InvalidData(format!(
                "backup source changed while reading: {}",
                entry.path
            )));
        }
        digest.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
        remaining -= u64::try_from(read)
            .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
    }
    let mut extra = [0_u8; 1];
    if input.read(&mut extra)? != 0 {
        return Err(StorageError::InvalidData(format!(
            "backup source grew while reading: {}",
            entry.path
        )));
    }
    require_digest(entry, &digest.finalize())
}

fn copy_verified_bytes(
    bytes: &[u8],
    entry: &BackupBundleEntry,
    output: &mut File,
) -> Result<(), StorageError> {
    if entry.size_bytes
        != u64::try_from(bytes.len())
            .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?
    {
        return Err(StorageError::InvalidData(format!(
            "backup source changed while reading: {}",
            entry.path
        )));
    }
    require_digest(entry, &Sha256::digest(bytes))?;
    output.write_all(bytes)?;
    Ok(())
}

fn read_header(input: &mut File) -> Result<(BackupBundleManifest, Vec<u8>), StorageError> {
    let mut magic = [0_u8; 8];
    input.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(StorageError::InvalidData("backup magic is invalid".into()));
    }
    let mut length = [0_u8; 4];
    input.read_exact(&mut length)?;
    let manifest_length = u64::from(u32::from_be_bytes(length));
    if manifest_length == 0 || manifest_length > MAX_MANIFEST_BYTES {
        return Err(StorageError::InvalidData(
            "backup manifest length is invalid".into(),
        ));
    }
    let manifest_length = usize::try_from(manifest_length)
        .map_err(|_| StorageError::InvalidData("backup manifest is too large".into()))?;
    let mut bytes = vec![0_u8; manifest_length];
    input.read_exact(&mut bytes)?;
    let manifest: BackupBundleManifest = serde_json::from_slice(&bytes)?;
    validate_manifest(&manifest)?;
    let mut header = Vec::with_capacity(4 + bytes.len());
    header.extend_from_slice(&length);
    header.extend_from_slice(&bytes);
    Ok((manifest, header))
}

fn validate_manifest(manifest: &BackupBundleManifest) -> Result<(), StorageError> {
    if !(MIN_BACKUP_BUNDLE_SCHEMA_VERSION..=BACKUP_BUNDLE_SCHEMA_VERSION)
        .contains(&manifest.schema_version)
    {
        return Err(StorageError::InvalidData(
            "backup schema version is unsupported".into(),
        ));
    }
    if manifest.schema_version == MIN_BACKUP_BUNDLE_SCHEMA_VERSION
        && (manifest.secrets_included || manifest.secret_encryption.is_some())
    {
        return Err(StorageError::InvalidData(
            "legacy backup bundles cannot contain encrypted credentials".into(),
        ));
    }
    if manifest.secrets_included != manifest.secret_encryption.is_some() {
        return Err(StorageError::InvalidData(
            "backup credential encryption claim is inconsistent".into(),
        ));
    }
    if let Some(metadata) = manifest.secret_encryption.as_ref() {
        validate_backup_secret_metadata(metadata)?;
    }
    if manifest.application_version.trim().is_empty() || manifest.application_version.len() > 128 {
        return Err(StorageError::InvalidData(
            "backup application version is invalid".into(),
        ));
    }
    if manifest.entries.is_empty() || manifest.entries.len() > MAX_ENTRY_COUNT {
        return Err(StorageError::InvalidData(
            "backup entry count is invalid".into(),
        ));
    }
    let mut paths = BTreeSet::new();
    let mut total = 0_u64;
    let mut database_entries = 0_u8;
    let mut secret_entries = 0_u8;
    for entry in &manifest.entries {
        validate_entry(entry)?;
        if !paths.insert(&entry.path) {
            return Err(StorageError::InvalidData(
                "backup contains duplicate entry paths".into(),
            ));
        }
        if entry.path == DATABASE_ENTRY {
            database_entries = database_entries.saturating_add(1);
        } else if entry.path == BACKUP_SECRET_ENTRY {
            secret_entries = secret_entries.saturating_add(1);
        }
        total = total
            .checked_add(entry.size_bytes)
            .ok_or_else(|| StorageError::InvalidData("backup size overflow".into()))?;
        if total > MAX_BUNDLE_BYTES {
            return Err(StorageError::InvalidData("backup is too large".into()));
        }
    }
    if database_entries != 1 {
        return Err(StorageError::InvalidData(
            "backup must contain exactly one database snapshot".into(),
        ));
    }
    if secret_entries != u8::from(manifest.secrets_included)
        || (manifest.secrets_included
            && manifest.entries.last().map(|entry| entry.path.as_str())
                != Some(BACKUP_SECRET_ENTRY))
    {
        return Err(StorageError::InvalidData(
            "backup encrypted credential entry is inconsistent".into(),
        ));
    }
    Ok(())
}

fn validate_entry(entry: &BackupBundleEntry) -> Result<(), StorageError> {
    if entry.size_bytes > MAX_ENTRY_BYTES
        || entry.sha256.len() != 64
        || !is_lower_hex(&entry.sha256)
    {
        return Err(StorageError::InvalidData(
            "backup entry metadata is invalid".into(),
        ));
    }
    if entry.path == DATABASE_ENTRY {
        return Ok(());
    }
    if entry.path == BACKUP_SECRET_ENTRY {
        let maximum = u64::try_from(MAX_SECRET_CIPHERTEXT_BYTES)
            .map_err(|_| StorageError::InvalidData("backup size overflow".into()))?;
        if !(16..=maximum).contains(&entry.size_bytes) {
            return Err(StorageError::InvalidData(
                "encrypted credential entry size is invalid".into(),
            ));
        }
        return Ok(());
    }
    let pieces = entry.path.split('/').collect::<Vec<_>>();
    if pieces.len() != 3
        || pieces[0] != "artifacts"
        || pieces[1].len() != 2
        || !is_lower_hex(pieces[1])
        || pieces[2] != entry.sha256
        || !entry.sha256.starts_with(pieces[1])
    {
        return Err(StorageError::InvalidData(
            "backup entry path is invalid".into(),
        ));
    }
    Ok(())
}

fn extract_bundle(
    bundle: &Path,
    staging: &Path,
    expected_manifest: &BackupBundleManifest,
) -> Result<(), StorageError> {
    let mut input = File::open(bundle)?;
    let (manifest, _) = read_header(&mut input)?;
    if manifest != *expected_manifest {
        return Err(StorageError::Conflict(
            "backup changed while it was being restored".into(),
        ));
    }
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    for entry in manifest.entries {
        let destination = staging.join(entry.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let parent = destination.parent().ok_or_else(|| {
            StorageError::InvalidData("backup entry has no parent directory".into())
        })?;
        fs::create_dir_all(parent)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)?;
        let mut remaining = entry.size_bytes;
        let mut digest = Sha256::new();
        while remaining > 0 {
            let wanted = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
                .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
            input.read_exact(&mut buffer[..wanted])?;
            digest.update(&buffer[..wanted]);
            output.write_all(&buffer[..wanted])?;
            remaining -= u64::try_from(wanted)
                .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
        }
        require_digest(&entry, &digest.finalize())?;
        output.sync_all()?;
    }
    let mut trailing = [0_u8; 1];
    if input.read(&mut trailing)? != 0 {
        return Err(StorageError::InvalidData(
            "backup bundle contains trailing bytes".into(),
        ));
    }
    Ok(())
}

fn validate_restored_database(path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?;
    if integrity != "ok" {
        return Err(StorageError::InvalidData(format!(
            "restored database failed integrity check: {integrity}"
        )));
    }
    let has_schema: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mcg_schema')",
        [],
        |row| row.get(0),
    )?;
    if !has_schema {
        return Err(StorageError::InvalidData(
            "restored database is not an MCG database".into(),
        ));
    }
    let newest: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM mcg_schema",
        [],
        |row| row.get(0),
    )?;
    if newest > STORAGE_SCHEMA_VERSION {
        return Err(StorageError::InvalidData(format!(
            "backup database schema version {newest} is newer than supported version {STORAGE_SCHEMA_VERSION}"
        )));
    }
    let mut foreign_keys = connection.prepare("PRAGMA foreign_key_check")?;
    if foreign_keys.query([])?.next()?.is_some() {
        return Err(StorageError::InvalidData(
            "backup database contains broken foreign-key references".into(),
        ));
    }
    Ok(())
}

fn validate_plugin_catalog_backup(
    database: &Path,
    entries: &[BackupBundleEntry],
) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if !table_exists(&connection, "plugin_revisions")? {
        return Ok(());
    }
    if !table_exists(&connection, "plugin_installations")? {
        return Err(StorageError::InvalidData(
            "backup plugin catalog is incomplete".into(),
        ));
    }
    let artifacts = entries
        .iter()
        .filter(|entry| entry.path != DATABASE_ENTRY)
        .map(|entry| (entry.sha256.as_str(), entry.size_bytes))
        .collect::<BTreeMap<_, _>>();
    let mut revisions = BTreeMap::new();
    let mut statement = connection.prepare(
        "SELECT revision_id, plugin_id, module_sha256, revision_json, installed_at
         FROM plugin_revisions ORDER BY revision_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (revision_id, plugin_id, module_sha256, revision_json, installed_at) = row?;
        let revision: PluginRevision = serde_json::from_str(&revision_json)?;
        validate_plugin_revision(&revision)?;
        if revision.id.to_string() != revision_id
            || revision.manifest.id != plugin_id
            || revision.module_sha256 != module_sha256
            || revision.installed_at.to_rfc3339() != installed_at
            || artifacts.get(revision.module_sha256.as_str()) != Some(&revision.module_size_bytes)
            || revisions.insert(revision_id, revision).is_some()
        {
            return Err(StorageError::InvalidData(
                "backup plugin revision or module evidence is inconsistent".into(),
            ));
        }
    }

    let mut statement = connection.prepare(
        "SELECT plugin_id, active_revision_id, enabled, revision, installation_json, updated_at
         FROM plugin_installations ORDER BY plugin_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    for row in rows {
        let (plugin_id, active_revision_id, enabled, revision_number, json, updated_at) = row?;
        let installation: PluginInstallation = serde_json::from_str(&json)?;
        let revision = revisions.get(&active_revision_id).ok_or_else(|| {
            StorageError::InvalidData("backup plugin head references a missing revision".into())
        })?;
        let stored_revision = u64::try_from(revision_number).map_err(|_| {
            StorageError::InvalidData("backup plugin head revision is invalid".into())
        })?;
        if installation.plugin_id != plugin_id
            || installation.active_revision_id.to_string() != active_revision_id
            || i64::from(installation.enabled) != enabled
            || installation.revision != stored_revision
            || installation.updated_at.to_rfc3339() != updated_at
        {
            return Err(StorageError::InvalidData(
                "backup plugin installation projection is inconsistent".into(),
            ));
        }
        validate_plugin_backup_head(&installation, revision)?;
    }
    validate_plugin_invocation_backup(&connection, &revisions, &artifacts)?;
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keeps projection/history/artifact agreement in one audit boundary.
fn validate_plugin_invocation_backup(
    connection: &Connection,
    revisions: &BTreeMap<String, PluginRevision>,
    artifacts: &BTreeMap<&str, u64>,
) -> Result<(), StorageError> {
    if !table_exists(connection, "plugin_invocations")?
        || !table_exists(connection, "plugin_invocation_events")?
    {
        return Err(StorageError::InvalidData(
            "backup plugin invocation history is incomplete".into(),
        ));
    }
    let mut statement = connection.prepare(
        "SELECT invocation_id, plugin_id, plugin_revision_id, state, invocation_json
         FROM plugin_invocations ORDER BY invocation_id",
    )?;
    let mut projections = BTreeMap::<String, PluginInvocationRecord>::new();
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (invocation_id, plugin_id, revision_id, state, json) = row?;
        let invocation: PluginInvocationRecord = serde_json::from_str(&json)?;
        validate_plugin_invocation_backup_record(&invocation)?;
        validate_plugin_invocation_identity(
            &invocation,
            &invocation_id,
            &plugin_id,
            &revision_id,
            &state,
            revisions,
            artifacts,
        )?;
        if projections.insert(invocation_id, invocation).is_some() {
            return Err(StorageError::InvalidData(
                "backup contains duplicate plugin invocation projections".into(),
            ));
        }
    }

    let mut statement = connection.prepare(
        "SELECT invocation_id, state, invocation_json
         FROM plugin_invocation_events ORDER BY sequence",
    )?;
    let mut histories = BTreeMap::<String, Vec<PluginInvocationRecord>>::new();
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (invocation_id, state, json) = row?;
        let invocation: PluginInvocationRecord = serde_json::from_str(&json)?;
        validate_plugin_invocation_backup_record(&invocation)?;
        let revision_id = invocation.plugin_revision_id.to_string();
        validate_plugin_invocation_identity(
            &invocation,
            &invocation_id,
            &invocation.plugin_id,
            &revision_id,
            &state,
            revisions,
            artifacts,
        )?;
        histories.entry(invocation_id).or_default().push(invocation);
    }
    if projections.len() != histories.len() {
        return Err(StorageError::InvalidData(
            "backup plugin invocation history is incomplete".into(),
        ));
    }
    for (invocation_id, projection) in projections {
        let history = histories.get(&invocation_id).ok_or_else(|| {
            StorageError::InvalidData("backup plugin invocation history is missing".into())
        })?;
        let history_shape_is_valid = history.first().is_some_and(|record| {
            record.state == PluginInvocationState::Running
                && record.result_sha256.is_none()
                && record.error_code.is_none()
                && record.completed_at.is_none()
        }) && match history.as_slice() {
            [running] => {
                projection.state == PluginInvocationState::Running && &projection == running
            }
            [running, terminal] => {
                terminal.state.is_terminal()
                    && running.id == terminal.id
                    && running.plugin_id == terminal.plugin_id
                    && running.plugin_revision_id == terminal.plugin_revision_id
                    && running.task_id == terminal.task_id
                    && running.input_sha256 == terminal.input_sha256
                    && running.started_at == terminal.started_at
                    && &projection == terminal
            }
            _ => false,
        };
        if !history_shape_is_valid {
            return Err(StorageError::InvalidData(
                "backup plugin invocation projection and history disagree".into(),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors the durable projection columns being cross-checked.
fn validate_plugin_invocation_identity(
    invocation: &PluginInvocationRecord,
    invocation_id: &str,
    plugin_id: &str,
    revision_id: &str,
    state: &str,
    revisions: &BTreeMap<String, PluginRevision>,
    artifacts: &BTreeMap<&str, u64>,
) -> Result<(), StorageError> {
    let revision = revisions.get(revision_id).ok_or_else(|| {
        StorageError::InvalidData("backup plugin invocation references a missing revision".into())
    })?;
    let result_is_present = invocation
        .result_sha256
        .as_deref()
        .is_none_or(|digest| artifacts.contains_key(digest));
    if invocation.id.to_string() != invocation_id
        || invocation.plugin_id != plugin_id
        || invocation.plugin_revision_id.to_string() != revision_id
        || revision.manifest.id != invocation.plugin_id
        || plugin_invocation_state(invocation.state) != state
        || !result_is_present
    {
        return Err(StorageError::InvalidData(
            "backup plugin invocation evidence is inconsistent".into(),
        ));
    }
    Ok(())
}

const fn plugin_invocation_state(state: PluginInvocationState) -> &'static str {
    match state {
        PluginInvocationState::Running => "running",
        PluginInvocationState::Succeeded => "succeeded",
        PluginInvocationState::Failed => "failed",
        PluginInvocationState::Uncertain => "uncertain",
    }
}

fn disable_restored_plugins(database: &Path, now: DateTime<Utc>) -> Result<(), StorageError> {
    let mut connection = Connection::open(database)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if !table_exists(&connection, "plugin_installations")? {
        return Ok(());
    }
    let transaction = connection.transaction()?;
    let installations = {
        let mut statement = transaction.prepare(
            "SELECT installation_json FROM plugin_installations
             WHERE enabled = 1 ORDER BY plugin_id",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for json in installations {
        let mut installation: PluginInstallation = serde_json::from_str(&json)?;
        let revision_json = transaction.query_row(
            "SELECT revision_json FROM plugin_revisions WHERE revision_id = ?1",
            [installation.active_revision_id.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        let revision: PluginRevision = serde_json::from_str(&revision_json)?;
        validate_plugin_backup_head(&installation, &revision)?;
        installation.enabled = false;
        installation.revision = installation.revision.checked_add(1).ok_or_else(|| {
            StorageError::InvalidData("restored plugin revision is exhausted".into())
        })?;
        installation.updated_at = now;
        let changed = transaction.execute(
            "UPDATE plugin_installations
             SET enabled = 0, revision = ?1, installation_json = ?2, updated_at = ?3
             WHERE plugin_id = ?4 AND enabled = 1",
            params![
                i64::try_from(installation.revision).map_err(|_| {
                    StorageError::InvalidData("restored plugin revision is invalid".into())
                })?,
                serde_json::to_string(&installation)?,
                installation.updated_at.to_rfc3339(),
                installation.plugin_id,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "restored plugin head changed while being disabled".into(),
            ));
        }
    }
    transaction.commit()?;
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, StorageError> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn merge_restored_artifacts(source: &Path, destination: &Path) -> Result<(), StorageError> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    for (relative, artifact) in artifact_sources(source)? {
        let relative = relative
            .strip_prefix("artifacts/")
            .ok_or_else(|| StorageError::InvalidData("restored artifact path is invalid".into()))?;
        let target = destination.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if target.exists() {
            let expected = inspect_source(&format!("artifacts/{relative}"), &artifact)?;
            let actual = inspect_source(&format!("artifacts/{relative}"), &target)?;
            if expected.sha256 != actual.sha256 || expected.size_bytes != actual.size_bytes {
                return Err(StorageError::Conflict(format!(
                    "existing artifact conflicts with restored content: {relative}"
                )));
            }
            continue;
        }
        fs::create_dir_all(
            target.parent().ok_or_else(|| {
                StorageError::InvalidData("restored artifact has no parent".into())
            })?,
        )?;
        fs::rename(artifact, target)?;
    }
    Ok(())
}

fn install_restored_database(
    restored: &Path,
    data_directory: &Path,
    now: DateTime<Utc>,
) -> Result<(), StorageError> {
    let database = data_directory.join("mcg.sqlite3");
    let backups = data_directory.join("backups");
    fs::create_dir_all(&backups)?;
    let stamp = now.format("%Y%m%dT%H%M%SZ");
    let rollback = backups.join(format!("pre-restore-{stamp}-{}.sqlite3", Uuid::new_v4()));
    let wal = database.with_extension("sqlite3-wal");
    let shm = database.with_extension("sqlite3-shm");
    if wal.exists() || shm.exists() {
        return Err(StorageError::Conflict(
            "database sidecar files are present; stop the daemon cleanly before restore".into(),
        ));
    }
    let had_database = database.exists();
    if had_database {
        fs::rename(&database, &rollback)?;
    }
    if let Err(error) = fs::rename(restored, &database) {
        if rollback.exists() {
            let _ = fs::rename(&rollback, &database);
        }
        return Err(error.into());
    }
    if let Err(sync_error) = sync_parent(data_directory) {
        let mut rollback_failed = fs::rename(&database, restored).is_err();
        if had_database {
            rollback_failed |= fs::rename(&rollback, &database).is_err();
        }
        let _ = sync_parent(data_directory);
        if rollback_failed {
            return Err(StorageError::Conflict(
                "database installation sync failed and filesystem rollback also failed".into(),
            ));
        }
        return Err(sync_error);
    }
    Ok(())
}

fn hash_reader(input: &mut File, expected_bytes: u64) -> Result<String, StorageError> {
    let mut digest = Sha256::new();
    let mut remaining = expected_bytes;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64))
            .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
        let read = input.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(StorageError::InvalidData(
                "backup source ended before its declared size".into(),
            ));
        }
        digest.update(&buffer[..read]);
        remaining -= u64::try_from(read)
            .map_err(|_| StorageError::InvalidData("backup entry size overflow".into()))?;
    }
    let mut trailing = [0_u8; 1];
    if input.read(&mut trailing)? != 0 {
        return Err(StorageError::InvalidData(
            "backup source exceeds its declared size".into(),
        ));
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn require_digest(
    entry: &BackupBundleEntry,
    digest: &sha2::digest::Output<Sha256>,
) -> Result<(), StorageError> {
    let actual = format!("{digest:x}");
    if actual != entry.sha256 {
        return Err(StorageError::InvalidData(format!(
            "backup entry digest mismatch: {}",
            entry.path
        )));
    }
    Ok(())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), StorageError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn sync_parent(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use mcg_domain::{
        PLUGIN_CATALOG_SCHEMA_VERSION, PluginInstallSource, PluginInvocationId,
        PluginInvocationRecord, PluginInvocationState, PluginRevision, PluginRevisionId,
        PluginSignature, PluginSignatureState, TaskEnvelope,
    };
    use mcg_mcp::{
        MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpConnection, McpCredential,
        McpProtocolMode, McpTransport,
    };
    use mcg_plugin_protocol::{PLUGIN_SCHEMA_VERSION, PluginClass, PluginManifest};
    use mcg_providers::{
        CONTRACT_VERSION, MemoryProviderCredentialStore, ProviderAuthMode, ProviderConnection,
        ProviderCredential, ProviderCredentialStore, ProviderKind, ProviderSecretSlot,
    };
    use tempfile::tempdir;

    use crate::{BackupMcpSecret, BackupProviderSecret};

    use super::*;

    fn assert_no_online_snapshot_files(data_directory: &Path) {
        let backups = data_directory.join("backups");
        let retained = fs::read_dir(backups)
            .expect("backup directory")
            .map(|entry| entry.expect("backup entry").file_name())
            .filter(|name| name.to_string_lossy().starts_with(".mcgb-"))
            .collect::<Vec<_>>();
        assert!(
            retained.is_empty(),
            "online backup retained temporary SQLite files: {retained:?}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One fixture proves database, module, token, and rollback behavior together.
    fn bundle_round_trip_is_secret_free_tamper_evident_and_restart_durable() {
        let source = tempdir().expect("source tempdir");
        let destination = tempdir().expect("destination tempdir");
        let backup_path = source.path().join("backups").join("owner.mcgb");
        let mut store = SqliteEventStore::open(source.path().join("mcg.sqlite3")).expect("store");
        let task = TaskEnvelope::new("survive portable restore");
        store.create_task(&task).expect("task");
        let artifact_bytes = b"portable evidence";
        let digest = format!("{:x}", Sha256::digest(artifact_bytes));
        let artifact = source
            .path()
            .join("artifacts")
            .join(&digest[..2])
            .join(&digest);
        fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
        fs::write(&artifact, artifact_bytes).expect("artifact");
        let module_bytes = b"\0asm\x01\0\0\0portable-plugin";
        let module_digest = format!("{:x}", Sha256::digest(module_bytes));
        let module_artifact = source
            .path()
            .join("artifacts")
            .join(&module_digest[..2])
            .join(&module_digest);
        fs::create_dir_all(module_artifact.parent().expect("module parent"))
            .expect("module directory");
        fs::write(&module_artifact, module_bytes).expect("plugin module");
        let installed_at = Utc::now();
        let plugin = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.portable".into(),
                display_name: "Portable plugin".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: module_digest.clone(),
            module_size_bytes: u64::try_from(module_bytes.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at,
        };
        let installation = store
            .install_unsigned_local_plugin_revision(&plugin)
            .expect("install plugin");
        let installation = store
            .set_plugin_enabled(
                &plugin.manifest.id,
                installation.revision,
                true,
                installed_at + Duration::seconds(1),
            )
            .expect("enable plugin");
        assert!(installation.enabled);
        let invocation = PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: plugin.manifest.id.clone(),
            plugin_revision_id: plugin.id,
            task_id: None,
            input_sha256: "b".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: installed_at + Duration::seconds(2),
            completed_at: None,
        };
        store
            .begin_plugin_invocation(&invocation)
            .expect("begin invocation");
        let mut completed_invocation = invocation.clone();
        completed_invocation.state = PluginInvocationState::Succeeded;
        completed_invocation.result_sha256 = Some(digest.clone());
        completed_invocation.completed_at = Some(installed_at + Duration::seconds(3));
        store
            .complete_plugin_invocation(&completed_invocation)
            .expect("complete invocation");
        fs::write(source.path().join("local-auth-token"), "private-token").expect("token");

        let summary =
            create_backup_bundle(&store, source.path(), &backup_path, "0.1.0", Utc::now())
                .expect("create backup");
        assert_no_online_snapshot_files(source.path());
        assert!(!summary.manifest.secrets_included);
        assert!(summary.manifest.entries.iter().any(|entry| {
            entry.sha256 == module_digest
                && entry.size_bytes == u64::try_from(module_bytes.len()).expect("module size")
        }));
        let bytes = fs::read(&backup_path).expect("bundle bytes");
        assert!(!String::from_utf8_lossy(&bytes).contains("private-token"));
        drop(store);

        let mut existing = SqliteEventStore::open(destination.path().join("mcg.sqlite3"))
            .expect("existing destination store");
        existing
            .create_task(&TaskEnvelope::new("replace this task"))
            .expect("existing task");
        drop(existing);
        fs::write(
            destination.path().join("local-auth-token"),
            "destination-token",
        )
        .expect("destination token");
        let lock = acquire_data_directory_lock(destination.path()).expect("data lock");
        assert!(matches!(
            restore_backup_bundle(&backup_path, destination.path(), Utc::now()),
            Err(StorageError::Conflict(_))
        ));
        drop(lock);
        restore_backup_bundle(&backup_path, destination.path(), Utc::now()).expect("restore");
        let restored =
            SqliteEventStore::open(destination.path().join("mcg.sqlite3")).expect("reopen");
        assert_eq!(restored.tasks().expect("task read"), vec![task]);
        assert_eq!(
            restored
                .plugin_revision(plugin.id)
                .expect("plugin revision"),
            Some(plugin.clone())
        );
        let restored_installation = restored
            .plugin_installation(&plugin.manifest.id)
            .expect("plugin installation")
            .expect("restored plugin");
        assert!(!restored_installation.enabled);
        assert_eq!(restored_installation.revision, installation.revision + 1);
        assert_eq!(
            restored
                .plugin_invocation(invocation.id)
                .expect("plugin invocation"),
            Some(completed_invocation.clone())
        );
        assert_eq!(
            restored
                .plugin_invocation_history(invocation.id)
                .expect("plugin invocation history"),
            vec![invocation, completed_invocation]
        );
        assert_eq!(
            fs::read(
                destination
                    .path()
                    .join("artifacts")
                    .join(&module_digest[..2])
                    .join(&module_digest)
            )
            .expect("restored plugin module"),
            module_bytes
        );
        assert_eq!(
            fs::read(
                destination
                    .path()
                    .join("artifacts")
                    .join(&digest[..2])
                    .join(&digest)
            )
            .expect("restored artifact"),
            artifact_bytes
        );
        assert_eq!(
            fs::read_to_string(destination.path().join("local-auth-token"))
                .expect("preserved destination token"),
            "destination-token"
        );
        assert!(
            fs::read_dir(destination.path().join("backups"))
                .expect("rollback directory")
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pre-restore-"))
        );

        let mut tampered = bytes;
        *tampered.last_mut().expect("bundle byte") ^= 1;
        let tampered_path = source.path().join("tampered.mcgb");
        fs::write(&tampered_path, tampered).expect("tampered bundle");
        assert!(verify_backup_bundle(&tampered_path).is_err());
    }

    #[test]
    fn encrypted_bundle_binds_credentials_and_legacy_secret_free_format_still_verifies() {
        let source = tempdir().expect("source tempdir");
        let destination = tempdir().expect("destination tempdir");
        let store = SqliteEventStore::open(source.path().join("mcg.sqlite3")).expect("store");
        insert_backup_connections(&store);
        let snapshot = source.path().join("snapshot.sqlite3");
        store.snapshot_database(&snapshot).expect("snapshot");
        let secrets = BackupSecretSnapshot::new(
            vec![BackupProviderSecret::Primary {
                connection_id: "openrouter".into(),
                auth_mode: ProviderAuthMode::ApiKey,
                credential: ProviderCredential::new("encrypted-provider-key")
                    .expect("provider credential"),
            }],
            vec![BackupMcpSecret::Bearer {
                connection_id: "private-mcp".into(),
                credential: McpCredential::new("encrypted-mcp-token").expect("MCP credential"),
            }],
        )
        .expect("secret snapshot");
        let passphrase = BackupPassphrase::new("correct horse battery staple").expect("passphrase");
        let encrypted_path = source.path().join("backups/encrypted.mcgb");
        let encrypted = create_encrypted_backup_bundle_from_snapshot(
            &snapshot,
            source.path(),
            &encrypted_path,
            "0.1.0",
            Utc::now(),
            &secrets,
            &passphrase,
        )
        .expect("encrypted backup");
        assert_eq!(
            encrypted.manifest.schema_version,
            BACKUP_BUNDLE_SCHEMA_VERSION
        );
        assert!(encrypted.manifest.secrets_included);
        assert!(encrypted.manifest.secret_encryption.is_some());
        assert_eq!(
            encrypted
                .manifest
                .entries
                .last()
                .map(|entry| entry.path.as_str()),
            Some(BACKUP_SECRET_ENTRY)
        );
        let bytes = fs::read(&encrypted_path).expect("encrypted bytes");
        for secret in ["encrypted-provider-key", "encrypted-mcp-token"] {
            assert!(
                !bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes())
            );
        }
        let (_, decrypted) = decrypt_backup_bundle_secrets(&encrypted_path, &passphrase)
            .expect("decrypt bundle secrets");
        assert_eq!(decrypted.providers.len(), 1);
        assert_eq!(decrypted.mcp_connections.len(), 1);
        let wrong = BackupPassphrase::new("definitely not the right passphrase").expect("wrong");
        assert!(decrypt_backup_bundle_secrets(&encrypted_path, &wrong).is_err());
        assert!(restore_backup_bundle(&encrypted_path, destination.path(), Utc::now()).is_err());
        assert!(!destination.path().join("mcg.sqlite3").exists());

        let current_path = source.path().join("backups/current-secret-free.mcgb");
        create_backup_bundle_from_snapshot(
            &snapshot,
            source.path(),
            &current_path,
            "0.1.0",
            Utc::now(),
        )
        .expect("secret-free backup");
        let mut legacy_bytes = fs::read(&current_path).expect("current bytes");
        let marker = b"\"schema_version\":2";
        let position = legacy_bytes
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("schema marker");
        legacy_bytes[position + marker.len() - 1] = b'1';
        let legacy_path = source.path().join("backups/legacy-secret-free.mcgb");
        fs::write(&legacy_path, legacy_bytes).expect("legacy bundle");
        let legacy = verify_backup_bundle(&legacy_path).expect("legacy schema 1 verification");
        assert_eq!(legacy.manifest.schema_version, 1);
        assert!(!legacy.manifest.secrets_included);
        assert!(legacy.manifest.secret_encryption.is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One end-to-end assertion covers exact replacement, preservation, token, and rollback evidence.
    fn encrypted_restore_replaces_exact_credentials_preserves_unrelated_slots_and_keeps_token() {
        let source = tempdir().expect("source tempdir");
        let destination = tempdir().expect("destination tempdir");
        let (bundle, passphrase) = encrypted_connection_bundle(source.path());
        let existing = SqliteEventStore::open(destination.path().join("mcg.sqlite3"))
            .expect("existing destination store");
        drop(existing);
        fs::write(
            destination.path().join("auth-token"),
            "destination-only-token",
        )
        .expect("destination token");

        let providers = MemoryProviderCredentialStore::default();
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::Primary,
                &ProviderCredential::new("old-provider-secret").expect("old provider"),
            )
            .expect("old primary");
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::OAuthIdToken,
                &ProviderCredential::new("stale-oauth-token").expect("stale OAuth"),
            )
            .expect("stale OAuth slot");
        providers
            .save(
                "unrelated",
                ProviderSecretSlot::Primary,
                &ProviderCredential::new("unrelated-provider-secret").expect("unrelated provider"),
            )
            .expect("unrelated provider slot");
        let mcp = mcg_mcp::MemoryMcpCredentialStore::default();
        mcp.save(
            "private-mcp",
            &McpCredential::new("old-mcp-secret").expect("old MCP"),
        )
        .expect("old MCP slot");
        mcp.save(
            "unrelated-mcp",
            &McpCredential::new("unrelated-mcp-secret").expect("unrelated MCP"),
        )
        .expect("unrelated MCP slot");

        let restored = restore_encrypted_backup_bundle(
            &bundle,
            destination.path(),
            Utc::now(),
            &passphrase,
            &providers,
            &mcp,
        )
        .expect("encrypted restore");
        assert!(restored.manifest.secrets_included);
        assert_eq!(
            providers
                .load("openrouter", ProviderSecretSlot::Primary)
                .expect("provider load")
                .expect("provider credential")
                .expose_to_host_executor(),
            "new-provider-secret"
        );
        assert!(
            providers
                .load("openrouter", ProviderSecretSlot::OAuthIdToken)
                .expect("OAuth slot load")
                .is_none()
        );
        assert_eq!(
            providers
                .load("unrelated", ProviderSecretSlot::Primary)
                .expect("unrelated load")
                .expect("unrelated credential")
                .expose_to_host_executor(),
            "unrelated-provider-secret"
        );
        assert_eq!(
            mcp.load("private-mcp")
                .expect("MCP load")
                .expect("MCP credential")
                .expose_to_host_executor(),
            "new-mcp-secret"
        );
        assert_eq!(
            mcp.load("unrelated-mcp")
                .expect("unrelated MCP load")
                .expect("unrelated MCP credential")
                .expose_to_host_executor(),
            "unrelated-mcp-secret"
        );
        assert_eq!(
            fs::read_to_string(destination.path().join("auth-token")).expect("retained token"),
            "destination-only-token"
        );
        let restored_store =
            SqliteEventStore::open(destination.path().join("mcg.sqlite3")).expect("restored store");
        assert_eq!(
            restored_store
                .provider_connection("openrouter")
                .expect("provider projection")
                .expect("provider connection")
                .auth_mode,
            ProviderAuthMode::ApiKey
        );
        assert_eq!(
            restored_store
                .mcp_connection("private-mcp")
                .expect("MCP projection")
                .expect("MCP connection")
                .auth_mode,
            McpAuthMode::BearerToken
        );
        drop(restored_store);
        assert!(
            fs::read_dir(destination.path().join("backups"))
                .expect("rollback directory")
                .any(|entry| entry
                    .expect("rollback entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pre-restore-"))
        );
    }

    #[test]
    fn encrypted_restore_rolls_credentials_back_when_database_installation_fails() {
        let source = tempdir().expect("source tempdir");
        let destination = tempdir().expect("destination tempdir");
        let (bundle, passphrase) = encrypted_connection_bundle(source.path());
        let existing = SqliteEventStore::open(destination.path().join("mcg.sqlite3"))
            .expect("existing destination store");
        drop(existing);
        let original_database = fs::read(destination.path().join("mcg.sqlite3"))
            .expect("original destination database");
        fs::write(destination.path().join("mcg.sqlite3-wal"), "active-sidecar")
            .expect("blocking sidecar");

        let providers = MemoryProviderCredentialStore::default();
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::Primary,
                &ProviderCredential::new("old-provider-secret").expect("old provider"),
            )
            .expect("old provider slot");
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::OAuthIdToken,
                &ProviderCredential::new("old-stale-id-token").expect("old stale ID token"),
            )
            .expect("old stale ID slot");
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::OAuthRefreshToken,
                &ProviderCredential::new("old-stale-refresh-token")
                    .expect("old stale refresh token"),
            )
            .expect("old stale refresh slot");
        let mcp = mcg_mcp::MemoryMcpCredentialStore::default();
        mcp.save(
            "private-mcp",
            &McpCredential::new("old-mcp-secret").expect("old MCP"),
        )
        .expect("old MCP slot");

        let error = restore_encrypted_backup_bundle(
            &bundle,
            destination.path(),
            Utc::now(),
            &passphrase,
            &providers,
            &mcp,
        )
        .expect_err("sidecar must reject database installation");
        assert!(error.to_string().contains("sidecar"));
        assert_eq!(
            fs::read(destination.path().join("mcg.sqlite3"))
                .expect("unchanged destination database"),
            original_database
        );
        assert_eq!(
            providers
                .load("openrouter", ProviderSecretSlot::Primary)
                .expect("provider load")
                .expect("provider credential")
                .expose_to_host_executor(),
            "old-provider-secret"
        );
        assert_eq!(
            providers
                .load("openrouter", ProviderSecretSlot::OAuthIdToken)
                .expect("ID slot load")
                .expect("old ID token")
                .expose_to_host_executor(),
            "old-stale-id-token"
        );
        assert_eq!(
            providers
                .load("openrouter", ProviderSecretSlot::OAuthRefreshToken)
                .expect("refresh slot load")
                .expect("old refresh token")
                .expose_to_host_executor(),
            "old-stale-refresh-token"
        );
        assert_eq!(
            mcp.load("private-mcp")
                .expect("MCP load")
                .expect("MCP credential")
                .expose_to_host_executor(),
            "old-mcp-secret"
        );
    }

    #[test]
    fn encrypted_restore_rejects_credential_catalog_mismatch_before_mutation() {
        let source = tempdir().expect("source tempdir");
        let destination = tempdir().expect("destination tempdir");
        let store = SqliteEventStore::open(source.path().join("mcg.sqlite3")).expect("store");
        insert_backup_connections(&store);
        let snapshot = source.path().join("snapshot.sqlite3");
        store.snapshot_database(&snapshot).expect("snapshot");
        let mismatched = BackupSecretSnapshot::new(
            vec![BackupProviderSecret::None {
                connection_id: "openrouter".into(),
            }],
            vec![BackupMcpSecret::Bearer {
                connection_id: "private-mcp".into(),
                credential: McpCredential::new("new-mcp-secret").expect("MCP credential"),
            }],
        )
        .expect("shape-valid but catalog-mismatched secrets");
        let passphrase = BackupPassphrase::new("correct horse battery staple").expect("passphrase");
        let bundle = source.path().join("mismatch.mcgb");
        create_backup_bundle_from_snapshot_inner(
            &snapshot,
            source.path(),
            &bundle,
            "0.1.0",
            Utc::now(),
            Some((&mismatched, &passphrase)),
        )
        .expect("mismatched fixture bundle");
        let existing = SqliteEventStore::open(destination.path().join("mcg.sqlite3"))
            .expect("existing destination store");
        drop(existing);
        let original_database = fs::read(destination.path().join("mcg.sqlite3"))
            .expect("original destination database");
        let providers = MemoryProviderCredentialStore::default();
        providers
            .save(
                "openrouter",
                ProviderSecretSlot::Primary,
                &ProviderCredential::new("old-provider-secret").expect("old provider"),
            )
            .expect("old provider slot");
        let mcp = mcg_mcp::MemoryMcpCredentialStore::default();

        assert!(
            restore_encrypted_backup_bundle(
                &bundle,
                destination.path(),
                Utc::now(),
                &passphrase,
                &providers,
                &mcp,
            )
            .is_err()
        );
        assert_eq!(
            fs::read(destination.path().join("mcg.sqlite3"))
                .expect("unchanged destination database"),
            original_database
        );
        assert_eq!(
            providers
                .load("openrouter", ProviderSecretSlot::Primary)
                .expect("provider load")
                .expect("provider credential")
                .expose_to_host_executor(),
            "old-provider-secret"
        );
    }

    fn encrypted_connection_bundle(root: &Path) -> (PathBuf, BackupPassphrase) {
        let store = SqliteEventStore::open(root.join("mcg.sqlite3")).expect("source store");
        insert_backup_connections(&store);
        let snapshot = root.join("snapshot.sqlite3");
        store.snapshot_database(&snapshot).expect("snapshot");
        let secrets = BackupSecretSnapshot::new(
            vec![BackupProviderSecret::Primary {
                connection_id: "openrouter".into(),
                auth_mode: ProviderAuthMode::ApiKey,
                credential: ProviderCredential::new("new-provider-secret")
                    .expect("provider credential"),
            }],
            vec![BackupMcpSecret::Bearer {
                connection_id: "private-mcp".into(),
                credential: McpCredential::new("new-mcp-secret").expect("MCP credential"),
            }],
        )
        .expect("secret snapshot");
        let passphrase = BackupPassphrase::new("correct horse battery staple").expect("passphrase");
        let bundle = root.join("encrypted.mcgb");
        create_encrypted_backup_bundle_from_snapshot(
            &snapshot,
            root,
            &bundle,
            "0.1.0",
            Utc::now(),
            &secrets,
            &passphrase,
        )
        .expect("encrypted fixture bundle");
        (bundle, passphrase)
    }

    fn insert_backup_connections(store: &SqliteEventStore) {
        let now = Utc::now().to_rfc3339();
        let provider = ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "openrouter".into(),
            display_name: "OpenRouter".into(),
            kind: ProviderKind::OpenRouter,
            base_url: "https://openrouter.ai/api/v1".into(),
            auth_mode: ProviderAuthMode::ApiKey,
            organization: None,
            project: None,
        };
        let mcp = McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "private-mcp".into(),
            display_name: "Private MCP".into(),
            transport: McpTransport::StreamableHttp {
                url: "https://mcp.example.test/rpc".into(),
            },
            auth_mode: McpAuthMode::BearerToken,
            protocol_mode: McpProtocolMode::Auto,
            enabled: true,
        };
        store
            .connection
            .execute(
                "INSERT INTO provider_connections(connection_id, connection_json, updated_at) VALUES (?1, ?2, ?3)",
                params![provider.id, serde_json::to_string(&provider).expect("provider JSON"), now],
            )
            .expect("provider projection");
        store
            .connection
            .execute(
                "INSERT INTO mcp_connections(connection_id, connection_json, updated_at) VALUES (?1, ?2, ?3)",
                params![mcp.id, serde_json::to_string(&mcp).expect("MCP JSON"), now],
            )
            .expect("MCP projection");
    }

    #[test]
    fn backup_refuses_a_plugin_catalog_without_its_exact_module_artifact() {
        let source = tempdir().expect("source tempdir");
        let backup_path = source.path().join("backups").join("missing-module.mcgb");
        let mut store = SqliteEventStore::open(source.path().join("mcg.sqlite3")).expect("store");
        let plugin = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.missing".into(),
                display_name: "Missing module".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: "a".repeat(64),
            module_size_bytes: 8,
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at: Utc::now(),
        };
        store
            .install_unsigned_local_plugin_revision(&plugin)
            .expect("catalog plugin");

        let error = create_backup_bundle(&store, source.path(), &backup_path, "0.1.0", Utc::now())
            .expect_err("missing module must fail backup");
        assert!(error.to_string().contains("module evidence"));
        assert!(!backup_path.exists());
        assert_no_online_snapshot_files(source.path());
    }

    #[test]
    fn backup_refuses_successful_plugin_history_without_result_evidence() {
        let source = tempdir().expect("source tempdir");
        let backup_path = source.path().join("backups").join("missing-result.mcgb");
        let mut store = SqliteEventStore::open(source.path().join("mcg.sqlite3")).expect("store");
        let module_bytes = b"\0asm\x01\0\0\0result-fixture";
        let module_digest = format!("{:x}", Sha256::digest(module_bytes));
        let module_artifact = source
            .path()
            .join("artifacts")
            .join(&module_digest[..2])
            .join(&module_digest);
        fs::create_dir_all(module_artifact.parent().expect("module parent"))
            .expect("module directory");
        fs::write(module_artifact, module_bytes).expect("module artifact");
        let installed_at = Utc::now();
        let plugin = PluginRevision {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginRevisionId::new(),
            manifest: PluginManifest {
                schema_version: PLUGIN_SCHEMA_VERSION,
                id: "example.missing_result".into(),
                display_name: "Missing result".into(),
                version: "1.0.0".into(),
                class: PluginClass::SandboxedWasi,
                requested_capabilities: Vec::new(),
            },
            module_sha256: module_digest,
            module_size_bytes: u64::try_from(module_bytes.len()).expect("module size"),
            source: PluginInstallSource::LocalImport,
            signature: PluginSignature {
                state: PluginSignatureState::Unsigned,
                scheme: None,
                signer: None,
                signature_sha256: None,
            },
            installed_at,
        };
        let installation = store
            .install_unsigned_local_plugin_revision(&plugin)
            .expect("install plugin");
        store
            .set_plugin_enabled(
                &plugin.manifest.id,
                installation.revision,
                true,
                installed_at + Duration::seconds(1),
            )
            .expect("enable plugin");
        let mut invocation = PluginInvocationRecord {
            schema_version: PLUGIN_CATALOG_SCHEMA_VERSION,
            id: PluginInvocationId::new(),
            plugin_id: plugin.manifest.id.clone(),
            plugin_revision_id: plugin.id,
            task_id: None,
            input_sha256: "b".repeat(64),
            state: PluginInvocationState::Running,
            result_sha256: None,
            error_code: None,
            started_at: installed_at + Duration::seconds(2),
            completed_at: None,
        };
        store
            .begin_plugin_invocation(&invocation)
            .expect("begin invocation");
        invocation.state = PluginInvocationState::Succeeded;
        invocation.result_sha256 = Some("c".repeat(64));
        invocation.completed_at = Some(installed_at + Duration::seconds(3));
        store
            .complete_plugin_invocation(&invocation)
            .expect("complete invocation");

        let error = create_backup_bundle(&store, source.path(), &backup_path, "0.1.0", Utc::now())
            .expect_err("missing result artifact must fail backup");
        assert!(error.to_string().contains("invocation evidence"));
        assert!(!backup_path.exists());
        assert_no_online_snapshot_files(source.path());
    }

    #[test]
    fn manifest_rejects_traversal_duplicates_and_secret_claims() {
        let database = BackupBundleEntry {
            path: DATABASE_ENTRY.into(),
            size_bytes: 1,
            sha256: "a".repeat(64),
        };
        let base = BackupBundleManifest {
            schema_version: BACKUP_BUNDLE_SCHEMA_VERSION,
            application_version: "0.1.0".into(),
            created_at: Utc::now(),
            secrets_included: false,
            secret_encryption: None,
            entries: vec![database.clone()],
        };
        assert!(validate_manifest(&base).is_ok());
        let mut secrets = base.clone();
        secrets.secrets_included = true;
        assert!(validate_manifest(&secrets).is_err());
        let mut duplicate = base.clone();
        duplicate.entries.push(database);
        assert!(validate_manifest(&duplicate).is_err());
        let mut traversal = base;
        traversal.entries.push(BackupBundleEntry {
            path: "artifacts/aa/../../local-auth-token".into(),
            size_bytes: 1,
            sha256: "a".repeat(64),
        });
        assert!(validate_manifest(&traversal).is_err());
    }

    #[test]
    fn future_database_schema_is_rejected_before_restore_or_open() {
        let temporary = tempdir().expect("tempdir");
        let database = temporary.path().join("future.sqlite3");
        let connection = Connection::open(&database).expect("database");
        connection
            .execute_batch(
                "CREATE TABLE mcg_schema(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 INSERT INTO mcg_schema(version, applied_at) VALUES (999, CURRENT_TIMESTAMP);",
            )
            .expect("future schema");
        drop(connection);
        assert!(validate_restored_database(&database).is_err());
        assert!(SqliteEventStore::open(&database).is_err());
    }
}
