import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";
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
const expectedSidecars = ["binaries/mcg-service", "binaries/mcg", "binaries/mcg-shadow-runner"];
if (JSON.stringify(config.bundle.externalBin) !== JSON.stringify(expectedSidecars)) {
  throw new Error("The installer sidecar contract changed or is incomplete.");
}
for (const resource of ["../../../THIRD_PARTY_NOTICES.md", "../../../licenses/Apache-2.0.txt"]) {
  if (!config.bundle.resources?.includes(resource)) throw new Error(`Installer resource missing: ${resource}`);
}
if (config.bundle.windows?.nsis?.installMode !== "currentUser") {
  throw new Error("Windows installation must stay per-user until a privileged service design is approved.");
}

const [windowsBuilder, linuxBuilder, windowsPrepare, linuxPrepare, notices, apache] = await Promise.all([
  read("scripts/build-release.ps1"),
  read("scripts/build-release.sh"),
  read("scripts/prepare-sidecars.ps1"),
  read("scripts/prepare-sidecars.sh"),
  read("THIRD_PARTY_NOTICES.md"),
  read("licenses/Apache-2.0.txt")
]);
requireText(windowsBuilder, "--bundles nsis", "Windows release builder");
requireText(linuxBuilder, "--bundles deb,appimage", "Linux release builder");
requireText(windowsBuilder, "tauri.release.conf.json", "Windows release builder");
requireText(linuxBuilder, "tauri.release.conf.json", "Linux release builder");
for (const [source, label] of [[windowsPrepare, "Windows sidecar preparer"], [linuxPrepare, "Linux sidecar preparer"]]) {
  for (const binary of ["mcg", "mcg-daemon", "mcg-service", "mcg-shadow-runner"]) requireText(source, binary, label);
  requireText(source, "rustc -vV", label);
}
requireText(notices, "OpenAI Codex", "third-party notices");
for (const section of ["1. Definitions.", "2. Grant of Copyright License.", "9. Accepting Warranty or Additional Liability.", "END OF TERMS AND CONDITIONS"]) {
  requireText(apache, section, "Apache-2.0 license payload");
}
if (apache.length < 10_000) throw new Error("Apache-2.0 license payload appears truncated.");

for (const file of ["scripts/build-release.ps1", "scripts/build-release.sh", "scripts/prepare-sidecars.ps1", "scripts/prepare-sidecars.sh"]) {
  await access(path.join(root, file), constants.R_OK);
}
const installerUat = await read("scripts/test-windows-installer.ps1");
for (const invariant of ["MCG_BIND_ADDRESS", "mcg-shadow-runner.exe", "installer-uat-retained.txt", "Repair installation removed external MCG data", "Uninstall removed external MCG data"]) {
  requireText(installerUat, invariant, "Windows installer UAT");
}
const releaseSourceCheck = await read("scripts/check-release-source.mjs");
requireText(releaseSourceCheck, "--untracked-files=all", "release source preflight");
requireText(windowsBuilder, "check-release-source.mjs", "Windows release builder");
requireText(linuxBuilder, "check-release-source.mjs", "Linux release builder");

console.log("Installer, sidecar, per-user, and license packaging contracts are valid.");
