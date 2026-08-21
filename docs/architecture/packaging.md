# Native Packaging, Startup, Upgrade, and Removal

MCG release artifacts are built only on owner-controlled machines. GitHub stores source; it does not build, test, sign, or publish binaries. Both release builders refuse a dirty or uncommitted tree so the package manifest identifies the exact source commit rather than a merely nearby revision.

## Installed layout

Every native package contains one version-matched set:

- `mcg-desktop`: the consumer Tauri application.
- `mcg-service`: the desktop-managed copy of the loopback control plane.
- `mcg`: the automation and administration CLI.
- `mcg-shadow-runner`: the internal disposable Windows Sandbox helper.
- `mcg-updater`: the bounded Windows installer supervisor that survives desktop exit and restores the previous launch path after installer failure.
- `THIRD_PARTY_NOTICES.md`, the complete Apache-2.0 license text required by the adapted OpenAI OAuth implementation, and Wasmtime's complete Apache-2.0-with-LLVM-exceptions license.
- The Linux `.deb` additionally installs the pinned non-setuid Bubblewrap 0.11.2 boundary, its complete corresponding source and GPL notice, and a path-specific capability-stripping AppArmor profile.

Source and portable/headless workflows retain the `mcg-daemon` name. Tauri consumes target-suffixed build inputs through `tauri.release.conf.json` and installs them under stable adjacent names. The default development configuration has bundling disabled and no generated sidecar dependency, so a clean checkout can run ordinary Rust tests without first producing release binaries. On Windows, `prepare-sidecars.ps1` builds `mcg-shadow-runner` separately with a statically linked C runtime and scans the resulting PE image to reject `VCRUNTIME` or `MSVCP` DLL imports; a pristine Windows Sandbox does not contain those redistributables. The runner's schema-8 request contract permits one optional, bounded, read-only host Node distribution at the fixed sandbox root `C:\mcg-toolchain`; MCG does not package, copy, or mutate that host toolchain. Page-render, bounded CSS-element, shared-contract accessibility, hash-bound typed-workflow, and digest-bound screenshot-comparison requests may additionally map the discovered, independently bounded Chromium distribution read-only at `C:\mcg-browser`; the helper uses a random loopback DevTools endpoint under fixed launch/render/shutdown bounds, explicitly emulates the canonical 1280x720 viewport, and writes only to its unmapped disposable workspace. Workflow JSON and screenshot PNG baselines are re-read from the copied private project and matched to their launch-time digests; workflows also require their bound name and shared contract and remain limited to same-origin assertions plus bounded click/type/key steps without arbitrary JavaScript. Screenshot comparison uses bounded PNG decoding and the established per-channel and changed-area thresholds. Windows directs the new-headless browser's endpoint announcement to the inherited runner stderr pipe because the Sandbox System identity does not emit it there by default. Linux maps the same bounded browser root and adjacent runner read-only inside Bubblewrap, so the candidate service and browser probe share the unshared network namespace. The packaged daemon is deliberately called `mcg-service` so native builds never overwrite or contend with a running developer/headless `mcg-daemon`. Both are built from the same crate and version. The desktop never searches `PATH`, accepts a configured public address, or launches an environment-selected executable. It starts only the regular, non-symlink `mcg-service` file adjacent to its own executable. Windows update installation reads and verifies only the adjacent regular `mcg-updater.exe`, copies its exact bytes into the private UUID-bound update directory, and launches that staged copy; the supervisor accepts one private schema-bound request and never an arbitrary executable path.

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

If Python is installed outside the account running the qualification shell, pass
its exact executable with `-PythonPath C:\path\to\python.exe`; the harness validates
that the resolved path is a file before invoking either Python verifier.
The successful project fixture also requires Node/npm. When it is not already on
the qualification account's `PATH`, pass the directory containing `node.exe` and
`npm.cmd` with `-NodeBinPath C:\path\to\node-bin`; the packaged daemon inherits only
that explicitly validated addition.

The harness refuses an existing registered installation, installs into a unique temporary root, verifies the complete payload and licenses, exercises valid and tampered packaged plugins, proves authenticated graceful shutdown with no SQLite sidecars, daemon restart, routine and explicitly encrypted backup/verify/wrong-passphrase/restore, CLI status/doctor plus bare/JSON/NDJSON/piped/oversized consumer behavior, canonical project reuse and token exclusion, successful provider-backed human/JSON/NDJSON work with independent evaluation and governed project-command evidence, modern MCP discovery and tool listing, launches the packaged desktop on its fixed loopback port, captures and validates a real nonblank Windows screenshot, then proves repair, uninstall, retained external data, reinstall, final removal, and failure cleanup. The successful provider fixture is loopback-only, credential-free, and removed before restart; retained terminal tasks, evaluator evidence, tool audit, and side-effect-free replay must remain consistent without the provider. The interactive consumer prompt remains a visible console UAT gate because redirected Windows standard input deliberately selects piped mode; encrypted-backup automation uses the separately explicit bounded `--passphrase-stdin` contract. `-SkipDesktop` exists only for an owner-host preflight whose port 7777 is deliberately occupied; clean-machine release UAT must not use it.

Linux:

```bash
./scripts/build-release.sh
```

This builds the same version-matched sidecars, creates `.deb` and AppImage packages, and writes the same manifest contract. The owner-controlled builder also fetches the reviewed Bubblewrap v0.11.2 commit by immutable Git identity, builds it without setuid support, runs its upstream Meson tests, and injects the binary, corresponding source, GPL text, and AppArmor profile into the `.deb` before hashing the package. The build machine must provide Tauri's documented GTK/WebKit and package-tool prerequisites plus Git, Meson, Ninja, AppArmor tools, and the Bubblewrap development libraries. Generated binaries, packages, source archives, and manifests stay under ignored build directories and are never committed. GitHub serves source only; it performs no build or test.

The `.deb` installs the sandbox binary only at root-owned `/usr/lib/MCG/mcg-bwrap`. Its maintainer scripts load and remove a path-specific AppArmor profile derived from the distro's `bwrap-userns-restrict` policy. Bubblewrap receives namespace setup authority, while the stacked child profile denies capabilities. MCG prefers this fixed protected executable, then accepts a supported protected system Bubblewrap. It never disables Ubuntu's global AppArmor user-namespace restriction, never uses a setuid Bubblewrap, and never falls back to host execution. The unprivileged AppImage cannot install system policy; when run without the `.deb` or another administrator-provided compatible backend, isolated shadow command/service conditions remain honestly inconclusive.

Run Linux package UAT as root while naming the non-root desktop owner:

```bash
sudo MCG_UAT_USER="$USER" ./scripts/test-linux-packages.sh
```

The harness refuses to replace an existing MCG package. In a unique external data directory it proves `.deb` install and same-version repair, the protected Bubblewrap payload/version/source/license and loaded capability-stripping AppArmor profile, real user/network namespace entry without changing global policy, SIGTERM graceful shutdown with no SQLite sidecars, daemon health and restart, authenticated CLI status/doctor plus routine/encrypted backup verification and offline restore, bare/JSON/NDJSON/piped/oversized/interactive consumer CLI behavior, canonical project reuse and token exclusion, successful provider-backed human/JSON/NDJSON work with independent evaluation, governed project-command evidence, and provider-free restart replay, modern MCP discovery and tool listing, installed desktop launch, AppImage extraction and launch, real X11 window creation, screenshot capture, uninstall/profile removal, retained data, reinstall, and final application-file removal. Secret-free evidence is retained for review; generated tokens and temporary credential-bearing state are removed after evidence extraction.

macOS:

```bash
./scripts/build-release-macos.sh
```

The owner-controlled macOS builder creates `MCG.app`, stages it with an `/Applications` shortcut, and uses native `hdiutil` to create and verify a headless DMG before writing the same exact-commit SHA-256 manifest. It never depends on Finder automation or substitutes Linux cross-compilation for Apple's native bundle, code-signing, LaunchServices, Keychain, or Gatekeeper behavior.

Run package UAT from the signed or explicitly owner-approved pre-release artifact:

```bash
MCG_MACOS_UAT_ALLOW_ADHOC=1 ./scripts/test-macos-packages.sh
```

The harness verifies the DMG and complete app payload, classifies its signature, copies it into a unique per-user UAT installation root, proves repair and retained external data, exercises the packaged plugin runner against valid and tampered modules, starts and restarts the packaged service, runs authenticated CLI diagnostics and backup, completes the same provider-backed human/JSON/NDJSON, independent-evaluation, governed-command, and provider-free restart-replay contract as Windows/Linux, negotiates the modern MCP catalog, launches the packaged desktop, captures a real macOS screenshot, removes and reinstalls the app, and retains evidence for review. A remote login that cannot call `screencapture` may set `MCG_MACOS_UAT_EXTERNAL_CAPTURE_REQUEST` to a new absolute request-file path; the harness publishes its unique screenshot destination and waits for a real interactive-session capture without bypassing image validation. It never replaces an existing `/Applications` installation. Ad-hoc or unsigned testing requires an explicit environment opt-in and never counts as a public release. Set `MCG_MACOS_UAT_REQUIRE_NOTARIZED=1` for the final Gatekeeper/stapling gate.

## Upgrade and repair

Install a newer package over the same per-user installation. Durable state remains outside the application directory (`%LOCALAPPDATA%\\MCG` on Windows, `$XDG_DATA_HOME/mcg` or `~/.local/share/mcg` on Linux, and the application-support directory resolved by macOS for `dev.mcg.desktop`). Database migrations run only after the new daemon acquires the exclusive data-directory lock.

### Signed consumer updates

Consumer update support uses the official Tauri v2 updater as a native Rust
boundary. A release build is update-capable only when its owner-controlled build
environment supplies all of `MCG_UPDATER_ENDPOINT`, `MCG_UPDATER_PUBKEY`, and
exactly one private-key source: key content in `TAURI_SIGNING_PRIVATE_KEY` or an
absolute bounded regular non-symlink file in `TAURI_SIGNING_PRIVATE_KEY_PATH`.
Supplying both or neither fails before Tauri starts, and putting a real filesystem
path in the content variable is rejected with guidance to use the path variable.
The endpoint must be bounded HTTPS without embedded
credentials or a fragment, and the public key must be bounded canonical base64
containing minisign public-key text. `scripts/check-updater-config.mjs` validates
those inputs without logging private-key bytes before the signing build starts. The private key and any
password remain outside the repository and are never placed in a manifest.
All platform release builders invoke Tauri build with `--ci`; updater packaging
therefore fails instead of waiting on an invisible interactive password prompt.
Encrypted-key passwords, when used, must be supplied through Tauri's external
build environment and are never read from source or a checked-in script.

The signing-only `tauri.updater.conf.json` overlay enables updater artifacts.
Ordinary development and unsigned package-UAT builds do not apply that overlay
and expose an honest `Updates are unavailable in this build` state. They never
substitute an unsigned channel. Tauri updater signatures are a separate trust
boundary from Windows Authenticode, Linux repository/package signatures, and
macOS Developer ID notarization; public releases require all platform-appropriate
trust gates.

For signed builds, each platform release builder validates those public inputs,
creates a new owner-only configuration in the operating-system temporary directory,
and passes it to Tauri so the bundler receives the same endpoint and canonical
base64 minisign public key that the native updater embeds. The temporary config
contains no private key or password, is created without overwrite, and is removed
on every build exit. The static updater overlay only enables artifact creation;
it is never a substitute for the generated, validated public bundler configuration.

`npm run test:updater-signing` creates a passwordless qualification key only
inside a freshly allocated OS temporary directory, signs a disposable artifact
through the pinned Tauri CLI, decodes the exact base64-wrapped Tauri key and
signature envelopes, verifies the authentic bytes, requires a one-byte mutation
to fail, and runs updater configuration preflight with the generated public key.
The private key is never printed and the temporary directory is removed in a
`finally` path. This proves local signing-tool and trust-format interoperability;
it does not substitute for installing a signed package through owner-controlled
HTTPS on each target platform.

`npm run write:updater-manifest -- CONFIG.json OUTPUT.json` creates the static
Tauri feed used by that HTTPS service. It accepts only canonical semantic versions,
UTC publication dates, bounded notes, known Windows/Linux/macOS updater target
keys, regular artifact/signature files, canonical base64 Tauri minisign envelopes,
and canonical credential-free HTTPS download URLs. Before publication it invokes
the qualification verifier with the supplied public key, artifact, and signature;
the verifier requires the authentic bytes to pass and a mutation to fail. The
published JSON contains no local artifact or public-key paths. Creation uses a
same-directory temporary file plus an atomic hard-link and refuses to replace an
existing release feed, so a published version cannot be silently rewritten by the
release helper.

`npm run serve:updater-uat -- CONFIG.json` is a qualification-only loopback HTTPS
server for packaged updater testing on owner-controlled machines. Its exact config
maps a bounded set of artifact routes and named manifest, corrupt-JSON, or allowed
failure-status scenarios. A separate atomic control file switches scenarios without
restarting the packaged application. The server requires caller-supplied disposable
TLS certificate/key files, TLS 1.2 or newer, an explicit loopback port, regular
non-symlink inputs, and a new request-log path. Logs contain only timestamp, method,
route, result size/status, and scenario; headers and query strings are never retained.
`npm run test:updater-uat-server` creates 24-hour disposable TLS material in the OS
temporary directory and proves loopback confinement, scenario switching, artifact
delivery, corrupt/status failures, traversal/query/method rejection, security headers,
credential-header redaction, and cleanup. The packaged VM lane must trust only that
ephemeral certificate for the test window and remove it afterward.

`python scripts/generate-updater-uat-tls.py ABSOLUTE_CA_CERT ABSOLUTE_SERVER_CHAIN
ABSOLUTE_SERVER_KEY` creates the fixture's 24-hour trust material. It uses a new
3072-bit RSA private CA only in memory, writes the public CA certificate, signs a
2048-bit localhost/127.0.0.1 server certificate with server-auth usage, writes the
server chain and unencrypted server key as exclusive owner-only files, refuses every
existing/noncanonical output, and removes partial output on failure. Its JSON output
contains only the schema, public DER fingerprints, expiry, and hosts—never key bytes
or paths. VM setup imports only the public CA for the bounded UAT window; teardown
must verify its exact fingerprint before removing that trust entry and all three files.

`npm run prepare:updater-uat-source -- ARCHIVE.tar.gz ABSOLUTE_DESTINATION VERSION`
creates one baseline or candidate build tree from the same sealed source boundary.
It accepts only a bounded regular archive containing regular canonical relative paths,
rejects duplicates, traversal, links, special files, missing release inputs, existing
destinations, and noncanonical semantic versions, then checks the extracted tree again.
Archive listing is inspected before extraction; if a platform `tar` rejects a malformed
header while exposing an unsafe entry type, MCG normalizes that result to the same bounded
unsafe-type refusal instead of leaking platform-specific parser behavior.
The helper updates the single Cargo workspace version and every root/desktop npm,
lockfile, and Tauri version surface; regenerates and rechecks Cargo metadata offline;
requires every workspace package to converge on the requested version; normalizes the
eight tracked POSIX executable modes; initializes an isolated local Git repository;
and deterministically converges exactly the no-dependency metadata workspace package
entries in `Cargo.lock` before the locked metadata check, so preparation needs no
registry cache and internal versions are committed before any build.
It returns the source archive digest, entry count, base/prepared versions, and clean
commit identity as JSON. Any failure removes only the newly created destination.
`npm run test:prepare-updater-uat-source -- ARCHIVE.tar.gz` proves one unchanged
baseline plus a newer prerelease candidate, clean distinct commits, locked metadata,
noncanonical-version and existing-destination rejection, and temporary-tree cleanup.
This prevents the update matrix from comparing unrelated source snapshots.

For the full setup, `npm run prepare:updater-uat-workspace -- ARCHIVE.tar.gz
ABSOLUTE_NEW_ROOT BASELINE_VERSION CANDIDATE_VERSION LOOPBACK_HTTPS_ORIGIN`
orchestrates both prepared trees, exact `npm ci` dependencies, short-lived TLS
material, and a fresh passwordless Tauri qualification signing key. It requires the
candidate to be newer, both source results to retain identical archive provenance,
and the generated public key to be a Tauri minisign envelope. The private descriptor
contains only the local paths required by build/serve/teardown automation and stays
inside the disposable root. A separate evidence-safe public descriptor contains no
paths or private fields—only source archive identity, clean commits/versions, endpoint,
public-key digest, and public TLS fingerprints/expiry. Failure removes the exact new
workspace. `npm run test:prepare-updater-uat-workspace -- ARCHIVE.tar.gz` proves
provenance, isolation, public-descriptor privacy, version/origin/destination rejection,
and complete temporary cleanup.

The workspace composer uses its own source preparer only for a disposable bootstrap
extraction. Both final source trees and the TLS chain are then produced by helper
scripts embedded in the sealed archive, not by possibly older scripts in the caller's
checkout. The bootstrap is deleted before dependency installation. The public
descriptor records SHA-256 digests for both archive-bound helpers. Qualification tests
also require each descriptor commit to equal the corresponding repository `HEAD`,
require clean trees, and inspect committed Cargo lockfile versions.

After signing the candidate updater artifact and a deliberately different decoy
artifact with the same disposable key, run `npm run write:updater-uat-scenarios --
VALID_MANIFEST CANDIDATE_ARTIFACT PUBLIC_KEY DECOY_SIGNATURE ABSOLUTE_OUTPUT_DIR
BASELINE_VERSION CHANGED_VERSION`. The helper requires one known platform and a
canonical loopback HTTPS artifact route, proves the valid signature still verifies
the candidate, proves the decoy signature does not, enforces strict semantic-version
ordering, and atomically creates a new scenario directory. It emits exact no-update,
valid, changed-version, and structurally valid but cryptographically wrong-signature
manifests plus corrupt-JSON/unavailable server definitions and a no-update default
control file. It refuses overwrite and removes partial output. This makes negative
feeds reproducible without mutating a signed artifact or weakening runtime checks.

On Windows, `scripts/windows-updater-uat.ps1` is the fail-closed operator boundary
for that matrix. It atomically selects a named feed, records lane-specific snapshots
of registration, payload hashes, backups, temporary residue, rooted processes,
port ownership, trust matches, and free space, and can inject reversible write denial
for the backup or installed-payload directory. ACL injection stores the original DACL
as SID-based SDDL inside the qualification root; restoration must reproduce its exact
protection flag and sorted SID/right/type/inheritance fingerprint even when Windows
canonicalizes equivalent ACE ordering. Only then is the state record archived as evidence. Trust import requires an
explicit action-time switch plus an exact in-root public CA, DER SHA-256, thumbprint,
and subject match in `Cert:\CurrentUser\Root`. Teardown removes only the single
confirmed current-user certificate after independently re-hashing its stored DER;
it deliberately does not depend on the feed marker or CA file still existing.
`scripts/test-windows-updater-uat.ps1` exercises atomic switching, secret-free
snapshots, trust-mutation refusal, reversible ACL failure, semantic-exact restoration, bounded
evidence names, and exact temporary cleanup without modifying a trust store. The
fixture runs in the normal Windows owner-machine verifier.

Update checks are owner-triggered. The native layer rechecks the manifest before
installation and requires the announced semantic version to equal the version
the owner reviewed; the review identity itself must be canonical SemVer text, and
Tauri's default comparator rejects same-version releases and downgrades. Both the
compiled public key and every announced release signature must decode from canonical
base64 into a structurally valid minisign envelope before the UI reports an available
release. It downloads and verifies the complete package against the embedded
minisign public key before changing local state. MCG then creates and independently
verifies a routine credential-free `.mcgb` backup. The backup must have the exact
fresh caller-owned UUID/file name, current application version, canonical digest,
non-empty database entry, and creation time bounded to the current attempt; a stale
idempotent replay cannot authorize installation. MCG then requests authenticated
graceful daemon shutdown, waits for the loopback authority to disappear, installs
the verified artifact, and restarts. Tauri's Windows updater implementation launches
NSIS and immediately terminates the desktop, so MCG does not rely on the unreachable
post-launch error branch there. It stages the already signature-verified raw NSIS
artifact plus a digest-bound schema-3 request and an exact copy of the packaged
supervisor under the private data directory. It launches that private copy so the
installer can replace every installed payload, including `mcg-updater.exe`, without
locking its own destination. The staged supervisor verifies its own name and digest,
the unchanged adjacent packaged supervisor, the canonical install root, the request,
installer name, directory boundaries, and originating packaged desktop PID. Before
backup or shutdown, MCG reads every PE version resource from the exact verified NSIS
bytes, requires one consistent canonical-semver `ProductVersion`, and requires it to
equal the signed manifest version the owner reviewed. Schema 3 binds that expected
and observed version pair to the staged installer digest and rejects a mismatch;
waits for the desktop to exit; invokes fixed passive `/UPDATE` arguments; and waits
for the real installer status. An adjacent packaged cleanup helper waits for the
staged supervisor to exit and removes only that exact UUID-bound private copy.
Failure retains a bounded outcome and relaunches the
previous desktop, which restarts the local service and displays a generic recovery
notice without backend paths or response bodies. A changed release, bad signature,
backup failure, unclean shutdown, or installer failure stops the workflow. If
installation fails after shutdown, MCG restores the previous launch path without
claiming success. The same recovery attempt runs when shutdown
does not reach the required loopback-unavailable state. Packaged service children
are waited by a dedicated reaper, so an exited daemon cannot remain as a zombie
while the desktop stays open.

Final updater qualification must use an ephemeral qualification signing key and
owner-controlled HTTPS fixture to prove no-update, valid update, changed-version,
bad-signature, corrupt response, backup failure, shutdown failure, installer
failure/recovery, successful relaunch, state migration, and supported rollback on
each packaged platform. Qualification keys must never become production trust roots.

If an older daemon is still running, the new desktop refuses the version mismatch and tells the owner to stop the old service and reopen MCG. It never connects a new UI to an old control plane. Repair reinstalls the complete version-matched binary set and license resources without changing application data.

Before a major upgrade, create and verify a `.mcgb` backup. Restore refuses databases newer than the running binary, preventing accidental downgrade migration.

## Uninstall and user data

Package removal deletes installed application files and shortcuts. It intentionally does not delete the external MCG data directory, credentials held by the OS credential service, portable backups, or pre-restore rollback databases. Destructive data removal is a separate, explicit future owner operation; uninstall must never imply consent to erase cognitive state.

Reinstalling the same or a later compatible version reuses the retained state. Installer UAT must cover fresh install, first launch, second launch, upgrade with a stopped daemon, visible mismatch with an old running daemon, repair, uninstall, retained-data reinstall, and an explicit check that no GitHub-hosted compute was used.

## Release gates still required

Source packaging and unsigned or ad-hoc owner-built artifacts are not a signed public release. Production completion requires trusted Windows, Linux, and macOS signing identities; macOS notarization and stapling; clean-machine installer/upgrade/repair/uninstall UAT on every supported desktop platform; malware reputation checks; and publication provenance. Missing signing authority must remain explicit rather than bypassed.
