# ADR-010 — Tools, Skills, Capabilities, and Sandbox Guarantees

- **Status:** Accepted for implementation
- **Date:** 2026-08-29

## Decision

Tools, Skills, hooks, plugins, MCP servers, browser/computer sessions, and subprocesses are untrusted execution mechanisms unless explicitly part of the protected TCB.

- A Skill is a validated procedure/configuration, not ambient arbitrary code.
- A Tool has a versioned manifest, argument schema, effect classification, required capabilities, resource limits, sandbox requirement, and executor identity.
- Model-generated arguments undergo schema, semantic, path/resource, capability, effect, operation-hash, and authority validation.
- Root intersects task, parent, role, tool, user approval, and hard policy; information never expands authority.
- Child execution receives only exact filesystem, network, process, credential, and time/resource scopes.
- Platform isolation is reported as `HARD`, `BEST_EFFORT`, or `UNAVAILABLE`; requests requiring stronger isolation fail closed.
- No implementation shell-concatenates model text.
- Stable deterministic repeated Skills may be compiled into Tools only after behavioral/evidence equivalence testing.

## Verification

Adversarial tests cover traversal, symlink/reparse escape, environment and dynamic-library poisoning, unauthorized network, process-tree escape, secret leakage, schema confusion, hook trust, plugin/MCP escalation, and sandbox-unavailable behavior on each platform.
