// Device authorization protocol behavior is adapted from OpenAI Codex's
// Apache-2.0-licensed `codex-rs/login/src/device_code_auth.rs`; see
// `THIRD_PARTY_NOTICES.md`. This module is standalone and does not read Codex
// credential files or invoke the Codex runtime.

use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ProviderCredential, ProviderError};

pub const OPENAI_AUTH_ISSUER: &str = "https://auth.openai.com";
pub const OPENAI_CODEX_PUBLIC_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_USERINFO_PATH: &str = "api/accounts/oauth/userinfo";
const CHATGPT_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api/";
const CHATGPT_ACCOUNTS_PATH: &str = "wham/accounts/check";
const CHATGPT_PROFILE_PATH: &str = "wham/profiles/me";
const CHATGPT_USAGE_PATH: &str = "wham/usage";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorizationConfig {
    pub issuer: String,
    pub client_id: String,
    pub timeout: Duration,
}

impl Default for DeviceAuthorizationConfig {
    fn default() -> Self {
        Self {
            issuer: OPENAI_AUTH_ISSUER.into(),
            client_id: OPENAI_CODEX_PUBLIC_CLIENT_ID.into(),
            timeout: Duration::from_mins(15),
        }
    }
}

impl DeviceAuthorizationConfig {
    fn validate(&self) -> Result<Url, ProviderError> {
        let issuer = Url::parse(&self.issuer)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        if issuer.scheme() != "https" && !issuer.host_str().is_some_and(is_loopback_host) {
            return Err(ProviderError::Configuration(
                "OAuth issuer must use HTTPS except in loopback tests".into(),
            ));
        }
        if self.client_id.trim().is_empty() || self.timeout.is_zero() {
            return Err(ProviderError::Configuration(
                "OAuth client id and a non-zero timeout are required".into(),
            ));
        }
        Ok(issuer)
    }
}

#[derive(Clone)]
pub struct DeviceAuthorization {
    pub verification_url: String,
    pub user_code: String,
    device_auth_id: ProviderCredential,
    interval: Duration,
}

impl std::fmt::Debug for DeviceAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceAuthorization")
            .field("verification_url", &self.verification_url)
            .field("user_code", &"[REDACTED]")
            .field("device_auth_id", &"[REDACTED]")
            .field("interval", &self.interval)
            .finish()
    }
}

#[derive(Clone)]
pub struct OAuthTokenSet {
    pub id_token: ProviderCredential,
    pub access_token: ProviderCredential,
    pub refresh_token: ProviderCredential,
}

/// Secret-free account identity projected from issuer-provided OAuth claims.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAccountProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_structure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<CodexUsageSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexUsageWindow {
    pub used_percent: u8,
    pub remaining_percent: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexAdditionalRateLimit {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_window: Option<CodexUsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_window: Option<CodexUsageWindow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexUsageSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_window: Option<CodexUsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_window: Option<CodexUsageWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_rate_limits: Vec<CodexAdditionalRateLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_reached: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits_unlimited: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_credits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overage_limit_reached: Option<bool>,
}

impl OAuthAccountProfile {
    fn from_claims(claims: &serde_json::Value) -> Option<Self> {
        let issuer_profile = claims
            .get("https://api.openai.com/profile")
            .and_then(serde_json::Value::as_object);
        let claim = |names: &[&str]| {
            names.iter().find_map(|name| {
                claims
                    .get(*name)
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        issuer_profile
                            .and_then(|profile| profile.get(*name))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
        };
        let profile = Self {
            display_name: claim(&["name", "display_name"]),
            username: claim(&["preferred_username", "username", "nickname"]),
            email: claim(&["email"]),
            avatar_url: claim(&["picture", "avatar_url"]).filter(|value| {
                Url::parse(value)
                    .is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
            }),
            ..Self::default()
        };
        (profile.display_name.is_some()
            || profile.username.is_some()
            || profile.email.is_some()
            || profile.avatar_url.is_some())
        .then_some(profile)
    }

    /// Prefers issuer `UserInfo` fields while retaining token claims omitted by that response.
    #[must_use]
    pub fn with_fallback(mut self, fallback: Option<Self>) -> Self {
        if let Some(fallback) = fallback {
            self.display_name = self.display_name.or(fallback.display_name);
            self.username = self.username.or(fallback.username);
            self.email = self.email.or(fallback.email);
            self.avatar_url = self.avatar_url.or(fallback.avatar_url);
            self.plan_type = self.plan_type.or(fallback.plan_type);
            self.account_structure = self.account_structure.or(fallback.account_structure);
            self.account_role = self.account_role.or(fallback.account_role);
            self.usage = self.usage.or(fallback.usage);
        }
        self
    }
}

impl OAuthTokenSet {
    /// Reads optional `OpenID` profile claims without exposing either OAuth token.
    /// The direct token exchange establishes the source of these display-only claims.
    ///
    /// # Errors
    /// Returns an authentication error when the ID-token claim payload is malformed.
    pub fn account_profile(&self) -> Result<Option<OAuthAccountProfile>, ProviderError> {
        let Some(claims) = jwt_claims(self.id_token.expose())? else {
            return Ok(None);
        };
        Ok(OAuthAccountProfile::from_claims(&claims))
    }

    /// Fetches standard `OpenID` profile claims from the issuer's `UserInfo` endpoint.
    /// The bearer token is used only in the authorization header and is never returned.
    ///
    /// # Errors
    /// Returns an authentication error for an invalid issuer, transport failure,
    /// rejected bearer token, or malformed `UserInfo` response.
    pub async fn fetch_account_profile(
        &self,
        issuer: &str,
    ) -> Result<Option<OAuthAccountProfile>, ProviderError> {
        let mut endpoint =
            Url::parse(issuer).map_err(|error| ProviderError::Configuration(error.to_string()))?;
        if endpoint.scheme() != "https" && !endpoint.host_str().is_some_and(is_loopback_host) {
            return Err(ProviderError::Configuration(
                "OAuth issuer must use HTTPS except in loopback tests".into(),
            ));
        }
        if !endpoint.path().ends_with('/') {
            endpoint.set_path(&format!("{}/", endpoint.path()));
        }
        let endpoint = endpoint
            .join(OPENAI_USERINFO_PATH)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        let response = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?
            .get(endpoint)
            .bearer_auth(self.access_token.expose())
            .send()
            .await
            .map_err(|error| auth_transport_error(&error))?;
        if !response.status().is_success() {
            return Err(auth_status_error(response.status()));
        }
        let claims: serde_json::Value = response
            .json()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        Ok(OAuthAccountProfile::from_claims(&claims))
    }

    /// Fetches the same secret-free account metadata and rolling usage summary used by Codex.
    /// These `ChatGPT` backend routes are best-effort and may be unavailable for some accounts.
    /// No cookies or credentials are returned to callers.
    ///
    /// # Errors
    ///
    /// Returns an authentication, transport, response, or configuration error when the
    /// authenticated account projection cannot be fetched safely.
    pub async fn fetch_codex_account_profile(
        &self,
    ) -> Result<Option<OAuthAccountProfile>, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        let base = Url::parse(CHATGPT_BACKEND_BASE_URL)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        let account_id = self.account_id()?;
        let mut profile = OAuthAccountProfile::default();
        let mut received = false;

        let accounts = fetch_chatgpt_json(
            &client,
            &base,
            CHATGPT_ACCOUNTS_PATH,
            self.access_token.expose(),
            account_id.as_deref(),
        );
        let chatgpt_profile = fetch_chatgpt_json(
            &client,
            &base,
            CHATGPT_PROFILE_PATH,
            self.access_token.expose(),
            account_id.as_deref(),
        );
        let usage = fetch_chatgpt_json(
            &client,
            &base,
            CHATGPT_USAGE_PATH,
            self.access_token.expose(),
            account_id.as_deref(),
        );
        let (accounts, chatgpt_profile, usage) = tokio::join!(accounts, chatgpt_profile, usage);

        if let Some(value) = accounts.ok().flatten() {
            apply_accounts_check(&mut profile, &value, account_id.as_deref());
            received = true;
        }
        if let Some(value) = chatgpt_profile.ok().flatten() {
            apply_chatgpt_profile(&mut profile, &value);
            received = true;
        }
        if let Some(value) = usage.ok().flatten() {
            if profile.plan_type.is_none() {
                profile.plan_type = clean_string(value.get("plan_type"));
            }
            profile.usage = parse_usage(&value);
            received = true;
        }
        Ok(received.then_some(profile))
    }

    /// Reads the account-routing claim from issuer-provided OAuth tokens.
    /// This decodes claims only; authenticity is established by the direct TLS
    /// token exchange with the configured issuer.
    ///
    /// # Errors
    /// Returns [`ProviderError::Authentication`] when a JWT claim payload is present
    /// but malformed. Access-token claims are preferred for Codex compatibility;
    /// the ID token remains a fallback for issuer variants that place it there.
    pub fn account_id(&self) -> Result<Option<String>, ProviderError> {
        oauth_account_id_from_credentials(&self.access_token, &self.id_token)
    }

    /// Rotates OAuth tokens through the issuer while preserving any fields the
    /// refresh response intentionally omits.
    ///
    /// # Errors
    /// Returns an authentication error for invalid configuration, transport,
    /// permanent refresh rejection, or malformed returned credentials.
    pub async fn refresh(&self, config: &DeviceAuthorizationConfig) -> Result<Self, ProviderError> {
        #[derive(Serialize)]
        struct RefreshRequest<'a> {
            client_id: &'a str,
            grant_type: &'static str,
            refresh_token: &'a str,
        }
        #[derive(Deserialize)]
        struct RefreshResponse {
            #[serde(rename = "id_token")]
            id: Option<String>,
            #[serde(rename = "access_token")]
            access: Option<String>,
            #[serde(rename = "refresh_token")]
            refresh: Option<String>,
        }
        let mut endpoint = config.validate()?;
        if !endpoint.path().ends_with('/') {
            endpoint.set_path(&format!("{}/", endpoint.path()));
        }
        let endpoint = endpoint
            .join("oauth/token")
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        let response = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?
            .post(endpoint)
            .json(&RefreshRequest {
                client_id: &config.client_id,
                grant_type: "refresh_token",
                refresh_token: self.refresh_token.expose(),
            })
            .send()
            .await
            .map_err(|error| auth_transport_error(&error))?;
        if !response.status().is_success() {
            return Err(auth_status_error(response.status()));
        }
        let refreshed: RefreshResponse = response
            .json()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        Ok(Self {
            id_token: refreshed
                .id
                .map(ProviderCredential::new)
                .transpose()?
                .unwrap_or_else(|| self.id_token.clone()),
            access_token: refreshed
                .access
                .map(ProviderCredential::new)
                .transpose()?
                .unwrap_or_else(|| self.access_token.clone()),
            refresh_token: refreshed
                .refresh
                .map(ProviderCredential::new)
                .transpose()?
                .unwrap_or_else(|| self.refresh_token.clone()),
        })
    }
}

pub(crate) fn oauth_account_id_from_credentials(
    access_token: &ProviderCredential,
    id_token: &ProviderCredential,
) -> Result<Option<String>, ProviderError> {
    for token in [access_token, id_token] {
        let parts = token.expose().split('.').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let payload = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        let claims: serde_json::Value = serde_json::from_slice(&payload)
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        if let Some(account_id) = claims
            .pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id")
            .and_then(serde_json::Value::as_str)
        {
            return Ok(Some(account_id.into()));
        }
    }
    Ok(None)
}

async fn fetch_chatgpt_json(
    client: &Client,
    base: &Url,
    path: &str,
    access_token: &str,
    account_id: Option<&str>,
) -> Result<Option<serde_json::Value>, ProviderError> {
    let endpoint = base
        .join(path)
        .map_err(|error| ProviderError::Configuration(error.to_string()))?;
    let mut request = client
        .get(endpoint)
        .bearer_auth(access_token)
        .header("OAI-App-Brand", "chatgpt");
    if let Some(account_id) = account_id {
        request = request.header("chatgpt-account-id", account_id);
    }
    let response = request
        .send()
        .await
        .map_err(|error| auth_transport_error(&error))?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
    ) {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(auth_status_error(response.status()));
    }
    response
        .json()
        .await
        .map(Some)
        .map_err(|error| ProviderError::Authentication(error.to_string()))
}

fn clean_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn https_url(value: Option<&serde_json::Value>) -> Option<String> {
    clean_string(value).filter(|value| {
        Url::parse(value).is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
    })
}

fn apply_accounts_check(
    profile: &mut OAuthAccountProfile,
    value: &serde_json::Value,
    claimed_account_id: Option<&str>,
) {
    let accounts = value.get("accounts").and_then(serde_json::Value::as_array);
    let ordered_id = value
        .get("account_ordering")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(serde_json::Value::as_str);
    let selected = accounts.and_then(|items| {
        let preferred = claimed_account_id.or(ordered_id);
        preferred
            .and_then(|id| {
                items
                    .iter()
                    .find(|item| item.get("id").and_then(serde_json::Value::as_str) == Some(id))
            })
            .or_else(|| items.first())
    });
    let Some(selected) = selected else { return };
    profile.display_name = clean_string(selected.get("name")).or(profile.display_name.take());
    profile.avatar_url =
        https_url(selected.get("profile_picture_url")).or(profile.avatar_url.take());
    profile.plan_type = clean_string(selected.get("plan_type")).or(profile.plan_type.take());
    profile.account_structure =
        clean_string(selected.get("structure")).or(profile.account_structure.take());
    profile.account_role =
        clean_string(selected.get("account_user_role")).or(profile.account_role.take());
}

fn apply_chatgpt_profile(profile: &mut OAuthAccountProfile, value: &serde_json::Value) {
    let value = value.get("profile").unwrap_or(value);
    profile.display_name = clean_string(value.get("name"))
        .or_else(|| clean_string(value.get("display_name")))
        .or(profile.display_name.take());
    profile.username = clean_string(value.get("username")).or(profile.username.take());
    profile.avatar_url = https_url(value.get("profile_picture_url"))
        .or_else(|| https_url(value.get("picture")))
        .or(profile.avatar_url.take());
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The value is rounded and clamped to the complete u8 percentage range first.
fn parse_usage_window(value: Option<&serde_json::Value>) -> Option<CodexUsageWindow> {
    let value = value?;
    let used = value.get("used_percent")?.as_f64()?;
    let used_percent = used.round().clamp(0.0, 100.0) as u8;
    Some(CodexUsageWindow {
        used_percent,
        remaining_percent: 100_u8.saturating_sub(used_percent),
        window_seconds: value
            .get("limit_window_seconds")
            .and_then(serde_json::Value::as_u64),
        reset_at: value.get("reset_at").and_then(serde_json::Value::as_i64),
    })
}

fn parse_rate_limit(
    value: Option<&serde_json::Value>,
) -> (Option<CodexUsageWindow>, Option<CodexUsageWindow>) {
    match value {
        Some(value) => (
            parse_usage_window(value.get("primary_window")),
            parse_usage_window(value.get("secondary_window")),
        ),
        None => (None, None),
    }
}

fn parse_usage(value: &serde_json::Value) -> Option<CodexUsageSummary> {
    let rate_limit = value.get("rate_limit");
    let (primary_window, secondary_window) = parse_rate_limit(rate_limit);
    let credits = value.get("credits");
    let additional_rate_limits = value
        .get("additional_rate_limits")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = clean_string(item.get("limit_name"))?;
            let (primary_window, secondary_window) = parse_rate_limit(item.get("rate_limit"));
            Some(CodexAdditionalRateLimit {
                name,
                primary_window,
                secondary_window,
            })
        })
        .collect::<Vec<_>>();
    let summary = CodexUsageSummary {
        primary_window,
        secondary_window,
        additional_rate_limits,
        allowed: rate_limit
            .and_then(|limit| limit.get("allowed"))
            .and_then(serde_json::Value::as_bool),
        limit_reached: rate_limit
            .and_then(|limit| limit.get("limit_reached"))
            .and_then(serde_json::Value::as_bool),
        credits_unlimited: credits
            .and_then(|item| item.get("unlimited"))
            .and_then(serde_json::Value::as_bool),
        has_credits: credits
            .and_then(|item| item.get("has_credits"))
            .and_then(serde_json::Value::as_bool),
        overage_limit_reached: credits
            .and_then(|item| item.get("overage_limit_reached"))
            .and_then(serde_json::Value::as_bool),
    };
    (summary.primary_window.is_some()
        || summary.secondary_window.is_some()
        || !summary.additional_rate_limits.is_empty()
        || summary.allowed.is_some()
        || summary.credits_unlimited.is_some())
    .then_some(summary)
}

fn jwt_claims(token: &str) -> Result<Option<serde_json::Value>, ProviderError> {
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Ok(None);
    }
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|error| ProviderError::Authentication(error.to_string()))?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| ProviderError::Authentication(error.to_string()))
}

impl std::fmt::Debug for OAuthTokenSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthTokenSet")
            .field("id_token", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

pub struct DeviceAuthorizationClient {
    config: DeviceAuthorizationConfig,
    issuer: Url,
    client: Client,
}

impl DeviceAuthorizationClient {
    /// Creates a standalone device authorization client.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] for an invalid issuer, client id,
    /// timeout, or HTTP client configuration.
    pub fn new(config: DeviceAuthorizationConfig) -> Result<Self, ProviderError> {
        let issuer = config.validate()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        Ok(Self {
            config,
            issuer,
            client,
        })
    }

    /// Requests the verification URL and one-time user code.
    ///
    /// # Errors
    /// Returns an authentication or transport error without changing auth mode.
    pub async fn start(&self) -> Result<DeviceAuthorization, ProviderError> {
        #[derive(Serialize)]
        struct Request<'a> {
            client_id: &'a str,
        }
        let response = self
            .client
            .post(self.endpoint("api/accounts/deviceauth/usercode")?)
            .json(&Request {
                client_id: &self.config.client_id,
            })
            .send()
            .await
            .map_err(|error| auth_transport_error(&error))?;
        if !response.status().is_success() {
            return Err(auth_status_error(response.status()));
        }
        let response: UserCodeResponse = response
            .json()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        Ok(DeviceAuthorization {
            verification_url: self.endpoint("codex/device")?.to_string(),
            user_code: response.user_code,
            device_auth_id: ProviderCredential::new(response.device_auth_id)?,
            interval: Duration::from_secs(response.interval.max(1)),
        })
    }

    /// Polls for authorization and exchanges the resulting code for subscription
    /// OAuth tokens. It never creates or requests an API-key credential.
    ///
    /// # Errors
    /// Returns an authentication or transport error on denial, timeout, malformed
    /// responses, or token-exchange failure.
    pub async fn complete(
        &self,
        authorization: &DeviceAuthorization,
    ) -> Result<OAuthTokenSet, ProviderError> {
        let started = Instant::now();
        let code = loop {
            let response = self
                .client
                .post(self.endpoint("api/accounts/deviceauth/token")?)
                .json(&TokenPollRequest {
                    device_auth_id: authorization.device_auth_id.expose(),
                    user_code: &authorization.user_code,
                })
                .send()
                .await
                .map_err(|error| auth_transport_error(&error))?;
            if response.status().is_success() {
                break response
                    .json::<AuthorizationCodeResponse>()
                    .await
                    .map_err(|error| ProviderError::Authentication(error.to_string()))?;
            }
            if !matches!(
                response.status(),
                StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            ) {
                return Err(auth_status_error(response.status()));
            }
            if started.elapsed() >= self.config.timeout {
                return Err(ProviderError::Authentication(
                    "device authorization timed out".into(),
                ));
            }
            tokio::time::sleep(
                authorization
                    .interval
                    .min(self.config.timeout.saturating_sub(started.elapsed())),
            )
            .await;
        };

        let redirect_uri = self.endpoint("deviceauth/callback")?.to_string();
        let response = self
            .client
            .post(self.endpoint("oauth/token")?)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.authorization_code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("client_id", self.config.client_id.as_str()),
                ("code_verifier", code.code_verifier.as_str()),
            ])
            .send()
            .await
            .map_err(|error| auth_transport_error(&error))?;
        if !response.status().is_success() {
            return Err(auth_status_error(response.status()));
        }
        let tokens: TokenResponse = response
            .json()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        Ok(OAuthTokenSet {
            id_token: ProviderCredential::new(tokens.id)?,
            access_token: ProviderCredential::new(tokens.access)?,
            refresh_token: ProviderCredential::new(tokens.refresh)?,
        })
    }

    fn endpoint(&self, relative: &str) -> Result<Url, ProviderError> {
        let mut issuer = self.issuer.clone();
        if !issuer.path().ends_with('/') {
            issuer.set_path(&format!("{}/", issuer.path()));
        }
        issuer
            .join(relative)
            .map_err(|error| ProviderError::Configuration(error.to_string()))
    }
}

#[derive(Deserialize)]
struct UserCodeResponse {
    device_auth_id: String,
    #[serde(alias = "user_code", alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "deserialize_interval")]
    interval: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrNumber {
    String(String),
    Number(u64),
}

fn deserialize_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match StringOrNumber::deserialize(deserializer)? {
        StringOrNumber::String(value) => value.trim().parse().map_err(de::Error::custom),
        StringOrNumber::Number(value) => Ok(value),
    }
}

#[derive(Serialize)]
struct TokenPollRequest<'a> {
    device_auth_id: &'a str,
    user_code: &'a str,
}

#[derive(Deserialize)]
struct AuthorizationCodeResponse {
    authorization_code: String,
    #[serde(rename = "code_challenge")]
    _code_challenge: String,
    code_verifier: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(rename = "id_token")]
    id: String,
    #[serde(rename = "access_token")]
    access: String,
    #[serde(rename = "refresh_token")]
    refresh: String,
}

fn auth_transport_error(error: &reqwest::Error) -> ProviderError {
    ProviderError::Authentication(format!("OAuth transport failed: {error}"))
}

fn auth_status_error(status: StatusCode) -> ProviderError {
    ProviderError::Authentication(format!("OAuth endpoint returned HTTP {status}"))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router,
        http::HeaderMap,
        routing::{get, post},
    };
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::*;

    async fn user_code() -> Json<Value> {
        Json(json!({"device_auth_id":"device-secret","user_code":"ABCD-1234","interval":"1"}))
    }

    async fn poll() -> Json<Value> {
        Json(
            json!({"authorization_code":"auth-secret","code_challenge":"challenge","code_verifier":"verifier-secret"}),
        )
    }

    async fn tokens() -> Json<Value> {
        Json(
            json!({"id_token":"id-secret","access_token":"access-secret","refresh_token":"refresh-secret"}),
        )
    }

    #[tokio::test]
    async fn device_flow_exchanges_tokens_without_exposing_secrets() {
        let app = Router::new()
            .route("/api/accounts/deviceauth/usercode", post(user_code))
            .route("/api/accounts/deviceauth/token", post(poll))
            .route("/oauth/token", post(tokens));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server") });
        let client = DeviceAuthorizationClient::new(DeviceAuthorizationConfig {
            issuer: format!("http://{address}"),
            client_id: "fixture-client".into(),
            timeout: Duration::from_secs(2),
        })
        .expect("client");
        let authorization = client.start().await.expect("start");
        assert!(format!("{authorization:?}").contains("[REDACTED]"));
        assert!(!format!("{authorization:?}").contains("ABCD-1234"));
        let tokens = client.complete(&authorization).await.expect("complete");
        let debug = format!("{tokens:?}");
        assert!(!debug.contains("access-secret"));
        assert_eq!(
            tokens.access_token.expose(),
            "access-secret",
            "the adapter can consume the token internally"
        );
    }

    #[test]
    fn non_https_non_loopback_issuer_is_rejected() {
        let result = DeviceAuthorizationClient::new(DeviceAuthorizationConfig {
            issuer: "http://example.com".into(),
            ..DeviceAuthorizationConfig::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn account_routing_claim_is_decoded_without_leaking_tokens() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"account-123"}}"#);
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{header}.{payload}.sig")).expect("id"),
            access_token: ProviderCredential::new(format!("{header}.{payload}.sig"))
                .expect("access"),
            refresh_token: ProviderCredential::new("refresh").expect("refresh"),
        };
        assert_eq!(
            tokens.account_id().expect("claims").as_deref(),
            Some("account-123")
        );
        assert!(!format!("{tokens:?}").contains("account-123"));
    }

    #[test]
    fn account_profile_projects_only_safe_display_claims() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            br#"{"name":"Hendo","preferred_username":"hendo420","email":"hendo@example.com","picture":"https://images.example.com/avatar.png"}"#,
        );
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{header}.{payload}.sig")).expect("id"),
            access_token: ProviderCredential::new("access-secret").expect("access"),
            refresh_token: ProviderCredential::new("refresh-secret").expect("refresh"),
        };
        assert_eq!(
            tokens.account_profile().expect("profile"),
            Some(OAuthAccountProfile {
                display_name: Some("Hendo".into()),
                username: Some("hendo420".into()),
                email: Some("hendo@example.com".into()),
                avatar_url: Some("https://images.example.com/avatar.png".into()),
                ..OAuthAccountProfile::default()
            })
        );
    }

    #[test]
    fn codex_account_and_usage_payloads_project_safe_fields() {
        let mut profile = OAuthAccountProfile::default();
        apply_accounts_check(
            &mut profile,
            &json!({
                "account_ordering": ["account-123"],
                "accounts": [{
                    "id": "account-123",
                    "name": "Hendo",
                    "plan_type": "plus",
                    "structure": "personal",
                    "account_user_role": "owner",
                    "profile_picture_url": "https://images.example.com/codex-avatar.png"
                }]
            }),
            Some("account-123"),
        );
        let usage = parse_usage(&json!({
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 23.6,
                    "limit_window_seconds": 18000,
                    "reset_at": 1_800_000_000
                },
                "secondary_window": {
                    "used_percent": 80,
                    "limit_window_seconds": 604800,
                    "reset_at": 1_800_500_000
                }
            },
            "credits": {"unlimited": false, "has_credits": true}
        }))
        .expect("usage");
        profile.usage = Some(usage);

        assert_eq!(profile.plan_type.as_deref(), Some("plus"));
        assert_eq!(profile.account_structure.as_deref(), Some("personal"));
        assert_eq!(profile.account_role.as_deref(), Some("owner"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://images.example.com/codex-avatar.png")
        );
        assert_eq!(
            profile
                .usage
                .as_ref()
                .and_then(|usage| usage.primary_window.as_ref())
                .map(|window| window.remaining_percent),
            Some(76)
        );
    }

    #[test]
    fn account_profile_rejects_non_https_avatar_claims() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD
            .encode(br#"{"name":"Hendo","picture":"http://example.com/avatar.png"}"#);
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{header}.{payload}.sig")).expect("id"),
            access_token: ProviderCredential::new("access-secret").expect("access"),
            refresh_token: ProviderCredential::new("refresh-secret").expect("refresh"),
        };
        let profile = tokens
            .account_profile()
            .expect("profile")
            .expect("identity");
        assert_eq!(profile.display_name.as_deref(), Some("Hendo"));
        assert_eq!(profile.avatar_url, None);
    }

    #[tokio::test]
    async fn userinfo_supplies_profile_claims_missing_from_id_token() {
        async fn userinfo(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer access-secret")
            );
            Json(json!({
                "name": "Hendo",
                "preferred_username": "hendo420",
                "email": "hendo@example.com",
                "picture": "https://images.example.com/avatar.png"
            }))
        }
        let app = Router::new().route("/api/accounts/oauth/userinfo", get(userinfo));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server") });
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"user-123"}"#);
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new(format!("{header}.{payload}.sig")).expect("id"),
            access_token: ProviderCredential::new("access-secret").expect("access"),
            refresh_token: ProviderCredential::new("refresh-secret").expect("refresh"),
        };

        let profile = tokens
            .fetch_account_profile(&format!("http://{address}"))
            .await
            .expect("userinfo")
            .expect("profile");
        assert_eq!(profile.display_name.as_deref(), Some("Hendo"));
        assert_eq!(profile.username.as_deref(), Some("hendo420"));
        assert_eq!(profile.email.as_deref(), Some("hendo@example.com"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://images.example.com/avatar.png")
        );
        assert!(!format!("{tokens:?}").contains("access-secret"));
    }

    #[tokio::test]
    async fn refresh_rotates_only_returned_token_fields() {
        async fn refresh() -> Json<Value> {
            Json(json!({"access_token":"new-access"}))
        }
        let app = Router::new().route("/oauth/token", post(refresh));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server") });
        let tokens = OAuthTokenSet {
            id_token: ProviderCredential::new("old.id.token").expect("id"),
            access_token: ProviderCredential::new("old-access").expect("access"),
            refresh_token: ProviderCredential::new("old-refresh").expect("refresh"),
        };
        let refreshed = tokens
            .refresh(&DeviceAuthorizationConfig {
                issuer: format!("http://{address}"),
                client_id: "fixture-client".into(),
                timeout: Duration::from_secs(2),
            })
            .await
            .expect("refresh");
        assert_eq!(refreshed.access_token.expose(), "new-access");
        assert_eq!(refreshed.refresh_token.expose(), "old-refresh");
    }
}
