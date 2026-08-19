use std::{
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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use sysinfo::{Pid, ProcessesToUpdate, System};
use uuid::Uuid;

const REQUEST_SCHEMA: u32 = 1;
#[cfg(windows)]
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
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
}

/// Stages an already signature-verified Windows installer and its bounded supervisor request.
///
/// # Errors
/// Returns an error when the private update directory or files cannot be created safely.
pub fn stage_verified_windows_installer(
    data_directory: &Path,
    request_id: Uuid,
    desktop_pid: u32,
    installer: &[u8],
) -> Result<StagedWindowsUpdate, String> {
    if installer.is_empty()
        || u64::try_from(installer.len()).unwrap_or(u64::MAX) > MAX_INSTALLER_BYTES
    {
        return Err("The verified Windows installer is empty or oversized.".into());
    }
    let updates = private_updates_directory(data_directory)?;
    let installer_name = format!("mcg-update-{request_id}.exe");
    let installer_path = updates.join(&installer_name);
    write_new_file(&installer_path, installer, "verified Windows installer")?;
    sync_parent(&updates)?;

    let installer_sha256 = hex_sha256(installer);
    let request = WindowsUpdateRequest {
        schema_version: REQUEST_SCHEMA,
        request_id,
        desktop_pid,
        installer_sha256,
        installer_name,
    };
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("The Windows update request could not be encoded: {error}"))?;
    let request_path = updates.join(format!("request-{request_id}.json"));
    if let Err(error) = write_new_file(&request_path, &request_bytes, "Windows update request") {
        let _ = fs::remove_file(&installer_path);
        return Err(error);
    }
    sync_parent(&updates)?;
    Ok(StagedWindowsUpdate {
        request_path,
        installer_path,
    })
}

/// Executes one staged Windows update request from the packaged supervisor process.
///
/// This function never interprets caller-provided executable paths. The request and installer
/// must be regular files in the configured private update directory, while the desktop is fixed
/// to the supervisor's installation directory.
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
pub fn recover_packaged_desktop_after_supervisor_error() -> Result<(), String> {
    #[cfg(not(windows))]
    {
        Err("Windows update recovery is available only on Windows.".into())
    }
    #[cfg(windows)]
    {
        let supervisor = std::env::current_exe()
            .map_err(|error| format!("The update supervisor location is unavailable: {error}"))?;
        let install_root = supervisor
            .parent()
            .ok_or_else(|| "The update supervisor has no installation directory.".to_owned())?;
        let desktop = canonical_packaged_desktop(install_root)?;
        thread::sleep(Duration::from_millis(500));
        launch_desktop(
            &desktop,
            &AppConfig::default(),
            Some(
                "The update could not start safely. MCG restarted the previous installation and preserved its data.",
            ),
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

    let supervisor = std::env::current_exe()
        .map_err(|error| format!("The update supervisor location is unavailable: {error}"))?;
    let install_root = supervisor
        .parent()
        .ok_or_else(|| "The update supervisor has no installation directory.".to_owned())?
        .to_path_buf();
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
    Ok(outcome)
}

fn validate_request(request: &WindowsUpdateRequest) -> Result<(), String> {
    if request.schema_version != REQUEST_SCHEMA || request.desktop_pid == 0 {
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
    let expected_name = format!("mcg-update-{}.exe", request.request_id);
    if request.installer_name != expected_name {
        return Err("The Windows installer name is invalid.".into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_exact_bounded_request_and_refuses_reuse() {
        let temporary = tempfile::tempdir().unwrap();
        let data = temporary.path().join("data");
        let request_id = Uuid::new_v4();
        let staged =
            stage_verified_windows_installer(&data, request_id, 42, b"signed installer").unwrap();
        let request: WindowsUpdateRequest =
            serde_json::from_slice(&fs::read(&staged.request_path).unwrap()).unwrap();
        assert_eq!(request.schema_version, 1);
        assert_eq!(request.request_id, request_id);
        assert_eq!(request.desktop_pid, 42);
        assert_eq!(
            request.installer_name,
            format!("mcg-update-{request_id}.exe")
        );
        assert_eq!(request.installer_sha256, hex_sha256(b"signed installer"));
        assert!(stage_verified_windows_installer(&data, request_id, 42, b"replacement").is_err());
    }

    #[test]
    fn request_contract_rejects_noncanonical_identity_and_digest() {
        let request_id = Uuid::new_v4();
        let valid = WindowsUpdateRequest {
            schema_version: 1,
            request_id,
            desktop_pid: 9,
            installer_sha256: "A".repeat(64),
            installer_name: format!("mcg-update-{request_id}.exe"),
        };
        assert!(validate_request(&valid).is_ok());
        let mut invalid = valid.clone();
        invalid.installer_sha256 = "a".repeat(64);
        assert!(validate_request(&invalid).is_err());
        let mut invalid = valid;
        invalid.installer_name = "other.exe".into();
        assert!(validate_request(&invalid).is_err());
    }

    #[test]
    fn staging_rejects_empty_relative_and_linked_roots() {
        let temporary = tempfile::tempdir().unwrap();
        assert!(
            stage_verified_windows_installer(Path::new("relative"), Uuid::new_v4(), 1, b"x")
                .is_err()
        );
        assert!(
            stage_verified_windows_installer(temporary.path(), Uuid::new_v4(), 1, b"").is_err()
        );
        let file = temporary.path().join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        assert!(stage_verified_windows_installer(&file, Uuid::new_v4(), 1, b"x").is_err());
    }
}
