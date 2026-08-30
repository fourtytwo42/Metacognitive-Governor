# Stage 4 — Governed Actions, Side Effects, and Reconciliation

**Status:** Authoritative ledger and narrow filesystem-runner foundation implemented; Root integration and release qualification remain open

**Last verified:** 2026-08-30

## Delivered foundation

Schema 55 gives Protected Root an authoritative action ledger with immutable intent,
receipt, validation, and event history; mutable generation-fenced execution state;
durable reconciliation jobs; explicit effect outcomes; validation-bound commit; causal
compensation; and separately bound elevated authority for irreversible effects.

`mcg-action-runner` adds a one-request filesystem process for the first real governed
effect family. It supports:

- bounded regular-file reads and state inspection;
- atomic create-new publication without overwriting an appearing destination;
- exact-hash compensation that preserves a changed owner file;
- explicit observational, no-effect, confirmed-effect, unknown, and denied outcomes;
- canonical operation payloads with the fixed `MetaCog.ActionOperation` hash domain;
- absolute real capability roots and normalized relative paths only;
- symlink and Windows reparse-point rejection for inspected roots and path components;
- no retained capability or process state between invocations.

The runner is included in Windows, Linux, and macOS Tauri sidecar contracts and in the
one-file Windows portable payload. Package harnesses require the executable and invoke
`--qualify-filesystem-root` in a newly created empty directory. Qualification exercises
create, lost-response-style inspection/reconciliation, duplicate suppression, exact
compensation, and zero-residue cleanup.

## Authority boundary

The runner is an effect executor, not an authorization service. Its request identifiers
and hashes do not independently prove Root authority. Production dispatch must come from
Protected Root after durable authorization, using the exact adjacent verified executable,
an invocation-scoped capability root, bounded environment and inherited resources, and a
deadline with kill/reap semantics. Root must persist and classify the returned observation
before the workflow can commit, compensate, retry, cancel, or settle.

Directly invoking the packaged runner grants no capability beyond the invoking OS user's
existing filesystem authority. It must never be treated as a substitute for Root policy,
OS sandboxing, process-tree confinement, or the action ledger.

## Verification completed

- Runner unit and separate-process tests: 6 passed.
- Strict runner all-target Clippy: passed with `-D warnings`.
- Desktop Rust tests: 20 passed.
- Whole workspace all-target compile: passed.
- Windows debug sidecar preparation and copied-sidecar qualification: passed.
- Windows PowerShell AST parsing and Git Bash syntax parsing for the modified package
  harnesses: passed.
- Packaging, documentation, formatting, metadata, and diff checks: passed.

The unit suite covers create/inspect/duplicate/compensate behavior, operation and hash-domain
substitution, traversal rejection, changed-target preservation, empty-root qualification,
and cleanup. Separate-process coverage proves one-shot request/response behavior, malformed
input failure, and the package qualification contract. Unix additionally compiles a symlink
escape test, which must run on the native Linux and macOS lanes.

## Remaining exit gates

Stage 4 is not complete until all of the following pass:

1. Protected Root launches the runner through a bounded parent client and maps each typed
   response into the exact authorized `ActionExecution` and reconciliation job.
2. Runner requests are derived from the immutable authorized `ActionEnvelope`; callers
   cannot substitute action, execution, operation, workflow generation, root, or actor.
3. Lost response, crash-before-effect, crash-after-effect, timeout, late response, and
   killed-process cases pass real process injection without blind redispatch.
4. Reconciliation observes the exact target and can resolve occurred, not occurred, or
   still unknown without performing the original effect.
5. Root API/IPC, CLI, and Simple/Advanced/Developer projections expose safe action status,
   source navigation, evidence, reconciliation, and compensation.
6. Semantic path policy, resource budgets, process-tree confinement, and platform sandbox
   capability reporting pass adversarial Windows, Linux, and macOS tests.
7. Fresh Windows installer/portable, Linux Debian/AppImage, and macOS app/DMG builds pass
   native package UAT. The Debian sudo and macOS authenticated owner gates remain intact.

## Release rule

Do not describe the runner as exactly-once external execution. Create-new is non-overwriting
and duplicate-safe, while a lost response is resolved by observation. The authoritative
guarantee is durable intent plus explicit outcome classification and reconciliation, not an
impossible claim that an arbitrary external effect executes exactly once.
