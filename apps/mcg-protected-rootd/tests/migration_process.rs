use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use mcg_domain::Conversation;
use mcg_platform::{query_root_status, root_ipc_session_path};
use mcg_storage::{
    STORAGE_SCHEMA_VERSION, SqliteEventStore, inspect_database_before_root, restore_backup_bundle,
    verify_backup_bundle,
};
use rusqlite::Connection;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

fn free_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve loopback port")
        .local_addr()
        .expect("reserved loopback address")
        .port()
}

fn spawn_root(data_directory: &Path, interrupt_after_backup: bool) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mcg-protected-rootd"));
    command
        .env("MCG_DATA_DIR", data_directory)
        .env(
            "MCG_BIND_ADDRESS",
            format!("127.0.0.1:{}", free_loopback_port()),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if interrupt_after_backup {
        command.env("MCG_TEST_INTERRUPT_AFTER_ROOT_BACKUP", "1");
    }
    command.spawn().expect("launch Protected Root process")
}

fn wait_for_exit(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().expect("query Root process") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "Root process did not exit in time"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

async fn wait_for_root(data_directory: &Path) -> mcg_contracts::RootStatus {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if root_ipc_session_path(data_directory).is_file()
            && let Ok(status) = query_root_status(data_directory).await
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "Protected Root IPC did not become ready"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn only_backup(data_directory: &Path) -> PathBuf {
    let backups = fs::read_dir(data_directory.join("backups"))
        .expect("read migration backups")
        .map(|entry| entry.expect("backup entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "mcgb")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        backups.len(),
        1,
        "interrupted migration must create one backup"
    );
    backups.into_iter().next().expect("migration backup")
}

fn stop_process(child: &mut Child) {
    child.kill().expect("stop Protected Root process");
    let _ = wait_for_exit(child);
}

#[tokio::test]
async fn old_profile_survives_interruption_retry_and_verified_backup_restore() {
    let original = tempfile::tempdir().expect("original old profile");
    let database = original.path().join("mcg.sqlite3");
    let conversation = Conversation::new("representative pre-Root conversation", None);
    {
        let mut store = SqliteEventStore::open(&database).expect("seed current profile");
        store
            .create_conversation(&conversation, Some("legacy-thread"))
            .expect("seed existing conversation");
    }
    Connection::open(&database)
        .expect("open fixture for schema downgrade")
        .execute_batch(
            "DROP TABLE workflow_events;
             DROP TABLE workflow_attempts;
             DROP TABLE task_contract_dependencies;
             DROP TABLE task_contract_heads;
             DROP TABLE task_contract_versions;
             DROP TABLE root_ownership;
             DELETE FROM mcg_schema WHERE version IN (51, 52);",
        )
        .expect("create representative schema-50 fixture");
    let before = inspect_database_before_root(&database).expect("inspect schema-50 fixture");
    assert_eq!(before.schema_version, 50);
    assert!(!before.has_root_ownership);

    let mut interrupted = spawn_root(original.path(), true);
    let interrupted_status = wait_for_exit(&mut interrupted);
    assert!(
        !interrupted_status.success(),
        "test interruption must stop startup"
    );
    let backup = only_backup(original.path());
    verify_backup_bundle(&backup).expect("verify interrupted migration backup");
    let after_interruption =
        inspect_database_before_root(&database).expect("inspect source after interruption");
    assert_eq!(after_interruption.schema_version, 50);
    assert!(!after_interruption.has_root_ownership);

    let mut retried = spawn_root(original.path(), false);
    let retry_status = wait_for_root(original.path()).await;
    assert_eq!(retry_status.storage_schema_version, STORAGE_SCHEMA_VERSION);
    assert_eq!(retry_status.migration_receipt.from_storage_schema, 50);
    stop_process(&mut retried);
    let migrated = SqliteEventStore::open(&database).expect("open migrated original profile");
    assert_eq!(
        migrated
            .conversation(conversation.id)
            .expect("read migrated conversation"),
        Some(conversation.clone())
    );
    drop(migrated);

    let restored = tempfile::tempdir().expect("restored profile");
    restore_backup_bundle(&backup, restored.path(), chrono::Utc::now())
        .expect("restore verified pre-migration backup");
    let restored_database = restored.path().join("mcg.sqlite3");
    let restored_before = inspect_database_before_root(&restored_database)
        .expect("inspect restored schema-50 profile");
    assert_eq!(restored_before.schema_version, 50);
    assert!(!restored_before.has_root_ownership);

    let mut restored_root = spawn_root(restored.path(), false);
    let restored_status = wait_for_root(restored.path()).await;
    assert_eq!(
        restored_status.storage_schema_version,
        STORAGE_SCHEMA_VERSION
    );
    assert_eq!(restored_status.migration_receipt.from_storage_schema, 50);
    stop_process(&mut restored_root);
    let restored_store =
        SqliteEventStore::open(&restored_database).expect("open re-migrated restored profile");
    assert_eq!(
        restored_store
            .conversation(conversation.id)
            .expect("read restored conversation"),
        Some(conversation)
    );
}
