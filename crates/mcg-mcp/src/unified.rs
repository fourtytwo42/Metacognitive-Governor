use serde_json::Value;

use crate::{
    McpConnection, McpCredential, McpError, McpToolCatalog, McpToolResult, McpTransport,
    NegotiatingHttpMcpClient, StdioMcpClient,
};

pub enum McpClient {
    Http(NegotiatingHttpMcpClient),
    Stdio(StdioMcpClient),
}

impl McpClient {
    /// Creates the configured HTTP or stdio client without executing it.
    ///
    /// # Errors
    /// Returns invalid metadata, transport, or credential-shape failures.
    pub fn new(
        connection: McpConnection,
        credential: Option<McpCredential>,
    ) -> Result<Self, McpError> {
        match connection.transport {
            McpTransport::StreamableHttp { .. } => {
                NegotiatingHttpMcpClient::new(connection, credential).map(Self::Http)
            }
            McpTransport::Stdio { .. } => {
                if credential.is_some() {
                    return Err(McpError::Configuration(
                        "stdio MCP connections cannot receive bearer credentials".into(),
                    ));
                }
                StdioMcpClient::new(connection).map(Self::Stdio)
            }
        }
    }

    /// Discovers one complete deterministic catalog.
    ///
    /// # Errors
    /// Returns transport, negotiation, authentication, remote, or schema failures.
    pub async fn discover(&self) -> Result<McpToolCatalog, McpError> {
        match self {
            Self::Http(client) => client.discover().await,
            Self::Stdio(client) => client.discover().await,
        }
    }

    /// Calls one remote tool through the configured transport and negotiated era.
    ///
    /// # Errors
    /// Returns transport, negotiation, authentication, remote, or schema failures.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        match self {
            Self::Http(client) => client.call_tool(name, arguments).await,
            Self::Stdio(client) => client.call_tool(name, arguments).await,
        }
    }
}
