# Models, Provider Proxy, and MCP

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 17. Model Fabric

MCG should support multiple model providers and local/custom endpoints.

### 17.1 OpenAI Codex / ChatGPT Authentication

Support official Codex/ChatGPT OAuth-based connection where available through supported OpenAI integration mechanisms.

Do not depend on reverse-engineering local credential files.

Supported authentication modes should include:

- ChatGPT sign-in.
- Device-code sign-in where supported.
- OpenAI API key.

### 17.2 OpenRouter

OpenRouter is a first-class provider.

MCG should support:

- Model listing.
- Provider/model selection.
- Streaming.
- Tool calls.
- Usage/cost tracking.
- Capability metadata.

### 17.3 Custom Endpoints

Support arbitrary OpenAI-compatible endpoints.

Examples:

- vLLM.
- llama.cpp server.
- LM Studio.
- Ollama-compatible adapters.
- Self-hosted inference servers.
- Third-party OpenAI-compatible APIs.

### 17.4 Canonical Internal Model Interface

All providers translate into a canonical internal request/response representation.

Canonical request should support:

- Messages.
- Instructions.
- Tools.
- Tool choice.
- Structured output.
- Reasoning configuration.
- Sampling parameters.
- Attachments.
- Metadata.
- Streaming.

### 17.5 Model Registry

MCG maintains a dynamic capability/performance profile for each model.

Example metrics:

- Coding quality.
- Debugging.
- Architecture.
- Research.
- Conversation.
- Tool use.
- Context handling.
- Latency.
- Cost.
- Tokens.
- Historical success by task type.

Model selection should be empirical and context-dependent.

---

---

## 31. Provider Proxy

MCG must be able to act as an OpenAI-compatible provider.

This allows other harnesses to gain MCG's:

- Memory.
- Project context.
- Model routing.
- Meta governance.
- Tool policy.
- Organizational intelligence.

Initial endpoints:

```text
GET  /v1/models
POST /v1/chat/completions
POST /v1/responses
```

Must support:

- Streaming.
- Tool/function calls.
- Structured output where possible.
- Usage accounting.
- Model aliases.

Project/task metadata binds a provider request to the same bounded project
instructions, discovered facts, task requirements, and fresh durable memory
used by MCG agents. The proxy returns a hash-only context receipt and must not
retain the injected content in its usage ledger.

Example aliases:

```text
mcg:auto
mcg:fast
mcg:best
mcg:coder
mcg:reasoner
mcg:local
mcg:cheap
mcg:ensemble
```

`mcg:fast` must be based on locally measured successful-call latency with a
minimum sample floor and deterministic cold-start behavior; model names or
provider marketing are not speed evidence. `mcg:best` must disclose and use
actual discovered/validated capability evidence rather than infer quality from a
name. `mcg:ensemble` is a separate multi-model execution contract and must not be
advertised by `/v1/models` until member selection, synthesis, per-member
accounting, failure policy, tools, restart, and streaming are implemented.
The current implementation satisfies that gate and advertises the alias, while
its `metadata.available` value remains false until at least two distinct
qualified physical provider/model targets are present.
It requires two to three distinct qualified members, one bounded synthesis call,
stable caller identities, exact governed-context reuse, durable phase/member
checkpoints, aggregate accounting, and one final public result. Member tool calls
are proposals only; no caller, host, or MCG tool executes before synthesis selects
the single public continuation. Direct shadow execution is independently opt-in
at both owner-setting and request levels and can never affect the active result.

The migration-28 ensemble engine currently runs behind the unadvertised alias.
It has durable member/synthesis/managed-tool phases, two-of-three partial-failure
settlement, private candidate synthesis, structured-output preservation,
proposal-only member tools, exact per-model evidence, final-only Responses/Chat
streaming, and approval/restart recovery. Direct shadow execution now requires
the owner setting, `metadata.mcg_shadow: true`, and stable conversation/call UUIDs;
it preflights a distinct secondary and retains no secondary content. Managed-tool
turns pin the shadow route and content-free result in their durable continuation,
execute the shadow at most once across approval/restart, and add successful physical
usage only to ledger accounting rather than the public active response. Discovery
remains closed until exact Windows/Linux regression and packaging gates pass.

---

---

## 32. Tool Call Proxy Semantics

Tool namespaces must remain distinct.

Example:

```text
caller::*
host::*
mcg::*
```

### Caller-Owned Tools

Returned to the external harness for execution.

### Host-Owned Tools

Executed by MCG's local execution plane.

### MCG-Owned Tools

Internal cognitive/runtime operations.

MCG must not accidentally hijack caller-owned tool calls.

Ownership prefixes are exact and fail closed. Unqualified tools and
`caller::*` belong to the external harness. Reserved `host::*` and `mcg::*`
turns require caller-owned task, conversation, and call identities so approval,
restart, and response-loss recovery can resume one durable turn rather than
replaying inference or effects. MCG replaces reserved caller schemas with its
own strict canonical definitions. Host operations execute only through normal
project-confined policy, approval, lease, redaction, and audit. MCG operations
are bounded read-only task/project/memory queries with immutable content-free
audit. Internal reserved calls are buffered and never leak through public
streams; only the durably settled final response is returned. Terminal duplicate
call identities fail closed because generated content is deliberately absent
from the privacy-preserving usage ledger.

---

---

## 33. MCP

MCG should operate as both:

- MCP client.
- MCP server.

### MCP Client

Agents may consume external MCP tools.

### MCP Server

External systems may access MCG capabilities such as:

```text
mcg_project_context
mcg_run_agent
mcg_query_project
mcg_memory_search
mcg_system_status
```

---
