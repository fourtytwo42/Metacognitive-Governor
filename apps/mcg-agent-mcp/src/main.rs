//! Role-scoped stdio MCP bridge for MCG's internal Chat/Coordinator/Coder sessions.

use std::{env, path::PathBuf};

use chrono::{DateTime, Duration, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Deserialize, Zeroize, ZeroizeOnDrop)]
struct BootstrapDocument {
    #[zeroize(skip)]
    schema_version: u16,
    #[zeroize(skip)]
    endpoint: String,
    bootstrap_token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Claims {
    role: String,
    allowed_tools: Vec<String>,
}

#[derive(Debug, Deserialize, Zeroize, ZeroizeOnDrop)]
struct AccessCredential {
    bearer: String,
    #[zeroize(skip)]
    expires_at: DateTime<Utc>,
    #[zeroize(skip)]
    claims: Claims,
}

#[derive(Debug, Serialize)]
struct RenewRequest<'a> {
    bearer: &'a str,
}

#[derive(Debug, Serialize)]
struct ToolCallRequest<'a> {
    bearer: &'a str,
    tool_name: &'a str,
    arguments: Value,
}

fn bootstrap_path() -> Result<PathBuf, String> {
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--bootstrap-file" {
            return arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "--bootstrap-file requires a path".to_owned());
        }
    }
    Err("mcg-agent-mcp requires --bootstrap-file".into())
}

async fn exchange_bootstrap(
    client: &reqwest::Client,
    document: &BootstrapDocument,
) -> Result<AccessCredential, String> {
    if document.schema_version != 1 || !document.endpoint.starts_with("http://127.0.0.1:") {
        return Err("bootstrap endpoint is not a supported loopback MCG endpoint".into());
    }
    client
        .post(format!(
            "{}/bootstrap",
            document.endpoint.trim_end_matches('/')
        ))
        .json(&json!({"bootstrap_token": document.bootstrap_token}))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

async fn renew(
    client: &reqwest::Client,
    endpoint: &str,
    access: &AccessCredential,
) -> Result<AccessCredential, String> {
    client
        .post(format!("{}/renew", endpoint.trim_end_matches('/')))
        .json(&RenewRequest {
            bearer: &access.bearer,
        })
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

fn tool_schema(name: &str) -> Value {
    let description = match name {
        "delegate_code" => "Delegate a bounded coding brief to an isolated MCG Coding Agent.",
        "message_coder" => "Answer or amend an active Coding Agent delegation.",
        "settle_delegation" => {
            "Accept or block a reported Coding Agent result after Coordinator review."
        }
        "message_coordinator" => {
            "Send a request, question, update, or result to the Coordinator Agent's backstage room."
        }
        "message_chat" => {
            "Send a concise coordination update to the Chat Agent; only Chat may address the owner."
        }
        "search_shared_context" => {
            "Search attributed context within this conversation and project only."
        }
        "read_blackboard" => "Read this conversation's append-only shared blackboard.",
        "write_blackboard" => "Append a typed entry to this conversation's shared blackboard.",
        "cancel_delegation" => "Cancel one active delegation belonging to this conversation.",
        "report_result" => {
            "Required final step for a Coding Agent. Call exactly once after verification to report structured results to the Coordinator Agent."
        }
        _ => "MCG internal role-scoped tool.",
    };
    let input_schema = match name {
        "delegate_code" => json!({
            "type":"object",
            "properties":{
                "objective":{"type":"string","minLength":1},
                "scope":{"type":"array","items":{"type":"string"}},
                "constraints":{"type":"array","items":{"type":"string"}},
                "success_conditions":{"type":"array","items":{"type":"string"}},
                "context_references":{"type":"array","items":{"type":"string"}}
            },
            "required":["objective","scope","constraints","success_conditions","context_references"],
            "additionalProperties":false
        }),
        "message_coordinator" => json!({
            "type":"object",
            "properties":{
                "content":{"type":"string","minLength":1},
                "question":{"type":"boolean"},
                "delegation_id":{"type":"string"}
            },
            "required":["content"],
            "additionalProperties":false
        }),
        "message_chat" => json!({
            "type":"object",
            "properties":{
                "content":{"type":"string","minLength":1},
                "delegation_id":{"type":"string"}
            },
            "required":["content"],
            "additionalProperties":false
        }),
        "message_coder" => json!({
            "type":"object",
            "properties":{
                "delegation_id":{"type":"string"},
                "content":{"type":"string","minLength":1}
            },
            "required":["delegation_id","content"],
            "additionalProperties":false
        }),
        "settle_delegation" => json!({
            "type":"object",
            "properties":{
                "delegation_id":{"type":"string"},
                "expected_revision":{"type":"integer","minimum":1},
                "decision":{"type":"string","enum":["accept","block"]},
                "rationale":{"type":"string","minLength":1}
            },
            "required":["delegation_id","expected_revision","decision","rationale"],
            "additionalProperties":false
        }),
        "search_shared_context" => json!({
            "type":"object",
            "properties":{
                "query":{"type":"string","minLength":1},
                "limit":{"type":"integer","minimum":1,"maximum":100}
            },
            "required":["query"],
            "additionalProperties":false
        }),
        "read_blackboard" => json!({"type":"object","properties":{},"additionalProperties":false}),
        "write_blackboard" => json!({
            "type":"object",
            "properties":{
                "kind":{"type":"string","enum":["goal","context","decision","status","evidence","question","answer","result","blocker"]},
                "content":{"type":"string","minLength":1},
                "delegation_id":{"type":"string"}
            },
            "required":["kind","content"],
            "additionalProperties":false
        }),
        "cancel_delegation" => json!({
            "type":"object",
            "properties":{
                "delegation_id":{"type":"string"},
                "expected_revision":{"type":"integer","minimum":1}
            },
            "required":["delegation_id","expected_revision"],
            "additionalProperties":false
        }),
        "report_result" => json!({
            "type":"object",
            "properties":{
                "summary":{"type":"string","minLength":1},
                "changed_files":{"type":"array","items":{"type":"string"}},
                "checks":{"type":"array","items":{"type":"string"}},
                "evidence":{"type":"array","items":{"type":"string"}},
                "unresolved_issues":{"type":"array","items":{"type":"string"}},
                "integration_ready":{"type":"boolean"}
            },
            "required":[
                "summary",
                "changed_files",
                "checks",
                "evidence",
                "unresolved_issues",
                "integration_ready"
            ],
            "additionalProperties":false
        }),
        _ => json!({"type":"object","properties":{},"additionalProperties":false}),
    };
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

async fn call_tool(
    client: &reqwest::Client,
    endpoint: &str,
    access: &AccessCredential,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, String> {
    let response = client
        .post(format!("{}/call", endpoint.trim_end_matches('/')))
        .json(&ToolCallRequest {
            bearer: &access.bearer,
            tool_name,
            arguments,
        })
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err("agent credential expired or was revoked".into());
    }
    response
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = bootstrap_path().map_err(io::Error::other)?;
    let bytes = tokio::fs::read(&path).await?;
    tokio::fs::remove_file(&path).await?;
    let mut document: BootstrapDocument = serde_json::from_slice(&bytes)?;
    let endpoint = document.endpoint.clone();
    let client = reqwest::Client::builder().no_proxy().build()?;
    let mut access = exchange_bootstrap(&client, &document)
        .await
        .map_err(io::Error::other)?;
    document.zeroize();

    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = io::stdout();
    while let Some(line) = lines.next_line().await? {
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                stdout
                    .write_all(
                        format!(
                            "{}\n",
                            json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":error.to_string()}})
                        )
                        .as_bytes(),
                    )
                    .await?;
                stdout.flush().await?;
                continue;
            }
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let response = match method {
            "initialize" => json!({
                "jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion":"2025-06-18",
                    "capabilities":{"tools":{"listChanged":false}},
                    "serverInfo":{"name":"mcg-agent-mcp","version":env!("CARGO_PKG_VERSION")},
                    "instructions":format!("Server-authorized internal tools for the MCG {} role.", access.claims.role)
                }
            }),
            "tools/list" => json!({
                "jsonrpc":"2.0","id":id,"result":{
                    "tools":access.claims.allowed_tools.iter().map(|name| tool_schema(name)).collect::<Vec<_>>()
                }
            }),
            "tools/call" => {
                if access.expires_at - Utc::now() < Duration::seconds(30) {
                    access = renew(&client, &endpoint, &access)
                        .await
                        .map_err(io::Error::other)?;
                }
                let params = request.get("params").cloned().unwrap_or(Value::Null);
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match call_tool(&client, &endpoint, &access, name, arguments).await {
                    Ok(result) => json!({
                        "jsonrpc":"2.0","id":id,"result":{
                            "content":[{"type":"text","text":serde_json::to_string(&result)?}],
                            "structuredContent":result,
                            "isError":false
                        }
                    }),
                    Err(error) => json!({
                        "jsonrpc":"2.0","id":id,"result":{
                            "content":[{"type":"text","text":error}],"isError":true
                        }
                    }),
                }
            }
            _ => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}})
            }
        };
        stdout.write_all(format!("{response}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}
