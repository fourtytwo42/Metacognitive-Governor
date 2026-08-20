# Automated Testing and Evidence Strategy

MCG controls files, processes, networks, browsers, and model spending. Testing is
therefore part of the safety architecture, not a release cleanup activity.

## Test layers

| Layer | Purpose | Typical tools | Default execution |
|---|---|---|---|
| Rust unit/property | State machines, policies, budgets, parsers, redaction, routing. | Built-in tests, `proptest` where valuable. | Every change. |
| Migration/storage | Forward migrations, transactions, event/projection agreement, recovery. | Temporary SQLite databases. | Every change. |
| Owner settings | Authenticated reads/writes, optimistic conflicts, immutable reasons, live authority changes, and restart persistence. | Temporary SQLite reopen, REST contracts, CLI parsing, and desktop interaction tests. | Every settings change. |
| Backup/restore | Online snapshot consistency, exact temporary snapshot/WAL/SHM cleanup, artifact integrity, default secret exclusion, explicit authenticated encryption, catalog binding, tamper/traversal bounds, authenticated graceful daemon stop, offline locking, credential/database rollback preservation, and migration/reopen. | Temporary data directories, success/failure residue assertions, byte mutation, wrong-passphrase and failure injection, in-memory protected stores, API/CLI/desktop contracts, real process shutdown with sidecar checks, and cross-version package fixtures. | Every storage/release change. |
| Diagnostics | Stable version/severity contract, SQLite quick-check, authenticated access, secret/path/content exclusion, CLI strict exit behavior, and simple/advanced desktop disclosure. | Storage unit tests, REST boundary tests, CLI parsing/process tests, desktop interaction tests, and packaged smoke tests. | Every storage, auth, packaging, or support-surface change. |
| Project intelligence and memory | Immutable snapshots, bounded context, stable provenance-bearing topology, readable relationship-aware worker/evaluator context, secret-safe manifest parsing, provenance-bearing claim lifecycle, one active head, explicit freshness, bounded automatic decay/retraction, managed Markdown, and simple/advanced desktop disclosure of the world model and history. | Cross-language/Compose/Kubernetes/Helm/OpenAPI/deployment fixtures, selector-matched Service and Ingress routing, bounded chart dependencies/database edges, Secret/unrelated-YAML/Helm-values exclusion, malformed and oversized manifests, credential-bearing URLs, precise watch invalidation, tight-budget relationship ranking, authenticated REST assembly, a real prepared Evaluator turn, explicit clocks, bounded candidate queries, temporary SQLite reopen, repeat scheduler ticks without a user schedule, REST/CLI contracts, Tauri type/Clippy checks, component interaction tests, and Playwright consumer journeys. | Every discovery, memory, context, scheduler, dependency, or project-interface change. |
| Consumer CLI | Bare/interactive/project-selected/piped forms, bounded retained input, project identity reuse, human and JSON output, authenticated task-filtered NDJSON events, fast-result history reconciliation, non-completed exit failure, successful provider-backed worker/evaluator settlement, discovered project-command evidence, and token/context exclusion. | Parser tests plus real CLI child processes against an isolated loopback daemon, deterministic OpenAI-compatible provider, and temporary discovered project. | Every CLI, task launch, event, provider, evaluation, or project-discovery change. |
| MCP task lifecycle | Atomic request-key/task/run/consumer-launch reservation, decomposition, independent evaluation, objective settlement, exact replay, task/event/audit inspection, retry/resume delegation, evidence-report validation, request-scoped durable SSE progress, stream isolation, and task/run cancellation on explicit calls or disconnect. | Authenticated JSON/SSE MCP calls with deterministic and delayed providers, concurrent unrelated task events, dropped response bodies, typed project evidence, file-backed SQLite reopen, and the pinned official TypeScript SDK against a real loopback server. | Every MCP, consumer-launch, progress, evaluation, cancellation, or SDK-pin change. |
| Deterministic run replay | Stable digest, pinned task/organization/plan/authority/accounting/history invariants, tamper rejection, restart equality, and zero provider/tool/storage side effects across REST, MCP, CLI, Tauri, and desktop. | File-backed SQLite reopen, repeated report equality, mutated evidence validation, authenticated interface contracts, CLI parsing, and desktop interaction tests. | Every agent schema, event, audit, interface, or replay change. |
| Portable organizations | Secret-free bounded export, digest tamper rejection, fresh local authority, caller-idempotent restart-safe import, conflicting replay rejection, immutable revision history, and identity-independent comparison. | Agent contract tests, file-backed runtime reopen, authenticated REST/MCP integration, CLI parsing, desktop lost-response retry interaction, and Playwright duplicate/export/history/compare journey. | Every organization schema, portability, import, history, or comparison change. |
| Scheduler | Atomic occurrence/task claims, non-overlap, restart reclaim, cron/event/watch/heartbeat wake behavior, cursor and notification persistence, exact-definition creation replay/conflict across REST/MCP/desktop, budget/approval safe pause, and idempotent dispatch. | Explicit clocks, temporary SQLite databases, daemon reopen, real temporary files, fake providers/tools, component and browser UI flows. | Every scheduler change. |
| Computer control | Display/region confinement, frame-digest freshness, explicit input grants, handoff privacy, cross-interface authority, and at-most-once crash disposition. | Real safe capture, deterministic validation, file-backed SQLite reopen, REST/MCP contracts, desktop component flows, and native UAT. | Every computer-use change. |
| Provider proxy | Authentication, automatic/local/cost/capability routing, evidence-based fast/best aliases, caller-owned tool preservation, exact reserved namespaces, canonical managed-tool schemas, project/task binding, bounded project/task/memory injection, hash-only context receipts, prompt/output exclusion from durable accounting and dispatch evidence, project-confined host execution, bounded read-only MCG execution, approval/resume, terminal outcome accounting, internal-call buffering, at-most-once physical dispatch, and terminal buffered/streaming compatibility. | Deterministic OpenAI-compatible fixtures, measured-duration cold-start/minimum-sample/p95 ranking, capability/context ranking without model-name inference, schema-1 duration compatibility, temporary discovered projects, durable memory/task fixtures, caller-schema substitution, traversal/unknown/ownership rejection, simultaneous same-call retries, approval/denial audit assertions, SQLite close/reopen, pre-dispatch crash seeding with zero resend, atomic checkpoint-to-ledger settlement, synthesized Responses/Chat SSE, completed-replay refusal, and ledger inspection. | Every provider-proxy, context, routing, tool-turn, stream, dispatch-journal, or ledger change. |
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

Ensemble fixtures require two- and three-member deterministic providers plus a
separate synthesizer. They cover deterministic diverse selection, identical
governed context, bounded concurrency, one-member failure, insufficient-success
settlement, synthesis failure, structured output, conflicting caller/reserved tool
proposals, approval after synthesis, restart at every paid-response boundary,
dropped client responses, aggregate per-member usage/cost/duration, completed
replay refusal, and synthesized Responses/Chat streams with no intermediate
member leakage. Shadow fixtures prove the owner setting and request metadata are
both required, the secondary is distinct, its output/tool calls cannot affect the
active result, paid evidence remains content-free, and release defaults never
silently spend.

The first migration-28 fixtures are green for three concurrent members, one
content-free member failure with a two-success synthesis, aggregate accounting,
member-output exclusion, terminal Responses/Chat streaming, completed replay
refusal, and synthesized managed-tool approval across daemon reopen with exactly
one host effect. Insufficient-success, synthesis-failure, structured-output, and
conflicting-tool adversarial fixtures remain release gates before discovery.

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
6. secrets never appear in events, logs, context exports, fixtures, routine backups, or plaintext encrypted-backup bytes;
7. OAuth and API-key billing modes never change implicitly;
8. shadow executions cannot mutate real task state;
9. event history and current projections agree;
10. configuration experiments can always revert to their parent revision.
11. an uncertain computer-input outcome is never automatically replayed.
12. terminal failure, approval rejection, and cancellation preserve checkpointed
    provider/tool usage exactly once, remove continuation authority, and never
    create a successful result; task cancellation leaves no pending approval.

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
- deny one checkpointed turn and cancel a retried checkpointed turn, proving both
  paid provider calls remain in usage/metrics, neither creates a success result,
  both checkpoints disappear, and cancellation terminates the pending approval;
- require approval for deployment while allowing autonomous local build/test work;
- exercise a fake provider failure, model escalation, budget stop, and user steering;
- compare active and shadow models only under both opt-ins, require a distinct
  qualified secondary before active spend, accept output/tools only from the
  active run, retain exact content-free success/failure accounting, and serialize
  duplicate buffered and streaming call IDs through terminal ledger settlement;
- restart an approval-pending managed-tool shadow turn, prove its pinned secondary
  runs once, discard its output/tool proposal, resume only the active continuation,
  perform one governed effect, and distinguish active-response usage from aggregate
  ledger usage;
- run two/three conflicting ensemble members concurrently, treat candidate data
  as untrusted, preserve an exact structured-output schema through synthesis,
  publish only synthesis-selected tools, settle partial/synthesis failures, and
  resume managed approval after restart without repeated inference or effects.
- seed a durable physical dispatch before any owning checkpoint, close and reopen
  SQLite/runtime, retry the same logical call, and require an explicit
  indeterminate conflict plus zero requests at the provider fixture;

Success evidence combines tests, build output, Git diff constraints, HTTP health,
browser assertions, accessibility checks, and screenshots when visual comparison
adds information. Model self-report is never sufficient.
HTTP objective fixtures cover exact expected 2xx and non-2xx statuses, mismatches,
unreachable origins, response artifact capture, capability confinement, and audit.
TCP-listener fixtures cover exact typed endpoint parsing, credential/path/query
rejection, capability confinement, successful and refused bounded handshakes,
artifact-free evidence, terminal replay, and same-sandbox loopback-only benchmark
profiles on Linux and Windows. Runtime assessment tests additionally require the
automatic experiment evaluator to admit `PortListening` at the shared isolated
service boundary, bind the returned transcript to the exact condition, and derive
pass/fail without accepting a mismatched or backend-supplied verdict.
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
- Force malformed provider tool output through more than the automatic retry
  allowance; authenticated Retry must retain the task/run identity and create one
  fresh attempt, while non-owner dispatch remains bounded and all other authority
  checks stay active.
- Seed a private migration-29 dispatch and require diagnostics to expose only
  started/completed/failed counts plus actionable no-resend remediation; provider,
  model, response, logical-call, and client-request identities must remain absent.
- Upgrade an exact migration-29 started record through migration 30, settle a
  confirmed HTTP failure once, reject terminal reclassification/completion, and
  prove only ambiguous request-transport errors retain the indeterminate warning.

## UI and E2E testing

UI tests operate in two modes:

- browser mode against the React app and fake daemon for fast deterministic flows;
- packaged Tauri mode for installer, IPC, daemon lifecycle, credential integration,
  filesystem dialogs, and OS-specific behavior.

The deterministic real-browser lane runs axe against both Simple and Advanced
interfaces for WCAG A/AA violations. A separate keyboard-only journey traverses
every primary workspace in order, requires a visible focus indicator, changes
interface mode with Enter, and requires the asynchronously persisted mode control
to regain focus. Native package UAT still covers platform assistive behavior that
browser automation cannot prove.

Updater contract tests prove unsigned builds fail closed, configured builds expose
only their bounded HTTPS origin, and an available release requires a second review
step before invoking installation with a fresh backup identity. Native tests reject
missing/HTTP/credential-bearing/fragmented endpoints, malformed minisign keys,
noncanonical SemVer review identities, and stale/wrong-version/credential-bearing/
database-free pre-update backups.
Windows updater tests also parse real PE resources without executing the payload,
require every `ProductVersion` to be identical canonical SemVer, bind the reviewed
and observed values into staged-request schema 3, and prove a mismatch creates no
private staging directory or installer process.
The owner-machine `npm run test:updater-signing` lane generates a disposable Tauri
key, signs an artifact, verifies the exact Tauri envelope, proves byte tampering is
rejected, validates preflight with the generated trust root, and removes all private
key material even on failure. It also creates a static target-specific release feed,
proves the feed retains the exact signature without local paths, and requires an HTTP
download URL to fail. Its preflight covers both private-key content and path modes,
rejects both/neither, rejects a file path passed as key content, and rejects relative
or missing key paths before Tauri can wait for interactive input. It never uses
production signing authority.
The owner-machine `npm run test:updater-uat-server` lane qualifies the reusable
loopback HTTPS scenario fixture used by final package UAT. It creates disposable TLS
material, proves bounded manifest/artifact serving, atomic valid/corrupt/failure
scenario switching, traversal/query/method rejection, header-free logs, and cleanup.
The TLS generator keeps the private CA only in memory, emits only a 24-hour public CA
plus localhost server chain/key, refuses overwrite, and reports public fingerprints
for exact trust-store setup/teardown evidence. The contract rejects a shorter lifetime
because native updater fault qualification can legitimately outlast a two-hour window.
Both canonical owner-machine verification scripts run the packaging contract,
ephemeral signing/tamper lane, loopback scenario-server lane, and disposable
source/workspace preparation contracts; they cannot be silently skipped by the
normal Windows or Linux gate. The preparation tests generate a disposable Git-based
archive when no exact sealed archive is supplied, while retaining the explicit
archive argument for release-candidate qualification.
This fixture is infrastructure for the package matrix, not evidence that an update
was installed.
The updater package matrix must derive its baseline and candidate from one sealed
source archive through `npm run prepare:updater-uat-source`. Its contract rejects
unsafe archive entries while accepting canonical regular files and explicit
directories from standard Git archives, rejects divergent version surfaces, updates Cargo/npm/Tauri locks
offline, and commits each prepared tree cleanly. The preparation test proves a
same-version baseline and distinct newer prerelease candidate while rejecting
noncanonical versions and destination reuse. Build manifests and UAT evidence retain
both prepared commit identities and the common source-archive digest.
The disposable-workspace preparation contract composes those trees with lockfile-exact
npm dependencies, a new updater keypair, and the bounded TLS chain. Tests require the
public evidence descriptor to contain no private label or local workspace path and
require destination reuse, version reversal, and non-loopback origins to fail before
the matrix can build packages.
Scenario-generation tests sign a candidate and distinct decoy artifact with the same
disposable key, then require the valid signature/candidate pair to verify and the
decoy signature/candidate pair to fail. The generated matrix has strictly ordered
baseline, valid, and changed versions plus exact no-update, bad-signature, corrupt,
and unavailable feeds; output reuse and invalid ordering fail closed.
The changed-version lane is accepted only when the signed manifest version and the
verified Windows installer's embedded `ProductVersion` are identical. A feed that
advertises a different version for otherwise valid signed installer bytes must fail
before backup, daemon shutdown, staging, or installer execution.
The Windows matrix uses `scripts/windows-updater-uat.ps1` to make scenario changes
atomic, retain immutable secret-free snapshots, and inject only reversible backup or
install-directory DACL failures. Its fixture runs under Windows PowerShell 5.1 and
proves exact SID/right/type/inheritance/protection restoration plus cleanup while
allowing Windows to canonicalize equivalent ACE order, and refuses CA import or removal without
the action-time confirmation switch. Real trust setup additionally pins the public
CA file, subject, DER SHA-256, and thumbprint. Teardown re-verifies the one stored
current-user root directly, so loss of a feed marker or public CA file cannot strand
the disposable trust entry; it removes no other subject or thumbprint.
Final packaged updater UAT must use owner-controlled HTTPS plus an ephemeral signing
key to exercise the complete signed download, verified backup, graceful shutdown,
installation, relaunch, migration, failure recovery, and rollback matrix. A green
component fixture is not evidence that a platform installer updated successfully.
Windows installer-failure qualification must execute through the packaged
`mcg-updater.exe` supervisor because the upstream Tauri Windows installer path exits
the desktop immediately after launching NSIS. Acceptance requires the real installer
exit status, restored application launch and loopback service, bounded recovery notice,
unchanged reviewed baseline payloads, exact ACL restoration, and no staged request or
installer residue; an unreachable Rust branch after `Update::install` is not evidence.
Shutdown-failure injection must also assert that the service child is reaped, the
verified backup remains usable, the reviewed application binary is unchanged, and
the recovery result is reported without claiming installation success.

Windows installer qualification executes bare-goal, single-JSON, durable NDJSON
stream, piped-context, and oversized-input consumer CLI forms against the installed
daemon, including canonical project reuse and token-exclusion assertions. The true
interactive prompt is exercised visibly because redirected stdin is intentionally
the piped-goal interface on Windows.
It also creates and verifies routine and encrypted schema-2 bundles, proves no
passphrase or local token bytes occur in the encrypted container, requests an
authenticated graceful stop, requires absent WAL/SHM sidecars, rejects one wrong
passphrase, restores with redirected bounded input, preserves the local token and
rollback database, and reopens the restored daemon consistently.

Linux release qualification additionally runs the real `.deb` and AppImage through
`scripts/test-linux-packages.sh`: package install/repair/uninstall/reinstall, protected
Bubblewrap payload/source/license and AppArmor load/namespace/unload checks, retained
external data, daemon restart, authenticated CLI/doctor/backup/MCP discovery, and
both GUI formats under an owner-controlled X11 display with captured screenshots.
The installed CLI lane also executes bare-goal, single-JSON, durable NDJSON stream,
piped-context, oversized-input, and real pseudo-terminal interactive forms; it
requires blocked work to fail visibly when no provider exists, preserves one
canonical project, and rejects bearer-token leakage from every captured stream.

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
qualified command fixture and a real loopback research source whose immutable
baseline report survives restart. Research-verifier tests prove the valid bound
source is fetched exactly once; mismatched scope, candidate URL substitution,
malformed JSON, and missing baseline evidence add no request. They also prove
that failed-source details omit query values. Cross-platform unit tests prove the Linux profile has
no network sharing, mounts the retained workspace read-only, executes in a
bounded tmpfs, preserves argument-vector semantics, rejects unsafe workspaces,
and creates no evidence when the sandbox executable is absent. On Linux, an
installed trusted bubblewrap additionally runs a live ephemeral-write,
`/etc/passwd` escape, and owner-local `node`/`npm` execution scenario. Unit tests
prove the optional Node distribution mapping is read-only and bounded by bytes,
file count, depth, entry type, and non-escaping relative symlinks. Cross-platform
Windows tests prove the generated
Windows Sandbox profile disables network/device/redirection surfaces, enables
Protected Client, XML-escapes host paths, exposes exactly two mandatory read-only
host mappings plus exactly one optional bounded Node toolchain mapping, runs the
package-local helper as `System`, preserves the schema-3 JSON argument and typed
loopback-probe
boundary behind the CLI's single compile-time command value, rejects canonical
path-prefix confusion, and produces no evidence or workspace effect when the CLI
is absent.
The helper separately proves bounded link/reparse-safe copying, exact internal
paths, one-shot destination creation, environment clearing, and normalized
command failure. Unit coverage also rejects a missing, malformed, oversized, or
non-integer nested `ExitCode` envelope instead of confusing the host CLI's zero
transport status with runner success. The Windows sidecar builder statically
links the runner's C runtime and rejects Visual C++ redistributable DLL imports.
Live Win11 qualification on the current Store-delivered Sandbox CLI proves
official-package discovery, serialized start/exec/probe/stop compatibility,
real nested runner execution, ephemeral command writes, and no persistence
through the read-only host mapping. Windows Sandbox may satisfy a mapped-folder
write through its disposable view, so the gate measures host non-persistence
rather than assuming the child must fail. The same owner-controlled gate proves
no default route or non-loopback IPv4 address, a two-second command timeout with
verified session teardown, deliberate schema-invalid runner setup rejection,
and an empty session list after every case. The Node-equipped lane additionally
proves `node --version` and the Windows `.cmd`-backed `npm --version` execute
inside the real disposable sandbox without a writable host share. It then
starts a real Node HTTP service and performs both matching-status and
wrong-status probes beside that service in the same disposable network
namespace. An identically addressed host-loopback listener must observe zero
requests, and every service case must leave the Windows Sandbox session list
empty.
Service-verifier contract tests reject HTTPS, non-loopback hosts, implicit ports,
credentials, fragments, control characters, ambiguous snapshot commands, stale
source-run bindings, and malformed transcript artifacts. They prove completed
consumer settlement durably retains the original project snapshot even after a
later discovery changes the preferred package runner. The live Linux lane starts
a Node HTTP service and probes it inside the same explicitly user- and
network-unshared bubblewrap namespace while an identically addressed host
listener remains unreachable; it
requires exact-status success, wrong-status failure, zero host hits, and service
cleanup. The corresponding real Win11 lane uses runner schema 3 and must prove
the same pass/fail behavior with no Sandbox session left behind.
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
- Windows/Linux/macOS clean-machine matrices and long-running soak;
- native macOS app/DMG build, signature/notarization validation, and installed-app UAT;
- reproducibility, SBOM, provenance, and artifact signature checks.

Installed desktop UAT must exercise populated simple and advanced surfaces, not
only prove that a window opens. Organization portability specifically creates a
five-role organization, duplicates it through the native UI, opens its portable
JSON and immutable history, and compares two loaded revisions at 1280x800. The
browser regression requires every organization card to remain within its own
client width and preserve a readable summary column in both modes. This gate was
added after native Linux UAT found generic schedule-card flex styling squeezing
organization metadata into one-character columns even though component tests and
the operations themselves passed.

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
On a restricted Windows host that permits Node to launch only trusted system
executables, set `MCG_SOAK_WINDOWS_COMMAND_BROKER=1`. The harness then uses the
repository's PowerShell process broker to own the exact release daemon, publish
its real PID and RSS, force-stop it for the crash boundary, and kill it if the
Node parent disappears. This mode does not relax any recovery, integrity,
performance, or memory assertion; ordinary Windows and Linux hosts continue to
launch the daemon directly.

Long release lanes are launched with `scripts/run-release-soak-unattended.mjs`,
which requires absolute canonical paths plus predeclared SHA-256 identities for
both the soak harness and optimized daemon. It creates a new no-clobber evidence
directory, redirects the complete child output to separate logs, and atomically
writes `completion.json` with timestamps, arguments, exit/signal state, and the
candidate and log hashes. The launcher deliberately records no executable paths
or credentials. This makes multi-hour Windows and Linux qualification
unattended: operators inspect meaningful milestones and the final completion
record rather than watching the process continuously. A missing or unsuccessful
completion record is never release evidence.

`npm run verify:release-soak-evidence -- ...` is the separate read-only release
acceptance boundary. It reopens only a canonical non-symlink evidence directory,
recomputes both log digests, validates the exact harness/daemon identities and
requested workload, and requires a successful exit with empty stderr. It then
parses the harness result and independently requires exact task/event recovery,
passing database/schema diagnostics, bounded startup/restart/p95/throughput/RSS
metrics, and internally consistent memory growth. Its emitted summary contains
only hashes and bounded metrics; paths and credentials are excluded. Modified,
failed, partial, noisy, or structurally expanded evidence is rejected.

### Typed SSH live qualification

`mcg-runtime::live_ssh_inspect_survives_runtime_restart_without_reexecution`
proves a real pinned, agent-authenticated inspection and observation-only replay
after reopening durable storage. `mcg-tools/tests/ssh_live.rs` additionally proves
wrong-host-key rejection, literal POSIX argv handling, verified upload/download
SHA-256 round trip, and remote cleanup. Both fixtures skip unless
`MCG_LIVE_SSH=1`; connection metadata and pinned key material are supplied through
the documented `MCG_LIVE_SSH_*` environment variables, while credentials remain
only in the controller's SSH agent.

Multi-host workflow tests additionally prove immutable DAG validation, exact
definition replay, stale host-revision rejection, normalized capability routing,
persisted selection, no-match refusal, atomic state events, cancellation,
and both restart gaps: a claimed step becomes `uncertain` with every dependent
skipped and no recreated operation, while a durably succeeded operation is adopted
when its workflow projection was interrupted. Authenticated REST and MCP contract
tests create/query/cancel the same graph; desktop interaction tests prove the simple
ordered-host composer pins revisions and exposes terminal state. Release evidence
still requires a real two-host workflow from both Windows and Linux controllers,
including approval pause/resume and one forced daemon crash. Credentials remain in
the controller SSH agent and no GitHub-hosted runner is permitted.

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
result artifacts. Authenticated routine and encrypted API fixtures also inspect
the runtime-owned backup directory after creation and require the caller-prefixed
online snapshot plus its `-wal` and `-shm` companions to be absent; lower-level
storage cleanup alone is not sufficient evidence for the updater route. Offline
restore retains immutable revision/invocation history
and, for routine bundles, destination credentials while incrementing and
disabling the restored installation head. Encrypted-backup tests additionally
prove Argon2id/XChaCha20-Poly1305 round trips, no plaintext/debug exposure,
wrong-passphrase and metadata/ciphertext/AAD rejection, exact provider/MCP catalog
binding, unrelated-credential preservation, local-token preservation, and
credential rollback when a forced SQLite-sidecar conflict rejects installation.
Missing modules, broken foreign keys, inconsistent typed JSON projections, and
missing successful-result artifacts fail verification before a database swap.

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
