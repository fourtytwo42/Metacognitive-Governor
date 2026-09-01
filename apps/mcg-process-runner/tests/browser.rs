use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use mcg_browser_contracts::{
    BROWSER_WORKFLOW_SCHEMA_VERSION, BrowserWorkflowDocument, BrowserWorkflowStep,
};
use mcg_process_runner::{
    BrowserRunnerClient, BrowserRunnerOutcome, GovernedBrowserOperation, ProcessRunnerClientError,
    browser_qualification_request_with_evidence, decode_governed_browser_observation,
    executable_sha256,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
};

async fn browser_fixture() -> (
    String,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind browser fixture");
    let address = listener.local_addr().expect("browser fixture address");
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = requests.clone();
    let posts = Arc::new(AtomicUsize::new(0));
    let observed_posts = posts.clone();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            observed.fetch_add(1, Ordering::SeqCst);
            let observed_posts = observed_posts.clone();
            tokio::spawn(async move {
                let mut request = vec![0_u8; 16 * 1024];
                let count = stream.read(&mut request).await.unwrap_or_default();
                if request[..count].starts_with(b"POST ") {
                    observed_posts.fetch_add(1, Ordering::SeqCst);
                }
                if request[..count].starts_with(b"GET /hang ") {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    return;
                }
                let html = r#"<!doctype html><html><head><title>Governed browser</title></head><body><main id="status">ready</main><label>Name<input id="name"></label><button id="go" onclick="document.querySelector('#status').textContent='done'; history.pushState({}, '', '/done')">Go</button><form method="post" action="/mutate"><button id="submit" type="submit">Mutate</button></form><img alt="blocked" src="http://127.0.0.1:9/cross-origin.png"></body></html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (format!("http://{address}"), requests, posts, task)
}

#[tokio::test]
async fn changed_browser_identity_is_rejected_before_child_launch() {
    let temporary = tempfile::tempdir().expect("browser identity fixture");
    let browser = temporary.path().join(if cfg!(windows) {
        "fixture-browser.exe"
    } else {
        "fixture-browser"
    });
    std::fs::write(&browser, b"original browser identity").expect("write fixture browser");
    let runner = PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client = BrowserRunnerClient::with_executables(&runner, &browser, Duration::from_secs(10))
        .expect("browser runner client");
    let operation = GovernedBrowserOperation {
        browser_executable: browser.clone(),
        browser_sha256: executable_sha256(&browser).expect("measure browser"),
        profile_root: temporary.path().to_path_buf(),
        origin: "https://example.com".into(),
        workflow: BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "identity check".into(),
            start_url: "https://example.com/start".into(),
            steps: vec![BrowserWorkflowStep::AssertElement {
                selector: "main".into(),
            }],
        },
        viewport_width: 1280,
        viewport_height: 720,
        timeout_ms: 5_000,
    };
    let request =
        browser_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("browser qualification request");
    std::fs::write(&browser, b"substituted browser identity").expect("replace fixture browser");
    assert_eq!(
        client.invoke(&request).await.expect_err("identity change"),
        ProcessRunnerClientError::ExecutableChanged
    );
}

#[tokio::test]
#[ignore = "requires MCG_BROWSER_EXECUTABLE and a real local Chromium runtime"]
#[allow(clippy::too_many_lines)]
async fn real_child_executes_exact_same_origin_workflow_with_minimized_evidence() {
    let browser = std::env::var_os("MCG_BROWSER_EXECUTABLE")
        .map(PathBuf::from)
        .expect("MCG_BROWSER_EXECUTABLE");
    let runner = PathBuf::from(env!("CARGO_BIN_EXE_mcg-process-runner"));
    let client = BrowserRunnerClient::with_executables(&runner, &browser, Duration::from_secs(45))
        .expect("browser runner client");
    let (origin, server_requests, server_posts, server_task) = browser_fixture().await;
    let profiles = tempfile::tempdir().expect("browser profile root");
    let workflow = BrowserWorkflowDocument {
        schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
        name: "real governed workflow".into(),
        start_url: format!("{origin}/start?secret=discarded"),
        steps: vec![
            BrowserWorkflowStep::AssertElement {
                selector: "#status".into(),
            },
            BrowserWorkflowStep::Type {
                selector: "#name".into(),
                value: "private typed value".into(),
            },
            BrowserWorkflowStep::Click {
                selector: "#go".into(),
            },
            BrowserWorkflowStep::AssertText {
                selector: "#status".into(),
                contains: "done".into(),
            },
            BrowserWorkflowStep::AssertUrl {
                url: format!("{origin}/done"),
            },
        ],
    };
    let operation = GovernedBrowserOperation {
        browser_executable: browser.clone(),
        browser_sha256: executable_sha256(&browser).expect("measure browser"),
        profile_root: profiles.path().to_path_buf(),
        origin: origin.clone(),
        workflow,
        viewport_width: 1280,
        viewport_height: 720,
        timeout_ms: 30_000,
    };
    let request =
        browser_qualification_request_with_evidence(&operation, client.capability_evidence())
            .expect("browser qualification request");
    let response = client.invoke(&request).await.expect("browser response");
    assert_eq!(
        response.outcome,
        BrowserRunnerOutcome::Completed,
        "browser runner response: {response:?}"
    );
    let observation = decode_governed_browser_observation(
        response.observation.as_ref().expect("browser observation"),
    )
    .expect("decode browser observation");
    assert!(observation.workflow_passed);
    assert_eq!(observation.steps_completed, 5);
    assert_eq!(observation.total_steps, 5);
    assert_eq!(observation.final_url, format!("{origin}/done"));
    assert!(observation.denied_requests >= 1);
    assert!(server_requests.load(Ordering::SeqCst) >= 1);
    let serialized = serde_json::to_string(&response).expect("serialize response");
    assert!(!serialized.contains("private typed value"));
    assert!(!serialized.contains("secret=discarded"));
    assert!(!serialized.contains("<html"));
    assert!(!serialized.contains("screenshot"));

    let denied_form = GovernedBrowserOperation {
        browser_executable: browser.clone(),
        browser_sha256: executable_sha256(&browser).expect("measure browser"),
        profile_root: profiles.path().to_path_buf(),
        origin: origin.clone(),
        workflow: BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "deny mutating form".into(),
            start_url: format!("{origin}/form"),
            steps: vec![BrowserWorkflowStep::Click {
                selector: "#submit".into(),
            }],
        },
        viewport_width: 1280,
        viewport_height: 720,
        timeout_ms: 30_000,
    };
    let denied_request =
        browser_qualification_request_with_evidence(&denied_form, client.capability_evidence())
            .expect("denied-form request");
    let denied_response = client
        .invoke(&denied_request)
        .await
        .expect("denied-form response");
    let denied_observation = decode_governed_browser_observation(
        denied_response
            .observation
            .as_ref()
            .expect("denied-form observation"),
    )
    .expect("decode denied-form observation");
    assert_eq!(denied_response.outcome, BrowserRunnerOutcome::Completed);
    assert!(!denied_observation.workflow_passed);
    assert_eq!(denied_observation.steps_completed, 0);
    assert_eq!(server_posts.load(Ordering::SeqCst), 0);

    let timed_out = GovernedBrowserOperation {
        browser_executable: browser.clone(),
        browser_sha256: executable_sha256(&browser).expect("measure browser"),
        profile_root: profiles.path().to_path_buf(),
        origin: origin.clone(),
        workflow: BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "timeout cleanup".into(),
            start_url: format!("{origin}/hang"),
            steps: vec![BrowserWorkflowStep::AssertElement {
                selector: "main".into(),
            }],
        },
        viewport_width: 1280,
        viewport_height: 720,
        timeout_ms: 1_000,
    };
    let timeout_request =
        browser_qualification_request_with_evidence(&timed_out, client.capability_evidence())
            .expect("timeout request");
    let timeout_response = client
        .invoke(&timeout_request)
        .await
        .expect("timeout response");
    assert_eq!(
        timeout_response.outcome,
        BrowserRunnerOutcome::OutcomeUnknown
    );
    assert!(timeout_response.observation.is_none());
    assert_eq!(
        std::fs::read_dir(profiles.path())
            .expect("inspect profile root")
            .count(),
        0,
        "governed browser left a residual profile"
    );
    server_task.abort();
}
