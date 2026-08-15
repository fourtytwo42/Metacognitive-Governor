# Measured Supervision and Experiments

M9 adds learning without allowing unmeasured self-modification. Supervision may
observe and recommend freely, but a candidate organization cannot become active
unless immutable benchmark evidence and an explicit promotion decision commit
together. Every promotion retains a known-good parent that can be restored.

## Loop boundaries

- The fast loop records task, tool, confidence, cost, latency, and correction
  observations. It may recommend intervention levels 0 through 6 but does not
  rewrite an organization.
- The medium loop groups durable signals into failure patterns, detects loops or
  disagreement, and proposes bounded routing, budget, stance, or escalation
  changes.
- The slow loop creates an immutable organization revision, prospective
  analysis, shadow/scenario experiment, paired benchmarks, and a promotion or
  rejection decision.

An observation always references a task, run, or organization revision and at
least one durable evidence record. Confidence and severity use basis points so
all interfaces serialize the same bounded values.

## Shadow isolation

Shadow execution receives copied task input and a candidate organization or
model route. Its authority contract has one representable value:
`evaluation_only`. Shadow work cannot hold the production task lease, settle the
production task, publish a Site, approve a tool, mutate a project, or write the
active result. It may retain content-addressed output for independent evaluation.
Runtime dispatch treats every run pinned to a non-active organization revision
as evaluation-only: an explicit host/MCP tool request fails closed, and an empty
request resolves to zero tools rather than the role's normal defaults.

Shadow mode remains opt-in for normal consumers and enabled in development and
scenario environments. A shadow result can influence production only after it
becomes benchmark evidence for a separately promoted revision.

`MCG_SHADOW_EXPERIMENTS` is the explicit headless opt-in (`1`, `true`, `yes`,
or `on`). Debug/development builds default it on; release builds default it off.
The authenticated runtime status reports the effective setting. When enabled,
each scheduler tick turns eligible slow-loop regression evidence into at most
one bounded inactive candidate per organization. Stable candidate/experiment
identities and migration 22 outcomes resume a candidate or draft experiment
that committed immediately before a crash. The active organization head never
changes during this automation; the generated shadow experiment stops in
`ready` until evaluation-only execution begins. Ready experiments then receive
a stable run identity derived from the experiment, a fresh plan rebuilt against
the inactive candidate, and no host authority. A crash after run creation or
the `running` transition resumes that same candidate run rather than creating a
second provider-work identity. When a discovered provider route exists, the
scheduler executes dependency-ready items sequentially with an empty tool set.
It never automatically retries an item after any non-completed durable lease;
successful complete-plan output is retained in the local content-addressed
artifact store before the immutable evaluation-only receipt commits. Missing
routes are a no-spend no-op.

Completed automatic receipts are assessed against the exact evaluation that
produced their source supervision observation. The candidate's completed
Evaluator instance is required. Until a separate isolated project sandbox can
re-execute production objectives, zero-tool output is recorded honestly as an
inconclusive evaluation rather than treating model self-report as success. The
paired baseline/candidate reports are themselves content-addressed, the
benchmark receives a stable identity, and the configured benchmark, regression,
and confidence gates select promotion or rejection. Current zero-tool automatic
shadows therefore reject while retaining evidence; they never weaken the gate
to manufacture a promotion.

## Durable state

SQLite migrations 20-22 retain:

- immutable supervision observations;
- one mutable experiment projection whose definition hash and revision pairing
  never change;
- immutable shadow receipts and paired benchmark results;
- immutable evidence-linked decisions;
- the existing immutable organization revisions and one current-revision
  pointer.
- exactly-once evaluation-supervision markers and immutable automatic candidate
  outcomes, including stale-baseline dispositions.

Experiment states are `draft`, `ready`, `running`, `awaiting_decision`,
`promoted`, `rejected`, and `reverted`. A candidate cannot jump directly from
running to promoted. Promotion requires the configured number of distinct
benchmarks, no more than the permitted regressions, the requested confidence,
the expected current baseline, and candidate/baseline membership in the same
organization.
Automatic promotion additionally re-loads every referenced run and evaluation
and requires each benchmark arm's revision, run, outcome, confidence, and
metrics to match that durable evidence exactly. A syntactically valid benchmark
cannot invent a passing arm for automatic promotion.

## Atomic promotion and reversion

Promotion is one SQLite transaction:

1. Re-read the experiment and current organization revision.
2. Validate all referenced immutable benchmarks and prospective analysis.
3. Insert the immutable decision.
4. Change the organization current-revision pointer to the candidate.
5. Change the experiment projection to `promoted`.

Any mismatch rolls back every write. Reversion uses the same boundary in the
opposite direction, requires a promotion decision in history, restores the
recorded baseline, and moves the experiment to `reverted`. Startup never
auto-promotes interrupted work. A measured regression may request automatic
reversion, but the same evidence and transaction invariants still apply.
Automatic assessment also recovers the crash window after a benchmark commits
but before `awaiting_decision`: the next scheduler tick advances and decides the
same stable evidence without recreating evaluation, benchmark, or decision IDs.
Reversion decisions carry explicit trigger evaluation IDs. Storage verifies
each trigger belongs to a run pinned to the promoted candidate, occurred after
the promotion decision, and reports failure or regression. Scheduler ticks find
that evidence, create one stable reversion decision, and restore the exact
baseline atomically; replay cannot revert twice.

## Interfaces

All callers use the same runtime and SQLite invariants:

- authenticated REST exposes candidate and observation recording, experiment
  creation/list/detail, compare-and-set transitions, shadow receipts, paired
  benchmarks, and decisions;
- CLI `mcg experiment` accepts complete versioned JSON for evidence-bearing
  writes and provides list/show/transition commands for automation;
- MCP exposes `mcg_experiments_inspect` and `mcg_experiments_mutate`; the latter
  requires an explicit action plus its complete versioned payload and cannot
  bypass evidence or transition checks;
- Tauri provides typed bindings for the same routes; and
- the desktop Improvement workspace summarizes observations, protected
  baselines, experiment state, and evidence counts in simple mode, with
  versioned JSON controls and legal state advances in advanced mode.

Caller-owned candidate, observation, experiment, shadow-run, benchmark, and
decision identities make exact lost-response retries observation-only. Every
newly committed experiment event is also published to the existing resumable
SSE stream.

## Exit evidence

M9 is not complete until automated scenarios prove that:

- seeded failure and calibration regressions produce durable observations;
- shadow output cannot change the real task or create real side effects;
- insufficient, conflicting, duplicated, or regressing benchmarks cannot
  promote;
- every successful promotion links to immutable measurements and prospective
  analysis;
- a seeded post-promotion regression restores the exact prior revision; and
- all histories and decisions remain consistent after daemon restart.
