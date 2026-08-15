# Project Memory

Durable project context lives here. Update this file whenever information should survive chat context compression or future agent handoff.

## Active Handoff

- Active long-term goal: Deliver the complete PRD as a production-ready general-consumer app on Windows and Linux; completion requires comprehensive local automated qualification and agent-performed UAT through computer-use or equivalent real user-facing interaction on both platforms.
- Current task: Close M8 native Linux UAT after completing a dedicated clean Win11-VM consumer qualification checkpoint, then begin M9 measured improvement.
- Last meaningful change: Commit `1b475b2` pushed the clean Win11-VM UAT fixes and portable qualification launcher. UAT exposed and fixed the production-build invocation, a Windows CLI/GUI filename collision, the native project-picker's obsolete daemon route, hidden project errors, and blocked evidence incorrectly labeled in progress; the corrected app passed project/task/provider/restart UAT with 20 desktop regression flows.
- Next step: Perform the equivalent corrected Linux native UI journey, record M8 exit evidence, and begin M9 supervision/experiment architecture.
- Blockers: None documented.

## Current State

- MCG now has a compilable implementation foundation alongside its product specifications.
- The complete vision remains in scope and will be delivered through integrated, end-to-end milestones rather than a throwaway narrow prototype.
- Canonical requirements are indexed by `MCG_PRD.md` and split across `docs/spec/`; the encoding-damaged source is retained at `docs/archive/MCG_PRD.original.md`.
- The Git repository uses `main`, with `origin` at `https://github.com/fourtytwo42/Metacognitive-Governor.git`; the GitHub repository is private (last verified 2026-08-13).
- The workspace contains shared Rust crates, runnable `mcg-daemon` and `mcg` CLI applications, and a Tauri 2/React desktop shell with simple/advanced modes.
- A Windows portable qualification layout keeps `mcg-desktop.exe`, `mcg-daemon.exe`, and `mcg.exe` distinct and launches the daemon plus GUI without requiring a developer toolchain. Production installer/signing/upgrade/uninstall work remains a later release gate.
- The desktop task view loads authoritative event history and immutable governed-tool audit together. Simple mode summarizes outcomes, checks, tool runs, artifacts, and condition results; advanced mode adds ordered events, resources, verification records, and artifact hashes.
- The desktop provider workspace supports OpenAI subscription OAuth, OpenAI API keys, OpenRouter, and arbitrary OpenAI-compatible/local endpoints. It discovers before saving, stores credentials through the existing OS credential service, labels billing explicitly, and displays known price metadata or `Cost unknown`.
- The desktop Projects panel lists durable discovered workspaces with language/build/test/Git context and exposes paths, branches, and instruction-conflict counts only in advanced mode.
- GitHub Actions is disabled at repository level and its workflow/Dependabot files are removed; GitHub is source hosting only.
- M1 task state is durable in SQLite, rebuilt from its authoritative event log at startup, protected by a persistent local bearer token, and exposed through authenticated REST plus resumable SSE.
- M6 has an authenticated stateless MCP `2026-07-28` Streamable HTTP server with deterministic read/query tools and an idempotent governed single-turn mutation. Its client supports current/legacy HTTP and stdio configuration/discovery/calls through API, CLI, and desktop; expanded run-control/evaluation contracts remain in scope.
- Consumer Start requires a selected durable project and uses migration 10 to bind each existing task to exactly one running consumer plan. API, CLI, Tauri, and desktop share the launch contract; retries never reissue model work, and model self-report remains insufficient for completion.
- Guided local-tool approvals can be reviewed and resumed from the desktop task card. Resume revalidates durable authority, the exact approval, current provider/tool metadata, and unchanged project context; stable request identity and the provider-turn checkpoint prevent duplicate inference and tool side effects.
- Provider/tool turns checkpoint bounded credential-free private execution state after each provider response and before/after governed tool calls. Waiting local or external MCP authority survives restart only with a matching live checkpoint/approval/lease; resume continues the pinned conversation without replaying paid inference, and terminal authority removes the checkpoint atomically.
- Migrations 13-14 store one-time/interval/UTC-cron/event/file-watch/adaptive-heartbeat definitions, occurrence history, subsystem wakes, and notifications. The daemon atomically admits one active occurrence per schedule, persists cursors/fingerprints, prevents self-trigger loops, immediately reclaims interrupted identity after restart, and dispatches through consumer launch. Preflight budget exhaustion and approval/manual-evaluation waits pause recurring admission; REST, CLI, Tauri, and the desktop dashboard expose creation, history, wake, notification, and state controls without deleting history.
- Schedule creation accepts a caller-owned UUID and stores a server-generated SHA-256 definition hash. Exact retries replay one definition (`201` first REST call, `200` replay); changed input conflicts, duplicate storage insertion is atomic, malformed non-empty hashes fail closed, the desktop preserves identity across lost responses, CLI accepts optional `--schedule-id`, and MCP exposes authenticated create/list/remove tools that retain history.
- M7 integrated exit evidence drives a real guided project-file watch through baseline/change, consumer provider work, exact objective approval, restart recovery, resume, three governed build/test/check executions, terminal occurrence reconciliation/notification, and an additional scheduler tick with one task, two model calls, and no duplicate tool completion.
- M8 uses `PLATFORM_SCHEMA_VERSION = 1` and migration 15 for immutable deduplicated platform-capability snapshots plus durable browser/computer session projections and immutable lifecycle events. Session creation requires a caller UUID and exact SHA-256 definition hash; exact replay is idempotent, conflicting reuse fails, authority custody is explicit, unsafe credential-bearing origins fail closed, expired live reservations close exactly once, and startup converts remaining live authority to `interrupted` with retained history. The persistent browser layer now proves launch before activation, retains one isolated ephemeral Chromium process/page, intercepts traffic under immutable origin and method grants, denies downloads, redacts query/fragment data, supports visible-only local handoff and privacy-blanked reclaim, and coordinates close/expiry cleanup across REST, CLI, MCP, Tauri, and desktop.
- Migration 16 stores non-deletable computer action batches. New actions are claimed as `executing` before host input and settle as `succeeded`, `rejected`, or `uncertain`; exact successful replay is observation-only, conflicting reuse fails, and startup converts crash-interrupted claims to non-replayable `uncertain`. Durable hashes exclude typed text and pixels. Browser reservation hashes remain compatible across the additive computer fields.
- Computer input uses an exact-session capability plus the same durable policy/approval/audit/lease broker as other host mutations. Guided approval is committed before action-ledger claim or host input. Caller-owned session/batch UUIDs are mapped to namespaced stable synthetic authority/request identities, preventing cross-domain collisions while preserving exact approval retry.
- Sites execute bounded same-origin health GETs and structured deployment processes through the shared broker. Publication requires approval even in Autonomous mode before a deployment record or process exists; output is retained as artifacts, exact retries are observation-only, and startup converts claimed work to non-replayable uncertainty. Revision-checked changes retain immutable history and archive is terminal. One durable scheduler-backed interval monitor survives restart, uses occurrence IDs as health-check replay IDs, atomically settles monitor state/transition notifications, coalesces repeated outages, and synchronizes pause/resume/archive with schedule state. REST, CLI, MCP, Tauri, and desktop share the same registry and identities.
- Consumer plans whose conditions are entirely discovered build/test/static-analysis commands now execute those checks through the governed broker after independent Evaluator completion. Validated reports and completed/failed task/run projections commit atomically; terminal Start replay is observation-only.
- Guided command evaluation stores its run/project/condition progress in SQLite and requests one approval for the immutable discovered batch. Denial blocks for bounded retry with a new request identity; approved work resumes across daemon restart without another provider turn or repeated completed check.
- Advanced consumer condition lines accept exact `HTTP STATUS URL` syntax. Runtime grants only the parsed credential-free origin, performs a body-bounded governed GET, captures response bytes as an artifact, and compares observed status exactly; status mismatch, timeout, and connection failure become failed evaluation evidence.
- Advanced consumer condition lines accept exact `PAGE RENDERS URL` syntax. Runtime launches a fresh local Chromium CDP session, permits only exact-origin GET/HEAD plus in-page data/blob resources, checks DOM readiness/layout after JavaScript, and captures rendered HTML plus a PNG. Missing browser runtime blocks as inconclusive; render failure is objective failure.
- Advanced consumer conditions accept `ELEMENT EXISTS URL selector` and `BROWSER WORKFLOW project/relative/file.json`. Workflows are immutable, versioned JSON with 1-64 typed assertion/click/type/key steps; arbitrary JavaScript is excluded. The executor revalidates file bytes, confines all page traffic and navigation to same-origin GET/HEAD, rejects downloads/new contexts/cross-origin targets/mutating forms, and retains JSON, final HTML, final PNG, and ordered step evidence.
- Advanced consumer condition lines accept `FILE EXISTS project/relative/path` and exact `COMMAND SUCCEEDS discovered command`. File evidence is regular-file-only, capability-confined, streamed with a 1 GiB ceiling, and records artifact size/SHA-256 without stdout. Custom commands execute only when the complete command still matches safe current project discovery; arbitrary prose remains inconclusive.
- Advanced consumer condition lines accept `SCREENSHOT MATCHES URL project/relative/baseline.png`. Launch hashes the regular baseline before model work; evaluation revalidates that digest, renders under exact-origin browser confinement, decodes bounded PNGs, and applies explicit channel/changed-area tolerances. Missing baselines fail before spend; dimension, visual, or post-binding baseline changes fail the objective.
- Advanced consumer condition lines accept `RESEARCH EVIDENCE project/relative/report.json`. Runtime captures and hashes bounded JSON under governed file authority, binds its scope to the exact task goal, derives and authorizes only declared credential-free source origins, then fetches at most 32 sources under one timeout with 1 MiB per-response limits. It structurally validates claims, grounded short quotes, optional freshness, and conflict references; the independent Evaluator retains semantic responsibility. JSON and fetched bodies are immutable artifacts, and terminal replay performs no model or network work.

## User Preferences

- Keep `MEMORY.md` updated consistently with durable context that should survive chat compression.
- Agents should read `AGENTS.md` at startup and check `MEMORY.md` whenever context is missing, stale, or uncertain.
- Use uppercase filenames `AGENTS.md` and `MEMORY.md`.
- Build MCG for general consumers with simple installation and minimal required human regression testing.
- Keep every primary interface in the first complete release: native GUI, CLI, daemon/headless mode, OpenAI-compatible provider API, and MCP client/server.
- Deeply integrate automated unit, integration, scenario, contract, and end-to-end testing.
- Run builds and automated tests locally only. Do not use GitHub Actions or any GitHub-hosted automation that can incur usage charges.
- Use the owner-controlled Ubuntu VM at `192.168.50.109` for Linux desktop, daemon, CLI, and test qualification; never persist its credentials in repository files or memory.
- Use the owner-controlled `Win11-VM` through Chrome Remote Desktop for clean Windows consumer installation and UAT; treat its browser/authentication state as stale until freshly verified.
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
- Stdio MCP operations spawn a direct child with a minimal environment, bounded line/request sizes, deterministic termination, a five-second current discovery probe, and same-process `2025-11-25` initialize fallback. Stdio accepts no bearer credential; CLI arguments are a direct vector and advanced desktop arguments are one per line.
- Migration 8 atomically reserves interface-owned runs under an immutable request key/SHA-256 input hash. `mcg_run_agent` requires explicit Coder/Tool Worker tool lists, executes through the existing provider/policy broker, never treats a model response as objective completion, and returns existing success/non-success evidence rather than duplicating work on retry.
- Migration 11 stores one consumer evaluation checkpoint per task/run. Its state, next condition, attempt, exact approval request, batch delegation flag, evidence, failures, and unsupported conditions are transactionally coordinated with task/run/approval events and removed only by terminal evaluation settlement.
- M5 SQLite migration 5 makes organization revisions, plans, usage, and evaluations immutable evidence while runs, instances, and leases are event-backed current projections. Startup never resumes stale authority: interrupted runs become `recovering` and active instances/leases expire before reassignment.
- Provider credentials are runtime-only redacting values and are not serializable; persisted provider connections contain an explicit auth mode but no secret. OpenAI-compatible requests omit absent optional fields, and buffered calls tolerate providers such as GPU45 that always return a terminal Responses SSE stream.
- OpenAI browser OAuth uses loopback PKCE; device OAuth uses the verification-code flow; both are standalone from Codex, support refresh-token rotation, and route subscription inference only through `https://chatgpt.com/backend-api/codex`. API-key billing is a separate explicit connection and never an OAuth fallback.
- M3 policy evaluates canonical resources, not prompt claims. Invalid or weakened owner constitutions fail closed on every broker request; audit and secret-isolation requirements cannot be disabled. Denied or approval-pending requests are audited before return and never reach a host executor.
- Linux browser packages use root-owned `/opt/metacognitive-governor/browser/chrome` plus `packaging/linux/apparmor.d/metacognitive-governor-browser` to selectively permit the Chromium user-namespace sandbox on restricted Ubuntu. Never use `--no-sandbox` or disable Ubuntu's global AppArmor user-namespace restriction.

## Verification Ledger

- 2026-08-14: Dedicated clean `Win11-VM` consumer UAT passed through Chrome Remote Desktop and the corrections were pushed as commit `1b475b2`. The production Tauri build rendered without a developer toolchain, discovered two live GPU45 models, registered projects through the native picker, created a project-bound task, failed closed on unsatisfied tool-routing requirements, and retained providers/projects/task/evidence across daemon termination and restart. The qualification found and fixed the obsolete native discovery route, hidden project errors, and blocked evidence labeled `In progress`; 20 Vitest flows, frontend typecheck, Rust formatting, two desktop Rust tests, strict desktop Clippy, and the production no-bundle build pass. Final `mcg-desktop.exe` SHA-256 is `E1CE0ECDD56D835A1CE0D07E5D190889696256481A77A3E489787A83E51C6CC2`; refreshed qualification archive `MCG-0.1.0-windows-x64-portable-tauri.zip` SHA-256 is `6830CAB0221488950B0710F4E240A8FD4C71AFB83AD0CBB47181EA9D41173C55`. No GitHub-hosted compute ran.

- 2026-08-14: Sites persistence foundation added migration 17 and `SITE_SCHEMA_VERSION = 1` contracts for revisioned project-bound properties, structured deployment commands, deployment dispositions, and health observations. The runtime now routes health through bounded exact-origin HTTP and deployments through exact Site/Process/Directory capabilities, approval, lease, audit, and artifact capture. REST, CLI, MCP, Tauri, and desktop expose registration, history, health, and deploy with caller-owned identities. Cross-interface tests prove one network request/process side effect under replay; 44 API tests, 7 CLI tests, 17 Vitest tests, two Playwright flows, strict workspace Clippy, and all unlocked workspace Rust tests pass. Full `cargo test --workspace` on Windows is blocked only because the intentionally preserved live daemon executable is locked; `--exclude mcg-daemon --exclude mcg-desktop` is green and desktop library tests pass separately. Exact commit `8650d205815a7a5973bc561bd77e2c2308114691` passed complete Ubuntu `./scripts/verify.sh` in 353.8 seconds, including the full Rust workspace, docs, frontend, debug Tauri build, and Playwright. Revision mutation, scheduler monitoring/notifications, and native Sites UAT remain.

- 2026-08-14: Bounded computer actions were integrated with the durable tool broker. Exact computer-session capability grants fail closed; Guided mode durably requests approval before action-ledger claim or host input; Balanced/Autonomous may proceed only within the exact grant; decision/completion audits and execution leases share the existing authority path. Caller-owned session/batch UUIDs map to namespaced stable synthetic task/request identities to prevent cross-domain collisions while preserving exact approval retry. Strict workspace Clippy and the complete Rust workspace passed on Windows: 43 API tests (including real Guided REST/MCP approval, retry, stale-frame rejection, and immutable audit), 23 storage, 47 tools (46 pass/1 explicit UAT ignore), restart recovery, live GPU45, real browser, and real safe computer capture. Exact commit `97f38d4` then passed the complete owner-controlled Ubuntu `./scripts/verify.sh`: the same Rust gates, documentation, 16 Vitest flows, production frontend, native Tauri/GTK, and two Playwright tests. `.qualified-commit=97f38d473dbe70d22b31cc4f825fb33ad77589d1` and normalized `verify-final.exit=0` remain with the source/log; after exact path/process/marker verification only the 16.6 GiB regenerable target cache was cleaned, restoring 25 GiB free. No GitHub-hosted compute ran.

- 2026-08-14: Corrected implementation commit `d690b4e` passed exact owner-controlled Ubuntu `./scripts/verify.sh` with `.qualified-commit=d690b4e9ee3b19b5e95f936cec4988ff040f4d18` and `verify-final.exit=0`; cache reuse reduced the complete rerun to 75 seconds while still executing strict Rust, docs, 16 Vitest, production frontend, native GTK/Tauri, and two Playwright gates. Linux CLI/native UAT then reserved and attached a 100x100 session on Xvfb display 60, visually verified the PNG, durably settled one action batch as `succeeded`, and proved exact replay returned `replayed: true` with `result: null`. Handoff blocked capture, reclaim required a fresh frame, and history retained requested/active/handed-off/active custody. The real Tauri UI was visually inspected, toggled to advanced mode through X input, navigated to the computer panel, displayed the live Linux session, and refreshed the 100x100 frame. The session and exact UAT processes were closed/stopped; only the 24.8 GiB regenerable target cache was cleaned after marker/process verification, returning the VM to 25 GiB free. Source, marker, zero exit, logs, and UAT images remain; no GitHub compute ran.
- 2026-08-14: Exact computer-control commit `ff1886a` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 356 seconds with `.qualified-commit=ff1886a40cca1daa7e0dd50a8044f159a04b0ebe` and `verify-final.exit=0`: strict Rust gates, 43 API, 23 storage, 46 tool tests, live GPU45, real browser/capture, three daemon-restart cases, documentation, 16 Vitest flows, production build, native Linux GTK/Tauri, and two Playwright flows. Native Xvfb inspection then showed the real 1180x760 Tauri consumer shell rendered correctly. CLI UAT reserved an immutable 100x100 computer target, but attach returned 503 because Xvfb exposes a valid screen without marking one primary; capability discovery only required a non-empty screen list. The fallback is being corrected to select the first enumerated screen only when no primary exists, while explicit display IDs remain exact.
- 2026-08-14: The complete owner-local Windows gate passed in 236 seconds after native regional-input UAT: clean `npm ci` with zero vulnerabilities, Playwright Chromium provisioning, strict workspace formatting/clippy, every Rust unit/integration/doc test (including 43 API, 23 storage, 46 tool with one explicit visible UAT ignore, live GPU45, real Chromium and screen capture, three daemon-restart cases), documentation, frontend typecheck, 16 Vitest flows, production build, native Tauri debug build, and two Playwright flows. The first gate invocation stopped before testing because the temporary Vite server held Rolldown's native module during `npm ci`; stopping only that disposable server resolved the file lock, and it was restored afterward. No GitHub-hosted compute ran; daemon PID 85024 and restored desktop PID 118820 remained alive.
- 2026-08-14: Native Windows computer-use UAT passed the rebuilt isolated desktop/daemon on `127.0.0.1:7788`. Full-display capture, refresh, owner handoff, privacy-preserving reclaim, and stale-frame rejection were first verified. The resulting ambient-pixel UX defect was fixed with validated optional advanced region fields. A second native session reserved an immutable `{x:0,y:0,width:100,height:100}` target, attached a real executor, displayed a 100x100 PNG/digest, and completed the digest-bound click without an application error while remaining active; authenticated session inspection confirmed the exact target and grants. Focus moved away after the test click, so Computer Use stopped per its safety rules; the session was closed and only the isolated daemon/desktop were stopped. The original daemon PID 85024 stayed alive. Both desktop shells had exited unexpectedly during an earlier Computer Use foreground-handle failure; the original shell was restored as PID 118820 against the unchanged daemon/data without touching OAuth controls or relinking.
- 2026-08-14: The region UX and loopback-build override focused gates pass: 16 Vitest flows, frontend typecheck, Rust formatting, two desktop Rust tests, and strict desktop all-target/all-feature clippy. The loopback validator accepts explicit IPv4/IPv6 loopback HTTP endpoints and rejects HTTPS, DNS/remote hosts, paths, credentials, queries, and fragments before bearer-token loading/network dispatch.
- 2026-08-14: The M8 computer-use Windows automated lane passed while the visible OAuth daemon PID 85024 and desktop PID 74436 remained untouched: strict workspace clippy; complete workspace tests including 43 API, 23 storage, 46 tool (45 pass/1 explicit UAT ignore), 8 runtime, real browser, real safe screen capture, daemon restart, and live GPU45; 15 Vitest flows; production frontend build; two Playwright flows; docs and diff hygiene. File-backed SQLite reopen proves executing input becomes non-replayable `uncertain`; REST/MCP share digest-bound authority. The native Computer Use UAT attempt was stopped by the physical Escape key and must be retried in a new turn.
- 2026-08-14: Rebuilding the computer capture/input dependencies exposed a pre-existing persistent-browser race: a stale initial `about:blank` CDP disposition could terminate the request interceptor, leaving later navigation paused until timeout. The interceptor now keeps processing after a per-request fail-closed error, and navigation waits on bounded observable DOM readiness. The direct persistent-browser fixture and combined real REST/MCP browser/computer tests pass after the fix.
- 2026-08-14: Visible-browser UAT passed on both operating systems without touching the existing OAuth daemon/desktop. Windows used the requested Computer Use skill to inspect and click the real governed Chromium window; Linux used an unprivileged user-staged Xvfb, captured and visually inspected the real 1280x800 framebuffer, and sent the local-owner click through X input. The reusable ignored fixture proved visible-only handoff, local-owner control, privacy reclaim to `about:blank`, Chromium closure, and isolated-profile deletion on both. The temporary Linux display was stopped; no privileged installation or stored credential was used. After re-verifying the exact `d5de79e` marker/zero exit/log, only its regenerable target cache was removed, returning the VM to about 25 GiB free.
- 2026-08-14: Exact persistent-browser commit `d5de79e` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 311 seconds with `.qualified-commit=d5de79ed556291d63d732ab1af3b01ca2f4baf36` and `verify-final.exit=0`: strict Rust gates including 42 API, 22 storage, 41 tool, 7 runtime, real persistent Chromium, daemon restart, and live GPU45; docs; typecheck; 14 Vitest flows; production build; native Tauri/GTK; and two Playwright flows. The first `935050e` run found and drove the explicit configured-browser fixture fix; a cache-reuse attempt exposed only Tauri absolute-path metadata, so the passing run used a clean target. The previous exact M8 target was marker/path/exit verified and only its 14 GiB regenerable cache removed for headroom; source and evidence remain. No GitHub-hosted compute ran. Visible handoff UAT remains pending.
- 2026-08-14: Persistent browser execution passed the complete available Windows local lane while visible OAuth processes remained untouched: strict all-feature/all-target clippy; 42 API, 22 storage, 41 tool, 7 runtime, and all other unlocked Rust suites; isolated daemon restart and desktop Rust tests; docs, typecheck, 14 Vitest flows, production build, and two Playwright flows. Real Chromium fixtures prove persistent navigation, cross-origin/mutating-request/download denial, redacted URLs, runtime activation/expiry/history, and one authenticated session controlled across REST and MCP.
- 2026-08-14: Exact M8 platform-authority commit `7bf25ed` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 306 seconds with `.qualified-commit=7bf25ed59d6b8103f5319362538451e78f8db558` and `verify-final.exit=0`: strict Rust gates including 41 API, 22 storage, 39 governed-tool, real Chromium, live GPU45, and the authenticated platform-session daemon restart fixture; docs; typecheck; 13 Vitest flows; production build; native Tauri/GTK; and two Playwright flows. The exact qualified M7 target cache was marker/path verified and only its regenerable target directory removed for headroom; source snapshot remains. No GitHub-hosted compute ran.
- 2026-08-14: M8 platform contract foundation passed focused Windows tests: the domain transition matrix and three storage fixtures covering migration 15, capability snapshot deduplication/immutability, session exact replay/conflict, unsafe-origin rejection, bounded expiry, atomic lifecycle history, file-backed reopen, and restart interruption recovery. Full runtime/interface integration and cross-platform qualification remain pending.
- 2026-08-14: M8 capability/session interfaces passed the complete available Windows local lane while visible OAuth processes remained untouched: strict all-feature/all-target clippy; all unlocked Rust suites including 41 API, 22 storage, 39 governed-tool, real Chromium, and live GPU45 tests; daemon/desktop tests in isolated target space; docs, typecheck, 13 Vitest flows, and production build. A real authenticated loopback daemon fixture additionally proves a requested session becomes `interrupted` with exactly two events after server abort/database reopen. REST, CLI, MCP, Tauri, and desktop share idempotent reservation/list/history/close semantics; capability reporting is privacy-safe, expired authority settles once, and no interface advertises an unattached executor as active. Exact Ubuntu qualification remains pending.
- 2026-08-14: Exact M7 exit implementation commit `6dd62b3` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 308 seconds with `.qualified-commit=6dd62b38fdac16dd095412cd35ff0735fec504e0` and `verify-final.exit=0`: all Rust suites including 39 API tests with the integrated watched-work approval/restart/exactly-once scenario, 19 storage, 39 governed-tool, daemon restart, real Chromium, and live GPU45; native Tauri/GTK; docs; typecheck; 12 Vitest tests; production build; and two Playwright flows. The exact predecessor marker was verified and only its 14.8 GiB regenerable target cache was cleaned before the run, leaving adequate packaging headroom. No GitHub-hosted compute ran.
- 2026-08-14: The M7 integrated watched-work exit scenario passed the complete available Windows Rust lane while visible OAuth processes remained untouched: 39 API tests plus all unlocked workspace suites, strict all-feature/all-target clippy, real Chromium, and live GPU45. The new fixture proves approval/restart recovery and one-time side effects at the actual scheduler-to-consumer boundary.
- 2026-08-14: Exact idempotent scheduler/MCP implementation commit `86e54b1` passed complete owner-controlled Ubuntu `./scripts/verify.sh` with `.qualified-commit=86e54b1cd02542f951a0b80773229f0af90113ca` and `verify-final.exit=0`: all Rust suites including 38 API, 19 storage, daemon restart, real Chromium, and live GPU45; native Tauri/GTK; docs; typecheck; 12 Vitest tests; production build; and two Playwright flows. The first native packaging attempt ran out of disk only after every Rust/frontend test passed; the predecessor `a1ea1d4` qualification marker and zero exit were verified, only its regenerable 14.8 GiB Rust target cache was cleaned, and the complete rerun passed in 40 seconds. No GitHub-hosted compute ran.
- 2026-08-14: Schedule idempotency/MCP checkpoint passed the complete available Windows lane while the visible OAuth daemon/desktop remained untouched: strict all-feature/all-target clippy; broad unlocked Rust suites including 38 API, 19 storage, 39 governed-tool, real Chromium, and live GPU45 tests; two real daemon restart tests in an isolated target; docs, typecheck, 12 Vitest tests, production build, and two Playwright flows. REST requires caller identity and distinguishes `201` creation from `200` replay; MCP invalid budget shapes/bounds fail closed. The isolated 3.9 GiB target cache was cleaned.
- 2026-08-14: Exact M7 trigger/dashboard commit `a1ea1d4` passed complete owner-controlled Ubuntu `./scripts/verify.sh` with `.qualified-commit=a1ea1d481de2f3db1515a751f3e79df77539bb49` and `verify-final.exit=0`: all Rust suites including 36 API, 19 storage, daemon restart, and live GPU45; native Tauri/GTK; docs; frontend typecheck, 11 Vitest tests, production build, and two Playwright tests. The first native packaging attempt exhausted disk after all Rust tests; the verified `9d0af9d` snapshot marker was checked and only its regenerable 14.1 GiB Rust target cache was cleaned, after which the full gate passed. No GitHub-hosted compute ran.
- 2026-08-14: Exact scheduler commit `9d0af9d` passed complete owner-controlled Ubuntu `./scripts/verify.sh` with `verify-final.exit=0`: all Rust suites including 34 API and scheduler restart tests, live GPU45, real Chromium, native Tauri/GTK, frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene. Retained snapshot: `/home/hendo420/mcg-worktree-9d0af9d`; the verified predecessor `ae28c50` and both transfer archives were removed after exact marker checks. No GitHub-hosted compute ran.
- 2026-08-14: Scheduler Windows lane is green while the visible OAuth daemon/desktop remained untouched: strict full-workspace all-feature/all-target clippy; broad Rust tests excluding only the two locked executable packages; 34 API and 39 tool tests; live GPU45; real Chromium; two file-backed daemon restart tests in isolated `target/scheduler-validation`; docs, typecheck, 10 Vitest tests, production build, and two Playwright tests. The reproducible 3.9 GiB isolated target cache was removed after the test.
- 2026-08-14: Exact provider-turn checkpoint commit `ae28c50` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 300 seconds with `verify-final.exit=0`: strict full-workspace Rust gates including 32 API and 39 tool tests, file-backed local/external MCP approval restart scenarios, live GPU45, real Chromium, native Tauri/GTK, frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene. The retained snapshot is `/home/hendo420/mcg-worktree-ae28c50`; the superseded verified `1d65197` snapshot was removed after exact path/marker validation. Transfer archives were removed and no GitHub-hosted automation ran.
- 2026-08-14: Provider-turn checkpointing passed the complete available Windows local lane while the visible OAuth daemon/desktop remained untouched: strict full-workspace formatting/clippy, broad Rust tests excluding only the two locked executable packages, 32 API tests, 39 tool tests, live GPU45, real Chromium, frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene. File-backed local and destructive external MCP fixtures prove waiting authority survives reopen, the paid pre-approval provider response is not replayed, the approved side effect runs exactly once, and the checkpoint is removed atomically.
- 2026-08-14: Exact desktop evidence/provider/project commit `1d65197` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 298 seconds with `verify-final.exit=0`: strict Rust workspace gates including 39 tool and 31 API tests, restart recovery, live GPU45, real Chromium, native Tauri/GTK, frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene. The retained snapshot is `/home/hendo420/mcg-worktree-1d65197`; the superseded verified `f9f9fc6` snapshot was removed after exact path/marker validation. Transfer archives were removed and no GitHub-hosted automation ran.
- 2026-08-14: Desktop evidence/provider/project UX passed the Windows local gate: strict workspace clippy and formatting, broad Rust tests excluding the visible locked daemon/desktop binaries (39 tool and 31 API tests plus live GPU45 and real Chromium), frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs validation, and diff hygiene. Browser visual QA found and fixed a 720px sidebar overflow; the mode switch is now reachable and Playwright protects the narrow layout.
- 2026-08-14: Exact provenance-backed research commit `7566cf4` passed complete owner-controlled Ubuntu `./scripts/verify.sh` in 291 seconds. The verified marker and `verify-final.exit=0` remain in `/home/hendo420/mcg-worktree-7566cf4`; all Rust gates (38 tool and 31 API tests plus live GPU45), docs, frontend, native Tauri/GTK, real Chromium, and Playwright passed. The superseded exact `4e5c432` snapshot was path/marker/exit verified and removed first to recover 13 GB; its source remains in Git. No GitHub-hosted automation ran.
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
- 2026-08-14: Exact commit `e706ba4` passed the complete owner-controlled Ubuntu `./scripts/verify.sh` gate, including native Tauri/GTK and Playwright qualification.
- 2026-08-14: Exact approval-continuation commit `43c63f3` passed the complete owner-controlled Ubuntu `./scripts/verify.sh` gate, including strict workspace tests/clippy, live GPU45, frontend/docs, native Tauri/GTK, and Playwright.
- 2026-08-14: Objective-settlement focused Windows gates passed 24 API tests, strict Rust clippy, nine desktop flows, frontend type/build, and docs. The public scenario uses real npm/Node build/test/check processes for atomic pass/fail settlement and terminal replay non-duplication.
- 2026-08-14: Exact objective-settlement commit `52da160` passed complete owner-controlled Ubuntu `./scripts/verify.sh`, including the real npm/Node pass/fail scenario, strict workspace gates, live GPU45, frontend/docs, native Tauri/GTK, and Playwright.
- 2026-08-14: Live daemon/CLI smoke test passed for health, human status, and JSON status; the exact temporary daemon process was stopped afterward.
- 2026-08-14: The first GitHub Actions frontend run exposed non-deterministic workspace dependency placement: Vitest was hoisted while the explicit jsdom dependency was nested. Moved jsdom 29.1.1 to the root workspace, verified a clean `npm ci`, Vitest, and Playwright run, and updated checkout/setup-node to their current v7 major releases.
- 2026-08-14: GitHub Actions run `31771954125` passed all documentation, frontend, Ubuntu Rust, and Windows Rust jobs for commit `b02d626`.
- 2026-08-14: Disabled GitHub Actions at the repository setting and removed `.github/workflows/ci.yml` plus `.github/dependabot.yml`; all future builds/tests run only on owner-controlled machines.
- 2026-08-14: Installed Rust 1.93, Node 24.14, npm, Tauri/WebKit native libraries, and Playwright Chromium on owner-controlled Ubuntu 24.04 VM `192.168.50.109`; no VM password was stored in repository files or memory.
- 2026-08-14: Windows and Ubuntu local verification scripts passed Rust fmt/clippy/tests, documentation validation, frontend typecheck/unit/build, native Tauri debug build, and Playwright E2E.
- 2026-08-14: Exact governed MCP mutation commit `c14fef2` passed `./scripts/verify.sh` from fresh archive `/home/hendo420/mcg-worktree-c14fef2` on the owner-controlled Ubuntu VM; a `set -e` confirmation emitted `__VERIFY_PASSED__` after all Rust, docs, frontend, native Tauri, and Playwright gates.
- 2026-08-14: Exact bounded-stdio commit `fdeb60f` passed `./scripts/verify.sh` from fresh archive `/home/hendo420/mcg-worktree-fdeb60f` on the owner-controlled Ubuntu VM. All Rust workspace tests (including modern and same-process legacy stdio fixtures), live GPU45, docs, frontend checks/build, native Tauri debug build, and Playwright passed with `__VERIFY_PASSED__`. The first attempt exposed a full VM filesystem; verified superseded generated worktrees `mcg-worktree-63d3dd4` and `mcg-worktree-c14fef2` were removed, recovering about 19 GB. Their source revisions remain in Git.
- 2026-08-14: Exact native-task commit `5b11909` passed `./scripts/verify.sh` from fresh archive `/home/hendo420/mcg-worktree-5b11909` on the owner-controlled Ubuntu VM in 222 seconds. Strict Rust workspace gates, durable task listing/API tests, restart recovery, live GPU45, five desktop flows, frontend/docs, native Linux Tauri build, and Playwright all passed with `__VERIFY_PASSED__`.
- 2026-08-14: Exact native-project-picker commit `352513c` passed `./scripts/verify.sh` from fresh archive `/home/hendo420/mcg-worktree-352513c` on the owner-controlled Ubuntu VM in 225 seconds. The official Tauri dialog plugin and Linux backend compiled; all workspace tests, restart recovery, live GPU45, six desktop flows, frontend/docs, native Tauri, and Playwright passed with `__VERIFY_PASSED__`. Superseded generated worktree `mcg-worktree-fdeb60f` was path-verified and removed for disk headroom; its source remains in Git.
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
- 2026-08-14: Exact file/custom evidence commit `4238ec5` passed an isolated complete `./scripts/verify.sh` in `/home/hendo420/mcg-worktree-4238ec5` with `verify-final.exit=0`: strict Rust workspace gates including 32 tool and 29 API tests, live GPU45, docs, frontend checks/build, sandboxed real Chromium, native Linux Tauri/GTK build, and Playwright. No GitHub-hosted automation ran.
- 2026-08-14: Exact screenshot-baseline commit `4e5c432` passed complete Ubuntu `./scripts/verify.sh` with `verify-final.exit=0`, including 33 tool/30 API tests, decoded-pixel Chromium comparison, live GPU45, docs/frontend, native Tauri/GTK, and Playwright. The first exact code commit proved a small 404 change was correctly within the 1% tolerance; the corrected full-viewport fixture passed. A later attempt exhausted disk while both generated snapshots remained; the failed snapshot was path-verified and removed, the exact rerun passed, and the superseded qualified `4238ec5` snapshot was removed after verifying both markers. Source remains in Git; 14 GB VM headroom remained.
- 2026-08-14: Typed element/workflow evidence passed the Windows local lane and exact commit `f9f9fc6` passed complete Ubuntu `./scripts/verify.sh` in 299 seconds with `verify-final.exit=0`: strict workspace clippy/tests including 39 tool and 31 API tests, restart recovery, live GPU45, real Chromium, native Tauri/GTK, frontend typecheck, 9 Vitest tests, production build, Playwright, docs, and diff hygiene. Fixtures prove hydration, five-step success, assertion failure, immutable-file mutation rejection, and fail-closed cross-origin/download/new-context/POST-form interactions. Its generated snapshot was later removed after exact marker validation when superseded by `1d65197`; the source remains in Git.

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

### 2026-08-14 - Public Governed MCP Agent Scenario

- Context: Unit tests for transport, binding, policy, and execution do not alone prove that the public MCP server mutation composes every boundary into one useful model/tool loop.
- Decision: Add deterministic live MCP and OpenAI-compatible fixtures, then call authenticated `mcg_run_agent` with canonical `mcp::governed::lookup` through `POST /mcp`.
- Files: `crates/mcg-api/src/lib.rs`, `docs/architecture/mcp.md`, `MEMORY.md`.
- Commands/tests: strict `mcg-api` clippy; exact public scenario test.
- Result: One durable run exposes only a generated alias, performs exactly one read-only closed-world remote call through the broker, feeds evidence back to the model, persists a succeeded tool outcome, and creates no approval record.
- Follow-up: Keep this scenario in every full gate; add approval-resume coverage for an open-world MCP tool after expanded run-resume contracts are implemented.

### 2026-08-14 - HTTP MCP Legacy Negotiation

- Context: Most deployed MCP servers still use initialize-capable protocol revisions, while consumers need safe automatic compatibility without weakening strict pin behavior.
- Decision: Probe current `2026-07-28` first in automatic HTTP mode and fall back only on structured protocol/header negotiation errors. Implement bounded `2025-11-25` initialize, optional session-header continuity, initialized notification, deterministic tools, JSON/SSE envelopes, and strict current/legacy modes. Persist the negotiated era and pin agent calls to it.
- Files: `crates/mcg-mcp/src/{legacy,negotiation}.rs`, `crates/mcg-runtime/src/lib.rs`, `docs/architecture/mcp.md`, `README.md`.
- Commands/tests: strict MCP/runtime clippy and tests; dual-era fixture with current rejection, legacy session assertion, discovery ordering, and tool call.
- Result: Automatic HTTP compatibility reaches a legacy session without relaunching or leaking credentials; strict modes remain explicit and current HTTP behavior stays green.
- Follow-up: Implement the stdio process lifecycle with a five-second current probe and same-process initialize fallback.

### 2026-08-14 - Bounded Stdio MCP Client

- Context: Local MCP servers need first-class cross-platform support without shell interpolation, unbounded process output, or an era change after agent authority is issued.
- Decision: Add one-child-per-operation stdio transport with a minimal environment, piped newline JSON-RPC, a 4 MiB line cap, request timeouts, kill-on-drop cleanup, five-second current discovery, and structured same-process legacy fallback. Route HTTP and stdio through one runtime client; expose stdio in CLI and advanced desktop only.
- Files: `crates/mcg-mcp/src/{stdio,unified}.rs`, `crates/mcg-runtime/src/lib.rs`, `apps/mcg-cli/src/main.rs`, `apps/desktop/src/{App,App.test,desktop-client}.tsx`, `docs/architecture/mcp.md`, `README.md`.
- Commands/tests: Modern and legacy spawned-process fixtures; focused Rust compile/tests; desktop Vitest and production build; CLI help smoke.
- Result: Modern stdio discovery/call and same-process automatic legacy fallback pass on Windows and in the complete Ubuntu gate; governed runtime configuration/execution now dispatches by durable transport, and all primary management interfaces accept stdio without persisting a credential.
- Follow-up: Run the complete Windows gate after the live OAuth handoff is released; add third-party SDK interoperability plus native UAT.

### 2026-08-14 - Native Durable Task Control

- Context: API and CLI could create and inspect tasks, but the consumer desktop hero was still a visual scaffold and SQLite had no public current-task listing.
- Decision: Add deterministic newest-updated task listing in storage/runtime/API; route create/list/transition/history through authenticated native Tauri commands; make simple mode goal-only and advanced mode expose constraints, success conditions, risk, cost ceiling, and ids. Expose only legal pending start, nonterminal cancel, and read-only history controls.
- Files: `crates/mcg-{storage,runtime,api}/`, `apps/mcg-cli/src/main.rs`, `apps/desktop/src-tauri/`, `apps/desktop/src/{desktop-client,App,App.test,styles}.tsx`, `docs/architecture/runtime-spine.md`, `README.md`.
- Commands/tests: strict focused Rust clippy/tests; frontend typecheck; five Vitest flows; production Vite build; docs and diff checks.
- Result: One durable task can be created, listed, started/cancelled, and observed with ordered event evidence through desktop, CLI, and API contracts; focused Windows and complete Ubuntu gates are green while the live Windows OAuth process remains preserved.
- Follow-up: Perform native Windows/Linux UAT, then connect project/run orchestration into the consumer flow.

### 2026-08-14 - Native Project Selection

- Context: The consumer task composer still had a disabled “Open folder” button and could not associate work with the existing durable project model.
- Decision: Use the official Tauri 2 dialog plugin only behind a dedicated async Rust command, return one local directory path, discover it through the authenticated daemon, and bind tasks by durable project id. Do not grant frontend code general filesystem scope. Load existing projects on startup for restart-safe reselection.
- Files: workspace/app Cargo manifests and lock, `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src/{desktop-client,App,App.test,styles}.tsx`, `docs/architecture/runtime-spine.md`, `README.md`.
- Commands/tests: strict native clippy/test; frontend typecheck and six Vitest flows; production build; diff check.
- Result: Folder cancellation is side-effect free; selection/discovery visibly identifies the project and sends its id in task creation. Focused Windows and complete Ubuntu automation are green; native dialog UAT remains required on both platforms.
- Follow-up: Real native folder-picker UAT, then consumer project/run orchestration.

### 2026-08-14 - Idempotent Consumer Task Launch

- Context: Desktop Start only changed a task projection to active and did not plan or execute governed work.
- Decision: Require a selected durable project; atomically bind each existing task to one consumer organization/run using migration 10; derive candidate build/test/static-analysis conditions from project discovery; advance Coder then independent Evaluator through existing provider/tool authority; and return durable blocked/approval/evaluation observations without treating model text as objective completion. Launch replay never executes duplicate model work.
- Files: `crates/mcg-{storage,runtime,api}/`, `apps/mcg-cli/`, `apps/desktop/`, `docs/architecture/runtime-spine.md`, `README.md`.
- Commands/tests: Strict focused Rust clippy; 43 targeted Rust tests; TypeScript; six Vitest flows; production Vite build; Playwright smoke; docs and diff checks.
- Result: Atomic/restart-durable one-run reservation, missing-provider reporting, inferred command evidence, CLI/Tauri/desktop launch, and UI progress are green on Windows. Existing provider/tool/evaluator fixtures remain green. Objective evaluation and terminal settlement are intentionally still required.
- Follow-up: Implement approval resume, bounded retry, cancellation propagation, objective condition execution, terminal settlement, and then the complete Ubuntu qualification.

### 2026-08-14 - Linux Consumer Launch Qualification And Bounded Recovery

- Context: Consumer launch needed exact cross-platform qualification plus owner-visible recovery/cancellation semantics.
- Decision: Keep Start replay observation-only; add explicit Retry only for blocked tasks; cap attempts using each durable agent lease retry limit; project waiting/failure into task state; and atomically cancel task, run, active instances, and active leases together.
- Files: `crates/mcg-{storage,runtime,api}/`, `apps/mcg-cli/`, `apps/desktop/`, runtime documentation.
- Commands/tests: Exact commit `6c666f5` complete `./scripts/verify.sh` on owner-controlled Ubuntu; strict focused Windows Rust clippy/tests; seven Vitest flows.
- Result: Exact consumer launch checkpoint passed Ubuntu native Tauri/GTK, Rust, frontend, live-provider, and Playwright gates. Superseded reproducible Linux snapshots `mcg-worktree-5b11909` and `mcg-worktree-352513c` were removed after exact realpath validation; source remains recoverable from Git. Retry/cancel changes are Windows-focused green and await their own exact Linux checkpoint.
- Follow-up: Approval-resume checkpoints, objective evaluator, terminal settlement, then exact Linux qualification of the expanded recovery slice.

### 2026-08-14 - Governed Local Approval Continuation

- Context: A consumer run could wait durably for a guided-mode tool approval, but no public task flow could safely continue it.
- Decision: Derive each tool-request id from the lease, tool name, and canonical arguments; after owner approval, require exactly one live waiting instance/lease and revalidate organization role, project context fingerprint, tools, and provider routes before restoring running authority. Replay the bounded local-tool turn under the same lease so the broker consumes the existing approval and records the side effect once.
- Files: `crates/mcg-{domain,agents,storage,runtime,api}/`, `apps/mcg-cli/`, `apps/desktop/`, runtime/agent documentation.
- Commands/tests: Strict focused Windows Rust clippy; 23 API tests including guided write approval/resume/replay; eight Vitest flows; frontend typecheck/build/docs/diff checks; exact commit `43c63f3` complete Ubuntu `./scripts/verify.sh`.
- Result: Local guided approval is available through API, CLI, Tauri, and a desktop task-card action. The deterministic public fixture proves the file is absent while pending, written once after approval, and not repeated by Start replay. Exact Windows-focused and complete Ubuntu native qualification are green.
- Follow-up: Denied/cancelled approvals must settle waiting task authority. Persist full mid-turn provider checkpoints before external MCP approval resume, target-scoped resume, or avoiding the repeated provider inference call; current unsupported cases fail closed.

### 2026-08-14 - Objective Command Evaluation And Terminal Settlement

- Context: Completed Coder/Evaluator model turns still left consumer tasks active because model self-report could not prove build, test, or static-analysis success; denying an approval also left retained waiting authority unresolved.
- Decision: Resolve denied/cancelled launched approvals together with failed instance/lease, retryable run, blocked task, and ordered events in one SQLite transaction. After every planned turn completes, rerun only exact currently discovered Build/Test/StaticAnalysis commands through typed process requests, capability policy, tool audit, and execution leases. Bind the report to the completed independent Evaluator and atomically persist it with completed/failed task/run projections; inconclusive evidence blocks without inventing success.
- Files: `crates/mcg-{domain,storage,runtime,api}/`, `apps/desktop/`, `docs/architecture/{runtime-spine,agent-organization}.md`, `README.md`.
- Commands/tests: Strict focused Windows Rust clippy; 24 API tests; real npm/Node objective pass/fail/replay scenario; nine Vitest flows; frontend typecheck/build; docs/diff checks; exact commit `52da160` complete Ubuntu `./scripts/verify.sh`.
- Result: Passing discovered commands complete the task/run, nonzero exit evidence fails both, and terminal Start replay adds no model call or tool audit record. Desktop History shows durable evaluation outcome/check counts and advanced details; Deny exposes the retryable blocked state. Exact Windows-focused and complete Ubuntu native qualification are green.
- Follow-up: Add a dedicated guided-mode objective-check approval checkpoint and deterministic HTTP, browser, artifact/file, research, screenshot, and custom-condition evaluators; non-command conditions deliberately remain nonterminal.

### 2026-08-14 - Durable Guided Objective Approval

- Context: Guided mode could not run harness-owned build/test/static-analysis checks because approval state and partial evaluation evidence did not survive a daemon restart.
- Decision: Persist one condition-indexed evaluation checkpoint per consumer run; request a single approval bound to the exact discovered command batch, attempt, and immutable project snapshot; rotate identity after denial/retry; and treat approval as delegation only for that batch. Resume restores a recovering run without replaying provider turns or completed checks, and terminal settlement deletes the checkpoint atomically with its report and projections.
- Files: `crates/mcg-{events,storage,runtime,api}/`, `docs/architecture/{runtime-spine,agent-organization}.md`, `docs/testing/strategy.md`, `README.md`.
- Commands/tests: Strict focused Windows Rust clippy; focused Rust suites including 25 API tests; nine Vitest flows; frontend typecheck/build; documentation and diff checks; exact commit `6e5f92e` complete Ubuntu `./scripts/verify.sh`.
- Result: A file-backed API fixture proves wait, denial, bounded retry with fresh identity, approval, daemon restart into recovery, exact resume, three real npm/Node checks, terminal completion, checkpoint cleanup, two total model calls, and no duplicate tool completion. Exact Windows-focused and complete Ubuntu native/Tauri/Playwright qualification are green.
- Follow-up: Implement HTTP-status and rendered-page objective evaluators.

### 2026-08-14 - Governed HTTP Status Evidence

- Context: Consumer plans treated HTTP health conditions as unsupported, and a plain fetch would have bypassed the harness policy/evidence plane or confused transport success with the expected status.
- Decision: Parse only exact advanced lines of `HTTP STATUS URL`; accept credential-free HTTP(S) URLs and valid status codes; grant only the condition origin; issue a server-owned GET with a 1 MiB response limit; and reuse `HttpExecutor` through `HostExecutor`, broker policy, execution lease, immutable audit, redaction, same-origin redirect enforcement, and artifact storage. Compare the executor's `http.status` evidence exactly, including expected non-2xx responses, and convert execution/timeout failures into failed condition evidence.
- Files: `crates/mcg-{runtime,api}/`, `README.md`, `docs/architecture/{runtime-spine,agent-organization}.md`, `docs/testing/strategy.md`.
- Commands/tests: Strict focused Rust clippy/tests including 27 API tests and all four consumer scenarios; HTTP parser safety cases; public HTTP pass/mismatch/unreachable fixture; nine Vitest flows; frontend typecheck/build; documentation and diff checks; exact commit `aa16d03` complete Ubuntu `./scripts/verify.sh`.
- Result: Expected 404 evidence passes with a content-addressed response artifact, 404 when 200 is expected fails, and connection refusal produces an audited failed execution plus atomic failed task/run evaluation rather than an API error. Exact Windows-focused and complete Ubuntu native/Tauri/Playwright qualification are green. Page rendering remains deliberately unsupported until backed by a real browser engine.
- Follow-up: Implement governed rendered-page evidence.

### 2026-08-14 - Real Browser Render Evidence

- Context: `PageRenders` required evidence from a JavaScript-capable browser across daemon, CLI, and desktop flows; treating fetched HTML as rendering would be false evidence, while unrestricted page scripts could silently broaden network authority or mutate services.
- Decision: Add typed `browser.render` backed by ephemeral Chromium over CDP. Parse only exact `PAGE RENDERS URL`; require credential-free HTTP(S); use a fresh profile and bounded 1280x720 viewport; intercept every request before send; allow only GET/HEAD to the condition origin plus data/blob resources; and block cross-origin and mutating requests. Verify post-JavaScript ready state/body layout, then content-address rendered HTML and a viewport PNG. Use configured `MCG_BROWSER_EXECUTABLE` or local system detection with no runtime download. Browser unavailability is inconclusive/blocking; launched-browser render failure fails the condition.
- Files: `crates/mcg-{config,tools,runtime,api}/`, workspace manifests/lock, owner verification scripts, `README.md`, runtime/testing documentation.
- Commands/tests: Strict full-workspace Windows clippy; all Rust tests except live-locked daemon/desktop executables, including 30 tool and 28 API tests; real Chromium tool fixture; public consumer render launch fixture; nine Vitest flows; frontend typecheck/build; docs/diff checks; exact commit `20c5f81` complete Ubuntu `./scripts/verify.sh`.
- Result: Windows-focused and complete Ubuntu gates prove JavaScript changes the DOM/title, two render artifacts are audited, screenshot hash reaches the independent report, a cross-origin image plus same-origin POST are blocked before network send, and missing browser is unavailable rather than false objective failure. Initial commit `4bca0c7` correctly failed on Ubuntu AppArmor; root-owned managed browser plus the path-specific profile fixed it without disabling sandbox or global restriction. Exact `20c5f81` passed all Rust, live-provider, native Tauri/GTK, frontend, and Playwright gates. No password was persisted.
- Follow-up: Artifact/file and custom condition evaluators, then expanded browser workflow/screenshot evidence.

### 2026-08-14 - Governed File And Custom Command Evidence

- Context: Artifact/file conditions and custom task text could not independently settle consumer work; executing arbitrary custom prose would have created an unsafe shell boundary.
- Decision: Add typed `FileExists` and server-owned `fs.verify`; require normalized project-relative paths and regular files; stream at most 1 GiB into content-addressed storage without exposing file bytes as stdout. Make `COMMAND SUCCEEDS` the only executable custom form and require its full command to remain an exact current discovery match. Discover all safely named package scripts as structured runner argv; leave other prose inconclusive.
- Files: `crates/mcg-{agents,tools,projects,runtime,api}/`, `README.md`, `docs/architecture/{runtime-spine,agent-organization}.md`, `docs/testing/strategy.md`.
- Commands/tests: Strict full-workspace Windows clippy; all Rust tests except live-locked daemon/desktop executables, including 32 tool and 29 API tests; real binary artifact and npm custom-command public fixture; nine Vitest flows; frontend typecheck/build; Playwright; docs/diff checks.
- Result: Windows qualification is green. Exact commit `4238ec5` also passed an isolated complete Ubuntu `./scripts/verify.sh` with `verify-final.exit=0`. Binary evidence is bounded, deduplicated, and hash/size checked; a missing file atomically fails task/run; exact discovered custom scripts pass through policy, leases, audit, and structured process execution.
- Follow-up: Continue research and screenshot-baseline evaluators.

### 2026-08-14 - Immutable Screenshot Baseline Evidence

- Context: Rendered-page evidence captured a screenshot but could not compare it with an approved visual baseline; byte-hash equality would be too brittle, while a mutable path could redefine success after approval.
- Decision: Add typed `BrowserScreenshotMatches` with URL, project-relative baseline, launch-bound baseline SHA-256, fixed viewport, per-channel threshold, and maximum changed-pixel basis points. Require both network-origin and file-read capabilities; decode only bounded PNGs; compare RGBA pixels; retain HTML, actual PNG, and baseline PNG as content-addressed evidence. Default consumer tolerance is channel delta 8 and 1% changed pixels.
- Files: workspace/tool manifests and lock, `crates/mcg-{agents,tools,runtime,api}/`, `README.md`, and runtime/agent/testing documentation.
- Commands/tests: Strict full-workspace Windows clippy; all Rust tests except live-locked daemon/desktop executables, including 33 tool and 30 API tests; real Chromium pass/mismatch/mutation scenario; nine Vitest flows; frontend typecheck/build; Playwright; docs/diff checks.
- Result: Windows qualification is green. Exact commit `4e5c432` also passed complete Ubuntu `./scripts/verify.sh` with `verify-final.exit=0`. Missing baselines stop before model calls; identical real renders pass; a full-viewport visual mismatch fails; dimension and area thresholds are exact; baseline mutation after launch is rejected. Initial exact commit `4ca6465` correctly showed that the former small 404 fixture changed fewer than the allowed 1% of pixels on Linux.
- Follow-up: Implement provenance-backed research evidence.

### 2026-08-14 - Provenance-Backed Research Evidence

- Context: Research success conditions had no deterministic evidence path; accepting a model-authored answer or citation list would not prove scope, source reachability, quote grounding, freshness, or durable provenance.
- Decision: Parse `RESEARCH EVIDENCE project/relative/report.json`; bind report scope to the exact task goal; capture its bounded UTF-8 bytes through governed filesystem read; derive exact credential-free network origins only after parsing; and execute typed `research.verify` through policy, leases, audit, and the hardened HTTP executor. Revalidate JSON SHA/artifact identity, cap claims/citations/sources, share one timeout, retain every fetched body, validate optional age and conflict references, and leave semantic truth/support to the independent Evaluator.
- Files: `crates/mcg-{agents,tools,runtime,api}/`, `README.md`, `docs/architecture/{runtime-spine,agent-organization}.md`, `docs/testing/strategy.md`.
- Commands/tests: Strict full-workspace Windows clippy; 38 tool tests including real HTML/plain-text sources; 31 API tests including public pass/fabricated-quote-fail/replay; focused runtime/agent tests; docs and diff checks.
- Result: Windows-focused gates are green. Exact commit `7566cf4` also passed complete Ubuntu `./scripts/verify.sh` with `verify-final.exit=0`. The report points to the immutable JSON SHA, audited `research.verify` retains JSON plus deterministically ordered source artifacts and URL/digest evidence, scope mismatch and post-binding mutation fail, optional freshness/conflict invariants are bounded, and terminal replay adds no model call, fetch, or audit record.
- Follow-up: Broaden representative workflows, UI depth, and remaining typed objective evaluators.

### 2026-08-14 - Immutable Typed Browser Workflow Evidence

- Context: A rendered-page snapshot could not prove that a real consumer journey remained usable, while arbitrary browser scripting would bypass the typed policy boundary and could navigate, download, or mutate remote services.
- Decision: Add `ELEMENT EXISTS URL selector` and `BROWSER WORKFLOW project/relative/file.json`. Bind version-1 JSON bytes/name/SHA before model work; re-read through governed project authority; interpret only bounded element/text/URL assertions, clicks, typing, and allowlisted keys; and confine the browser to same-origin GET/HEAD. Inspect interaction targets before action and reject downloads, new contexts, cross-origin destinations, and non-GET forms.
- Files: `crates/mcg-{agents,tools,runtime,api}/`, `README.md`, `docs/architecture/{runtime-spine,agent-organization}.md`, `docs/testing/strategy.md`.
- Commands/tests: Windows strict workspace clippy and broad Rust suites excluding visible live-locked binaries; 39 tool and 31 API tests; live GPU45; frontend typecheck, 9 Vitest tests, build, Playwright, and docs. Exact `f9f9fc6` complete Ubuntu `./scripts/verify.sh` in 299 seconds.
- Result: Element checks objectively pass/fail after hydration. Browser workflows retain immutable JSON, final HTML/PNG, and ordered step evidence; intended multi-step interaction passes, false text fails, file mutation is rejected, unsafe interactions fail before action, and a missing workflow fails before model spend. Windows and Ubuntu qualification are green with no GitHub-hosted automation.
- Follow-up: Build richer consumer task progress and evidence UX, provider/project setup flows, accessibility evidence, and representative end-to-end projects.

### 2026-08-14 - Consumer Evidence And Provider Workspace UX

- Context: The daemon already held durable task/tool evidence and all required provider contracts, but the consumer desktop exposed only event history and subscription login; project discovery also lacked a useful workspace view.
- Decision: Load events and governed tool audit as one evidence experience with progressive disclosure. Reuse the authenticated provider configure/list/discover boundary for OpenAI API, OpenRouter, and compatible/local endpoints, keep credentials in an explicit password field that crosses only the Tauri command and is zeroized there, and render unknown costs honestly. Surface durable projects with simple context and advanced diagnostics.
- Files: `apps/desktop/`, `tests/e2e/desktop-smoke.spec.ts`, `README.md`, `docs/architecture/{provider-fabric,system}.md`, `docs/testing/strategy.md`.
- Commands/tests: Windows formatting and strict workspace clippy; broad Rust workspace tests excluding visible locked binaries; 39 tool and 31 API tests; live GPU45; real Chromium; frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene. In-app browser visual QA covered desktop and 720px layouts.
- Result: Consumer task cards now expose objective evidence without reconstructing state in the frontend; every required non-OAuth provider family can be configured and refreshed natively; discovered projects are selectable and informative. A narrow-layout overflow found during visual QA was fixed and regression-tested. Windows and exact-commit Ubuntu qualification are green at `1d65197`.
- Follow-up: Exercise these flows inside the native app with representative projects and continue the remaining product surfaces.

### 2026-08-14 - Restart-Safe Provider Turn Continuation

- Context: Approval paused an agent after a paid provider response but persisted no conversation position. Local resume replayed the first inference, external MCP resume failed closed, and daemon recovery expired the waiting agent/lease. The in-memory provider router also lacked durable catalog routes after restart until explicit rediscovery.
- Decision: Add migration 12 and a bounded `AgentTurnCheckpoint` containing the exact request/response, pinned route, attributed calls, redacted completed tool outcomes, next tool index, and pending request identity. Persist after every provider response and before/after each tool call; preserve waiting authority on restart only when checkpoint, unexpired approval, lease, and projections agree. Revalidate project context, role, current provider catalog, MCP schema/classification, and exact approval before continuation. Remove the checkpoint atomically on completion, failure, rejection, or cancellation. Restore validated provider catalogs locally without network discovery after restart.
- Files: `crates/mcg-{agents,providers,storage,runtime,api}/`, `README.md`, `docs/architecture/{agent-organization,mcp,provider-fabric,runtime-spine}.md`, `docs/testing/strategy.md`.
- Commands/tests: Windows strict full-workspace formatting/clippy; broad Rust tests excluding only the visible locked daemon/desktop executables; 32 API and 39 tool tests; live GPU45 and real Chromium; frontend typecheck, 10 Vitest tests, production build, two Playwright tests, docs, and diff hygiene.
- Result: Both fixtures preserve the waiting run/instance/lease across daemon reopen, record zero side effects before approval, continue the pinned conversation without replaying the pre-approval inference, execute the approved side effect exactly once, and remove the consumed checkpoint atomically. Windows and exact-commit Ubuntu qualification are green at `ae28c50`.
- Follow-up: Extend the same checkpoint discipline to unattended scheduling and broader run-control surfaces.

### 2026-08-14 - Durable Scheduler Spine

- Context: MCG had durable task/run recovery but no persisted unattended-work admission. A timer-only worker could duplicate a paid turn or side effect after a crash and had no reliable approval/budget boundary.
- Decision: Add migration 13 with versioned one-time/fixed-interval definitions and durable occurrences. Atomically advance the definition, create the occurrence lease, durable task, projection, and task event; admit one active occurrence per schedule; expire scheduler authority on startup; and reclaim the same occurrence/task before invoking idempotent consumer launch. Pause admission at preflight budget exhaustion and approval/manual-evaluation boundaries. Retain history for pause/resume/remove and reconcile terminal approval-gated tasks.
- Files: `crates/mcg-{domain,storage,runtime,api}/`, `apps/mcg-{daemon,cli}/`, `README.md`, `docs/architecture/system.md`, `docs/testing/strategy.md`.
- Commands/tests: Focused storage/API tests, isolated file-backed daemon restart tests, workspace check, and strict all-feature/all-target clippy on Windows.
- Result: Atomicity, non-overlap, immediate restart reclaim, identity preservation, safe pause, authenticated control, and governed dispatch are implemented and qualified on Windows plus exact-commit Ubuntu at `9d0af9d`.
- Follow-up: Add cron, event/file watches, wake conditions, adaptive heartbeat, notifications, and native scheduler UI over the same occurrence/lease contract.

### 2026-08-14 - Durable Scheduler Triggers And Native Dashboard

- Context: The qualified occurrence/lease spine lacked cron, durable observations, subsystem attention, user-visible outcomes, and a consumer scheduling surface.
- Decision: Add migration 14 wake/notification records; five-field UTC cron; durable event cursors that exclude their own occurrence tasks; content-hashed, normalized, symlink-rejecting project file watches; adaptive busy/idle heartbeat; safe default token/duration/model-call limits; and owner-pause-preserving wake requests. Expose the contracts through authenticated REST, CLI, Tauri, and a simple/advanced desktop dashboard with history, notifications, budgets, and state controls.
- Files: `crates/mcg-{domain,storage,runtime,api}/`, `apps/{mcg-daemon,mcg-cli,desktop}/`, `tests/e2e/desktop-smoke.spec.ts`, `README.md`, and scheduler architecture/testing documentation.
- Commands/tests: Windows strict all-target clippy; broad Rust suites excluding only live-locked daemon/desktop package binaries; 36 API, 19 storage, 39 tool, live GPU45, 11 Vitest, production build, two Playwright flows, docs/diff checks; isolated two-test real daemon restart suite. The in-app and connected Chrome browser plugin both blocked localhost with `ERR_BLOCKED_BY_CLIENT`; local Playwright interaction remained green.
- Result: Windows and exact-commit Ubuntu gates are green at `a1ea1d4`. Multi-year overdue intervals advance in constant time, watch/event state and notifications survive reopen, own events cannot recurse, wake text/schema inputs are bounded, approval completion is reported, and daemon restart preserves the same occurrence/task identity.
- Follow-up: Add idempotent first-party schedule creation and MCP `scheduler.create/list/remove` tools.

### 2026-08-14 - Idempotent Schedule Creation And MCP Lifecycle

- Context: First-party schedule callers could lose a successful response and create duplicate unattended work on retry; MCP clients could inspect runs but not manage the scheduler lifecycle.
- Decision: Let callers own the schedule UUID, hash the complete creation request server-side, replay only an exact match, and conflict on changed reuse. Preserve desktop identity until success, expose optional CLI `--schedule-id`, and add authenticated MCP create/list/remove tools over the shared creation boundary. Removal stops admission but retains definitions and occurrences.
- Files: `crates/mcg-{domain,storage,api}/`, `apps/{mcg-cli,mcg-daemon,desktop}/`, `README.md`, `docs/architecture/{system,mcp}.md`, and `docs/testing/strategy.md`.
- Commands/tests: Focused Windows storage/API/MCP/CLI/desktop suites, TypeScript check, and strict all-feature/all-target clippy are green.
- Result: Exact retries converge on one durable definition across REST, desktop, CLI scripts, and MCP; conflicting identities and malformed definition hashes fail closed. Windows and exact-commit Ubuntu gates are green at `86e54b1`.
- Follow-up: Continue broader M7 exit scenarios.

### 2026-08-14 - Integrated M7 Watched-Work Exit Scenario

- Context: Scheduler restart identity, watch persistence, consumer checkpointing, governed objective checks, and approval safe-pause were each qualified separately, but M7 exit evidence needed one adversarial scenario spanning all boundaries.
- Decision: Add a real guided file-watch fixture that triggers one scheduled consumer task, checkpoints provider work at owner approval, approves the exact discovered checks, reopens the runtime, resumes without another model call, executes each check once, reconciles the original occurrence, retains completion notification/history, and proves a later tick admits nothing.
- Files: `crates/mcg-api/src/lib.rs`, `docs/testing/strategy.md`.
- Commands/tests: Focused scenario, strict workspace clippy, documentation validation, and the broad Windows Rust lane with real Chromium/live GPU45 are green.
- Result: M7's restart/no-duplicate-side-effect/approval-boundary exit invariant is proven end to end on Windows and exact-commit Ubuntu at `6dd62b3`.
- Follow-up: Begin M8 expanded execution-plane work.

### 2026-08-14 - Persistent Browser Execution Plane

- Context: M8 had durable browser reservations and authority history but no long-lived executor, so `requested` could not truthfully become `active` and no interface could reuse or hand off a real page.
- Decision: Launch one isolated ephemeral Chromium profile/process per browser session; require an immutable non-empty origin grant; allow only intercepted GET/HEAD traffic; deny downloads; commit activation only after the browser and page are live; expose only redacted observations; permit handoff only for visible windows; blank to `about:blank` before reclaim; and let durable close/expiry win before bounded executor/profile cleanup.
- Files: `crates/mcg-tools/src/persistent_browser.rs`, `crates/mcg-{storage,runtime,api}/`, `apps/{mcg-cli,desktop}/`, and `docs/architecture/platform-sessions.md`.
- Commands/tests: Strict workspace clippy; broad unlocked Rust suites (42 API, 22 storage, 41 tools, 7 runtime); isolated daemon/desktop Rust tests; docs; typecheck; 14 Vitest flows; production build; two Playwright flows. Real Chromium tests cover reused page state, origin enforcement, redaction, expiry cleanup, history, and authenticated REST/MCP interoperability.
- Result: The complete Windows lane and exact Ubuntu commit `d5de79e` are green. Every primary interface can request, attach, navigate, hand off, reclaim, inspect, and close the same truthful durable browser session. The first exact Ubuntu run of `935050e` correctly failed because the manager's direct real-browser unit fixture used library auto-detection instead of the configured `MCG_BROWSER_EXECUTABLE`; the corrected fixture and complete clean rerun pass.
- Follow-up: Persistent-browser automated qualification and native UAT are complete on Windows and Linux; continue durable computer-use execution on the same authority model.

### 2026-08-14 - Immutable Site Revisions And Durable Monitoring

- Context: The governed Site runtime lacked revision-safe lifecycle changes and automatic health admission; splitting occurrence settlement from monitor projection updates could lose an alert during a crash.
- Decision: Require expected-revision updates with immutable history and terminal archive. Bind one Site to one regular interval schedule, derive health-check identity from the occurrence UUID, and atomically settle the leased occurrence, monitor projection, and state-change notification. Treat reachable unexpected statuses as degraded; emit no baseline alert and coalesce unchanged states.
- Files: `crates/mcg-{domain,storage,runtime,api}/`, `apps/{mcg-cli,desktop}/`, `README.md`, `docs/architecture/sites.md`, `docs/testing/strategy.md`.
- Commands/tests: Exact commit `045dc939831a6afc6179578ef58c7fa464e1d2c5` passed complete clean Windows `scripts/verify.ps1` in 339.8 seconds and complete Ubuntu `scripts/verify.sh` in 354.7 seconds. This includes strict workspace clippy, full Rust/restart/live GPU45/browser/computer suites, 44 API tests, 17 desktop Vitest tests, documentation/typecheck/build, native Tauri builds, and two Playwright E2E tests per OS. Ubuntu log: `/home/hendo420/mcg-worktree-045dc93/verify-linux.log` (last verified 2026-08-14).
- Result: REST, MCP, CLI, Tauri, and desktop expose update/archive and monitor configuration. Restart persistence, exact monitor replay, healthy/outage/repeated-outage/recovery behavior, and transactional pause/archive schedule synchronization are cross-platform qualified. Computer-use inspection of the exact Windows binary confirmed all day-one navigation and the simple-mode Sites registration surface with accessible labels.
- Follow-up: The exact native binary could not perform stateful Site UAT against the currently running older daemon without replacing its preserved OAuth handoff; stateful behavior is covered by exact-runtime integration/E2E tests. Repeat the complete native stateful journey when the live daemon can be upgraded safely, and continue typed SSH.

### 2026-08-14 - Typed Outbound SSH Checkpoint

- Context: M8 required useful remote execution without adding built-in remote access to the harness.
- Decision: Use outbound OpenSSH only, immutable owner-pinned host revisions, SSH-agent authentication, four typed POSIX operations, exact capability resources, and the shared approval/lease/audit broker. A claimed crash becomes non-replayable uncertainty.
- Files: `crates/mcg-{domain,policy,storage,tools,runtime,api}/`, `apps/{mcg-cli,desktop}/`, `docs/architecture/typed-ssh.md`.
- Commands/tests: Exact `f5a1a1b` complete Windows `scripts/verify.ps1` passed in 424 seconds. Exact archived source passed Ubuntu `scripts/verify.sh` in 136 seconds after disk cleanup. Live Ubuntu agent-backed runtime inspect/restart/replay passed in 0.28 seconds. Strict workspace Clippy; 50 tool tests, 28 storage tests, 10 runtime tests, 44 API tests, 18 Vitest tests, production/native builds, two Playwright flows, and docs are green.
- Result: REST, CLI, MCP, Tauri, and desktop share one credential-free pinned-host/operation contract. Real Windows-to-Ubuntu probing found that the local Windows `ssh-agent` service is disabled; MCG correctly did not fall back to a password/private-key option. The executor now uses a real isolated empty config, and Windows prefers native System32 OpenSSH to avoid Git/MSYS path-dialect ambiguity. Ubuntu initially filled its 48 GiB root volume with retained build targets; `cargo clean` removed 16.8 GiB only from `/home/hendo420/mcg-worktree-045dc93/target`, after which the complete gate passed.
- Follow-up: Windows live SSH UAT requires enabling the native OpenSSH Authentication Agent service and loading the owner key; native stateful UI UAT still requires safely upgrading the visible daemon. Linux live qualification passed wrong-key rejection, literal typed execution, hash-verified upload/download, cleanup, restart, and exact replay.

### 2026-08-14 - Dedicated Win11 Consumer UAT Checkpoint

- Context: M8 required real Windows UAT on a clean owner-controlled machine rather than a development host.
- Decision: Use `Win11-VM` through Chrome Remote Desktop and transfer only hash-verified locally built qualification artifacts. Keep GitHub source-only and do not weaken the host firewall for transfer.
- Files: `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src/App.tsx`, `apps/desktop/src/App.test.tsx`, `packaging/windows/portable/`.
- Commands/tests: Tauri production `build --no-bundle`; 20 Vitest flows; root typecheck; Rust formatting; 2 desktop Rust tests; strict desktop Clippy. Final native desktop SHA-256 `E1CE0ECDD56D835A1CE0D07E5D190889696256481A77A3E489787A83E51C6CC2` was verified through the VM transfer path; refreshed qualification archive SHA-256 is `6830CAB0221488950B0710F4E240A8FD4C71AFB83AD0CBB47181EA9D41173C55`.
- Result: The VM had no Git, Node, Rust, Cargo, or rustc. The production app launched with its bundled web assets; GPU45 discovered 2 current models; CLI status/provider/project/task commands worked; the native picker registered a second project; a real task blocked honestly when no model satisfied tool-routing requirements; daemon termination/restart preserved the provider, both projects, blocked task, and evidence. UAT found and fixed the obsolete `/api/v1/projects/discover` desktop route, invisible project-screen errors, and a blocked evidence summary that said `In progress`. Direct Cargo packaging was rejected because it embedded the development URL; Tauri's production build path is required. A case-insensitive `MCG.exe`/`mcg.exe` collision was removed from the portable layout.
- Follow-up: Run the corrected native journey on Linux, then close M8. Production installer, signing, service registration, upgrade/repair, and uninstall gates remain open and the portable bundle is qualification-only.

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
- 2026-08-14: Completed and verified M7 scheduler and durable autonomy across Windows and Ubuntu: all trigger families, leases/restart recovery, notifications/history/UI, bounded unattended policy, caller-idempotent creation across first-party/MCP interfaces, and an integrated watched-work approval/restart scenario proving no duplicate model/tool side effects.

## Open Follow-ups

- Choose a public-source license only if/when public distribution is considered; repository remains private.
- Include the full Apache-2.0 license text for adapted OpenAI Codex login code in release packages.
- Continue refining the established simple/advanced information architecture and visual system through representative project workflows and accessibility UAT.
- Add owner-controlled macOS build and real-device qualification when capacity becomes available.
- Continue M5 representative useful projects and broader end-to-end evidence while M6 interfaces are built; the durable provider/tool website and independent evaluator scenarios are complete checkpoints, not the full product.
- Persist standalone provider-proxy usage/conversations and attach optional governed project/task context; initial proxy responses currently return usage without a durable proxy ledger.
- Add independent browser/evaluator evidence to representative governed website work as M5 progresses.
- Complete MCP server subscriptions/interoperability plus MCP client stdio negotiation and real native MCP UAT.
- Expand consumer objective settlement beyond the qualified guided, command, HTTP, browser render, element, typed browser workflow, file, exact-discovered custom, screenshot, and research evaluators to accessibility checks, service-health semantics, and other typed conditions; unsupported conditions deliberately remain nonterminal.
- Qualify the newly wired standalone OpenAI OAuth provider management on Windows and Ubuntu, then perform live-account OAuth/model/tool UAT; do not expose tokens or fall back to API billing.

## Do Not Forget

- Read `AGENTS.md` and `MEMORY.md` before making future changes in this workspace.
- Update `MEMORY.md` after durable discoveries, decisions, fixes, verification results, blocked work, and handoffs.
- Never store secrets, tokens, passwords, private keys, or sensitive personal data in `MEMORY.md`.

## Archive

- No archived entries yet.
