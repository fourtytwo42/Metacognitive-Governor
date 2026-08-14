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
use mcg_domain::{ProjectId, RiskLevel, TaskBudget, TaskEnvelope, TaskId, TaskState, ThreadId};
use mcg_events::EventEnvelope;
use mcg_runtime::{AppRuntime, RuntimeError, RuntimeStatus};
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

#[derive(Debug, Default, Deserialize)]
struct EventStreamQuery {
    after: Option<u64>,
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

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use mcg_config::AppConfig;
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
}
