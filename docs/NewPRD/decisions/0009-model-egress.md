# ADR-009 — Model Data Egress and Secret Handles

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

A remote model call is governed data egress and cost. Runtime proposes a bounded invocation; Root issues a `ModelInvocationPermit` only after validating exact context disclosure, destination, provider/model route, role, task, budget, policy, and expiry.

- Provider gateways receive the permit and opaque credential handle, not ambient Root secrets.
- Only the authorized adapter resolves a credential handle for the exact provider/destination and invocation.
- Secrets are not placed in argv, generic environment, manifests, events, errors, telemetry, or artifacts.
- Retry is bounded and tied to the same logical invocation/idempotency contract.
- Provider/model identity and observed fingerprint are persisted.
- Local providers still require an explicit route and disclosure boundary, though remote network egress policy may not apply.
- Codex app-server, authenticated remote transport, and subscription identity remain gateway implementations rather than authority owners.

## Verification

Fake providers cover permit mismatch, destination substitution, expired/replayed permits, retry, late response, streaming errors, and secret redaction. Real-provider tests are opt-in/nightly except for bounded release smoke qualification.
