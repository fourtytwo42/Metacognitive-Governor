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
