# Stage 2 — Protected Root and Authoritative Storage

**Status:** Cross-platform source and local process implementation qualified; native packaged three-level UAT remains a cumulative gate

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
- Exact commit `f0129cedcccad112934d645d0fa30deaaca943e2` passes the native Unix process harness on Ubuntu and macOS. Both runs exercise direct authenticated IPC status/inspect, legacy HTTP health/status/stop, and clean Root exit. The transferred source archive SHA-256 is `DD01309C2ACE6DE99B0223ABB2D0320328A36FEB189D60C07B7ABC00D4227061` on all three hosts.
- The same Unix source line passes platform 2/2, Protected Root 3/3, strict no-dependency Stage 2 Clippy, representative schema-50 migration, and native Root/CLI builds on Ubuntu and macOS.
- Clean commit `2ceaaa80f67f5ca211468fe8dc9b4a0121c1526a` produces the v0.4.67 Windows one-file portable candidate (515,992,576 bytes, SHA-256 `1AA2D43D9A07C09ECB1817EE21B168B1E99C8E79A6F852BB5C88D652F6904404`). `scripts/test-portable-stage2-windows.ps1` launches it with an isolated profile, proves the embedded listener is the packaged `mcg-service`, and drives the real WebView through native UI Automation.
- Windows native portable UAT proves named/focusable chat controls, Simple → Advanced → Developer persistence through one authoritative settings projection, the Developer Protected Root contract inspector, and the read-only protected-control boundary. Evidence is retained under `target/stage2-portable-uat/d93a42e1aa814072ab7ff32a36a21cf8/`.
- Desktop typecheck, focused Vitest, focused Playwright, documentation validation, and packaging-source validation pass at the current checkpoint.

## Remaining Stage 2 exit evidence

- Exercise interrupted migration, retry, and backup restore with a representative old-profile fixture in one process-level test.
- Add an automated production dependency/packaging assertion that presentation binaries cannot acquire normal authoritative write access. Confirmed offline restore remains a separately authorized, stopped-service recovery operation.
- Run native packaged three-level UAT across the same conversations/projects before and after restart; Windows portable runtime/interface projection is now qualified, while the full installed workflow and Unix package lanes remain open.
- Run the package and native three-level UAT suites on macOS and Linux. Cross-platform local IPC, migration, compatibility, and source-build qualification plus Windows portable UAT are complete for this checkpoint.
- Repeat full workspace Rust, Vitest, Playwright, portable, installer, accessibility, and release-smoke gates before declaring Stage 2 complete.

Until those cumulative gates pass, this document is implementation evidence rather than a release-complete claim.
