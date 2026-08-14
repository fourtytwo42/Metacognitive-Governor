use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{TimeDelta, Utc};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    MCP_PROTOCOL_VERSION, McpAuthMode, McpConnection, McpCredential, McpError, McpProtocolMode,
    McpServerInfo, McpToolCatalog, McpToolDescriptor, McpToolResult, McpTransport,
};

const DEFAULT_TTL_MS: i64 = 300_000;
const MAX_TTL_MS: i64 = 86_400_000;

pub struct ModernHttpMcpClient {
    http: reqwest::Client,
    connection: McpConnection,
    credential: Option<McpCredential>,
    next_id: AtomicU64,
}

impl ModernHttpMcpClient {
    /// Creates a current stateless Streamable HTTP client.
    ///
    /// # Errors
    /// Returns configuration errors for invalid metadata, non-HTTP transports,
    /// legacy-only mode, or credential/auth-mode mismatch.
    pub fn new(
        connection: McpConnection,
        credential: Option<McpCredential>,
    ) -> Result<Self, McpError> {
        connection.validate()?;
        if !matches!(connection.transport, McpTransport::StreamableHttp { .. }) {
            return Err(McpError::Configuration(
                "modern HTTP client requires a Streamable HTTP connection".into(),
            ));
        }
        if connection.protocol_mode == McpProtocolMode::Legacy {
            return Err(McpError::Configuration(
                "modern HTTP client cannot use a legacy-only connection".into(),
            ));
        }
        if (connection.auth_mode == McpAuthMode::BearerToken) != credential.is_some() {
            return Err(McpError::Configuration(
                "MCP credential shape does not match the explicit authentication mode".into(),
            ));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| McpError::Transport(error.to_string()))?;
        Ok(Self {
            http,
            connection,
            credential,
            next_id: AtomicU64::new(1),
        })
    }

    /// Discovers current protocol support and returns a deterministic tool snapshot.
    ///
    /// # Errors
    /// Returns transport, authentication, protocol, or schema validation failures.
    pub async fn discover(&self) -> Result<McpToolCatalog, McpError> {
        let discover = self.request("server/discover", None, json!({})).await?;
        let discover: DiscoverResult = serde_json::from_value(discover)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if !discover
            .supported_versions
            .iter()
            .any(|version| version == MCP_PROTOCOL_VERSION)
        {
            return Err(McpError::Negotiation(format!(
                "server does not advertise {MCP_PROTOCOL_VERSION}"
            )));
        }
        let listed = self.request("tools/list", None, json!({})).await?;
        let listed: ListToolsResult = serde_json::from_value(listed)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        let server_info = listed.server_info.unwrap_or(discover.server_info);
        let ttl_ms = listed.ttl_ms.or(discover.ttl_ms).unwrap_or(DEFAULT_TTL_MS);
        if !(0..=MAX_TTL_MS).contains(&ttl_ms) {
            return Err(McpError::InvalidResponse(
                "MCP discovery cache TTL is outside the supported range".into(),
            ));
        }
        let now = Utc::now();
        let mut tools = listed.tools;
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        let catalog = McpToolCatalog {
            connection_id: self.connection.id.clone(),
            protocol_version: MCP_PROTOCOL_VERSION.into(),
            server_info,
            capabilities: discover.capabilities,
            tools,
            discovered_at: now,
            expires_at: now + TimeDelta::milliseconds(ttl_ms),
        };
        catalog.validate()?;
        Ok(catalog)
    }

    /// Calls one discovered remote tool without assigning any local policy authority.
    ///
    /// Runtime callers must first validate the namespaced tool against an active
    /// agent lease and execute through the policy gateway.
    ///
    /// # Errors
    /// Returns transport, authentication, remote JSON-RPC, or response failures.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        if name.trim().is_empty() || !arguments.is_object() {
            return Err(McpError::Configuration(
                "MCP tool name is required and arguments must be an object".into(),
            ));
        }
        let result = self
            .request(
                "tools/call",
                Some(name),
                json!({"name": name, "arguments": arguments}),
            )
            .await?;
        serde_json::from_value(result).map_err(|error| McpError::InvalidResponse(error.to_string()))
    }

    async fn request(
        &self,
        method: &str,
        tool_name: Option<&str>,
        mut params: Value,
    ) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        params["_meta"] = json!({
            "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientCapabilities": {}
        });
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "mcp-protocol-version",
            HeaderValue::from_static(MCP_PROTOCOL_VERSION),
        );
        headers.insert(
            "mcp-method",
            HeaderValue::from_str(method)
                .map_err(|_| McpError::Configuration("invalid MCP method".into()))?,
        );
        if let Some(name) = tool_name {
            let mirrored = mirror_header_value(name);
            headers.insert(
                "mcp-name",
                HeaderValue::from_str(&mirrored)
                    .map_err(|_| McpError::Configuration("invalid MCP tool name".into()))?,
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
        let McpTransport::StreamableHttp { url } = &self.connection.transport else {
            return Err(McpError::Configuration("HTTP transport is required".into()));
        };
        let response = self
            .http
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        let envelope: RpcEnvelope = serde_json::from_value(value)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if envelope.jsonrpc != "2.0" || envelope.id != json!(id) {
            return Err(McpError::InvalidResponse(
                "JSON-RPC version or response id does not match the request".into(),
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
        if !status.is_success() {
            return Err(McpError::Transport(format!(
                "MCP endpoint returned HTTP {status} without a JSON-RPC error"
            )));
        }
        envelope.result.ok_or_else(|| {
            McpError::InvalidResponse("JSON-RPC response has neither result nor error".into())
        })
    }
}

fn mirror_header_value(value: &str) -> String {
    if !value.is_empty()
        && value.trim() == value
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        && !(value.starts_with("=?base64?") && value.ends_with("?="))
    {
        value.to_owned()
    } else {
        format!("=?base64?{}?=", STANDARD.encode(value))
    }
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverResult {
    supported_versions: Vec<String>,
    #[serde(default)]
    capabilities: Value,
    server_info: McpServerInfo,
    #[serde(default)]
    ttl_ms: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListToolsResult {
    tools: Vec<McpToolDescriptor>,
    #[serde(default)]
    server_info: Option<McpServerInfo>,
    #[serde(default)]
    ttl_ms: Option<i64>,
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::{MCP_CONNECTION_CONTRACT_VERSION, McpTransport};

    #[derive(Clone)]
    struct FixtureState {
        expected_token: String,
    }

    async fn fixture(
        State(state): State<FixtureState>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some(state.expected_token.as_str())
        );
        let id = body["id"].clone();
        let method = body["method"].as_str().expect("method");
        assert_eq!(
            headers
                .get("mcp-method")
                .and_then(|value| value.to_str().ok()),
            Some(method)
        );
        assert_eq!(
            body["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        let result = match method {
            "server/discover" => json!({
                "resultType": "complete",
                "supportedVersions": [MCP_PROTOCOL_VERSION],
                "capabilities": {"tools": {}},
                "ttlMs": 5000,
                "serverInfo": {"name": "fixture", "version": "1"}
            }),
            "tools/list" => json!({
                "resultType": "complete",
                "tools": [
                    {"name": "zeta", "inputSchema": {"type": "object"}},
                    {"name": "alpha", "inputSchema": {"type": "object"}}
                ],
                "serverInfo": {"name": "fixture", "version": "1"}
            }),
            "tools/call" => {
                assert_eq!(
                    headers
                        .get("mcp-name")
                        .and_then(|value| value.to_str().ok()),
                    Some("alpha")
                );
                json!({
                    "resultType": "complete",
                    "isError": false,
                    "content": [{"type": "text", "text": "ok"}],
                    "structuredContent": {"ok": true},
                    "serverInfo": {"name": "fixture", "version": "1"}
                })
            }
            _ => unreachable!(),
        };
        Json(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    async fn client() -> ModernHttpMcpClient {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let app = Router::new()
            .route("/mcp", post(fixture))
            .with_state(FixtureState {
                expected_token: "Bearer private-fixture".into(),
            });
        tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
        ModernHttpMcpClient::new(
            McpConnection {
                contract_version: MCP_CONNECTION_CONTRACT_VERSION,
                id: "fixture".into(),
                display_name: "Fixture".into(),
                transport: McpTransport::StreamableHttp {
                    url: format!("http://{address}/mcp"),
                },
                auth_mode: McpAuthMode::BearerToken,
                protocol_mode: McpProtocolMode::Auto,
                enabled: true,
            },
            Some(McpCredential::new("private-fixture").expect("credential")),
        )
        .expect("client")
    }

    #[tokio::test]
    async fn modern_discovery_sorts_tools_and_calls_with_mirrored_headers() {
        let client = client().await;
        let catalog = client.discover().await.expect("discover");
        assert_eq!(
            catalog
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert!(catalog.expires_at > catalog.discovered_at);
        let result = client.call_tool("alpha", json!({})).await.expect("call");
        assert!(!result.is_error);
        assert_eq!(result.structured_content, Some(json!({"ok": true})));
    }
}
