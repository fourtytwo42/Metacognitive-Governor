# New PRD Implementation Status Report

**Audit date:** 2026-08-30

**Branch:** `codex/v34-updater-version-binding`

**Repository implementation checkpoint:** `65f7073` (`Interrupt active runtime invocations safely`)

**Prior Root/CLI checkpoint:** `28de9fb` (`Expose canonical actions through Root IPC and CLI`)

**Audit boundary:** the completed Stage 4 checkpoint plus Stage 5 reconstruction contracts, schemas 56–58, the separate authenticated Runtime worker, Root-owned plan-before-dispatch orchestration, exact result fencing, immutable responses, real Work/Verify completion, recovery tests, authenticated Root/CLI/Desktop reconstruction, and adjacent-sidecar package contracts. The concurrent untracked `STAGE_3_DURABLE_WORKFLOW.md` is excluded from this audit's changes and commits.

**Specifications audited:** Adaptive Cognitive Agent Runtime PRD v1.4.2, Cognitive Runtime Implementation Contract v0.1, Cognitive Runtime Experimental Protocol Specification v0.2, the staged implementation plan, ADRs, traceability matrix, current source, tests, and durable repository memory.

## Executive assessment

The new PRD is **not production-complete**. MetaCog has completed the architectural foundation, stable contracts, Protected Root implementation, and substantial workflow/action slices. Stage 5 now includes complete bounded reconstruction DTOs, Root-owned schemas 56–58, a dependency-isolated authenticated Runtime worker, plan-before-dispatch persistence, attempt/revision/lease/generation-fenced result acceptance, immutable response retention, real independent Work/Verify completion, conservative crash settlement, authenticated Root/CLI/Desktop reconstruction, generation-safe idle service replacement, and sidecar package contracts. The global workflow/invocation browser, the Root-permitted provider/tool pipeline, Protected Evaluator, persistent cognition, bounded improvement, culture, and longitudinal research layers remain to be built.

The current state is best summarized as:

- **Stage 0:** substantively complete, but its checklist document is stale.
- **Stage 1:** implementation complete; cumulative packaged compatibility/UAT remains.
- **Stage 2:** implementation complete; two owner-bound native package lanes and the final cumulative release gate remain.
- **Stage 3:** canonical kernel, Root IPC/CLI, Desktop projection, and objectively provable dependency/backoff recovery are pushed. The complete recovery/event/process/package gate remains open.
- **Stage 4:** the schema-55 ledger, post-effect validation/commit, causal compensation, elevated irreversible authority, filesystem runner, bounded Root client, exact dispatch/reconciliation/recovery, bounded authenticated action list/show IPC, CLI inspection, progressive Desktop inspection, immutable source-binding foundation, real-child exit/timeout/kill/reap coverage through durable Root reopen and inspection-only reconciliation, and package contracts are implemented and focused-test green. Compatibility producers and exact approval-source focusing, packaged crash and broader adversarial coverage, fresh packages, and native qualification remain open.
- **Stage 5:** reconstruction contracts, Root persistence, separate worker, production Root client, real-process Work/Verify recovery, authenticated Root/CLI/Desktop reconstruction, generation-safe idle service replacement, active invocation cancellation/late-result rejection, and package contracts are implemented and cumulative-test green; global-browser inspection, real-provider integration, native packages, and release gates remain open.
- **Stages 6–13:** not implemented as the new canonical cognitive-runtime architecture. Existing MetaCog capabilities provide useful compatibility foundations, but they do not satisfy these stages' new authority, evaluation, or experimental requirements.

The traceability ledger is useful as a requirements inventory but is not a completion percentage. This report classifies evidence directly from current source, pushed history, and freshly run tests.

## Stage-by-stage status

| Stage | Current status | Complete now | Still required |
|---|---|---|---|
| 0 — Baseline and architecture freeze | **Substantively complete; documentation reconciliation required** | Hook-trust prerequisite released and pushed; three-interface decision accepted; current-to-target map, staged plan, traceability matrix, and ADR-001 through ADR-014 exist. | Reconcile the unchecked Stage 0 exit checklist with actual evidence and record a single current baseline receipt. |
| 1 — Stable contracts and canonical encoding | **Implementation complete** | `mcg-contracts`; stable IDs/discriminators; canonical CBOR/normalization; domain-separated hashes; golden fixtures; upcasters; shared error/redaction contracts; dependency boundary; Simple/Advanced/Developer setting and legacy migration. | Final cross-version/package replay and final packaged migration/UAT on all supported platforms. |
| 2 — Protected Root and authoritative storage | **Code complete; release gate open** | Real `mcg-protected-rootd`; exclusive authoritative-store ownership; migration backup/restore; Root identity; bounded authenticated local IPC; Windows named pipes; Unix sockets; capability/session/replay/version checks; Root-owned compatibility adapter; `mcg runtime status/inspect`; GUI Root inspection. | Exact Debian install/repair/restart/native/uninstall UAT with owner-entered sudo; macOS owner-authenticated provider/restart/native UAT; final cumulative workspace, Desktop, package, migration, and release-smoke rerun. Trusted production signing/notarization remains external. |
| 3 — Durable workflows, leases, and recovery | **Substantially implemented; release gate open** | Schema 53; immutable TaskContract versions/heads/dependencies; head revision CAS; WorkflowAttempts and ordered events; legal transitions; terminal monotonicity; generation-fenced claim/heartbeat/cancel/retry; dependency gating; exact idempotent replay; Root startup recovery for expired Running/Verifying work and objectively satisfied dependency/retry-backoff waits; authenticated Root workflow list/show/cancel IPC; `mcg workflow list/show/cancel`; deterministic Simple/Advanced/Developer Desktop projection and E2E; pushed action-execution fencing during cancellation and effect-free cancellation settlement on restart. | Machine-readable event streaming; compatibility projection/reattachment for existing conversations/tasks/runs; child propagation; external/approval/protected-component wake triggers; reconciliation completion before cancellation settlement; kill/restart matrix for every nonterminal state, including forced-kill/reassignment cases not covered by Stage 5's delayed Runtime fixture; three-platform packaged UAT. |
| 4 — Governed actions and reconciliation | **Root lifecycle and IPC/CLI/Desktop inspection implemented; release gate open** | Schema 55 immutable ledger and generation-fenced execution/reconciliation; exact binding; explicit outcomes; validation-bound commit; causal compensation with bounded bidirectional Root/Desktop history traversal; elevated irreversible authority; action-aware cancellation; bounded adjacent runner client; authority-derived create/read/inspect/compensate; platform-stable lexical semantic paths; durable dispatch/outcome mapping; exact reconciliation; due worker; stranded-action recovery without redispatch; bounded content-minimized action summaries and full bounded evidence inspection through authenticated Root IPC; human and JSON `mcg action list/show`; Simple/Advanced/Developer Desktop projection over the same Root state; backward-compatible immutable conversation/invocation/command/approval/agent/evidence source binding; deterministic real-child pre/post-effect exit, timeout, kill/reap, late-work rejection, durable Root reopen, and inspection-only outcome resolution; bounded malformed/diagnostic/oversized-output handling; Windows Job Object/POSIX process-group confinement; package sidecar contracts. | Populate source bindings from compatibility/runtime producers and focus exact approval/invocation/command/evidence records; expose separately authorized owner-facing compensation initiation; rebuild/run native package UAT; run the real-child fault matrix against packaged processes; cross-platform native path/process qualification, case-collision/filesystem aliases, broader resource/network/sandbox policy; adversarial E2E. Add Root HTTP/API only if a non-local consumer requires it. |
| 5 — Runtime worker and reconstructable cognition | **Root-owned real-process Work/Verify, reconstruction, service replacement, and active cancellation implemented; release gates open** | Complete bounded reconstruction contracts and canonical fixture; schemas 56–58 immutable manifests/certificates/responses, append-only model-run history, and durable worker generations; dependency-isolated authenticated worker; production adjacent-child Root client; plan-before-dispatch persistence; exact attempt/revision/lease/generation/result fencing; conservative unknown outcomes; real Work→Verify→certificate→Succeeded path; death/timeout/late/duplicate/replay/Root-reopen coverage; fail-closed dead/poisoned/expired idle worker replacement; active delayed Work cancellation with workflow-first fencing, exact-child kill/reap, `Cancelled` model-run settlement, late-result refusal, and Root reopen; bounded global invocation summaries and Root/CLI/Desktop inspection; Windows/Linux/macOS sidecar package contracts. | Global workflow/agent browser; governed real-provider path; broader combined crash injection; packaged cancellation/replacement, native package, and GUI qualification. |
| 6 — Governed providers, tools, hooks, and plugins | **Not built as a Root-permit pipeline** | Existing hook trust is complete, including exact-hash trust and invocation-only `--dangerously-bypass-hook-trust`; MetaCog already has providers, tools, plugins, MCP, browser/computer, and Codex compatibility surfaces. | Route these capabilities through exact Root-bound egress/tool permits and opaque secret handles; enforce manifests, scopes, sandbox requirements, redaction, escalation resistance, and end-to-end CompletionCertificate production. Existing capability is not evidence that the new trust path exists. |
| 7 — Protected Evaluator and Phase 0 gate | **Not built** | Contracts and accepted ADRs only. | Separate protected evaluator process/store/artifacts; EvaluationSeed equivalence; immutable experiments; fixtures/TCB/exposure; signed disclosures; invalidation; bounded search; Root-only promotion; full Phase 0 integrity/adversarial gate. |
| 8 — Persistent epistemic cognition | **Not built** | Existing memories and project context are compatibility inputs only. | Versioned Records/Claims/Engrams; perspectives, exposure, provenance, common-ground derivation, progressive authority-aware retrieval, correction/forgetting, causal contribution tests, and lineage/fork behavior. |
| 9 — Skills, autonomy, and minimal metacognition | **Not built** | Existing skills and telemetry are not yet the PRD mechanism. | Validated Skills/Tool compilation, competence envelopes, SELF/WORLD/OTHER predictions, query-first self-model projection, calibration/ablation, coordination-value tests, and mechanism retention gates. |
| 10 — Bounded improvement and semantic rollback | **Not built** | ADRs and experimental contracts only. | ImprovementCases, bounded Change Grammar, candidate generation, exposure/taint/search accounting, evaluator-governed promotion, monitoring, causal credit, semantic rollback, deletion challengers, and Phase 4 gates. |
| 11 — Cultural transfer | **Not built** | None of the new authoritative transfer model is implemented. | CultureArtifact, attestations, local import/export, provenance/conflict handling, privacy approval, poisoning tests, and demonstrated fresh-task value. |
| 12 — Development and longitudinal research | **Not built** | Protocol specification only. | Matched forks, transplantation/replacement/lesion protocols, evidence-level reporting, evaluator-only continuity measurements, carryover controls, replication, and falsification gates. |
| 13 — Optional evolution/challengers | **Intentionally deferred** | Correctly gated behind earlier stages. | Consider only after Stage 10/Phase 4 succeeds and a mechanism demonstrates product or scientific rent. |

## Completed foundations in more detail

### Hook trust and three interface complexities

The P0 hook-trust cycle is complete and pushed. Exact-hash trust, enable/disable behavior, managed-hook immutability, stale-hash refusal, lifecycle activity, project-aware CLI/Desktop inspection, and the visibly warned invocation-only bypass are implemented. The bypass is not a durable GUI preference.

Simple, Advanced, and Developer are implemented as presentation levels over the same runtime and durable state. Legacy `advanced_mode=false` migrates to Simple, `true` migrates to Advanced, and migration never silently selects Developer. Developer adds inspection and controls but does not grant database writes, secret extraction, protected fixture access, self-promotion, or a persistent trust bypass.

### Protected Root

Protected Root is a real executable and the authoritative storage owner. Its migration and local IPC boundaries have automated and native process evidence on Windows, Linux, and macOS. Windows installed-package qualification and Linux AppImage qualification are strong; Debian's root-required package mutation lane and the macOS authenticated provider lane remain deliberately unweakened.

### Canonical durable workflow kernel

The pushed Stage 3 commits are:

- `466d27d` — canonical durable workflow kernel;
- `41c1ed3` — atomic TaskContract revision/head CAS;
- `293d34d` — authenticated Root workflow IPC and CLI projection;
- `3c16346` — Simple/Advanced/Developer Desktop workflow projection;
- `24b3de4` — objective dependency and retry-backoff wait recovery.

The kernel includes exact historical snapshots, ordered correlation/causation/idempotency evidence, concurrent-claim protection, stale-generation rejection, retry ancestry, dependency gating, and deterministic startup handling for expired Running and Verifying attempts. The canonical TaskContract fixture hash is `18b0ba14d68d758351d78d11c1d6a0e9839bd509930f1d3ba27a718e51918c32`.

## Current implementation checkpoint

The pushed schema-55 and governed-runner checkpoints add:

- stable governed-action, effect-outcome, and reconciliation contracts;
- immutable action intent, receipt, and event storage plus mutable generation-fenced execution state;
- atomic authorization that rejects operation, authority, budget, actor, workflow-revision, generation, and idempotency substitution;
- separate dispatch, observation, and outcome-classification phases;
- durable `OUTCOME_UNKNOWN` handling with reconciliation-job creation and no blind redispatch;
- bounded due-job discovery, exact revision/generation claims, and independent reconciler leases;
- typed `CONFIRMED_NOT_OCCURRED`, `CONFIRMED_OCCURRED(receipt)`, and `STILL_UNKNOWN` observations;
- safe reconciliation deferral, immutable confirmed-occurrence receipts, deadline escalation, and real-store reopen recovery;
- action-aware cancellation: pre-dispatch or observational work can settle `NO_EFFECT`, while uncertain effectful work moves to reconciliation;
- Root restart fencing of expired reconcilers, settlement for effect-free cancellation, and durable waiting for uncertain effects;
- exact post-effect validation evidence and validation-event-bound authoritative commit;
- compensation as a new exactly bound action whose successful commit atomically marks—but never erases—the original;
- separate hash-bound elevated authority for irreversible effects;
- schema-50→55 and schema-54→55 migration coverage.

Checkpoint `abce753` additionally adds:

- a bounded adjacent-binary client with fixed executable identity, cleared environment, bounded request/response/diagnostics, timeout, kill/reap behavior, and strict response identity validation;
- a client-only dependency split that excludes filesystem executor capabilities from Protected Root;
- a fixed, hash-bound filesystem authority that binds the action, actor, capability root, operation, effect class, and expiration;
- request derivation from the immutable authorized ActionEnvelope and current execution rather than a caller-supplied filesystem root;
- Root-owned durable dispatch and classification for confirmed effects, observational/no-effect outcomes, lost or invalid post-launch responses, and proven pre-launch failures;
- Root-owned generation-fenced reconciliation that invokes only the committed inspection adapter and resolves exact occurrence/non-occurrence while preserving changed owner content as unknown;
- a bounded due-job worker in the Root process and startup recovery that never redispatches stranded effects;
- focused real-store reopen coverage showing confirmed effects, lost-after-effect, lost-without-effect, ambiguous changed-file, launch-failure, and stranded-dispatch behavior.

Checkpoint `28de9fb` adds bounded canonical `ActionSummary`/`ActionInspection` contracts, storage projections derived from authoritative rows, authenticated and correlated Root action list/show IPC, content-minimized human CLI presentation, and explicit full-evidence JSON output.

Checkpoint `9b881dd` adds Tauri and TypeScript Root action list/show clients and one canonical Desktop state projected progressively across all three interface levels. Simple shows only owner-facing identity and plain-language state; Advanced adds effect/revision/generation/actor/operation/reconciliation details and source-workflow evidence loading; Developer adds read-only exact intent, execution, receipt, reconciliation, correlation, and causation evidence. The deterministic browser journey proves Simple → Advanced → Developer → Simple preserves the same action state and passes a scoped Axe accessibility scan.

Checkpoint `cccdee0` adds a backward-compatible immutable `ActionOrigin` for conversation, invocation, command, approval, agent, and evidence `SourceRef` bindings. Storage authorization rejects empty/oversized/duplicate/malformed sources and replay substitution, while inspection preserves valid bindings and older envelopes decode with no origin. Advanced and Developer show the exact bound locators/hashes; a conversation control stays disabled when the exact compatibility record is absent rather than guessing another source.

Checkpoint `b544b1a` adds safe human CLI origin inspection. `mcg action show` prints every bound source type, locator, hash domain, and full digest without dumping policy/authority payloads; explicit JSON continues to expose the complete canonical envelope. Its formatter has focused unit coverage.

Fresh audit verification on 2026-08-30:

- `cargo test -p mcg-contracts` — current checkpoint 16/16 passed;
- `cargo test -p mcg-storage -- --test-threads=1` — current checkpoint 108/108 passed;
- `cargo test -p mcg-protected-root --all-targets` — 11/11 passed (10 unit and 1 real-process integration covering four fault/restart paths);
- `cargo test -p mcg-runtime` — 108 passed, with two explicit Windows Sandbox package qualifications ignored;
- `cargo test -p mcg-protected-rootd --test migration_process` — 1/1 passed.
- `cargo test -p mcg-action-runner --all-targets -- --test-threads=1` — current working tree 18/18 passed (6 unit and 12 separate-process tests), including platform-stable semantic-path rejection, pre/post-effect exit, timeout, whole-process-tree kill/reap, inherited-pipe deadlines, bounded malformed/diagnostic/oversized output, and late-work rejection.
- `cargo clippy -p mcg-action-runner --all-targets -- -D warnings` — passed.
- `cargo clippy -p mcg-action-runner --no-default-features --all-targets -- -D warnings` — passed for the client-only dependency shape.
- `cargo check -p mcg-protected-root --all-targets` — passed with the current local integration.
- `cargo test -p mcg-platform` — 2/2 passed; strict contracts/platform all-target Clippy passed.
- `cargo test -p mcg-cli` — 68 unit and 10 integration passed.
- `cargo test -p mcg-desktop` — 20/20 passed.
- Desktop Vitest — 124/124 passed.
- Serialized Playwright — 31/31 passed, including governed-action and Runtime-invocation progressive disclosure and scoped accessibility.
- Desktop TypeScript and production frontend build — passed.
- `cargo check --workspace --all-targets` — passed.
- Windows debug sidecar preparation and copied-sidecar filesystem qualification — passed.

The Stage 4 checkpoint establishes the authoritative action storage kernel, real-filesystem adapter, bounded Root dispatch, exact reconciliation, restart-safe no-redispatch recovery, authenticated bounded action inspection through Root IPC, CLI, and Desktop, deterministic real-child fault recovery across a durable Root reopen, bounded hostile-output handling, whole-process-tree termination, platform-stable lexical path semantics, and owner-visible exact compensation history. Stage 4 remains open until exact approval-source navigation, separately authorized compensation initiation, native/case-collision path qualification, broader resource/network/sandbox policy, packaged fault qualification, native GUI UAT, and fresh packaged qualification pass.

Checkpoint `83b5629` begins Stage 5 without weakening Root ownership. Contracts and schema 56 persist exact ContextManifest and InvocationManifest hashes, append-only CAS-fenced ModelRun revisions, and CompletionCertificates whose Work/Verify invocations must resolve to completed role-correct runs under one attempt and TaskContract. Checkpoint `522ba92` adds the separate dependency-isolated worker, launch-only executable/generation capability, Windows named-pipe/Unix-socket protocol, distinct deterministic Work/Verify execution, and real-process rejection of wrong capabilities and executable substitution.

Checkpoint `4a2a905` adds the production Root client, schemas 57–58 immutable responses and monotonic worker generations, plan/dispatched/terminal persistence, exact workflow/lease/generation/result fencing, conservative post-dispatch ambiguity, real Work→Verify→CompletionCertificate settlement, full Root reopen reconstruction, and adjacent-sidecar package contracts. Platform 3/3, worker 11/11, storage 110/110, Protected Root 11/11, the protected-root migration process 1/1, strict changed-boundary Clippy, whole-workspace all-target check, documentation, packaging, PowerShell parsing, formatting, and diff gates pass. At that checkpoint, product inspection, active-service replacement, governed real-provider execution, and native package UAT remained open. The concurrent untracked `docs/NewPRD/STAGE_3_DURABLE_WORKFLOW.md` was not changed.

Checkpoint `bde06f0` adds stable content-minimized invocation summary/inspection contracts, a bounded Root-owned global query, authenticated correlated Root IPC list/show, IPC-specific response ceilings, and human/JSON `mcg invocation list/show`. Contracts 17/17, platform 3/3, storage 110/110, Protected Root 12/12, CLI 69 unit plus 10 integration, whole-workspace all-target check, strict boundary Clippy, packaging/docs/format/diff gates pass. At that checkpoint, Desktop progressive projection and the combined global workflow/agent browser remained open.

Checkpoint `1e94efb` adds Tauri and TypeScript invocation list/show clients and projects one bounded Root state through Simple, Advanced, and Developer. Simple exposes only Work/Verification identity and plain lifecycle state; Advanced adds exact invocation/workflow identity, provider/model, context counts, TaskContract version, hashes, run history, and source-workflow loading; Developer adds read-only canonical evidence while withholding prompt/response material, secrets, private reasoning, database access, and Root mutation authority. TypeScript/build, Desktop Rust 20/20, Vitest 124/124, serialized Playwright 31/31 with scoped Axe, whole-workspace check, packaging/docs/format/diff, and changed-boundary Clippy pass. The combined global workflow/invocation browser remains open.

Checkpoint `dc54a88` adds Root-owned active-service supervision for the exclusively held idle cognition worker. Root checks poison, process exit, and fixed session expiry; reaps the exact old child before allocating a new generation; leaves the slot empty on ambiguous failure; retries safe relaunch; and rejects stale-generation work before persistence. Real-process coverage proves no healthy churn, dead/unusable generation 1→2 replacement, stale generation-1 rejection, and expired generation 3→4 replacement. Runtime worker 12/12, Protected Root 12/12, whole-workspace check, strict changed-boundary Clippy, packaging/docs/format/diff gates pass. Packaged long-running service replacement remains a release qualification gate.

Checkpoint `65f7073` adds independent exact-child control so active cancellation cannot queue behind synchronous Work. Root binds cancellation to the exact attempt revision, lease generation, and worker generation; commits workflow cancellation first; kills/reaps the active child; settles the model run `Cancelled`; and refuses any late response artifact. A distinct unpackaged delayed worker proves the real process is interrupted in bounded time and full Protected Root reopen retains `Planned → Dispatched → Cancelled` while settling the workflow `Cancelled`. Runtime worker 13/13, Protected Root 12/12, whole-workspace check, strict changed-boundary Clippy, packaging/docs/format/diff gates pass.

## Highest-priority remaining build sequence

1. Populate immutable action origins from compatibility/runtime producers and implement exact focus for the originating conversation, invocation, agent, command, approval, and evidence; the binding and fail-closed Desktop presentation are implemented.
2. Complete Stage 3 compatibility projection, child cancellation, event streaming, external/approval/protected-component wake triggers, and the complete nonterminal restart matrix.
3. Extend the passing real-child pre/post-effect exit, response-loss, timeout, kill/reap, late-work, Root-reopen, and changed-target tests through packaged-process qualification.
4. Run deterministic CLI, Desktop, migration, combined real-effect crash-injection, and adversarial E2E for Stages 3–4.
5. Close the exact Debian sudo and macOS authenticated package lanes; rebuild and rerun the matched Windows candidate because product binaries and schema have changed.
6. Continue Stage 5 with the canonical workflow/invocation/agent browser and broader packaged crash E2E while preserving compatibility labels and `agents --remote` where applicable; idle service replacement, active cancellation, and Root/CLI/Desktop invocation inspection are implemented.
7. Implement Stage 6 Root-bound provider/tool/hook/plugin permits and complete security/redaction qualification.
8. Implement and pass Stage 7 protected evaluation before any recursive optimizer work.
9. Proceed through Stages 8–13 only when each preceding scientific and production gate is recorded as passing.

## Documentation drift to correct

The following documents cannot presently be treated as an exact live status dashboard:

- `STAGE_0_ALIGNMENT.md` still has unchecked exit items that repository memory and later evidence say are complete.
- The concurrent untracked `STAGE_3_DURABLE_WORKFLOW.md` must be reconciled by its owner against the pushed schema-53, Root IPC/CLI, Desktop, and wait-recovery implementation before it becomes release evidence.
- `REQUIREMENTS_TRACEABILITY.md` is current through Root-integrated deterministic real-child fault recovery; advance it again after producer integration/deep source focus, packaged crash qualification, and native package gates.
- The implementation plan now identifies active staged delivery; stage documents and the traceability ledger must continue to advance with each checkpoint.

Before the next release claim, reconcile those documents so every row is one of: implemented with evidence, partially implemented with an explicit remaining gate, superseded with a reason, unavailable through capability discovery, or not started.

## Release conclusion

MetaCog has a credible, well-tested foundation for the new PRD, but it is not production-complete. The protected contracts, three-level interface model, Root boundary, durable workflow kernel, substantial action/effect ledger, local filesystem runner, and Root-orchestrated reconstructable Runtime process are real. Production completion still requires the remainder of Stages 3–5, all canonical work in Stages 6–12, any justified Stage 13 mechanisms, and the complete matched Windows/macOS/Linux automated, crash/security E2E, native UAT, migration, packaging, signing/notarization, cleanup, commit, and push gates.
