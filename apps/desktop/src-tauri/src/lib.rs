use mcg_agents::{
    AgentRunState, ExperimentBenchmark, ExperimentDecision, ExperimentDefinition,
    GovernorControlPolicy, GovernorProposal, GovernorProposalState, OrganizationGenome,
    OrganizationShareReview, PortableOrganizationComparison, PortableOrganizationGenome,
    ShadowExecutionReceipt, SupervisionObservation, TrustedOrganizationPublisher,
};
use mcg_api::{
    AgentRunListResponse, AnswerConversationInputRequest, ApplyCloudDiffRequest,
    ApplyComputerActionsRequest, ApprovalListResponse, ArchiveConversationRequest,
    AttachBrowserSessionRequest, BackupListResponse, BackupSummaryResponse, CheckSiteHealthRequest,
    ClosePlatformSessionRequest, CloudDiffApplyListResponse, CorrectMemoryClaimRequest,
    CreateConversationRequest, CreateEncryptedBackupRequest, CreateGitHubRepository,
    CreateMemoryClaimRequest, CreateOrganizationRequest, CreatePlatformSessionRequest,
    CreateScheduleRequest, CreateSiteMonitorRequest, CreateSiteRequest, CreateSshHostRequest,
    CreateTaskRequest, DecideSharedOrganizationRequest, DelegationMutationRequest,
    DelegationRetryRequest, DeploySiteRequest, DiscoverProjectRequest,
    ExperimentBenchmarkListResponse, ExperimentDecisionListResponse, ExperimentListResponse,
    GitHubConnectionStatus, GitHubLoginStart, GitHubRepository, GitHubRepositoryList,
    GovernorOverviewResponse, GovernorProposalListResponse, ImportOrganizationRequest,
    ImportSharedOrganizationRequest, InheritOrganizationRequest, InstallLocalPluginRequest,
    InstallRegistryPluginRequest, InvokePluginRequest, LaunchTaskRequest, LaunchTaskResponse,
    LinkGitHubRepository, ManagedMemoryResponse, ManagedWorktreeListResponse, McpListResponse,
    MemoryClaimHistoryResponse, MemoryClaimListResponse, NavigateBrowserSessionRequest,
    OrganizationAssignmentResponse, OrganizationHistoryResponse, OrganizationListResponse,
    OrganizationMutationResponse, OrganizationPublisherListResponse, OrganizationRevisionPreview,
    OrganizationShareReviewListResponse, OwnerGovernorApprovalRequest,
    PlatformSessionHistoryResponse, PlatformSessionListResponse, PluginInvocationHistoryResponse,
    PluginListResponse, PrepareCloudDiffApplyRequest, PreviewOrganizationRevisionRequest,
    ProjectListResponse, ProjectMemoryTransferListResponse, ProjectTopologyPlan,
    ProviderListResponse, PublishOrganizationRevisionRequest, RequestGovernorImplementationRequest,
    ResolveApprovalRequest, ReviewSharedOrganizationRequest, ReviewedTopologyPlan,
    ReviewedTopologyPlanListResponse, RevokeOrganizationPublisherRequest, RunSshOperationRequest,
    ScheduleHistoryResponse, ScheduleListResponse, SchedulerNotificationListResponse,
    SendConversationMessageRequest, SetOrganizationAssignmentRequest, SetPluginEnabledRequest,
    SetPluginGrantsRequest, SetScheduleStateRequest, ShadowExecutionListResponse,
    SiteDeploymentListResponse, SiteHealthListResponse, SiteListResponse, SiteRevisionListResponse,
    SshHostListResponse, SshHostRevisionListResponse, SshOperationListResponse,
    SshWorkflowListResponse, StartOpenAiOAuthRequest, SteerConversationTurnRequest,
    SupervisionObservationListResponse, TaskEventsResponse, TaskListResponse, TaskMutationResponse,
    ToolAuditResponse, TransferProjectMemoryRequest, TransitionAgentRunRequest,
    TransitionExperimentRequest, TransitionMemoryClaimRequest, TransitionTaskRequest,
    TrustOrganizationPublisherRequest, UpdateConversationGoalRequest,
    UpdateConversationModeRequest, UpdateConversationRequest, UpdateSiteRequest,
    UpdateSshHostRequest, UpdateUserSettingsRequest, UserSettingsHistoryResponse,
    WakeScheduleRequest, WorktreeMutationRequest,
};
use mcg_config::{AppConfig, UserSettings};
use mcg_domain::{
    CloudDiffApply, Conversation, ConversationPreview, ConversationTeamState, ConversationView,
    PlatformCapabilitySnapshot, PlatformSession, PluginInstallation, ScheduleDefinition,
    SchedulerNotification, Site, SiteMonitor, SshHost, SshWorkflow, TaskEnvelope,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{McpConnection, McpToolCatalog};
use mcg_providers::{
    ProviderConnection, ProviderModelCatalog, ProxyConversation, ProxyUsageSummary,
};
use mcg_runtime::{
    AgentRunView, CandidateRevisionMutation, CodexSiteInventory, DelegationRetryMutation,
    DelegationReview, DiagnosticReport, EvidenceMutation, ExperimentMutation,
    GovernorProposalMutation, McpMutation, MemoryClaimMutation, OAuthLoginStart,
    OrganizationPublisherMutation, OrganizationShareImportMutation,
    OrganizationShareReviewMutation, PlatformBrowserMutation, PlatformComputerActionMutation,
    PlatformComputerMutation, PluginInvocationMutation, PortableOrganizationImportMutation,
    ProjectMemoryTransferMutation, ProjectMemoryTransferPreview, ProjectMutation, ProviderMutation,
    ReviewedTopologyPlanCreateRequest, ReviewedTopologyPlanMutation, RunReplayReport,
    ScheduleWakeMutation, SiteDeploymentMutation, SiteHealthMutation, SiteMonitorMutation,
    SiteMutation, SshHostMutation, SshOperationMutation, SshWorkflowCreateRequest,
    SshWorkflowMutation, SupervisionMutation,
};
use mcg_tools::{ApprovalRequest, ComputerUseFrame, PersistentBrowserObservation};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use tauri::Manager as _;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroize;

mod portable_runtime;
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

#[derive(Deserialize)]
struct TerminalCommandRequest {
    working_directory: String,
    command: String,
}

#[derive(Serialize)]
struct TerminalCommandResult {
    working_directory: String,
    output: String,
    exit_code: i32,
}

#[derive(Serialize)]
struct OpenInApplication {
    id: &'static str,
    label: &'static str,
    available: bool,
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
    "MetaCog desktop scaffold"
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
                "Cannot connect to the local MetaCog service at {}: {error}",
                path.display()
            )
        })
}

fn validate_local_daemon(endpoint: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|error| format!("Invalid local MetaCog service endpoint: {error}"))?;
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
        return Err("The MetaCog desktop service endpoint must be an explicit HTTP loopback address and port without credentials, paths, query strings, or fragments".into());
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
async fn github_status() -> Result<GitHubConnectionStatus, String> {
    daemon_get("/api/v1/github", "GitHub connection").await
}

#[tauri::command]
async fn start_github_login() -> Result<GitHubLoginStart, String> {
    daemon_post(
        "/api/v1/github/login/start",
        &serde_json::json!({}),
        "GitHub sign-in",
    )
    .await
}

#[tauri::command]
async fn complete_github_login(session_id: Uuid) -> Result<GitHubConnectionStatus, String> {
    daemon_post(
        &format!("/api/v1/github/login/{session_id}/complete"),
        &serde_json::json!({}),
        "GitHub sign-in completion",
    )
    .await
}

#[tauri::command]
async fn disconnect_github() -> Result<GitHubConnectionStatus, String> {
    let response = reqwest::Client::new()
        .delete(format!("{LOCAL_DAEMON}/api/v1/github"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("GitHub disconnect failed: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "GitHub disconnect").await);
    }
    response
        .json()
        .await
        .map_err(|error| format!("GitHub disconnect response was invalid: {error}"))
}

#[tauri::command]
async fn list_github_repositories() -> Result<GitHubRepositoryList, String> {
    daemon_get("/api/v1/github/repositories", "GitHub repositories").await
}

#[tauri::command]
async fn create_github_repository(
    request: CreateGitHubRepository,
) -> Result<GitHubRepository, String> {
    daemon_post(
        "/api/v1/github/repositories",
        &request,
        "GitHub repository creation",
    )
    .await
}

#[tauri::command]
async fn link_github_repository(request: LinkGitHubRepository) -> Result<(), String> {
    daemon_post_empty(
        "/api/v1/github/repositories/link",
        &request,
        "GitHub repository link",
    )
    .await
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

async fn daemon_post_empty(
    path: &str,
    request: &impl Serialize,
    label: &str,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(format!("{LOCAL_DAEMON}{path}"))
        .bearer_auth(daemon_token()?)
        .json(request)
        .send()
        .await
        .map_err(|error| format!("{label} could not be completed: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(response_error(response, label).await)
    }
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
async fn list_conversations(
    include_archived: Option<bool>,
) -> Result<mcg_api::ConversationListResponse, String> {
    let include_archived = include_archived.unwrap_or(false);
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/conversations?include_archived={include_archived}"
        )),
        None::<&()>,
        "Conversation listing",
    )
    .await
}

#[tauri::command]
async fn create_conversation(request: CreateConversationRequest) -> Result<Conversation, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/conversations")),
        Some(&request),
        "Conversation creation",
    )
    .await
}

#[tauri::command]
async fn get_conversation(conversation_id: String) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}"
        )),
        None::<&()>,
        "Conversation loading",
    )
    .await
}

#[tauri::command]
async fn update_conversation(
    conversation_id: String,
    request: UpdateConversationRequest,
) -> Result<Conversation, String> {
    daemon_json(
        reqwest::Client::new().put(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}"
        )),
        Some(&request),
        "Conversation update",
    )
    .await
}

#[tauri::command]
async fn archive_conversation(
    conversation_id: String,
    archived: bool,
) -> Result<Conversation, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/archive"
        )),
        Some(&ArchiveConversationRequest { archived }),
        if archived {
            "Conversation archive"
        } else {
            "Conversation restore"
        },
    )
    .await
}

#[tauri::command]
async fn send_conversation_message(
    conversation_id: String,
    request: SendConversationMessageRequest,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/messages"
        )),
        Some(&request),
        "Conversation message",
    )
    .await
}

#[tauri::command]
async fn search_project_mentions(
    project_id: String,
    query: String,
    limit: usize,
) -> Result<mcg_api::MentionCandidateListResponse, String> {
    daemon_json(
        reqwest::Client::new()
            .get(format!(
                "{LOCAL_DAEMON}/api/v1/projects/{project_id}/mentions"
            ))
            .query(&[("q", query), ("limit", limit.clamp(1, 100).to_string())]),
        None::<&()>,
        "Project mention search",
    )
    .await
}

#[tauri::command]
async fn update_conversation_mode(
    conversation_id: String,
    request: UpdateConversationModeRequest,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/mode"
        )),
        Some(&request),
        "Conversation mode",
    )
    .await
}

#[tauri::command]
async fn update_conversation_goal(
    conversation_id: String,
    request: UpdateConversationGoalRequest,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/goal"
        )),
        Some(&request),
        "Conversation goal",
    )
    .await
}

#[tauri::command]
async fn clear_conversation_goal(conversation_id: String) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().delete(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/goal"
        )),
        None::<&()>,
        "Conversation goal removal",
    )
    .await
}

#[tauri::command]
async fn answer_conversation_input(
    conversation_id: String,
    request: AnswerConversationInputRequest,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/input"
        )),
        Some(&request),
        "Planning answer",
    )
    .await
}

#[tauri::command]
async fn get_conversation_team(conversation_id: String) -> Result<ConversationTeamState, String> {
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/team"
        )),
        None::<&()>,
        "Conversation team",
    )
    .await
}

#[tauri::command]
async fn review_conversation_delegation(
    conversation_id: String,
    delegation_id: String,
) -> Result<DelegationReview, String> {
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/delegations/{delegation_id}/review"
        )),
        None::<&()>,
        "Coding delegation review",
    )
    .await
}

#[tauri::command]
async fn cancel_conversation_delegation(
    conversation_id: String,
    delegation_id: String,
    expected_revision: u64,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/delegations/{delegation_id}/cancel"
        )),
        Some(&DelegationMutationRequest { expected_revision }),
        "Coding delegation cancellation",
    )
    .await
}

#[tauri::command]
async fn retry_conversation_delegation(
    conversation_id: String,
    delegation_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<DelegationRetryMutation, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/delegations/{delegation_id}/retry"
        )),
        Some(&DelegationRetryRequest {
            expected_revision,
            idempotency_key,
        }),
        "Coding delegation retry",
    )
    .await
}

#[tauri::command]
async fn apply_conversation_delegation(
    conversation_id: String,
    delegation_id: String,
    expected_revision: u64,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/delegations/{delegation_id}/apply"
        )),
        Some(&DelegationMutationRequest { expected_revision }),
        "Coding delegation integration",
    )
    .await
}

#[tauri::command]
async fn discard_conversation_delegation(
    conversation_id: String,
    delegation_id: String,
    expected_revision: u64,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/delegations/{delegation_id}/discard"
        )),
        Some(&WorktreeMutationRequest { expected_revision }),
        "Coding delegation discard",
    )
    .await
}

#[tauri::command]
async fn start_conversation_preview(
    conversation_id: String,
    preview_id: String,
) -> Result<ConversationPreview, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/previews/{preview_id}/start"
        )),
        None::<&()>,
        "Conversation preview start",
    )
    .await
}

#[tauri::command]
async fn stop_conversation_preview(
    conversation_id: String,
    preview_id: String,
) -> Result<ConversationPreview, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/previews/{preview_id}/stop"
        )),
        None::<&()>,
        "Conversation preview stop",
    )
    .await
}

#[tauri::command]
async fn interrupt_conversation_turn(
    conversation_id: String,
    turn_id: String,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/turns/{turn_id}/interrupt"
        )),
        None::<&()>,
        "Conversation interruption",
    )
    .await
}

#[tauri::command]
async fn steer_conversation_turn(
    conversation_id: String,
    turn_id: String,
    content: String,
    mentions: Vec<mcg_domain::ComposerMentionInput>,
) -> Result<ConversationView, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/turns/{turn_id}/steer"
        )),
        Some(&SteerConversationTurnRequest { content, mentions }),
        "Conversation steer",
    )
    .await
}

#[tauri::command]
async fn codex_models() -> Result<Vec<mcg_codex_app_server::CodexModel>, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/models")),
        None::<&()>,
        "Codex model discovery",
    )
    .await
}

#[tauri::command]
async fn codex_features() -> Result<Vec<mcg_codex_app_server::CodexFeature>, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/features")),
        None::<&()>,
        "Codex feature discovery",
    )
    .await
}

#[tauri::command]
async fn codex_permission_profiles(
    project_id: Option<String>,
) -> Result<Vec<mcg_codex_app_server::CodexPermissionProfile>, String> {
    let mut request =
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/permission-profiles"));
    if let Some(project_id) = project_id {
        request = request.query(&[("project_id", project_id)]);
    }
    daemon_json(request, None::<&()>, "Codex permission profile discovery").await
}

#[tauri::command]
async fn set_codex_feature(
    name: String,
    enabled: bool,
) -> Result<Vec<mcg_codex_app_server::CodexFeature>, String> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("Codex feature name must be a lowercase key".into());
    }
    daemon_json(
        reqwest::Client::new().put(format!("{LOCAL_DAEMON}/api/v1/codex/features/{name}")),
        Some(&serde_json::json!({"enabled": enabled})),
        "Codex feature update",
    )
    .await
}

#[tauri::command]
async fn codex_account() -> Result<mcg_codex_app_server::CodexAccount, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/account")),
        None::<&()>,
        "Codex account",
    )
    .await
}

#[tauri::command]
async fn codex_rate_limits() -> Result<mcg_codex_app_server::CodexRateLimits, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/rate-limits")),
        None::<&()>,
        "Codex rate limits",
    )
    .await
}

#[tauri::command]
async fn codex_harness_status() -> Result<mcg_codex_app_server::CodexHarnessStatus, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/harness")),
        None::<&()>,
        "Codex harness status",
    )
    .await
}

#[tauri::command]
async fn codex_account_usage() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/account/usage")),
        None::<&()>,
        "Codex account usage",
    )
    .await
}

#[tauri::command]
async fn codex_stable_discovery(
    cwds: Vec<String>,
    force_reload: bool,
) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/codex/discovery")),
        Some(&serde_json::json!({ "cwds": cwds, "force_reload": force_reload })),
        "Codex stable discovery",
    )
    .await
}

#[tauri::command]
async fn codex_configuration_status() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/configuration")),
        None::<&()>,
        "Codex configuration status",
    )
    .await
}

#[tauri::command]
async fn codex_pending_requests() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/pending-requests")),
        None::<&()>,
        "Codex pending requests",
    )
    .await
}

#[tauri::command]
async fn codex_owned_threads() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/codex/owned-threads")),
        None::<&()>,
        "Codex owned threads",
    )
    .await
}

#[tauri::command]
async fn reconcile_owned_codex_thread(
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/reconcile"
        )),
        None::<&()>,
        "Codex thread reconciliation",
    )
    .await
}

#[tauri::command]
async fn fork_owned_codex_thread(conversation_id: String) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/fork"
        )),
        None::<&()>,
        "Codex thread fork",
    )
    .await
}

#[tauri::command]
async fn compact_owned_codex_thread(conversation_id: String) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/compact"
        )),
        None::<&()>,
        "Codex thread compaction",
    )
    .await
}

#[tauri::command]
async fn unsubscribe_owned_codex_thread(
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/unsubscribe"
        )),
        None::<&()>,
        "Codex thread unsubscribe",
    )
    .await
}

#[tauri::command]
async fn owned_codex_thread_history(conversation_id: String) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().get(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/history"
        )),
        None::<&()>,
        "Codex thread history",
    )
    .await
}

#[tauri::command]
async fn start_owned_codex_review(
    conversation_id: String,
    target: serde_json::Value,
    delivery: String,
) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/codex/owned-threads/{conversation_id}/review"
        )),
        Some(&serde_json::json!({ "target": target, "delivery": delivery })),
        "Codex review",
    )
    .await
}

#[tauri::command]
async fn exec_governed_codex_command(
    conversation_id: String,
    request: mcg_codex_app_server::CodexCommandExecRequest,
) -> Result<mcg_codex_app_server::CodexCommandExecResult, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/codex/command"
        )),
        Some(&request),
        "Governed Codex command",
    )
    .await
}

#[tauri::command]
async fn watch_conversation_project(conversation_id: String) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/codex/watch"
        )),
        None::<&()>,
        "Codex project watch",
    )
    .await
}

#[tauri::command]
async fn unwatch_conversation_project(
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().delete(format!(
            "{LOCAL_DAEMON}/api/v1/conversations/{conversation_id}/codex/watch"
        )),
        None::<&()>,
        "Codex project unwatch",
    )
    .await
}

#[tauri::command]
async fn start_codex_login() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/codex/account/login")),
        None::<&()>,
        "Codex login",
    )
    .await
}

#[tauri::command]
async fn cancel_codex_login(login_id: String) -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/codex/account/login/cancel")),
        Some(&serde_json::json!({ "login_id": login_id })),
        "Codex login cancellation",
    )
    .await
}

#[tauri::command]
async fn logout_codex_account() -> Result<serde_json::Value, String> {
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/codex/account/logout")),
        None::<&()>,
        "Codex logout",
    )
    .await
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
async fn governor_overview() -> Result<GovernorOverviewResponse, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/agents/overview")),
        None::<&()>,
        "Agent governor overview",
    )
    .await
}

#[tauri::command]
async fn governor_control_policy() -> Result<GovernorControlPolicy, String> {
    daemon_get("/api/v1/governor/policy", "Governor control policy").await
}

#[tauri::command]
async fn list_governor_proposals(
    organization_id: String,
) -> Result<GovernorProposalListResponse, String> {
    let organization_id = canonical_uuid(&organization_id, "Organization identity")?;
    daemon_get(
        &format!("/api/v1/governor/organizations/{organization_id}/proposals"),
        "Governor proposal listing",
    )
    .await
}

#[tauri::command]
async fn get_governor_proposal(proposal_id: String) -> Result<GovernorProposal, String> {
    let proposal_id = canonical_uuid(&proposal_id, "Governor proposal identity")?;
    daemon_get(
        &format!("/api/v1/governor/proposals/{proposal_id}"),
        "Governor proposal",
    )
    .await
}

#[tauri::command]
async fn decide_governor_proposal(
    proposal_id: String,
    approve: bool,
) -> Result<GovernorProposalMutation, String> {
    let proposal_id = canonical_uuid(&proposal_id, "Governor proposal identity")?;
    daemon_post(
        &format!("/api/v1/governor/proposals/{proposal_id}/owner-approval"),
        &OwnerGovernorApprovalRequest {
            expected: GovernorProposalState::AwaitingOwnerApproval,
            approve,
        },
        "Governor proposal owner decision",
    )
    .await
}

#[tauri::command]
async fn request_governor_implementation(
    proposal_id: String,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    let proposal_id = canonical_uuid(&proposal_id, "Governor proposal identity")?;
    let conversation_id = canonical_uuid(&conversation_id, "Conversation identity")?;
    daemon_post(
        &format!("/api/v1/governor/proposals/{proposal_id}/implementation"),
        &RequestGovernorImplementationRequest {
            conversation_id: conversation_id
                .to_string()
                .parse()
                .map_err(|_| "Conversation identity is invalid".to_owned())?,
        },
        "Governor implementation request",
    )
    .await
}

#[tauri::command]
async fn list_managed_worktrees() -> Result<ManagedWorktreeListResponse, String> {
    daemon_get("/api/v1/worktrees", "Managed worktree listing").await
}

#[tauri::command]
async fn prepare_cloud_diff_apply(
    project_id: String,
    task_id: String,
) -> Result<CloudDiffApply, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/codex-cloud-applies"),
        &PrepareCloudDiffApplyRequest {
            task_id,
            config_overrides: Vec::new(),
            enable_features: Vec::new(),
            disable_features: Vec::new(),
        },
        "Codex cloud diff preparation",
    )
    .await
}

#[tauri::command]
async fn list_cloud_diff_applies(project_id: String) -> Result<CloudDiffApplyListResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_get(
        &format!("/api/v1/projects/{project_id}/codex-cloud-applies"),
        "Codex cloud diff listing",
    )
    .await
}

#[tauri::command]
async fn apply_cloud_diff(
    apply_id: String,
    expected_revision: u64,
) -> Result<CloudDiffApply, String> {
    let apply_id = canonical_uuid(&apply_id, "Cloud diff apply identity")?;
    daemon_post(
        &format!("/api/v1/codex-cloud-applies/{apply_id}/apply"),
        &ApplyCloudDiffRequest { expected_revision },
        "Codex cloud diff integration",
    )
    .await
}

#[tauri::command]
async fn discard_cloud_diff_apply(
    apply_id: String,
    expected_revision: u64,
) -> Result<CloudDiffApply, String> {
    let apply_id = canonical_uuid(&apply_id, "Cloud diff apply identity")?;
    daemon_post(
        &format!("/api/v1/codex-cloud-applies/{apply_id}/discard"),
        &ApplyCloudDiffRequest { expected_revision },
        "Codex cloud diff discard",
    )
    .await
}

#[tauri::command]
async fn organization_assignments() -> Result<OrganizationAssignmentResponse, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/agents/assignments")),
        None::<&()>,
        "Organization assignments",
    )
    .await
}

#[tauri::command]
async fn set_global_organization_assignment(
    request: SetOrganizationAssignmentRequest,
) -> Result<OrganizationAssignmentResponse, String> {
    daemon_json(
        reqwest::Client::new().put(format!("{LOCAL_DAEMON}/api/v1/agents/assignments")),
        Some(&request),
        "Global organization assignment",
    )
    .await
}

#[tauri::command]
async fn set_project_organization_assignment(
    project_id: String,
    request: SetOrganizationAssignmentRequest,
) -> Result<OrganizationAssignmentResponse, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_json(
        reqwest::Client::new().put(format!(
            "{LOCAL_DAEMON}/api/v1/agents/assignments/projects/{project_id}"
        )),
        Some(&request),
        "Project organization assignment",
    )
    .await
}

#[tauri::command]
async fn preview_organization_revision(
    organization_id: String,
    request: PreviewOrganizationRevisionRequest,
) -> Result<OrganizationRevisionPreview, String> {
    let organization_id = canonical_uuid(&organization_id, "Organization identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organizations/{organization_id}/revisions/preview"
        )),
        Some(&request),
        "Organization revision preview",
    )
    .await
}

#[tauri::command]
async fn publish_organization_revision(
    organization_id: String,
    request: PublishOrganizationRevisionRequest,
) -> Result<OrganizationMutationResponse, String> {
    let organization_id = canonical_uuid(&organization_id, "Organization identity")?;
    daemon_json(
        reqwest::Client::new().post(format!(
            "{LOCAL_DAEMON}/api/v1/organizations/{organization_id}/revisions"
        )),
        Some(&request),
        "Organization revision publish",
    )
    .await
}

#[tauri::command]
async fn list_agent_runs() -> Result<AgentRunListResponse, String> {
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/runs?limit=50")),
        None::<&()>,
        "Agent run listing",
    )
    .await
}

#[tauri::command]
async fn get_agent_run(run_id: String) -> Result<AgentRunView, String> {
    let run_id = canonical_uuid(&run_id, "Agent run identity")?;
    daemon_json(
        reqwest::Client::new().get(format!("{LOCAL_DAEMON}/api/v1/runs/{run_id}")),
        None::<&()>,
        "Agent run detail",
    )
    .await
}

#[tauri::command]
async fn cancel_agent_run(run_id: String) -> Result<AgentRunView, String> {
    let run_id = canonical_uuid(&run_id, "Agent run identity")?;
    daemon_json(
        reqwest::Client::new().post(format!("{LOCAL_DAEMON}/api/v1/runs/{run_id}/transition")),
        Some(&TransitionAgentRunRequest {
            state: AgentRunState::Cancelled,
        }),
        "Agent run cancellation",
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
async fn list_codex_sites() -> Result<CodexSiteInventory, String> {
    site_get("/codex", "OpenAI Sites listing").await
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
async fn pick_mention_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    app.dialog()
        .file()
        .set_title("Attach files to this message")
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|_| "A selected file is not a local filesystem path".to_owned())
        })
        .collect()
}

#[tauri::command]
async fn pick_mention_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.dialog()
        .file()
        .set_title("Attach a read-only folder to this message")
        .blocking_pick_folder()
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|_| "The selected folder is not a local filesystem path".to_owned())
        })
        .transpose()
}

#[tauri::command]
async fn pick_writable_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.dialog()
        .file()
        .set_title("Allow MetaCog to write to an additional folder")
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
async fn initialize_project_git(project_id: String) -> Result<ProjectMutation, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/initialize-git"),
        &serde_json::json!({}),
        "Local Git initialization",
    )
    .await
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
async fn archive_project(project_id: String, archived: bool) -> Result<serde_json::Value, String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    daemon_post(
        &format!("/api/v1/projects/{project_id}/archive"),
        &serde_json::json!({ "archived": archived }),
        if archived {
            "Project archive"
        } else {
            "Project restore"
        },
    )
    .await
}

#[tauri::command]
async fn remove_project(project_id: String) -> Result<(), String> {
    let project_id = canonical_uuid(&project_id, "Project identity")?;
    let response = reqwest::Client::new()
        .delete(format!("{LOCAL_DAEMON}/api/v1/projects/{project_id}"))
        .bearer_auth(daemon_token()?)
        .send()
        .await
        .map_err(|error| format!("Project removal could not start: {error}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Project removal").await);
    }
    Ok(())
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

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes owned command arguments.
fn open_site_external_url(url: String) -> Result<(), String> {
    let parsed = validate_site_external_url(&url)?;
    webbrowser::open(parsed.as_str())
        .map_err(|error| format!("The system browser could not be opened: {error}"))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes owned command arguments.
async fn open_site_in_app_browser(
    app: tauri::AppHandle,
    tab_id: String,
    url: String,
    bounds: SiteBrowserBounds,
) -> Result<(), String> {
    let target = validate_in_app_browser_url(&url)?;
    let (position, size) = validate_site_browser_bounds(&bounds)?;
    let label = site_browser_label(&tab_id)?;

    if let Some(webview) = app.get_webview(&label) {
        webview.navigate(target).map_err(|error| {
            format!("The MetaCog browser could not navigate to the Site: {error}")
        })?;
        webview
            .set_position(position)
            .map_err(|error| format!("The MetaCog browser could not be positioned: {error}"))?;
        webview
            .set_size(size)
            .map_err(|error| format!("The MetaCog browser could not be resized: {error}"))?;
        webview
            .show()
            .map_err(|error| format!("The MetaCog browser could not be shown: {error}"))?;
        webview
            .set_focus()
            .map_err(|error| format!("The MetaCog browser could not be focused: {error}"))?;
        return Ok(());
    }

    let profile_directory = AppConfig::default()
        .data_directory
        .join("site-browser-profile");
    std::fs::create_dir_all(&profile_directory)
        .map_err(|error| format!("The MetaCog browser profile could not be prepared: {error}"))?;

    let parent = app
        .get_window("main")
        .ok_or_else(|| "The main MetaCog window is unavailable".to_string())?;
    let popup_app = app.clone();
    let popup_label = label.clone();
    let builder = tauri::webview::WebviewBuilder::new(label, tauri::WebviewUrl::External(target))
        .data_directory(profile_directory)
        .enable_clipboard_access()
        .on_navigation(site_browser_navigation_allowed)
        .on_new_window(move |candidate, _features| {
            if site_browser_navigation_allowed(&candidate) {
                if let Some(webview) = popup_app.get_webview(&popup_label) {
                    std::thread::spawn(move || {
                        let _ = webview.navigate(candidate);
                    });
                }
            }
            tauri::webview::NewWindowResponse::Deny
        });
    parent
        .add_child(builder, position, size)
        .map_err(|error| format!("The MetaCog browser could not be opened: {error}"))?;

    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes owned command arguments.
async fn update_site_in_app_browser(
    app: tauri::AppHandle,
    tab_id: String,
    bounds: Option<SiteBrowserBounds>,
    visible: bool,
) -> Result<(), String> {
    let label = site_browser_label(&tab_id)?;
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    if !visible {
        return webview
            .hide()
            .map_err(|error| format!("The MetaCog browser could not be hidden: {error}"));
    }
    let bounds = bounds.ok_or_else(|| "Visible MetaCog browser bounds are required".to_string())?;
    let (position, size) = validate_site_browser_bounds(&bounds)?;
    webview
        .set_position(position)
        .map_err(|error| format!("The MetaCog browser could not be positioned: {error}"))?;
    webview
        .set_size(size)
        .map_err(|error| format!("The MetaCog browser could not be resized: {error}"))?;
    webview
        .show()
        .map_err(|error| format!("The MetaCog browser could not be shown: {error}"))
}

#[tauri::command]
async fn close_site_in_app_browser(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = site_browser_label(&tab_id)?;
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    webview
        .close()
        .map_err(|error| format!("The MetaCog browser tab could not be closed: {error}"))
}

#[tauri::command]
async fn control_site_in_app_browser(
    app: tauri::AppHandle,
    tab_id: String,
    action: String,
) -> Result<(), String> {
    let label = site_browser_label(&tab_id)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "The MetaCog browser tab is unavailable".to_string())?;
    match action.as_str() {
        "back" => webview.eval("history.back()"),
        "forward" => webview.eval("history.forward()"),
        "reload" => webview.reload(),
        _ => return Err("Unsupported MetaCog browser action".to_string()),
    }
    .map_err(|error| format!("The MetaCog browser action failed: {error}"))
}

fn site_browser_label(tab_id: &str) -> Result<String, String> {
    if tab_id.is_empty()
        || tab_id.len() > 64
        || !tab_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("The MetaCog browser tab identity is invalid".to_string());
    }
    Ok(format!("site-browser-{tab_id}"))
}

#[derive(Deserialize)]
struct SiteBrowserBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn validate_site_browser_bounds(
    bounds: &SiteBrowserBounds,
) -> Result<(tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>), String> {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.x < 0.0
        || bounds.y < 0.0
        || !(1.0..=10_000.0).contains(&bounds.width)
        || !(1.0..=10_000.0).contains(&bounds.height)
    {
        return Err("The MetaCog browser panel bounds are invalid".into());
    }
    Ok((
        tauri::LogicalPosition::new(bounds.x, bounds.y),
        tauri::LogicalSize::new(bounds.width, bounds.height),
    ))
}

fn validate_external_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| format!("The verification URL is invalid: {error}"))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("auth.openai.com") {
        return Err("Only the official OpenAI authorization site may be opened".into());
    }
    Ok(parsed)
}

fn validate_site_external_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("The Site URL is invalid: {error}"))?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.host_str().is_none()
    {
        return Err("Only credential-free HTTPS Site URLs may be opened".into());
    }
    Ok(parsed)
}

fn validate_in_app_browser_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("The preview URL is invalid: {error}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Credential-bearing preview URLs may not be opened".into());
    }
    let remote_https = parsed.scheme() == "https" && parsed.host_str().is_some();
    let managed_loopback = parsed.scheme() == "http"
        && parsed.host_str() == Some("127.0.0.1")
        && parsed.port().is_some();
    if !remote_https && !managed_loopback {
        return Err(
            "Only credential-free HTTPS Sites or managed 127.0.0.1 previews may be opened".into(),
        );
    }
    Ok(parsed)
}

fn site_browser_navigation_allowed(url: &reqwest::Url) -> bool {
    validate_in_app_browser_url(url.as_str()).is_ok()
}

fn canonical_directory(value: &str) -> Result<PathBuf, String> {
    let requested = Path::new(value);
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("The workspace directory could not be opened: {error}"))?;
    if !canonical.is_dir() {
        return Err("The workspace location is not a directory".into());
    }
    Ok(canonical)
}

#[cfg(windows)]
fn command_available(command: &str) -> bool {
    std::process::Command::new("where.exe")
        .arg(command)
        .creation_flags(0x0800_0000)
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(not(windows))]
fn command_available(command: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", "command -v -- \"$1\" >/dev/null 2>&1", "sh", command])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[tauri::command]
fn list_open_in_applications() -> Vec<OpenInApplication> {
    #[cfg(windows)]
    {
        vec![
            OpenInApplication {
                id: "vscode",
                label: "VS Code",
                available: command_available("code.cmd") || command_available("code.exe"),
            },
            OpenInApplication {
                id: "visual_studio",
                label: "Visual Studio",
                available: command_available("devenv.exe"),
            },
            OpenInApplication {
                id: "cursor",
                label: "Cursor",
                available: command_available("cursor.cmd") || command_available("cursor.exe"),
            },
            OpenInApplication {
                id: "file_explorer",
                label: "File Explorer",
                available: true,
            },
            OpenInApplication {
                id: "terminal",
                label: "Terminal",
                available: command_available("wt.exe") || command_available("powershell.exe"),
            },
            OpenInApplication {
                id: "git_bash",
                label: "Git Bash",
                available: Path::new(r"C:\Program Files\Git\git-bash.exe").is_file(),
            },
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            OpenInApplication {
                id: "vscode",
                label: "VS Code",
                available: command_available("code"),
            },
            OpenInApplication {
                id: "cursor",
                label: "Cursor",
                available: command_available("cursor"),
            },
            OpenInApplication {
                id: "file_explorer",
                label: "Finder",
                available: true,
            },
            OpenInApplication {
                id: "terminal",
                label: "Terminal",
                available: true,
            },
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            OpenInApplication {
                id: "vscode",
                label: "VS Code",
                available: command_available("code"),
            },
            OpenInApplication {
                id: "cursor",
                label: "Cursor",
                available: command_available("cursor"),
            },
            OpenInApplication {
                id: "file_explorer",
                label: "File Manager",
                available: command_available("xdg-open"),
            },
            OpenInApplication {
                id: "terminal",
                label: "Terminal",
                available: command_available("x-terminal-emulator"),
            },
        ]
    }
}

#[tauri::command]
fn open_workspace_in(application: String, working_directory: String) -> Result<(), String> {
    let root = canonical_directory(&working_directory)?;
    #[cfg(windows)]
    if application == "terminal"
        && !command_available("wt.exe")
        && command_available("powershell.exe")
    {
        let mut command = std::process::Command::new("powershell.exe");
        command
            .args(["-NoExit", "-Command", "Set-Location -LiteralPath $args[0]"])
            .arg(&root)
            .current_dir(&root);
        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("The workspace could not be opened: {error}"));
    }
    #[cfg(windows)]
    if application == "git_bash" && Path::new(r"C:\Program Files\Git\git-bash.exe").is_file() {
        let mut command = std::process::Command::new(r"C:\Program Files\Git\git-bash.exe");
        command
            .arg(format!("--cd={}", root.to_string_lossy()))
            .current_dir(&root);
        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("The workspace could not be opened: {error}"));
    }
    #[cfg(windows)]
    let mut command = match application.as_str() {
        "vscode" if command_available("code.cmd") => std::process::Command::new("code.cmd"),
        "vscode" if command_available("code.exe") => std::process::Command::new("code.exe"),
        "visual_studio" if command_available("devenv.exe") => {
            std::process::Command::new("devenv.exe")
        }
        "cursor" if command_available("cursor.cmd") => std::process::Command::new("cursor.cmd"),
        "cursor" if command_available("cursor.exe") => std::process::Command::new("cursor.exe"),
        "file_explorer" => std::process::Command::new("explorer.exe"),
        "terminal" if command_available("wt.exe") => {
            let mut command = std::process::Command::new("wt.exe");
            command.arg("-d");
            command
        }
        _ => return Err("That application is not installed or is not available on PATH".into()),
    };
    #[cfg(target_os = "macos")]
    let mut command = match application.as_str() {
        "vscode" if command_available("code") => std::process::Command::new("code"),
        "cursor" if command_available("cursor") => std::process::Command::new("cursor"),
        "file_explorer" => std::process::Command::new("open"),
        "terminal" => {
            let mut command = std::process::Command::new("open");
            command.args(["-a", "Terminal"]);
            command
        }
        _ => return Err("That application is not installed or is not available on PATH".into()),
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = match application.as_str() {
        "vscode" if command_available("code") => std::process::Command::new("code"),
        "cursor" if command_available("cursor") => std::process::Command::new("cursor"),
        "file_explorer" if command_available("xdg-open") => std::process::Command::new("xdg-open"),
        "terminal" if command_available("x-terminal-emulator") => {
            std::process::Command::new("x-terminal-emulator")
        }
        _ => return Err("That application is not installed or is not available on PATH".into()),
    };
    command.arg(&root).current_dir(&root);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("The workspace could not be opened: {error}"))
}

#[tauri::command]
async fn run_terminal_command(
    request: TerminalCommandRequest,
) -> Result<TerminalCommandResult, String> {
    let mut root = canonical_directory(&request.working_directory)?;
    let input = request.command.trim();
    if input.is_empty() {
        return Ok(TerminalCommandResult {
            working_directory: root.to_string_lossy().into_owned(),
            output: String::new(),
            exit_code: 0,
        });
    }
    if input.len() > 4096 {
        return Err("Terminal commands are limited to 4096 characters".into());
    }
    if input.eq_ignore_ascii_case("cd") {
        return Ok(TerminalCommandResult {
            working_directory: root.to_string_lossy().into_owned(),
            output: root.to_string_lossy().into_owned(),
            exit_code: 0,
        });
    }
    if let Some(destination) = input.strip_prefix("cd ") {
        let destination = destination.trim().trim_matches(['\"', '\'']);
        let requested = Path::new(destination);
        let destination = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            root.join(requested)
        };
        root = canonical_directory(
            destination
                .to_str()
                .ok_or("The destination path is not valid Unicode")?,
        )?;
        return Ok(TerminalCommandResult {
            working_directory: root.to_string_lossy().into_owned(),
            output: String::new(),
            exit_code: 0,
        });
    }

    #[cfg(windows)]
    let mut command = {
        let mut command = tokio::process::Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            input,
        ]);
        command.creation_flags(0x0800_0000);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = tokio::process::Command::new("sh");
        command.args(["-lc", input]);
        command
    };
    command.current_dir(&root).kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(120), command.output())
        .await
        .map_err(|_| "The terminal command timed out after 2 minutes".to_string())?
        .map_err(|error| format!("The terminal command could not run: {error}"))?;
    let mut combined = output.stdout;
    combined.extend_from_slice(&output.stderr);
    const MAX_OUTPUT_BYTES: usize = 1_048_576;
    if combined.len() > MAX_OUTPUT_BYTES {
        combined.truncate(MAX_OUTPUT_BYTES);
        combined.extend_from_slice(b"\n[output truncated at 1 MiB]\n");
    }
    Ok(TerminalCommandResult {
        working_directory: root.to_string_lossy().into_owned(),
        output: String::from_utf8_lossy(&combined).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
    })
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
    let shutdown_service_on_exit = service_status.ready;
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
            github_status,
            start_github_login,
            complete_github_login,
            disconnect_github,
            list_github_repositories,
            create_github_repository,
            link_github_repository,
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
            list_codex_sites,
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
            open_site_in_app_browser,
            update_site_in_app_browser,
            close_site_in_app_browser,
            control_site_in_app_browser,
            open_site_external_url,
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
            list_conversations,
            create_conversation,
            get_conversation,
            update_conversation,
            archive_conversation,
            send_conversation_message,
            search_project_mentions,
            update_conversation_mode,
            update_conversation_goal,
            clear_conversation_goal,
            answer_conversation_input,
            get_conversation_team,
            review_conversation_delegation,
            cancel_conversation_delegation,
            retry_conversation_delegation,
            apply_conversation_delegation,
            discard_conversation_delegation,
            start_conversation_preview,
            stop_conversation_preview,
            interrupt_conversation_turn,
            steer_conversation_turn,
            codex_models,
            codex_features,
            codex_permission_profiles,
            set_codex_feature,
            codex_account,
            codex_rate_limits,
            codex_harness_status,
            codex_account_usage,
            codex_stable_discovery,
            codex_configuration_status,
            codex_pending_requests,
            codex_owned_threads,
            reconcile_owned_codex_thread,
            fork_owned_codex_thread,
            compact_owned_codex_thread,
            unsubscribe_owned_codex_thread,
            owned_codex_thread_history,
            start_owned_codex_review,
            exec_governed_codex_command,
            watch_conversation_project,
            unwatch_conversation_project,
            start_codex_login,
            cancel_codex_login,
            logout_codex_account,
            governor_overview,
            governor_control_policy,
            list_governor_proposals,
            get_governor_proposal,
            decide_governor_proposal,
            request_governor_implementation,
            list_managed_worktrees,
            prepare_cloud_diff_apply,
            list_cloud_diff_applies,
            apply_cloud_diff,
            discard_cloud_diff_apply,
            organization_assignments,
            set_global_organization_assignment,
            set_project_organization_assignment,
            preview_organization_revision,
            publish_organization_revision,
            list_agent_runs,
            get_agent_run,
            cancel_agent_run,
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
            pick_mention_files,
            pick_mention_folder,
            pick_writable_folder,
            discover_project,
            initialize_project_git,
            list_projects,
            archive_project,
            remove_project,
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
            open_external_url,
            list_open_in_applications,
            open_workspace_in,
            run_terminal_command
        ])
        .build(tauri::generate_context!())
        .expect("error while building MetaCog desktop application")
        .run(move |_app, event| {
            if shutdown_service_on_exit && matches!(event, tauri::RunEvent::Exit) {
                let _ = service_bootstrap::shutdown_local_service(LOCAL_DAEMON);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        SiteBrowserBounds, TerminalCommandRequest, bounded_api_error_code, canonical_directory,
        canonical_uuid, run_terminal_command, site_browser_label, site_browser_navigation_allowed,
        validate_external_url, validate_in_app_browser_url, validate_local_daemon,
        validate_site_browser_bounds, validate_site_external_url,
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
    fn site_launcher_allows_credential_free_https_only() {
        assert!(validate_site_external_url("https://springfield-it-support.example/").is_ok());
        assert!(validate_site_external_url("http://example.com/").is_err());
        assert!(validate_site_external_url("https://user:secret@example.com/").is_err());
        assert!(validate_site_external_url("file:///C:/secret").is_err());
    }

    #[test]
    fn in_app_browser_allows_only_https_or_managed_loopback_previews() {
        assert!(validate_in_app_browser_url("https://springfield.chatgpt.site/").is_ok());
        assert!(validate_in_app_browser_url("http://127.0.0.1:61410/").is_ok());
        assert!(validate_in_app_browser_url("http://localhost:61410/").is_err());
        assert!(validate_in_app_browser_url("http://example.com:61410/").is_err());
        assert!(validate_in_app_browser_url("http://127.0.0.1/").is_err());
        assert!(validate_in_app_browser_url("http://user:secret@127.0.0.1:61410/").is_err());
    }

    #[test]
    fn site_browser_allows_https_auth_redirects_but_rejects_unsafe_navigation() {
        assert!(site_browser_navigation_allowed(
            &reqwest::Url::parse("https://auth.openai.com/authorize").expect("OpenAI auth URL")
        ));
        assert!(site_browser_navigation_allowed(
            &reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
                .expect("Google auth URL")
        ));
        assert!(!site_browser_navigation_allowed(
            &reqwest::Url::parse("http://example.com/").expect("HTTP URL")
        ));
        assert!(site_browser_navigation_allowed(
            &reqwest::Url::parse("http://127.0.0.1:61410/").expect("loopback URL")
        ));
        assert!(!site_browser_navigation_allowed(
            &reqwest::Url::parse("https://user:secret@example.com/").expect("credential URL")
        ));
        assert!(!site_browser_navigation_allowed(
            &reqwest::Url::parse("file:///C:/secret").expect("file URL")
        ));
    }

    #[test]
    fn site_browser_tab_labels_are_scoped_and_bounded() {
        assert_eq!(
            site_browser_label("9ea1e248-0fa0-4b4f-a210-584454d4fcdd")
                .expect("valid browser tab identity"),
            "site-browser-9ea1e248-0fa0-4b4f-a210-584454d4fcdd"
        );
        assert!(site_browser_label("").is_err());
        assert!(site_browser_label("../../main").is_err());
        assert!(site_browser_label(&"a".repeat(65)).is_err());
    }

    #[test]
    fn site_browser_bounds_are_finite_positive_and_bounded() {
        assert!(
            validate_site_browser_bounds(&SiteBrowserBounds {
                x: 800.0,
                y: 160.0,
                width: 340.0,
                height: 520.0,
            })
            .is_ok()
        );
        assert!(
            validate_site_browser_bounds(&SiteBrowserBounds {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 520.0,
            })
            .is_err()
        );
        assert!(
            validate_site_browser_bounds(&SiteBrowserBounds {
                x: f64::NAN,
                y: 0.0,
                width: 340.0,
                height: 520.0,
            })
            .is_err()
        );
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

    #[tokio::test]
    async fn terminal_changes_only_to_an_existing_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let child = workspace.path().join("child");
        std::fs::create_dir(&child).expect("child directory");
        let result = run_terminal_command(TerminalCommandRequest {
            working_directory: workspace.path().to_string_lossy().into_owned(),
            command: "cd child".into(),
        })
        .await
        .expect("change directory");
        assert_eq!(
            canonical_directory(&result.working_directory).expect("canonical result"),
            child.canonicalize().expect("canonical child")
        );

        assert!(
            run_terminal_command(TerminalCommandRequest {
                working_directory: workspace.path().to_string_lossy().into_owned(),
                command: "cd missing".into(),
            })
            .await
            .is_err()
        );
    }
}
