# M4 Project Intelligence and Memory

- Status: Implemented and locally qualified on Windows and Linux.
- Date: 2026-08-14.
- Governing milestone: M4 in `docs/roadmap/milestones.md`.

## Project discovery

`mcg-projects` builds bounded, deterministic snapshots without following
symlinks or indexing generated dependency/build directories. Discovery resolves
the nearest Git root and records Git branch, head, dirty state, recent commits,
languages, build/test systems, package managers, Docker/service indicators, CI
systems, important directories, manifest commands, existing project memory, and
a small provenance-bearing world model.

Snapshot fingerprints cover instruction files, build/package/service manifests,
CI definitions, and Git head/dirty state. A polling `ProjectWatcher` compares
fingerprints and reports changed inputs plus whether instructions, commands, or
the world model require rebuilding. Unchanged polls produce no snapshot.

Every accepted snapshot is stored immutably in SQLite. The current project table
is a rebuildable projection, and snapshot insertion, projection update, and the
`project_discovered` event commit in one transaction. Canonical roots cannot be
silently reassigned to a different project identity.

## Instruction resolution

Discovery recognizes hierarchical `AGENTS.md`, `AGENTS.override.md`,
`CLAUDE.md`, `CLAUDE.local.md`, `.claude/rules/*.md`, `.cursor/rules/*.mdc`,
`README.md`, and `CONTRIBUTING.md` documents. Each document retains its content,
scope, precedence, and SHA-256 provenance.

Resolution is performed for a specific project-relative target. Nested rules do
not leak into root context, while a nested rule can supersede its applicable
parent. Known package-manager contradictions are emitted as explicit conflicts
with `more_specific`, `higher_precedence`, or `unresolved` status rather than
being silently concatenated.

## Context assembly

Context assembly combines applicable instruction documents, surfaced conflicts,
manifest-derived commands, current fresh memory, and task-provided items. It is
deterministic and token-bounded. Goals, constraints, decisions, open questions,
evidence, and plans are ranked ahead of old transcript material; protected items
are structurally truncated only when needed to honor the requested bound.

The current estimator is deliberately provider-independent and conservative.
Later model-specific tokenizers may refine estimates without changing the
preservation order or context contract.

## Structured and managed memory

Every claim includes project scope, namespace/key, JSON value, confidence,
provenance, observation time, optional freshness deadline, lifecycle state,
usefulness/correction counts, and a predecessor link. Secret-like namespaces and
keys fail closed before persistence.

Corrections, reinforcement, decay, and retraction create successor versions.
The prior head becomes superseded in the same SQLite transaction that inserts
the successor and event. A partial unique index permits only one active head for
each project/namespace/key, preventing concurrent correction forks. Claim
versions cannot be deleted. Current retrieval is project-scoped and can exclude
stale claims; semantic/vector indexes remain optional derivatives and are never
canonical state.

Working and durable heads can be rendered as stable human-readable Markdown.
JSON API/CLI history is the lossless export surface because it retains lifecycle
and provenance fields that Markdown intentionally summarizes.

## Interfaces

The authenticated loopback API and CLI support:

- project add/list/get/refresh/history;
- target-aware bounded context assembly;
- memory add/list/correct/history;
- reinforcement, decay, and retraction;
- managed-memory Markdown rendering.

The CLI accepts ordinary consumer strings as memory values and also preserves
objects, arrays, numbers, booleans, and null when valid JSON is supplied.
Desktop project/memory inspection and editing use these same contracts in M6.

## Qualification

Fixture and integration tests cover stable project fingerprints, hierarchical
scope, instruction conflicts, manifest invalidation, protected context
compression, secret rejection, correction-driven durability, immutable snapshot
history, atomic claim versioning, concurrent/stale correction rejection, API
lifecycle behavior, and restart recovery.

Complete owner-machine verification uses:

```powershell
.\scripts\verify.ps1
```

```bash
./scripts/verify.sh
```
