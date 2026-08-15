# Automated Testing and Evidence Strategy

MCG controls files, processes, networks, browsers, and model spending. Testing is
therefore part of the safety architecture, not a release cleanup activity.

## Test layers

| Layer | Purpose | Typical tools | Default execution |
|---|---|---|---|
| Rust unit/property | State machines, policies, budgets, parsers, redaction, routing. | Built-in tests, `proptest` where valuable. | Every change. |
| Migration/storage | Forward migrations, transactions, event/projection agreement, recovery. | Temporary SQLite databases. | Every change. |
| Owner settings | Authenticated reads/writes, optimistic conflicts, immutable reasons, live authority changes, and restart persistence. | Temporary SQLite reopen, REST contracts, CLI parsing, and desktop interaction tests. | Every settings change. |
| Backup/restore | Online snapshot consistency, artifact integrity, secret exclusion, tamper/traversal bounds, offline locking, rollback preservation, and migration/reopen. | Temporary data directories, byte mutation, API/CLI/desktop contracts, and cross-version package fixtures. | Every storage/release change. |
| Diagnostics | Stable version/severity contract, SQLite quick-check, authenticated access, secret/path/content exclusion, CLI strict exit behavior, and simple/advanced desktop disclosure. | Storage unit tests, REST boundary tests, CLI parsing/process tests, desktop interaction tests, and packaged smoke tests. | Every storage, auth, packaging, or support-surface change. |
| Consumer CLI | Bare/interactive/project-selected/piped forms, bounded retained input, project identity reuse, human and JSON output, authenticated task-filtered NDJSON events, fast-result history reconciliation, non-completed exit failure, and token exclusion. | Parser tests plus real CLI child processes against an isolated loopback daemon and temporary project. | Every CLI, task launch, event, or project-discovery change. |
| MCP task lifecycle | Atomic request-key/task/run/consumer-launch reservation, decomposition, independent evaluation, objective settlement, exact replay, task/event/audit inspection, retry/resume delegation, evidence-report validation, request-scoped durable SSE progress, stream isolation, and task/run cancellation on explicit calls or disconnect. | Authenticated JSON/SSE MCP calls with deterministic and delayed providers, concurrent unrelated task events, dropped response bodies, typed project evidence, file-backed SQLite reopen, and the pinned official TypeScript SDK against a real loopback server. | Every MCP, consumer-launch, progress, evaluation, cancellation, or SDK-pin change. |
| Scheduler | Atomic occurrence/task claims, non-overlap, restart reclaim, cron/event/watch/heartbeat wake behavior, cursor and notification persistence, exact-definition creation replay/conflict across REST/MCP/desktop, budget/approval safe pause, and idempotent dispatch. | Explicit clocks, temporary SQLite databases, daemon reopen, real temporary files, fake providers/tools, component and browser UI flows. | Every scheduler change. |
| Computer control | Display/region confinement, frame-digest freshness, explicit input grants, handoff privacy, cross-interface authority, and at-most-once crash disposition. | Real safe capture, deterministic validation, file-backed SQLite reopen, REST/MCP contracts, desktop component flows, and native UAT. | Every computer-use change. |
| Contract | Provider, tool, API, event, MCP, and OpenAI compatibility. | Deterministic fake servers and golden schemas. | Every change. |
| Integration | Multiple crates with real filesystem/process/SQLite boundaries. | Temporary directories and controlled child processes. | Every change. |
| UI component | Normal/advanced modes, approvals, progress, errors, accessibility. | Vitest, Testing Library, axe. | Every UI change. |
| Application E2E | Packaged daemon + desktop/CLI/API workflows. | Playwright and process fixtures. | Pull requests and release candidates. |
| Scenario/eval | Outcome quality, loops, routing, governance, overhead, calibration. | Versioned scenario runner and graders. | Nightly/opt-in live. |
| Security/adversarial | Policy bypass, prompt injection, secret leakage, path/network escapes. | Generated cases and sandbox fixtures. | Pull requests and scheduled deep runs. |
| Performance/soak | Startup, memory, event throughput, long tasks, scheduler drift, leaks. | Benchmarks and long-running fixtures. | Scheduled/release. |
| Installer/upgrade | Clean install, migration, rollback, uninstall, retained state. | Platform VMs/runners. | Release candidates. |

## Determinism boundary

Protocol correctness must never require a paid or nondeterministic model call.
`mcg-test-support` will provide:

- a scripted OpenAI-compatible fake supporting streaming, tools, failures, usage,
  delays, disconnects, rate limits, and malformed responses;
- deterministic agent/model transcripts;
- temporary project repositories and Git histories;
- a controllable clock and scheduler;
- fake credential, browser, computer-use, and approval services;
- content-addressed fixture artifacts with secret scanning.

Live provider tests answer different questions: whether current providers still
conform and whether model behavior meets scenario thresholds. They require an
explicit environment flag, use spending/token ceilings, redact outputs, and never
run on untrusted pull requests.

## Core invariants

Property and scenario tests enforce that:

1. the owner constitution cannot be weakened by MCG;
2. every real-world action has a capability decision and audit event;
3. denied or approval-pending actions have no side effect;
4. retries do not duplicate non-idempotent work without reconciliation;
5. interrupted leases recover to a valid state;
6. secrets never appear in events, logs, context exports, fixtures, or backups;
7. OAuth and API-key billing modes never change implicitly;
8. shadow executions cannot mutate real task state;
9. event history and current projections agree;
10. configuration experiments can always revert to their parent revision.
11. an uncertain computer-input outcome is never automatically replayed.

## Website/application reference scenarios

The first real-project suite includes:

- create a small website from a goal and verify build, semantics, layout, and links;
- repair a seeded defect and prove regression tests fail before and pass after;
- run concurrent research and coding only when the task benefits from decomposition;
- restart the daemon during a long task and resume without duplicate modifications;
- restart while a guided objective batch awaits approval and resume without a
  repeated model call, completed check, or broadened command authority;
- trigger a project-file watch in guided mode, restart after exact approval,
  resume the original task/occurrence, execute each discovered objective check
  once, retain terminal notification/history, and admit no duplicate task;
- restart while local or external MCP work awaits approval and prove the exact
  pinned continuation runs without replaying the paid provider response or
  duplicating the governed side effect;
- require approval for deployment while allowing autonomous local build/test work;
- exercise a fake provider failure, model escalation, budget stop, and user steering;
- compare active and shadow models while accepting output only from the active run.

Success evidence combines tests, build output, Git diff constraints, HTTP health,
browser assertions, accessibility checks, and screenshots when visual comparison
adds information. Model self-report is never sufficient.
HTTP objective fixtures cover exact expected 2xx and non-2xx statuses, mismatches,
unreachable origins, response artifact capture, capability confinement, and audit.
Rendered-page success uses a real Chromium fixture; an HTTP body is not a render
substitute. The fixture requires JavaScript-mutated DOM/title evidence, HTML and PNG
artifacts, exact-origin GET/HEAD confinement, and blocked cross-origin/mutating
requests. Owner-machine verification installs Chromium before Rust tests and passes
its local path through `MCG_BROWSER_EXECUTABLE`; runtime never performs that download.
Element/workflow fixtures additionally cover hydrated selector pass/failure, exact
workflow syntax and size bounds, immutable file binding, typed interaction order,
same-origin URL assertions, retained JSON/HTML/PNG artifacts, and terminal replay.
Negative cases reject unsafe keys, cross-origin destinations, downloads, new browser
contexts, mutating forms, fabricated text, and post-binding workflow mutation.
File-objective fixtures cover cross-platform relative-path parsing, traversal and
absolute-path rejection, regular-file-only enforcement, bounded streaming capture,
binary digest/size evidence, deduplication, missing files, and audit. Custom command
fixtures require an exact safely discovered manifest command and verify that arbitrary
prose or undiscovered commands cannot execute.
Visual-baseline fixtures cover pre-model missing-file rejection, immutable baseline
digest binding, post-binding mutation, bounded PNG decoding, dimension mismatch,
per-channel noise, changed-pixel thresholds, actual/baseline artifact provenance,
same-origin browser confinement, and terminal pass/fail settlement.
Research fixtures cover exact directive/path parsing, task-goal scope binding,
immutable evidence-byte hashing, source-count and credential-free URL limits,
optional freshness enforcement, conflict-reference integrity, real bounded HTTP
fetches, HTML/plain-text quote grounding, retained JSON/source artifacts, fabricated
quotation failure, durable task settlement, and terminal replay with no repeated
model work, source fetch, or audit record. Semantic support remains the independent
Evaluator's responsibility and is never inferred from a substring match alone.

## API and compatibility testing

- Snapshot JSON schemas for application commands/events and task state.
- Run consumer-driven contracts for desktop, CLI, OpenAI, and MCP clients.
- Verify SSE/WebSocket ordering, reconnect cursors, cancellation, and backpressure.
- Test OpenAI-compatible streaming, tool calls, structured output, errors, and usage.
- Maintain provider-specific conformance exceptions as data with an expiry date.
- Fuzz parsers and reject oversized/deep/malformed untrusted payloads safely.
- Assert the same 8,192-token consumer response default across REST, MCP, CLI,
  desktop launch/retry/resume, and schedules; live representative generation must
  complete file-tool JSON rather than relying on malformed-output repair.

## UI and E2E testing

UI tests operate in two modes:

- browser mode against the React app and fake daemon for fast deterministic flows;
- packaged Tauri mode for installer, IPC, daemon lifecycle, credential integration,
  filesystem dialogs, and OS-specific behavior.

Linux release qualification additionally runs the real `.deb` and AppImage through
`scripts/test-linux-packages.sh`: package install/repair/uninstall/reinstall, retained
external data, daemon restart, authenticated CLI/doctor/backup/MCP discovery, and
both GUI formats under an owner-controlled X11 display with captured screenshots.

Every major workflow is tested in simple and advanced mode. Accessibility checks
cover keyboard navigation, focus, labels, contrast, reduced motion, and readable
progress/errors. E2E tests preserve traces/screenshots only on failure and sanitize
private paths and content.
Desktop contract fixtures cover evidence/event co-loading, simple summaries,
advanced tool verification and artifact provenance, every provider family, explicit
billing/unknown-cost labels, and project discovery context. Responsive E2E asserts
the mode switch remains visible and the page does not overflow at the narrow layout.
Sites fixtures cross REST and MCP on one durable identity, prove exact health and
deployment replay, require publication approval before ledger claim, retain response
and process-log artifacts, and exercise the simple/advanced native dashboard plus
caller-owned CLI identities. Monitor fixtures prove durable restart recovery,
atomic occurrence/projection/notification settlement, coalesced repeated outages,
recovery alerts, and transactional pause/archive schedule synchronization. Release
qualification additionally requires restart-uncertain deployment and scheduler-
backed health transition scenarios on Windows and Linux.

## Local verification lanes

MCG does not use GitHub Actions or other GitHub-hosted build automation. All build,
test, packaging, scheduled, and release verification runs on owner-controlled local
machines. GitHub is source hosting only.

### Change verification

- documentation/link validation;
- Rust format, clippy with warnings denied, unit/integration/contract tests;
- frontend format, lint, typecheck, unit tests, and production build;
- deterministic browser E2E smoke suite;
- real-browser accessibility baseline pass/failure/replay scenarios;
- secret and dependency-policy checks.

### Extended/local scheduled

- full scenario/eval suite with fake providers;
- opt-in GPU45 and configured live-provider conformance;
- adversarial, fuzz, performance, and restart/recovery suites;
- Windows and Linux packaged application smoke tests.

M9 restart fixtures additionally prove that automatic ready shadow experiments
create one stable inactive-candidate run, rebuild the copied task plan with an
independent evaluator, never acquire host tools, and resume the same running
identity after durable storage is reopened. A missing provider route creates no
lease or model spend; crash-interrupted and failed leases remain explicit review
boundaries instead of automatic paid retries. The deterministic provider fixture
then completes every dependency-ordered candidate item using only the private
workspace filesystem plane, verifies the content-addressed result artifact, and
confirms the receipt prevents another model call.
Command-verifier tests separately prove unavailable isolation launches no host
process, qualified pass/fail results require a bounded content-addressed
transcript, malformed references remain inconclusive, and transcript bytes are
independently re-hashed and matched to the exact condition before trusted code
derives pass/fail from timeout and exit status. The automatic
promotion/restart scenario combines real private-file evidence with an injected
qualified command fixture. Cross-platform unit tests prove the Linux profile has
no network sharing, mounts the retained workspace read-only, executes in a
bounded tmpfs, preserves argument-vector semantics, rejects unsafe workspaces,
and creates no evidence when the sandbox executable is absent. On Linux, an
installed trusted bubblewrap additionally runs a live ephemeral-write and
`/etc/passwd` escape scenario. Cross-platform Windows tests prove the generated
Windows Sandbox profile disables network/device/redirection surfaces, enables
Protected Client, XML-escapes host paths, exposes exactly two read-only host
mappings, runs the package-local helper as `System`, preserves a direct argument
boundary, and produces no evidence or workspace effect when the CLI is absent.
The helper separately proves bounded link/reparse-safe copying, exact internal
paths, one-shot destination creation, environment clearing, and normalized
command failure. Live Win11 qualification must still prove ephemeral writes,
blocked network access, host-file protection, timeout/stop behavior, and
setup-failure handling before the Windows backend is production-qualified.
The same file-backed scenario simulates a crash after paired benchmark commit,
then reopens storage and proves the stable automatic decision promotes only the
independently verified candidate while retaining one evaluation, benchmark, and
decision history. A later durable seeded regression restores the exact baseline.
Seeded promotion/reversion evidence additionally proves automatic promotion
accepts only benchmark arms matching their durable run/evaluation records,
moves the head only for matching passing evidence, detects a later candidate
regression by evaluation ID, restores the exact baseline, and makes subsequent
reversion scans observation-only.

### Local release candidate

- signed installer, upgrade, rollback, backup/restore, and uninstall tests;
- Windows/Linux clean-machine matrices and long-running soak;
- macOS package build with an explicit best-effort support label until qualified;
- reproducibility, SBOM, provenance, and artifact signature checks.

`npm run test:release-soak` drives the optimized `mcg-daemon` through its real
authenticated loopback API. The default 60-second lane admits 2,000 tasks with
bounded concurrency, enforces cold-start, p95 latency, throughput, absolute-RSS,
and RSS-growth limits, continuously probes health, force-terminates the daemon,
then reopens the same SQLite data. It requires every task to survive, samples
ordered event history, and requires post-crash database/schema diagnostics and
exact durable counts. `npm run test:release-soak -- --quick` is the 10-second,
300-task developer lane; release evidence must use the default lane or stricter
explicit arguments on both Windows and Linux. Final release qualification also
runs the same harness for an owner-controlled multi-hour window; the one-minute
lane is a fast release regression gate, not a substitute for long-duration proof.

### Typed SSH live qualification

`mcg-runtime::live_ssh_inspect_survives_runtime_restart_without_reexecution`
proves a real pinned, agent-authenticated inspection and observation-only replay
after reopening durable storage. `mcg-tools/tests/ssh_live.rs` additionally proves
wrong-host-key rejection, literal POSIX argv handling, verified upload/download
SHA-256 round trip, and remote cleanup. Both fixtures skip unless
`MCG_LIVE_SSH=1`; connection metadata and pinned key material are supplied through
the documented `MCG_LIVE_SSH_*` environment variables, while credentials remain
only in the controller's SSH agent.

### Plugin execution qualification

`mcg-plugins` contract tests prove static trusted-native identity/routing and the
closed-authority WASI kernel. Sandboxed fixtures cover one bounded JSON result,
unknown-import denial, fuel exhaustion, memory/output ceilings, and the absence
of inherited environment or filesystem preopens even when a manifest declares
capabilities. Runner tests additionally prove bounded/malformed request rejection,
pre-execution module-digest rejection, and a real child process that consumes one
request, returns one result, keeps stderr empty, and exits. The plugin feature is
not release-qualified until the native UAT gates in
`docs/architecture/plugins.md` are complete. Package UAT invokes the installed NSIS, `.deb`, and
AppImage plugin runners with one real digest-bound WASI module, requires empty
diagnostic output, and proves digest tampering returns
`module_digest_mismatch` with a failing process status.

Parent-client process tests additionally prove fixed-name runner selection,
bounded one-shot pipes, a whole-transaction outer deadline with kill/reap cleanup,
and strict agreement between response schema, plugin identity, result/error
shape, and child exit status. These tests run natively on each owner-controlled
release platform; they do not use GitHub-hosted execution.

The daemon-side dependency shape is also a release contract:
`mcg-plugin-runner --no-default-features` must remain free of `mcg-plugins`,
Wasmtime, and WASI dependencies. The executable's required `executor` feature is
tested separately with the full sandbox kernel.

Catalog tests prove migration/reopen durability, atomic immutable revisions,
optimistic enable-state conflicts, exact invocation replay, immutable start and
terminal history, and crash conversion from `running` to `uncertain`. Diagnostic
inventories expose only plugin/invocation counts. Verified-signature claims are
negative-tested as unavailable until the cryptographic verifier boundary exists.

Runtime tests prove module-byte digest binding, disabled-by-default installation,
explicit optimistic enablement, durable start-before-dispatch, result-artifact
replay across restart, changed-input identity rejection, timeout uncertainty, and
content-free diagnostic counts. Artifact tests reject lowercase-identity forgery,
tampering, and deduplication over changed bytes. Packaging contracts require the
runner adjacent to the service in NSIS, `.deb`, and AppImage payloads while the
daemon dependency tree remains free of Wasmtime/WASI.

`mcg-plugin-trust` tests use fixed Ed25519 keys to prove that the versioned,
domain-separated package payload binds signer, canonical HTTPS registry, package,
manifest/capabilities, module digest, and length. They reject tampering,
cross-registry replay, malformed or cross-labeled trust roots, noncanonical
encodings, and verified claims on local imports. Runtime integration additionally
proves an invalid proof leaves both the artifact store and catalog untouched,
while a valid registry package is persisted as verified but disabled.

REST integration installs a local module, lists its active manifest, replaces
the full scoped grant set, and enables it with optimistic revisions. The same
fixture confirms the MCP inventory and configure tool observe and mutate the
same durable head. CLI parser/integration tests keep install, grant, enable,
invoke, and history operations explicit; desktop component tests prove an owner
cannot enable a capability-bearing plugin until an exact scope has been granted.

Portable-backup tests install and enable a real catalog revision, record a
successful invocation, then prove the bundle contains the exact module and
result artifacts. Offline restore retains immutable revision/invocation history
and destination credentials while incrementing and disabling the restored
installation head. Missing modules, broken foreign keys, inconsistent typed JSON
projections, and missing successful-result artifacts fail verification before a
database swap.

Capability-grant storage tests prove strict manifest subsets, exact scope
normalization, optimistic conflicts, enable refusal for incomplete grants,
disable-on-change, and reopen durability. Protocol tests reject undeclared
operations, malformed credential references, duplicate turn identities, and
excessive histories. Runtime integration proves a declared and owner-granted
project read executes only through the durable broker, produces decision and
completion audit records, and returns a bounded host outcome on the next guest
turn. Cross-platform qualification must additionally cover approval/retry,
origin/executable/credential denial, turn limits, tool-success/runner-failure
uncertainty, and real packaged sidecar execution before consumer routing is
enabled.

## Coverage and release policy

Coverage is a diagnostic, not a substitute for outcomes. Critical policy, state
machine, migration, redaction, and billing-mode code requires branch coverage and
negative tests. Changes cannot lower agreed thresholds without an explicit review.

A release is blocked by failing deterministic tests, unresolved critical security
findings, migration/rollback failure, installer failure on a qualified platform,
or unmet scenario thresholds. Flaky tests are treated as defects: quarantine must
have an owner, reason, issue, and expiry.

## Local command contract

The repository converges on these stable entry points:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm ci
npm run check
npm run test
npm run build
npm run test:e2e
```

Platform-specific setup belongs behind scripts or task-runner targets so every
developer and release machine executes the same logical commands.
