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

`mcg:auto` selects the deterministic qualified route. `mcg:fast` uses the lowest
locally observed p95 end-to-end latency after at least three successful samples
for a provider/model; until any route has that evidence it behaves exactly like
automatic routing, and an unmeasured route never wins merely by claiming to be
fast. `mcg:best` ranks observed-true discovered capabilities first, then context
window, without trusting model names or unknown capability claims. It is an
explicit discovery-evidence heuristic, not an unevidenced correctness claim.
`mcg:coder` and `mcg:reasoner` require observed tool and reasoning support.
`mcg:cheap` requires known cost data instead of pretending an unknown-price
endpoint is cheapest. `mcg:local` accepts only loopback, RFC1918, or
private-address `nip.io` connections. The selected physical provider/model and
estimated cost provenance are returned in `mcg_route`, while the public `model`
remains the requested MCG alias.

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
retains route/model identifiers, token totals, known/unknown cost, bounded
aggregate physical-provider duration, exact content-free per-model call evidence,
timestamps, interface, streaming state, response identity, and a SHA-256 request
digest. Older records without duration/member evidence remain readable and are
excluded from latency routing. `mcg:fast` ranks the individual physical calls,
not a managed turn's aggregate duration. It
does not retain prompts, generated text, credentials, or arbitrary provider
metadata.

Clients may pass `mcg_conversation_id`, `mcg_project_id`, and `mcg_task_id` as
UUID strings inside request `metadata`. MCG strips those reserved keys before
the provider call, validates project/task membership before provider work, and
prevents a conversation identity from being rebound. If no conversation id is
provided, MCG creates one and returns it with the call id in `mcg_ledger`.

When a project or project-bound task is attached, MCG assembles at most 4,096
estimated tokens of current governed context before routing. A task contributes
its goal, constraints, and success conditions; the project contributes target
instructions, discovered facts, and fresh durable memory. The versioned JSON
assembly is injected as an MCG-owned developer message, and the actual provider
request digest is calculated after injection. Responses expose only an
`mcg_context` receipt containing the project/task identities, injected and
omitted item counts, estimated tokens, and SHA-256 of the exact assembly. They
never echo the context through that receipt.

Authenticated history is available at
`GET /api/v1/provider-proxy/conversations`, its per-conversation child route,
and `GET /api/v1/provider-proxy/usage`. The desktop Providers workspace shows
the summary and advanced history; `mcg provider usage` and
`mcg provider conversations` provide equivalent CLI access.

Unqualified tools and names beginning with `caller::` are caller-owned and are
returned to the external harness for execution. MCG never executes or hijacks
them. `host::` and `mcg::` are reserved managed namespaces. Using either requires
an existing governed task and project plus caller-supplied stable conversation
and call UUIDs.

Migration 27 provides the crash-safe continuation contract for managed turns.
A private schema-2 checkpoint binds one caller-owned call id
to its immutable conversation, project, task, request/context digests, selected
route, paid provider responses, governed tool outcomes, and current tool
position. Checkpoints may advance only monotonically, survive daemon restart,
and are removed in the same SQLite transaction that commits the final
content-free usage record. The final record distinguishes completed, failed,
denied, and cancelled outcomes while preserving real token and known-cost totals.

Ownership-qualified names are parsed before routing. `host::name` and
`mcg::name` are reserved to MCG, `caller::name` is explicitly caller-owned, and
unqualified names remain caller-owned for OpenAI compatibility. Empty,
malformed, or unknown `prefix::name` forms fail closed rather than silently
becoming caller tools. A managed request must provide all three stable
UUIDs in metadata: `mcg_task_id`, `mcg_conversation_id`, and `mcg_call_id`.
Caller-only requests may not supply `mcg_call_id`, because ordinary calls do not
promise managed-turn idempotency. All four reserved metadata keys, including
the existing optional project id, are stripped before provider transport.

Caller-owned tools retain the description and JSON Schema supplied by the
caller. Reserved tools do not: MCG replaces every supported `host::` and
`mcg::` definition with its own strict, bounded canonical schema and rejects
unknown reserved names. The runtime also has an internal provider-host execution
boundary that derives a stable tool-request identity from the managed call and
model tool-call ids, resolves only project-relative paths, and delegates through
the same capability, approval, lease, redaction, and audit broker used by agent
turns. Concurrent same-process retries are serialized by call identity. A
pending approval returns HTTP 409 with the stable approval and tool-request
identities; retry after approval or daemon restart resumes the exact checkpoint
without repeating paid inference or completed host effects. Denial and managed
failure settle paid usage once and remove continuation authority. A retry of a
terminal call returns a conflict instead of repeating inference because the
privacy ledger intentionally does not retain generated response content.

The canonical read-only MCG executor currently exposes `mcg::task.get`,
`mcg::project.context`, and `mcg::memory.search`. It validates exact task/project
binding and bounded arguments/results, rejects absolute or escaping targets, and
records content-free decision/completion audit entries. Host and MCG calls may
iterate only within strict model/tool limits; unknown tools, duplicate or empty
tool-call ids, mixed caller/managed response batches, and ownership changes fail
closed.

Managed turns buffer all internal provider/tool interactions. Intermediate
reserved calls never cross the public stream. After durable terminal settlement,
MCG returns the final buffered response or synthesizes compatible Responses API
events / Chat Completion chunks (including `[DONE]`). Approval conflicts are
returned before SSE headers. The ensemble alias uses the separate durable
contract below.

## Ensemble and provider-shadow implementation contract

`mcg:ensemble` is advertised only because all of this contract is implemented:

- A request supplies stable conversation and call UUIDs. The immutable caller
  digest, governed-context receipt, member set, synthesis route, and phase are
  checkpointed before or immediately after every paid response.
- Selection requires at least two and permits at most three distinct qualified
  physical provider/model targets. It prefers provider diversity after normal
  capability/budget filtering and never invents quality from model names.
- Members receive the same post-context request and run concurrently within one
  bounded deadline. At least two successful members are required; otherwise the
  call settles failed with all known usage retained. A failed member is never
  silently replaced after another member has returned.
- Member outputs are private bounded checkpoint material, not ledger content.
  One explicitly selected synthesis route receives a typed, delimited candidate
  bundle and produces the sole public response. It preserves requested structured
  output and may select/rewrite one proposed tool call, but member tool proposals
  never execute or escape directly.
- Caller-owned tools are returned only from the synthesized response. Reserved
  host/MCG tools then enter the existing canonical managed loop; approval and
  restart resume the same synthesis/tool position without repeating members.
- The content-free terminal ledger lists every successful member and synthesis
  call with purpose, physical target, response identity, usage, known/unknown
  cost, and duration. Logical usage/cost/duration must equal the bounded aggregate.
  Restart or completed-call replay cannot create a second member or synthesis
  call. A durable pre-dispatch claim closes automatic replay across the
  upstream-response-before-checkpoint window by settling that physical position
  as indeterminate instead of sending it again.
- Internal member and synthesis events are always buffered. Only the durably
  settled synthesized result is returned, with terminal SSE synthesized for
  Responses or Chat compatibility.

Every physical proxy request receives a deterministic, content-free client trace
identity derived from its logical call UUID, purpose, and ordinal. For first-party
OpenAI API connections, MCG sends the bounded ASCII value as
`X-Client-Request-Id`, which the
[official OpenAI request-debugging contract](https://platform.openai.com/docs/api-reference/debugging-requests)
supports for Responses and Chat requests. OpenRouter, subscription OAuth, local,
and arbitrary compatible transports receive no additional header. The trace value
contains no prompt, output, provider credential, project data, or arbitrary
metadata. It supports reconciliation when the server response is lost; OpenAI
does not document it as a deduplication key, so MCG never relies on the header to
justify automatic replay.

Migration 29 adds the complementary local safety boundary. Immediately before
network I/O, every physical primary, fallback, managed continuation, ensemble
member/synthesis, and shadow request durably claims its logical call, purpose,
ordinal, target, and stable client trace identity in an append-only SQLite
journal. A validated response advances that record once with only its response
identity. Prompt, output, arbitrary metadata, and credentials are never stored.
If the daemon restarts after the claim but before its owning checkpoint or ledger
commit, the existing record is treated as indeterminate and MCG refuses to
resend it. This deliberately prefers an explicit 409/typed failed member or
shadow receipt over a possible duplicate paid call or side effect. It is
at-most-once dispatch protection, not upstream deduplication and not response
replay; content privacy means MCG cannot reconstruct a lost response.

Migration 30 refines terminal classification without weakening that boundary.
Configuration rejection, authentication/rate-limit/HTTP responses, and invalid
provider responses advance the dispatch once to a typed content-free `failed`
state. Only a request/transport error that may have occurred after bytes crossed
the network remains `started` and therefore active or indeterminate. Definite
failures may participate in the same in-process fallback policy; a later retry
still cannot rebind or resend their original physical position.

Direct provider shadowing is separate from ensemble. It requires both the
owner's shadow-experiment setting and `metadata.mcg_shadow: true`, plus explicit
conversation and call UUIDs. Before active inference, routing requires exactly one
available distinct qualified secondary. The secondary is bounded to 120 seconds
and 1 MiB of normalized output; its output and tool calls are discarded, its
failure cannot fail the active result, and successful usage/cost/duration is added
only to content-free physical-call accounting. Failed and timed-out attempts retain
only a typed content-free receipt. Same-process retries are serialized through the
terminal buffered or SSE ledger event and completed IDs are refused without a
second spend. Release defaults remain opt-in; development/test configurations may
enable the owner setting, but real paid calls still require request-level opt-in so
a test binary cannot silently spend against a configured account. For a turn with
MCG-managed reserved tools, the distinct secondary route is pinned into the same
durable checkpoint before tool execution. Its content-free receipt/evidence advances
once, survives approval and daemon restart, and cannot be rebound or replayed by a
retry. The active response keeps active-only usage while the private ledger includes
the successful shadow call. Direct shadow still rejects `mcg:ensemble`, whose member
and synthesis authorities remain separate.

Migration 28 and the private ensemble engine now implement the member/synthesis
checkpoint, fixed two-or-three-member set, provider-diversity preference,
concurrent bounded member calls, private typed candidate bundle, partial-failure
threshold, exact physical accounting, terminal buffering/SSE, caller-tool
proposal boundary, structured-output preservation, conflicting-tool isolation,
canonical managed-tool approval/restart loop, and exact terminal failure
settlement. Direct shadow now implements double opt-in, distinct-secondary
preflight, output/tool isolation, success/failure accounting, replay refusal, and
Responses/Chat streaming. The alias remains omitted from model discovery until
the exact Windows/Linux cross-platform regression and packaging qualification is
green.
