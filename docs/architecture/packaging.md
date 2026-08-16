# Native Packaging, Startup, Upgrade, and Removal

MCG release artifacts are built only on owner-controlled machines. GitHub stores source; it does not build, test, sign, or publish binaries. Both release builders refuse a dirty or uncommitted tree so the package manifest identifies the exact source commit rather than a merely nearby revision.

## Installed layout

Every native package contains one version-matched set:

- `mcg-desktop`: the consumer Tauri application.
- `mcg-service`: the desktop-managed copy of the loopback control plane.
- `mcg`: the automation and administration CLI.
- `mcg-shadow-runner`: the internal disposable Windows Sandbox helper.
- `THIRD_PARTY_NOTICES.md`, the complete Apache-2.0 license text required by the adapted OpenAI OAuth implementation, and Wasmtime's complete Apache-2.0-with-LLVM-exceptions license.

Source and portable/headless workflows retain the `mcg-daemon` name. Tauri consumes target-suffixed build inputs through `tauri.release.conf.json` and installs them under stable adjacent names. The default development configuration has bundling disabled and no generated sidecar dependency, so a clean checkout can run ordinary Rust tests without first producing release binaries. The packaged daemon is deliberately called `mcg-service` so native builds never overwrite or contend with a running developer/headless `mcg-daemon`. Both are built from the same crate and version. The desktop never searches `PATH`, accepts a configured public address, or launches an environment-selected executable. It starts only the regular, non-symlink `mcg-service` file adjacent to its own executable.

## One-click local service startup

At desktop startup, the native layer probes only the compiled numeric HTTP loopback address. It accepts an exact `200` MCG health document with service name `mcg-daemon`, status `ok`, and the same application version as the desktop. It also requires the installation-local bearer token to be readable, non-empty, header-safe, and accepted by `/api/v1/system/status`; the authenticated status identity must match public health. A health-shaped foreign process therefore cannot be mistaken for the local authority.

If nothing owns the port, the desktop launches the adjacent daemon without a console, pins the child's bind address to that compiled loopback socket and its data directory to the desktop's resolved application state, and waits up to eight seconds for the exact health and authentication contract. Inherited environment cannot redirect a desktop-launched daemon onto a public interface. Concurrent desktop launches may start competing processes, but SQLite's data-directory lock and the loopback listener allow only one authority owner; both desktops converge on the surviving service. Closing the UI does not terminate headless or scheduled work.

A foreign service, malformed health response, missing sidecar, unreadable token, or version mismatch is shown as `Local runtime needs attention`; MCG does not disguise it as a healthy runtime. Advanced mode includes the concrete startup diagnostic.

## Local release builds

Windows:

```powershell
./scripts/build-release.ps1
```

This builds release CLI/daemon/sandbox sidecars, stages them under target-triple names, creates a per-user NSIS installer, and writes `target/release/release-manifest.json` with the exact source commit, sizes, and SHA-256 digests.

Run Windows package UAT from an interactive owner session:

```powershell
./scripts/test-windows-installer.ps1 -InstallerPath C:\path\to\MCG_0.1.0_x64-setup.exe
```

The harness refuses an existing registered installation, installs into a unique temporary root, verifies the complete payload and licenses, exercises valid and tampered packaged plugins, proves daemon restart, CLI status/doctor/backup, modern MCP discovery and tool listing, launches the packaged desktop on its fixed loopback port, captures and validates a real nonblank Windows screenshot, then proves repair, uninstall, retained external data, reinstall, final removal, and failure cleanup. `-SkipDesktop` exists only for an owner-host preflight whose port 7777 is deliberately occupied; clean-machine release UAT must not use it.

Linux:

```bash
./scripts/build-release.sh
```

This builds the same version-matched sidecars, creates `.deb` and AppImage packages, and writes the same manifest contract. The build machine must provide Tauri's documented GTK/WebKit and package-tool prerequisites. Generated binaries, packages, and manifests stay under ignored build directories and are never committed.

Run Linux package UAT as root while naming the non-root desktop owner:

```bash
sudo MCG_UAT_USER="$USER" ./scripts/test-linux-packages.sh
```

The harness refuses to replace an existing MCG package. In a unique external data directory it proves `.deb` install and same-version repair, daemon health and restart, authenticated CLI status/doctor/backup, bare/JSON/NDJSON/piped/oversized/interactive consumer CLI behavior, canonical project reuse and token exclusion, modern MCP discovery and tool listing, installed desktop launch, AppImage extraction and launch, real X11 window creation, screenshot capture, uninstall, retained data, reinstall, and final application-file removal. Its temporary evidence directory is retained for review; it never writes credentials to output.

macOS:

```bash
./scripts/build-release-macos.sh
```

The owner-controlled macOS builder creates `MCG.app`, stages it with an `/Applications` shortcut, and uses native `hdiutil` to create and verify a headless DMG before writing the same exact-commit SHA-256 manifest. It never depends on Finder automation or substitutes Linux cross-compilation for Apple's native bundle, code-signing, LaunchServices, Keychain, or Gatekeeper behavior.

Run package UAT from the signed or explicitly owner-approved pre-release artifact:

```bash
MCG_MACOS_UAT_ALLOW_ADHOC=1 ./scripts/test-macos-packages.sh
```

The harness verifies the DMG and complete app payload, classifies its signature, copies it into a unique per-user UAT installation root, proves repair and retained external data, exercises the packaged plugin runner against valid and tampered modules, starts and restarts the packaged service, runs authenticated CLI diagnostics and backup, negotiates the modern MCP catalog, launches the packaged desktop, captures a real macOS screenshot, removes and reinstalls the app, and retains evidence for review. A remote login that cannot call `screencapture` may set `MCG_MACOS_UAT_EXTERNAL_CAPTURE_REQUEST` to a new absolute request-file path; the harness publishes its unique screenshot destination and waits for a real interactive-session capture without bypassing image validation. It never replaces an existing `/Applications` installation. Ad-hoc or unsigned testing requires an explicit environment opt-in and never counts as a public release. Set `MCG_MACOS_UAT_REQUIRE_NOTARIZED=1` for the final Gatekeeper/stapling gate.

## Upgrade and repair

Install a newer package over the same per-user installation. Durable state remains outside the application directory (`%LOCALAPPDATA%\\MCG` on Windows, `$XDG_DATA_HOME/mcg` or `~/.local/share/mcg` on Linux, and the application-support directory resolved by macOS for `dev.mcg.desktop`). Database migrations run only after the new daemon acquires the exclusive data-directory lock.

If an older daemon is still running, the new desktop refuses the version mismatch and tells the owner to stop the old service and reopen MCG. It never connects a new UI to an old control plane. Repair reinstalls the complete version-matched binary set and license resources without changing application data.

Before a major upgrade, create and verify a `.mcgb` backup. Restore refuses databases newer than the running binary, preventing accidental downgrade migration.

## Uninstall and user data

Package removal deletes installed application files and shortcuts. It intentionally does not delete the external MCG data directory, credentials held by the OS credential service, portable backups, or pre-restore rollback databases. Destructive data removal is a separate, explicit future owner operation; uninstall must never imply consent to erase cognitive state.

Reinstalling the same or a later compatible version reuses the retained state. Installer UAT must cover fresh install, first launch, second launch, upgrade with a stopped daemon, visible mismatch with an old running daemon, repair, uninstall, retained-data reinstall, and an explicit check that no GitHub-hosted compute was used.

## Release gates still required

Source packaging and unsigned or ad-hoc owner-built artifacts are not a signed public release. Production completion requires trusted Windows, Linux, and macOS signing identities; macOS notarization and stapling; clean-machine installer/upgrade/repair/uninstall UAT on every supported desktop platform; malware reputation checks; and publication provenance. Missing signing authority must remain explicit rather than bypassed.
