# Project Memory

Durable project context lives here. Update this file whenever information should survive chat context compression or future agent handoff.

## Active Handoff

- Active long-term goal: Deliver the complete PRD as a production-ready general-consumer app on Windows and Linux; completion requires comprehensive local automated qualification and agent-performed UAT through computer-use or equivalent real user-facing interaction on both platforms.
- Current task: M6 interface-family work continues while live OAuth UAT waits for owner authentication. MCP current HTTP management is available through API, CLI, and native desktop; remote tools execute through governed agent leases.
- Last meaningful change: Added native simple/advanced MCP management with durable listing, live add/discover, optional protected bearer entry, unit coverage, production frontend build, native Rust checks, and Playwright smoke coverage.
- Next step: Add stdio/current-to-legacy interoperability and a full public-run remote MCP scenario, then qualify the complete MCP slice on Windows/Ubuntu. Preserve the visible OAuth handoff.
- Blockers: None documented.

## Current State

- MCG now has a compilable implementation foundation alongside its product specifications.
- The complete vision remains in scope and will be delivered through integrated, end-to-end milestones rather than a throwaway narrow prototype.
- Canonical requirements are indexed by `MCG_PRD.md` and split across `docs/spec/`; the encoding-damaged source is retained at `docs/archive/MCG_PRD.original.md`.
- The Git repository uses `main`, with `origin` at `https://github.com/fourtytwo42/Metacognitive-Governor.git`; the GitHub repository is private (last verified 2026-08-13).
- The workspace contains shared Rust crates, runnable `mcg-daemon` and `mcg` CLI applications, and a Tauri 2/React desktop shell with simple/advanced modes.
- GitHub Actions is disabled at repository level and its workflow/Dependabot files are removed; GitHub is source hosting only.
- M1 task state is durable in SQLite, rebuilt from its authoritative event log at startup, protected by a persistent local bearer token, and exposed through authenticated REST plus resumable SSE.
- M6 has an authenticated stateless MCP `2026-07-28` Streamable HTTP server with deterministic read/query tools and an idempotent governed single-turn mutation. MCP client support and expanded run-control/evaluation contracts remain in scope.

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
- M5 uses a separate `mcg-agents` boundary for versioned organization genomes, roles/instances, work plans, agent leases, success conditions, evaluation evidence, confidence/calibration, and organizational-cost metrics.
- The M6 provider proxy is authenticated by the installation bearer token and never executes externally supplied caller tools. `/v1/responses` and `/v1/chat/completions` share exact provider routing/fallback rules; authentication errors cannot cross provider or billing boundaries. `mcg:cheap` requires known cost and `mcg:local` accepts only loopback/private endpoints.
- The M6 MCP server uses one stateless `POST /mcp`; it requires per-request protocol/capability metadata and matching `MCP-Protocol-Version`, `Mcp-Method`, and tool `Mcp-Name` headers. Present Origins must be literal loopback/localhost and are rejected before authentication otherwise. Read/query tools return private-cache structured evidence; the agent-run mutation is explicitly marked destructive/open-world and remains policy-governed.
- The `mcg-mcp` client boundary stores only secret-free connection metadata/catalogs in migration 9 and bearer secrets under a separate OS credential-store service. Current HTTP configuration performs `server/discover` and `tools/list` before committing; public plaintext endpoints and URL-embedded credentials are rejected. Explicit remote agent tools use canonical `mcp::connection::tool` requests, per-lease aliases, immutable internal bindings, conservative catalog annotations, and the existing policy/approval/audit/execution-lease broker; the executor revalidates enabled catalog identity/classification before every call.
- Migration 8 atomically reserves interface-owned runs under an immutable request key/SHA-256 input hash. `mcg_run_agent` requires explicit Coder/Tool Worker tool lists, executes through the existing provider/policy broker, never treats a model response as objective completion, and returns existing success/non-success evidence rather than duplicating work on retry.
- M5 SQLite migration 5 makes organization revisions, plans, usage, and evaluations immutable evidence while runs, instances, and leases are event-backed current projections. Startup never resumes stale authority: interrupted runs become `recovering` and active instances/leases expire before reassignment.
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
- 2026-08-14: Exact governed MCP mutation commit `c14fef2` passed `./scripts/verify.sh` from fresh archive `/home/hendo420/mcg-worktree-c14fef2` on the owner-controlled Ubuntu VM; a `set -e` confirmation emitted `__VERIFY_PASSED__` after all Rust, docs, frontend, native Tauri, and Playwright gates.
- 2026-08-14: Windows and Ubuntu live daemon/CLI smoke tests created and transitioned a task, terminated the daemon, restarted it, and recovered the same projection with two ordered events; Ubuntu token mode verified as `0600`.
- 2026-08-14: M2 provider fixture tests passed for model discovery, canonical response/stream normalization, bounded transient retry, credential redaction, and OAuth/API billing-boundary enforcement; opt-in live GPU45 discovery and Responses inference passed against the dynamically reported active model.
- 2026-08-14: Complete Windows and Ubuntu local verification passed with M2: Rust format/clippy/workspace tests, documentation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E. The common live GPU45 discovery/buffered/streaming probe also passed independently from both platforms.
- 2026-08-14: Standalone OpenAI subscription OAuth runtime/API/CLI/desktop fixture tests passed for authenticated device login, one-time session consumption, cancellation, strict subscription billing isolation, secret-free responses/events, access-token account routing, refresh rotation, and injected protected-store rollback. Full platform qualification and live-account UAT remain pending.
- 2026-08-14: The complete standalone OAuth checkpoint passed `scripts/verify.ps1` on Windows and `scripts/verify.sh` on the owner-controlled Ubuntu VM, including strict Rust gates, restart tests, documentation, frontend tests/build, native Tauri builds, and Playwright. No GitHub-hosted automation ran.
- 2026-08-14: Computer-use UAT on the real Windows Tauri app found two usability defects: Advanced mode was below the initial viewport and `window.open` did not create a browser tab. A sticky sidebar and native `auth.openai.com`-only launcher fixed both; retest visibly reached Chrome's real OpenAI login page. Complete Windows and Ubuntu gates passed again after the fixes. Authentication itself was not automated and remains an owner handoff.
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
- 2026-08-14: M4 commit `41abefc` passed `scripts/verify.sh` on the owner-controlled Ubuntu VM, including native Tauri and Playwright. A real Linux daemon/CLI smoke then discovered the mirrored repository, corrected `npm` to `pnpm`, retrieved the correction in context, killed/restarted the daemon, and recovered both provenance-bearing versions.
- 2026-08-14: M5 contract foundation added 2 focused tests for organization graph validation/round-trip, lease budget exhaustion, and objective overhead. `cargo test --workspace`, focused clippy with warnings denied, and `git diff --check` passed on Windows.
- 2026-08-14: M5 minimal-organization planning added 4 focused tests. The consumer default has the five required core roles; atomic rename work selects only Coder with zero coordination overhead; compound site work selects only Executive/Coder/Tool Worker/Evaluator, injects independent evaluation, tracks dependencies, and preserves aggregate budgets; dependency cycles fail closed. Complete Rust workspace tests, focused strict clippy, formatting, and `git diff --check` passed on Windows.
- 2026-08-14: M5 migration 5 file-backed qualification persisted an organization, planned/run state, instance, active lease, attributed provider usage, and objective evaluation; after close/reopen it recovered the run/instance/lease to explicit safe states and retained ordered event evidence. Complete workspace tests and strict workspace clippy passed on Windows.
- 2026-08-14: M5 bounded turn tests route a context-constrained Coder through a priced capable model, decode `fs.read` into the typed tool plane, feed its broker outcome back using exact Responses function-call items, and return a final response. Authentication failure makes one provider call with no fallback; an unleased `fs.write` produces zero tool calls. Complete workspace tests and strict workspace clippy passed on Windows.
- 2026-08-14: M5 runtime turn commit test used the durable runtime, organization/run/instance/lease state, a two-call provider fixture, and the real policy broker to execute typed `fs.read`. Migration 6 atomically stored one immutable final result, three usage records, assigned model, run metrics, completed instance/lease, and ordered events. Restart evidence also verifies approval waiting retains authority for resume while later failure creates no usage/result. Complete workspace tests and strict workspace clippy passed on Windows.
- 2026-08-14: Safe dispatch focused tests passed on Windows. SQLite rejects duplicate/incomplete/excess authority and atomically creates a running instance, active work lease, and three events. The authenticated execution API test rejects anonymous use, calls a configured compatible fixture, and durably completes instance, lease, usage, and result. Provider probes now include required function-call behavior and missing model metadata remains unknown. Live GPU45 passed buffered and streaming Responses; its active model ignored the required tool call, so tools remain unknown and it is not routed tool-using Coder work.
- 2026-08-14: Complete Windows `scripts/verify.ps1` passed for safe dispatch: strict Rust format/clippy/workspace tests, docs validation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E. No GitHub-hosted automation ran.
- 2026-08-14: Exact commit `d8d56b9` passed clean Ubuntu `scripts/verify.sh`: strict Rust format/clippy/workspace tests, docs validation, frontend typecheck/unit/build, native Linux Tauri debug build, and Playwright E2E. The mirror was synced from a Git archive and no GitHub-hosted automation ran.
- 2026-08-14: Authenticated M5 API/CLI control surfaces created a consumer organization, planned atomic and dependency-ordered work, started a run, and returned its full durable evidence view. Real restart smoke found task replay's delete/reinsert strategy violated agent-run foreign keys; replay now upserts authoritative tasks and only removes unexpected orphans. The repaired database restarted into `recovering`, CLI inspection passed, and `scripts/verify.ps1` passed the full Windows Rust/frontend/docs/native/Playwright gate.
- 2026-08-14: Configured provider registry added native OS credential-store isolation, exact registered/discovered target dispatch, SQLite migration 7 connection/catalog evidence, and authenticated `mcg provider` API/CLI operations. Failed initial discovery is atomic and leaves no connection, route, or credential slot. Full Windows verification passed. Real CLI GPU45 discovery found 10 models at `http://192-168-50-189.nip.io:30001/v1`, and restart rediscovery returned the same catalog; the temporary daemons were stopped.
- 2026-08-14: Exact commit `8c93506` passed `scripts/verify.sh` on Ubuntu, including the Linux Secret Service/keyring dependency, all Rust gates, frontend/docs, native Tauri build, and Playwright. After explicitly building runnable binaries, a real Linux daemon/CLI configured GPU45, listed 10 models, stopped/restarted, lazily reconstructed the registry, rediscovered 10 models, and stopped cleanly.
- 2026-08-14: The deterministic M5 website scenario passed the complete Windows `scripts/verify.ps1` gate. A tool-capable provider emitted three `fs.write` calls for accessible HTML, responsive CSS, and interactive JavaScript; the real policy broker confined and atomically verified each write, six immutable decision/completion audits were stored, and the terminal two-call provider result replayed from the durable run view. Strict Rust, live GPU45, docs, frontend unit/build, native Tauri, and Playwright checks all passed locally; no GitHub-hosted automation ran.
- 2026-08-14: Exact commit `f573cff` passed `scripts/verify.sh` on the owner-controlled Ubuntu VM: strict Rust format/clippy/workspace tests including the governed website and live GPU45 scenarios, documentation, frontend unit/type/build, native Linux Tauri build, and Playwright all passed. The first SSH invocation lacked login-shell toolchain paths and the next transport window expired mid-build; rerunning through the configured login shell with a sufficient local timeout completed successfully. No GitHub-hosted automation ran.
- 2026-08-14: M5 evaluation authoring checkpoint (full cross-platform qualification pending): runtime/API/CLI can record immutable `EvaluationReport` files; plan- or genome-required independent review now requires a same-run Evaluator instance. The governed website fixture executes Coder and Evaluator turns, rejects unauthenticated and evaluator-less reports, stores three objective write hashes, and replays both turn results plus the report. Focused strict clippy and 30 API/CLI/runtime/storage tests passed while the visible OAuth UAT daemon/app remained running; complete verifier is intentionally deferred until that owner handoff is idle.
- 2026-08-14: Exact evaluation commit `8e3188a` passed complete owner-controlled Ubuntu `scripts/verify.sh`: strict Rust format/clippy/workspace tests, the independent-evaluator website scenario, live GPU45, docs, frontend unit/type/build, native Linux Tauri build, and Playwright all passed. Windows full verification remains deferred only to preserve the visible uncompleted OAuth browser session; focused Windows strict checks for changed packages are green.
- 2026-08-14: Initial M6 OpenAI-compatible proxy checkpoint (full cross-platform qualification pending) added authenticated `/v1/models`, `/v1/responses`, and `/v1/chat/completions`; deterministic alias/capability/cost/local routing; bounded retriable fallback; Responses and Chat buffered/stream normalization; Chat `[DONE]`; usage and physical-route evidence; standard null-content tool history; caller-owned function-call passthrough; and fail-closed `host::`/`mcg::` handling. Three proxy tests plus strict API/provider/runtime clippy and 43 focused tests passed on Windows while the OAuth UAT app remained open.
- 2026-08-14: Exact provider-proxy commit `ba8580b` passed complete owner-controlled Ubuntu `scripts/verify.sh`: all strict Rust/workspace tests including 16 API/proxy tests and live GPU45, documentation, frontend checks/build, native Linux Tauri build, and Playwright passed. Windows full verification remains deferred only to preserve the still-visible owner OAuth handoff; its focused proxy checks are green.
- 2026-08-14: Initial MCP server checkpoint passed Windows focused qualification: 19 `mcg-api` tests, strict full-workspace clippy with warnings denied, formatting, docs validation, and diff hygiene. Contract evidence covers pre-auth hostile-Origin rejection, auth, mirrored-header mismatch, modern discovery, stable tools, status, exact project query, bounded context, memory search, structured results, and encoded `Mcp-Name`. Full workspace tests reached only the expected Windows access-denied replacement of the live `mcg-daemon.exe`; PIDs 85024/74436 remained active and provider listing was still empty, so the visible uncompleted OAuth handoff was preserved. Ubuntu qualification is next.
- 2026-08-14: Exact MCP server commit `63d3dd4` passed complete owner-controlled Ubuntu `scripts/verify.sh` from a fresh Git archive: strict full-workspace Rust checks/tests including 19 API/MCP tests and live GPU45, documentation, frontend type/unit/build, native Linux Tauri debug build, and Playwright all passed in 215 seconds. No GitHub-hosted automation ran. The fresh mirror is `/home/hendo420/mcg-worktree-63d3dd4`; the older canonical mirror was left untouched because destructive replacement was denied.
- 2026-08-14: Governed MCP mutation checkpoint passed Windows focused qualification: full-workspace clippy, 19 API tests, 4 runtime tests, 12 storage tests, docs, formatting, and diff hygiene. A provider-backed MCP call creates and executes one durable run; exact retry returns the same one result/instance, a changed-input key conflicts, and `mcg_query_run` replays evidence. The file-backed migration-8 fixture proves atomic creation, restart replay, immutable key conflict, and zero duplicate events.

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
- Follow-up: M4 later passed identical Ubuntu qualification and live smoke; begin M5 organization and useful work.

### 2026-08-14 - M5 Organization Contract Foundation

- Context: Agent orchestration must be durable, budgeted, measurable, and replayable before model calls or host tools are attached to it.
- Decision: Create `mcg-agents` with exportable/versioned genomes; explicit Executive, Conversation, Coder, Evaluator, Tool Worker, Researcher, Tester, and temporary-specialist roles; persistent role definitions separated from ephemeral instances; acyclic delegation validation; bounded work leases; typed success conditions/evidence; basis-point confidence; calibration buckets; and an explicit coordination-overhead ratio.
- Files: `crates/mcg-agents/`, `crates/mcg-domain/src/lib.rs`, `Cargo.toml`.
- Commands/tests: `cargo test -p mcg-agents`; `cargo clippy -p mcg-agents --all-targets -- -D warnings`; `cargo test --workspace`; `git diff --check`.
- Result: The M5 data boundary compiles, validates imported graph/policy invariants, and leaves all prior tests green.
- Follow-up: Add minimal-organization selection and dependency-aware Executive planning.

### 2026-08-14 - M5 Minimal Organization Planning

- Context: MCG must avoid multi-agent ceremony for atomic work while safely coordinating genuinely compound tasks.
- Decision: Accept structured task intent rather than treating prompt keywords as authority; deterministically choose one canonical persistent role per declared capability; add the Executive only for decomposition/dependencies and Evaluator only when requested by task or organization policy; reject cycles and excessive estimated overhead; distribute bounded task budgets across work items without exceeding the parent.
- Files: `crates/mcg-agents/src/{organization,planning}.rs`, `MEMORY.md`.
- Commands/tests: `cargo fmt --all -- --check`; `cargo clippy -p mcg-agents --all-targets -- -D warnings`; `cargo test --workspace`; `git diff --check`.
- Result: Atomic and compound fixture plans produce the smallest sufficient organizations with explicit dependency, readiness, budget, evaluation, and overhead evidence.
- Follow-up: Persist the complete organization/run/lease/evaluation lifecycle and its replay evidence.

### 2026-08-14 - M5 Durable Run Evidence

- Context: Orchestration cannot safely attach real providers or tools until run authority, cost attribution, evaluation, and interruption behavior survive crashes.
- Decision: Use SQLite migration 5 for current run/instance/lease projections and immutable organization revisions, work plans, usage, and evaluation evidence; commit each lifecycle mutation with a versioned task event; attribute provider/model and tool usage without credentials; expire stale instance/lease authority and mark interrupted runs recovering during runtime startup.
- Files: `crates/mcg-{domain,events,agents,storage,runtime}/`, `docs/architecture/agent-organization.md`, `README.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `git diff --check`.
- Result: A real file-backed close/reopen fixture retains all immutable evidence and emits ordered recovery events for every interrupted projection.
- Follow-up: Build provider/context/tool-backed agent-run orchestration and authenticated control surfaces.

### 2026-08-14 - M5 Bounded Agent Turn Engine

- Context: Real useful work requires a provider-neutral model/tool loop without letting model output become authority.
- Decision: Add canonical Responses function-call/result input parts; route against discovered capabilities/cost/context windows and remaining lease budget; permit retriable fallback only before the first response and only when the role allows it; pin subsequent calls; expose exact lease tool names; decode arguments into `ToolRequestKind`; execute exclusively through an injected tool gateway/broker; cap model/tool iterations.
- Files: `crates/mcg-agents/src/orchestration.rs`, `crates/mcg-providers/src/{contracts,compatible}.rs`, `docs/architecture/agent-organization.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `npm run check:docs`; `git diff --check`.
- Result: Fake-provider/broker tests prove typed tool round-trip, cost evidence, authentication billing isolation, provider pinning, and zero side effects for unleased tool calls.
- Follow-up: Add runtime registry/persistence coordination so completed and failed turns atomically update durable run evidence and states.

### 2026-08-14 - M5 Atomic Runtime Turn Settlement

- Context: External model/tool work must never leave a successful-looking partial projection or lose its final replay evidence during normal persistence.
- Decision: Revalidate the complete durable authority tuple before execution; add migration 6 immutable turn results; atomically commit successful result, usage, assigned model, run metrics, completed instance/lease, and events; map approval-required to waiting with the lease retained; map other failures to failed instance/lease using stable non-sensitive reason codes and no fabricated usage/result.
- Files: `crates/mcg-{events,storage,runtime}/`, `docs/architecture/agent-organization.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `git diff --check`.
- Result: Runtime fixture executes a two-call model loop through the real policy broker and verifies the complete atomic projection/evidence result; storage restart fixture verifies waiting/resume/failure settlement.
- Follow-up: Register configured real providers and host broker capabilities, then expose organization/run control through authenticated API and CLI.

### 2026-08-14 - M5 Authenticated Organization And Run Control

- Context: Real users and scripts need safe durable control of organizations and work plans before configured providers can execute them.
- Decision: Expose authenticated loopback organization create/list/get and run plan/get/start-resume/cancel operations; accept stable client work keys and resolve them to typed dependency identities; prohibit external completion claims. Rebuild task projections with upserts so dependent run evidence remains foreign-key valid during restart.
- Files: `crates/mcg-{storage,runtime,api}/`, `apps/mcg-cli/`, `docs/architecture/agent-organization.md`, `README.md`.
- Commands/tests: Real daemon/CLI create-plan-start-restart-inspect smoke; `scripts/verify.ps1`.
- Result: The full Windows gate passes, including 9 API tests, 10 storage tests, all workspace tests/clippy, documentation, frontend, native Tauri build, and Playwright E2E. The interrupted run recovered with its plan intact.
- Follow-up: Build configured provider/broker registries and safe instance/lease dispatch, then repeat full qualification on Ubuntu.

### 2026-08-14 - Durable Configured Provider Registry

- Context: Useful runs need configured endpoints to survive restart without persisting secrets or allowing the router to call undeclared targets.
- Decision: Store secrets only through the native OS credential-store boundary; store validated connection metadata and model catalogs in migration 7; register and call exact provider/model identities; make initial discovery plus both projections/events atomic; accept API/bearer credentials only through request bodies and CLI standard input, never command arguments. OAuth connections remain restricted to the dedicated login flow.
- Files: `crates/mcg-{providers,storage,runtime,api}/`, `apps/mcg-cli/`, `docs/architecture/provider-fabric.md`.
- Commands/tests: `scripts/verify.ps1`; real daemon/CLI GPU45 configure/list/models/restart/discover smoke.
- Result: Full Windows qualification passes. GPU45 returned 10 dynamically discovered models before and after restart. A wrong console URL exposed failed-setup debris and opaque CLI errors; both were fixed with atomic setup rollback and surfaced API error bodies.
- Follow-up: Create bounded instances/leases and dispatch work through the real registry and tool broker.

### 2026-08-14 - Safe Provider And Tool Dispatch

- Context: Configured providers could not perform planned work until every runtime authority and host capability was derived and committed safely.
- Decision: Revalidate run/project/work/role/dependencies/budgets/concurrency server-side; atomically create running instance plus active lease and three events; expose fixed role tool schemas only; construct a project-confined secure broker before dispatch; settle every provider/tool outcome through the existing atomic turn boundary. Preserve absent model capabilities as unknown and grant tool routing only after an explicit required-call probe passes.
- Files: `crates/mcg-{storage,runtime,api,providers}/`, `apps/mcg-cli/`, `docs/architecture/{agent-organization,provider-fabric}.md`, `README.md`.
- Commands/tests: Focused provider/runtime/storage/API tests and strict clippy; live `MCG_LIVE_GPU45=1` probe; complete `scripts/verify.ps1` on Windows.
- Result: Authenticated provider-backed work execution reaches terminal durable evidence in the HTTP fixture and complete Windows/Ubuntu gates pass. GPU45 passed Responses and streaming, but its active model ignored a required function call, so MCG honestly retains unknown tool capability and will not route Coder tool work to it.
- Follow-up: Use a probed tool-capable provider for the real website fixture.

### 2026-08-14 - Standalone OAuth Provider Management

- Context: OpenAI subscription access had standalone protocol fixtures but no consumer-facing runtime lifecycle.
- Decision: Keep pending browser/device authorization bounded, expiring, in-memory, and consume-once; persist completed tokens only in the native credential store; persist a fixed ChatGPT Codex subscription connection with an initially empty catalog; keep model discovery explicit. Use the access-token account claim first with ID-token fallback. Rotate token sets transactionally with rollback and a secret-free event.
- Files: `crates/mcg-{providers,runtime,api,events,storage}/`, `apps/mcg-cli/`, `apps/desktop/`, `docs/architecture/provider-fabric.md`.
- Commands/tests: Focused strict Rust clippy/tests plus complete `scripts/verify.ps1` and owner-controlled Ubuntu `scripts/verify.sh`; frontend typecheck, two Vitest flows, production build, native Tauri builds, and Playwright smoke passed on both platforms.
- Result: Authenticated API fixture proves device start/complete/cancel/refresh, exact billing isolation, and response secrecy. CLI opens/prints verification and cancels on Ctrl+C. Desktop exposes simple browser login and advanced device-code selection through native daemon commands. Full local cross-platform qualification is green.
- Follow-up: Complete both-platform qualification and live OAuth UAT, then probe a subscription model for tool-capable real website work.

### 2026-08-14 - Modern MCP Server Foundation

- Context: M6 requires MCG to be an MCP server without reintroducing the retired connection-scoped handshake/session model or giving external clients an ungoverned host-effect path.
- Decision: Implement the current MCP `2026-07-28` stateless Streamable HTTP contract at authenticated `POST /mcp`; validate loopback Origins before auth, require body/header agreement, advertise only read-safe capabilities, and expose status/project/context/memory through existing runtime reads. Do not expose `mcg_run_agent` until its mutation can create durable governed task/run evidence.
- Files: `crates/mcg-api/src/mcp.rs`, `crates/mcg-api/src/lib.rs`, `crates/mcg-api/Cargo.toml`, `docs/architecture/mcp.md`, `README.md`.
- Commands/tests: `cargo test -p mcg-api`; `cargo clippy -p mcg-api --all-targets -- -D warnings`; `cargo fmt --all`; `npm run check:docs`; `git diff --check`.
- Result: Nineteen API tests pass on Windows. Four deterministic tools return structured/private-cache results and hostile Origins, malformed/mismatched mirrored headers, unknown tools, invalid inputs, and internal read failures fail without side effects or credential exposure.
- Follow-up: Add governed `mcg_run_agent`, MCP client configuration/discovery/namespace/policy execution, interoperability fixtures, and both-platform UAT.

### 2026-08-14 - Idempotent Governed MCP Agent Run

- Context: A stateless MCP mutation can be retried after a lost HTTP response; sequential REST-style creation would otherwise leave unknown partial state or duplicate costly/provider/tool work.
- Decision: Require a safe caller request key, hash canonical tool arguments, and use migration 8 to atomically persist the key, pending task, consumer organization revision, immutable plan, active task, and running run. Execute only a single bounded work item through existing runtime authority and policy. Exact success retries return existing evidence; non-success authority is returned as an MCP tool error; changed input under the same key conflicts.
- Files: `crates/mcg-{storage,runtime,api}/`, `docs/architecture/{agent-organization,mcp}.md`, `README.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p mcg-storage -p mcg-runtime -p mcg-api`; `npm run check:docs`; `git diff --check`.
- Result: Focused Windows qualification is green, and exact commit `c14fef2` passed the complete owner-controlled Ubuntu gate. The mutation never grants an implicit Coder/Tool Worker tool set and does not mark the task/run complete from model self-report alone.
- Follow-up: Implement MCP client configuration, protected auth, modern/legacy negotiation, discovery/cache, tool namespaces, agent-policy execution, interoperability, and UI/CLI management.

### 2026-08-14 - Durable Modern MCP Client Foundation

- Context: MCG needs to consume external MCP servers without storing bearer values in SQLite or letting a model-named remote tool bypass local authority.
- Decision: Add a separate `mcg-mcp` crate. Validate secret-free HTTP/stdio connection contracts; restrict plaintext HTTP to loopback; protect bearer credentials with a dedicated OS keyring service; implement current stateless `2026-07-28` HTTP discovery/call framing; sort and validate catalogs; and commit migration 9 metadata/catalog/events only after live discovery succeeds. Keep remote calls disconnected from agent execution until namespace/lease/policy integration exists.
- Files: `crates/mcg-mcp/`, `crates/mcg-{events,storage,runtime,api}/`, `apps/mcg-cli/`, `docs/architecture/mcp.md`, `README.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --exclude mcg-daemon --exclude mcg-desktop`; authenticated API fixture; CLI help smoke; `npm run check:docs`; `git diff --check`.
- Result: All applicable Windows Rust tests pass (the live OAuth handoff keeps daemon/desktop test binaries excluded), including 20 API tests, four MCP tests, and 13 storage tests. Current Streamable HTTP connections can be configured, rediscovered, listed, and inspected through authenticated API/CLI surfaces; setup rollback and durable restart fixtures retain no secret material.
- Follow-up: Implement stdio/legacy negotiation, namespaced agent policy execution, desktop management, interoperability fixtures, and cross-platform qualification.

### 2026-08-14 - Governed External MCP Agent Tools

- Context: A model must not gain external network authority merely by emitting a discovered MCP tool name, and remote annotations can drift after a lease is issued.
- Decision: Require explicit canonical `mcp::connection::tool` requests; translate them to bounded provider-safe aliases and immutable internal bindings; include only aliases in active tool permissions; reject model attempts to override connection/name/classification; route typed `external_mcp` calls through the existing broker; treat destructive/open-world calls as approval-requiring; and reload the enabled catalog plus classification immediately before transport execution.
- Files: `crates/mcg-{tools,agents,runtime,api}/`, `docs/architecture/mcp.md`, `README.md`.
- Commands/tests: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --exclude mcg-daemon --exclude mcg-desktop`; `npm run check:docs`; `git diff --check`.
- Result: All applicable Windows tests pass, including binding/argument-isolation, read-versus-open-world policy, live discovery/call normalization, runtime namespace/executor integration, and all prior API/provider/storage scenarios.
- Follow-up: Add stdio/current-to-legacy negotiation, desktop management/UAT, and one complete remote-MCP agent scenario through the public run surface.

### 2026-08-14 - Native MCP Management

- Context: The consumer release requires every primary interface on day one, with complexity hidden by default and advanced protocol/auth controls available when needed.
- Decision: Add native daemon commands and a desktop MCP panel. Simple mode accepts only server name/URL and chooses automatic current protocol with no auth; advanced mode reveals stable id, protocol pin, and bearer token. Load only secret-free durable list/catalog data, clear the submitted token after success, and provide explicit rediscovery.
- Files: `apps/desktop/src/{App.tsx,desktop-client.ts,styles.css,App.test.tsx}`, `apps/desktop/src-tauri/`, `tests/e2e/desktop-smoke.spec.ts`.
- Commands/tests: strict `mcg-desktop` clippy/tests; frontend typecheck; three Vitest flows; production Vite build; Playwright smoke; `git diff --check`.
- Result: All focused Windows native/frontend gates pass. Simple/advanced disclosure, current tool counts, setup request shape, and advanced browser-visible controls are automated.
- Follow-up: Perform real native Windows/Linux MCP setup UAT after stdio/legacy work and Linux qualification.

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
- 2026-08-14: Completed and verified M4 project discovery, target-scoped instruction resolution, polling invalidation, bounded context assembly, immutable snapshots, and versioned provenance-bearing memory across Windows and Ubuntu, including real daemon/CLI restart workflows.

## Open Follow-ups

- Choose a public-source license only if/when public distribution is considered; repository remains private.
- Include the full Apache-2.0 license text for adapted OpenAI Codex login code in release packages.
- Define the simple/advanced UI information architecture and visual system through prototypes and real project workflows.
- Add owner-controlled macOS build and real-device qualification when capacity becomes available.
- Continue M5 representative useful projects and broader end-to-end evidence while M6 interfaces are built; the durable provider/tool website and independent evaluator scenarios are complete checkpoints, not the full product.
- Persist standalone provider-proxy usage/conversations and attach optional governed project/task context; initial proxy responses currently return usage without a durable proxy ledger.
- Add independent browser/evaluator evidence to representative governed website work as M5 progresses.
- Complete MCP server subscriptions/interoperability plus MCP client stdio/legacy negotiation; add a public-surface remote-MCP agent scenario and real native MCP UAT.
- Qualify the newly wired standalone OpenAI OAuth provider management on Windows and Ubuntu, then perform live-account OAuth/model/tool UAT; do not expose tokens or fall back to API billing.

## Do Not Forget

- Read `AGENTS.md` and `MEMORY.md` before making future changes in this workspace.
- Update `MEMORY.md` after durable discoveries, decisions, fixes, verification results, blocked work, and handoffs.
- Never store secrets, tokens, passwords, private keys, or sensitive personal data in `MEMORY.md`.

## Archive

- No archived entries yet.
