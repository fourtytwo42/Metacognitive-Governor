//! Stateless MCP 2026-07-28 Streamable HTTP server boundary.

use std::{collections::HashSet, convert::Infallible, str::FromStr, time::Duration};

use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event as SseEvent},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentRun, AgentRunState, EvaluationReport, ExecutivePlanner,
    ExperimentBenchmark, ExperimentDecision, ExperimentDefinition, ExperimentState,
    GovernorProposal, GovernorProposalState, OrganizationGenome, OrganizationSharePackage,
    OrganizationalMetrics, PlannedWork, PlanningRequest, PortableOrganizationGenome, RoleKind,
    ShadowExecutionReceipt, SuccessCondition, SupervisionObservation, TaskComplexity,
};
use mcg_domain::{
    ConversationId, ExperimentId, GovernorProposalId, MemoryClaimId, MemoryTransferId,
    OrganizationId, OrganizationImportId, OrganizationRevisionId, OrganizationShareReviewId,
    PlatformSessionId, PlatformSessionKind, PluginCapabilityGrant, PluginInvocationId, ProjectId,
    ProjectSnapshotId, ReviewedTopologyPlanId, RiskLevel, RunId, ScheduleId, ScheduleState,
    ScheduleTrigger, SiteDeploymentCommand, SiteDeploymentId, SiteEnvironment, SiteHealthCheckId,
    SiteId, SiteState, SshAuthentication, SshHostId, SshHostPlatform, SshHostState, SshOperationId,
    SshOperationKind, SshWorkflowId, TaskBudget, TaskEnvelope, TaskId, TaskState, WorkItemId,
};
use mcg_runtime::{
    AgentDispatchSpec, PlatformSessionRequest, ReviewedTopologyPlanCreateRequest,
    ReviewedTopologyStepRequest, SiteMonitorRequest, SiteRegistrationRequest, SiteUpdateRequest,
    SshHostRegistrationRequest, SshWorkflowCreateRequest, SshWorkflowStepRequest,
};
use mcg_tools::PersistentBrowserMode;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_stream::wrappers::ReceiverStream;
use url::{Host, Url};

use super::{ApiState, CreateScheduleRequest, LaunchTaskRequest, create_schedule_record};

const PROTOCOL_VERSION: &str = "2026-07-28";
const SERVER_NAME: &str = "metacognitive-governor";
const HEADER_MISMATCH: i64 = -32_020;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32_022;
const PARSE_ERROR: i64 = -32_700;
const INVALID_REQUEST: i64 = -32_600;
const METHOD_NOT_FOUND: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;
const INTERNAL_ERROR: i64 = -32_603;
const DEFAULT_CONTEXT_TOKENS: u32 = 16_384;
const MAX_CONTEXT_TOKENS: u32 = 131_072;
const DEFAULT_MEMORY_LIMIT: usize = 20;
const MAX_MEMORY_LIMIT: usize = 100;

#[derive(Debug)]
struct RpcRequest {
    id: Value,
    method: String,
    params: serde_json::Map<String, Value>,
}

#[derive(Debug)]
struct RpcFailure {
    status: StatusCode,
    id: Option<Value>,
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcFailure {
    fn new(status: StatusCode, id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            status,
            id,
            code,
            message: message.into(),
            data: None,
        }
    }

    fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    fn response(self) -> Response {
        let mut error = json!({
            "code": self.code,
            "message": self.message,
        });
        if let Some(data) = self.data {
            error["data"] = data;
        }
        let mut body = json!({
            "jsonrpc": "2.0",
            "error": error,
        });
        if let Some(id) = self.id {
            body["id"] = id;
        }
        json_response(self.status, &body)
    }
}

/// Handles one stateless MCP JSON-RPC request over Streamable HTTP.
pub(super) async fn handle(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = origin_rejection(&headers) {
        return response;
    }

    let value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return RpcFailure::new(StatusCode::BAD_REQUEST, None, PARSE_ERROR, "Parse error")
                .response();
        }
    };
    let request = match parse_request(&value) {
        Ok(request) => request,
        Err(error) => return error.response(),
    };
    if let Err(error) = validate_metadata_and_headers(&headers, &request) {
        return error.response();
    }
    if request.method == "tools/call"
        && request.params.get("name").and_then(Value::as_str) == Some("mcg_run_task")
        && accepts_sse(&headers)
        && let Some(progress_token) = progress_token(&request.params)
    {
        return stream_task_call(state, request, progress_token);
    }

    let id = request.id.clone();
    let result = match request.method.as_str() {
        "server/discover" => Ok(discover_result()),
        "tools/list" => list_tools(&request.params),
        "tools/call" => call_tool(&state, &request.params).await,
        _ => Err(RpcFailure::new(
            StatusCode::NOT_FOUND,
            Some(id.clone()),
            METHOD_NOT_FOUND,
            "Method not found",
        )),
    };

    match result {
        Ok(result) => {
            let body = json!({"jsonrpc": "2.0", "id": id, "result": result});
            json_response(StatusCode::OK, &body)
        }
        Err(mut error) => {
            error.id.get_or_insert(id);
            error.response()
        }
    }
}

fn progress_token(params: &serde_json::Map<String, Value>) -> Option<Value> {
    params
        .get("_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("progressToken"))
        .cloned()
}

fn accepts_sse(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| {
            accept.split(',').any(|value| {
                value.split(';').next().is_some_and(|media_type| {
                    media_type.trim().eq_ignore_ascii_case("text/event-stream")
                })
            })
        })
}

fn stream_task_call(state: ApiState, request: RpcRequest, progress_token: Value) -> Response {
    let request_key = request
        .params
        .get("arguments")
        .and_then(Value::as_object)
        .and_then(|arguments| arguments.get("request_key"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut event_receiver = state.runtime.subscribe();
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(32);
    tokio::spawn(async move {
        if send_mcp_sse(&sender, initial_progress_notification(&progress_token))
            .await
            .is_err()
        {
            return;
        }
        let id = request.id.clone();
        let params = request.params;
        let call = call_tool(&state, &params);
        tokio::pin!(call);
        let mut task_id = None;
        let mut last_sequence = 0;
        let mut events_open = true;
        loop {
            tokio::select! {
                () = sender.closed() => {
                    cancel_streamed_task(&state, request_key.as_deref(), task_id);
                    return;
                }
                result = &mut call => {
                    emit_durable_task_progress(
                        &state,
                        request_key.as_deref(),
                        &mut task_id,
                        &mut last_sequence,
                        &progress_token,
                        &sender,
                    ).await;
                    let body = match result {
                        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                        Err(mut error) => {
                            error.id.get_or_insert(id);
                            rpc_failure_body(&error)
                        }
                    };
                    if send_mcp_sse(&sender, body).await.is_err() {
                        cancel_streamed_task(&state, request_key.as_deref(), task_id);
                    }
                    return;
                }
                event = event_receiver.recv(), if events_open => {
                    match event {
                        Ok(event) => {
                            if task_id.is_none() {
                                task_id = streamed_task_id(&state, request_key.as_deref());
                            }
                            if event.task_id == task_id && event.sequence > last_sequence {
                                last_sequence = event.sequence;
                                if send_mcp_sse(
                                    &sender,
                                    progress_notification(
                                        &progress_token,
                                        event.sequence,
                                        event.kind.as_str(),
                                    ),
                                )
                                .await
                                .is_err()
                                {
                                    cancel_streamed_task(&state, request_key.as_deref(), task_id);
                                    return;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            emit_durable_task_progress(
                                &state,
                                request_key.as_deref(),
                                &mut task_id,
                                &mut last_sequence,
                                &progress_token,
                                &sender,
                            ).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => events_open = false,
                    }
                }
            }
        }
    });
    Sse::new(ReceiverStream::new(receiver))
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

async fn emit_durable_task_progress(
    state: &ApiState,
    request_key: Option<&str>,
    task_id: &mut Option<TaskId>,
    last_sequence: &mut u64,
    progress_token: &Value,
    sender: &tokio::sync::mpsc::Sender<Result<SseEvent, Infallible>>,
) {
    if task_id.is_none() {
        *task_id = streamed_task_id(state, request_key);
    }
    if let Some(task_id) = *task_id
        && let Ok(events) = state.runtime.task_events(task_id)
    {
        for event in events {
            if event.sequence > *last_sequence {
                *last_sequence = event.sequence;
                if send_mcp_sse(
                    sender,
                    progress_notification(progress_token, event.sequence, event.kind.as_str()),
                )
                .await
                .is_err()
                {
                    break;
                }
            }
        }
    }
}

fn streamed_task_id(state: &ApiState, request_key: Option<&str>) -> Option<TaskId> {
    request_key
        .and_then(|request_key| {
            state
                .runtime
                .interface_run_request("mcp_task", request_key)
                .ok()
                .flatten()
        })
        .map(|(task_id, _)| task_id)
}

fn cancel_streamed_task(state: &ApiState, request_key: Option<&str>, task_id: Option<TaskId>) {
    let task_id = task_id.or_else(|| streamed_task_id(state, request_key));
    if let Some(task_id) = task_id
        && state
            .runtime
            .task(task_id)
            .ok()
            .flatten()
            .is_some_and(|task| {
                !matches!(
                    task.state,
                    TaskState::Completed | TaskState::Failed | TaskState::Cancelled
                )
            })
    {
        let _ = state.runtime.transition_task(task_id, TaskState::Cancelled);
    }
}

fn progress_notification(progress_token: &Value, progress: u64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {
            "progressToken": progress_token,
            "progress": progress,
            "message": message,
        }
    })
}

fn initial_progress_notification(progress_token: &Value) -> Value {
    progress_notification(
        progress_token,
        0,
        "MCG is reserving the durable consumer task",
    )
}

async fn send_mcp_sse(
    sender: &tokio::sync::mpsc::Sender<Result<SseEvent, Infallible>>,
    message: Value,
) -> Result<(), ()> {
    sender
        .send(Ok(SseEvent::default()
            .event("message")
            .data(message.to_string())))
        .await
        .map_err(|_| ())
}

fn rpc_failure_body(error: &RpcFailure) -> Value {
    let mut detail = json!({
        "code": error.code,
        "message": error.message,
    });
    if let Some(data) = &error.data {
        detail["data"] = data.clone();
    }
    let mut body = json!({"jsonrpc": "2.0", "error": detail});
    if let Some(id) = &error.id {
        body["id"] = id.clone();
    }
    body
}

fn parse_request(value: &Value) -> Result<RpcRequest, RpcFailure> {
    let object = value.as_object().ok_or_else(|| {
        RpcFailure::new(
            StatusCode::BAD_REQUEST,
            None,
            INVALID_REQUEST,
            "Invalid Request",
        )
    })?;
    let id = object.get("id").cloned().ok_or_else(|| {
        RpcFailure::new(
            StatusCode::BAD_REQUEST,
            None,
            INVALID_REQUEST,
            "Client notifications are not supported by this HTTP endpoint",
        )
    })?;
    if !id.is_string() && !id.is_number() {
        return Err(RpcFailure::new(
            StatusCode::BAD_REQUEST,
            None,
            INVALID_REQUEST,
            "Request id must be a string or number",
        ));
    }
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(RpcFailure::new(
            StatusCode::BAD_REQUEST,
            Some(id),
            INVALID_REQUEST,
            "jsonrpc must be 2.0",
        ));
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .filter(|method| !method.is_empty())
        .ok_or_else(|| {
            RpcFailure::new(
                StatusCode::BAD_REQUEST,
                Some(id.clone()),
                INVALID_REQUEST,
                "Request method is required",
            )
        })?
        .to_owned();
    let params = object
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            RpcFailure::new(
                StatusCode::BAD_REQUEST,
                Some(id.clone()),
                INVALID_PARAMS,
                "Request params must be an object",
            )
        })?;
    Ok(RpcRequest { id, method, params })
}

fn validate_metadata_and_headers(
    headers: &HeaderMap,
    request: &RpcRequest,
) -> Result<(), RpcFailure> {
    let id = Some(request.id.clone());
    let meta = request
        .params
        .get("_meta")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RpcFailure::new(
                StatusCode::BAD_REQUEST,
                id.clone(),
                INVALID_PARAMS,
                "params._meta is required",
            )
        })?;
    let body_version = meta
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RpcFailure::new(
                StatusCode::BAD_REQUEST,
                id.clone(),
                INVALID_PARAMS,
                "Protocol version metadata is required",
            )
        })?;
    if !meta
        .get("io.modelcontextprotocol/clientCapabilities")
        .is_some_and(Value::is_object)
    {
        return Err(RpcFailure::new(
            StatusCode::BAD_REQUEST,
            id,
            INVALID_PARAMS,
            "Client capabilities metadata must be an object",
        ));
    }
    if meta
        .get("progressToken")
        .is_some_and(|token| !token.is_string() && !token.is_number())
    {
        return Err(RpcFailure::new(
            StatusCode::BAD_REQUEST,
            id,
            INVALID_PARAMS,
            "Progress token must be a string or number",
        ));
    }

    let header_version = required_header(headers, "mcp-protocol-version", &request.id)?;
    if header_version != body_version {
        return Err(header_mismatch(
            &request.id,
            "MCP-Protocol-Version does not match request metadata",
        ));
    }
    if body_version != PROTOCOL_VERSION {
        return Err(RpcFailure::new(
            StatusCode::BAD_REQUEST,
            Some(request.id.clone()),
            UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
        )
        .with_data(json!({
            "supported": [PROTOCOL_VERSION],
            "requested": body_version,
        })));
    }

    let method = required_header(headers, "mcp-method", &request.id)?;
    if method != request.method {
        return Err(header_mismatch(
            &request.id,
            "Mcp-Method does not match the request method",
        ));
    }
    if request.method == "tools/call" {
        let body_name = request
            .params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RpcFailure::new(
                    StatusCode::OK,
                    Some(request.id.clone()),
                    INVALID_PARAMS,
                    "Tool name is required",
                )
            })?;
        let encoded_name = required_header(headers, "mcp-name", &request.id)?;
        let header_name = decode_mirrored_value(encoded_name)
            .map_err(|()| header_mismatch(&request.id, "Mcp-Name is malformed"))?;
        if header_name != body_name {
            return Err(header_mismatch(
                &request.id,
                "Mcp-Name does not match the requested tool",
            ));
        }
    }
    Ok(())
}

fn required_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
    id: &Value,
) -> Result<&'a str, RpcFailure> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            header_mismatch(
                id,
                format!("Required {name} header is missing or malformed"),
            )
        })
}

fn header_mismatch(id: &Value, message: impl Into<String>) -> RpcFailure {
    RpcFailure::new(
        StatusCode::BAD_REQUEST,
        Some(id.clone()),
        HEADER_MISMATCH,
        format!("Header mismatch: {}", message.into()),
    )
}

fn decode_mirrored_value(value: &str) -> Result<String, ()> {
    if let Some(encoded) = value
        .strip_prefix("=?base64?")
        .and_then(|value| value.strip_suffix("?="))
    {
        let bytes = STANDARD.decode(encoded).map_err(|_| ())?;
        return String::from_utf8(bytes).map_err(|_| ());
    }
    if value.is_empty()
        || value.trim() != value
        || value.bytes().any(|byte| !(0x20..=0x7e).contains(&byte))
        || (value.starts_with("=?base64?") && value.ends_with("?="))
    {
        return Err(());
    }
    Ok(value.to_owned())
}

pub(super) fn origin_rejection(headers: &HeaderMap) -> Option<Response> {
    (!valid_origin(headers)).then(|| {
        RpcFailure::new(
            StatusCode::FORBIDDEN,
            None,
            INVALID_REQUEST,
            "Origin is not trusted by this loopback MCP server",
        )
        .response()
    })
}

fn valid_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(origin) = Url::parse(origin) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https") {
        return false;
    }
    match origin.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn discover_result() -> Value {
    with_server_info(json!({
        "resultType": "complete",
        "supportedVersions": [PROTOCOL_VERSION],
        "capabilities": {"tools": {}},
        "instructions": "Read-only access to MCG status, project intelligence, bounded context, and provenance-bearing memory.",
        "ttlMs": 300_000,
        "cacheScope": "private",
    }))
}

fn list_tools(params: &serde_json::Map<String, Value>) -> Result<Value, RpcFailure> {
    if params.get("cursor").is_some() {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "This tool list has one page and does not accept a cursor",
        ));
    }
    Ok(with_server_info(json!({
        "resultType": "complete",
        "tools": tool_definitions(),
        "ttlMs": 300_000,
        "cacheScope": "private",
    })))
}

#[allow(clippy::too_many_lines)] // One stable ordered registry keeps advertised schemas auditable.
fn tool_definitions() -> Vec<Value> {
    let read_annotations = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false,
    });
    vec![
        json!({
            "name": "mcg_memory_search",
            "title": "Search MCG Project Memory",
            "description": "Search current provenance-bearing memory claims for one known project. Secret-like namespaces are never admitted to MCG memory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"},
                    "query": {"type": "string", "minLength": 1},
                    "namespace": {"type": "string", "minLength": 1},
                    "fresh_only": {"type": "boolean", "default": true},
                    "limit": {"type": "integer", "minimum": 1, "maximum": MAX_MEMORY_LIMIT, "default": DEFAULT_MEMORY_LIMIT}
                },
                "required": ["project_id", "query"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_project_memory_transfer_preview",
            "title": "Review MCG Project Memory Transfer",
            "description": "Load one exact fresh durable source claim and its canonical SHA-256 before an explicit cross-project transfer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_project_id": {"type": "string", "format": "uuid"},
                    "source_claim_id": {"type": "string", "format": "uuid"}
                },
                "required": ["source_project_id", "source_claim_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_project_memory_transfers",
            "title": "List MCG Project Memory Transfers",
            "description": "List immutable inbound and outbound cross-project memory transfer evidence for one project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"}
                },
                "required": ["project_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_project_memory_transfer",
            "title": "Transfer MCG Project Memory",
            "description": "Copy one explicitly reviewed, digest-bound fresh durable claim into a different project. The caller owns transfer_id for safe retry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "transfer_id": {"type": "string", "format": "uuid"},
                    "source_project_id": {"type": "string", "format": "uuid"},
                    "source_claim_id": {"type": "string", "format": "uuid"},
                    "source_claim_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "target_project_id": {"type": "string", "format": "uuid"},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 512}
                },
                "required": ["transfer_id", "source_project_id", "source_claim_id", "source_claim_sha256", "target_project_id", "reason"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false
            },
        }),
        json!({
            "name": "mcg_project_context",
            "title": "Assemble MCG Project Context",
            "description": "Assemble bounded, target-aware project context from the current project snapshot and fresh memory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"},
                    "target_path": {"type": "string"},
                    "token_budget": {"type": "integer", "minimum": 1, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS}
                },
                "required": ["project_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_project_topology_plan",
            "title": "Plan MCG Project Topology",
            "description": "Derive stable read-only stages from explicit project manifest dependencies. Cycles fail closed and routing or inventory never invents execution order.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"}
                },
                "required": ["project_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_reviewed_topology_plans",
            "title": "Inspect Reviewed MCG Topology Plans",
            "description": "Read separately reviewed, snapshot-pinned project execution plans and their durable command evidence. This query never executes a command.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"},
                    "plan_id": {"type": "string", "format": "uuid"}
                },
                "required": ["project_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_reviewed_topology_plan_mutate",
            "title": "Review or Run an MCG Topology Plan",
            "description": "Create, run, or cancel a caller-identified plan pinned to the exact current snapshot, canonical topology digest, and manifest-discovered structured commands. Execution still passes through ordinary policy and approval controls.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["create", "run", "cancel"]},
                    "project_id": {"type": "string", "format": "uuid"},
                    "plan_id": {"type": "string", "format": "uuid"},
                    "project_snapshot_id": {"type": "string", "format": "uuid"},
                    "topology_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "display_name": {"type": "string", "minLength": 1, "maxLength": 160},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 1024},
                    "steps": {
                        "type": "array", "minItems": 1, "maxItems": 128,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string", "format": "uuid"},
                                "stage_index": {"type": "integer", "minimum": 0, "maximum": 65535},
                                "name": {"type": "string", "minLength": 1, "maxLength": 160},
                                "command_name": {"type": "string", "minLength": 1},
                                "command": {"type": "string", "minLength": 1},
                                "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 7200, "default": 1800}
                            },
                            "required": ["id", "stage_index", "name", "command_name", "command"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["action", "project_id", "plan_id"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_query_project",
            "title": "Query MCG Projects",
            "description": "List current project models or return one exact project model by id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": {"type": "string", "format": "uuid"}
                },
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_query_organization",
            "title": "Query MCG Organizations",
            "description": "List one organization's immutable history, export one revision as a bounded content-addressed portable genome, or compare two revisions without treating local identities as semantic changes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "organization_id": {"type": "string", "format": "uuid"},
                    "revision_id": {"type": "string", "format": "uuid"},
                    "compare_revision_id": {"type": "string", "format": "uuid"}
                },
                "oneOf": [
                    {"required": ["organization_id"]},
                    {"required": ["revision_id"]}
                ],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_import_organization",
            "title": "Import an MCG Organization",
            "description": "Idempotently import one bounded content-addressed portable genome as a fresh local organization with no inherited source authority.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "import_id": {"type": "string", "format": "uuid"},
                    "portable": {
                        "type": "object",
                        "properties": {
                            "schema_version": {"type": "integer", "const": 1},
                            "source": {"type": "object"},
                            "definition": {"type": "object"},
                            "content_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
                        },
                        "required": ["schema_version", "source", "definition", "content_sha256"],
                        "additionalProperties": false
                    },
                    "name": {"type": "string", "minLength": 1, "maxLength": 100}
                },
                "required": ["import_id", "portable"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_organization_publishers",
            "title": "List MCG Organization Publishers",
            "description": "List owner-selected organization publisher trust heads, exact registry scopes, public-key fingerprints, and terminal revocation state.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_organization_publisher_mutate",
            "title": "Change MCG Organization Publisher Trust",
            "description": "Explicitly trust/revise or permanently revoke one signed-organization publisher. Public keys are immutable per identity and trust is scoped to canonical HTTPS registries.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["trust", "revoke"]},
                    "publisher_id": {"type": "string", "minLength": 1, "maxLength": 128},
                    "display_name": {"type": "string", "minLength": 1, "maxLength": 100},
                    "public_key_base64": {"type": "string", "minLength": 44, "maxLength": 44},
                    "registries": {"type": "array", "minItems": 1, "maxItems": 32, "items": {"type": "string", "format": "uri"}},
                    "expected_revision": {"type": "integer", "minimum": 1},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 2000}
                },
                "required": ["action", "publisher_id", "reason"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_organization_share_reviews",
            "title": "List MCG Organization Share Reviews",
            "description": "List durable signed-organization reviews and their verified publisher/package evidence without granting import authority.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_organization_share_mutate",
            "title": "Review or Decide an MCG Organization Share",
            "description": "Verify a signed package into pending review, explicitly import it after current-trust revalidation, or reject it. Review alone never imports or activates authority.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["review", "import", "reject"]},
                    "review_id": {"type": "string", "format": "uuid"},
                    "package": {"type": "object"},
                    "import_id": {"type": "string", "format": "uuid"},
                    "name": {"type": "string", "minLength": 1, "maxLength": 100},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 2000}
                },
                "required": ["action", "review_id"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_query_task",
            "title": "Query an MCG Consumer Task",
            "description": "Read one durable task together with its authoritative event history, governed-tool audit, and consumer run when present.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "format": "uuid"},
                    "after_sequence": {"type": "integer", "minimum": 0, "default": 0},
                    "audit_offset": {"type": "integer", "minimum": 0, "default": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 200}
                },
                "required": ["task_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_query_run",
            "title": "Query MCG Agent Run",
            "description": "Read one durable run, or deterministically replay and verify its complete retained evidence without provider or tool side effects.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": {"type": "string", "format": "uuid"},
                    "replay": {"type": "boolean", "default": false, "description": "Return the deterministic replay report instead of the ordinary run view."}
                },
                "required": ["run_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_scheduler_list",
            "title": "List MCG Schedules",
            "description": "List durable unattended-work definitions with their current state and next wake.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_platform_capabilities",
            "title": "Read MCG Platform Capabilities",
            "description": "Read the latest privacy-safe host capability snapshot. Availability is reported only for verified local support.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_platform_sessions",
            "title": "List MCG Platform Sessions",
            "description": "List durable browser and computer-use session reservations and their explicit authority state.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_platform_request",
            "title": "Request an MCG Platform Session",
            "description": "Idempotently reserve a browser or computer-use session. Requested does not mean an executor is attached; reuse session_id only for an exact retry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "format": "uuid"},
                    "kind": {"type": "string", "enum": ["browser", "computer_use"]},
                    "project_id": {"type": "string", "format": "uuid"},
                    "task_id": {"type": "string", "format": "uuid"},
                    "display_name": {"type": "string", "minLength": 1, "maxLength": 256},
                    "allowed_origins": {"type": "array", "items": {"type": "string", "format": "uri", "maxLength": 2048}, "uniqueItems": true, "maxItems": 32},
                    "computer_capabilities": {"type": "array", "items": {"type": "string", "enum": ["screen_read", "mouse", "keyboard", "clipboard", "window_list", "process_inspect"]}, "uniqueItems": true, "maxItems": 6},
                    "computer_target": {
                        "type": "object",
                        "properties": {
                            "display_id": {"type": "integer", "minimum": 0},
                            "region": {
                                "type": "object",
                                "properties": {
                                    "x": {"type": "integer", "minimum": 0},
                                    "y": {"type": "integer", "minimum": 0},
                                    "width": {"type": "integer", "minimum": 1},
                                    "height": {"type": "integer", "minimum": 1}
                                },
                                "required": ["x", "y", "width", "height"],
                                "additionalProperties": false
                            }
                        },
                        "additionalProperties": false
                    },
                    "lifetime_seconds": {"type": "integer", "minimum": 1, "maximum": 86400}
                },
                "required": ["session_id", "kind", "display_name", "lifetime_seconds"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_platform_close",
            "title": "Close an MCG Platform Session",
            "description": "Close one durable platform session while retaining its immutable lifecycle history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "format": "uuid"},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 2048}
                },
                "required": ["session_id", "reason"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_browser_attach",
            "title": "Attach an MCG Browser",
            "description": "Launch the requested browser in a private ephemeral profile. Headless is the safe default; visible mode permits an explicit owner handoff.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "format": "uuid"},
                    "mode": {"type": "string", "enum": ["headless", "visible"]}
                },
                "required": ["session_id", "mode"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_browser_navigate",
            "title": "Navigate an MCG Browser",
            "description": "Navigate an active MCG-owned browser within its immutable origin grant. Returned URLs never include query strings or fragments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "format": "uuid"},
                    "url": {"type": "string", "format": "uri", "maxLength": 2048}
                },
                "required": ["session_id", "url"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_browser_handoff",
            "title": "Hand Browser to Local Owner",
            "description": "Stop MCG browser operations and hand a visible browser window to its local owner.",
            "inputSchema": {
                "type": "object",
                "properties": {"session_id": {"type": "string", "format": "uuid"}},
                "required": ["session_id"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_browser_reclaim",
            "title": "Return Browser Control to MCG",
            "description": "Blank owner-visible content before returning a handed-off browser to MCG control.",
            "inputSchema": {
                "type": "object",
                "properties": {"session_id": {"type": "string", "format": "uuid"}},
                "required": ["session_id"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_computer_attach",
            "title": "Attach an MCG Computer Session",
            "description": "Bind the reserved immutable display region and capability grant before MCG control becomes active.",
            "inputSchema": session_id_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_computer_capture",
            "title": "Capture an MCG Computer Frame",
            "description": "Capture the active MCG-owned display region. The image digest must be supplied unchanged to any later input batch.",
            "inputSchema": session_id_schema(),
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_computer_actions",
            "title": "Apply MCG Computer Actions",
            "description": "Apply a bounded typed input batch only while the live pixels still match the observed frame digest. Keyboard input requires an earlier in-region click in the same batch.",
            "inputSchema": computer_actions_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": false, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_computer_handoff",
            "title": "Hand Computer Control to Local Owner",
            "description": "Stop all MCG observation and input while the local owner controls the bound desktop.",
            "inputSchema": session_id_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_computer_reclaim",
            "title": "Return Computer Control to MCG",
            "description": "Return authority to MCG without reading owner activity. A fresh capture is required before any new input.",
            "inputSchema": session_id_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": false, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_experiments_inspect",
            "title": "Inspect MCG Measured Improvement",
            "description": "Inspect immutable supervision observations, experiments for one organization, or one experiment with shadow, benchmark, and decision evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["observations", "organization", "experiment"]},
                    "id": {"type": "string", "format": "uuid"}
                },
                "required": ["scope"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_experiments_mutate",
            "title": "Apply an MCG Measured Experiment Action",
            "description": "Idempotently record a candidate, observation, experiment, shadow receipt, paired benchmark, state transition, or measured decision. Promotion and reversion remain evidence-gated and atomic.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["candidate", "observation", "create", "transition", "shadow", "benchmark", "decision"]},
                    "payload": {"type": "object", "description": "The complete versioned contract for the selected action."}
                },
                "required": ["action", "payload"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_governor_inspect",
            "title": "Inspect Dormant Governor Controls",
            "description": "Inspect the hard Governor authority policy or durable proposals. The Governor model remains disabled.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["policy", "organization", "proposal"]},
                    "id": {"type": "string", "format": "uuid"}
                },
                "required": ["scope"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_governor_propose",
            "title": "Submit or Advance a Governed Improvement Proposal",
            "description": "Submit an immutable external-operator proposal, apply a revision-checked lifecycle transition, or route an owner-approved implementation brief through Chat to the Coordinator. This tool cannot edit code, contact Coders directly, activate the Governor, or promote without independent evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["create", "transition", "request_implementation"]},
                    "payload": {"type": "object"}
                },
                "required": ["action", "payload"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_sites_list",
            "title": "List MCG Sites",
            "description": "List durable Site definitions, optionally with immutable revision, deployment, and health history for one Site.",
            "inputSchema": {
                "type": "object",
                "properties": {"site_id": {"type": "string", "format": "uuid"}},
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_sites_register",
            "title": "Register an MCG Site",
            "description": "Idempotently register one project-bound web property with credential-free public and health URLs and an optional structured deployment command.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": {"type": "string", "format": "uuid"},
                    "project_id": {"type": "string", "format": "uuid"},
                    "display_name": {"type": "string", "minLength": 1, "maxLength": 160},
                    "environment": {"type": "string", "enum": ["development", "preview", "production"]},
                    "public_url": {"type": "string", "format": "uri", "maxLength": 2048},
                    "health_url": {"type": "string", "format": "uri", "maxLength": 2048},
                    "hosting_provider": {"type": "string", "minLength": 1, "maxLength": 128},
                    "deployment_command": {
                        "type": "object",
                        "properties": {
                            "executable": {"type": "string", "minLength": 1, "maxLength": 256},
                            "arguments": {"type": "array", "items": {"type": "string", "maxLength": 4096}, "maxItems": 64},
                            "working_directory": {"type": "string", "maxLength": 1024}
                        },
                        "required": ["executable", "working_directory"],
                        "additionalProperties": false
                    },
                    "credential_reference": {"type": "string", "pattern": "^[A-Za-z0-9._:-]{1,128}$"},
                    "state": {"type": "string", "enum": ["active", "monitoring_paused"], "default": "active"}
                },
                "required": ["site_id", "project_id", "display_name", "environment", "public_url", "health_url", "hosting_provider"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_sites_health",
            "title": "Check MCG Site Health",
            "description": "Run one idempotent bounded same-origin health GET and retain immutable status, latency, audit, and response artifact evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": {"type": "string", "format": "uuid"},
                    "check_id": {"type": "string", "format": "uuid"}
                },
                "required": ["site_id", "check_id"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_sites_update",
            "title": "Update an MCG Site",
            "description": "Atomically advance one Site definition with immutable revision history. An exact retry after a lost response replays the committed revision; archive is terminal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": {"type": "string", "format": "uuid"},
                    "expected_revision": {"type": "integer", "minimum": 1},
                    "display_name": {"type": "string", "minLength": 1, "maxLength": 160},
                    "environment": {"type": "string", "enum": ["development", "preview", "production"]},
                    "public_url": {"type": "string", "format": "uri", "maxLength": 2048},
                    "health_url": {"type": "string", "format": "uri", "maxLength": 2048},
                    "hosting_provider": {"type": "string", "minLength": 1, "maxLength": 128},
                    "deployment_command": {
                        "type": "object",
                        "properties": {
                            "executable": {"type": "string", "minLength": 1, "maxLength": 256},
                            "arguments": {"type": "array", "items": {"type": "string", "maxLength": 4096}, "maxItems": 64},
                            "working_directory": {"type": "string", "maxLength": 1024}
                        },
                        "required": ["executable", "working_directory"],
                        "additionalProperties": false
                    },
                    "credential_reference": {"type": "string", "pattern": "^[A-Za-z0-9._:-]{1,128}$"},
                    "state": {"type": "string", "enum": ["active", "monitoring_paused", "archived"]},
                    "reason": {"type": "string", "minLength": 1, "maxLength": 512}
                },
                "required": ["site_id", "expected_revision", "display_name", "environment", "public_url", "health_url", "hosting_provider", "state", "reason"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_sites_monitor",
            "title": "Configure MCG Site Monitoring",
            "description": "Idempotently attach one durable interval monitor to a Site. The monitor survives restart, runs bounded health checks, and coalesces unchanged alerts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": {"type": "string", "format": "uuid"},
                    "schedule_id": {"type": "string", "format": "uuid"},
                    "interval_seconds": {"type": "integer", "minimum": 5, "maximum": 2_592_000},
                    "expected_status": {"type": "integer", "minimum": 100, "maximum": 599, "default": 200},
                    "first_run_at": {"type": "string", "format": "date-time"}
                },
                "required": ["site_id", "schedule_id", "interval_seconds"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_sites_deploy",
            "title": "Deploy an MCG Site",
            "description": "Request one exact revision-bound deployment. External publication always requires owner approval before the deployment ledger is claimed or a process starts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": {"type": "string", "format": "uuid"},
                    "deployment_id": {"type": "string", "format": "uuid"},
                    "task_id": {"type": "string", "format": "uuid"}
                },
                "required": ["site_id", "deployment_id"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_ssh_hosts",
            "title": "List MCG SSH Hosts",
            "description": "List durable pinned outbound SSH hosts and, for one host, retained operation history.",
            "inputSchema": {"type":"object","properties":{"host_id":{"type":"string","format":"uuid"}},"additionalProperties":false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_ssh_register",
            "title": "Register an MCG SSH Host",
            "description": "Idempotently register a POSIX SSH-agent host with an owner-verified pinned known_hosts entry, fingerprint, and optional normalized routing capabilities.",
            "inputSchema": {"type":"object","properties":{
                "host_id":{"type":"string","format":"uuid"},"display_name":{"type":"string"},
                "hostname":{"type":"string"},"port":{"type":"integer","minimum":1,"maximum":65535,"default":22},
                "username":{"type":"string"},"known_hosts_entry":{"type":"string"},"host_key_sha256":{"type":"string"},
                "capabilities":{"type":"array","maxItems":64,"items":{"type":"string","maxLength":96}}
            },"required":["host_id","display_name","hostname","username","known_hosts_entry","host_key_sha256"],"additionalProperties":false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_ssh_run",
            "title": "Run a Typed MCG SSH Operation",
            "description": "Run an idempotent host-revision-bound inspect, structured POSIX argv execution, upload, or download through policy, approval, lease, audit, and retained evidence.",
            "inputSchema": {"type":"object","properties":{
                "host_id":{"type":"string","format":"uuid"},"operation_id":{"type":"string","format":"uuid"},
                "task_id":{"type":"string","format":"uuid"},"operation":{"type":"object"},
                "timeout_seconds":{"type":"integer","minimum":1,"maximum":3600,"default":60}
            },"required":["host_id","operation_id","operation"],"additionalProperties":false},
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_ssh_workflows",
            "title": "Inspect MCG SSH Workflows",
            "description": "List durable multi-host workflow graphs or inspect one workflow and its pinned step state.",
            "inputSchema": {"type":"object","properties":{"workflow_id":{"type":"string","format":"uuid"}},"additionalProperties":false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_ssh_workflow_mutate",
            "title": "Create, Run, or Cancel an MCG SSH Workflow",
            "description": "Create one immutable dependency graph or safely run, resume, or cancel it through the durable typed SSH broker.",
            "inputSchema": {"type":"object","properties":{
                "action":{"type":"string","enum":["create","run","cancel"]},
                "workflow_id":{"type":"string","format":"uuid"},
                "display_name":{"type":"string","minLength":1,"maxLength":160},
                "task_id":{"type":"string","format":"uuid"},
                "steps":{"type":"array","minItems":1,"maxItems":128,"items":{
                    "type":"object",
                    "properties":{
                        "id":{"type":"string","format":"uuid"},
                        "name":{"type":"string","minLength":1,"maxLength":160},
                        "host_id":{"type":"string","format":"uuid"},
                        "host_revision":{"type":"integer","minimum":1},
                        "required_capabilities":{"type":"array","maxItems":64,"items":{"type":"string","maxLength":96}},
                        "dependencies":{"type":"array","maxItems":64,"items":{"type":"string","format":"uuid"}},
                        "operation":{"type":"object","properties":{"kind":{"type":"string","enum":["inspect","execute","upload","download"]}},"required":["kind"]},
                        "timeout_seconds":{"type":"integer","minimum":1,"maximum":3600,"default":60}
                    },
                    "required":["id","name","operation"],
                    "additionalProperties":false,
                    "oneOf":[
                        {"required":["host_id","host_revision"]},
                        {"required":["required_capabilities"],"not":{"anyOf":[{"required":["host_id"]},{"required":["host_revision"]}]}}
                    ]
                }}
            },"required":["action","workflow_id"],"additionalProperties":false},
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_scheduler_create",
            "title": "Create an MCG Schedule",
            "description": "Idempotently create one bounded durable schedule. Reuse the same schedule_id only for an exact retry of the same definition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "schedule_id": {"type": "string", "format": "uuid"},
                    "project_id": {"type": "string", "format": "uuid"},
                    "name": {"type": "string", "minLength": 1, "maxLength": 256},
                    "goal": {"type": "string", "minLength": 1, "maxLength": 32768},
                    "constraints": {"type": "array", "items": {"type": "string", "maxLength": 8192}, "maxItems": 64},
                    "success_conditions": {"type": "array", "items": {"type": "string", "maxLength": 8192}, "maxItems": 64},
                    "risk": {"type": "string", "enum": ["low", "medium", "high", "critical"], "default": "medium"},
                    "budget": {
                        "type": "object",
                        "properties": {
                            "max_tokens": {"type": "integer", "minimum": 1},
                            "max_cost_usd": {"type": "number", "minimum": 0},
                            "max_duration_seconds": {"type": "integer", "minimum": 1}
                        },
                        "additionalProperties": false
                    },
                    "trigger": {
                        "oneOf": [
                            {"type": "object", "properties": {"kind": {"const": "once"}, "at": {"type": "string", "format": "date-time"}}, "required": ["kind", "at"], "additionalProperties": false},
                            {"type": "object", "properties": {"kind": {"const": "interval"}, "seconds": {"type": "integer", "minimum": 1}}, "required": ["kind", "seconds"], "additionalProperties": false},
                            {"type": "object", "properties": {"kind": {"const": "cron"}, "expression": {"type": "string", "minLength": 1, "maxLength": 256}}, "required": ["kind", "expression"], "additionalProperties": false},
                            {"type": "object", "properties": {"kind": {"const": "file_watch"}, "relative_path": {"type": "string", "minLength": 1, "maxLength": 4096}, "poll_seconds": {"type": "integer", "minimum": 1}}, "required": ["kind", "relative_path", "poll_seconds"], "additionalProperties": false},
                            {"type": "object", "properties": {"kind": {"const": "event"}, "event_kind": {"type": "string", "minLength": 1, "maxLength": 128}}, "required": ["kind", "event_kind"], "additionalProperties": false},
                            {"type": "object", "properties": {"kind": {"const": "heartbeat"}, "idle_seconds": {"type": "integer", "minimum": 1}, "busy_seconds": {"type": "integer", "minimum": 1}}, "required": ["kind", "idle_seconds", "busy_seconds"], "additionalProperties": false}
                        ]
                    },
                    "first_run_at": {"type": "string", "format": "date-time"},
                    "target_path": {"type": "string", "maxLength": 4096},
                    "context_token_budget": {"type": "integer", "minimum": 1, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS},
                    "requested_tools": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 256}, "uniqueItems": true, "maxItems": 128},
                    "estimated_output_tokens": {"type": "integer", "minimum": 1, "maximum": 100_000, "default": 8192},
                    "max_model_calls": {"type": "integer", "minimum": 1, "maximum": 32, "default": 16}
                },
                "required": ["schedule_id", "project_id", "name", "goal", "trigger"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            }
        }),
        json!({
            "name": "mcg_scheduler_remove",
            "title": "Remove an MCG Schedule",
            "description": "Permanently stop future admission for one schedule while retaining its durable history.",
            "inputSchema": {
                "type": "object",
                "properties": {"schedule_id": {"type": "string", "format": "uuid"}},
                "required": ["schedule_id"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_run_task",
            "title": "Run a Complete MCG Consumer Task",
            "description": "Idempotently create, decompose, execute, independently evaluate, and settle one durable consumer task through the same workflow used by the desktop and CLI.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request_key": {"type": "string", "minLength": 1, "maxLength": 128, "pattern": "^[A-Za-z0-9._:-]+$"},
                    "project_id": {"type": "string", "format": "uuid"},
                    "goal": {"type": "string", "minLength": 1, "maxLength": 32768},
                    "constraints": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 8192}, "maxItems": 64},
                    "success_conditions": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 8192}, "maxItems": 64},
                    "risk": {"type": "string", "enum": ["low", "medium", "high", "critical"], "default": "medium"},
                    "budget": {
                        "type": "object",
                        "properties": {
                            "max_tokens": {"type": "integer", "minimum": 1},
                            "max_cost_usd": {"type": "number", "minimum": 0},
                            "max_duration_seconds": {"type": "integer", "minimum": 1}
                        },
                        "additionalProperties": false
                    },
                    "target_path": {"type": "string", "maxLength": 4096},
                    "requested_tools": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 512}, "uniqueItems": true, "maxItems": 128},
                    "context_token_budget": {"type": "integer", "minimum": 64, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS},
                    "estimated_output_tokens": {"type": "integer", "minimum": 1, "maximum": 100_000, "default": 8192},
                    "max_model_calls": {"type": "integer", "minimum": 1, "maximum": 32, "default": 16}
                },
                "required": ["request_key", "project_id", "goal"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            }
        }),
        json!({
            "name": "mcg_continue_task",
            "title": "Continue an MCG Consumer Task",
            "description": "Retry a blocked consumer task or resume one whose exact governed approval has been resolved, without repeating completed model or tool work.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "format": "uuid"},
                    "action": {"type": "string", "enum": ["retry", "resume"]},
                    "target_path": {"type": "string", "maxLength": 4096},
                    "requested_tools": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 512}, "uniqueItems": true, "maxItems": 128},
                    "context_token_budget": {"type": "integer", "minimum": 64, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS},
                    "estimated_output_tokens": {"type": "integer", "minimum": 1, "maximum": 100_000, "default": 8192},
                    "max_model_calls": {"type": "integer", "minimum": 1, "maximum": 32, "default": 16}
                },
                "required": ["task_id", "action"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            }
        }),
        json!({
            "name": "mcg_cancel_task",
            "title": "Cancel an MCG Consumer Task",
            "description": "Atomically cancel one nonterminal durable task and its consumer run while retaining all history and completed evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {"task_id": {"type": "string", "format": "uuid"}},
                "required": ["task_id"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_record_evaluation",
            "title": "Record MCG Evaluation Evidence",
            "description": "Record one complete immutable EvaluationReport. MCG validates run, task, evaluator independence, condition coverage, outcome consistency, and evidence policy before accepting it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report": {"type": "object", "description": "A complete versioned MCG EvaluationReport contract."}
                },
                "required": ["report"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "mcg_run_agent",
            "title": "Run a Governed MCG Agent",
            "description": "Idempotently create and start a durable task/run, then execute its one bounded agent turn through configured providers and MCG's policy broker. Coder and tool-worker tools must be explicitly listed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request_key": {"type": "string", "minLength": 1, "maxLength": 128, "pattern": "^[A-Za-z0-9._:-]+$"},
                    "project_id": {"type": "string", "format": "uuid"},
                    "goal": {"type": "string", "minLength": 1, "maxLength": 20000},
                    "role": {"type": "string", "enum": ["conversation", "coder", "tool_worker"], "default": "conversation"},
                    "success_conditions": {"type": "array", "items": {"type": "string", "minLength": 1, "maxLength": 2000}, "maxItems": 20},
                    "risk": {"type": "string", "enum": ["low", "medium", "high", "critical"], "default": "medium"},
                    "budget": {
                        "type": "object",
                        "properties": {
                            "max_tokens": {"type": "integer", "minimum": 1},
                            "max_cost_usd": {"type": "number", "minimum": 0},
                            "max_duration_seconds": {"type": "integer", "minimum": 1}
                        },
                        "additionalProperties": false
                    },
                    "target_path": {"type": "string"},
                    "requested_tools": {
                        "type": "array",
                        "items": {
                            "anyOf": [
                                {"type": "string", "enum": ["fs.read", "fs.list", "fs.write", "fs.patch", "fs.mkdir", "process.exec", "git.status", "git.diff", "git.apply_patch", "system.info"]},
                                {"type": "string", "pattern": "^mcp::[A-Za-z0-9_-]{1,128}::.+$", "maxLength": 512}
                            ]
                        },
                        "uniqueItems": true,
                        "maxItems": 16
                    },
                    "context_token_budget": {"type": "integer", "minimum": 64, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS},
                    "estimated_output_tokens": {"type": "integer", "minimum": 1, "maximum": 100_000, "default": 8192},
                    "max_model_calls": {"type": "integer", "minimum": 1, "maximum": 32, "default": 16}
                },
                "required": ["request_key", "project_id", "goal"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            }
        }),
        json!({
            "name": "mcg_plugins_list",
            "title": "List MCG Plugins",
            "description": "List installed plugin heads, active manifests, trust provenance, requested capabilities, exact grants, and enabled state without module or invocation content.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_annotations,
        }),
        json!({
            "name": "mcg_plugins_configure",
            "title": "Configure an MCG Plugin",
            "description": "Replace one plugin's complete scoped grant set or explicitly enable/disable it using optimistic revision control. Grant changes always disable the plugin.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin_id": {"type": "string", "minLength": 1, "maxLength": 128},
                    "expected_revision": {"type": "integer", "minimum": 1},
                    "action": {"type": "string", "enum": ["set_grants", "set_enabled"]},
                    "grants": {"type": "array", "maxItems": 5, "items": {"type": "object"}},
                    "enabled": {"type": "boolean"}
                },
                "required": ["plugin_id", "expected_revision", "action"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "mcg_plugins_invoke",
            "title": "Invoke an MCG Plugin",
            "description": "Invoke one explicitly enabled sandboxed plugin with a caller-owned UUID. Capability requests remain subject to the task project, grants, policy, approvals, audit, and non-replayable uncertainty rules.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin_id": {"type": "string", "minLength": 1, "maxLength": 128},
                    "invocation_id": {"type": "string", "format": "uuid"},
                    "task_id": {"type": "string", "format": "uuid"},
                    "input": {}
                },
                "required": ["plugin_id", "invocation_id", "input"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true}
        }),
        json!({
            "name": "mcg_system_status",
            "title": "Read MCG System Status",
            "description": "Return the daemon version, readiness, mode, and restart-recovery counts.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "annotations": read_annotations,
        }),
    ]
}

fn session_id_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"session_id": {"type": "string", "format": "uuid"}},
        "required": ["session_id"],
        "additionalProperties": false
    })
}

fn computer_actions_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session_id": {"type": "string", "format": "uuid"},
            "batch_id": {"type": "string", "format": "uuid"},
            "expected_frame_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "actions": {
                "type": "array",
                "minItems": 1,
                "maxItems": 32,
                "items": {
                    "oneOf": [
                        {"type": "object", "properties": {"type": {"const": "move"}, "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0}}, "required": ["type", "x", "y"], "additionalProperties": false},
                        {"type": "object", "properties": {"type": {"const": "click"}, "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0}, "button": {"type": "string", "enum": ["left", "middle", "right"]}}, "required": ["type", "x", "y", "button"], "additionalProperties": false},
                        {"type": "object", "properties": {"type": {"const": "scroll"}, "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0}, "delta_x": {"type": "integer", "minimum": -100, "maximum": 100}, "delta_y": {"type": "integer", "minimum": -100, "maximum": 100}}, "required": ["type", "x", "y", "delta_x", "delta_y"], "additionalProperties": false},
                        {"type": "object", "properties": {"type": {"const": "type_text"}, "text": {"type": "string", "maxLength": 4096}}, "required": ["type", "text"], "additionalProperties": false},
                        {"type": "object", "properties": {"type": {"const": "key"}, "key": {"type": "string", "enum": ["enter", "tab", "escape", "backspace", "delete", "space", "home", "end", "page_up", "page_down", "arrow_up", "arrow_down", "arrow_left", "arrow_right"]}}, "required": ["type", "key"], "additionalProperties": false}
                    ]
                }
            }
        },
        "required": ["session_id", "batch_id", "expected_frame_sha256", "actions"],
        "additionalProperties": false
    })
}

async fn call_tool(
    state: &ApiState,
    params: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Tool name is required",
        )
    })?;
    let arguments = match params.get("arguments") {
        None => serde_json::Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        Some(_) => {
            return Ok(tool_error(
                "invalid_arguments",
                "Tool arguments must be an object",
            ));
        }
    };
    match name {
        "mcg_system_status" => {
            require_only_keys(&arguments, &[])?;
            tool_success(serde_json::to_value(state.runtime.status()))
        }
        "mcg_query_project" => query_project(state, &arguments),
        "mcg_query_organization" => query_organization(state, &arguments),
        "mcg_import_organization" => import_organization(state, &arguments),
        "mcg_organization_publishers" => organization_publishers(state, &arguments),
        "mcg_organization_publisher_mutate" => organization_publisher_mutate(state, &arguments),
        "mcg_organization_share_reviews" => organization_share_reviews(state, &arguments),
        "mcg_organization_share_mutate" => organization_share_mutate(state, &arguments),
        "mcg_query_task" => query_task(state, &arguments),
        "mcg_query_run" => query_run(state, &arguments),
        "mcg_project_context" => project_context(state, &arguments),
        "mcg_project_topology_plan" => project_topology_plan(state, &arguments),
        "mcg_reviewed_topology_plans" => reviewed_topology_plans(state, &arguments),
        "mcg_reviewed_topology_plan_mutate" => {
            reviewed_topology_plan_mutate(state, &arguments).await
        }
        "mcg_memory_search" => memory_search(state, &arguments),
        "mcg_project_memory_transfer_preview" => memory_transfer_preview(state, &arguments),
        "mcg_project_memory_transfers" => memory_transfers(state, &arguments),
        "mcg_project_memory_transfer" => memory_transfer(state, &arguments),
        "mcg_scheduler_list" => scheduler_list(state, &arguments),
        "mcg_scheduler_create" => scheduler_create(state, &arguments),
        "mcg_scheduler_remove" => scheduler_remove(state, &arguments),
        "mcg_platform_capabilities" => platform_capabilities(state, &arguments),
        "mcg_platform_sessions" => platform_sessions(state, &arguments).await,
        "mcg_platform_request" => platform_request(state, &arguments),
        "mcg_platform_close" => platform_close(state, &arguments).await,
        "mcg_browser_attach" => browser_attach(state, &arguments).await,
        "mcg_browser_navigate" => browser_navigate(state, &arguments).await,
        "mcg_browser_handoff" => browser_handoff(state, &arguments).await,
        "mcg_browser_reclaim" => browser_reclaim(state, &arguments).await,
        "mcg_computer_attach" => computer_attach(state, &arguments).await,
        "mcg_computer_capture" => computer_capture(state, &arguments).await,
        "mcg_computer_actions" => computer_actions(state, &arguments).await,
        "mcg_computer_handoff" => computer_handoff(state, &arguments).await,
        "mcg_computer_reclaim" => computer_reclaim(state, &arguments).await,
        "mcg_experiments_inspect" => experiments_inspect(state, &arguments),
        "mcg_experiments_mutate" => experiments_mutate(state, &arguments),
        "mcg_governor_inspect" => governor_inspect(state, &arguments),
        "mcg_governor_propose" => governor_propose(state, &arguments).await,
        "mcg_sites_list" => sites_list(state, &arguments),
        "mcg_sites_register" => sites_register(state, &arguments),
        "mcg_sites_update" => sites_update(state, &arguments),
        "mcg_sites_monitor" => sites_monitor(state, &arguments),
        "mcg_sites_health" => sites_health(state, &arguments).await,
        "mcg_sites_deploy" => sites_deploy(state, &arguments).await,
        "mcg_ssh_hosts" => ssh_hosts(state, &arguments),
        "mcg_ssh_register" => ssh_register(state, &arguments),
        "mcg_ssh_run" => ssh_run(state, &arguments).await,
        "mcg_ssh_workflows" => ssh_workflows(state, &arguments),
        "mcg_ssh_workflow_mutate" => ssh_workflow_mutate(state, &arguments).await,
        "mcg_plugins_list" => plugins_list(state, &arguments),
        "mcg_plugins_configure" => plugins_configure(state, &arguments),
        "mcg_plugins_invoke" => plugins_invoke(state, &arguments).await,
        "mcg_run_task" => run_task(state, &arguments).await,
        "mcg_continue_task" => continue_task(state, &arguments).await,
        "mcg_cancel_task" => cancel_task(state, &arguments),
        "mcg_record_evaluation" => record_evaluation(state, &arguments),
        "mcg_run_agent" => run_agent(state, &arguments).await,
        _ => Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Unknown tool name",
        )),
    }
}

fn platform_capabilities(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    tool_success(Ok(json!({
        "capabilities": state.runtime.platform_capabilities()
    })))
}

fn plugins_list(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    let installations = match state.runtime.plugin_installations() {
        Ok(installations) => installations,
        Err(error) => return Ok(plugin_mcp_error(&error)),
    };
    let mut plugins = Vec::with_capacity(installations.len());
    for installation in installations {
        let revision = match state
            .runtime
            .plugin_revision(installation.active_revision_id)
        {
            Ok(Some(revision)) => revision,
            Ok(None) => {
                return Ok(tool_error(
                    "plugin_catalog_invalid",
                    "An installed plugin active revision is missing",
                ));
            }
            Err(error) => return Ok(plugin_mcp_error(&error)),
        };
        plugins.push(json!({
            "installation": installation,
            "active_revision": revision,
        }));
    }
    tool_success(Ok(json!({"plugins": plugins})))
}

fn plugins_configure(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<PluginConfigureArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Plugin configuration arguments are invalid",
        ));
    };
    let result = match request.action.as_str() {
        "set_grants" if request.enabled.is_none() => state.runtime.set_plugin_capability_grants(
            &request.plugin_id,
            request.expected_revision,
            request.grants.unwrap_or_default(),
        ),
        "set_enabled" if request.grants.is_none() && request.enabled.is_some() => {
            state.runtime.set_plugin_enabled(
                &request.plugin_id,
                request.expected_revision,
                request.enabled.unwrap_or(false),
            )
        }
        _ => {
            return Ok(tool_error(
                "invalid_arguments",
                "set_grants accepts only grants; set_enabled accepts only enabled",
            ));
        }
    };
    match result {
        Ok(installation) => tool_success(serde_json::to_value(installation)),
        Err(error) => Ok(plugin_mcp_error(&error)),
    }
}

async fn plugins_invoke(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<PluginInvokeArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Plugin invocation arguments are invalid",
        ));
    };
    let Ok(invocation_id) = PluginInvocationId::from_str(&request.invocation_id) else {
        return Ok(tool_error(
            "invalid_arguments",
            "Plugin invocation id must be a UUID",
        ));
    };
    let task_id = match request.task_id {
        Some(task_id) => match TaskId::from_str(&task_id) {
            Ok(task_id) => Some(task_id),
            Err(_) => {
                return Ok(tool_error(
                    "invalid_arguments",
                    "Plugin task id must be a UUID",
                ));
            }
        },
        None => None,
    };
    match state
        .runtime
        .invoke_sandboxed_plugin(invocation_id, &request.plugin_id, task_id, request.input)
        .await
    {
        Ok(mutation) => tool_success(serde_json::to_value(mutation)),
        Err(error) => Ok(plugin_mcp_error(&error)),
    }
}

fn plugin_mcp_error(error: &mcg_runtime::RuntimeError) -> Value {
    let code = if error.is_not_found() {
        "plugin_not_found"
    } else if error.is_conflict() {
        "plugin_conflict"
    } else if error.is_invalid_request() {
        "invalid_arguments"
    } else {
        "runtime_error"
    };
    tool_error(code, &error.to_string())
}

fn experiments_inspect(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<ExperimentInspectArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Measured improvement inspection arguments are invalid",
        ));
    };
    let result = match request.scope {
        ExperimentInspectScope::Observations => {
            if request.id.is_some() {
                return Ok(tool_error(
                    "invalid_arguments",
                    "Observation listing does not accept an id",
                ));
            }
            state
                .runtime
                .supervision_observations()
                .map(|observations| json!({"observations": observations}))
        }
        ExperimentInspectScope::Organization => {
            let Some(id) = request.id else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "Organization id is required",
                ));
            };
            let Ok(id) = OrganizationId::from_str(&id) else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "Organization id is invalid",
                ));
            };
            state
                .runtime
                .experiments(id)
                .map(|experiments| json!({"experiments": experiments}))
        }
        ExperimentInspectScope::Experiment => {
            let Some(id) = request.id else {
                return Ok(tool_error("invalid_arguments", "Experiment id is required"));
            };
            let Ok(id) = ExperimentId::from_str(&id) else {
                return Ok(tool_error("invalid_arguments", "Experiment id is invalid"));
            };
            let experiment = state.runtime.experiment(id);
            let shadow = state.runtime.shadow_executions(id);
            let benchmarks = state.runtime.experiment_benchmarks(id);
            let decisions = state.runtime.experiment_decisions(id);
            match (experiment, shadow, benchmarks, decisions) {
                (Ok(Some(experiment)), Ok(shadow), Ok(benchmarks), Ok(decisions)) => Ok(json!({
                    "experiment": experiment,
                    "shadow_executions": shadow,
                    "benchmarks": benchmarks,
                    "decisions": decisions,
                })),
                (Ok(None), _, _, _) => {
                    return Ok(tool_error(
                        "experiment_not_found",
                        "Experiment was not found",
                    ));
                }
                (Err(error), _, _, _)
                | (_, Err(error), _, _)
                | (_, _, Err(error), _)
                | (_, _, _, Err(error)) => Err(error),
            }
        }
    };
    match result {
        Ok(value) => tool_success(Ok(value)),
        Err(error) => Ok(experiment_error(&error)),
    }
}

fn experiments_mutate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<ExperimentMutationArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Measured experiment action arguments are invalid",
        ));
    };
    let result = match request.action {
        ExperimentMutationAction::Candidate => {
            decode_experiment_action::<OrganizationGenome, _, _>(request.payload, |value| {
                state.runtime.record_candidate_revision(&value)
            })
        }
        ExperimentMutationAction::Observation => {
            decode_experiment_action::<SupervisionObservation, _, _>(request.payload, |value| {
                state.runtime.record_supervision_observation(&value)
            })
        }
        ExperimentMutationAction::Create => {
            decode_experiment_action::<ExperimentDefinition, _, _>(request.payload, |value| {
                state.runtime.create_experiment(&value)
            })
        }
        ExperimentMutationAction::Transition => decode_experiment_action::<
            ExperimentTransitionPayload,
            _,
            _,
        >(request.payload, |value| {
            state
                .runtime
                .transition_experiment(value.experiment_id, value.expected, value.next)
        }),
        ExperimentMutationAction::Shadow => {
            decode_experiment_action::<ShadowExecutionReceipt, _, _>(request.payload, |value| {
                state.runtime.record_shadow_execution(&value)
            })
        }
        ExperimentMutationAction::Benchmark => {
            decode_experiment_action::<ExperimentBenchmark, _, _>(request.payload, |value| {
                state.runtime.record_experiment_benchmark(&value)
            })
        }
        ExperimentMutationAction::Decision => {
            decode_experiment_action::<ExperimentDecision, _, _>(request.payload, |value| {
                state.runtime.decide_experiment(&value)
            })
        }
    };
    match result {
        Ok(value) => tool_success(Ok(value)),
        Err(ExperimentActionFailure::InvalidPayload) => Ok(tool_error(
            "invalid_or_rejected_experiment_action",
            "The versioned action payload was invalid",
        )),
        Err(ExperimentActionFailure::Runtime(error)) => Ok(experiment_error(&error)),
        Err(ExperimentActionFailure::Serialization) => Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not serialize the experiment action result",
        )),
    }
}

fn governor_inspect(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<GovernorInspectArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Governor inspection arguments are invalid",
        ));
    };
    let result = match request.scope {
        GovernorInspectScope::Policy => {
            if request.id.is_some() {
                return Ok(tool_error(
                    "invalid_arguments",
                    "Policy inspection does not accept an id",
                ));
            }
            Ok(json!({"policy": state.runtime.governor_control_policy()}))
        }
        GovernorInspectScope::Organization => {
            let Some(id) = request
                .id
                .and_then(|value| value.parse::<OrganizationId>().ok())
            else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "A valid organization id is required",
                ));
            };
            state
                .runtime
                .governor_proposals(id)
                .map(|proposals| json!({"proposals": proposals}))
        }
        GovernorInspectScope::Proposal => {
            let Some(id) = request
                .id
                .and_then(|value| value.parse::<GovernorProposalId>().ok())
            else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "A valid proposal id is required",
                ));
            };
            match state.runtime.governor_proposal(id) {
                Ok(Some(proposal)) => Ok(json!({
                    "proposal": proposal,
                    "implementation_brief": proposal.implementation_brief(),
                    "policy": state.runtime.governor_control_policy(),
                })),
                Ok(None) => {
                    return Ok(tool_error(
                        "proposal_not_found",
                        "Governor proposal was not found",
                    ));
                }
                Err(error) => Err(error),
            }
        }
    };
    match result {
        Ok(value) => tool_success(Ok(value)),
        Err(error) => Ok(experiment_error(&error)),
    }
}

async fn governor_propose(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<GovernorMutationArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Governor proposal arguments are invalid",
        ));
    };
    let result = match request.action {
        GovernorMutationAction::Create => {
            decode_experiment_action::<GovernorProposal, _, _>(request.payload, |value| {
                state.runtime.create_governor_proposal(&value)
            })
        }
        GovernorMutationAction::Transition => {
            let value = serde_json::from_value::<GovernorTransitionPayload>(request.payload)
                .map_err(|_| ExperimentActionFailure::InvalidPayload);
            value.and_then(|value| {
                if matches!(
                    value.next,
                    GovernorProposalState::Approved
                        | GovernorProposalState::Promoted
                        | GovernorProposalState::Reverted
                ) {
                    return Err(ExperimentActionFailure::InvalidPayload);
                }
                let mutation = state
                    .runtime
                    .transition_governor_proposal(value.proposal_id, value.expected, value.next)
                    .map_err(ExperimentActionFailure::Runtime)?;
                serde_json::to_value(mutation).map_err(|_| ExperimentActionFailure::Serialization)
            })
        }
        GovernorMutationAction::RequestImplementation => {
            let value = serde_json::from_value::<GovernorImplementationPayload>(request.payload)
                .map_err(|_| ExperimentActionFailure::InvalidPayload);
            match value {
                Ok(value) => state
                    .runtime
                    .request_governor_proposal_implementation(
                        value.proposal_id,
                        value.conversation_id,
                    )
                    .await
                    .map_err(ExperimentActionFailure::Runtime),
                Err(error) => Err(error),
            }
        }
    };
    match result {
        Ok(value) => tool_success(Ok(value)),
        Err(ExperimentActionFailure::InvalidPayload) => Ok(tool_error(
            "invalid_or_rejected_governor_proposal",
            "The proposal was malformed, targeted a protected surface, or understated risk",
        )),
        Err(ExperimentActionFailure::Runtime(error)) => Ok(experiment_error(&error)),
        Err(ExperimentActionFailure::Serialization) => Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not serialize the Governor proposal result",
        )),
    }
}

enum ExperimentActionFailure {
    InvalidPayload,
    Runtime(mcg_runtime::RuntimeError),
    Serialization,
}

fn decode_experiment_action<T, R, F>(
    payload: Value,
    action: F,
) -> Result<Value, ExperimentActionFailure>
where
    T: serde::de::DeserializeOwned,
    R: serde::Serialize,
    F: FnOnce(T) -> Result<R, mcg_runtime::RuntimeError>,
{
    let value =
        serde_json::from_value(payload).map_err(|_| ExperimentActionFailure::InvalidPayload)?;
    let result = action(value).map_err(ExperimentActionFailure::Runtime)?;
    serde_json::to_value(result).map_err(|_| ExperimentActionFailure::Serialization)
}

fn experiment_error(error: &mcg_runtime::RuntimeError) -> Value {
    if error.is_not_found() {
        tool_error("experiment_not_found", "Experiment was not found")
    } else if error.is_conflict() {
        tool_error("conflict", "Measured experiment state or evidence is stale")
    } else if error.is_invalid_request() {
        tool_error(
            "invalid_arguments",
            "Measured experiment evidence is invalid",
        )
    } else {
        tool_error(
            "runtime_error",
            "MCG could not inspect measured improvement",
        )
    }
}

async fn platform_sessions(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    match state.runtime.platform_sessions().await {
        Ok(sessions) => tool_success(Ok(json!({"sessions": sessions}))),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not list durable platform sessions",
        )),
    }
}

fn platform_request(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request =
        serde_json::from_value::<PlatformRequestArguments>(Value::Object(arguments.clone()));
    let Ok(request) = request else {
        return Ok(tool_error(
            "invalid_arguments",
            "Platform session arguments do not match the advertised schema",
        ));
    };
    match state.runtime.request_platform_session(
        &PlatformSessionRequest {
            id: request.session_id,
            kind: request.kind,
            project_id: request.project_id,
            task_id: request.task_id,
            display_name: request.display_name,
            allowed_origins: request.allowed_origins,
            computer_capabilities: request.computer_capabilities,
            computer_target: request.computer_target,
            lifetime_seconds: request.lifetime_seconds,
        },
        Utc::now(),
    ) {
        Ok(result) => tool_success(Ok(json!({
            "replayed": !result.created,
            "session": result.session
        }))),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "conflict",
            "session_id was already used with a different definition",
        )),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "reference_not_found",
            "The requested project or task is not known to MCG",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_arguments",
            "The platform session violates a durable runtime invariant",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not reserve the platform session",
        )),
    }
}

async fn platform_close(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request =
        serde_json::from_value::<PlatformCloseArguments>(Value::Object(arguments.clone()));
    let Ok(request) = request else {
        return Ok(tool_error(
            "invalid_arguments",
            "Platform close arguments do not match the advertised schema",
        ));
    };
    match state
        .runtime
        .close_platform_session(request.session_id, &request.reason, Utc::now())
        .await
    {
        Ok(session) => tool_success(Ok(json!({"session": session}))),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "session_not_found",
            "The requested platform session is not known to MCG",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_state",
            "The platform session cannot be closed from its current state",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not close the platform session",
        )),
    }
}

async fn browser_attach(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<BrowserAttachArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Browser attach arguments do not match the advertised schema",
        ));
    };
    browser_result(
        state
            .runtime
            .attach_browser_session(request.session_id, request.mode, Utc::now())
            .await,
        "attach",
    )
}

async fn browser_navigate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<BrowserNavigateArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Browser navigation arguments do not match the advertised schema",
        ));
    };
    browser_result(
        state
            .runtime
            .navigate_browser_session(request.session_id, &request.url)
            .await,
        "navigate",
    )
}

async fn browser_handoff(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<BrowserSessionArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Browser handoff arguments do not match the advertised schema",
        ));
    };
    browser_result(
        state
            .runtime
            .handoff_browser_session(request.session_id, Utc::now())
            .await,
        "hand off",
    )
}

async fn browser_reclaim(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<BrowserSessionArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Browser reclaim arguments do not match the advertised schema",
        ));
    };
    browser_result(
        state
            .runtime
            .reclaim_browser_session(request.session_id, Utc::now())
            .await,
        "reclaim",
    )
}

fn browser_result<T: serde::Serialize>(
    result: Result<T, mcg_runtime::RuntimeError>,
    operation: &str,
) -> Result<Value, RpcFailure> {
    match result {
        Ok(value) => tool_success(serde_json::to_value(value)),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "session_not_found",
            "The requested browser session is not known to MCG",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_state",
            &format!("MCG could not {operation} the browser in its current authority state"),
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            &format!("MCG could not {operation} the browser"),
        )),
    }
}

async fn computer_attach(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Some(request) = computer_session_arguments(arguments) else {
        return Ok(tool_error(
            "invalid_arguments",
            "Computer session_id is invalid",
        ));
    };
    computer_result(
        state
            .runtime
            .attach_computer_session(request.session_id, Utc::now())
            .await,
        "attach",
    )
}

async fn computer_capture(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Some(request) = computer_session_arguments(arguments) else {
        return Ok(tool_error(
            "invalid_arguments",
            "Computer session_id is invalid",
        ));
    };
    match state
        .runtime
        .capture_computer_session(request.session_id)
        .await
    {
        Ok(frame) => {
            let structured = json!({"frame": frame_metadata(&frame)});
            computer_frame_success(&frame, &structured)
        }
        Err(error) => Ok(computer_error(&error, "capture")),
    }
}

async fn computer_actions(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<ComputerActionsArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Computer action arguments do not match the advertised schema",
        ));
    };
    match state
        .runtime
        .apply_computer_actions(
            request.batch_id,
            request.session_id,
            &request.expected_frame_sha256,
            &request.actions,
        )
        .await
    {
        Ok(mutation) => match &mutation.result {
            Some(result) => {
                let structured = json!({
                    "batch": mutation.batch,
                    "replayed": mutation.replayed,
                    "prior_frame_sha256": result.prior_frame_sha256,
                    "actions_executed": result.actions_executed,
                    "frame": frame_metadata(&result.frame)
                });
                computer_frame_success(&result.frame, &structured)
            }
            None => tool_success(Ok(json!({
                "batch": mutation.batch,
                "replayed": mutation.replayed,
                "result": null
            }))),
        },
        Err(error) => Ok(computer_error(&error, "apply actions to")),
    }
}

async fn computer_handoff(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Some(request) = computer_session_arguments(arguments) else {
        return Ok(tool_error(
            "invalid_arguments",
            "Computer session_id is invalid",
        ));
    };
    computer_result(
        state
            .runtime
            .handoff_computer_session(request.session_id, Utc::now())
            .await,
        "hand off",
    )
}

async fn computer_reclaim(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Some(request) = computer_session_arguments(arguments) else {
        return Ok(tool_error(
            "invalid_arguments",
            "Computer session_id is invalid",
        ));
    };
    computer_result(
        state
            .runtime
            .reclaim_computer_session(request.session_id, Utc::now())
            .await,
        "reclaim",
    )
}

fn computer_session_arguments(
    arguments: &serde_json::Map<String, Value>,
) -> Option<BrowserSessionArguments> {
    serde_json::from_value(Value::Object(arguments.clone())).ok()
}

fn computer_result<T: serde::Serialize>(
    result: Result<T, mcg_runtime::RuntimeError>,
    operation: &str,
) -> Result<Value, RpcFailure> {
    match result {
        Ok(value) => tool_success(serde_json::to_value(value)),
        Err(error) => Ok(computer_error(&error, operation)),
    }
}

fn computer_error(error: &mcg_runtime::RuntimeError, operation: &str) -> Value {
    if error.is_not_found() {
        tool_error(
            "session_not_found",
            "The requested computer session is not known to MCG",
        )
    } else if error.is_invalid_request() {
        tool_error(
            "invalid_state",
            &format!("MCG could not {operation} the computer in its current authority state"),
        )
    } else {
        tool_error(
            "runtime_error",
            &format!("MCG could not {operation} the computer"),
        )
    }
}

fn sites_list(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesListArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site list arguments are invalid",
        ));
    };
    if let Some(site_id) = request.site_id {
        let site = match state.runtime.site(site_id) {
            Ok(Some(site)) => site,
            Ok(None) => {
                return Ok(tool_error(
                    "site_not_found",
                    "The requested Site is not known to MCG",
                ));
            }
            Err(_) => return Ok(tool_error("runtime_error", "MCG could not read the Site")),
        };
        let Ok(revisions) = state.runtime.site_revisions(site_id) else {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the Site revision history",
            ));
        };
        let Ok(deployments) = state.runtime.site_deployments(site_id) else {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the Site deployment history",
            ));
        };
        let Ok(health_checks) = state.runtime.site_health_checks(site_id) else {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the Site health history",
            ));
        };
        let Ok(monitor) = state.runtime.site_monitor(site_id) else {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the Site monitor",
            ));
        };
        return tool_success(Ok(json!({
            "site": site,
            "revisions": revisions,
            "deployments": deployments,
            "health_checks": health_checks,
            "monitor": monitor
        })));
    }
    match state.runtime.sites() {
        Ok(sites) => tool_success(Ok(json!({"sites": sites}))),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not list durable Sites",
        )),
    }
}

fn sites_register(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesRegisterArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site registration arguments do not match the advertised schema",
        ));
    };
    match state.runtime.register_site(&SiteRegistrationRequest {
        id: request.site_id,
        project_id: request.project_id,
        display_name: request.display_name,
        environment: request.environment,
        public_url: request.public_url,
        health_url: request.health_url,
        hosting_provider: request.hosting_provider,
        deployment_command: request.deployment_command,
        credential_reference: request.credential_reference,
        state: request.state,
    }) {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(site_error(&error, "register")),
    }
}

fn sites_update(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesUpdateArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site update arguments do not match the advertised schema",
        ));
    };
    match state.runtime.update_site(
        request.site_id,
        &SiteUpdateRequest {
            expected_revision: request.expected_revision,
            display_name: request.display_name,
            environment: request.environment,
            public_url: request.public_url,
            health_url: request.health_url,
            hosting_provider: request.hosting_provider,
            deployment_command: request.deployment_command,
            credential_reference: request.credential_reference,
            state: request.state,
            reason: request.reason,
        },
    ) {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(site_error(&error, "update")),
    }
}

fn sites_monitor(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesMonitorArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site monitor arguments do not match the advertised schema",
        ));
    };
    match state.runtime.create_site_monitor(
        request.site_id,
        &SiteMonitorRequest {
            schedule_id: request.schedule_id,
            interval_seconds: request.interval_seconds,
            expected_status: request.expected_status,
            first_run_at: request.first_run_at,
        },
    ) {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(site_error(&error, "configure monitor")),
    }
}

async fn sites_health(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesHealthArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site health arguments are invalid",
        ));
    };
    match state
        .runtime
        .check_site_health(request.site_id, request.check_id)
        .await
    {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(site_error(&error, "check")),
    }
}

async fn sites_deploy(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SitesDeployArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "Site deployment arguments are invalid",
        ));
    };
    match state
        .runtime
        .deploy_site(request.site_id, request.deployment_id, request.task_id)
        .await
    {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(site_error(&error, "deploy")),
    }
}

fn ssh_hosts(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) = serde_json::from_value::<SshHostsArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "SSH host arguments are invalid",
        ));
    };
    if let Some(host_id) = request.host_id {
        let Some(host) = state.runtime.ssh_host(host_id).map_err(|error| {
            RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
        })?
        else {
            return Ok(tool_error(
                "ssh_host_not_found",
                "The requested SSH host is not known to MCG",
            ));
        };
        let revisions = state.runtime.ssh_host_revisions(host_id).map_err(|error| {
            RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
        })?;
        let operations = state.runtime.ssh_operations(host_id).map_err(|error| {
            RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
        })?;
        tool_success(Ok(
            json!({"host": host, "revisions": revisions, "operations": operations}),
        ))
    } else {
        let hosts = state.runtime.ssh_hosts().map_err(|error| {
            RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
        })?;
        tool_success(Ok(json!({"hosts": hosts})))
    }
}

fn ssh_register(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SshRegisterArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "SSH registration arguments are invalid",
        ));
    };
    match state
        .runtime
        .register_ssh_host(&SshHostRegistrationRequest {
            id: request.host_id,
            display_name: request.display_name,
            hostname: request.hostname,
            port: request.port,
            username: request.username,
            platform: SshHostPlatform::Posix,
            authentication: SshAuthentication::Agent,
            known_hosts_entry: request.known_hosts_entry,
            host_key_sha256: request.host_key_sha256,
            capabilities: request.capabilities,
            state: SshHostState::Active,
        }) {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(ssh_error(&error, "register")),
    }
}

async fn ssh_run(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) = serde_json::from_value::<SshRunArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "SSH operation arguments are invalid",
        ));
    };
    match state
        .runtime
        .run_ssh_operation(
            request.host_id,
            request.operation_id,
            request.task_id,
            request.operation,
            request.timeout_seconds,
        )
        .await
    {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(ssh_error(&error, "run")),
    }
}

fn ssh_workflows(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SshWorkflowsArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "SSH workflow query arguments are invalid",
        ));
    };
    if let Some(workflow_id) = request.workflow_id {
        match state.runtime.ssh_workflow(workflow_id) {
            Ok(Some(workflow)) => tool_success(Ok(json!({"workflow": workflow}))),
            Ok(None) => Ok(tool_error(
                "ssh_workflow_not_found",
                "The requested SSH workflow is not known to MCG",
            )),
            Err(error) => Err(RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                error.to_string(),
            )),
        }
    } else {
        let workflows = state.runtime.ssh_workflows().map_err(|error| {
            RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
        })?;
        tool_success(Ok(json!({"workflows": workflows})))
    }
}

async fn ssh_workflow_mutate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<SshWorkflowMutateArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "SSH workflow mutation arguments are invalid",
        ));
    };
    let result = match request.action.as_str() {
        "create" if request.display_name.is_some() && request.steps.is_some() => state
            .runtime
            .create_ssh_workflow(&SshWorkflowCreateRequest {
                id: request.workflow_id,
                display_name: request.display_name.unwrap_or_default(),
                task_id: request.task_id,
                steps: request.steps.unwrap_or_default(),
            }),
        "run"
            if request.display_name.is_none()
                && request.task_id.is_none()
                && request.steps.is_none() =>
        {
            state.runtime.run_ssh_workflow(request.workflow_id).await
        }
        "cancel"
            if request.display_name.is_none()
                && request.task_id.is_none()
                && request.steps.is_none() =>
        {
            state.runtime.cancel_ssh_workflow(request.workflow_id)
        }
        _ => {
            return Ok(tool_error(
                "invalid_arguments",
                "Create requires display_name and steps; run/cancel accept only action and workflow_id",
            ));
        }
    };
    match result {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) => Ok(ssh_error(&error, "mutate workflow")),
    }
}

fn ssh_error(error: &mcg_runtime::RuntimeError, operation: &str) -> Value {
    if error.is_not_found() {
        tool_error(
            "ssh_host_not_found",
            "The requested SSH host or task is not known to MCG",
        )
    } else if matches!(
        error,
        mcg_runtime::RuntimeError::Tool(mcg_tools::ToolError::ApprovalRequired { .. })
    ) {
        tool_error("approval_required", &error.to_string())
    } else if error.is_conflict() {
        tool_error(
            "conflict",
            &format!("MCG could not {operation} SSH due to an identity conflict"),
        )
    } else if error.is_invalid_request() {
        tool_error(
            "invalid_arguments",
            &format!("MCG could not {operation} the SSH request"),
        )
    } else {
        tool_error("runtime_error", &format!("MCG could not {operation} SSH"))
    }
}

fn site_error(error: &mcg_runtime::RuntimeError, operation: &str) -> Value {
    if error.is_not_found() {
        tool_error(
            "site_not_found",
            "The requested Site or project is not known to MCG",
        )
    } else if matches!(
        error,
        mcg_runtime::RuntimeError::Tool(mcg_tools::ToolError::ApprovalRequired { .. })
    ) {
        tool_error("approval_required", &error.to_string())
    } else if error.is_conflict() {
        tool_error(
            "conflict",
            &format!("MCG could not {operation} the Site due to identity conflict"),
        )
    } else if error.is_invalid_request() {
        tool_error(
            "invalid_arguments",
            &format!("MCG could not {operation} the Site definition"),
        )
    } else {
        tool_error(
            "runtime_error",
            &format!("MCG could not {operation} the Site"),
        )
    }
}

fn frame_metadata(frame: &mcg_tools::ComputerUseFrame) -> Value {
    json!({
        "observation": frame.observation,
        "sha256": frame.sha256,
        "media_type": frame.media_type,
        "captured_at": frame.captured_at,
    })
}

fn computer_frame_success(
    frame: &mcg_tools::ComputerUseFrame,
    structured_content: &Value,
) -> Result<Value, RpcFailure> {
    let text = serde_json::to_string_pretty(&structured_content).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not serialize the computer result",
        )
    })?;
    Ok(with_server_info(json!({
        "resultType": "complete",
        "content": [
            {"type": "text", "text": text},
            {"type": "image", "data": frame.png_base64, "mimeType": frame.media_type}
        ],
        "structuredContent": structured_content,
        "isError": false,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExperimentInspectScope {
    Observations,
    Organization,
    Experiment,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperimentInspectArguments {
    scope: ExperimentInspectScope,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExperimentMutationAction {
    Candidate,
    Observation,
    Create,
    Transition,
    Shadow,
    Benchmark,
    Decision,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperimentMutationArguments {
    action: ExperimentMutationAction,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GovernorInspectScope {
    Policy,
    Organization,
    Proposal,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernorInspectArguments {
    scope: GovernorInspectScope,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GovernorMutationAction {
    Create,
    Transition,
    RequestImplementation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernorMutationArguments {
    action: GovernorMutationAction,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernorTransitionPayload {
    proposal_id: GovernorProposalId,
    expected: GovernorProposalState,
    next: GovernorProposalState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernorImplementationPayload {
    proposal_id: GovernorProposalId,
    conversation_id: ConversationId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperimentTransitionPayload {
    experiment_id: ExperimentId,
    expected: ExperimentState,
    next: ExperimentState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformRequestArguments {
    session_id: PlatformSessionId,
    kind: PlatformSessionKind,
    #[serde(default)]
    project_id: Option<ProjectId>,
    #[serde(default)]
    task_id: Option<mcg_domain::TaskId>,
    display_name: String,
    #[serde(default)]
    allowed_origins: Vec<String>,
    #[serde(default)]
    computer_capabilities: Vec<mcg_domain::ComputerUseCapability>,
    #[serde(default)]
    computer_target: Option<mcg_domain::ComputerUseTarget>,
    lifetime_seconds: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformCloseArguments {
    session_id: PlatformSessionId,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserAttachArguments {
    session_id: PlatformSessionId,
    mode: PersistentBrowserMode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserNavigateArguments {
    session_id: PlatformSessionId,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerActionsArguments {
    session_id: PlatformSessionId,
    batch_id: mcg_domain::ComputerActionBatchId,
    expected_frame_sha256: String,
    actions: Vec<mcg_tools::ComputerUseAction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserSessionArguments {
    session_id: PlatformSessionId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesListArguments {
    #[serde(default)]
    site_id: Option<SiteId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesRegisterArguments {
    site_id: SiteId,
    project_id: ProjectId,
    display_name: String,
    environment: SiteEnvironment,
    public_url: String,
    health_url: String,
    hosting_provider: String,
    #[serde(default)]
    deployment_command: Option<SiteDeploymentCommand>,
    #[serde(default)]
    credential_reference: Option<String>,
    #[serde(default = "default_site_state")]
    state: SiteState,
}

const fn default_site_state() -> SiteState {
    SiteState::Active
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesHealthArguments {
    site_id: SiteId,
    check_id: SiteHealthCheckId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesUpdateArguments {
    site_id: SiteId,
    expected_revision: u32,
    display_name: String,
    environment: SiteEnvironment,
    public_url: String,
    health_url: String,
    hosting_provider: String,
    #[serde(default)]
    deployment_command: Option<SiteDeploymentCommand>,
    #[serde(default)]
    credential_reference: Option<String>,
    state: SiteState,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesMonitorArguments {
    site_id: SiteId,
    schedule_id: ScheduleId,
    interval_seconds: u64,
    #[serde(default = "default_expected_site_status")]
    expected_status: u16,
    #[serde(default)]
    first_run_at: Option<DateTime<Utc>>,
}

const fn default_expected_site_status() -> u16 {
    200
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Exact public MCP field names are part of the wire contract.
struct SitesDeployArguments {
    site_id: SiteId,
    deployment_id: SiteDeploymentId,
    #[serde(default)]
    task_id: Option<TaskId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshHostsArguments {
    #[serde(default)]
    host_id: Option<SshHostId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshRegisterArguments {
    host_id: SshHostId,
    display_name: String,
    hostname: String,
    #[serde(default = "default_ssh_port")]
    port: u16,
    username: String,
    known_hosts_entry: String,
    host_key_sha256: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshRunArguments {
    host_id: SshHostId,
    operation_id: SshOperationId,
    #[serde(default)]
    task_id: Option<TaskId>,
    operation: SshOperationKind,
    #[serde(default = "default_ssh_timeout")]
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshWorkflowsArguments {
    #[serde(default)]
    workflow_id: Option<SshWorkflowId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshWorkflowMutateArguments {
    action: String,
    workflow_id: SshWorkflowId,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    task_id: Option<TaskId>,
    #[serde(default)]
    steps: Option<Vec<SshWorkflowStepRequest>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedTopologyPlansArguments {
    project_id: ProjectId,
    #[serde(default)]
    plan_id: Option<ReviewedTopologyPlanId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedTopologyPlanMutateArguments {
    action: String,
    project_id: ProjectId,
    plan_id: ReviewedTopologyPlanId,
    #[serde(default)]
    project_snapshot_id: Option<ProjectSnapshotId>,
    #[serde(default)]
    topology_sha256: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    steps: Option<Vec<ReviewedTopologyStepRequest>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginConfigureArguments {
    plugin_id: String,
    expected_revision: u64,
    action: String,
    #[serde(default)]
    grants: Option<Vec<PluginCapabilityGrant>>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginInvokeArguments {
    plugin_id: String,
    invocation_id: String,
    #[serde(default)]
    task_id: Option<String>,
    input: Value,
}

const fn default_ssh_port() -> u16 {
    22
}
const fn default_ssh_timeout() -> u64 {
    60
}

fn query_run(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["run_id", "replay"])?;
    let run_id = required_string(arguments, "run_id")?
        .parse::<RunId>()
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "run_id must be a UUID",
            )
        })?;
    let result = if optional_bool(arguments, "replay")?.unwrap_or(false) {
        match state.runtime.replay_agent_run(run_id) {
            Ok(report) => return tool_success(serde_json::to_value(report)),
            Err(error) => Err(error),
        }
    } else {
        match state.runtime.agent_run_view(run_id) {
            Ok(view) => return tool_success(serde_json::to_value(view)),
            Err(error) => Err(error),
        }
    };
    match result {
        Err(error) if error.is_not_found() => Ok(tool_error(
            "run_not_found",
            "The requested agent run is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not read the agent run",
        )),
        Ok(()) => unreachable!("successful queries return above"),
    }
}

#[allow(clippy::too_many_lines)] // Keeps the task, bounded evidence pages, and optional run one consistent read response.
fn query_task(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: QueryTaskArguments = serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Task query arguments do not match the advertised schema",
            )
        })?;
    if request.limit == 0 || request.limit > 1_000 {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Task query limit must be between 1 and 1000",
        ));
    }
    let task_id = request.task_id;
    let task = match state.runtime.task(task_id) {
        Ok(Some(task)) => task,
        Ok(None) => {
            return Ok(tool_error(
                "task_not_found",
                "The requested consumer task is not known to MCG",
            ));
        }
        Err(_) => {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the consumer task",
            ));
        }
    };
    let events = state.runtime.task_events(task_id).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not read the task event history",
        )
    })?;
    let mut events = events
        .into_iter()
        .filter(|event| event.sequence > request.after_sequence)
        .take(request.limit.saturating_add(1))
        .collect::<Vec<_>>();
    let events_truncated = events.len() > request.limit;
    events.truncate(request.limit);
    let next_after_sequence = if events_truncated {
        events.last().map(|event| event.sequence)
    } else {
        None
    };
    let tool_audit = state.runtime.tool_audit(task_id).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not read the task tool audit",
        )
    })?;
    let mut tool_audit = tool_audit
        .into_iter()
        .skip(request.audit_offset)
        .take(request.limit.saturating_add(1))
        .collect::<Vec<_>>();
    let tool_audit_truncated = tool_audit.len() > request.limit;
    tool_audit.truncate(request.limit);
    let next_audit_offset =
        tool_audit_truncated.then_some(request.audit_offset.saturating_add(tool_audit.len()));
    let run = state
        .runtime
        .task_launch_run_id(task_id)
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not read the task launch identity",
            )
        })?
        .map(|run_id| state.runtime.agent_run_view(run_id))
        .transpose()
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not read the task run",
            )
        })?;
    tool_success(Ok(json!({
        "task": task,
        "events": events,
        "events_truncated": events_truncated,
        "next_after_sequence": next_after_sequence,
        "tool_audit": tool_audit,
        "tool_audit_truncated": tool_audit_truncated,
        "next_audit_offset": next_audit_offset,
        "run": run,
    })))
}

fn scheduler_list(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    match state.runtime.schedules() {
        Ok(schedules) => tool_success(Ok(json!({"schedules": schedules}))),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not list durable schedules",
        )),
    }
}

fn scheduler_create(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request =
        serde_json::from_value::<SchedulerCreateArguments>(Value::Object(arguments.clone()));
    let Ok(request) = request else {
        return Ok(tool_error(
            "invalid_arguments",
            "Schedule arguments do not match the advertised schema",
        ));
    };
    if request.context_token_budget == 0
        || request.context_token_budget > MAX_CONTEXT_TOKENS
        || request.estimated_output_tokens == 0
        || request.estimated_output_tokens > 100_000
        || request.max_model_calls == 0
        || request.max_model_calls > 32
        || request.budget.as_ref().is_some_and(|budget| {
            budget.max_tokens == Some(0)
                || budget
                    .max_cost_usd
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || budget.max_duration_seconds == Some(0)
        })
        || request.requested_tools.iter().collect::<HashSet<_>>().len()
            != request.requested_tools.len()
    {
        return Ok(tool_error(
            "invalid_arguments",
            "Schedule execution bounds are outside the advertised limits",
        ));
    }
    let request = CreateScheduleRequest {
        schedule_id: request.schedule_id,
        name: request.name,
        project_id: request.project_id,
        goal: request.goal,
        constraints: request.constraints,
        success_conditions: request.success_conditions,
        risk: request.risk,
        budget: request.budget.map(Into::into),
        trigger: request.trigger,
        first_run_at: request.first_run_at,
        target_path: request.target_path,
        context_token_budget: request.context_token_budget,
        requested_tools: request.requested_tools,
        estimated_output_tokens: request.estimated_output_tokens,
        max_model_calls: request.max_model_calls,
    };
    match create_schedule_record(state, request) {
        Ok((replayed, schedule)) => {
            tool_success(Ok(json!({"replayed": replayed, "schedule": schedule})))
        }
        Err(error) => Ok(tool_error(error.code, &error.message)),
    }
}

fn scheduler_remove(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["schedule_id"])?;
    let schedule_id = required_string(arguments, "schedule_id")?
        .parse::<ScheduleId>()
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "schedule_id must be a UUID",
            )
        })?;
    match state
        .runtime
        .set_schedule_state(schedule_id, ScheduleState::Completed, Utc::now())
    {
        Ok(schedule) => tool_success(Ok(json!({"schedule": schedule}))),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "schedule_not_found",
            "The requested schedule is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not stop the durable schedule",
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchedulerCreateArguments {
    schedule_id: ScheduleId,
    project_id: ProjectId,
    name: String,
    goal: String,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    success_conditions: Vec<String>,
    #[serde(default)]
    risk: Option<RiskLevel>,
    #[serde(default)]
    budget: Option<SchedulerBudgetArguments>,
    trigger: ScheduleTrigger,
    #[serde(default)]
    first_run_at: Option<DateTime<Utc>>,
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default = "default_scheduler_context_tokens")]
    context_token_budget: u32,
    #[serde(default)]
    requested_tools: Vec<String>,
    #[serde(default = "default_scheduler_output_tokens")]
    estimated_output_tokens: u64,
    #[serde(default = "default_scheduler_model_calls")]
    max_model_calls: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Mirrors the public TaskBudget wire contract.
struct SchedulerBudgetArguments {
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    max_cost_usd: Option<f64>,
    #[serde(default)]
    max_duration_seconds: Option<u64>,
}

impl From<SchedulerBudgetArguments> for TaskBudget {
    fn from(value: SchedulerBudgetArguments) -> Self {
        Self {
            max_tokens: value.max_tokens,
            max_cost_usd: value.max_cost_usd,
            max_duration_seconds: value.max_duration_seconds,
        }
    }
}

const fn default_scheduler_context_tokens() -> u32 {
    DEFAULT_CONTEXT_TOKENS
}

const fn default_scheduler_output_tokens() -> u64 {
    8_192
}

const fn default_scheduler_model_calls() -> u16 {
    8
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunTaskArguments {
    request_key: String,
    project_id: ProjectId,
    goal: String,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    success_conditions: Vec<String>,
    #[serde(default = "default_run_risk")]
    risk: RiskLevel,
    #[serde(default = "default_run_budget")]
    budget: TaskBudget,
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default)]
    requested_tools: Vec<String>,
    #[serde(default = "default_context_tokens")]
    context_token_budget: u32,
    #[serde(default = "default_output_tokens")]
    estimated_output_tokens: u64,
    #[serde(default = "default_model_calls")]
    max_model_calls: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContinueTaskAction {
    Retry,
    Resume,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinueTaskArguments {
    task_id: TaskId,
    action: ContinueTaskAction,
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default)]
    requested_tools: Vec<String>,
    #[serde(default = "default_context_tokens")]
    context_token_budget: u32,
    #[serde(default = "default_output_tokens")]
    estimated_output_tokens: u64,
    #[serde(default = "default_model_calls")]
    max_model_calls: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryTaskArguments {
    task_id: TaskId,
    #[serde(default)]
    after_sequence: u64,
    #[serde(default)]
    audit_offset: usize,
    #[serde(default = "default_task_query_limit")]
    limit: usize,
}

const fn default_task_query_limit() -> usize {
    200
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordEvaluationArguments {
    report: EvaluationReport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunAgentArguments {
    request_key: String,
    project_id: ProjectId,
    goal: String,
    #[serde(default = "default_run_role")]
    role: RoleKind,
    #[serde(default)]
    success_conditions: Vec<String>,
    #[serde(default = "default_run_risk")]
    risk: RiskLevel,
    #[serde(default = "default_run_budget")]
    budget: TaskBudget,
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default)]
    requested_tools: Vec<String>,
    #[serde(default = "default_context_tokens")]
    context_token_budget: u32,
    #[serde(default = "default_output_tokens")]
    estimated_output_tokens: u64,
    #[serde(default = "default_model_calls")]
    max_model_calls: u16,
}

const fn default_run_role() -> RoleKind {
    RoleKind::Conversation
}

const fn default_run_risk() -> RiskLevel {
    RiskLevel::Medium
}

fn default_run_budget() -> TaskBudget {
    TaskBudget {
        max_tokens: Some(20_000),
        max_cost_usd: Some(5.0),
        max_duration_seconds: Some(900),
    }
}

const fn default_context_tokens() -> u32 {
    DEFAULT_CONTEXT_TOKENS
}

const fn default_output_tokens() -> u64 {
    8_192
}

const fn default_model_calls() -> u16 {
    8
}

#[allow(clippy::too_many_lines)] // The atomic reservation and shared consumer advancement are one lifecycle boundary.
async fn run_task(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(arguments).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not canonicalize the consumer task request",
            )
        })?)
    );
    let request: RunTaskArguments = serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Consumer task arguments do not match the advertised schema",
            )
        })?;
    validate_run_task_arguments(&request)?;
    let project = match state.runtime.project(request.project_id) {
        Ok(Some(project)) => project,
        Ok(None) => {
            return Ok(tool_error(
                "project_not_found",
                "The requested project is not known to MCG",
            ));
        }
        Err(_) => {
            return Err(RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not read the requested project",
            ));
        }
    };

    let mut task = TaskEnvelope::new(request.goal.trim());
    task.project_id = Some(request.project_id);
    task.constraints = request
        .constraints
        .iter()
        .map(|value| value.trim().to_owned())
        .collect();
    task.success_conditions = request
        .success_conditions
        .iter()
        .map(|value| value.trim().to_owned())
        .collect();
    task.risk = request.risk;
    task.budget = request.budget.clone();
    let conditions = super::consumer_success_conditions(&task, &project)
        .map_err(|error| RpcFailure::new(StatusCode::OK, None, INVALID_PARAMS, error.message))?;
    let now = Utc::now();
    let organization = OrganizationGenome::consumer_default(now);
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
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "The requested consumer task cannot form a valid bounded plan",
            )
        })?;
    let run = AgentRun {
        schema_version: AGENT_RUN_SCHEMA_VERSION,
        id: run_id,
        task_id: task.id,
        organization_revision_id: organization.revision_id,
        state: AgentRunState::Planned,
        metrics: OrganizationalMetrics::default(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let reservation = state
        .runtime
        .create_interface_agent_run(
            "mcp_task",
            request.request_key.trim(),
            &request_hash,
            true,
            &task,
            &organization,
            &run,
            &plan,
        )
        .map_err(|error| {
            let message = if error.is_conflict() {
                "request_key was already used with different consumer task input"
            } else if error.is_invalid_request() {
                "The consumer task violates a durable runtime invariant"
            } else {
                "MCG could not reserve the durable consumer task"
            };
            RpcFailure::new(StatusCode::OK, None, INVALID_PARAMS, message)
        })?;

    let response = if reservation.replayed {
        super::LaunchTaskResponse {
            execution: super::observe_launched_run(&reservation.task, &reservation.view),
            task: reservation.task,
            run: reservation.view,
            replayed: true,
        }
    } else {
        super::advance_consumer_run(
            state,
            reservation.task.id,
            request.project_id,
            &organization,
            &reservation.view,
            &LaunchTaskRequest {
                organization_id: None,
                target_path: request.target_path,
                context_token_budget: request.context_token_budget,
                requested_tools: request.requested_tools,
                estimated_output_tokens: request.estimated_output_tokens,
                max_model_calls: request.max_model_calls,
                provider_id: None,
                model_id: None,
                reasoning_effort: None,
                service_tier: None,
            },
            false,
        )
        .await
        .map_err(|error| RpcFailure::new(StatusCode::OK, None, INVALID_PARAMS, error.message))?
    };
    tool_success(Ok(json!({
        "request_key": request.request_key,
        "task": response.task,
        "run": response.run,
        "replayed": response.replayed,
        "execution": response.execution,
    })))
}

async fn continue_task(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: ContinueTaskArguments = serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Consumer continuation arguments do not match the advertised schema",
        )
    })?;
    validate_continue_task_arguments(&request)?;
    let task_id = request.task_id;
    let launch = LaunchTaskRequest {
        organization_id: None,
        target_path: request.target_path,
        context_token_budget: request.context_token_budget,
        requested_tools: request.requested_tools,
        estimated_output_tokens: request.estimated_output_tokens,
        max_model_calls: request.max_model_calls,
        provider_id: None,
        model_id: None,
        reasoning_effort: None,
        service_tier: None,
    };
    let response = match request.action {
        ContinueTaskAction::Retry => {
            super::retry_task(
                State(state.clone()),
                Path(task_id.to_string()),
                Json(launch),
            )
            .await
        }
        ContinueTaskAction::Resume => {
            super::resume_task(
                State(state.clone()),
                Path(task_id.to_string()),
                Json(launch),
            )
            .await
        }
    };
    match response {
        Ok(Json(response)) => tool_success(serde_json::to_value(response)),
        Err(error) => Ok(tool_error(error.code, &error.message)),
    }
}

fn cancel_task(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["task_id"])?;
    let task_id = required_string(arguments, "task_id")?
        .parse::<TaskId>()
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "task_id must be a UUID",
            )
        })?;
    let existing = match state.runtime.task(task_id) {
        Ok(Some(task)) => task,
        Ok(None) => {
            return Ok(tool_error(
                "task_not_found",
                "The requested consumer task is not known to MCG",
            ));
        }
        Err(_) => {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the consumer task",
            ));
        }
    };
    let replayed = existing.state == TaskState::Cancelled;
    let task = if replayed {
        existing
    } else {
        match state.runtime.transition_task(task_id, TaskState::Cancelled) {
            Ok(task) => task,
            Err(error) if error.is_invalid_request() => {
                return Ok(tool_error(
                    "invalid_state",
                    "Only a nonterminal consumer task can be cancelled",
                ));
            }
            Err(_) => {
                return Ok(tool_error(
                    "runtime_error",
                    "MCG could not cancel the consumer task",
                ));
            }
        }
    };
    let run_id = state.runtime.task_launch_run_id(task_id).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG cancelled the task but could not read its launch identity",
        )
    })?;
    let run = run_id
        .map(|run_id| state.runtime.agent_run_view(run_id))
        .transpose()
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG cancelled the task but could not read its durable run",
            )
        })?;
    tool_success(Ok(json!({"task": task, "run": run, "replayed": replayed})))
}

fn record_evaluation(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: RecordEvaluationArguments =
        serde_json::from_value(Value::Object(arguments.clone())).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Evaluation arguments do not contain a complete EvaluationReport",
            )
        })?;
    let view = match state.runtime.agent_run_view(request.report.run_id) {
        Ok(view) => view,
        Err(error) if error.is_not_found() => {
            return Ok(tool_error(
                "run_not_found",
                "The evaluation run is not known to MCG",
            ));
        }
        Err(_) => {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not inspect the evaluation run",
            ));
        }
    };
    if let Some(existing) = view
        .evaluations
        .iter()
        .find(|evaluation| evaluation.id == request.report.id)
    {
        if existing != &request.report {
            return Ok(tool_error(
                "conflict",
                "evaluation id was already used with different evidence",
            ));
        }
        return tool_success(Ok(json!({
            "report": existing,
            "replayed": true,
        })));
    }
    match state.runtime.record_evaluation(&request.report) {
        Ok(event) => tool_success(Ok(json!({
            "report": request.report,
            "event": event,
            "replayed": false,
        }))),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "conflict",
            "The evaluation conflicts with existing durable evidence",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_evaluation",
            "The evaluation failed identity, independence, coverage, outcome, or evidence validation",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not record the evaluation",
        )),
    }
}

fn validate_run_task_arguments(request: &RunTaskArguments) -> Result<(), RpcFailure> {
    if !valid_request_key(&request.request_key)
        || request.goal.trim().is_empty()
        || request.goal.chars().count() > 32_768
        || !valid_bounded_strings(&request.constraints, 64, 8_192)
        || !valid_bounded_strings(&request.success_conditions, 64, 8_192)
        || request
            .target_path
            .as_ref()
            .is_some_and(|path| path.len() > 4_096)
        || !valid_execution_bounds(
            request.context_token_budget,
            request.estimated_output_tokens,
            request.max_model_calls,
            &request.requested_tools,
        )
        || request.budget.max_tokens == Some(0)
        || request.budget.max_duration_seconds == Some(0)
        || request
            .budget
            .max_cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Consumer task values are outside the advertised bounds",
        ));
    }
    Ok(())
}

fn validate_continue_task_arguments(request: &ContinueTaskArguments) -> Result<(), RpcFailure> {
    if request
        .target_path
        .as_ref()
        .is_some_and(|path| path.len() > 4_096)
        || !valid_execution_bounds(
            request.context_token_budget,
            request.estimated_output_tokens,
            request.max_model_calls,
            &request.requested_tools,
        )
    {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Consumer continuation values are outside the advertised bounds",
        ));
    }
    Ok(())
}

fn valid_request_key(value: &str) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn valid_bounded_strings(values: &[String], max_items: usize, max_chars: usize) -> bool {
    values.len() <= max_items
        && values
            .iter()
            .all(|value| !value.trim().is_empty() && value.chars().count() <= max_chars)
}

fn valid_execution_bounds(
    context_token_budget: u32,
    estimated_output_tokens: u64,
    max_model_calls: u16,
    requested_tools: &[String],
) -> bool {
    (64..=MAX_CONTEXT_TOKENS).contains(&context_token_budget)
        && (1..=100_000).contains(&estimated_output_tokens)
        && (1..=32).contains(&max_model_calls)
        && requested_tools.len() <= 128
        && requested_tools.iter().collect::<HashSet<_>>().len() == requested_tools.len()
        && requested_tools
            .iter()
            .all(|tool| !tool.trim().is_empty() && tool.len() <= 512)
}

#[allow(clippy::too_many_lines)] // Keeps validation, durable reservation, and execution together.
async fn run_agent(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(arguments).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not canonicalize the agent request",
            )
        })?)
    );
    let request: RunAgentArguments = serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Agent request arguments do not match the advertised schema",
            )
        })?;
    validate_run_agent_arguments(&request)?;
    if state
        .runtime
        .project(request.project_id)
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INTERNAL_ERROR,
                "MCG could not read the requested project",
            )
        })?
        .is_none()
    {
        return Ok(tool_error(
            "project_not_found",
            "The requested project is not known to MCG",
        ));
    }

    let mut task = TaskEnvelope::new(request.goal.trim());
    task.project_id = Some(request.project_id);
    task.risk = request.risk;
    task.budget = request.budget.clone();
    task.success_conditions = request
        .success_conditions
        .iter()
        .map(|condition| condition.trim().to_owned())
        .collect();
    if task.success_conditions.is_empty() {
        task.success_conditions.push(format!(
            "Deliver a grounded terminal response for: {}",
            task.goal
        ));
    }
    let now = Utc::now();
    let organization = OrganizationGenome::consumer_default(now);
    let success_conditions = task
        .success_conditions
        .iter()
        .map(|description| SuccessCondition::Custom {
            description: description.clone(),
        })
        .collect();
    let run_id = RunId::new();
    let planning = PlanningRequest {
        run_id,
        task: task.clone(),
        complexity: TaskComplexity::Atomic,
        work: vec![PlannedWork::atomic(
            task.goal.clone(),
            request.role,
            success_conditions,
        )],
        require_independent_evaluation: false,
    };
    let plan = ExecutivePlanner::default()
        .plan(&organization, &planning, now)
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "The requested agent work cannot form a valid bounded plan",
            )
        })?;
    let run = AgentRun {
        schema_version: AGENT_RUN_SCHEMA_VERSION,
        id: run_id,
        task_id: task.id,
        organization_revision_id: organization.revision_id,
        state: AgentRunState::Planned,
        metrics: OrganizationalMetrics::default(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let mut reservation = state
        .runtime
        .create_interface_agent_run(
            "mcp",
            request.request_key.trim(),
            &request_hash,
            false,
            &task,
            &organization,
            &run,
            &plan,
        )
        .map_err(|error| {
            let message = if error.is_conflict() {
                "request_key was already used with different agent input"
            } else if error.is_invalid_request() {
                "The agent request violates a durable runtime invariant"
            } else {
                "MCG could not reserve the durable agent run"
            };
            RpcFailure::new(StatusCode::OK, None, INVALID_PARAMS, message)
        })?;

    if reservation.replayed
        && reservation.view.run.state == AgentRunState::Recovering
        && reservation.view.results.is_empty()
        && reservation.view.instances.is_empty()
    {
        state
            .runtime
            .transition_agent_run(
                reservation.view.run.id,
                AgentRunState::Running,
                reservation.view.run.metrics.clone(),
            )
            .map_err(|_| {
                RpcFailure::new(
                    StatusCode::OK,
                    None,
                    INVALID_PARAMS,
                    "The recovered agent run could not safely resume",
                )
            })?;
        reservation.view = state
            .runtime
            .agent_run_view(reservation.view.run.id)
            .map_err(|_| {
                RpcFailure::new(
                    StatusCode::OK,
                    None,
                    INTERNAL_ERROR,
                    "MCG resumed the run but could not reload its durable view",
                )
            })?;
    }
    if reservation.replayed && !reservation.view.results.is_empty() {
        return tool_success(Ok(json!({
            "request_key": request.request_key,
            "replayed": true,
            "execution": "existing_evidence",
            "task": reservation.task,
            "run": reservation.view,
        })));
    }
    if reservation.replayed && !reservation.view.instances.is_empty() {
        return Ok(tool_error_with_data(
            "existing_non_success_evidence",
            "This idempotent run already has non-success authority evidence; inspect it and resume through the appropriate approval or run-control boundary",
            json!({
                "request_key": request.request_key,
                "replayed": true,
                "task": reservation.task,
                "run": reservation.view,
            }),
        ));
    }
    let work_item_id = reservation.view.plan.items[0].id;
    let execution = state
        .runtime
        .execute_agent_work(&AgentDispatchSpec {
            run_id: reservation.view.run.id,
            work_item_id,
            project_id: request.project_id,
            target_path: request.target_path,
            context_token_budget: request.context_token_budget,
            requested_tools: request.requested_tools,
            estimated_output_tokens: request.estimated_output_tokens,
            max_model_calls: request.max_model_calls,
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_tier: None,
        })
        .await;
    match execution {
        Ok(turn) => {
            let view = state
                .runtime
                .agent_run_view(reservation.view.run.id)
                .map_err(|_| {
                    RpcFailure::new(
                        StatusCode::OK,
                        None,
                        INTERNAL_ERROR,
                        "MCG committed the turn but could not reload its durable view",
                    )
                })?;
            tool_success(Ok(json!({
                "request_key": request.request_key,
                "replayed": reservation.replayed,
                "execution": "turn_completed",
                "task": reservation.task,
                "turn": turn,
                "run": view,
            })))
        }
        Err(error) => {
            let view = state
                .runtime
                .agent_run_view(reservation.view.run.id)
                .unwrap_or(reservation.view);
            let (code, message) = if matches!(
                error,
                mcg_runtime::RuntimeError::Provider(mcg_providers::ProviderError::Authentication(
                    _
                ))
            ) {
                (
                    "provider_authentication",
                    "The selected provider requires authentication; MCG did not fall back to another billing mode",
                )
            } else if matches!(error, mcg_runtime::RuntimeError::Provider(_)) {
                (
                    "provider_unavailable",
                    "No eligible provider completed the bounded agent turn",
                )
            } else if error.is_conflict() {
                (
                    "run_conflict",
                    "The durable run cannot dispatch work in its current state",
                )
            } else {
                (
                    "agent_execution_failed",
                    "The bounded agent turn failed; inspect the durable run evidence before retrying with a new request key",
                )
            };
            Ok(tool_error_with_data(
                code,
                message,
                json!({
                    "request_key": request.request_key,
                    "replayed": reservation.replayed,
                    "task": reservation.task,
                    "run": view,
                }),
            ))
        }
    }
}

fn validate_run_agent_arguments(request: &RunAgentArguments) -> Result<(), RpcFailure> {
    if request.request_key.trim() != request.request_key
        || request.request_key.is_empty()
        || request.request_key.len() > 128
        || !request
            .request_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
        || request.goal.trim().is_empty()
        || request.goal.chars().count() > 20_000
        || request.success_conditions.len() > 20
        || request
            .success_conditions
            .iter()
            .any(|condition| condition.trim().is_empty() || condition.chars().count() > 2_000)
        || request.context_token_budget < 64
        || request.context_token_budget > MAX_CONTEXT_TOKENS
        || request.estimated_output_tokens == 0
        || request.estimated_output_tokens > 100_000
        || request.max_model_calls == 0
        || request.max_model_calls > 32
        || request
            .budget
            .max_cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Agent request values are outside the advertised bounds",
        ));
    }
    let allowed: &[&str] = match request.role {
        RoleKind::Conversation => &[],
        RoleKind::Coder => &[
            "fs.read",
            "fs.list",
            "fs.write",
            "fs.patch",
            "fs.mkdir",
            "process.exec",
            "git.status",
            "git.diff",
            "git.apply_patch",
        ],
        RoleKind::ToolWorker => &["fs.read", "fs.list", "process.exec", "system.info"],
        _ => {
            return Err(RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "role must be conversation, coder, or tool_worker",
            ));
        }
    };
    if (request.role != RoleKind::Conversation && request.requested_tools.is_empty())
        || request.requested_tools.len() > 16
        || request
            .requested_tools
            .iter()
            .any(|tool| !allowed.contains(&tool.as_str()) && !valid_mcp_tool_reference(tool))
        || request
            .requested_tools
            .iter()
            .enumerate()
            .any(|(index, tool)| request.requested_tools[..index].contains(tool))
    {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "requested_tools must be unique, explicit, and grantable to the selected role",
        ));
    }
    Ok(())
}

fn valid_mcp_tool_reference(value: &str) -> bool {
    let mut parts = value.splitn(3, "::");
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("mcp"), Some(connection_id), Some(tool_name))
            if !connection_id.is_empty()
                && connection_id.len() <= 128
                && connection_id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
                && !tool_name.trim().is_empty()
                && value.len() <= 512
    )
}

fn query_project(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["project_id"])?;
    let value = if arguments.contains_key("project_id") {
        let project_id = project_id(arguments)?;
        match state.runtime.project(project_id) {
            Ok(Some(project)) => json!({"project": project}),
            Ok(None) => {
                return Ok(tool_error(
                    "project_not_found",
                    "The requested project is not known to MCG",
                ));
            }
            Err(_) => {
                return Ok(tool_error(
                    "runtime_error",
                    "MCG could not read the project model",
                ));
            }
        }
    } else {
        match state.runtime.projects() {
            Ok(projects) => json!({"projects": projects}),
            Err(_) => {
                return Ok(tool_error(
                    "runtime_error",
                    "MCG could not list project models",
                ));
            }
        }
    };
    tool_success(Ok(value))
}

fn query_organization(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(
        arguments,
        &["organization_id", "revision_id", "compare_revision_id"],
    )?;
    if let Some(organization_id) = optional_string(arguments, "organization_id")? {
        if arguments.contains_key("revision_id") || arguments.contains_key("compare_revision_id") {
            return Err(RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "organization_id cannot be combined with revision ids",
            ));
        }
        let organization_id = OrganizationId::from_str(&organization_id).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "organization_id must be a UUID",
            )
        })?;
        return match state.runtime.organization_history(organization_id) {
            Ok(revisions) if revisions.is_empty() => Ok(tool_error(
                "organization_not_found",
                "The requested organization is not known to MCG",
            )),
            Ok(revisions) => tool_success(Ok(json!({"revisions": revisions}))),
            Err(_) => Ok(tool_error(
                "runtime_error",
                "MCG could not load organization history",
            )),
        };
    }
    let revision_id = OrganizationRevisionId::from_str(required_string(arguments, "revision_id")?)
        .map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "revision_id must be a UUID",
            )
        })?;
    if let Some(compare) = optional_string(arguments, "compare_revision_id")? {
        let compare = OrganizationRevisionId::from_str(&compare).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "compare_revision_id must be a UUID",
            )
        })?;
        return match state.runtime.compare_organizations(revision_id, compare) {
            Ok(comparison) => tool_success(serde_json::to_value(comparison)),
            Err(error) if error.is_not_found() => Ok(tool_error(
                "organization_not_found",
                "One of the requested organization revisions is not known to MCG",
            )),
            Err(_) => Ok(tool_error(
                "runtime_error",
                "MCG could not compare the organization revisions",
            )),
        };
    }
    match state.runtime.export_organization(revision_id) {
        Ok(portable) => tool_success(serde_json::to_value(portable)),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "organization_not_found",
            "The requested organization revision is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not export the organization revision",
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportOrganizationArguments {
    import_id: OrganizationImportId,
    portable: PortableOrganizationGenome,
    #[serde(default)]
    name: Option<String>,
}

fn import_organization(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: ImportOrganizationArguments =
        serde_json::from_value(Value::Object(arguments.clone())).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Organization import arguments do not match the advertised schema",
            )
        })?;
    let name = request.name.as_deref().map(str::trim);
    if name.is_some_and(|name| name.is_empty() || name.chars().count() > 100) {
        return Ok(tool_error(
            "invalid_arguments",
            "name must contain 1-100 characters",
        ));
    }
    match state
        .runtime
        .import_organization(request.import_id, &request.portable, name)
    {
        Ok(mutation) => tool_success(serde_json::to_value(mutation)),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "import_conflict",
            "import_id was already bound to different organization content",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_portable_organization",
            "The portable organization failed bounded validation",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not import the organization",
        )),
    }
}

fn organization_publishers(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    match state.runtime.organization_publishers() {
        Ok(publishers) => tool_success(Ok(json!({"publishers": publishers}))),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not list organization publisher trust",
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizationPublisherMutationArguments {
    action: String,
    publisher_id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    public_key_base64: Option<String>,
    #[serde(default)]
    registries: Option<Vec<String>>,
    #[serde(default)]
    expected_revision: Option<u64>,
    reason: String,
}

fn organization_publisher_mutate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: OrganizationPublisherMutationArguments =
        serde_json::from_value(Value::Object(arguments.clone())).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Organization publisher arguments do not match the advertised schema",
            )
        })?;
    let mutation = match request.action.as_str() {
        "trust" => {
            let (Some(display_name), Some(public_key_base64), Some(registries)) = (
                request.display_name.as_deref(),
                request.public_key_base64.as_deref(),
                request.registries,
            ) else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "trust requires display_name, public_key_base64, and registries",
                ));
            };
            state.runtime.trust_organization_publisher(
                &request.publisher_id,
                display_name,
                public_key_base64,
                registries,
                request.expected_revision,
                &request.reason,
            )
        }
        "revoke" => {
            let Some(expected_revision) = request.expected_revision else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "revoke requires expected_revision",
                ));
            };
            if request.display_name.is_some()
                || request.public_key_base64.is_some()
                || request.registries.is_some()
            {
                return Ok(tool_error(
                    "invalid_arguments",
                    "revoke does not accept trust-definition fields",
                ));
            }
            state.runtime.revoke_organization_publisher(
                &request.publisher_id,
                expected_revision,
                &request.reason,
            )
        }
        _ => {
            return Ok(tool_error(
                "invalid_arguments",
                "action must be trust or revoke",
            ));
        }
    };
    match mutation {
        Ok(mutation) => tool_success(serde_json::to_value(mutation)),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "publisher_conflict",
            "Organization publisher trust changed or is already terminal",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_publisher",
            "Organization publisher trust failed bounded validation",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not change organization publisher trust",
        )),
    }
}

fn organization_share_reviews(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &[])?;
    match state.runtime.organization_share_reviews() {
        Ok(reviews) => tool_success(Ok(json!({"reviews": reviews}))),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not list organization share reviews",
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizationShareMutationArguments {
    action: String,
    review_id: OrganizationShareReviewId,
    #[serde(default)]
    package: Option<OrganizationSharePackage>,
    #[serde(default)]
    import_id: Option<OrganizationImportId>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

fn organization_share_mutate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let request: OrganizationShareMutationArguments =
        serde_json::from_value(Value::Object(arguments.clone())).map_err(|_| {
            RpcFailure::new(
                StatusCode::OK,
                None,
                INVALID_PARAMS,
                "Organization share arguments do not match the advertised schema",
            )
        })?;
    match request.action.as_str() {
        "review" => {
            let Some(package) = request.package.as_ref() else {
                return Ok(tool_error("invalid_arguments", "review requires package"));
            };
            if request.import_id.is_some() || request.name.is_some() || request.reason.is_some() {
                return Ok(tool_error(
                    "invalid_arguments",
                    "review accepts only review_id and package",
                ));
            }
            match state
                .runtime
                .review_shared_organization(request.review_id, package)
            {
                Ok(mutation) => tool_success(serde_json::to_value(mutation)),
                Err(error) if error.is_conflict() => Ok(tool_error(
                    "review_conflict",
                    "Review identity is already bound to another package",
                )),
                Err(error) if error.is_invalid_request() => Ok(tool_error(
                    "invalid_share_package",
                    "The organization share package or publisher trust is invalid",
                )),
                Err(_) => Ok(tool_error(
                    "runtime_error",
                    "MCG could not review the organization share package",
                )),
            }
        }
        "import" => {
            let (Some(import_id), Some(reason)) = (request.import_id, request.reason.as_deref())
            else {
                return Ok(tool_error(
                    "invalid_arguments",
                    "import requires import_id and reason",
                ));
            };
            if request.package.is_some() {
                return Ok(tool_error(
                    "invalid_arguments",
                    "import uses the retained reviewed package",
                ));
            }
            organization_share_decision_result(state.runtime.import_shared_organization(
                request.review_id,
                import_id,
                request.name.as_deref().map(str::trim),
                reason.trim(),
            ))
        }
        "reject" => {
            let Some(reason) = request.reason.as_deref() else {
                return Ok(tool_error("invalid_arguments", "reject requires reason"));
            };
            if request.package.is_some() || request.import_id.is_some() || request.name.is_some() {
                return Ok(tool_error(
                    "invalid_arguments",
                    "reject accepts only review_id and reason",
                ));
            }
            organization_share_decision_result(
                state
                    .runtime
                    .reject_shared_organization(request.review_id, reason.trim()),
            )
        }
        _ => Ok(tool_error(
            "invalid_arguments",
            "action must be review, import, or reject",
        )),
    }
}

fn organization_share_decision_result(
    result: Result<impl serde::Serialize, mcg_runtime::RuntimeError>,
) -> Result<Value, RpcFailure> {
    match result {
        Ok(value) => tool_success(serde_json::to_value(value)),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "review_conflict",
            "Organization share review is stale or terminal",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_share_decision",
            "Organization share decision failed bounded validation or current-trust verification",
        )),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "review_not_found",
            "The organization share review is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not resolve the organization share review",
        )),
    }
}

fn project_context(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["project_id", "target_path", "token_budget"])?;
    let project_id = project_id(arguments)?;
    let target_path = optional_string(arguments, "target_path")?;
    let token_budget =
        optional_u64(arguments, "token_budget")?.unwrap_or(u64::from(DEFAULT_CONTEXT_TOKENS));
    if token_budget == 0 || token_budget > u64::from(MAX_CONTEXT_TOKENS) {
        return Ok(tool_error(
            "invalid_arguments",
            "token_budget must be between 1 and 131072",
        ));
    }
    match state.runtime.assemble_project_context(
        project_id,
        Vec::new(),
        u32::try_from(token_budget).unwrap_or(MAX_CONTEXT_TOKENS),
        target_path.as_deref(),
    ) {
        Ok(context) => tool_success(serde_json::to_value(context)),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "project_not_found",
            "The requested project is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not assemble project context",
        )),
    }
}

fn project_topology_plan(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["project_id"])?;
    let project_id = project_id(arguments)?;
    match state.runtime.project(project_id) {
        Ok(Some(project)) => tool_success(serde_json::to_value(project.topology_plan())),
        Ok(None) => Ok(tool_error(
            "project_not_found",
            "The requested project is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not derive the project topology plan",
        )),
    }
}

fn reviewed_topology_plans(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) =
        serde_json::from_value::<ReviewedTopologyPlansArguments>(Value::Object(arguments.clone()))
    else {
        return Ok(tool_error(
            "invalid_arguments",
            "reviewed topology plan query arguments are invalid",
        ));
    };
    if let Some(plan_id) = request.plan_id {
        match state.runtime.reviewed_topology_plan(plan_id) {
            Ok(Some(plan)) if plan.project_id == request.project_id => {
                tool_success(Ok(json!({"plan": plan})))
            }
            Ok(_) => Ok(tool_error(
                "reviewed_topology_plan_not_found",
                "The requested reviewed topology plan is not known to MCG",
            )),
            Err(_) => Ok(tool_error(
                "runtime_error",
                "MCG could not read the reviewed topology plan",
            )),
        }
    } else {
        match state.runtime.reviewed_topology_plans(request.project_id) {
            Ok(plans) => tool_success(Ok(json!({"plans": plans}))),
            Err(error) if error.is_not_found() => Ok(tool_error(
                "project_not_found",
                "The requested project is not known to MCG",
            )),
            Err(_) => Ok(tool_error(
                "runtime_error",
                "MCG could not list reviewed topology plans",
            )),
        }
    }
}

#[allow(clippy::too_many_lines)] // One strict parser keeps all mutation actions on one MCP boundary.
async fn reviewed_topology_plan_mutate(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    let Ok(request) = serde_json::from_value::<ReviewedTopologyPlanMutateArguments>(Value::Object(
        arguments.clone(),
    )) else {
        return Ok(tool_error(
            "invalid_arguments",
            "reviewed topology plan mutation arguments are invalid",
        ));
    };
    let result = match request.action.as_str() {
        "create"
            if request.project_snapshot_id.is_some()
                && request.topology_sha256.is_some()
                && request.display_name.is_some()
                && request.reason.is_some()
                && request.steps.is_some() =>
        {
            state.runtime.create_reviewed_topology_plan(
                request.project_id,
                &ReviewedTopologyPlanCreateRequest {
                    id: request.plan_id,
                    project_snapshot_id: request.project_snapshot_id.expect("checked"),
                    topology_sha256: request.topology_sha256.unwrap_or_default(),
                    display_name: request.display_name.unwrap_or_default(),
                    reason: request.reason.unwrap_or_default(),
                    steps: request.steps.unwrap_or_default(),
                },
            )
        }
        "run"
            if request.project_snapshot_id.is_none()
                && request.topology_sha256.is_none()
                && request.display_name.is_none()
                && request.reason.is_none()
                && request.steps.is_none() =>
        {
            if state
                .runtime
                .reviewed_topology_plan(request.plan_id)
                .map_err(|error| {
                    RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
                })?
                .is_none_or(|plan| plan.project_id != request.project_id)
            {
                return Ok(tool_error(
                    "reviewed_topology_plan_not_found",
                    "The requested reviewed topology plan is not known to MCG",
                ));
            }
            state
                .runtime
                .run_reviewed_topology_plan(request.plan_id)
                .await
        }
        "cancel"
            if request.project_snapshot_id.is_none()
                && request.topology_sha256.is_none()
                && request.display_name.is_none()
                && request.reason.is_none()
                && request.steps.is_none() =>
        {
            if state
                .runtime
                .reviewed_topology_plan(request.plan_id)
                .map_err(|error| {
                    RpcFailure::new(StatusCode::OK, None, INTERNAL_ERROR, error.to_string())
                })?
                .is_none_or(|plan| plan.project_id != request.project_id)
            {
                return Ok(tool_error(
                    "reviewed_topology_plan_not_found",
                    "The requested reviewed topology plan is not known to MCG",
                ));
            }
            state.runtime.cancel_reviewed_topology_plan(request.plan_id)
        }
        _ => {
            return Ok(tool_error(
                "invalid_arguments",
                "Create requires snapshot, topology digest, name, reason, and steps; run/cancel accept only action and identities",
            ));
        }
    };
    match result {
        Ok(mutation) => tool_success(Ok(json!(mutation))),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "reviewed_topology_plan_not_found",
            "The requested project or reviewed topology plan is not known to MCG",
        )),
        Err(error) if error.is_conflict() => Ok(tool_error(
            "conflict",
            "The reviewed topology authority no longer matches the current project",
        )),
        Err(error) if error.is_invalid_request() => Ok(tool_error(
            "invalid_arguments",
            "The reviewed topology plan is invalid",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not mutate the reviewed topology plan",
        )),
    }
}

fn memory_search(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(
        arguments,
        &["project_id", "query", "namespace", "fresh_only", "limit"],
    )?;
    let project_id = project_id(arguments)?;
    let query = required_string(arguments, "query")?;
    if query.trim().is_empty() {
        return Ok(tool_error("invalid_arguments", "query cannot be empty"));
    }
    let namespace = optional_string(arguments, "namespace")?;
    let fresh_only = optional_bool(arguments, "fresh_only")?.unwrap_or(true);
    let limit = optional_u64(arguments, "limit")?.unwrap_or(DEFAULT_MEMORY_LIMIT as u64);
    if limit == 0 || limit > MAX_MEMORY_LIMIT as u64 {
        return Ok(tool_error(
            "invalid_arguments",
            "limit must be between 1 and 100",
        ));
    }
    match state.runtime.project(project_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(tool_error(
                "project_not_found",
                "The requested project is not known to MCG",
            ));
        }
        Err(_) => {
            return Ok(tool_error(
                "runtime_error",
                "MCG could not read the project model",
            ));
        }
    }
    let Ok(claims) = state
        .runtime
        .memory_claims(project_id, namespace.as_deref(), fresh_only)
    else {
        return Ok(tool_error(
            "runtime_error",
            "MCG could not search project memory",
        ));
    };
    let query = query.to_lowercase();
    let matches: Vec<_> = claims
        .into_iter()
        .filter(|claim| {
            claim.namespace.to_lowercase().contains(&query)
                || claim.key.to_lowercase().contains(&query)
                || claim.value.to_string().to_lowercase().contains(&query)
                || claim.provenance.source.to_lowercase().contains(&query)
        })
        .take(usize::try_from(limit).unwrap_or(MAX_MEMORY_LIMIT))
        .collect();
    tool_success(Ok(json!({"claims": matches})))
}

fn memory_transfer_preview(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["source_project_id", "source_claim_id"])?;
    let source_project_id = required_project_id(arguments, "source_project_id")?;
    let source_claim_id = required_memory_claim_id(arguments, "source_claim_id")?;
    match state
        .runtime
        .project_memory_transfer_preview(source_project_id, source_claim_id)
    {
        Ok(preview) => tool_success(serde_json::to_value(preview)),
        Err(_) => Ok(tool_error(
            "transfer_source_refused",
            "The source claim was not found in that project or is not currently fresh and durable",
        )),
    }
}

fn memory_transfers(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["project_id"])?;
    let project_id = project_id(arguments)?;
    match state.runtime.project_memory_transfers(project_id) {
        Ok(transfers) => tool_success(Ok(json!({"transfers": transfers}))),
        Err(_) => Ok(tool_error(
            "project_not_found",
            "The requested project is not known to MCG",
        )),
    }
}

fn memory_transfer(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(
        arguments,
        &[
            "transfer_id",
            "source_project_id",
            "source_claim_id",
            "source_claim_sha256",
            "target_project_id",
            "reason",
        ],
    )?;
    let transfer_id = required_memory_transfer_id(arguments, "transfer_id")?;
    let source_project_id = required_project_id(arguments, "source_project_id")?;
    let source_claim_id = required_memory_claim_id(arguments, "source_claim_id")?;
    let source_claim_sha256 = required_string(arguments, "source_claim_sha256")?;
    let target_project_id = required_project_id(arguments, "target_project_id")?;
    let reason = required_string(arguments, "reason")?;
    match state.runtime.transfer_project_memory(
        transfer_id,
        source_project_id,
        source_claim_id,
        source_claim_sha256,
        target_project_id,
        reason,
    ) {
        Ok(mutation) => tool_success(serde_json::to_value(mutation)),
        Err(_) => Ok(tool_error(
            "memory_transfer_refused",
            "The reviewed transfer was refused because its evidence, identity, target, or current project memory no longer matches",
        )),
    }
}

fn project_id(arguments: &serde_json::Map<String, Value>) -> Result<ProjectId, RpcFailure> {
    required_project_id(arguments, "project_id")
}

fn required_project_id(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<ProjectId, RpcFailure> {
    let value = required_string(arguments, name)?;
    ProjectId::from_str(value).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            format!("{name} must be a UUID"),
        )
    })
}

fn required_memory_claim_id(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<MemoryClaimId, RpcFailure> {
    let value = required_string(arguments, name)?;
    MemoryClaimId::from_str(value).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            format!("{name} must be a UUID"),
        )
    })
}

fn required_memory_transfer_id(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<MemoryTransferId, RpcFailure> {
    let value = required_string(arguments, name)?;
    MemoryTransferId::from_str(value).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            format!("{name} must be a UUID"),
        )
    })
}

fn required_string<'a>(
    arguments: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a str, RpcFailure> {
    arguments.get(name).and_then(Value::as_str).ok_or_else(|| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            format!("{name} must be a string"),
        )
    })
}

fn optional_string(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<String>, RpcFailure> {
    arguments
        .get(name)
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                RpcFailure::new(
                    StatusCode::OK,
                    None,
                    INVALID_PARAMS,
                    format!("{name} must be a string"),
                )
            })
        })
        .transpose()
}

fn optional_bool(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<bool>, RpcFailure> {
    arguments
        .get(name)
        .map(|value| {
            value.as_bool().ok_or_else(|| {
                RpcFailure::new(
                    StatusCode::OK,
                    None,
                    INVALID_PARAMS,
                    format!("{name} must be a boolean"),
                )
            })
        })
        .transpose()
}

fn optional_u64(
    arguments: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<u64>, RpcFailure> {
    arguments
        .get(name)
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                RpcFailure::new(
                    StatusCode::OK,
                    None,
                    INVALID_PARAMS,
                    format!("{name} must be a positive integer"),
                )
            })
        })
        .transpose()
}

fn require_only_keys(
    arguments: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), RpcFailure> {
    if let Some(key) = arguments
        .keys()
        .find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            format!("Unknown tool argument: {key}"),
        ));
    }
    Ok(())
}

fn tool_success(value: Result<Value, serde_json::Error>) -> Result<Value, RpcFailure> {
    let value = value.map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not serialize the tool result",
        )
    })?;
    let text = serde_json::to_string_pretty(&value).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INTERNAL_ERROR,
            "MCG could not serialize the tool result",
        )
    })?;
    Ok(with_server_info(json!({
        "resultType": "complete",
        "content": [{"type": "text", "text": text}],
        "structuredContent": value,
        "isError": false,
    })))
}

fn tool_error(code: &str, message: &str) -> Value {
    tool_error_with_data(code, message, json!({}))
}

fn tool_error_with_data(code: &str, message: &str, mut data: Value) -> Value {
    data["error"] = json!(code);
    data["message"] = json!(message);
    with_server_info(json!({
        "resultType": "complete",
        "content": [{"type": "text", "text": message}],
        "structuredContent": data,
        "isError": true,
    }))
}

fn with_server_info(mut result: Value) -> Value {
    result["_meta"] = json!({
        "io.modelcontextprotocol/serverInfo": {
            "name": SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        }
    });
    result
}

fn json_response(status: StatusCode, body: &Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_tools_default_to_a_complete_file_sized_response_budget() {
        assert_eq!(default_output_tokens(), 8_192);
        assert_eq!(default_scheduler_output_tokens(), 8_192);
    }

    #[test]
    fn mirrored_names_decode_only_the_standard_safe_forms() {
        assert_eq!(
            decode_mirrored_value("mcg_system_status"),
            Ok("mcg_system_status".into())
        );
        assert_eq!(
            decode_mirrored_value("=?base64?bWNnX3N5c3RlbV9zdGF0dXM=?="),
            Ok("mcg_system_status".into())
        );
        assert_eq!(decode_mirrored_value(" padded "), Err(()));
        assert_eq!(decode_mirrored_value("=?base64?%%%?="), Err(()));
    }

    #[test]
    fn origin_policy_allows_only_absent_or_loopback_origins() {
        let mut headers = HeaderMap::new();
        assert!(valid_origin(&headers));
        headers.insert(header::ORIGIN, "http://127.0.0.1:1420".parse().unwrap());
        assert!(valid_origin(&headers));
        headers.insert(header::ORIGIN, "http://localhost:1420".parse().unwrap());
        assert!(valid_origin(&headers));
        headers.insert(header::ORIGIN, "http://[::1]:1420".parse().unwrap());
        assert!(valid_origin(&headers));
        headers.insert(header::ORIGIN, "mcg://localhost".parse().unwrap());
        assert!(!valid_origin(&headers));
        headers.insert(header::ORIGIN, "https://example.com".parse().unwrap());
        assert!(!valid_origin(&headers));
    }
}
