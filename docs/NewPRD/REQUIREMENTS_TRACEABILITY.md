# Cognitive Runtime Requirements Traceability

**Status:** Active implementation matrix; Stages 1–2 implementation complete with owner-bound package lanes open, Stages 3–4 substantially implemented through schema-60 exact compatibility-agent-lease attachment and atomic production claim, and Stage 5 complete through Root-owned real-process orchestration/recovery, authenticated Root/CLI/Desktop reconstruction, active cancellation, governed local/global agent inventory, canonical workflow/current-attempt invocation drill-down, direct remote Codex sub-agent inventory, and the compatibility-agent model bridge through Root permits, the isolated runner, bounded distinct-authority retry, and independent certificate closure
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
| Existing MetaCog data migrates into a default runtime lineage | 2, 3 | Migration fixtures | Packaged upgrade UAT | Schema 50→60 interruption retry, verified restore, data preservation, and stable Root identity pass. An exact active compatibility task/run/plan/instance/lease can now be atomically attached to one immutable canonical contract/workflow/attempt using only its lease ID / production turn invocation, remaining source classes, and packaged UAT pending |
| Unsafe downgrade fails clearly | 2 | Version downgrade tests | Old package against new data copy | Newer schema refusal implemented / old-package UAT pending |

## 3. IPC and Process Security

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Windows named pipes and Unix-domain sockets | 2 | Platform IPC tests | Native Windows/macOS/Linux process E2E | Implemented / native process E2E passes on all three platforms |
| Versioned length-framed bounded protocol | 1, 2 | Frame/property tests | Cross-process compatibility | Implemented / cumulative cross-platform UAT pending |
| OS peer identity and per-launch capability | 2 | Wrong-peer/session tests | Forged client attack | Capability/session rejection implemented / cross-platform peer UAT pending |
| Replay/idempotency protection | 2–4 | Duplicate request tests | Replayed action request | Root request-ID replay rejection plus durable exact workflow cancellation replay/substitution rejection implemented / action coverage grows in Stage 4 |
| Oversized/malformed/unknown-version messages fail closed | 2 | Fuzz/boundary tests | Hostile client process | Boundary/version rejection implemented / hostile-process UAT pending |
| Cancellation and late responses are safe | 3–5 | State/process tests | Cancel during execution | Workflow/action generations are fenced; pre-dispatch/observational work settles no-effect; dispatched effectful work becomes `OUTCOME_UNKNOWN`; late reconcilers are rejected; authenticated IPC/CLI/Desktop cancellation passes; and real delayed Runtime Work is durably fenced, killed/reaped, settled `Cancelled`, late-result rejected, and Root-reopen verified / broader child propagation, real external-effect process E2E, and package UAT pending |
| Root/Runtime/Evaluator death has an explicit restart story | 3–7 | Crash matrix | Packaged kill/restart UAT | Root startup scanner handles expired Running/Verifying leases / complete process and action matrix pending |

## 4. Workflow and Leases

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Durable `TaskContract` and `WorkflowAttempt` | 3 | Transition/unit tests | Create, restart, inspect | Schema 53 immutable contract versions/heads/dependencies, revision-head CAS with exact replay, attempts, ordered events, atomic v1 Ready creation, and schema-60 immutable source attachment with exact replay/full rollback/cross-table revalidation are implemented. Root derives and atomically claims canonical authority from one active legacy lease ID; newly created compatibility contracts bind acceptance clauses to the exact planned `SuccessCondition` values. Compatibility wait/resume, Work-to-Verifying, failure, approval rejection, fresh-generation reclaim, and stale-authority rollback share the legacy transaction. The prepared objective report is immutable; Root reloads its exact Work targets and dispatches independent Verify; CompletionCertificate persistence plus canonical and legacy terminal settlement is rollback-safe. Real-process satisfied/failed/replay and injected rollback tests pass. Authenticated Root IPC/CLI and deterministic Desktop inspection pass / remaining source classes and process/package E2E pending |
| Only legal workflow transitions occur | 3 | Generated state-machine tests | Invalid transition through API/CLI | Exhaustive contract table, Root-owned revision/generation CAS transitions, authenticated IPC, CLI parser/process coverage, and Desktop cancellation fencing pass / packaged invalid-transition UAT pending |
| Terminal workflows do not ordinarily reactivate | 3 | Property tests | Retry creates explicit new attempt | Terminal monotonicity and explicit parent-linked retry attempts pass / cumulative process UAT pending |
| Generation-fenced leases prevent stale commit | 3 | Race/property tests | Kill owner, reassign, deliver stale result | Claim/heartbeat/cancel/recovery CAS, concurrent-claim winner, stale-result tests, and Stage 5 delayed Runtime cancellation/late-result process coverage pass / complete Stage 3 forced-kill/reassignment matrix and packaged UAT pending |
| Restart scanner resumes only satisfied work | 3 | Recovery integration | Process/application restart | Root startup fences expired Running work, requeues expired Verifying work, resumes only objectively satisfied dependency/retry-backoff waits, fences expired reconciliation workers without redispatch, escalates elapsed reconciliation deadlines, and settles effect-free cancellations / external trigger and full forced-process/package matrix pending |
| Cancellation is durable and propagated | 3–5 | Cancellation tests | UI/CLI cancel and restart | Durable `CancelRequested` fencing plus authenticated `mcg workflow cancel`, Desktop cancellation, action-effect fencing/reconciliation, and independent active Runtime kill/reap with `Cancelled` model-run settlement pass deterministic and real-process reopen tests / broader child propagation, actual external-effect crash E2E, native, and packaged UAT pending |
| UI reconnect cannot alter backend truth | 3 | Projection tests | Close/reopen during work | Simple/Advanced/Developer deterministic component/browser E2E projects one unchanged Root attempt and cancellation result / real process/application reconnect and packaged UAT pending |

## 5. Actions and External Effects

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Intent is durable before an external effect | 4 | Crash injection | Kill immediately around effect | Immutable envelope plus atomic Authorized event and durable Root dispatch-before-launch are implemented; startup converts stranded dispatch into reconciliation without redispatch; real-child pre/post-effect exit and timeout cases persist unknown state, reopen Root, and reconcile by inspection only / packaged timing matrix pending |
| Authorization binds the exact normalized operation | 4 | Substitution/hash tests | Change target after approval | Canonical operation hash, fixed domains, authority, budget, task-contract, workflow revision/generation, actor, capability root, runner identity, and reconciliation-adapter substitution are rejected / approval UI and packaged process attacks pending |
| Execution and observation are distinct | 4 | State tests | Dropped receipt scenario | Separate execution and immutable receipt/event persistence plus real Root runner mapping implemented; deterministic real-child tests prove pre-effect exit and post-effect lost-response classification, durable Root reopen, and inspection-only resolution / packaged E2E pending |
| `NO_EFFECT`, `EFFECT_CONFIRMED`, and `OUTCOME_UNKNOWN` are explicit | 4 | Outcome classification tests | Native reconciliation display | Root durably maps typed runner outcomes and client failures; authenticated bounded action list/show IPC, `mcg action` CLI, and Simple/Advanced/Developer Desktop expose one canonical state with plain-language Simple labels and progressively disclosed evidence / native package UAT pending |
| Consequential unknown effects are not blindly retried | 4 | Crash/property tests | Effect succeeds, receipt lost | Dispatch is legal only from Authorized; unknown/reconciling states reject dispatch; Root startup and its due worker invoke only the bound inspection adapter, never the original operation; real-child post-effect exit/timeout survives Root reopen and confirms the existing effect without replay / packaged E2E pending |
| Reconciliation resolves or escalates ambiguity | 4 | Reconciler tests | Restart and confirm existing file | Root claims exact jobs, derives authority-bound inspection requests, confirms occurred/not-occurred, preserves changed owner content as still unknown, safely defers, recovers leases, escalates deadlines, and exposes bounded job/event evidence through IPC/CLI/Desktop / packaged process UAT pending |
| Compensation is a new causal action | 4 | Ledger/Root IPC/replay-substitution tests | Revert visible in history | Schema 55 binds one separately authorized compensation envelope to the exact committed original plan; the original becomes Compensated only atomically after the causal action commits, and both immutable histories remain. Owner-confirmed CLI and Advanced/Developer initiation send no operation, authority, actor, capability root, reconciler, or budget; Root derives those fields, fences exact action/workflow generations, executes the bound plan, and rejects stale or same-key substituted intent. Exact replay never re-effects an execution beyond Authorized, while bounded Root/Desktop inspection traverses both directions even outside the summary window / packaged and native real reversible-write UAT pending |
| Irreversible actions receive stronger authority treatment | 4 | Policy tests | Approval UAT | Irreversible authorization requires a separately hash-bound elevated-authority policy and exact reconciler/deadline / policy-engine threshold, explicit UI, and native approval UAT pending |
| Action and approval sources are immutable and navigable | 4 | Source-binding/replay/hash-stability tests | Open exact originating records | Backward-compatible `ActionOrigin` binds conversation, invocation, command, approval, agent, and evidence locators plus hashes. Every current approval producer emits validated `ApprovalOrigin`: Codex conversation requests bind conversation/turn/command/agent/parameters; governed tools bind the complete typed request and kind; Guided objective checks bind request/run/attempt/conditions and the independent Evaluator. Owner compensation adds immutable original-envelope and confirmed-request evidence. Historical origin-free approvals remain decodable. Desktop reloads exact approval/conversation identities and focuses an invocation only after exact ID plus full manifest-hash verification. Missing, mismatched, changed, or unsupported projections fail closed. Component and serialized Playwright traversal plus scoped Axe pass / packaged/native UAT pending |
| Read-only operations cannot mutate state | 4 | Snapshot/security tests | Governed read attack | Bounded regular-file read/inspect runner path, Root-bound observational no-effect classification, normalized relative paths, and link/reparse rejection implemented / adversarial snapshots and native package attack pending |

## 6. Model Invocation, Context, and Data Egress

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Every material call has a reconstructable `InvocationManifest` | 5 | Manifest completeness tests | CLI reconstruction | Complete bounded DTO, canonical hash fixture, schema-56–58 immutable persistence, exact replay/substitution, restart reconstruction, model-run lineage, separate deterministic Runtime process, Root-owned plan-before-dispatch/result-fenced orchestration, generation-safe idle service replacement, authenticated bounded Root list/show, human/JSON `mcg invocation list/show`, progressive Desktop reconstruction, bounded local/global agent inventory, integrity-checked workflow/current-attempt invocation drill-down, and direct remote Codex sub-agent inventory implemented / packaged remote-inventory/replacement/native product UAT and real governed provider path pending |
| Exact context refs are captured in `ContextManifest` | 5, 8 | Context tests | Developer inspector | Exact source/perspective/influence/transformation/order/token/selection/expansion contract, content-addressed schema-56 storage, Root-bound real-process dispatch, restart reconstruction, CLI JSON inspection, and Advanced/Developer Desktop context/hash inspection implemented / live governed-provider capture and Stage 8 retrieval integration pending |
| Remote model calls require Root-bound egress permit | 6 | Permit/policy/gateway tests | Unauthorized destination/context attack | Proposal/permit v2 binds invocation/role, exact attempt revision/lease generation/owner, authenticated Runtime identity, TaskContract, request artifact/hash, exact ContextManifest disclosure, provider/model/destination/classification, token/cost budget, policy, expiry, and opaque handles. Fresh/resumed compatibility-agent callers select a lease-scoped Root dispatcher; the gateway persists plan/dispatch before invocation; the isolated child independently recomputes and validates every fence before one real OpenAI-compatible buffered or streaming HTTP request; streaming has a distinct domain-separated request artifact and correlated bounded frames. Missing Root dispatch, substituted requests, stale authority, and legacy fallback fail closed; a safe precontact retry requires a new exact Root permit; cancellation/staleness/consumer drop kills the process tree, records possible contact `OutcomeUnknown`, prevents retry, and fences late output / remaining provider surfaces, OAuth, external-provider smoke, packaged attack UAT, and native qualification pending |
| Runtime receives no ambient provider secrets | 6 | Secret scanning/process tests | Compromised Runtime probe | Runtime launches cleared and has no provider/network/secret dependency. Schema 59 persists only opaque handle metadata; the Root-to-runner request contains no raw credential; the isolated child loads only the exact authorized operating-system credential, zeroizes it on drop/exit, and emits categorical errors. The fixed regular non-link executable is SHA-256 pinned and revalidated before each launch; substitution is denied before process/network access. API-key tests prove the secret appears only in the outbound authorization header / packaged process probes, backup/log/UI scanning, OAuth broker, and external-provider UAT pending |
| Secret handles resolve only for exact authorized execution | 6 | Gateway tests | Wrong provider/action handle | Root lookup and the isolated child both validate the exact permit/handle/provider connection/credential slot and refuse pre-issuance, expiry, wrong-pair, changed-provider, request/model substitution, authentication-shape substitution, and credential mutation. The child supports API-key, bearer, and no-auth routes while explicitly denying OAuth / packaged adversarial and native UAT pending |
| Provider retry respects effect and idempotency semantics | 6 | Fake provider fault tests | Dropped/late stream | Deterministic plan-before-dispatch settles all possible-contact ambiguity `OutcomeUnknown`; the isolated provider client makes exactly one physical attempt and cannot hide a retry. Root permits at most one new attempt after proved transient pre-contact failure, durably settles the prior ModelRun `Failed`, reloads current authority, and creates a distinct invocation, permit, client request ID, and ModelRun with the same logical request hash plus exact ordinal/ceiling/`retry_of_invocation_id`. Final pre-contact denial stops after one attempt; ambiguous failure stops `OutcomeUnknown`; hard exhaustion stops after two. Buffered and streaming cancellation after dispatch cannot prove no contact: Root tears down the child, records `OutcomeUnknown`, never retries, rejects late output, and preserves the evidence across reopen. Streaming events remain provisional and completion is withheld until the correlated terminal frame and child exit validate / broader streaming fault injection and packaged/native UAT pending |
| Resolved provider/model/config fingerprint is persisted | 5, 6 | Manifest tests | Run inspection | InvocationManifest and immutable Root storage bind resolved provider/model, configuration, prompts, role, sampling, reasoning, environment, executable build, request hash, and immutable Runtime response hash/payload. Successful governed buffered/streaming runs persist a content-minimized fingerprint over exact authorized provider/model/request/policy, returned model, and hashed allow-listed revision metadata; legacy JSON remains stable, uncertain runs cannot acquire identity, Root reopen preserves it, and CLI plus Advanced/Developer inspection expose the same evidence while Simple hides it / opt-in external-provider and native package inspection pending |
| Private prompts, scratch reasoning, and secrets are excluded from generic telemetry | 5, 6 | Redaction tests | Logs/backups/UI inspection | Context and invocation records retain governed refs/hashes and bounded metadata rather than materialized prompt contents; deterministic provider completion persists only the response artifact hash/length and integer usage, while raw response/provider metadata remains in memory / Runtime log, backup, UI, isolated-adapter, and real-provider redaction adversarial gates pending |

## 7. Tools, Skills, Capabilities, and Sandboxing

| Requirement | Stage | Automated evidence | E2E/UAT evidence | Status |
|---|---:|---|---|---|
| Tool arguments pass schema, semantic, scope, effect, and authority validation | 4, 6 | Validator/property tests | Hostile model/tool request | Planned |
| Tool processes receive only exact permitted resources | 4, 6 | Capability tests | Filesystem/network/secret escape | Root derives one hash-bound absolute capability root, launches the fixed adjacent runner with a cleared environment and bounded I/O, retains no child authority, rejects traversal/link paths plus cross-platform semantic aliases (non-NFC/backslash/control/ADS/invalid characters/trailing-dot-space/reserved devices), and confines descendants with a Windows Job Object or POSIX process group; real Windows timeout/inherited-pipe escape tests pass / native Linux/macOS path/process-tree, case-collision/filesystem-alias, OS sandbox, network, and broader resource-budget gates pending |
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
| Developer exposes manifests/diagnostics and authorized research summaries | 1 onward | UI contract tests | Native Developer workflows | Root compatibility inspector passes Windows portable and Linux AppImage native UAT; read-only raw canonical workflow and governed-action intent/execution/receipt/reconciliation/event evidence passes component/browser E2E and action-specific Axe analysis / expands by stage and needs native packaged UAT |
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
| Unit/property/integration test receipt | Every implementation change | Current owner-compensation checkpoint passes contracts 18/18, platform 3/3, Protected Root 13/13 plus real-child integration, Runtime 110 with two explicit Windows Sandbox qualifications ignored, CLI 70 unit plus 10 integration, action runner 6 unit plus serialized 12/12 process tests, Desktop Rust 20/20, Desktop component 126/126, serialized Playwright 32/32, TypeScript/build, strict changed-boundary Clippy, and packaging/docs/format/diff gates. One resource-contended runner timing miss passed focused and in the full isolated rerun; cumulative release-package receipt pending |
| Crash and process-boundary receipt | Every affected durable boundary | Real-child pre/post-effect exit, timeout, direct-child and descendant kill/reap, inherited-pipe deadline, bounded hostile output, and late-work rejection pass; all four effect fault classes also persist unknown state through Root reopen and reconcile by bound inspection without original-effect replay / packaged and native three-platform forced-kill matrix pending |
| Security/adversarial receipt | Every affected trust/capability boundary | Planned |
| Desktop deterministic E2E receipt | Every visible workflow change | Complete Desktop Vitest 125/125 and serialized Playwright 32/32 pass, including Simple/Advanced/Developer workflow, governed-action, exact approval-source, hash-verified Runtime-invocation focus, and global-agent projection; fail-closed missing/mismatched source handling; revision/generation-fenced cancellation; keyboard navigation; search; and scoped approval/action/invocation/agent accessibility |
| Native Simple/Advanced/Developer UAT | Every meaningful interface stage | Windows portable and Linux AppImage Stage 2 slices pass; current-source Windows installer and macOS remain open |
| Windows/macOS/Linux package receipt | Every process/package identity change and final candidate | Action-runner sidecar, portable payload, packaging invariants, and all three native package-UAT harnesses are updated; Windows debug copied-sidecar qualification passes. Fresh release builds and native Windows/Linux/macOS package UAT remain open. |
| Migration + backup/restore receipt | Every schema/ownership change | Schema 50→55 protected-root migration-process and focused schema-54 action-history migration tests pass / packaged backup/restore UAT pending |
| Golden payload/hash compatibility receipt | Every codec/schema release | TaskContract v1 fixture/hash plus existing canonical-v1 fixtures pass / cross-version package replay pending |
| Experiment artifact set | Every protected experiment | Planned |
| Commit/remote parity and artifact manifest | Every verified major stage | Stage 5 direct remote Codex sub-agent inventory `fb03f89` and documentation evidence `9645e5c` are pushed with exact local/remote parity. Cumulative release artifacts remain pending |
