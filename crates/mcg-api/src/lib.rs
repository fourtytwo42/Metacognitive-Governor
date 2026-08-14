//! Versioned authenticated loopback HTTP and streaming surface.

use std::{convert::Infallible, future::Future, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use chrono::Utc;
use mcg_domain::{
    ApprovalId, MemoryClaimId, ProjectId, RiskLevel, TaskBudget, TaskEnvelope, TaskId, TaskState,
    ThreadId,
};
use mcg_events::EventEnvelope;
use mcg_projects::{
    ClaimProvenance, ContextAssembly, ContextItem, MemoryClaim, MemoryLifecycleAction, ProjectModel,
};
use mcg_runtime::{AppRuntime, MemoryClaimMutation, ProjectMutation, RuntimeError, RuntimeStatus};
use mcg_tools::{ApprovalRequest, ApprovalResolution, ApprovalState, ToolAuditRecord};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tower_http::trace::TraceLayer;

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

pub fn router(runtime: Arc<AppRuntime>) -> Router {
    let state = ApiState { runtime };
    let protected = Router::new()
        .route("/api/v1/system/status", get(status))
        .route("/api/v1/tasks", post(create_task))
        .route(
            "/api/v1/tasks/{task_id}",
            get(get_task).post(transition_task),
        )
        .route("/api/v1/tasks/{task_id}/events", get(task_events))
        .route("/api/v1/tasks/{task_id}/tool-audit", get(task_tool_audit))
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

#[allow(clippy::needless_pass_by_value)]
fn map_runtime_error(error: RuntimeError) -> ApiError {
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
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use mcg_config::AppConfig;
    use mcg_domain::{ArtifactId, ToolExecutionId, ToolRequestId};
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

    fn authorized(request: axum::http::request::Builder) -> axum::http::request::Builder {
        request.header("authorization", format!("Bearer {TEST_TOKEN}"))
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
            Some(created.task)
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
}
