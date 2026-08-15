# OpenAI-Compatible Provider Proxy

MCG exposes an authenticated loopback provider surface for external harnesses:

```text
GET  /v1/models
POST /v1/responses
POST /v1/chat/completions
```

The same installation bearer token used by bundled clients is required. The
proxy never creates a second unauthenticated local control path.

## Aliases and routing

`mcg:auto` selects the deterministic qualified route. `mcg:coder` and
`mcg:reasoner` require observed tool and reasoning support. `mcg:cheap` requires
known cost data instead of pretending an unknown-price endpoint is cheapest.
`mcg:local` accepts only loopback, RFC1918, or private-address `nip.io`
connections. The selected physical provider/model and estimated cost provenance
are returned in `mcg_route`, while the public `model` remains the requested MCG
alias.

Initial routing uses the same bounded fallback policy as agent turns. Retriable
transport/rate-limit failures may try a bounded fallback. Authentication,
configuration, and protocol errors never cross to another provider or billing
mode.

## Compatibility behavior

Responses and Chat Completions accept text/message inputs, images, prior caller
function calls/results, tools, named/automatic tool choice, temperature, token
limits, and JSON-schema output requests. Buffered responses normalize output,
tool calls, token usage, cached-token details, and reasoning-token details.
Streaming emits Responses events or Chat Completion chunks; Chat streams finish
with `data: [DONE]`.

## Durable privacy-preserving ledger

Every completed buffered response is committed before it is returned, and every
stream completion is committed before the terminal completion event is emitted.
Migration 24 groups immutable call records into durable local conversations and
retains route/model identifiers, token totals, known/unknown cost, timestamps,
interface, streaming state, response identity, and a SHA-256 request digest. It
does not retain prompts, generated text, credentials, or arbitrary provider
metadata.

Clients may pass `mcg_conversation_id`, `mcg_project_id`, and `mcg_task_id` as
UUID strings inside request `metadata`. MCG strips those reserved keys before
the provider call, validates project/task membership before provider work, and
prevents a conversation identity from being rebound. If no conversation id is
provided, MCG creates one and returns it with the call id in `mcg_ledger`.

Authenticated history is available at
`GET /api/v1/provider-proxy/conversations`, its per-conversation child route,
and `GET /api/v1/provider-proxy/usage`. The desktop Providers workspace shows
the summary and advanced history; `mcg provider usage` and
`mcg provider conversations` provide equivalent CLI access.

All tools supplied by the external request are caller-owned and are returned to
that caller for execution. MCG does not execute or hijack them. Names beginning
with `host::` or `mcg::` are rejected at this proxy boundary until the request is
attached to a governed task context with explicit capabilities and audit.

Managed host/MCG tool execution, project-memory injection, and ensemble aliases
remain later work rather than being represented as complete.
