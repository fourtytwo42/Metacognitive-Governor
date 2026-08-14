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

## Bounded Model And Tool Turns

The agent turn engine consumes an already-running instance, active lease,
bounded context assembly, discovered route candidates, and an exact set of tool
definitions. It rechecks schema, identity, remaining token/cost/time budgets,
model context windows, and lease permissions before calling a provider.

The first provider call may use only deterministic retriable fallbacks allowed
by the role. Authentication never falls back. Once any provider responds, all
later calls in that tool loop stay pinned to the same provider and model.

Model function calls are decoded into `ToolRequestKind`, receive the task
identity and a timeout capped by the remaining agent lease, and execute only
through the supplied policy broker. The model receives the broker's bounded,
redacted `ToolOutcome` as a canonical Responses `function_call_output`. Unknown,
unexposed, unleased, duplicate, or malformed tool calls fail before the executor
is invoked. Model calls and total tool calls have hard iteration limits.

Runtime rechecks the supplied instance, lease, role, run, and organization
revision against current durable state before releasing the storage lock and
performing external work. On success, SQLite migration 6 commits the immutable
final turn result, provider/tool usage, assigned model, run metrics, completed
instance, completed lease, and all events in one transaction.

An approval-required tool pauses the run and instance while retaining the active
lease for explicit resume. Other provider, tool, policy, malformed-output, and
iteration failures fail the instance and lease with a stable reason code. Error
details from external providers are not copied into durable failure events, and
no unsuccessful turn creates fabricated model/tool usage or a success result.
Local-tool resume is explicit and context-bound: runtime verifies an approved
request, the one waiting instance and lease, the original organization role, the
current provider routes and tools, and an unchanged project-context fingerprint.
The model turn is replayed under the same lease; deterministic request identity
lets the broker consume the prior approval without executing the tool twice.
Persisted mid-turn transcripts/checkpoints remain required for external MCP tools
and to avoid a repeated inference call.

Consumer terminal settlement is evidence-driven. Completing the Evaluator model
turn is necessary but insufficient: supported build/test/static-analysis
conditions are rerun by the harness through governed typed processes. Only a
report tied to the completed Evaluator instance and covering every planned
condition may settle the task and run. Passed reports atomically complete both;
failed reports atomically fail both; inconclusive reports block the task without
inventing terminal success. In guided mode, the harness stores a condition-indexed
evaluation checkpoint and asks once for the exact discovered command batch. The
approval is bound to the run, attempt, conditions, and immutable project snapshot;
restart and resume therefore preserve evidence without granting broader process
authority or replaying completed checks.
Typed HTTP-status conditions use the same checkpoint and request identity. Their
capability contains only the condition's credential-free origin, while the immutable
request retains the exact URL and expected status. Response bytes are content-addressed;
the report records the observed status and artifact hash without copying the body into
evaluation history.
Typed file conditions retain only a normalized project-relative path and optional
minimum-size/digest constraints. Runtime resolves that path beneath the immutable
project authority, captures a bounded regular-file artifact through `fs.verify`,
and records its size and digest without placing file bytes in report text. Executable
custom conditions are limited to the canonical `COMMAND SUCCEEDS ` prefix and an
exact current discovery match; all other custom descriptions remain unsupported.

## Authenticated Control Surface

The loopback API can create and list safe consumer-default organizations, load
current or immutable revisions, plan a run against stable client work keys,
inspect the complete durable run view, and start, resume, or cancel a run. The
external transition boundary intentionally cannot claim completion; only runtime
settlement and evaluation may produce terminal success evidence. All routes use
the same persistent local bearer authentication as the rest of the daemon.

The CLI exposes these operations through `mcg organization` and `mcg run`.
Human output stays concise for creation and transitions, while inspection and
plan-file operations return the complete JSON evidence needed by scripts.
Objective reports are recorded through authenticated
`POST /api/v1/runs/{run}/evaluations` or `mcg run evaluate RUN REPORT.json`.
Reports are immutable, must cover every planned success condition, and must carry
objective evidence when the organization requires it. If organization policy or
the plan calls for independent evaluation, the report must identify an actual
Evaluator instance from the same run; a self-report or unrelated instance fails
before any evaluation event is stored.

SQLite migration 8 gives mutating stateless interfaces an immutable idempotency
boundary. `mcg_run_agent` records its canonical request hash and creates the task,
consumer organization revision, immutable plan, active task, and running run in
one transaction. Exact retries resolve to the same task/run; key reuse with
different input conflicts before any duplicate authority or event is created.
Provider/tool execution remains outside that transaction and retains the normal
dispatch, settlement, approval, and recovery rules.

Dispatch resolves project, work item, assigned role, dependencies, context,
server-owned tool schemas, provider routes, organization concurrency, lease
duration, and remaining budget again at execution time. SQLite writes the fresh
instance, its created-to-running transition, the active lease, and all three
events atomically. Duplicate active/completed work, incomplete dependencies, and
exhausted parallel/total-agent limits produce no new authority or event.

The runtime constructs every filesystem, process, Git, HTTP, system, and memory
executor before dispatch. Host capabilities are confined to the registered
project root, process credentials reject by default, network origins are empty by
default, and the secure owner constitution remains in force. The authenticated
`POST /api/v1/runs/{run}/work/{item}/execute` route and `mcg run execute` command
cannot supply role identities, lease limits, arbitrary tool schemas, or completion
claims.

Task replay upserts authoritative event-derived projections before removing
unexpected orphan projections. It does not delete referenced task rows during
normal replay, so organization/run evidence remains foreign-key valid while a
daemon restart moves interrupted work to `recovering`.
