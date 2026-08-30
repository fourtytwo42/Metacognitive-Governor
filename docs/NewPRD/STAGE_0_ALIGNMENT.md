# Stage 0 Current-to-Target Alignment

**Status:** Provisional inventory while the concurrent parity release finishes
**Last inspected:** 2026-08-29
**Baseline:** Must be replaced with the clean handoff commit before Stage 0 closes

## 1. Purpose

This document maps the current MetaCog implementation to the target trust and runtime architecture defined by the new PRDs. It is an implementation aid, not a claim that the target boundaries already exist.

## 2. Current Physical Topology

The current product is approximately:

```text
Tauri / TypeScript UI
        |
        | Tauri commands + authenticated loopback HTTP
        v
mcg-daemon
        |
        v
AppRuntime
  |-- opens and directly mutates SqliteEventStore
  |-- hosts workflow/recovery logic
  |-- launches model/Codex/provider work
  |-- launches tools/plugins/agents
  |-- records ordinary evaluations/experiments
  `-- exposes the mcg-api router
```

Verified observations:

- `apps/mcg-daemon/src/main.rs` opens `AppRuntime` and serves `mcg-api` on a socket address.
- `AppRuntime::open` in `crates/mcg-runtime/src/lib.rs` opens `SqliteEventStore`, performs migrations/recovery, creates credentials and execution managers, and owns the resulting store.
- `apps/desktop/src-tauri/src/lib.rs` defaults to the authenticated loopback service at `http://127.0.0.1:7777`.
- `mcg-api` exposes conversation, task, agent, approval, provider, tool, Codex, experiment, and configuration mutations against the shared `AppRuntime`.
- Current experiments and promotion-shaped records live in ordinary production crates/storage and do not yet have a physically protected evaluator process/database.
- Current storage already contains extensive durable events, recovery, leases, actions, agents, approvals, plugins, schedules, platform operations, and backups that should be reused or migrated rather than discarded.

## 3. Target Physical Topology

```text
Tauri / CLI
    |
    | authenticated versioned local IPC
    v
Protected Root
  |-- sole state.db writer
  |-- authority and policy
  |-- workflow/lease ownership
  |-- action intent/authorization/commit/reconciliation
  |-- model-egress and tool permits
  |-- configuration activation and promotion
  |
  |-- typed IPC --> Runtime worker
  |                  |-- state interpretation
  |                  |-- context and memory logic
  |                  |-- Chat/Work/Verify cognition
  |                  |-- writable optimizer proposals
  |                  `-- no authoritative DB handle
  |
  `-- protected IPC --> Protected Evaluator
                         |-- sole evaluation.db writer
                         |-- protected fixtures/artifacts
                         |-- baseline/candidate execution
                         |-- exposure/taint/metrics
                         `-- signed decision summaries
```

Provider, Codex, MCP, hook, plugin, browser, computer, process, and filesystem execution remains subordinate to exact Root-bound permits.

## 4. Current-to-Target Responsibility Matrix

| Current component | Current responsibility | Target disposition | Principal stage |
|---|---|---|---|
| `mcg-domain` | Shared domain IDs and models | Retain compatibility name; narrow toward stable cross-boundary cognitive contracts and versioned envelopes | 1 |
| `mcg-storage` | Shared SQLite schema and direct mutation API | Retain storage implementation but make it callable only by Root in production; split protected evaluator storage | 2, 7 |
| `mcg-runtime` | Monolithic state owner, recovery, cognition, gateways, tools | Decompose authority/storage into Root; retain writable cognition and orchestration behind typed IPC | 2–6 |
| `mcg-api` | Loopback HTTP API directly backed by `AppRuntime` | Become a Root-owned compatibility/consumer adapter; no independent authority | 2–4 |
| `mcg-daemon` | Opens `AppRuntime` and serves API | Transitional packaging/compatibility entry; evolve into or supervise Root without breaking installed `mcg` identities | 2 |
| `mcg-policy` | Policy evaluation | Keep policy evaluation library; Root owns final decision and policy bundle | 2, 4 |
| `mcg-events` | Event contracts/projections | Reuse behind canonical versioned encoding; Root ledger is production history | 1–3 |
| `mcg-agents` | Roles, organization, experiments, evaluation-shaped records | Keep execution/organization compatibility; move protected experiment judgment out; map Coder/Tester to Work/Verify invocations | 5, 7 |
| `mcg-providers` | Provider registry/execution | Treat as gateway; require Root-bound egress permits and opaque secret handles | 6 |
| `mcg-tools` | Filesystem, HTTP, browser, computer, research tools | Treat as child/gateway execution; require exact action/tool permits and sandbox capability | 4, 6 |
| `mcg-mcp` | MCP server/consumer bridge | Keep behind Tool Gateway; no ambient authority | 6 |
| `mcg-plugins` and plugin crates | Plugin trust, grants, runner | Preserve stronger sandbox/trust work; bind invocation to Root permits and action semantics | 6 |
| `mcg-codex-app-server` | Codex transport, account, models, conversations, hooks | Keep as replaceable model/tool gateway behind Root egress/authority | 6 |
| `mcg-config` | Configuration and data paths | Split protected Root/Evaluator configuration from writable cognitive configuration | 1, 2, 7 |
| `mcg-projects` | Project identity, memory, worktrees | Reuse project/worktree semantics; move authoritative commits to Root and epistemic memory logic to Runtime | 4, 8 |
| Tauri Rust | Service bootstrap, OS integration, command carrier | Presentation and OS adapter only; authoritative operations go through Root IPC | 2–6 |
| React UI | Chat, agents, approvals, settings, diagnostics | Preserve product shell; add Simple/Advanced/Developer progressive disclosure over identical backend state | 1 onward |
| `mcg` CLI | Conversation and broad management surface | Preserve compatibility; route authoritative work through Root; add explicit runtime/workflow/knowledge/improvement/evaluation inspectors | 2 onward |
| Current experiment API/storage | Ordinary mutable experiment definitions, benchmarks, decisions | Preserve as legacy evidence where needed; new protected experiments live in Evaluator-owned storage and cross only through signed summaries | 7 |

## 5. Recommended Workspace Evolution

Do not create one crate per cognitive noun. Add or split code only for a trust boundary, executable boundary, stable multi-consumer contract, platform boundary, or separate release ownership.

Recommended minimum additions:

```text
crates/
  mcg-protected-root/          Root authority, writer, workflow/action commit
  mcg-protected-evaluator/     protected experiment store and decisions
  mcg-platform/                IPC, peer identity, process, path, sandbox primitives

apps/
  mcg-runtime-worker/          writable cognition process
  mcg-protected-rootd/         Root process or compatibility-backed root entry
  mcg-protected-evaluator/     evaluator process
```

Compatibility-sensitive installed names may remain `mcg-service`, `mcg`, and MetaCog. The internal crate/binary mapping must be decided in ADR-001 before creation so packaging does not acquire redundant supervisors.

Existing crates can satisfy PRD semantic namespaces:

- `mcg-domain` can serve the `cognition-types` role if stable encoding and dependency rules are enforced.
- `mcg-providers`, `mcg-tools`, `mcg-mcp`, `mcg-plugins`, and `mcg-codex-app-server` can collectively satisfy gateway responsibilities without a new umbrella crate.
- `mcg-runtime` can retain Runtime implementation logic after direct storage and protected judgment are removed.
- Existing research helpers can move behind a production-forbidden `research` boundary rather than creating production services.

## 6. Major Gaps

### 6.1 Trust ownership

Current `AppRuntime` owns the database and cognition in one process. Target Runtime must be safe to treat as compromised and must not possess Root or Evaluator storage handles, secrets, or commit authority.

### 6.2 Protected evaluation

Current experiment/evaluation APIs are in the ordinary service/runtime dependency graph. Target evaluation requires a separately launched process, separate database and artifacts, protected fixtures and policy, controlled disclosure, and signed/hash-bound decisions.

### 6.3 IPC

Current desktop/service communication is authenticated loopback HTTP. Target trust-domain communication requires versioned length-framed local IPC, OS peer identity, session capability, replay/idempotency controls, bounded messages, and fail-closed version negotiation. Consumer HTTP may remain as a Root-owned adapter, but it cannot be the implicit trust boundary between Root, Runtime, and Evaluator.

### 6.4 Canonical persistence protocol

Current Rust/JSON models and SQLite migrations provide useful versioning, but the PRD requires explicit stable discriminators, canonical encoding/normalization versions, canonical hashes, immutable historical bytes, and long-lived decoder/upcaster fixtures.

### 6.5 Action uncertainty

Existing operations include good recovery and uncertainty mechanisms, especially plugin/platform work, but all consequential effects must converge on the canonical intent, execution, receipt, outcome, reconciliation, compensation, and commit contract.

### 6.6 Invocation reconstruction and egress

Current conversation/tool activity is extensive. The target requires a single reconstructable `InvocationManifest` and exact Root-bound remote data-egress permit for every material model call.

### 6.7 Persistent epistemics

Current project memory, evidence, events, and agent state provide a base, but Claims, perspectives, exposure, acquisition provenance, narrow Engrams, empirical memory contribution, and authorized context manifests require explicit semantics.

### 6.8 Agent model

The current product visibly models Coordinator, Coder, Tester, and organizational agents. The target distinguishes persistent runtime lineage from temporary Chat/Work/Verify invocations. UI, storage, and CLI must preserve old history while making new semantics accurate.

### 6.9 Interface complexity

Current user settings persist an `advanced_mode` Boolean. The target presentation preference is an enum with `Simple`, `Advanced`, and `Developer`. Migration must map `false -> Simple` and `true -> Advanced`; no existing user should be placed into Developer automatically.

## 7. Migration Boundaries

### Existing production database

- Create a verified backup before the first ownership migration.
- Preserve the existing database path for compatibility unless ADR-002 proves a safer atomic relocation.
- Backfill one default `RuntimeIdentity` and bind existing durable objects without rewriting historical payload meaning.
- Root becomes the only production opener/writer after cutover.
- Runtime tests may use in-memory stores through test-only adapters, but the production Runtime binary must not link or acquire a Root database path.

### Existing evaluation records

- Treat existing evaluation/experiment records as legacy production evidence, not retroactively protected evidence.
- Import only explicit immutable references into the evaluator when provenance supports it.
- Do not relabel old results as protected, independent, same-seed, or contamination-aware.

### Existing agents

- Preserve historical agent names and messages.
- Add canonical invocation-role metadata to new work.
- Project existing Coder/Tester records as compatibility Work/Verify activity where defensible.
- Do not fabricate an invocation manifest for historical runs that lacks reconstructable source evidence.

### Existing settings

- Replace `advanced_mode: bool` with a revisioned interface-complexity value while accepting the old request/record shape during the supported migration window.
- Interface complexity remains presentation-only.

## 8. Required ADRs

| ADR | Decision required | Must close before |
|---|---|---|
| ADR-001 | Exact process topology, executable names, supervision, and compatibility with `mcg-service` | Accepted |
| ADR-002 | Stable encoding, normalization, hashing, discriminators, and decoder/upcaster support | Accepted |
| ADR-003 | Interface-complexity migration and Simple/Advanced/Developer information architecture | Accepted |
| ADR-004 | Root-owned state migration, backup, rollback, evaluator-store creation, and downgrade | Accepted |
| ADR-005 | Named-pipe/Unix-socket framing, peer identity, authentication, replay, and version negotiation | Stage 2 |
| ADR-006 | Workflow state machine, attempt lineage, lease fencing, queues, restart scanner | Stage 3 |
| ADR-007 | Action intent/execution/receipt/outcome/reconciliation/compensation | Stage 4 |
| ADR-008 | Invocation and context reconstruction, privacy, redaction, artifact references | Stage 5 |
| ADR-009 | Model data-egress permit and secret-handle resolution | Stage 6 |
| ADR-010 | Tool/Skill manifest, sandbox capability, process execution, and gateway authority | Stage 4/6 |
| ADR-011 | Evaluator database, fixture/artifact protection, worker launch, result signing/disclosure | Stage 7 |
| ADR-012 | Promotion transaction, candidate rebase, monitoring, semantic rollback | Stage 7/10 |
| ADR-013 | Agent-to-invocation migration and global activity browser semantics | Stage 5 |
| ADR-014 | Research crate/dependency isolation and evaluator-only measurement retention | Stage 7 |

## 9. Stage 0 Exit Checklist

- [ ] Concurrent parity/hook work is committed and pushed.
- [ ] Clean baseline commit and package are recorded.
- [x] New PRDs are present in `docs/NewPRD`.
- [x] Staged implementation plan exists.
- [x] Provisional current-to-target component map exists.
- [ ] Source-precedence decision is recorded in durable repository memory.
- [ ] Requirements traceability matrix covers every normative contract area and experiment phase gate.
- [x] ADR-001 through ADR-014 have accepted decisions.
- [ ] Representative existing database backup/restore is verified on the clean baseline.
- [ ] Existing full automated and native smoke gates pass on the clean baseline.
- [ ] No unresolved ownership ambiguity blocks stable contract implementation.
