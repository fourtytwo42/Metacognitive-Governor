# Portable Backup and Restore

MCG's `.mcgb` format preserves the evolved local cognitive organization without
silently turning a routine backup into a credential export. Every bundle contains
one online-consistent SQLite snapshot plus the content-addressed artifact files
that the snapshot references. Routine backups exclude provider/MCP credentials.
An advanced, explicit encrypted-backup action can include those credentials. The
installation-local bearer-token file is never read into either bundle type.

## Format and integrity

The physical container remains the bounded `MCGB0001` streaming format:

1. eight-byte `MCGB0001` magic;
2. a big-endian 32-bit manifest length;
3. a JSON manifest; and
4. entry bodies in manifest order.

Schema 1 is retained for reading older secret-free bundles. Schema 2 records the
application version, creation time, an explicit `secrets_included` value, optional
credential-encryption metadata, and each entry's canonical path, byte length, and
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
After the bundle operation, both the storage convenience path and the runtime
REST/CLI/desktop path remove the exact random online-snapshot database plus its
SQLite `-wal` and `-shm` companions on both success and failure. The runtime owns
its caller-identity-prefixed snapshot name, so this cleanup is enforced at that
layer rather than assumed from the lower-level bundle writer. A successful bundle
is not reported if temporary-snapshot cleanup fails.

An encrypted bundle has exactly one final `secrets/credentials.v1.enc` entry.
The entry contains the complete provider and MCP credential catalog, including
explicit no-auth records, encrypted with XChaCha20-Poly1305. A 12-1024-byte owner
passphrase derives the 256-bit key through fixed, bounded Argon2id v19 parameters:
64 MiB memory, three iterations, and one lane. A random 128-bit salt and 192-bit
nonce are public manifest metadata. Authenticated associated data binds the
ciphertext to schema 2, application version, creation time, and the exact ordered
database/artifact entries. The passphrase, derived key, and plaintext payload are
zeroized and never stored. Parameter substitution, ciphertext mutation,
mix-and-match manifests, weak passphrases, wrong passphrases, and connection/auth
shape mismatches fail closed.

## Restore boundary

Restore is deliberately offline. The daemon holds an operating-system advisory
lock on the data directory for its entire lifetime; the restore command must
acquire that same exclusive lock. This prevents a new daemon and current restore
tool from operating on the database together. Restore also rejects live SQLite
WAL/SHM sidecars rather than guessing whether another process is using them.
Use authenticated `mcg stop` to ask a packaged or headless daemon to drain its
HTTP work and close SQLite cleanly; Linux service termination also handles both
SIGINT and SIGTERM. A force-killed process is crash-recovered on its next start,
but it is not an acceptable precondition for offline restore while sidecars remain.

The routine restore command verifies the complete bundle, extracts only validated fixed
paths into a private staging directory, runs SQLite `integrity_check`, and proves
the `mcg_schema` table exists. Content-addressed artifacts merge only when an
existing path has identical bytes. Before swapping the database, MCG moves the
current database to a uniquely named `backups/pre-restore-*.sqlite3` rollback
copy. It never deletes that backup. The destination's auth token remains in
place; credentials are intentionally absent and the destination protected-store
entries remain unchanged.

Encrypted credential restore is a separate explicit operation. It requires
`--restore-secrets`, obtains the passphrase from a hidden terminal prompt, and
authenticates the complete bundle before changing destination state. The staged
database's provider/MCP identities and authentication modes must match the
decrypted catalog exactly. MCG snapshots every affected operating-system
credential slot, replaces only those exact connection identities, and leaves
unrelated protected-store entries untouched. If any protected-store write,
artifact merge, or database installation fails, it restores the prior credential
slots; a rollback failure is surfaced as a hard conflict. Live WAL/SHM rejection
is covered after credential replacement to prove the old database and old
credentials remain aligned. The local bearer token is still never replaced.

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
and re-verify daemon-owned bundles. Advanced mode exposes encrypted creation with
two password fields and explicit warnings; `mcg backup create --include-secrets`
uses a hidden confirmation prompt. `mcg backup verify PATH` works offline for a
transferred file. Restore remains offline because a running GUI/daemon cannot
truthfully provide the exclusive recovery boundary: run `mcg stop`, then use
`mcg backup restore PATH --confirm` for routine bundles or add
`--restore-secrets` for an encrypted credential bundle. Owner-controlled
automation can add `--passphrase-stdin`; standard input must be redirected,
encrypted creation requires passphrase and confirmation on separate lines, and
the ordinary interactive path remains hidden. Installer qualification uses this
bounded route without placing a passphrase in process arguments or environment,
runs the same verifier/restorer, and restarts only after success.
