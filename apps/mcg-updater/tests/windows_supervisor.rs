#![cfg(windows)]

use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use mcg_updater::{WindowsUpdateOutcome, stage_verified_windows_installer};
use uuid::Uuid;

#[test]
fn real_pe_version_mismatch_is_rejected_before_staging() {
    let temporary = tempfile::tempdir().unwrap();
    let install = temporary.path().join("install");
    let data = temporary.path().join("data");
    fs::create_dir_all(&install).unwrap();
    let system_root = std::env::var_os("SystemRoot").unwrap();
    let installer = fs::read(
        std::path::PathBuf::from(system_root)
            .join("System32")
            .join("curl.exe"),
    )
    .unwrap();
    let actual = mcg_updater::windows_installer_product_version(&installer).unwrap();
    assert_ne!(actual, "0.0.0");

    let error = stage_verified_windows_installer(
        &data,
        &install,
        Uuid::new_v4(),
        1,
        "0.0.0",
        &installer,
        b"packaged supervisor",
    )
    .unwrap_err();
    assert!(error.contains("does not match the reviewed update version"));
    assert!(!data.exists());
}

#[test]
fn real_supervisor_waits_for_desktop_records_failure_and_cleans_staging() {
    let temporary = tempfile::tempdir().unwrap();
    let install = temporary.path().join("install");
    let data = temporary.path().join("data");
    fs::create_dir_all(&install).unwrap();

    let supervisor = install.join("mcg-updater.exe");
    fs::copy(env!("CARGO_BIN_EXE_mcg-updater"), &supervisor).unwrap();
    let system_root = std::env::var_os("SystemRoot").unwrap();
    let system32 = std::path::PathBuf::from(system_root).join("System32");
    let desktop = install.join("mcg-desktop.exe");
    fs::copy(system32.join("PING.EXE"), &desktop).unwrap();

    let mut desktop_process = Command::new(&desktop)
        .args(["-n", "8", "127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let request_id = Uuid::new_v4();
    let installer = fs::read(system32.join("curl.exe")).unwrap();
    let expected_version = mcg_updater::windows_installer_product_version(&installer).unwrap();
    let staged = stage_verified_windows_installer(
        &data,
        &install,
        request_id,
        desktop_process.id(),
        &expected_version,
        &installer,
        &fs::read(&supervisor).unwrap(),
    )
    .unwrap();

    let packaged_supervisor_bytes = fs::read(&supervisor).unwrap();
    let mut update_process = Command::new(&staged.supervisor_path)
        .arg("--apply")
        .arg(&staged.request_path)
        .env("MCG_DATA_DIR", &data)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(750));
    fs::write(&supervisor, b"replaceable while the staged supervisor runs").unwrap();
    fs::write(&supervisor, packaged_supervisor_bytes).unwrap();
    let status = update_process.wait().unwrap();
    assert!(!status.success());
    let _ = desktop_process.wait();
    thread::sleep(Duration::from_millis(500));

    let outcome_path = data
        .join("updates")
        .join(format!("outcome-{request_id}.json"));
    let outcome: WindowsUpdateOutcome =
        serde_json::from_slice(&fs::read(outcome_path).unwrap()).unwrap();
    assert_eq!(outcome.result, "failed");
    assert_eq!(outcome.request_id, request_id);
    assert!(outcome.installer_exit_code.is_some());
    assert!(
        outcome
            .detail
            .contains("restarted the previous installation")
    );
    assert!(
        !outcome
            .detail
            .contains(temporary.path().to_string_lossy().as_ref())
    );
    assert!(!staged.request_path.exists());
    assert!(!staged.installer_path.exists());
    assert!(!staged.supervisor_path.exists());
}
