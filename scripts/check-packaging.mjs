import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const read = (file) => readFile(path.join(root, file), "utf8");
const requireText = (value, needle, source) => {
  if (!value.includes(needle)) throw new Error(`${source} is missing ${JSON.stringify(needle)}`);
};

const developmentConfig = JSON.parse(await read("apps/desktop/src-tauri/tauri.conf.json"));
if (developmentConfig.bundle?.active !== false || developmentConfig.bundle?.externalBin) {
  throw new Error("Default development and test builds must not depend on generated release sidecars.");
}
const desktopWindowCapability = JSON.parse(await read("apps/desktop/src-tauri/capabilities/desktop-window.json"));
const requiredDesktopWindowPermissions = [
  "core:default",
  "core:window:allow-close",
  "core:window:allow-destroy",
  "core:window:allow-minimize",
  "core:window:allow-toggle-maximize",
  "core:window:allow-set-fullscreen",
  "core:window:allow-start-dragging"
];
if (JSON.stringify(desktopWindowCapability.windows) !== JSON.stringify(["*"])) {
  throw new Error("The custom desktop title bar capability must cover every MetaCog-owned window.");
}
for (const permission of requiredDesktopWindowPermissions) {
  if (!desktopWindowCapability.permissions?.includes(permission)) {
    throw new Error(`The custom desktop title bar is missing its native permission: ${permission}`);
  }
}
const [rootPackage, desktopPackage, cargoManifest, desktopHtml, portableWindowsBuilder] = await Promise.all([
  read("package.json").then(JSON.parse),
  read("apps/desktop/package.json").then(JSON.parse),
  read("Cargo.toml"),
  read("apps/desktop/index.html"),
  read("scripts/build-portable-windows.ps1")
]);
const applicationVersion = developmentConfig.version;
if (
  rootPackage.version !== applicationVersion ||
  desktopPackage.version !== applicationVersion ||
  !cargoManifest.includes(`version = "${applicationVersion}"`)
) {
  throw new Error("Cargo, npm, desktop, and Tauri application versions must remain identical.");
}
if (developmentConfig.productName !== "MetaCog") {
  throw new Error("The packaged desktop product name must use the MetaCog brand.");
}
if (developmentConfig.app?.windows?.[0]?.title !== `MetaCog v${applicationVersion}`) {
  throw new Error("The native window title must expose the exact application version.");
}
requireText(desktopHtml, `<title>MetaCog v${applicationVersion}</title>`, "desktop HTML title");
for (const invariant of [
  'MetaCog-Portable-Windows-x64-v$version',
  'MetaCog-Desktop-v$version.exe',
  '--features single-file-portable',
  'one self-contained executable',
  'Removed obsolete portable directories'
]) requireText(portableWindowsBuilder, invariant, "Windows portable builder");
const config = JSON.parse(await read("apps/desktop/src-tauri/tauri.release.conf.json"));
const updaterConfig = JSON.parse(await read("apps/desktop/src-tauri/tauri.updater.conf.json"));
if (config.bundle?.active !== true || config.bundle?.targets !== "all") {
  throw new Error("Tauri production bundling must remain active for every native platform target.");
}
const expectedSidecars = ["binaries/mcg-service", "binaries/mcg", "binaries/mcg-agent-mcp", "binaries/mcg-plugin-runner", "binaries/mcg-shadow-runner", "binaries/mcg-updater"];
if (JSON.stringify(config.bundle.externalBin) !== JSON.stringify(expectedSidecars)) {
  throw new Error("The installer sidecar contract changed or is incomplete.");
}
const expectedIcons = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico"
];
if (JSON.stringify(config.bundle.icon) !== JSON.stringify(expectedIcons)) {
  throw new Error("The production bundle must explicitly declare the cross-platform icon set.");
}
for (const icon of expectedIcons) {
  const iconPath = path.join(root, "apps/desktop/src-tauri", icon);
  await access(iconPath, constants.R_OK);
  if (path.extname(icon) === ".png") {
    const bytes = await readFile(iconPath);
    const pngSignature = "89504e470d0a1a0a";
    if (bytes.length < 24 || bytes.subarray(0, 8).toString("hex") !== pngSignature) {
      throw new Error(`Production bundle icon is not a valid PNG: ${icon}`);
    }
    const width = bytes.readUInt32BE(16);
    const height = bytes.readUInt32BE(20);
    if (width === 0 || width !== height) {
      throw new Error(`AppImage requires a non-empty square PNG icon: ${icon}`);
    }
  }
}
for (const resource of [
  "../../../THIRD_PARTY_NOTICES.md",
  "../../../licenses/Apache-2.0.txt",
  "../../../licenses/Wasmtime-Apache-2.0-WITH-LLVM-exception.txt"
]) {
  if (!config.bundle.resources?.includes(resource)) throw new Error(`Installer resource missing: ${resource}`);
}
if (config.bundle.windows?.nsis?.installMode !== "currentUser") {
  throw new Error("Windows installation must stay per-user until a privileged service design is approved.");
}
if (updaterConfig.bundle?.createUpdaterArtifacts !== true) {
  throw new Error("The signed updater overlay must create Tauri updater artifacts.");
}
if (JSON.stringify(developmentConfig.plugins?.updater) !== JSON.stringify({ endpoints: [], pubkey: "" })) {
  throw new Error("The base Tauri config must provide a valid inert updater object so unsigned packages start without update authority.");
}

const [windowsBuilder, linuxBuilder, macosBuilder, macosDmgBuilder, updaterSigningTest, releaseArtifactScanner, releaseArtifactScannerTest, updaterBuildConfigWriter, updaterManifestWriter, updaterSignatureVerifier, updaterUatScenarioWriter, updaterUatTlsGenerator, updaterUatServer, updaterUatServerTest, updaterUatSourcePreparer, updaterUatSourcePreparerTest, updaterUatWorkspacePreparer, updaterUatWorkspacePreparerTest, windowsUpdaterHarness, windowsUpdaterHarnessTest, windowsVerifier, windowsPrepare, linuxPrepare, linuxSandboxPrepare, linuxDebHardener, linuxSandboxProfile, notices, apache, wasmtime, shadowRunner, shadowVerification, updaterSupervisor, windowsPackageHarness, linuxPackageHarness, macosPackageHarness, windowsNativeAccessibility, linuxNativeAccessibility, nativeReviewedTopology, macosNativeAccessibility, macosWindowCapture] = await Promise.all([
  read("scripts/build-release.ps1"),
  read("scripts/build-release.sh"),
  read("scripts/build-release-macos.sh"),
  read("scripts/create-macos-dmg.sh"),
  read("scripts/test-updater-signing.mjs"),
  read("scripts/scan-release-artifacts.mjs"),
  read("scripts/test-scan-release-artifacts.mjs"),
  read("scripts/write-updater-build-config.mjs"),
  read("scripts/write-updater-manifest.mjs"),
  read("apps/desktop/src-tauri/examples/verify-updater-signature.rs"),
  read("scripts/write-updater-uat-scenarios.mjs"),
  read("scripts/generate-updater-uat-tls.py"),
  read("scripts/updater-uat-server.mjs"),
  read("scripts/test-updater-uat-server.mjs"),
  read("scripts/prepare-updater-uat-source.mjs"),
  read("scripts/test-prepare-updater-uat-source.mjs"),
  read("scripts/prepare-updater-uat-workspace.mjs"),
  read("scripts/test-prepare-updater-uat-workspace.mjs"),
  read("scripts/windows-updater-uat.ps1"),
  read("scripts/test-windows-updater-uat.ps1"),
  read("scripts/verify.ps1"),
  read("scripts/prepare-sidecars.ps1"),
  read("scripts/prepare-sidecars.sh"),
  read("scripts/prepare-linux-sandbox.sh"),
  read("scripts/harden-linux-deb.sh"),
  read("packaging/linux/apparmor.d/metacognitive-governor-bwrap"),
  read("THIRD_PARTY_NOTICES.md"),
  read("licenses/Apache-2.0.txt"),
  read("licenses/Wasmtime-Apache-2.0-WITH-LLVM-exception.txt"),
  read("apps/mcg-shadow-runner/src/main.rs"),
  read("crates/mcg-runtime/src/shadow_verification.rs"),
  read("apps/mcg-updater/src/lib.rs"),
  read("scripts/test-windows-installer.ps1"),
  read("scripts/test-linux-packages.sh"),
  read("scripts/test-macos-packages.sh"),
  read("scripts/test-installed-native-accessibility-windows.ps1"),
  read("scripts/test-installed-native-accessibility-linux.py"),
  read("scripts/test-installed-native-reviewed-topology.py"),
  read("scripts/test-installed-native-accessibility-macos.swift"),
  read("scripts/capture-installed-macos-window.swift")
]);
const installedOrganizationSharing = await read("scripts/test-installed-organization-sharing.mjs");
for (const [harness, verifier, label] of [
  [windowsPackageHarness, "test-installed-native-accessibility-windows.ps1", "Windows package harness"],
  [linuxPackageHarness, "test-installed-native-accessibility-linux.py", "Linux package harness"],
  [macosPackageHarness, "test-installed-native-accessibility-macos.swift", "macOS package harness"],
  [macosPackageHarness, "capture-installed-macos-window.swift", "macOS package harness"]
]) requireText(harness, verifier, label);
for (const [harness, label] of [
  [windowsPackageHarness, "Windows package harness"],
  [linuxPackageHarness, "Linux package harness"],
  [macosPackageHarness, "macOS package harness"]
]) {
  requireText(harness, "test-installed-organization-sharing.mjs", label);
  requireText(harness, "organization-sharing-restart.json", label);
}
for (const invariant of ["UI Automation", "IsPassword", "keyboard_focus_sequence", "credential_accessible_value_redacted", "MCG_NATIVE_SECRET_CANARY", "Reviewed topology reason", "Run reviewed plan", "Approve and run", "reviewed_topology_terminal_succeeded_visible"]) {
  requireText(windowsNativeAccessibility, invariant, "Windows native accessibility verifier");
}
for (const invariant of ["AT-SPI", "password text", "keyboard_focus_sequence", "credential_accessible_value_redacted", "MCG_NATIVE_SECRET_CANARY", "Reviewed topology reason", "Run reviewed plan", "Approve and run", "reviewed_topology_terminal_succeeded_visible"]) {
  requireText(linuxNativeAccessibility, invariant, "Linux native accessibility verifier");
}
for (const invariant of ["AXIsProcessTrusted", "kAXSecureTextFieldSubrole", "keyboard_focus_sequence", "credential_accessible_value_redacted", "MCG_NATIVE_SECRET_CANARY"]) {
  requireText(macosNativeAccessibility, invariant, "macOS native accessibility verifier");
}
for (const invariant of ["CGWindowListCopyWindowInfo", "kCGWindowOwnerPID", "screencapture", "-l"]) {
  requireText(macosWindowCapture, invariant, "macOS window capture verifier");
}
for (const invariant of ["generateKeyPairSync", "ed25519", "private_signing_key_persisted", "tampered_package_rejected", "revoked_review_mutation_free", "cli_rest_mcp_agree", "post_restart_import_replay_observation_only"]) {
  requireText(installedOrganizationSharing, invariant, "installed organization-sharing verifier");
}
for (const invariant of ["mkdtemp", '"signer"', '"generate"', '"sign"', "TAURI_SIGNING_PRIVATE_KEY_PATH", "finally", "rm(resolvedTemporaryRoot"]) {
  requireText(updaterSigningTest, invariant, "ephemeral updater signing qualification");
}
for (const invariant of ["Microsoft Defender Antivirus", "-DisableRemediation", "ClamAV", "--infected", "signatures must be no more than 72 hours old", "source commit does not match repository HEAD", "Release artifact SHA-256 mismatch", "Refusing to replace existing release scan evidence", "await link(temporary, output)", "macOS public-release reputation is enforced by the notarization, stapling, and Gatekeeper package lane"]) {
  requireText(releaseArtifactScanner, invariant, "release artifact malware scanner");
}
for (const invariant of ["mcg-release-scan-", "Qualification Scanner", "must not disclose absolute paths", "unsafe path", "SHA-256 mismatch", "fixture detection", "Refusing to replace existing release scan evidence"]) {
  requireText(releaseArtifactScannerTest, invariant, "release artifact malware-scanner qualification");
}
for (const invariant of ["canonical bounded credential-free HTTPS", "canonical bounded base64", "Tauri minisign envelope", "createUpdaterArtifacts", "endpoints", "pubkey", 'flag: "wx"', "mode: 0o600", "output already exists"]) {
  requireText(updaterBuildConfigWriter, invariant, "temporary updater bundler configuration writer");
}
for (const invariant of ["windows-x86_64-nsis", "linux-x86_64-appimage", "darwin-aarch64-app", "canonical credential-free HTTPS", "public key is not a Tauri minisign envelope", "signature is not canonical base64", "signature is not a Tauri minisign envelope", "verify-updater-signature", "signature does not verify the selected artifact", "output already exists", "await link(temporaryOutput, outputPath)"]) {
  requireText(updaterManifestWriter, invariant, "updater publication manifest writer");
}
for (const invariant of ["decode_tauri_envelope", "PublicKey::decode", "Signature::decode", "artifact[0] ^= 1", "unexpectedly verified"]) {
  requireText(updaterSignatureVerifier, invariant, "updater signature verifier");
}
for (const invariant of ["baseline version must be older", "changed version must be newer", "canonical loopback HTTPS", "bad signature must differ", "bad signature unexpectedly verifies", "no-update.json", "changed-version.json", "bad-signature.json", "corrupt-json", "output directory already exists", "rm(output"]) {
  requireText(updaterUatScenarioWriter, invariant, "cryptographic updater UAT scenario writer");
}
for (const invariant of ["canonical loopback HTTPS", "TLSv1.2", "cache-control", "no-store", "invalid fixture control", "corrupt-json", "FAILURE_STATUSES", "request_log"]) {
  requireText(updaterUatServer, invariant, "owner-controlled updater UAT server");
}
for (const invariant of ["VALIDITY_HOURS = 24", "BasicConstraints(ca=True", "ExtendedKeyUsageOID.SERVER_AUTH", "127.0.0.1", "O_EXCL", "server_key_pem", "ca_sha256", "server_sha256", "candidate.unlink(missing_ok=True)"]) {
  requireText(updaterUatTlsGenerator, invariant, "disposable updater UAT TLS generator");
}
for (const invariant of ["mkdtemp", "generate-updater-uat-tls.py", "validity_hours, 24", "TLS fixture expires too soon", "unexpectedly replaced existing material", "rejectUnauthorized: false", "must-not-be-logged", "credential=forbidden", "finally", "rm(resolved"]) {
  requireText(updaterUatServerTest, invariant, "updater UAT server qualification");
}
for (const invariant of ["archive entry path is unsafe", "archive entry has an unsafe type", "source version surfaces do not agree", "metadata", "--offline", "--locked", "workspace_members", "destination already exists", "status", "--porcelain", "rm(destination"]) {
  requireText(updaterUatSourcePreparer, invariant, "updater UAT source preparer");
}
for (const invariant of ["0.1.1-uat.1", "01.1.0", "invalid preparation unexpectedly succeeded", "notEqual(baselineResult.commit", "finally", "rm(resolved"]) {
  requireText(updaterUatSourcePreparerTest, invariant, "updater UAT source preparation test");
}
for (const invariant of ["candidate version must be newer", "canonical loopback HTTPS", "prepared source provenance diverged", "archive source preparer", "archive TLS generator", "source_preparer_sha256", "tls_generator_sha256", "npm.cmd ci", "generate-updater-uat-tls.py", '"signer", "generate"', "qualification-public.json", "qualification-private.json", "updater_public_key_sha256", "rm(workspace"]) {
  requireText(updaterUatWorkspacePreparer, invariant, "disposable updater UAT workspace preparer");
}
for (const invariant of ["0.1.1-uat.1", "includes(\"private\")", "includes(workspace)", "invalid workspace preparation unexpectedly succeeded", "bad-origin", "finally", "rm(resolved"]) {
  requireText(updaterUatWorkspacePreparerTest, invariant, "updater UAT workspace preparation test");
}
for (const invariant of ["RemoveCa", "ConfirmTrustMutation", "Cert:\\CurrentUser\\Root", "ExpectedCaDerSha256", "ExpectedCaThumbprint", "[IO.File]::Replace", ".bak", "current_user_ca_matches", "GetSecurityDescriptorSddlForm", "SetAccessControl", "Move-Item", "Assert-ChildPath"]) {
  requireText(windowsUpdaterHarness, invariant, "Windows updater UAT harness");
}
for (const forbidden of ["CommandLine", "auth_token", "access_token", "refresh_token", "private_key"]) {
  if (windowsUpdaterHarness.includes(forbidden)) throw new Error(`Windows updater UAT evidence contract contains forbidden field text: ${forbidden}`);
}
for (const invariant of ["mcg-windows-updater-uat-test-", "ExpectFailure", '"ImportCa"', "GetSecurityDescriptorSddlForm", "SetAccessControl", "Compare-Object", "Refusing to remove an unexpected fixture path"]) {
  requireText(windowsUpdaterHarnessTest, invariant, "Windows updater UAT harness fixture");
}
requireText(windowsVerifier, "test-windows-updater-uat.ps1", "Windows canonical verifier");
requireText(windowsBuilder, "--bundles nsis", "Windows release builder");
requireText(linuxBuilder, "--bundles deb,appimage", "Linux release builder");
requireText(linuxBuilder, "prepare-linux-sandbox.sh", "Linux release builder");
requireText(linuxBuilder, "harden-linux-deb.sh", "Linux release builder");
requireText(macosBuilder, "--bundles app", "macOS release builder");
requireText(macosBuilder, "create-macos-dmg.sh", "macOS release builder");
requireText(macosBuilder, 'rm -rf -- "$dmg_directory"', "macOS release builder");
for (const [source, label] of [[windowsBuilder, "Windows release builder"], [linuxBuilder, "Linux release builder"]]) {
  requireText(source, "scan-release-artifacts.mjs", label);
  requireText(source, "release-security-scan.json", label);
  requireText(source, "Refusing to replace existing release security-scan evidence", label);
}
for (const invariant of ["hdiutil create", "hdiutil verify", "staging/MCG.app", "staging/Applications", "Refusing to replace existing disk image", "macOS disk images must be created on macOS"]) {
  requireText(macosDmgBuilder, invariant, "macOS disk-image builder");
}
requireText(windowsBuilder, "tauri.release.conf.json", "Windows release builder");
requireText(linuxBuilder, "tauri.release.conf.json", "Linux release builder");
requireText(macosBuilder, "tauri.release.conf.json", "macOS release builder");
for (const [source, label] of [[windowsBuilder, "Windows release builder"], [linuxBuilder, "Linux release builder"], [macosBuilder, "macOS release builder"]]) {
  for (const invariant of ["MCG_UPDATER_ENDPOINT", "MCG_UPDATER_PUBKEY", "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PATH", "exactly one of", "tauri.updater.conf.json", "write-updater-build-config.mjs", "updater.json", "build --ci", "loadedSigningKeyFromPath"]) {
    requireText(source, invariant, label);
  }
}
for (const invariant of ["updaterConfigDirectory", "Remove-Item", "finally"]) requireText(windowsBuilder, invariant, "Windows updater config cleanup");
for (const [source, label] of [[linuxBuilder, "Linux updater config cleanup"], [macosBuilder, "macOS updater config cleanup"]]) {
  for (const invariant of ["mktemp -d", "trap cleanup_updater_config EXIT", 'rm -rf -- "$updater_config_directory"']) requireText(source, invariant, label);
}
for (const [source, label] of [[windowsPrepare, "Windows sidecar preparer"], [linuxPrepare, "Linux sidecar preparer"]]) {
  for (const binary of ["mcg", "mcg-daemon", "mcg-service", "mcg-agent-mcp", "mcg-plugin-runner", "mcg-shadow-runner", "mcg-updater"]) requireText(source, binary, label);
  requireText(source, "rustc -vV", label);
}
for (const invariant of ["target-feature=+crt-static", "VCRUNTIME", "MSVCP", "ReadAllBytes", '"rustc", "-p", "mcg-shadow-runner"']) {
  requireText(windowsPrepare, invariant, "Windows self-contained Sandbox runner build");
}
for (const invariant of ['REQUEST_SCHEMA_VERSION: u16 = 8', 'toolchain_root', 'C:\\mcg-toolchain', 'browser_executable', 'C:\\mcg-browser', 'Browser::launch', 'BROWSER_RENDER_TIMEOUT', 'ElementExists', 'Accessibility', 'BrowserWorkflow', 'ScreenshotMatches', 'expected_baseline_sha256', 'compare_png_screenshots', 'ACCESSIBILITY_AUDIT_SCRIPT', 'enable-logging']) {
  requireText(shadowRunner, invariant, "Windows Sandbox runner contract");
}
for (const invariant of ["NODE_TOOLCHAIN_MAX_BYTES", "NODE_TOOLCHAIN_MAX_FILES", "NODE_TOOLCHAIN_MAX_DEPTH", "BROWSER_RUNTIME_MAX_BYTES", "BROWSER_RUNTIME_MAX_FILES", "BROWSER_RUNTIME_MAX_DEPTH", '"--ro-bind"', 'r"C:\\mcg-toolchain"', 'r"C:\\mcg-browser"', '"/mcg-shadow-runner"']) {
  requireText(shadowVerification, invariant, "bounded read-only shadow toolchain contract");
}
for (const invariant of ["REQUEST_SCHEMA: u32 = 3", "stage_verified_windows_installer", "windows_installer_product_version", "expected_version", "installer_product_version", "canonical_regular_child", "installer_sha256", "supervisor_sha256", "validate_running_staged_supervisor", "cleanup_staged_windows_supervisor", "wait_for_desktop_exit", 'args(["/P", "/R", "/UPDATE", "/ARGS"])', "MCG_UPDATE_RECOVERY_NOTICE", "The update did not complete"]) {
  requireText(updaterSupervisor, invariant, "supervised Windows updater contract");
}
for (const invariant of ["1b80120ef26a28e065e67f89bfef873f13bdd317", "support_setuid=false", "meson test", "bubblewrap-v0.11.2.tar.gz", "SHA256SUMS"]) {
  requireText(linuxSandboxPrepare, invariant, "pinned Linux Bubblewrap builder");
}
for (const invariant of ["/usr/lib/MCG/mcg-bwrap", "/usr/sbin/apparmor_parser -r", "/usr/sbin/apparmor_parser -R", "--root-owner-group", "Bubblewrap-GPL-2.0-or-later.txt"]) {
  requireText(linuxDebHardener, invariant, "Linux Debian sandbox hardener");
}
for (const invariant of ["allow userns", "audit deny capability", "/usr/lib/MCG/mcg-bwrap", "metacognitive-governor-unpriv-bwrap"]) {
  requireText(linuxSandboxProfile, invariant, "Linux Bubblewrap AppArmor profile");
}
requireText(shadowVerification, '/usr/lib/MCG/mcg-bwrap', "packaged Linux Bubblewrap selection");
requireText(notices, "OpenAI Codex", "third-party notices");
requireText(notices, "Bubblewrap", "third-party notices");
for (const section of ["1. Definitions.", "2. Grant of Copyright License.", "9. Accepting Warranty or Additional Liability.", "END OF TERMS AND CONDITIONS"]) {
  requireText(apache, section, "Apache-2.0 license payload");
}
if (apache.length < 10_000) throw new Error("Apache-2.0 license payload appears truncated.");
for (const section of ["1. Definitions.", "END OF TERMS AND CONDITIONS", "LLVM Exceptions to the Apache 2.0 License", "embedded into an Object form of such source code"]) {
  requireText(wasmtime, section, "Wasmtime license payload");
}
if (wasmtime.length < 12_000) throw new Error("Wasmtime license payload appears truncated.");
const wasmtimeLicenseDigest = createHash("sha256")
  .update(wasmtime.replaceAll("\r\n", "\n").trimEnd())
  .digest("hex");
if (wasmtimeLicenseDigest !== "23823edf263108ef7acbefde293225117b4784759c9c5bcd0b4966d0a1e34f55") {
  throw new Error("Wasmtime 45.0.3 license payload does not match the reviewed upstream text.");
}

for (const file of ["scripts/build-release.ps1", "scripts/build-release.sh", "scripts/build-release-macos.sh", "scripts/create-macos-dmg.sh", "scripts/scan-release-artifacts.mjs", "scripts/test-scan-release-artifacts.mjs", "scripts/write-updater-build-config.mjs", "scripts/prepare-sidecars.ps1", "scripts/prepare-sidecars.sh", "scripts/prepare-linux-sandbox.sh", "scripts/harden-linux-deb.sh"]) {
  await access(path.join(root, file), constants.R_OK);
}
const installerUat = await read("scripts/test-windows-installer.ps1");
for (const invariant of ["InstallerPath", "PythonPath", "NodeBinPath", "npm.cmd", "MCG_BIND_ADDRESS", "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--force-renderer-accessibility", "mcg-plugin-runner.exe", "mcg-shadow-runner.exe", "mcg-updater.exe", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "module_digest_mismatch", "PluginRunnerQualified", "installer-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data", "doctor --json", "backup create --json", "server/discover", "tools/list", "test-installed-experiments.py", "experiments-restart.json", "test-installed-provider-success.py", "provider-success-restart.json", "test-installed-memory-transfer.py", "memory-transfer-restart.json", "test-installed-interface-success.py", "interface-success-restart.json", "test-installed-reviewed-topology.py", "ReviewedTopologyRestartReport", "test-installed-native-reviewed-topology.py", "NativeReviewedTopologyReport", "NativeReviewedTopologyScreenshot", "test-installed-real-project.py", "RealProjectRestartReport", "test-installed-real-api-project.py", "RealApiProjectRestartReport", "test-installed-real-fullstack-project.py", "RealFullstackProjectRestartReport", "Save-DesktopScreenshot", "Get-NetTCPConnection", "ReinstallPreservedData", "Refusing installer cleanup outside the exact UAT root"]) {
  requireText(installerUat, invariant, "Windows installer UAT");
}
const linuxPackageUat = await read("scripts/test-linux-packages.sh");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-plugin-runner", "mcg-shadow-runner", "mcg-updater", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "Bubblewrap-GPL-2.0-or-later.txt", "bubblewrap-v0.11.2.tar.gz", "metacognitive-governor-unpriv-bwrap", "module_digest_mismatch", "PLUGIN_RUNNERS_QUALIFIED", "linux-package-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data", "test-installed-experiments.py", "experiments-restart.json", "test-installed-provider-success.py", "provider-success-restart.json", "test-installed-memory-transfer.py", "memory-transfer-restart.json", "test-installed-interface-success.py", "interface-success-restart.json", "test-installed-reviewed-topology.py", "REVIEWED_TOPOLOGY_RESTART_REPORT", "test-installed-native-reviewed-topology.py", "INSTALLED_NATIVE_REVIEWED_TOPOLOGY", "APPIMAGE_NATIVE_REVIEWED_TOPOLOGY", "INSTALLED_REVIEWED_TOPOLOGY_SCREENSHOT", "APPIMAGE_REVIEWED_TOPOLOGY_SCREENSHOT", "test-installed-real-project.py", "REAL_PROJECT_RESTART_REPORT", "test-installed-real-api-project.py", "REAL_API_PROJECT_RESTART_REPORT", "test-installed-real-fullstack-project.py", "REAL_FULLSTACK_PROJECT_RESTART_REPORT", "--appimage-extract", "protocolVersion", "xdotool", "import -window", "screenshot_colors"]) {
  requireText(linuxPackageUat, invariant, "Linux package UAT");
}
const macosPackageUat = await read("scripts/test-macos-packages.sh");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-plugin-runner", "mcg-shadow-runner", "mcg-updater", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "module_digest_mismatch", "PLUGIN_RUNNERS_QUALIFIED", "macos-package-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data", "test-installed-experiments.py", "experiments-restart.json", "test-installed-provider-success.py", "provider-success-restart.json", "test-installed-memory-transfer.py", "memory-transfer-restart.json", "test-installed-interface-success.py", "interface-success-restart.json", "test-installed-reviewed-topology.py", "REVIEWED_TOPOLOGY_RESTART_REPORT", "test-installed-real-project.py", "REAL_PROJECT_RESTART_REPORT", "test-installed-real-api-project.py", "REAL_API_PROJECT_RESTART_REPORT", "test-installed-real-fullstack-project.py", "REAL_FULLSTACK_PROJECT_RESTART_REPORT", "test-installed-native-accessibility-macos.swift", "NATIVE_ACCESSIBILITY_REPORT", "xcrun swift", "npm", "hdiutil verify", "codesign --verify", "MCG_MACOS_UAT_ALLOW_ADHOC", "MCG_MACOS_UAT_REQUIRE_NOTARIZED", "MCG_MACOS_UAT_EXTERNAL_CAPTURE_REQUEST", "MCG_MACOS_UAT_EXTERNAL_CAPTURE_TIMEOUT_SECONDS", "Refusing to replace external capture request", "pwd -P", "protocolVersion", "screencapture", "remove_installed_app"]) {
  requireText(macosPackageUat, invariant, "macOS package UAT");
}
const releaseSourceCheck = await read("scripts/check-release-source.mjs");
requireText(releaseSourceCheck, "--untracked-files=all", "release source preflight");
const unattendedSoakLauncher = await read("scripts/run-release-soak-unattended.mjs");
for (const invariant of ["expected-harness-sha256", "expected-daemon-sha256", "bounded regular non-symlink file", "--output must not already exist", 'flag: "wx"', "completion.json", "stdout_sha256", "stderr_sha256", "spawn_error_code", "MCG_SOAK_WINDOWS_COMMAND_BROKER", "await rename(temporaryCompletion, completionPath)"]) {
  requireText(unattendedSoakLauncher, invariant, "unattended release-soak launcher");
}
const unattendedSoakLauncherTest = await read("scripts/test-release-soak-unattended.mjs");
for (const invariant of ["hash binding", "successCompletion", "failureCompletion", '"0".repeat(64)', "reused", "refusing to remove unexpected soak-launcher fixture root"]) {
  requireText(unattendedSoakLauncherTest, invariant, "unattended release-soak launcher fixture");
}
const soakEvidenceVerifier = await read("scripts/verify-release-soak-evidence.mjs");
for (const invariant of ["canonical non-symlink directory", "bounded regular non-symlink file", "completion evidence", "successful soak evidence must have empty stderr", "timestamps and elapsed_ms diverge", "recovered_tasks", "database_integrity", "schema_compatibility", "reported RSS growth is inconsistent", "completion_sha256"]) {
  requireText(soakEvidenceVerifier, invariant, "release-soak evidence verifier");
}
const soakEvidenceVerifierTest = await read("scripts/test-verify-release-soak-evidence.mjs");
for (const invariant of ["tampered", "stderr-present", "completion.success = false", "privacy", "refusing to remove unexpected soak-evidence fixture root"]) {
  requireText(soakEvidenceVerifierTest, invariant, "release-soak evidence verifier fixture");
}
const updaterConfigCheck = await read("scripts/check-updater-config.mjs");
for (const invariant of ["MCG_UPDATER_ENDPOINT", "MCG_UPDATER_PUBKEY", "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PATH", "exactly one of", "contains a file path", "bounded regular non-symlink file", "https:", "minisign public key"]) {
  requireText(updaterConfigCheck, invariant, "signed updater configuration preflight");
}
requireText(windowsBuilder, "check-release-source.mjs", "Windows release builder");
requireText(linuxBuilder, "check-release-source.mjs", "Linux release builder");
requireText(macosBuilder, "check-release-source.mjs", "macOS release builder");

await access(path.join(root, "scripts/test-linux-packages.sh"), constants.R_OK);
await access(path.join(root, "scripts/test-macos-packages.sh"), constants.R_OK);
const installedExperimentUat = await read("scripts/test-installed-experiments.py");
for (const invariant of ["evaluation_only", "mcg_experiments_inspect", "mcg_experiments_mutate", "awaiting-decision", "shadowAuthority", "benchmarkCount", "decisionCount", "os.replace"]) {
  requireText(installedExperimentUat, invariant, "installed experiment UAT");
}
const installedProviderSuccessUat = await read("scripts/test-installed-provider-success.py");
for (const invariant of ["127.0.0.1", "installed provider-backed result", "independent_evaluation_passed", "governed_project_command_per_task", "replay_side_effect_free", "provider-success-restart-replay", "token_excluded", "private_context_excluded"]) {
  requireText(installedProviderSuccessUat, invariant, "installed provider success UAT");
}
const installedMemoryTransferUat = await read("scripts/test-installed-memory-transfer.py");
for (const invariant of ["explicit_preview", "cli_transfer_created", "mcp_exact_replay", "changed_replay_failed_closed", "inbound_and_outbound_history", "encrypted_restore_survived", "exact_replay_created_no_event"]) {
  requireText(installedMemoryTransferUat, invariant, "installed memory transfer UAT");
}
const installedInterfaceSuccessUat = await read("scripts/test-installed-interface-success.py");
for (const invariant of ["direct_rest_project_discovery", "responses_completed", "chat_completed", "responses_stream_completed", "mcp_task_completed", "mcp_exact_replay", "content_free_history", "provider_required_for_mcp_replay"]) {
  requireText(installedInterfaceSuccessUat, invariant, "installed interface success UAT");
}
const installedReviewedTopologyUat = await read("scripts/test-installed-reviewed-topology.py");
for (const invariant of ["topology_schema_version", "mcg_reviewed_topology_plans", "mcg_reviewed_topology_plan_mutate", 'waiting["plan"]["state"] == "waiting"', "approval_required_and_resolved", "terminal_replay_observation_only", "stale_authority_rejected", "settings_restored", "private_path_persisted", "os.link"]) {
  requireText(installedReviewedTopologyUat, invariant, "installed reviewed-topology UAT");
}
for (const invariant of ["native_ui_mutation_pending", "topology_stage_count", "guided_autonomy_pending_native_ui", "PLAN_REASON", "PLAN_DISPLAY_NAME", "command_executions", "approval_resolved", "settings_restored", "private_path_persisted", "os.link"]) {
  requireText(nativeReviewedTopology, invariant, "installed native reviewed-topology UAT");
}
const installedRealProjectUat = await read("scripts/test-installed-real-project.py");
for (const invariant of ["seeded_regression_failed", "governed_files_written", "independent_evaluation_passed", "browser_rendered", "browser_process_sandboxed", "canonical_viewport", "audit_unchanged", "provider_required_for_replay", "real-project-restart", "token_excluded"]) {
  requireText(installedRealProjectUat, invariant, "installed real-project UAT");
}
const installedRealApiProjectUat = await read("scripts/test-installed-real-api-project.py");
for (const invariant of ["seeded_regression_failed", "governed_files_written", "independent_evaluation_passed", "runtime_http_passed", "loopback_only", "invalid_input_failed_closed", "oversized_body_failed_closed", "runtime_behavior_unchanged", "provider_required_for_replay", "token_excluded"]) {
  requireText(installedRealApiProjectUat, invariant, "installed real API project UAT");
}
const installedRealFullstackProjectUat = await read("scripts/test-installed-real-fullstack-project.py");
for (const invariant of ["seeded_regression_failed", "governed_files_written", "independent_evaluation_passed", "browser_quick_add_passed", "persistent_after_app_restart", "invalid_input_failed_closed", "oversized_body_failed_closed", "listener_teardown_passed", "runtime_behavior_unchanged", "provider_required_for_replay", "token_excluded"]) {
  requireText(installedRealFullstackProjectUat, invariant, "installed real full-stack project UAT");
}

console.log("Windows, Linux, and macOS installer, sidecar, data-retention, and license packaging contracts are valid.");
