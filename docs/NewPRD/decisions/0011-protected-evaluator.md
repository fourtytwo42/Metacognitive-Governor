# ADR-011 — Protected Evaluator Ownership and Disclosure

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Protected evaluation runs in a separately installed process with its own `evaluation.db`, artifact root, fixtures, objective manifests, portfolio policy, promotion policy versions, and event history.

- Only Evaluator opens/writes protected evaluation state.
- Only Root may request evaluation over a validated immutable ExperimentSpec.
- Baseline and candidate launch from an equivalent EvaluationSeed through pinned isolated Runtime worker binaries.
- Evaluator validates declared mutation, execution conditions, learning mode, environment/TCB fingerprint, and causal-analysis contract before accepting results.
- Raw fixtures, measurements, negative evidence, and protected policies remain unavailable to Runtime/candidate cognition.
- Evaluator records exposure, result-channel disclosure, retirement, taint/influence evidence, replication/clustering, and search breadth.
- Evaluator emits only signed/hash-bound authorized summaries/references.
- Research measurements are evaluator-only by default and never ordinary self-preservation/control signals.

## Verification

Tests prove store/fixture isolation, same-seed reproducibility, undeclared-divergence invalidation, learning-mode enforcement, exposure retirement, contamination propagation, result-channel accounting, pseudoreplication resistance, and signed-summary verification.
