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

The world model is a deterministic provenance graph rather than a flat filename
list. Root package/Cargo/Python/Go manifests contribute bounded dependencies and
runtimes. Docker Compose contributes named services, container images, explicit
`depends_on` edges, database types derived from declared images, and published
local endpoints. OpenAPI/Swagger JSON or YAML contributes named APIs and
credential-free server origins. Known Vercel, Netlify, Fly.io, Render,
Cloudflare Workers, Railway, and Docker manifests contribute deployment nodes.
Kubernetes YAML in conventional root files or `k8s/`, `kubernetes/`,
`deploy/kubernetes/`, and `manifests/kubernetes/` contributes Deployment,
StatefulSet, DaemonSet, Job, and CronJob workloads; init and application images;
image-derived databases; selector-matched Service endpoints; and HTTP(S)
Ingress routes. Arbitrary YAML is not interpreted as Kubernetes configuration,
and Secret documents and secret-like labels never enter the graph.
Package `homepage` contributes only a credential-free HTTP(S) origin. Every node
and relationship retains its manifest source and uses a content-stable identity;
the desktop resolves those identities back to consumer-readable labels.

Discovery caps each manifest at 2 MiB, YAML documents, services, ports,
dependencies, nodes, and relationships at explicit limits, never follows
symlinks, and rejects malformed topology manifests. URLs containing user info
are discarded so the graph cannot become a credential store. Manifest changes
invalidate the world-model projection while unrelated files do not.

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
manifest-derived commands, readable world-model nodes and relationships,
current fresh memory, and task-provided items. It is deterministic and
token-bounded. Goals, constraints, decisions, open questions, evidence, and
plans are ranked ahead of old transcript material; protected items are
structurally truncated only when needed to honor the requested bound. Declared
dependency and database relationships rank ahead of isolated inventory under
compression, and graph identities never enter model-facing text.

The same assembly supplies REST, CLI, MCP, provider-proxy, ordinary worker, and
isolated evaluator turns. Every role therefore receives source-backed topology
within its caller-selected bound; evaluators do not use a less-informed parallel
context path. This makes topology available to model planning and evaluation
without allowing discovery to invent a plan or an undeclared relationship.

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

An optional explicit freshness deadline is available through REST, CLI, Tauri,
and advanced desktop creation. The daemon's durable scheduler runs a bounded
deterministic consolidation batch on each tick. An elapsed deadline creates a
`decaying` successor even for previously durable knowledge—durability records
past usefulness, not perpetual truth. After a seven-day decay grace period, a
later tick creates a terminal `retracted` successor. The original value,
provenance, usefulness, and correction history remain immutable throughout;
concurrent owner edits win and are never overwritten by consolidation. Managed
Markdown and fresh context exclude expired or decaying claims immediately.

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

The CLI and desktop accept ordinary consumer strings as memory values and also
preserve objects, arrays, numbers, booleans, and null when valid JSON is
supplied. The desktop simple mode provides a small fact-name/value composer. Its
advanced mode exposes namespace, confidence, correction, reinforcement, decay,
retraction, immutable history, the discovered world graph, and managed-memory
Markdown. Tauri commands remain thin authenticated adapters over the same REST
contracts rather than a second memory implementation.

## Qualification

Fixture and integration tests cover stable project fingerprints, hierarchical
scope, instruction conflicts, manifest invalidation, protected context
compression, secret rejection, correction-driven durability, immutable snapshot
history, atomic claim versioning, concurrent/stale correction rejection, API
lifecycle behavior, and restart recovery.

Topology fixtures cover cross-language runtimes/dependencies, named Compose
service-to-database relationships, scalar and structured port declarations,
Kubernetes workloads/init containers/images/selector-matched Services/TLS
Ingress routes, Kubernetes Secret exclusion, unrelated YAML exclusion, OpenAPI
origins, deployment targets, stable graph identities, complete source
provenance, credential-bearing URL exclusion, malformed YAML, oversized
manifests, precise watch invalidation, compression priority, authenticated REST
assembly, and an actual bounded Evaluator turn with readable labels rather than
internal graph identifiers.

Desktop component and Playwright journeys additionally prove that a consumer
can record a plain-language fact in simple mode, reveal its provenance and
lifecycle controls in advanced mode, inspect the world model, reinforce the
claim, and read version history plus managed Markdown.

Runtime and scheduler integration fixtures use explicit clocks to prove bounded
expiration, durable-to-decaying behavior, seven-day retraction, daemon reopen,
repeat-tick idempotency, ordinary state-change events, and the absence of any
schedule requirement. CLI parsing rejects malformed freshness timestamps.

Complete owner-machine verification uses:

```powershell
.\scripts\verify.ps1
```

```bash
./scripts/verify.sh
```
