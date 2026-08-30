# ADR-008 — Invocation and Context Reconstruction

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Every material model invocation persists a versioned `InvocationManifest` referencing a versioned `ContextManifest` and enough immutable evidence to reconstruct execution conditions without storing prohibited private reasoning.

The manifest binds:

- TaskContract, workflow attempt, runtime lineage, and configuration;
- invocation role and prompt/configuration hashes;
- exact authorized context references and FocusSet;
- perspective and access state relevant to retrieval;
- provider/model/revision and sampling/reasoning controls;
- Skill and Tool manifests;
- authority and model-egress permit;
- project/worktree/environment fingerprints;
- request/response hashes, timing, usage, and outcome;
- parent/child/correlation lineage.

Private scratch reasoning, secrets, authorization headers, protected fixtures, and evaluator-only measurements are never generic manifest fields. Large or sensitive authorized evidence is referenced by governed artifacts with redacted projections.

Historical runs lacking evidence remain explicitly partial/legacy; MetaCog does not fabricate completeness.

## Verification

For every new material invocation, CLI and Developer inspection can answer the contract's reconstruction questions from persisted evidence. Redaction and access tests prove unauthorized content cannot enter UI, logs, telemetry, backups, or downstream model context.
