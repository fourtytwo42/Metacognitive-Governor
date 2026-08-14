//! Stateless MCP 2026-07-28 Streamable HTTP server boundary.

use std::str::FromStr;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use mcg_agents::{
    AGENT_RUN_SCHEMA_VERSION, AgentRun, AgentRunState, ExecutivePlanner, OrganizationGenome,
    OrganizationalMetrics, PlannedWork, PlanningRequest, RoleKind, SuccessCondition,
    TaskComplexity,
};
use mcg_domain::{ProjectId, RiskLevel, RunId, TaskBudget, TaskEnvelope};
use mcg_runtime::AgentDispatchSpec;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::{Host, Url};

use super::ApiState;

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
            "name": "mcg_query_run",
            "title": "Query MCG Agent Run",
            "description": "Read the complete durable plan, authority, usage, evaluation, and turn-result view for one run UUID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": {"type": "string", "format": "uuid"}
                },
                "required": ["run_id"],
                "additionalProperties": false
            },
            "annotations": read_annotations,
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
                        "items": {"type": "string", "enum": ["fs.read", "fs.list", "fs.write", "fs.patch", "fs.mkdir", "process.exec", "git.status", "git.diff", "git.apply_patch", "system.info"]},
                        "uniqueItems": true,
                        "maxItems": 16
                    },
                    "context_token_budget": {"type": "integer", "minimum": 64, "maximum": MAX_CONTEXT_TOKENS, "default": DEFAULT_CONTEXT_TOKENS},
                    "estimated_output_tokens": {"type": "integer", "minimum": 1, "maximum": 100_000, "default": 4096},
                    "max_model_calls": {"type": "integer", "minimum": 1, "maximum": 32, "default": 8}
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
        "mcg_query_run" => query_run(state, &arguments),
        "mcg_project_context" => project_context(state, &arguments),
        "mcg_memory_search" => memory_search(state, &arguments),
        "mcg_run_agent" => run_agent(state, &arguments).await,
        _ => Err(RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "Unknown tool name",
        )),
    }
}

fn query_run(
    state: &ApiState,
    arguments: &serde_json::Map<String, Value>,
) -> Result<Value, RpcFailure> {
    require_only_keys(arguments, &["run_id"])?;
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
    match state.runtime.agent_run_view(run_id) {
        Ok(view) => tool_success(serde_json::to_value(view)),
        Err(error) if error.is_not_found() => Ok(tool_error(
            "run_not_found",
            "The requested agent run is not known to MCG",
        )),
        Err(_) => Ok(tool_error(
            "runtime_error",
            "MCG could not read the agent run",
        )),
    }
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
    4_096
}

const fn default_model_calls() -> u16 {
    8
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
            .any(|tool| !allowed.contains(&tool.as_str()))
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

fn project_id(arguments: &serde_json::Map<String, Value>) -> Result<ProjectId, RpcFailure> {
    let value = required_string(arguments, "project_id")?;
    ProjectId::from_str(value).map_err(|_| {
        RpcFailure::new(
            StatusCode::OK,
            None,
            INVALID_PARAMS,
            "project_id must be a UUID",
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
