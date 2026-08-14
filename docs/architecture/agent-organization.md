# Agent Organization And Run Evidence

M5 separates the versioned cognitive organization from ephemeral execution. A
role persists with prompt, model, tool, and memory policy; an agent instance is a
short-lived assignment to one run and one selected role.

## Minimal Selection

Planning accepts structured intent. Text keywords do not grant capabilities or
authorize tools. The selector deterministically chooses one canonical role for
each declared work capability. Atomic work is dispatched directly to that role.
The Executive is added for decomposition or dependency coordination, and an
Evaluator is added only when task intent or organization policy requires an
independent evaluation.

Imported plans are revalidated. Unknown, duplicate, unused, or excessive roles,
dependency cycles, missing policy-required evaluation, invalid overhead, and
incomplete work items fail closed. Child token, cost, and duration budgets are
apportioned within the parent task envelope.

## Durable State

SQLite migration 5 stores:

- a current organization projection plus immutable numbered revisions;
- current agent-run, instance, and lease projections;
- one immutable work plan per run;
- immutable provider/model, tool, and coordination usage records;
- immutable evidence-backed evaluation reports.

Every creation or state transition is committed with a versioned global event.
Provider usage names the configured connection and model but never stores a
credential. Tool usage refers to an existing typed execution identity.
Evaluations must cover the plan's success conditions and comply with the
organization's objective-evidence policy.

## Restart Recovery

At daemon startup, an interrupted `running` or `waiting` run becomes
`recovering`. Its active instances and agent leases become `expired`. Each
change appends a task event in the same SQLite transaction, so no interrupted
agent is silently treated as successful and clients can replay the exact
recovery history through the existing event stream.

Run resumption or reassignment must create fresh instances and leases. Old
plans, usage, evaluations, and organization revisions remain immutable evidence.
