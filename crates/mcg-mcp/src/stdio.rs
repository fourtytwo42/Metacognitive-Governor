use std::{process::Stdio, time::Duration};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

use crate::{
    LEGACY_MCP_PROTOCOL_VERSION, MCP_PROTOCOL_VERSION, McpConnection, McpError, McpProtocolMode,
    McpServerInfo, McpToolCatalog, McpToolDescriptor, McpToolResult, McpTransport,
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_TTL_MS: i64 = 300_000;

pub struct StdioMcpClient {
    connection: McpConnection,
}

impl StdioMcpClient {
    /// Creates a stdio client. Child processes receive a minimal non-secret
    /// environment and are executed directly without a shell.
    ///
    /// # Errors
    /// Returns invalid transport, authentication, or connection metadata.
    pub fn new(connection: McpConnection) -> Result<Self, McpError> {
        connection.validate()?;
        if !matches!(connection.transport, McpTransport::Stdio { .. }) {
            return Err(McpError::Configuration(
                "stdio MCP client requires a stdio connection".into(),
            ));
        }
        Ok(Self { connection })
    }

    /// Spawns a bounded child, negotiates current or legacy on that same process,
    /// discovers one tool page, then terminates the child.
    ///
    /// # Errors
    /// Returns spawn, framing, timeout, negotiation, remote, or schema failures.
    pub async fn discover(&self) -> Result<McpToolCatalog, McpError> {
        let mut process = self.spawn()?;
        let result = process.discover(self.connection.protocol_mode).await;
        process.stop().await;
        let discovered = result?;
        let now = Utc::now();
        let mut tools = discovered.tools;
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        let catalog = McpToolCatalog {
            connection_id: self.connection.id.clone(),
            protocol_version: discovered.protocol_version,
            server_info: discovered.server_info,
            capabilities: discovered.capabilities,
            tools,
            discovered_at: now,
            expires_at: now + TimeDelta::milliseconds(discovered.ttl_ms),
        };
        catalog.validate()?;
        Ok(catalog)
    }

    /// Spawns a bounded child, negotiates on that same process, calls one tool,
    /// and terminates the child.
    ///
    /// # Errors
    /// Returns spawn, framing, timeout, negotiation, remote, or schema failures.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        if name.trim().is_empty() || !arguments.is_object() {
            return Err(McpError::Configuration(
                "MCP tool name is required and arguments must be an object".into(),
            ));
        }
        let mut process = self.spawn()?;
        let result = process
            .call_tool(self.connection.protocol_mode, name, arguments)
            .await;
        process.stop().await;
        result
    }

    /// Spawns one bounded child, negotiates and lists on that same process, revalidates the exact
    /// Root-authorized catalog, calls one tool, and terminates the child.
    ///
    /// # Errors
    /// Returns spawn, framing, timeout, negotiation, catalog-substitution, remote, or schema
    /// failures.
    pub async fn call_tool_against_catalog(
        &self,
        catalog: &McpToolCatalog,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpError> {
        if name.trim().is_empty() || !arguments.is_object() {
            return Err(McpError::Configuration(
                "MCP tool name is required and arguments must be an object".into(),
            ));
        }
        let mut process = self.spawn()?;
        let result = process
            .call_tool_against_catalog(
                &self.connection.id,
                self.connection.protocol_mode,
                catalog,
                name,
                arguments,
            )
            .await;
        process.stop().await;
        result
    }

    fn spawn(&self) -> Result<StdioProcess, McpError> {
        let McpTransport::Stdio { command, args } = &self.connection.transport else {
            return Err(McpError::Configuration(
                "stdio transport is required".into(),
            ));
        };
        let mut command_builder = Command::new(command);
        command_builder
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .env_clear();
        for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "LANG"] {
            if let Some(value) = std::env::var_os(name) {
                command_builder.env(name, value);
            }
        }
        #[cfg(windows)]
        command_builder.as_std_mut().creation_flags(0x0800_0000);
        let mut child = command_builder
            .spawn()
            .map_err(|error| McpError::Transport(format!("cannot spawn MCP server: {error}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("MCP child stdin was unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("MCP child stdout was unavailable".into()))?;
        Ok(StdioProcess {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }
}

struct StdioProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl StdioProcess {
    async fn discover(&mut self, mode: McpProtocolMode) -> Result<Discovered, McpError> {
        let era = self.negotiate(mode).await?;
        let result = self
            .request(
                "tools/list",
                if matches!(era, Era::Modern { .. }) {
                    modern_params(json!({}))
                } else {
                    json!({})
                },
                REQUEST_TIMEOUT,
            )
            .await?;
        let listed: ListTools = serde_json::from_value(result)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        Ok(Discovered {
            protocol_version: era.protocol_version().into(),
            server_info: era.server_info().clone(),
            capabilities: era.capabilities().clone(),
            tools: listed.tools,
            ttl_ms: listed.ttl_ms.unwrap_or(DEFAULT_TTL_MS).clamp(0, 86_400_000),
        })
    }

    async fn call_tool(
        &mut self,
        mode: McpProtocolMode,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpError> {
        let era = self.negotiate(mode).await?;
        let params = json!({"name": name, "arguments": arguments});
        let result = self
            .request(
                "tools/call",
                if matches!(era, Era::Modern { .. }) {
                    modern_params(params)
                } else {
                    params
                },
                REQUEST_TIMEOUT,
            )
            .await?;
        serde_json::from_value(result).map_err(|error| McpError::InvalidResponse(error.to_string()))
    }

    async fn call_tool_against_catalog(
        &mut self,
        connection_id: &str,
        mode: McpProtocolMode,
        catalog: &McpToolCatalog,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpError> {
        let era = self.negotiate(mode).await?;
        let listed = self
            .request(
                "tools/list",
                if matches!(era, Era::Modern { .. }) {
                    modern_params(json!({}))
                } else {
                    json!({})
                },
                REQUEST_TIMEOUT,
            )
            .await?;
        let listed: ListTools = serde_json::from_value(listed)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        catalog.validate_live_snapshot(
            connection_id,
            era.protocol_version(),
            era.server_info(),
            era.capabilities(),
            listed.tools,
            Utc::now(),
        )?;
        if !catalog.tools.iter().any(|tool| tool.name == name) {
            return Err(McpError::InvalidResponse(
                "requested MCP tool is absent from the authorized catalog".into(),
            ));
        }
        let params = json!({"name": name, "arguments": arguments});
        let result = self
            .request(
                "tools/call",
                if matches!(era, Era::Modern { .. }) {
                    modern_params(params)
                } else {
                    params
                },
                REQUEST_TIMEOUT,
            )
            .await?;
        serde_json::from_value(result).map_err(|error| McpError::InvalidResponse(error.to_string()))
    }

    async fn negotiate(&mut self, mode: McpProtocolMode) -> Result<Era, McpError> {
        if mode == McpProtocolMode::Legacy {
            return self.initialize_legacy().await;
        }
        let modern = self
            .request("server/discover", modern_params(json!({})), PROBE_TIMEOUT)
            .await;
        match modern {
            Ok(value) => {
                let discover: Discover = serde_json::from_value(value)
                    .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
                if !discover
                    .supported_versions
                    .iter()
                    .any(|version| version == MCP_PROTOCOL_VERSION)
                {
                    if mode == McpProtocolMode::Modern20260728 {
                        return Err(McpError::Negotiation(
                            "stdio server did not advertise the pinned current protocol".into(),
                        ));
                    }
                    return self.initialize_legacy().await;
                }
                Ok(Era::Modern {
                    server_info: discover.server_info,
                    capabilities: discover.capabilities,
                })
            }
            Err(error) if mode == McpProtocolMode::Auto && fallback_error(&error) => {
                self.initialize_legacy().await
            }
            Err(error) => Err(error),
        }
    }

    async fn initialize_legacy(&mut self) -> Result<Era, McpError> {
        let value = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": LEGACY_MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "metacognitive-governor", "version": env!("CARGO_PKG_VERSION")}
                }),
                REQUEST_TIMEOUT,
            )
            .await?;
        let initialized: Initialize = serde_json::from_value(value)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        if initialized.protocol_version != LEGACY_MCP_PROTOCOL_VERSION {
            return Err(McpError::Negotiation(format!(
                "stdio server selected unsupported legacy version {}",
                initialized.protocol_version
            )));
        }
        self.notification("notifications/initialized", json!({}))
            .await?;
        Ok(Era::Legacy {
            server_info: initialized.server_info,
            capabilities: initialized.capabilities,
        })
    }

    async fn request(
        &mut self,
        method: &str,
        params: Value,
        duration: Duration,
    ) -> Result<Value, McpError> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await?;
        timeout(duration, self.read_response(id))
            .await
            .map_err(|_| McpError::Transport(format!("MCP stdio {method} timed out")))?
    }

    async fn notification(&mut self, method: &str, params: Value) -> Result<(), McpError> {
        self.write(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .await
    }

    async fn write(&mut self, value: &Value) -> Result<(), McpError> {
        let mut bytes = serde_json::to_vec(value)
            .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))
    }

    async fn read_response(&mut self, id: u64) -> Result<Value, McpError> {
        loop {
            let line = read_bounded_line(&mut self.stdout).await?;
            let Some(line) = line else {
                return Err(McpError::Transport("MCP stdio server closed stdout".into()));
            };
            let Ok(value) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            if value.get("id") != Some(&json!(id)) {
                continue;
            }
            let envelope: RpcEnvelope = serde_json::from_value(value)
                .map_err(|error| McpError::InvalidResponse(error.to_string()))?;
            if envelope.jsonrpc != "2.0" {
                return Err(McpError::InvalidResponse(
                    "MCP stdio response jsonrpc must be 2.0".into(),
                ));
            }
            if let Some(error) = envelope.error {
                return Err(McpError::Remote {
                    code: error.code,
                    message: error.message,
                });
            }
            return envelope.result.ok_or_else(|| {
                McpError::InvalidResponse("MCP stdio response has neither result nor error".into())
            });
        }
    }

    async fn stop(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

async fn read_bounded_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, McpError> {
    let mut output = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        if available.is_empty() {
            return Ok((!output.is_empty()).then_some(output));
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if output.len().saturating_add(consumed) > MAX_LINE_BYTES {
            return Err(McpError::InvalidResponse(
                "MCP stdio response line exceeded 4 MiB".into(),
            ));
        }
        output.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if output.last() == Some(&b'\n') {
            while matches!(output.last(), Some(b'\n' | b'\r')) {
                output.pop();
            }
            return Ok(Some(output));
        }
    }
}

fn modern_params(mut params: Value) -> Value {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {
            "name": "metacognitive-governor",
            "version": env!("CARGO_PKG_VERSION")
        }
    });
    params
}

fn fallback_error(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Remote {
            code: -32_022..=-32_020,
            ..
        }
    ) || matches!(error, McpError::Transport(message) if message.ends_with("timed out"))
}

#[derive(PartialEq)]
enum Era {
    Modern {
        server_info: McpServerInfo,
        capabilities: Value,
    },
    Legacy {
        server_info: McpServerInfo,
        capabilities: Value,
    },
}

impl Era {
    const fn protocol_version(&self) -> &'static str {
        match self {
            Self::Modern { .. } => MCP_PROTOCOL_VERSION,
            Self::Legacy { .. } => LEGACY_MCP_PROTOCOL_VERSION,
        }
    }

    const fn server_info(&self) -> &McpServerInfo {
        match self {
            Self::Modern { server_info, .. } | Self::Legacy { server_info, .. } => server_info,
        }
    }

    const fn capabilities(&self) -> &Value {
        match self {
            Self::Modern { capabilities, .. } | Self::Legacy { capabilities, .. } => capabilities,
        }
    }
}

struct Discovered {
    protocol_version: String,
    server_info: McpServerInfo,
    capabilities: Value,
    tools: Vec<McpToolDescriptor>,
    ttl_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Discover {
    supported_versions: Vec<String>,
    #[serde(default)]
    capabilities: Value,
    server_info: McpServerInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initialize {
    protocol_version: String,
    #[serde(default)]
    capabilities: Value,
    server_info: McpServerInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListTools {
    tools: Vec<McpToolDescriptor>,
    #[serde(default)]
    ttl_ms: Option<i64>,
}

#[derive(Deserialize)]
struct RpcEnvelope {
    jsonrpc: String,
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
    use std::io::{BufRead, Write};

    use super::*;
    use crate::{MCP_CONNECTION_CONTRACT_VERSION, McpAuthMode};

    fn connection(server_test: &str, protocol_mode: McpProtocolMode) -> McpConnection {
        McpConnection {
            contract_version: MCP_CONNECTION_CONTRACT_VERSION,
            id: "stdio-fixture".into(),
            display_name: "Stdio fixture".into(),
            transport: McpTransport::Stdio {
                command: std::env::current_exe()
                    .expect("current test executable")
                    .to_string_lossy()
                    .into_owned(),
                args: vec![
                    "--ignored".into(),
                    "--exact".into(),
                    server_test.into(),
                    "--nocapture".into(),
                ],
            },
            auth_mode: McpAuthMode::None,
            protocol_mode,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn modern_stdio_discovers_and_calls() {
        let client = StdioMcpClient::new(connection(
            "stdio::tests::modern_server_fixture",
            McpProtocolMode::Auto,
        ))
        .expect("client");
        let catalog = client.discover().await.expect("discover");
        assert_eq!(catalog.protocol_version, MCP_PROTOCOL_VERSION);
        assert_eq!(catalog.tools[0].name, "echo");

        let result = client
            .call_tool_against_catalog(&catalog, "echo", json!({"message": "hello"}))
            .await
            .expect("catalog-bound call");
        assert_eq!(result.structured_content, Some(json!({"message": "hello"})));

        let mut substituted = catalog;
        substituted.tools[0].description = Some("substituted".into());
        assert!(
            client
                .call_tool_against_catalog(&substituted, "echo", json!({}))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn auto_falls_back_to_legacy_on_the_same_stdio_process() {
        let client = StdioMcpClient::new(connection(
            "stdio::tests::legacy_server_fixture",
            McpProtocolMode::Auto,
        ))
        .expect("client");
        let catalog = client.discover().await.expect("discover");
        assert_eq!(catalog.protocol_version, LEGACY_MCP_PROTOCOL_VERSION);
        assert_eq!(catalog.server_info.name, "legacy-fixture");
        let result = client
            .call_tool_against_catalog(&catalog, "echo", json!({"message": "legacy"}))
            .await
            .expect("legacy catalog-bound call");
        assert_eq!(
            result.structured_content,
            Some(json!({"message": "legacy"}))
        );

        let mut substituted = catalog;
        substituted.server_info.version = Some("substituted".into());
        assert!(
            client
                .call_tool_against_catalog(&substituted, "echo", json!({}))
                .await
                .is_err()
        );
    }

    #[test]
    #[ignore = "spawned as an MCP protocol fixture by the parent test"]
    fn modern_server_fixture() {
        run_server(false);
    }

    #[test]
    #[ignore = "spawned as an MCP protocol fixture by the parent test"]
    fn legacy_server_fixture() {
        run_server(true);
    }

    fn run_server(legacy: bool) {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().lock();
        let mut saw_modern_probe = false;
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let Ok(request) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let Some(id) = request.get("id").cloned() else {
                continue;
            };
            let method = request["method"].as_str().unwrap_or_default();
            let response = match method {
                "server/discover" if legacy => {
                    saw_modern_probe = true;
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32022,"message":"unsupported protocol"}})
                }
                "server/discover" => json!({
                    "jsonrpc":"2.0","id":id,"result":{
                        "supportedVersions":[MCP_PROTOCOL_VERSION],
                        "capabilities":{"tools":{}},
                        "serverInfo":{"name":"modern-fixture","version":"1"}
                    }
                }),
                "initialize" if legacy && saw_modern_probe => json!({
                    "jsonrpc":"2.0","id":id,"result":{
                        "protocolVersion":LEGACY_MCP_PROTOCOL_VERSION,
                        "capabilities":{"tools":{}},
                        "serverInfo":{"name":"legacy-fixture","version":"1"}
                    }
                }),
                "tools/list" => json!({
                    "jsonrpc":"2.0","id":id,"result":{"tools":[{
                        "name":"echo","description":"Echo input",
                        "inputSchema":{"type":"object"}
                    }]}
                }),
                "tools/call" => json!({
                    "jsonrpc":"2.0","id":id,"result":{
                        "content":[],
                        "structuredContent":request["params"]["arguments"].clone()
                    }
                }),
                _ => {
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}})
                }
            };
            writeln!(stdout, "{response}").expect("write fixture response");
            stdout.flush().expect("flush fixture response");
        }
    }
}
