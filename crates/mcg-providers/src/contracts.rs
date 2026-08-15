use std::{fmt, pin::Pin, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_core::Stream;
use mcg_domain::{ProjectId, ProxyCallId, ProxyConversationId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAi,
    OpenRouter,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMode {
    ChatGptBrowserOauth,
    ChatGptDeviceCode,
    ApiKey,
    BearerToken,
    None,
}

impl ProviderAuthMode {
    #[must_use]
    pub const fn billing_mode(self) -> BillingMode {
        match self {
            Self::ChatGptBrowserOauth | Self::ChatGptDeviceCode => BillingMode::Subscription,
            Self::ApiKey | Self::BearerToken => BillingMode::UsageBased,
            Self::None => BillingMode::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    Subscription,
    UsageBased,
    Unknown,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct ProviderCredential(String);

impl ProviderCredential {
    /// Creates a non-empty, redacting provider credential.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] when the secret is empty.
    pub fn new(secret: impl Into<String>) -> Result<Self, ProviderError> {
        let secret = secret.into();
        if secret.trim().is_empty() {
            return Err(ProviderError::Configuration(
                "provider credential cannot be empty".into(),
            ));
        }
        Ok(Self(secret))
    }

    /// Exposes the secret only to an executor that has already passed the
    /// host's credential-reference policy boundary. Callers must never log,
    /// serialize, or return this value to an agent or plugin guest.
    #[must_use]
    pub fn expose_to_host_executor(&self) -> &str {
        &self.0
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderCredential([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConnection {
    pub contract_version: u16,
    pub id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub auth_mode: ProviderAuthMode,
    #[serde(default)]
    pub organization: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
}

impl ProviderConnection {
    /// Validates the version, identity, and HTTP(S) base URL.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] for an invalid connection.
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.contract_version != CONTRACT_VERSION {
            return Err(ProviderError::Configuration(format!(
                "unsupported provider contract version {}",
                self.contract_version
            )));
        }
        if self.id.is_empty()
            || self.id.len() > 128
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            || self.display_name.trim().is_empty()
        {
            return Err(ProviderError::Configuration(
                "provider id must use 1-128 ASCII letters, digits, '-' or '_'; display name is required".into(),
            ));
        }
        let url = reqwest::Url::parse(&self.base_url)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") || url.cannot_be_a_base() {
            return Err(ProviderError::Configuration(
                "provider base URL must be an HTTP(S) base URL".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub responses: Option<bool>,
    pub chat_completions: Option<bool>,
    pub streaming: Option<bool>,
    pub tools: Option<bool>,
    pub structured_output: Option<bool>,
    pub vision: Option<bool>,
    pub reasoning: Option<bool>,
    /// The provider honors per-request reasoning effort, including `none`.
    #[serde(default)]
    pub reasoning_control: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input_per_million: Option<f64>,
    pub cached_input_per_million: Option<f64>,
    pub output_per_million: Option<f64>,
    pub currency: String,
    pub provenance: PriceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceProvenance {
    ProviderReported,
    McgCatalog { catalog_date: String },
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRouterRouting {
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "order")]
    pub provider_order: Vec<String>,
    #[serde(default)]
    pub allow_fallback: bool,
    #[serde(default)]
    pub require_parameters: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub display_name: String,
    pub owned_by: Option<String>,
    pub context_window: Option<u64>,
    pub capabilities: ModelCapabilities,
    pub price: Option<ModelPrice>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderModelCatalog {
    pub connection_id: String,
    pub models: Vec<ModelDescriptor>,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        url: String,
        detail: Option<String>,
    },
    File {
        file_id: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: Vec<ContentPart>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
    #[serde(default)]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredOutput {
    pub name: String,
    pub description: Option<String>,
    pub schema: Value,
    #[serde(default)]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningConfig {
    pub effort: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub contract_version: u16,
    pub model: String,
    #[serde(default)]
    pub instructions: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default)]
    pub structured_output: Option<StructuredOutput>,
    #[serde(default)]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}

pub const PROXY_LEDGER_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyInterface {
    Responses,
    ChatCompletions,
}

/// Secret-free grouping metadata for calls made through the local provider proxy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConversation {
    pub schema_version: u16,
    pub id: ProxyConversationId,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Immutable, privacy-preserving accounting evidence for one completed proxy call.
/// Prompt text, generated output, credentials, and arbitrary provider metadata are
/// deliberately excluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyCallRecord {
    pub schema_version: u16,
    pub id: ProxyCallId,
    pub conversation_id: ProxyConversationId,
    pub interface: ProxyInterface,
    pub alias: String,
    pub provider_id: String,
    pub model_id: String,
    pub response_id: String,
    pub request_sha256: String,
    pub streamed: bool,
    pub usage: Usage,
    pub estimated_cost_usd: Option<f64>,
    pub actual_cost_usd: Option<f64>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyConversationView {
    pub conversation: ProxyConversation,
    pub calls: Vec<ProxyCallRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyUsageSummary {
    pub conversation_count: u64,
    pub call_count: u64,
    pub usage: Usage,
    pub known_actual_cost_usd: f64,
    pub unknown_cost_call_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Message {
        role: MessageRole,
        content: Vec<ContentPart>,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Reasoning {
        summary: Vec<String>,
    },
    Refusal {
        text: String,
    },
    Unknown {
        raw: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub contract_version: u16,
    pub id: String,
    pub model: String,
    pub status: Option<String>,
    pub output: Vec<OutputItem>,
    pub usage: Usage,
    #[serde(default)]
    pub provider_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelStreamEvent {
    ResponseCreated {
        id: String,
        model: Option<String>,
    },
    OutputTextDelta {
        item_id: Option<String>,
        delta: String,
    },
    ToolArgumentsDelta {
        item_id: String,
        delta: String,
    },
    OutputItemAdded {
        item: OutputItem,
    },
    Completed {
        response: ModelResponse,
    },
    ProviderEvent {
        event_type: String,
        data: Value,
    },
}

pub type ProviderStream =
    Pin<Box<dyn Stream<Item = Result<ModelStreamEvent, ProviderError>> + Send>>;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider configuration is invalid: {0}")]
    Configuration(String),
    #[error("provider authentication failed: {0}")]
    Authentication(String),
    #[error("provider rate limit exceeded{retry}", retry = retry_after.map_or(String::new(), |d| format!("; retry after {} ms", d.as_millis())))]
    RateLimited { retry_after: Option<Duration> },
    #[error("provider request failed: {message}")]
    Request { message: String, retriable: bool },
    #[error("provider returned HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
        retriable: bool,
    },
    #[error("provider returned an incompatible response: {0}")]
    Incompatible(String),
}

impl ProviderError {
    #[must_use]
    pub const fn is_retriable(&self) -> bool {
        match self {
            Self::RateLimited { .. } => true,
            Self::Request { retriable, .. } | Self::Http { retriable, .. } => *retriable,
            Self::Configuration(_) | Self::Authentication(_) | Self::Incompatible(_) => false,
        }
    }
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn connection(&self) -> &ProviderConnection;
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError>;
    async fn create_response(&self, request: ModelRequest) -> Result<ModelResponse, ProviderError>;
    async fn stream_response(&self, request: ModelRequest)
    -> Result<ProviderStream, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_and_api_keys_have_distinct_billing_modes() {
        assert_eq!(
            ProviderAuthMode::ChatGptBrowserOauth.billing_mode(),
            BillingMode::Subscription
        );
        assert_eq!(
            ProviderAuthMode::ApiKey.billing_mode(),
            BillingMode::UsageBased
        );
    }

    #[test]
    fn credential_debug_output_is_redacted() {
        let credential = ProviderCredential::new("do-not-print").expect("credential");
        assert_eq!(format!("{credential:?}"), "ProviderCredential([REDACTED])");
        assert!(!format!("{credential:?}").contains("do-not-print"));
    }

    #[test]
    fn older_capability_catalogs_default_reasoning_control_to_unknown() {
        let capabilities: ModelCapabilities = serde_json::from_value(serde_json::json!({
            "responses": true,
            "chat_completions": null,
            "streaming": true,
            "tools": true,
            "structured_output": null,
            "vision": null,
            "reasoning": null
        }))
        .expect("older catalog capabilities");
        assert_eq!(capabilities.reasoning_control, None);
    }
}
