# Stage 4 — Governed Actions, Side Effects, and Reconciliation

**Status:** Authoritative ledger, filesystem runner, Root dispatch, reconciliation, restart recovery, and progressive Desktop inspection implemented; release qualification and exact source navigation remain open

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

Protected Root now links only the client/protocol feature, never the filesystem executor.
It persists dispatch before launch, derives the request exclusively from the immutable
authorized envelope, supervises the exact adjacent sidecar with bounded I/O and timeout,
and durably classifies confirmed, no-effect, and unknown outcomes. A Root-owned worker
claims due reconciliation jobs with revision/generation leases, invokes only their bound
inspection adapter, and resolves exact occurrence, exact non-occurrence, or bounded
ambiguity. Startup recovery converts stranded effectful `Executing`/`Observed` work into
reconciliation without redispatch and safely settles stranded observational work as
no-effect.

Authenticated Root IPC now provides bounded action list/show queries. List responses use
content-minimized summaries; show returns the immutable envelope, bounded executions,
receipts and reconciliation jobs, plus the newest bounded event window in canonical order.
`mcg action list` presents safe state metadata, while human `mcg action show` omits raw
authority payloads and the explicit `--json` path exposes the full Developer contract.

The Desktop consumes the same bounded Root projections through Tauri. Simple shows an
owner-facing action identity and plain-language outcome only. Advanced adds effect class,
revision/generation, actor, operation type, record counts, reconciliation state, and a
source-workflow evidence loader. Developer adds the read-only canonical envelope,
executions, receipts, reconciliation jobs, and ordered event evidence. Switching levels
does not reload, fork, or mutate the Root action state and never exposes mutation authority.

## Authority boundary

The runner is an effect executor, not an authorization service. Its request identifiers
and hashes do not independently prove Root authority. Production dispatch comes from
Protected Root after durable authorization, using the exact adjacent executable, a
hash-bound capability root, a cleared environment, bounded request/response/diagnostics,
and a deadline with kill/reap semantics. Root persists and classifies the returned
observation before the workflow can commit, compensate, retry, cancel, or settle.

Directly invoking the packaged runner grants no capability beyond the invoking OS user's
existing filesystem authority. It must never be treated as a substitute for Root policy,
OS sandboxing, process-tree confinement, or the action ledger.

## Verification completed

- Runner unit and separate-process tests: 9 passed.
- Storage action/recovery suite within 102 passing storage tests.
- Protected Root tests: 10 passed, including confirmed/lost/pre-launch classification,
  occurred/not-occurred/ambiguous reconciliation, real-store reopen, and stranded dispatch
  restart recovery, plus authenticated bounded action list/show IPC.
- CLI tests: 67 unit and 10 integration passed, including bounded `action list/show` parsing.
- Contracts and platform strict all-target Clippy: passed.
- Runtime tests: 108 passed with two explicit native Windows Sandbox package gates ignored.
- Protected Root migration-process test: passed.
- Strict runner all-target Clippy: passed with `-D warnings`.
- Strict client-only runner Clippy and dependency-shape checks: passed; `cap-fs-ext` and
  `cap-std` are absent from the Root-linked feature graph.
- Desktop Rust tests: 20 passed.
- Desktop component tests: 123 passed, including canonical action evidence and read-only
  accessibility semantics.
- Serialized browser E2E: 30 passed, including Simple → Advanced → Developer → Simple
  action-state preservation and an action-card Axe accessibility scan.
- Desktop TypeScript, production frontend build, and affected Tauri all-target check: passed.
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

1. Lost response, crash-before-effect, crash-after-effect, timeout, late response, and
   killed-process cases pass real process injection without blind redispatch.
2. Exact approval-to-source navigation reaches the originating conversation, agent,
   command, approval, and evidence, and compensation history/controls become owner-visible.
   Authenticated local IPC, CLI list/show, and progressive Desktop status/evidence are
   implemented; the current Desktop source-workflow loader is not the complete navigation
   contract. Add a Root HTTP/API projection only if a non-local product consumer requires it.
3. Semantic path policy, resource budgets, process-tree confinement, and platform sandbox
   capability reporting pass adversarial Windows, Linux, and macOS tests.
4. Fresh Windows installer/portable, Linux Debian/AppImage, and macOS app/DMG builds pass
   native package UAT. The Debian sudo and macOS authenticated owner gates remain intact.

## Release rule

Do not describe the runner as exactly-once external execution. Create-new is non-overwriting
and duplicate-safe, while a lost response is resolved by observation. The authoritative
guarantee is durable intent plus explicit outcome classification and reconciliation, not an
impossible claim that an arbitrary external effect executes exactly once.
