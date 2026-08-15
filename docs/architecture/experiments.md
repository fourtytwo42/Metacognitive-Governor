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

Shadow mode remains opt-in for normal consumers and enabled in development and
scenario environments. A shadow result can influence production only after it
becomes benchmark evidence for a separately promoted revision.

## Durable state

The storage boundary will retain:

- immutable supervision observations;
- one mutable experiment projection whose definition hash and revision pairing
  never change;
- immutable shadow receipts and paired benchmark results;
- immutable evidence-linked decisions;
- the existing immutable organization revisions and one current-revision
  pointer.

Experiment states are `draft`, `ready`, `running`, `awaiting_decision`,
`promoted`, `rejected`, and `reverted`. A candidate cannot jump directly from
running to promoted. Promotion requires the configured number of distinct
benchmarks, no more than the permitted regressions, the requested confidence,
the expected current baseline, and candidate/baseline membership in the same
organization.

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
