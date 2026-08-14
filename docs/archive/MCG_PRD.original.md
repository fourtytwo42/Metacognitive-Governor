# Product Requirements Document (PRD)
# MCG — MetaCognitive Governor

## 1. Executive Summary

MCG is a cross-platform, GUI-first AI harness and cognitive runtime designed to operate as both:

1. A complete autonomous multi-agent system capable of working directly on the host operating system.
2. A provider/proxy layer that other AI harnesses can use to gain MCG's metacognitive governance, memory, project intelligence, model routing, and optimization capabilities.

MCG is designed around a central **MetaCognitive Governor** that does not normally perform user-facing work itself. Instead, it observes the operation of the broader agent organization, evaluates effectiveness, adjusts prompts, roles, models, routing, budgets, permissions, and organizational structure, and learns from historical outcomes.

The guiding architectural principle is:

> Agents think about the world.  
> The Executive thinks about the work.  
> The MCG thinks about the thinking and the organization doing the thinking.

MCG is intended to run close to bare metal, with a small native runtime, strong operating-system access, persistent scheduling, browser/computer use, multi-provider model support, project awareness, robust CLI integration, and a clean native desktop UI.

---

## 2. Product Vision

MCG should feel like an AI-native operating environment rather than a chatbot application.

The system should:

- Understand projects automatically when opened in a folder.
- Work directly with files, repositories, shells, browsers, and computers.
- Persist long-running work across threads and scheduled tasks.
- Adapt which models are used for which jobs.
- Continuously evaluate whether its current agent organization is effective.
- Experiment with alternate models, prompts, roles, and topologies.
- Learn from completed work and user corrections.
- Keep internal complexity mostly hidden from normal users.
- Expose enough metrics and organizational information that users understand what the system is doing.
- Offer deeper internals only through developer mode.
- Be usable interactively from a GUI, from a CLI like Codex CLI or Claude Code, through structured automation interfaces, through MCP, and as an OpenAI-compatible provider endpoint.
- Be portable across Linux, Windows, macOS, and Android, with platform-specific capability differences where necessary.

---

## 3. Core Design Principles

### 3.1 GUI First, Daemon First Architecturally

The native GUI is the primary human experience.

The actual product core is a persistent native daemon/service that owns state, scheduling, agent execution, memory, model routing, permissions, and tool execution.

The GUI, CLI, provider gateway, and MCP interfaces are clients of the same runtime.

### 3.2 Native and Close to Bare Metal

The preferred implementation stack is Rust for the core runtime.

Primary goals:

- Native compilation.
- Low runtime overhead.
- Fast startup.
- Direct OS integration.
- Strong async/concurrency support.
- Cross-platform builds.
- Minimal dependency on heavyweight interpreted runtimes.

A Tauri-based UI is preferred for a native-feeling desktop application while preserving a Rust backend.

### 3.3 Model-Agnostic Agents

An agent is not a model.

An agent consists of:

- Role.
- Prompt/policy.
- Memory.
- Tools.
- Permissions.
- Context.
- Performance history.
- Relationships to other agents.

The underlying model is an interchangeable cognitive engine selected by MCG.

### 3.4 Objective Outcomes Over Self-Reported Success

Whenever possible, task completion should be evaluated using executable success criteria:

- Tests.
- Health checks.
- Build results.
- Browser flows.
- API responses.
- File state.
- Git diff validation.
- Explicit user confirmation.
- Other externally observable signals.

### 3.5 Self-Improvement Must Be Measurable and Reversible

MCG may alter prompts, model assignments, routing, agent roles, and topology, but meaningful changes must be versioned, observable, testable, and reversible.

### 3.6 User Control Above MCG

The user/owner defines an immutable constitutional policy layer.

MCG may govern the system beneath that policy layer but may not remove, weaken, or bypass the owner's constitutional restrictions.

---

## 4. High-Level Architecture

```text
                    USER / CLIENTS
                         │
        ┌────────────────┼─────────────────┐
        │                │                 │
       GUI              CLI       External Harnesses
        │                │        OpenAI API / MCP
        └────────────────┼─────────────────┘
                         │
                         ▼
              ┌───────────────────────┐
              │      MCG DAEMON       │
              │     CONTROL PLANE     │
              └──────────┬────────────┘
                         │
             ┌───────────┼──────────────┐
             ▼           ▼              ▼
            MCG      Executive       Scheduler
             │           │              │
             └───────────┼──────────────┘
                         │
                ORGANIZATION GRAPH
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
   Conversation        Coder          Specialists
      Agent            Agent             Agents
        │                │                │
        └────────────────┼────────────────┘
                         │
                  TOOL BROKER
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
     Filesystem         Shell          Browser
        │                │                │
        ▼                ▼                ▼
       Git            Network        Computer Use
                         │
                         ▼
                    HOST SYSTEM
```

Surrounding services:

```text
Memory
Event Log
Project Intelligence
Provider Fabric
Metrics
Evaluation
Experiments
Backup / Restore
Configuration Versioning
```

---

## 5. Control Plane vs Execution Plane

### 5.1 Control Plane

The control plane decides how work should be performed.

Components include:

- MetaCognitive Governor.
- Executive/Manager.
- Agent organization graph.
- Model router.
- Scheduler.
- Policy engine.
- Memory retrieval.
- Experiment engine.
- Evaluators.
- Metrics.

### 5.2 Execution Plane

The execution plane interacts with the real world.

Components include:

- Filesystem.
- Process execution.
- Shell.
- Git.
- Network.
- Browser.
- Computer use.
- MCP tools.
- Plugins.
- Remote SSH execution.

Agents do not bypass the execution plane.

All real-world actions are performed through typed tool requests and governed by capability policy.

---

## 6. MetaCognitive Governor

### 6.1 Purpose

The MCG automates the job a human normally performs while managing and improving a multi-agent system.

It observes:

- Agent performance.
- Model performance.
- Task success/failure.
- Repeated failure patterns.
- Agent disagreements.
- Excessive decomposition.
- Runaway loops.
- Overconfidence.
- Underutilized or overutilized agents.
- Cost.
- Latency.
- Tool failures.
- User corrections.
- Organizational overhead.

It may modify:

- Agent prompts.
- Cognitive control layers.
- Model assignments.
- Model escalation rules.
- Routing.
- Reasoning budgets.
- Context allocation.
- Tool availability.
- Agent responsibilities.
- Agent topology.
- Scheduler policies.
- Evaluation strategies.

### 6.2 MCG Should Rarely Perform User-Facing Work

MCG is not the default conversation agent.

Its primary role is governance.

### 6.3 Intervention Levels

MCG should support graduated intervention:

```text
0 — Observe
1 — Prime
2 — Nudge
3 — Reframe
4 — Redirect
5 — Halt
6 — Restructure
```

### 6.4 Intervention Cost

MCG should consider the disruption cost of intervention.

It should not micromanage agents simply because it could choose a different strategy.

### 6.5 Fast, Medium, and Slow Loops

#### Fast Loop

Normal agent work:

- Seconds to minutes.
- Task execution.
- Tool calls.
- User interaction.

#### Medium Loop

Live metacognitive supervision:

- Minutes to hours.
- Detect loops.
- Detect disagreement.
- Escalate models.
- Adjust cognitive stance.
- Reallocate resources.

#### Slow Loop

Organizational learning:

- Hours to months.
- Analyze historical performance.
- Compare models.
- Compare prompts.
- Compare organizations.
- Modify agent topology.
- Promote/revert experiments.

---

## 7. Owner Constitution

The owner constitution is a policy layer above MCG.

Examples:

```text
Never expose stored secrets to model context.

Never modify protected system paths without approval.

Never delete backups.

Production deployment requires approval.

Maximum daily API spend is $X.

MCG cannot disable audit logging.

MCG cannot modify or weaken constitutional policy.
```

MCG may change lower-level policies but may not change this layer.

---

## 8. Organization Model

### 8.1 Organization Definition

The persistent organization is separate from runtime state.

Organization contains:

- Agents.
- Roles.
- Agent prompts.
- Cognitive policies.
- Relationships.
- Model policies.
- Tool capabilities.
- Escalation rules.
- Routing rules.
- Evaluation rules.

### 8.2 Runtime State

Runtime state includes:

- Active threads.
- Active tasks.
- Running agents.
- Current tool calls.
- Open browser sessions.
- Current computer-use sessions.
- Temporary context.
- Running processes.

### 8.3 Organization Graph

The organization should be represented as a graph.

Node examples:

- Executive.
- Coder.
- Conversation.
- Researcher.
- Tester.
- Tool worker.
- Temporary specialists.

Edge examples:

- Delegates to.
- Reviews.
- Escalates to.
- Reports to.
- Shares information with.
- Observed by.

MCG may modify the graph over time.

### 8.4 Persistent Roles, Ephemeral Agent Instances

Roles persist.

Execution instances are created as needed.

Example:

```text
Coder Role
- Memory
- Performance history
- Prompt policy
- Tool permissions
- Model policy

CoderInstance #1048
- Current task
- Current context
- Current model
- Temporary runtime state
```

---

## 9. Executive / Manager

The Executive manages work.

Responsibilities:

- Convert user goals into tasks.
- Decide whether decomposition is needed.
- Delegate work.
- Track dependencies.
- Manage task leases.
- Gather results.
- Determine when work is ready for evaluation.

The Executive should not perform the MCG's organizational optimization role.

---

## 10. Task Envelope

Every task should have a structured envelope.

Example:

```json
{
  "goal": "Fix failing authentication",
  "project": "foo",
  "constraints": [],
  "success_conditions": [
    "authentication works",
    "existing tests pass"
  ],
  "risk": "medium",
  "budget": {
    "tokens": 100000,
    "cost_usd": 3.00
  },
  "permissions": {},
  "provenance": "user",
  "state": "active"
}
```

The Task Envelope prevents goal drift and gives MCG objective information about work.

---

## 11. Task Leases

Agents should receive bounded leases rather than unbounded work.

Lease fields may include:

- Goal.
- Time budget.
- Token budget.
- Cost budget.
- Tool permissions.
- Reporting interval.
- Maximum retry count.
- Escalation conditions.

When a lease expires, the Executive/MCG decides whether to:

- Extend.
- Escalate.
- Reassign.
- Terminate.

---

## 12. Evaluation and Success Conditions

Whenever possible, success should be machine-verifiable.

Examples:

### Coding

- Build succeeds.
- Tests pass.
- No unexpected regressions.
- Static analysis passes.
- Requested behavior is observed.

### Website

- HTTP endpoint responds.
- Page renders.
- Browser workflow succeeds.
- Target element exists.
- Screenshot or semantic validation succeeds.

### System Administration

- Service is running.
- Port is listening.
- Configuration validates.
- Health endpoint passes.

### Research

- Claims have sources.
- Sources are recent enough.
- Conflicting evidence is surfaced.
- Output satisfies requested scope.

---

## 13. Confidence and Calibration

MCG should track confidence at multiple levels:

- Agent confidence.
- Claim confidence.
- Task confidence.
- System confidence.

The system should compare historical confidence with actual correctness.

Example:

If an agent repeatedly reports 95% confidence but is only correct 70% of the time at that level, MCG should reduce trust in that agent's confidence estimates.

---

## 14. Provenance

Important claims should retain provenance.

Example:

```text
Claim:
Postgres is listening on port 5432.

Source:
process.exec("ss -lnt")

Observed:
2026-08-13T23:00

Confidence:
1.0

Scope:
host-dev-01
```

This enables:

- Freshness detection.
- Conflict detection.
- Better memory.
- Better debugging.
- Better trust decisions.

---

## 15. Memory Architecture

Memory should not be a single vector store.

MCG memory should use multiple layers with different purposes.

### 15.1 Layer 1 — Active Context Window

The active model context is the immediate working memory.

As it approaches limits, the harness progressively compresses older context while preserving:

- Current goals.
- Important decisions.
- Unresolved questions.
- Constraints.
- Relevant evidence.
- Active plans.

Compression should be structured and iterative rather than simple truncation.

### 15.2 Layer 2 — Managed MEMORY.md

Each relevant scope may maintain a structured Markdown memory file.

Possible scopes:

- User/global.
- Project.
- Agent role.
- Long-running thread.

The memory file is:

- Human-readable.
- Versioned.
- Managed by the system.
- Suitable for important durable knowledge.
- Diffable.
- Exportable.
- Restorable.

Example:

```text
MEMORY.md
```

or internal equivalents not necessarily exposed in project folders.

A dedicated memory agent may exist, but MCG should be able to decide:

- Whether a memory specialist is warranted.
- Whether memory updates can be performed directly.
- When to create new memory.
- When to consolidate or revise memory.
- When an entry should be removed because it is stale or incorrect.

### 15.3 Layer 3 — Versioned Structured Memory

Durable knowledge should also be represented in structured storage with provenance and version history.

Examples:

- Project facts.
- User preferences.
- Historical decisions.
- Agent lessons.
- Model performance.
- MCG lessons.
- Task outcomes.

### 15.4 Layer 4 — Vector / Semantic Retrieval

Vector storage is useful for relatively stable or large bodies of material such as:

- Codebases.
- Documentation.
- Historical threads.
- Large project corpora.
- Reference material.

Vectors should not be treated as the canonical source for frequently changing operational state.

### 15.5 Memory Lifecycle

Memories should support:

```text
New
↓
Working
↓
Reinforced?
├── No → Decay
└── Yes → Durable
```

Retention should be based on usefulness, recurrence, correction history, and confidence.

Examples:

```text
"npm failed once"
→ likely decay

"this project uses pnpm"
→ durable

"user corrected this assumption three times"
→ strongly durable

"coder repeatedly fails using strategy X"
→ metacognitive memory
```

---

## 16. Project Intelligence

Opening or invoking MCG inside a project folder should immediately provide structured project understanding.

### 16.1 Project Discovery

MCG should detect:

- Repository root.
- Git state.
- Languages.
- Build system.
- Test system.
- Package managers.
- Services.
- Docker configuration.
- CI configuration.
- Important directories.
- Recent commits.
- Project instructions.
- Known commands.
- Existing MCG project memory.

### 16.2 Instruction Standards

MCG should support existing ecosystem conventions rather than requiring proprietary conversion.

Initial standards:

- `AGENTS.md`
- `AGENTS.override.md`
- `CLAUDE.md`
- `CLAUDE.local.md`
- `.claude/rules/`
- `.cursor/rules/*.mdc`
- `README.md`
- `CONTRIBUTING.md`

The resolver should support hierarchical scope and applicability.

### 16.3 Conflict Detection

MCG should not blindly concatenate contradictory instruction files.

Example:

```text
AGENTS.md → use pnpm
CLAUDE.md → use npm
.cursor rule → use bun
```

MCG should:

- Detect conflict.
- Determine scope.
- Apply known precedence semantics.
- Prefer more specific applicable rules where appropriate.
- Surface unresolved ambiguity.

### 16.4 Project World Model

Project understanding should go beyond file indexing.

Example:

```text
Frontend
   │ calls
   ▼
API
   │ writes
   ▼
Postgres

GitHub main
   │ deploys
   ▼
Production
```

The world model may include:

- Services.
- Databases.
- Domains.
- APIs.
- Deployment targets.
- Dependencies.
- Repositories.
- Runtime environments.
- Commands.
- Relationships.

---

## 17. Model Fabric

MCG should support multiple model providers and local/custom endpoints.

### 17.1 OpenAI Codex / ChatGPT Authentication

Support official Codex/ChatGPT OAuth-based connection where available through supported OpenAI integration mechanisms.

Do not depend on reverse-engineering local credential files.

Supported authentication modes should include:

- ChatGPT sign-in.
- Device-code sign-in where supported.
- OpenAI API key.

### 17.2 OpenRouter

OpenRouter is a first-class provider.

MCG should support:

- Model listing.
- Provider/model selection.
- Streaming.
- Tool calls.
- Usage/cost tracking.
- Capability metadata.

### 17.3 Custom Endpoints

Support arbitrary OpenAI-compatible endpoints.

Examples:

- vLLM.
- llama.cpp server.
- LM Studio.
- Ollama-compatible adapters.
- Self-hosted inference servers.
- Third-party OpenAI-compatible APIs.

### 17.4 Canonical Internal Model Interface

All providers translate into a canonical internal request/response representation.

Canonical request should support:

- Messages.
- Instructions.
- Tools.
- Tool choice.
- Structured output.
- Reasoning configuration.
- Sampling parameters.
- Attachments.
- Metadata.
- Streaming.

### 17.5 Model Registry

MCG maintains a dynamic capability/performance profile for each model.

Example metrics:

- Coding quality.
- Debugging.
- Architecture.
- Research.
- Conversation.
- Tool use.
- Context handling.
- Latency.
- Cost.
- Tokens.
- Historical success by task type.

Model selection should be empirical and context-dependent.

---

## 18. Model Escalation

MCG should be able to dynamically escalate or downgrade the model powering an agent.

Example:

```text
Tier 0 — tiny/local model
Tier 1 — efficient model
Tier 2 — strong model
Tier 3 — frontier model
Tier 4 — ensemble
```

A Coder role may remain the same agent while its underlying model changes.

---

## 19. Shadow Model Testing

MCG should support shadow execution.

A candidate model receives a copy of a task without affecting the real result.

Example:

```text
Real task
  ├── Active model → used result
  └── Shadow model → evaluation only
```

MCG compares:

- Correctness.
- Tests.
- Cost.
- Latency.
- Tool usage.
- Retries.

This enables continuous model benchmarking using real work.

---

## 20. Shadow Organization Testing

The same concept should apply to organizational structures.

Example:

```text
Organization A
Executive → Coder

Organization B
Executive → Researcher → Coder
```

MCG may compare alternate organizations on duplicated benchmark or shadow tasks.

The system can then determine which topology performs better for particular task classes.

---

## 21. Experiment Engine

MCG should use a scientific improvement loop:

```text
Observe
↓
Detect Pattern
↓
Form Hypothesis
↓
Propose Intervention
↓
Run Experiment
↓
Measure
↓
Compare
↓
Keep or Revert
↓
Store Lesson
```

Changes should produce measurable outcome data.

---

## 22. Prospective Simulation

Before major organizational changes, MCG should perform a lightweight prospective analysis.

Example:

```text
Proposed:
Remove Researcher.

Likely benefits:
- lower latency
- fewer model calls

Likely risks:
- lower source verification
- increased coder context pollution

Historical evidence:
Researcher improved dependency tasks 14%.

Decision:
Do not remove.
```

---

## 23. Cost Model

Cost is broader than money.

MCG should consider:

- API dollars.
- Tokens.
- Time.
- CPU.
- GPU.
- RAM.
- Energy.
- User attention.
- Risk.
- Tool overhead.
- Organizational overhead.

MCG should attempt to achieve reliable outcomes with the minimum cognitive machinery required.

---

## 24. Avoiding Multi-Agent Overhead

MCG should actively detect when multi-agent decomposition is unnecessary.

Example:

```text
User: rename foo to bar

Bad:
Executive → Planner → Researcher → Coder → Reviewer

Better:
Coder → Done
```

One explicit metric should track organizational overhead.

---

## 25. Scheduler, Heartbeat, and Wake System

MCG must operate persistently.

### 25.1 Trigger Types

#### Event Triggers

Examples:

- Agent failure.
- Tool failure.
- Low confidence.
- User correction.
- Conflicting conclusions.
- Task stall.

#### Heartbeat

Periodic system-health inspection.

The heartbeat itself should be cheap and may use a lightweight evaluator before waking the full MCG.

#### Cron

Scheduled deeper reviews.

Examples:

- Periodic stalled-task review.
- Hourly performance aggregation.
- Nightly memory consolidation.
- Model-performance analysis.
- Organizational review.

#### Wake Conditions

Any subsystem may request MCG attention.

Example:

```text
WAKE_MCG(
  severity="high",
  reason="two agents reached incompatible conclusions"
)
```

### 25.2 Adaptive Heartbeat

Heartbeat frequency may change based on activity.

Idle systems should wake less frequently.

Busy, risky, or complex workflows may wake more frequently.

---

## 26. Tool Runtime

Tools should be typed and first-class.

Initial primitives:

```text
fs.read
fs.write
fs.patch
fs.list
fs.mkdir
fs.move
fs.copy
fs.delete

process.exec
process.spawn
process.status
process.stdin
process.kill

shell.exec

http.request
http.download

system.info
system.env
system.processes

git.status
git.diff
git.commit
git.checkout

scheduler.create
scheduler.remove
scheduler.list

memory.search
memory.read
memory.write
```

### 26.1 Structured Process Execution

Prefer:

```text
executable = "git"
argv = ["status", "--short"]
cwd = "/repo"
```

over blindly executing shell strings.

An explicit shell tool remains available for cases where shell semantics are required.

---

## 27. Capability and Permission System

Every agent receives explicit capabilities.

Example:

```text
Coder:
  filesystem:
    read:
      - /projects/**
    write:
      - /projects/**
    deny:
      - ~/.ssh/**
      - ~/.config/mcg/secrets/**

  process:
    allow:
      - git
      - cargo
      - npm
      - node
      - python

  destructive:
    require_approval: true
```

Permissions are enforced by the harness, not by model prompt compliance.

---

## 28. Browser Use

Browser sessions are persistent resources rather than one-off calls.

A browser session should track:

- Profile.
- Owner.
- Current task.
- Tabs.
- URLs.
- Authentication state references.
- Last activity.
- Session status.

Agents may:

- Acquire.
- Operate.
- Release.
- Hand off.

---

## 29. Computer Use

Computer-use sessions should represent a device or desktop environment.

Possible capability fields:

```text
screen.read
mouse
keyboard
clipboard
window.list
process.inspect
```

Computer-use capabilities are governed by the same policy engine as shell/filesystem tools.

---

## 30. Multi-Host Operation

Primary multi-host capability should come from the tool runtime's ability to use SSH and remote execution.

MCG may:

- SSH into another host.
- Inspect system capabilities.
- Execute commands.
- Transfer files.
- Run remote agents/tasks.
- Route work to machines with needed capabilities.

Example use cases:

- Build on macOS from a Linux controller.
- Use a GPU server for local inference.
- Test on Windows.
- Interact with a remote Linux server.

A separate distributed-agent protocol may be added later if SSH-based operation becomes insufficient.

---

## 31. Provider Proxy

MCG must be able to act as an OpenAI-compatible provider.

This allows other harnesses to gain MCG's:

- Memory.
- Project context.
- Model routing.
- Meta governance.
- Tool policy.
- Organizational intelligence.

Initial endpoints:

```text
GET  /v1/models
POST /v1/chat/completions
POST /v1/responses
```

Must support:

- Streaming.
- Tool/function calls.
- Structured output where possible.
- Usage accounting.
- Model aliases.

Example aliases:

```text
mcg:auto
mcg:fast
mcg:best
mcg:coder
mcg:reasoner
mcg:local
mcg:cheap
mcg:ensemble
```

---

## 32. Tool Call Proxy Semantics

Tool namespaces must remain distinct.

Example:

```text
caller::*
host::*
mcg::*
```

### Caller-Owned Tools

Returned to the external harness for execution.

### Host-Owned Tools

Executed by MCG's local execution plane.

### MCG-Owned Tools

Internal cognitive/runtime operations.

MCG must not accidentally hijack caller-owned tool calls.

---

## 33. MCP

MCG should operate as both:

- MCP client.
- MCP server.

### MCP Client

Agents may consume external MCP tools.

### MCP Server

External systems may access MCG capabilities such as:

```text
mcg_project_context
mcg_run_agent
mcg_query_project
mcg_memory_search
mcg_system_status
```

---

## 34. Plugin Architecture

Support two plugin classes.

### Trusted Native Plugins

- Rust/native.
- Highest performance.
- Direct integration.

### Sandboxed Plugins

- WASM/WASI.
- Capability-restricted.
- Suitable for third-party/community plugins.

---

## 35. Event Bus

The event bus is the nervous system.

Events may include:

```text
UserMessageReceived
TaskCreated
TaskStarted
AgentStarted
AgentOutput
ToolRequested
ToolStarted
ToolCompleted
ToolFailed
FileModified
CommandExecuted
ModelCallStarted
ModelCallCompleted
AgentBlocked
AgentConfidenceChanged
UserCorrectedAgent
TaskCompleted
Heartbeat
CronTriggered
MCGIntervention
OrganizationChanged
ExperimentStarted
ExperimentCompleted
```

MCG, memory, metrics, UI, and audit systems subscribe to relevant events.

---

## 36. Deterministic Replay and Audit

Every task should preserve enough information to reconstruct what happened.

Store:

- Task ID.
- Organization version.
- Agent role/version.
- Model/provider.
- Prompt version.
- Inputs.
- Tool requests.
- Tool outputs.
- File hashes.
- Events.
- MCG decisions.
- Result.
- Evaluation.

Replay should be as deterministic as external dependencies permit.

---

## 37. Scenario Test Framework

MCG must ship with an internal scenario framework.

Possible categories:

```text
coding/
research/
browser/
sysadmin/
conversation/
tool_use/
ambiguity/
adversarial/
```

Each scenario defines expected outcomes.

Candidate organization versions can be benchmarked before promotion.

---

## 38. Organizational Genome

The complete cognitive organization should serialize into a portable definition.

Example:

```yaml
organization:
  version: 44

agents:
  executive:
    role: executive
    model_policy: balanced

  coder:
    role: software_engineer
    model_policy: coding_best

relationships:
  - executive -> coder: delegate
  - coder -> tester: review

policies:
  ...

evaluation:
  ...
```

The organization genome may be:

- Exported.
- Imported.
- Cloned.
- Forked.
- Compared.
- Versioned.
- Shared.

---

## 39. Organization Inheritance

Organizations may inherit from base profiles.

Example:

```text
Base Organization
├── Software Engineering
├── Research
├── Sysadmin
└── Personal Assistant
```

Project-specific organizations may extend global profiles.

---

## 40. Configuration Versioning

Every meaningful organizational change creates a revision.

Example:

```text
v37 Current
- Added Research agent
- Changed Coder model
- Reduced Manager autonomy

v36 Previous stable

v35 Experimental
- Automatically reverted
```

Users should be able to:

- Compare revisions.
- Restore revisions.
- Export revisions.
- Clone revisions.

MCG should automatically revert experimental configurations when evidence shows regression.

---

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

## 44. Threads

Threads represent conversational and task continuity.

Threads should show user-visible progress while avoiding noisy raw agent traces.

Detailed activity can be expanded when desired.

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

## 49. Remote Connectivity

GUI and CLI should be able to connect to a local or remote MCG daemon.

Examples:

```bash
mcg --host localhost
mcg --host devbox
mcg --host https://mcg.example
```

This allows a local workstation to control an MCG runtime on a GPU server or remote machine.

---

## 50. Security

Security should be enforced structurally.

Core requirements:

- Typed tool calls.
- Explicit capabilities.
- Owner constitution.
- Secret isolation.
- Audit log.
- Protected paths.
- Approval gates.
- Sandboxed third-party plugins.
- Versioned config.
- Reversible organizational changes.
- Optional remote-auth layer.
- Encrypted sensitive backup data.

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

## 58. Non-Goals

MCG should not initially attempt to:

- Reimplement an entire operating system.
- Replace every IDE.
- Invent proprietary project instruction formats where existing standards work.
- Treat every task as multi-agent.
- Store all memory in vectors.
- Allow MCG to bypass owner policy.
- Expose internal metacognitive details to normal users by default.
- Depend on one model vendor.

---

## 59. Success Criteria

MCG is successful when it can:

1. Be opened on a code/project folder and immediately understand relevant project instructions and structure.
2. Perform useful real-world work through files, shell, git, browser, and eventually computer use.
3. Maintain continuity across threads and scheduled work.
4. Dynamically select and escalate models based on observed effectiveness.
5. Learn which model works best for which type of work.
6. Detect when agents or organizational structures are inefficient.
7. Modify prompts, models, routing, and eventually topology based on evidence.
8. Revert changes that reduce performance.
9. Preserve the evolved organization through versioning and backup.
10. Be used through a clean native GUI.
11. Be invoked from the terminal like Codex CLI or Claude Code.
12. Be called programmatically through structured CLI/API interfaces.
13. Act as an OpenAI-compatible provider for other harnesses.
14. Act as an MCP client and server.
15. Hide unnecessary internal complexity from ordinary users while exposing deep diagnostics in developer mode.
16. Achieve reliable outcomes using the minimum cognitive machinery necessary.

---

## 60. Product Identity

**Name:** MCG  
**Meaning:** MetaCognitive Governor

MCG governs the cognitive organization rather than directly controlling every action.

The product should communicate a simple external experience:

> One AI system that understands your projects, remembers what matters, works directly on your machines, chooses the right intelligence for the job, and gets better at organizing itself over time.

Internally, MCG is a versioned, measurable, experimentally improvable cognitive organization.
