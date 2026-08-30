# Stage 2 — Protected Root and Authoritative Storage

**Status:** Windows implementation and local process evidence in progress; native packaged and macOS/Linux qualification remain cumulative gates

## Delivered boundary

- `mcg-protected-rootd` is the production Protected Root executable. Packaging installs that binary under the compatibility name `mcg-service`; `mcg-daemon` remains a compatibility entrypoint into the same Root library.
- Protected Root acquires the exclusive data-directory lock before inspecting, backing up, migrating, or opening authoritative storage. The lock remains held for the process lifetime.
- Storage schema 51 creates one immutable `root_ownership` record containing the default migrated `RuntimeIdentity` and its migration receipt.
- Existing databases are opened read-only for integrity and version inspection, copied through SQLite's online backup mechanism, wrapped in a verified `.mcgb` bundle, and only then migrated. Restart returns the original identity and receipt without creating another migration backup.
- Fresh installations record an explicit no-backup-required receipt. Databases newer than the supported schema fail closed without mutation.
- SQLite authority retains WAL, foreign keys, a five-second busy timeout, full synchronous durability, serialized mutation through the Root-owned store, concurrent WAL readers, and bounded compatibility/API admission.
- The existing loopback HTTP API is a Root-owned compatibility adapter. It retains its legacy health identity for existing clients while all storage mutation occurs inside the Root process.

## Local IPC

- `mcg-platform` provides canonical-CBOR, four-byte length-framed local IPC with a 1 MiB frame limit.
- Each Root launch publishes a fresh 256-bit capability and session identity. Capability values are redacted from `Debug`, compared in constant time, and zeroized when released.
- Windows uses local-only named pipes. macOS/Linux use a mode-0600 Unix-domain socket and reject a peer UID that differs from the data-directory owner.
- Root rejects malformed or oversized frames, wrong capabilities, stale sessions, expired requests, replayed request identifiers, wrong message types, and unsupported protocol versions.
- IPC accepts at most 32 simultaneous connections. The Root-owned compatibility adapter admits at most 64 in-flight requests. `RootStatus.writer_queue_capacity` reports that compatibility admission bound; a dedicated durable workflow queue arrives in Stage 3.

## GUI and CLI

- `mcg runtime status [--json]` and `mcg runtime inspect [--json]` query Protected Root directly through local IPC and do not require the legacy HTTP bearer token.
- Advanced shows Root connection, runtime identity, and storage schema.
- Developer shows process/session identity, protocol, schema, executable hash, queue bound, and immutable migration evidence.
- Simple retains the existing conversation workflow and receives the same Root-owned projections.
- All three levels remain presentation projections over one runtime and one authoritative state.

## Automated evidence

- Contract/platform tests cover canonical bounded framing and launch-scoped session lifecycle.
- Protected Root tests cover verified pre-migration backup, stable restart identity/receipt, authenticated real named-pipe exchange, and rejection of replay, expiry, unsupported protocol, stale session, and forged capability.
- Storage tests cover schema 50 to 51 migration, WAL/foreign-key/busy-timeout configuration, concurrent read during an active writer transaction, and newer-schema refusal.
- The complete 102-test Root-owned compatibility API suite passes with bounded request admission.
- CLI parser coverage proves both Root inspection commands.
- `scripts/test-protected-root-ipc.ps1` launches the real Root process, exercises both new IPC commands and legacy HTTP CLI behavior, then performs an authenticated clean shutdown.
- Desktop typecheck, focused Vitest, focused Playwright, documentation validation, and packaging-source validation pass at the current checkpoint.

## Remaining Stage 2 exit evidence

- Exercise interrupted migration, retry, and backup restore with a representative old-profile fixture in one process-level test.
- Add an automated production dependency/packaging assertion that presentation binaries cannot acquire normal authoritative write access. Confirmed offline restore remains a separately authorized, stopped-service recovery operation.
- Run native packaged three-level UAT across the same conversations/projects before and after restart.
- Run the local IPC, migration, compatibility, package, and UAT suites on Windows, macOS, and Linux.
- Repeat full workspace Rust, Vitest, Playwright, portable, installer, accessibility, and release-smoke gates before declaring Stage 2 complete.

Until those cumulative gates pass, this document is implementation evidence rather than a release-complete claim.
