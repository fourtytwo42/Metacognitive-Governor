# ADR-005 — Authenticated Local IPC

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Root, Runtime, and Evaluator communicate through versioned, length-framed local IPC:

- Windows uses named pipes with owner-only ACLs and verified peer process identity where available.
- macOS/Linux use owner-only Unix-domain sockets with peer credential checks.
- Each child launch receives a fresh session capability bound to process role, generation, executable identity, and protocol range.
- The wire envelope carries protocol version, stable message discriminator, request ID, idempotency key when applicable, correlation lineage, deadline, payload length, and authenticated session proof.
- Canonical CBOR is used for typed payloads. Large objects cross by verified `ArtifactRef`, not oversized frames.
- Frame, nesting, collection, text, and artifact limits are explicit before decoding allocations.
- Unknown versions/messages, malformed frames, wrong peers, stale generations, replay, and mismatched responses fail closed.
- Cancellation is an explicit message. A timeout or broken channel does not determine external-effect outcome.
- Root persists any request that can cause a durable transition before dispatch.

The public loopback HTTP API remains a Root-owned compatibility surface and does not substitute for protected Root/Runtime/Evaluator IPC.

## Verification

Cross-platform tests cover partial frames, malformed lengths, oversized payloads, unknown versions, stale sessions, wrong roles, replay, duplicate idempotency keys, late responses, cancellation, child death, Root death, and artifact hash mismatch.
