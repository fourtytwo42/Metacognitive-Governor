# ADR-013 — Runtime Identity, Temporary Invocation Roles, and UI Migration

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

`RuntimeIdentity` is the durable administrative and causal lineage anchor. Chat, Work, Verify, Coder, Tester, Coordinator, and temporary specialists do not automatically define persistent cognitive identities.

- `Chat`, `Work`, and `Verify` are canonical invocation roles/configurations.
- Current Coder activity projects as Work and current Tester/Evaluator activity projects as Verify when provenance supports the mapping.
- Coordination is initially a small Runtime policy/decision, not a permanently running cognitive agent.
- Protected Root/MCG governance is represented as deterministic authority, not an agent personality.
- Existing historical names/messages remain visible and are not rewritten.
- New activity records carry runtime, workflow, attempt, invocation, role, configuration, and parent/child lineage.
- The global Agents page evolves into a workflow/invocation browser; genuinely durable organizations remain separate configuration/ownership concepts.
- Runtime forks create descendant identities with shared pre-fork and separate post-fork history. They do not assert philosophical continuity.

## Verification

Migration and UI tests preserve old timelines, correctly map eligible roles, avoid fabricated manifests, browse cross-conversation invocations, jump to source, and distinguish runtime lineage from temporary agents.
