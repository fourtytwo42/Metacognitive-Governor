# Measured Supervision and Experiments

M9 adds learning without allowing unmeasured self-modification. Supervision may
observe and recommend freely, but a candidate organization cannot become active
unless immutable benchmark evidence and an explicit promotion decision commit
together. Every promotion retains a known-good parent that can be restored.

## Dormant Governor operator controls

The metacognitive-governor model is disabled. Schema 47 exposes the same bounded proposal
controls to an authenticated external operator through `mcg_governor_inspect` and
`mcg_governor_propose`. The operator may inspect attributed evidence, create an immutable
proposal, and advance its optimistic lifecycle. The control has no filesystem, process, Git,
patch, browser, deployment, credential, direct-Coder, or owner-messaging capability.

Every proposal pins an active organization baseline, cited supervision observations, target,
risk, benefits, risks, success criteria, expiration, and rollback revision. Source changes are
always high risk. Protected authority changes—including the security contract, runtime
authorization, approval thresholds, audit history, Governor enablement, dormant-role activation,
and parallel-agent limits—are rejected server-side. MCP operators cannot approve, promote, or
revert their own proposals. Approval is a distinct first-party owner transition. After approval,
the `request_implementation` action requires a project-linked conversation and submits the brief
to Chat; Chat must call the Coordinator, and only the Coordinator may create Coding Agent work.
A per-proposal lock plus a durable hidden transcript marker makes retry and crash recovery
idempotent. The control never contacts a Coder directly. Promotion continues to require
independent paired evidence, and the baseline remains available for reversion.

## Loop boundaries

- The fast loop records task, tool, confidence, cost, latency, and correction
  observations. It may recommend intervention levels 0 through 6 but does not
  rewrite an organization.
- The medium loop groups durable signals into failure patterns, detects loops or
  disagreement, and proposes bounded routing, budget, stance, or escalation
  changes.
- The slow loop creates an immutable organization revision, prospective
  analysis, shadow/scenario experiment, paired benchmarks, and a promotion or
  rejection decision.

An observation always references a task, run, or organization revision and at
least one durable evidence record. Confidence and severity use basis points so
all interfaces serialize the same bounded values.

## Shadow isolation

Shadow execution receives a private, bounded copy of the selected project and a
candidate organization or model route. Its authority contract has one
representable value: `evaluation_only`. Shadow work cannot hold the production
task lease, settle the production task, publish a Site, approve a tool, mutate
the source project, use MCP/network/system/process authority, or write the active
result. It may use only role-appropriate filesystem primitives rooted at the
private copy and retain content-addressed output for independent evaluation.
Dispatch and atomic turn commit both revalidate the running experiment,
candidate revision, project identity, and ready workspace.

Migration 23 claims each stable `shadow-workspaces/{experiment_id}` path before
copying. Preparation rejects symlinks/junctions, non-regular entries, non-Unicode
relative paths, excessive depth, files, or bytes; skips `.git` and the app-data
subtree; uses create-new writes; preserves source permissions; and hashes ordered
relative paths, lengths, and bytes. A completed copy records its source
fingerprint, definition hash, file/byte bounds, and content SHA-256. A
crash-interrupted or failed preparation becomes `uncertain` and is never run or
silently replaced; completed `ready` evidence replays exactly after restart.
Receipt creation atomically moves that workspace to non-runnable `retained`.

Shadow mode remains opt-in for normal consumers and enabled in development and
scenario environments. A shadow result can influence production only after it
becomes benchmark evidence for a separately promoted revision.

`MCG_SHADOW_EXPERIMENTS` is the explicit headless opt-in (`1`, `true`, `yes`,
or `on`). Debug/development builds default it on; release builds default it off.
The authenticated runtime status reports the effective setting. When enabled,
each scheduler tick turns eligible slow-loop regression evidence into at most
one bounded inactive candidate per organization. Stable candidate/experiment
identities and migration 22 outcomes resume a candidate or draft experiment
that committed immediately before a crash. The active organization head never
changes during this automation; the generated shadow experiment stops in
`ready` until evaluation-only execution begins. Ready experiments then receive
a stable run identity derived from the experiment, a fresh plan rebuilt against
the inactive candidate, and only private-copy filesystem authority. A crash after run creation or
the `running` transition resumes that same candidate run rather than creating a
second provider-work identity. When a discovered provider route exists, the
scheduler prepares the durable private copy and executes dependency-ready items
sequentially with the bounded filesystem tool set.
It never automatically retries an item after any non-completed durable lease;
successful complete-plan output is retained in the local content-addressed
artifact store before the immutable evaluation-only receipt commits. Missing
routes are a no-spend no-op.

Completed automatic receipts are assessed against the exact evaluation that
produced their source supervision observation. The candidate's completed
Evaluator instance is required. The retained turn evidence proves real
provider/tool-loop execution against the isolated copy. Conditions without an
objective-specific verifier remain honestly inconclusive rather than treating
model self-report or a generic file operation as success. `FileExists` is the
first supported verifier: assessment reopens the retained root through the
capability-confined filesystem, accepts only a normalized project-relative path,
captures at most 1 GiB of one regular file into the content-addressed artifact
store, and compares declared minimum bytes and optional SHA-256. A missing,
oversized, mismatched, traversal, or symlink-escape target fails closed. If all
conditions are supported, all passes produce `passed` and any failure produces
`failed`; the presence of any unsupported condition produces `inconclusive`.
`ResearchSources` assessment reopens the candidate's bounded JSON evidence and
re-hashes the exact baseline research artifact that was retained by the
owner-governed source evaluation. The baseline condition must have passed and
exactly one artifact with the same bound scope must exist. The candidate may
not add, remove, or change a source URL: any URL-set change fails before a
request, while a missing, ambiguous, or unverifiable baseline binding remains
inconclusive and launches no request. With that authority re-established, the
credential-rejecting research executor refetches only the exact baseline URL
set and independently checks schema, scope, source minimum, freshness,
conflicts, and bounded quote grounding. The report and fetched bodies remain
content-addressed; query values are omitted from failure details.
Build, test, static-analysis, typed configuration-validation, and exact
`COMMAND SUCCEEDS` conditions now cross
an explicit shadow-command-verifier boundary. A conforming backend must enforce
filesystem, child-process, credential, and network isolation at the operating
system boundary and return a bounded content-addressed transcript. The default
backend is unavailable on unsupported Windows editions/builds and on Linux
hosts without a trusted root-owned, non-setuid/non-setgid, non-writable
`/usr/bin/bwrap` plus `/usr/bin/prlimit`; bubblewrap must be version 0.11.2 or
newer. Otherwise it launches no process and keeps those conditions
inconclusive. The Linux backend uses an empty mount namespace, unshares the
network and every supported namespace, explicitly unshares the user namespace
before disabling nested user namespaces, starts
a new session, drops all capabilities, clears the environment, exposes only
read-only system runtime paths and the retained source, and copies that source
into a size-bounded private tmpfs. CPU time, address space, individual file
size, process count, open files, output, source size/count/depth, and wall time
are bounded. For recognized Node-family commands, a non-system executable can
contribute one optional distribution-root mapping after its root and every entry
pass fixed 2 GiB, 100,000-file, depth-24, regular-file, and relative-symlink
bounds. That root is exposed read-only at `/mcg-toolchain`, and the cleared
`PATH` contains only its `bin` directory plus the already mapped system paths.
This permits owner-installed Node/npm without exposing the owner's home or
silently claiming support for arbitrary language toolchains. Command filesystem
changes disappear with the tmpfs. A separate
private proof file must show that setup completed before any exit can count as
command evidence. Constraining only the child working directory is explicitly
insufficient. The trusted runtime reopens and hashes the versioned transcript,
requires it to bind the exact condition, independently verifies optional output
artifacts, and derives pass/fail from timeout and exit status rather than
trusting a backend-supplied verdict. Linux adversarial host qualification and
the complete Windows positive/negative lifecycle qualification pass. The Windows
backend requires the Windows Sandbox CLI available in current Windows 11
releases and the package-adjacent internal `mcg-shadow-runner.exe`. It prefers a
fixed non-reparse System32 CLI when present. Current Store-delivered releases
are discovered through the fixed System32 Windows PowerShell/Appx module under
a minimal environment, and only the single official
`MicrosoftWindows.WindowsSandbox` package beneath `Program Files\WindowsApps`
with publisher ID `cw5n1h2txyewy` and a non-reparse, nonempty `wsb.exe` is
accepted. App-execution aliases and path lookup are never trusted. It starts one
serialized, UUID-addressed hypervisor
sandbox per condition with networking, vGPU, microphone, camera, printers, and
clipboard disabled and Protected Client enabled. The retained source and the
two-file harness are separate read-only mappings; the bounded runner copies the
source to an unmapped disposable workspace, clears the command environment,
executes a direct argument vector as `System`, and normalizes objective failure
separately from setup failure. Recognized Node-family commands may add exactly
one bounded read-only distribution mapping at `C:\mcg-toolchain`; request schema
7 binds that exact internal root plus an optional typed probe. `PageRenders`,
`ElementExists`, `Accessibility`, and `BrowserWorkflow` may
also add one independently bounded, read-only Chromium distribution at
`C:\mcg-browser`. The runner prepends only the toolchain to its otherwise fixed
system `PATH`. Windows `.cmd` launch normalization covers the Node package
manager shims, while native Bun executables remain native rather than being
misclassified as command scripts. The Windows package build statically links
this small helper's C runtime and rejects imports of Visual C++ redistributable DLLs,
because those DLLs are absent from a pristine Sandbox image. Preflight and
postflight runner probes prevent a missing or lost sandbox session from
masquerading as test failure. The host CLI's own process status is only transport
status: every `exec --raw` response must contain a bounded JSON `ExitCode`, and
MCG uses that nested runner status for preflight, objective, setup-failure, and
postflight decisions. Wall timeout always triggers a best-effort session stop,
and no writable host share or host process fallback exists. The Windows CLI
cannot return child process I/O, so the transcript records only the independently
interpreted exit/timeout disposition. Because the Store CLI accepts one
`--command` value rather than a
POSIX `--` argument tail, MCG selects only two compile-time runner command lines
(probe or request); the untrusted objective executable and argument array remain
inside the bounded JSON request and never enter that CLI string.
`HttpResponds`, `ServiceHealthy`, `PortListening`, `PageRenders`,
`ElementExists`, `Accessibility`, `BrowserWorkflow`, and `ScreenshotMatches` automatic assessment additionally require
the source evaluation to contain exactly one passing identical condition and a
retained completed consumer-evaluation checkpoint whose run matches that source
evaluation. The checkpoint selects the exact immutable project snapshot; MCG
uses its single `start` command, or its single `dev` command only when no `start`
exists. Missing, stale, empty, or ambiguous authority stays inconclusive and
launches nothing. Probes accept only credential-free plaintext HTTP or TCP on
exact loopback with an explicit nonzero port; TCP additionally rejects paths,
queries, and fragments. Service startup and its bounded probe run in the same
network-isolated OS sandbox: Linux uses fixed system curl for HTTP or Bash's
connect-only TCP facility inside the bubblewrap namespace, while Windows runner
schema 8 performs the loopback HTTP request or TCP handshake inside the disposable
Sandbox VM. For `PageRenders`, `ElementExists`, `Accessibility`, `BrowserWorkflow`, and `ScreenshotMatches`, both platforms map the bounded browser distribution
read-only and invoke the package-adjacent schema-8 runner inside the sandbox. The
runner first requires an exact loopback 2xx response, then drives Chromium through
its DevTools protocol under fixed launch, request, render, DOM-size, viewport,
profile, and shutdown bounds. It requires an interactive or complete document
with a body and nonzero layout and removes the disposable profile before returning.
`ElementExists` additionally validates one bounded nonempty CSS selector and
requires Chromium to return at least one matching element after navigation.
`Accessibility` evaluates the same versioned, dependency-light audit contract as
the governed consumer browser and requires its language, title, landmark, image,
control-name, interactive-name, duplicate-ID, heading-order, and tabindex
invariants to pass after rendering.
`BrowserWorkflow` additionally reopens one normalized project-relative regular
JSON file from the copied candidate workspace, requires its exact launch-bound
SHA-256 and name, validates the shared 1-to-64-step non-script language, and then
performs bounded element, text, URL, click, type, and allowlisted-key steps. Click
and key targets are rejected before use when they would download, open another
context, submit a mutating form, or leave the exact start origin; origin is
rechecked after every interactive step.
`ScreenshotMatches` reopens one normalized project-relative regular PNG from the
copied candidate workspace, requires its exact launch-bound SHA-256, captures the
candidate at the canonical 1280x720 viewport with device scale factor 1, and applies
the same explicit per-channel noise and changed-area basis-point thresholds as the
governed consumer browser. Oversized, malformed, dimension-mismatched, or changed
baseline bytes and pixel mismatch fail closed.
Expected status is exact for `HttpResponds`, 200-299 for
`ServiceHealthy`, and a completed no-payload handshake for `PortListening`;
startup timeout, early process exit, mismatch/refusal, malformed evidence, or
sandbox loss fails closed. There is no host-network or host-process fallback.
Page-render, element-existence, accessibility, typed-workflow, and screenshot
conditions are now supported at this boundary.
The paired
baseline/candidate reports are themselves content-addressed, the benchmark
receives a stable identity, and the configured benchmark, regression, and
confidence gates select promotion or rejection. A fully verified file objective
can therefore promote automatically; unverified conditions reject while
retaining evidence and never weaken the gate to manufacture a promotion.

## Durable state

SQLite migrations 20-23 retain:

- immutable supervision observations;
- one mutable experiment projection whose definition hash and revision pairing
  never change;
- immutable shadow receipts and paired benchmark results;
- immutable evidence-linked decisions;
- the existing immutable organization revisions and one current-revision
  pointer.
- exactly-once evaluation-supervision markers and immutable automatic candidate
  outcomes, including stale-baseline dispositions.
- durable private-workspace lifecycle and immutable scope/content evidence.

Experiment states are `draft`, `ready`, `running`, `awaiting_decision`,
`promoted`, `rejected`, and `reverted`. A candidate cannot jump directly from
running to promoted. Promotion requires the configured number of distinct
benchmarks, no more than the permitted regressions, the requested confidence,
the expected current baseline, and candidate/baseline membership in the same
organization.
Automatic promotion additionally re-loads every referenced run and evaluation
and requires each benchmark arm's revision, run, outcome, confidence, and
metrics to match that durable evidence exactly. A syntactically valid benchmark
cannot invent a passing arm for automatic promotion.

## Atomic promotion and reversion

Promotion is one SQLite transaction:

1. Re-read the experiment and current organization revision.
2. Validate all referenced immutable benchmarks and prospective analysis.
3. Insert the immutable decision.
4. Change the organization current-revision pointer to the candidate.
5. Change the experiment projection to `promoted`.

Any mismatch rolls back every write. Reversion uses the same boundary in the
opposite direction, requires a promotion decision in history, restores the
recorded baseline, and moves the experiment to `reverted`. Startup never
auto-promotes interrupted work. A measured regression may request automatic
reversion, but the same evidence and transaction invariants still apply.
Automatic assessment also recovers the crash window after a benchmark commits
but before `awaiting_decision`: the next scheduler tick advances and decides the
same stable evidence without recreating evaluation, benchmark, or decision IDs.
Reversion decisions carry explicit trigger evaluation IDs. Storage verifies
each trigger belongs to a run pinned to the promoted candidate, occurred after
the promotion decision, and reports failure or regression. Scheduler ticks find
that evidence, create one stable reversion decision, and restore the exact
baseline atomically; replay cannot revert twice.

## Interfaces

All callers use the same runtime and SQLite invariants:

- authenticated REST exposes candidate and observation recording, experiment
  creation/list/detail, compare-and-set transitions, shadow receipts, paired
  benchmarks, and decisions;
- CLI `mcg experiment` accepts complete versioned JSON for evidence-bearing
  writes and provides list/show/transition commands for automation;
- MCP exposes `mcg_experiments_inspect` and `mcg_experiments_mutate`; the latter
  requires an explicit action plus its complete versioned payload and cannot
  bypass evidence or transition checks;
- Tauri provides typed bindings for the same routes; and
- the desktop Improvement workspace summarizes observations, protected
  baselines, experiment state, and evidence counts in simple mode, with
  versioned JSON controls and legal state advances in advanced mode.

Caller-owned candidate, observation, experiment, shadow-run, benchmark, and
decision identities make exact lost-response retries observation-only. Every
newly committed experiment event is also published to the existing resumable
SSE stream.

## Exit evidence

M9 is not complete until automated scenarios prove that:

- seeded failure and calibration regressions produce durable observations;
- shadow output cannot change the real task or create real side effects;
- insufficient, conflicting, duplicated, or regressing benchmarks cannot
  promote;
- every successful promotion links to immutable measurements and prospective
  analysis;
- a seeded post-promotion regression restores the exact prior revision; and
- all histories and decisions remain consistent after daemon restart.
