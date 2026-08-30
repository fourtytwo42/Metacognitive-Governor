# ADR-012 — Promotion, Rebase, Monitoring, and Semantic Rollback

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Evaluator owns protected judgment; Root owns activation. A candidate cannot promote itself.

- Promotion decisions are `PROMOTE`, `REJECT`, `CONTINUE`, `INCONCLUSIVE`, or `INVALID_EXPERIMENT`.
- Decisions bind champion/candidate configuration hashes, seed/spec, declared mutation, metrics, uncertainty, guardrails, evaluation exposure/taint, mechanically counted search breadth, and policy version.
- A candidate whose champion changed must rebase and re-evaluate before promotion.
- Interaction tests are required before assuming independently promoted changes compose.
- Root verifies the evaluator signature/hash and current expected versions in one promotion transaction.
- Each promotion begins a promotion epoch with causal provenance and monitoring conditions.
- Regression triggers retention, rollback, deletion, or case reopening according to policy.
- Semantic rollback removes or revalidates affected cognitive descendants; binary/config rollback alone is not claimed complete.
- Protected Root/Evaluator source patches require normalized human review and a separately signed release/install path.

## Verification

Tests cover forged/stale decisions, candidate self-promotion, changed champion, interaction failure, monitoring regression, synthetic and deep-lineage rollback, unsupported residue, and deletion challengers.
