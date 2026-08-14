# Governor, Evaluation, and Learning

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

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
