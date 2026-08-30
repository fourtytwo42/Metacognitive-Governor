# ADR-006 — Workflow Attempts, Queues, and Generation-Fenced Leases

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Root owns a single legal workflow state machine and all durable queues. A `TaskContract` may have multiple explicit `WorkflowAttempt` records, but retry never rewinds or mutates a prior attempt.

- Every lease has an owner, role, resource scope, expiry, and monotonically increasing generation.
- Only the currently persisted generation may commit a result.
- Reassignment increments generation before new work is dispatched.
- Heartbeats may extend time but cannot expand authority or scope.
- Terminal states do not transition back to ordinary active states.
- Cancellation is durable, revokes future commit authority, propagates to children, and retains completed effects/evidence.
- Root restart scanning reconstructs queues from durable state, expires stale generations, and resumes only work whose wait condition and policy are satisfied.
- UI disconnect/reconnect has no effect on backend truth.
- Queue capacity and retry ceilings are bounded and visible.

## Verification

Generated state-machine tests cover legal/illegal transitions, retry ancestry, concurrent claims, stale commits, heartbeat races, cancellation, and restart in every nonterminal state. Real-process E2E kills lease owners and delivers delayed stale results after reassignment.
