# Automated Testing and Evidence Strategy

MCG controls files, processes, networks, browsers, and model spending. Testing is
therefore part of the safety architecture, not a release cleanup activity.

## Test layers

| Layer | Purpose | Typical tools | Default execution |
|---|---|---|---|
| Rust unit/property | State machines, policies, budgets, parsers, redaction, routing. | Built-in tests, `proptest` where valuable. | Every change. |
| Migration/storage | Forward migrations, transactions, event/projection agreement, recovery. | Temporary SQLite databases. | Every change. |
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

## Website/application reference scenarios

The first real-project suite includes:

- create a small website from a goal and verify build, semantics, layout, and links;
- repair a seeded defect and prove regression tests fail before and pass after;
- run concurrent research and coding only when the task benefits from decomposition;
- restart the daemon during a long task and resume without duplicate modifications;
- restart while a guided objective batch awaits approval and resume without a
  repeated model call, completed check, or broadened command authority;
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
File-objective fixtures cover cross-platform relative-path parsing, traversal and
absolute-path rejection, regular-file-only enforcement, bounded streaming capture,
binary digest/size evidence, deduplication, missing files, and audit. Custom command
fixtures require an exact safely discovered manifest command and verify that arbitrary
prose or undiscovered commands cannot execute.
Visual-baseline fixtures cover pre-model missing-file rejection, immutable baseline
digest binding, post-binding mutation, bounded PNG decoding, dimension mismatch,
per-channel noise, changed-pixel thresholds, actual/baseline artifact provenance,
same-origin browser confinement, and terminal pass/fail settlement.

## API and compatibility testing

- Snapshot JSON schemas for application commands/events and task state.
- Run consumer-driven contracts for desktop, CLI, OpenAI, and MCP clients.
- Verify SSE/WebSocket ordering, reconnect cursors, cancellation, and backpressure.
- Test OpenAI-compatible streaming, tool calls, structured output, errors, and usage.
- Maintain provider-specific conformance exceptions as data with an expiry date.
- Fuzz parsers and reject oversized/deep/malformed untrusted payloads safely.

## UI and E2E testing

UI tests operate in two modes:

- browser mode against the React app and fake daemon for fast deterministic flows;
- packaged Tauri mode for installer, IPC, daemon lifecycle, credential integration,
  filesystem dialogs, and OS-specific behavior.

Every major workflow is tested in simple and advanced mode. Accessibility checks
cover keyboard navigation, focus, labels, contrast, reduced motion, and readable
progress/errors. E2E tests preserve traces/screenshots only on failure and sanitize
private paths and content.

## Local verification lanes

MCG does not use GitHub Actions or other GitHub-hosted build automation. All build,
test, packaging, scheduled, and release verification runs on owner-controlled local
machines. GitHub is source hosting only.

### Change verification

- documentation/link validation;
- Rust format, clippy with warnings denied, unit/integration/contract tests;
- frontend format, lint, typecheck, unit tests, and production build;
- deterministic browser E2E smoke suite;
- secret and dependency-policy checks.

### Extended/local scheduled

- full scenario/eval suite with fake providers;
- opt-in GPU45 and configured live-provider conformance;
- adversarial, fuzz, performance, and restart/recovery suites;
- Windows and Linux packaged application smoke tests.

### Local release candidate

- signed installer, upgrade, rollback, backup/restore, and uninstall tests;
- Windows/Linux clean-machine matrices and long-running soak;
- macOS package build with an explicit best-effort support label until qualified;
- reproducibility, SBOM, provenance, and artifact signature checks.

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
