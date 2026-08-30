# Cognitive Runtime Requirements Traceability

**Status:** Active implementation matrix; Stages 1–2 implementation complete with owner-bound package lanes open, and the Stage 3 durable-workflow kernel in progress
**Purpose:** Map normative requirements to implementation stages, automated evidence, E2E/UAT evidence, and release artifacts

This matrix is updated as implementation lands. `Implemented / cumulative UAT pending` means local implementation and its stage-level automated evidence pass, while final native/cross-platform evidence remains open.

## 1. Core Architecture and Ownership

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Root, Runtime, and Evaluator are separate backend trust-domain processes | 2, 5, 7 | Dependency/process tests | Kill/probe each process independently | Planned |
| Root is sole authoritative `state.db` writer | 2 | Exclusive owner/DB access tests | Runtime/UI direct-write attack | Root process ownership implemented / attack and package evidence pending |
| Evaluator is sole `evaluation.db` writer | 7 | Protected store ownership tests | Root/Runtime fixture/store attack | Planned |
| Runtime proposes cognition but cannot commit authority | 2–5 | Authority/property tests | Proposed mutation must cross Root | Planned |
| UI is presentation/approval carrier, not authority owner | 2–4 | Tauri/API boundary tests | Client-side tamper/stale approval UAT | Stage 2 read-only Root inspection implemented / later approval gates pending |
| Evaluator cannot activate a configuration | 7 | Promotion transaction tests | Forged decision/activation attempt | Planned |
| Production crates cannot depend on research implementation | 1, 7 | Cargo dependency rule | Packaging inspection | Stage 1 boundary implemented / Stage 7 pending |
| Runtime remains safe when treated as compromised within its OS identity | 2–7 | Security/adversarial suite | Protected path, DB, secret, IPC attacks | Planned |

## 2. Persistence and Compatibility

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Stable type/message discriminators | 1 | Golden fixtures | Cross-version process exchange | Implemented / cumulative UAT pending |
| Versioned canonical encoding and normalization | 1 | Codec/property tests | Old fixture replay | Implemented / cumulative UAT pending |
| Domain-separated canonical hashes | 1 | Golden hash tests | Upgrade verification | Implemented / cumulative UAT pending |
| Historical payload bytes remain interpretable | 1 onward | Decoder/upcaster corpus | Multi-version migration | Stage 1 chain implemented / corpus grows by stage |
| Old canonical history is not rewritten during ordinary migration | 1, 2 | Byte-for-byte migration assertions | Backup/upgrade/restore | Verified interruption/retry/restore process fixture passes / packaged upgrade pending |
| Root and Evaluator maintain separate canonical histories | 7 | Cross-store reference tests | Disclosed-summary inspection | Planned |
| Large immutable artifacts use verified atomic content-addressed storage | 2, 7 | Temp/flush/hash/rename tests | Crash during artifact creation | Planned |
| Derived indexes are rebuildable and non-authoritative | 8 | Delete/rebuild equivalence | Search before/after rebuild | Planned |
| Existing MetaCog data migrates into a default runtime lineage | 2 | Migration fixtures | Packaged upgrade UAT | Schema 50→55, interruption retry, verified restore, data preservation, stable Root identity, and current migration-process tests pass / current-source packaged UAT pending |
| Unsafe downgrade fails clearly | 2 | Version downgrade tests | Old package against new data copy | Newer schema refusal implemented / old-package UAT pending |

## 3. IPC and Process Security

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Windows named pipes and Unix-domain sockets | 2 | Platform IPC tests | Native Windows/macOS/Linux process E2E | Implemented / native process E2E passes on all three platforms |
| Versioned length-framed bounded protocol | 1, 2 | Frame/property tests | Cross-process compatibility | Implemented / cumulative cross-platform UAT pending |
| OS peer identity and per-launch capability | 2 | Wrong-peer/session tests | Forged client attack | Capability/session rejection implemented / cross-platform peer UAT pending |
| Replay/idempotency protection | 2–4 | Duplicate request tests | Replayed action request | Root request-ID replay rejection plus durable exact workflow cancellation replay/substitution rejection implemented / action coverage grows in Stage 4 |
| Oversized/malformed/unknown-version messages fail closed | 2 | Fuzz/boundary tests | Hostile client process | Boundary/version rejection implemented / hostile-process UAT pending |
| Cancellation and late responses are safe | 3–5 | State/process tests | Cancel during execution | Workflow and action generations are fenced; pre-dispatch/observational work settles no-effect; dispatched effectful work becomes `OUTCOME_UNKNOWN`; late reconcilers are rejected; authenticated IPC/CLI/Desktop cancellation passes / child propagation, real-effect process E2E, and package UAT pending |
| Root/Runtime/Evaluator death has an explicit restart story | 3–7 | Crash matrix | Packaged kill/restart UAT | Root startup scanner handles expired Running/Verifying leases / complete process and action matrix pending |

## 4. Workflow and Leases

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Durable `TaskContract` and `WorkflowAttempt` | 3 | Transition/unit tests | Create, restart, inspect | Schema 53 immutable contract versions/heads/dependencies, revision-head CAS with exact replay, attempts, ordered events, atomic v1 Ready creation, authenticated Root IPC/CLI inspection, and deterministic Desktop inspection implemented / process-restart E2E pending |
| Only legal workflow transitions occur | 3 | Generated state-machine tests | Invalid transition through API/CLI | Exhaustive contract table, Root-owned revision/generation CAS transitions, authenticated IPC, CLI parser/process coverage, and Desktop cancellation fencing pass / packaged invalid-transition UAT pending |
| Terminal workflows do not ordinarily reactivate | 3 | Property tests | Retry creates explicit new attempt | Terminal monotonicity and explicit parent-linked retry attempts pass / cumulative process UAT pending |
| Generation-fenced leases prevent stale commit | 3 | Race/property tests | Kill owner, reassign, deliver stale result | Claim/heartbeat/cancel/recovery CAS, concurrent-claim winner, and stale-result tests pass / delayed-worker process E2E pending |
| Restart scanner resumes only satisfied work | 3 | Recovery integration | Process/application restart | Root startup fences expired Running work, requeues expired Verifying work, resumes only objectively satisfied dependency/retry-backoff waits, fences expired reconciliation workers without redispatch, escalates elapsed reconciliation deadlines, and settles effect-free cancellations / external trigger and full forced-process/package matrix pending |
| Cancellation is durable and propagated | 3–5 | Cancellation tests | UI/CLI cancel and restart | Durable CancelRequested fencing plus authenticated `mcg workflow cancel`, Desktop cancellation, action-effect fencing, reconciliation waiting, and post-reconciliation settlement pass deterministic tests / child propagation, actual-effect crash E2E, native, and packaged UAT pending |
| UI reconnect cannot alter backend truth | 3 | Projection tests | Close/reopen during work | Simple/Advanced/Developer deterministic component/browser E2E projects one unchanged Root attempt and cancellation result / real process/application reconnect and packaged UAT pending |

## 5. Actions and External Effects

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Intent is durable before an external effect | 4 | Crash injection | Kill immediately around effect | Schema-54 immutable envelope plus atomic Authorized execution/event implemented and tested / actual runner boundary crash injection pending |
| Authorization binds the exact normalized operation | 4 | Substitution/hash tests | Change target after approval | Canonical operation hash, authority, budget, task-contract, workflow revision/generation, and actor substitution are rejected / approval UI and process attack pending |
| Execution and observation are distinct | 4 | State tests | Dropped receipt scenario | Separate mutable execution and immutable receipt/event persistence implemented; lost-receipt path becomes unknown / real external adapter pending |
| `NO_EFFECT`, `EFFECT_CONFIRMED`, and `OUTCOME_UNKNOWN` are explicit | 4 | Outcome classification tests | Native reconciliation display | Typed outcomes and legal execution transitions implemented / Root IPC/CLI/Desktop projection pending |
| Consequential unknown effects are not blindly retried | 4 | Crash/property tests | Effect succeeds, receipt lost | Dispatch is legal only from Authorized; unknown/reconciling states reject dispatch; late reconciler generations are fenced; restart requeues reconciliation only / actual exactly-once filesystem E2E pending |
| Reconciliation resolves or escalates ambiguity | 4 | Reconciler tests | Restart and confirm existing file | Due discovery, generation-fenced claim, confirmed occurred/not-occurred, safe defer, real-store reopen recovery, deadline escalation, immutable receipts/events, and cancellation settlement implemented / filesystem reconciler process and native presentation pending |
| Compensation is a new causal action | 4 | Ledger tests | Revert visible in history | Schema 55 binds one separately authorized compensation envelope to the exact committed original plan; the original becomes Compensated only atomically after the causal action commits, and both immutable histories remain / real reversible-write UAT pending |
| Irreversible actions receive stronger authority treatment | 4 | Policy tests | Approval UAT | Irreversible authorization requires a separately hash-bound elevated-authority policy and exact reconciler/deadline / policy-engine threshold, explicit UI, and native approval UAT pending |
| Read-only operations cannot mutate state | 4 | Snapshot/security tests | Governed read attack | Planned |

## 6. Model Invocation, Context, and Data Egress

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Every material call has a reconstructable `InvocationManifest` | 5 | Manifest completeness tests | CLI reconstruction | Planned |
| Exact context refs are captured in `ContextManifest` | 5, 8 | Context tests | Developer inspector | Planned |
| Remote model calls require Root-bound egress permit | 6 | Permit/policy tests | Unauthorized destination/context attack | Planned |
| Runtime receives no ambient provider secrets | 6 | Secret scanning/process tests | Compromised Runtime probe | Planned |
| Secret handles resolve only for exact authorized execution | 6 | Gateway tests | Wrong provider/action handle | Planned |
| Provider retry respects effect and idempotency semantics | 6 | Fake provider fault tests | Dropped/late stream | Planned |
| Resolved provider/model/config fingerprint is persisted | 5, 6 | Manifest tests | Run inspection | Planned |
| Private prompts, scratch reasoning, and secrets are excluded from generic telemetry | 5, 6 | Redaction tests | Logs/backups/UI inspection | Planned |

## 7. Tools, Skills, Capabilities, and Sandboxing

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Tool arguments pass schema, semantic, scope, effect, and authority validation | 4, 6 | Validator/property tests | Hostile model/tool request | Planned |
| Tool processes receive only exact permitted resources | 4, 6 | Capability tests | Filesystem/network/secret escape | Planned |
| Sandbox capability is `HARD`, `BEST_EFFORT`, or `UNAVAILABLE` | 4 | Platform tests | Native capability display | Planned |
| Required unavailable isolation fails closed | 4 | Policy/platform tests | Run on insufficient backend | Planned |
| Skills are validated procedures, not ambient arbitrary code | 6, 9 | Manifest/runner tests | Skill invocation inspection | Planned |
| Stable deterministic procedures may compile to Tools | 9 | Equivalence tests | Skill/Tool comparison | Planned |
| Hooks, plugins, MCP, browser, computer, and process tools cannot expand authority | 6 | Adversarial suites | Cross-capability escalation attempts | Planned |

## 8. Protected Evaluation and Promotion

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Baseline/candidate begin from equivalent EvaluationSeed | 7 | Seed/hash equivalence | Deterministic A/B demo | Planned |
| ExperimentSpec is immutable and names execution conditions | 7 | Mutation/replay tests | Developer experiment inspection | Planned |
| Learning mode is explicit and enforced | 7 | Cross-fixture carryover tests | Experiment run comparison | Planned |
| Exposure and taint follow material information/search lineage | 7, 10 | Provenance tests | Contamination laundering attack | Planned |
| Undeclared relevant divergence invalidates experiment | 7 | Divergence tests | Mandatory failure demo | Planned |
| Search breadth is mechanically countable | 7, 10 | Accounting/property tests | Search receipt inspection | Planned |
| Hard stopping ceilings cannot be negotiated away | 7, 10 | Adversarial tests | Candidate overrun attempt | Planned |
| Decision supports promote/reject/continue/inconclusive/invalid | 7 | State tests | UI/CLI decision display | Planned |
| Stale candidates rebase before promotion | 7, 10 | CAS/rebase tests | Champion changes mid-test | Planned |
| Only signed/hash-bound disclosed summaries cross boundary | 7 | Signature/disclosure tests | Root verification | Planned |
| Only Root activates promotion | 7 | Authority tests | Evaluator/Runtime activation attack | Planned |
| Exposed tests retire from independent status | 7 | Exposure lifecycle tests | Repeat exposed evaluation | Planned |

## 9. Persistent Cognition

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Events remain distinct from Claims and Engrams | 8 | Schema/lifecycle tests | Developer knowledge inspection | Planned |
| Claims are perspective-indexed | 8 | Partition/property tests | False-belief scenario | Planned |
| Exposure remains distinct from belief | 8 | Retrieval/inference tests | Asymmetric information task | Planned |
| ExperienceMode records acquisition provenance | 8 | Provenance tests | Source-monitoring UAT | Planned |
| Actor goals/constraints use existing record machinery | 8 | Query tests | OTHER prediction task | Planned |
| Common ground is derived rather than authoritative duplicate state | 8 | Derivation tests | Multi-actor task | Planned |
| Engrams are selective, revisable, mergeable, and forgettable | 8 | Lifecycle/property tests | Memory correction/removal UAT | Planned |
| Memory contribution is measured causally where practical | 8 | Ablation tests | Fresh homologous task comparison | Planned |
| Retrieval is progressive and authority-aware | 8 | Ranking/access tests | Missing/denied context presentation | Planned |
| Runtime forks retain lineage and separate post-fork experience | 8, 12 | Fork tests | Fork/restart/inspect | Planned |

## 10. Metacognition, Coordination, and Improvement

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| RuntimeTelemetry baseline remains available for ablation | 9 | Snapshot/ablation tests | Developer telemetry view | Planned |
| SelfModelProjection is query-first, not an authoritative service | 9 | Dependency/storage tests | No persistent self-agent surface | Planned |
| SELF predictions name interventions and expected outcomes | 9 | Validation tests | Prediction/outcome inspection | Planned |
| Richer SELF mechanism beats telemetry and generic deliberation | 9 | Blinded held-out protocols | Evaluation summary | Planned |
| Coordination starts with one Work and expands only when valuable | 5, 9 | Topology ablations | One/coalition task comparison | Planned |
| Verifier diversity is justified by reduced correlated error | 9 | Correlation tests | Evaluation report | Planned |
| ImprovementCases localize before mutation | 10 | Localization tests | Case inspection | Planned |
| Mutation uses approved bounded Change Grammar | 10 | Grammar/property tests | Candidate manifest | Planned |
| Search can stop without promotion | 10 | Futility/stop tests | Inconclusive case UAT | Planned |
| Deletion competes with added complexity | 10 | Challenger tests | Retention decision | Planned |
| Causal credit is provisional and reversible | 10 | Promotion epoch tests | Monitoring/rollback | Planned |
| Semantic rollback removes unsupported descendants | 10 | Deep-lineage tests | Rollback inspection | Planned |
| Protected source changes require external signed update path | 10 | Authority/package tests | Human review packet UAT | Planned |

## 11. Interface and Product Compatibility

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Simple, Advanced, and Developer use the same runtime | 1 onward | State/API comparison | Switch levels during active workflow | Implemented / Windows portable and Linux AppImage native UAT pass / cumulative pending |
| Interface selection changes presentation only | 1 | UI/state tests | Switch and return without state change | Implemented / Windows portable and Linux AppImage native UAT pass / cumulative pending |
| `advanced_mode=false` migrates to Simple | 1 | Settings migration test | Existing profile UAT | Automated pass / packaged UAT pending |
| `advanced_mode=true` migrates to Advanced, never Developer | 1 | Settings migration test | Existing advanced profile UAT | Automated pass / packaged UAT pending |
| Protected controls remain locked in Developer | 1 onward | API rejection tests | Client-side tamper UAT | Stage 2 Root projection is read-only in Windows portable and Linux AppImage native UAT / later controls pending |
| Simple avoids research ontology and raw protected data | 7 onward | UI visibility tests | Native Simple audit | Planned |
| Advanced exposes practical workflows/evidence | 3 onward | UI tests | Native Advanced workflows | Canonical attempt, revision, generation, wait reason, lease owner, criteria, and ordered-event count pass component/browser E2E / native packaged workflow UAT pending |
| Developer exposes manifests/diagnostics and authorized research summaries | 1 onward | UI contract tests | Native Developer workflows | Root compatibility inspector passes Windows portable and Linux AppImage native UAT; read-only raw canonical workflow contract/attempt/event evidence passes component/browser E2E / expands by stage and needs native packaged workflow UAT |
| Existing CLI compatibility remains functional | Every stage | Parser/process regression | Real terminal UAT | Canonical `mcg workflow list/show/cancel` parser and cumulative CLI/process suites pass / packaged terminal UAT pending |
| Existing conversations/projects/history remain accessible | 2 onward | Migration/integration tests | Packaged upgrade UAT | Planned |

## 12. Research and Later Gates

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| No recursive optimization before Phase 0 passes | 7 | Feature/authority gate tests | Developer disabled-state inspection | Planned |
| No Evolution Lab before bounded optimizer passes Phase 4 | 10, 13 | Feature gate tests | Developer disabled-state inspection | Planned |
| Culture starts local/simple and non-authoritative | 11 | Transfer/authority tests | Import/export UAT | Planned |
| Private-derived publication requires approval | 11 | Privacy/policy tests | Publication approval UAT | Planned |
| Development claims name highest demonstrated level | 12 | Report validation | Research report inspection | Planned |
| Initial developmental cohorts freeze/disable Culture | 12 | Protocol validation | Cohort manifest | Planned |
| Continuity measurements remain evaluator-only, not self-preservation objectives | 12 | Influence/access tests | Developer disclosure boundary | Planned |
| Negative results kill, narrow, or isolate mechanisms | 9–13 | Gate decision tests | Documentation/release review | Planned |
| Optional challengers enter production only after measured product rent | 13 | Retention gate | Release decision packet | Planned |

## 13. Release Evidence

| Evidence | Required frequency | Status |
|---|---|---|
| Unit/property/integration test receipt | Every implementation change | Contracts 12/12, storage 100/100, runtime 108 passed plus two explicit Windows Sandbox ignores, Protected Root 6/6, protected-root migration process 1/1, and affected all-target checks pass locally; cumulative workspace/package receipt pending |
| Crash and process-boundary receipt | Every affected durable boundary | Root open/startup recovery integration passes; forced-kill package matrix pending |
| Security/adversarial receipt | Every affected trust/capability boundary | Planned |
| Desktop deterministic E2E receipt | Every visible workflow change | Complete Desktop Vitest 122/122 and Playwright 29/29 pass, including Simple/Advanced/Developer canonical projection and revision/generation-fenced cancellation |
| Native Simple/Advanced/Developer UAT | Every meaningful interface stage | Windows portable and Linux AppImage Stage 2 slices pass; current-source Windows installer and macOS remain open |
| Windows/macOS/Linux package receipt | Every process/package identity change and final candidate | Exact `cea1e07` Linux clean manifest/scan and AppImage native UAT pass; Debian install/remove plus current-source Windows/macOS remain open |
| Migration + backup/restore receipt | Every schema/ownership change | Schema 50→55 protected-root migration-process and focused schema-54 action-history migration tests pass / packaged backup/restore UAT pending |
| Golden payload/hash compatibility receipt | Every codec/schema release | TaskContract v1 fixture/hash plus existing canonical-v1 fixtures pass / cross-version package replay pending |
| Experiment artifact set | Every protected experiment | Planned |
| Commit/remote parity and artifact manifest | Every verified major stage | Stage 3 through schema-54 action reconciliation is pushed through `b353896`; schema-55 validation/commit/compensation is locally verified pending checkpoint, and cumulative release artifacts remain pending |
