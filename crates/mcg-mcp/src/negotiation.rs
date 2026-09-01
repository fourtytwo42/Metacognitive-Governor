use serde_json::Value;

use crate::{
    LegacyHttpMcpClient, MCP_PROTOCOL_VERSION, McpConnection, McpCredential, McpError,
    McpProtocolMode, McpToolCatalog, McpToolResult, ModernHttpMcpClient,
};

pub struct NegotiatingHttpMcpClient {
    connection: McpConnection,
    credential: Option<McpCredential>,
    confined_http: Option<(reqwest::Client, usize)>,
}

impl NegotiatingHttpMcpClient {
    /// Creates an HTTP client that honors strict modern/legacy pins or automatic
    /// current-first fallback.
    ///
    /// # Errors
    /// Returns connection or authentication-shape validation failures.
    pub fn new(
        connection: McpConnection,
        credential: Option<McpCredential>,
    ) -> Result<Self, McpError> {
        connection.validate()?;
        Ok(Self {
            connection,
            credential,
            confined_http: None,
        })
    }

    /// Creates a negotiating client with caller-supplied transport confinement and response limit.
    ///
    /// # Errors
    /// Returns connection, authentication-shape, or byte-limit validation failures.
    pub fn new_with_http(
        connection: McpConnection,
        credential: Option<McpCredential>,
        http: reqwest::Client,
        max_response_bytes: usize,
    ) -> Result<Self, McpError> {
        connection.validate()?;
        if max_response_bytes == 0 {
            return Err(McpError::Configuration(
                "MCP response byte authority is invalid".into(),
            ));
        }
        Ok(Self {
            connection,
            credential,
            confined_http: Some((http, max_response_bytes)),
        })
    }

    /// Discovers tools using the pinned era or current-first automatic negotiation.
    ///
    /// # Errors
    /// Returns transport, authentication, negotiation, remote, or schema failures.
    pub async fn discover(&self) -> Result<McpToolCatalog, McpError> {
        match self.connection.protocol_mode {
            McpProtocolMode::Modern20260728 => self.modern()?.discover().await,
            McpProtocolMode::Legacy => self.legacy()?.discover().await,
            McpProtocolMode::Auto => match self.modern()?.discover().await {
                Ok(catalog) => Ok(catalog),
                Err(error) if should_fallback(&error) => self.legacy()?.discover().await,
                Err(error) => Err(error),
            },
        }
    }

    /// Calls one tool using the pinned era. Automatic mode performs current-first
    /// discovery and uses the resulting era for this call.
    ///
    /// # Errors
    /// Returns transport, authentication, negotiation, remote, or schema failures.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        match self.connection.protocol_mode {
            McpProtocolMode::Modern20260728 => self.modern()?.call_tool(name, arguments).await,
            McpProtocolMode::Legacy => self.legacy()?.call_tool(name, arguments).await,
            McpProtocolMode::Auto => {
                let catalog = self.discover().await?;
                if catalog.protocol_version == MCP_PROTOCOL_VERSION {
                    self.modern()?.call_tool(name, arguments).await
                } else {
                    self.legacy()?.call_tool(name, arguments).await
                }
            }
        }
    }

    fn modern(&self) -> Result<ModernHttpMcpClient, McpError> {
        let mut connection = self.connection.clone();
        connection.protocol_mode = McpProtocolMode::Modern20260728;
        if let Some((http, maximum)) = &self.confined_http {
            ModernHttpMcpClient::new_with_http(
                connection,
                self.credential.clone(),
                http.clone(),
                *maximum,
            )
        } else {
            ModernHttpMcpClient::new(connection, self.credential.clone())
        }
    }

    fn legacy(&self) -> Result<LegacyHttpMcpClient, McpError> {
        let mut connection = self.connection.clone();
        connection.protocol_mode = McpProtocolMode::Legacy;
        if let Some((http, maximum)) = &self.confined_http {
            LegacyHttpMcpClient::new_with_http(
                connection,
                self.credential.clone(),
                http.clone(),
                *maximum,
            )
        } else {
            LegacyHttpMcpClient::new(connection, self.credential.clone())
        }
    }
}

fn should_fallback(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Remote {
            code: -32_022..=-32_020,
            ..
        } | McpError::Negotiation(_)
    )
}
