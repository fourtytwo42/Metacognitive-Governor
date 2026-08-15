# Product and Engineering Baseline

- Status: Accepted baseline; individual choices may be superseded by later ADRs.
- Date: 2026-08-13
- Source: Owner answers plus conservative engineering defaults.

## Product

- MCG is for general consumers, including people who are not developers.
- Installation and first-run setup must be simple. A normal user should not need
  to install Rust, Node.js, Python, databases, or a separate daemon manually.
- The complete PRD remains the committed product direction. Delivery will be
  incremental, but milestones are sequencing devices rather than excuses to
  discard major interfaces or subsystems.
- The first validation projects will include building and testing websites or
  small applications. Several real projects will be used to expose weaknesses in
  governance, tools, memory, routing, and evaluation.
- The first public product release includes the native GUI, CLI, daemon/headless
  operation, OpenAI-compatible API, and MCP client/server. They share one runtime
  and domain model; none is implemented as an unrelated second product.
- MCG is single-owner and local-first initially. Built-in remote access to the
  harness is out of scope for the initial product. Local API surfaces bind only
  to loopback. Future remote or multi-user operation requires a new threat model
  and an explicit owner decision.

## Experience

- The desktop GUI is the primary consumer surface and defaults to a project/task
  experience, not an infrastructure dashboard.
- The visual language blends a consumer chat application, coding workspace, and
  operational dashboard. Like LM Studio, it starts in a simple mode and reveals
  deeper controls through a small number of understandable advanced settings.
- Normal mode shows goals, progress, approvals, costs, outcomes, and concise
  agent activity. Raw prompts, model calls, events, tool payloads, and experiments
  remain in developer mode.
- A single installer bundles the desktop application, daemon, CLI, migrations,
  and required web assets. The daemon continues eligible work when the GUI closes.
- Destructive or externally consequential actions require clear approval. Safe,
  project-local reads and reversible edits can proceed under project permission.
- Users can interrupt, steer, cancel, retry, and resume work. Background spending
  is constrained by explicit budgets and scheduled-task policy.

## Application architecture

- Use a Rust workspace with separate crates for domain types, persistence, event
  bus, policy, tools, providers, agents/governor, scheduler, daemon/API, CLI, and
  Tauri integration. Dependency direction points toward stable domain crates.
- Use Tokio for asynchronous work, Serde for boundary types, tracing for
  observability, and SQLite as canonical local operational storage.
- Treat the event log as append-oriented audit history. Build transactional
  projections for current state rather than reconstructing all UI state at every
  read. Version schemas and migrations from the first commit.
- Use a versioned localhost HTTP API plus WebSocket/SSE event streaming between
  clients and the daemon. Bind locally by default. Remote binding requires
  authentication, TLS termination guidance, and explicit enablement.
- Tauri with React and TypeScript is the default UI stack. The UI consumes the
  same public daemon contracts as other clients where practical.
- A user-level persistent daemon is the initial installation model. OS service
  installation is an advanced/headless option because service accounts complicate
  desktop permissions, browser profiles, and consumer setup.

## Tasks, agents, and governance

- A thread is human conversation continuity. A task is an executable goal with a
  task envelope. A run is one attempt under a specific organization/configuration
  version. Agent instances are ephemeral workers fulfilling persistent roles.
- Start with Executive, Conversation, Coder, Evaluator, and Tool Worker roles.
  The Executive avoids decomposition when one role can complete the work.
- Agents receive isolated task context plus explicit shared artifacts and event
  references. Direct agent communication is mediated and auditable.
- MCG begins with deterministic observation and recorded recommendations. Prompt
  and routing changes are versioned proposals before increasingly autonomous
  promotion is enabled by owner policy and benchmark evidence.
- Users can pin known-good organization revisions and disable experimentation.
- Shadow model execution is opt-in for normal users and enabled in development
  and scenario environments. Shadow output never affects the real task unless it
  is separately promoted through an evaluated path.

## Providers and authentication

- Initial provider families: OpenAI, OpenRouter, local OpenAI-compatible servers,
  and arbitrary OpenAI-compatible endpoints. Users bring their own credentials.
- Credentials are stored through the operating-system credential store. Database
  records contain opaque credential references, never plaintext secrets.
- Provider onboarding calls `/models` when offered, records the raw advertised
  models, and enriches them with capability probes and known metadata. Endpoint
  claims are not blindly trusted; streaming, tools, structured output, modalities,
  and context behavior can be probed and cached with freshness timestamps.
- Known model prices are shown with provenance and an effective date. Unknown
  custom-provider prices remain explicitly unknown; users may enter overrides.
- Routing begins with explainable policy and capability filters, then incorporates
  measured success, latency, and cost. Users can pin models and providers.
- ChatGPT subscription authentication is an optional but encouraged provider path.
  OpenAI API keys are an equally supported, explicit alternative. Official OpenAI
  documentation supports browser-based ChatGPT sign-in for local Codex clients
  and a beta device-code flow for headless clients. MCG should offer both OAuth
  flows, preferring the browser callback in the desktop UI and device code when
  callbacks are unavailable.
- Implement ChatGPT/Codex authentication as an isolated Rust adapter derived from
  reviewed, license-compatible open-source components such as OpenAI's
  `codex-login` crate and OpenClaw's integration. Reusing an auth component does
  not make MCG dependent on the Codex agent runtime. The adapter owns secure token
  storage, refresh, revocation, account display, and conformance tests.
- API-key access is a separately supported OpenAI provider mode. OAuth failures
  block only the affected OAuth connection and surface a precise recovery message.
  MCG never silently changes an existing connection from subscription OAuth to API
  billing; the user must explicitly add or select an API-key connection.
- Development integration targets include the owner's OpenAI OAuth account and
  local OpenAI-compatible provider `gpu45`. Hostnames, tokens, and credentials are
  configuration, never committed repository data.

## Tools, permissions, and security

- All real-world actions pass through typed tool requests and capability checks.
  Structured process execution is preferred; an explicit shell capability exists
  for commands that require shell semantics.
- Permissions combine capability, executable, arguments, path scope, network
  destination, risk class, and task context. Prompt instructions are never the
  enforcement boundary.
- Default policy is meaningfully autonomous: project-scoped reads, reversible
  writes, builds, tests, and approved tool classes may proceed without repeated
  confirmation. Always ask for destructive deletion, credential access, external
  publication, Git push, production deployment, purchases, or privilege elevation.
  Users can select simpler autonomy presets and customize individual capabilities.
- Approval grants can be one-time, task-scoped, session-scoped, or persisted as an
  owner policy. Unattended jobs fail safely or wait for approval unless a specific
  preauthorization covers the exact action class.
- The owner constitution is versioned, audit-visible, and immutable to MCG. Policy
  edits require owner authentication/confirmation and cannot disable audit history.
- Remote access is not built into the initial harness. Secret redaction occurs
  before persistence and model-context assembly. Users can inspect, export, and
  delete stored memory.

## Memory and project intelligence

- Private MCG storage is the default for global, thread, role, and operational
  memory. Repository `MEMORY.md` is supported when already present or explicitly
  enabled, with visible diffs and version history.
- Global preference memory and cross-project learning remain distinct from
  project facts. Project facts do not silently leak into unrelated model context.
- Raw task/event retention is configurable. Derived memories retain provenance,
  confidence, freshness, and links to supporting observations.
- Embeddings are optional retrieval indexes, never canonical state. Local embedding
  is preferred when available; external embedding requires informed consent.
- Project discovery is incremental: instructions and cheap metadata first, deeper
  indexing asynchronously. File watching invalidates relevant cached intelligence.
- Concurrent modifying agents use isolated worktrees or equivalent sandboxes when
  possible. Git commits, pushes, and pull requests follow explicit user policy.

## Testing and evidence

- Testing is part of the runtime architecture. Required layers are Rust unit and
  property tests, database migration tests, provider/tool contract tests,
  integration tests, deterministic fake-provider tests, scenario/evaluation tests,
  UI component tests, and Playwright end-to-end tests through the packaged app.
- Every shipped workflow has machine-observable success conditions. Website work
  uses builds, tests, HTTP checks, browser semantics, screenshots when useful, and
  deployment health checks.
- Windows, Linux, and macOS packages ship together. Development and automated
  release qualification run on owner-controlled hosts for all three platforms;
  macOS is a first-class gate with native package, signing, and real-device UAT.
  Release gates include clean installation, first-run onboarding, provider connection, task
  execution, restart recovery, approval enforcement, and backup/restore.
- Nondeterministic model behavior is tested with recorded fixtures and fake models
  for protocol guarantees, plus opt-in live-provider evals for behavior quality.
- Manual exploratory testing remains useful, but releases should not depend on
  repetitive human regression testing.

## Delivery sequence without scope reduction

1. Repository foundations, contracts, persistence, eventing, policy, and test
   harnesses.
2. One complete website-building workflow through daemon, GUI, CLI, provider API,
   and MCP boundaries using fake and live providers.
3. Full project/thread/scheduler/tool/provider/memory functionality and consumer
   packaging.
4. Browser, computer use, sites, remote execution, and hardened approvals.
5. Measured MCG intervention, shadow evaluation, experiments, reversible learning,
   and organizational evolution.
6. Cross-platform completion, Android capability profile, plugin ecosystem,
   multi-host operation, and long-horizon optimization.

Each stage must leave production-quality contracts and tests. “Whole product”
means every committed subsystem is ultimately delivered; it does not mean all
subsystems are implemented simultaneously before integration begins.

## Deferred owner choices

The repository remains private. A public-source license is intentionally deferred
until public distribution is considered. Product naming, detailed visual design,
and the exact macOS support disclaimer require later decisions; no schedule or
spending constraint is currently imposed.
