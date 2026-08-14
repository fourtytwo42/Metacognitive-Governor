# Project Memory

Durable project context lives here. Update this file whenever information should survive chat context compression or future agent handoff.

## Active Handoff

- Current task: Product discovery is complete enough to begin implementation architecture and scaffolding.
- Last meaningful change: Incorporated the owner's remaining decisions and documented the verified GPU45 development-provider contract.
- Next step: Produce the implementation architecture, milestone dependency map, test strategy, and Rust/Tauri workspace scaffold.
- Blockers: None documented.

## Current State

- MCG is currently a documentation-only project describing a native, cross-platform metacognitive AI harness/runtime.
- The complete vision remains in scope and will be delivered through integrated, end-to-end milestones rather than a throwaway narrow prototype.
- Canonical requirements are indexed by `MCG_PRD.md` and split across `docs/spec/`; the encoding-damaged source is retained at `docs/archive/MCG_PRD.original.md`.
- The Git repository uses `main`, with `origin` at `https://github.com/fourtytwo42/Metacognitive-Governor.git`; the GitHub repository is private (last verified 2026-08-13).
- There is no application scaffold yet.

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
- OpenAI subscription OAuth is required; API-key access is supported separately but is not an acceptable silent fallback for broken OAuth.

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

- No build, test, or development commands exist yet because implementation has not been scaffolded.

## Project Map

- Project purpose: Build MCG (MetaCognitive Governor), a GUI-first, daemon-centered AI harness that can also serve as an OpenAI-compatible provider and MCP client/server.
- Main workflow: A persistent control plane routes user work through an Executive and model-agnostic agents into a typed, policy-governed tool execution plane while MCG observes and improves organization performance.
- External systems: Planned support includes OpenAI/Codex, OpenRouter, custom OpenAI-compatible model endpoints, MCP, host OS tools, Git, browsers/computer use, and SSH.
- Important paths:
  - `AGENTS.md`: repository-local agent workflow and memory rules.
  - `MEMORY.md`: durable context store.
  - `MCG_PRD.md`: current product requirements and phased vision.

## Architecture Notes

- Proposed stack in the PRD: Rust/Tokio/Serde/SQLite daemon, Tauri desktop UI, typed tool broker, event bus, filesystem artifact storage, and optional WASM/WASI sandboxed plugins.
- Architectural invariants: owner constitution outranks MCG; agents never bypass the execution plane; meaningful self-modification is versioned, measurable, and reversible; objective evaluation is preferred over self-reported success.

## Verification Ledger

- 2026-07-12: Read `AGENTS.md` and `MEMORY.md`; identified stale project-specific memory and preserved the repository-local instruction logic.
- 2026-07-12: Checked the workspace root; confirmed `AGENTS.md` and `MEMORY.md` are present and no `.git` directory is present at this level.
- 2026-08-13: Read all 60 sections of `MCG_PRD.md`; inspected the workspace with `rg --files` and `git status`; confirmed it contains only the three Markdown files and is not a Git repository.
- 2026-08-13: Ran `scripts/split-prd.ps1`; verified eight canonical spec files contain all 60 numbered sections exactly once and no known mojibake signatures.
- 2026-08-13: Verified GitHub CLI 2.86.0 is authenticated as `fourtytwo42` with repository permissions.
- 2026-08-13: Initialized Git on `main`, created root commit `e5e053d` (`Establish MCG product baseline`), created private repository `fourtytwo42/Metacognitive-Governor`, and pushed `main`.
- 2026-08-13: Verified official OpenAI documentation supports browser ChatGPT login and beta device-code login (`codex login --device-auth`) for local Codex clients; OpenAI's repository exposes this through the Rust `codex-login` crate.
- 2026-08-13: Read GPU45 `/llms.txt`, `/api/version`, `/api/health/summary`, and inference `/v1/models`; documented the connection at `docs/integrations/gpu45.md`.

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

## Known Issues

- The archived original PRD intentionally retains mojibake for provenance; canonical split specifications have repaired UTF-8 text.
- MCG must validate that reuse or adaptation of OpenAI's `codex-login` and/or OpenClaw OAuth code is license-compatible and remains functional without depending on the Codex agent runtime. OAuth failures must never silently switch users to API billing.
- macOS packages are intended to ship alongside Windows and Linux but initially will not receive equivalent automated or maintainer qualification; document this clearly until macOS CI/testing exists.

## Completed Work

- 2026-07-12: Reset stale project-specific memory while retaining the durable memory-management logic.
- 2026-08-13: Split and repaired the PRD, added the product/engineering baseline and repository hygiene files, initialized Git, and published the private GitHub repository.

## Open Follow-ups

- Choose a public-source license only if/when public distribution is considered; repository remains private.
- Research and document the exact license/security boundary for a standalone Rust ChatGPT/Codex OAuth adapter using browser callback and device-code flows.
- Define the simple/advanced UI information architecture and visual system through prototypes and real project workflows.
- Add macOS CI and real-device qualification when capacity becomes available.
- Produce the implementation architecture, milestone dependency map, test strategy, and Rust/Tauri workspace scaffold.

## Do Not Forget

- Read `AGENTS.md` and `MEMORY.md` before making future changes in this workspace.
- Update `MEMORY.md` after durable discoveries, decisions, fixes, verification results, blocked work, and handoffs.
- Never store secrets, tokens, passwords, private keys, or sensitive personal data in `MEMORY.md`.

## Archive

- No archived entries yet.
