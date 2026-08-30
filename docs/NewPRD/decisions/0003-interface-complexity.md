# ADR-003 — Simple, Advanced, and Developer Interface Complexity

- **Status:** Accepted by product owner
- **Date:** 2026-08-29
- **Source:** Owner design discussion and the PRD presentation/authority boundary

## Context

MetaCog currently stores an `advanced_mode` Boolean. The cognitive runtime introduces useful professional controls and much deeper experimental/diagnostic machinery. Combining both in one Advanced interface would make Advanced unsuitable for routine engineering work, while exposing everything in Simple would make the product inaccessible.

## Decision

MetaCog has one runtime and three presentation complexities:

```text
Simple -> Advanced -> Developer
```

The preference controls visibility, information density, terminology, and access to editors/inspectors. Selecting a complexity does not itself change runtime configuration, model routing, approval policy, memory, evaluation, authority, or task behavior.

### Simple

Shows ordinary chat, projects, approvals, understandable workflow progress, verified outcomes, remaining uncertainty, and concise retained-learning summaries.

### Advanced

Adds practical engineering control and evidence: workflows, invocations, worktrees, agents, commands, diffs, hooks, plugins, providers, policies, capabilities, memory, recovery, usage, and detailed verification.

### Developer

Adds under-the-hood inspection and authorized configuration: runtime lineage, canonical manifests, context/memory policy, competence and coordination controls, ImprovementCases, experiments, disclosed evaluator summaries, exposure/taint, promotion/rollback, IPC, process, sandbox, and diagnostic state.

Developer does not mean unrestricted. Protected policy, evaluator fixtures/raw results, Root authority, signing keys, secrets, direct database writes, self-promotion, and persistent trust bypass remain inaccessible or read-only with an explanation.

### Migration

- Legacy `advanced_mode=false` maps to `Simple`.
- Legacy `advanced_mode=true` maps to `Advanced`.
- No existing or new user enters `Developer` automatically.
- During the compatibility window, old API requests are accepted and translated with revision/CAS checks.
- New records persist a stable enum/discriminator rather than two partially synchronized settings.

### CLI

Interactive palettes and default presentation may follow the selected complexity. Explicit stable commands remain callable and deterministic regardless of GUI preference. `mcg help --all` or equivalent full help exposes all supported commands; unavailable capabilities remain absent or clearly unavailable rather than simulated.

## Consequences

- The existing Boolean must be migrated across domain, storage, API, Tauri, React, CLI presentation, tests, backups, and documentation.
- Every visible feature must declare its minimum presentation complexity.
- Tests must prove switching complexity does not mutate backend workflow/configuration state.
- UAT must exercise the same active workflow across all three levels.

## Rejected alternatives

- **Keep Simple/Advanced only:** overloads Advanced with research machinery.
- **Separate applications:** risks divergent behavior and data.
- **Call the third level Hacker/Root/God mode:** implies security bypass or authority that the UI does not possess.
- **Make complexity an execution profile:** violates the single-runtime product decision.

## Verification

- Settings migration is deterministic and reversible within the supported downgrade window.
- Simple, Advanced, and Developer query the same underlying workflow IDs and revisions.
- Switching levels during active work changes only presentation.
- Protected mutations fail at Root even if a Developer client fabricates a request.
