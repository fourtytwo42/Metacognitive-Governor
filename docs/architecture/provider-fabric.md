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
storage will use MCG's operating-system credential-store integration; tokens must
not be written to provider configuration or logs.

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
available. The common conformance probe then runs one buffered and one streaming
Responses request and records supported, unsupported, or indeterminate evidence.
Tool and structured-output quality are not guessed from model prose; later evals
separate protocol support from model reliability.

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
- future configured OpenAI/OpenRouter connections when their credentials are
  deliberately supplied on an owner-controlled machine.

GPU45 currently returns a terminal SSE response even for `stream: false`; the
adapter accepts this documented exception. Live aliases remain dynamically
discovered and are never compiled into MCG.
