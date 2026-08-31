# Stage 6 — Governed Providers, Tools, Hooks, and Plugins

**Status:** Root-bound model-egress permit kernel and deterministic in-process governed provider gateway implemented; real provider execution, isolated adapter credentials, and the broader capability gateway remain open

**Product checkpoint:** `0e4c0e8` (`Add Root-bound model egress permits`)

## Authority boundary

Remote model execution is governed disclosure and cost, not authority-free inference. Runtime may propose one bounded invocation, but only Protected Root may validate the exact persisted workflow/context/provider state and issue durable authority. Runtime receives opaque secret-handle identifiers, never raw credentials. A provider adapter may eventually resolve a handle only for the exact active permit, provider connection, credential slot, and expiry.

The schema-59 permit kernel and deterministic gateway do not send network traffic. Existing network provider implementations remain compatibility foundations until their requests execute only through the governed gateway described below.

## Implemented permit kernel

- Stable, bounded `ModelInvocationProposal`, `ModelDisclosureManifest`, `ModelEgressRoute`, `ModelBudget`, `ModelEgressPolicy`, and `ModelInvocationPermit` contracts. Proposal and permit v2 add exact Runtime/lease fencing without silently changing the v1 discriminator.
- Exact binding to invocation, role, workflow attempt, attempt revision, lease generation/owner, authenticated Runtime identity/build/protocol, TaskContract version/hash, immutable request artifact/hash, persisted ContextManifest and disclosed sources, provider, model, destination/class, data classification, token/cost budget, Root policy, issuance, expiry, and opaque handles.
- Root classification of loopback, first-party OpenAI/ChatGPT, and third-party destinations; remote routes require HTTPS and reject embedded credentials, query strings, and fragments.
- Root resolution of the currently persisted provider connection and model catalog before authorization.
- Schema 59 immutable proposal, permit, and secret-handle-binding records with foreign keys, uniqueness, indexes, and update/delete rejection triggers.
- Exact replay is idempotent; proposal, policy, route, provider/model, disclosure, budget, role, task, Runtime identity, workflow revision/generation/owner, time, permit, or handle substitution fails closed. Work is accepted only in `Running`; Verify only in `Verifying`; expired workflow leases fail closed.
- Stored authorizations are canonically revalidated on every load, including reconstructed Root policy, request/disclosure/permit hashes, current provider/model evidence, and secret-handle shape.
- Secret-handle lookup rejects wrong permit/handle pairs, use before issuance, use at or after expiry, and provider authentication-shape substitution.
- `InvocationManifest.egress_permit` is accepted only when the exact Root authorization exists and its invocation, role, attempt, TaskContract, request, context, canonical route payload, resolved provider/model, and active time window match.
- Deterministic/local pre-Stage-6 invocations may continue to use `egress_permit: None`; this compatibility path does not authorize remote governed egress.

## Implemented deterministic gateway checkpoint

- Protected Root exposes one `GovernedModelProviderGateway` boundary that receives the complete secret-free authorization receipt plus the exact request; Runtime never receives raw provider credentials.
- Provider requests use a separately versioned exact JSON wire artifact with SHA-256 domain separation and explicit rejection of non-finite sampling values and payloads above 16 MiB. The canonical contract codec remains strict and unchanged.
- Root recomputes the exact request artifact and checks invocation, permit, Runtime identity, route/model, client request identity, output limit, permit time, and workflow lease immediately before planning and dispatch.
- The immutable `InvocationManifest` and `Planned` `ModelRun` are durable before the run becomes `Dispatched`; the stable client request identity is retained without prompt content.
- Response acceptance revalidates the exact current permit, workflow revision/generation/owner/lease, provider catalog, response contract/model/identity, token usage, and response-size bound.
- Completed runs retain content-addressed response metadata and bounded integer usage only. Raw provider output and arbitrary provider metadata are returned in memory but are not persisted in generic Runtime response storage.
- Provider failures, invalid responses, and results arriving after workflow reassignment or revision change settle the already-dispatched run conservatively as `OutcomeUnknown`; they are never accepted or blindly retried.
- A deterministic fake gateway proves exact one-call dispatch. Request/model substitution is rejected before the gateway is called, and a post-dispatch workflow revision change rejects the response and preserves `OutcomeUnknown` evidence.

## Verified evidence

- `mcg-contracts`: 20/20, including model overscope, substitution, duplicate-handle, invalid-time, and bounded-evidence failures.
- `mcg-storage`: 113/113, including schema-59 authorization, replay, secret opacity, handle expiry, and InvocationManifest permit binding.
- `mcg-protected-root`: 14/14 plus the real-child recovery integration test; the Root permit test covers exact route/disclosure/budget binding, opaque handle resolution, replay, destination substitution, and under-disclosure.
- `cargo check --workspace --all-targets` passes.
- Strict contracts Clippy passes. The new storage path passes with only the repository's recorded legacy lint classes allowed; Root passes with only its recorded legacy oversized IPC-dispatch function allowed.
- Documentation, packaging-contract, formatting, and diff checks pass.

## Next implementation slices

### 6A — Deterministic governed provider gateway — partially complete

- Move the implemented in-process gateway behind a bounded provider-runner request/response protocol carrying the exact request artifact, permit, cancellation identity, and no ambient credentials.
- Resolve opaque handles inside the authorized adapter boundary and zeroize transient credential material.
- Add declared retry/idempotency policy with a distinct invocation/permit/ModelRun for every physical attempt; the storage model intentionally allows one physical ModelRun head per invocation.
- Route recorded parser fixtures through the implemented deterministic path and add cancellation plus invalid/oversized response cases.

### 6B — Real governed provider route

- Route one existing provider through the same gateway without a compatibility bypass.
- Add streaming cancellation, response-size bounds, late/duplicate result fencing, retry fault injection, and observed provider/model fingerprint persistence.
- Run opt-in real-provider tests and a bounded release smoke test without retaining prompts, responses, or credentials in generic logs/evidence.

### 6C — Governed tools and capability manifests

- Define `ToolManifest`, `SkillManifest`, `CapabilityProfile`, and explicit sandbox capability requirements.
- Move process, filesystem, MCP, plugin, hook, browser, computer, Codex app-server, and remote transport execution behind exact Root permits.
- Preserve exact hook trust and invocation-only bypass semantics; neither becomes ambient capability authority.

### 6D — Product surfaces and exit gate

- Simple retains current provider and permission controls.
- Advanced exposes bounded egress/capability/reliability summaries.
- Developer exposes read-only manifests, scopes, permit hashes, resolved identities, sandbox capability, and gateway diagnostics.
- Pass secret-leak, unauthorized-destination, schema-bypass, argument-substitution, trust-escalation, plugin/MCP escalation, backup/log/UI redaction, packaged E2E, native Windows/macOS/Linux UAT, and a real Root → Runtime → governed provider/tools → Verify → CompletionCertificate journey.

Stage 6 is complete only after all four slices and the cross-platform exit gate pass.
