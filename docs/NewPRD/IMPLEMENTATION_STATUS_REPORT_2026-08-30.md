# New PRD Implementation Status Report

**Audit date:** 2026-08-30

**Branch:** `codex/v34-updater-version-binding`

**Pushed checkpoint:** `293d34d` (`Expose canonical workflows through Root IPC and CLI`)

**Specifications audited:** Adaptive Cognitive Agent Runtime PRD v1.4.2, Cognitive Runtime Implementation Contract v0.1, Cognitive Runtime Experimental Protocol Specification v0.2, the staged implementation plan, ADRs, traceability matrix, current source, tests, and durable repository memory.

## Executive assessment

The new PRD is **not production-complete**. MetaCog has completed the architectural foundation and most of the first three delivery stages, but the protected action, runtime-worker, governed provider/tool, evaluator, persistent cognition, improvement, culture, and research layers remain to be built.

The current state is best summarized as:

- **Stage 0:** substantively complete, but its checklist document is stale.
- **Stage 1:** implementation complete; cumulative packaged compatibility/UAT remains.
- **Stage 2:** implementation complete; two owner-bound native package lanes and the final cumulative release gate remain.
- **Stage 3:** canonical kernel and Root IPC/CLI are implemented; Desktop projection is uncommitted work in progress and the full recovery/cancellation/process/package gate remains.
- **Stages 4–13:** not implemented as the new canonical cognitive-runtime architecture. Existing MetaCog capabilities provide useful compatibility foundations, but they do not satisfy these stages' new authority, persistence, reconstruction, evaluation, or experimental requirements.

The traceability ledger currently contains 110 assessable requirement/evidence rows: 80 are still marked `Planned` and 30 contain some implementation or partial evidence. This is an inventory count, not a completion percentage; later requirements are substantially larger than early contract rows, and several Stage 3 rows are stale relative to the current code.

## Stage-by-stage status

| Stage | Current status | Complete now | Still required |
|---|---|---|---|
| 0 — Baseline and architecture freeze | **Substantively complete; documentation reconciliation required** | Hook-trust prerequisite released and pushed; three-interface decision accepted; current-to-target map, staged plan, traceability matrix, and ADR-001 through ADR-014 exist. | Reconcile the unchecked Stage 0 exit checklist with actual evidence and record a single current baseline receipt. |
| 1 — Stable contracts and canonical encoding | **Implementation complete** | `mcg-contracts`; stable IDs/discriminators; canonical CBOR/normalization; domain-separated hashes; golden fixtures; upcasters; shared error/redaction contracts; dependency boundary; Simple/Advanced/Developer setting and legacy migration. | Final cross-version/package replay and final packaged migration/UAT on all supported platforms. |
| 2 — Protected Root and authoritative storage | **Code complete; release gate open** | Real `mcg-protected-rootd`; exclusive authoritative-store ownership; migration backup/restore; Root identity; bounded authenticated local IPC; Windows named pipes; Unix sockets; capability/session/replay/version checks; Root-owned compatibility adapter; `mcg runtime status/inspect`; GUI Root inspection. | Exact Debian install/repair/restart/native/uninstall UAT with owner-entered sudo; macOS owner-authenticated provider/restart/native UAT; final cumulative workspace, Desktop, package, migration, and release-smoke rerun. Trusted production signing/notarization remains external. |
| 3 — Durable workflows, leases, and recovery | **Partially complete** | Schema 53; immutable TaskContract versions/heads/dependencies; head revision CAS; WorkflowAttempts and ordered events; legal transitions; terminal monotonicity; generation-fenced claim/heartbeat/cancel/retry; dependency gating; exact idempotent replay; Root startup recovery for expired Running/Verifying work and objectively satisfied dependency/retry-backoff waits; authenticated Root workflow list/show/cancel IPC; `mcg workflow list/show/cancel`; deterministic Desktop projection/E2E. | Machine-readable event streaming; compatibility projection/reattachment for existing conversations/tasks/runs; child and external-effect-aware cancellation; external/approval/protected-component wake triggers; kill/restart matrix for every nonterminal state; delayed-worker/forced-kill tests; three-platform packaged UAT. |
| 4 — Governed actions and reconciliation | **Not built in the new architecture** | ADR and initial Stage 1 DTO vocabulary exist; legacy governed integrations provide reusable implementation knowledge. | Durable intent/authorization/execution/receipt chain; exact operation binding; explicit effect outcome classification; reconciliation and compensation; sandbox capability profiles; exact approval-source navigation; crash/security/E2E/UAT matrix. |
| 5 — Runtime worker and reconstructable cognition | **Not built** | Existing agent/runtime code and role terminology offer compatibility inputs. | Separate `runtime-worker`; authenticated Root-to-Runtime protocol; deterministic fake Work/Verify path; complete ContextManifest/InvocationManifest/CompletionCertificate persistence; invocation reconstruction; canonical global workflow/invocation browser; process-death and late/duplicate-response gates. |
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
- `293d34d` — authenticated Root workflow IPC and CLI projection.

The kernel includes exact historical snapshots, ordered correlation/causation/idempotency evidence, concurrent-claim protection, stale-generation rejection, retry ancestry, dependency gating, and deterministic startup handling for expired Running and Verifying attempts. The canonical TaskContract fixture hash is `18b0ba14d68d758351d78d11c1d6a0e9839bd509930f1d3ba27a718e51918c32`.

## Current uncommitted work

The worktree is not a release-clean checkpoint. Five Desktop files currently contain an uncommitted Stage 3 projection:

- Tauri commands for workflow list/show/cancel;
- TypeScript canonical workflow contracts and client calls;
- Simple lifecycle labels, Advanced lease/attempt/criteria inspection, and Developer raw canonical evidence;
- revision/generation-fenced cancellation confirmation;
- focused component coverage and layout styling.

Audit verification of that uncommitted slice:

- `cargo check -p mcg-desktop --all-targets` — passed;
- focused Vitest `durable workflows progressively disclose Root evidence and fence cancellation` — passed;
- complete Desktop Vitest — 122/122 passed;
- complete deterministic Playwright — 29/29 passed, including one-Root-truth Simple/Advanced/Developer projection and revision/generation-fenced cancellation;
- native GUI UAT, packaged UAT, and final release qualification — not yet demonstrated for this slice.

`docs/NewPRD/STAGE_3_DURABLE_WORKFLOW.md` is also untracked concurrent-agent work and was not changed by this audit.

## Highest-priority remaining build sequence

1. Finish and verify the Stage 3 Desktop projection without mixing or overwriting concurrent work.
2. Complete child/effect-aware cancellation, waiting-condition recovery, existing-work projection, event streaming, and the complete nonterminal restart matrix.
3. Run deterministic process, CLI, Desktop, crash, migration, and adversarial E2E for Stage 3.
4. Close the exact Debian sudo and macOS authenticated Stage 2/3 package lanes; rerun the matched Windows candidate because product binaries have changed since the last package evidence.
5. Implement Stage 4 governed effects and approval-source navigation before adding new autonomous execution paths.
6. Implement Stage 5 runtime-worker separation and replace the global-agent-browser concept with the canonical workflow/invocation browser while preserving compatibility labels and `agents --remote` where applicable.
7. Implement Stage 6 Root-bound provider/tool/hook/plugin permits and complete security/redaction qualification.
8. Implement and pass Stage 7 protected evaluation before any recursive optimizer work.
9. Proceed through Stages 8–13 only when each preceding scientific and production gate is recorded as passing.

## Documentation drift to correct

The following documents cannot presently be treated as an exact live status dashboard:

- `STAGE_0_ALIGNMENT.md` still has unchecked exit items that repository memory and later evidence say are complete.
- `STAGE_3_DURABLE_WORKFLOW.md` says canonical persistence, IPC, and CLI do not exist, but the pushed Stage 3 commits now implement them.
- `REQUIREMENTS_TRACEABILITY.md` still labels several Stage 3 API/CLI/interface items as pending even though list/show/cancel and the Desktop work-in-progress now exist.
- The implementation plan still says “Proposed staged delivery plan,” although Stages 0–3 are already underway.

Before the next release claim, reconcile those documents so every row is one of: implemented with evidence, partially implemented with an explicit remaining gate, superseded with a reason, unavailable through capability discovery, or not started.

## Release conclusion

MetaCog has a credible, well-tested foundation for the new PRD, but it is still early in the total program. The protected contracts, interface model, Root boundary, and durable workflow kernel are real. Production completion still requires the remainder of Stage 3, all canonical work in Stages 4–12, any justified Stage 13 mechanisms, and the complete matched Windows/macOS/Linux automated, E2E, native UAT, migration, packaging, signing/notarization, cleanup, commit, and push gates.
