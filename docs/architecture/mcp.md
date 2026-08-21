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

Most calls return one `application/json` response. A caller can opt a
`mcg_run_task` request into request-scoped progress by sending a string or number
in `params._meta.progressToken` and accepting `text/event-stream`. That POST then
stays open long enough to emit `notifications/progress` messages followed by the
one final JSON-RPC response. Progress is the committed global event sequence, so
it is strictly increasing, reconciles from SQLite if the in-memory broadcast
lags, and never substitutes an ephemeral counter for durable history.

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
| `mcg_query_organization` | Lists immutable revision history, exports one bounded content-addressed genome, or compares two identity-independent revisions. |
| `mcg_import_organization` | Imports one validated portable genome under caller-owned idempotent authority; conflicting retries fail closed. |
| `mcg_organization_publishers` | Lists exact owner-selected publisher keys, registry scopes, revisions, and terminal trust state. |
| `mcg_organization_publisher_mutate` | Trusts/revises or permanently revokes one publisher under expected-revision and reason checks. |
| `mcg_organization_share_reviews` | Lists durable verified package evidence and pending/imported/rejected review state. |
| `mcg_organization_share_mutate` | Verifies a package for review or separately imports/rejects it with caller-owned retry authority. |
| `mcg_project_context` | Assembles current target-aware context within a caller-selected, server-bounded token budget. |
| `mcg_project_topology_plan` | Reads deterministic snapshot-bound stages derived only from explicit manifest dependencies; cycles return no partial stages. |
| `mcg_reviewed_topology_plans` | Lists reviewed plans for one project or reads one exact immutable plan. |
| `mcg_reviewed_topology_plan_mutate` | Creates, runs, or cancels a separately reviewed topology plan through normal policy, approval, recovery, and idempotency controls. |
| `mcg_memory_search` | Searches current provenance-bearing claims by project, optional namespace, freshness, and bounded result count. |
| `mcg_query_task` | Reads one task with cursor-bounded authoritative events, offset-bounded governed-tool audit, and its consumer run when present. |
| `mcg_query_run` | Reads a complete durable plan, authority, usage, evaluation, and turn-result view. |
| `mcg_scheduler_list` | Lists retained durable schedule definitions and current admission state. |
| `mcg_scheduler_create` | Idempotently creates one bounded schedule from a caller-owned UUID and exact definition. |
| `mcg_scheduler_remove` | Stops future admission for one schedule while retaining its definition and occurrence history. |
| `mcg_run_task` | Idempotently creates, decomposes, executes, independently evaluates, and settles a complete consumer task. |
| `mcg_continue_task` | Retries blocked work or resumes an exactly approved consumer checkpoint without repeating completed work. |
| `mcg_cancel_task` | Atomically cancels a nonterminal task and its consumer run while retaining evidence; exact retries replay. |
| `mcg_record_evaluation` | Validates and records a complete immutable evaluation report; exact evaluation-id retries replay. |
| `mcg_run_agent` | Idempotently creates an active task/running one-work-item run and executes one bounded provider/tool turn. |

`mcg_query_run` accepts optional `replay: true`. The ordinary form returns the
durable run view; replay mode returns the same deterministic digest-bearing report
as REST, CLI, Tauri, and desktop and is observation-only.

The query tools, including scheduler listing, declare read-only, idempotent,
non-destructive, closed-world behavior. Project context does not accept
caller-supplied context items, so an MCP
caller cannot inject material into MCG's canonical project or memory state through
the read surface. `mcg_run_task` and `mcg_run_agent` declare mutating/open-world
behavior and require caller request keys. The complete task form uses the same
consumer organization, decomposition, independent Evaluator, objective verifier,
approval, and terminal-settlement path as the desktop and CLI. The atomic agent
form remains useful when another harness intentionally owns decomposition and
evaluation. Coder and Tool Worker calls through `mcg_run_agent` must explicitly
list every requested tool; omitting the list does not grant the role's full tool set.
Scheduler creation is mutating, idempotent, and open-world because admitted work
can later reach configured providers and governed tools. Scheduler removal is an
idempotent closed-world state transition and never erases evidence.

## Request-scoped task progress

Progress streaming is opt-in and isolated to the task reserved by the exact
`mcg_run_task` request key. Each notification contains only the caller's original
progress token, durable event sequence, and event-kind message; it does not expose
event payloads, goals, prompts, results, task ids, or unrelated task events. The
last SSE message is the ordinary JSON-RPC tool result or error, preserving the
same complete tool contract as JSON response mode. An exact replay can therefore
return a short stream followed by the retained result without repeating provider
or tool authority.

Closing the response body is the MCP cancellation signal for that in-flight
request. Once the durable request identity exists, MCG atomically cancels the
nonterminal task, its consumer run, and active leases while retaining committed
history. Closing after a terminal result is harmless. This stream is scoped to
one request; it is not a session-wide subscription channel and does not add a GET
stream or server session.

## Governed run idempotency

Migration 8 adds an immutable `(interface, request_key)` ledger with a canonical
SHA-256 input hash and unique task/run identities. One SQLite transaction writes:

1. the pending task and `task_created` event;
2. the consumer organization revision;
3. the immutable work plan and planned run;
4. the active task and running run projections/events; and
5. the interface request mapping; and
6. for `mcg_run_task`, the unique task-to-consumer-run launch identity.

Any validation or SQLite failure rolls back all five. Repeating the exact input
returns the same task/run. Reusing a key with different input fails. If the first
HTTP response was lost after a successful turn, retry returns the existing single
instance/result and never calls the provider again. If reservation succeeded but
no authority was dispatched, retry may safely execute; a restart-recovered empty
run first transitions from `recovering` back to `running` through the normal event
boundary.

`mcg_run_agent` deliberately creates one atomic work item. It does not claim that
a model response proves the task complete: the task and run remain active with
their turn evidence available through `mcg_query_run`. `mcg_run_task` is the
complete consumer counterpart. It creates the durable consumer-launch mapping in
the same reservation transaction, executes the Coder and independent Evaluator,
and settles only from objective evidence. Exact request replay is observation-only.
`mcg_continue_task` delegates to the same retry/approval-resume boundary as REST,
and `mcg_cancel_task` atomically cancels the task, run, and active leases. External
evaluation reports remain subject to the same identity, independence, condition
coverage, outcome-consistency, and evidence-policy validation as REST.

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
replay with one result/instance, conflicting-key rejection, ordinary and
deterministic run querying, and
structured status results. A file-backed storage fixture proves atomic reservation,
immutable request-key conflict handling, restart replay, and zero duplicate events.
Unit fixtures cover loopback-Origin classification and encoded `Mcp-Name` handling.
A complete public-surface fixture calls `mcg_run_agent` with
`mcp::governed::lookup`, drives a real two-call OpenAI-compatible model loop,
executes exactly one read-only external MCP call through the broker, returns its
result to the model, and verifies the durable lease alias plus succeeded tool
evidence with no approval record.

A complete MCP consumer lifecycle fixture creates a two-role task, executes both
provider turns, independently verifies a typed file objective, settles task and
run, replays the exact request without duplicate authority, queries retained
events/evaluation, replays immutable evaluation evidence, rejects conflicting
request/evaluation identities, and atomically cancels a second nonterminal task.
The file-backed interface fixture proves the consumer-launch identity and request
mapping survive reopen together.

A request-scoped progress fixture proves string-token validation, SSE content
negotiation (including media-type parameters), monotonic SQLite-backed progress,
unrelated-task isolation, payload omission, a final complete JSON-RPC result,
ordinary JSON replay when no token is supplied, and atomic task/run cancellation
when a client drops a live response stream.

The pinned official `@modelcontextprotocol/client` 2.0.0 fixture runs as a real
Node client against a real loopback MCG server. It proves automatic modern-era
discovery, per-request bearer authentication, server identity/capability parsing,
typed tool listing, a read-only status call, a complete provider-backed consumer
task with SDK progress callbacks, exact replay, invalid-token rejection, and SDK
`AbortSignal` closure reaching durable task/run cancellation. The fixture passes
the bearer only through a child environment variable and asserts that neither
child output stream contains it.

A second file-backed consumer fixture uses a destructive/open-world external MCP
tool in guided mode. It proves the remote call remains at zero before approval,
reopens the daemon while waiting, restores provider/catalog and agent authority,
executes the remote side effect exactly once, and continues the pinned model turn
without repeating the paid pre-approval inference.

This is a broad MCP server plus HTTP/stdio client implementation with official
TypeScript SDK interoperability. MCG's server-side tools are fixed for the life
of a daemon build, and it currently advertises no prompts or resources. It
therefore deliberately does not advertise or open a `subscriptions/listen`
stream: an empty permanent stream would provide no useful catalog-change event.
Add that capability when a server-side tool, prompt, or resource catalog becomes
runtime-mutable. Owner-controlled installed-package UAT now passes on both Windows
and Linux: each packaged daemon negotiates modern MCP discovery, exposes the fixed
typed catalog, and completes authenticated read-only status/tool-list calls through
the adjacent production CLI/service layout. The broader provider-backed lifecycle
and progress/cancellation matrix remains covered by the source integration fixtures
above. Additional SDK/client matrix entries can deepen compatibility evidence; the
remaining release work is final-candidate regression/signing rather than a missing
native MCP transport.
