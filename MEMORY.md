# Project Memory

Durable project context lives here. Update this file whenever information should survive chat context compression or future agent handoff.

## Active Handoff

- Active long-term goal: Deliver the complete PRD as a production-ready general-consumer app on Windows and Linux; completion requires comprehensive local automated qualification and agent-performed UAT through computer-use or equivalent real user-facing interaction on both platforms.
- Current task: M4 projects, context, and durable memory is implementation-complete and Windows-qualified; Ubuntu qualification remains before milestone closure.
- Last meaningful change: Added SQLite migration 4, authenticated runtime/API/CLI project and memory surfaces, target-scoped instructions, polling invalidation, versioned lifecycle transitions, and a real daemon/CLI restart smoke workflow.
- Next step: Commit/push the Windows-green M4 revision, run the identical owner-controlled Ubuntu verifier and live daemon/CLI restart smoke, fix platform defects, then close M4 and begin M5.
- Blockers: None documented.

## Current State

- MCG now has a compilable implementation foundation alongside its product specifications.
- The complete vision remains in scope and will be delivered through integrated, end-to-end milestones rather than a throwaway narrow prototype.
- Canonical requirements are indexed by `MCG_PRD.md` and split across `docs/spec/`; the encoding-damaged source is retained at `docs/archive/MCG_PRD.original.md`.
- The Git repository uses `main`, with `origin` at `https://github.com/fourtytwo42/Metacognitive-Governor.git`; the GitHub repository is private (last verified 2026-08-13).
- The workspace contains shared Rust crates, runnable `mcg-daemon` and `mcg` CLI applications, and a Tauri 2/React desktop shell with simple/advanced modes.
- GitHub Actions is disabled at repository level and its workflow/Dependabot files are removed; GitHub is source hosting only.
- M1 task state is durable in SQLite, rebuilt from its authoritative event log at startup, protected by a persistent local bearer token, and exposed through authenticated REST plus resumable SSE.

## User Preferences

- Keep `MEMORY.md` updated consistently with durable context that should survive chat compression.
- Agents should read `AGENTS.md` at startup and check `MEMORY.md` whenever context is missing, stale, or uncertain.
- Use uppercase filenames `AGENTS.md` and `MEMORY.md`.
- Build MCG for general consumers with simple installation and minimal required human regression testing.
- Keep every primary interface in the first complete release: native GUI, CLI, daemon/headless mode, OpenAI-compatible provider API, and MCP client/server.
- Deeply integrate automated unit, integration, scenario, contract, and end-to-end testing.
- Run builds and automated tests locally only. Do not use GitHub Actions or any GitHub-hosted automation that can incur usage charges.
- Use the owner-controlled Ubuntu VM at `192.168.50.109` for Linux desktop, daemon, CLI, and test qualification; never persist its credentials in repository files or memory.
- Production completion requires the agent to perform real user acceptance testing on both Windows and Linux through computer-use or equivalent user-facing interaction, fix material defects found, and leave minimal routine human regression work.
- Keep the full PRD vision in scope; use multiple real projects, initially websites/apps, to refine the system.
- Support OpenAI OAuth and API keys, OpenRouter, local OpenAI-compatible servers, and arbitrary OpenAI-compatible providers with user-supplied credentials.
- Discover provider models/capabilities from endpoints and probes, display known costs, and represent unknown costs honestly.
- Shadow model execution is opt-in for consumers and enabled during development/testing.
- Keep the repository private. Ship Windows, Linux, and macOS packages together, while initially qualifying releases on Windows and Linux only.
- Default to meaningful autonomy with configurable presets and advanced capability controls.
- Do not include built-in remote harness access initially.
- Use a simple-by-default UI that blends consumer chat, coding workspace, and operations views, with an advanced mode inspired by LM Studio.
- OpenAI subscription OAuth is optional but encouraged; API-key access is equally supported when explicitly configured. Never silently switch an OAuth connection to API billing.

## Source Of Truth

When sources conflict, use this order:

1. Current files in the workspace.
2. Latest command output or freshly verified tool output.
3. This `MEMORY.md` file.
4. Chat history.

## Freshness Rules

- Treat browser state, email state, authentication state, and external service results as stale unless verified in the current session.
- Treat documented repo structure, user preferences, and decisions as durable unless current files or fresh verification contradict them.
- Use `Last verified YYYY-MM-DD` for facts that can drift.

## Privacy Rules

- Do not store secrets, tokens, passwords, private keys, or sensitive personal data.
- Store only the minimum information needed for future handoff, especially when external systems or personal data are involved.

## Setup And Commands

- Prerequisites (last verified 2026-08-14): Rust 1.93+, Node.js 24+, and npm 10+.
- Install frontend dependencies: `npm install`; use `npm ci` for clean local verification.
- Complete Windows verification: `.\scripts\verify.ps1`; complete Linux verification: `./scripts/verify.sh`.
- Rust checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Frontend checks: `npm run check`, `npm run test`, `npm run build`, `npm run test:e2e`.
- Documentation check: `npm run check:docs`.
- Run daemon: `cargo run -p mcg-daemon`; query it with `cargo run -p mcg-cli -- status`.
- Run UI development server: `npm run dev`.

## Project Map

- Project purpose: Build MCG (MetaCognitive Governor), a GUI-first, daemon-centered AI harness that can also serve as an OpenAI-compatible provider and MCP client/server.
- Main workflow: A persistent control plane routes user work through an Executive and model-agnostic agents into a typed, policy-governed tool execution plane while MCG observes and improves organization performance.
- External systems: Planned support includes OpenAI/Codex, OpenRouter, custom OpenAI-compatible model endpoints, MCP, host OS tools, Git, browsers/computer use, and SSH.
- Important paths:
  - `AGENTS.md`: repository-local agent workflow and memory rules.
  - `MEMORY.md`: durable context store.
  - `MCG_PRD.md`: current product requirements and phased vision.
  - `docs/architecture/system.md`: implementation architecture and boundaries.
  - `docs/roadmap/milestones.md`: full dependency-ordered delivery plan.
  - `docs/testing/strategy.md`: automated evidence and release test strategy.
  - `Cargo.toml`: Rust workspace and shared dependency/lint policy.
  - `apps/mcg-daemon`: persistent local runtime process.
  - `apps/mcg-cli`: `mcg` terminal client.
  - `apps/desktop`: React/Tauri desktop application.
  - `crates/`: domain, events, config, policy, providers, tools, storage, runtime, and API boundaries.

## Architecture Notes

- Proposed stack in the PRD: Rust/Tokio/Serde/SQLite daemon, Tauri desktop UI, typed tool broker, event bus, filesystem artifact storage, and optional WASM/WASI sandboxed plugins.
- Architectural invariants: owner constitution outranks MCG; agents never bypass the execution plane; meaningful self-modification is versioned, measurable, and reversible; objective evaluation is preferred over self-reported success.
- Workspace dependency direction: stable domain/config/events at the center; policy/providers/tools/storage implement boundaries; runtime orchestrates; API and GUI/CLI are adapters.
- Initial daemon API binds to `127.0.0.1:7777`; built-in remote harness access is out of initial scope.
- Provider auth modes are explicit and billing-aware: browser OAuth, device-code OAuth, API key, bearer token, or none. OAuth failure never mutates a connection into API-key billing.
- The M1 event log is authoritative; task projections are written in the same SQLite transaction and rebuilt by validated replay at daemon startup.
- The local API rejects non-loopback binds. All routes except health require a generated persistent bearer token; SSE reconnects can replay from `after` or `Last-Event-ID`.
- M3 implements typed filesystem, process/shell, HTTP/download, system, Git/worktree/patch, and memory primitives behind one policy broker. SQLite durably stores approval state, execution leases, immutable tool audit, artifact registrations, and associated task events.
- M4 uses a separate `mcg-projects` boundary for stable project snapshots, hierarchical instruction resolution, context assembly, watched-input invalidation, world-model facts, and structured memory claims. Semantic indexes are optional derivatives, never canonical memory.
- Provider credentials are runtime-only redacting values and are not serializable; persisted provider connections contain an explicit auth mode but no secret. OpenAI-compatible requests omit absent optional fields, and buffered calls tolerate providers such as GPU45 that always return a terminal Responses SSE stream.
- OpenAI browser OAuth uses loopback PKCE; device OAuth uses the verification-code flow; both are standalone from Codex, support refresh-token rotation, and route subscription inference only through `https://chatgpt.com/backend-api/codex`. API-key billing is a separate explicit connection and never an OAuth fallback.
- M3 policy evaluates canonical resources, not prompt claims. Invalid or weakened owner constitutions fail closed on every broker request; audit and secret-isolation requirements cannot be disabled. Denied or approval-pending requests are audited before return and never reach a host executor.

## Verification Ledger

- 2026-07-12: Read `AGENTS.md` and `MEMORY.md`; identified stale project-specific memory and preserved the repository-local instruction logic.
- 2026-07-12: Checked the workspace root; confirmed `AGENTS.md` and `MEMORY.md` are present and no `.git` directory is present at this level.
- 2026-08-13: Read all 60 sections of `MCG_PRD.md`; inspected the workspace with `rg --files` and `git status`; confirmed it contains only the three Markdown files and is not a Git repository.
- 2026-08-13: Ran `scripts/split-prd.ps1`; verified eight canonical spec files contain all 60 numbered sections exactly once and no known mojibake signatures.
- 2026-08-13: Verified GitHub CLI 2.86.0 is authenticated as `fourtytwo42` with repository permissions.
- 2026-08-13: Initialized Git on `main`, created root commit `e5e053d` (`Establish MCG product baseline`), created private repository `fourtytwo42/Metacognitive-Governor`, and pushed `main`.
- 2026-08-13: Verified official OpenAI documentation supports browser ChatGPT login and beta device-code login (`codex login --device-auth`) for local Codex clients; OpenAI's repository exposes this through the Rust `codex-login` crate.
- 2026-08-13: Read GPU45 `/llms.txt`, `/api/version`, `/api/health/summary`, and inference `/v1/models`; documented the connection at `docs/integrations/gpu45.md`.
- 2026-08-14: Verified Rust 1.93.0, Cargo 1.93.0, Node 24.14.0, and npm 10.8.2; pinned jsdom 29.1.1 for Node 24 compatibility.
- 2026-08-14: `cargo check --workspace --all-targets`, Rust format, and clippy with warnings denied passed.
- 2026-08-14: `cargo test --workspace` passed 8 initial tests covering task serialization, defaults, policy invariants, auth billing modes, SQLite migration/event append, and API health.
- 2026-08-14: Frontend typecheck, Vitest (1 test), Vite production build, documentation/link validation, and Playwright E2E (1 test) passed.
- 2026-08-14: Live daemon/CLI smoke test passed for health, human status, and JSON status; the exact temporary daemon process was stopped afterward.
- 2026-08-14: The first GitHub Actions frontend run exposed non-deterministic workspace dependency placement: Vitest was hoisted while the explicit jsdom dependency was nested. Moved jsdom 29.1.1 to the root workspace, verified a clean `npm ci`, Vitest, and Playwright run, and updated checkout/setup-node to their current v7 major releases.
- 2026-08-14: GitHub Actions run `31771954125` passed all documentation, frontend, Ubuntu Rust, and Windows Rust jobs for commit `b02d626`.
- 2026-08-14: Disabled GitHub Actions at the repository setting and removed `.github/workflows/ci.yml` plus `.github/dependabot.yml`; all future builds/tests run only on owner-controlled machines.
- 2026-08-14: Installed Rust 1.93, Node 24.14, npm, Tauri/WebKit native libraries, and Playwright Chromium on owner-controlled Ubuntu 24.04 VM `192.168.50.109`; no VM password was stored in repository files or memory.
- 2026-08-14: Windows and Ubuntu local verification scripts passed Rust fmt/clippy/tests, documentation validation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E.
- 2026-08-14: Windows and Ubuntu live daemon/CLI smoke tests created and transitioned a task, terminated the daemon, restarted it, and recovered the same projection with two ordered events; Ubuntu token mode verified as `0600`.
- 2026-08-14: M2 provider fixture tests passed for model discovery, canonical response/stream normalization, bounded transient retry, credential redaction, and OAuth/API billing-boundary enforcement; opt-in live GPU45 discovery and Responses inference passed against the dynamically reported active model.
- 2026-08-14: Complete Windows and Ubuntu local verification passed with M2: Rust format/clippy/workspace tests, documentation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E. The common live GPU45 discovery/buffered/streaming probe also passed independently from both platforms.
- 2026-08-14: M3 contract/policy/broker checkpoint passed 14 focused tests plus formatting, clippy with warnings denied, and `git diff --check`. Tests cover schema round-trip, immutable constitutional gates, secret/protected/path-scope bypasses, zero-side-effect denial/approval, content-addressed artifacts, secret redaction, and duplicate active leases.
- 2026-08-14: M3 capability-safe filesystem checkpoint passed the complete Rust workspace test and clippy gates on Windows. Ten `mcg-tools` tests cover traversal/sibling denial, atomic write/patch/read, stale-hash conflict isolation, artifact hashes, approvals/audit, redaction, and leases. `cap-std` 4.0.2 confines file operations to pre-opened directory handles.
- 2026-08-14: Committed and pushed the M3 control-plane/filesystem checkpoint as `96a378f`; `main` matched `origin/main` afterward and no GitHub-hosted automation was used.
- 2026-08-14: Completed 27 `mcg-tools`, 8 policy, 7 storage, 3 runtime, and 7 API tests for the full M3 boundary. Focused tests use real temporary processes, repositories/worktrees, local HTTP servers, content-addressed artifacts, and SQLite lifecycle/restart behavior.
- 2026-08-14: Fixed `scripts/verify.ps1` to check every native command's `$LASTEXITCODE`. The prior script could print Cargo failures but still return success; the repaired script immediately exposed a missing test import and a fixed-delay process-test race, both of which were corrected.
- 2026-08-14: The repaired complete Windows verifier passed Rust format/clippy/workspace tests, documentation validation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E for the implementation-complete M3 revision.
- 2026-08-14: Synchronized commit `ec065d8` to the owner-controlled Ubuntu 24.04 VM and passed `scripts/verify.sh`: Rust format/clippy/workspace tests, documentation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E. No GitHub-hosted automation ran.
- 2026-08-14: M4 foundation added 6 tests for stable fixture discovery, Git-root resolution, instruction precedence/conflict surfacing, protected context compression, correction-driven durability, and secret-namespace rejection. Full Rust workspace tests and clippy with warnings denied passed on Windows.
- 2026-08-14: M4 SQLite migration 4 and authenticated interfaces passed 7 project tests, 9 storage tests, 8 API tests, and the full Windows verifier. Evidence covers immutable snapshots, root identity conflicts, target-scoped instructions, polling invalidation, atomic correction/lifecycle successors, secret-like key rejection, bounded context, and restart durability.
- 2026-08-14: Real Windows daemon/CLI smoke registered this repository as a project, added a plain-string claim, corrected it to `pnpm`, retrieved it in assembled context, force-stopped/restarted the daemon, and recovered both versions. The first attempt exposed PowerShell JSON-string quoting friction; CLI values now accept either valid JSON or ordinary strings, with a regression test.

## Decisions

### 2026-07-12 - Reset Stale Project Context

- Removed old project identity, history, setup details, external-system references, and follow-ups from this memory file.
- Preserved the memory-system rules, privacy guidance, source-of-truth ordering, freshness rules, and handoff structure.

### 2026-08-13 - Product And Engineering Baseline

- Context: The owner answered initial discovery questions and asked the agent to infer sensible defaults for the rest.
- Decision: Adopt `docs/decisions/0001-product-engineering-baseline.md` as the working product/architecture baseline.
- Files: `README.md`, `MCG_PRD.md`, `docs/spec/`, `docs/decisions/0001-product-engineering-baseline.md`, `scripts/split-prd.ps1`.
- Result: Consumer audience, full interface set, provider strategy, security defaults, runtime boundaries, memory behavior, testing layers, and delivery sequence are now explicit.
- Follow-up: Resolve the short list of owner-level choices covering license/visibility, platform release order, OAuth risk posture, remote/multi-user scope, local provider connection, and visual direction.

### 2026-08-14 - Implementation Foundation

- Context: The owner authorized moving from product discovery into architecture, milestones, testing, and scaffolding.
- Decision: Use the crate/process boundaries in `docs/architecture/system.md` and delivery order in `docs/roadmap/milestones.md`.
- Files: `Cargo.toml`, `apps/`, `crates/`, `.github/workflows/ci.yml`, `package.json`, `docs/architecture/`, `docs/roadmap/`, `docs/testing/`.
- Commands/tests: Rust check/fmt/clippy/test; npm documentation/typecheck/unit/build/E2E; live daemon/CLI smoke.
- Result: M0 foundation is compilable, tested, and ready for durable M1 behavior.
- Follow-up: Build M1 runtime spine without collapsing established interface boundaries.

### 2026-08-14 - Deterministic Frontend Test Runtime

- Context: GitHub Actions run `31771792457` failed on Ubuntu because root-hoisted Vitest could not resolve workspace-nested jsdom; the same test passed on Windows due to a differently resolved optional dependency.
- Decision: Declare jsdom at the npm workspace root and rely on the lockfile rather than Vitest's optional peer installation.
- Files: `package.json`, `apps/desktop/package.json`, `package-lock.json`, `.github/workflows/ci.yml`.
- Commands/tests: Clean `npm ci`, `npm run test`, and `npm run test:e2e` passed locally.
- Result: Frontend unit-test environment installation is deterministic across supported CI platforms.
- Follow-up: Begin M1 durable runtime spine implementation.

### 2026-08-14 - M1 Durable Runtime Spine

- Context: The owner authorized the complete M1 task/event, SQLite, recovery, local authentication, streaming, and restart-test slice and prohibited GitHub-hosted builds or paid automation.
- Decision: Use the append-only event log as authority, atomically maintain rebuildable task projections, require persistent local bearer authentication outside the public health route, and support durable SSE catch-up by sequence.
- Files: `crates/mcg-domain`, `crates/mcg-events`, `crates/mcg-storage`, `crates/mcg-runtime`, `crates/mcg-api`, `apps/mcg-daemon`, `apps/mcg-cli`, `docs/architecture/runtime-spine.md`, `scripts/verify.ps1`, `scripts/verify.sh`.
- Commands/tests: Both local platform scripts passed; native Windows and Linux desktop binaries built; real daemon/CLI forced-restart smoke tests passed on both platforms.
- Result: The requested M1 runtime spine is complete and cross-platform qualified without GitHub automation.
- Follow-up: Begin M2 provider fabric while retaining M1 invariants and local-only verification.

### 2026-08-14 - Production Completion Goal

- Context: The owner explicitly established the end gate for the full project.
- Decision: Keep the goal active until the complete PRD and all interfaces are production-ready on Windows and Linux, comprehensive tests pass on owner-controlled machines, installers and lifecycle behavior are qualified, and the agent completes real UAT on both platforms using computer-use or equivalent methods.
- Files: `MEMORY.md`; active Codex goal for this task.
- Result: Passing builds or automated tests alone cannot complete the goal; material defects, missing interfaces, unverified release gates, or substantial remaining human regression work keep it active.
- Follow-up: Continue with M2 provider fabric, then progress through the dependency-ordered roadmap and final cross-platform UAT.

### 2026-08-14 - M2 Provider Contract Foundation

- Context: M2 requires one canonical interface across OpenAI, OpenRouter, GPU45, and arbitrary OpenAI-compatible endpoints without obscuring authentication or cost provenance.
- Decision: Version provider contracts independently; persist secret-free connection metadata; keep credentials redacted and runtime-only; normalize Responses JSON/SSE; retry only pre-stream transient failures; omit absent fields on the wire; represent unreported prices as unknown.
- Files: `crates/mcg-providers/src/contracts.rs`, `crates/mcg-providers/src/compatible.rs`, `crates/mcg-providers/tests/gpu45_live.rs`, `docs/integrations/gpu45.md`.
- Commands/tests: `cargo test -p mcg-providers`; `cargo clippy -p mcg-providers --all-targets -- -D warnings`; `MCG_LIVE_GPU45=1 cargo test -p mcg-providers --test gpu45_live`.
- Result: Deterministic provider tests and live GPU45 discovery/inference pass. GPU45 currently returns SSE even for `stream: false`, which the adapter handles as an explicit conformance exception.
- Follow-up: Complete the remaining M2 routing and authentication layers.

### 2026-08-14 - M2 Provider Fabric Complete

- Context: The full M2 exit gate requires common provider behavior, explicit billing, standalone OAuth, dynamic discovery/probes, routing/cost behavior, and opted-in live evidence.
- Decision: Use separate browser/device OAuth and API-key connection modes; route OAuth only to the ChatGPT Codex subscription backend; never fall back on auth failure; pin each request to one deterministic route with bounded transient fallbacks; enable shadow policy by debug/test default and release opt-in.
- Files: `crates/mcg-providers/`, `docs/architecture/provider-fabric.md`, `docs/integrations/gpu45.md`, `THIRD_PARTY_NOTICES.md`.
- Commands/tests: `scripts/verify.ps1`; Ubuntu `scripts/verify.sh`; `MCG_LIVE_GPU45=1 cargo test -p mcg-providers --test gpu45_live` on both Windows and Ubuntu.
- Result: Nineteen provider unit/fixture tests and the common fake/fixture/live conformance paths pass. Full local platform gates pass, including native desktop builds and E2E. No GitHub automation ran.
- Follow-up: Begin M3. Add OS credential-store persistence when provider connection management is wired into runtime/UI, and include the full upstream Apache license text in distributable packaging.

### 2026-08-14 - M3 Control-Plane Foundation

- Context: M3 needs all host effects to pass through typed, policy-governed requests with durable evidence and no prompt-trust boundary.
- Decision: Version the complete primitive/approval/lease/artifact/audit contract first; evaluate resolved resource capabilities against a revalidated owner constitution; audit before invoking an executor; represent credentials as references resolved only into non-serializable executor values.
- Files: `crates/mcg-domain/src/lib.rs`, `crates/mcg-policy/src/lib.rs`, `crates/mcg-tools/src/{contracts,broker,secrets,evidence}.rs`.
- Commands/tests: `cargo test -p mcg-policy -p mcg-tools`; `cargo clippy -p mcg-policy -p mcg-tools --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `git diff --check`.
- Result: Fourteen focused tests pass. Denied and approval-pending fixture requests produce audit records and zero executor calls; artifacts are SHA-256 addressed; secret debug/output is redacted; duplicate active leases are rejected. A later same-day checkpoint integrated these controls into broker execution and completed the filesystem primitive family through capability handles.
- Follow-up: Implement the remaining host executors, then persist approvals/audits and expose daemon API/CLI lifecycle operations.

### 2026-08-14 - M3 Complete Execution And Evidence Plane

- Context: M3 requires every supported host effect to use typed contracts, constitutional capability checks, durable approvals/evidence, and externally observable verification.
- Decision: Keep host execution exhaustive behind `HostToolExecutor`; use exact executable/shell capability entries, project-confined filesystem/Git paths, credential references, bounded/redacted output, no automatic HTTP redirects, and atomic SQLite approval/audit/event transactions. Do not expose raw execution over transport until project orchestration can construct safe scoped capabilities.
- Files: `crates/mcg-tools/`, `crates/mcg-policy/`, `crates/mcg-storage/`, `crates/mcg-runtime/`, `crates/mcg-api/`, `apps/mcg-cli/`, `docs/architecture/tool-policy-plane.md`, `scripts/verify.ps1`.
- Commands/tests: Five repeated process lifecycle tests and `scripts/verify.ps1` passed on Windows; `scripts/verify.sh` passed on Ubuntu for the identical `ec065d8` archive.
- Result: All M3 primitive families and durable control surfaces are implemented and qualified on both initially supported platforms, including native desktop and E2E checks.
- Follow-up: Begin M4 projects, context, and durable memory.

### 2026-08-14 - M4 Project Intelligence Foundation

- Context: Project discovery, context, and memory need stable contracts before storage or interface adapters can safely depend on them.
- Decision: Create a dedicated `mcg-projects` crate. Snapshot only bounded non-symlink project inputs; resolve known instruction families with explicit precedence and scope; preserve goals/constraints/evidence before transcript during context compression; represent every durable claim with provenance, confidence, freshness, lifecycle state, and version linkage. Secret-like namespaces fail closed.
- Files: `crates/mcg-projects/`, `crates/mcg-domain/src/lib.rs`, `Cargo.toml`.
- Commands/tests: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `git diff --check`.
- Result: Six new deterministic tests and all prior workspace tests pass on Windows.
- Follow-up: Add SQLite project/claim persistence and authenticated runtime/API/CLI integration.

### 2026-08-14 - M4 Durable Project And Memory Plane

- Context: M4 exit requires stable fixture models plus correction/restart durability, not only in-memory contracts.
- Decision: Store immutable project snapshots with a current projection; enforce one canonical-root owner and one active memory head per scoped key; make correction, reinforcement, decay, and retraction successor versions committed atomically with events. Resolve instructions for a target path, keep semantic indexes derivative/optional, and expose the same behavior through authenticated API and CLI.
- Files: `crates/mcg-projects/`, `crates/mcg-events/`, `crates/mcg-storage/`, `crates/mcg-runtime/`, `crates/mcg-api/`, `apps/mcg-cli/`, `docs/architecture/project-intelligence-memory.md`.
- Commands/tests: `scripts/verify.ps1`; real `mcg-daemon` + `mcg project` CLI restart smoke on Windows.
- Result: Full Windows qualification passes. A real CLI usability defect with quoted string values was found and fixed during smoke testing.
- Follow-up: Run identical Ubuntu qualification and live smoke before closing M4.

## Known Issues

- The archived original PRD intentionally retains mojibake for provenance; canonical split specifications have repaired UTF-8 text.
- The standalone OAuth implementation is adapted from Apache-2.0 OpenAI Codex source, does not depend on the Codex runtime or credential files, and has fixture coverage for browser/device/refresh flows and billing isolation.
- OpenAI Codex source is Apache-2.0 licensed and the adapted standalone OAuth code is recorded in `THIRD_PARTY_NOTICES.md`; distributable packages must include the full upstream license text. OAuth endpoint/client compatibility remains an externally drifting integration and needs live UAT before release.
- macOS packages are intended to ship alongside Windows and Linux but initially will not receive equivalent owner-controlled qualification; document this clearly until a macOS build/test machine exists.

## Completed Work

- 2026-07-12: Reset stale project-specific memory while retaining the durable memory-management logic.
- 2026-08-13: Split and repaired the PRD, added the product/engineering baseline and repository hygiene files, initialized Git, and published the private GitHub repository.
- 2026-08-14: Completed and verified M0 architecture, roadmap, test strategy, Rust/Tauri/React workspace, CI, daemon/CLI smoke path, and initial UI/invariant tests.
- 2026-08-14: Completed and verified the requested M1 runtime spine on Windows and Ubuntu, then disabled and removed GitHub-hosted automation.
- 2026-08-14: Completed and verified M2 provider fabric on Windows and Ubuntu, including standalone OAuth fixtures and live GPU45 conformance on both platforms.
- 2026-08-14: Completed and verified M3 typed tools, constitutional capability enforcement, durable approvals/leases/audit/artifacts, worktree isolation, API/CLI control surfaces, and adversarial cross-platform qualification on Windows and Ubuntu.

## Open Follow-ups

- Choose a public-source license only if/when public distribution is considered; repository remains private.
- Persist provider credentials through Windows Credential Manager/Linux Secret Service when runtime connection management is added; never fall back to plaintext configuration.
- Include the full Apache-2.0 license text for adapted OpenAI Codex login code in release packages.
- Define the simple/advanced UI information architecture and visual system through prototypes and real project workflows.
- Add owner-controlled macOS build and real-device qualification when capacity becomes available.
- Complete Ubuntu M4 qualification and live restart smoke, then begin M5 organization and useful work.

## Do Not Forget

- Read `AGENTS.md` and `MEMORY.md` before making future changes in this workspace.
- Update `MEMORY.md` after durable discoveries, decisions, fixes, verification results, blocked work, and handoffs.
- Never store secrets, tokens, passwords, private keys, or sensitive personal data in `MEMORY.md`.

## Archive

- No archived entries yet.
