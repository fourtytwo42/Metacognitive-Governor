# ADR-007 — Actions, Outcomes, Reconciliation, and Compensation

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Every consequential effect follows one Root-owned causal chain:

```text
proposal
-> durable ActionEnvelope intent
-> authorization and resource reservation
-> ActionExecution dispatch
-> observation/receipt
-> Root classification and commit
-> reconciliation when uncertain
```

The authorization transaction binds the exact normalized operation hash, authority, scope, budget, expiry, and expected state. The executor cannot substitute another target or operation.

Effect outcome is one of:

- `NO_EFFECT`
- `EFFECT_CONFIRMED`
- `OUTCOME_UNKNOWN`

Timeout, cancellation after dispatch, process death, IPC loss, or a missing receipt cannot be interpreted as `NO_EFFECT`. Consequential unknown effects enter a typed `ReconciliationJob`; they are not blindly retried.

Reversal or compensation is a new authorized Action with its own evidence. History is never erased. Irreversible operations require explicit classification, stronger authority, and clear approval presentation.

## Verification

Crash injection occurs before/after intent, authorization, launch, effect, receipt, commit, and UI delivery. Filesystem E2E proves an effect that succeeds before Root crashes is reconciled and committed exactly once after restart.
