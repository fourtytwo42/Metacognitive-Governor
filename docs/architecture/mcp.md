# MCP boundary

MCG exposes one MCP Streamable HTTP endpoint:

```text
POST /mcp
```

The endpoint implements the stateless MCP `2026-07-28` request model. Each
JSON-RPC request is an independent POST and carries its protocol version and
client capabilities in `params._meta`. There is no `initialize` session, GET
event stream, `Mcp-Session-Id`, or resumable `Last-Event-ID` behavior in this
protocol revision.

The implementation follows the official [MCP discovery
contract](https://modelcontextprotocol.io/specification/2026-07-28/server/discover),
[Streamable HTTP binding](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http),
and [tools contract](https://modelcontextprotocol.io/specification/2026-07-28/server/tools).

## Security boundary

- The daemon still binds only to loopback.
- Every MCP request requires the same installation-local bearer token as the
  REST, SSE, and OpenAI-compatible interfaces.
- An absent `Origin` is accepted for native clients. A present `Origin` must be
  `localhost` or a literal loopback IP; all other origins receive HTTP 403 before
  authentication or request execution. This prevents DNS-rebinding access from
  websites.
- `MCP-Protocol-Version` and `Mcp-Method` are required and must match the JSON
  body. `tools/call` also requires a matching `Mcp-Name`, including the standard
  Base64 sentinel decoding form. A mismatch receives HTTP 400 and MCP error
  `-32020`.
- Only `2026-07-28` is currently accepted. Unsupported versions receive HTTP 400,
  MCP error `-32022`, and the supported-version list.
- The server never returns credential material. Read tools reach project/memory
  data only through the runtime boundary. The one mutation creates bounded
  authority transactionally and executes tools only through the existing policy
  broker.

## Server methods

`server/discover` advertises the exact version, server identity, and tools
capability. `tools/list` is deterministically ordered, privately cacheable for
five minutes, and returns JSON Schema input contracts. `tools/call` returns both
text content and `structuredContent`, with `resultType: "complete"` and server
identity metadata on every successful tool result.

| Tool | Behavior |
| --- | --- |
| `mcg_system_status` | Reads readiness, version, mode, and recovery counts. |
| `mcg_query_project` | Lists current project models or reads one exact project UUID. |
| `mcg_project_context` | Assembles current target-aware context within a caller-selected, server-bounded token budget. |
| `mcg_memory_search` | Searches current provenance-bearing claims by project, optional namespace, freshness, and bounded result count. |
| `mcg_query_run` | Reads a complete durable plan, authority, usage, evaluation, and turn-result view. |
| `mcg_run_agent` | Idempotently creates an active task/running one-work-item run and executes one bounded provider/tool turn. |

The five query tools declare read-only, idempotent, non-destructive, closed-world
behavior. Project context does not accept caller-supplied context items, so an MCP
caller cannot inject material into MCG's canonical project or memory state through
the read surface. `mcg_run_agent` declares its mutating/open-world behavior and
requires a caller request key. Coder and Tool Worker calls must explicitly list
every requested tool; omitting the list does not grant the role's full tool set.

## Governed run idempotency

Migration 8 adds an immutable `(interface, request_key)` ledger with a canonical
SHA-256 input hash and unique task/run identities. One SQLite transaction writes:

1. the pending task and `task_created` event;
2. the consumer organization revision;
3. the immutable work plan and planned run;
4. the active task and running run projections/events; and
5. the interface request mapping.

Any validation or SQLite failure rolls back all five. Repeating the exact input
returns the same task/run. Reusing a key with different input fails. If the first
HTTP response was lost after a successful turn, retry returns the existing single
instance/result and never calls the provider again. If reservation succeeded but
no authority was dispatched, retry may safely execute; a restart-recovered empty
run first transitions from `recovering` back to `running` through the normal event
boundary.

`mcg_run_agent` deliberately creates one atomic work item. It does not claim that
a model response proves the task complete: the task and run remain active with
their turn evidence available through `mcg_query_run`. Broader decomposition,
independent evaluation, completion settlement, cancellation, and approval-resume
operations remain on the versioned REST/CLI surfaces until equivalent MCP
contracts are implemented.

## Error behavior

Malformed JSON and invalid JSON-RPC envelopes use standard parse/request errors.
Unknown RPC methods receive HTTP 404 with `-32601`. Unknown tool names and
schema-invalid arguments use `-32602`. Expected tool-domain failures such as an
unknown project return a successful JSON-RPC envelope whose tool result has
`isError: true`, allowing a model client to inspect and correct the call.
Unexpected storage details are not reflected to the client.

## MCP client foundation

The `mcg-mcp` crate owns secret-free server connection contracts, redacting
credentials, current stateless Streamable HTTP and bounded stdio discovery/call
transports, and normalized tool catalogs/results. Connection ids are stable and bounded. Public
HTTP endpoints are rejected unless they use HTTPS; unencrypted HTTP is limited
to `localhost` or literal loopback addresses. URLs cannot embed credentials or
fragments. Bearer values live under a separate MCP service in the native OS
credential store and never enter SQLite, events, API list responses, or CLI
arguments.

HTTP protocol mode can be automatic, current-only, or legacy-only. Automatic
mode sends current `server/discover` first and falls back only for structured
era/header negotiation failures. The legacy path performs `initialize` for
`2025-11-25`, retains an optional `Mcp-Session-Id` for the bounded operation,
sends `notifications/initialized`, and supports JSON or SSE JSON-RPC responses.
The negotiated era is stored in the catalog; agent execution pins directly to
that era so it cannot silently renegotiate to a different contract after
authority was issued.

Stdio servers are executed directly with piped stdin/stdout, null stderr, no
shell expansion, a minimal inherited environment, a 4 MiB line limit, bounded
request timeouts, and kill-on-drop cleanup. Automatic mode gives
`server/discover` five seconds, then performs legacy initialization on the same
child process only for a timeout or structured unsupported-protocol response.
Current-only and legacy-only pins do not silently cross eras. Each discovery or
tool call owns one child lifecycle, so no process or negotiated authority
survives beyond the bounded operation.

SQLite migration 9 persists connection metadata and the latest deterministic
tool catalog. Configuration follows a discover-before-commit transaction shape:
the protected credential is written, live `server/discover` and `tools/list`
succeed, and then metadata, catalog, and both audit events commit atomically. Any
client creation, negotiation, discovery, or persistence failure removes the
newly written credential and leaves no configured connection. Rediscovery keeps
the prior durable catalog unless a complete replacement succeeds.

Authenticated management endpoints are:

| Endpoint | Behavior |
| --- | --- |
| `GET/POST /api/v1/mcp/connections` | List secret-free connections/catalogs or configure and live-discover one server. |
| `GET /api/v1/mcp/connections/{id}/tools` | Read the latest durable tool catalog. |
| `POST /api/v1/mcp/connections/{id}/discover` | Refresh live discovery and atomically replace the catalog. |

The CLI exposes the same foundation through `mcg mcp add-http`, `add-stdio`,
`list`, `tools`, and `discover`. Stdio arguments after `--` are passed as an
argument vector rather than parsed by a shell.

The native desktop exposes the same authenticated daemon boundary. Simple mode
asks only for a display name and URL and uses automatic current protocol with no
authentication. Advanced mode reveals HTTP/stdio transport, the stable connection
id, protocol pin, optional bearer field, and local executable arguments. Stdio
arguments are entered one per line to keep boundaries explicit. The submitted secret is cleared from React state
after successful configuration and is never returned in the durable connection
list. Connection cards show discovered tool counts and support explicit refresh.

## Agent namespace and policy execution

An agent dispatch explicitly requests a discovered remote identity as
`mcp::CONNECTION_ID::REMOTE_TOOL_NAME`. Empty/default role tool selection never
adds MCP tools. The runtime resolves that canonical identity only against an
enabled durable connection and current catalog, then creates a bounded
provider-safe exposed alias plus an internal `ExternalToolBinding`. The model
sees the alias and JSON Schema but cannot supply or replace the connection id,
remote name, or annotation-derived policy classification.

The agent lease contains the exact exposed alias. A model call must match both
the exposed definitions and active lease permissions. The typed request enters
the existing `ToolBroker` as namespace `external_mcp`, where it receives the
same policy decision, approval persistence, execution lease, immutable audit,
redaction, timeout, and result-identity checks as host tools. Read-only,
non-destructive, closed-world tools can run under explicitly issued capability;
destructive or open-world tools require owner approval. Immediately before a
network call, the executor reloads the enabled connection/catalog and rejects a
changed or missing tool classification, preventing stale authority after
rediscovery.

## Qualification and remaining scope

The API contract fixture proves authentication, pre-auth Origin rejection,
mirrored-header failure, discovery, stable tool order, project lookup, bounded
context assembly, memory search, a real provider-backed governed turn, exact-key
replay with one result/instance, conflicting-key rejection, run querying, and
structured status results. A file-backed storage fixture proves atomic reservation,
immutable request-key conflict handling, restart replay, and zero duplicate events.
Unit fixtures cover loopback-Origin classification and encoded `Mcp-Name` handling.
A complete public-surface fixture calls `mcg_run_agent` with
`mcp::governed::lookup`, drives a real two-call OpenAI-compatible model loop,
executes exactly one read-only external MCP call through the broker, returns its
result to the model, and verifies the durable lease alias plus succeeded tool
evidence with no approval record.

This is the MCP server plus HTTP/stdio client foundation, not the completed MCP feature family. Remaining
M6 work includes:

- expanded MCP run decomposition, independent evaluation, approval-resume,
  completion, and cancellation contracts;
- request-scoped SSE progress and opt-in subscription streams where useful;
- interoperability fixtures against representative third-party MCP SDKs and
  both-platform native desktop UAT.
