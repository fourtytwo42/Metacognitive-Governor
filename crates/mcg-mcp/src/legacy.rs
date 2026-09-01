use std::time::Duration;

use chrono::{TimeDelta, Utc};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    DEFAULT_MCP_RESPONSE_MAX_BYTES, McpAuthMode, McpConnection, McpCredential, McpError,
    McpProtocolMode, McpServerInfo, McpToolCatalog, McpToolDescriptor, McpToolResult, McpTransport,
    bounded_response_body,
};

pub const LEGACY_MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const DEFAULT_TTL_MS: i64 = 300_000;

pub struct LegacyHttpMcpClient {
    http: reqwest::Client,
    connection: McpConnection,
    credential: Option<McpCredential>,
    max_response_bytes: usize,
}

impl LegacyHttpMcpClient {
    /// Creates an initialize-capable Streamable HTTP client.
    ///
    /// # Errors
    /// Returns invalid transport, protocol mode, or authentication shape.
    pub fn new(
        connection: McpConnection,
        credential: Option<McpCredential>,
    ) -> Result<Self, McpError> {
        connection.validate()?;
        if !matches!(connection.transport, McpTransport::StreamableHttp { .. })
            || connection.protocol_mode != McpProtocolMode::Legacy
        {
            return Err(McpError::Configuration(
                "legacy HTTP client requires a legacy-pinned Streamable HTTP connection".into(),
            ));
        }
        if (connection.auth_mode == McpAuthMode::BearerToken) != credential.is_some() {
            return Err(McpError::Configuration(
                "MCP credential shape does not match the explicit authentication mode".into(),
            ));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_mins(1))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| McpError::Transport(error.to_string()))?;
        Self::new_with_http(connection, credential, http, DEFAULT_MCP_RESPONSE_MAX_BYTES)
    }

    /// Creates a legacy client with caller-supplied transport confinement and response limit.
    ///
    /// # Errors
    /// Returns configuration errors for invalid metadata, transport, auth shape, or byte limit.
    pub fn new_with_http(
        connection: McpConnection,
        credential: Option<McpCredential>,
        http: reqwest::Client,
        max_response_bytes: usize,
    ) -> Result<Self, McpError> {
        connection.validate()?;
        if !matches!(connection.transport, McpTransport::StreamableHttp { .. })
            || connection.protocol_mode != McpProtocolMode::Legacy
            || max_response_bytes == 0
        {
            return Err(McpError::Configuration(
                "legacy HTTP client authority is invalid".into(),
            ));
        }
        if (connection.auth_mode == McpAuthMode::BearerToken) != credential.is_some() {
            return Err(McpError::Configuration(
                "MCP credential shape does not match the explicit authentication mode".into(),
            ));
        }
        Ok(Self {
            http,
            connection,
            credential,
            max_response_bytes,
        })
    }

    /// Initializes a bounded session and discovers one deterministic tool page.
    ///
    /// # Errors
    /// Returns transport, authentication, legacy negotiation, or schema failures.
    pub async fn discover(&self) -> Result<McpToolCatalog, McpError> {
        let initialized = self.initialize().await?;
        let listed = self
            .request(
                2,
                "tools/list",
                json!({}),
                initialized.session_id.as_deref(),
            )
            .await?;
        let listed: LegacyListTools = serde_json::from_value(listed)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        let mut tools = listed.tools;
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        let now = Utc::now();
        let ttl_ms = listed.ttl_ms.unwrap_or(DEFAULT_TTL_MS).clamp(0, 86_400_000);
        let catalog = McpToolCatalog {
            connection_id: self.connection.id.clone(),
            protocol_version: initialized.protocol_version,
            server_info: initialized.server_info,
            capabilities: initialized.capabilities,
            tools,
            discovered_at: now,
            expires_at: now + TimeDelta::milliseconds(ttl_ms),
        };
        catalog.validate()?;
        Ok(catalog)
    }

    /// Initializes a bounded session and calls one legacy tool.
    ///
    /// # Errors
    /// Returns transport, authentication, remote, or schema failures.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        if name.trim().is_empty() || !arguments.is_object() {
            return Err(McpError::Configuration(
                "MCP tool name is required and arguments must be an object".into(),
            ));
        }
        let initialized = self.initialize().await?;
        let result = self
            .request(
                2,
                "tools/call",
                json!({"name": name, "arguments": arguments}),
                initialized.session_id.as_deref(),
            )
            .await?;
        serde_json::from_value(result).map_err(|error| McpError::InvalidResponse(error.to_string()))
    }

    async fn initialize(&self) -> Result<Initialized, McpError> {
        let (result, session_id) = self
            .request_with_session(
                1,
                "initialize",
                json!({
                    "protocolVersion": LEGACY_MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "metacognitive-governor", "version": env!("CARGO_PKG_VERSION")}
                }),
                None,
            )
            .await?;
        let initialized: InitializeResult = serde_json::from_value(result)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if initialized.protocol_version != LEGACY_MCP_PROTOCOL_VERSION {
            return Err(McpError::Negotiation(format!(
                "legacy server selected unsupported version {}",
                initialized.protocol_version
            )));
        }
        self.notification(
            "notifications/initialized",
            json!({}),
            session_id.as_deref(),
        )
        .await?;
        Ok(Initialized {
            session_id,
            protocol_version: initialized.protocol_version,
            capabilities: initialized.capabilities,
            server_info: initialized.server_info,
        })
    }

    async fn request(
        &self,
        id: u64,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, McpError> {
        self.request_with_session(id, method, params, session_id)
            .await
            .map(|(result, _)| result)
    }

    async fn request_with_session(
        &self,
        id: u64,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<(Value, Option<String>), McpError> {
        let response = self
            .http
            .post(self.url()?)
            .headers(self.headers(session_id)?)
            .json(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .send()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        let new_session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let bytes = bounded_response_body(response, self.max_response_bytes).await?;
        let text = String::from_utf8(bytes)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        let value = decode_legacy_body(&content_type, &text)?;
        let envelope: RpcEnvelope = serde_json::from_value(value)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if envelope.jsonrpc != "2.0" || envelope.id != json!(id) {
            return Err(McpError::InvalidResponse(
                "legacy JSON-RPC version or response id mismatch".into(),
            ));
        }
        if let Some(error) = envelope.error {
            return if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                Err(McpError::Authentication(error.message))
            } else {
                Err(McpError::Remote {
                    code: error.code,
                    message: error.message,
                })
            };
        }
        let result = envelope.result.ok_or_else(|| {
            McpError::InvalidResponse("legacy response has neither result nor error".into())
        })?;
        Ok((
            result,
            new_session.or_else(|| session_id.map(str::to_owned)),
        ))
    }

    async fn notification(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<(), McpError> {
        let response = self
            .http
            .post(self.url()?)
            .headers(self.headers(session_id)?)
            .json(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .send()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(McpError::Transport(format!(
                "legacy initialized notification returned HTTP {}",
                response.status()
            )))
        }
    }

    fn headers(&self, session_id: Option<&str>) -> Result<HeaderMap, McpError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        if let Some(session_id) = session_id {
            headers.insert(
                "mcp-session-id",
                HeaderValue::from_str(session_id)
                    .map_err(|_| McpError::InvalidResponse("invalid MCP session id".into()))?,
            );
        }
        if let Some(credential) = &self.credential {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", credential.expose())).map_err(
                    |_| McpError::Authentication("credential is not header-safe".into()),
                )?,
            );
        }
        Ok(headers)
    }

    fn url(&self) -> Result<&str, McpError> {
        match &self.connection.transport {
            McpTransport::StreamableHttp { url } => Ok(url),
            McpTransport::Stdio { .. } => Err(McpError::Configuration(
                "legacy HTTP client requires an HTTP endpoint".into(),
            )),
        }
    }
}

fn decode_legacy_body(content_type: &str, body: &str) -> Result<Value, McpError> {
    if content_type.starts_with("text/event-stream") {
        return body
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .next_back()
            .ok_or_else(|| {
                McpError::InvalidResponse("legacy SSE contained no JSON result".into())
            });
    }
    serde_json::from_str(body).map_err(|error| McpError::InvalidResponse(error.to_string()))
}

struct Initialized {
    session_id: Option<String>,
    protocol_version: String,
    capabilities: Value,
    server_info: McpServerInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    protocol_version: String,
    #[serde(default)]
    capabilities: Value,
    server_info: McpServerInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyListTools {
    tools: Vec<McpToolDescriptor>,
    #[serde(default)]
    ttl_ms: Option<i64>,
}

#[derive(Deserialize)]
struct RpcEnvelope {
    jsonrpc: String,
    id: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::{
        MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode, McpProtocolMode, NegotiatingHttpMcpClient,
    };

    async fn fixture(
        headers: axum::http::HeaderMap,
        Json(body): Json<Value>,
    ) -> axum::response::Response {
        let method = body["method"].as_str().expect("method");
        if method == "server/discover" {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"].clone(),
                    "error": {"code": -32022, "message": "Unsupported protocol version"}
                })),
            )
                .into_response();
        }
        if method == "notifications/initialized" {
            assert_eq!(
                headers
                    .get("mcp-session-id")
                    .and_then(|value| value.to_str().ok()),
                Some("legacy-session")
            );
            return StatusCode::ACCEPTED.into_response();
        }
        let id = body["id"].clone();
        let result = match method {
            "initialize" => json!({
                "protocolVersion": LEGACY_MCP_PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "legacy-fixture", "version": "1"}
            }),
            "tools/list" => {
                assert_eq!(
                    headers
                        .get("mcp-session-id")
                        .and_then(|value| value.to_str().ok()),
                    Some("legacy-session")
                );
                json!({
                    "tools": [
                        {"name": "zeta", "inputSchema": {"type": "object"}},
                        {"name": "alpha", "inputSchema": {"type": "object"}}
                    ]
                })
            }
            "tools/call" => json!({
                "isError": false,
                "content": [{"type": "text", "text": "legacy ok"}],
                "structuredContent": {"ok": true}
            }),
            _ => unreachable!(),
        };
        let mut response =
            Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response();
        if method == "initialize" {
            response.headers_mut().insert(
                "mcp-session-id",
                axum::http::HeaderValue::from_static("legacy-session"),
            );
        }
        response
    }

    async fn client() -> NegotiatingHttpMcpClient {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/mcp", post(fixture)))
                .await
                .expect("serve");
        });
        NegotiatingHttpMcpClient::new(
            McpConnection {
                contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                id: "legacy".into(),
                display_name: "Legacy".into(),
                transport: McpTransport::StreamableHttp {
                    url: format!("http://{address}/mcp"),
                },
                auth_mode: McpAuthMode::None,
                protocol_mode: McpProtocolMode::Auto,
                enabled: true,
            },
            None,
        )
        .expect("client")
    }

    #[tokio::test]
    async fn automatic_http_negotiation_falls_back_to_legacy_session() {
        let client = client().await;
        let catalog = client.discover().await.expect("discover");
        assert_eq!(catalog.protocol_version, LEGACY_MCP_PROTOCOL_VERSION);
        assert_eq!(
            catalog
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        let result = client.call_tool("alpha", json!({})).await.expect("call");
        assert_eq!(result.structured_content, Some(json!({"ok": true})));
    }
}
