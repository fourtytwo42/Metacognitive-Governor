# ADR-001 — Cognitive Runtime Process Topology

- **Status:** Accepted for implementation
- **Date:** 2026-08-29
- **Supersedes:** The single-authority-process portions of `docs/decisions/0001-product-engineering-baseline.md` and `docs/architecture/system.md`
- **Source:** Cognitive Runtime Implementation Contract v0.1, Sections 2–5 and 8

## Context

MetaCog currently runs one `mcg-daemon` process containing `AppRuntime`, the SQLite store, workflow recovery, cognition, gateway execution, and ordinary evaluation logic. The new contract requires the writable cognitive runtime to be safe to treat as compromised and requires protected evaluation to remain independent of candidate cognition.

Module boundaries inside one address space cannot enforce those claims.

## Decision

The production package will contain three backend trust-domain processes:

```text
Root process
  |-- Runtime worker process
  `-- Protected Evaluator process
```

### Root process

Root owns:

- the only production `state.db` connection;
- authority and protected policy;
- workflow state, queues, attempts, and leases;
- action intent, authorization, execution-state commit, receipts, reconciliation, and compensation;
- exact model-egress and Tool permits;
- protected secret-handle resolution policy;
- active cognitive configuration;
- verification of evaluator decisions and promotion commit;
- process supervision, shutdown, quarantine, and signed update handoff;
- the authenticated local consumer/API compatibility adapter.

The existing installed `mcg-service` identity and user data paths remain compatible. The Rust package may remain `mcg-daemon` during migration, but its production executable responsibility becomes Protected Root. Packaging may build a target named `mcg-protected-rootd` and install/rename it as `mcg-service`; the artifact manifest must record both build and installed identities.

### Runtime worker

Runtime owns writable cognition:

- state interpretation and mutation proposals;
- context construction and retrieval requests;
- Chat, Work, and Verify invocation planning;
- memory, Claim, Skill, and capability logic;
- runtime telemetry and query-first self-model projections;
- coordination policy;
- ImprovementCases and optimizer candidate proposals.

Runtime receives no production database path, Root/Evaluator credential, ambient provider secret, protected fixture path, or authoritative commit primitive.

### Protected Evaluator

Evaluator owns:

- the only `evaluation.db` connection;
- protected evaluator artifacts and fixtures;
- EvaluationSeed and ExperimentSpec validation;
- pinned baseline/candidate worker launch;
- raw measurements, exposure, taint, portfolio governance, and statistical decisions;
- signed/hash-bound disclosed decision summaries.

Evaluator cannot activate configurations or write Root state. Runtime cannot read Evaluator storage or raw protected results.

### Child execution

Model providers, Codex app-server, Tools, MCP, plugins, hooks, browsers, computer-use processes, shells, and other child processes are subordinate gateways. They receive only exact operation-scoped permits. They are not additional authority owners.

### Client transport

Tauri and privileged CLI operations communicate with Root over the authenticated local IPC defined by ADR-003. The current loopback HTTP API may remain as a Root-owned compatibility and consumer surface. It cannot directly host Runtime or Evaluator authority and cannot bypass the same Root transitions.

Runtime and Evaluator expose no general network listener.

### Supervision

Root launches pinned adjacent Runtime and Evaluator binaries, verifies their expected identities/manifests, applies OS restrictions, tracks process generations, and terminates their descendant trees during shutdown/quarantine. A restarted child receives a new session capability and cannot commit a stale response.

## Consequences

- `AppRuntime` must be decomposed; it cannot remain the production storage owner.
- `mcg-api` becomes a Root-owned adapter over Root application contracts.
- Packaging and updater manifests must bind all three backend binaries.
- Backup/restore must coordinate both authoritative stores.
- Development tests may use in-process fakes, but production-boundary tests must use real processes.
- Existing daemon/API compatibility is preserved while its internal authority model changes.

## Rejected alternatives

- **One monolithic process with Rust modules:** insufficient against a compromised Runtime.
- **Network microservices:** unnecessary exposure and operational complexity for a local-first desktop.
- **Runtime-owned storage with Root approval callbacks:** Runtime could bypass or corrupt authoritative state.
- **Evaluator linked into Runtime or Root:** candidate/runtime compromise or Root implementation coupling would weaken protected judgment.

## Verification

- Runtime cannot open `state.db`, `evaluation.db`, or protected artifacts.
- Evaluator cannot open `state.db` or activate a configuration.
- UI cannot produce a state change without a valid Root session and transition.
- Wrong peer, stale session, replay, malformed frame, and version mismatch fail closed.
- Killing each process independently produces the specified recovery behavior.
