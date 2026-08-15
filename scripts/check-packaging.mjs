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
const config = JSON.parse(await read("apps/desktop/src-tauri/tauri.release.conf.json"));
if (config.bundle?.active !== true || config.bundle?.targets !== "all") {
  throw new Error("Tauri production bundling must remain active for every native platform target.");
}
const expectedSidecars = ["binaries/mcg-service", "binaries/mcg", "binaries/mcg-plugin-runner", "binaries/mcg-shadow-runner"];
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

const [windowsBuilder, linuxBuilder, macosBuilder, macosDmgBuilder, windowsPrepare, linuxPrepare, notices, apache, wasmtime] = await Promise.all([
  read("scripts/build-release.ps1"),
  read("scripts/build-release.sh"),
  read("scripts/build-release-macos.sh"),
  read("scripts/create-macos-dmg.sh"),
  read("scripts/prepare-sidecars.ps1"),
  read("scripts/prepare-sidecars.sh"),
  read("THIRD_PARTY_NOTICES.md"),
  read("licenses/Apache-2.0.txt"),
  read("licenses/Wasmtime-Apache-2.0-WITH-LLVM-exception.txt")
]);
requireText(windowsBuilder, "--bundles nsis", "Windows release builder");
requireText(linuxBuilder, "--bundles deb,appimage", "Linux release builder");
requireText(macosBuilder, "--bundles app", "macOS release builder");
requireText(macosBuilder, "create-macos-dmg.sh", "macOS release builder");
for (const invariant of ["hdiutil create", "hdiutil verify", "staging/MCG.app", "staging/Applications", "Refusing to replace existing disk image", "macOS disk images must be created on macOS"]) {
  requireText(macosDmgBuilder, invariant, "macOS disk-image builder");
}
requireText(windowsBuilder, "tauri.release.conf.json", "Windows release builder");
requireText(linuxBuilder, "tauri.release.conf.json", "Linux release builder");
requireText(macosBuilder, "tauri.release.conf.json", "macOS release builder");
for (const [source, label] of [[windowsPrepare, "Windows sidecar preparer"], [linuxPrepare, "Linux sidecar preparer"]]) {
  for (const binary of ["mcg", "mcg-daemon", "mcg-service", "mcg-plugin-runner", "mcg-shadow-runner"]) requireText(source, binary, label);
  requireText(source, "rustc -vV", label);
}
requireText(notices, "OpenAI Codex", "third-party notices");
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

for (const file of ["scripts/build-release.ps1", "scripts/build-release.sh", "scripts/build-release-macos.sh", "scripts/create-macos-dmg.sh", "scripts/prepare-sidecars.ps1", "scripts/prepare-sidecars.sh"]) {
  await access(path.join(root, file), constants.R_OK);
}
const installerUat = await read("scripts/test-windows-installer.ps1");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-plugin-runner.exe", "mcg-shadow-runner.exe", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "module_digest_mismatch", "PluginRunnerQualified", "installer-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data"]) {
  requireText(installerUat, invariant, "Windows installer UAT");
}
const linuxPackageUat = await read("scripts/test-linux-packages.sh");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-plugin-runner", "mcg-shadow-runner", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "module_digest_mismatch", "PLUGIN_RUNNERS_QUALIFIED", "linux-package-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data", "--appimage-extract", "protocolVersion", "xdotool", "import -window", "screenshot_colors"]) {
  requireText(linuxPackageUat, invariant, "Linux package UAT");
}
const macosPackageUat = await read("scripts/test-macos-packages.sh");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-plugin-runner", "mcg-shadow-runner", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt", "module_digest_mismatch", "PLUGIN_RUNNERS_QUALIFIED", "macos-package-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data", "hdiutil verify", "codesign --verify", "MCG_MACOS_UAT_ALLOW_ADHOC", "MCG_MACOS_UAT_REQUIRE_NOTARIZED", "protocolVersion", "screencapture", "remove_installed_app"]) {
  requireText(macosPackageUat, invariant, "macOS package UAT");
}
const releaseSourceCheck = await read("scripts/check-release-source.mjs");
requireText(releaseSourceCheck, "--untracked-files=all", "release source preflight");
requireText(windowsBuilder, "check-release-source.mjs", "Windows release builder");
requireText(linuxBuilder, "check-release-source.mjs", "Linux release builder");
requireText(macosBuilder, "check-release-source.mjs", "macOS release builder");

await access(path.join(root, "scripts/test-linux-packages.sh"), constants.R_OK);
await access(path.join(root, "scripts/test-macos-packages.sh"), constants.R_OK);

console.log("Windows, Linux, and macOS installer, sidecar, data-retention, and license packaging contracts are valid.");
