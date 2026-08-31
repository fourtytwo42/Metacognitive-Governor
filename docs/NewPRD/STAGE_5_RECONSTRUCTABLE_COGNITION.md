# Stage 5 — Runtime Worker and Reconstructable Deterministic Cognition

**Status:** Contract/persistence foundation (`83b5629`), separate authenticated worker (`522ba92`), Root-owned real-process orchestration/recovery (`4a2a905`), authenticated Root/CLI reconstruction (`bde06f0`), and progressive Desktop reconstruction (`1e94efb`) implemented; global-browser, active-service replacement, and release qualification remain open

## Delivered contract boundary

`mcg-contracts` now defines complete versioned DTOs for:

- `ContextManifest`, including exact source and perspective refs, influence metadata, transformation refs, prompt placement, approximate token accounting, selection policy, and progressive expansion steps;
- `InvocationManifest`, including Work/Verify role, exact TaskContract and RuntimeIdentity, cognitive configuration, role and prompt hashes, context hash, model route and resolved provider/model identity, sampling/reasoning, tools, skills, authority, egress, repository, environment, build, request, and optional response bindings;
- `ModelRun`, including lifecycle state, provider request identity, response artifact, usage, and timestamps;
- `CompletionCertificate`, including independent Work and Verify lineage, criterion results, evidence, tests, artifacts, untested paths, Record-backed assumptions and risks, and verdict.

All trust-boundary values validate bounds and canonical structure after deserialization. Context placement and token totals are exact, tool and Skill manifests cannot be duplicated, model-run metadata cannot appear before its state permits it, and `Satisfied` cannot contradict a failed criterion. A checked-in canonical hash fixture detects wire drift across all four records.

## Root-owned schemas 56–58

Schema 56 adds:

- immutable, content-addressed `context_manifests`;
- immutable, content-addressed `invocation_manifests` bound by foreign keys to workflow attempts, exact TaskContract versions, and persisted context hashes;
- append-only `model_run_versions` plus a one-revision-at-a-time CAS head;
- immutable `completion_certificates` and ordered Work/Verify invocation lineage.

The storage API validates exact attempt, workflow, TaskContract, context, Runtime build, and invocation-role bindings. Model-run transitions are restricted to `Planned → Dispatched → terminal` or `Planned → Cancelled`; stale revisions, rewritten historical fields, late terminal rewinds, and idempotency substitution fail closed. A CompletionCertificate cannot be persisted until every named Work and Verify invocation has a completed model run under the same attempt and TaskContract.

Runtime receives no database handle through this slice. These APIs live only on the existing authoritative `SqliteEventStore`, which production opens through Protected Root.

Schema 57 adds immutable exact Runtime response artifacts. Schema 58 adds a Root-owned
monotonic cognition-worker generation that survives restart and cannot be reset, skipped,
updated out of sequence, or deleted through ordinary storage operations.

## Inspection and compatibility

Root storage can reconstruct one invocation with its exact context, manifest hashes, and bounded chronological model-run revisions. Attempt-level invocation listing is bounded and stable, with Work ordered before Verify when persistence timestamps tie. Completion certificates round-trip exactly and survive a full database reopen.

Schema-50 and schema-54 migration fixtures were extended so they remain genuine historical profiles after schema 56. A focused schema-55→56 test and the real Protected Root interruption/backup/retry/restore process test pass without weakening backup or ownership behavior.

## Separate Runtime worker checkpoint

`mcg-runtime-worker` is now a real dependency-isolated executable. Its dependency tree contains only contracts, the platform IPC boundary, canonical serialization/hashing, async transport, and error/secret-zeroization support; it does not link storage, Protected Root, the legacy runtime implementation, providers, Tools, SQLite, or HTTP clients.

Root-side test orchestration binds the endpoint before launch and passes the unpredictable capability only through inherited stdin. The worker connects back through a reject-remote Windows named pipe or owner-only Unix socket, then proves the exact executable hash, process ID, cognition role, protocol, and generation. Every subsequent frame is canonical, bounded, session/deadline/correlation bound, and authenticated; replay tracking is local to the worker session. Debug output omits the capability and shutdown is explicit.

The deterministic provider path has different Work and Verify semantics. Work produces no verification claim. Verify consumes the exact Work response hash plus bounded criterion evidence and proposes `Satisfied` or `NotSatisfied`; it cannot commit a certificate or Root state. Wrong capabilities and executable substitution terminate without accepted execution.

## Protected Root orchestration checkpoint

Checkpoint `4a2a905` adds the production Root client without linking the worker implementation.
Protected Root binds the endpoint before process launch, passes the launch capability only over
inherited stdin, clears the child environment, authenticates the exact adjacent executable/PID/
role/protocol/generation/runtime identity, and poisons and terminates a channel after timeout,
transport failure, malformed output, or substituted evidence.

Root persists the ContextManifest, InvocationManifest, and `Planned` ModelRun before dispatch,
then advances to `Dispatched` before any invocation bytes reach Runtime. It accepts a result only
while the exact workflow attempt, revision, lease owner, lease generation, worker generation,
invocation, model run, role, request hash, RuntimeIdentity, and executable build remain current.
An ambiguous post-dispatch failure becomes `OutcomeUnknown`; a late result cannot revive it.
Exact responses are retained immutably before the run completes.

Only Root can persist a CompletionCertificate and settle the workflow. A real separate-process
test drives Work, transitions the workflow to Verifying, drives independent Verify over the exact
Work response hash, persists the immutable certificate, settles Succeeded, and reconstructs the
same evidence after a full Root reopen. Additional real-process tests cover deadline poisoning,
duplicate authenticated requests, worker death, conservative unknown outcome, cancellation/late
result rejection, generation persistence, and clean exact-child shutdown.

The fixed adjacent worker is now part of Tauri external-bin, Windows portable, and Windows/Linux/
macOS package contracts. Each package harness requires it and runs `--qualify-runtime-worker`,
which reports its exact domain-separated executable hash, protocol, cognition role, and absence
of storage linkage.

## Root and CLI reconstruction checkpoint

Checkpoint `bde06f0` moves `ModelRunRecord`, `InvocationSummary`, and `InvocationInspection` into
the stable contracts boundary and adds a Root-owned newest-first global invocation query. Summary
rows contain workflow/attempt/role/TaskContract identity and only current run state/revision/time;
they do not materialize prompts, responses, secrets, or private reasoning. Show returns the exact
content-minimized InvocationManifest, ContextManifest refs/hashes, and bounded immutable model-run
history. IPC-specific list and history ceilings keep responses below the canonical frame boundary.

Authenticated correlated Root IPC now exposes invocation list/show, including invalid-bound
rejection. `mcg invocation list/show` consumes that boundary before bearer-token compatibility
loading. Human output presents identities, provider/model locators, hashes, and run lifecycle;
explicit `--json` returns the canonical reconstruction contract without granting storage access.

## Progressive Desktop reconstruction checkpoint

Checkpoint `1e94efb` adds authenticated Tauri and TypeScript invocation list/show clients and
projects the same Root-owned summary and inspection state through every interface level. Simple
shows only Work/Verification identity and plain-language lifecycle state. Advanced adds exact
invocation/workflow identity, revision/time, governed provider/model locators, context counts,
TaskContract version, bounded request/response hashes, immutable run count, and a source-workflow
evidence control. Developer adds read-only canonical JSON while explicitly withholding prompt and
response content, secrets, private reasoning, storage access, and Root mutation authority.

Component and deterministic browser tests prove Simple → Advanced → Developer → Simple preserves
one invocation state. The browser journey also runs a scoped Axe accessibility analysis over the
invocation section.

## Verification

- `cargo test -p mcg-contracts` — 17/17 passed.
- `cargo test -p mcg-platform` — 3/3 passed.
- `cargo test -p mcg-runtime-worker --all-targets -- --test-threads=1` — 11/11 passed, including nine real-process/dependency tests.
- Strict all-target no-dependency Clippy for platform and runtime-worker — passed.
- `cargo clippy -p mcg-contracts --all-targets -- -D warnings` — passed.
- `cargo test -p mcg-storage -- --test-threads=1` — 110/110 passed.
- `cargo test -p mcg-protected-root --all-targets` — 11/11 passed.
- `cargo test -p mcg-protected-rootd --test migration_process -- --test-threads=1` — 1/1 passed.
- Strict no-dependency Protected Root Clippy passed with only its documented pre-existing `needless-pass-by-value` and `too-many-lines` classes allowed.
- `cargo check --workspace --all-targets` — passed.
- Packaging and documentation validators, PowerShell parsing, copied debug-sidecar qualification, formatting, and `git diff --check` — passed.
- Root/CLI reconstruction checkpoint: contracts 17/17, platform 3/3, storage 110/110, Protected Root 12/12, CLI 69 unit plus 10 integration, whole-workspace all-target check, strict boundary Clippy, packaging/docs/format/diff gates — passed.
- Desktop reconstruction checkpoint: TypeScript and production build passed; Desktop Vitest 124/124; serialized Playwright 31/31; Desktop Rust 20/20; whole-workspace all-target check; packaging/docs/format/diff gates — passed. Strict Desktop boundary Clippy passes with only its recorded legacy lint classes allowed; no finding names the invocation path.

Strict whole-storage Clippy still reaches the repository’s documented pre-existing storage lint backlog. Rerunning the storage library with only those established lint classes allowed passes, and strict contracts Clippy is clean.

## Remaining Stage 5 work

Stage 5 is not complete until all of the following are implemented and qualified:

1. Add active-service supervision that replaces an idle dead or expired worker with a newly allocated generation without accepting work from the old generation.
2. Extend the canonical invocation list into the global workflow/agent browser and reconcile compatible `agents --remote` presentation.
3. Connect real governed provider/model execution through Stage 6 permits without giving Runtime ambient network, storage, or secret authority.
4. Run cancellation during active execution, active-service replacement, combined workflow/Runtime crash injection, native GUI UAT, migration, and matched Windows/Linux/macOS package gates.

No real provider is required for this stage’s correctness. Root-bound remote egress permits and secret handles begin in Stage 6.
