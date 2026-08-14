use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder, Response, StatusCode, Url, header};
use serde_json::{Value, json};
use tokio::time::sleep;

use crate::{
    CONTRACT_VERSION, ContentPart, MessageRole, ModelCapabilities, ModelDescriptor, ModelPrice,
    ModelProvider, ModelRequest, ModelResponse, ModelStreamEvent, OAuthTokenSet, OpenRouterRouting,
    OutputItem, PriceProvenance, ProviderAuthMode, ProviderConnection, ProviderCredential,
    ProviderError, ProviderKind, ProviderStream, RetryPolicy, ToolChoice, Usage,
};

pub const CHATGPT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

pub struct OpenAiCompatibleProviderBuilder {
    connection: ProviderConnection,
    credential: Option<ProviderCredential>,
    retry_policy: RetryPolicy,
    timeout: Duration,
    headers: HashMap<header::HeaderName, header::HeaderValue>,
    openrouter_routing: Option<OpenRouterRouting>,
}

impl OpenAiCompatibleProviderBuilder {
    /// Creates an `OpenAI` subscription provider from standalone OAuth tokens.
    /// This always uses the `ChatGPT` Codex backend and cannot select API billing.
    ///
    /// # Errors
    /// Returns an error for a non-OAuth mode, malformed account claim, invalid
    /// routing header, or provider construction failure.
    pub fn openai_subscription(
        id: impl Into<String>,
        display_name: impl Into<String>,
        auth_mode: ProviderAuthMode,
        tokens: &OAuthTokenSet,
    ) -> Result<OpenAiCompatibleProvider, ProviderError> {
        if !matches!(
            auth_mode,
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
        ) {
            return Err(ProviderError::Configuration(
                "OpenAI subscription provider requires a ChatGPT OAuth mode".into(),
            ));
        }
        let mut builder = Self::new(ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: id.into(),
            display_name: display_name.into(),
            kind: ProviderKind::OpenAi,
            base_url: CHATGPT_CODEX_BASE_URL.into(),
            auth_mode,
            organization: None,
            project: None,
        })
        .credential(tokens.access_token.clone());
        if let Some(account_id) = tokens.account_id()? {
            let account_id = header::HeaderValue::from_str(&account_id)
                .map_err(|error| ProviderError::Authentication(error.to_string()))?;
            builder.headers.insert(
                header::HeaderName::from_static("chatgpt-account-id"),
                account_id,
            );
        }
        builder.build()
    }

    #[must_use]
    pub fn new(connection: ProviderConnection) -> Self {
        Self {
            connection,
            credential: None,
            retry_policy: RetryPolicy::default(),
            timeout: Duration::from_secs(120),
            headers: HashMap::new(),
            openrouter_routing: None,
        }
    }

    #[must_use]
    pub fn credential(mut self, credential: ProviderCredential) -> Self {
        self.credential = Some(credential);
        self
    }

    #[must_use]
    pub const fn retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn header(mut self, name: header::HeaderName, value: header::HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    #[must_use]
    pub fn openrouter_routing(mut self, routing: OpenRouterRouting) -> Self {
        self.openrouter_routing = Some(routing);
        self
    }

    /// Adds `OpenRouter`'s optional application attribution headers.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] if a value is not a valid HTTP
    /// header or the connection is not an `OpenRouter` connection.
    pub fn openrouter_attribution(
        mut self,
        site_url: &str,
        app_name: &str,
    ) -> Result<Self, ProviderError> {
        if self.connection.kind != ProviderKind::OpenRouter {
            return Err(ProviderError::Configuration(
                "OpenRouter attribution requires an OpenRouter connection".into(),
            ));
        }
        let site_url = header::HeaderValue::from_str(site_url)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        let app_name = header::HeaderValue::from_str(app_name)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        self.headers
            .insert(header::HeaderName::from_static("http-referer"), site_url);
        self.headers
            .insert(header::HeaderName::from_static("x-title"), app_name);
        Ok(self)
    }

    /// Constructs a provider while enforcing the configured authentication boundary.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] for an invalid connection or an
    /// absent, unexpected, or provider-incompatible credential.
    pub fn build(self) -> Result<OpenAiCompatibleProvider, ProviderError> {
        self.connection.validate()?;
        let needs_credential = !matches!(self.connection.auth_mode, ProviderAuthMode::None);
        if needs_credential != self.credential.is_some() {
            return Err(ProviderError::Configuration(if needs_credential {
                format!(
                    "auth mode {:?} requires a credential",
                    self.connection.auth_mode
                )
            } else {
                "auth mode none cannot carry a credential".into()
            }));
        }
        if matches!(
            self.connection.auth_mode,
            ProviderAuthMode::ChatGptBrowserOauth | ProviderAuthMode::ChatGptDeviceCode
        ) && !matches!(self.connection.kind, ProviderKind::OpenAi)
        {
            return Err(ProviderError::Configuration(
                "ChatGPT OAuth credentials are only valid for an OpenAI connection".into(),
            ));
        }
        if self.openrouter_routing.is_some() && self.connection.kind != ProviderKind::OpenRouter {
            return Err(ProviderError::Configuration(
                "OpenRouter routing requires an OpenRouter connection".into(),
            ));
        }

        let mut base_url = Url::parse(&self.connection.base_url)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }
        let client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        Ok(OpenAiCompatibleProvider {
            connection: self.connection,
            credential: self.credential,
            retry_policy: self.retry_policy,
            headers: Arc::new(self.headers),
            base_url,
            client,
            openrouter_routing: self.openrouter_routing,
        })
    }
}

pub struct OpenAiCompatibleProvider {
    connection: ProviderConnection,
    credential: Option<ProviderCredential>,
    retry_policy: RetryPolicy,
    headers: Arc<HashMap<header::HeaderName, header::HeaderValue>>,
    base_url: Url,
    client: Client,
    openrouter_routing: Option<OpenRouterRouting>,
}

impl OpenAiCompatibleProvider {
    fn endpoint(&self, relative: &str) -> Result<Url, ProviderError> {
        self.base_url
            .join(relative)
            .map_err(|error| ProviderError::Configuration(error.to_string()))
    }

    fn authorize(&self, mut request: RequestBuilder) -> RequestBuilder {
        if let Some(credential) = &self.credential {
            request = request.bearer_auth(credential.expose());
        }
        if let Some(organization) = &self.connection.organization {
            request = request.header("OpenAI-Organization", organization);
        }
        if let Some(project) = &self.connection.project {
            request = request.header("OpenAI-Project", project);
        }
        for (name, value) in self.headers.iter() {
            request = request.header(name, value);
        }
        request
    }

    async fn send_with_retry<F>(&self, mut make_request: F) -> Result<Response, ProviderError>
    where
        F: FnMut() -> RequestBuilder,
    {
        let max_attempts = self.retry_policy.max_attempts.max(1);
        let mut backoff = self.retry_policy.initial_backoff;
        for attempt in 1..=max_attempts {
            match make_request().send().await {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let error = response_error(response).await;
                    if attempt == max_attempts || !error.is_retriable() {
                        return Err(error);
                    }
                }
                Err(error) => {
                    let provider_error = ProviderError::Request {
                        message: error.to_string(),
                        retriable: error.is_timeout() || error.is_connect() || error.is_request(),
                    };
                    if attempt == max_attempts || !provider_error.is_retriable() {
                        return Err(provider_error);
                    }
                }
            }
            sleep(backoff).await;
            backoff = backoff.saturating_mul(2).min(self.retry_policy.max_backoff);
        }
        Err(ProviderError::Request {
            message: "provider retry loop ended unexpectedly".into(),
            retriable: false,
        })
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn connection(&self) -> &ProviderConnection {
        &self.connection
    }

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let endpoint = self.endpoint("models")?;
        let response = self
            .send_with_retry(|| self.authorize(self.client.get(endpoint.clone())))
            .await?;
        let body: Value = response
            .json()
            .await
            .map_err(|error| ProviderError::Incompatible(error.to_string()))?;
        let models = body
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Incompatible("model list has no data array".into()))?;
        models
            .iter()
            .map(|model| parse_model(model, self.connection.kind))
            .collect()
    }

    async fn create_response(&self, request: ModelRequest) -> Result<ModelResponse, ProviderError> {
        validate_request(&request)?;
        let endpoint = self.endpoint("responses")?;
        let body = request_json(&request, false, self.openrouter_routing.as_ref());
        let response = self
            .send_with_retry(|| self.authorize(self.client.post(endpoint.clone()).json(&body)))
            .await?;
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| ProviderError::Request {
                message: error.to_string(),
                retriable: false,
            })?;
        let value: Value = if content_type.starts_with("text/event-stream") {
            completed_response_from_sse(&String::from_utf8_lossy(&bytes))?
        } else {
            serde_json::from_slice(&bytes)
                .map_err(|error| ProviderError::Incompatible(error.to_string()))?
        };
        parse_response(&value)
    }

    async fn stream_response(
        &self,
        request: ModelRequest,
    ) -> Result<ProviderStream, ProviderError> {
        validate_request(&request)?;
        let endpoint = self.endpoint("responses")?;
        let body = request_json(&request, true, self.openrouter_routing.as_ref());
        let response = self
            .send_with_retry(|| self.authorize(self.client.post(endpoint.clone()).json(&body)))
            .await?;
        let stream = response.bytes_stream().eventsource().map(|event| {
            let event = event.map_err(|error| ProviderError::Request {
                message: error.to_string(),
                retriable: false,
            })?;
            parse_stream_event(&event.event, &event.data)
        });
        Ok(Box::pin(stream))
    }
}

fn validate_request(request: &ModelRequest) -> Result<(), ProviderError> {
    if request.contract_version != CONTRACT_VERSION {
        return Err(ProviderError::Configuration(format!(
            "unsupported request contract version {}",
            request.contract_version
        )));
    }
    if request.model.trim().is_empty() {
        return Err(ProviderError::Configuration("model is required".into()));
    }
    if request.messages.is_empty() {
        return Err(ProviderError::Configuration(
            "at least one message is required".into(),
        ));
    }
    Ok(())
}

fn request_json(
    request: &ModelRequest,
    stream: bool,
    openrouter_routing: Option<&OpenRouterRouting>,
) -> Value {
    let input: Vec<Value> = request
        .messages
        .iter()
        .map(|message| {
            let content: Vec<Value> = message
                .content
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => json!({"type": "input_text", "text": text}),
                    ContentPart::ImageUrl { url, detail } => {
                        json!({"type": "input_image", "image_url": url, "detail": detail})
                    }
                    ContentPart::File { file_id } => {
                        json!({"type": "input_file", "file_id": file_id})
                    }
                })
                .collect();
            json!({
                "role": role_name(&message.role),
                "content": content,
                "name": message.name,
            })
        })
        .collect();
    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
                "strict": tool.strict,
            })
        })
        .collect();
    let tool_choice = request.tool_choice.as_ref().map(|choice| match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::None => json!("none"),
        ToolChoice::Required => json!("required"),
        ToolChoice::Named(name) => json!({"type": "function", "name": name}),
    });
    let text = request.structured_output.as_ref().map(|format| {
        json!({"format": {
            "type": "json_schema",
            "name": format.name,
            "description": format.description,
            "schema": format.schema,
            "strict": format.strict,
        }})
    });
    let mut value = json!({
        "model": request.model,
        "instructions": request.instructions,
        "input": input,
        "tools": (!tools.is_empty()).then_some(tools),
        "tool_choice": tool_choice,
        "text": text,
        "reasoning": request.reasoning,
        "temperature": request.temperature,
        "top_p": request.top_p,
        "max_output_tokens": request.max_output_tokens,
        "metadata": request.metadata,
        "stream": stream,
        "provider": openrouter_routing,
    });
    remove_null_object_fields(&mut value);
    value
}

fn remove_null_object_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|_, value| !value.is_null());
            for value in object.values_mut() {
                remove_null_object_fields(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                remove_null_object_fields(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn parse_model(model: &Value, kind: ProviderKind) -> Result<ModelDescriptor, ProviderError> {
    let id = model
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::Incompatible("model has no string id".into()))?;
    let context_window = model
        .get("context_window")
        .or_else(|| model.get("context_length"))
        .or_else(|| model.get("max_context_window"))
        .and_then(Value::as_u64);
    let price = if kind == ProviderKind::OpenRouter {
        parse_openrouter_price(model.get("pricing"))
    } else {
        None
    };
    let supported = model
        .get("supported_parameters")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let modalities = model
        .pointer("/architecture/input_modalities")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(ModelDescriptor {
        id: id.into(),
        display_name: model
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .into(),
        owned_by: model
            .get("owned_by")
            .and_then(Value::as_str)
            .map(Into::into),
        context_window,
        capabilities: ModelCapabilities {
            responses: None,
            chat_completions: None,
            streaming: None,
            tools: Some(supported.contains(&"tools")),
            structured_output: Some(
                supported
                    .iter()
                    .any(|value| matches!(*value, "response_format" | "structured_outputs")),
            ),
            vision: Some(modalities.contains(&"image")),
            reasoning: Some(supported.iter().any(|value| value.starts_with("reasoning"))),
        },
        price,
        metadata: model.clone(),
    })
}

fn parse_openrouter_price(pricing: Option<&Value>) -> Option<ModelPrice> {
    let pricing = pricing?.as_object()?;
    let per_million = |name: &str| {
        pricing
            .get(name)
            .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
            .map(|value| value * 1_000_000.0)
    };
    Some(ModelPrice {
        input_per_million: per_million("prompt"),
        cached_input_per_million: per_million("input_cache_read"),
        output_per_million: per_million("completion"),
        currency: "USD".into(),
        provenance: PriceProvenance::ProviderReported,
    })
}

fn parse_response(value: &Value) -> Result<ModelResponse, ProviderError> {
    let id = string_field(value, "id")?;
    let model = string_field(value, "model")?;
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Incompatible("response has no output array".into()))?
        .iter()
        .map(parse_output_item)
        .collect();
    Ok(ModelResponse {
        contract_version: CONTRACT_VERSION,
        id: id.into(),
        model: model.into(),
        status: value.get("status").and_then(Value::as_str).map(Into::into),
        output,
        usage: parse_usage(value.get("usage")),
        provider_metadata: value.clone(),
    })
}

fn parse_output_item(item: &Value) -> OutputItem {
    match item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "message" => {
            let role = parse_role(
                item.get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("assistant"),
            );
            let mut content = Vec::new();
            for part in item
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                match part.get("type").and_then(Value::as_str) {
                    Some("output_text") => content.push(ContentPart::Text {
                        text: part
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .into(),
                    }),
                    Some("refusal") => {
                        return OutputItem::Refusal {
                            text: part
                                .get("refusal")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .into(),
                        };
                    }
                    _ => {}
                }
            }
            OutputItem::Message { role, content }
        }
        "function_call" => OutputItem::ToolCall {
            id: item
                .get("call_id")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            name: item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            arguments: item
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        },
        "reasoning" => OutputItem::Reasoning {
            summary: item
                .get("summary")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|part| part.get("text").and_then(Value::as_str).map(Into::into))
                .collect(),
        },
        _ => OutputItem::Unknown { raw: item.clone() },
    }
}

fn parse_stream_event(event_name: &str, data: &str) -> Result<ModelStreamEvent, ProviderError> {
    if data == "[DONE]" {
        return Ok(ModelStreamEvent::ProviderEvent {
            event_type: "done".into(),
            data: Value::Null,
        });
    }
    let data: Value = serde_json::from_str(data)
        .map_err(|error| ProviderError::Incompatible(error.to_string()))?;
    let event_type = data
        .get("type")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(event_name);
    match event_type {
        "response.created" => {
            let response = data.get("response").unwrap_or(&data);
            Ok(ModelStreamEvent::ResponseCreated {
                id: string_field(response, "id")?.into(),
                model: response
                    .get("model")
                    .and_then(Value::as_str)
                    .map(Into::into),
            })
        }
        "response.output_text.delta" => Ok(ModelStreamEvent::OutputTextDelta {
            item_id: data.get("item_id").and_then(Value::as_str).map(Into::into),
            delta: data
                .get("delta")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        }),
        "response.function_call_arguments.delta" => Ok(ModelStreamEvent::ToolArgumentsDelta {
            item_id: data
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            delta: data
                .get("delta")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        }),
        "response.output_item.added" => Ok(ModelStreamEvent::OutputItemAdded {
            item: parse_output_item(data.get("item").unwrap_or(&Value::Null)),
        }),
        "response.completed" => Ok(ModelStreamEvent::Completed {
            response: parse_response(data.get("response").unwrap_or(&data))?,
        }),
        _ => Ok(ModelStreamEvent::ProviderEvent {
            event_type: event_type.into(),
            data,
        }),
    }
}

fn completed_response_from_sse(body: &str) -> Result<Value, ProviderError> {
    let mut last_event = None;
    for line in body.lines().rev() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let value: Value = serde_json::from_str(data.trim())
            .map_err(|error| ProviderError::Incompatible(error.to_string()))?;
        if last_event.is_none() {
            last_event = Some(value.clone());
        }
        if value.get("type").and_then(Value::as_str) == Some("response.completed") {
            return value.get("response").cloned().ok_or_else(|| {
                ProviderError::Incompatible("completed SSE event has no response".into())
            });
        }
    }
    let detail = last_event
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/response/error/message")
                .or_else(|| value.pointer("/error/message"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            last_event
                .as_ref()
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
        })
        .unwrap_or("no terminal event");
    Err(ProviderError::Incompatible(format!(
        "SSE response ended without response.completed: {detail}"
    )))
}

fn parse_usage(usage: Option<&Value>) -> Usage {
    let usage = usage.unwrap_or(&Value::Null);
    let input_tokens = u64_field(usage, "input_tokens");
    let output_tokens = u64_field(usage, "output_tokens");
    Usage {
        input_tokens,
        cached_input_tokens: usage
            .pointer("/input_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens,
        reasoning_tokens: usage
            .pointer("/output_tokens_details/reasoning_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: usage
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(input_tokens + output_tokens),
    }
}

fn role_name(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::Developer => "developer",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn parse_role(role: &str) -> MessageRole {
    match role {
        "system" => MessageRole::System,
        "developer" => MessageRole::Developer,
        "user" => MessageRole::User,
        "tool" => MessageRole::Tool,
        _ => MessageRole::Assistant,
    }
}

fn string_field<'a>(value: &'a Value, name: &str) -> Result<&'a str, ProviderError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::Incompatible(format!("response has no string {name}")))
}

fn u64_field(value: &Value, name: &str) -> u64 {
    value.get(name).and_then(Value::as_u64).unwrap_or(0)
}

async fn response_error(response: Response) -> ProviderError {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    let body = response.text().await.unwrap_or_default();
    let message = error_message(&body);
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        ProviderError::Authentication(message)
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        ProviderError::RateLimited { retry_after }
    } else {
        ProviderError::Http {
            status: status.as_u16(),
            message,
            retriable: status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT,
        }
    }
}

fn error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
        })
        .and_then(Value::as_str)
        .unwrap_or(body);
    message.chars().take(500).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        extract::State,
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
    };
    use base64::Engine;
    use futures_util::StreamExt;
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;
    use crate::{ContentPart, Message, ModelRequest};

    async fn fixture_server() -> String {
        async fn models() -> Json<Value> {
            Json(
                json!({"object":"list","data":[{"id":"fixture-model","owned_by":"test","context_window":8192}]}),
            )
        }
        fn responses() -> Json<Value> {
            Json(json!({
                "id":"resp_fixture","model":"fixture-model","status":"completed",
                "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello"}]}],
                "usage":{"input_tokens":4,"output_tokens":1,"total_tokens":5}
            }))
        }
        fn stream() -> impl IntoResponse {
            (
                [(header::CONTENT_TYPE, "text/event-stream")],
                concat!(
                    "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_stream\",\"model\":\"fixture-model\"}}\n\n",
                    "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n"
                ),
            )
        }
        async fn response_route(Json(body): Json<Value>) -> impl IntoResponse {
            if body.get("stream").and_then(Value::as_bool) == Some(true) {
                stream().into_response()
            } else {
                responses().into_response()
            }
        }
        let app = Router::new()
            .route("/v1/models", get(models))
            .route("/v1/responses", post(response_route));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move { axum::serve(listener, app).await.expect("fixture server") });
        format!("http://{address}/v1")
    }

    fn connection(base_url: String) -> ProviderConnection {
        ProviderConnection {
            contract_version: CONTRACT_VERSION,
            id: "fixture".into(),
            display_name: "Fixture".into(),
            kind: ProviderKind::OpenAiCompatible,
            base_url,
            auth_mode: ProviderAuthMode::None,
            organization: None,
            project: None,
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            contract_version: CONTRACT_VERSION,
            model: "fixture-model".into(),
            instructions: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".into(),
                }],
                name: None,
            }],
            tools: vec![],
            tool_choice: None,
            structured_output: None,
            reasoning: None,
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            metadata: Value::Null,
        }
    }

    #[tokio::test]
    async fn discovers_models_and_normalizes_response() {
        let provider = OpenAiCompatibleProviderBuilder::new(connection(fixture_server().await))
            .build()
            .expect("provider");
        let models = provider.list_models().await.expect("models");
        assert_eq!(models[0].id, "fixture-model");
        assert_eq!(models[0].context_window, Some(8192));
        let response = provider.create_response(request()).await.expect("response");
        assert_eq!(response.usage.total_tokens, 5);
        assert!(
            matches!(&response.output[0], OutputItem::Message { content, .. } if content[0] == ContentPart::Text { text: "hello".into() })
        );
    }

    #[tokio::test]
    async fn parses_streaming_events() {
        let provider = OpenAiCompatibleProviderBuilder::new(connection(fixture_server().await))
            .build()
            .expect("provider");
        let events = provider
            .stream_response(request())
            .await
            .expect("stream")
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[1], Ok(ModelStreamEvent::OutputTextDelta { delta, .. }) if delta == "hi")
        );
    }

    #[tokio::test]
    async fn common_conformance_probe_passes_openai_fixture() {
        let provider = OpenAiCompatibleProviderBuilder::new(connection(fixture_server().await))
            .build()
            .expect("provider");
        let report = crate::probe_model(&provider, "fixture-model")
            .await
            .expect("conformance probe");
        assert_eq!(report.model.capabilities.responses, Some(true));
        assert_eq!(report.model.capabilities.streaming, Some(true));
    }

    #[tokio::test]
    async fn retries_transient_server_errors() {
        async fn flaky(State(counter): State<Arc<AtomicUsize>>) -> impl IntoResponse {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                StatusCode::SERVICE_UNAVAILABLE.into_response()
            } else {
                Json(json!({"data":[]})).into_response()
            }
        }
        let counter = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/v1/models", get(flaky))
            .with_state(Arc::clone(&counter));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server") });
        let provider =
            OpenAiCompatibleProviderBuilder::new(connection(format!("http://{address}/v1")))
                .retry_policy(RetryPolicy {
                    max_attempts: 2,
                    initial_backoff: Duration::ZERO,
                    max_backoff: Duration::ZERO,
                })
                .build()
                .expect("provider");
        assert!(provider.list_models().await.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn auth_mode_cannot_fall_back_or_cross_provider_boundary() {
        let mut config = connection("https://example.com/v1".into());
        config.kind = ProviderKind::OpenAi;
        config.auth_mode = ProviderAuthMode::ChatGptBrowserOauth;
        assert!(
            OpenAiCompatibleProviderBuilder::new(config.clone())
                .build()
                .is_err()
        );
        config.kind = ProviderKind::OpenRouter;
        assert!(
            OpenAiCompatibleProviderBuilder::new(config)
                .credential(ProviderCredential::new("oauth-token").expect("token"))
                .build()
                .is_err()
        );
    }

    #[test]
    fn openrouter_metadata_supplies_capabilities_and_cost_provenance() {
        let model = parse_model(
            &json!({
                "id": "vendor/model",
                "name": "Model",
                "context_length": 128_000,
                "supported_parameters": ["tools", "response_format", "reasoning"],
                "architecture": {"input_modalities": ["text", "image"]},
                "pricing": {"prompt": "0.000002", "completion": "0.000006"}
            }),
            ProviderKind::OpenRouter,
        )
        .expect("OpenRouter model");
        assert_eq!(model.capabilities.tools, Some(true));
        assert_eq!(model.capabilities.structured_output, Some(true));
        assert_eq!(model.capabilities.vision, Some(true));
        assert_eq!(
            model
                .price
                .as_ref()
                .and_then(|price| price.input_per_million),
            Some(2.0)
        );
        assert!(matches!(
            model.price.map(|price| price.provenance),
            Some(PriceProvenance::ProviderReported)
        ));
    }

    #[test]
    fn openrouter_options_cannot_leak_to_custom_connections() {
        let result =
            OpenAiCompatibleProviderBuilder::new(connection("https://example.com/v1".into()))
                .openrouter_routing(OpenRouterRouting {
                    provider_order: vec!["provider-a".into()],
                    allow_fallback: false,
                    require_parameters: true,
                    data_collection: Some("deny".into()),
                })
                .build();
        assert!(result.is_err());
    }

    #[test]
    fn subscription_builder_cannot_select_api_billing() {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{}");
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{}");
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{header}.{payload}.sig")).expect("id"),
            access_token: ProviderCredential::new("access-token").expect("access"),
            refresh_token: ProviderCredential::new("refresh-token").expect("refresh"),
        };
        let result = OpenAiCompatibleProviderBuilder::openai_subscription(
            "openai-subscription",
            "OpenAI subscription",
            ProviderAuthMode::ApiKey,
            &tokens,
        );
        assert!(result.is_err());
        let provider = OpenAiCompatibleProviderBuilder::openai_subscription(
            "openai-subscription",
            "OpenAI subscription",
            ProviderAuthMode::ChatGptDeviceCode,
            &tokens,
        )
        .expect("subscription provider");
        assert_eq!(provider.connection().base_url, CHATGPT_CODEX_BASE_URL);
        assert_eq!(
            provider.connection().auth_mode.billing_mode(),
            crate::BillingMode::Subscription
        );
    }
}
