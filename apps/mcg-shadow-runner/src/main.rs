use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    path::{Component, Path},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use chromiumoxide::{Browser, BrowserConfig, Element, Page};
use futures_util::StreamExt;
use mcg_browser_contracts::{
    ACCESSIBILITY_AUDIT_SCRIPT, AccessibilityAudit, BrowserWorkflowDocument, BrowserWorkflowStep,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

const REQUEST_SCHEMA_VERSION: u16 = 7;
const SETUP_FAILURE_EXIT_CODE: i32 = 125;
const COMMAND_FAILURE_EXIT_CODE: i32 = 1;
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_COMMAND_BYTES: usize = 32 * 1024;
const MAX_ARGUMENTS: usize = 128;
const MAX_FILES: u64 = 50_000;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_PROBE_URL_BYTES: usize = 2 * 1024;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_WORKFLOW_PATH_BYTES: usize = 4 * 1024;
const MAX_WORKFLOW_BYTES: u64 = 1024 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 8 * 1024;
const MAX_RENDERED_DOM_BYTES: usize = 4 * 1024 * 1024;
const SERVICE_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const BROWSER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(20);
const BROWSER_RENDER_TIMEOUT: Duration = Duration::from_secs(20);
const BROWSER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
const SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RunnerRequest {
    schema_version: u16,
    source_root: String,
    workspace_root: String,
    executable: String,
    argv: Vec<String>,
    toolchain_root: Option<String>,
    browser_executable: Option<String>,
    probe: Option<RunnerProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum RunnerProbe {
    HttpStatus {
        url: String,
        expected_status: Option<u16>,
    },
    TcpConnect {
        endpoint: String,
    },
    PageRender {
        url: String,
    },
    ElementExists {
        url: String,
        selector: String,
    },
    Accessibility {
        url: String,
    },
    BrowserWorkflow {
        name: String,
        workflow_path: String,
        expected_sha256: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderState {
    ready_state: String,
    body_present: bool,
    width: u64,
    height: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractionTarget {
    href: Option<String>,
    target: Option<String>,
    download: bool,
    has_form: bool,
    form_action: Option<String>,
    form_target: Option<String>,
    form_method: Option<String>,
}

fn main() {
    let argument = std::env::args_os().nth(1);
    let exit_code = if argument.as_deref() == Some(std::ffi::OsStr::new("--probe")) {
        0
    } else {
        argument
            .ok_or("request path is required")
            .and_then(|path| load_request(Path::new(&path)))
            .and_then(|request| execute_request(&request, true))
            .unwrap_or(SETUP_FAILURE_EXIT_CODE)
    };
    std::process::exit(exit_code);
}

fn load_request(path: &Path) -> Result<RunnerRequest, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "inspect request")?;
    if path_is_link_or_reparse(path, &metadata) || !metadata.is_file() {
        return Err("request must be a regular file");
    }
    if metadata.len() == 0 || metadata.len() > MAX_REQUEST_BYTES {
        return Err("request size is invalid");
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| "size")?);
    fs::File::open(path)
        .map_err(|_| "open request")?
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read request")?;
    serde_json::from_slice(&bytes).map_err(|_| "parse request")
}

fn execute_request(
    request: &RunnerRequest,
    enforce_sandbox_paths: bool,
) -> Result<i32, &'static str> {
    validate_request(request, enforce_sandbox_paths)?;
    let source = Path::new(&request.source_root);
    let workspace = Path::new(&request.workspace_root);
    copy_workspace(source, workspace)?;
    if let Some(probe) = request.probe.as_ref() {
        execute_service_probe(request, workspace, probe)
    } else {
        let mut command = prepared_command(request, workspace)?;
        let status = command.status().map_err(|_| "spawn command")?;
        Ok(if status.success() {
            0
        } else {
            COMMAND_FAILURE_EXIT_CODE
        })
    }
}

fn prepared_command(request: &RunnerRequest, workspace: &Path) -> Result<Command, &'static str> {
    let mut command = Command::new(&request.executable);
    command
        .args(&request.argv)
        .current_dir(workspace)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    preserve_runtime_environment(&mut command, request.toolchain_root.as_deref())?;
    Ok(command)
}

#[allow(clippy::too_many_lines)] // Keep every typed probe adjacent to the one service lifecycle.
fn execute_service_probe(
    request: &RunnerRequest,
    workspace: &Path,
    probe: &RunnerProbe,
) -> Result<i32, &'static str> {
    enum ActiveProbe {
        Http {
            target: LoopbackHttpTarget,
            expected_status: Option<u16>,
        },
        Tcp(LoopbackTcpTarget),
        PageRender {
            target: LoopbackHttpTarget,
            browser: String,
        },
        ElementExists {
            target: LoopbackHttpTarget,
            browser: String,
            selector: String,
        },
        Accessibility {
            target: LoopbackHttpTarget,
            browser: String,
        },
        BrowserWorkflow {
            target: LoopbackHttpTarget,
            browser: String,
            document: BrowserWorkflowDocument,
        },
    }
    let active = match probe {
        RunnerProbe::HttpStatus {
            url,
            expected_status,
        } => ActiveProbe::Http {
            target: LoopbackHttpTarget::parse(url)?,
            expected_status: *expected_status,
        },
        RunnerProbe::TcpConnect { endpoint } => {
            ActiveProbe::Tcp(LoopbackTcpTarget::parse(endpoint)?)
        }
        RunnerProbe::PageRender { url } => ActiveProbe::PageRender {
            target: LoopbackHttpTarget::parse(url)?,
            browser: request
                .browser_executable
                .clone()
                .ok_or("page render probe requires a browser")?,
        },
        RunnerProbe::ElementExists { url, selector } => ActiveProbe::ElementExists {
            target: LoopbackHttpTarget::parse(url)?,
            browser: request
                .browser_executable
                .clone()
                .ok_or("element probe requires a browser")?,
            selector: selector.clone(),
        },
        RunnerProbe::Accessibility { url } => ActiveProbe::Accessibility {
            target: LoopbackHttpTarget::parse(url)?,
            browser: request
                .browser_executable
                .clone()
                .ok_or("accessibility probe requires a browser")?,
        },
        RunnerProbe::BrowserWorkflow {
            name,
            workflow_path,
            expected_sha256,
        } => {
            let document = load_bound_workflow(workspace, name, workflow_path, expected_sha256)?;
            ActiveProbe::BrowserWorkflow {
                target: LoopbackHttpTarget::parse(&document.start_url)?,
                browser: request
                    .browser_executable
                    .clone()
                    .ok_or("browser workflow probe requires a browser")?,
                document,
            }
        }
    };
    let (service_host, port) = match &active {
        ActiveProbe::Tcp(target) => (target.service_host(), target.port()),
        ActiveProbe::Http { target, .. }
        | ActiveProbe::PageRender { target, .. }
        | ActiveProbe::ElementExists { target, .. }
        | ActiveProbe::Accessibility { target, .. }
        | ActiveProbe::BrowserWorkflow { target, .. } => (target.service_host(), target.port()),
    };
    let mut command = prepared_command(request, workspace)?;
    command
        .env("HOST", service_host)
        .env("PORT", port.to_string());
    let mut child = command.spawn().map_err(|_| "spawn service command")?;
    let deadline = Instant::now() + SERVICE_STARTUP_TIMEOUT;
    let disposition = loop {
        let passed = match &active {
            ActiveProbe::Http {
                target,
                expected_status,
            } => target.request_status().map(|status| {
                expected_status.map_or_else(
                    || (200..300).contains(&status),
                    |expected| status == expected,
                )
            }),
            ActiveProbe::Tcp(target) => target.is_listening().then_some(true),
            ActiveProbe::PageRender { target, browser } => target
                .request_status()
                .filter(|status| (200..300).contains(status))
                .map(|_| render_page(browser, workspace, target, None, false, None)),
            ActiveProbe::ElementExists {
                target,
                browser,
                selector,
            } => target
                .request_status()
                .filter(|status| (200..300).contains(status))
                .map(|_| render_page(browser, workspace, target, Some(selector), false, None)),
            ActiveProbe::Accessibility { target, browser } => target
                .request_status()
                .filter(|status| (200..300).contains(status))
                .map(|_| render_page(browser, workspace, target, None, true, None)),
            ActiveProbe::BrowserWorkflow {
                target,
                browser,
                document,
            } => target
                .request_status()
                .filter(|status| (200..300).contains(status))
                .map(|_| render_page(browser, workspace, target, None, false, Some(document))),
        };
        if let Some(passed) = passed {
            break if passed { 0 } else { COMMAND_FAILURE_EXIT_CODE };
        }
        if child
            .try_wait()
            .map_err(|_| "inspect service command")?
            .is_some()
            || Instant::now() >= deadline
        {
            break COMMAND_FAILURE_EXIT_CODE;
        }
        thread::sleep(SERVICE_POLL_INTERVAL);
    };
    let _ = child.kill();
    let _ = child.wait();
    Ok(disposition)
}

fn validate_request(
    request: &RunnerRequest,
    enforce_sandbox_paths: bool,
) -> Result<(), &'static str> {
    if request.schema_version != REQUEST_SCHEMA_VERSION
        || request.executable.trim().is_empty()
        || request.executable.contains('\0')
        || request.argv.len() > MAX_ARGUMENTS
        || request.argv.iter().any(|argument| argument.contains('\0'))
    {
        return Err("request contract is invalid");
    }
    if let Some(toolchain_root) = &request.toolchain_root {
        let path = Path::new(toolchain_root);
        if toolchain_root.contains('\0')
            || !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || (enforce_sandbox_paths && !toolchain_root_is_exact(toolchain_root))
        {
            return Err("toolchain root escaped the sandbox contract");
        }
    }
    match request.browser_executable.as_deref() {
        Some(browser) if browser_is_exact(browser, enforce_sandbox_paths) => {}
        Some(_) => return Err("browser executable escaped the sandbox contract"),
        None => {}
    }
    if let Some(probe) = &request.probe {
        match probe {
            RunnerProbe::HttpStatus {
                url,
                expected_status,
            } => {
                LoopbackHttpTarget::parse(url)?;
                if expected_status.is_some_and(|status| !(100..=599).contains(&status)) {
                    return Err("expected HTTP status is invalid");
                }
            }
            RunnerProbe::TcpConnect { endpoint } => {
                LoopbackTcpTarget::parse(endpoint)?;
            }
            RunnerProbe::PageRender { url } => {
                LoopbackHttpTarget::parse(url)?;
                if request.browser_executable.is_none() {
                    return Err("page render probe requires a browser");
                }
            }
            RunnerProbe::ElementExists { url, selector } => {
                LoopbackHttpTarget::parse(url)?;
                validate_selector(selector)?;
                if request.browser_executable.is_none() {
                    return Err("element probe requires a browser");
                }
            }
            RunnerProbe::Accessibility { url } => {
                LoopbackHttpTarget::parse(url)?;
                if request.browser_executable.is_none() {
                    return Err("accessibility probe requires a browser");
                }
            }
            RunnerProbe::BrowserWorkflow {
                name,
                workflow_path,
                expected_sha256,
            } => {
                validate_workflow_binding(name, workflow_path, expected_sha256)?;
                if request.browser_executable.is_none() {
                    return Err("browser workflow probe requires a browser");
                }
            }
        }
    } else if request.browser_executable.is_some() {
        return Err("browser executable requires a page render probe");
    }
    let command_bytes = request
        .argv
        .iter()
        .try_fold(request.executable.len(), |total, argument| {
            total
                .checked_add(argument.len())
                .and_then(|value| value.checked_add(1))
        })
        .ok_or("command size overflow")?;
    if command_bytes > MAX_COMMAND_BYTES {
        return Err("command is too large");
    }
    if enforce_sandbox_paths && !sandbox_paths_are_exact(request) {
        return Err("request paths escaped the sandbox contract");
    }
    for value in [&request.source_root, &request.workspace_root] {
        let path = Path::new(value);
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err("request paths must be absolute and normalized");
        }
    }
    Ok(())
}

fn sandbox_paths_are_exact(request: &RunnerRequest) -> bool {
    #[cfg(windows)]
    {
        request.source_root.eq_ignore_ascii_case(r"C:\mcg-source")
            && request
                .workspace_root
                .eq_ignore_ascii_case(r"C:\mcg-workspace")
    }
    #[cfg(not(windows))]
    {
        request.source_root == "/mcg-source" && request.workspace_root == "/tmp/mcg-workspace"
    }
}

fn toolchain_root_is_exact(toolchain_root: &str) -> bool {
    #[cfg(windows)]
    {
        toolchain_root.eq_ignore_ascii_case(r"C:\mcg-toolchain")
    }
    #[cfg(not(windows))]
    {
        toolchain_root == "/mcg-toolchain"
    }
}

fn browser_is_exact(browser: &str, enforce_sandbox_paths: bool) -> bool {
    let path = Path::new(browser);
    if browser.contains('\0')
        || !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return false;
    }
    if !enforce_sandbox_paths {
        return true;
    }
    #[cfg(windows)]
    {
        path.parent().is_some_and(|parent| {
            parent
                .to_string_lossy()
                .eq_ignore_ascii_case(r"C:\mcg-browser")
        })
    }
    #[cfg(not(windows))]
    {
        path.parent() == Some(Path::new("/mcg-browser"))
    }
}

fn validate_selector(selector: &str) -> Result<(), &'static str> {
    if selector.trim().is_empty()
        || selector.len() > MAX_SELECTOR_BYTES
        || selector.contains('\0')
        || selector.chars().any(char::is_control)
    {
        return Err("element selector is invalid");
    }
    Ok(())
}

fn validate_workflow_binding(
    name: &str,
    workflow_path: &str,
    expected_sha256: &str,
) -> Result<(), &'static str> {
    let path = Path::new(workflow_path);
    if name.trim().is_empty()
        || name.chars().count() > 128
        || workflow_path.is_empty()
        || workflow_path.len() > MAX_WORKFLOW_PATH_BYTES
        || workflow_path.contains(['\\', ':', '\0'])
        || workflow_path.chars().any(char::is_control)
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::CurDir
                    | Component::ParentDir
            )
        })
        || expected_sha256.len() != 64
        || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("browser workflow binding is invalid");
    }
    Ok(())
}

fn load_bound_workflow(
    workspace: &Path,
    name: &str,
    workflow_path: &str,
    expected_sha256: &str,
) -> Result<BrowserWorkflowDocument, &'static str> {
    validate_workflow_binding(name, workflow_path, expected_sha256)?;
    let path = workspace.join(workflow_path);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "inspect browser workflow")?;
    if path_is_link_or_reparse(&path, &metadata)
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_WORKFLOW_BYTES
    {
        return Err("browser workflow must be one bounded regular file");
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| "size")?);
    fs::File::open(&path)
        .map_err(|_| "open browser workflow")?
        .take(MAX_WORKFLOW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read browser workflow")?;
    if bytes.len() as u64 != metadata.len()
        || !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(expected_sha256)
    {
        return Err("browser workflow changed after binding");
    }
    let document: BrowserWorkflowDocument =
        serde_json::from_slice(&bytes).map_err(|_| "parse browser workflow")?;
    document
        .validate()
        .map_err(|_| "validate browser workflow")?;
    if document.name != name {
        return Err("browser workflow name changed after binding");
    }
    LoopbackHttpTarget::parse(&document.start_url)?;
    Ok(document)
}

fn render_page(
    browser: &str,
    workspace: &Path,
    target: &LoopbackHttpTarget,
    selector: Option<&str>,
    audit_accessibility: bool,
    workflow: Option<&BrowserWorkflowDocument>,
) -> bool {
    let profile = workspace.join(".mcg-browser-profile");
    if fs::create_dir(&profile).is_err() {
        return false;
    }
    let rendered = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .is_ok_and(|runtime| {
            runtime.block_on(render_page_with_browser(
                browser,
                &profile,
                target,
                selector,
                audit_accessibility,
                workflow,
            ))
        });
    let _ = fs::remove_dir_all(&profile);
    rendered
}

#[allow(clippy::too_many_lines)] // Keep navigation, typed interaction, and final render checks in one browser lifecycle.
async fn render_page_with_browser(
    executable: &str,
    profile: &Path,
    target: &LoopbackHttpTarget,
    selector: Option<&str>,
    audit_accessibility: bool,
    workflow: Option<&BrowserWorkflowDocument>,
) -> bool {
    let config = BrowserConfig::builder()
        .chrome_executable(executable)
        .user_data_dir(profile)
        .window_size(1_280, 800)
        .new_headless_mode()
        .no_sandbox()
        // Chrome launched as the Windows Sandbox System identity does not emit
        // its DevTools endpoint on the inherited stderr pipe unless logging is
        // explicitly directed there. Chromiumoxide discovers the random,
        // loopback-only endpoint from that line; keeping port 0 also prevents
        // an untrusted sibling service from pre-binding a known debug port.
        .arg(("enable-logging", "stderr"))
        .launch_timeout(BROWSER_LAUNCH_TIMEOUT)
        .request_timeout(BROWSER_RENDER_TIMEOUT)
        .build();
    let Ok(config) = config else {
        return false;
    };
    let Ok((mut browser, mut handler)) = Browser::launch(config).await else {
        return false;
    };
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });
    let url = format!("http://{}{}", target.authority, target.request_target);
    let rendered = tokio::time::timeout(BROWSER_RENDER_TIMEOUT, async {
        let page = browser.new_page("about:blank").await.ok()?;
        page.goto(&url).await.ok()?;
        if workflow.is_some() {
            page.evaluate(
                "(() => { const deny = () => null; try { Object.defineProperty(window, 'open', { value: deny, configurable: false, writable: false }); } catch {} try { Object.defineProperty(window, 'showOpenFilePicker', { value: deny, configurable: false, writable: false }); } catch {} try { Object.defineProperty(window, 'showSaveFilePicker', { value: deny, configurable: false, writable: false }); } catch {} })()",
            )
            .await
            .ok()?;
        }
        let final_url = page.url().await.ok().flatten()?;
        let final_target = LoopbackHttpTarget::parse(&final_url).ok()?;
        if final_target.authority != target.authority {
            return None;
        }
        if let Some(selector) = selector {
            let found = page.find_elements(selector).await.ok()?;
            if found.is_empty() {
                return None;
            }
        }
        if audit_accessibility {
            let audit: AccessibilityAudit = page
                .evaluate(ACCESSIBILITY_AUDIT_SCRIPT)
                .await
                .ok()?
                .into_value()
                .ok()?;
            if !audit.passed() {
                return None;
            }
        }
        if let Some(workflow) = workflow {
            let origin = Url::parse(&workflow.start_url)
                .ok()?
                .origin()
                .ascii_serialization();
            for step in &workflow.steps {
                execute_workflow_step(&page, step, &origin).await.ok()?;
                if matches!(
                    step,
                    BrowserWorkflowStep::Click { .. }
                        | BrowserWorkflowStep::Type { .. }
                        | BrowserWorkflowStep::Press { .. }
                ) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                let current = current_browser_url(&page).await.ok()?;
                if current.origin().ascii_serialization() != origin {
                    return None;
                }
            }
        }
        let state: RenderState = page
            .evaluate(
                "({readyState: document.readyState, bodyPresent: !!document.body, width: Math.max(document.documentElement?.scrollWidth || 0, document.body?.scrollWidth || 0), height: Math.max(document.documentElement?.scrollHeight || 0, document.body?.scrollHeight || 0)})",
            )
            .await
            .ok()?
            .into_value()
            .ok()?;
        let bytes = page.content_bytes().await.ok()?;
        (matches!(state.ready_state.as_str(), "interactive" | "complete")
            && state.body_present
            && state.width > 0
            && state.height > 0
            && bytes.len() <= MAX_RENDERED_DOM_BYTES
            && rendered_dom_is_valid(&bytes))
        .then_some(())
    })
    .await
    .is_ok_and(|result| result.is_some());
    let _ = tokio::time::timeout(BROWSER_SHUTDOWN_TIMEOUT, browser.close()).await;
    let _ = tokio::time::timeout(BROWSER_SHUTDOWN_TIMEOUT, browser.wait()).await;
    handler_task.abort();
    rendered
}

async fn current_browser_url(page: &Page) -> Result<Url, ()> {
    page.url()
        .await
        .ok()
        .flatten()
        .and_then(|value| Url::parse(&value).ok())
        .ok_or(())
}

async fn execute_workflow_step(
    page: &Page,
    step: &BrowserWorkflowStep,
    origin: &str,
) -> Result<(), ()> {
    match step {
        BrowserWorkflowStep::AssertElement { selector } => page
            .find_elements(selector)
            .await
            .ok()
            .filter(|elements| !elements.is_empty())
            .map(|_| ())
            .ok_or(()),
        BrowserWorkflowStep::Click { selector } => {
            let element = page.find_element(selector).await.map_err(|_| ())?;
            validate_interaction_target(&element, origin).await?;
            element.click().await.map(|_| ()).map_err(|_| ())
        }
        BrowserWorkflowStep::Type { selector, value } => {
            let element = page.find_element(selector).await.map_err(|_| ())?;
            element.click().await.map_err(|_| ())?;
            element.type_str(value).await.map(|_| ()).map_err(|_| ())
        }
        BrowserWorkflowStep::Press { selector, key } => {
            let element = page.find_element(selector).await.map_err(|_| ())?;
            validate_interaction_target(&element, origin).await?;
            element.focus().await.map_err(|_| ())?;
            element.press_key(key).await.map(|_| ()).map_err(|_| ())
        }
        BrowserWorkflowStep::AssertText { selector, contains } => {
            let text = page
                .find_element(selector)
                .await
                .map_err(|_| ())?
                .inner_text()
                .await
                .map_err(|_| ())?
                .unwrap_or_default();
            text.contains(contains).then_some(()).ok_or(())
        }
        BrowserWorkflowStep::AssertUrl { url } => (current_browser_url(page).await?.as_str()
            == url)
            .then_some(())
            .ok_or(()),
    }
}

async fn validate_interaction_target(element: &Element, origin: &str) -> Result<(), ()> {
    let response = element
        .call_js_fn(
            "function() { const form = this.form || this.closest?.('form') || null; return JSON.stringify({ href: this.href || null, target: this.target || null, download: this.hasAttribute?.('download') || false, hasForm: !!form, formAction: form ? (this.formAction || form.action || null) : null, formTarget: form ? (this.formTarget || form.target || null) : null, formMethod: form ? (this.formMethod || form.method || 'get') : null }); }",
            false,
        )
        .await
        .map_err(|_| ())?;
    let target_json = response
        .result
        .value
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(())?;
    let target: InteractionTarget = serde_json::from_str(&target_json).map_err(|_| ())?;
    if target.download
        || target
            .target
            .as_deref()
            .is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("_self"))
        || target
            .form_target
            .as_deref()
            .is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("_self"))
    {
        return Err(());
    }
    for destination in [target.href.as_deref(), target.form_action.as_deref()]
        .into_iter()
        .flatten()
    {
        let url = Url::parse(destination).map_err(|_| ())?;
        if !matches!(url.scheme(), "http" | "https") || url.origin().ascii_serialization() != origin
        {
            return Err(());
        }
    }
    if target.has_form
        && !target
            .form_method
            .as_deref()
            .unwrap_or("get")
            .eq_ignore_ascii_case("get")
    {
        return Err(());
    }
    Ok(())
}

fn rendered_dom_is_valid(bytes: &[u8]) -> bool {
    bytes
        .windows(5)
        .any(|window| window.eq_ignore_ascii_case(b"<body"))
        && bytes
            .windows(6)
            .any(|window| window.eq_ignore_ascii_case(b"</body"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopbackHttpTarget {
    addresses: Vec<SocketAddr>,
    authority: String,
    request_target: String,
    service_host: &'static str,
    port: u16,
}

impl LoopbackHttpTarget {
    fn parse(url: &str) -> Result<Self, &'static str> {
        if url.is_empty()
            || url.len() > MAX_PROBE_URL_BYTES
            || url
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ')
            || url.contains('#')
        {
            return Err("HTTP probe URL is invalid");
        }
        let remainder = url
            .strip_prefix("http://")
            .ok_or("HTTP probe must use loopback plaintext HTTP")?;
        let (authority, request_target) = remainder
            .split_once('/')
            .map_or((remainder, "/".to_owned()), |(authority, path)| {
                (authority, format!("/{path}"))
            });
        if authority.is_empty() || authority.contains('@') {
            return Err("HTTP probe authority is invalid");
        }
        let (host, port) = if let Some(port) = authority.strip_prefix("[::1]:") {
            ("::1", parse_port(port)?)
        } else {
            let (host, port) = authority
                .split_once(':')
                .ok_or("HTTP probe requires an explicit port")?;
            if port.contains(':') {
                return Err("HTTP probe authority is ambiguous");
            }
            (host, parse_port(port)?)
        };
        let (addresses, service_host) = match host.to_ascii_lowercase().as_str() {
            "127.0.0.1" => (
                vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)],
                "127.0.0.1",
            ),
            "::1" => (
                vec![SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)],
                "::1",
            ),
            "localhost" => (
                vec![
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
                ],
                "127.0.0.1",
            ),
            _ => return Err("HTTP probe must target loopback"),
        };
        Ok(Self {
            addresses,
            authority: authority.to_owned(),
            request_target,
            service_host,
            port,
        })
    }

    fn service_host(&self) -> &'static str {
        self.service_host
    }

    const fn port(&self) -> u16 {
        self.port
    }

    fn request_status(&self) -> Option<u16> {
        self.addresses
            .iter()
            .find_map(|address| self.request_status_from(*address))
    }

    fn request_status_from(&self, address: SocketAddr) -> Option<u16> {
        let mut stream = TcpStream::connect_timeout(&address, HTTP_ATTEMPT_TIMEOUT).ok()?;
        stream.set_read_timeout(Some(HTTP_ATTEMPT_TIMEOUT)).ok()?;
        stream.set_write_timeout(Some(HTTP_ATTEMPT_TIMEOUT)).ok()?;
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            self.request_target, self.authority
        )
        .ok()?;
        stream.flush().ok()?;
        let mut response = Vec::with_capacity(512);
        let mut buffer = [0_u8; 512];
        while response.len() < MAX_HTTP_HEADER_BYTES {
            let read = stream.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..read]);
            if response.windows(2).any(|window| window == b"\r\n") {
                break;
            }
        }
        let first_line_end = response.windows(2).position(|window| window == b"\r\n")?;
        let first_line = std::str::from_utf8(&response[..first_line_end]).ok()?;
        let mut fields = first_line.split_ascii_whitespace();
        let version = fields.next()?;
        let status = fields.next()?.parse::<u16>().ok()?;
        if !version.starts_with("HTTP/1.") || !(100..=599).contains(&status) {
            return None;
        }
        Some(status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopbackTcpTarget {
    addresses: Vec<SocketAddr>,
    service_host: &'static str,
    port: u16,
}

impl LoopbackTcpTarget {
    fn parse(endpoint: &str) -> Result<Self, &'static str> {
        if endpoint.is_empty()
            || endpoint.len() > MAX_PROBE_URL_BYTES
            || endpoint
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ')
        {
            return Err("TCP probe endpoint is invalid");
        }
        let authority = endpoint
            .strip_prefix("tcp://")
            .ok_or("TCP probe must use the tcp scheme")?;
        if authority.is_empty() || authority.contains(['@', '/', '?', '#']) {
            return Err("TCP probe authority is invalid");
        }
        let (host, port) = if let Some(port) = authority.strip_prefix("[::1]:") {
            ("::1", parse_port(port)?)
        } else {
            let (host, port) = authority
                .split_once(':')
                .ok_or("TCP probe requires an explicit port")?;
            if port.contains(':') {
                return Err("TCP probe authority is ambiguous");
            }
            (host, parse_port(port)?)
        };
        let (addresses, service_host) = match host.to_ascii_lowercase().as_str() {
            "127.0.0.1" => (
                vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)],
                "127.0.0.1",
            ),
            "::1" => (
                vec![SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port)],
                "::1",
            ),
            "localhost" => (
                vec![
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
                ],
                "127.0.0.1",
            ),
            _ => return Err("TCP probe must target loopback"),
        };
        Ok(Self {
            addresses,
            service_host,
            port,
        })
    }

    const fn service_host(&self) -> &'static str {
        self.service_host
    }

    const fn port(&self) -> u16 {
        self.port
    }

    fn is_listening(&self) -> bool {
        self.addresses
            .iter()
            .any(|address| TcpStream::connect_timeout(address, HTTP_ATTEMPT_TIMEOUT).is_ok())
    }
}

fn parse_port(value: &str) -> Result<u16, &'static str> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or("HTTP probe port is invalid")
}

fn copy_workspace(source: &Path, workspace: &Path) -> Result<(), &'static str> {
    let source = source.canonicalize().map_err(|_| "resolve source")?;
    let metadata = fs::symlink_metadata(&source).map_err(|_| "inspect source")?;
    if path_is_link_or_reparse(&source, &metadata) || !metadata.is_dir() {
        return Err("source must be a real directory");
    }
    if workspace.exists() {
        return Err("workspace destination already exists");
    }
    fs::create_dir(workspace).map_err(|_| "create workspace")?;
    let mut pending = vec![(source.clone(), workspace.to_path_buf(), 0_usize)];
    let mut files = 0_u64;
    let mut total_bytes = 0_u64;
    while let Some((from, to, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err("source depth limit exceeded");
        }
        for entry in fs::read_dir(&from).map_err(|_| "read source")? {
            let entry = entry.map_err(|_| "read source entry")?;
            let name = entry.file_name();
            let source_path = entry.path();
            let destination_path = to.join(&name);
            let metadata =
                fs::symlink_metadata(&source_path).map_err(|_| "inspect source entry")?;
            if path_is_link_or_reparse(&source_path, &metadata) {
                return Err("source links and reparse points are forbidden");
            }
            if metadata.is_dir() {
                fs::create_dir(&destination_path).map_err(|_| "create destination directory")?;
                pending.push((source_path, destination_path, depth.saturating_add(1)));
            } else if metadata.is_file() {
                files = files.checked_add(1).ok_or("file count overflow")?;
                total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or("source size overflow")?;
                if files > MAX_FILES
                    || metadata.len() > MAX_FILE_BYTES
                    || total_bytes > MAX_TOTAL_BYTES
                {
                    return Err("source file or byte limit exceeded");
                }
                copy_file(&source_path, &destination_path, metadata.len())?;
            } else {
                return Err("source contains a non-regular entry");
            }
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path, expected_bytes: u64) -> Result<(), &'static str> {
    let input = fs::File::open(source).map_err(|_| "open source file")?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| "create destination file")?;
    let copied = std::io::copy(&mut input.take(MAX_FILE_BYTES + 1), &mut output)
        .map_err(|_| "copy source file")?;
    if copied != expected_bytes || copied > MAX_FILE_BYTES {
        return Err("source changed while copying");
    }
    output.flush().map_err(|_| "flush destination file")?;
    Ok(())
}

fn path_is_link_or_reparse(_path: &Path, metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn preserve_runtime_environment(
    command: &mut Command,
    toolchain_root: Option<&str>,
) -> Result<(), &'static str> {
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(root) = toolchain_root {
        let root = Path::new(root);
        if !root.is_absolute()
            || root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err("toolchain root must be absolute and normalized");
        }
        let bin = if cfg!(windows) {
            root.to_path_buf()
        } else {
            root.join("bin")
        };
        let mut values = vec![bin];
        values.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(values).map_err(|_| "compose toolchain PATH")?;
        command.env("PATH", path);
    } else {
        command.env("PATH", inherited_path);
    }
    for name in ["PATHEXT", "SYSTEMROOT", "WINDIR", "COMSPEC", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_http_fixture(
        request_line: &'static [u8],
        response: &'static [u8],
    ) -> (u16, thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            loop {
                let (mut stream, _) = listener.accept().expect("accept");
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .expect("read timeout");
                let mut request = [0_u8; 1024];
                let mut read = 0;
                while read < request.len()
                    && !request[..read]
                        .windows(4)
                        .any(|window| window == b"\r\n\r\n")
                {
                    match stream.read(&mut request[read..]) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => read += count,
                    }
                }
                stream.write_all(response).expect("response");
                stream.flush().expect("flush response");
                if request[..read].starts_with(request_line) {
                    break;
                }
            }
        });
        (port, server)
    }

    fn command(success: bool) -> (String, Vec<String>) {
        #[cfg(windows)]
        {
            (
                "cmd.exe".into(),
                vec![
                    "/D".into(),
                    "/S".into(),
                    "/C".into(),
                    if success { "exit 0" } else { "exit 9" }.into(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "/bin/sh".into(),
                vec![
                    "-c".into(),
                    if success { "exit 0" } else { "exit 9" }.into(),
                ],
            )
        }
    }

    #[test]
    fn copies_regular_workspace_and_normalizes_command_failure() {
        let source = tempfile::tempdir().expect("source");
        let destination_parent = tempfile::tempdir().expect("destination parent");
        fs::create_dir(source.path().join("nested")).expect("nested");
        fs::write(source.path().join("nested/file.txt"), b"fixture").expect("fixture");
        let destination = destination_parent.path().join("workspace");
        let (executable, argv) = command(false);
        let request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: source.path().to_string_lossy().into_owned(),
            workspace_root: destination.to_string_lossy().into_owned(),
            executable,
            argv,
            toolchain_root: None,
            browser_executable: None,
            probe: None,
        };
        assert_eq!(
            execute_request(&request, false),
            Ok(COMMAND_FAILURE_EXIT_CODE)
        );
        assert_eq!(
            fs::read(destination.join("nested/file.txt")).expect("copied file"),
            b"fixture"
        );
    }

    #[test]
    fn rejects_replay_oversize_and_links_before_process_execution() {
        let source = tempfile::tempdir().expect("source");
        let destination_parent = tempfile::tempdir().expect("destination parent");
        let destination = destination_parent.path().join("workspace");
        let (executable, argv) = command(true);
        let request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: source.path().to_string_lossy().into_owned(),
            workspace_root: destination.to_string_lossy().into_owned(),
            executable,
            argv,
            toolchain_root: None,
            browser_executable: None,
            probe: None,
        };
        assert_eq!(execute_request(&request, false), Ok(0));
        assert!(execute_request(&request, false).is_err());

        let oversized_source = tempfile::tempdir().expect("oversized source");
        let oversized =
            fs::File::create(oversized_source.path().join("oversized")).expect("oversized file");
        oversized
            .set_len(MAX_TOTAL_BYTES + 1)
            .expect("sparse length");
        let oversized_destination = destination_parent.path().join("oversized-workspace");
        assert!(copy_workspace(oversized_source.path(), &oversized_destination).is_err());

        let link_source = tempfile::tempdir().expect("link source");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), b"secret").expect("secret");
        let link = link_source.path().join("escape");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(outside.path().join("secret"), &link).is_ok();
        #[cfg(windows)]
        let linked =
            std::os::windows::fs::symlink_file(outside.path().join("secret"), &link).is_ok();
        if linked {
            assert!(
                copy_workspace(
                    link_source.path(),
                    &destination_parent.path().join("link-workspace")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_untrusted_schema_paths_and_oversized_argument_vectors() {
        let mut request = RunnerRequest {
            schema_version: 0,
            source_root: r"C:\mcg-source".into(),
            workspace_root: r"C:\mcg-workspace".into(),
            executable: "tool.exe".into(),
            argv: Vec::new(),
            toolchain_root: None,
            browser_executable: None,
            probe: None,
        };
        assert!(validate_request(&request, true).is_err());
        request.schema_version = REQUEST_SCHEMA_VERSION;
        request.argv = vec!["argument".into(); MAX_ARGUMENTS + 1];
        assert!(validate_request(&request, false).is_err());
        request.argv.clear();
        request.executable = "x".repeat(MAX_COMMAND_BYTES + 1);
        assert!(validate_request(&request, false).is_err());
        request.executable = "node.exe".into();
        request.toolchain_root = Some(r"C:\different-toolchain".into());
        assert!(validate_request(&request, true).is_err());
        request.toolchain_root = Some(if cfg!(windows) {
            r"C:\mcg-toolchain".into()
        } else {
            "/mcg-toolchain".into()
        });
        if !cfg!(windows) {
            request.source_root = "/mcg-source".into();
            request.workspace_root = "/tmp/mcg-workspace".into();
        }
        assert!(validate_request(&request, false).is_ok());
        #[cfg(windows)]
        assert!(validate_request(&request, true).is_ok());
        request.toolchain_root = Some(r"C:\mcg-toolchain\..\escape".into());
        assert!(validate_request(&request, false).is_err());
    }

    #[test]
    fn loopback_tcp_probe_accepts_only_exact_listeners() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let endpoint = format!("tcp://{}", listener.local_addr().expect("address"));
        let target = LoopbackTcpTarget::parse(&endpoint).expect("target");
        assert!(target.is_listening());
        assert_eq!(target.service_host(), "127.0.0.1");
        for rejected in [
            "http://127.0.0.1:80",
            "tcp://127.0.0.1",
            "tcp://127.0.0.2:80",
            "tcp://user@127.0.0.1:80",
            "tcp://127.0.0.1:80/path",
            "tcp://127.0.0.1:80?query=1",
        ] {
            assert!(
                LoopbackTcpTarget::parse(rejected).is_err(),
                "accepted unsafe endpoint {rejected}"
            );
        }
    }

    #[test]
    fn loopback_http_probe_rejects_authority_escape_and_observes_status() {
        for rejected in [
            "https://127.0.0.1:8443/",
            "http://192.168.1.10:8080/",
            "http://user@127.0.0.1:8080/",
            "http://127.0.0.1/",
            "http://127.0.0.1:8080/#fragment",
            "http://127.0.0.1:8080/\r\nInjected: true",
        ] {
            assert!(LoopbackHttpTarget::parse(rejected).is_err(), "{rejected}");
        }

        let (port, server) = spawn_http_fixture(
            b"GET /health HTTP/1.1\r\n",
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
        );
        let target =
            LoopbackHttpTarget::parse(&format!("http://127.0.0.1:{port}/health")).expect("target");
        let observed = (0..10).find_map(|_| {
            let status = target.request_status();
            if status.is_none() {
                thread::sleep(Duration::from_millis(10));
            }
            status
        });
        assert_eq!(observed, Some(204));
        server.join().expect("server");
    }

    #[test]
    fn service_probe_runs_beside_the_service_and_normalizes_status_failure() {
        fn exercise(expected_status: u16) -> i32 {
            let (port, server) = spawn_http_fixture(
                b"GET /ready HTTP/1.1\r\n",
                b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n",
            );
            let source = tempfile::tempdir().expect("source");
            let destination_parent = tempfile::tempdir().expect("destination parent");
            let destination = destination_parent.path().join("workspace");
            #[cfg(windows)]
            let (executable, argv) = (
                "powershell.exe".into(),
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    "Start-Sleep -Seconds 10".into(),
                ],
            );
            #[cfg(not(windows))]
            let (executable, argv) = ("/bin/sh".into(), vec!["-c".into(), "sleep 10".into()]);
            let request = RunnerRequest {
                schema_version: REQUEST_SCHEMA_VERSION,
                source_root: source.path().to_string_lossy().into_owned(),
                workspace_root: destination.to_string_lossy().into_owned(),
                executable,
                argv,
                toolchain_root: None,
                browser_executable: None,
                probe: Some(RunnerProbe::HttpStatus {
                    url: format!("http://127.0.0.1:{port}/ready"),
                    expected_status: Some(expected_status),
                }),
            };
            let result = execute_request(&request, false).expect("service probe");
            server.join().expect("server");
            result
        }

        assert_eq!(exercise(201), 0);
        assert_eq!(exercise(204), COMMAND_FAILURE_EXIT_CODE);
    }

    #[test]
    fn browser_contract_requires_exact_runtime_hydrated_body_and_bounded_selector() {
        assert!(rendered_dom_is_valid(
            b"<!doctype html><html><body><main>ready</main></body></html>"
        ));
        assert!(!rendered_dom_is_valid(b"<html><head></head></html>"));

        let mut request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: if cfg!(windows) {
                r"C:\mcg-source".into()
            } else {
                "/mcg-source".into()
            },
            workspace_root: if cfg!(windows) {
                r"C:\mcg-workspace".into()
            } else {
                "/tmp/mcg-workspace".into()
            },
            executable: "service".into(),
            argv: Vec::new(),
            toolchain_root: None,
            browser_executable: None,
            probe: Some(RunnerProbe::PageRender {
                url: "http://127.0.0.1:8080/".into(),
            }),
        };
        assert!(validate_request(&request, true).is_err());
        request.browser_executable = Some(if cfg!(windows) {
            r"C:\mcg-browser\chrome.exe".into()
        } else {
            "/mcg-browser/chrome".into()
        });
        assert!(validate_request(&request, true).is_ok());
        request.browser_executable = Some(if cfg!(windows) {
            r"C:\outside\chrome.exe".into()
        } else {
            "/outside/chrome".into()
        });
        assert!(validate_request(&request, true).is_err());
        request.browser_executable = Some(if cfg!(windows) {
            r"C:\mcg-browser\chrome.exe".into()
        } else {
            "/mcg-browser/chrome".into()
        });
        request.probe = Some(RunnerProbe::ElementExists {
            url: "http://127.0.0.1:8080/".into(),
            selector: "main#app[data-state='ready']".into(),
        });
        assert!(validate_request(&request, true).is_ok());
        request.probe = Some(RunnerProbe::ElementExists {
            url: "http://127.0.0.1:8080/".into(),
            selector: " ".into(),
        });
        assert!(validate_request(&request, true).is_err());
        request.probe = Some(RunnerProbe::ElementExists {
            url: "http://127.0.0.1:8080/".into(),
            selector: "x".repeat(MAX_SELECTOR_BYTES + 1),
        });
        assert!(validate_request(&request, true).is_err());
        request.probe = Some(RunnerProbe::Accessibility {
            url: "http://127.0.0.1:8080/".into(),
        });
        assert!(validate_request(&request, true).is_ok());

        request.probe = Some(RunnerProbe::BrowserWorkflow {
            name: "consumer flow".into(),
            workflow_path: "tests/browser-flow.json".into(),
            expected_sha256: "a".repeat(64),
        });
        assert!(validate_request(&request, true).is_ok());
        request.probe = Some(RunnerProbe::BrowserWorkflow {
            name: "consumer flow".into(),
            workflow_path: "../escape.json".into(),
            expected_sha256: "a".repeat(64),
        });
        assert!(validate_request(&request, true).is_err());
        request.probe = Some(RunnerProbe::BrowserWorkflow {
            name: "consumer flow".into(),
            workflow_path: "tests/browser-flow.json".into(),
            expected_sha256: "not-a-digest".into(),
        });
        assert!(validate_request(&request, true).is_err());
    }

    #[test]
    fn workflow_binding_revalidates_exact_regular_json_inside_the_copied_workspace() {
        let workspace = tempfile::tempdir().expect("workspace");
        fs::create_dir(workspace.path().join("tests")).expect("workflow directory");
        let document = BrowserWorkflowDocument {
            schema_version: mcg_browser_contracts::BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "consumer flow".into(),
            start_url: "http://127.0.0.1:43123/app".into(),
            steps: vec![BrowserWorkflowStep::AssertElement {
                selector: "main#app".into(),
            }],
        };
        let bytes = serde_json::to_vec(&document).expect("workflow JSON");
        let digest = format!("{:x}", Sha256::digest(&bytes));
        fs::write(workspace.path().join("tests/browser-flow.json"), &bytes).expect("workflow file");
        assert_eq!(
            load_bound_workflow(
                workspace.path(),
                "consumer flow",
                "tests/browser-flow.json",
                &digest,
            )
            .expect("bound workflow"),
            document
        );
        assert!(
            load_bound_workflow(
                workspace.path(),
                "different name",
                "tests/browser-flow.json",
                &digest,
            )
            .is_err()
        );
        assert!(
            load_bound_workflow(
                workspace.path(),
                "consumer flow",
                "tests/browser-flow.json",
                &"0".repeat(64),
            )
            .is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires MCG_BROWSER_EXECUTABLE and a real local Chromium runtime"]
    #[allow(clippy::too_many_lines)] // One real service proves all browser probe dispositions.
    fn real_browser_renders_page_beside_service() {
        let browser = std::env::var_os("MCG_BROWSER_EXECUTABLE")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_file())
            .expect("MCG_BROWSER_EXECUTABLE");
        let reservation =
            std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("reserve loopback port");
        let port = reservation.local_addr().expect("reserved address").port();
        drop(reservation);
        let source = tempfile::tempdir().expect("source");
        fs::write(
            source.path().join("server.ps1"),
            format!(
                r#"$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, {port})
$listener.Start()
while ($true) {{
  $client = $listener.AcceptTcpClient()
  $stream = $client.GetStream()
  $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::ASCII, $false, 1024, $true)
  $requestLine = $reader.ReadLine()
  while (($line = $reader.ReadLine()) -ne '') {{ if ($null -eq $line) {{ break }} }}
  if ($requestLine -like 'GET /inaccessible *') {{
    $body = '<!doctype html><html><body><button></button></body></html>'
  }} else {{
    $body = '<!doctype html><html lang="en"><head><title>Ready</title></head><body><main id="app"><input id="name" aria-label="Name"><button id="go">Go</button><p id="result">waiting</p></main><script>document.getElementById("go").addEventListener("click",()=>{{document.getElementById("result").textContent="Hello "+document.getElementById("name").value;}});</script></body></html>'
  }}
  $bytes = [Text.Encoding]::UTF8.GetBytes($body)
  $header = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 200 OK`r`nContent-Type: text/html; charset=utf-8`r`nContent-Length: $($bytes.Length)`r`nConnection: close`r`n`r`n")
  $stream.Write($header, 0, $header.Length)
  $stream.Write($bytes, 0, $bytes.Length)
  $stream.Flush()
  $client.Dispose()
}}
"#
            ),
        )
        .expect("service fixture");
        let destination_parent = tempfile::tempdir().expect("destination parent");
        let request = RunnerRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            source_root: source.path().to_string_lossy().into_owned(),
            workspace_root: destination_parent
                .path()
                .join("workspace")
                .to_string_lossy()
                .into_owned(),
            executable: "powershell.exe".into(),
            argv: vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                "server.ps1".into(),
            ],
            toolchain_root: None,
            browser_executable: Some(browser.to_string_lossy().into_owned()),
            probe: Some(RunnerProbe::PageRender {
                url: format!("http://127.0.0.1:{port}/"),
            }),
        };
        assert_eq!(execute_request(&request, false), Ok(0));

        let element_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("element-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::ElementExists {
                url: format!("http://127.0.0.1:{port}/"),
                selector: "main#app".into(),
            }),
            ..request.clone()
        };
        assert_eq!(execute_request(&element_request, false), Ok(0));

        let missing_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("missing-element-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::ElementExists {
                url: format!("http://127.0.0.1:{port}/"),
                selector: "#missing".into(),
            }),
            ..request.clone()
        };
        assert_eq!(
            execute_request(&missing_request, false),
            Ok(COMMAND_FAILURE_EXIT_CODE)
        );

        let accessibility_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("accessibility-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::Accessibility {
                url: format!("http://127.0.0.1:{port}/"),
            }),
            ..request.clone()
        };
        assert_eq!(execute_request(&accessibility_request, false), Ok(0));

        let workflow = BrowserWorkflowDocument {
            schema_version: mcg_browser_contracts::BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "consumer flow".into(),
            start_url: format!("http://127.0.0.1:{port}/"),
            steps: vec![
                BrowserWorkflowStep::AssertElement {
                    selector: "main#app".into(),
                },
                BrowserWorkflowStep::Type {
                    selector: "#name".into(),
                    value: "Ada".into(),
                },
                BrowserWorkflowStep::Click {
                    selector: "#go".into(),
                },
                BrowserWorkflowStep::AssertText {
                    selector: "#result".into(),
                    contains: "Hello Ada".into(),
                },
                BrowserWorkflowStep::AssertUrl {
                    url: format!("http://127.0.0.1:{port}/"),
                },
            ],
        };
        let workflow_bytes = serde_json::to_vec(&workflow).expect("workflow JSON");
        let workflow_digest = format!("{:x}", Sha256::digest(&workflow_bytes));
        fs::write(source.path().join("workflow.json"), &workflow_bytes).expect("workflow file");
        let workflow_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("workflow-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::BrowserWorkflow {
                name: workflow.name.clone(),
                workflow_path: "workflow.json".into(),
                expected_sha256: workflow_digest.clone(),
            }),
            ..request.clone()
        };
        assert_eq!(execute_request(&workflow_request, false), Ok(0));

        let workflow_mismatch_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("workflow-mismatch-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::BrowserWorkflow {
                name: workflow.name,
                workflow_path: "workflow.json".into(),
                expected_sha256: "0".repeat(64),
            }),
            ..request.clone()
        };
        assert!(execute_request(&workflow_mismatch_request, false).is_err());

        let inaccessible_request = RunnerRequest {
            workspace_root: destination_parent
                .path()
                .join("inaccessible-workspace")
                .to_string_lossy()
                .into_owned(),
            probe: Some(RunnerProbe::Accessibility {
                url: format!("http://127.0.0.1:{port}/inaccessible"),
            }),
            ..request
        };
        assert_eq!(
            execute_request(&inaccessible_request, false),
            Ok(COMMAND_FAILURE_EXIT_CODE)
        );
    }
}
