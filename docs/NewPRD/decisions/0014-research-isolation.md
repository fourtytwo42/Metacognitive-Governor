# ADR-014 — Research Isolation and Mechanism Retention

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Research drivers, analyses, developmental protocols, identity/continuity measurements, evolution, and speculative cognitive challengers remain outside the production dependency graph.

- Production crates do not import research implementation.
- Research invokes stable Root/Evaluator contracts or operates on authorized exported artifacts.
- Raw research measurements remain Evaluator-owned unless an explicit disclosure policy permits a bounded summary.
- A mechanism may enter normal production cognition only after demonstrating product rent under protected evaluation.
- A mechanism may remain research-only when it demonstrates scientific discriminability but not product value.
- A mechanism earning neither is deleted.
- Phase 0 gates recursive optimization; Phase 4 gates Evolution Lab; developmental claims require their protocol-specific gate.
- Negative results narrow claims or pivot the product rather than being hidden or optimized away.

## Verification

Cargo dependency checks prohibit production-to-research edges. Feature gates prevent premature optimizer/evolution activation. Tests verify research artifacts cannot grant authority or feed evaluator-only information into Runtime cognition.
