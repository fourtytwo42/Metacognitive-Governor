# Project Memory

Durable project context lives here. Update this file whenever information should survive chat context compression or future agent handoff.

## Active Handoff

- Current task: M0 repository and engineering foundations are implemented and verified; ready to begin M1 runtime spine.
- Last meaningful change: Added the implementation architecture, complete dependency-ordered roadmap, testing strategy, Rust workspace, daemon/CLI, Tauri/React desktop shell, CI, and initial invariant/E2E tests.
- Next step: Implement durable task creation, SQLite event/projection transactions, daemon restart recovery, local client authentication, and API event streaming from M1.
- Blockers: None documented.

## Current State

- MCG now has a compilable implementation foundation alongside its product specifications.
- The complete vision remains in scope and will be delivered through integrated, end-to-end milestones rather than a throwaway narrow prototype.
- Canonical requirements are indexed by `MCG_PRD.md` and split across `docs/spec/`; the encoding-damaged source is retained at `docs/archive/MCG_PRD.original.md`.
- The Git repository uses `main`, with `origin` at `https://github.com/fourtytwo42/Metacognitive-Governor.git`; the GitHub repository is private (last verified 2026-08-13).
- The workspace contains shared Rust crates, runnable `mcg-daemon` and `mcg` CLI applications, and a Tauri 2/React desktop shell with simple/advanced modes.

## User Preferences

- Keep `MEMORY.md` updated consistently with durable context that should survive chat compression.
- Agents should read `AGENTS.md` at startup and check `MEMORY.md` whenever context is missing, stale, or uncertain.
- Use uppercase filenames `AGENTS.md` and `MEMORY.md`.
- Build MCG for general consumers with simple installation and minimal required human regression testing.
- Keep every primary interface in the first complete release: native GUI, CLI, daemon/headless mode, OpenAI-compatible provider API, and MCP client/server.
- Deeply integrate automated unit, integration, scenario, contract, and end-to-end testing.
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
- Install frontend dependencies: `npm install` (use `npm ci` in CI).
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

## Known Issues

- The archived original PRD intentionally retains mojibake for provenance; canonical split specifications have repaired UTF-8 text.
- MCG must validate that reuse or adaptation of OpenAI's `codex-login` and/or OpenClaw OAuth code is license-compatible and remains functional without depending on the Codex agent runtime. OAuth failures must never silently switch users to API billing.
- macOS packages are intended to ship alongside Windows and Linux but initially will not receive equivalent automated or maintainer qualification; document this clearly until macOS CI/testing exists.

## Completed Work

- 2026-07-12: Reset stale project-specific memory while retaining the durable memory-management logic.
- 2026-08-13: Split and repaired the PRD, added the product/engineering baseline and repository hygiene files, initialized Git, and published the private GitHub repository.
- 2026-08-14: Completed and verified M0 architecture, roadmap, test strategy, Rust/Tauri/React workspace, CI, daemon/CLI smoke path, and initial UI/invariant tests.

## Open Follow-ups

- Choose a public-source license only if/when public distribution is considered; repository remains private.
- Research and document the exact license/security boundary for a standalone Rust ChatGPT/Codex OAuth adapter using browser callback and device-code flows.
- Define the simple/advanced UI information architecture and visual system through prototypes and real project workflows.
- Add macOS CI and real-device qualification when capacity becomes available.
- Implement M1 durable task/event/projection/recovery contracts, local client authentication, and streaming API.

## Do Not Forget

- Read `AGENTS.md` and `MEMORY.md` before making future changes in this workspace.
- Update `MEMORY.md` after durable discoveries, decisions, fixes, verification results, blocked work, and handoffs.
- Never store secrets, tokens, passwords, private keys, or sensitive personal data in `MEMORY.md`.

## Archive

- No archived entries yet.
