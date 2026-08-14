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
- The server never returns credential material. The initial tool family is
  read-only and reaches project/memory data only through the runtime boundary.

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

The tool annotations declare read-only, idempotent, non-destructive, closed-world
behavior. Project context does not accept caller-supplied context items, so an MCP
caller cannot inject material into MCG's canonical project or memory state through
this read surface.

## Error behavior

Malformed JSON and invalid JSON-RPC envelopes use standard parse/request errors.
Unknown RPC methods receive HTTP 404 with `-32601`. Unknown tool names and
schema-invalid arguments use `-32602`. Expected tool-domain failures such as an
unknown project return a successful JSON-RPC envelope whose tool result has
`isError: true`, allowing a model client to inspect and correct the call.
Unexpected storage details are not reflected to the client.

## Qualification and remaining scope

The API contract fixture proves authentication, pre-auth Origin rejection,
mirrored-header failure, discovery, stable tool order, project lookup, bounded
context assembly, memory search, and structured status results. Unit fixtures
cover loopback-Origin classification and encoded `Mcp-Name` handling.

This is the MCP server foundation, not the completed MCP feature family. Remaining
M6 work includes:

- the governed mutating `mcg_run_agent` operation with durable task/run evidence;
- MCP client configuration, protected credentials, modern/legacy negotiation,
  tool discovery, namespace isolation, and policy-broker execution by agents;
- request-scoped SSE progress and opt-in subscription streams where useful;
- interoperability fixtures against representative third-party MCP SDKs and
  both-platform user-facing configuration/UAT.
