use std::{
    io::Cursor,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chromiumoxide::{
    Browser, BrowserConfig, Element, Page,
    cdp::browser_protocol::{
        fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
        network::ErrorReason,
        page::CaptureScreenshotFormat,
    },
    page::ScreenshotParams,
};
use chrono::Utc;
use futures_util::StreamExt;
use image::{ImageFormat, ImageReader, Limits};
use mcg_domain::ToolExecutionId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    ArtifactStore, BrowserWorkflowDocument, BrowserWorkflowStep, FilesystemExecutor,
    TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest,
    ToolRequestKind, VerificationEvidence,
};

pub struct BrowserExecutor {
    executable: Option<PathBuf>,
    artifacts: ArtifactStore,
    filesystem: Option<Arc<FilesystemExecutor>>,
}

/// Reports whether the same browser configuration path used by the executor
/// can resolve a local Chrome/Chromium runtime without launching it.
#[must_use]
pub fn browser_runtime_available(executable: Option<&std::path::Path>) -> bool {
    let mut builder = BrowserConfig::builder();
    if let Some(executable) = executable {
        if !executable.is_file() {
            return false;
        }
        builder = builder.chrome_executable(executable);
    }
    builder.build().is_ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderState {
    ready_state: String,
    body_present: bool,
    width: u64,
    height: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessibilityAudit {
    schema_version: u16,
    document_language_present: bool,
    non_empty_title: bool,
    main_landmarks: u32,
    images_missing_alt: u32,
    controls_without_name: u32,
    interactive_without_name: u32,
    duplicate_ids: u32,
    heading_order_violations: u32,
    positive_tabindex: u32,
}

impl AccessibilityAudit {
    fn passed(&self) -> bool {
        self.schema_version == 1
            && self.document_language_present
            && self.non_empty_title
            && self.main_landmarks == 1
            && self.images_missing_alt == 0
            && self.controls_without_name == 0
            && self.interactive_without_name == 0
            && self.duplicate_ids == 0
            && self.heading_order_violations == 0
            && self.positive_tabindex == 0
    }

    fn detail(&self) -> String {
        format!(
            "language={}; title={}; main_landmarks={}; images_missing_alt={}; controls_without_name={}; interactive_without_name={}; duplicate_ids={}; heading_order_violations={}; positive_tabindex={}",
            self.document_language_present,
            self.non_empty_title,
            self.main_landmarks,
            self.images_missing_alt,
            self.controls_without_name,
            self.interactive_without_name,
            self.duplicate_ids,
            self.heading_order_violations,
            self.positive_tabindex
        )
    }
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

const ACCESSIBILITY_AUDIT_SCRIPT: &str = r#"
(() => {
  const text = (element) => (element?.textContent || '').trim();
  const labelledBy = (element) => (element.getAttribute('aria-labelledby') || '')
    .split(/\s+/)
    .filter(Boolean)
    .map((id) => text(document.getElementById(id)))
    .join(' ')
    .trim();
  const hasName = (element) => Boolean(
    (element.getAttribute('aria-label') || '').trim()
    || labelledBy(element)
    || text(element)
    || (element.getAttribute('alt') || '').trim()
    || (element.getAttribute('title') || '').trim()
    || (element.getAttribute('value') || '').trim()
  );
  const controlsWithoutName = Array.from(
    document.querySelectorAll("input:not([type='hidden']), select, textarea")
  ).filter((element) => {
    const labels = Array.from(element.labels || []).some((label) => text(label));
    return !labels && !hasName(element);
  }).length;
  const interactiveWithoutName = Array.from(
    document.querySelectorAll("button, a[href], [role='button'], [role='link']")
  ).filter((element) => !hasName(element)).length;
  const ids = new Set();
  let duplicateIds = 0;
  for (const element of document.querySelectorAll('[id]')) {
    if (!element.id) continue;
    if (ids.has(element.id)) duplicateIds += 1;
    ids.add(element.id);
  }
  let previousHeading = 0;
  let headingOrderViolations = 0;
  for (const heading of document.querySelectorAll('h1,h2,h3,h4,h5,h6')) {
    const level = Number(heading.tagName.slice(1));
    if (previousHeading && level > previousHeading + 1) headingOrderViolations += 1;
    previousHeading = level;
  }
  return {
    schemaVersion: 1,
    documentLanguagePresent: Boolean((document.documentElement.lang || '').trim()),
    nonEmptyTitle: Boolean((document.title || '').trim()),
    mainLandmarks: document.querySelectorAll('main, [role="main"]').length,
    imagesMissingAlt: document.querySelectorAll('img:not([alt])').length,
    controlsWithoutName,
    interactiveWithoutName,
    duplicateIds,
    headingOrderViolations,
    positiveTabindex: document.querySelectorAll('[tabindex]:not([tabindex="0"]):not([tabindex="-1"])').length
  };
})()
"#;

impl BrowserExecutor {
    /// Creates an ephemeral browser renderer. An omitted executable uses the
    /// browser library's system Chrome/Chromium detection without downloading.
    ///
    /// # Errors
    ///
    /// Returns an error when the content-addressed artifact store cannot open.
    pub fn new(
        executable: Option<PathBuf>,
        artifact_root: impl AsRef<std::path::Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            executable,
            artifacts: ArtifactStore::open(artifact_root)?,
            filesystem: None,
        })
    }

    /// Creates a renderer that can also compare screenshots with
    /// capability-confined project baselines.
    ///
    /// # Errors
    ///
    /// Returns an error when the content-addressed artifact store cannot open.
    pub fn with_filesystem(
        executable: Option<PathBuf>,
        artifact_root: impl AsRef<std::path::Path>,
        filesystem: Arc<FilesystemExecutor>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            executable,
            artifacts: ArtifactStore::open(artifact_root)?,
            filesystem: Some(filesystem),
        })
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Keep browser lifecycle cleanup adjacent to evidence capture.
    async fn render(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        url: &str,
        viewport_width: u32,
        viewport_height: u32,
        comparison: Option<(&str, &str, u16, u8)>,
        element_selector: Option<&str>,
        audit_accessibility: bool,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let target = parse_browser_url(url)?;
        if !(320..=7_680).contains(&viewport_width) || !(240..=4_320).contains(&viewport_height) {
            return Err(ToolError::Invalid(
                "browser viewport must be between 320x240 and 7680x4320".into(),
            ));
        }
        if comparison
            .is_some_and(|(_, _, max_changed_basis_points, _)| max_changed_basis_points > 10_000)
        {
            return Err(ToolError::Invalid(
                "screenshot changed-pixel tolerance cannot exceed 10000 basis points".into(),
            ));
        }
        if element_selector.is_some_and(|selector| {
            selector.trim().is_empty()
                || selector.chars().count() > 512
                || selector.chars().any(char::is_control)
        }) {
            return Err(ToolError::Invalid(
                "browser element selector must contain 1 to 512 printable characters".into(),
            ));
        }
        let origin = target.origin().ascii_serialization();
        let mut builder = BrowserConfig::builder()
            .enable_request_intercept()
            .disable_cache()
            .window_size(viewport_width, viewport_height)
            .request_timeout(Duration::from_secs(request.timeout_seconds))
            .launch_timeout(Duration::from_secs(request.timeout_seconds.min(30)));
        if let Some(executable) = &self.executable {
            if !executable.is_file() {
                return Err(ToolError::Unavailable(format!(
                    "configured browser executable does not exist: {}",
                    executable.display()
                )));
            }
            builder = builder.chrome_executable(executable);
        }
        let config = builder
            .build()
            .map_err(|error| ToolError::Execution(format!("build browser config: {error}")))?;
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|error| ToolError::Unavailable(format!("launch browser: {error}")))?;
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });
        let page = Arc::new(
            browser
                .new_page("about:blank")
                .await
                .map_err(|error| ToolError::Execution(format!("open browser page: {error}")))?,
        );
        let mut request_events =
            page.event_listener::<EventRequestPaused>()
                .await
                .map_err(|error| {
                    ToolError::Execution(format!("intercept browser requests: {error}"))
                })?;
        let intercept_page = page.clone();
        let intercept_origin = origin.clone();
        let denied_requests = Arc::new(AtomicU64::new(0));
        let denied_for_task = denied_requests.clone();
        let intercept_task = tokio::spawn(async move {
            while let Some(event) = request_events.next().await {
                let allowed = browser_request_allowed(
                    &event.request.url,
                    &event.request.method,
                    &intercept_origin,
                );
                let result = if allowed {
                    intercept_page
                        .execute(ContinueRequestParams::new(event.request_id.clone()))
                        .await
                        .map(|_| ())
                } else {
                    denied_for_task.fetch_add(1, Ordering::SeqCst);
                    intercept_page
                        .execute(
                            FailRequestParams::builder()
                                .request_id(event.request_id.clone())
                                .error_reason(ErrorReason::BlockedByClient)
                                .build()
                                .expect("complete browser failure request"),
                        )
                        .await
                        .map(|_| ())
                };
                if result.is_err() {
                    break;
                }
            }
        });
        let render_result = async {
            page.goto(target.as_str())
                .await
                .map_err(|error| ToolError::Execution(format!("navigate browser page: {error}")))?;
            let final_url = page
                .url()
                .await
                .map_err(|error| ToolError::Execution(format!("read final browser URL: {error}")))?;
            let final_url = final_url.ok_or_else(|| {
                ToolError::Execution("browser page did not report a final URL".into())
            })?;
            let final_parsed = parse_browser_url(&final_url)?;
            if final_parsed.origin().ascii_serialization() != origin {
                return Err(ToolError::Execution(
                    "cross-origin browser navigation was blocked by the render scope".into(),
                ));
            }
            let state: RenderState = page
                .evaluate(
                    "({readyState: document.readyState, bodyPresent: !!document.body, width: Math.max(document.documentElement?.scrollWidth || 0, document.body?.scrollWidth || 0), height: Math.max(document.documentElement?.scrollHeight || 0, document.body?.scrollHeight || 0)})",
                )
                .await
                .map_err(|error| ToolError::Execution(format!("inspect rendered page: {error}")))?
                .into_value()
                .map_err(|error| ToolError::Execution(format!("decode render evidence: {error}")))?;
            let title = page
                .get_title()
                .await
                .map_err(|error| ToolError::Execution(format!("read page title: {error}")))?;
            let html = page
                .content_bytes()
                .await
                .map_err(|error| ToolError::Execution(format!("capture rendered HTML: {error}")))?;
            let screenshot = page
                .screenshot(
                    ScreenshotParams::builder()
                        .format(CaptureScreenshotFormat::Png)
                        .full_page(false)
                        .build(),
                )
                .await
                .map_err(|error| ToolError::Execution(format!("capture page screenshot: {error}")))?;
            let html_artifact = self.artifacts.capture(&html, "text/html; charset=utf-8")?;
            let screenshot_artifact = self.artifacts.capture(&screenshot, "image/png")?;
            let ready = matches!(state.ready_state.as_str(), "interactive" | "complete");
            let laid_out = state.body_present && state.width > 0 && state.height > 0;
            let captured = !html.is_empty() && !screenshot.is_empty();
            let mut artifacts = vec![html_artifact, screenshot_artifact];
            let mut verification = vec![
                VerificationEvidence {
                    method: "browser.ready_state".into(),
                    passed: ready,
                    detail: state.ready_state,
                },
                VerificationEvidence {
                    method: "browser.body_layout".into(),
                    passed: laid_out,
                    detail: format!(
                        "body_present={}; width={}; height={}",
                        state.body_present, state.width, state.height
                    ),
                },
                VerificationEvidence {
                    method: "browser.screenshot".into(),
                    passed: captured,
                    detail: format!("png_bytes={}; html_bytes={}", screenshot.len(), html.len()),
                },
            ];
            let screenshot_passed = if let Some((
                baseline_path,
                expected_baseline_sha256,
                max_changed,
                threshold,
            )) = comparison
            {
                let filesystem = self.filesystem.as_ref().ok_or_else(|| {
                    ToolError::Unavailable(
                        "screenshot comparison has no project filesystem capability".into(),
                    )
                })?;
                let baseline = filesystem.read_bounded_binary(baseline_path, 67_108_864)?;
                let baseline_artifact = self.artifacts.capture(&baseline, "image/png")?;
                if !baseline_artifact
                    .sha256
                    .eq_ignore_ascii_case(expected_baseline_sha256)
                {
                    return Err(ToolError::Execution(
                        "screenshot baseline changed after the objective was bound".into(),
                    ));
                }
                let difference = compare_png_screenshots(
                    &baseline,
                    &screenshot,
                    viewport_width,
                    viewport_height,
                    max_changed,
                    threshold,
                )?;
                artifacts.push(baseline_artifact);
                verification.push(VerificationEvidence {
                    method: "browser.screenshot_pixel_diff".into(),
                    passed: difference.passed,
                    detail: difference.detail,
                });
                difference.passed
            } else {
                true
            };
            let element_passed = if let Some(selector) = element_selector {
                match page.find_elements(selector).await {
                    Ok(elements) => {
                        let found = !elements.is_empty();
                        verification.push(VerificationEvidence {
                            method: "browser.element_exists".into(),
                            passed: found,
                            detail: format!("selector={selector}; matches={}", elements.len()),
                        });
                        found
                    }
                    Err(error) => {
                        verification.push(VerificationEvidence {
                            method: "browser.element_exists".into(),
                            passed: false,
                            detail: format!("selector={selector}; inspection_failed={error}"),
                        });
                        false
                    }
                }
            } else {
                true
            };
            let accessibility_passed = if audit_accessibility {
                let audit: AccessibilityAudit = page
                    .evaluate(ACCESSIBILITY_AUDIT_SCRIPT)
                    .await
                    .map_err(|error| {
                        ToolError::Execution(format!("audit page accessibility: {error}"))
                    })?
                    .into_value()
                    .map_err(|error| {
                        ToolError::Execution(format!("decode accessibility evidence: {error}"))
                    })?;
                if audit.schema_version != 1 {
                    return Err(ToolError::Execution(
                        "browser returned an unsupported accessibility evidence schema".into(),
                    ));
                }
                let passed = audit.passed();
                let detail = audit.detail();
                let bytes = serde_json::to_vec(&audit).map_err(|error| {
                    ToolError::Execution(format!("serialize accessibility evidence: {error}"))
                })?;
                artifacts.push(
                    self.artifacts
                        .capture(&bytes, "application/vnd.mcg.accessibility+json;version=1")?,
                );
                verification.push(VerificationEvidence {
                    method: "browser.accessibility".into(),
                    passed,
                    detail,
                });
                passed
            } else {
                true
            };
            let render_passed = ready
                && laid_out
                && captured
                && screenshot_passed
                && element_passed
                && accessibility_passed;
            let denied = denied_requests.load(Ordering::SeqCst);
            verification.push(VerificationEvidence {
                method: "browser.network_scope".into(),
                passed: true,
                detail: format!("same_origin={origin}; blocked_requests={denied}"),
            });
            Ok(ToolOutcome {
                schema_version: TOOL_SCHEMA_VERSION,
                execution_id,
                request_id: request.id,
                status: if render_passed {
                    ToolOutcomeStatus::Succeeded
                } else {
                    ToolOutcomeStatus::Failed
                },
                summary: format!(
                    "Rendered {} at {}x{}{}",
                    safe_browser_url(&final_parsed),
                    state.width,
                    state.height,
                    title.map_or_else(String::new, |value| format!("; title={value}"))
                ),
                exit_code: None,
                stdout: None,
                stderr: None,
                artifacts,
                verification,
                started_at,
                finished_at: Utc::now(),
            })
        }
        .await;
        let _ = browser.close().await;
        intercept_task.abort();
        handler_task.abort();
        render_result
    }

    #[allow(clippy::too_many_lines)] // Browser lifecycle, confinement, steps, and evidence stay adjacent.
    async fn workflow(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: WorkflowParameters<'_>,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let target = self.validate_workflow(&parameters)?;
        validate_viewport(parameters.viewport_width, parameters.viewport_height)?;
        let origin = target.origin().ascii_serialization();
        let mut builder = BrowserConfig::builder()
            .enable_request_intercept()
            .disable_cache()
            .window_size(parameters.viewport_width, parameters.viewport_height)
            .request_timeout(Duration::from_secs(request.timeout_seconds))
            .launch_timeout(Duration::from_secs(request.timeout_seconds.min(30)));
        if let Some(executable) = &self.executable {
            if !executable.is_file() {
                return Err(ToolError::Unavailable(format!(
                    "configured browser executable does not exist: {}",
                    executable.display()
                )));
            }
            builder = builder.chrome_executable(executable);
        }
        let config = builder
            .build()
            .map_err(|error| ToolError::Execution(format!("build browser config: {error}")))?;
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|error| ToolError::Unavailable(format!("launch browser: {error}")))?;
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });
        let page = Arc::new(
            browser
                .new_page("about:blank")
                .await
                .map_err(|error| ToolError::Execution(format!("open browser page: {error}")))?,
        );
        let mut request_events =
            page.event_listener::<EventRequestPaused>()
                .await
                .map_err(|error| {
                    ToolError::Execution(format!("intercept browser requests: {error}"))
                })?;
        let intercept_page = page.clone();
        let intercept_origin = origin.clone();
        let denied_requests = Arc::new(AtomicU64::new(0));
        let denied_for_task = denied_requests.clone();
        let intercept_task = tokio::spawn(async move {
            while let Some(event) = request_events.next().await {
                let allowed = browser_request_allowed(
                    &event.request.url,
                    &event.request.method,
                    &intercept_origin,
                );
                let result = if allowed {
                    intercept_page
                        .execute(ContinueRequestParams::new(event.request_id.clone()))
                        .await
                        .map(|_| ())
                } else {
                    denied_for_task.fetch_add(1, Ordering::SeqCst);
                    intercept_page
                        .execute(
                            FailRequestParams::builder()
                                .request_id(event.request_id.clone())
                                .error_reason(ErrorReason::BlockedByClient)
                                .build()
                                .expect("complete browser failure request"),
                        )
                        .await
                        .map(|_| ())
                };
                if result.is_err() {
                    break;
                }
            }
        });

        let workflow_result = match tokio::time::timeout(
            Duration::from_secs(request.timeout_seconds),
            self.run_workflow(
                execution_id,
                request,
                &parameters,
                &page,
                &target,
                &origin,
                &denied_requests,
                started_at,
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(ToolError::TimedOut),
        };
        let _ = browser.close().await;
        intercept_task.abort();
        handler_task.abort();
        workflow_result
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn run_workflow(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: &WorkflowParameters<'_>,
        page: &Page,
        target: &Url,
        origin: &str,
        denied_requests: &AtomicU64,
        started_at: chrono::DateTime<Utc>,
    ) -> Result<ToolOutcome, ToolError> {
        page.goto(target.as_str())
            .await
            .map_err(|error| ToolError::Execution(format!("navigate browser page: {error}")))?;
        page.evaluate(
            "(() => { const deny = () => null; try { Object.defineProperty(window, 'open', { value: deny, configurable: false, writable: false }); } catch {} try { Object.defineProperty(window, 'showOpenFilePicker', { value: deny, configurable: false, writable: false }); } catch {} try { Object.defineProperty(window, 'showSaveFilePicker', { value: deny, configurable: false, writable: false }); } catch {} })()",
        )
        .await
        .map_err(|error| ToolError::Execution(format!("confine workflow dialogs: {error}")))?;
        let mut verification = vec![VerificationEvidence {
            method: "browser.workflow_binding".into(),
            passed: true,
            detail: format!(
                "name={}; steps={}; sha256={}",
                parameters.document.name,
                parameters.document.steps.len(),
                parameters.expected_sha256
            ),
        }];
        let mut steps_passed = true;
        for (index, step) in parameters.document.steps.iter().enumerate() {
            let result = execute_workflow_step(page, step, origin).await;
            let (passed, detail) = match result {
                Ok(detail) => (true, detail),
                Err(detail) => (false, detail),
            };
            verification.push(VerificationEvidence {
                method: "browser.workflow_step".into(),
                passed,
                detail: format!("step={}; {detail}", index + 1),
            });
            if !passed {
                steps_passed = false;
                break;
            }
            if matches!(
                step,
                BrowserWorkflowStep::Click { .. }
                    | BrowserWorkflowStep::Type { .. }
                    | BrowserWorkflowStep::Press { .. }
            ) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let current = current_browser_url(page).await?;
            if current.origin().ascii_serialization() != origin {
                steps_passed = false;
                verification.push(VerificationEvidence {
                    method: "browser.workflow_origin".into(),
                    passed: false,
                    detail: "workflow attempted cross-origin navigation".into(),
                });
                break;
            }
        }

        let final_url = current_browser_url(page).await?;
        let state: RenderState = page
            .evaluate(
                "({readyState: document.readyState, bodyPresent: !!document.body, width: Math.max(document.documentElement?.scrollWidth || 0, document.body?.scrollWidth || 0), height: Math.max(document.documentElement?.scrollHeight || 0, document.body?.scrollHeight || 0)})",
            )
            .await
            .map_err(|error| ToolError::Execution(format!("inspect workflow page: {error}")))?
            .into_value()
            .map_err(|error| ToolError::Execution(format!("decode workflow evidence: {error}")))?;
        let html = page
            .content_bytes()
            .await
            .map_err(|error| ToolError::Execution(format!("capture workflow HTML: {error}")))?;
        let screenshot = page
            .screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(false)
                    .build(),
            )
            .await
            .map_err(|error| {
                ToolError::Execution(format!("capture workflow screenshot: {error}"))
            })?;
        let html_artifact = self.artifacts.capture(&html, "text/html; charset=utf-8")?;
        let screenshot_artifact = self.artifacts.capture(&screenshot, "image/png")?;
        let ready = matches!(state.ready_state.as_str(), "interactive" | "complete");
        let laid_out = state.body_present && state.width > 0 && state.height > 0;
        let captured = !html.is_empty() && !screenshot.is_empty();
        verification.extend([
            VerificationEvidence {
                method: "browser.ready_state".into(),
                passed: ready,
                detail: state.ready_state,
            },
            VerificationEvidence {
                method: "browser.body_layout".into(),
                passed: laid_out,
                detail: format!(
                    "body_present={}; width={}; height={}",
                    state.body_present, state.width, state.height
                ),
            },
            VerificationEvidence {
                method: "browser.network_scope".into(),
                passed: true,
                detail: format!(
                    "same_origin={origin}; blocked_requests={}",
                    denied_requests.load(Ordering::SeqCst)
                ),
            },
        ]);
        let passed = steps_passed
            && ready
            && laid_out
            && captured
            && final_url.origin().ascii_serialization() == origin;
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: if passed {
                ToolOutcomeStatus::Succeeded
            } else {
                ToolOutcomeStatus::Failed
            },
            summary: format!(
                "Browser workflow {} {} at {}",
                parameters.document.name,
                if passed { "passed" } else { "failed" },
                safe_browser_url(&final_url)
            ),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts: vec![
                parameters.workflow_artifact.clone(),
                html_artifact,
                screenshot_artifact,
            ],
            verification,
            started_at,
            finished_at: Utc::now(),
        })
    }

    fn validate_workflow(&self, parameters: &WorkflowParameters<'_>) -> Result<Url, ToolError> {
        parameters.document.validate().map_err(ToolError::Invalid)?;
        validate_sha256(parameters.expected_sha256)?;
        let observed_json_sha = format!("{:x}", Sha256::digest(parameters.workflow_json));
        if !observed_json_sha.eq_ignore_ascii_case(parameters.expected_sha256)
            || !observed_json_sha.eq_ignore_ascii_case(&parameters.workflow_artifact.sha256)
        {
            return Err(ToolError::Execution(
                "browser workflow JSON does not match its immutable binding".into(),
            ));
        }
        if !matches!(
            serde_json::from_str::<BrowserWorkflowDocument>(parameters.workflow_json),
            Ok(document) if document == *parameters.document
        ) {
            return Err(ToolError::Execution(
                "parsed browser workflow does not match the immutable JSON bytes".into(),
            ));
        }
        let filesystem = self.filesystem.as_ref().ok_or_else(|| {
            ToolError::Unavailable("browser workflow has no project filesystem capability".into())
        })?;
        let current = filesystem.read_bounded_binary(parameters.workflow_path, 1_048_576)?;
        if current != parameters.workflow_json.as_bytes() {
            return Err(ToolError::Execution(
                "browser workflow changed after the objective was bound".into(),
            ));
        }
        let target = parse_browser_url(&parameters.document.start_url)?;
        Ok(target)
    }
}

#[async_trait]
impl ToolExecutor for BrowserExecutor {
    #[allow(clippy::too_many_lines)] // Keep exhaustive browser request dispatch explicit and auditable.
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::BrowserRender {
                url,
                viewport_width,
                viewport_height,
            } => {
                self.render(
                    execution_id,
                    request,
                    url,
                    *viewport_width,
                    *viewport_height,
                    None,
                    None,
                    false,
                )
                .await
            }
            ToolRequestKind::BrowserElementExists {
                url,
                selector,
                viewport_width,
                viewport_height,
            } => {
                self.render(
                    execution_id,
                    request,
                    url,
                    *viewport_width,
                    *viewport_height,
                    None,
                    Some(selector),
                    false,
                )
                .await
            }
            ToolRequestKind::BrowserAccessibility {
                url,
                viewport_width,
                viewport_height,
            } => {
                self.render(
                    execution_id,
                    request,
                    url,
                    *viewport_width,
                    *viewport_height,
                    None,
                    None,
                    true,
                )
                .await
            }
            ToolRequestKind::BrowserWorkflow {
                workflow_path,
                expected_sha256,
                workflow_artifact,
                workflow_json,
                document,
                viewport_width,
                viewport_height,
            } => {
                self.workflow(
                    execution_id,
                    request,
                    WorkflowParameters {
                        workflow_path,
                        expected_sha256,
                        workflow_artifact,
                        workflow_json,
                        document,
                        viewport_width: *viewport_width,
                        viewport_height: *viewport_height,
                    },
                )
                .await
            }
            ToolRequestKind::BrowserScreenshotMatches {
                url,
                baseline_path,
                expected_baseline_sha256,
                viewport_width,
                viewport_height,
                max_changed_basis_points,
                per_channel_threshold,
            } => {
                self.render(
                    execution_id,
                    request,
                    url,
                    *viewport_width,
                    *viewport_height,
                    Some((
                        baseline_path,
                        expected_baseline_sha256,
                        *max_changed_basis_points,
                        *per_channel_threshold,
                    )),
                    None,
                    false,
                )
                .await
            }
            _ => Err(ToolError::Invalid(
                "browser executor received a non-browser request".into(),
            )),
        }
    }
}

struct WorkflowParameters<'a> {
    workflow_path: &'a str,
    expected_sha256: &'a str,
    workflow_artifact: &'a crate::ArtifactReference,
    workflow_json: &'a str,
    document: &'a BrowserWorkflowDocument,
    viewport_width: u32,
    viewport_height: u32,
}

fn validate_viewport(width: u32, height: u32) -> Result<(), ToolError> {
    if !(320..=7_680).contains(&width) || !(240..=4_320).contains(&height) {
        return Err(ToolError::Invalid(
            "browser viewport must be between 320x240 and 7680x4320".into(),
        ));
    }
    Ok(())
}

async fn current_browser_url(page: &Page) -> Result<Url, ToolError> {
    let value = page
        .url()
        .await
        .map_err(|error| ToolError::Execution(format!("read browser URL: {error}")))?
        .ok_or_else(|| ToolError::Execution("browser page did not report a URL".into()))?;
    parse_browser_url(&value)
}

async fn execute_workflow_step(
    page: &Page,
    step: &BrowserWorkflowStep,
    origin: &str,
) -> Result<String, String> {
    match step {
        BrowserWorkflowStep::AssertElement { selector } => page
            .find_elements(selector)
            .await
            .map_err(|error| format!("assert_element selector={selector}; error={error}"))
            .and_then(|elements| {
                (!elements.is_empty())
                    .then(|| {
                        format!(
                            "assert_element selector={selector}; matches={}",
                            elements.len()
                        )
                    })
                    .ok_or_else(|| format!("assert_element selector={selector}; matches=0"))
            }),
        BrowserWorkflowStep::Click { selector } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| format!("click selector={selector}; error={error}"))?;
            validate_interaction_target(&element, origin).await?;
            element
                .click()
                .await
                .map_err(|error| format!("click selector={selector}; error={error}"))?;
            Ok(format!("click selector={selector}"))
        }
        BrowserWorkflowStep::Type { selector, value } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| format!("type selector={selector}; error={error}"))?;
            element
                .click()
                .await
                .map_err(|error| format!("type selector={selector}; error={error}"))?;
            element
                .type_str(value)
                .await
                .map_err(|error| format!("type selector={selector}; error={error}"))?;
            Ok(format!(
                "type selector={selector}; characters={}",
                value.chars().count()
            ))
        }
        BrowserWorkflowStep::Press { selector, key } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| format!("press selector={selector}; key={key}; error={error}"))?;
            validate_interaction_target(&element, origin).await?;
            element
                .focus()
                .await
                .map_err(|error| format!("press selector={selector}; key={key}; error={error}"))?;
            element
                .press_key(key)
                .await
                .map_err(|error| format!("press selector={selector}; key={key}; error={error}"))?;
            Ok(format!("press selector={selector}; key={key}"))
        }
        BrowserWorkflowStep::AssertText { selector, contains } => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|error| format!("assert_text selector={selector}; error={error}"))?;
            let text = element
                .inner_text()
                .await
                .map_err(|error| format!("assert_text selector={selector}; error={error}"))?
                .unwrap_or_default();
            text.contains(contains)
                .then(|| {
                    format!(
                        "assert_text selector={selector}; expected_characters={}",
                        contains.chars().count()
                    )
                })
                .ok_or_else(|| {
                    format!("assert_text selector={selector}; expected substring was not found")
                })
        }
        BrowserWorkflowStep::AssertUrl { url } => {
            let observed = current_browser_url(page)
                .await
                .map_err(|error| format!("assert_url error={error}"))?;
            (observed.as_str() == url)
                .then(|| format!("assert_url matched={}", safe_browser_url(&observed)))
                .ok_or_else(|| {
                    format!(
                        "assert_url expected={}; observed={}",
                        sanitized_expected_url(url),
                        safe_browser_url(&observed)
                    )
                })
        }
    }
}

async fn validate_interaction_target(element: &Element, origin: &str) -> Result<(), String> {
    let response = element
        .call_js_fn(
            "function() { const form = this.form || this.closest?.('form') || null; return JSON.stringify({ href: this.href || null, target: this.target || null, download: this.hasAttribute?.('download') || false, hasForm: !!form, formAction: form ? (this.formAction || form.action || null) : null, formTarget: form ? (this.formTarget || form.target || null) : null, formMethod: form ? (this.formMethod || form.method || 'get') : null }); }",
            false,
        )
        .await
        .map_err(|error| format!("inspect interaction target: {error}"))?;
    let target_json = response
        .result
        .value
        .ok_or_else(|| "interaction target did not return metadata".to_owned())
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "interaction target metadata was not a string".to_owned())
        })?;
    let target: InteractionTarget = serde_json::from_str(&target_json)
        .map_err(|error| format!("decode interaction target: {error}"))?;
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
        return Err("interaction would open a new context or download a file".into());
    }
    for destination in [target.href.as_deref(), target.form_action.as_deref()]
        .into_iter()
        .flatten()
    {
        let url = Url::parse(destination)
            .map_err(|error| format!("interaction has invalid destination: {error}"))?;
        if !matches!(url.scheme(), "http" | "https") || url.origin().ascii_serialization() != origin
        {
            return Err("interaction destination is outside the workflow origin".into());
        }
    }
    if target.has_form
        && !target
            .form_method
            .as_deref()
            .unwrap_or("get")
            .eq_ignore_ascii_case("get")
    {
        return Err("browser workflows cannot submit mutating forms".into());
    }
    Ok(())
}

fn sanitized_expected_url(value: &str) -> String {
    Url::parse(value).map_or_else(|_| "invalid-url".into(), |url| safe_browser_url(&url))
}

fn validate_sha256(value: &str) -> Result<(), ToolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ToolError::Invalid(
            "expected_sha256 must contain exactly 64 hexadecimal characters".into(),
        ));
    }
    Ok(())
}

struct ScreenshotDifference {
    passed: bool,
    detail: String,
}

fn compare_png_screenshots(
    baseline: &[u8],
    actual: &[u8],
    max_width: u32,
    max_height: u32,
    max_changed_basis_points: u16,
    per_channel_threshold: u8,
) -> Result<ScreenshotDifference, ToolError> {
    let baseline = decode_bounded_png(baseline, max_width, max_height, "baseline")?;
    let actual = decode_bounded_png(actual, max_width, max_height, "actual")?;
    if baseline.dimensions() != actual.dimensions() {
        return Ok(ScreenshotDifference {
            passed: false,
            detail: format!(
                "dimensions_match=false; baseline={}x{}; actual={}x{}; threshold={per_channel_threshold}; max_changed_basis_points={max_changed_basis_points}",
                baseline.width(),
                baseline.height(),
                actual.width(),
                actual.height()
            ),
        });
    }
    let total_pixels = u64::from(baseline.width()) * u64::from(baseline.height());
    let changed_pixels = baseline
        .pixels()
        .zip(actual.pixels())
        .filter(|(left, right)| {
            left.0
                .iter()
                .zip(right.0.iter())
                .any(|(left, right)| left.abs_diff(*right) > per_channel_threshold)
        })
        .count();
    let changed_pixels = u64::try_from(changed_pixels)
        .map_err(|_| ToolError::Execution("screenshot pixel count overflow".into()))?;
    let changed_basis_points = if total_pixels == 0 {
        10_000
    } else {
        changed_pixels
            .saturating_mul(10_000)
            .saturating_add(total_pixels - 1)
            / total_pixels
    };
    let passed = total_pixels > 0
        && changed_pixels.saturating_mul(10_000)
            <= u64::from(max_changed_basis_points).saturating_mul(total_pixels);
    Ok(ScreenshotDifference {
        passed,
        detail: format!(
            "dimensions_match=true; width={}; height={}; changed_pixels={changed_pixels}; total_pixels={total_pixels}; changed_basis_points={changed_basis_points}; max_changed_basis_points={max_changed_basis_points}; per_channel_threshold={per_channel_threshold}",
            baseline.width(),
            baseline.height()
        ),
    })
}

fn decode_bounded_png(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
    label: &str,
) -> Result<image::RgbaImage, ToolError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_width);
    limits.max_image_height = Some(max_height);
    limits.max_alloc = Some(
        u64::from(max_width)
            .saturating_mul(u64::from(max_height))
            .saturating_mul(8),
    );
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    reader.limits(limits);
    reader
        .decode()
        .map(image::DynamicImage::into_rgba8)
        .map_err(|error| ToolError::Execution(format!("decode {label} PNG: {error}")))
}

fn parse_browser_url(value: &str) -> Result<Url, ToolError> {
    let url = Url::parse(value)
        .map_err(|error| ToolError::Invalid(format!("invalid browser URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ToolError::Invalid(
            "browser rendering requires a credential-free HTTP(S) URL".into(),
        ));
    }
    Ok(url)
}

fn browser_request_allowed(value: &str, method: &str, origin: &str) -> bool {
    if !matches!(method, "GET" | "HEAD") {
        return false;
    }
    if value.starts_with("data:") || value.starts_with("blob:") || value == "about:blank" {
        return true;
    }
    Url::parse(value).is_ok_and(|url| url.origin().ascii_serialization() == origin)
}

fn safe_browser_url(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use axum::{Router, response::Html, routing::get};
    use image::{DynamicImage, RgbaImage};
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    use super::*;

    #[tokio::test]
    async fn missing_configured_browser_is_unavailable_not_render_failure() {
        let artifacts = tempdir().expect("browser artifacts");
        let executor = BrowserExecutor::new(
            Some(artifacts.path().join("definitely-missing-browser")),
            artifacts.path(),
        )
        .expect("browser executor");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "render fixture",
            ToolRequestKind::BrowserRender {
                url: "http://127.0.0.1:1/".into(),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        assert!(matches!(
            executor.execute(ToolExecutionId::new(), &request).await,
            Err(ToolError::Unavailable(_))
        ));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Render and comparison share one real browser/server fixture.
    async fn real_browser_renders_javascript_and_blocks_cross_origin_requests() {
        let blocked = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("blocked-origin reservation");
        let blocked_url = format!(
            "http://{}/tracker.png",
            blocked.local_addr().expect("blocked-origin address")
        );
        drop(blocked);
        let html = format!(
            r#"<!doctype html><html><head><title>Before JS</title></head><body><main id="app"></main><img src="{blocked_url}"><script>document.title='Rendered by JS'; document.querySelector('#app').textContent='Hydrated'; fetch('/mutation', {{method: 'POST'}}).catch(() => {{}});</script></body></html>"#
        );
        let app = Router::new()
            .route(
                "/",
                get(move || {
                    let html = html.clone();
                    async move { Html(html) }
                }),
            )
            .route(
                "/workflow",
                get(|| async {
                    Html(
                        r#"<!doctype html><html><body><label>Name <input id="name"></label><button id="submit">Continue</button><output id="result"></output><a id="external" href="https://example.com/">External</a><a id="download" href="/file" download>Download</a><a id="new-context" href="/other" target="_blank">New context</a><form method="post" action="/mutation"><button id="post-form-button">Post</button></form><script>document.querySelector('#submit').addEventListener('click',()=>{document.querySelector('#result').textContent='Hello '+document.querySelector('#name').value; history.pushState({},'', '/done');});</script></body></html>"#,
                    )
                }),
            )
            .route(
                "/accessible",
                get(|| async {
                    Html(
                        r#"<!doctype html><html lang="en"><head><title>Accessible fixture</title></head><body><main><h1>Account</h1><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" alt=""><label for="email">Email</label><input id="email"><button>Save</button><a href="/accessible">Refresh</a></main></body></html>"#,
                    )
                }),
            )
            .route(
                "/inaccessible",
                get(|| async {
                    Html(
                        r#"<!doctype html><html><head><title> </title></head><body><div id="duplicate"></div><div id="duplicate"></div><h1>Account</h1><h3>Details</h3><img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw=="><input><button></button><a href="/inaccessible"></a><div tabindex="2">Late focus</div></body></html>"#,
                    )
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("browser fixture listener");
        let address = listener.local_addr().expect("browser fixture address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("browser fixture");
        });
        let artifacts = tempdir().expect("browser artifacts");
        let executor = BrowserExecutor::new(
            std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from),
            artifacts.path(),
        )
        .expect("browser executor");
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "render JavaScript fixture",
            ToolRequestKind::BrowserRender {
                url: format!("http://{address}/"),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let outcome = executor
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("render outcome");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(outcome.summary.contains("title=Rendered by JS"));
        assert_eq!(outcome.artifacts.len(), 2);
        assert_eq!(outcome.artifacts[0].media_type, "text/html; charset=utf-8");
        assert_eq!(outcome.artifacts[1].media_type, "image/png");
        assert!(
            outcome
                .artifacts
                .iter()
                .all(|artifact| artifact.size_bytes > 0)
        );
        assert!(outcome.verification.iter().all(|evidence| evidence.passed));
        assert!(
            outcome
                .verification
                .iter()
                .find(|evidence| evidence.method == "browser.network_scope")
                .is_some_and(|evidence| evidence.detail.contains("blocked_requests=2"))
        );

        let element_request = ToolRequest::new(
            request.task_id,
            "verify hydrated element",
            ToolRequestKind::BrowserElementExists {
                url: format!("http://{address}/"),
                selector: "#app".into(),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let element_outcome = executor
            .execute(ToolExecutionId::new(), &element_request)
            .await
            .expect("element outcome");
        assert_eq!(element_outcome.status, ToolOutcomeStatus::Succeeded);
        assert!(element_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.element_exists"
                && evidence.passed
                && evidence.detail.contains("matches=1")
        }));
        let mut missing_request = element_request;
        if let ToolRequestKind::BrowserElementExists { selector, .. } = &mut missing_request.kind {
            *selector = "#missing".into();
        }
        let missing_outcome = executor
            .execute(ToolExecutionId::new(), &missing_request)
            .await
            .expect("missing element outcome");
        assert_eq!(missing_outcome.status, ToolOutcomeStatus::Failed);
        assert!(missing_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.element_exists"
                && !evidence.passed
                && evidence.detail.contains("matches=0")
        }));

        let accessibility_request = ToolRequest::new(
            request.task_id,
            "audit accessible page",
            ToolRequestKind::BrowserAccessibility {
                url: format!("http://{address}/accessible"),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let accessibility_outcome = executor
            .execute(ToolExecutionId::new(), &accessibility_request)
            .await
            .expect("accessibility outcome");
        assert_eq!(
            accessibility_outcome.status,
            ToolOutcomeStatus::Succeeded,
            "{accessibility_outcome:#?}"
        );
        assert_eq!(accessibility_outcome.artifacts.len(), 3);
        assert_eq!(
            accessibility_outcome.artifacts[2].media_type,
            "application/vnd.mcg.accessibility+json;version=1"
        );
        assert!(accessibility_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.accessibility"
                && evidence.passed
                && evidence.detail.contains("images_missing_alt=0")
        }));

        let inaccessible_request = ToolRequest::new(
            request.task_id,
            "audit inaccessible page",
            ToolRequestKind::BrowserAccessibility {
                url: format!("http://{address}/inaccessible"),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let inaccessible_outcome = executor
            .execute(ToolExecutionId::new(), &inaccessible_request)
            .await
            .expect("inaccessible outcome");
        assert_eq!(inaccessible_outcome.status, ToolOutcomeStatus::Failed);
        assert_eq!(inaccessible_outcome.artifacts.len(), 3);
        assert!(inaccessible_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.accessibility"
                && !evidence.passed
                && evidence.detail.contains("language=false")
                && evidence.detail.contains("images_missing_alt=1")
                && evidence.detail.contains("controls_without_name=1")
                && evidence.detail.contains("duplicate_ids=1")
                && evidence.detail.contains("heading_order_violations=1")
                && evidence.detail.contains("positive_tabindex=1")
        }));

        let project = tempdir().expect("baseline project");
        let baseline_path = project.path().join("baseline.png");
        std::fs::copy(
            artifacts.path().join(&outcome.artifacts[1].relative_path),
            &baseline_path,
        )
        .expect("copy baseline");
        let filesystem = Arc::new(
            FilesystemExecutor::open(
                [project.path().to_path_buf()],
                artifacts.path().join("file-artifacts"),
            )
            .expect("baseline filesystem"),
        );
        let comparator = BrowserExecutor::with_filesystem(
            std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from),
            artifacts.path(),
            filesystem,
        )
        .expect("comparison executor");
        let workflow_path = project.path().join("consumer-workflow.json");
        let workflow_document = BrowserWorkflowDocument {
            schema_version: crate::BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "consumer greeting".into(),
            start_url: format!("http://{address}/workflow"),
            steps: vec![
                BrowserWorkflowStep::AssertElement {
                    selector: "#name".into(),
                },
                BrowserWorkflowStep::Type {
                    selector: "#name".into(),
                    value: "Ada".into(),
                },
                BrowserWorkflowStep::Click {
                    selector: "#submit".into(),
                },
                BrowserWorkflowStep::AssertText {
                    selector: "#result".into(),
                    contains: "Hello Ada".into(),
                },
                BrowserWorkflowStep::AssertUrl {
                    url: format!("http://{address}/done"),
                },
            ],
        };
        let workflow_json = serde_json::to_string(&workflow_document).expect("workflow JSON");
        std::fs::write(&workflow_path, &workflow_json).expect("workflow file");
        let workflow_artifact = ArtifactStore::open(artifacts.path())
            .expect("workflow artifacts")
            .capture(workflow_json.as_bytes(), "application/json")
            .expect("workflow artifact");
        let workflow_request = ToolRequest::new(
            request.task_id,
            "exercise consumer workflow",
            ToolRequestKind::BrowserWorkflow {
                workflow_path: workflow_path.to_string_lossy().into_owned(),
                expected_sha256: workflow_artifact.sha256.clone(),
                workflow_artifact,
                workflow_json: workflow_json.clone(),
                document: workflow_document.clone(),
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let workflow_outcome = comparator
            .execute(ToolExecutionId::new(), &workflow_request)
            .await
            .expect("workflow outcome");
        assert_eq!(
            workflow_outcome.status,
            ToolOutcomeStatus::Succeeded,
            "{workflow_outcome:#?}"
        );
        assert_eq!(workflow_outcome.artifacts.len(), 3);
        assert_eq!(
            workflow_outcome
                .verification
                .iter()
                .filter(|evidence| evidence.method == "browser.workflow_step")
                .count(),
            5
        );
        assert!(
            workflow_outcome
                .verification
                .iter()
                .filter(|evidence| evidence.method == "browser.workflow_step")
                .all(|evidence| evidence.passed)
        );
        for selector in [
            "#external",
            "#download",
            "#new-context",
            "#post-form-button",
        ] {
            let unsafe_document = BrowserWorkflowDocument {
                schema_version: crate::BROWSER_WORKFLOW_SCHEMA_VERSION,
                name: format!("reject unsafe interaction {selector}"),
                start_url: format!("http://{address}/workflow"),
                steps: vec![BrowserWorkflowStep::Click {
                    selector: selector.into(),
                }],
            };
            let unsafe_json =
                serde_json::to_string(&unsafe_document).expect("unsafe workflow JSON");
            std::fs::write(&workflow_path, &unsafe_json).expect("unsafe workflow file");
            let unsafe_artifact = ArtifactStore::open(artifacts.path())
                .expect("unsafe workflow artifacts")
                .capture(unsafe_json.as_bytes(), "application/json")
                .expect("unsafe workflow artifact");
            let unsafe_request = ToolRequest::new(
                request.task_id,
                "reject unsafe browser interaction",
                ToolRequestKind::BrowserWorkflow {
                    workflow_path: workflow_path.to_string_lossy().into_owned(),
                    expected_sha256: unsafe_artifact.sha256.clone(),
                    workflow_artifact: unsafe_artifact,
                    workflow_json: unsafe_json,
                    document: unsafe_document,
                    viewport_width: 1_280,
                    viewport_height: 720,
                },
            );
            let unsafe_outcome = comparator
                .execute(ToolExecutionId::new(), &unsafe_request)
                .await
                .expect("unsafe workflow outcome");
            assert_eq!(unsafe_outcome.status, ToolOutcomeStatus::Failed);
            assert!(unsafe_outcome.verification.iter().any(|evidence| {
                evidence.method == "browser.workflow_step" && !evidence.passed
            }));
        }
        let mut failing_document = workflow_document.clone();
        if let BrowserWorkflowStep::AssertText { contains, .. } = &mut failing_document.steps[3] {
            *contains = "Goodbye Ada".into();
        }
        let failing_json = serde_json::to_string(&failing_document).expect("failing workflow JSON");
        std::fs::write(&workflow_path, &failing_json).expect("failing workflow file");
        let failing_artifact = ArtifactStore::open(artifacts.path())
            .expect("failing workflow artifacts")
            .capture(failing_json.as_bytes(), "application/json")
            .expect("failing workflow artifact");
        let failing_request = ToolRequest::new(
            request.task_id,
            "reject failed consumer workflow",
            ToolRequestKind::BrowserWorkflow {
                workflow_path: workflow_path.to_string_lossy().into_owned(),
                expected_sha256: failing_artifact.sha256.clone(),
                workflow_artifact: failing_artifact,
                workflow_json: failing_json,
                document: failing_document,
                viewport_width: 1_280,
                viewport_height: 720,
            },
        );
        let failing_outcome = comparator
            .execute(ToolExecutionId::new(), &failing_request)
            .await
            .expect("failing workflow outcome");
        assert_eq!(failing_outcome.status, ToolOutcomeStatus::Failed);
        assert!(failing_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.workflow_step"
                && !evidence.passed
                && evidence.detail.contains("expected substring was not found")
        }));
        std::fs::write(&workflow_path, "{}").expect("mutated workflow");
        assert!(matches!(
            comparator
                .execute(ToolExecutionId::new(), &workflow_request)
                .await,
            Err(ToolError::Execution(detail)) if detail.contains("changed after the objective was bound")
        ));
        std::fs::write(&workflow_path, &workflow_json).expect("restore workflow");
        let comparison_request = ToolRequest::new(
            request.task_id,
            "compare rendered fixture",
            ToolRequestKind::BrowserScreenshotMatches {
                url: format!("http://{address}/"),
                baseline_path: baseline_path.to_string_lossy().into_owned(),
                expected_baseline_sha256: outcome.artifacts[1].sha256.clone(),
                viewport_width: 1_280,
                viewport_height: 720,
                max_changed_basis_points: 100,
                per_channel_threshold: 8,
            },
        );
        let comparison_outcome = comparator
            .execute(ToolExecutionId::new(), &comparison_request)
            .await
            .expect("comparison outcome");
        assert_eq!(comparison_outcome.status, ToolOutcomeStatus::Succeeded);
        assert_eq!(comparison_outcome.artifacts.len(), 3);
        assert!(comparison_outcome.verification.iter().any(|evidence| {
            evidence.method == "browser.screenshot_pixel_diff"
                && evidence.passed
                && evidence.detail.contains("dimensions_match=true")
        }));
        std::fs::write(&baseline_path, b"changed after objective binding")
            .expect("mutate baseline fixture");
        assert!(matches!(
            comparator
                .execute(ToolExecutionId::new(), &comparison_request)
                .await,
            Err(ToolError::Execution(detail))
                if detail.contains("baseline changed after the objective was bound")
        ));
        server.abort();
    }

    #[test]
    fn pixel_comparison_applies_noise_and_changed_area_thresholds() {
        fn png(bytes: Vec<u8>, width: u32, height: u32) -> Vec<u8> {
            let image = RgbaImage::from_raw(width, height, bytes).expect("rgba image");
            let mut encoded = Vec::new();
            DynamicImage::ImageRgba8(image)
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
                .expect("encode png");
            encoded
        }

        let baseline = png(vec![0, 0, 0, 255, 0, 0, 0, 255], 2, 1);
        let under_noise = png(vec![8, 0, 0, 255, 0, 0, 0, 255], 2, 1);
        assert!(
            compare_png_screenshots(&baseline, &under_noise, 2, 1, 0, 8)
                .expect("noise comparison")
                .passed
        );

        let changed = png(vec![9, 0, 0, 255, 0, 0, 0, 255], 2, 1);
        let rejected = compare_png_screenshots(&baseline, &changed, 2, 1, 4_999, 8)
            .expect("rejected comparison");
        assert!(!rejected.passed);
        assert!(rejected.detail.contains("changed_basis_points=5000"));
        assert!(
            compare_png_screenshots(&baseline, &changed, 2, 1, 5_000, 8)
                .expect("accepted comparison")
                .passed
        );

        let wrong_size = png(vec![0, 0, 0, 255], 1, 1);
        assert!(
            !compare_png_screenshots(&baseline, &wrong_size, 2, 1, 10_000, 8)
                .expect("dimension comparison")
                .passed
        );
    }
}
