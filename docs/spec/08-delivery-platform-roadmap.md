# Delivery, Platform Strategy, and Roadmap

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 41. Backup and Restore

Backups must preserve the evolved cognitive organization.

Portable bundle example:

```text
backup.mcgb
```

Possible contents:

```text
manifest.json

configuration/
  settings
  providers
  permissions
  policies

organization/
  agents
  prompts
  topology
  model-routing
  revisions

scheduler/
  schedules
  watches

learning/
  model-performance
  agent-performance
  metacognitive-lessons

projects/
  definitions
  project-specific-config

optional/
  thread-history
  project-memory
  browser-profiles
```

Secrets should be:

- Excluded by default.
- Exportable only through encrypted backup.

---

---

## 51. Proposed Technology Stack

### Core

- Rust.
- Tokio.
- Serde.
- SQLite.
- tracing.
- reqwest/hyper or equivalent.
- Git integration libraries where appropriate.

### UI

- Tauri.
- Lightweight web frontend framework.
- Event-streamed state updates.

### Plugins

- Native Rust plugins for trusted modules.
- Wasmtime/WASI for sandboxed plugins.

### Data

- SQLite for canonical operational state.
- Filesystem for large blobs/artifacts.
- Vector index only where semantically useful.

---

---

## 52. Platform Strategy

### Linux

Full capability target.

### Windows

Full capability target with Windows-specific process/shell/filesystem behavior.

### macOS

Full capability target subject to OS permissions and security model.

### Android

Same Rust core where feasible, but platform capability limits are expected.

Android should advertise capabilities rather than forcing desktop behavior into an incompatible sandbox.

---

---

## 55. MVP Scope

The MVP should prove the architecture without implementing every advanced feature.

### MVP Must Have

- Rust daemon.
- Native/Tauri GUI shell.
- Projects.
- Threads.
- Scheduler.
- CLI.
- OpenAI-compatible custom providers.
- OpenRouter.
- Official OpenAI/Codex integration path.
- Model registry.
- Basic Executive.
- Coder agent.
- Conversation agent.
- Tool execution.
- Filesystem tools.
- Process execution.
- Git tools.
- Project instruction discovery.
- `AGENTS.md`.
- Claude rule discovery.
- Cursor rule discovery.
- SQLite state.
- Active context compression.
- Versioned managed memory file.
- Event bus.
- Basic MCG observation.
- Model switching.
- Organization definition.
- Config versioning.
- Backup/restore.
- OpenAI-compatible proxy.
- MCP client/server foundation.
- Metrics.
- Developer mode diagnostics.

### MVP Nice to Have

- Browser use.
- Persistent browser sessions.
- Computer use.
- Sites.
- Shadow model testing.
- Basic experiments.
- SSH remote execution.

---

---

## 56. Phase 2

- Full browser/computer use.
- Multi-model shadow tests.
- Shadow organizations.
- Automated experiment engine.
- Confidence calibration.
- Provenance graph.
- Project world model.
- Multi-host workflows.
- WASI plugin ecosystem.
- Automated organization restructuring.
- Organization inheritance.
- Advanced memory lifecycle.
- Scenario benchmark framework.
- Replay tooling.

---

---

## 57. Phase 3

- Sophisticated self-optimizing organizations.
- Cross-project organizational learning.
- Rich remote fleet management.
- Distributed execution beyond SSH.
- Organization genome marketplace/sharing.
- Advanced ensemble routing.
- Long-term autonomous operational workflows.
- Deep Sites monitoring and maintenance.
- Predictive MCG intervention.

---
