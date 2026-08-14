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
            timeout: Duration::from_secs(15 * 60),
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

impl OAuthTokenSet {
    /// Reads the account-routing claim from issuer-provided OAuth tokens.
    /// This decodes claims only; authenticity is established by the direct TLS
    /// token exchange with the configured issuer.
    ///
    /// # Errors
    /// Returns [`ProviderError::Authentication`] when a JWT claim payload is present
    /// but malformed. Access-token claims are preferred for Codex compatibility;
    /// the ID token remains a fallback for issuer variants that place it there.
    pub fn account_id(&self) -> Result<Option<String>, ProviderError> {
        for token in [&self.access_token, &self.id_token] {
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
    use axum::{Json, Router, routing::post};
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
