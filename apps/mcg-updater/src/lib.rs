use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
#[cfg(windows)]
use std::{
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use mcg_config::AppConfig;
use object::{
    FileKind,
    endian::LittleEndian as LE,
    pe,
    read::pe::{PeFile32, PeFile64, ResourceDirectory, ResourceDirectoryEntryData, SectionTable},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use sysinfo::{Pid, ProcessesToUpdate, System};
use uuid::Uuid;

const REQUEST_SCHEMA: u32 = 3;
#[cfg(windows)]
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_SUPERVISOR_BYTES: u64 = 64 * 1024 * 1024;
const MAX_INSTALLER_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(windows)]
const PARENT_EXIT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(windows)]
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsUpdateRequest {
    pub schema_version: u32,
    pub request_id: Uuid,
    pub desktop_pid: u32,
    pub installer_sha256: String,
    pub installer_name: String,
    pub expected_version: String,
    pub installer_product_version: String,
    pub supervisor_sha256: String,
    pub supervisor_name: String,
    pub install_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsUpdateOutcome {
    pub schema_version: u32,
    pub request_id: Uuid,
    pub result: String,
    pub installer_exit_code: Option<i32>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedWindowsUpdate {
    pub request_path: PathBuf,
    pub installer_path: PathBuf,
    pub supervisor_path: PathBuf,
}

/// Stages an already signature-verified Windows installer and its bounded supervisor request.
///
/// # Errors
/// Returns an error when the private update directory or files cannot be created safely.
pub fn stage_verified_windows_installer(
    data_directory: &Path,
    install_root: &Path,
    request_id: Uuid,
    desktop_pid: u32,
    expected_version: &str,
    installer: &[u8],
    supervisor: &[u8],
) -> Result<StagedWindowsUpdate, String> {
    if installer.is_empty()
        || u64::try_from(installer.len()).unwrap_or(u64::MAX) > MAX_INSTALLER_BYTES
    {
        return Err("The verified Windows installer is empty or oversized.".into());
    }
    if supervisor.is_empty()
        || u64::try_from(supervisor.len()).unwrap_or(u64::MAX) > MAX_SUPERVISOR_BYTES
    {
        return Err("The packaged Windows update supervisor is empty or oversized.".into());
    }
    let expected_version = canonical_version(expected_version, "expected update")?;
    let installer_product_version = windows_installer_product_version(installer)?;
    if installer_product_version != expected_version {
        return Err(format!(
            "The signed Windows installer version {installer_product_version} does not match the reviewed update version {expected_version}."
        ));
    }
    stage_windows_installer(
        data_directory,
        install_root,
        request_id,
        desktop_pid,
        expected_version,
        installer_product_version,
        installer,
        supervisor,
    )
}

#[allow(clippy::too_many_arguments)]
fn stage_windows_installer(
    data_directory: &Path,
    install_root: &Path,
    request_id: Uuid,
    desktop_pid: u32,
    expected_version: String,
    installer_product_version: String,
    installer: &[u8],
    supervisor: &[u8],
) -> Result<StagedWindowsUpdate, String> {
    #[cfg(windows)]
    let install_root = canonical_install_root(install_root)?;
    #[cfg(not(windows))]
    let install_root = install_root.to_path_buf();
    let updates = private_updates_directory(data_directory)?;
    let installer_name = format!("mcg-update-{request_id}.exe");
    let installer_path = updates.join(&installer_name);
    write_new_file(&installer_path, installer, "verified Windows installer")?;
    let supervisor_name = format!("mcg-update-supervisor-{request_id}.exe");
    let supervisor_path = updates.join(&supervisor_name);
    if let Err(error) = write_new_file(
        &supervisor_path,
        supervisor,
        "staged Windows update supervisor",
    ) {
        let _ = fs::remove_file(&installer_path);
        return Err(error);
    }
    sync_parent(&updates)?;

    let installer_sha256 = hex_sha256(installer);
    let request = WindowsUpdateRequest {
        schema_version: REQUEST_SCHEMA,
        request_id,
        desktop_pid,
        installer_sha256,
        installer_name,
        expected_version,
        installer_product_version,
        supervisor_sha256: hex_sha256(supervisor),
        supervisor_name,
        install_root,
    };
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("The Windows update request could not be encoded: {error}"))?;
    let request_path = updates.join(format!("request-{request_id}.json"));
    if let Err(error) = write_new_file(&request_path, &request_bytes, "Windows update request") {
        let _ = fs::remove_file(&installer_path);
        let _ = fs::remove_file(&supervisor_path);
        return Err(error);
    }
    sync_parent(&updates)?;
    Ok(StagedWindowsUpdate {
        request_path,
        installer_path,
        supervisor_path,
    })
}

/// Executes one staged Windows update request from the packaged supervisor process.
///
/// This function never interprets caller-provided executable paths. The request and installer
/// must be regular files in the configured private update directory. The staged supervisor and
/// unchanged packaged supervisor are digest-matched before the desktop is resolved from the
/// canonical installation directory retained by the packaged desktop.
///
/// # Errors
/// Returns an error when validation, process supervision, outcome retention, or recovery fails.
pub fn run_staged_windows_update(request_path: &Path) -> Result<WindowsUpdateOutcome, String> {
    #[cfg(not(windows))]
    {
        let _ = request_path;
        Err("The packaged update supervisor is available only on Windows.".into())
    }
    #[cfg(windows)]
    {
        run_staged_windows_update_inner(request_path)
    }
}

/// Restarts the fixed adjacent packaged desktop after a supervisor-level failure.
///
/// # Errors
/// Returns an error when the packaged desktop identity or launch cannot be verified.
pub fn recover_packaged_desktop_after_supervisor_error(request_path: &Path) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        Err("Windows update recovery is available only on Windows.".into())
    }
    #[cfg(windows)]
    {
        let config = AppConfig::default();
        let updates = canonical_private_updates_directory(&config.data_directory)?;
        let request_path =
            canonical_regular_child(request_path, &updates, MAX_REQUEST_BYTES, "request")?;
        let request: WindowsUpdateRequest =
            serde_json::from_slice(&fs::read(&request_path).map_err(|error| {
                format!("The Windows update request could not be read: {error}")
            })?)
            .map_err(|_| "The Windows update request is invalid.".to_owned())?;
        validate_request(&request)?;
        let staged_supervisor = validate_running_staged_supervisor(&request, &updates)?;
        let install_root = canonical_install_root(&request.install_root)?;
        validate_packaged_supervisor(&install_root, &request.supervisor_sha256)?;
        let desktop = canonical_packaged_desktop(&install_root)?;
        thread::sleep(Duration::from_millis(500));
        launch_desktop(
            &desktop,
            &config,
            Some(
                "The update could not start safely. MCG restarted the previous installation and preserved its data.",
            ),
        )?;
        launch_supervisor_cleanup(
            &install_root.join("mcg-updater.exe"),
            &staged_supervisor,
            std::process::id(),
            &config,
        )
    }
}

#[cfg(windows)]
fn run_staged_windows_update_inner(request_path: &Path) -> Result<WindowsUpdateOutcome, String> {
    let config = AppConfig::default();
    let updates = canonical_private_updates_directory(&config.data_directory)?;
    let request_path =
        canonical_regular_child(request_path, &updates, MAX_REQUEST_BYTES, "request")?;
    let mut request_bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .open(&request_path)
        .and_then(|mut file| file.read_to_end(&mut request_bytes))
        .map_err(|error| format!("The Windows update request could not be read: {error}"))?;
    let request: WindowsUpdateRequest = serde_json::from_slice(&request_bytes)
        .map_err(|_| "The Windows update request is invalid.".to_owned())?;
    validate_request(&request)?;

    let installer_path = canonical_regular_child(
        &updates.join(&request.installer_name),
        &updates,
        MAX_INSTALLER_BYTES,
        "installer",
    )?;
    let actual_hash = file_sha256(&installer_path)?;
    if actual_hash != request.installer_sha256 {
        return Err("The staged Windows installer digest changed.".into());
    }

    let staged_supervisor = validate_running_staged_supervisor(&request, &updates)?;
    let install_root = canonical_install_root(&request.install_root)?;
    validate_packaged_supervisor(&install_root, &request.supervisor_sha256)?;
    let desktop = canonical_packaged_desktop(&install_root)?;
    wait_for_desktop_exit(request.desktop_pid, &desktop)?;

    let status = launch_installer(&installer_path)?;
    let succeeded = status.success();
    let outcome = WindowsUpdateOutcome {
        schema_version: REQUEST_SCHEMA,
        request_id: request.request_id,
        result: if succeeded { "succeeded" } else { "failed" }.into(),
        installer_exit_code: status.code(),
        detail: if succeeded {
            "The verified Windows update installer completed.".into()
        } else {
            "The update did not complete. MCG restarted the previous installation and preserved its data.".into()
        },
    };
    write_outcome(&updates, &outcome)?;
    let _ = fs::remove_file(&installer_path);
    let _ = fs::remove_file(&request_path);

    if !succeeded {
        launch_desktop(&desktop, &config, Some(&outcome.detail))?;
    }
    launch_supervisor_cleanup(
        &install_root.join("mcg-updater.exe"),
        &staged_supervisor,
        std::process::id(),
        &config,
    )?;
    Ok(outcome)
}

fn validate_request(request: &WindowsUpdateRequest) -> Result<(), String> {
    if request.schema_version != REQUEST_SCHEMA
        || request.desktop_pid == 0
        || !request.install_root.is_absolute()
    {
        return Err("The Windows update request contract is invalid.".into());
    }
    if request.installer_sha256.len() != 64
        || !request
            .installer_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || request.installer_sha256 != request.installer_sha256.to_ascii_uppercase()
    {
        return Err("The Windows installer digest is invalid.".into());
    }
    let expected_version = canonical_version(&request.expected_version, "expected update")?;
    let installer_product_version = canonical_version(
        &request.installer_product_version,
        "Windows installer product",
    )?;
    if expected_version != installer_product_version {
        return Err("The Windows installer version binding is invalid.".into());
    }
    let expected_name = format!("mcg-update-{}.exe", request.request_id);
    if request.installer_name != expected_name {
        return Err("The Windows installer name is invalid.".into());
    }
    let expected_supervisor = format!("mcg-update-supervisor-{}.exe", request.request_id);
    if request.supervisor_name != expected_supervisor
        || request.supervisor_sha256.len() != 64
        || !request
            .supervisor_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || request.supervisor_sha256 != request.supervisor_sha256.to_ascii_uppercase()
    {
        return Err("The Windows update supervisor identity is invalid.".into());
    }
    Ok(())
}

fn canonical_version(value: &str, label: &str) -> Result<String, String> {
    if value.is_empty() || value.len() > 128 || value.trim() != value {
        return Err(format!("The {label} version is invalid."));
    }
    let version = Version::parse(value).map_err(|_| format!("The {label} version is invalid."))?;
    if version.to_string() != value {
        return Err(format!("The {label} version is not canonical."));
    }
    Ok(value.to_owned())
}

/// Reads the canonical `ProductVersion` embedded in a Windows installer's PE resources.
///
/// Every version resource must contain the same canonical semantic version. This intentionally
/// does not trust the signed feed URL or filename: the value comes from the exact installer bytes
/// whose updater signature has already been verified.
///
/// # Errors
/// Returns an error for malformed/non-PE input, missing or conflicting version resources, or a
/// product version that is not canonical semantic version text.
pub fn windows_installer_product_version(installer: &[u8]) -> Result<String, String> {
    if installer.is_empty()
        || u64::try_from(installer.len()).unwrap_or(u64::MAX) > MAX_INSTALLER_BYTES
    {
        return Err("The verified Windows installer is empty or oversized.".into());
    }
    let resources = match FileKind::parse(installer)
        .map_err(|_| "The verified Windows installer is not a valid PE image.".to_owned())?
    {
        FileKind::Pe32 => {
            let file = PeFile32::parse(installer)
                .map_err(|_| "The verified Windows installer PE image is invalid.".to_owned())?;
            version_resource_blobs(
                installer,
                file.data_directories()
                    .resource_directory(installer, &file.section_table())
                    .map_err(|_| {
                        "The Windows installer resource directory is invalid.".to_owned()
                    })?,
                file.section_table(),
            )?
        }
        FileKind::Pe64 => {
            let file = PeFile64::parse(installer)
                .map_err(|_| "The verified Windows installer PE image is invalid.".to_owned())?;
            version_resource_blobs(
                installer,
                file.data_directories()
                    .resource_directory(installer, &file.section_table())
                    .map_err(|_| {
                        "The Windows installer resource directory is invalid.".to_owned()
                    })?,
                file.section_table(),
            )?
        }
        _ => return Err("The verified Windows installer is not a PE image.".into()),
    };
    if resources.is_empty() {
        return Err("The Windows installer has no version resource.".into());
    }
    canonical_product_version(&resources)
}

fn canonical_product_version(resources: &[&[u8]]) -> Result<String, String> {
    let mut versions = BTreeSet::new();
    for resource in resources {
        let values = product_versions_from_resource(resource)?;
        if values.is_empty() {
            return Err("A Windows installer version resource has no ProductVersion.".into());
        }
        for value in values {
            versions.insert(canonical_version(&value, "Windows installer product")?);
        }
    }
    if versions.len() != 1 {
        return Err("The Windows installer ProductVersion resources conflict.".into());
    }
    versions
        .into_iter()
        .next()
        .ok_or_else(|| "The Windows installer has no ProductVersion.".to_owned())
}

fn version_resource_blobs<'data>(
    installer: &'data [u8],
    directory: Option<ResourceDirectory<'data>>,
    sections: SectionTable<'data>,
) -> Result<Vec<&'data [u8]>, String> {
    let directory =
        directory.ok_or_else(|| "The Windows installer has no resource directory.".to_owned())?;
    let root = directory
        .root()
        .map_err(|_| "The Windows installer resource root is invalid.".to_owned())?;
    let version_entries = root
        .entries
        .iter()
        .filter(|entry| entry.name_or_id().id() == Some(pe::RT_VERSION))
        .collect::<Vec<_>>();
    if version_entries.len() != 1 {
        return Err("The Windows installer version resource identity is ambiguous.".into());
    }
    let names = version_entries[0]
        .data(directory)
        .map_err(|_| "The Windows installer version resource is invalid.".to_owned())?
        .table()
        .ok_or_else(|| "The Windows installer version resource table is missing.".to_owned())?;

    let mut blobs = Vec::new();
    for name in names.entries {
        let languages = name
            .data(directory)
            .map_err(|_| "The Windows installer version name table is invalid.".to_owned())?
            .table()
            .ok_or_else(|| "The Windows installer version language table is missing.".to_owned())?;
        for language in languages.entries {
            let data = match language.data(directory).map_err(|_| {
                "The Windows installer version language entry is invalid.".to_owned()
            })? {
                ResourceDirectoryEntryData::Data(data) => data,
                ResourceDirectoryEntryData::Table(_) => {
                    return Err("The Windows installer version resource is over-nested.".into());
                }
            };
            let size = usize::try_from(data.size.get(LE))
                .map_err(|_| "The Windows installer version resource is oversized.".to_owned())?;
            let containing = sections
                .pe_data_at(installer, data.offset_to_data.get(LE))
                .ok_or_else(|| "The Windows installer version bytes are unavailable.".to_owned())?;
            let blob = containing
                .get(..size)
                .ok_or_else(|| "The Windows installer version bytes are truncated.".to_owned())?;
            if blob.is_empty() || blob.len() > 1024 * 1024 {
                return Err("The Windows installer version resource is empty or oversized.".into());
            }
            blobs.push(blob);
        }
    }
    Ok(blobs)
}

fn product_versions_from_resource(resource: &[u8]) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut nodes = 0_usize;
    let mut path = Vec::new();
    let consumed = parse_version_block(resource, 0, 0, &mut nodes, &mut path, &mut values)?;
    if resource
        .get(consumed..)
        .is_some_and(|trailing| trailing.iter().any(|byte| *byte != 0))
    {
        return Err("The Windows installer version resource has trailing data.".into());
    }
    Ok(values)
}

fn parse_version_block(
    resource: &[u8],
    start: usize,
    depth: usize,
    nodes: &mut usize,
    path: &mut Vec<String>,
    values: &mut Vec<String>,
) -> Result<usize, String> {
    *nodes += 1;
    if depth > 8 || *nodes > 512 {
        return Err("The Windows installer version resource is too complex.".into());
    }
    let length = usize::from(read_u16(resource, start)?);
    let value_length = usize::from(read_u16(resource, start + 2)?);
    let value_type = read_u16(resource, start + 4)?;
    if length < 8 {
        return Err("The Windows installer version block is invalid.".into());
    }
    let end = start
        .checked_add(length)
        .filter(|end| *end <= resource.len())
        .ok_or_else(|| "The Windows installer version block is truncated.".to_owned())?;

    let mut key_cursor = start + 6;
    let mut key_units = Vec::new();
    loop {
        if key_cursor.checked_add(2).is_none_or(|next| next > end) {
            return Err("The Windows installer version key is truncated.".into());
        }
        let unit = read_u16(resource, key_cursor)?;
        key_cursor += 2;
        if unit == 0 {
            break;
        }
        if key_cursor > end || key_units.len() >= 128 {
            return Err("The Windows installer version key is invalid.".into());
        }
        key_units.push(unit);
    }
    let key = String::from_utf16(&key_units)
        .map_err(|_| "The Windows installer version key is invalid UTF-16.".to_owned())?;
    if depth == 0 && key != "VS_VERSION_INFO" {
        return Err("The Windows installer version root identity is invalid.".into());
    }
    let value_start = align_four(key_cursor)
        .filter(|offset| *offset <= end)
        .ok_or_else(|| "The Windows installer version value offset is invalid.".to_owned())?;
    let value_bytes = if value_type == 1 {
        value_length
            .checked_mul(2)
            .ok_or_else(|| "The Windows installer text value is oversized.".to_owned())?
    } else {
        value_length
    };
    let value_end = value_start
        .checked_add(value_bytes)
        .filter(|offset| *offset <= end)
        .ok_or_else(|| "The Windows installer version value is truncated.".to_owned())?;

    if let Some(product_version) = product_version_value(
        &key,
        value_type,
        value_length,
        &resource[value_start..value_end],
        depth,
        path,
    )? {
        values.push(product_version);
    }

    if value_end == end {
        return Ok(end);
    }
    let Some(mut child) = align_four(value_end) else {
        return Err("The Windows installer child block offset is invalid.".into());
    };
    if child > end {
        if resource[value_end..end].iter().all(|byte| *byte == 0) {
            return Ok(end);
        }
        return Err("The Windows installer child block offset is invalid.".into());
    }
    path.push(key);
    while child < end {
        if resource[child..end].iter().all(|byte| *byte == 0) {
            break;
        }
        let child_length = usize::from(read_u16(resource, child)?);
        if child_length == 0 {
            return Err("The Windows installer child version block is invalid.".into());
        }
        let child_end = parse_version_block(resource, child, depth + 1, nodes, path, values)?;
        child = align_four(child_end)
            .filter(|offset| *offset <= end)
            .ok_or_else(|| "The Windows installer child block exceeds its parent.".to_owned())?;
    }
    path.pop();
    Ok(end)
}

fn product_version_value(
    key: &str,
    value_type: u16,
    value_length: usize,
    value: &[u8],
    depth: usize,
    path: &[String],
) -> Result<Option<String>, String> {
    if key != "ProductVersion" {
        return Ok(None);
    }
    let is_string_table = path.get(2).is_some_and(|table| {
        table.len() == 8 && table.bytes().all(|byte| byte.is_ascii_hexdigit())
    });
    if depth != 3
        || path.first().map(String::as_str) != Some("VS_VERSION_INFO")
        || path.get(1).map(String::as_str) != Some("StringFileInfo")
        || !is_string_table
    {
        return Err("The Windows installer ProductVersion hierarchy is invalid.".into());
    }
    if value_type != 1 || value.is_empty() || !value.len().is_multiple_of(2) {
        return Err("The Windows installer ProductVersion value is invalid.".into());
    }
    let mut units = Vec::with_capacity(value_length);
    for pair in value.chunks_exact(2) {
        units.push(u16::from_le_bytes([pair[0], pair[1]]));
    }
    while units.last() == Some(&0) {
        units.pop();
    }
    if units.is_empty() || units.contains(&0) {
        return Err("The Windows installer ProductVersion text is invalid.".into());
    }
    String::from_utf16(&units)
        .map(Some)
        .map_err(|_| "The Windows installer ProductVersion is invalid UTF-16.".to_owned())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let pair = bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|slice| <[u8; 2]>::try_from(slice).ok())
        .ok_or_else(|| "The Windows installer version resource is truncated.".to_owned())?;
    Ok(u16::from_le_bytes(pair))
}

fn align_four(offset: usize) -> Option<usize> {
    offset.checked_add(3).map(|value| value & !3)
}

fn private_updates_directory(data_directory: &Path) -> Result<PathBuf, String> {
    if data_directory.as_os_str().is_empty() || !data_directory.is_absolute() {
        return Err("The MCG data directory must be absolute before staging an update.".into());
    }
    fs::create_dir_all(data_directory)
        .map_err(|error| format!("The MCG data directory could not be prepared: {error}"))?;
    reject_link(data_directory, "MCG data directory")?;
    let updates = data_directory.join("updates");
    fs::create_dir_all(&updates)
        .map_err(|error| format!("The private update directory could not be prepared: {error}"))?;
    reject_link(&updates, "private update directory")?;
    updates
        .canonicalize()
        .map_err(|error| format!("The private update directory could not be resolved: {error}"))
}

#[cfg(windows)]
fn canonical_private_updates_directory(data_directory: &Path) -> Result<PathBuf, String> {
    let updates = data_directory.join("updates");
    reject_link(&updates, "private update directory")?;
    updates
        .canonicalize()
        .map_err(|error| format!("The private update directory could not be resolved: {error}"))
}

fn reject_link(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("The {label} could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("The {label} must be a regular directory."));
    }
    Ok(())
}

#[cfg(windows)]
fn canonical_regular_child(
    path: &Path,
    parent: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("The Windows update {label} could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        return Err(format!(
            "The Windows update {label} is not a bounded regular file."
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("The Windows update {label} could not be resolved: {error}"))?;
    if canonical.parent() != Some(parent) {
        return Err(format!(
            "The Windows update {label} escaped the private update directory."
        ));
    }
    Ok(canonical)
}

#[cfg(windows)]
fn canonical_install_root(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("The MCG installation directory must be absolute.".into());
    }
    reject_link(path, "MCG installation directory")?;
    path.canonicalize()
        .map_err(|error| format!("The MCG installation directory could not be resolved: {error}"))
}

#[cfg(windows)]
fn validate_running_staged_supervisor(
    request: &WindowsUpdateRequest,
    updates: &Path,
) -> Result<PathBuf, String> {
    let expected = canonical_regular_child(
        &updates.join(&request.supervisor_name),
        updates,
        MAX_SUPERVISOR_BYTES,
        "supervisor",
    )?;
    let current = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| format!("The update supervisor location is unavailable: {error}"))?;
    if current != expected || file_sha256(&current)? != request.supervisor_sha256 {
        return Err("The running Windows update supervisor identity changed.".into());
    }
    Ok(current)
}

#[cfg(windows)]
fn validate_packaged_supervisor(
    install_root: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, String> {
    let candidate = install_root.join("mcg-updater.exe");
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        format!("The packaged update supervisor could not be inspected: {error}")
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_SUPERVISOR_BYTES
    {
        return Err("The packaged update supervisor is not a bounded regular file.".into());
    }
    let canonical = candidate.canonicalize().map_err(|error| {
        format!("The packaged update supervisor could not be resolved: {error}")
    })?;
    if canonical.parent() != Some(install_root) || file_sha256(&canonical)? != expected_sha256 {
        return Err("The packaged update supervisor identity changed.".into());
    }
    Ok(canonical)
}

fn write_new_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("The {label} could not be created: {error}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("The {label} could not be retained: {error}"))
}

fn sync_parent(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        fs::metadata(path).map(drop).map_err(|error| {
            format!("The private update directory could not be rechecked: {error}")
        })
    }
    #[cfg(not(windows))]
    {
        OpenOptions::new()
            .read(true)
            .open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!("The private update directory could not be synchronized: {error}")
            })
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:X}", Sha256::digest(bytes))
}

#[cfg(windows)]
fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("The staged Windows installer could not be read: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!("The staged Windows installer could not be hashed: {error}")
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:X}", hasher.finalize()))
}

#[cfg(windows)]
fn canonical_packaged_desktop(install_root: &Path) -> Result<PathBuf, String> {
    let candidate = install_root.join("mcg-desktop.exe");
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("The packaged MCG desktop could not be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("The packaged MCG desktop is not a regular installation file.".into());
    }
    candidate
        .canonicalize()
        .map_err(|error| format!("The packaged MCG desktop could not be resolved: {error}"))
}

#[cfg(windows)]
fn wait_for_desktop_exit(desktop_pid: u32, desktop: &Path) -> Result<(), String> {
    let pid = Pid::from_u32(desktop_pid);
    let started = Instant::now();
    let mut system = System::new_all();
    if let Some(process) = system.process(pid) {
        let executable = process
            .exe()
            .and_then(|path| path.canonicalize().ok())
            .ok_or_else(|| "The requesting MCG desktop identity is unavailable.".to_owned())?;
        if executable != desktop {
            return Err(
                "The Windows update request did not originate from the packaged desktop.".into(),
            );
        }
    }
    while started.elapsed() < PARENT_EXIT_TIMEOUT {
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if system.process(pid).is_none() {
            return Ok(());
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
    Err("The MCG desktop did not exit before the supervised update timeout.".into())
}

#[cfg(windows)]
fn launch_installer(installer: &Path) -> Result<ExitStatus, String> {
    Command::new(installer)
        .args(["/P", "/R", "/UPDATE", "/ARGS"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("The verified Windows installer could not be supervised: {error}"))
}

#[cfg(windows)]
fn write_outcome(updates: &Path, outcome: &WindowsUpdateOutcome) -> Result<(), String> {
    let bytes = serde_json::to_vec(outcome)
        .map_err(|error| format!("The Windows update outcome could not be encoded: {error}"))?;
    let path = updates.join(format!("outcome-{}.json", outcome.request_id));
    write_new_file(&path, &bytes, "Windows update outcome")?;
    sync_parent(updates)
}

#[cfg(windows)]
fn launch_desktop(
    desktop: &Path,
    config: &AppConfig,
    recovery_notice: Option<&str>,
) -> Result<(), String> {
    let mut command = Command::new(desktop);
    command
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(notice) = recovery_notice {
        command.env("MCG_UPDATE_RECOVERY_NOTICE", notice);
    }
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map(drop)
        .map_err(|error| format!("The previous MCG installation could not be restarted: {error}"))
}

#[cfg(windows)]
fn launch_supervisor_cleanup(
    packaged_supervisor: &Path,
    staged_supervisor: &Path,
    parent_pid: u32,
    config: &AppConfig,
) -> Result<(), String> {
    let mut command = Command::new(packaged_supervisor);
    command
        .arg("--cleanup")
        .arg(staged_supervisor)
        .arg(parent_pid.to_string())
        .env("MCG_DATA_DIR", &config.data_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map(drop)
        .map_err(|error| format!("The staged update supervisor cleanup could not start: {error}"))
}

/// Waits for a staged supervisor to exit and removes only that exact private copy.
///
/// # Errors
/// Returns an error when the packaged cleanup identity, parent process, or exact staged file
/// cannot be validated and cleaned within the bounded timeout.
pub fn cleanup_staged_windows_supervisor(
    staged_supervisor: &Path,
    parent_pid: u32,
) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (staged_supervisor, parent_pid);
        Err("Windows update cleanup is available only on Windows.".into())
    }
    #[cfg(windows)]
    {
        if parent_pid == 0 || parent_pid == std::process::id() {
            return Err("The staged update cleanup parent identity is invalid.".into());
        }
        let config = AppConfig::default();
        let updates = canonical_private_updates_directory(&config.data_directory)?;
        let staged = canonical_regular_child(
            staged_supervisor,
            &updates,
            MAX_SUPERVISOR_BYTES,
            "supervisor cleanup target",
        )?;
        let name = staged
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "The staged update supervisor name is invalid.".to_owned())?;
        let request_id = name
            .strip_prefix("mcg-update-supervisor-")
            .and_then(|value| value.strip_suffix(".exe"))
            .ok_or_else(|| "The staged update supervisor name is invalid.".to_owned())?;
        Uuid::parse_str(request_id)
            .map_err(|_| "The staged update supervisor name is invalid.".to_owned())?;

        let current = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|error| format!("The packaged cleanup helper is unavailable: {error}"))?;
        if current.file_name().and_then(|value| value.to_str()) != Some("mcg-updater.exe")
            || current.parent() == Some(updates.as_path())
        {
            return Err(
                "The staged update cleanup was not launched by a packaged supervisor.".into(),
            );
        }

        let pid = Pid::from_u32(parent_pid);
        let started = Instant::now();
        let mut system = System::new_all();
        while started.elapsed() < PARENT_EXIT_TIMEOUT {
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if system.process(pid).is_none() {
                fs::remove_file(&staged).map_err(|error| {
                    format!("The staged update supervisor could not be removed: {error}")
                })?;
                return Ok(());
            }
            thread::sleep(PROCESS_POLL_INTERVAL);
        }
        Err("The staged update supervisor did not exit before cleanup timed out.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_block(key: &str, value_type: u16, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
        let value_length = if value_type == 1 {
            u16::try_from(value.len() / 2).unwrap()
        } else {
            u16::try_from(value.len()).unwrap()
        };
        let mut block = vec![0_u8; 6];
        for unit in key.encode_utf16().chain([0]) {
            block.extend_from_slice(&unit.to_le_bytes());
        }
        while !block.len().is_multiple_of(4) {
            block.push(0);
        }
        block.extend_from_slice(value);
        while !block.len().is_multiple_of(4) {
            block.push(0);
        }
        for child in children {
            block.extend_from_slice(child);
            while !block.len().is_multiple_of(4) {
                block.push(0);
            }
        }
        let length = u16::try_from(block.len()).unwrap();
        block[0..2].copy_from_slice(&length.to_le_bytes());
        block[2..4].copy_from_slice(&value_length.to_le_bytes());
        block[4..6].copy_from_slice(&value_type.to_le_bytes());
        block
    }

    fn text_value(value: &str) -> Vec<u8> {
        value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    fn product_version_resource(version: &str) -> Vec<u8> {
        let product = version_block("ProductVersion", 1, &text_value(version), &[]);
        let table = version_block("040904B0", 1, &[], &[product]);
        let strings = version_block("StringFileInfo", 1, &[], &[table]);
        version_block("VS_VERSION_INFO", 0, &[], &[strings])
    }

    #[test]
    fn version_resource_requires_one_canonical_consistent_product_version() {
        let first = product_version_resource("1.2.3-uat.4");
        let same = product_version_resource("1.2.3-uat.4");
        assert_eq!(
            canonical_product_version(&[&first, &same]).unwrap(),
            "1.2.3-uat.4"
        );

        let conflicting = product_version_resource("1.2.3-uat.5");
        assert!(canonical_product_version(&[&first, &conflicting]).is_err());
        let noncanonical = product_version_resource("01.2.3");
        assert!(canonical_product_version(&[&noncanonical]).is_err());
    }

    #[test]
    fn stages_exact_bounded_request_and_refuses_reuse() {
        let temporary = tempfile::tempdir().unwrap();
        let data = temporary.path().join("data");
        let install = temporary.path().join("install");
        fs::create_dir(&install).unwrap();
        let request_id = Uuid::new_v4();
        let staged = stage_windows_installer(
            &data,
            &install,
            request_id,
            42,
            "1.2.3".into(),
            "1.2.3".into(),
            b"signed installer",
            b"packaged supervisor",
        )
        .unwrap();
        let request: WindowsUpdateRequest =
            serde_json::from_slice(&fs::read(&staged.request_path).unwrap()).unwrap();
        assert_eq!(request.schema_version, 3);
        assert_eq!(request.request_id, request_id);
        assert_eq!(request.desktop_pid, 42);
        assert_eq!(
            request.installer_name,
            format!("mcg-update-{request_id}.exe")
        );
        assert_eq!(request.installer_sha256, hex_sha256(b"signed installer"));
        assert_eq!(request.expected_version, "1.2.3");
        assert_eq!(request.installer_product_version, "1.2.3");
        assert_eq!(
            request.supervisor_name,
            format!("mcg-update-supervisor-{request_id}.exe")
        );
        assert_eq!(
            request.supervisor_sha256,
            hex_sha256(b"packaged supervisor")
        );
        assert_eq!(request.install_root, install.canonicalize().unwrap());
        assert!(
            stage_windows_installer(
                &data,
                &install,
                request_id,
                42,
                "1.2.3".into(),
                "1.2.3".into(),
                b"replacement",
                b"packaged supervisor",
            )
            .is_err()
        );
    }

    #[test]
    fn request_contract_rejects_noncanonical_identity_and_digest() {
        let request_id = Uuid::new_v4();
        let valid = WindowsUpdateRequest {
            schema_version: 3,
            request_id,
            desktop_pid: 9,
            installer_sha256: "A".repeat(64),
            installer_name: format!("mcg-update-{request_id}.exe"),
            expected_version: "1.2.3".into(),
            installer_product_version: "1.2.3".into(),
            supervisor_sha256: "B".repeat(64),
            supervisor_name: format!("mcg-update-supervisor-{request_id}.exe"),
            install_root: std::env::current_dir().unwrap(),
        };
        assert!(validate_request(&valid).is_ok());
        let mut invalid = valid.clone();
        invalid.installer_sha256 = "a".repeat(64);
        assert!(validate_request(&invalid).is_err());
        let mut invalid = valid;
        invalid.installer_name = "other.exe".into();
        assert!(validate_request(&invalid).is_err());

        let mut invalid = WindowsUpdateRequest {
            schema_version: 3,
            request_id,
            desktop_pid: 9,
            installer_sha256: "A".repeat(64),
            installer_name: format!("mcg-update-{request_id}.exe"),
            expected_version: "1.2.4".into(),
            installer_product_version: "1.2.3".into(),
            supervisor_sha256: "B".repeat(64),
            supervisor_name: format!("mcg-update-supervisor-{request_id}.exe"),
            install_root: std::env::current_dir().unwrap(),
        };
        assert!(validate_request(&invalid).is_err());
        invalid.installer_product_version = "1.2.4".into();
        assert!(validate_request(&invalid).is_ok());
    }

    #[test]
    fn staging_rejects_empty_relative_and_linked_roots() {
        let temporary = tempfile::tempdir().unwrap();
        let install = temporary.path().join("install");
        fs::create_dir(&install).unwrap();
        assert!(
            stage_verified_windows_installer(
                Path::new("relative"),
                &install,
                Uuid::new_v4(),
                1,
                "1.2.3",
                b"x",
                b"y"
            )
            .is_err()
        );
        assert!(
            stage_verified_windows_installer(
                temporary.path(),
                &install,
                Uuid::new_v4(),
                1,
                "1.2.3",
                b"",
                b"y"
            )
            .is_err()
        );
        let file = temporary.path().join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        assert!(
            stage_verified_windows_installer(
                &file,
                &install,
                Uuid::new_v4(),
                1,
                "1.2.3",
                b"x",
                b"y"
            )
            .is_err()
        );
    }
}
