# M3 Tool and Policy Plane

- Status: Implemented and locally qualified on Windows; Linux qualification is
  required before closing M3.
- Date: 2026-08-14.
- Governing milestone: M3 in `docs/roadmap/milestones.md`.

## Execution boundary

Every host effect is a versioned `ToolRequest`. The broker resolves its concrete
resource, revalidates the owner constitution, evaluates the request against an
explicit capability set, and records the decision before any executor is called.
An allow decision acquires an execution lease and invokes the host only through
the typed composite executor. Denied and approval-pending requests never reach
the host.

The implemented primitive families are:

- capability-confined filesystem read, list, atomic write, patch, and delete;
- structured process exec/spawn/status/stdin/kill and separately gated shell;
- bounded HTTP requests and hash-verified downloads with same-origin redirects;
- privacy-safe system/process inspection;
- Git status, diff, commit, checkout, isolated worktrees, and conflict-aware
  patch application;
- a bounded memory repository interface with secret namespaces denied.

Executables and shells are exact capability entries, not basename matches.
Filesystem paths are confined beneath a pre-opened project root, including on
Windows where sibling-prefix and traversal tricks are rejected. Network audit
records remove URL query strings and fragments. Sensitive HTTP headers and
process environment values accept credential references rather than serialized
secret values.

## Evidence and recovery

SQLite migration 3 adds durable approvals, execution leases, immutable tool
audit records, and artifact registrations. Approval creation/resolution and tool
audit completion are each committed atomically with task events. Database
triggers reject audit mutation or deletion. Startup expires interrupted active
leases so an abandoned effect cannot remain falsely marked as running.

Large or binary results are stored in the content-addressed artifact store.
Audit records link the request, policy result, outcome, artifacts, and explicit
verification evidence. Output is bounded while pipes continue to drain, and
credential values are redacted before results can enter events or logs.

## Approval interfaces

Authenticated loopback API routes provide approval list/get/resolve and per-task
tool-audit reads. The CLI exposes the same lifecycle through `mcg approval list`,
`get`, `approve`, `deny`, and `cancel`, plus `mcg task audit`. Resolution is
conflict-safe: a terminal approval cannot be decided a second time.

Raw tool execution is deliberately not exposed as a public transport endpoint
yet. M4/M5 project and agent orchestration will construct project-scoped
capabilities; M6 will add the consumer approval UI using these same contracts.

## Qualification

Deterministic tests use temporary directories, repositories, SQLite databases,
local HTTP servers, and controlled child processes. They cover path escapes,
stale patches, cross-origin redirects, download hash mismatch, credential echo,
bounded output, timeout cleanup, worktree isolation, approval side-effect
gating, immutable evidence, and restart lease recovery.

Run the complete owner-machine suites with:

```powershell
.\scripts\verify.ps1
```

```bash
./scripts/verify.sh
```

These scripts fail immediately when any native Rust, npm, Tauri, documentation,
or Playwright command returns a nonzero exit code. No GitHub-hosted build or test
automation is used.
