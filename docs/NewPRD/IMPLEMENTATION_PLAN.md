# MetaCog Cognitive Runtime Implementation Plan

**Status:** Active staged delivery plan; Stages 0–4 have implementation checkpoints, Stage 5 is implemented through reconstructable real-process Work/Verify, governed agent inspection, and the compatibility-agent model bridge, and Stage 6 now includes exact Runtime/lease-bound model-egress permits, deterministic Root dispatch/result fencing, a verified isolated credential-resolving OpenAI-compatible provider runner, fresh/resumed production compatibility-agent routing, bounded distinct-authority retry, governed buffered and true-streaming cancellation, least-authority ChatGPT OAuth execution, stable Tool/Skill/Capability/Sandbox contracts, schema-61 immutable manifest/proposal/permit/opaque-handle authority with deterministic Root issuance and active replay, permit-consuming filesystem execution, cancellation-aware no-capability WASI plugin execution, source-bound capability-bearing plugin project reads/writes/exact-executable process/exact-origin HTTP requests, exact modern Streamable HTTP and catalog-bound legacy HTTP/current-or-legacy stdio plugin MCP execution with child-only opaque bearer resolution and conservative no-redispatch settlement, bounded Root/CLI/Simple/Advanced/Developer capability inspection, content-minimized observed provider/model fingerprints with progressive inspection, and independent certificate closure
**Scope:** Alignment with the Adaptive Cognitive Agent Runtime PRD v1.4.2, Cognitive Runtime Implementation Contract v0.1, and Cognitive Runtime Experimental Protocol Specification v0.2
**Product:** MetaCog desktop and `mcg` CLI on Windows, macOS, and Linux

## 1. Purpose

This plan turns the new cognitive-runtime specifications into cumulative, independently testable delivery stages. Each stage must leave MetaCog usable, preserve existing user data, and establish the invariants required by later stages.

The work is an evolution of the existing product, not a second application or a rewrite that discards current capabilities. Existing conversations, projects, approvals, tools, plugins, hooks, schedules, agents, evidence, backups, and Codex-compatible workflows must be preserved or migrated deliberately.

The implementation is governed by these rules:

1. Protected judgment precedes recursive optimization.
2. Deterministic infrastructure precedes model-dependent cognition.
3. One component owns each authoritative state transition.
4. An external effect is never assumed to have failed merely because its caller timed out or died.
5. Existing data is migrated in place with tested rollback and backup paths.
6. Every stage has automated, E2E, native UAT, packaging, and documentation gates proportional to its risk.
7. A later stage cannot begin until the preceding stage's required exit gate is recorded as passing.

## 2. Product and Interface Decisions

### 2.1 One runtime, three interface complexities

MetaCog has one underlying runtime. The interface preference changes only how much control and detail are exposed.

```text
Same runtime, workflows, data, authority, and security boundaries

Simple  ->  Advanced  ->  Developer
          increasing visibility and control
```

- **Simple** presents chat, projects, approvals, concise progress, verified results, and brief retained-learning summaries.
- **Advanced** exposes workflows, invocations, worktrees, agents, tools, hooks, providers, policies, memory, evidence, and recovery state.
- **Developer** exposes runtime lineage, manifests, cognitive configuration, context and memory policies, ImprovementCases, experiments, evaluation disclosures, taint, promotion, rollback, IPC, sandbox, and process diagnostics.

Changing interface complexity must not by itself change runtime behavior, authority, execution policy, model selection, memory, or evaluation state. It is a presentation preference, not a security boundary or execution mode.

Some protected values may be inspectable but remain immutable through the UI. Developer mode must not expose direct database mutation, protected fixture access, secret extraction, authority bypass, self-promotion, or persistent trust bypass.

### 2.2 Canonical user-facing workflow

The everyday workflow remains conversational:

```text
User request
  -> Contract
  -> Context preparation
  -> Work
  -> Verify
  -> Commit or report blocker
  -> Retain only useful learning
  -> Report result and uncertainty
```

The Simple UI may use approachable labels such as Preparing, Working, Verifying, and Finishing. Advanced and Developer views may reveal the exact canonical states and manifests.

### 2.3 Agent terminology

`Chat`, `Work`, and `Verify` are invocation roles, not automatically persistent agent identities.

- Protected Root is a deterministic authority boundary, not an agent.
- Coordination is a runtime policy/decision, not necessarily a permanent Coordinator personality.
- Existing Coder activity maps to temporary Work invocations.
- Existing Tester activity maps to independent Verify invocations.
- Durable organizational agents may remain only when they represent a genuine durable product concept rather than a transient model call.

The planned global agent browser should therefore become a global workflow/invocation browser, with compatibility labels where migration requires them.

## 3. Source Precedence and Change Control

When requirements conflict, use this order:

1. Cognitive Runtime Implementation Contract v0.1 for executable ownership, IPC, persistence, workflow, security, and recovery semantics.
2. Adaptive Cognitive Agent Runtime PRD v1.4.2 for product architecture and phase ordering.
3. Cognitive Runtime Experimental Protocol Specification v0.2 for experiment validity and research claims.
4. Existing MetaCog behavior and Codex parity requirements, unless explicitly preserved as a stronger safety or compatibility guarantee.

Before Stage 1 begins, record an explicit product decision that these documents govern new cognitive-runtime work. Each stage must update the architecture/parity matrix and durable repository memory.

## 4. Delivery Strategy

### 4.1 Incremental cutover

Use a strangler-style migration rather than a big-bang rewrite:

1. Establish stable contracts and compatibility fixtures.
2. Introduce Protected Root as the only new authority path.
3. Route one narrow deterministic workflow through Root.
4. Expand the routed surface until all consequential work uses Root.
5. Remove obsolete authority paths after equivalence and migration gates pass.

Do not maintain two authoritative writers. Temporary compatibility adapters may translate old API shapes into the new Root contract, but only one component may commit canonical state.

### 4.2 Stage completion

A stage is complete only when:

- its implementation and migration are complete;
- its required tests pass locally;
- its E2E and fault-injection gates pass;
- visible changes pass native UAT;
- affected platform packages are built and exercised;
- documentation, threat model, and compatibility matrices are current;
- a verification receipt is recorded;
- the change is committed and pushed without unrelated worktree changes.

### 4.3 Concurrent work

Before implementation starts, allow the active hook-trust release to finish and establish a clean baseline. For later parallel work:

- assign non-overlapping files or crates to each workstream;
- designate one integration owner for schema and shared-contract changes;
- prohibit two agents from independently editing migrations, root contracts, or canonical codecs;
- merge contract changes before dependent frontend or gateway work;
- rerun the complete affected gate after integration, not merely each branch's focused tests.

## 5. Staged Roadmap

## Stage 0 — Baseline, Reconciliation, and Architecture Freeze

### Goal

Create a trustworthy starting point and a precise current-to-target map before changing runtime ownership.

### Work

- Finish, verify, package, commit, and push the active hook-trust cycle.
- Capture the exact clean commit, package version, database schema, CLI surface, desktop surface, and cross-platform qualification state.
- Inventory all current authoritative write paths, SQLite owners, service processes, sidecars, tool launchers, model/provider paths, approval paths, and recovery paths.
- Map every existing crate and binary to the target responsibilities: cognition types, Protected Root, Runtime, Evaluator, gateways, platform, research, presentation, or compatibility adapter.
- Produce ADRs for process topology, database ownership, local IPC, canonical encoding, workflow/lease state, action reconciliation, model egress, sandbox capability, evaluator isolation, and migration strategy.
- Create a requirements traceability matrix linking every normative requirement to a planned stage and test.
- Freeze GUI terminology and navigation for Simple, Advanced, and Developer.
- Define feature activation and rollback strategy for the staged cutover.

### Testing and exit gate

- Existing full workspace tests, desktop tests, E2E, native smoke UAT, packaging, and backup/restore pass on the clean baseline.
- A production-like database backup is restored into a disposable environment and verified.
- Every current authoritative mutation path has an identified target owner.
- No unresolved architecture decision blocks Stages 1–4.

## Stage 1 — Stable Contracts, Canonical Encoding, and Compatibility Fixtures

### Goal

Establish the durable language shared by Root, Runtime, Evaluator, CLI, and Tauri.

### Work

- Add stable identifier newtypes and stable type/message discriminators.
- Define versioned payload envelopes, canonical normalization, canonical encoding, domain-separated hashing, decoder/upcaster chains, and protocol negotiation.
- Define initial contracts for `RuntimeIdentity`, `TaskContract`, `WorkflowAttempt`, `Event`, `ActionEnvelope`, `ActionExecution`, `ActionReceipt`, `ContextManifest`, `InvocationManifest`, `CompletionCertificate`, `EvaluationSeed`, and `ExperimentSpec`.
- Add dependency rules that prevent forbidden trust-boundary imports.
- Create checked-in golden payload and hash fixtures.
- Define error taxonomy and redaction requirements shared across all interfaces.
- Add the interface-complexity preference as presentation-only data: `Simple`, `Advanced`, or `Developer`.

### GUI and CLI

- Add the three-level interface selector and explanatory copy without changing runtime behavior.
- Add Developer-only contract/manifest viewers backed by current compatibility projections where possible.
- Keep all existing CLI commands working; introduce only contract inspection commands needed for testing.

### Testing and exit gate

- Unit tests cover IDs, codecs, normalization, hashing, validation, redaction, and upcasting.
- Property tests cover encode/decode stability, malformed inputs, unknown versions, and canonical equivalence.
- Golden v1 fixtures decode and retain stable hashes.
- UI tests prove switching interface complexity only changes presentation.
- CLI snapshot tests prove existing commands and output contracts have not regressed.

## Stage 2 — Protected Root and Authoritative Storage

### Goal

Introduce the actual Root trust domain and make it the sole owner of production-state mutation.

### Work

- Create the `protected-rootd` executable and protected configuration boundary.
- Establish Root-owned `state.db`, schema metadata, bounded writer queue, short transactions, WAL, foreign keys, busy timeout, and bounded readers.
- Implement authenticated, versioned, bounded local IPC using Windows named pipes and Unix-domain sockets on macOS/Linux.
- Validate OS peer identity and per-launch session capability.
- Route Tauri and CLI requests through Root; neither interface may mutate authoritative state directly.
- Create the default `RuntimeIdentity` for migrated installations.
- Migrate existing data into Root ownership without changing user-visible conversation/project identity.
- Maintain a verified backup and rollback path for the migration.
- Ensure current service APIs become Root-owned adapters or read-only projections rather than competing authorities.

### GUI and CLI

- Simple: no workflow change beyond reliable reconnect/recovery messages.
- Advanced: show Root connection, runtime identity, and storage/migration health.
- Developer: show process identity, protocol version, schema version, session state, and immutable migration receipt.
- CLI: add `mcg runtime status`, `mcg runtime inspect`, and structured health output.

### Testing and exit gate

- SQLite integration tests cover migrations, WAL, foreign keys, writer serialization, concurrent reads, backups, restore, and downgrade refusal.
- IPC tests reject malformed, oversized, replayed, stale-session, wrong-peer, and unsupported-version requests.
- Security tests prove Runtime/UI cannot directly write Root state.
- Migration E2E covers fresh install, representative existing database, interrupted migration, retry, backup restore, and old-client compatibility behavior.
- Native UAT proves all three interfaces show the same underlying conversations and projects before and after restart.

## Stage 3 — Durable Workflow, Leases, and Recovery

### Goal

Make every accepted unit of work durable and restart-safe before adding new side effects.

### Work

- Implement versioned `TaskContract` and legal `WorkflowAttempt` transitions.
- Add durable queues, attempt lineage, generation-fenced leases, heartbeats, cancellation, retry semantics, and restart scanning.
- Persist events before projecting them to GUI/CLI consumers.
- Reattach or safely settle active work after Root, Runtime, UI, or application restart.
- Ensure terminal workflows cannot return to ordinary active states.
- Preserve current conversations and queue/steer behavior through compatibility projections.

### GUI and CLI

- Simple: Preparing, Working, Verifying, Recovering, Completed, Blocked, and Cancelled states.
- Advanced: exact workflow state, attempt, lease owner, retry ancestry, dependency, and blocker.
- Developer: raw versioned state transition and event history with correlation identifiers.
- CLI: expose `mcg workflow list/show/events/activity/cancel`; `events` emits bounded cursor-fenced NDJSON and can follow through Root IPC session restarts.

### Testing and exit gate

- State-machine/property tests generate legal and illegal workflow and lease sequences.
- Kill/restart tests cover every nonterminal state.
- Stale lease generations cannot commit after reassignment.
- UI reconnect cannot change backend truth.
- E2E demonstration: create workflow, kill the application/process, restart, and recover the exact workflow without duplicating work.

## Stage 4 — Governed Actions, Side Effects, and Reconciliation

### Goal

Make consequential effects durable, authorized, observable, and safe under ambiguous failure.

### Work

- Implement the full intent -> authorization/reservation -> execution -> receipt -> commit chain.
- Add `NO_EFFECT`, `EFFECT_CONFIRMED`, and `OUTCOME_UNKNOWN` semantics.
- Persist normalized operation hashes before authorization.
- Implement one governed read and one reversible write/patch inside an isolated worktree.
- Add semantic path validation, symlink/reparse defense, resource budgets, process-tree control, and platform sandbox capability profiles.
- Implement reconciliation jobs for ambiguous filesystem effects.
- Represent compensation as a new causal action rather than history deletion.
- Require stronger authority and explicit presentation for irreversible actions.

### GUI and CLI

- Approval cards show exact origin, effect, scope, egress, operation hash, reversibility, evidence, and policy reason.
- Approval-source navigation opens the originating conversation, invocation, command, and evidence.
- Simple shows understandable reconciliation messages.
- Advanced shows action state and receipt.
- Developer shows the complete action envelope, authorization result, effect classification, and reconciliation evidence.
- CLI adds action inspection and separately confirmed exact compensation initiation; consequential unknown effects are never blindly retried.

### Testing and exit gate

- Crash injection at every durable boundary: before intent, after intent, around authorization, before/during tool launch, immediately after effect, before receipt, after receipt, and after commit before UI delivery.
- Security tests cover path traversal, symlink/reparse escape, environment poisoning, process escape, protected-path access, and authorization substitution.
- E2E failure demonstration: effect succeeds, Root dies before receipt persistence, restart produces `OUTCOME_UNKNOWN`, reconciliation confirms the existing effect, and the effect occurs exactly once.
- Native approval UAT is completed in Simple, Advanced, and Developer on all supported platforms where the mechanism differs.

## Stage 5 — Runtime Worker and Reconstructable Deterministic Cognition

### Goal

Separate writable cognition from Root authority and prove reconstructable Work and Verify execution without model stochasticity.

### Work

- Create the `runtime-worker` process and authenticated Root-to-Runtime protocol.
- Implement deterministic fake provider and distinct fake Work and Verify invocations.
- Persist `ContextManifest`, `InvocationManifest`, model run, tool run, and `CompletionCertificate`.
- Bind configuration, prompts, roles, context references, tools, skills, authority, environment, and request/response hashes.
- Ensure Runtime proposes typed mutations and cannot open or commit Root storage directly.
- Convert current agent activity into workflow-scoped invocation projections.

### GUI and CLI

- Replace permanent-agent assumptions with workflow/invocation presentation.
- Preserve friendly current labels where needed, while Developer shows exact Chat/Work/Verify roles.
- Add the global workflow/invocation browser and remote inspection only after the canonical ownership model exists. The bounded compatibility-agent inventory, exact local selection, Root-owned workflow/current-attempt invocation drill-down, and content-minimized direct remote Codex sub-agent inventory are implemented. Remote inventory remains inspection-only and never invents a MetaCog workflow binding.
- CLI can reconstruct and print the conditions of any material invocation.

### Testing and exit gate

- Deterministic Work + Verify E2E produces a valid completion certificate.
- Engineering can answer every invocation-reconstruction question required by Gate 6 from persisted evidence.
- Runtime process death, late response, duplicate response, cancellation, and restart are covered.
- No test relies on a real model for correctness of workflow infrastructure.

## Stage 6 — Governed Providers, Tools, Hooks, Plugins, and Codex Compatibility

### Goal

Route existing MetaCog capabilities through Root-bound permits without losing current product value.

### Work

- Implement model-egress proposals and Root-bound permits for exact disclosed context, provider, model, cost/budget, and destination.
- Resolve secrets only inside authorized gateway adapters using opaque secret handles.
- Retain the completed permit-consuming filesystem, source-bound plugin process/HTTP/browser, and exact modern Streamable HTTP plus catalog-bound legacy HTTP/current-or-legacy stdio plugin MCP executors, and move Codex app-server, local Ollama/LM Studio, remote transport, hook, general browser/computer, and general process operations behind governed gateway contracts.
- Preserve exact hook trust and invocation-only trust-bypass semantics.
- Define `ToolManifest`, `SkillManifest`, `CapabilityProfile`, and sandbox requirements.
- Retain deterministic fake providers and recorded responses for regression testing.
- Add one real provider route only after the governed path is complete.
- Persist immutable terminal provider latency, actual-cost provenance, and categorical error evidence without retaining response content or arbitrary provider metadata.

### GUI and CLI

- Simple retains current provider and permission controls.
- Advanced exposes tools, hooks, plugins, capabilities, provider reliability, and egress summaries.
- Developer exposes manifests, exact scopes, resolved model identity, permit hashes, sandbox capability, and gateway diagnostics.
- Existing Codex-compatible CLI behavior remains an acceptance suite, not a competing authority path.

### Testing and exit gate

- Provider contract tests use fakes by default and recorded responses for parser regressions.
- Real provider tests are opt-in/nightly except for bounded release smoke tests.
- Security tests cover secret leakage, unauthorized network destination, schema bypass, argument substitution, hook trust, and plugin/MCP capability escalation.
- E2E verifies a real coding request through Root -> Runtime -> governed provider/tools -> Verify -> CompletionCertificate.
- Token, prompt, and private-data redaction tests cover logs, errors, events, backups, and UI projections.

## Stage 7 — Protected Evaluator and Phase 0 Integrity Gate

### Goal

Create the independent judgment boundary required before automatic improvement.

### Work

- Create the `protected-evaluator` process, protected `evaluation.db`, protected artifact root, and Root-only authenticated IPC.
- Add evaluator objective manifests, portfolio governance, fixtures, TCB manifests, learning modes, exposure tracking, retirement, and signed/hash-bound decision summaries.
- Implement canonical `EvaluationSeed`, seed equivalence, immutable `ExperimentSpec`, deterministic baseline/candidate launch, declared mutation checks, and environment fingerprints.
- Add promotion decision states: `PROMOTE`, `REJECT`, `CONTINUE`, `INCONCLUSIVE`, and `INVALID_EXPERIMENT`.
- Implement mechanical search-breadth accounting, hard ceilings, candidate rebase checks, and synthetic promotion-epoch rollback semantics.
- Allow only Root to activate an evaluator-authorized promotion.

### GUI and CLI

- Simple shows only whether a change was independently verified and any user-relevant uncertainty.
- Advanced shows disclosed evaluator summaries and promotion state.
- Developer adds Evaluation Lab with experiment manifests, arms, exposure/taint, decision summary, replication status, and protected-boundary explanations.
- Raw protected fixtures and measurements remain unavailable to candidate cognition and ordinary telemetry.

### Testing and exit gate

- Same-seed deterministic A/B is reproducible.
- Undeclared relevant divergence produces `INVALID_EXPERIMENT`.
- Candidate/Runtime cannot alter evaluator assets, metrics, policy, negative evidence, or results.
- Tests cover false causal credit, contamination laundering, pseudoreplication, shared-infrastructure interference, protected-result-channel inference, stale candidate rebase, and hard-stop enforcement.
- Root verifies signed decisions; Evaluator cannot activate configurations.
- All Implementation Contract acceptance Gates 1–7 pass.
- No recursive optimizer work begins until the complete Phase 0 experimental-integrity gate is recorded as passing.

## Stage 8 — Persistent Epistemic Cognition

### Goal

Add useful persistent cognition while preserving distinctions between history, belief, exposure, provenance, and authority.

### Work

- Implement generic Record/RecordVersion/edge persistence and rebuildable FTS/index projections.
- Add perspective-indexed Claims, EpistemicExposure, ExperienceMode, actor goals/constraints, and derived common-ground queries.
- Add repository projection, Work State, ContextPolicy, EpistemicAccessState, progressive retrieval, and context manifests.
- Add narrow Engram create/revise/merge/forget operations and empirical contribution measurement.
- Add runtime fork/lineage handling, pre-fork inheritance, configuration/promotion provenance, and expiration of invocation-specific beliefs.
- Do not add separate SelfModel, UserModel, ActorModel, Theory-of-Mind, or identity databases.

### GUI and CLI

- Simple shows concise retained-learning summaries and safe correction/deletion controls.
- Advanced exposes useful memories, Skills, sources, confidence, contribution, and project knowledge coverage.
- Developer exposes Claims, perspectives, exposure, provenance, edges, retrieval traces, context manifests, lineage, and retention-policy inputs.
- CLI adds bounded knowledge search, memory inspection/correction, provenance, and context-manifest inspection.

### Testing and exit gate

- Record CAS, source invalidation, merge/dedupe, retrieval authorization, perspective isolation, and fork inheritance have unit/property/integration coverage.
- Retrieval tests prove information access never creates authority.
- Privacy tests prove unauthorized perspectives and protected evaluator content cannot enter context.
- Ablations compare no memory, retrieval-only, and retained Engram conditions on fresh homologous tasks.
- Persist only when measured expected future value justifies retention.

## Stage 9 — Skills, Reliable Autonomy, and Minimal Metacognition

### Goal

Improve task selection and execution using bounded competence, calibration, and falsifiable self-prediction.

### Work

- Implement validated Skills, deterministic Tool compilation where warranted, small competence envelopes, and a capability index only as needed.
- Add RuntimeTelemetry baseline, intervention-conditioned WORLD/SELF/OTHER predictions, prediction errors, abstention, value-of-information decisions, and a small CoordinationPolicy.
- Add query-first `SelfModelProjection`; do not create a persistent self agent or authoritative self-model store.
- Measure verifier error correlation and coalition benefit before increasing coordination complexity.
- Add safe fast paths and restore richer reasoning on out-of-distribution or low-confidence work.

### GUI and CLI

- Simple shows calibrated confidence, verification status, and meaningful abstention/blockers.
- Advanced shows capability, reliability, cost/latency, selected execution topology, and prediction-versus-outcome summaries.
- Developer exposes telemetry, SELF/WORLD/OTHER predictions, interventions, prediction errors, competence envelopes, coordination decisions, and ablation comparisons.

### Testing and exit gate

- Compare richer SELF mechanisms against telemetry-only and equal-cost generic deliberation baselines.
- Test FULL, PARTIAL, DELAYED, NOISY, and BLINDED telemetry conditions.
- Require held-out intervention prediction and useful control changes.
- Test false-belief/exposure/goal tasks and role/coalition ablations.
- Remove or demote mechanisms that do not beat simpler baselines under the Phase 3 research gate.

## Stage 10 — Bounded Improvement Engine and Semantic Rollback

### Goal

Allow narrow, evidence-driven improvement without allowing cognition to judge or promote itself.

### Work

- Implement ImprovementCase creation, deduplication, merge, reopening, and failure localization.
- Implement the approved Change Grammar and smallest-plausible-mutation selection.
- Generate model-proposed variants plus a mechanically bounded local neighborhood.
- Run candidates and baselines from equivalent EvaluationSeeds under immutable ExperimentSpecs.
- Track learning mode, evaluation exposure, taint, search breadth, causal credit, candidate freshness, interactions, and promotion epochs.
- Implement champion/challenger/deletion challengers, stop/futility rules, monitoring, rollback, deletion, and escalation to the next causal layer.
- Restrict automatic mutation to Root-approved configuration, policy, prompt, Skill, Tool/Probe, role, coordination, verification, and rebuildable-representation surfaces.
- Protected source changes remain human-reviewed signed-release proposals.

### GUI and CLI

- Simple shows only verified improvements that materially affect the user and provides clear rollback/regression notices.
- Advanced shows ImprovementCases, candidate status, evidence, monitoring, and rollback availability.
- Developer exposes localization, mutation operator, predicted mechanism, search budget, seed/spec, taint, decision, promotion epoch, interaction tests, and retained/deleted complexity.

### Testing and exit gate

- Hidden repair-layer discovery, false promotion, stale rebase, interaction, deletion, stopping, and fresh-transfer tests pass.
- Semantic rollback removes unsupported Engrams, Claims, Skills, calibration, configuration, and descendants according to provenance.
- Deep-lineage poison-persistence tests pass or the durable-learning horizon is narrowed.
- The optimizer demonstrates minimal gains, appropriate inconclusive outcomes, simplification, and acceptable search cost.
- No Evolution Lab or population search begins until the Phase 4 gate passes.

## Stage 11 — Simple Cultural Transfer

### Goal

Transfer validated capability between runtimes without making networking or publication part of ordinary cognition.

### Work

- Implement immutable CultureArtifact, attestation, snapshots, local resolution, provenance, conflict handling, and local Skill validation.
- Start with local files, SQLite export/import, or a simple endpoint.
- Require manual approval for private-derived outbound artifacts.
- Keep culture non-authoritative and disabled/frozen for initial developmental cohorts.
- Defer P2P, reputation systems, global culture stores, and automatic private-derived publication.

### Testing and exit gate

- Cross-runtime transfer retains provenance and cannot grant authority.
- Poisoning, conflict, stale attestation, cumulative disclosure, and private-derived publication tests pass.
- Native UAT verifies clear source, privacy, trust, and approval presentation.
- Transfer must demonstrate useful fresh-task value before broader transport is considered.

## Stage 12 — Development, Identity, and Longitudinal Research

### Goal

Run research protocols over the stable runtime without converting speculative identity claims into product control signals.

### Work

- Implement research-only matched forks, memory/Skill transplantation, model replacement, lesions, future commitments, false autobiography, split/reintegration, targeted forgetting, and sparse longitudinal checkpoints.
- Record the highest demonstrated evidence level: retention, procedural reuse, structural transfer, behavioral development, learning-to-learn, plasticity change, or meta-development.
- Keep research measurements evaluator-owned and unavailable as self-preservation objectives.
- Keep first cohorts culture-disabled or identically frozen.

### Testing and exit gate

- Protocols use matched seeds, novel homologous tasks, explicit carryover controls, replication, and lineage integrity.
- Strong claims are killed or narrowed when simpler retrieval/Skill explanations account for results.
- Research results include explicit falsifiers and claim-level bounds.
- Negative results produce documented pivots rather than forced product mechanisms.

## Stage 13 — Optional Evolution and Advanced Challengers

### Goal

Add only mechanisms that pass explicit product-rent or scientific-rent gates.

### Preconditions

- Phase 4 bounded optimizer is boringly reliable.
- Stages 11–12 demonstrate a measured need not met by simpler mechanisms.
- Resource, privacy, evaluation, and rollback budgets are established.

### Possible work

- Simple configuration population search and archives.
- Quality diversity or islands only if bounded local search demonstrably loses.
- P2P culture only if simple transfer volume requires it.
- Global Workspace, Attention Schema, richer SELF/ToM, predictive hierarchy, ontology evolution, adapters, or parametric consolidation only as isolated challengers.

### Exit rule

Every mechanism must earn product rent, scientific rent, or be deleted. Research-only mechanisms must remain outside normal production cognition.

## 6. Testing Methodology

## 6.1 Test pyramid

The repository will maintain nine complementary tiers.

| Tier | Purpose | Default dependency |
|---|---|---|
| 1. Unit | IDs, codecs, hashing, policy, transitions, validation, accounting | None |
| 2. Property/state machine | Legal/illegal sequences and invariants | Deterministic generators |
| 3. Persistence integration | SQLite, migrations, CAS, backup/restore, indexes | Temporary real databases |
| 4. Crash injection | Every durable boundary and uncertain effect | Deterministic fault controller |
| 5. Process/IPC | Framing, authentication, replay, death, late results | Real local processes |
| 6. Security/adversarial | Escape, poisoning, leakage, authority violation | Isolated fixtures/VMs |
| 7. Provider contract | Provider parsing, streaming, retry, egress | Fake/recorded by default |
| 8. Protected evaluator | Seeds, experiments, taint, promotion validity | Fake deterministic runtime/model |
| 9. Real cognition E2E | Model reasoning is the tested variable | Opt-in real local/remote model |

Tests must be deterministic unless model cognition is explicitly the variable. A real provider must never be required to prove workflow, persistence, authority, crash, or evaluator infrastructure correctness.

## 6.2 Required invariants

At minimum, automated tests continuously enforce:

- authority intersection never expands authority;
- UI, Runtime, tools, plugins, hooks, providers, and Evaluator cannot commit Root-owned state;
- Runtime cannot access protected evaluator storage or fixtures;
- Evaluator cannot activate a configuration;
- a terminal workflow cannot ordinarily become active again;
- a stale lease generation cannot commit;
- one idempotency key cannot cause a second logical effect;
- action intent is durable before effect;
- an unknown consequential effect is reconciled rather than blindly retried;
- compensation is a new causal action;
- historical payload bytes remain interpretable and immutable;
- canonical hashes change only under an explicit encoding/normalization version change;
- unavailable sandbox strength fails closed;
- information access never creates authority;
- protected research information never enters ordinary cognition or telemetry;
- only Root can activate a valid evaluator-authorized promotion.

## 6.3 E2E suites

Maintain separate suites so failures identify the broken boundary:

1. **Contract E2E:** typed request/response and stable JSON/CBOR fixtures.
2. **Process E2E:** Tauri/CLI -> Root -> Runtime -> gateway/tool -> Root commit.
3. **Recovery E2E:** kill each process at controlled points and verify restart truth.
4. **CLI E2E:** interactive TTY, scripted commands, JSON/JSONL, exit codes, resume, fork, approval, and reconciliation.
5. **Desktop browser E2E:** deterministic UI navigation and state projection through Playwright.
6. **Native desktop E2E/UAT:** packaged Tauri application with real OS dialogs, menus, process lifecycle, filesystem, sandbox, and accessibility behavior.
7. **Security E2E:** adversarial repositories, paths, IPC frames, providers, tools, plugins, hooks, and evaluator probes.
8. **Migration E2E:** old database/package -> backup -> upgrade -> restart -> verification -> rollback/restore.
9. **Experimental E2E:** same-seed A/B, divergence invalidation, exposure/taint, signed decision, Root promotion, monitoring, rollback.

## 6.4 Native UAT matrix

Every visually or behaviorally meaningful stage receives a written UAT script. At minimum, exercise:

| Dimension | Required coverage |
|---|---|
| Interface complexity | Simple, Advanced, Developer |
| Platform | Windows, macOS, Linux |
| Installation | Development build where useful, then final packaged artifact |
| Theme/layout | Dark, light, normal width, compact width |
| Input | Keyboard-only, pointer, focus loss, Escape, screen-reader labels |
| Lifecycle | Fresh start, active work, restart, reconnect, cancellation, recovery |
| Authority | Approve, reject, stale approval, changed operation, unavailable sandbox |
| Data | Fresh profile, migrated profile, backup restore |

The same workflow must remain the same underlying workflow when switching Simple -> Advanced -> Developer -> Simple. UAT must verify that only presentation changes unless the user deliberately changes a newly exposed setting.

Developer UAT must verify that protected fields are visibly locked, their protection reason is understandable, and no inspector permits mutation through client-side manipulation.

## 6.5 Cross-platform qualification

- Run pure Rust/frontend tests on every supported platform in CI where practical.
- Run real IPC, process-tree, path, sandbox, migration, installer, and updater tests on native Windows, macOS, and Linux hosts.
- Do not claim identical sandbox mechanisms. Record `HARD`, `BEST_EFFORT`, or `UNAVAILABLE` capability and test fail-closed behavior.
- Repeat the full matched-candidate package/UAT gate whenever Root, Runtime, Evaluator, IPC, persistence, sandbox, installer, or updater identity changes.

## 6.6 Experiment evidence

Each executed experiment should retain the required protected artifacts:

- ExperimentSpec;
- EvaluationSeed and equivalence specification;
- causal estimand/unit/assignment/clustering/interference declaration where applicable;
- arm manifests and environment fingerprints;
- TCB and fixture provenance;
- learning mode and exposure record;
- protected raw measurements;
- contamination/influence analysis;
- causal validity and replication status;
- signed decision summary;
- research and production retention verdicts.

Only authorized summaries cross into Root or runtime-visible presentation.

## 7. Continuous Integration and Release Gates

### 7.1 Pull-request gate

- Formatting and lint checks.
- Dependency-boundary and unsafe-code checks.
- Unit, property, state-machine, and affected SQLite tests.
- Golden codec/hash compatibility.
- Desktop typecheck and deterministic component tests.
- Focused CLI/API contract tests.
- Documentation and traceability validation.

### 7.2 Integration gate

- Full workspace tests with serialized variants where state/process timing requires them.
- Complete deterministic Playwright suite.
- Process/IPC tests.
- Crash-injection suite for affected state machines.
- Security/adversarial suite for affected boundaries.
- Migration and backup/restore tests.
- Fake-provider end-to-end workflow.

### 7.3 Release-candidate gate

- Clean-tree, commit-bound builds.
- Windows, macOS, and Linux packages with all required sidecars.
- Native UAT for affected surfaces and all three interface complexities.
- Real provider smoke test without secret leakage.
- Upgrade, restart, reconciliation, backup, restore, and uninstall/data-retention checks.
- Artifact manifest, hashes, versions, notices, and signing/notarization status.
- Performance, resource, and soak qualification when process/runtime identities changed materially.
- Only the current intended portable/package set is retained.
- Verification ledger, open follow-ups, and repository memory reflect actual results.

## 8. Data Migration and Compatibility Policy

- Preserve compatibility-sensitive `mcg`/`MCG` names, application identity, data paths, and user-visible MetaCog brand unless a separately approved migration changes them.
- Back up before the first Root-owned schema migration.
- Assign existing data to a documented default RuntimeIdentity without rewriting historical meaning.
- Preserve existing conversation, project, task, run, approval, plugin, hook, schedule, provider, and evidence identifiers when possible.
- Historical records retain original bytes and schema identifiers; new projections may be rebuilt.
- Never silently reinterpret an old success/failure as a new effect classification without evidence.
- Refuse unsafe downgrade when newer authoritative state cannot be interpreted.
- Keep compatibility adapters time-bounded and delete them after supported upgrade paths and rollback windows close.

## 9. Documentation Deliverables

Each stage updates:

- architecture diagrams and trust-boundary descriptions;
- database/process ownership matrix;
- CLI reference and machine-readable schemas;
- Simple, Advanced, and Developer UI behavior;
- security threat model and known platform capability limits;
- migration, backup, recovery, and rollback procedures;
- requirement-to-test traceability;
- experiment protocol/version compatibility where applicable;
- release verification ledger and unresolved follow-ups.

Do not claim consciousness, subjective identity, unrestricted self-improvement, universal sandboxing, complete rollback, causal self-knowledge, or independent evaluation beyond what the recorded gates demonstrate.

## 10. Immediate Next Actions

1. Finish Stage 3 capability-executor dispatched-child process-controller propagation and the remaining nonterminal crash/reassignment matrix. Planned model children, production Runtime workers, the base no-capability WASI plugin runner, and governed plugin process execution now observe authoritative parent cancellation; their exact active child tree is terminated and reaped, late output is refused, and durable settlement survives Root reopen. The base plugin runner enters `Cancelled` only after its observational action proves no effect. A cancelled irreversible plugin process remains `OutcomeUnknown`, enters reconciliation, and is never redispatched. Active plugin browser, HTTP, and modern/legacy/stdio MCP capability subprocesses still need the same controller propagation. Schema-62 immutable one-to-many conversation/task/run source linkage, bounded authenticated Root/CLI/Desktop lookup with progressive Advanced/Developer navigation, fail-closed startup reattachment from still-active exact compatibility authority, durable cursor-fenced workflow event streaming, source-bound Approved-only compatibility approval wakes, exact external/protected-component wake bindings, and atomic reconciliation-before-cancellation settlement are implemented. Every future external or protected producer must consume the exact wake boundary rather than directly changing workflow state.
2. Extend Stage 4's implemented owner-authorized compensation and real-child fault paths through packaged adversarial and native three-platform qualification.
3. Finish Stage 5 packaged remote-inventory, cancellation, replacement, crash-injection, and native qualification; direct remote Codex sub-agent inventory and canonical workflow/current-attempt invocation drill-down are implemented.
4. Build on the completed permit-consuming plugin-browser and modern Streamable HTTP plus catalog-bound legacy HTTP/current-or-legacy stdio MCP paths, then migrate hook/general-browser/computer/general-process/Codex/remote executors behind persistent schema-61 Tool permits and extend bounded Root/CLI/Simple/Advanced/Developer authority inspection to each family. No-capability WASI execution and source-bound task-project reads, creates, preimage-bound replacements, exact-executable process requests, exact-origin HTTP requests, exact-origin ephemeral browser workflows, and all supported MCP calls already consume exact Root permits; writes have exact remove/restore compensation and restart-safe read-only reconciliation, while ambiguous process, mutating HTTP, browser, or MCP effects require manual review and no redispatch. Add stronger optional platform process/browser sandboxes and parent-task plugin-to-Verify/CompletionCertificate E2E without treating the adapter itself as a model invocation. In parallel, extend the governed provider path with external OAuth/provider smoke, redaction/security E2E, installed-package MCP/browser qualification, and cross-platform product qualification.
5. Build and pass the Stage 7 Protected Evaluator and Phase 0 integrity gate before beginning recursive improvement work.
6. Continue through Stages 8–13 only behind the recorded scientific and production gates, with matched Windows/macOS/Linux package and native UAT evidence at every affected boundary.

## 11. Final Completion Definition

The new PRD alignment is not complete merely because the GUI exposes cognitive concepts or because model output resembles adaptive behavior. It is complete only when:

- Root, Runtime, and Evaluator ownership is enforced by real process, storage, IPC, and OS boundaries;
- durable work and consequential effects survive interruption without duplication;
- material invocations are reconstructable;
- persistent cognition improves fresh work without collapsing evidence, belief, exposure, provenance, or authority;
- protected evaluation can reject invalid or contaminated experiments;
- promotion and rollback are causal, bounded, monitored, and Root-controlled;
- Simple, Advanced, and Developer present the same underlying runtime faithfully;
- automated, E2E, native UAT, migration, package, and cross-platform release gates pass;
- unsupported scientific claims and mechanisms are explicitly killed, narrowed, deferred, or removed.
