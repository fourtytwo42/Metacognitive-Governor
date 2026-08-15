use mcg_api::{
    ApplyComputerActionsRequest, ApprovalListResponse, AttachBrowserSessionRequest,
    CheckSiteHealthRequest, ClosePlatformSessionRequest, CreatePlatformSessionRequest,
    CreateScheduleRequest, CreateSiteMonitorRequest, CreateSiteRequest, CreateSshHostRequest,
    CreateTaskRequest, DeploySiteRequest, DiscoverProjectRequest, LaunchTaskRequest,
    LaunchTaskResponse, McpListResponse, NavigateBrowserSessionRequest,
    PlatformSessionHistoryResponse, PlatformSessionListResponse, ProjectListResponse,
    ProviderListResponse, ResolveApprovalRequest, RunSshOperationRequest, ScheduleHistoryResponse,
    ScheduleListResponse, SchedulerNotificationListResponse, SetScheduleStateRequest,
    SiteDeploymentListResponse, SiteHealthListResponse, SiteListResponse, SiteRevisionListResponse,
    SshHostListResponse, SshHostRevisionListResponse, SshOperationListResponse,
    StartOpenAiOAuthRequest, TaskEventsResponse, TaskListResponse, TaskMutationResponse,
    ToolAuditResponse, TransitionTaskRequest, UpdateSiteRequest, UpdateSshHostRequest,
    WakeScheduleRequest,
};
use mcg_config::AppConfig;
use mcg_domain::{
    PlatformCapabilitySnapshot, PlatformSession, ScheduleDefinition, SchedulerNotification, Site,
    SiteMonitor, SshHost, TaskEnvelope,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{McpConnection, McpToolCatalog};
use mcg_providers::{ProviderConnection, ProviderModelCatalog};
use mcg_runtime::{
    McpMutation, OAuthLoginStart, PlatformBrowserMutation, PlatformComputerActionMutation,
    PlatformComputerMutation, ProjectMutation, ProviderMutation, ScheduleWakeMutation,
    SiteDeploymentMutation, SiteHealthMutation, SiteMonitorMutation, SiteMutation, SshHostMutation,
    SshOperationMutation,
};
use mcg_tools::{ApprovalRequest, ComputerUseFrame, PersistentBrowserObservation};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroize;

const LOCAL_DAEMON: &str = match option_env!("MCG_DAEMON_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:7777",
};

#[derive(Deserialize, Serialize)]
struct ConfigureMcpCommand {
    connection: McpConnection,
    credential: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ConfigureProviderCommand {
    connection: ProviderConnection,
    credential: Option<String>,
}

impl Drop for ConfigureProviderCommand {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

impl Drop for ConfigureMcpCommand {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

#[tauri::command]
const fn runtime_label() -> &'static str {
    "MCG desktop scaffold"
}

fn daemon_token() -> Result<String, String> {
    validate_local_daemon(LOCAL_DAEMON)?;
    let path = AppConfig::default().auth_token_path();
    std::fs::read_to_string(&path)
        .map(|token| token.trim().to_owned())
        .map_err(|error| {
            format!(
                "Cannot connect to the local MCG service at {}: {error}",
                path.display()
            )
        })
}

fn validate_local_daemon(endpoint: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|error| format!("Invalid local MCG service endpoint: {error}"))?;
    let loopback = url
        .host_str()
        .and_then(|host| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .is_some_and(|address| address.is_loopback());
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || !loopback
        || url.port().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("The MCG desktop service endpoint must be an explicit HTTP loopback address and port without credentials, paths, query strings, or fragments".into());
    }
    Ok(())
}

async fn response_error(response: reqwest::Response, action: &str) -> String {
    let status = response.status();
    let detail = response
        .text()
        .await
        .unwrap_or_else(|_| "No error details were returned".into());
    format!("{action} failed ({status}): {detail}")
}

#[tauri::command]
async fn start_openai_oauth(request: StartOpenAiOAuthRequest) -> Result<OAuthLoginStart, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/oauth/openai/start"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI login start").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI login response was invalid: {error}"))
}

#[tauri::command]
async fn complete_openai_oauth(session_id: Uuid) -> Result<ProviderMutation, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/oauth/openai/{session_id}/complete"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not complete: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI login completion").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI login response was invalid: {error}"))
}

#[tauri::command]
async fn cancel_openai_oauth(session_id: Uuid) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/oauth/openai/{session_id}/cancel"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not be cancelled: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(response_error(response, "OpenAI login cancellation").await)
    }
}

#[tauri::command]
async fn refresh_openai_oauth(connection_id: String) -> Result<EventEnvelope, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/providers/{connection_id}/oauth/refresh"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("OpenAI login could not be refreshed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "OpenAI token refresh").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("OpenAI refresh response was invalid: {error}"))
}

#[tauri::command]
async fn list_providers() -> Result<ProviderListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/providers"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Providers could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider list response was invalid: {error}"))
}

#[tauri::command]
async fn configure_provider(request: ConfigureProviderCommand) -> Result<ProviderMutation, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/providers"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Provider could not be configured: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider setup").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider setup response was invalid: {error}"))
}

#[tauri::command]
async fn discover_provider(connection_id: String) -> Result<ProviderModelCatalog, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/providers/{connection_id}/discover"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Provider discovery could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider discovery").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider discovery response was invalid: {error}"))
}

#[tauri::command]
async fn list_mcp_connections() -> Result<McpListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/mcp/connections"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("MCP connections could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP connection listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP connection response was invalid: {error}"))
}

#[tauri::command]
async fn configure_mcp_connection(request: ConfigureMcpCommand) -> Result<McpMutation, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/mcp/connections"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("MCP connection could not be configured: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP connection setup").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP setup response was invalid: {error}"))
}

#[tauri::command]
async fn discover_mcp_connection(connection_id: String) -> Result<McpToolCatalog, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/mcp/connections/{connection_id}/discover"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("MCP discovery could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "MCP discovery").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("MCP discovery response was invalid: {error}"))
}

#[tauri::command]
async fn list_tasks() -> Result<TaskListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/tasks"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Tasks could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task list response was invalid: {error}"))
}

#[tauri::command]
async fn list_schedules() -> Result<ScheduleListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/schedules"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Schedules could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Schedule listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Schedule list response was invalid: {error}"))
}

#[tauri::command]
async fn list_sites() -> Result<SiteListResponse, String> {
    site_get("", "Site listing").await
}

#[tauri::command]
async fn get_site(site_id: String) -> Result<Site, String> {
    site_get(&format!("/{site_id}"), "Site loading").await
}

#[tauri::command]
async fn register_site(request: CreateSiteRequest) -> Result<Site, String> {
    site_post("", &request, "Site registration").await
}

#[tauri::command]
async fn update_site(site_id: String, request: UpdateSiteRequest) -> Result<SiteMutation, String> {
    site_put(&format!("/{site_id}"), &request, "Site update").await
}

#[tauri::command]
async fn site_revisions(site_id: String) -> Result<SiteRevisionListResponse, String> {
    site_get(&format!("/{site_id}/revisions"), "Site revision loading").await
}

#[tauri::command]
async fn site_health_history(site_id: String) -> Result<SiteHealthListResponse, String> {
    site_get(&format!("/{site_id}/health"), "Site health history loading").await
}

#[tauri::command]
async fn check_site_health(
    site_id: String,
    request: CheckSiteHealthRequest,
) -> Result<SiteHealthMutation, String> {
    site_post(&format!("/{site_id}/health"), &request, "Site health check").await
}

#[tauri::command]
async fn get_site_monitor(site_id: String) -> Result<SiteMonitor, String> {
    site_get(&format!("/{site_id}/monitor"), "Site monitor loading").await
}

#[tauri::command]
async fn create_site_monitor(
    site_id: String,
    request: CreateSiteMonitorRequest,
) -> Result<SiteMonitorMutation, String> {
    site_post(
        &format!("/{site_id}/monitor"),
        &request,
        "Site monitor configuration",
    )
    .await
}

#[tauri::command]
async fn site_deployment_history(site_id: String) -> Result<SiteDeploymentListResponse, String> {
    site_get(
        &format!("/{site_id}/deployments"),
        "Site deployment history loading",
    )
    .await
}

#[tauri::command]
async fn deploy_site(
    site_id: String,
    request: DeploySiteRequest,
) -> Result<SiteDeploymentMutation, String> {
    site_post(
        &format!("/{site_id}/deployments"),
        &request,
        "Site deployment",
    )
    .await
}

async fn site_get<T>(suffix: &str, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/sites{suffix}"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

async fn site_post<T, B>(suffix: &str, body: &B, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/sites{suffix}"))
        .bearer_auth(daemon_token()?)
        .json(body)
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

async fn site_put<T, B>(suffix: &str, body: &B, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let response = reqwest::Client::new()
        .put(format!("{LOCAL_DAEMON}/api/v1/sites{suffix}"))
        .bearer_auth(daemon_token()?)
        .json(body)
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

#[tauri::command]
async fn list_ssh_hosts() -> Result<SshHostListResponse, String> {
    ssh_get("", "SSH host listing").await
}

#[tauri::command]
async fn get_ssh_host(host_id: String) -> Result<SshHost, String> {
    ssh_get(&format!("/{host_id}"), "SSH host loading").await
}

#[tauri::command]
async fn register_ssh_host(request: CreateSshHostRequest) -> Result<SshHost, String> {
    ssh_post("", &request, "SSH host registration").await
}

#[tauri::command]
async fn update_ssh_host(
    host_id: String,
    request: UpdateSshHostRequest,
) -> Result<SshHostMutation, String> {
    ssh_put(&format!("/{host_id}"), &request, "SSH host update").await
}

#[tauri::command]
async fn ssh_host_revisions(host_id: String) -> Result<SshHostRevisionListResponse, String> {
    ssh_get(&format!("/{host_id}/revisions"), "SSH revision loading").await
}

#[tauri::command]
async fn ssh_operation_history(host_id: String) -> Result<SshOperationListResponse, String> {
    ssh_get(
        &format!("/{host_id}/operations"),
        "SSH operation history loading",
    )
    .await
}

#[tauri::command]
async fn run_ssh_operation(
    host_id: String,
    request: RunSshOperationRequest,
) -> Result<SshOperationMutation, String> {
    ssh_post(&format!("/{host_id}/operations"), &request, "SSH operation").await
}

async fn ssh_get<T>(suffix: &str, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/ssh/hosts{suffix}")),
        None::<&()>,
        label,
    )
    .await
}

async fn ssh_post<T, B>(suffix: &str, body: &B, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/ssh/hosts{suffix}"))
        .bearer_auth(daemon_token()?)
        .json(body)
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    parse_daemon_json(response, label).await
}

async fn ssh_put<T, B>(suffix: &str, body: &B, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let response = reqwest::Client::new()
        .put(format!("{LOCAL_DAEMON}/api/v1/ssh/hosts{suffix}"))
        .bearer_auth(daemon_token()?)
        .json(body)
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    parse_daemon_json(response, label).await
}

async fn daemon_json<T, B>(
    builder: reqwest::RequestBuilder,
    body: Option<&B>,
    label: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let mut builder = builder.bearer_auth(daemon_token()?);
    if let Some(body) = body {
        builder = builder.json(body);
    }
    let response = builder
        .send()
        .await
        .map_err(|error| format!("{label} failed: {error}"))?;
    parse_daemon_json(response, label).await
}

async fn parse_daemon_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    label: &str,
) -> Result<T, String> {
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

#[tauri::command]
async fn platform_capabilities() -> Result<PlatformCapabilitySnapshot, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/platform/capabilities"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Platform capabilities could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Platform capability listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Platform capability response was invalid: {error}"))
}

#[tauri::command]
async fn list_platform_sessions() -> Result<PlatformSessionListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/platform/sessions"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Platform sessions could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Platform session listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Platform session response was invalid: {error}"))
}

#[tauri::command]
async fn request_platform_session(
    request: CreatePlatformSessionRequest,
) -> Result<PlatformSession, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/platform/sessions"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Platform session could not be requested: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Platform session request").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Platform session request response was invalid: {error}"))
}

#[tauri::command]
async fn platform_session_history(
    session_id: String,
) -> Result<PlatformSessionHistoryResponse, String> {
    let response = reqwest::Client::new()
        .get(format!(
            "{LOCAL_DAEMON}/api/v1/platform/sessions/{session_id}/history"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Platform session history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Platform session history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Platform session history response was invalid: {error}"))
}

#[tauri::command]
async fn close_platform_session(
    session_id: String,
    request: ClosePlatformSessionRequest,
) -> Result<PlatformSession, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/platform/sessions/{session_id}/close"
        ))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Platform session could not be closed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Platform session close").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Platform session close response was invalid: {error}"))
}

#[tauri::command]
async fn attach_browser_session(
    session_id: String,
    request: AttachBrowserSessionRequest,
) -> Result<PlatformBrowserMutation, String> {
    platform_browser_post(
        &session_id,
        "attach",
        Some(&request),
        "Browser session attach",
    )
    .await
}

#[tauri::command]
async fn navigate_browser_session(
    session_id: String,
    request: NavigateBrowserSessionRequest,
) -> Result<PersistentBrowserObservation, String> {
    platform_browser_post(
        &session_id,
        "navigate",
        Some(&request),
        "Browser session navigation",
    )
    .await
}

#[tauri::command]
async fn handoff_browser_session(session_id: String) -> Result<PlatformBrowserMutation, String> {
    platform_browser_post::<PlatformBrowserMutation, ()>(
        &session_id,
        "handoff",
        None,
        "Browser session handoff",
    )
    .await
}

#[tauri::command]
async fn reclaim_browser_session(session_id: String) -> Result<PlatformBrowserMutation, String> {
    platform_browser_post::<PlatformBrowserMutation, ()>(
        &session_id,
        "reclaim",
        None,
        "Browser session reclaim",
    )
    .await
}

#[tauri::command]
async fn attach_computer_session(session_id: String) -> Result<PlatformComputerMutation, String> {
    platform_computer_post::<PlatformComputerMutation, ()>(
        &session_id,
        "attach",
        None,
        "Computer session attach",
    )
    .await
}

#[tauri::command]
async fn capture_computer_session(session_id: String) -> Result<ComputerUseFrame, String> {
    platform_computer_post::<ComputerUseFrame, ()>(
        &session_id,
        "capture",
        None,
        "Computer session capture",
    )
    .await
}

#[tauri::command]
async fn apply_computer_actions(
    session_id: String,
    request: ApplyComputerActionsRequest,
) -> Result<PlatformComputerActionMutation, String> {
    platform_computer_post(
        &session_id,
        "actions",
        Some(&request),
        "Computer session actions",
    )
    .await
}

#[tauri::command]
async fn handoff_computer_session(session_id: String) -> Result<PlatformComputerMutation, String> {
    platform_computer_post::<PlatformComputerMutation, ()>(
        &session_id,
        "handoff",
        None,
        "Computer session handoff",
    )
    .await
}

#[tauri::command]
async fn reclaim_computer_session(session_id: String) -> Result<PlatformComputerMutation, String> {
    platform_computer_post::<PlatformComputerMutation, ()>(
        &session_id,
        "reclaim",
        None,
        "Computer session reclaim",
    )
    .await
}

async fn platform_computer_post<T, B>(
    session_id: &str,
    operation: &str,
    body: Option<&B>,
    label: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let client = reqwest::Client::new();
    let request = client
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/platform/sessions/{session_id}/computer/{operation}"
        ))
        .bearer_auth(daemon_token()?);
    let response = match body {
        Some(body) => request.json(body),
        None => request,
    }
    .send()
    .await
    .map_err(|error| format!("{label} failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

async fn platform_browser_post<T, B>(
    session_id: &str,
    operation: &str,
    body: Option<&B>,
    label: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let client = reqwest::Client::new();
    let request = client
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/platform/sessions/{session_id}/browser/{operation}"
        ))
        .bearer_auth(daemon_token()?);
    let response = match body {
        Some(body) => request.json(body),
        None => request,
    }
    .send()
    .await
    .map_err(|error| format!("{label} failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

#[tauri::command]
async fn create_schedule(request: CreateScheduleRequest) -> Result<ScheduleDefinition, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/schedules"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Schedule could not be created: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Schedule creation").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Schedule creation response was invalid: {error}"))
}

#[tauri::command]
async fn set_schedule_state(
    schedule_id: String,
    request: SetScheduleStateRequest,
) -> Result<ScheduleDefinition, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/schedules/{schedule_id}"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Schedule state could not be changed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Schedule state change").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Schedule state response was invalid: {error}"))
}

#[tauri::command]
async fn schedule_history(schedule_id: String) -> Result<ScheduleHistoryResponse, String> {
    let response = reqwest::Client::new()
        .get(format!(
            "{LOCAL_DAEMON}/api/v1/schedules/{schedule_id}/history"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Schedule history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Schedule history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Schedule history response was invalid: {error}"))
}

#[tauri::command]
async fn wake_schedule(
    schedule_id: String,
    request: WakeScheduleRequest,
) -> Result<ScheduleWakeMutation, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/schedules/{schedule_id}/wake"
        ))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Schedule wake could not be requested: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Schedule wake").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Schedule wake response was invalid: {error}"))
}

#[tauri::command]
async fn list_scheduler_notifications() -> Result<SchedulerNotificationListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/scheduler/notifications"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Scheduler notifications could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Scheduler notifications").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Scheduler notification response was invalid: {error}"))
}

#[tauri::command]
async fn acknowledge_scheduler_notification(
    notification_id: String,
) -> Result<SchedulerNotification, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/scheduler/notifications/{notification_id}/acknowledge"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Scheduler notification could not be acknowledged: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Scheduler notification acknowledgement").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Scheduler notification response was invalid: {error}"))
}

#[tauri::command]
async fn create_task(request: CreateTaskRequest) -> Result<TaskMutationResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not be created: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task creation").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task creation response was invalid: {error}"))
}

#[tauri::command]
async fn transition_task(
    task_id: String,
    request: TransitionTaskRequest,
) -> Result<TaskEnvelope, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task state could not be changed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task transition").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task transition response was invalid: {error}"))
}

#[tauri::command]
async fn launch_task(
    task_id: String,
    request: LaunchTaskRequest,
) -> Result<LaunchTaskResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/launch"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not be launched: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task launch").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task launch response was invalid: {error}"))
}

#[tauri::command]
async fn retry_task(
    task_id: String,
    request: LaunchTaskRequest,
) -> Result<LaunchTaskResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/retry"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not be retried: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task retry").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task retry response was invalid: {error}"))
}

#[tauri::command]
async fn resume_task(
    task_id: String,
    request: LaunchTaskRequest,
) -> Result<LaunchTaskResponse, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/resume"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Task could not resume: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task resume").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task resume response was invalid: {error}"))
}

#[tauri::command]
async fn list_approvals() -> Result<ApprovalListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/approvals?state=pending"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Approvals could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Approval listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Approval list response was invalid: {error}"))
}

#[tauri::command]
async fn resolve_approval(
    approval_id: String,
    request: ResolveApprovalRequest,
) -> Result<ApprovalRequest, String> {
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/approvals/{approval_id}/resolve"
        ))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Approval could not be resolved: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Approval resolution").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Approval response was invalid: {error}"))
}

#[tauri::command]
async fn task_events(task_id: String) -> Result<TaskEventsResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/events"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Task history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task history response was invalid: {error}"))
}

#[tauri::command]
async fn task_tool_audit(task_id: String) -> Result<ToolAuditResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/tasks/{task_id}/tool-audit"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Task evidence could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Task evidence").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Task evidence response was invalid: {error}"))
}

#[tauri::command]
async fn pick_project_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.dialog()
        .file()
        .set_title("Choose a project folder")
        .blocking_pick_folder()
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|_| "The selected folder is not a local filesystem path".to_owned())
        })
        .transpose()
}

#[tauri::command]
async fn discover_project(request: DiscoverProjectRequest) -> Result<ProjectMutation, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/projects"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Project discovery could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project discovery").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Project discovery response was invalid: {error}"))
}

#[tauri::command]
async fn list_projects() -> Result<ProjectListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/projects"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Projects could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Project list response was invalid: {error}"))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes owned command arguments.
fn open_external_url(url: String) -> Result<(), String> {
    let parsed = validate_external_url(&url)?;
    webbrowser::open(parsed.as_str())
        .map_err(|error| format!("The system browser could not be opened: {error}"))
}

fn validate_external_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| format!("The verification URL is invalid: {error}"))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("auth.openai.com") {
        return Err("Only the official OpenAI authorization site may be opened".into());
    }
    Ok(parsed)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the native desktop application.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            runtime_label,
            start_openai_oauth,
            complete_openai_oauth,
            cancel_openai_oauth,
            refresh_openai_oauth,
            list_providers,
            configure_provider,
            discover_provider,
            list_mcp_connections,
            configure_mcp_connection,
            discover_mcp_connection,
            list_schedules,
            list_sites,
            get_site,
            register_site,
            update_site,
            site_revisions,
            site_health_history,
            check_site_health,
            get_site_monitor,
            create_site_monitor,
            site_deployment_history,
            deploy_site,
            list_ssh_hosts,
            get_ssh_host,
            register_ssh_host,
            update_ssh_host,
            ssh_host_revisions,
            ssh_operation_history,
            run_ssh_operation,
            platform_capabilities,
            list_platform_sessions,
            request_platform_session,
            platform_session_history,
            close_platform_session,
            attach_browser_session,
            navigate_browser_session,
            handoff_browser_session,
            reclaim_browser_session,
            attach_computer_session,
            capture_computer_session,
            apply_computer_actions,
            handoff_computer_session,
            reclaim_computer_session,
            create_schedule,
            set_schedule_state,
            schedule_history,
            wake_schedule,
            list_scheduler_notifications,
            acknowledge_scheduler_notification,
            list_tasks,
            create_task,
            launch_task,
            retry_task,
            resume_task,
            list_approvals,
            resolve_approval,
            transition_task,
            task_events,
            task_tool_audit,
            pick_project_directory,
            discover_project,
            list_projects,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCG desktop application");
}

#[cfg(test)]
mod tests {
    use super::{validate_external_url, validate_local_daemon};

    #[test]
    fn external_launcher_allows_only_official_openai_https() {
        assert!(validate_external_url("https://auth.openai.com/authorize?client_id=test").is_ok());
        assert!(validate_external_url("http://auth.openai.com/authorize").is_err());
        assert!(validate_external_url("https://auth.openai.com.evil.example/authorize").is_err());
        assert!(validate_external_url("file:///C:/secret").is_err());
    }

    #[test]
    fn daemon_endpoint_allows_only_explicit_http_loopback() {
        assert!(validate_local_daemon("http://127.0.0.1:7777").is_ok());
        assert!(validate_local_daemon("http://[::1]:7788").is_ok());
        assert!(validate_local_daemon("https://127.0.0.1:7777").is_err());
        assert!(validate_local_daemon("http://example.com:7777").is_err());
        assert!(validate_local_daemon("http://127.0.0.1:7777/api").is_err());
        assert!(validate_local_daemon("http://user:secret@127.0.0.1:7777").is_err());
    }
}
