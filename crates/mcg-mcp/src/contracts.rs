use std::fmt;

use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::{Host, Url};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const MCP_CONNECTION_CONTRACT_VERSION: u16 = 1;
pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
pub const DEFAULT_MCP_RESPONSE_MAX_BYTES: usize = 4 * 1_024 * 1_024;

pub(crate) async fn bounded_response_body(
    response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, McpError> {
    if maximum_bytes == 0
        || response
            .content_length()
            .is_some_and(|length| length > u64::try_from(maximum_bytes).unwrap_or(u64::MAX))
    {
        return Err(McpError::InvalidResponse(
            "MCP response exceeded its authorized byte limit".into(),
        ));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(McpError::InvalidResponse(
                "MCP response exceeded its authorized byte limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthMode {
    None,
    BearerToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
    StreamableHttp { url: String },
    Stdio { command: String, args: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpProtocolMode {
    Auto,
    Modern20260728,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConnection {
    pub contract_version: u16,
    pub id: String,
    pub display_name: String,
    pub transport: McpTransport,
    pub auth_mode: McpAuthMode,
    pub protocol_mode: McpProtocolMode,
    #[serde(default)]
    pub enabled: bool,
}

impl McpConnection {
    /// Validates secret-free connection metadata before persistence or network use.
    ///
    /// # Errors
    /// Returns a configuration error for invalid identities, insecure public HTTP,
    /// URL credentials/fragments, or unsafe stdio configuration.
    pub fn validate(&self) -> Result<(), McpError> {
        if self.contract_version != MCP_CONNECTION_CONTRACT_VERSION {
            return Err(McpError::Configuration(format!(
                "unsupported MCP connection contract version {}",
                self.contract_version
            )));
        }
        if !valid_identity(&self.id) || self.display_name.trim().is_empty() {
            return Err(McpError::Configuration(
                "MCP connection id must use 1-128 ASCII letters, digits, '-' or '_'; display name is required".into(),
            ));
        }
        match &self.transport {
            McpTransport::StreamableHttp { url } => validate_http_url(url),
            McpTransport::Stdio { command, args } => {
                if command.trim().is_empty()
                    || command.contains('\0')
                    || args.len() > 128
                    || args.iter().any(|argument| argument.contains('\0'))
                {
                    return Err(McpError::Configuration(
                        "MCP stdio command must be non-empty and NUL-free with at most 128 NUL-free arguments".into(),
                    ));
                }
                if self.auth_mode != McpAuthMode::None {
                    return Err(McpError::Configuration(
                        "stdio MCP connections cannot persist or inject bearer authentication"
                            .into(),
                    ));
                }
                Ok(())
            }
        }
    }
}

fn validate_http_url(value: &str) -> Result<(), McpError> {
    let url = Url::parse(value).map_err(|error| McpError::Configuration(error.to_string()))?;
    if url.cannot_be_a_base()
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(McpError::Configuration(
            "MCP endpoint must be an HTTP(S) URL without credentials or a fragment".into(),
        ));
    }
    if url.scheme() == "http" && !is_loopback_host(url.host().as_ref()) {
        return Err(McpError::Configuration(
            "unencrypted MCP HTTP is allowed only for literal loopback or localhost endpoints"
                .into(),
        ));
    }
    Ok(())
}

fn is_loopback_host(host: Option<&Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

pub(crate) fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct McpCredential(String);

impl McpCredential {
    /// Creates a non-empty redacting bearer credential.
    ///
    /// # Errors
    /// Returns a configuration error for an empty value.
    pub fn new(value: impl Into<String>) -> Result<Self, McpError> {
        let mut value = value.into();
        if value.trim().is_empty() {
            value.zeroize();
            return Err(McpError::Configuration(
                "MCP credential cannot be empty".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Exposes the secret only to a host boundary that has already received
    /// explicit credential authority. Callers must never log or return it.
    #[must_use]
    pub fn expose_to_host_executor(&self) -> &str {
        &self.0
    }

    pub(crate) fn expose(&self) -> &str {
        self.expose_to_host_executor()
    }
}

impl fmt::Debug for McpCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpCredential([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub input_schema: Value,
    #[serde(default)]
    pub output_schema: Option<Value>,
    #[serde(default)]
    pub annotations: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCatalog {
    pub connection_id: String,
    pub protocol_version: String,
    pub server_info: McpServerInfo,
    pub capabilities: Value,
    pub tools: Vec<McpToolDescriptor>,
    pub discovered_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl McpToolCatalog {
    /// Validates identity, protocol, cache bounds, and a deterministic unique tool list.
    ///
    /// # Errors
    /// Returns invalid-response when remote discovery evidence is inconsistent.
    pub fn validate(&self) -> Result<(), McpError> {
        if !valid_identity(&self.connection_id)
            || self.protocol_version.trim().is_empty()
            || self.server_info.name.trim().is_empty()
            || self.expires_at < self.discovered_at
        {
            return Err(McpError::InvalidResponse(
                "MCP discovery metadata is inconsistent".into(),
            ));
        }
        let mut previous: Option<&str> = None;
        for tool in &self.tools {
            if tool.name.trim().is_empty()
                || !tool.input_schema.is_object()
                || previous.is_some_and(|name| name >= tool.name.as_str())
            {
                return Err(McpError::InvalidResponse(
                    "MCP tools must have unique names, object schemas, and deterministic order"
                        .into(),
                ));
            }
            previous = Some(&tool.name);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolResult {
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub content: Vec<Value>,
    #[serde(default)]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub server_info: Option<McpServerInfo>,
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error("MCP configuration is invalid: {0}")]
    Configuration(String),
    #[error("MCP authentication failed: {0}")]
    Authentication(String),
    #[error("MCP transport failed: {0}")]
    Transport(String),
    #[error("MCP server rejected the request ({code}): {message}")]
    Remote { code: i64, message: String },
    #[error("MCP response is invalid: {0}")]
    InvalidResponse(String),
    #[error("MCP protocol negotiation failed: {0}")]
    Negotiation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(url: &str) -> McpConnection {
        McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "fixture".into(),
            display_name: "Fixture".into(),
            transport: McpTransport::StreamableHttp { url: url.into() },
            auth_mode: McpAuthMode::None,
            protocol_mode: McpProtocolMode::Auto,
            enabled: true,
        }
    }

    #[test]
    fn connection_rejects_public_plaintext_and_embedded_credentials() {
        assert!(http("http://example.com/mcp").validate().is_err());
        assert!(
            http("https://user:secret@example.com/mcp")
                .validate()
                .is_err()
        );
        assert!(http("http://127.0.0.1:7777/mcp").validate().is_ok());
        assert!(http("https://example.com/mcp").validate().is_ok());
    }

    #[test]
    fn credential_debug_is_redacted() {
        let credential = McpCredential::new("private-token").expect("credential");
        assert_eq!(format!("{credential:?}"), "McpCredential([REDACTED])");
        assert!(!format!("{credential:?}").contains("private-token"));
    }
}
