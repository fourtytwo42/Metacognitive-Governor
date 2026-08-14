use std::{collections::BTreeSet, convert::Infallible};

use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response, Sse, sse::Event},
};
use chrono::Utc;
use mcg_agents::AgentModelGateway;
use mcg_providers::{
    CONTRACT_VERSION, ContentPart, Message, MessageRole, ModelRequest, ModelResponse,
    ModelStreamEvent, OutputItem, PlannedRoute, ProviderError, ProviderModelRef,
    RequiredCapability, RoutePlan, RoutingPolicy, RoutingRequirements, StructuredOutput,
    ToolChoice, ToolDefinition, Usage, plan_routes, should_fallback,
};
use serde_json::{Value, json};
use tokio_stream::StreamExt;

use super::{ApiError, ApiState};

const MODEL_ALIASES: &[(&str, &str)] = &[
    ("mcg:auto", "Automatic governed routing"),
    ("mcg:coder", "Tool-capable coding route"),
    ("mcg:reasoner", "Reasoning-capable route"),
    ("mcg:cheap", "Lowest known-cost qualified route"),
    (
        "mcg:local",
        "Qualified route on a local or private endpoint",
    ),
];

#[derive(Debug)]
struct ProxyRequest {
    alias: String,
    request: ModelRequest,
    stream: bool,
}

pub(super) async fn models(State(state): State<ApiState>) -> Json<Value> {
    let available = !state.runtime.provider_route_candidates().await.is_empty();
    Json(json!({
        "object": "list",
        "data": MODEL_ALIASES.iter().map(|(id, description)| json!({
            "id": id,
            "object": "model",
            "created": Utc::now().timestamp(),
            "owned_by": "mcg",
            "metadata": {
                "description": description,
                "available": available,
                "governed": true
            }
        })).collect::<Vec<_>>()
    }))
}

pub(super) async fn responses(
    State(state): State<ApiState>,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    let proxy = parse_responses(&body)?;
    let plan = route(&state, &proxy).await?;
    if proxy.stream {
        let (selected, stream) = open_stream(&state, &proxy, &plan).await?;
        let alias = proxy.alias.clone();
        let events = stream.map(move |event| {
            Ok::<_, Infallible>(responses_stream_event(event, &alias, &selected.target))
        });
        Ok(Sse::new(events).into_response())
    } else {
        let (selected, response) = call_buffered(&state, &proxy, &plan).await?;
        Ok(Json(responses_wire(&proxy.alias, &selected, response)).into_response())
    }
}

pub(super) async fn chat_completions(
    State(state): State<ApiState>,
    Json(body): Json<Value>,
) -> Result<Response, ApiError> {
    let proxy = parse_chat(&body)?;
    let plan = route(&state, &proxy).await?;
    if proxy.stream {
        let (selected, stream) = open_stream(&state, &proxy, &plan).await?;
        let alias = proxy.alias.clone();
        let events = stream
            .map(move |event| {
                Ok::<_, Infallible>(chat_stream_event(event, &alias, &selected.target))
            })
            .chain(tokio_stream::once(Ok::<_, Infallible>(
                Event::default().data("[DONE]"),
            )));
        Ok(Sse::new(events).into_response())
    } else {
        let (selected, response) = call_buffered(&state, &proxy, &plan).await?;
        Ok(Json(chat_wire(&proxy.alias, &selected, &response)).into_response())
    }
}

async fn route(state: &ApiState, proxy: &ProxyRequest) -> Result<RoutePlan, ApiError> {
    if !MODEL_ALIASES.iter().any(|(alias, _)| *alias == proxy.alias) {
        return Err(ApiError::bad_request(format!(
            "unknown MCG model alias {}",
            proxy.alias
        )));
    }
    let mut candidates = state.runtime.provider_route_candidates().await;
    if proxy.alias == "mcg:local" {
        let local_ids = state
            .runtime
            .provider_connections()
            .map_err(ApiError::from)?
            .into_iter()
            .filter(|connection| is_local_endpoint(&connection.base_url))
            .map(|connection| connection.id)
            .collect::<BTreeSet<_>>();
        candidates.retain(|candidate| local_ids.contains(&candidate.target.provider_id));
    }
    let mut capabilities = Vec::new();
    if !proxy.request.tools.is_empty() || proxy.alias == "mcg:coder" {
        capabilities.push(RequiredCapability::Tools);
    }
    if proxy.request.structured_output.is_some() {
        capabilities.push(RequiredCapability::StructuredOutput);
    }
    if proxy.request.reasoning.is_some() || proxy.alias == "mcg:reasoner" {
        capabilities.push(RequiredCapability::Reasoning);
    }
    let input_tokens = serde_json::to_vec(&proxy.request.messages)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .len()
        .div_ceil(4) as u64;
    let output_tokens = proxy.request.max_output_tokens.unwrap_or(4_096);
    let policy = RoutingPolicy {
        allow_unknown_cost: proxy.alias != "mcg:cheap",
        ..RoutingPolicy::default()
    };
    plan_routes(
        &candidates,
        &RoutingRequirements { capabilities },
        policy,
        input_tokens,
        output_tokens,
    )
    .map_err(proxy_error)
}

async fn call_buffered(
    state: &ApiState,
    proxy: &ProxyRequest,
    plan: &RoutePlan,
) -> Result<(PlannedRoute, ModelResponse), ApiError> {
    let gateway = state.runtime.provider_gateway();
    let mut last_error = None;
    for selected in std::iter::once(&plan.pinned).chain(&plan.fallbacks) {
        match gateway
            .create_response(&selected.target, proxy.request.clone())
            .await
        {
            Ok(response) => return Ok((selected.clone(), response)),
            Err(error) if should_fallback(&error) => last_error = Some(error),
            Err(error) => return Err(proxy_error(error)),
        }
    }
    Err(proxy_error(last_error.unwrap_or_else(|| {
        ProviderError::Configuration("route plan produced no callable route".into())
    })))
}

async fn open_stream(
    state: &ApiState,
    proxy: &ProxyRequest,
    plan: &RoutePlan,
) -> Result<(PlannedRoute, mcg_providers::ProviderStream), ApiError> {
    let gateway = state.runtime.provider_gateway();
    let mut last_error = None;
    for selected in std::iter::once(&plan.pinned).chain(&plan.fallbacks) {
        match gateway
            .stream_response(&selected.target, proxy.request.clone())
            .await
        {
            Ok(stream) => return Ok((selected.clone(), stream)),
            Err(error) if should_fallback(&error) => last_error = Some(error),
            Err(error) => return Err(proxy_error(error)),
        }
    }
    Err(proxy_error(last_error.unwrap_or_else(|| {
        ProviderError::Configuration("route plan produced no streamable route".into())
    })))
}

fn parse_responses(body: &Value) -> Result<ProxyRequest, ApiError> {
    let alias = required_string(body, "model")?.to_owned();
    let input = body
        .get("input")
        .ok_or_else(|| ApiError::bad_request("input is required"))?;
    let messages = parse_responses_input(input)?;
    let structured_output = body
        .pointer("/text/format")
        .map(parse_structured_output)
        .transpose()?;
    Ok(ProxyRequest {
        alias: alias.clone(),
        request: ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: alias,
            instructions: optional_string(body, "instructions")?,
            messages,
            tools: parse_tools(body.get("tools"), false)?,
            tool_choice: parse_tool_choice(body.get("tool_choice"))?,
            structured_output,
            reasoning: parse_reasoning(body.get("reasoning"))?,
            temperature: optional_f64(body, "temperature")?,
            top_p: optional_f64(body, "top_p")?,
            max_output_tokens: optional_u64(body, "max_output_tokens")?,
            metadata: body.get("metadata").cloned().unwrap_or(Value::Null),
        },
        stream: body.get("stream").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn parse_chat(body: &Value) -> Result<ProxyRequest, ApiError> {
    let alias = required_string(body, "model")?.to_owned();
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("messages must be an array"))?
        .iter()
        .flat_map(parse_chat_message)
        .collect::<Result<Vec<_>, _>>()?;
    if messages.is_empty() {
        return Err(ApiError::bad_request("messages cannot be empty"));
    }
    let structured_output = body
        .get("response_format")
        .filter(|format| format.get("type").and_then(Value::as_str) == Some("json_schema"))
        .and_then(|format| format.get("json_schema"))
        .map(parse_structured_output)
        .transpose()?;
    Ok(ProxyRequest {
        alias: alias.clone(),
        request: ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: alias,
            instructions: None,
            messages,
            tools: parse_tools(body.get("tools"), true)?,
            tool_choice: parse_tool_choice(body.get("tool_choice"))?,
            structured_output,
            reasoning: None,
            temperature: optional_f64(body, "temperature")?,
            top_p: optional_f64(body, "top_p")?,
            max_output_tokens: optional_u64(body, "max_completion_tokens")?
                .or(optional_u64(body, "max_tokens")?),
            metadata: body.get("metadata").cloned().unwrap_or(Value::Null),
        },
        stream: body.get("stream").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn parse_responses_input(input: &Value) -> Result<Vec<Message>, ApiError> {
    if let Some(text) = input.as_str() {
        return Ok(vec![text_message(MessageRole::User, text)]);
    }
    let items = input
        .as_array()
        .ok_or_else(|| ApiError::bad_request("input must be a string or array"))?;
    items
        .iter()
        .map(|item| match item.get("type").and_then(Value::as_str) {
            Some("function_call") => Ok(Message {
                role: MessageRole::Assistant,
                content: vec![ContentPart::ToolCall {
                    call_id: required_string(item, "call_id")?.into(),
                    name: required_string(item, "name")?.into(),
                    arguments: required_string(item, "arguments")?.into(),
                }],
                name: None,
            }),
            Some("function_call_output") => Ok(Message {
                role: MessageRole::Tool,
                content: vec![ContentPart::ToolResult {
                    call_id: required_string(item, "call_id")?.into(),
                    output: value_as_text(item.get("output").unwrap_or(&Value::Null)),
                }],
                name: None,
            }),
            _ => parse_wire_message(item),
        })
        .collect()
}

fn parse_chat_message(message: &Value) -> Vec<Result<Message, ApiError>> {
    let role = match message.get("role").and_then(Value::as_str) {
        Some("system") => MessageRole::System,
        Some("developer") => MessageRole::Developer,
        Some("assistant") => MessageRole::Assistant,
        Some("tool") => MessageRole::Tool,
        _ => MessageRole::User,
    };
    if role == MessageRole::Tool {
        return vec![Ok(Message {
            role,
            content: vec![ContentPart::ToolResult {
                call_id: message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                output: value_as_text(message.get("content").unwrap_or(&Value::Null)),
            }],
            name: message.get("name").and_then(Value::as_str).map(Into::into),
        })];
    }
    let has_tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|calls| !calls.is_empty());
    let mut parsed = if message.get("content").is_none_or(Value::is_null) && has_tool_calls {
        Vec::new()
    } else {
        vec![parse_wire_message(message)]
    };
    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        parsed.push(Ok(Message {
            role: MessageRole::Assistant,
            content: vec![ContentPart::ToolCall {
                call_id: call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                name: call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                arguments: call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
            }],
            name: None,
        }));
    }
    parsed
}

fn parse_wire_message(value: &Value) -> Result<Message, ApiError> {
    let role = match value.get("role").and_then(Value::as_str) {
        Some("system") => MessageRole::System,
        Some("developer") => MessageRole::Developer,
        Some("assistant") => MessageRole::Assistant,
        Some("tool") => MessageRole::Tool,
        Some("user") | None => MessageRole::User,
        Some(other) => return Err(ApiError::bad_request(format!("unsupported role {other}"))),
    };
    let content = value.get("content").unwrap_or(&Value::Null);
    let parts = if let Some(text) = content.as_str() {
        vec![ContentPart::Text { text: text.into() }]
    } else {
        content
            .as_array()
            .ok_or_else(|| ApiError::bad_request("message content must be text or an array"))?
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str)?;
                match kind {
                    "input_text" | "output_text" | "text" => Some(Ok(ContentPart::Text {
                        text: part
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .into(),
                    })),
                    "input_image" | "image_url" => Some(Ok(ContentPart::ImageUrl {
                        url: part
                            .get("image_url")
                            .and_then(|url| url.as_str().or_else(|| url.get("url")?.as_str()))
                            .unwrap_or_default()
                            .into(),
                        detail: part.get("detail").and_then(Value::as_str).map(Into::into),
                    })),
                    _ => None,
                }
            })
            .collect::<Result<Vec<_>, ApiError>>()?
    };
    Ok(Message {
        role,
        content: parts,
        name: value.get("name").and_then(Value::as_str).map(Into::into),
    })
}

fn parse_tools(value: Option<&Value>, chat: bool) -> Result<Vec<ToolDefinition>, ApiError> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|tool| {
            let function = if chat {
                tool.get("function").unwrap_or(tool)
            } else {
                tool
            };
            let name = required_string(function, "name")?;
            if name.starts_with("host::") || name.starts_with("mcg::") {
                return Err(ApiError::bad_request(
                    "host:: and mcg:: tools require a governed MCG task context",
                ));
            }
            Ok(ToolDefinition {
                name: name.into(),
                description: function
                    .get("description")
                    .and_then(Value::as_str)
                    .map(Into::into),
                parameters: function
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"})),
                strict: function.get("strict").and_then(Value::as_bool),
            })
        })
        .collect()
}

fn parse_tool_choice(value: Option<&Value>) -> Result<Option<ToolChoice>, ApiError> {
    let Some(value) = value else { return Ok(None) };
    if let Some(choice) = value.as_str() {
        return Ok(Some(match choice {
            "auto" => ToolChoice::Auto,
            "none" => ToolChoice::None,
            "required" => ToolChoice::Required,
            _ => return Err(ApiError::bad_request("unsupported tool_choice")),
        }));
    }
    let name = value
        .pointer("/function/name")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("named tool_choice has no function name"))?;
    Ok(Some(ToolChoice::Named(name.into())))
}

fn parse_structured_output(value: &Value) -> Result<StructuredOutput, ApiError> {
    let schema = value.get("schema").cloned().unwrap_or(Value::Null);
    if !schema.is_object() {
        return Err(ApiError::bad_request(
            "json_schema schema must be an object",
        ));
    }
    Ok(StructuredOutput {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("response")
            .into(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(Into::into),
        schema,
        strict: value.get("strict").and_then(Value::as_bool),
    })
}

fn parse_reasoning(
    value: Option<&Value>,
) -> Result<Option<mcg_providers::ReasoningConfig>, ApiError> {
    let Some(value) = value else { return Ok(None) };
    if !value.is_object() {
        return Err(ApiError::bad_request("reasoning must be an object"));
    }
    Ok(Some(mcg_providers::ReasoningConfig {
        effort: value.get("effort").and_then(Value::as_str).map(Into::into),
        summary: value.get("summary").and_then(Value::as_str).map(Into::into),
    }))
}

fn responses_wire(alias: &str, selected: &PlannedRoute, response: ModelResponse) -> Value {
    json!({
        "id": response.id,
        "object": "response",
        "created_at": Utc::now().timestamp(),
        "status": response.status.unwrap_or_else(|| "completed".into()),
        "model": alias,
        "output": response.output.iter().map(output_item_wire).collect::<Vec<_>>(),
        "usage": usage_wire(&response.usage),
        "mcg_route": route_wire(selected)
    })
}

fn chat_wire(alias: &str, selected: &PlannedRoute, response: &ModelResponse) -> Value {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut refusal = None;
    for item in &response.output {
        match item {
            OutputItem::Message { content, .. } => {
                for part in content {
                    if let ContentPart::Text { text: part } = part {
                        text.push_str(part);
                    }
                }
            }
            OutputItem::ToolCall {
                id,
                name,
                arguments,
            } => tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {"name": name, "arguments": arguments}
            })),
            OutputItem::Refusal { text } => refusal = Some(text.clone()),
            OutputItem::Reasoning { .. } | OutputItem::Unknown { .. } => {}
        }
    }
    let finish_reason = if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    json!({
        "id": response.id,
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": alias,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": text,
                "refusal": refusal,
                "tool_calls": tool_calls
            },
            "finish_reason": finish_reason
        }],
        "usage": chat_usage_wire(&response.usage),
        "mcg_route": route_wire(selected)
    })
}

fn responses_stream_event(
    event: Result<ModelStreamEvent, ProviderError>,
    alias: &str,
    target: &ProviderModelRef,
) -> Event {
    match event {
        Ok(ModelStreamEvent::ResponseCreated { id, .. }) => sse(
            "response.created",
            json!({"type":"response.created","response":{"id":id,"model":alias,"status":"in_progress","mcg_route":{"provider_id":target.provider_id,"model_id":target.model_id}}}),
        ),
        Ok(ModelStreamEvent::OutputTextDelta { item_id, delta }) => sse(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","item_id":item_id,"delta":delta}),
        ),
        Ok(ModelStreamEvent::ToolArgumentsDelta { item_id, delta }) => sse(
            "response.function_call_arguments.delta",
            json!({"type":"response.function_call_arguments.delta","item_id":item_id,"delta":delta}),
        ),
        Ok(ModelStreamEvent::OutputItemAdded { item }) => sse(
            "response.output_item.added",
            json!({"type":"response.output_item.added","item":output_item_wire(&item)}),
        ),
        Ok(ModelStreamEvent::Completed { mut response }) => {
            response.model = alias.into();
            sse(
                "response.completed",
                json!({"type":"response.completed","response":responses_wire(alias, &PlannedRoute { target: target.clone(), estimated_cost: mcg_providers::CostEstimate { amount_usd: None, provenance: mcg_providers::PriceProvenance::Unknown } }, response)}),
            )
        }
        Ok(ModelStreamEvent::ProviderEvent { event_type, data }) => sse(&event_type, data),
        Err(error) => sse(
            "error",
            json!({"type":"error","error":{"message":error.to_string()}}),
        ),
    }
}

fn chat_stream_event(
    event: Result<ModelStreamEvent, ProviderError>,
    alias: &str,
    target: &ProviderModelRef,
) -> Event {
    let (id, delta, finish_reason, usage) = match event {
        Ok(ModelStreamEvent::ResponseCreated { id, .. }) => (
            id,
            json!({"role":"assistant","content":""}),
            Value::Null,
            Value::Null,
        ),
        Ok(ModelStreamEvent::OutputTextDelta { item_id, delta }) => (
            item_id.unwrap_or_else(|| "mcg-stream".into()),
            json!({"content":delta}),
            Value::Null,
            Value::Null,
        ),
        Ok(ModelStreamEvent::ToolArgumentsDelta { item_id, delta }) => (
            item_id.clone(),
            json!({"tool_calls":[{"index":0,"id":item_id,"function":{"arguments":delta}}]}),
            Value::Null,
            Value::Null,
        ),
        Ok(ModelStreamEvent::OutputItemAdded {
            item:
                OutputItem::ToolCall {
                    id,
                    name,
                    arguments,
                },
        }) => (
            id.clone(),
            json!({"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":arguments}}]}),
            Value::Null,
            Value::Null,
        ),
        Ok(ModelStreamEvent::Completed { response }) => (
            response.id,
            json!({}),
            json!(if response
                .output
                .iter()
                .any(|item| matches!(item, OutputItem::ToolCall { .. }))
            {
                "tool_calls"
            } else {
                "stop"
            }),
            chat_usage_wire(&response.usage),
        ),
        Ok(ModelStreamEvent::ProviderEvent { .. } | ModelStreamEvent::OutputItemAdded { .. }) => {
            ("mcg-stream".into(), json!({}), Value::Null, Value::Null)
        }
        Err(error) => return sse("error", json!({"error":{"message":error.to_string()}})),
    };
    sse(
        "message",
        json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": Utc::now().timestamp(),
            "model": alias,
            "choices": [{"index":0,"delta":delta,"finish_reason":finish_reason}],
            "usage": usage,
            "mcg_route": {"provider_id":target.provider_id,"model_id":target.model_id}
        }),
    )
}

fn output_item_wire(item: &OutputItem) -> Value {
    match item {
        OutputItem::Message { role, content } => json!({
            "type":"message",
            "role": role_wire(role),
            "content": content.iter().filter_map(|part| match part {
                ContentPart::Text { text } => Some(json!({"type":"output_text","text":text,"annotations":[]})),
                _ => None
            }).collect::<Vec<_>>()
        }),
        OutputItem::ToolCall {
            id,
            name,
            arguments,
        } => json!({
            "type":"function_call","id":id,"call_id":id,"name":name,"arguments":arguments,"status":"completed"
        }),
        OutputItem::Reasoning { summary } => json!({"type":"reasoning","summary":summary}),
        OutputItem::Refusal { text } => {
            json!({"type":"message","role":"assistant","content":[{"type":"refusal","refusal":text}]})
        }
        OutputItem::Unknown { raw } => raw.clone(),
    }
}

fn usage_wire(usage: &Usage) -> Value {
    json!({
        "input_tokens": usage.input_tokens,
        "input_tokens_details": {"cached_tokens": usage.cached_input_tokens},
        "output_tokens": usage.output_tokens,
        "output_tokens_details": {"reasoning_tokens": usage.reasoning_tokens},
        "total_tokens": usage.total_tokens
    })
}

fn chat_usage_wire(usage: &Usage) -> Value {
    json!({
        "prompt_tokens": usage.input_tokens,
        "completion_tokens": usage.output_tokens,
        "total_tokens": usage.total_tokens,
        "prompt_tokens_details": {"cached_tokens":usage.cached_input_tokens},
        "completion_tokens_details": {"reasoning_tokens":usage.reasoning_tokens}
    })
}

fn route_wire(route: &PlannedRoute) -> Value {
    json!({
        "provider_id": route.target.provider_id,
        "model_id": route.target.model_id,
        "estimated_cost_usd": route.estimated_cost.amount_usd,
        "cost_provenance": route.estimated_cost.provenance
    })
}

#[allow(clippy::needless_pass_by_value)] // Values are temporary wire payloads at every call site.
fn sse(event_type: &str, data: Value) -> Event {
    Event::default().event(event_type).data(data.to_string())
}

fn text_message(role: MessageRole, text: &str) -> Message {
    Message {
        role,
        content: vec![ContentPart::Text { text: text.into() }],
        name: None,
    }
}

fn role_wire(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::Developer => "developer",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn required_string<'a>(value: &'a Value, name: &str) -> Result<&'a str, ApiError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("{name} must be a non-empty string")))
}

fn optional_string(value: &Value, name: &str) -> Result<Option<String>, ApiError> {
    match value.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| ApiError::bad_request(format!("{name} must be a string"))),
    }
}

fn optional_f64(value: &Value, name: &str) -> Result<Option<f64>, ApiError> {
    match value.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Some)
            .ok_or_else(|| ApiError::bad_request(format!("{name} must be a finite number"))),
    }
}

fn optional_u64(value: &Value, name: &str) -> Result<Option<u64>, ApiError> {
    match value.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or_else(|| ApiError::bad_request(format!("{name} must be a positive integer"))),
    }
}

fn value_as_text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), ToOwned::to_owned)
}

fn is_local_endpoint(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let parsed_ip = host
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .ok();
    host == "localhost"
        || parsed_ip.is_some_and(|address| match address {
            std::net::IpAddr::V4(address) => address.is_private() || address.is_loopback(),
            std::net::IpAddr::V6(address) => address.is_loopback() || address.is_unique_local(),
        })
        || nip_host_is_private(host)
}

fn nip_host_is_private(host: &str) -> bool {
    let Some(encoded) = host.strip_suffix(".nip.io") else {
        return false;
    };
    let decoded = encoded.replace('-', ".");
    decoded
        .parse::<std::net::Ipv4Addr>()
        .is_ok_and(|address| address.is_private() || address.is_loopback())
}

fn proxy_error(error: ProviderError) -> ApiError {
    match error {
        ProviderError::Configuration(message) | ProviderError::Incompatible(message) => {
            ApiError::bad_request(message)
        }
        ProviderError::Authentication(message) => ApiError {
            status: axum::http::StatusCode::UNAUTHORIZED,
            code: "provider_authentication",
            message,
        },
        other => ApiError::bad_gateway(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_alias_accepts_only_loopback_or_private_endpoints() {
        assert!(is_local_endpoint("http://127.0.0.1:8080/v1"));
        assert!(is_local_endpoint("http://192.168.50.189:8080/v1"));
        assert!(is_local_endpoint("http://192-168-50-189.nip.io:30001/v1"));
        assert!(is_local_endpoint("http://[::1]:8080/v1"));
        assert!(!is_local_endpoint("https://api.openai.com/v1"));
        assert!(!is_local_endpoint("https://example.nip.io/v1"));
        assert!(!is_local_endpoint("https://8-8-8-8.nip.io/v1"));
    }

    #[test]
    fn chat_tool_history_accepts_standard_null_assistant_content() {
        let proxy = parse_chat(&json!({
            "model":"mcg:auto",
            "messages":[
                {"role":"user","content":"look up a value"},
                {"role":"assistant","content":null,"tool_calls":[{
                    "id":"call-1","type":"function",
                    "function":{"name":"caller::lookup","arguments":"{\"key\":\"x\"}"}
                }]},
                {"role":"tool","tool_call_id":"call-1","content":"value"}
            ]
        }))
        .unwrap_or_else(|_| panic!("chat request should parse"));
        assert_eq!(proxy.request.messages.len(), 3);
        assert!(matches!(
            proxy.request.messages[1].content.as_slice(),
            [ContentPart::ToolCall { call_id, .. }] if call_id == "call-1"
        ));
        assert!(matches!(
            proxy.request.messages[2].content.as_slice(),
            [ContentPart::ToolResult { call_id, .. }] if call_id == "call-1"
        ));
    }
}
