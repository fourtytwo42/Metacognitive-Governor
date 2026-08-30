# Stage 5 — Runtime Worker and Reconstructable Deterministic Cognition

**Status:** Contract and Root-owned persistence foundation implemented at `83b5629`; separate Runtime process, deterministic Work/Verify dispatch, product inspection, and release qualification remain open

## Delivered contract boundary

`mcg-contracts` now defines complete versioned DTOs for:

- `ContextManifest`, including exact source and perspective refs, influence metadata, transformation refs, prompt placement, approximate token accounting, selection policy, and progressive expansion steps;
- `InvocationManifest`, including Work/Verify role, exact TaskContract and RuntimeIdentity, cognitive configuration, role and prompt hashes, context hash, model route and resolved provider/model identity, sampling/reasoning, tools, skills, authority, egress, repository, environment, build, request, and optional response bindings;
- `ModelRun`, including lifecycle state, provider request identity, response artifact, usage, and timestamps;
- `CompletionCertificate`, including independent Work and Verify lineage, criterion results, evidence, tests, artifacts, untested paths, Record-backed assumptions and risks, and verdict.

All trust-boundary values validate bounds and canonical structure after deserialization. Context placement and token totals are exact, tool and Skill manifests cannot be duplicated, model-run metadata cannot appear before its state permits it, and `Satisfied` cannot contradict a failed criterion. A checked-in canonical hash fixture detects wire drift across all four records.

## Root-owned schema 56

Schema 56 adds:

- immutable, content-addressed `context_manifests`;
- immutable, content-addressed `invocation_manifests` bound by foreign keys to workflow attempts, exact TaskContract versions, and persisted context hashes;
- append-only `model_run_versions` plus a one-revision-at-a-time CAS head;
- immutable `completion_certificates` and ordered Work/Verify invocation lineage.

The storage API validates exact attempt, workflow, TaskContract, context, Runtime build, and invocation-role bindings. Model-run transitions are restricted to `Planned → Dispatched → terminal` or `Planned → Cancelled`; stale revisions, rewritten historical fields, late terminal rewinds, and idempotency substitution fail closed. A CompletionCertificate cannot be persisted until every named Work and Verify invocation has a completed model run under the same attempt and TaskContract.

Runtime receives no database handle through this slice. These APIs live only on the existing authoritative `SqliteEventStore`, which production opens through Protected Root.

## Inspection and compatibility

Root storage can reconstruct one invocation with its exact context, manifest hashes, and bounded chronological model-run revisions. Attempt-level invocation listing is bounded and stable, with Work ordered before Verify when persistence timestamps tie. Completion certificates round-trip exactly and survive a full database reopen.

Schema-50 and schema-54 migration fixtures were extended so they remain genuine historical profiles after schema 56. A focused schema-55→56 test and the real Protected Root interruption/backup/retry/restore process test pass without weakening backup or ownership behavior.

## Verification

- `cargo test -p mcg-contracts` — 16/16 passed.
- `cargo clippy -p mcg-contracts --all-targets -- -D warnings` — passed.
- `cargo test -p mcg-storage -- --test-threads=1` — 108/108 passed.
- `cargo test -p mcg-protected-rootd --test migration_process -- --test-threads=1` — 1/1 passed.
- `cargo check -p mcg-protected-root --all-targets` — passed.
- `cargo check --workspace --all-targets` — passed.
- Formatting and `git diff --check` — passed.

Strict whole-storage Clippy still reaches the repository’s documented pre-existing storage lint backlog. Rerunning the storage library with only those established lint classes allowed passes, and strict contracts Clippy is clean.

## Remaining Stage 5 work

Stage 5 is not complete until all of the following are implemented and qualified:

1. Create the separate `runtime-worker` executable with no dependency on Protected Root, storage, evaluator, or effect implementations.
2. Add bounded, authenticated, versioned Root↔Runtime local IPC and an unpredictable launch-session capability.
3. Implement deterministic fake Work and independent Verify behavior without Tools or a real model.
4. Have Root persist planned/dispatched/terminal run state and accept only fenced typed Runtime proposals.
5. Prove process death, cancellation, timeout, late response, duplicate response, replay, and Root/Runtime restart behavior.
6. Add authenticated Root IPC, CLI reconstruction, and Simple/Advanced/Developer invocation presentation.
7. Add the canonical global workflow/invocation browser and reconcile compatible `agents --remote` presentation.
8. Run deterministic E2E, native GUI UAT, migration, package, and Windows/Linux/macOS release gates.

No real provider is required for this stage’s correctness. Root-bound remote egress permits and secret handles begin in Stage 6.
