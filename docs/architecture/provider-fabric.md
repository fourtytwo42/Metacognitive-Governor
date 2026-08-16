# Provider Fabric

The M2 provider fabric lives in `mcg-providers` and keeps provider transport,
authentication, routing, and accounting decisions explicit.

## Canonical contract

Contract version 1 normalizes:

- provider connections without embedded secrets;
- model discovery, declared capabilities, context windows, and metadata;
- messages, multimodal content, instructions, tools and tool choice;
- structured output, reasoning, sampling, token limits, and metadata;
- response messages, reasoning, refusals, function calls, usage, and SSE events;
- known prices with provenance and honest unknown-cost values.

Unknown provider fields are retained as provider metadata so a newer endpoint does
not require lossy parsing. Optional request fields are omitted rather than sent as
JSON `null`, which improves compatibility with strict local servers.

## Authentication and billing boundary

| Connection mode | Credential | Backend | Billing |
| --- | --- | --- | --- |
| ChatGPT browser OAuth | OAuth access/refresh tokens | `chatgpt.com/backend-api/codex` | Subscription |
| ChatGPT device code | OAuth access/refresh tokens | `chatgpt.com/backend-api/codex` | Subscription |
| OpenAI API key | User API key | Explicit OpenAI API base URL | Usage based |
| OpenRouter | User API key | Explicit OpenRouter base URL | Usage based |
| Compatible bearer/API key | User credential | User-selected base URL | Provider-defined |
| Compatible anonymous | None | User-selected base URL | Unknown/provider-defined |

OAuth and API-key configurations are separate. An OAuth authentication failure is
not retryable as a model fallback and can never instantiate an API-key connection.
The subscription constructor always selects the ChatGPT Codex backend and attaches
the ID-token account-routing claim when present.

The standalone OAuth implementation does not invoke Codex or read Codex credential
files. Browser login uses a state-validated loopback callback and PKCE. Device login
shows a verification URL and one-time code, then polls until completion. Both return
redacting, non-serializable tokens and support refresh-token rotation. Durable token
storage uses MCG's operating-system credential-store boundary backed by Windows
Credential Manager, Linux Secret Service, or macOS Keychain. API keys, bearer
tokens, and the three OAuth token slots are isolated from SQLite, events, request
debug output, and CLI arguments. Tokens must not be written to provider
configuration or logs.

OAuth login is a dedicated authenticated daemon lifecycle: start creates a bounded,
expiring in-memory session; complete consumes it exactly once and only then stores
credentials plus a fixed subscription connection; cancel drops callback/device-code
state. Pending sessions intentionally do not survive daemon restart. The CLI and
native desktop call the same start/complete/cancel endpoints. Refresh replaces all
token slots transactionally, restores the prior complete set on any partial store
failure, leaves the provider on OAuth billing, and appends only secret-free rotation
evidence to SQLite. A subscription authentication failure enters one per-connection
refresh gate: concurrent failures coalesce to one rotation, then each caller retries
the exact pinned route once. It never selects another provider or API billing.
Disconnect unregisters the live provider, clears every protected credential slot,
atomically removes only the active connection/catalog projections, and retains
immutable usage/audit history. Failed durable removal restores the prior protected
credential set and registry entry. OAuth login never performs an API model request
as a fallback; model discovery is an explicit subsequent operation.

The subscription adapter pins first-party Codex wire compatibility revision
`0.147.0`. Its `/models` request declares that client version and filters only
visible API-supported models. Subscription Responses always use SSE, explicitly
set `store: false`, omit public-API controls rejected by the Codex backend, and
advertise the pinned wire revision separately from MCG's product version. API-key,
OpenRouter, and custom-compatible payloads retain their normal public/provider
semantics. The bounded buffered decoder reconstructs output from completed SSE item
frames when the terminal envelope is sparse.

The native desktop uses the same authenticated configure/list/discover boundary for
OpenAI API-key, OpenRouter, and arbitrary OpenAI-compatible endpoints. Credential
fields are password controls, cross Tauri only in the explicit setup command, and
are zeroized after the daemon stores them through the OS credential service. Simple
mode shows connection, billing class, and catalog size. Advanced mode additionally
shows endpoint/auth metadata, model identifiers, and known input/output price data;
missing price metadata is rendered as unknown rather than estimated.

OpenAI documents ChatGPT subscription and API-key sign-in as distinct login paths
with different billing and administration behavior in its
[Codex authentication guide](https://learn.chatgpt.com/docs/auth). The compatible
SSE implementation follows the official
[Responses streaming guide](https://developers.openai.com/api/docs/guides/streaming-responses).
Compatibility details for the standalone flows and subscription transport are
adapted from OpenAI's Apache-2.0-licensed
[Codex source](https://github.com/openai/codex); see `THIRD_PARTY_NOTICES.md`.

## Discovery and probes

Discovery starts with the endpoint's model list. Provider-native metadata supplies
context length, supported parameters, modalities, and provider-reported prices when
available. Missing metadata remains unknown rather than becoming a false claim.
The explicit conformance probe requires real assistant output from buffered and
streaming Responses requests, a harmless required function-call request, and a
per-request `reasoning.effort=none` control check, then records supported,
unsupported, or indeterminate evidence. An empty terminal envelope does not count
as inference. A successful response that ignores the required call remains indeterminate; tool
authority is not granted from model prose or wishful inference. Tool-using agent
turns disable internal reasoning only after the exact model has positively proven
that control, preserving output budget for complete JSON tool arguments. Unknown
or unsupported models receive no speculative reasoning parameter. Structured-
output reliability remains a separate later eval.

SQLite migration 7 stores only validated connection metadata and the latest
discovered model catalog. Initial discovery must succeed before the connection,
catalog, and two global events commit atomically. A failed setup unregisters the
candidate and clears any newly written credential slots. The runtime registry
accepts only exact registered provider identities and exact discovered model
targets. After daemon restart it lazily reconstructs provider transports from
protected credentials and restores the validated secret-free catalog without a
network call, so pinned work can continue immediately. Explicit authenticated
discovery operations remain the only way to refresh that durable catalog.

## Routing and accounting

Routing filters candidates by required capabilities and optional maximum estimated
cost, then deterministically pins a provider/model for the request. It produces a
bounded fallback list and never falls back for authentication, configuration, or
other non-transient failures. Known-cost candidates sort ahead of unknown-cost
candidates at equal preference. Unknown costs remain selectable only when policy
allows them.

Cost calculation records provenance and separates cached input, uncached input, and
output tokens. Shadow execution is enabled by the debug/test default and disabled by
the release default unless a user opts in.

## Conformance

The common probe runs against:

- an in-memory deterministic provider;
- an HTTP fixture with model discovery, buffered Responses, and SSE;
- GPU45 when `MCG_LIVE_GPU45=1` is set;
- configured OpenAI/OpenRouter connections when their credentials are
  deliberately supplied on an owner-controlled machine.

GPU45 currently returns a terminal SSE response even for `stream: false`; the
adapter accepts this documented exception. On 2026-08-14 its active model passed
buffered and streaming Responses but ignored the required function-call probe, so
tool support remained unknown and it was not eligible for tool-using Coder work.
Live aliases remain dynamically discovered and are never compiled into MCG.

On 2026-08-15 an isolated owner-host ChatGPT device-code connection survived daemon
restart, discovered six visible subscription models with honest unknown prices, and
passed real buffered output, streaming output, required function calling, and
reasoning-control probes with empty daemon stderr. Secure disconnect then removed
the active projection and all matching Windows Credential Manager entries.
