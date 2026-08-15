# Durable Owner Settings

Owner settings are live application state, not browser preferences or unchecked
environment variables. Migration 25 stores one current projection and an
append-only sequence of reasoned revisions in the same SQLite database as the
rest of the local runtime.

## Mutable controls

- `advanced_mode` changes information density without changing constitutional
  authority.
- `autonomy` selects Guided, Balanced, or Autonomous broker policy.
- `shadow_experiments` is explicit opt-in for automatic candidate experiments;
  development/test defaults may initialize it enabled.

Network binding, data directories, credential backends, browser executables, and
other trust roots remain startup-only configuration. They cannot be changed by
the Settings API.

## Consistency and audit

Every mutation supplies the expected revision and a bounded printable reason.
SQLite atomically advances the singleton projection and inserts its immutable
history record. Stale writers receive a conflict and cannot overwrite a newer
choice. Revision one is initialized from startup defaults only when no durable
settings exist; later daemon starts always load the stored revision.

The runtime caches the committed projection behind a read/write lock. A governed
operation snapshots autonomy when it constructs its authority broker, so one
operation cannot change policy halfway through. New operations and automatic
shadow scheduler scans observe the latest committed settings. A setting change
does not retroactively grant, revoke, or repeat an already claimed side effect.

## Interfaces

Authenticated loopback REST exposes `GET/PUT /api/v1/settings` and
`GET /api/v1/settings/history`. CLI, Tauri, and the desktop Settings workspace use
those same contracts. The desktop persists simple/advanced mode rather than
keeping it in React session state, shows shadow controls only in advanced mode,
and reloads authoritative state after a revision conflict.

Tests cover authentication, stale revisions, live runtime status, immutable
history, daemon/store reopen, and desktop interaction. Cross-platform release UAT
must also change a setting, restart the daemon and desktop, and confirm the same
revision and behavior on Windows and Linux.
