use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use rusqlite::{Connection, MAIN_DB, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{STORAGE_SCHEMA_VERSION, SqliteEventStore, StorageError};

pub const BACKUP_BUNDLE_SCHEMA_VERSION: u16 = 1;
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
    let _ = fs::remove_file(snapshot);
    result
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
        if sources.len() > MAX_ENTRY_COUNT {
            return Err(StorageError::InvalidData(
                "backup contains too many entries".into(),
            ));
        }
        let mut entries = Vec::with_capacity(sources.len());
        for (path, source) in &sources {
            entries.push(inspect_source(path, source)?);
        }
        let manifest = BackupBundleManifest {
            schema_version: BACKUP_BUNDLE_SCHEMA_VERSION,
            application_version: application_version.to_owned(),
            created_at: now,
            secrets_included: false,
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
    Ok(BackupBundleSummary {
        path: path.to_path_buf(),
        sha256: format!("{:x}", bundle_digest.finalize()),
        size_bytes,
        manifest,
    })
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
    fs::create_dir_all(data_directory)?;
    let nonce = Uuid::new_v4();
    let staging = data_directory.join(format!(".restore-{nonce}"));
    fs::create_dir(&staging)?;
    let result = (|| {
        extract_bundle(bundle, &staging)?;
        let restored_database =
            staging.join(DATABASE_ENTRY.replace('/', std::path::MAIN_SEPARATOR_STR));
        validate_restored_database(&restored_database)?;
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
    if manifest.schema_version != BACKUP_BUNDLE_SCHEMA_VERSION {
        return Err(StorageError::InvalidData(
            "backup schema version is unsupported".into(),
        ));
    }
    if manifest.secrets_included {
        return Err(StorageError::InvalidData(
            "unencrypted backups cannot contain secrets".into(),
        ));
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
    for entry in &manifest.entries {
        validate_entry(entry)?;
        if !paths.insert(&entry.path) {
            return Err(StorageError::InvalidData(
                "backup contains duplicate entry paths".into(),
            ));
        }
        if entry.path == DATABASE_ENTRY {
            database_entries = database_entries.saturating_add(1);
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

fn extract_bundle(bundle: &Path, staging: &Path) -> Result<(), StorageError> {
    let mut input = File::open(bundle)?;
    let (manifest, _) = read_header(&mut input)?;
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
    Ok(())
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
    if database.exists() {
        fs::rename(&database, &rollback)?;
    }
    if let Err(error) = fs::rename(restored, &database) {
        if rollback.exists() {
            let _ = fs::rename(&rollback, &database);
        }
        return Err(error.into());
    }
    sync_parent(data_directory)?;
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

#[cfg_attr(not(unix), allow(clippy::unnecessary_wraps))]
fn sync_parent(_path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    File::open(_path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use mcg_domain::TaskEnvelope;
    use tempfile::tempdir;

    use super::*;

    #[test]
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
        fs::write(source.path().join("local-auth-token"), "private-token").expect("token");

        let summary =
            create_backup_bundle(&store, source.path(), &backup_path, "0.1.0", Utc::now())
                .expect("create backup");
        assert!(!summary.manifest.secrets_included);
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
