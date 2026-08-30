# Stage 2 — Protected Root and Authoritative Storage

**Status:** Cross-platform source/local process and Linux AppImage implementation qualified; installed package and final cross-platform native gates remain cumulative

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
- `migration_process` launches the real debug Root executable against a representative schema-50 conversation, forces a test-only interruption immediately after its verified backup, proves the source remained unmigrated, retries successfully, restores the `.mcgb` into a second profile, re-migrates it, and verifies the original conversation in both profiles.
- The complete 102-test Root-owned compatibility API suite passes with bounded request admission.
- CLI parser coverage proves both Root inspection commands.
- `scripts/test-protected-root-ipc.ps1` launches the real Root process, exercises both new IPC commands and legacy HTTP CLI behavior, then performs an authenticated clean shutdown.
- Exact commit `f0129cedcccad112934d645d0fa30deaaca943e2` passes the native Unix process harness on Ubuntu and macOS. Both runs exercise direct authenticated IPC status/inspect, legacy HTTP health/status/stop, and clean Root exit. The transferred source archive SHA-256 is `DD01309C2ACE6DE99B0223ABB2D0320328A36FEB189D60C07B7ABC00D4227061` on all three hosts.
- The same Unix source line passes platform 2/2, Protected Root 3/3, strict no-dependency Stage 2 Clippy, representative schema-50 migration, and native Root/CLI builds on Ubuntu and macOS.
- Clean commit `2ceaaa80f67f5ca211468fe8dc9b4a0121c1526a` produces the v0.4.67 Windows one-file portable candidate (515,992,576 bytes, SHA-256 `1AA2D43D9A07C09ECB1817EE21B168B1E99C8E79A6F852BB5C88D652F6904404`). `scripts/test-portable-stage2-windows.ps1` launches it with an isolated profile, proves the embedded listener is the packaged `mcg-service`, and drives the real WebView through native UI Automation.
- Windows native portable UAT proves named/focusable chat controls, Simple → Advanced → Developer persistence through one authoritative settings projection, the Developer Protected Root contract inspector, and the read-only protected-control boundary. Evidence is retained under `target/stage2-portable-uat/d93a42e1aa814072ab7ff32a36a21cf8/`.
- Desktop typecheck, focused Vitest, focused Playwright, documentation validation, and packaging-source validation pass at the current checkpoint.
- Packaging validation rejects production Desktop linkage to `mcg-storage` and ordinary CLI SQLite access while retaining only the explicit stopped-service CLI restore primitives.
- Exact source `8504f8bfaf04e61e8ed59c703c36a61e31cd2cec` produced a Linux v0.4.67 AppImage (129,321,464 bytes, SHA-256 `AE4491CE5E10EF4F11BFFFE7DDA7A69271FEA21DDCA42A73B79D35EB356EF6F3`) and hardened Debian package (44,921,870 bytes, SHA-256 `456B8D06A6DAD0B3F287753269A28BDD11D9595DA43AC8A0343F8EBF632F899A`). ClamAV 1.5.3 with daily signatures version 28107 reported both clean. This is pre-final package evidence because subsequent migration-process and scanner corrections require a matched rebuild.
- Exact clean source `bb6842717e060973eb0cc1e0fbd81c080a16e3e6` produced a Linux v0.4.67 AppImage (129,325,560 bytes, SHA-256 `CA4A5B5ABD2A703F3D5C6FBF4358C1D2A5B42D40B4224CD5A36E708ECB602E30`) and hardened Debian package (44,916,624 bytes, SHA-256 `3A826A3CD3C64A061E7D5BA13801805A4AAC5AEC6BBDDD5703753A3F7937251A`). The commit-bound manifest and ClamAV 1.5.3/daily-signature 28108 receipt report both artifacts clean.
- The real AppImage native journey passes under isolated X11/AT-SPI with the packaged adjacent `mcg-service` and pinned Codex 0.150.1. It proves named/focusable Simple controls, Simple → Advanced → Developer through one runtime, the governed Developer warning, read-only Root JSON contract, masked API-key accessibility, Guided settings persistence, reviewed three-stage project topology, no execution before explicit approval, terminal success, and restart verification. Sanitized evidence is retained on the Ubuntu qualification host at `target/stage2-linux-appimage-uat-bb68427/`; generated home/runtime/data state and the one-run loopback token were removed from the retained copy.
- Exact clean source `cea1e07da48c7083a6c319b0e65f88ad24f1a510` is the current Linux checkpoint: AppImage 129,325,560 bytes / SHA-256 `4FE0F0DB27AFA09AE84F93F83AAB4BC0AE0B090D7C7525112E2E24711D81A2EF`, Debian package 44,916,676 bytes / SHA-256 `C98A95DF2C12ADB1E8B1101CF4969E5BD60623E673DA77ADCB6C3732EE6352AD`, and 596-byte commit-bound manifest SHA-256 `E24AE2BD6B92DCAFF3E12D42BB29287D2FB7C89456BA31BB8F979D462E4B1406`. ClamAV 1.5.3/daily-signature 28108 reports both packages clean. The complete real AppImage journey passed again, with sanitized evidence retained at `target/stage2-linux-appimage-uat-cea1e07/`.
- A fresh exact-source macOS build produced the native `MetaCog.app`, exposing a post-bundle script drift that still expected the historical `MCG.app` name. The packaging correction resolves the current name first, retains the compatibility fallback, preserves that resolved name through DMG creation and native UAT, and uses the owner-visible MetaCog DMG identity. Windows, Linux, and macOS UAT defaults now derive current product/version names from Tauri metadata rather than stale `MCG_0.1.0` literals. The exact corrected DMG/package/native gate remains open.

## Remaining Stage 2 exit evidence

- Run the full root-required Debian install/repair/restart/native/AppImage/uninstall/data-retention harness. The rootless AppImage native lane is qualified; installer mutation remains deliberately unweakened.
- Run the current-source package and native three-level UAT suites on macOS and Windows. Earlier Windows portable evidence remains valid for its Root/interface slice but is not the final candidate.
- Repeat full workspace Rust, Vitest, Playwright, portable, installer, accessibility, and release-smoke gates before declaring Stage 2 complete.

Until those cumulative gates pass, this document is implementation evidence rather than a release-complete claim.
