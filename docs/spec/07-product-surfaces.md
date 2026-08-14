# GUI, Projects, Threads, Scheduler, Sites, and CLI

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 42. Native GUI

The GUI is the primary product surface.

Primary navigation:

```text
Projects
Threads
Scheduler
Browser
Computer
Sites
Settings
```

Memory and MCG are not top-level normal-user pages.

### 42.1 Normal Mode

Shows:

- Work status.
- Task progress.
- Agent count.
- Model usage.
- Tool usage.
- Metrics.
- Organization summary.
- Configuration changes in user-friendly language.

Example:

```text
Working on authentication issue

✓ Inspected project instructions
✓ Analyzed relevant files
● Testing implementation
○ Preparing final changes

Agents active: 3
Models: Local + Codex
Tools running: 1
```

### 42.2 Organization View

Users may inspect the system's current AI organization without seeing low-level MCG internals.

Example:

```text
Executive
├── Conversation
├── Coder
├── Research
└── Tool Worker
```

Agent detail may include:

- Purpose.
- Current model policy.
- Fallback.
- Tools.
- Performance.
- Why this role exists.
- Recent high-level changes.

### 42.3 Metrics

Possible metrics:

- Task count.
- Success rate.
- Local model usage.
- API spend.
- Average task time.
- Tool failure rate.
- Retry rate.
- Model escalation rate.
- Organizational overhead.

### 42.4 Developer Mode

Developer mode reveals internal diagnostics such as:

- Raw MCG interventions.
- Memory inspector.
- Event stream.
- Prompt versions.
- Context assembly.
- Belief/provenance state.
- Model calls.
- Raw tool calls.
- Experiment details.
- Scheduler internals.

---

---

## 43. Projects

Projects are folder/repository-centric workspaces.

Project page may contain:

```text
Overview
Threads
Tasks
Files
Git
Agents
Rules
Sites
Scheduler
```

Project intelligence should be loaded automatically from the folder.

---

---

## 44. Threads

Threads represent conversational and task continuity.

Threads should show user-visible progress while avoiding noisy raw agent traces.

Detailed activity can be expanded when desired.

---

---

## 45. Scheduler UI

The scheduler should expose:

- Running tasks.
- Upcoming jobs.
- Watches.
- Recurring tasks.
- Heartbeats.
- Execution history.

Example:

```text
NOW
● Test deployment
● Browser monitor

UPCOMING
10:00 PM Check site health
12:00 AM Project consolidation

WATCHES
● GitHub response
● Site health change

RECURRING
Every 5m System heartbeat
Daily Performance review
```

---

---

## 46. Sites

A Site is a persistent web property or deployed application known to MCG.

Possible fields:

- URL.
- Repository.
- Hosting provider.
- Deployment command.
- Health endpoint.
- Related project.
- Credential reference.
- Logs.
- Analytics.
- Scheduled checks.
- Known issues.

---

---

## 47. CLI

The CLI is first-class.

Interactive usage:

```bash
mcg
```

One-shot:

```bash
mcg "fix the failing tests"
```

Project selection:

```bash
mcg -C ~/projects/foo "review this repo"
```

Piped usage:

```bash
git diff | mcg "review this diff"
```

Structured:

```bash
mcg run --json "fix issue #419"
```

Streaming:

```bash
mcg run --stream-json "fix issue #419"
```

The CLI should feel comparable to Codex CLI or Claude Code.

---

---

## 48. Machine/Automation Interface

Structured CLI and API responses should support machine callers.

Example:

```json
{
  "task_id": "task_9d81",
  "status": "completed",
  "result": "...",
  "files_changed": [
    "src/auth.rs"
  ],
  "tests": {
    "passed": 42,
    "failed": 0
  }
}
```

Possible consumers:

- Shell scripts.
- CI/CD.
- IDE plugins.
- Other AI agents.
- Automation systems.

---

---

## 53. Initial Product Modes

### Autonomous Mode

MCG acts as the complete harness.

### Provider Mode

Other harnesses call MCG through OpenAI-compatible APIs.

### MCP Mode

Other harnesses call MCG's tools/context through MCP.

### CLI Mode

Interactive terminal-based operation.

### Headless Mode

Daemon runs without GUI for servers.

---

---

## 54. MCG Autonomy Levels

Users should control how much MCG may modify the system.

Suggested levels:

```text
1 — Observe only
2 — Prompt tuning
3 — Model routing
4 — Agent configuration
5 — Organization restructuring
```

Owner constitution remains authoritative at all levels.

---
