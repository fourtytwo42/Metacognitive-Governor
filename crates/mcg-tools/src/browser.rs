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
use image::{ImageFormat, ImageReader, Limits};
use mcg_domain::ToolExecutionId;
use serde::Deserialize;
use url::Url;

use crate::{
    ArtifactStore, FilesystemExecutor, TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome,
    ToolOutcomeStatus, ToolRequest, ToolRequestKind, VerificationEvidence,
};

pub struct BrowserExecutor {
    executable: Option<PathBuf>,
    artifacts: ArtifactStore,
    filesystem: Option<Arc<FilesystemExecutor>>,
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

    #[allow(clippy::too_many_lines)] // Keep browser lifecycle cleanup adjacent to evidence capture.
    async fn render(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        url: &str,
        viewport_width: u32,
        viewport_height: u32,
        comparison: Option<(&str, &str, u16, u8)>,
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
            let render_passed = ready && laid_out && captured && screenshot_passed;
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
                    None,
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
                )
                .await
            }
            _ => Err(ToolError::Invalid(
                "browser executor received a non-browser request".into(),
            )),
        }
    }
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
