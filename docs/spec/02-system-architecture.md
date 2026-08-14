# System Architecture and Organization

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

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
