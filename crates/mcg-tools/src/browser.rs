use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chromiumoxide::{
    Browser, BrowserConfig,
    cdp::browser_protocol::{
        fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
        network::ErrorReason,
        page::CaptureScreenshotFormat,
    },
    page::ScreenshotParams,
};
use chrono::Utc;
use futures_util::StreamExt;
use mcg_domain::ToolExecutionId;
use serde::Deserialize;
use url::Url;

use crate::{
    ArtifactStore, TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus,
    ToolRequest, ToolRequestKind, VerificationEvidence,
};

pub struct BrowserExecutor {
    executable: Option<PathBuf>,
    artifacts: ArtifactStore,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderState {
    ready_state: String,
    body_present: bool,
    width: u64,
    height: u64,
}

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
        })
    }

    #[allow(clippy::too_many_lines)] // Keep browser lifecycle cleanup adjacent to evidence capture.
    async fn render(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        url: &str,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let target = parse_browser_url(url)?;
        if !(320..=7_680).contains(&viewport_width) || !(240..=4_320).contains(&viewport_height) {
            return Err(ToolError::Invalid(
                "browser viewport must be between 320x240 and 7680x4320".into(),
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
            let render_passed = ready && laid_out && captured;
            let denied = denied_requests.load(Ordering::SeqCst);
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
                artifacts: vec![html_artifact, screenshot_artifact],
                verification: vec![
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
                    VerificationEvidence {
                        method: "browser.network_scope".into(),
                        passed: true,
                        detail: format!("same_origin={origin}; blocked_requests={denied}"),
                    },
                ],
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
}

#[async_trait]
impl ToolExecutor for BrowserExecutor {
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
                )
                .await
            }
            _ => Err(ToolError::Invalid(
                "browser executor received a non-browser request".into(),
            )),
        }
    }
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
    use axum::{Router, response::Html, routing::get};
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
        let app = Router::new().route(
            "/",
            get(move || {
                let html = html.clone();
                async move { Html(html) }
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
        server.abort();
    }
}
