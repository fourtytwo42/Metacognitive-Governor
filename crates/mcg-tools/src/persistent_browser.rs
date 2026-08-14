use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use chromiumoxide::{
    Browser, BrowserConfig, Page,
    cdp::browser_protocol::{
        browser::{SetDownloadBehaviorBehavior, SetDownloadBehaviorParams},
        fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
        network::ErrorReason,
    },
};
use futures_util::StreamExt;
use mcg_domain::PlatformSessionId;
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, task::JoinHandle};
use url::Url;

use crate::ToolError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistentBrowserMode {
    Headless,
    Visible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentBrowserObservation {
    pub session_id: PlatformSessionId,
    pub mode: PersistentBrowserMode,
    pub safe_url: String,
    pub title: Option<String>,
    pub handed_off: bool,
    pub denied_requests: u64,
}

struct LiveBrowserSession {
    browser: Browser,
    page: Arc<Page>,
    handler_task: JoinHandle<()>,
    intercept_task: JoinHandle<()>,
    mode: PersistentBrowserMode,
    handed_off: bool,
    allowed_origins: BTreeSet<String>,
    denied_requests: Arc<AtomicU64>,
    profile_directory: PathBuf,
}

pub struct PersistentBrowserManager {
    executable: Option<PathBuf>,
    profile_root: PathBuf,
    sessions: Mutex<HashMap<PlatformSessionId, LiveBrowserSession>>,
}

impl std::fmt::Debug for PersistentBrowserManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersistentBrowserManager")
            .field("configured_executable", &self.executable.is_some())
            .field("profile_root", &self.profile_root)
            .finish_non_exhaustive()
    }
}

impl PersistentBrowserManager {
    /// Creates a manager and removes stale ephemeral profiles left by an interrupted process.
    ///
    /// # Errors
    /// Returns an error when the private profile root cannot be prepared or safely cleaned.
    pub fn new(
        executable: Option<PathBuf>,
        profile_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let profile_root = profile_root.as_ref().to_path_buf();
        fs::create_dir_all(&profile_root).map_err(|error| {
            ToolError::Execution(format!("create browser profile root: {error}"))
        })?;
        restrict_directory_to_owner(&profile_root)?;
        for entry in fs::read_dir(&profile_root)
            .map_err(|error| ToolError::Execution(format!("read browser profile root: {error}")))?
        {
            let entry = entry.map_err(|error| {
                ToolError::Execution(format!("read browser profile entry: {error}"))
            })?;
            let path = entry.path();
            if entry
                .file_type()
                .map_err(|error| {
                    ToolError::Execution(format!("inspect browser profile entry: {error}"))
                })?
                .is_dir()
                && is_direct_child(&profile_root, &path)
            {
                fs::remove_dir_all(&path).map_err(|error| {
                    ToolError::Execution(format!("remove stale browser profile: {error}"))
                })?;
            }
        }
        Ok(Self {
            executable,
            profile_root,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    /// Launches one persistent isolated Chromium process and page.
    ///
    /// # Errors
    /// Returns an error for invalid origins, duplicate identity, profile I/O, or browser failure.
    pub async fn attach(
        &self,
        session_id: PlatformSessionId,
        allowed_origins: &[String],
        mode: PersistentBrowserMode,
    ) -> Result<PersistentBrowserObservation, ToolError> {
        let allowed_origins = canonical_origins(allowed_origins)?;
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&session_id) {
            return Err(ToolError::Invalid(
                "persistent browser session is already attached".into(),
            ));
        }
        let profile_directory = self.profile_root.join(session_id.to_string());
        if !is_direct_child(&self.profile_root, &profile_directory) {
            return Err(ToolError::Invalid(
                "browser profile identity escaped its private root".into(),
            ));
        }
        fs::create_dir(&profile_directory).map_err(|error| {
            ToolError::Execution(format!("create isolated browser profile: {error}"))
        })?;
        if let Err(error) = restrict_directory_to_owner(&profile_directory) {
            let _ = fs::remove_dir(&profile_directory);
            return Err(error);
        }
        let launch = self
            .launch(
                session_id,
                allowed_origins.clone(),
                mode,
                &profile_directory,
            )
            .await;
        let mut live = match launch {
            Ok(live) => live,
            Err(error) => {
                let _ = fs::remove_dir_all(&profile_directory);
                return Err(error);
            }
        };
        let observation = match observe(session_id, &live).await {
            Ok(observation) => observation,
            Err(error) => {
                let _ = shutdown_live(&mut live).await;
                let _ = fs::remove_dir_all(&profile_directory);
                return Err(error);
            }
        };
        sessions.insert(session_id, live);
        Ok(observation)
    }

    async fn launch(
        &self,
        _session_id: PlatformSessionId,
        allowed_origins: BTreeSet<String>,
        mode: PersistentBrowserMode,
        profile_directory: &Path,
    ) -> Result<LiveBrowserSession, ToolError> {
        let mut builder = BrowserConfig::builder()
            .enable_request_intercept()
            .disable_cache()
            .window_size(1_280, 800)
            .user_data_dir(profile_directory)
            .request_timeout(Duration::from_secs(30))
            .launch_timeout(Duration::from_secs(30));
        if mode == PersistentBrowserMode::Visible {
            builder = builder.with_head();
        }
        if let Some(executable) = &self.executable {
            if !executable.is_file() {
                return Err(ToolError::Unavailable(
                    "configured browser executable does not exist".into(),
                ));
            }
            builder = builder.chrome_executable(executable);
        }
        let config = builder
            .build()
            .map_err(|error| ToolError::Unavailable(format!("build browser config: {error}")))?;
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
        let initialized = async {
            browser
                .execute(SetDownloadBehaviorParams::new(
                    SetDownloadBehaviorBehavior::Deny,
                ))
                .await
                .map_err(|error| {
                    ToolError::Execution(format!("deny browser downloads: {error}"))
                })?;
            let page =
                Arc::new(browser.new_page("about:blank").await.map_err(|error| {
                    ToolError::Execution(format!("open browser page: {error}"))
                })?);
            let (intercept_task, denied_requests) =
                spawn_request_interceptor(page.clone(), allowed_origins.clone()).await?;
            Ok::<_, ToolError>((page, intercept_task, denied_requests))
        }
        .await;
        let (page, intercept_task, denied_requests) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                let _ = tokio::time::timeout(Duration::from_secs(10), browser.close()).await;
                let _ = tokio::time::timeout(Duration::from_secs(5), browser.wait()).await;
                handler_task.abort();
                return Err(error);
            }
        };
        Ok(LiveBrowserSession {
            browser,
            page,
            handler_task,
            intercept_task,
            mode,
            handed_off: false,
            allowed_origins,
            denied_requests,
            profile_directory: profile_directory.to_path_buf(),
        })
    }

    /// Navigates the existing page within its immutable origin grant.
    ///
    /// # Errors
    /// Returns an error for unknown/handed-off sessions, disallowed URLs, or browser failure.
    pub async fn navigate(
        &self,
        session_id: PlatformSessionId,
        url: &str,
    ) -> Result<PersistentBrowserObservation, ToolError> {
        let target = parse_target(url)?;
        let mut sessions = self.sessions.lock().await;
        let live = sessions.get_mut(&session_id).ok_or_else(|| {
            ToolError::Unavailable("persistent browser session is not attached".into())
        })?;
        if live.handed_off {
            return Err(ToolError::Invalid(
                "MCG cannot operate a browser while its local owner has control".into(),
            ));
        }
        if !live
            .allowed_origins
            .contains(&target.origin().ascii_serialization())
        {
            return Err(ToolError::Invalid(
                "browser navigation is outside the session origin grant".into(),
            ));
        }
        live.page
            .goto(target.as_str())
            .await
            .map_err(|error| ToolError::Execution(format!("navigate browser page: {error}")))?;
        let observation = observe(session_id, live).await?;
        let final_url = Url::parse(&observation.safe_url)
            .map_err(|_| ToolError::Execution("browser returned an invalid final URL".into()))?;
        if !live
            .allowed_origins
            .contains(&final_url.origin().ascii_serialization())
        {
            live.page.goto("about:blank").await.map_err(|error| {
                ToolError::Execution(format!("clear disallowed browser redirect: {error}"))
            })?;
            return Err(ToolError::Execution(
                "cross-origin browser redirect was blocked".into(),
            ));
        }
        Ok(observation)
    }

    /// Transfers a visible browser window to its local owner.
    ///
    /// # Errors
    /// Returns an error for unknown, headless, or already handed-off sessions.
    pub async fn handoff(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<PersistentBrowserObservation, ToolError> {
        let mut sessions = self.sessions.lock().await;
        let live = sessions.get_mut(&session_id).ok_or_else(|| {
            ToolError::Unavailable("persistent browser session is not attached".into())
        })?;
        if live.mode != PersistentBrowserMode::Visible {
            return Err(ToolError::Invalid(
                "only a visible browser session can be handed to its local owner".into(),
            ));
        }
        if live.handed_off {
            return Err(ToolError::Invalid(
                "browser session is already handed to its local owner".into(),
            ));
        }
        live.handed_off = true;
        observe(session_id, live).await
    }

    /// Returns control to MCG after blanking owner-visible content.
    ///
    /// # Errors
    /// Returns an error for unknown/non-handed-off sessions or browser failure.
    pub async fn reclaim(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<PersistentBrowserObservation, ToolError> {
        let mut sessions = self.sessions.lock().await;
        let live = sessions.get_mut(&session_id).ok_or_else(|| {
            ToolError::Unavailable("persistent browser session is not attached".into())
        })?;
        if !live.handed_off {
            return Err(ToolError::Invalid(
                "browser session is not handed to its local owner".into(),
            ));
        }
        live.page.goto("about:blank").await.map_err(|error| {
            ToolError::Execution(format!("blank browser before reclaim: {error}"))
        })?;
        live.handed_off = false;
        observe(session_id, live).await
    }

    /// Closes the Chromium process and removes its isolated ephemeral profile.
    ///
    /// # Errors
    /// Returns an error for unknown sessions or failed browser/profile cleanup.
    pub async fn close(&self, session_id: PlatformSessionId) -> Result<(), ToolError> {
        if self.close_if_attached(session_id).await? {
            Ok(())
        } else {
            Err(ToolError::Unavailable(
                "persistent browser session is not attached".into(),
            ))
        }
    }

    /// Closes an attached session, succeeding when no process is attached.
    ///
    /// # Errors
    /// Returns an error only when an attached browser or its profile cannot be cleaned up.
    pub async fn close_if_attached(
        &self,
        session_id: PlatformSessionId,
    ) -> Result<bool, ToolError> {
        let Some(mut live) = self.sessions.lock().await.remove(&session_id) else {
            return Ok(false);
        };
        let close_result = shutdown_live(&mut live).await;
        let mut cleanup_result = Ok(());
        if is_direct_child(&self.profile_root, &live.profile_directory)
            && live.profile_directory.exists()
        {
            cleanup_result = fs::remove_dir_all(&live.profile_directory).map_err(|error| {
                ToolError::Execution(format!("remove isolated browser profile: {error}"))
            });
        }
        close_result.and(cleanup_result)?;
        Ok(true)
    }
}

async fn spawn_request_interceptor(
    page: Arc<Page>,
    allowed_origins: BTreeSet<String>,
) -> Result<(JoinHandle<()>, Arc<AtomicU64>), ToolError> {
    let mut request_events = page
        .event_listener::<EventRequestPaused>()
        .await
        .map_err(|error| ToolError::Execution(format!("intercept browser requests: {error}")))?;
    let denied_requests = Arc::new(AtomicU64::new(0));
    let denied_for_task = denied_requests.clone();
    let intercept_task = tokio::spawn(async move {
        while let Some(event) = request_events.next().await {
            let allowed = persistent_request_allowed(
                &event.request.url,
                &event.request.method,
                &allowed_origins,
            );
            let result = if allowed {
                page.execute(ContinueRequestParams::new(event.request_id.clone()))
                    .await
                    .map(|_| ())
            } else {
                denied_for_task.fetch_add(1, Ordering::SeqCst);
                page.execute(
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
    Ok((intercept_task, denied_requests))
}

async fn shutdown_live(live: &mut LiveBrowserSession) -> Result<(), ToolError> {
    let close_result = tokio::time::timeout(Duration::from_secs(10), live.browser.close())
        .await
        .map_err(|_| ToolError::TimedOut)
        .and_then(|result| {
            result.map_err(|error| ToolError::Execution(format!("close browser: {error}")))
        });
    let wait_result = tokio::time::timeout(Duration::from_secs(5), live.browser.wait())
        .await
        .map_err(|_| ToolError::TimedOut)
        .map(|_| ());
    live.intercept_task.abort();
    live.handler_task.abort();
    close_result.and(wait_result)
}

async fn observe(
    session_id: PlatformSessionId,
    live: &LiveBrowserSession,
) -> Result<PersistentBrowserObservation, ToolError> {
    let url = live
        .page
        .url()
        .await
        .map_err(|error| ToolError::Execution(format!("read browser URL: {error}")))?
        .unwrap_or_else(|| "about:blank".into());
    let safe_url = safe_url(&url)?;
    let title = if live.handed_off {
        None
    } else {
        live.page
            .get_title()
            .await
            .map_err(|error| ToolError::Execution(format!("read browser title: {error}")))?
    };
    Ok(PersistentBrowserObservation {
        session_id,
        mode: live.mode,
        safe_url,
        title,
        handed_off: live.handed_off,
        denied_requests: live.denied_requests.load(Ordering::SeqCst),
    })
}

fn canonical_origins(origins: &[String]) -> Result<BTreeSet<String>, ToolError> {
    if origins.is_empty() {
        return Err(ToolError::Invalid(
            "persistent browser sessions require at least one allowed origin".into(),
        ));
    }
    origins
        .iter()
        .map(|origin| {
            let url = parse_target(origin)?;
            if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some()
            {
                return Err(ToolError::Invalid(
                    "browser session grants must contain HTTP(S) origins only".into(),
                ));
            }
            Ok(url.origin().ascii_serialization())
        })
        .collect()
}

fn parse_target(value: &str) -> Result<Url, ToolError> {
    let url = Url::parse(value)
        .map_err(|error| ToolError::Invalid(format!("invalid browser URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return Err(ToolError::Invalid(
            "persistent browser requires a credential-free HTTP(S) URL".into(),
        ));
    }
    Ok(url)
}

fn persistent_request_allowed(
    value: &str,
    method: &str,
    allowed_origins: &BTreeSet<String>,
) -> bool {
    if !matches!(method, "GET" | "HEAD") {
        return false;
    }
    if value.starts_with("data:") || value.starts_with("blob:") || value == "about:blank" {
        return true;
    }
    Url::parse(value).is_ok_and(|url| {
        allowed_origins.contains(&url.origin().ascii_serialization())
            && url.username().is_empty()
            && url.password().is_none()
    })
}

fn safe_url(value: &str) -> Result<String, ToolError> {
    if value == "about:blank" {
        return Ok(value.into());
    }
    let mut url = parse_target(value)?;
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn is_direct_child(root: &Path, candidate: &Path) -> bool {
    candidate.parent() == Some(root) && candidate.file_name().is_some()
}

#[cfg(unix)]
fn restrict_directory_to_owner(path: &Path) -> Result<(), ToolError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| ToolError::Execution(format!("secure browser profile directory: {error}")))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // Windows confinement is inherited from the private app data ACL.
fn restrict_directory_to_owner(_path: &Path) -> Result<(), ToolError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::{Router, response::Html, routing::get};
    use tokio::{net::TcpListener, sync::Notify};

    use super::*;

    #[tokio::test]
    async fn persistent_session_reuses_page_blocks_cross_origin_and_cleans_profile() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let app = Router::new()
            .route(
                "/one",
                get(|| async { Html("<title>One</title><p>first</p>") }),
            )
            .route(
                "/two",
                get(|| async { Html("<title>Two</title><p>second</p>") }),
            );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fixture server");
        });
        let profiles = tempfile::tempdir().expect("profiles");
        let manager = PersistentBrowserManager::new(
            std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from),
            profiles.path(),
        )
        .expect("manager");
        let session_id = PlatformSessionId::new();
        let origin = format!("http://{address}");
        let attached = manager
            .attach(
                session_id,
                std::slice::from_ref(&origin),
                PersistentBrowserMode::Headless,
            )
            .await
            .expect("attach");
        assert_eq!(attached.safe_url, "about:blank");
        let first = manager
            .navigate(session_id, &format!("{origin}/one?secret=hidden"))
            .await
            .expect("first navigation");
        assert_eq!(first.title.as_deref(), Some("One"));
        assert!(!first.safe_url.contains("secret"));
        let second = manager
            .navigate(session_id, &format!("{origin}/two"))
            .await
            .expect("second navigation");
        assert_eq!(second.title.as_deref(), Some("Two"));
        assert!(matches!(
            manager.navigate(session_id, "https://example.com").await,
            Err(ToolError::Invalid(_))
        ));
        assert!(matches!(
            manager.handoff(session_id).await,
            Err(ToolError::Invalid(_))
        ));
        let profile = profiles.path().join(session_id.to_string());
        assert!(profile.is_dir());
        manager.close(session_id).await.expect("close");
        assert!(!profile.exists());
        server.abort();
    }

    #[tokio::test]
    #[ignore = "requires a local desktop owner or Computer Use to confirm visible handoff"]
    async fn visible_handoff_uat_blanks_before_reclaim_and_cleans_profile() {
        let owner_confirmed = Arc::new(Notify::new());
        let confirm_signal = owner_confirmed.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let app = Router::new()
            .route(
                "/start",
                get(|| async {
                    Html(
                        "<title>MCG Visible Handoff UAT</title>\
                         <main><h1>MCG Visible Handoff UAT</h1>\
                         <p>The local owner now controls this governed browser.</p>\
                         <a href='/owner-complete'>Confirm local control</a></main>",
                    )
                }),
            )
            .route(
                "/owner-complete",
                get(move || {
                    let signal = confirm_signal.clone();
                    async move {
                        signal.notify_one();
                        Html("<title>Owner control confirmed</title><h1>Owner control confirmed</h1>")
                    }
                }),
            );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fixture server");
        });
        let profiles = tempfile::tempdir().expect("profiles");
        let manager = PersistentBrowserManager::new(
            std::env::var_os("MCG_BROWSER_EXECUTABLE").map(PathBuf::from),
            profiles.path(),
        )
        .expect("manager");
        let session_id = PlatformSessionId::new();
        let origin = format!("http://{address}");
        manager
            .attach(
                session_id,
                std::slice::from_ref(&origin),
                PersistentBrowserMode::Visible,
            )
            .await
            .expect("attach visible browser");
        manager
            .navigate(session_id, &format!("{origin}/start"))
            .await
            .expect("navigate UAT page");
        let handed_off = manager.handoff(session_id).await.expect("handoff");
        assert!(handed_off.handed_off);
        assert!(handed_off.title.is_none());
        println!("Visible browser handed off; activate it and select Confirm local control.");
        if tokio::time::timeout(Duration::from_secs(300), owner_confirmed.notified())
            .await
            .is_err()
        {
            let _ = manager.close_if_attached(session_id).await;
            server.abort();
            panic!("local owner did not confirm visible browser control within five minutes");
        }
        let reclaimed = manager.reclaim(session_id).await.expect("privacy reclaim");
        assert_eq!(reclaimed.safe_url, "about:blank");
        assert!(!reclaimed.handed_off);
        manager.close(session_id).await.expect("close");
        assert!(!profiles.path().join(session_id.to_string()).exists());
        server.abort();
    }

    #[test]
    fn stale_profile_cleanup_is_confined_to_directories() {
        let profiles = tempfile::tempdir().expect("profiles");
        let stale = profiles.path().join("stale");
        fs::create_dir(&stale).expect("stale directory");
        fs::write(profiles.path().join("keep.txt"), "not a profile directory").expect("file");
        let _manager = PersistentBrowserManager::new(None, profiles.path()).expect("manager");
        assert!(!stale.exists());
        assert!(profiles.path().join("keep.txt").is_file());
    }
}
