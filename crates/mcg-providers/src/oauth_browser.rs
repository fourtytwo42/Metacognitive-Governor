// Browser authorization behavior is independently integrated around OAuth 2.0
// authorization-code + PKCE. OpenAI Codex's Apache-2.0 loopback implementation
// informed compatibility details; see `THIRD_PARTY_NOTICES.md`.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

use crate::{
    OAuthTokenSet, OPENAI_AUTH_ISSUER, OPENAI_CODEX_PUBLIC_CLIENT_ID, ProviderCredential,
    ProviderError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserAuthorizationConfig {
    pub issuer: String,
    pub client_id: String,
    pub callback_ports: Vec<u16>,
    pub timeout: Duration,
}

impl Default for BrowserAuthorizationConfig {
    fn default() -> Self {
        Self {
            issuer: OPENAI_AUTH_ISSUER.into(),
            client_id: OPENAI_CODEX_PUBLIC_CLIENT_ID.into(),
            callback_ports: vec![1455, 1457],
            timeout: Duration::from_mins(10),
        }
    }
}

pub struct BrowserAuthorization {
    pub authorization_url: String,
    pub callback_port: u16,
    listener: TcpListener,
    state: ProviderCredential,
    code_verifier: ProviderCredential,
}

impl std::fmt::Debug for BrowserAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserAuthorization")
            .field("authorization_url", &"[REDACTED]")
            .field("callback_port", &self.callback_port)
            .field("state", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

pub struct BrowserAuthorizationClient {
    config: BrowserAuthorizationConfig,
    issuer: Url,
    client: Client,
}

impl BrowserAuthorizationClient {
    /// Creates a standalone browser OAuth client.
    ///
    /// # Errors
    /// Returns [`ProviderError::Configuration`] for insecure/invalid issuer,
    /// missing client/ports, zero timeout, or HTTP client failure.
    pub fn new(config: BrowserAuthorizationConfig) -> Result<Self, ProviderError> {
        let issuer = Url::parse(&config.issuer)
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        if issuer.scheme() != "https" && !issuer.host_str().is_some_and(is_loopback_host) {
            return Err(ProviderError::Configuration(
                "OAuth issuer must use HTTPS except in loopback tests".into(),
            ));
        }
        if config.client_id.trim().is_empty()
            || config.callback_ports.is_empty()
            || config.timeout.is_zero()
        {
            return Err(ProviderError::Configuration(
                "OAuth client id, callback ports, and timeout are required".into(),
            ));
        }
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

    /// Binds a loopback callback and builds the browser authorization URL. The
    /// caller owns opening this URL in the platform browser.
    ///
    /// # Errors
    /// Returns a configuration error if no configured loopback port can bind.
    pub async fn start(&self) -> Result<BrowserAuthorization, ProviderError> {
        let listener = bind_callback(&self.config.callback_ports).await?;
        let callback_port = listener
            .local_addr()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?
            .port();
        let state = random_urlsafe(32);
        let code_verifier = random_urlsafe(64);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        let redirect_uri = callback_uri(callback_port);
        let mut url = self.endpoint("oauth/authorize")?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", "openid profile email offline_access")
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("id_token_add_organizations", "true")
            .append_pair("codex_cli_simplified_flow", "true")
            .append_pair("state", &state)
            .append_pair("originator", "mcg");
        Ok(BrowserAuthorization {
            authorization_url: url.to_string(),
            callback_port,
            listener,
            state: ProviderCredential::new(state)?,
            code_verifier: ProviderCredential::new(code_verifier)?,
        })
    }

    /// Waits for a state-validated loopback callback and exchanges its code for
    /// subscription OAuth tokens. No API-key exchange or billing fallback exists.
    ///
    /// # Errors
    /// Returns an authentication error on timeout, invalid callback/state, denial,
    /// transport failure, or malformed token response.
    pub async fn complete(
        &self,
        authorization: BrowserAuthorization,
    ) -> Result<OAuthTokenSet, ProviderError> {
        let (mut socket, _) = timeout(self.config.timeout, authorization.listener.accept())
            .await
            .map_err(|_| ProviderError::Authentication("browser authorization timed out".into()))?
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        let mut request = vec![0_u8; 16 * 1024];
        let read = socket
            .read(&mut request)
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        let callback = parse_callback(&request[..read])?;
        let valid_state: bool = callback
            .state
            .as_bytes()
            .ct_eq(authorization.state.expose().as_bytes())
            .into();
        if !valid_state {
            send_browser_response(&mut socket, 400, "State mismatch").await;
            return Err(ProviderError::Authentication(
                "browser callback state mismatch".into(),
            ));
        }
        if let Some(error) = callback.error {
            send_browser_response(&mut socket, 400, "Sign-in was not completed").await;
            return Err(ProviderError::Authentication(format!(
                "browser authorization was denied: {error}"
            )));
        }
        let code = callback.code.ok_or_else(|| {
            ProviderError::Authentication("browser callback did not contain a code".into())
        })?;
        let redirect_uri = callback_uri(authorization.callback_port);
        let response = self
            .client
            .post(self.endpoint("oauth/token")?)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("client_id", self.config.client_id.as_str()),
                ("code_verifier", authorization.code_verifier.expose()),
            ])
            .send()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        if !response.status().is_success() {
            send_browser_response(&mut socket, 400, "Token exchange failed").await;
            return Err(ProviderError::Authentication(format!(
                "OAuth token endpoint returned HTTP {}",
                response.status()
            )));
        }
        let tokens: TokenResponse = response
            .json()
            .await
            .map_err(|error| ProviderError::Authentication(error.to_string()))?;
        send_browser_response(&mut socket, 200, "Sign-in complete. You can return to MCG.").await;
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
struct TokenResponse {
    #[serde(rename = "id_token")]
    id: String,
    #[serde(rename = "access_token")]
    access: String,
    #[serde(rename = "refresh_token")]
    refresh: String,
}

struct Callback {
    code: Option<String>,
    state: String,
    error: Option<String>,
}

fn parse_callback(request: &[u8]) -> Result<Callback, ProviderError> {
    let request = std::str::from_utf8(request)
        .map_err(|error| ProviderError::Authentication(error.to_string()))?;
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| ProviderError::Authentication("invalid browser callback request".into()))?;
    let url = Url::parse(&format!("http://localhost{target}"))
        .map_err(|error| ProviderError::Authentication(error.to_string()))?;
    if url.path() != "/auth/callback" {
        return Err(ProviderError::Authentication(
            "unexpected browser callback path".into(),
        ));
    }
    let parameters = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let state = parameters
        .get("state")
        .filter(|state| !state.is_empty())
        .cloned()
        .ok_or_else(|| ProviderError::Authentication("browser callback has no state".into()))?;
    Ok(Callback {
        code: parameters.get("code").cloned(),
        state,
        error: parameters.get("error").cloned(),
    })
}

async fn bind_callback(ports: &[u16]) -> Result<TcpListener, ProviderError> {
    for port in ports {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", *port)).await {
            return Ok(listener);
        }
    }
    Err(ProviderError::Configuration(
        "no configured OAuth loopback callback port is available".into(),
    ))
}

async fn send_browser_response(socket: &mut tokio::net::TcpStream, status: u16, body: &str) {
    let reason = if status == StatusCode::OK.as_u16() {
        "OK"
    } else {
        "Bad Request"
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

fn callback_uri(port: u16) -> String {
    format!("http://localhost:{port}/auth/callback")
}

fn random_urlsafe(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::rng().fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, routing::post};
    use serde_json::{Value, json};

    use super::*;

    async fn tokens() -> Json<Value> {
        Json(
            json!({"id_token":"id-secret","access_token":"access-secret","refresh_token":"refresh-secret"}),
        )
    }

    #[tokio::test]
    async fn browser_flow_validates_state_and_exchanges_tokens() {
        let issuer_listener = TcpListener::bind("127.0.0.1:0").await.expect("issuer bind");
        let issuer_address = issuer_listener.local_addr().expect("issuer address");
        tokio::spawn(async move {
            axum::serve(
                issuer_listener,
                Router::new().route("/oauth/token", post(tokens)),
            )
            .await
            .expect("issuer server");
        });
        let callback_probe = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("callback probe");
        let callback_port = callback_probe
            .local_addr()
            .expect("callback address")
            .port();
        drop(callback_probe);
        let client = BrowserAuthorizationClient::new(BrowserAuthorizationConfig {
            issuer: format!("http://{issuer_address}"),
            client_id: "fixture-client".into(),
            callback_ports: vec![callback_port],
            timeout: Duration::from_secs(2),
        })
        .expect("client");
        let authorization = client.start().await.expect("start");
        let url = Url::parse(&authorization.authorization_url).expect("auth URL");
        let state = url
            .query_pairs()
            .find(|(name, _)| name == "state")
            .map(|(_, value)| value.into_owned())
            .expect("state");
        let callback_port = authorization.callback_port;
        let completion = client.complete(authorization);
        let callback = async move {
            let response = reqwest::get(format!(
                "http://127.0.0.1:{callback_port}/auth/callback?code=code-secret&state={state}"
            ))
            .await
            .expect("callback");
            assert_eq!(response.status(), StatusCode::OK);
        };
        let (tokens, ()) = tokio::join!(completion, callback);
        let tokens = tokens.expect("tokens");
        assert_eq!(tokens.access_token.expose(), "access-secret");
        assert!(!format!("{tokens:?}").contains("access-secret"));
    }
}
