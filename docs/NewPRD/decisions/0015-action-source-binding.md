# ADR-015 — Immutable Action Source Binding

- **Status:** Accepted for implementation
- **Date:** 2026-08-30

## Decision

Approval and action navigation must be derived from immutable canonical references, never
from UI search, timestamps, titles, or best-effort legacy joins.

An `ActionEnvelope` may carry a backward-compatible `ActionOrigin` that independently
binds the originating conversation, invocation, command, approval, agent, and evidence.
Each link is a `SourceRef` containing a stable type discriminator, a bounded locator, and
the exact content hash observed when Root authorizes the action.

- Existing and non-interactive actions may omit the origin without breaking replay.
- A present origin must contain at least one source, may contain at most 64 evidence links,
  and rejects empty, oversized, malformed-hash, and duplicate references.
- The complete origin is immutable action intent. Reusing an authorization idempotency key
  with any changed locator or hash is substitution and fails closed.
- Simple does not expose raw source topology. Advanced may show practical source links and
  enable navigation only when the exact record exists. Developer may inspect every bound
  locator and hash but receives no authority to alter the source or Root ledger.
- Compatibility adapters must explicitly populate these references when they project
  conversations, approvals, and commands into canonical workflows/actions. Missing links
  remain visibly unavailable rather than being guessed.

## Verification

Storage tests prove legacy envelopes deserialize without an origin, valid origins survive
canonical inspection, malformed origins fail before authorization, and replay substitution
is rejected. Desktop component and browser E2E prove progressively disclosed source records
remain read-only and unavailable conversation links do not navigate to an unrelated record.

