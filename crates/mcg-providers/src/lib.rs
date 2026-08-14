//! Canonical model-provider contracts and explicit authentication modes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub display_name: String,
    pub context_window: Option<u64>,
    pub supports_tools: Option<bool>,
    pub supports_structured_output: Option<bool>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider authentication failed: {0}")]
    Authentication(String),
    #[error("provider request failed: {0}")]
    Request(String),
    #[error("provider returned an incompatible response: {0}")]
    Incompatible(String),
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn auth_mode(&self) -> ProviderAuthMode;
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, ProviderError>;
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
}
