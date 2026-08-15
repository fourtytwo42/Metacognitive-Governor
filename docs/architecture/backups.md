# Portable Backup and Restore

MCG's `.mcgb` format preserves the evolved local cognitive organization without
turning a routine backup into a credential export. It contains one
online-consistent SQLite snapshot plus the content-addressed artifact files that
the snapshot references. Provider/MCP credentials remain in the operating-system
credential store, and the installation-local bearer-token file is never read into
the bundle.

## Format and integrity

Version 1 is a bounded streaming container:

1. eight-byte `MCGB0001` magic;
2. a big-endian 32-bit manifest length;
3. a JSON manifest; and
4. entry bodies in manifest order.

The manifest records the application version, creation time, explicit
`secrets_included: false`, and each entry's canonical path, byte length, and
SHA-256. Exactly one `state/mcg.sqlite3` entry is required. Artifact entries must
be `artifacts/xx/<sha256>` and match their content-addressed path. Counts,
manifest size, entry size, and total bytes are bounded. Duplicate paths,
traversal, symlinks, unexpected files, changed sources, truncated bodies,
trailing bytes, and hash mismatches fail closed.

Plugin catalog evidence has an additional relational check. Every immutable
plugin revision in the snapshot must decode as a valid sandboxed revision and
have an exact-size artifact entry at its declared module SHA-256. Installation
heads, canonical grants, invocation projections, and immutable invocation
snapshots must agree with their typed JSON and referenced revision. Successful
invocation result digests must also exist in the bundle. Creation and offline
verification apply the same checks, so a catalog without its executable or
result evidence is never represented as a healthy backup.

Creation uses SQLite's online backup API, then writes a create-new temporary
bundle and atomically renames it after every entry has been re-hashed while
streaming. A caller-owned UUID provides lost-response replay: creating the same
backup identity again verifies and returns the existing immutable bundle.

## Restore boundary

Restore is deliberately offline. The daemon holds an operating-system advisory
lock on the data directory for its entire lifetime; the restore command must
acquire that same exclusive lock. This prevents a new daemon and current restore
tool from operating on the database together. Restore also rejects live SQLite
WAL/SHM sidecars rather than guessing whether another process is using them.

The restore command verifies the complete bundle, extracts only validated fixed
paths into a private staging directory, runs SQLite `integrity_check`, and proves
the `mcg_schema` table exists. Content-addressed artifacts merge only when an
existing path has identical bytes. Before swapping the database, MCG moves the
current database to a uniquely named `backups/pre-restore-*.sqlite3` rollback
copy. It never deletes that backup. The destination's auth token remains in
place; credentials are intentionally absent and must be reconnected.

Plugin revisions, invocation history, modules, and result evidence are restored,
but every enabled plugin head is revisioned back to `disabled` in the staged
database before it becomes active. This forces an explicit owner review on the
destination machine, where project paths, available executables, network trust,
and configured credential references may differ from the source installation.
The original bundle remains immutable and the rollback database retains the
pre-restore destination state.

On the next daemon start, ordinary forward migrations and projection recovery
run against the restored database. Restore never downgrades a newer bundle into
an older binary; release qualification must exercise supported cross-version
upgrade paths explicitly.

## Interfaces

Authenticated REST, Tauri, CLI, and the desktop Settings workspace create, list,
and re-verify daemon-owned bundles. `mcg backup verify PATH` works offline for a
transferred file. `mcg backup restore PATH --confirm` is the only restore path in
this checkpoint because a running GUI/daemon cannot truthfully provide the
exclusive recovery boundary. Installer integration will stop the managed daemon,
run this same verifier/restorer, and restart only after success.
