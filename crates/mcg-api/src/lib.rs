//! Versioned authenticated loopback HTTP and streaming surface.

mod mcp;
mod provider_proxy;

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    future::Future,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use chrono::Utc;
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentRun, AgentRunState, AgentTurnError, EvaluationReport,
    ExecutivePlanner, OrganizationGenome, OrganizationalMetrics, PlannedWork, PlanningRequest,
    RoleKind, SuccessCondition, TaskComplexity,
};
use mcg_domain::{
    ApprovalId, MemoryClaimId, OrganizationId, OrganizationRevisionId, ProjectId, RiskLevel, RunId,
    TaskBudget, TaskEnvelope, TaskId, TaskState, ThreadId, WorkItemId,
};
use mcg_events::EventEnvelope;
use mcg_mcp::{McpConnection, McpCredential, McpError, McpToolCatalog};
use mcg_projects::{
    ClaimProvenance, ContextAssembly, ContextItem, MemoryClaim, MemoryLifecycleAction, ProjectModel,
};
use mcg_providers::{
    ModelProbeReport, ProviderAuthMode, ProviderConnection, ProviderCredential,
    ProviderModelCatalog,
};
use mcg_runtime::{
    AgentDispatchSpec, AgentRunView, AgentTurnCommit, AppRuntime, ConsumerEvaluationProgress,
    McpMutation, MemoryClaimMutation, OAuthLoginStart, OpenAiOAuthMethod, ProjectMutation,
    ProviderMutation, RuntimeError, RuntimeStatus,
};
use mcg_tools::{ApprovalRequest, ApprovalResolution, ApprovalState, ToolAuditRecord};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tower_http::trace::TraceLayer;
use zeroize::Zeroize;

#[derive(Clone)]
struct ApiState {
    runtime: Arc<AppRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub goal: String,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub success_conditions: Vec<String>,
    #[serde(default)]
    pub risk: Option<RiskLevel>,
    #[serde(default)]
    pub budget: Option<TaskBudget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionTaskRequest {
    pub state: TaskState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskMutationResponse {
    pub task: TaskEnvelope,
    pub event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationMutationResponse {
    pub report: EvaluationReport,
    pub event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEventsResponse {
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalListResponse {
    pub approvals: Vec<ApprovalRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveApprovalRequest {
    pub state: ApprovalState,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolAuditResponse {
    pub records: Vec<ToolAuditRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverProjectRequest {
    pub root: String,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectHistoryResponse {
    pub snapshots: Vec<ProjectModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMemoryClaimRequest {
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub confidence: f64,
    pub provenance: ClaimProvenance,
    #[serde(default)]
    pub fresh_until: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectMemoryClaimRequest {
    pub value: serde_json::Value,
    pub confidence: f64,
    pub provenance: ClaimProvenance,
    #[serde(default)]
    pub fresh_until: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionMemoryClaimRequest {
    pub action: MemoryLifecycleAction,
    pub provenance: ClaimProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryClaimListResponse {
    pub claims: Vec<MemoryClaim>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryClaimHistoryResponse {
    pub claims: Vec<MemoryClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedMemoryResponse {
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssembleContextRequest {
    #[serde(default)]
    pub items: Vec<ContextItem>,
    pub token_budget: u32,
    #[serde(default)]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateOrganizationRequest {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationMutationResponse {
    pub organization: OrganizationGenome,
    pub event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizationListResponse {
    pub organizations: Vec<OrganizationGenome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanAgentRunRequest {
    pub organization_revision_id: OrganizationRevisionId,
    pub complexity: TaskComplexity,
    pub work: Vec<PlanWorkRequest>,
    #[serde(default)]
    pub require_independent_evaluation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanWorkRequest {
    pub key: String,
    pub goal: String,
    pub role: RoleKind,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub success_conditions: Vec<SuccessCondition>,
    #[serde(default)]
    pub risk_notes: Vec<String>,
    #[serde(default = "default_budget_weight")]
    pub budget_weight: u32,
    #[serde(default = "default_estimated_useful_seconds")]
    pub estimated_useful_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionAgentRunRequest {
    pub state: AgentRunState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteAgentWorkRequest {
    pub project_id: ProjectId,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default = "default_context_token_budget")]
    pub context_token_budget: u32,
    #[serde(default)]
    pub requested_tools: Vec<String>,
    #[serde(default = "default_estimated_output_tokens")]
    pub estimated_output_tokens: u64,
    #[serde(default = "default_max_model_calls")]
    pub max_model_calls: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchTaskRequest {
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default = "default_context_token_budget")]
    pub context_token_budget: u32,
    #[serde(default)]
    pub requested_tools: Vec<String>,
    #[serde(default = "default_estimated_output_tokens")]
    pub estimated_output_tokens: u64,
    #[serde(default = "default_max_model_calls")]
    pub max_model_calls: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaunchTaskResponse {
    pub task: TaskEnvelope,
    pub run: AgentRunView,
    pub replayed: bool,
    pub execution: LaunchExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum LaunchExecution {
    NotStarted { detail: String },
    AwaitingApproval { detail: String },
    AwaitingEvaluation { detail: String },
    Blocked { detail: String },
    Completed { detail: String },
    Failed { detail: String },
}

#[derive(Deserialize)]
pub struct ConfigureProviderRequest {
    pub connection: ProviderConnection,
    #[serde(default)]
    pub credential: Option<String>,
}

impl Drop for ConfigureProviderRequest {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderListItem {
    pub connection: ProviderConnection,
    pub catalog: Option<ProviderModelCatalog>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderListResponse {
    pub providers: Vec<ProviderListItem>,
}

#[derive(Deserialize)]
pub struct ConfigureMcpRequest {
    pub connection: McpConnection,
    #[serde(default)]
    pub credential: Option<String>,
}

impl Drop for ConfigureMcpRequest {
    fn drop(&mut self) {
        if let Some(credential) = &mut self.credential {
            credential.zeroize();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpListItem {
    pub connection: McpConnection,
    pub catalog: Option<McpToolCatalog>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpListResponse {
    pub connections: Vec<McpListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartOpenAiOAuthRequest {
    pub connection_id: String,
    pub display_name: String,
    pub method: OpenAiOAuthMethod,
}

const fn default_budget_weight() -> u32 {
    1
}

const fn default_estimated_useful_seconds() -> u64 {
    60
}

const fn default_context_token_budget() -> u32 {
    16_384
}

const fn default_estimated_output_tokens() -> u64 {
    4_096
}

const fn default_max_model_calls() -> u16 {
    8
}

#[derive(Debug, Default, Deserialize)]
struct ApprovalListQuery {
    state: Option<ApprovalState>,
}

#[derive(Debug, Default, Deserialize)]
struct EventStreamQuery {
    after: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct MemoryListQuery {
    namespace: Option<String>,
    #[serde(default = "default_true")]
    fresh_only: bool,
}

#[derive(Debug, Deserialize)]
struct MemoryHistoryQuery {
    namespace: String,
    key: String,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
    message: String,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: message.into(),
        }
    }

    fn unprocessable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "provider_authentication",
            message: message.into(),
        }
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "provider_error",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "runtime_error",
            message: message.into(),
        }
    }
}

impl From<RuntimeError> for ApiError {
    fn from(error: RuntimeError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "runtime_error",
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

#[allow(clippy::too_many_lines)] // Keeps the complete versioned route map visible in one place.
pub fn router(runtime: Arc<AppRuntime>) -> Router {
    let state = ApiState { runtime };
    let protected = Router::new()
        .route("/api/v1/system/status", get(status))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/tasks/{task_id}",
            get(get_task).post(transition_task),
        )
        .route("/api/v1/tasks/{task_id}/events", get(task_events))
        .route("/api/v1/tasks/{task_id}/tool-audit", get(task_tool_audit))
        .route("/api/v1/tasks/{task_id}/runs", post(plan_agent_run))
        .route("/api/v1/tasks/{task_id}/launch", post(launch_task))
        .route("/api/v1/tasks/{task_id}/retry", post(retry_task))
        .route("/api/v1/tasks/{task_id}/resume", post(resume_task))
        .route(
            "/api/v1/organizations",
            get(list_organizations).post(create_default_organization),
        )
        .route(
            "/api/v1/organizations/{organization_id}",
            get(get_organization),
        )
        .route("/api/v1/runs/{run_id}", get(get_agent_run))
        .route("/api/v1/runs/{run_id}/evaluations", post(record_evaluation))
        .route(
            "/api/v1/runs/{run_id}/work/{work_item_id}/execute",
            post(execute_agent_work),
        )
        .route(
            "/api/v1/runs/{run_id}/transition",
            post(transition_agent_run),
        )
        .route(
            "/api/v1/providers",
            get(list_providers).post(configure_provider),
        )
        .route(
            "/api/v1/providers/{connection_id}/models",
            get(get_provider_models),
        )
        .route(
            "/api/v1/providers/{connection_id}/discover",
            post(discover_provider),
        )
        .route(
            "/api/v1/providers/{connection_id}/oauth/refresh",
            post(refresh_openai_oauth),
        )
        .route(
            "/api/v1/providers/{connection_id}/models/{model_id}/probe",
            post(probe_provider_model),
        )
        .route(
            "/api/v1/mcp/connections",
            get(list_mcp_connections).post(configure_mcp_connection),
        )
        .route(
            "/api/v1/mcp/connections/{connection_id}/tools",
            get(get_mcp_tools),
        )
        .route(
            "/api/v1/mcp/connections/{connection_id}/discover",
            post(discover_mcp_connection),
        )
        .route("/api/v1/oauth/openai/start", post(start_openai_oauth))
        .route(
            "/api/v1/oauth/openai/{session_id}/complete",
            post(complete_openai_oauth),
        )
        .route(
            "/api/v1/oauth/openai/{session_id}/cancel",
            post(cancel_openai_oauth),
        )
        .route(
            "/api/v1/projects",
            get(list_projects).post(discover_project),
        )
        .route("/api/v1/projects/{project_id}", get(get_project))
        .route(
            "/api/v1/projects/{project_id}/refresh",
            post(refresh_project),
        )
        .route(
            "/api/v1/projects/{project_id}/history",
            get(project_history),
        )
        .route(
            "/api/v1/projects/{project_id}/context",
            post(assemble_project_context),
        )
        .route(
            "/api/v1/projects/{project_id}/memory",
            get(list_memory_claims).post(create_memory_claim),
        )
        .route(
            "/api/v1/projects/{project_id}/memory/history",
            get(memory_claim_history),
        )
        .route(
            "/api/v1/projects/{project_id}/memory/managed",
            get(managed_memory),
        )
        .route(
            "/api/v1/projects/{project_id}/memory/{claim_id}/correct",
            post(correct_memory_claim),
        )
        .route(
            "/api/v1/projects/{project_id}/memory/{claim_id}/transition",
            post(transition_memory_claim),
        )
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/approvals/{approval_id}", get(get_approval))
        .route(
            "/api/v1/approvals/{approval_id}/resolve",
            post(resolve_approval),
        )
        .route("/api/v1/events/stream", get(stream_events))
        .route("/mcp", post(mcp::handle))
        .route("/v1/models", get(provider_proxy::models))
        .route("/v1/responses", post(provider_proxy::responses))
        .route(
            "/v1/chat/completions",
            post(provider_proxy::chat_completions),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/v1/system/health", get(health))
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Binds the loopback API and serves until an operating-system shutdown signal.
///
/// # Errors
///
/// Rejects non-loopback addresses and returns listener/server I/O failures.
pub async fn serve(address: SocketAddr, runtime: Arc<AppRuntime>) -> Result<(), std::io::Error> {
    if !address.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MCG local API must bind to a loopback address",
        ));
    }
    let listener = TcpListener::bind(address).await?;
    serve_listener(listener, runtime, async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for shutdown signal");
        }
    })
    .await
}

/// Serves an already-bound loopback listener until the supplied shutdown future resolves.
///
/// # Errors
///
/// Rejects non-loopback listeners and returns server I/O failures.
pub async fn serve_listener<F>(
    listener: TcpListener,
    runtime: Arc<AppRuntime>,
    shutdown: F,
) -> Result<(), std::io::Error>
where
    F: Future<Output = ()> + Send + 'static,
{
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MCG local API must bind to a loopback address",
        ));
    }
    axum::serve(listener, router(runtime))
        .with_graceful_shutdown(shutdown)
        .await
}

async fn require_auth(
    State(state): State<ApiState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() == "/mcp"
        && let Some(response) = mcp::origin_rejection(&headers)
    {
        return response;
    }
    let candidate = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if candidate.is_some_and(|token| state.runtime.auth().verifies(token)) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "unauthorized",
                message: "a valid local bearer token is required".to_owned(),
            }),
        )
            .into_response()
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "mcg-daemon",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status(State(state): State<ApiState>) -> Json<RuntimeStatus> {
    Json(state.runtime.status())
}

async fn create_task(
    State(state): State<ApiState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskMutationResponse>), ApiError> {
    if request.goal.trim().is_empty() {
        return Err(ApiError::bad_request("task goal cannot be empty"));
    }
    let mut task = TaskEnvelope::new(request.goal.trim());
    task.project_id = request.project_id;
    task.thread_id = request.thread_id;
    task.constraints = request.constraints;
    task.success_conditions = request.success_conditions;
    if let Some(risk) = request.risk {
        task.risk = risk;
    }
    if let Some(budget) = request.budget {
        task.budget = budget;
    }
    let event = state.runtime.create_task(&task)?;
    Ok((
        StatusCode::CREATED,
        Json(TaskMutationResponse { task, event }),
    ))
}

async fn list_tasks(State(state): State<ApiState>) -> Result<Json<TaskListResponse>, ApiError> {
    Ok(Json(TaskListResponse {
        tasks: state.runtime.tasks().map_err(map_runtime_error)?,
    }))
}

async fn get_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskEnvelope>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    state
        .runtime
        .task(task_id)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))
}

async fn transition_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(request): Json<TransitionTaskRequest>,
) -> Result<Json<TaskEnvelope>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    state
        .runtime
        .transition_task(task_id, request.state)
        .map(Json)
        .map_err(|error| {
            if error.is_not_found() {
                ApiError::not_found(error.to_string())
            } else if error.is_invalid_request() {
                ApiError::bad_request(error.to_string())
            } else {
                ApiError::from(error)
            }
        })
}

async fn task_events(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskEventsResponse>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    if state.runtime.task(task_id)?.is_none() {
        return Err(ApiError::not_found(format!("task {task_id} was not found")));
    }
    Ok(Json(TaskEventsResponse {
        events: state.runtime.task_events(task_id)?,
    }))
}

async fn task_tool_audit(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ToolAuditResponse>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    if state.runtime.task(task_id)?.is_none() {
        return Err(ApiError::not_found(format!("task {task_id} was not found")));
    }
    Ok(Json(ToolAuditResponse {
        records: state.runtime.tool_audit(task_id)?,
    }))
}

async fn create_default_organization(
    State(state): State<ApiState>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationMutationResponse>), ApiError> {
    let mut organization = OrganizationGenome::consumer_default(Utc::now());
    if let Some(name) = request.name {
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err(ApiError::bad_request(
                "organization name must contain 1-100 characters",
            ));
        }
        organization.name = name.trim().into();
    }
    let event = state
        .runtime
        .record_organization_revision(&organization)
        .map_err(map_runtime_error)?;
    Ok((
        StatusCode::CREATED,
        Json(OrganizationMutationResponse {
            organization,
            event,
        }),
    ))
}

async fn list_organizations(
    State(state): State<ApiState>,
) -> Result<Json<OrganizationListResponse>, ApiError> {
    Ok(Json(OrganizationListResponse {
        organizations: state.runtime.organizations().map_err(map_runtime_error)?,
    }))
}

async fn get_organization(
    State(state): State<ApiState>,
    Path(organization_id): Path<String>,
) -> Result<Json<OrganizationGenome>, ApiError> {
    let organization_id = parse_organization_id(&organization_id)?;
    state
        .runtime
        .organization(organization_id)
        .map_err(map_runtime_error)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("organization {organization_id} was not found")))
}

#[allow(clippy::too_many_lines)] // Stable-key dependency conversion is one validation boundary.
async fn plan_agent_run(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(request): Json<PlanAgentRunRequest>,
) -> Result<(StatusCode, Json<AgentRunView>), ApiError> {
    let task_id = parse_task_id(&task_id)?;
    let task = state
        .runtime
        .task(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))?;
    let organization = state
        .runtime
        .organization_revision(request.organization_revision_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| {
            ApiError::not_found(format!(
                "organization revision {} was not found",
                request.organization_revision_id
            ))
        })?;
    if request.work.is_empty() {
        return Err(ApiError::bad_request("run plan must contain work"));
    }
    let keys = request
        .work
        .iter()
        .map(|work| work.key.trim())
        .collect::<BTreeSet<_>>();
    if keys.len() != request.work.len()
        || keys.iter().any(|key| key.is_empty())
        || request.work.iter().any(|work| {
            work.dependencies
                .iter()
                .any(|key| !keys.contains(key.trim()))
        })
    {
        return Err(ApiError::bad_request(
            "work keys must be unique/non-empty and dependencies must reference known keys",
        ));
    }
    let identities = request
        .work
        .iter()
        .map(|work| (work.key.trim().to_owned(), WorkItemId::new()))
        .collect::<BTreeMap<_, _>>();
    let work = request
        .work
        .into_iter()
        .map(|work| PlannedWork {
            id: identities[work.key.trim()],
            goal: work.goal,
            role: work.role,
            dependencies: work
                .dependencies
                .iter()
                .map(|key| identities[key.trim()])
                .collect(),
            success_conditions: work.success_conditions,
            risk_notes: work.risk_notes,
            budget_weight: work.budget_weight,
            estimated_useful_seconds: work.estimated_useful_seconds,
        })
        .collect();
    let now = Utc::now();
    let run_id = RunId::new();
    let planning = PlanningRequest {
        run_id,
        task: task.clone(),
        complexity: request.complexity,
        work,
        require_independent_evaluation: request.require_independent_evaluation,
    };
    let plan = ExecutivePlanner::default()
        .plan(&organization, &planning, now)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let run = AgentRun {
        schema_version: AGENT_RUN_SCHEMA_VERSION,
        id: run_id,
        task_id,
        organization_revision_id: organization.revision_id,
        state: AgentRunState::Planned,
        metrics: OrganizationalMetrics::default(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    state
        .runtime
        .create_agent_run(&run, &plan)
        .map_err(map_runtime_error)?;
    let view = state
        .runtime
        .agent_run_view(run_id)
        .map_err(map_runtime_error)?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// Composes the consumer Start action into one durable plan/run reservation and
/// bounded autonomous execution. The durable launch is idempotent by task id;
/// replaying Start observes the original run without issuing another model call.
#[allow(clippy::too_many_lines)] // Keep the complete consumer launch composition visible together.
async fn launch_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(request): Json<LaunchTaskRequest>,
) -> Result<Json<LaunchTaskResponse>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    let task = state
        .runtime
        .task(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))?;
    let project_id = task
        .project_id
        .ok_or_else(|| ApiError::bad_request("consumer task launch requires a selected project"))?;
    let project = state
        .runtime
        .project(project_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::not_found(format!("project {project_id} was not found")))?;
    let now = Utc::now();
    let organization = OrganizationGenome::consumer_default(now);
    let conditions = consumer_success_conditions(&task, &project);
    let run_id = RunId::new();
    let planning = PlanningRequest {
        run_id,
        task: task.clone(),
        complexity: TaskComplexity::Compound,
        work: vec![PlannedWork {
            id: WorkItemId::new(),
            goal: task.goal.clone(),
            role: RoleKind::Coder,
            dependencies: Vec::new(),
            success_conditions: conditions,
            risk_notes: task.constraints.clone(),
            budget_weight: 4,
            estimated_useful_seconds: task.budget.max_duration_seconds.unwrap_or(900),
        }],
        require_independent_evaluation: true,
    };
    let plan = ExecutivePlanner::default()
        .plan(&organization, &planning, now)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let run = AgentRun {
        schema_version: AGENT_RUN_SCHEMA_VERSION,
        id: run_id,
        task_id,
        organization_revision_id: organization.revision_id,
        state: AgentRunState::Planned,
        metrics: OrganizationalMetrics::default(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let launched = state
        .runtime
        .launch_task_agent_run(task_id, &organization, &run, &plan)
        .map_err(map_runtime_error)?;
    if launched.replayed {
        let execution = observe_launched_run(&launched.task, &launched.view);
        return Ok(Json(LaunchTaskResponse {
            task: launched.task,
            run: launched.view,
            replayed: true,
            execution,
        }));
    }

    advance_consumer_run(
        &state,
        task_id,
        project_id,
        &organization,
        &launched.view,
        &request,
    )
    .await
    .map(Json)
}

async fn retry_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(request): Json<LaunchTaskRequest>,
) -> Result<Json<LaunchTaskResponse>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    let task = state
        .runtime
        .task(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))?;
    if task.state != TaskState::Blocked {
        return Err(ApiError::conflict(
            "only a blocked consumer task can be retried",
        ));
    }
    let project_id = task
        .project_id
        .ok_or_else(|| ApiError::bad_request("consumer task retry requires a selected project"))?;
    let run_id = state
        .runtime
        .task_launch_run_id(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::conflict("task has no consumer launch to retry"))?;
    let evaluation_retry = state
        .runtime
        .retry_consumer_evaluation(task_id)
        .map_err(map_runtime_error)?
        .is_some();
    let view = state
        .runtime
        .agent_run_view(run_id)
        .map_err(map_runtime_error)?;
    if view.run.state != AgentRunState::Running {
        return Err(ApiError::conflict(
            "consumer retry requires a running durable run",
        ));
    }
    let organization = state
        .runtime
        .organization_revision(view.run.organization_revision_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::internal("consumer run organization disappeared"))?;
    if !evaluation_retry {
        state
            .runtime
            .transition_task(task_id, TaskState::Active)
            .map_err(map_runtime_error)?;
    }
    advance_consumer_run(&state, task_id, project_id, &organization, &view, &request)
        .await
        .map(Json)
}

async fn resume_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
    Json(request): Json<LaunchTaskRequest>,
) -> Result<Json<LaunchTaskResponse>, ApiError> {
    let task_id = parse_task_id(&task_id)?;
    if state
        .runtime
        .consumer_evaluation_checkpoint(task_id)
        .map_err(map_runtime_error)?
        .is_some()
    {
        state
            .runtime
            .resume_consumer_evaluation(task_id)
            .map_err(map_runtime_error)?;
    } else {
        state
            .runtime
            .resume_approved_agent_work(
                task_id,
                request.context_token_budget,
                request.estimated_output_tokens,
                request.max_model_calls,
            )
            .await
            .map_err(map_runtime_error)?;
    }
    let task = state
        .runtime
        .task(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::internal("resumed task disappeared"))?;
    let project_id = task
        .project_id
        .ok_or_else(|| ApiError::internal("resumed task lost its project"))?;
    let run_id = state
        .runtime
        .task_launch_run_id(task_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::internal("resumed task lost its run"))?;
    let view = state
        .runtime
        .agent_run_view(run_id)
        .map_err(map_runtime_error)?;
    let organization = state
        .runtime
        .organization_revision(view.run.organization_revision_id)
        .map_err(map_runtime_error)?
        .ok_or_else(|| ApiError::internal("resumed run organization disappeared"))?;
    advance_consumer_run(&state, task_id, project_id, &organization, &view, &request)
        .await
        .map(Json)
}

#[allow(clippy::too_many_lines)] // Agent advancement and immediate objective settlement form one public operation.
async fn advance_consumer_run(
    state: &ApiState,
    task_id: TaskId,
    project_id: ProjectId,
    organization: &OrganizationGenome,
    view: &AgentRunView,
    request: &LaunchTaskRequest,
) -> Result<LaunchTaskResponse, ApiError> {
    let completed = view
        .leases
        .iter()
        .filter(|lease| lease.state == mcg_agents::AgentLeaseState::Completed)
        .map(|lease| lease.work_item_id)
        .collect::<BTreeSet<_>>();
    let mut execution = LaunchExecution::AwaitingEvaluation {
        detail: "All planned agent turns finished; objective evaluation is still required.".into(),
    };
    for item in &view.plan.items {
        if completed.contains(&item.id) {
            continue;
        }
        let role = organization
            .roles
            .iter()
            .find(|role| role.id == item.assigned_role)
            .map(|role| role.kind)
            .ok_or_else(|| ApiError::internal("planned role was not found"))?;
        let requested_tools = if role == RoleKind::Coder {
            request.requested_tools.clone()
        } else {
            Vec::new()
        };
        let result = state
            .runtime
            .execute_agent_work(&AgentDispatchSpec {
                run_id: view.run.id,
                work_item_id: item.id,
                project_id,
                target_path: request.target_path.clone(),
                context_token_budget: request.context_token_budget,
                requested_tools,
                estimated_output_tokens: request.estimated_output_tokens,
                max_model_calls: request.max_model_calls,
            })
            .await;
        if let Err(error) = result {
            execution = if let RuntimeError::AgentTurn(AgentTurnError::Tool(
                mcg_tools::ToolError::ApprovalRequired { .. },
            )) = &error
            {
                LaunchExecution::AwaitingApproval {
                    detail: "A governed tool action is waiting for owner approval.".into(),
                }
            } else {
                if state
                    .runtime
                    .task(task_id)
                    .map_err(map_runtime_error)?
                    .is_some_and(|task| task.state == TaskState::Active)
                {
                    state
                        .runtime
                        .transition_task(task_id, TaskState::Blocked)
                        .map_err(map_runtime_error)?;
                }
                LaunchExecution::Blocked {
                    detail: error.to_string(),
                }
            };
            break;
        }
    }
    let advanced_view = state
        .runtime
        .agent_run_view(view.run.id)
        .map_err(map_runtime_error)?;
    let all_turns_completed = advanced_view.plan.items.iter().all(|item| {
        advanced_view.leases.iter().any(|lease| {
            lease.work_item_id == item.id && lease.state == mcg_agents::AgentLeaseState::Completed
        })
    });
    let conditions_are_automatable = advanced_view
        .plan
        .items
        .iter()
        .flat_map(|item| &item.success_conditions)
        .all(is_automatable_consumer_condition);
    if all_turns_completed
        && conditions_are_automatable
        && state
            .runtime
            .task(task_id)
            .map_err(map_runtime_error)?
            .is_some_and(|task| task.state == TaskState::Active)
    {
        let progress = state
            .runtime
            .evaluate_and_settle_consumer_run(task_id)
            .await
            .map_err(map_runtime_error)?;
        execution = match progress {
            ConsumerEvaluationProgress::AwaitingApproval { .. } => {
                LaunchExecution::AwaitingApproval {
                    detail: "A guided objective check is waiting for owner approval.".into(),
                }
            }
            ConsumerEvaluationProgress::Settled(report) => match report.outcome {
                mcg_agents::EvaluationOutcome::Passed => LaunchExecution::Completed {
                    detail: "All planned work and objective checks passed.".into(),
                },
                mcg_agents::EvaluationOutcome::Failed => LaunchExecution::Failed {
                    detail:
                        "One or more objective checks failed; inspect evaluation evidence.".into(),
                },
                mcg_agents::EvaluationOutcome::Inconclusive => LaunchExecution::Blocked {
                    detail: "Objective checks were inconclusive; configuration or additional evidence is required.".into(),
                },
            },
        };
    }
    Ok(LaunchTaskResponse {
        task: state
            .runtime
            .task(task_id)
            .map_err(map_runtime_error)?
            .ok_or_else(|| ApiError::internal("launched task disappeared"))?,
        run: state
            .runtime
            .agent_run_view(view.run.id)
            .map_err(map_runtime_error)?,
        replayed: false,
        execution,
    })
}

fn observe_launched_run(task: &TaskEnvelope, view: &AgentRunView) -> LaunchExecution {
    if task.state == TaskState::Completed || view.run.state == AgentRunState::Completed {
        LaunchExecution::Completed {
            detail: "The task and run completed with recorded objective evaluation evidence."
                .into(),
        }
    } else if task.state == TaskState::Failed || view.run.state == AgentRunState::Failed {
        LaunchExecution::Failed {
            detail: "The task failed an objective evaluation; no duplicate work was issued.".into(),
        }
    } else if task.state == TaskState::WaitingForApproval
        || view.run.state == AgentRunState::Waiting
    {
        LaunchExecution::AwaitingApproval {
            detail: "This run is waiting for owner approval; no duplicate model work was issued."
                .into(),
        }
    } else if view.results.len() == view.plan.items.len() && view.evaluations.is_empty() {
        LaunchExecution::AwaitingEvaluation {
            detail:
                "Agent turns are durable; objective evaluation is still required before completion."
                    .into(),
        }
    } else if task.state == TaskState::Blocked
        || view
            .leases
            .iter()
            .any(|lease| lease.state == mcg_agents::AgentLeaseState::Failed)
    {
        LaunchExecution::Blocked {
            detail: "The prior launch has failed work evidence; use run details to retry safely."
                .into(),
        }
    } else {
        LaunchExecution::NotStarted {
            detail: "This task was already launched; no duplicate model work was issued.".into(),
        }
    }
}

fn is_automatable_consumer_condition(condition: &SuccessCondition) -> bool {
    matches!(
        condition,
        SuccessCondition::Build { .. }
            | SuccessCondition::Test { .. }
            | SuccessCondition::StaticAnalysis { .. }
    )
}

fn consumer_success_conditions(
    task: &TaskEnvelope,
    project: &ProjectModel,
) -> Vec<SuccessCondition> {
    let mut conditions = task
        .success_conditions
        .iter()
        .filter(|description| !description.trim().is_empty())
        .map(|description| SuccessCondition::Custom {
            description: description.trim().to_owned(),
        })
        .collect::<Vec<_>>();
    for command in &project.commands {
        let name = command.name.to_ascii_lowercase();
        let condition = if name.contains("test") {
            Some(SuccessCondition::Test {
                command: command.command.clone(),
            })
        } else if name.contains("lint") || name.contains("check") {
            Some(SuccessCondition::StaticAnalysis {
                command: command.command.clone(),
            })
        } else if name.contains("build") {
            Some(SuccessCondition::Build {
                command: command.command.clone(),
            })
        } else {
            None
        };
        if let Some(condition) = condition
            && !conditions.contains(&condition)
        {
            conditions.push(condition);
        }
    }
    if conditions.is_empty() {
        conditions.push(SuccessCondition::Custom {
            description: format!("Verify the completed goal: {}", task.goal),
        });
    }
    conditions
}

async fn get_agent_run(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Json<AgentRunView>, ApiError> {
    let run_id = parse_run_id(&run_id)?;
    state
        .runtime
        .agent_run_view(run_id)
        .map(Json)
        .map_err(map_runtime_error)
}

async fn record_evaluation(
    State(state): State<ApiState>,
    Path(run_id): Path<RunId>,
    Json(report): Json<EvaluationReport>,
) -> Result<(StatusCode, Json<EvaluationMutationResponse>), ApiError> {
    if report.run_id != run_id {
        return Err(ApiError::bad_request(
            "evaluation run does not match the request path",
        ));
    }
    let event = state
        .runtime
        .record_evaluation(&report)
        .map_err(map_runtime_error)?;
    Ok((
        StatusCode::CREATED,
        Json(EvaluationMutationResponse { report, event }),
    ))
}

async fn transition_agent_run(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
    Json(request): Json<TransitionAgentRunRequest>,
) -> Result<Json<AgentRunView>, ApiError> {
    if !matches!(
        request.state,
        AgentRunState::Running | AgentRunState::Cancelled
    ) {
        return Err(ApiError::bad_request(
            "external run transition may only start/resume or cancel",
        ));
    }
    let run_id = parse_run_id(&run_id)?;
    let current = state
        .runtime
        .agent_run_view(run_id)
        .map_err(map_runtime_error)?;
    state
        .runtime
        .transition_agent_run(run_id, request.state, current.run.metrics)
        .map_err(map_runtime_error)?;
    state
        .runtime
        .agent_run_view(run_id)
        .map(Json)
        .map_err(map_runtime_error)
}

async fn execute_agent_work(
    State(state): State<ApiState>,
    Path((run_id, work_item_id)): Path<(String, String)>,
    Json(request): Json<ExecuteAgentWorkRequest>,
) -> Result<Json<AgentTurnCommit>, ApiError> {
    let run_id = parse_run_id(&run_id)?;
    let work_item_id = work_item_id
        .parse::<WorkItemId>()
        .map_err(|_| ApiError::bad_request("work item id must be a UUID"))?;
    state
        .runtime
        .execute_agent_work(&AgentDispatchSpec {
            run_id,
            work_item_id,
            project_id: request.project_id,
            target_path: request.target_path,
            context_token_budget: request.context_token_budget,
            requested_tools: request.requested_tools,
            estimated_output_tokens: request.estimated_output_tokens,
            max_model_calls: request.max_model_calls,
        })
        .await
        .map(Json)
        .map_err(map_runtime_error)
}

async fn configure_provider(
    State(state): State<ApiState>,
    Json(mut request): Json<ConfigureProviderRequest>,
) -> Result<(StatusCode, Json<ProviderMutation>), ApiError> {
    if matches!(
        request.connection.auth_mode,
        ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
    ) {
        return Err(ApiError::bad_request(
            "ChatGPT OAuth connections must be created through an OAuth login flow",
        ));
    }
    let primary = request
        .credential
        .take()
        .map(ProviderCredential::new)
        .transpose()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let mutation = state
        .runtime
        .configure_provider(request.connection.clone(), primary.as_ref(), None)
        .await
        .map_err(map_runtime_error)?;
    Ok((StatusCode::CREATED, Json(mutation)))
}

async fn start_openai_oauth(
    State(state): State<ApiState>,
    Json(request): Json<StartOpenAiOAuthRequest>,
) -> Result<(StatusCode, Json<OAuthLoginStart>), ApiError> {
    let started = state
        .runtime
        .start_openai_oauth(request.connection_id, request.display_name, request.method)
        .await
        .map_err(map_runtime_error)?;
    Ok((StatusCode::CREATED, Json(started)))
}

async fn complete_openai_oauth(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<(StatusCode, Json<ProviderMutation>), ApiError> {
    let session_id = parse_oauth_session_id(&session_id)?;
    let mutation = state
        .runtime
        .complete_openai_oauth(session_id)
        .await
        .map_err(map_runtime_error)?;
    Ok((StatusCode::CREATED, Json(mutation)))
}

async fn cancel_openai_oauth(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let session_id = parse_oauth_session_id(&session_id)?;
    state
        .runtime
        .cancel_openai_oauth(session_id)
        .await
        .map_err(map_runtime_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_providers(
    State(state): State<ApiState>,
) -> Result<Json<ProviderListResponse>, ApiError> {
    let providers = state
        .runtime
        .provider_connections()
        .map_err(map_runtime_error)?
        .into_iter()
        .map(|connection| {
            let catalog = state
                .runtime
                .provider_catalog(&connection.id)
                .map_err(map_runtime_error)?;
            Ok(ProviderListItem {
                connection,
                catalog,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(ProviderListResponse { providers }))
}

async fn get_provider_models(
    State(state): State<ApiState>,
    Path(connection_id): Path<String>,
) -> Result<Json<ProviderModelCatalog>, ApiError> {
    state
        .runtime
        .provider_catalog(&connection_id)
        .map_err(map_runtime_error)?
        .map(Json)
        .ok_or_else(|| {
            ApiError::not_found(format!(
                "provider catalog for {connection_id} was not found"
            ))
        })
}

async fn discover_provider(
    State(state): State<ApiState>,
    Path(connection_id): Path<String>,
) -> Result<Json<ProviderModelCatalog>, ApiError> {
    state
        .runtime
        .discover_provider(&connection_id)
        .await
        .map(Json)
        .map_err(map_runtime_error)
}

async fn configure_mcp_connection(
    State(state): State<ApiState>,
    Json(mut request): Json<ConfigureMcpRequest>,
) -> Result<(StatusCode, Json<McpMutation>), ApiError> {
    let credential = request
        .credential
        .take()
        .map(McpCredential::new)
        .transpose()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let mutation = state
        .runtime
        .configure_mcp(request.connection.clone(), credential.as_ref())
        .await
        .map_err(map_runtime_error)?;
    Ok((StatusCode::CREATED, Json(mutation)))
}

async fn list_mcp_connections(
    State(state): State<ApiState>,
) -> Result<Json<McpListResponse>, ApiError> {
    let connections = state
        .runtime
        .mcp_connections()
        .map_err(map_runtime_error)?
        .into_iter()
        .map(|connection| {
            let catalog = state
                .runtime
                .mcp_catalog(&connection.id)
                .map_err(map_runtime_error)?;
            Ok(McpListItem {
                connection,
                catalog,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(McpListResponse { connections }))
}

async fn get_mcp_tools(
    State(state): State<ApiState>,
    Path(connection_id): Path<String>,
) -> Result<Json<McpToolCatalog>, ApiError> {
    state
        .runtime
        .mcp_catalog(&connection_id)
        .map_err(map_runtime_error)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("MCP catalog {connection_id} was not found")))
}

async fn discover_mcp_connection(
    State(state): State<ApiState>,
    Path(connection_id): Path<String>,
) -> Result<Json<McpToolCatalog>, ApiError> {
    state
        .runtime
        .discover_mcp(&connection_id)
        .await
        .map(Json)
        .map_err(map_runtime_error)
}

async fn refresh_openai_oauth(
    State(state): State<ApiState>,
    Path(connection_id): Path<String>,
) -> Result<Json<EventEnvelope>, ApiError> {
    state
        .runtime
        .refresh_openai_oauth(&connection_id)
        .await
        .map(Json)
        .map_err(map_runtime_error)
}

async fn probe_provider_model(
    State(state): State<ApiState>,
    Path((connection_id, model_id)): Path<(String, String)>,
) -> Result<Json<ModelProbeReport>, ApiError> {
    if connection_id.trim().is_empty() || model_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "provider connection and model ids are required",
        ));
    }
    state
        .runtime
        .probe_provider_model(&connection_id, &model_id)
        .await
        .map(Json)
        .map_err(map_runtime_error)
}

async fn discover_project(
    State(state): State<ApiState>,
    Json(request): Json<DiscoverProjectRequest>,
) -> Result<(StatusCode, Json<ProjectMutation>), ApiError> {
    if request.root.trim().is_empty() {
        return Err(ApiError::bad_request("project root cannot be empty"));
    }
    let project_id = request.project_id.unwrap_or_default();
    state
        .runtime
        .discover_project(std::path::Path::new(request.root.trim()), project_id)
        .map(|mutation| (StatusCode::CREATED, Json(mutation)))
        .map_err(map_runtime_error)
}

async fn list_projects(
    State(state): State<ApiState>,
) -> Result<Json<ProjectListResponse>, ApiError> {
    Ok(Json(ProjectListResponse {
        projects: state.runtime.projects()?,
    }))
}

async fn get_project(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectModel>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    state
        .runtime
        .project(project_id)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("project {project_id} was not found")))
}

async fn refresh_project(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectMutation>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let project = state
        .runtime
        .project(project_id)?
        .ok_or_else(|| ApiError::not_found(format!("project {project_id} was not found")))?;
    state
        .runtime
        .discover_project(std::path::Path::new(&project.root), project_id)
        .map(Json)
        .map_err(map_runtime_error)
}

async fn project_history(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectHistoryResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    if state.runtime.project(project_id)?.is_none() {
        return Err(ApiError::not_found(format!(
            "project {project_id} was not found"
        )));
    }
    Ok(Json(ProjectHistoryResponse {
        snapshots: state.runtime.project_history(project_id)?,
    }))
}

async fn create_memory_claim(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    Json(request): Json<CreateMemoryClaimRequest>,
) -> Result<(StatusCode, Json<MemoryClaimMutation>), ApiError> {
    let project_id = parse_project_id(&project_id)?;
    state
        .runtime
        .create_memory_claim(
            project_id,
            request.namespace,
            request.key,
            request.value,
            request.confidence,
            request.provenance,
            request.fresh_until,
        )
        .map(|mutation| (StatusCode::CREATED, Json(mutation)))
        .map_err(map_runtime_error)
}

async fn correct_memory_claim(
    State(state): State<ApiState>,
    Path((project_id, claim_id)): Path<(String, String)>,
    Json(request): Json<CorrectMemoryClaimRequest>,
) -> Result<Json<MemoryClaimMutation>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let claim_id = parse_memory_claim_id(&claim_id)?;
    state
        .runtime
        .correct_memory_claim(
            project_id,
            claim_id,
            request.value,
            request.confidence,
            request.provenance,
            request.fresh_until,
        )
        .map(Json)
        .map_err(map_runtime_error)
}

async fn transition_memory_claim(
    State(state): State<ApiState>,
    Path((project_id, claim_id)): Path<(String, String)>,
    Json(request): Json<TransitionMemoryClaimRequest>,
) -> Result<Json<MemoryClaimMutation>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let claim_id = parse_memory_claim_id(&claim_id)?;
    state
        .runtime
        .transition_memory_claim(project_id, claim_id, request.action, request.provenance)
        .map(Json)
        .map_err(map_runtime_error)
}

async fn list_memory_claims(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    Query(query): Query<MemoryListQuery>,
) -> Result<Json<MemoryClaimListResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    if state.runtime.project(project_id)?.is_none() {
        return Err(ApiError::not_found(format!(
            "project {project_id} was not found"
        )));
    }
    Ok(Json(MemoryClaimListResponse {
        claims: state.runtime.memory_claims(
            project_id,
            query.namespace.as_deref(),
            query.fresh_only,
        )?,
    }))
}

async fn memory_claim_history(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    Query(query): Query<MemoryHistoryQuery>,
) -> Result<Json<MemoryClaimHistoryResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    if state.runtime.project(project_id)?.is_none() {
        return Err(ApiError::not_found(format!(
            "project {project_id} was not found"
        )));
    }
    Ok(Json(MemoryClaimHistoryResponse {
        claims: state
            .runtime
            .memory_claim_history(project_id, &query.namespace, &query.key)?,
    }))
}

async fn managed_memory(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
) -> Result<Json<ManagedMemoryResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    if state.runtime.project(project_id)?.is_none() {
        return Err(ApiError::not_found(format!(
            "project {project_id} was not found"
        )));
    }
    Ok(Json(ManagedMemoryResponse {
        markdown: state.runtime.managed_memory(project_id)?,
    }))
}

async fn assemble_project_context(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    Json(request): Json<AssembleContextRequest>,
) -> Result<Json<ContextAssembly>, ApiError> {
    if request.token_budget == 0 || request.token_budget > 1_000_000 {
        return Err(ApiError::bad_request(
            "context token budget must be between 1 and 1000000",
        ));
    }
    let project_id = parse_project_id(&project_id)?;
    let target_path = request.target_path;
    state
        .runtime
        .assemble_project_context(
            project_id,
            request.items,
            request.token_budget,
            target_path.as_deref(),
        )
        .map(Json)
        .map_err(map_runtime_error)
}

async fn list_approvals(
    State(state): State<ApiState>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<Json<ApprovalListResponse>, ApiError> {
    Ok(Json(ApprovalListResponse {
        approvals: state.runtime.approvals(query.state)?,
    }))
}

async fn get_approval(
    State(state): State<ApiState>,
    Path(approval_id): Path<String>,
) -> Result<Json<ApprovalRequest>, ApiError> {
    let approval_id = parse_approval_id(&approval_id)?;
    state
        .runtime
        .approval(approval_id)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("approval {approval_id} was not found")))
}

async fn resolve_approval(
    State(state): State<ApiState>,
    Path(approval_id): Path<String>,
    Json(request): Json<ResolveApprovalRequest>,
) -> Result<Json<ApprovalRequest>, ApiError> {
    let approval_id = parse_approval_id(&approval_id)?;
    if !matches!(
        request.state,
        ApprovalState::Approved | ApprovalState::Denied | ApprovalState::Cancelled
    ) {
        return Err(ApiError::bad_request(
            "resolution state must be approved, denied, or cancelled",
        ));
    }
    state
        .runtime
        .resolve_approval(&ApprovalResolution {
            approval_id,
            state: request.state,
            resolved_by: "local_owner".into(),
            reason: request.reason,
            resolved_at: Utc::now(),
        })
        .map(Json)
        .map_err(|error| {
            if error.is_conflict() {
                ApiError::conflict(error.to_string())
            } else {
                ApiError::from(error)
            }
        })
}

async fn stream_events(
    State(state): State<ApiState>,
    Query(query): Query<EventStreamQuery>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let receiver = state.runtime.subscribe();
    let header_sequence = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|_| ApiError::bad_request("Last-Event-ID must be an event sequence"))?;
    let after = query.after.or(header_sequence);
    let backlog = after
        .map(|sequence| state.runtime.events_after(sequence))
        .transpose()?
        .unwrap_or_default();
    let replay_cutoff = backlog
        .last()
        .map_or(after.unwrap_or(0), |event| event.sequence);
    let durable = tokio_stream::iter(backlog).filter_map(|event| to_sse_event(&event));
    let live = BroadcastStream::new(receiver).filter_map(move |result| {
        result
            .ok()
            .filter(|event| event.sequence > replay_cutoff)
            .and_then(|event| to_sse_event(&event))
    });
    let stream = durable.chain(live);
    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn to_sse_event(event: &EventEnvelope) -> Option<Result<Event, Infallible>> {
    serde_json::to_string(&event).ok().map(|data| {
        Ok(Event::default()
            .id(event.sequence.to_string())
            .event(event.kind.as_str())
            .data(data))
    })
}

fn parse_task_id(value: &str) -> Result<TaskId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("task id must be a UUID"))
}

fn parse_organization_id(value: &str) -> Result<OrganizationId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("organization id must be a UUID"))
}

fn parse_run_id(value: &str) -> Result<RunId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("run id must be a UUID"))
}

fn parse_approval_id(value: &str) -> Result<ApprovalId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("approval id must be a UUID"))
}

fn parse_project_id(value: &str) -> Result<ProjectId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("project id must be a UUID"))
}

fn parse_memory_claim_id(value: &str) -> Result<MemoryClaimId, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("memory claim id must be a UUID"))
}

fn parse_oauth_session_id(value: &str) -> Result<uuid::Uuid, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("OAuth session id must be a UUID"))
}

#[allow(clippy::needless_pass_by_value)]
fn map_runtime_error(error: RuntimeError) -> ApiError {
    let provider_error = match &error {
        RuntimeError::Provider(provider)
        | RuntimeError::AgentTurn(AgentTurnError::Provider(provider)) => Some(provider),
        _ => None,
    };
    if let Some(provider) = provider_error {
        return match provider {
            mcg_providers::ProviderError::Configuration(_) => {
                ApiError::bad_request(error.to_string())
            }
            mcg_providers::ProviderError::Authentication(_) => {
                ApiError::unprocessable(error.to_string())
            }
            _ => ApiError::bad_gateway(error.to_string()),
        };
    }
    if let RuntimeError::Mcp(mcp) = &error {
        return match mcp {
            McpError::Configuration(_) | McpError::InvalidResponse(_) => {
                ApiError::bad_request(error.to_string())
            }
            McpError::Authentication(_) => ApiError::unprocessable(error.to_string()),
            McpError::Transport(_) | McpError::Remote { .. } | McpError::Negotiation(_) => {
                ApiError::bad_gateway(error.to_string())
            }
        };
    }
    if error.is_not_found() {
        ApiError::not_found(error.to_string())
    } else if error.is_invalid_request() {
        ApiError::bad_request(error.to_string())
    } else if error.is_conflict() {
        ApiError::conflict(error.to_string())
    } else {
        ApiError::from(error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use mcg_agents::{AgentState, ConditionResult, EvaluationEvidence, EvaluationOutcome};
    use mcg_config::{AppConfig, AutonomyPreset};
    use mcg_domain::{ArtifactId, EvaluationId, ToolExecutionId, ToolRequestId};
    use mcg_mcp::{
        MCP_CONNECTION_CONTRACT_VERSION, MCP_PROTOCOL_VERSION, McpAuthMode, McpProtocolMode,
        McpTransport,
    };
    use mcg_providers::{
        BrowserAuthorizationConfig, CHATGPT_CODEX_BASE_URL, CONTRACT_VERSION,
        DeviceAuthorizationConfig, ProviderKind,
    };
    use mcg_tools::{
        ApprovalRepository, TOOL_SCHEMA_VERSION, ToolAuditPhase, ToolAuditSink, ToolOutcomeStatus,
    };
    use tower::ServiceExt;

    use super::*;

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn test_runtime() -> Arc<AppRuntime> {
        Arc::new(
            AppRuntime::open_in_memory(AppConfig::default(), TEST_TOKEN.to_owned())
                .expect("runtime should open"),
        )
    }

    async fn model_discovery_fixture() -> String {
        async fn response(Json(body): Json<serde_json::Value>) -> Response {
            if body.get("stream").and_then(serde_json::Value::as_bool) == Some(true) {
                return (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/event-stream",
                    )],
                    concat!(
                        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"fixture-stream\",\"model\":\"fixture-model\"}}\n\n",
                        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"item_id\":\"message-1\",\"delta\":\"fixture\"}\n\n",
                        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"fixture-stream\",\"model\":\"fixture-model\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"fixture\"}]}],\"usage\":{\"input_tokens\":8,\"output_tokens\":1,\"total_tokens\":9}}}\n\n"
                    ),
                )
                    .into_response();
            }
            Json(serde_json::json!({
                "id": "fixture-response",
                "model": "fixture-model",
                "status": "completed",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "fixture complete"}]
                }],
                "usage": {"input_tokens": 32, "output_tokens": 4, "total_tokens": 36}
            }))
            .into_response()
        }

        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "fixture-model",
                            "object": "model",
                            "owned_by": "fixture",
                            "context_window": 32768
                        }]
                    }))
                }),
            )
            .route("/v1/responses", post(response));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture listener");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fixture server");
        });
        format!("http://{address}/v1")
    }

    async fn caller_tool_fixture() -> String {
        async fn response(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
            let name = body
                .get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("caller_tool");
            Json(serde_json::json!({
                "id": "caller-tool-response",
                "model": "caller-tool-model",
                "status": "completed",
                "output": [{
                    "type": "function_call",
                    "call_id": "caller-call-1",
                    "name": name,
                    "arguments": "{\"value\":\"fixture\"}"
                }],
                "usage": {"input_tokens": 12, "output_tokens": 5, "total_tokens": 17}
            }))
        }
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "caller-tool-model",
                            "owned_by": "fixture",
                            "context_window": 16384,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route("/v1/responses", post(response));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("caller tool fixture listener");
        let address = listener.local_addr().expect("caller tool fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("caller tool fixture server");
        });
        format!("http://{address}/v1")
    }

    async fn governed_mcp_tool_fixture(calls: Arc<AtomicUsize>) -> String {
        async fn response(
            State(calls): State<Arc<AtomicUsize>>,
            Json(body): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            let result = match body["method"].as_str().expect("method") {
                "server/discover" => serde_json::json!({
                    "supportedVersions": [MCP_PROTOCOL_VERSION],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "governed-fixture"}
                }),
                "tools/list" => serde_json::json!({
                    "tools": [{
                        "name": "lookup",
                        "description": "Read deterministic fixture data",
                        "inputSchema": {"type": "object", "additionalProperties": false},
                        "annotations": {
                            "readOnlyHint": true,
                            "destructiveHint": false,
                            "openWorldHint": false
                        }
                    }],
                    "serverInfo": {"name": "governed-fixture"}
                }),
                "tools/call" => {
                    calls.fetch_add(1, Ordering::SeqCst);
                    serde_json::json!({
                        "isError": false,
                        "content": [{"type": "text", "text": "governed fixture result"}],
                        "structuredContent": {"value": 42},
                        "serverInfo": {"name": "governed-fixture"}
                    })
                }
                _ => unreachable!(),
            };
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": body["id"].clone(),
                "result": result
            }))
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind MCP");
        let address = listener.local_addr().expect("MCP address");
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/mcp", post(response))
                    .with_state(calls),
            )
            .await
            .expect("serve MCP");
        });
        format!("http://{address}/mcp")
    }

    async fn governed_mcp_model_fixture() -> String {
        async fn response(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
            let has_result = body.to_string().contains("function_call_output");
            let output = if has_result {
                serde_json::json!([{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "Remote evidence received"}]
                }])
            } else {
                let name = body
                    .get("tools")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|tools| tools.first())
                    .and_then(|tool| tool.get("name"))
                    .and_then(serde_json::Value::as_str)
                    .expect("exposed MCP tool");
                serde_json::json!([{
                    "type": "function_call",
                    "call_id": "mcp-call-1",
                    "name": name,
                    "arguments": "{}"
                }])
            };
            Json(serde_json::json!({
                "id": if has_result {"mcp-final"} else {"mcp-call"},
                "model": "mcp-agent-model",
                "status": "completed",
                "output": output,
                "usage": {"input_tokens": 16, "output_tokens": 4, "total_tokens": 20}
            }))
        }
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "mcp-agent-model",
                            "owned_by": "fixture",
                            "context_window": 16384,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route("/v1/responses", post(response));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind model");
        let address = listener.local_addr().expect("model address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve model");
        });
        format!("http://{address}/v1")
    }

    #[allow(clippy::too_many_lines)] // Complete generated website stays readable as one fixture.
    async fn website_builder_fixture(project_root: &std::path::Path) -> String {
        const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>LaunchPad</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <main>
    <p class="eyebrow">A tiny project planner</p>
    <h1>Turn today’s idea into a clear next step.</h1>
    <form id="task-form">
      <label for="task">What do you want to ship?</label>
      <div class="controls">
        <input id="task" name="task" required maxlength="80" autocomplete="off">
        <button type="submit">Add next step</button>
      </div>
    </form>
    <p id="status" role="status" aria-live="polite">No steps yet.</p>
    <ul id="task-list" aria-label="Project steps"></ul>
  </main>
  <script src="app.js"></script>
</body>
</html>
"#;
        const STYLES_CSS: &str = r":root { color-scheme: light; font-family: system-ui, sans-serif; background: #f4f1ea; color: #17201b; }
body { margin: 0; min-height: 100vh; display: grid; place-items: center; }
main { width: min(42rem, calc(100% - 2rem)); background: #fffdf8; padding: clamp(1.5rem, 5vw, 3.5rem); border-radius: 1.5rem; box-shadow: 0 1rem 3rem #1833231f; }
.eyebrow { color: #24734b; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; }
h1 { font-size: clamp(2rem, 6vw, 3.6rem); line-height: 1; max-width: 12ch; }
label { display: block; font-weight: 700; margin-bottom: .5rem; }
.controls { display: flex; gap: .6rem; flex-wrap: wrap; }
input { flex: 1 1 16rem; border: 2px solid #9aab9f; border-radius: .7rem; padding: .8rem; font: inherit; }
button { border: 0; border-radius: .7rem; padding: .8rem 1rem; background: #195f3d; color: white; font: inherit; font-weight: 700; cursor: pointer; }
button:focus-visible, input:focus-visible { outline: 3px solid #e39d27; outline-offset: 2px; }
li { margin-block: .7rem; padding: .8rem 1rem; background: #eef5ef; border-radius: .7rem; }
";
        const APP_JS: &str = r"const form = document.querySelector('#task-form');
const input = document.querySelector('#task');
const list = document.querySelector('#task-list');
const status = document.querySelector('#status');
form.addEventListener('submit', (event) => {
  event.preventDefault();
  const value = input.value.trim();
  if (!value) return;
  const item = document.createElement('li');
  item.textContent = value;
  list.append(item);
  input.value = '';
  status.textContent = `${list.children.length} step${list.children.length === 1 ? '' : 's'} ready.`;
  input.focus();
});
";

        let tool_calls = Arc::new(
            [
                ("write-index", "index.html", INDEX_HTML),
                ("write-styles", "styles.css", STYLES_CSS),
                ("write-script", "app.js", APP_JS),
            ]
            .into_iter()
            .map(|(id, path, content)| {
                serde_json::json!({
                    "type": "function_call",
                    "call_id": id,
                    "name": "fs.write",
                    "arguments": serde_json::to_string(&serde_json::json!({
                        "path": project_root.join(path).to_string_lossy(),
                        "content": content,
                        "create_new": true
                    })).expect("tool arguments")
                })
            })
            .collect::<Vec<_>>(),
        );

        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "website-builder",
                            "object": "model",
                            "owned_by": "fixture",
                            "context_window": 32768,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route(
                "/v1/responses",
                post(move |Json(body): Json<serde_json::Value>| {
                    let tool_calls = tool_calls.clone();
                    async move {
                        let has_tool_result = body
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|items| {
                                items.iter().any(|item| {
                                    item.get("type").and_then(serde_json::Value::as_str)
                                        == Some("function_call_output")
                                })
                            });
                        let has_write_tool = body
                            .get("tools")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|tools| {
                                tools.iter().any(|tool| {
                                    tool.pointer("/function/name")
                                        .or_else(|| tool.get("name"))
                                        .and_then(serde_json::Value::as_str)
                                        == Some("fs.write")
                                })
                            });
                        let (response_id, output) = if has_tool_result {
                            (
                                "website-complete",
                                vec![serde_json::json!({
                                    "type": "message",
                                    "role": "assistant",
                                    "content": [{"type": "output_text", "text": "LaunchPad was built and is ready to open."}]
                                })],
                            )
                        } else if has_write_tool {
                            ("website-tools", (*tool_calls).clone())
                        } else {
                            (
                                "evaluation-complete",
                                vec![serde_json::json!({
                                    "type": "message",
                                    "role": "assistant",
                                    "content": [{"type": "output_text", "text": "Independent evaluation reviewed the durable write evidence."}]
                                })],
                            )
                        };
                        Json(serde_json::json!({
                            "id": response_id,
                            "model": "website-builder",
                            "status": "completed",
                            "output": output,
                            "usage": {"input_tokens": 64, "output_tokens": 32, "total_tokens": 96}
                        }))
                    }
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("website fixture listener");
        let address = listener.local_addr().expect("website fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("website fixture server");
        });
        format!("http://{address}/v1")
    }

    async fn approval_resume_fixture(target: &std::path::Path) -> String {
        let target = target.to_string_lossy().into_owned();
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "approval-resume-model",
                            "object": "model",
                            "owned_by": "fixture",
                            "context_window": 32768,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route(
                "/v1/responses",
                post(move |Json(body): Json<serde_json::Value>| {
                    let target = target.clone();
                    async move {
                        let has_tool_result = body
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|items| {
                                items.iter().any(|item| {
                                    item.get("type").and_then(serde_json::Value::as_str)
                                        == Some("function_call_output")
                                })
                            });
                        let names = body
                            .get("tools")
                            .and_then(serde_json::Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|tool| {
                                tool.pointer("/function/name")
                                    .or_else(|| tool.get("name"))
                                    .and_then(serde_json::Value::as_str)
                            })
                            .collect::<BTreeSet<_>>();
                        let output = if has_tool_result {
                            vec![serde_json::json!({
                                "type": "message", "role": "assistant",
                                "content": [{"type": "output_text", "text": "Approved write completed."}]
                            })]
                        } else if names.contains("report_ok") {
                            vec![serde_json::json!({
                                "type": "function_call", "call_id": "probe-call",
                                "name": "report_ok", "arguments": "{\"ok\":true}"
                            })]
                        } else if names.contains("fs.write") {
                            vec![serde_json::json!({
                                "type": "function_call", "call_id": "approved-write",
                                "name": "fs.write",
                                "arguments": serde_json::to_string(&serde_json::json!({
                                    "path": target,
                                    "content": "approval continuation succeeded\n",
                                    "create_new": true
                                })).expect("arguments")
                            })]
                        } else {
                            vec![serde_json::json!({
                                "type": "message", "role": "assistant",
                                "content": [{"type": "output_text", "text": "Independent review finished."}]
                            })]
                        };
                        Json(serde_json::json!({
                            "id": "approval-resume-response",
                            "model": "approval-resume-model",
                            "status": "completed",
                            "output": output,
                            "usage": {"input_tokens": 32, "output_tokens": 16, "total_tokens": 48}
                        }))
                    }
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("approval fixture listener");
        let address = listener.local_addr().expect("approval fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("approval fixture server");
        });
        format!("http://{address}/v1")
    }

    async fn objective_model_fixture(calls: Arc<AtomicUsize>) -> String {
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "objective-model",
                            "object": "model",
                            "owned_by": "fixture",
                            "context_window": 32768,
                            "supported_parameters": ["tools"]
                        }]
                    }))
                }),
            )
            .route(
                "/v1/responses",
                post(move || {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Json(serde_json::json!({
                            "id": "objective-response",
                            "model": "objective-model",
                            "status": "completed",
                            "output": [{
                                "type": "message",
                                "role": "assistant",
                                "content": [{
                                    "type": "output_text",
                                    "text": "Work turn finished; objective checks decide completion."
                                }]
                            }],
                            "usage": {
                                "input_tokens": 24,
                                "output_tokens": 8,
                                "total_tokens": 32
                            }
                        }))
                    }
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("objective fixture listener");
        let address = listener.local_addr().expect("objective fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("objective fixture server");
        });
        format!("http://{address}/v1")
    }

    async fn device_oauth_fixture() -> String {
        let app = Router::new()
            .route(
                "/api/accounts/deviceauth/usercode",
                post(|| async {
                    Json(serde_json::json!({
                        "device_auth_id": "fixture-device-secret",
                        "user_code": "ABCD-1234",
                        "interval": 1
                    }))
                }),
            )
            .route(
                "/api/accounts/deviceauth/token",
                post(|| async {
                    Json(serde_json::json!({
                        "authorization_code": "fixture-authorization-secret",
                        "code_challenge": "fixture-challenge",
                        "code_verifier": "fixture-verifier-secret"
                    }))
                }),
            )
            .route(
                "/oauth/token",
                post(|| async {
                    Json(serde_json::json!({
                        "id_token": "fixture-id-secret",
                        "access_token": "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC0xMjMifX0.sig",
                        "refresh_token": "fixture-refresh-secret"
                    }))
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("OAuth fixture listener");
        let address = listener.local_addr().expect("OAuth fixture address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("OAuth fixture server");
        });
        format!("http://{address}")
    }

    fn authorized(request: axum::http::request::Builder) -> axum::http::request::Builder {
        request.header("authorization", format!("Bearer {TEST_TOKEN}"))
    }

    fn mcp_request(
        id: &serde_json::Value,
        method: &str,
        mut params: serde_json::Map<String, serde_json::Value>,
        name: Option<&str>,
    ) -> Request<Body> {
        params.insert(
            "_meta".into(),
            serde_json::json!({
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": {
                    "name": "mcg-api-test",
                    "version": "1.0.0"
                },
                "io.modelcontextprotocol/clientCapabilities": {}
            }),
        );
        let mut request = authorized(Request::post("/mcp"))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28")
            .header("mcp-method", method);
        if let Some(name) = name {
            request = request.header("mcp-name", name);
        }
        request
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": method,
                    "params": params,
                })
                .to_string(),
            ))
            .expect("MCP request")
    }

    async fn response_json(response: Response) -> serde_json::Value {
        serde_json::from_slice(
            &response
                .into_body()
                .collect()
                .await
                .expect("response body")
                .to_bytes(),
        )
        .expect("JSON response")
    }

    #[tokio::test]
    async fn health_is_public_but_status_requires_authentication() {
        let app = router(test_runtime());
        let health = app
            .clone()
            .oneshot(
                Request::get("/api/v1/system/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = app
            .oneshot(
                Request::get("/api/v1/system/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(status.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Covers the complete stateless MCP read boundary.
    async fn mcp_server_is_authenticated_header_safe_and_project_aware() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(
            project_root.join("package.json"),
            r#"{"scripts":{"test":"vitest"}}"#,
        )
        .expect("project fixture");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let runtime = Arc::new(
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        runtime
            .create_memory_claim(
                project.project_id,
                "project",
                "package_manager",
                serde_json::json!("npm"),
                0.9,
                ClaimProvenance {
                    source_kind: mcg_projects::ProvenanceKind::User,
                    source: "MCP fixture".into(),
                    task_id: None,
                    artifact_sha256: None,
                },
                None,
            )
            .expect("memory claim");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "mcp_execution_fixture".into(),
                    display_name: "MCP Execution Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: model_discovery_fixture().await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("MCP provider");
        let app = router(runtime);

        let valid_discover = mcp_request(
            &serde_json::json!("discover"),
            "server/discover",
            serde_json::Map::new(),
            None,
        );
        let unauthorized = app
            .clone()
            .oneshot({
                let (mut parts, body) = valid_discover.into_parts();
                parts.headers.remove("authorization");
                Request::from_parts(parts, body)
            })
            .await
            .expect("unauthorized MCP response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let mut unauthenticated_hostile = mcp_request(
            &serde_json::json!(0),
            "server/discover",
            serde_json::Map::new(),
            None,
        );
        unauthenticated_hostile
            .headers_mut()
            .remove("authorization");
        unauthenticated_hostile
            .headers_mut()
            .insert("origin", "https://attacker.example".parse().unwrap());
        let unauthenticated_hostile = app
            .clone()
            .oneshot(unauthenticated_hostile)
            .await
            .expect("unauthenticated hostile origin response");
        assert_eq!(unauthenticated_hostile.status(), StatusCode::FORBIDDEN);

        let mut hostile = mcp_request(
            &serde_json::json!(1),
            "server/discover",
            serde_json::Map::new(),
            None,
        );
        hostile
            .headers_mut()
            .insert("origin", "https://attacker.example".parse().unwrap());
        let hostile = app
            .clone()
            .oneshot(hostile)
            .await
            .expect("hostile origin response");
        assert_eq!(hostile.status(), StatusCode::FORBIDDEN);

        let mut mismatch = mcp_request(
            &serde_json::json!(2),
            "server/discover",
            serde_json::Map::new(),
            None,
        );
        mismatch.headers_mut().remove("mcp-method");
        let mismatch = app
            .clone()
            .oneshot(mismatch)
            .await
            .expect("header mismatch response");
        assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(mismatch).await.pointer("/error/code"),
            Some(&serde_json::json!(-32_020))
        );

        let discover = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(3),
                "server/discover",
                serde_json::Map::new(),
                None,
            ))
            .await
            .expect("discover response");
        assert_eq!(discover.status(), StatusCode::OK);
        let discover = response_json(discover).await;
        assert_eq!(
            discover.pointer("/result/supportedVersions/0"),
            Some(&serde_json::json!("2026-07-28"))
        );
        assert_eq!(
            discover.pointer("/result/cacheScope"),
            Some(&serde_json::json!("private"))
        );

        let tools = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(4),
                "tools/list",
                serde_json::Map::new(),
                None,
            ))
            .await
            .expect("tools response");
        assert_eq!(tools.status(), StatusCode::OK);
        let tools = response_json(tools).await;
        let names: Vec<_> = tools["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "mcg_memory_search",
                "mcg_project_context",
                "mcg_query_project",
                "mcg_query_run",
                "mcg_run_agent",
                "mcg_system_status"
            ]
        );

        let status = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(5),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_system_status")),
                    ("arguments".into(), serde_json::json!({})),
                ]),
                Some("mcg_system_status"),
            ))
            .await
            .expect("status tool response");
        assert_eq!(status.status(), StatusCode::OK);
        let status = response_json(status).await;
        assert_eq!(
            status.pointer("/result/structuredContent/state"),
            Some(&serde_json::json!("ready"))
        );

        let query = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(6),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_query_project")),
                    (
                        "arguments".into(),
                        serde_json::json!({"project_id": project.project_id}),
                    ),
                ]),
                Some("mcg_query_project"),
            ))
            .await
            .expect("project tool response");
        let query = response_json(query).await;
        assert_eq!(
            query.pointer("/result/structuredContent/project/project_id"),
            Some(&serde_json::json!(project.project_id))
        );

        let context = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(7),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_project_context")),
                    (
                        "arguments".into(),
                        serde_json::json!({
                            "project_id": project.project_id,
                            "token_budget": 4096
                        }),
                    ),
                ]),
                Some("mcg_project_context"),
            ))
            .await
            .expect("context tool response");
        let context = response_json(context).await;
        assert!(
            context["result"]["structuredContent"]["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );

        let memory = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(8),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_memory_search")),
                    (
                        "arguments".into(),
                        serde_json::json!({
                            "project_id": project.project_id,
                            "query": "npm"
                        }),
                    ),
                ]),
                Some("mcg_memory_search"),
            ))
            .await
            .expect("memory tool response");
        let memory = response_json(memory).await;
        assert_eq!(
            memory.pointer("/result/structuredContent/claims/0/key"),
            Some(&serde_json::json!("package_manager"))
        );

        let run_arguments = serde_json::json!({
            "request_key": "mcp-agent-fixture-1",
            "project_id": project.project_id,
            "goal": "Explain the project in one grounded response",
            "role": "conversation",
            "success_conditions": ["return a terminal provider response"],
            "context_token_budget": 512,
            "estimated_output_tokens": 128,
            "max_model_calls": 2
        });
        let run_agent = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(9),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_run_agent")),
                    ("arguments".into(), run_arguments.clone()),
                ]),
                Some("mcg_run_agent"),
            ))
            .await
            .expect("run agent response");
        assert_eq!(run_agent.status(), StatusCode::OK);
        let run_agent = response_json(run_agent).await;
        assert_eq!(
            run_agent.pointer("/result/isError"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            run_agent.pointer("/result/structuredContent/task/state"),
            Some(&serde_json::json!("active"))
        );
        assert_eq!(
            run_agent.pointer("/result/structuredContent/run/results/0/response/id"),
            Some(&serde_json::json!("fixture-response"))
        );
        let run_id = run_agent["result"]["structuredContent"]["run"]["run"]["id"].clone();

        let replay = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(10),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_run_agent")),
                    ("arguments".into(), run_arguments.clone()),
                ]),
                Some("mcg_run_agent"),
            ))
            .await
            .expect("replayed agent response");
        let replay = response_json(replay).await;
        assert_eq!(
            replay.pointer("/result/structuredContent/replayed"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            replay["result"]["structuredContent"]["run"]["results"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            replay["result"]["structuredContent"]["run"]["instances"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );

        let queried_run = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(11),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_query_run")),
                    ("arguments".into(), serde_json::json!({"run_id": run_id})),
                ]),
                Some("mcg_query_run"),
            ))
            .await
            .expect("query run response");
        let queried_run = response_json(queried_run).await;
        assert_eq!(
            queried_run["result"]["structuredContent"]["results"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );

        let conflicting_key = app
            .clone()
            .oneshot(mcp_request(
                &serde_json::json!(12),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_run_agent")),
                    (
                        "arguments".into(),
                        serde_json::json!({
                            "request_key": "mcp-agent-fixture-1",
                            "project_id": project.project_id,
                            "goal": "A different goal"
                        }),
                    ),
                ]),
                Some("mcg_run_agent"),
            ))
            .await
            .expect("conflicting key response");
        assert_eq!(
            response_json(conflicting_key).await.pointer("/error/code"),
            Some(&serde_json::json!(-32_602))
        );

        let wrong_name = app
            .oneshot(mcp_request(
                &serde_json::json!(13),
                "tools/call",
                serde_json::Map::from_iter([
                    ("name".into(), serde_json::json!("mcg_system_status")),
                    ("arguments".into(), serde_json::json!({})),
                ]),
                Some("mcg_query_project"),
            ))
            .await
            .expect("name mismatch response");
        assert_eq!(wrong_name.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(wrong_name).await.pointer("/error/code"),
            Some(&serde_json::json!(-32_020))
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Exercises the complete secret-isolated OAuth lifecycle.
    async fn device_oauth_login_is_authenticated_cancellable_and_billing_isolated() {
        let issuer = device_oauth_fixture().await;
        let runtime = Arc::new(
            AppRuntime::open_in_memory_with_oauth_configs(
                AppConfig::default(),
                TEST_TOKEN.to_owned(),
                BrowserAuthorizationConfig::default(),
                DeviceAuthorizationConfig {
                    issuer,
                    client_id: "fixture-client".into(),
                    timeout: Duration::from_secs(2),
                },
            )
            .expect("runtime should open"),
        );
        let app = router(runtime.clone());
        let request = StartOpenAiOAuthRequest {
            connection_id: "openai-subscription".into(),
            display_name: "OpenAI Subscription".into(),
            method: OpenAiOAuthMethod::DeviceCode,
        };
        let body = serde_json::to_vec(&request).expect("start request");
        let unauthorized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/oauth/openai/start")
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("unauthorized response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let started = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/oauth/openai/start"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("start response");
        assert_eq!(started.status(), StatusCode::CREATED);
        let started: OAuthLoginStart = serde_json::from_slice(
            &started
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("OAuth start response");
        assert_eq!(started.user_code.as_deref(), Some("ABCD-1234"));
        assert_eq!(started.auth_mode, ProviderAuthMode::ChatGptDeviceCode);
        assert!(!format!("{started:?}").contains("ABCD-1234"));
        assert!(
            runtime
                .provider_connections()
                .expect("providers")
                .is_empty()
        );

        let complete_path = format!("/api/v1/oauth/openai/{}/complete", started.session_id);
        let completed = app
            .clone()
            .oneshot(
                authorized(Request::post(&complete_path))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("complete response");
        assert_eq!(completed.status(), StatusCode::CREATED);
        let completed_bytes = completed
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let mutation: ProviderMutation =
            serde_json::from_slice(&completed_bytes).expect("provider mutation");
        assert_eq!(mutation.connection.base_url, CHATGPT_CODEX_BASE_URL);
        assert_eq!(
            mutation.connection.auth_mode,
            ProviderAuthMode::ChatGptDeviceCode
        );
        assert_eq!(mutation.connection.kind, ProviderKind::OpenAi);
        assert!(mutation.catalog.models.is_empty());
        let response_text = String::from_utf8(completed_bytes.to_vec()).expect("UTF-8 response");
        assert!(!response_text.contains("fixture-refresh-secret"));
        assert!(!response_text.contains("account-123"));
        assert_eq!(runtime.provider_connections().expect("providers").len(), 1);
        let consumed = app
            .clone()
            .oneshot(
                authorized(Request::post(&complete_path))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("consumed response");
        assert_eq!(consumed.status(), StatusCode::BAD_REQUEST);

        let refreshed = app
            .clone()
            .oneshot(
                authorized(Request::post(
                    "/api/v1/providers/openai-subscription/oauth/refresh",
                ))
                .body(Body::empty())
                .expect("request"),
            )
            .await
            .expect("refresh response");
        assert_eq!(refreshed.status(), StatusCode::OK);
        let refreshed_bytes = refreshed
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let refreshed_event: EventEnvelope =
            serde_json::from_slice(&refreshed_bytes).expect("refresh event");
        assert_eq!(
            refreshed_event.kind,
            mcg_events::EventKind::ProviderCredentialsRefreshed
        );
        assert_eq!(
            refreshed_event.payload["connection_id"],
            "openai-subscription"
        );
        let refresh_text = String::from_utf8(refreshed_bytes.to_vec()).expect("UTF-8 response");
        assert!(!refresh_text.contains("fixture-refresh-secret"));
        assert!(!refresh_text.contains("account-123"));

        let cancel_body = serde_json::to_vec(&StartOpenAiOAuthRequest {
            connection_id: "cancelled-subscription".into(),
            display_name: "Cancelled Subscription".into(),
            method: OpenAiOAuthMethod::DeviceCode,
        })
        .expect("cancel start request");
        let cancel_started = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/oauth/openai/start"))
                    .header("content-type", "application/json")
                    .body(Body::from(cancel_body))
                    .expect("request"),
            )
            .await
            .expect("cancel start response");
        let cancel_started: OAuthLoginStart = serde_json::from_slice(
            &cancel_started
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("cancel start");
        let cancel_path = format!("/api/v1/oauth/openai/{}/cancel", cancel_started.session_id);
        let cancelled = app
            .clone()
            .oneshot(
                authorized(Request::post(cancel_path))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("cancel response");
        assert_eq!(cancelled.status(), StatusCode::NO_CONTENT);
        let cancelled_complete = app
            .oneshot(
                authorized(Request::post(format!(
                    "/api/v1/oauth/openai/{}/complete",
                    cancel_started.session_id
                )))
                .body(Body::empty())
                .expect("request"),
            )
            .await
            .expect("cancelled complete response");
        assert_eq!(cancelled_complete.status(), StatusCode::BAD_REQUEST);
        assert_eq!(runtime.provider_connections().expect("providers").len(), 1);
    }

    #[tokio::test]
    async fn authenticated_create_task_is_committed_and_queryable() {
        let runtime = test_runtime();
        let app = router(runtime.clone());
        let body = serde_json::to_vec(&CreateTaskRequest {
            goal: "durable API task".to_owned(),
            project_id: None,
            thread_id: None,
            constraints: Vec::new(),
            success_conditions: vec!["survives restart".to_owned()],
            risk: None,
            budget: None,
        })
        .expect("request should serialize");
        let response = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/tasks"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let created: TaskMutationResponse =
            serde_json::from_slice(&bytes).expect("response should deserialize");
        assert_eq!(
            runtime.task(created.task.id).expect("task should read"),
            Some(created.task.clone())
        );
        let listed = app
            .oneshot(
                authorized(Request::get("/api/v1/tasks"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list response");
        assert_eq!(listed.status(), StatusCode::OK);
        let listed: TaskListResponse =
            serde_json::from_slice(&listed.into_body().collect().await.expect("body").to_bytes())
                .expect("task list");
        assert_eq!(listed.tasks, vec![created.task]);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One public launch/replay/retry/cancel scenario is intentional evidence.
    async fn consumer_launch_is_durable_idempotent_and_reports_missing_provider() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(
            project_root.join("package.json"),
            r#"{"scripts":{"build":"vite build","test":"vitest run"}}"#,
        )
        .expect("project fixture");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let runtime = Arc::new(
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        let mut task = TaskEnvelope::new("Build and test the fixture");
        task.project_id = Some(project.project_id);
        runtime.create_task(&task).expect("create task");
        let app = router(runtime.clone());
        let body = serde_json::to_vec(&LaunchTaskRequest {
            target_path: None,
            context_token_budget: 2_048,
            requested_tools: Vec::new(),
            estimated_output_tokens: 512,
            max_model_calls: 2,
        })
        .expect("launch body");
        let launch_path = format!("/api/v1/tasks/{}/launch", task.id);
        let response = app
            .clone()
            .oneshot(
                authorized(Request::post(&launch_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("launch response");
        assert_eq!(response.status(), StatusCode::OK);
        let launched: LaunchTaskResponse = serde_json::from_slice(
            &response
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("launch response body");
        assert!(!launched.replayed);
        assert_eq!(launched.task.state, TaskState::Blocked);
        assert_eq!(launched.run.run.state, AgentRunState::Running);
        assert_eq!(launched.run.plan.items.len(), 2);
        assert!(matches!(
            launched.execution,
            LaunchExecution::Blocked { .. }
        ));
        assert!(launched.run.plan.items.iter().any(|item| {
            item.success_conditions
                .iter()
                .any(|condition| matches!(condition, SuccessCondition::Test { .. }))
        }));

        let replay = app
            .oneshot(
                authorized(Request::post(&launch_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("replay response");
        let replayed: LaunchTaskResponse =
            serde_json::from_slice(&replay.into_body().collect().await.expect("body").to_bytes())
                .expect("replay body");
        assert!(replayed.replayed);
        assert_eq!(replayed.run.run.id, launched.run.run.id);
        assert!(matches!(
            replayed.execution,
            LaunchExecution::Blocked { .. }
        ));
        assert_eq!(runtime.organizations().expect("organizations").len(), 1);
        let retried = router(runtime.clone())
            .oneshot(
                authorized(Request::post(format!("/api/v1/tasks/{}/retry", task.id)))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LaunchTaskRequest {
                            target_path: None,
                            context_token_budget: 2_048,
                            requested_tools: Vec::new(),
                            estimated_output_tokens: 512,
                            max_model_calls: 2,
                        })
                        .expect("retry body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("retry response");
        let retried: LaunchTaskResponse = serde_json::from_slice(
            &retried
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("retry response body");
        assert_eq!(retried.run.run.id, launched.run.run.id);
        assert_eq!(retried.task.state, TaskState::Blocked);
        assert!(matches!(retried.execution, LaunchExecution::Blocked { .. }));
        let cancelled = router(runtime.clone())
            .oneshot(
                authorized(Request::post(format!("/api/v1/tasks/{}", task.id)))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"state":"cancelled"}"#))
                    .expect("request"),
            )
            .await
            .expect("cancel response");
        assert_eq!(cancelled.status(), StatusCode::OK);
        assert_eq!(
            runtime
                .agent_run_view(launched.run.run.id)
                .expect("cancelled run")
                .run
                .state,
            AgentRunState::Cancelled
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Public launch/approve/resume composition is one scenario.
    async fn consumer_approval_resume_and_rejection_settle_authority_safely() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(project_root.join("README.md"), "# Approval fixture\n")
            .expect("project fixture");
        let mut config = AppConfig::default().with_data_directory(directory.path().join("data"));
        config.autonomy = AutonomyPreset::Guided;
        let runtime = Arc::new(
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        let target = std::path::PathBuf::from(&project.root).join("approved.txt");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "approval_resume_fixture".into(),
                    display_name: "Approval Resume Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: approval_resume_fixture(&target).await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");
        let mut task = TaskEnvelope::new("Create the approved fixture file");
        task.project_id = Some(project.project_id);
        task.success_conditions = vec!["approved.txt exists with the requested content".into()];
        runtime.create_task(&task).expect("task");
        let body = serde_json::to_vec(&LaunchTaskRequest {
            target_path: None,
            context_token_budget: 2_048,
            requested_tools: Vec::new(),
            estimated_output_tokens: 512,
            max_model_calls: 4,
        })
        .expect("launch body");
        let app = router(runtime.clone());
        let launch_path = format!("/api/v1/tasks/{}/launch", task.id);
        let launched = app
            .clone()
            .oneshot(
                authorized(Request::post(&launch_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("launch response");
        let launched: LaunchTaskResponse = serde_json::from_slice(
            &launched
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("launch body");
        assert_eq!(
            launched.task.state,
            TaskState::WaitingForApproval,
            "unexpected launch execution: {:?}",
            launched.execution
        );
        assert_eq!(launched.run.run.state, AgentRunState::Waiting);
        assert!(matches!(
            launched.execution,
            LaunchExecution::AwaitingApproval { .. }
        ));
        assert!(!target.exists());
        let approvals = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("pending approvals");
        assert_eq!(approvals.len(), 1);
        let approval_path = format!("/api/v1/approvals/{}/resolve", approvals[0].id);
        let approved = app
            .clone()
            .oneshot(
                authorized(Request::post(approval_path))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"state":"approved","reason":"fixture owner approval"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("approval response");
        assert_eq!(approved.status(), StatusCode::OK);

        let resumed = app
            .clone()
            .oneshot(
                authorized(Request::post(format!("/api/v1/tasks/{}/resume", task.id)))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("resume response");
        assert_eq!(resumed.status(), StatusCode::OK);
        let resumed: LaunchTaskResponse = serde_json::from_slice(
            &resumed
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("resume body");
        assert!(matches!(
            resumed.execution,
            LaunchExecution::AwaitingEvaluation { .. }
        ));
        assert_eq!(resumed.run.results.len(), 2);
        assert_eq!(
            std::fs::read_to_string(&target).expect("approved side effect"),
            "approval continuation succeeded\n"
        );
        assert_eq!(
            runtime
                .approvals(Some(ApprovalState::Approved))
                .expect("approved records")
                .len(),
            1
        );

        let replayed = app
            .clone()
            .oneshot(
                authorized(Request::post(&launch_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("replay response");
        let replayed: LaunchTaskResponse = serde_json::from_slice(
            &replayed
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes(),
        )
        .expect("replay body");
        assert!(replayed.replayed);
        assert!(matches!(
            replayed.execution,
            LaunchExecution::AwaitingEvaluation { .. }
        ));
        assert_eq!(
            std::fs::read_to_string(&target).expect("single approved side effect"),
            "approval continuation succeeded\n"
        );

        std::fs::remove_file(&target).expect("reset side effect fixture");
        let mut denied_task = TaskEnvelope::new("Do not write without permission");
        denied_task.project_id = Some(project.project_id);
        denied_task.success_conditions = vec!["approved.txt must remain absent when denied".into()];
        runtime.create_task(&denied_task).expect("denied task");
        let denied_launch_path = format!("/api/v1/tasks/{}/launch", denied_task.id);
        let denied_launch = app
            .clone()
            .oneshot(
                authorized(Request::post(&denied_launch_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("denied launch response");
        assert_eq!(denied_launch.status(), StatusCode::OK);
        assert_eq!(
            runtime
                .task(denied_task.id)
                .expect("denied task read")
                .expect("denied task present")
                .state,
            TaskState::WaitingForApproval
        );
        let pending = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("denied pending approval");
        assert_eq!(pending.len(), 1);
        let rejected_id = pending[0].id;
        let rejected = app
            .clone()
            .oneshot(
                authorized(Request::post(format!(
                    "/api/v1/approvals/{rejected_id}/resolve"
                )))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"state":"denied","reason":"fixture owner denied"}"#,
                ))
                .expect("request"),
            )
            .await
            .expect("deny response");
        assert_eq!(rejected.status(), StatusCode::OK);
        assert!(!target.exists());
        let denied_projection = runtime
            .task(denied_task.id)
            .expect("denied task read")
            .expect("denied task present");
        assert_eq!(denied_projection.state, TaskState::Blocked);
        let denied_run_id = runtime
            .task_launch_run_id(denied_task.id)
            .expect("denied run lookup")
            .expect("denied run");
        let denied_view = runtime.agent_run_view(denied_run_id).expect("denied view");
        assert_eq!(denied_view.run.state, AgentRunState::Running);
        assert!(
            denied_view
                .instances
                .iter()
                .any(|instance| instance.state == AgentState::Failed)
        );
        assert!(
            denied_view
                .leases
                .iter()
                .any(|lease| lease.state == mcg_agents::AgentLeaseState::Failed)
        );
        assert!(
            runtime
                .approvals(Some(ApprovalState::Pending))
                .expect("no stale pending approval")
                .is_empty()
        );

        let retried = app
            .oneshot(
                authorized(Request::post(format!(
                    "/api/v1/tasks/{}/retry",
                    denied_task.id
                )))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request"),
            )
            .await
            .expect("retry response");
        assert_eq!(retried.status(), StatusCode::OK);
        assert!(!target.exists());
        let retry_approval = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("retry pending approval");
        assert_eq!(retry_approval.len(), 1);
        assert_ne!(retry_approval[0].id, rejected_id);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Success, replay, and failure share one real process-evidence fixture.
    async fn consumer_objective_checks_atomically_settle_success_and_failure() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(project_root.join("README.md"), "# Objective fixture\n").expect("readme");
        std::fs::write(
            project_root.join("package.json"),
            r#"{"scripts":{"build":"node build.mjs","test":"node test.mjs","check":"node check.mjs"}}"#,
        )
        .expect("package manifest");
        std::fs::write(
            project_root.join("build.mjs"),
            "import { mkdirSync } from 'node:fs'; mkdirSync('dist', { recursive: true });\n",
        )
        .expect("build script");
        std::fs::write(
            project_root.join("test.mjs"),
            "import { existsSync } from 'node:fs'; if (!existsSync('README.md')) process.exit(2);\n",
        )
        .expect("test script");
        std::fs::write(project_root.join("check.mjs"), "process.exit(0);\n").expect("check script");
        let runtime = Arc::new(
            AppRuntime::open_in_memory(
                AppConfig::default().with_data_directory(directory.path().join("data")),
                TEST_TOKEN.to_owned(),
            )
            .expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        assert_eq!(project.commands.len(), 3);
        let model_calls = Arc::new(AtomicUsize::new(0));
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "objective_fixture".into(),
                    display_name: "Objective Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: objective_model_fixture(model_calls.clone()).await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");
        let body = serde_json::to_vec(&LaunchTaskRequest {
            target_path: None,
            context_token_budget: 2_048,
            requested_tools: Vec::new(),
            estimated_output_tokens: 512,
            max_model_calls: 4,
        })
        .expect("launch body");
        let mut passing_task = TaskEnvelope::new("Run the objective project gates");
        passing_task.project_id = Some(project.project_id);
        runtime.create_task(&passing_task).expect("passing task");
        let app = router(runtime.clone());
        let passing_path = format!("/api/v1/tasks/{}/launch", passing_task.id);
        let passing = app
            .clone()
            .oneshot(
                authorized(Request::post(&passing_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("passing response");
        assert_eq!(passing.status(), StatusCode::OK);
        let passing: LaunchTaskResponse = serde_json::from_slice(
            &passing
                .into_body()
                .collect()
                .await
                .expect("passing body")
                .to_bytes(),
        )
        .expect("passing response body");
        assert_eq!(passing.task.state, TaskState::Completed);
        assert_eq!(passing.run.run.state, AgentRunState::Completed);
        assert!(matches!(
            passing.execution,
            LaunchExecution::Completed { .. }
        ));
        assert_eq!(passing.run.evaluations.len(), 1);
        assert_eq!(
            passing.run.evaluations[0].outcome,
            EvaluationOutcome::Passed
        );
        assert_eq!(passing.run.evaluations[0].conditions.len(), 3);
        assert!(
            passing.run.evaluations[0]
                .conditions
                .iter()
                .all(|condition| condition.passed)
        );
        assert!(project_root.join("dist").is_dir());
        assert_eq!(model_calls.load(Ordering::SeqCst), 2);
        let passing_audit = runtime.tool_audit(passing_task.id).expect("passing audit");
        assert_eq!(
            passing_audit
                .iter()
                .filter(|record| record.phase == ToolAuditPhase::Completion)
                .count(),
            3
        );

        let replay = app
            .clone()
            .oneshot(
                authorized(Request::post(&passing_path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("passing replay");
        let replay: LaunchTaskResponse = serde_json::from_slice(
            &replay
                .into_body()
                .collect()
                .await
                .expect("replay body")
                .to_bytes(),
        )
        .expect("replay response body");
        assert!(replay.replayed);
        assert!(matches!(
            replay.execution,
            LaunchExecution::Completed { .. }
        ));
        assert_eq!(model_calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            runtime.tool_audit(passing_task.id).expect("replay audit"),
            passing_audit
        );

        std::fs::write(project_root.join("test.mjs"), "process.exit(7);\n")
            .expect("failing test script");
        let mut failing_task = TaskEnvelope::new("Prove a failing objective gate blocks release");
        failing_task.project_id = Some(project.project_id);
        runtime.create_task(&failing_task).expect("failing task");
        let failing = app
            .oneshot(
                authorized(Request::post(format!(
                    "/api/v1/tasks/{}/launch",
                    failing_task.id
                )))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request"),
            )
            .await
            .expect("failing response");
        assert_eq!(failing.status(), StatusCode::OK);
        let failing: LaunchTaskResponse = serde_json::from_slice(
            &failing
                .into_body()
                .collect()
                .await
                .expect("failing body")
                .to_bytes(),
        )
        .expect("failing response body");
        assert_eq!(failing.task.state, TaskState::Failed);
        assert_eq!(failing.run.run.state, AgentRunState::Failed);
        assert!(matches!(failing.execution, LaunchExecution::Failed { .. }));
        assert_eq!(failing.run.evaluations.len(), 1);
        assert_eq!(
            failing.run.evaluations[0].outcome,
            EvaluationOutcome::Failed
        );
        assert!(
            failing.run.evaluations[0]
                .conditions
                .iter()
                .any(|condition| !condition.passed)
        );
        assert!(
            failing.run.evaluations[0]
                .evidence
                .iter()
                .all(|evidence| evidence.method == "governed_process_exit")
        );
        assert_eq!(model_calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Deny/retry/restart/approve/resume is one durable public scenario.
    async fn guided_objective_batch_approval_survives_restart_and_resumes_once() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(
            project_root.join("README.md"),
            "# Guided objective fixture\n",
        )
        .expect("readme");
        std::fs::write(
            project_root.join("package.json"),
            r#"{"scripts":{"build":"node build.mjs","test":"node test.mjs","check":"node check.mjs"}}"#,
        )
        .expect("package manifest");
        std::fs::write(
            project_root.join("build.mjs"),
            "import { mkdirSync } from 'node:fs'; mkdirSync('dist', { recursive: true });\n",
        )
        .expect("build script");
        std::fs::write(project_root.join("test.mjs"), "process.exit(0);\n").expect("test script");
        std::fs::write(project_root.join("check.mjs"), "process.exit(0);\n").expect("check script");
        let mut config = AppConfig::default().with_data_directory(directory.path().join("data"));
        config.autonomy = AutonomyPreset::Guided;
        let runtime = Arc::new(AppRuntime::open(config.clone()).expect("durable runtime"));
        let token = std::fs::read_to_string(config.auth_token_path())
            .expect("auth token")
            .trim()
            .to_owned();
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        let model_calls = Arc::new(AtomicUsize::new(0));
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "guided_objective_fixture".into(),
                    display_name: "Guided Objective Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: objective_model_fixture(model_calls.clone()).await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");
        let mut task = TaskEnvelope::new("Run approved objective gates once");
        task.project_id = Some(project.project_id);
        runtime.create_task(&task).expect("task");
        let body = serde_json::to_vec(&LaunchTaskRequest {
            target_path: None,
            context_token_budget: 2_048,
            requested_tools: Vec::new(),
            estimated_output_tokens: 512,
            max_model_calls: 4,
        })
        .expect("launch body");
        let app = router(runtime.clone());
        let task_path = format!("/api/v1/tasks/{}/launch", task.id);
        let waiting = app
            .clone()
            .oneshot(
                Request::post(&task_path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("guided launch");
        assert_eq!(waiting.status(), StatusCode::OK);
        let waiting: LaunchTaskResponse = serde_json::from_slice(
            &waiting
                .into_body()
                .collect()
                .await
                .expect("waiting body")
                .to_bytes(),
        )
        .expect("waiting response");
        assert_eq!(waiting.task.state, TaskState::WaitingForApproval);
        assert!(matches!(
            waiting.execution,
            LaunchExecution::AwaitingApproval { .. }
        ));
        assert_eq!(model_calls.load(Ordering::SeqCst), 2);
        let first_approval = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("first approval")
            .pop()
            .expect("pending batch approval");
        assert_eq!(first_approval.action, "evaluation.run_discovered_checks");
        let checkpoint = runtime
            .consumer_evaluation_checkpoint(task.id)
            .expect("checkpoint read")
            .expect("waiting checkpoint");
        assert_eq!(checkpoint.next_condition_index, 0);
        assert_eq!(checkpoint.attempt, 0);

        let denied = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/approvals/{}/resolve", first_approval.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"state":"denied","reason":"exercise fresh retry authority"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("deny response");
        assert_eq!(denied.status(), StatusCode::OK);
        assert_eq!(
            runtime
                .task(task.id)
                .expect("task read")
                .expect("task present")
                .state,
            TaskState::Blocked
        );
        let blocked_checkpoint = runtime
            .consumer_evaluation_checkpoint(task.id)
            .expect("blocked checkpoint")
            .expect("checkpoint present");
        assert_eq!(blocked_checkpoint.attempt, 1);

        let retried = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/tasks/{}/retry", task.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("retry response");
        assert_eq!(retried.status(), StatusCode::OK);
        assert_eq!(model_calls.load(Ordering::SeqCst), 2);
        let second_approval = runtime
            .approvals(Some(ApprovalState::Pending))
            .expect("retry approval")
            .pop()
            .expect("fresh pending approval");
        assert_ne!(second_approval.id, first_approval.id);
        assert_ne!(
            second_approval.tool_request_id,
            first_approval.tool_request_id
        );

        let approved = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/approvals/{}/resolve", second_approval.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"state":"approved","reason":"approve exact discovered checks"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("approve response");
        assert_eq!(approved.status(), StatusCode::OK);
        let run_id = waiting.run.run.id;
        drop(app);
        drop(runtime);

        let reopened = Arc::new(AppRuntime::open(config).expect("reopen durable runtime"));
        assert_eq!(
            reopened
                .agent_run_view(run_id)
                .expect("recovered run")
                .run
                .state,
            AgentRunState::Recovering
        );
        assert_eq!(
            reopened
                .task(task.id)
                .expect("recovered task")
                .expect("task present")
                .state,
            TaskState::WaitingForApproval
        );
        let resumed = router(reopened.clone())
            .oneshot(
                Request::post(format!("/api/v1/tasks/{}/resume", task.id))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("resume response");
        assert_eq!(resumed.status(), StatusCode::OK);
        let resumed: LaunchTaskResponse = serde_json::from_slice(
            &resumed
                .into_body()
                .collect()
                .await
                .expect("resumed body")
                .to_bytes(),
        )
        .expect("resumed response");
        assert_eq!(resumed.task.state, TaskState::Completed);
        assert_eq!(resumed.run.run.state, AgentRunState::Completed);
        assert!(matches!(
            resumed.execution,
            LaunchExecution::Completed { .. }
        ));
        assert_eq!(resumed.run.evaluations.len(), 1);
        assert_eq!(resumed.run.evaluations[0].conditions.len(), 3);
        assert_eq!(model_calls.load(Ordering::SeqCst), 2);
        assert!(
            reopened
                .consumer_evaluation_checkpoint(task.id)
                .expect("terminal checkpoint read")
                .is_none()
        );
        assert_eq!(
            reopened
                .tool_audit(task.id)
                .expect("objective audit")
                .iter()
                .filter(|record| record.phase == ToolAuditPhase::Completion)
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn event_stream_emits_committed_task_event() {
        let runtime = test_runtime();
        let response = router(runtime.clone())
            .oneshot(
                authorized(Request::get("/api/v1/events/stream"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        runtime
            .create_task(&TaskEnvelope::new("stream event"))
            .expect("task should commit");

        let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
            .await
            .expect("stream should respond")
            .expect("stream should contain frame")
            .expect("frame should be valid");
        let data = frame.into_data().expect("frame should contain data");
        let text = String::from_utf8(data.to_vec()).expect("SSE should be UTF-8");
        assert!(text.contains("event: task_created"));
    }

    #[tokio::test]
    async fn event_stream_replays_durable_events_after_sequence() {
        let runtime = test_runtime();
        runtime
            .create_task(&TaskEnvelope::new("replay event"))
            .expect("task should commit");
        let response = router(runtime)
            .oneshot(
                authorized(Request::get("/api/v1/events/stream?after=0"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let mut body = response.into_body();
        let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
            .await
            .expect("replay should respond")
            .expect("replay should contain frame")
            .expect("frame should be valid");
        let text = String::from_utf8(
            frame
                .into_data()
                .expect("frame should contain data")
                .to_vec(),
        )
        .expect("SSE should be UTF-8");
        assert!(text.contains("id: 1"));
        assert!(text.contains("event: task_created"));
    }

    #[tokio::test]
    async fn non_loopback_binding_is_rejected() {
        let address: SocketAddr = "0.0.0.0:7777".parse().expect("address");
        let error = serve(address, test_runtime())
            .await
            .expect_err("public binding must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn approval_lifecycle_is_authenticated_durable_and_conflict_safe() {
        let runtime = test_runtime();
        let task = TaskEnvelope::new("approval fixture");
        runtime.create_task(&task).expect("task");
        let approval = ApprovalRequest {
            schema_version: TOOL_SCHEMA_VERSION,
            id: ApprovalId::new(),
            tool_request_id: ToolRequestId::new(),
            task_id: task.id,
            action: "fs.delete".into(),
            resource: "project/generated".into(),
            reason: "destructive action".into(),
            state: ApprovalState::Pending,
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        runtime
            .tool_control()
            .create(approval.clone())
            .await
            .expect("create approval");
        let app = router(runtime);
        let list = app
            .clone()
            .oneshot(
                authorized(Request::get("/api/v1/approvals?state=pending"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list");
        assert_eq!(list.status(), StatusCode::OK);
        let list_bytes = list.into_body().collect().await.expect("body").to_bytes();
        let listed: ApprovalListResponse = serde_json::from_slice(&list_bytes).expect("json");
        assert_eq!(listed.approvals, vec![approval.clone()]);

        let body = serde_json::to_vec(&ResolveApprovalRequest {
            state: ApprovalState::Approved,
            reason: Some("owner approved".into()),
        })
        .expect("body");
        let path = format!("/api/v1/approvals/{}/resolve", approval.id);
        let resolved = app
            .clone()
            .oneshot(
                authorized(Request::post(&path))
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("resolve");
        assert_eq!(resolved.status(), StatusCode::OK);
        let duplicate = app
            .oneshot(
                authorized(Request::post(&path))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("duplicate");
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn task_tool_audit_endpoint_returns_immutable_broker_evidence() {
        let runtime = test_runtime();
        let task = TaskEnvelope::new("audit fixture");
        runtime.create_task(&task).expect("task");
        let record = ToolAuditRecord {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id: ToolExecutionId::new(),
            request_id: ToolRequestId::new(),
            task_id: task.id,
            tool: "fs.read".into(),
            resource: "project/file".into(),
            request_fingerprint: "a".repeat(64),
            phase: ToolAuditPhase::Completion,
            decision: "execution completed".into(),
            result: Some(ToolOutcomeStatus::Succeeded),
            artifacts: vec![mcg_tools::ArtifactReference {
                id: ArtifactId::new(),
                sha256: "b".repeat(64),
                media_type: "text/plain".into(),
                size_bytes: 1,
                relative_path: "bb/value".into(),
            }],
            verification: Vec::new(),
            recorded_at: Utc::now(),
        };
        runtime
            .tool_control()
            .record(record.clone())
            .await
            .expect("audit");
        let response = router(runtime)
            .oneshot(
                authorized(Request::get(format!(
                    "/api/v1/tasks/{}/tool-audit",
                    task.id
                )))
                .body(Body::empty())
                .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let audit: ToolAuditResponse = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(audit.records, vec![record]);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn project_memory_context_lifecycle_is_authenticated_and_conflict_safe() {
        let project_root = tempfile::tempdir().expect("project root");
        std::fs::write(
            project_root.path().join("package.json"),
            r#"{"scripts":{"build":"vite build","test":"vitest"},"devDependencies":{"vitest":"1"}}"#,
        )
        .expect("manifest");
        std::fs::write(
            project_root.path().join("AGENTS.md"),
            "Use npm for package management.",
        )
        .expect("instructions");
        let app = router(test_runtime());
        let discover_body = serde_json::to_vec(&DiscoverProjectRequest {
            root: project_root.path().to_string_lossy().into_owned(),
            project_id: None,
        })
        .expect("discover body");
        let discovered = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/projects"))
                    .header("content-type", "application/json")
                    .body(Body::from(discover_body))
                    .expect("request"),
            )
            .await
            .expect("discover");
        assert_eq!(discovered.status(), StatusCode::CREATED);
        let bytes = discovered
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let project: ProjectMutation = serde_json::from_slice(&bytes).expect("project response");
        assert!(project.project.test_systems.contains(&"Vitest".to_owned()));

        let provenance = ClaimProvenance {
            source_kind: mcg_projects::ProvenanceKind::User,
            source: "API fixture".into(),
            task_id: None,
            artifact_sha256: None,
        };
        let create_body = serde_json::to_vec(&CreateMemoryClaimRequest {
            namespace: "project".into(),
            key: "package_manager".into(),
            value: serde_json::json!("npm"),
            confidence: 0.8,
            provenance: provenance.clone(),
            fresh_until: None,
        })
        .expect("claim body");
        let memory_path = format!("/api/v1/projects/{}/memory", project.project.project_id);
        let created = app
            .clone()
            .oneshot(
                authorized(Request::post(&memory_path))
                    .header("content-type", "application/json")
                    .body(Body::from(create_body))
                    .expect("request"),
            )
            .await
            .expect("create memory");
        assert_eq!(created.status(), StatusCode::CREATED);
        let bytes = created
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let original: MemoryClaimMutation =
            serde_json::from_slice(&bytes).expect("memory response");

        let correct_body = serde_json::to_vec(&CorrectMemoryClaimRequest {
            value: serde_json::json!("pnpm"),
            confidence: 1.0,
            provenance,
            fresh_until: None,
        })
        .expect("correct body");
        let correct_path = format!(
            "/api/v1/projects/{}/memory/{}/correct",
            project.project.project_id, original.claim.id
        );
        let corrected = app
            .clone()
            .oneshot(
                authorized(Request::post(&correct_path))
                    .header("content-type", "application/json")
                    .body(Body::from(correct_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("correct memory");
        assert_eq!(corrected.status(), StatusCode::OK);
        let bytes = corrected
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let corrected: MemoryClaimMutation =
            serde_json::from_slice(&bytes).expect("corrected response");
        let duplicate = app
            .clone()
            .oneshot(
                authorized(Request::post(&correct_path))
                    .header("content-type", "application/json")
                    .body(Body::from(correct_body))
                    .expect("request"),
            )
            .await
            .expect("duplicate correction");
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);

        let transition_body = serde_json::to_vec(&TransitionMemoryClaimRequest {
            action: MemoryLifecycleAction::Reinforce,
            provenance: ClaimProvenance {
                source_kind: mcg_projects::ProvenanceKind::User,
                source: "reinforcement fixture".into(),
                task_id: None,
                artifact_sha256: None,
            },
        })
        .expect("transition body");
        let transition_path = format!(
            "/api/v1/projects/{}/memory/{}/transition",
            project.project.project_id, corrected.claim.id
        );
        let transitioned = app
            .clone()
            .oneshot(
                authorized(Request::post(&transition_path))
                    .header("content-type", "application/json")
                    .body(Body::from(transition_body))
                    .expect("request"),
            )
            .await
            .expect("transition memory");
        assert_eq!(transitioned.status(), StatusCode::OK);

        let context_path = format!("/api/v1/projects/{}/context", project.project.project_id);
        let context_body = serde_json::to_vec(&AssembleContextRequest {
            items: vec![ContextItem {
                category: mcg_projects::ContextCategory::Goal,
                content: "Build the project".into(),
                source: "owner".into(),
                priority: 1_000,
                estimated_tokens: 0,
            }],
            token_budget: 128,
            target_path: None,
        })
        .expect("context body");
        let context = app
            .clone()
            .oneshot(
                authorized(Request::post(&context_path))
                    .header("content-type", "application/json")
                    .body(Body::from(context_body))
                    .expect("request"),
            )
            .await
            .expect("context");
        assert_eq!(context.status(), StatusCode::OK);
        let bytes = context
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let context: ContextAssembly = serde_json::from_slice(&bytes).expect("context response");
        assert!(
            context
                .items
                .iter()
                .any(|item| item.content.contains("pnpm"))
        );

        let history_path = format!(
            "/api/v1/projects/{}/memory/history?namespace=project&key=package_manager",
            project.project.project_id
        );
        let history = app
            .oneshot(
                authorized(Request::get(&history_path))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("history");
        assert_eq!(history.status(), StatusCode::OK);
        let bytes = history
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let history: MemoryClaimHistoryResponse =
            serde_json::from_slice(&bytes).expect("history response");
        assert_eq!(history.claims.len(), 3);
        assert_eq!(history.claims[2].value, serde_json::json!("pnpm"));
        assert_eq!(history.claims[2].usefulness_count, 1);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Complete authenticated organization/run HTTP workflow.
    async fn organization_and_dependency_plan_are_authenticated_durable_and_inspectable() {
        let runtime = test_runtime();
        let app = router(runtime.clone());
        let organization_response = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/organizations"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Consumer Fixture"}"#))
                    .expect("request"),
            )
            .await
            .expect("organization response");
        assert_eq!(organization_response.status(), StatusCode::CREATED);
        let bytes = organization_response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let organization: OrganizationMutationResponse =
            serde_json::from_slice(&bytes).expect("organization body");
        assert_eq!(organization.organization.name, "Consumer Fixture");

        let mut task = TaskEnvelope::new("Build then test a fixture site");
        task.budget = TaskBudget {
            max_tokens: Some(20_000),
            max_cost_usd: Some(4.0),
            max_duration_seconds: Some(900),
        };
        runtime.create_task(&task).expect("task");
        let plan_body = serde_json::to_vec(&PlanAgentRunRequest {
            organization_revision_id: organization.organization.revision_id,
            complexity: TaskComplexity::Compound,
            work: vec![
                PlanWorkRequest {
                    key: "build".into(),
                    goal: "Build the fixture".into(),
                    role: RoleKind::Coder,
                    dependencies: Vec::new(),
                    success_conditions: vec![SuccessCondition::Build {
                        command: "npm run build".into(),
                    }],
                    risk_notes: Vec::new(),
                    budget_weight: 2,
                    estimated_useful_seconds: 120,
                },
                PlanWorkRequest {
                    key: "test".into(),
                    goal: "Test the fixture".into(),
                    role: RoleKind::ToolWorker,
                    dependencies: vec!["build".into()],
                    success_conditions: vec![SuccessCondition::Test {
                        command: "npm test".into(),
                    }],
                    risk_notes: Vec::new(),
                    budget_weight: 1,
                    estimated_useful_seconds: 60,
                },
            ],
            require_independent_evaluation: true,
        })
        .expect("plan body");
        let response = app
            .clone()
            .oneshot(
                authorized(Request::post(format!("/api/v1/tasks/{}/runs", task.id)))
                    .header("content-type", "application/json")
                    .body(Body::from(plan_body))
                    .expect("request"),
            )
            .await
            .expect("plan response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let view: AgentRunView = serde_json::from_slice(&bytes).expect("run view");
        assert_eq!(view.run.state, AgentRunState::Planned);
        assert_eq!(view.plan.items.len(), 3);
        assert_eq!(
            view.plan
                .items
                .iter()
                .filter(|item| item.dependencies.is_empty())
                .count(),
            1
        );

        let response = app
            .oneshot(
                authorized(Request::post(format!(
                    "/api/v1/runs/{}/transition",
                    view.run.id
                )))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"state":"running"}"#))
                .expect("request"),
            )
            .await
            .expect("start response");
        assert_eq!(response.status(), StatusCode::OK);
        let durable = runtime
            .agent_run_view(view.run.id)
            .expect("durable run view");
        assert_eq!(durable.run.state, AgentRunState::Running);
        assert_eq!(durable.plan, view.plan);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Complete authenticated dispatch/provider/settlement boundary.
    async fn ready_work_executes_only_with_auth_and_commits_terminal_evidence() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(project_root.join("README.md"), "# Fixture").expect("project fixture");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let runtime = Arc::new(
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        let mut task = TaskEnvelope::new("Complete a provider-backed fixture turn");
        task.project_id = Some(project.project_id);
        task.budget = TaskBudget {
            max_tokens: Some(10_000),
            max_cost_usd: Some(2.0),
            max_duration_seconds: Some(300),
        };
        runtime.create_task(&task).expect("task");
        let now = Utc::now();
        let organization = OrganizationGenome::consumer_default(now);
        runtime
            .record_organization_revision(&organization)
            .expect("organization");
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Report fixture completion",
                RoleKind::Conversation,
                vec![SuccessCondition::Custom {
                    description: "provider returns a terminal response".into(),
                }],
            )],
            require_independent_evaluation: false,
        };
        let plan = ExecutivePlanner::default()
            .plan(&organization, &planning, now)
            .expect("work plan");
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: organization.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        runtime.create_agent_run(&run, &plan).expect("run");
        runtime
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
            )
            .expect("start run");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "execution_fixture".into(),
                    display_name: "Execution Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: model_discovery_fixture().await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");
        let endpoint = format!("/api/v1/runs/{}/work/{}/execute", run.id, plan.items[0].id);
        let body = serde_json::to_vec(&ExecuteAgentWorkRequest {
            project_id: project.project_id,
            target_path: None,
            context_token_budget: 512,
            requested_tools: Vec::new(),
            estimated_output_tokens: 128,
            max_model_calls: 2,
        })
        .expect("request body");
        let app = router(runtime.clone());
        let unauthorized = app
            .clone()
            .oneshot(
                Request::post(&endpoint)
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("unauthorized response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let response = app
            .oneshot(
                authorized(Request::post(&endpoint))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("execution response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        let commit: AgentTurnCommit = serde_json::from_slice(&bytes).expect("turn commit");
        assert_eq!(commit.result.response.id, "fixture-response");
        let durable = runtime.agent_run_view(run.id).expect("durable run");
        assert_eq!(durable.instances.len(), 1);
        assert_eq!(
            durable.instances[0].state,
            mcg_agents::AgentState::Completed
        );
        assert_eq!(
            durable.leases[0].state,
            mcg_agents::AgentLeaseState::Completed
        );
        assert_eq!(durable.results, vec![commit.result]);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Full provider -> governed tools -> durable evidence contract.
    async fn website_is_built_and_independently_evaluated_with_durable_evidence() {
        let directory = tempfile::tempdir().expect("temporary runtime");
        let project_root = directory.path().join("launchpad");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::fs::write(project_root.join("README.md"), "# LaunchPad\n").expect("project fixture");
        let config = AppConfig::default().with_data_directory(directory.path().join("data"));
        let runtime = Arc::new(
            AppRuntime::open_in_memory(config, TEST_TOKEN.to_owned()).expect("runtime should open"),
        );
        let project = runtime
            .discover_project(&project_root, ProjectId::new())
            .expect("discover project")
            .project;
        let mut task =
            TaskEnvelope::new("Build an accessible interactive project-planning website");
        task.project_id = Some(project.project_id);
        task.budget = TaskBudget {
            max_tokens: Some(20_000),
            max_cost_usd: Some(2.0),
            max_duration_seconds: Some(300),
        };
        runtime.create_task(&task).expect("task");
        let now = Utc::now();
        let organization = OrganizationGenome::consumer_default(now);
        runtime
            .record_organization_revision(&organization)
            .expect("organization");
        let planning = PlanningRequest {
            run_id: RunId::new(),
            task: task.clone(),
            complexity: TaskComplexity::Atomic,
            work: vec![PlannedWork::atomic(
                "Build the complete LaunchPad website",
                RoleKind::Coder,
                vec![SuccessCondition::Custom {
                    description:
                        "responsive HTML, CSS, and interactive JavaScript are written and verified"
                            .into(),
                }],
            )],
            require_independent_evaluation: true,
        };
        let plan = ExecutivePlanner::default()
            .plan(&organization, &planning, now)
            .expect("work plan");
        let coder_role = organization
            .roles
            .iter()
            .find(|role| role.kind == RoleKind::Coder)
            .expect("coder role")
            .id;
        let evaluator_role = organization
            .roles
            .iter()
            .find(|role| role.kind == RoleKind::Evaluator)
            .expect("evaluator role")
            .id;
        let coder_item = plan
            .items
            .iter()
            .find(|item| item.assigned_role == coder_role)
            .expect("coder work")
            .clone();
        let evaluator_item = plan
            .items
            .iter()
            .find(|item| item.assigned_role == evaluator_role)
            .expect("evaluator work")
            .clone();
        let run = AgentRun {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: planning.run_id,
            task_id: task.id,
            organization_revision_id: organization.revision_id,
            state: AgentRunState::Planned,
            metrics: OrganizationalMetrics::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        runtime.create_agent_run(&run, &plan).expect("run");
        runtime
            .transition_agent_run(
                run.id,
                AgentRunState::Running,
                OrganizationalMetrics::default(),
            )
            .expect("start run");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "website_fixture".into(),
                    display_name: "Website Builder Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: website_builder_fixture(std::path::Path::new(&project.root)).await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");

        let endpoint = format!("/api/v1/runs/{}/work/{}/execute", run.id, coder_item.id);
        let body = serde_json::to_vec(&ExecuteAgentWorkRequest {
            project_id: project.project_id,
            target_path: None,
            context_token_budget: 1_024,
            requested_tools: vec!["fs.write".into()],
            estimated_output_tokens: 512,
            max_model_calls: 2,
        })
        .expect("request body");
        let response = router(runtime.clone())
            .oneshot(
                authorized(Request::post(&endpoint))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("execution response");
        let response_status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        assert_eq!(
            response_status,
            StatusCode::OK,
            "execution failed: {}",
            String::from_utf8_lossy(&bytes)
        );
        let coder_commit: AgentTurnCommit = serde_json::from_slice(&bytes).expect("turn commit");

        assert_eq!(coder_commit.result.response.id, "website-complete");
        assert_eq!(coder_commit.result.model_calls.len(), 2);
        assert_eq!(coder_commit.result.tool_outcomes.len(), 3);
        assert!(coder_commit.result.tool_outcomes.iter().all(|outcome| {
            outcome.status == ToolOutcomeStatus::Succeeded
                && outcome
                    .verification
                    .iter()
                    .any(|evidence| evidence.method == "sha256.read_after_write" && evidence.passed)
        }));
        let html = std::fs::read_to_string(project_root.join("index.html")).expect("website HTML");
        let css = std::fs::read_to_string(project_root.join("styles.css")).expect("website CSS");
        let javascript =
            std::fs::read_to_string(project_root.join("app.js")).expect("website JavaScript");
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("role=\"status\""));
        assert!(html.contains("styles.css") && html.contains("app.js"));
        assert!(css.contains(":focus-visible") && css.contains("clamp("));
        assert!(javascript.contains("addEventListener('submit'"));
        assert!(javascript.contains("document.createElement('li')"));

        let audit = runtime.tool_audit(task.id).expect("tool audit");
        assert_eq!(audit.len(), 6);
        assert_eq!(
            audit
                .iter()
                .filter(|record| record.phase == ToolAuditPhase::Completion)
                .count(),
            3
        );
        assert!(audit.iter().all(|record| record.tool == "fs.write"));

        let evaluator_endpoint =
            format!("/api/v1/runs/{}/work/{}/execute", run.id, evaluator_item.id);
        let evaluator_body = serde_json::to_vec(&ExecuteAgentWorkRequest {
            project_id: project.project_id,
            target_path: None,
            context_token_budget: 1_024,
            requested_tools: Vec::new(),
            estimated_output_tokens: 256,
            max_model_calls: 1,
        })
        .expect("evaluator request");
        let evaluator_response = router(runtime.clone())
            .oneshot(
                authorized(Request::post(&evaluator_endpoint))
                    .header("content-type", "application/json")
                    .body(Body::from(evaluator_body))
                    .expect("evaluator request"),
            )
            .await
            .expect("evaluator response");
        assert_eq!(evaluator_response.status(), StatusCode::OK);
        let evaluator_commit: AgentTurnCommit = serde_json::from_slice(
            &evaluator_response
                .into_body()
                .collect()
                .await
                .expect("evaluator body")
                .to_bytes(),
        )
        .expect("evaluator commit");
        assert_eq!(evaluator_commit.instance.role_id, evaluator_role);
        assert_eq!(evaluator_commit.result.response.id, "evaluation-complete");
        assert!(evaluator_commit.result.tool_outcomes.is_empty());

        let condition = coder_item.success_conditions[0].clone();
        let report = EvaluationReport {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            id: EvaluationId::new(),
            run_id: run.id,
            task_id: task.id,
            evaluator_instance_id: Some(evaluator_commit.instance.id),
            conditions: vec![ConditionResult {
                condition,
                passed: true,
                detail: "three project files and their interaction/accessibility hooks were independently inspected"
                    .into(),
            }],
            outcome: EvaluationOutcome::Passed,
            confidence: mcg_agents::ConfidenceEstimate::new(9_500, 3).expect("confidence"),
            regressions: Vec::new(),
            evidence: coder_commit
                .result
                .tool_outcomes
                .iter()
                .filter_map(|outcome| outcome.verification.first())
                .map(|verification| EvaluationEvidence {
                    method: verification.method.clone(),
                    artifact_sha256: Some(verification.detail.clone()),
                    event_sequence: coder_commit.events.last().map(|event| event.sequence),
                    detail: "governed filesystem write passed read-after-write verification".into(),
                })
                .collect(),
            evaluated_at: Utc::now(),
        };
        let evaluation_endpoint = format!("/api/v1/runs/{}/evaluations", run.id);
        let unauthorized = router(runtime.clone())
            .oneshot(
                Request::post(&evaluation_endpoint)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&report).expect("evaluation report"),
                    ))
                    .expect("unauthorized evaluation request"),
            )
            .await
            .expect("unauthorized evaluation response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let mut missing_evaluator = report.clone();
        missing_evaluator.evaluator_instance_id = None;
        let rejected = router(runtime.clone())
            .oneshot(
                authorized(Request::post(&evaluation_endpoint))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&missing_evaluator).expect("invalid report"),
                    ))
                    .expect("invalid evaluation request"),
            )
            .await
            .expect("invalid evaluation response");
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        assert!(
            runtime
                .agent_run_view(run.id)
                .expect("run")
                .evaluations
                .is_empty()
        );

        let evaluation_response = router(runtime.clone())
            .oneshot(
                authorized(Request::post(&evaluation_endpoint))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&report).expect("evaluation report"),
                    ))
                    .expect("evaluation request"),
            )
            .await
            .expect("evaluation response");
        assert_eq!(evaluation_response.status(), StatusCode::CREATED);
        let mutation: EvaluationMutationResponse = serde_json::from_slice(
            &evaluation_response
                .into_body()
                .collect()
                .await
                .expect("evaluation body")
                .to_bytes(),
        )
        .expect("evaluation mutation");
        assert_eq!(mutation.report, report);

        let durable = runtime.agent_run_view(run.id).expect("durable run");
        assert_eq!(durable.results.len(), 2);
        assert!(
            durable
                .results
                .iter()
                .any(|result| result == &coder_commit.result)
        );
        assert!(
            durable
                .results
                .iter()
                .any(|result| result == &evaluator_commit.result)
        );
        assert_eq!(durable.evaluations, vec![report]);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Complete external provider compatibility contract.
    async fn openai_provider_proxy_is_authenticated_routed_and_streamable() {
        let runtime = test_runtime();
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "proxy_fixture".into(),
                    display_name: "Proxy Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: model_discovery_fixture().await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("provider");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "tool_proxy_fixture".into(),
                    display_name: "Caller Tool Proxy Fixture".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: caller_tool_fixture().await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("tool provider");
        let app = router(runtime);

        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/v1/models")
                    .body(Body::empty())
                    .expect("models request"),
            )
            .await
            .expect("unauthorized models");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let models = app
            .clone()
            .oneshot(
                authorized(Request::get("/v1/models"))
                    .body(Body::empty())
                    .expect("models request"),
            )
            .await
            .expect("models response");
        assert_eq!(models.status(), StatusCode::OK);
        let models: serde_json::Value = serde_json::from_slice(
            &models
                .into_body()
                .collect()
                .await
                .expect("models body")
                .to_bytes(),
        )
        .expect("models json");
        assert_eq!(models.pointer("/object"), Some(&serde_json::json!("list")));
        assert!(
            models
                .pointer("/data")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|models| models.iter().any(|model| {
                    model.get("id").and_then(serde_json::Value::as_str) == Some("mcg:auto")
                        && model.pointer("/metadata/available") == Some(&serde_json::json!(true))
                }))
        );

        let buffered = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"model":"mcg:auto","input":"hello"}).to_string(),
                    ))
                    .expect("responses request"),
            )
            .await
            .expect("responses response");
        assert_eq!(buffered.status(), StatusCode::OK);
        let buffered: serde_json::Value = serde_json::from_slice(
            &buffered
                .into_body()
                .collect()
                .await
                .expect("responses body")
                .to_bytes(),
        )
        .expect("responses json");
        assert_eq!(
            buffered.pointer("/model"),
            Some(&serde_json::json!("mcg:auto"))
        );
        assert_eq!(
            buffered.pointer("/mcg_route/provider_id"),
            Some(&serde_json::json!("proxy_fixture"))
        );
        assert_eq!(
            buffered.pointer("/usage/total_tokens"),
            Some(&serde_json::json!(36))
        );

        let chat = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/chat/completions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model":"mcg:local",
                            "messages":[{"role":"user","content":"hello"}]
                        })
                        .to_string(),
                    ))
                    .expect("chat request"),
            )
            .await
            .expect("chat response");
        assert_eq!(chat.status(), StatusCode::OK);
        let chat: serde_json::Value = serde_json::from_slice(
            &chat
                .into_body()
                .collect()
                .await
                .expect("chat body")
                .to_bytes(),
        )
        .expect("chat json");
        assert_eq!(
            chat.pointer("/object"),
            Some(&serde_json::json!("chat.completion"))
        );
        assert_eq!(
            chat.pointer("/choices/0/message/content"),
            Some(&serde_json::json!("fixture complete"))
        );

        let streaming = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model":"mcg:auto",
                            "input":"stream",
                            "stream":true
                        })
                        .to_string(),
                    ))
                    .expect("stream request"),
            )
            .await
            .expect("stream response");
        assert_eq!(streaming.status(), StatusCode::OK);
        assert_eq!(
            streaming
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let streaming = String::from_utf8(
            streaming
                .into_body()
                .collect()
                .await
                .expect("stream body")
                .to_bytes()
                .to_vec(),
        )
        .expect("stream utf8");
        assert!(streaming.contains("event: response.output_text.delta"));
        assert!(streaming.contains("event: response.completed"));
        assert!(streaming.contains("mcg:auto"));

        let caller_tool = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model":"mcg:auto",
                            "input":"look it up",
                            "tools":[{
                                "type":"function",
                                "name":"caller::lookup",
                                "description":"A caller-owned lookup",
                                "parameters":{"type":"object","properties":{}}
                            }]
                        })
                        .to_string(),
                    ))
                    .expect("caller tool request"),
            )
            .await
            .expect("caller tool response");
        assert_eq!(caller_tool.status(), StatusCode::OK);
        let caller_tool: serde_json::Value = serde_json::from_slice(
            &caller_tool
                .into_body()
                .collect()
                .await
                .expect("caller tool body")
                .to_bytes(),
        )
        .expect("caller tool json");
        assert_eq!(
            caller_tool.pointer("/output/0/name"),
            Some(&serde_json::json!("caller::lookup"))
        );
        assert_eq!(
            caller_tool.pointer("/mcg_route/provider_id"),
            Some(&serde_json::json!("tool_proxy_fixture"))
        );

        let reserved_tool = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model":"mcg:auto",
                            "input":"read host",
                            "tools":[{
                                "type":"function",
                                "name":"host::fs.read",
                                "parameters":{"type":"object"}
                            }]
                        })
                        .to_string(),
                    ))
                    .expect("reserved tool request"),
            )
            .await
            .expect("reserved tool response");
        assert_eq!(reserved_tool.status(), StatusCode::BAD_REQUEST);

        let chat_stream = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/chat/completions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model":"mcg:auto",
                            "messages":[{"role":"user","content":"stream"}],
                            "stream":true
                        })
                        .to_string(),
                    ))
                    .expect("chat stream request"),
            )
            .await
            .expect("chat stream response");
        assert_eq!(chat_stream.status(), StatusCode::OK);
        let chat_stream = String::from_utf8(
            chat_stream
                .into_body()
                .collect()
                .await
                .expect("chat stream body")
                .to_bytes()
                .to_vec(),
        )
        .expect("chat stream utf8");
        assert!(chat_stream.contains("chat.completion.chunk"));
        assert!(chat_stream.contains("\"content\":\"fixture\""));

        let cheap = app
            .clone()
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"model":"mcg:cheap","input":"hello"}).to_string(),
                    ))
                    .expect("cheap request"),
            )
            .await
            .expect("cheap response");
        assert_eq!(cheap.status(), StatusCode::BAD_REQUEST);

        let unknown = app
            .oneshot(
                authorized(Request::post("/v1/responses"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"model":"mcg:missing","input":"hello"}).to_string(),
                    ))
                    .expect("unknown request"),
            )
            .await
            .expect("unknown response");
        assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Covers atomic setup, explicit probe, rollback, and secret-free listing.
    async fn anonymous_provider_configuration_discovers_and_persists_secret_free_catalog() {
        let runtime = test_runtime();
        let app = router(runtime.clone());
        let request = serde_json::json!({
            "connection": {
                "contract_version": CONTRACT_VERSION,
                "id": "fixture_local",
                "display_name": "Fixture Local",
                "kind": ProviderKind::OpenAiCompatible,
                "base_url": model_discovery_fixture().await,
                "auth_mode": ProviderAuthMode::None,
                "organization": null,
                "project": null
            }
        });
        let response = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/providers"))
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .expect("request"),
            )
            .await
            .expect("provider response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let mutation: ProviderMutation = serde_json::from_slice(&body).expect("mutation");
        assert_eq!(mutation.catalog.models[0].id, "fixture-model");
        assert_eq!(mutation.catalog.models[0].capabilities.tools, None);
        assert_eq!(runtime.provider_route_candidates().await.len(), 1);

        let probed = app
            .clone()
            .oneshot(
                authorized(Request::post(
                    "/api/v1/providers/fixture_local/models/fixture-model/probe",
                ))
                .body(Body::empty())
                .expect("probe request"),
            )
            .await
            .expect("probe response");
        assert_eq!(probed.status(), StatusCode::OK);
        let report: ModelProbeReport = serde_json::from_slice(
            &probed
                .into_body()
                .collect()
                .await
                .expect("probe body")
                .to_bytes(),
        )
        .expect("probe report");
        assert_eq!(report.observations.len(), 3);
        assert_eq!(report.model.capabilities.responses, Some(true));
        assert_eq!(
            runtime
                .provider_catalog("fixture_local")
                .expect("catalog")
                .expect("durable catalog")
                .models[0]
                .capabilities
                .responses,
            Some(true)
        );

        let failed_request = serde_json::json!({
            "connection": {
                "contract_version": CONTRACT_VERSION,
                "id": "bad_endpoint",
                "display_name": "Bad Endpoint",
                "kind": ProviderKind::OpenAiCompatible,
                "base_url": format!("{}/wrong", model_discovery_fixture().await),
                "auth_mode": ProviderAuthMode::None,
                "organization": null,
                "project": null
            }
        });
        let failed = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/providers"))
                    .header("content-type", "application/json")
                    .body(Body::from(failed_request.to_string()))
                    .expect("request"),
            )
            .await
            .expect("failed provider response");
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            runtime.provider_connections().expect("connections").len(),
            1
        );
        assert_eq!(runtime.provider_route_candidates().await.len(), 1);

        let response = app
            .oneshot(
                authorized(Request::get("/api/v1/providers"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let list: ProviderListResponse = serde_json::from_slice(&body).expect("list");
        assert_eq!(list.providers.len(), 1);
        let encoded = String::from_utf8(body.to_vec()).expect("UTF-8");
        assert!(!encoded.contains("credential"));
        assert!(!encoded.contains("token"));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Exercises authenticated configure, discovery, persistence, and listing.
    async fn mcp_client_configuration_is_authenticated_discovered_and_secret_free() {
        async fn mcp_fixture(
            headers: HeaderMap,
            Json(body): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            let method = body["method"].as_str().expect("method");
            assert_eq!(
                headers
                    .get("mcp-protocol-version")
                    .and_then(|value| value.to_str().ok()),
                Some(MCP_PROTOCOL_VERSION)
            );
            let result = match method {
                "server/discover" => serde_json::json!({
                    "resultType": "complete",
                    "supportedVersions": [MCP_PROTOCOL_VERSION],
                    "capabilities": {"tools": {}},
                    "ttlMs": 300_000,
                    "serverInfo": {"name": "api-fixture", "version": "1"}
                }),
                "tools/list" => serde_json::json!({
                    "resultType": "complete",
                    "tools": [{
                        "name": "fixture_read",
                        "description": "Read fixture data",
                        "inputSchema": {"type": "object", "additionalProperties": false}
                    }],
                    "serverInfo": {"name": "api-fixture", "version": "1"}
                }),
                _ => unreachable!(),
            };
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": body["id"].clone(),
                "result": result
            }))
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/mcp", post(mcp_fixture)))
                .await
                .expect("serve fixture");
        });
        let runtime = test_runtime();
        let app = router(runtime.clone());
        let connection = McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "fixture_mcp".into(),
            display_name: "Fixture MCP".into(),
            transport: McpTransport::StreamableHttp {
                url: format!("http://{address}/mcp"),
            },
            auth_mode: McpAuthMode::None,
            protocol_mode: McpProtocolMode::Auto,
            enabled: true,
        };
        let configured = app
            .clone()
            .oneshot(
                authorized(Request::post("/api/v1/mcp/connections"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"connection": connection}).to_string(),
                    ))
                    .expect("configure request"),
            )
            .await
            .expect("configure response");
        assert_eq!(configured.status(), StatusCode::CREATED);
        assert_eq!(runtime.mcp_connections().expect("connections").len(), 1);
        assert_eq!(
            runtime
                .mcp_catalog("fixture_mcp")
                .expect("catalog")
                .expect("present")
                .tools[0]
                .name,
            "fixture_read"
        );

        let rediscovered = app
            .clone()
            .oneshot(
                authorized(Request::post(
                    "/api/v1/mcp/connections/fixture_mcp/discover",
                ))
                .body(Body::empty())
                .expect("discover request"),
            )
            .await
            .expect("discover response");
        assert_eq!(rediscovered.status(), StatusCode::OK);

        let listed = app
            .oneshot(
                authorized(Request::get("/api/v1/mcp/connections"))
                    .body(Body::empty())
                    .expect("list request"),
            )
            .await
            .expect("list response");
        assert_eq!(listed.status(), StatusCode::OK);
        let body = listed.into_body().collect().await.expect("body").to_bytes();
        let list: McpListResponse = serde_json::from_slice(&body).expect("list");
        assert_eq!(list.connections.len(), 1);
        let encoded = String::from_utf8(body.to_vec()).expect("UTF-8");
        assert!(!encoded.contains("credential"));
        assert!(!encoded.contains("authorization"));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Full public MCP run proves namespace, provider loop, policy, and remote call composition.
    async fn public_mcp_agent_run_executes_one_governed_remote_tool() {
        let runtime = test_runtime();
        let project_root = tempfile::tempdir().expect("project");
        std::fs::write(project_root.path().join("README.md"), "fixture project")
            .expect("fixture file");
        let project_id = ProjectId::new();
        runtime
            .discover_project(project_root.path(), project_id)
            .expect("discover project");
        runtime
            .configure_provider(
                ProviderConnection {
                    contract_version: CONTRACT_VERSION,
                    id: "mcp_agent_provider".into(),
                    display_name: "MCP Agent Provider".into(),
                    kind: ProviderKind::OpenAiCompatible,
                    base_url: governed_mcp_model_fixture().await,
                    auth_mode: ProviderAuthMode::None,
                    organization: None,
                    project: None,
                },
                None,
                None,
            )
            .await
            .expect("configure provider");
        let remote_calls = Arc::new(AtomicUsize::new(0));
        runtime
            .configure_mcp(
                McpConnection {
                    contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                    id: "governed".into(),
                    display_name: "Governed Fixture".into(),
                    transport: McpTransport::StreamableHttp {
                        url: governed_mcp_tool_fixture(remote_calls.clone()).await,
                    },
                    auth_mode: McpAuthMode::None,
                    protocol_mode: McpProtocolMode::Auto,
                    enabled: true,
                },
                None,
            )
            .await
            .expect("configure MCP");

        let response = router(runtime.clone())
            .oneshot(mcp_request(
                &serde_json::json!(501),
                "tools/call",
                serde_json::json!({
                    "name": "mcg_run_agent",
                    "arguments": {
                        "request_key": "public-governed-mcp-1",
                        "project_id": project_id,
                        "goal": "Use the external lookup once and report its evidence",
                        "role": "tool_worker",
                        "requested_tools": ["mcp::governed::lookup"],
                        "estimated_output_tokens": 256,
                        "max_model_calls": 4
                    }
                })
                .as_object()
                .expect("params")
                .clone(),
                Some("mcg_run_agent"),
            ))
            .await
            .expect("run response");
        assert_eq!(response.status(), StatusCode::OK);
        let response = response_json(response).await;
        assert_eq!(
            response.pointer("/result/isError"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(remote_calls.load(Ordering::SeqCst), 1);
        let structured = response
            .pointer("/result/structuredContent")
            .expect("structured result");
        assert_eq!(
            structured.pointer("/run/results/0/tool_outcomes/0/status"),
            Some(&serde_json::json!("succeeded"))
        );
        assert!(
            structured
                .pointer("/run/leases/0/tool_permissions/0")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.starts_with("mcp_governed_lookup_"))
        );
        assert!(runtime.approvals(None).expect("approvals").is_empty());
    }
}
