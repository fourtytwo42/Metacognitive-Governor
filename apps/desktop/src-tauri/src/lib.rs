use mcg_agents::{
    ExperimentBenchmark, ExperimentDecision, ExperimentDefinition, OrganizationGenome,
    OrganizationShareReview, PortableOrganizationComparison, PortableOrganizationGenome,
    ShadowExecutionReceipt, SupervisionObservation, TrustedOrganizationPublisher,
};
use mcg_api::{
    ApplyComputerActionsRequest, ApprovalListResponse, AttachBrowserSessionRequest,
    BackupListResponse, BackupSummaryResponse, CheckSiteHealthRequest, ClosePlatformSessionRequest,
    CorrectMemoryClaimRequest, CreateEncryptedBackupRequest, CreateMemoryClaimRequest,
    CreateOrganizationRequest, CreatePlatformSessionRequest, CreateScheduleRequest,
    CreateSiteMonitorRequest, CreateSiteRequest, CreateSshHostRequest, CreateTaskRequest,
    DecideSharedOrganizationRequest, DeploySiteRequest, DiscoverProjectRequest,
    ExperimentBenchmarkListResponse, ExperimentDecisionListResponse, ExperimentListResponse,
    ImportOrganizationRequest, ImportSharedOrganizationRequest, InheritOrganizationRequest,
    InstallLocalPluginRequest, InstallRegistryPluginRequest, InvokePluginRequest,
    LaunchTaskRequest, LaunchTaskResponse, ManagedMemoryResponse, McpListResponse,
    MemoryClaimHistoryResponse, MemoryClaimListResponse, NavigateBrowserSessionRequest,
    OrganizationHistoryResponse, OrganizationListResponse, OrganizationMutationResponse,
    OrganizationPublisherListResponse, OrganizationShareReviewListResponse,
    PlatformSessionHistoryResponse, PlatformSessionListResponse, PluginInvocationHistoryResponse,
    PluginListResponse, ProjectListResponse, ProjectMemoryTransferListResponse,
    ProjectTopologyPlan, ProviderListResponse, ResolveApprovalRequest,
    ReviewSharedOrganizationRequest, ReviewedTopologyPlan, ReviewedTopologyPlanListResponse,
    RevokeOrganizationPublisherRequest, RunSshOperationRequest, ScheduleHistoryResponse,
    ScheduleListResponse, SchedulerNotificationListResponse, SetPluginEnabledRequest,
    SetPluginGrantsRequest, SetScheduleStateRequest, ShadowExecutionListResponse,
    SiteDeploymentListResponse, SiteHealthListResponse, SiteListResponse, SiteRevisionListResponse,
    SshHostListResponse, SshHostRevisionListResponse, SshOperationListResponse,
    SshWorkflowListResponse, StartOpenAiOAuthRequest, SupervisionObservationListResponse,
    TaskEventsResponse, TaskListResponse, TaskMutationResponse, ToolAuditResponse,
    TransferProjectMemoryRequest, TransitionExperimentRequest, TransitionMemoryClaimRequest,
    TransitionTaskRequest, TrustOrganizationPublisherRequest, UpdateSiteRequest,
    UpdateSshHostRequest, UpdateUserSettingsRequest, UserSettingsHistoryResponse,
    WakeScheduleRequest,
};
use mcg_config::{AppConfig, UserSettings};
use mcg_domain::{
    PlatformCapabilitySnapshot, PlatformSession, PluginInstallation, ScheduleDefinition,
    SchedulerNotification, Site, SiteMonitor, SshHost, SshWorkflow, TaskEnvelope,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{McpConnection, McpToolCatalog};
use mcg_providers::{
    ProviderConnection, ProviderModelCatalog, ProxyConversation, ProxyUsageSummary,
};
use mcg_runtime::{
    CandidateRevisionMutation, DiagnosticReport, EvidenceMutation, ExperimentMutation, McpMutation,
    MemoryClaimMutation, OAuthLoginStart, OrganizationPublisherMutation,
    OrganizationShareImportMutation, OrganizationShareReviewMutation, PlatformBrowserMutation,
    PlatformComputerActionMutation, PlatformComputerMutation, PluginInvocationMutation,
    PortableOrganizationImportMutation, ProjectMemoryTransferMutation,
    ProjectMemoryTransferPreview, ProjectMutation, ProviderMutation,
    ReviewedTopologyPlanCreateRequest, ReviewedTopologyPlanMutation, RunReplayReport,
    ScheduleWakeMutation, SiteDeploymentMutation, SiteHealthMutation, SiteMonitorMutation,
    SiteMutation, SshHostMutation, SshOperationMutation, SshWorkflowCreateRequest,
    SshWorkflowMutation, SupervisionMutation,
};
use mcg_tools::{ApprovalRequest, ComputerUseFrame, PersistentBrowserObservation};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroize;

mod service_bootstrap;
mod updater;

use service_bootstrap::LocalServiceStatus;

const LOCAL_DAEMON: &str = match option_env!("MCG_DAEMON_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:7777",
};

static LOCAL_SERVICE_STATUS: std::sync::OnceLock<LocalServiceStatus> = std::sync::OnceLock::new();

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

#[derive(Deserialize, Serialize)]
struct ProxyConversationListResponse {
    conversations: Vec<ProxyConversation>,
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

#[tauri::command]
fn local_service_status() -> LocalServiceStatus {
    LOCAL_SERVICE_STATUS.get().cloned().unwrap_or_else(|| {
        LocalServiceStatus::failed("The local service bootstrap has not run yet.".into())
    })
}

#[tauri::command]
async fn run_diagnostics() -> Result<DiagnosticReport, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/system/diagnostics"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Diagnostics could not be run: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Diagnostics").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Diagnostics response was invalid: {error}"))
}

fn daemon_token() -> Result<String, String> {
    if let Some(status) = LOCAL_SERVICE_STATUS.get()
        && !status.ready
    {
        return Err(status.detail.clone());
    }
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
    let mut response = response;
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if body.len().saturating_add(chunk.len()) > 16 * 1024 {
            body.clear();
            break;
        }
        body.extend_from_slice(&chunk);
    }
    bounded_api_error_code(&body).map_or_else(
        || format!("{action} failed ({status})."),
        |code| format!("{action} failed ({status}; code {code})."),
    )
}

fn bounded_api_error_code(body: &[u8]) -> Option<String> {
    let document: serde_json::Value = serde_json::from_slice(body).ok()?;
    let code = document.get("error")?.as_str()?;
    if code.is_empty()
        || code.len() > 64
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return None;
    }
    Some(code.to_owned())
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
async fn disconnect_provider(connection_id: String) -> Result<EventEnvelope, String> {
    let response = reqwest::Client::new()
        .delete(format!("{LOCAL_DAEMON}/api/v1/providers/{connection_id}"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Provider could not be disconnected: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider disconnect").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider disconnect response was invalid: {error}"))
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
async fn get_user_settings() -> Result<UserSettings, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/settings"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Settings could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Settings loading").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Settings response was invalid: {error}"))
}

#[tauri::command]
async fn update_user_settings(request: UpdateUserSettingsRequest) -> Result<UserSettings, String> {
    let response = reqwest::Client::new()
        .put(format!("{LOCAL_DAEMON}/api/v1/settings"))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Settings could not be updated: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Settings update").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Settings update response was invalid: {error}"))
}

#[tauri::command]
async fn user_settings_history() -> Result<UserSettingsHistoryResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/settings/history"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Settings history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Settings history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Settings history response was invalid: {error}"))
}

#[tauri::command]
async fn list_backups() -> Result<BackupListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/backups"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Backups could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Backup listing").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Backup list response was invalid: {error}"))
}

#[tauri::command]
async fn list_plugins() -> Result<PluginListResponse, String> {
    daemon_get("/api/v1/plugins", "Plugin listing").await
}

#[tauri::command]
async fn install_local_plugin(
    request: InstallLocalPluginRequest,
) -> Result<PluginInstallation, String> {
    daemon_post(
        "/api/v1/plugins/install/local",
        &request,
        "Plugin installation",
    )
    .await
}

#[tauri::command]
async fn install_registry_plugin(
    request: InstallRegistryPluginRequest,
) -> Result<PluginInstallation, String> {
    daemon_post(
        "/api/v1/plugins/install/registry",
        &request,
        "Verified plugin installation",
    )
    .await
}

#[tauri::command]
async fn set_plugin_grants(
    plugin_id: String,
    request: SetPluginGrantsRequest,
) -> Result<PluginInstallation, String> {
    daemon_post(
        &format!("/api/v1/plugins/{plugin_id}/grants"),
        &request,
        "Plugin grants",
    )
    .await
}

#[tauri::command]
async fn set_plugin_enabled(
    plugin_id: String,
    request: SetPluginEnabledRequest,
) -> Result<PluginInstallation, String> {
    daemon_post(
        &format!("/api/v1/plugins/{plugin_id}/enabled"),
        &request,
        "Plugin state",
    )
    .await
}

#[tauri::command]
async fn invoke_plugin(
    plugin_id: String,
    invocation_id: String,
    request: InvokePluginRequest,
) -> Result<PluginInvocationMutation, String> {
    let invocation_id = uuid::Uuid::parse_str(&invocation_id)
        .map_err(|_| "Plugin invocation identity must be a UUID".to_owned())?;
    daemon_post(
        &format!("/api/v1/plugins/{plugin_id}/invocations/{invocation_id}"),
        &request,
        "Plugin invocation",
    )
    .await
}

#[tauri::command]
async fn get_plugin_invocation(
    plugin_id: String,
    invocation_id: String,
) -> Result<PluginInvocationHistoryResponse, String> {
    let invocation_id = uuid::Uuid::parse_str(&invocation_id)
        .map_err(|_| "Plugin invocation identity must be a UUID".to_owned())?;
    daemon_get(
        &format!("/api/v1/plugins/{plugin_id}/invocations/{invocation_id}"),
        "Plugin invocation history",
    )
    .await
}

async fn daemon_get<T: serde::de::DeserializeOwned>(path: &str, label: &str) -> Result<T, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}{path}"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("{label} could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

async fn daemon_post<T: serde::de::DeserializeOwned>(
    path: &str,
    request: &impl Serialize,
    label: &str,
) -> Result<T, String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}{path}"))
        .bearer_auth(daemon_token()?)
        .json(request)
        .send()
        .await
        .map_err(|error| format!("{label} could not be completed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, label).await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("{label} response was invalid: {error}"))
}

#[tauri::command]
async fn create_backup(backup_id: String) -> Result<BackupSummaryResponse, String> {
    let backup_id = uuid::Uuid::parse_str(&backup_id)
        .map_err(|_| "Backup identity must be a UUID".to_owned())?;
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/backups/{backup_id}"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Backup could not be created: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Backup creation").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Backup response was invalid: {error}"))
}

#[tauri::command]
async fn create_encrypted_backup(
    backup_id: String,
    passphrase: String,
) -> Result<BackupSummaryResponse, String> {
    let backup_id = uuid::Uuid::parse_str(&backup_id)
        .map_err(|_| "Backup identity must be a UUID".to_owned())?;
    let request = CreateEncryptedBackupRequest { passphrase };
    let response = reqwest::Client::new()
        .post(format!(
            "{LOCAL_DAEMON}/api/v1/backups/{backup_id}/encrypted"
        ))
        .bearer_auth(daemon_token()?)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Encrypted backup could not be created: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Encrypted backup creation").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Encrypted backup response was invalid: {error}"))
}

#[tauri::command]
async fn verify_backup(backup_id: String) -> Result<BackupSummaryResponse, String> {
    let backup_id = uuid::Uuid::parse_str(&backup_id)
        .map_err(|_| "Backup identity must be a UUID".to_owned())?;
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/backups/{backup_id}/verify"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Backup could not be verified: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Backup verification").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Backup verification response was invalid: {error}"))
}

#[tauri::command]
async fn provider_proxy_usage() -> Result<ProxyUsageSummary, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/provider-proxy/usage"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Provider usage could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider usage").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider usage response was invalid: {error}"))
}

#[tauri::command]
async fn provider_proxy_conversations() -> Result<ProxyConversationListResponse, String> {
    let response = reqwest::Client::new()
        .get(format!(
            "{LOCAL_DAEMON}/api/v1/provider-proxy/conversations"
        ))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Provider conversations could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Provider conversations").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Provider conversation response was invalid: {error}"))
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
async fn list_organizations() -> Result<OrganizationListResponse, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/organizations")),
        None::<&()>,
        "Organization listing",
    )
    .await
}

#[tauri::command]
async fn organization_history(
    organization_id: String,
) -> Result<OrganizationHistoryResponse, String> {
    let organization_id = canonical_uuid(&organization_id, "Organization identity")?;
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/organizations/{organization_id}/history"
        )),
        None::<&()>,
        "Organization history",
    )
    .await
}

#[tauri::command]
async fn create_organization(
    request: CreateOrganizationRequest,
) -> Result<OrganizationMutationResponse, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/organizations")),
        Some(&request),
        "Organization creation",
    )
    .await
}

#[tauri::command]
async fn inherit_organization(
    organization_id: String,
    request: InheritOrganizationRequest,
) -> Result<OrganizationMutationResponse, String> {
    let organization_id = canonical_uuid(&organization_id, "Organization identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organizations/{organization_id}/inherit"
        )),
        Some(&request),
        "Organization inheritance",
    )
    .await
}

#[tauri::command]
async fn export_organization(revision_id: String) -> Result<PortableOrganizationGenome, String> {
    let revision_id = canonical_uuid(&revision_id, "Organization revision identity")?;
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/organization-revisions/{revision_id}/export"
        )),
        None::<&()>,
        "Organization export",
    )
    .await
}

#[tauri::command]
async fn import_organization(
    request: ImportOrganizationRequest,
) -> Result<PortableOrganizationImportMutation, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/organizations/import")),
        Some(&request),
        "Organization import",
    )
    .await
}

#[tauri::command]
async fn compare_organizations(
    left_revision_id: String,
    right_revision_id: String,
) -> Result<PortableOrganizationComparison, String> {
    let left_revision_id =
        canonical_uuid(&left_revision_id, "Left organization revision identity")?;
    let right_revision_id =
        canonical_uuid(&right_revision_id, "Right organization revision identity")?;
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/organization-revisions/{left_revision_id}/compare/{right_revision_id}"
        )),
        None::<&()>,
        "Organization comparison",
    )
    .await
}

#[tauri::command]
async fn list_organization_publishers() -> Result<Vec<TrustedOrganizationPublisher>, String> {
    let response: OrganizationPublisherListResponse = daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/organization-publishers")),
        None::<&()>,
        "Organization publisher listing",
    )
    .await?;
    Ok(response.publishers)
}

#[tauri::command]
async fn trust_organization_publisher(
    request: TrustOrganizationPublisherRequest,
) -> Result<OrganizationPublisherMutation, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/organization-publishers")),
        Some(&request),
        "Organization publisher trust",
    )
    .await
}

#[tauri::command]
async fn revoke_organization_publisher(
    publisher_id: String,
    request: RevokeOrganizationPublisherRequest,
) -> Result<OrganizationPublisherMutation, String> {
    let publisher_id = canonical_marketplace_id(&publisher_id, "Publisher identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organization-publishers/{publisher_id}/revoke"
        )),
        Some(&request),
        "Organization publisher revocation",
    )
    .await
}

#[tauri::command]
async fn list_organization_share_reviews() -> Result<Vec<OrganizationShareReview>, String> {
    let response: OrganizationShareReviewListResponse = daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/organization-share-reviews")),
        None::<&()>,
        "Organization share review listing",
    )
    .await?;
    Ok(response.reviews)
}

#[tauri::command]
async fn review_shared_organization(
    request: ReviewSharedOrganizationRequest,
) -> Result<OrganizationShareReviewMutation, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/organization-share-reviews")),
        Some(&request),
        "Organization share review",
    )
    .await
}

#[tauri::command]
async fn import_shared_organization(
    review_id: String,
    request: ImportSharedOrganizationRequest,
) -> Result<OrganizationShareImportMutation, String> {
    let review_id = canonical_uuid(&review_id, "Organization share review identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organization-share-reviews/{review_id}/import"
        )),
        Some(&request),
        "Organization share import",
    )
    .await
}

#[tauri::command]
async fn reject_shared_organization(
    review_id: String,
    request: DecideSharedOrganizationRequest,
) -> Result<OrganizationShareReviewMutation, String> {
    let review_id = canonical_uuid(&review_id, "Organization share review identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organization-share-reviews/{review_id}/reject"
        )),
        Some(&request),
        "Organization share rejection",
    )
    .await
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
async fn list_supervision_observations() -> Result<SupervisionObservationListResponse, String> {
    experiment_get("/api/v1/metacognition/observations", "Observation listing").await
}

#[tauri::command]
async fn record_supervision_observation(
    observation: SupervisionObservation,
) -> Result<SupervisionMutation, String> {
    experiment_post(
        "/api/v1/metacognition/observations",
        &observation,
        "Observation recording",
    )
    .await
}

#[tauri::command]
async fn record_candidate_revision(
    candidate: OrganizationGenome,
) -> Result<CandidateRevisionMutation, String> {
    experiment_post(
        "/api/v1/metacognition/candidates",
        &candidate,
        "Candidate recording",
    )
    .await
}

#[tauri::command]
async fn list_experiments(organization_id: String) -> Result<ExperimentListResponse, String> {
    experiment_get(
        &format!("/api/v1/organizations/{organization_id}/experiments"),
        "Experiment listing",
    )
    .await
}

#[tauri::command]
async fn get_experiment(experiment_id: String) -> Result<ExperimentDefinition, String> {
    experiment_get(
        &format!("/api/v1/experiments/{experiment_id}"),
        "Experiment loading",
    )
    .await
}

#[tauri::command]
async fn create_experiment(experiment: ExperimentDefinition) -> Result<ExperimentMutation, String> {
    experiment_post("/api/v1/experiments", &experiment, "Experiment creation").await
}

#[tauri::command]
async fn transition_experiment(
    experiment_id: String,
    request: TransitionExperimentRequest,
) -> Result<ExperimentMutation, String> {
    experiment_post(
        &format!("/api/v1/experiments/{experiment_id}/transition"),
        &request,
        "Experiment transition",
    )
    .await
}

#[tauri::command]
async fn list_shadow_executions(
    experiment_id: String,
) -> Result<ShadowExecutionListResponse, String> {
    experiment_get(
        &format!("/api/v1/experiments/{experiment_id}/shadow-executions"),
        "Shadow evidence loading",
    )
    .await
}

#[tauri::command]
async fn record_shadow_execution(
    experiment_id: String,
    receipt: ShadowExecutionReceipt,
) -> Result<EvidenceMutation<ShadowExecutionReceipt>, String> {
    experiment_post(
        &format!("/api/v1/experiments/{experiment_id}/shadow-executions"),
        &receipt,
        "Shadow evidence recording",
    )
    .await
}

#[tauri::command]
async fn list_experiment_benchmarks(
    experiment_id: String,
) -> Result<ExperimentBenchmarkListResponse, String> {
    experiment_get(
        &format!("/api/v1/experiments/{experiment_id}/benchmarks"),
        "Benchmark loading",
    )
    .await
}

#[tauri::command]
async fn record_experiment_benchmark(
    experiment_id: String,
    benchmark: ExperimentBenchmark,
) -> Result<EvidenceMutation<ExperimentBenchmark>, String> {
    experiment_post(
        &format!("/api/v1/experiments/{experiment_id}/benchmarks"),
        &benchmark,
        "Benchmark recording",
    )
    .await
}

#[tauri::command]
async fn list_experiment_decisions(
    experiment_id: String,
) -> Result<ExperimentDecisionListResponse, String> {
    experiment_get(
        &format!("/api/v1/experiments/{experiment_id}/decisions"),
        "Decision loading",
    )
    .await
}

#[tauri::command]
async fn decide_experiment(
    experiment_id: String,
    decision: ExperimentDecision,
) -> Result<ExperimentMutation, String> {
    experiment_post(
        &format!("/api/v1/experiments/{experiment_id}/decisions"),
        &decision,
        "Experiment decision",
    )
    .await
}

async fn experiment_get<T>(path: &str, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}{path}"))
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

async fn experiment_post<T, B>(path: &str, body: &B, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize,
{
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}{path}"))
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

#[tauri::command]
async fn list_ssh_workflows() -> Result<SshWorkflowListResponse, String> {
    ssh_workflow_get("", "SSH workflow listing").await
}

#[tauri::command]
async fn get_ssh_workflow(workflow_id: String) -> Result<SshWorkflow, String> {
    ssh_workflow_get(&format!("/{workflow_id}"), "SSH workflow loading").await
}

#[tauri::command]
async fn create_ssh_workflow(
    request: SshWorkflowCreateRequest,
) -> Result<SshWorkflowMutation, String> {
    ssh_workflow_post("", Some(&request), "SSH workflow creation").await
}

#[tauri::command]
async fn run_ssh_workflow(workflow_id: String) -> Result<SshWorkflowMutation, String> {
    ssh_workflow_post::<SshWorkflowMutation, ()>(
        &format!("/{workflow_id}/run"),
        None,
        "SSH workflow execution",
    )
    .await
}

#[tauri::command]
async fn cancel_ssh_workflow(workflow_id: String) -> Result<SshWorkflowMutation, String> {
    ssh_workflow_post::<SshWorkflowMutation, ()>(
        &format!("/{workflow_id}/cancel"),
        None,
        "SSH workflow cancellation",
    )
    .await
}

async fn ssh_workflow_get<T>(suffix: &str, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/ssh/workflows{suffix}")),
        None::<&()>,
        label,
    )
    .await
}

async fn ssh_workflow_post<T, B>(suffix: &str, body: Option<&B>, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    B: Serialize + ?Sized,
{
    let request = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}/api/v1/ssh/workflows{suffix}"))
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
async fn replay_agent_run(run_id: String) -> Result<RunReplayReport, String> {
    let response = reqwest::Client::new()
        .get(format!("{LOCAL_DAEMON}/api/v1/runs/{run_id}/replay"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Run evidence could not be replayed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Run evidence replay").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Run replay response was invalid: {error}"))
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
async fn project_topology_plan(project_id: String) -> Result<ProjectTopologyPlan, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/topology-plan"),
        "Project topology plan",
    )
    .await
}

#[tauri::command]
async fn list_reviewed_topology_plans(
    project_id: String,
) -> Result<ReviewedTopologyPlanListResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/reviewed-topology-plans"),
        "Reviewed topology plans",
    )
    .await
}

#[tauri::command]
async fn create_reviewed_topology_plan(
    project_id: String,
    request: ReviewedTopologyPlanCreateRequest,
) -> Result<ReviewedTopologyPlanMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/reviewed-topology-plans"),
        &request,
        "Reviewed topology plan creation",
    )
    .await
}

#[tauri::command]
async fn get_reviewed_topology_plan(
    project_id: String,
    plan_id: String,
) -> Result<ReviewedTopologyPlan, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let plan_id = canonical_uuid(&plan_id, "Reviewed plan identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/reviewed-topology-plans/{plan_id}"),
        "Reviewed topology plan",
    )
    .await
}

#[tauri::command]
async fn run_reviewed_topology_plan(
    project_id: String,
    plan_id: String,
) -> Result<ReviewedTopologyPlanMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let plan_id = canonical_uuid(&plan_id, "Reviewed plan identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/reviewed-topology-plans/{plan_id}/run"),
        &serde_json::json!({}),
        "Reviewed topology plan execution",
    )
    .await
}

#[tauri::command]
async fn cancel_reviewed_topology_plan(
    project_id: String,
    plan_id: String,
) -> Result<ReviewedTopologyPlanMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let plan_id = canonical_uuid(&plan_id, "Reviewed plan identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/reviewed-topology-plans/{plan_id}/cancel"),
        &serde_json::json!({}),
        "Reviewed topology plan cancellation",
    )
    .await
}

fn canonical_uuid(value: &str, label: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|_| format!("{label} must be a UUID"))
}

fn canonical_marketplace_id<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    if !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        Ok(value)
    } else {
        Err(format!("{label} is invalid"))
    }
}

#[tauri::command]
async fn list_project_memory(project_id: String) -> Result<MemoryClaimListResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/memory"),
        "Project memory",
    )
    .await
}

#[tauri::command]
async fn create_project_memory(
    project_id: String,
    request: CreateMemoryClaimRequest,
) -> Result<MemoryClaimMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/memory"),
        &request,
        "Project memory creation",
    )
    .await
}

#[tauri::command]
async fn correct_project_memory(
    project_id: String,
    claim_id: String,
    request: CorrectMemoryClaimRequest,
) -> Result<MemoryClaimMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let claim_id = canonical_uuid(&claim_id, "Memory claim identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/memory/{claim_id}/correct"),
        &request,
        "Project memory correction",
    )
    .await
}

#[tauri::command]
async fn transition_project_memory(
    project_id: String,
    claim_id: String,
    request: TransitionMemoryClaimRequest,
) -> Result<MemoryClaimMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let claim_id = canonical_uuid(&claim_id, "Memory claim identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/memory/{claim_id}/transition"),
        &request,
        "Project memory transition",
    )
    .await
}

#[tauri::command]
async fn review_project_memory_transfer(
    project_id: String,
    claim_id: String,
) -> Result<ProjectMemoryTransferPreview, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let claim_id = canonical_uuid(&claim_id, "Memory claim identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/memory/{claim_id}/transfer-preview"),
        "Project memory transfer review",
    )
    .await
}

#[tauri::command]
async fn transfer_project_memory(
    project_id: String,
    request: TransferProjectMemoryRequest,
) -> Result<ProjectMemoryTransferMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/memory/transfers"),
        &request,
        "Project memory transfer",
    )
    .await
}

#[tauri::command]
async fn list_project_memory_transfers(
    project_id: String,
) -> Result<ProjectMemoryTransferListResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/memory/transfers"),
        "Project memory transfers",
    )
    .await
}

#[tauri::command]
async fn project_memory_history(
    project_id: String,
    namespace: String,
    key: String,
) -> Result<MemoryClaimHistoryResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let mut url = reqwest::Url::parse(&format!(
        "{LOCAL_DAEMON}/api/v1/projects/{project_id}/memory/history"
    ))
    .map_err(|error| format!("Project memory history URL was invalid: {error}"))?;
    url.query_pairs_mut()
        .append_pair("namespace", &namespace)
        .append_pair("key", &key);
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Project memory history could not be loaded: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project memory history").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("Project memory history response was invalid: {error}"))
}

#[tauri::command]
async fn managed_project_memory(project_id: String) -> Result<ManagedMemoryResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/memory/managed"),
        "Managed project memory",
    )
    .await
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
#[allow(clippy::too_many_lines)] // Keeps the complete audited native command registry explicit.
/// Starts the native desktop application.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
pub fn run() {
    let service_status = service_bootstrap::ensure_local_service(LOCAL_DAEMON)
        .unwrap_or_else(LocalServiceStatus::failed);
    let _ = LOCAL_SERVICE_STATUS.set(service_status);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            runtime_label,
            local_service_status,
            run_diagnostics,
            updater::update_configuration,
            updater::check_for_update,
            updater::install_update,
            start_openai_oauth,
            complete_openai_oauth,
            cancel_openai_oauth,
            refresh_openai_oauth,
            disconnect_provider,
            list_providers,
            get_user_settings,
            update_user_settings,
            user_settings_history,
            list_backups,
            create_backup,
            create_encrypted_backup,
            verify_backup,
            list_plugins,
            install_local_plugin,
            install_registry_plugin,
            set_plugin_grants,
            set_plugin_enabled,
            invoke_plugin,
            get_plugin_invocation,
            provider_proxy_usage,
            provider_proxy_conversations,
            configure_provider,
            discover_provider,
            list_mcp_connections,
            configure_mcp_connection,
            discover_mcp_connection,
            list_schedules,
            list_supervision_observations,
            record_supervision_observation,
            record_candidate_revision,
            list_experiments,
            get_experiment,
            create_experiment,
            transition_experiment,
            list_shadow_executions,
            record_shadow_execution,
            list_experiment_benchmarks,
            record_experiment_benchmark,
            list_experiment_decisions,
            decide_experiment,
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
            list_ssh_workflows,
            get_ssh_workflow,
            create_ssh_workflow,
            run_ssh_workflow,
            cancel_ssh_workflow,
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
            list_organizations,
            organization_history,
            create_organization,
            inherit_organization,
            export_organization,
            import_organization,
            compare_organizations,
            list_organization_publishers,
            trust_organization_publisher,
            revoke_organization_publisher,
            list_organization_share_reviews,
            review_shared_organization,
            import_shared_organization,
            reject_shared_organization,
            create_task,
            launch_task,
            retry_task,
            resume_task,
            list_approvals,
            resolve_approval,
            transition_task,
            task_events,
            task_tool_audit,
            replay_agent_run,
            pick_project_directory,
            discover_project,
            list_projects,
            project_topology_plan,
            list_reviewed_topology_plans,
            create_reviewed_topology_plan,
            get_reviewed_topology_plan,
            run_reviewed_topology_plan,
            cancel_reviewed_topology_plan,
            list_project_memory,
            create_project_memory,
            correct_project_memory,
            transition_project_memory,
            review_project_memory_transfer,
            transfer_project_memory,
            list_project_memory_transfers,
            project_memory_history,
            managed_project_memory,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCG desktop application");
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_api_error_code, canonical_uuid, validate_external_url, validate_local_daemon,
    };

    #[test]
    fn daemon_error_details_expose_only_a_bounded_code() {
        let body = br#"{"error":"runtime_error","message":"unable to open C:\\Users\\owner\\private.sqlite3?token=secret"}"#;
        assert_eq!(
            bounded_api_error_code(body).as_deref(),
            Some("runtime_error")
        );
        assert!(bounded_api_error_code(br#"{"error":"bad code","message":"secret"}"#).is_none());
        assert!(bounded_api_error_code(&vec![b'a'; 20_000]).is_none());
    }

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

    #[test]
    fn organization_route_identities_reject_path_shaping() {
        assert!(canonical_uuid("00000000-0000-4000-8000-000000000001", "id").is_ok());
        assert!(canonical_uuid("../providers", "id").is_err());
        assert!(canonical_uuid("id?revision=other", "id").is_err());
    }
}
