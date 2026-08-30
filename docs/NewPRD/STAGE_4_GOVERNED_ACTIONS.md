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
- platform-stable semantic paths: NFC, forward-slash-only, 4 KiB total/255-byte components,
  with empty/dot components, controls, alternate streams, invalid Windows characters,
  trailing-dot/space aliases, and reserved device names rejected on every OS;
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
authority payloads and prints each bound origin type, locator, domain, and digest. The
explicit `--json` path exposes the full Developer contract.

The Desktop consumes the same bounded Root projections through Tauri. Simple shows an
owner-facing action identity and plain-language outcome only. Advanced adds effect class,
revision/generation, actor, operation type, record counts, reconciliation state, and a
source-workflow evidence loader. Developer adds the read-only canonical envelope,
executions, receipts, reconciliation jobs, and ordered event evidence. Switching levels
does not reload, fork, or mutate the Root action state and never exposes mutation authority.
Canonical inspection also returns a bounded creation-ordered list of actions that compensate
the inspected action. Advanced and Developer can traverse both directions of the exact causal
relationship, including a target outside the current summary window, without manufacturing a
new effect or receiving compensation authority.

Checkpoint `cccdee0` establishes the canonical prerequisite for exact source navigation.
The optional, backward-compatible `ActionOrigin` binds conversation, invocation, command,
approval, agent, and evidence `SourceRef` values into immutable action intent. Root rejects
empty, oversized, duplicate, malformed-hash, and idempotently substituted origins. Advanced
shows the bound locators and short hashes and enables the originating-conversation control
only when that exact conversation is already present; Developer raw evidence includes the
complete binding. Compatibility producers and deeper invocation/command/approval focus are
still required before approval-source navigation is complete.

## Authority boundary

The runner is an effect executor, not an authorization service. Its request identifiers
and hashes do not independently prove Root authority. Production dispatch comes from
Protected Root after durable authorization, using the exact adjacent executable, a
hash-bound capability root, a cleared environment, bounded request/response/diagnostics,
and a deadline with kill/reap semantics. Root persists and classifies the returned
observation before the workflow can commit, compensate, retry, cancel, or settle.

Directly invoking the packaged runner grants no capability beyond the invoking OS user's
existing filesystem authority. It must never be treated as a substitute for Root policy,
OS sandboxing, or the action ledger. Root-supervised execution confines each request to a
Windows Job Object or POSIX process group and terminates the whole tree at the shared
deadline; this process boundary is defense in depth, not a filesystem or network sandbox.

## Verification completed

- Runner unit and separate-process tests: 17 passed (5 unit and 12 separate-process).
- Storage action/recovery suite within 103 passing storage tests, including backward-compatible
  origin decoding, bounds validation, canonical inspection, and replay-substitution rejection.
- Protected Root tests: 11 passed (10 unit and 1 real-process integration), including
  confirmed/lost/pre-launch classification,
  occurred/not-occurred/ambiguous reconciliation, real-store reopen, and stranded dispatch
  restart recovery, authenticated bounded action list/show IPC, and four real-child failures
  durably reconciled after a full Root reopen without redispatch.
- CLI tests: 68 unit and 10 integration passed, including bounded `action list/show` parsing
  and exact content-minimized human origin output.
- Contracts and platform strict all-target Clippy: passed.
- Runtime tests: 108 passed with two explicit native Windows Sandbox package gates ignored.
- Protected Root migration-process test: passed.
- Action runner tests: 18 passed (6 unit and 12 separate-process), including deterministic
  pre-effect exit, post-effect lost response, timeout-before-effect, timeout-after-effect,
  direct-child and descendant reaping, rejection of work scheduled after the deadline,
  malformed responses, diagnostic output, bounded stdout/stderr overflow, and a leader that
  exits while a descendant retains inherited pipes.
- Strict runner all-target Clippy: passed with `-D warnings`.
- Strict client-only runner Clippy and dependency-shape checks: passed; `cap-fs-ext` and
  `cap-std` are absent from the Root-linked feature graph.
- Desktop Rust tests: 20 passed.
- Desktop component tests: 123 passed, including canonical action evidence and read-only
  accessibility semantics.
- Current governed-action component path: 113/113 passed, including exact causal compensation
  traversal and materialization of a linked action outside the current summary window.
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
input failure, the package qualification contract, uncertain classification after a lost
response, bounded output/diagnostic failure, and whole-process-tree kill/reap behavior before
and after an effect. The fault fixture is a
separate executable used only from tests; release packaging's exact sidecar allowlist copies
only `mcg-action-runner`. Unix additionally compiles a symlink escape test, which must run on
the native Linux and macOS lanes.

## Remaining exit gates

Stage 4 is not complete until all of the following pass:

1. Extend the passing real-child plus Root-restart exit/lost-response/timeout/kill/reap matrix
   through packaged-process tests, including the existing changed-target ambiguity case,
   without blind redispatch.
2. Exact approval-to-source navigation reaches the originating conversation, agent,
   command, approval, and evidence, and compensation controls become owner-visible.
   Authenticated local IPC, CLI list/show, and progressive Desktop status/evidence are
   implemented. Immutable source binding and fail-closed origin display are implemented;
   compatibility producers plus exact invocation/command/approval focus remain. The current
   Desktop source-workflow loader is not the complete navigation contract. Bounded bidirectional
   compensation history and exact inspection are implemented; initiating a new separately
   authorized compensation is not yet exposed. Add a Root HTTP/API projection only if a
   non-local product consumer requires it.
3. Resource budgets beyond bounded I/O/deadlines, network denial, case-collision/filesystem
   alias qualification, and platform sandbox capability reporting pass adversarial Windows,
   Linux, and macOS tests. Platform-stable lexical semantic-path policy and Windows Job
   Object/POSIX process-group confinement are implemented; native Linux/macOS and packaged
   path/process-tree qualification remain.
4. Fresh Windows installer/portable, Linux Debian/AppImage, and macOS app/DMG builds pass
   native package UAT. The Debian sudo and macOS authenticated owner gates remain intact.

## Release rule

Do not describe the runner as exactly-once external execution. Create-new is non-overwriting
and duplicate-safe, while a lost response is resolved by observation. The authoritative
guarantee is durable intent plus explicit outcome classification and reconciliation, not an
impossible claim that an arbitrary external effect executes exactly once.
