use std::{fs, path::PathBuf};

use mcg_domain::{SshHostId, SshOperationId, SshOperationKind, TaskId, ToolExecutionId};
use mcg_tools::{SshExecutor, ToolExecutor, ToolOutcomeStatus, ToolRequest, ToolRequestKind};
use sha2::{Digest, Sha256};

#[tokio::test]
#[allow(clippy::too_many_lines)] // Keeps the ordered remote round trip auditable as one fixture.
async fn live_agent_executes_and_hash_verifies_round_trip_transfer() {
    if std::env::var("MCG_LIVE_SSH").as_deref() != Ok("1") {
        eprintln!("skipped: set MCG_LIVE_SSH=1 with documented host metadata and an SSH agent");
        return;
    }
    let hostname = std::env::var("MCG_LIVE_SSH_HOST").expect("live SSH hostname");
    let username = std::env::var("MCG_LIVE_SSH_USER").expect("live SSH username");
    let known_hosts_entry = std::env::var("MCG_LIVE_SSH_KNOWN_HOSTS").expect("pinned host line");
    let host_key_sha256 = std::env::var("MCG_LIVE_SSH_FINGERPRINT").expect("host fingerprint");
    let wrong_known_hosts_entry =
        std::env::var("MCG_LIVE_SSH_WRONG_KNOWN_HOSTS").expect("wrong pinned host fixture");
    let directory = tempfile::tempdir().expect("temporary transfer root");
    let upload = directory.path().join("upload.txt");
    let download = directory.path().join("download.txt");
    let bytes = b"MCG typed SSH transfer evidence\n";
    fs::write(&upload, bytes).expect("write upload fixture");
    let expected_sha256 = format!("{:x}", Sha256::digest(bytes));
    let remote_path = format!("/tmp/mcg-live-{}.txt", SshOperationId::new());
    let executor = SshExecutor::new(
        executable("ssh"),
        executable("scp"),
        directory.path().join("scratch"),
        [directory.path().to_path_buf()],
    )
    .expect("SSH executor");

    let wrong_key = request(
        &hostname,
        &username,
        &wrong_known_hosts_entry,
        &host_key_sha256,
        SshOperationKind::Inspect,
    );
    let rejected = executor
        .execute(ToolExecutionId::new(), &wrong_key)
        .await
        .expect("wrong-key SSH outcome");
    assert_eq!(rejected.status, ToolOutcomeStatus::Failed);
    assert!(rejected.stderr.as_deref().is_some_and(|stderr| {
        stderr.contains("Host key verification failed")
            || stderr.contains("REMOTE HOST IDENTIFICATION")
    }));

    let execute = request(
        &hostname,
        &username,
        &known_hosts_entry,
        &host_key_sha256,
        SshOperationKind::Execute {
            executable: "printf".into(),
            arguments: vec!["%s".into(), "literal;$(no-shell-expansion)".into()],
            working_directory: Some("/tmp".into()),
        },
    );
    let executed = executor
        .execute(ToolExecutionId::new(), &execute)
        .await
        .expect("typed execute");
    assert_eq!(executed.status, ToolOutcomeStatus::Succeeded);
    assert_eq!(
        executed.stdout.as_deref(),
        Some("literal;$(no-shell-expansion)")
    );

    let upload_request = request(
        &hostname,
        &username,
        &known_hosts_entry,
        &host_key_sha256,
        SshOperationKind::Upload {
            local_path: path_text(&upload),
            remote_path: remote_path.clone(),
            overwrite: false,
            expected_sha256: expected_sha256.clone(),
        },
    );
    assert_eq!(
        executor
            .execute(ToolExecutionId::new(), &upload_request)
            .await
            .expect("upload")
            .status,
        ToolOutcomeStatus::Succeeded
    );

    let download_request = request(
        &hostname,
        &username,
        &known_hosts_entry,
        &host_key_sha256,
        SshOperationKind::Download {
            remote_path: remote_path.clone(),
            local_path: path_text(&download),
            overwrite: false,
            expected_sha256: Some(expected_sha256),
        },
    );
    assert_eq!(
        executor
            .execute(ToolExecutionId::new(), &download_request)
            .await
            .expect("download")
            .status,
        ToolOutcomeStatus::Succeeded
    );
    assert_eq!(fs::read(&download).expect("download bytes"), bytes);

    let cleanup = request(
        &hostname,
        &username,
        &known_hosts_entry,
        &host_key_sha256,
        SshOperationKind::Execute {
            executable: "rm".into(),
            arguments: vec!["-f".into(), remote_path],
            working_directory: None,
        },
    );
    assert_eq!(
        executor
            .execute(ToolExecutionId::new(), &cleanup)
            .await
            .expect("cleanup")
            .status,
        ToolOutcomeStatus::Succeeded
    );
}

fn request(
    hostname: &str,
    username: &str,
    known_hosts_entry: &str,
    host_key_sha256: &str,
    kind: SshOperationKind,
) -> ToolRequest {
    let mut request = ToolRequest::new(
        TaskId::new(),
        "live typed SSH qualification",
        ToolRequestKind::SshOperation {
            operation_id: SshOperationId::new(),
            host_id: SshHostId::new(),
            host_revision: 1,
            hostname: hostname.into(),
            port: 22,
            username: username.into(),
            known_hosts_entry: known_hosts_entry.into(),
            host_key_sha256: host_key_sha256.into(),
            kind,
        },
    );
    request.timeout_seconds = 30;
    request
}

fn executable(name: &str) -> PathBuf {
    PathBuf::from(format!("/usr/bin/{name}"))
}

fn path_text(path: &std::path::Path) -> String {
    path.to_str().expect("UTF-8 test path").into()
}
