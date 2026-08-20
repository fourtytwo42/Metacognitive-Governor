import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import {
  chmod,
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const MAX_ARCHIVE_BYTES = 128 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES = 4_096;
const MAX_FILE_BYTES = 4 * 1024 * 1024;
const REQUIRED_FILES = [
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
  "package-lock.json",
  "apps/desktop/package.json",
  "apps/desktop/src-tauri/tauri.conf.json",
];
const EXECUTABLE_FILES = [
  "packaging/linux/portable/Start-MCG.sh",
  "scripts/build-release-macos.sh",
  "scripts/build-release.sh",
  "scripts/create-macos-dmg.sh",
  "scripts/prepare-sidecars.sh",
  "scripts/test-linux-packages.sh",
  "scripts/test-macos-packages.sh",
  "scripts/verify.sh",
];

function fail(message) {
  throw new Error(`Updater UAT source preparation rejected: ${message}`);
}

function canonicalSemver(value, label) {
  if (typeof value !== "string" || value.length > 128) fail(`${label} is not bounded text`);
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/u.exec(value);
  if (!match) fail(`${label} is not canonical semantic version text`);
  if (match[4]?.split(".").some((part) => /^\d+$/u.test(part) && part.length > 1 && part.startsWith("0"))) {
    fail(`${label} has a noncanonical numeric prerelease identifier`);
  }
  return value;
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
    ...options,
  });
  if (result.status !== 0) {
    const detail = `${result.stderr ?? ""}\n${result.stdout ?? ""}`.trim().slice(0, 4_000);
    fail(`${path.basename(command)} failed with status ${result.status}: ${detail}`);
  }
  return result.stdout;
}

function archiveListing(archive, verbose) {
  const result = spawnSync("tar", [verbose ? "-tvzf" : "-tzf", archive], {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
  });
  const lines = (result.stdout ?? "").split(/\r?\n/u).filter(Boolean);
  if (result.status !== 0) {
    if (verbose && lines.some((line) => line[0] !== "-" && line[0] !== "d")) {
      fail("archive entry has an unsafe type");
    }
    const detail = `${result.stderr ?? ""}\n${result.stdout ?? ""}`.trim().slice(0, 4_000);
    fail(`tar rejected the archive listing with status ${result.status}: ${detail}`);
  }
  return lines;
}

async function regularFile(value, label, maximum) {
  if (typeof value !== "string" || !path.isAbsolute(value) || value.includes("\0")) {
    fail(`${label} must be an absolute path`);
  }
  const metadata = await lstat(value);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size === 0 || metadata.size > maximum) {
    fail(`${label} must be a bounded regular non-symlink file`);
  }
  return realpath(value);
}

async function sha256(file) {
  const digest = createHash("sha256");
  for await (const chunk of createReadStream(file)) digest.update(chunk);
  return digest.digest("hex");
}

function archiveEntries(archive) {
  const verbose = archiveListing(archive, true);
  const names = archiveListing(archive, false);
  if (names.length === 0 || names.length > MAX_ARCHIVE_ENTRIES || verbose.length !== names.length) {
    fail("archive entry count is invalid");
  }
  const seen = new Set();
  const files = [];
  for (let index = 0; index < names.length; index += 1) {
    const name = names[index];
    const type = verbose[index][0];
    const isFile = type === "-";
    const isDirectory = type === "d";
    if (!isFile && !isDirectory) fail(`archive entry has an unsafe type: ${name}`);
    if ((isDirectory && !name.endsWith("/")) || (isFile && name.endsWith("/"))) {
      fail(`archive entry type and path disagree: ${name}`);
    }
    const canonicalName = isDirectory ? name.slice(0, -1) : name;
    if (
      !canonicalName ||
      canonicalName.includes("\\") ||
      canonicalName.includes("\0") ||
      path.posix.isAbsolute(canonicalName) ||
      /^[A-Za-z]:/u.test(canonicalName)
    ) {
      fail(`archive entry path is unsafe: ${name}`);
    }
    const parts = canonicalName.split("/");
    if (parts.some((part) => !part || part === "." || part === "..")) {
      fail(`archive entry path is noncanonical: ${name}`);
    }
    if (path.posix.normalize(canonicalName) !== canonicalName || seen.has(canonicalName)) {
      fail(`archive entry is duplicate or noncanonical: ${name}`);
    }
    seen.add(canonicalName);
    if (isFile) files.push(canonicalName);
  }
  for (const required of [...REQUIRED_FILES, ...EXECUTABLE_FILES]) {
    if (!files.includes(required)) fail(`archive is missing ${required}`);
  }
  return files;
}

async function inspectExtractedTree(directory, expectedFiles) {
  let files = 0;
  async function visit(current) {
    for (const entry of await readdir(current, { withFileTypes: true })) {
      const candidate = path.join(current, entry.name);
      const metadata = await lstat(candidate);
      if (metadata.isSymbolicLink()) fail(`extracted tree contains a symbolic link: ${candidate}`);
      if (metadata.isDirectory()) await visit(candidate);
      else if (metadata.isFile()) files += 1;
      else fail(`extracted tree contains a special file: ${candidate}`);
    }
  }
  await visit(directory);
  if (files !== expectedFiles) fail(`extracted file count changed from ${expectedFiles} to ${files}`);
}

async function boundedText(file, label) {
  const bytes = await readFile(file);
  if (bytes.length === 0 || bytes.length > MAX_FILE_BYTES) fail(`${label} is empty or oversized`);
  return bytes.toString("utf8");
}

async function jsonFile(file, label) {
  const text = await boundedText(file, label);
  try {
    return JSON.parse(text);
  } catch {
    fail(`${label} is not valid UTF-8 JSON`);
  }
}

const [archiveArgument, destinationArgument, versionArgument] = process.argv.slice(2);
if (!archiveArgument || !destinationArgument || !versionArgument || process.argv.length !== 5) {
  fail("usage: node scripts/prepare-updater-uat-source.mjs ARCHIVE.tar.gz ABSOLUTE_DESTINATION VERSION");
}
const archive = await regularFile(path.resolve(archiveArgument), "archive", MAX_ARCHIVE_BYTES);
const version = canonicalSemver(versionArgument, "target version");
if (!path.isAbsolute(destinationArgument) || destinationArgument.includes("\0")) {
  fail("destination must be an absolute new path");
}
const destination = path.resolve(destinationArgument);
const destinationParent = await realpath(path.dirname(destination));
if (path.dirname(destination) !== destinationParent || destination === destinationParent) {
  fail("destination parent must be canonical and distinct");
}
try {
  await lstat(destination);
  fail("destination already exists");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}
const entries = archiveEntries(archive);
let created = false;
try {
  await mkdir(destination, { mode: 0o700 });
  created = true;
  run("tar", ["-xzf", archive, "-C", destination]);
  await inspectExtractedTree(destination, entries.length);
  for (const executable of EXECUTABLE_FILES) {
    if (process.platform !== "win32") await chmod(path.join(destination, executable), 0o755);
  }

  const cargoManifestPath = path.join(destination, "Cargo.toml");
  const cargoManifest = await boundedText(cargoManifestPath, "Cargo.toml");
  const workspaceVersion = /(\[workspace\.package\]\r?\nversion = ")([^"]+)(")/u;
  const match = workspaceVersion.exec(cargoManifest);
  if (!match || cargoManifest.match(new RegExp(workspaceVersion.source, "gu"))?.length !== 1) {
    fail("Cargo.toml must contain one workspace package version");
  }
  const baseVersion = canonicalSemver(match[2], "source version");
  await writeFile(cargoManifestPath, cargoManifest.replace(workspaceVersion, `$1${version}$3`), "utf8");

  const rootPackagePath = path.join(destination, "package.json");
  const desktopPackagePath = path.join(destination, "apps/desktop/package.json");
  const tauriConfigPath = path.join(destination, "apps/desktop/src-tauri/tauri.conf.json");
  const lockPath = path.join(destination, "package-lock.json");
  const cargoLockPath = path.join(destination, "Cargo.lock");
  const rootPackage = await jsonFile(rootPackagePath, "root package manifest");
  const desktopPackage = await jsonFile(desktopPackagePath, "desktop package manifest");
  const tauriConfig = await jsonFile(tauriConfigPath, "Tauri configuration");
  const lock = await jsonFile(lockPath, "npm lockfile");
  if (
    rootPackage.version !== baseVersion ||
    desktopPackage.version !== baseVersion ||
    tauriConfig.version !== baseVersion ||
    lock.version !== baseVersion ||
    lock.packages?.[""]?.version !== baseVersion ||
    lock.packages?.["apps/desktop"]?.version !== baseVersion
  ) {
    fail("source version surfaces do not agree");
  }
  rootPackage.version = version;
  desktopPackage.version = version;
  tauriConfig.version = version;
  lock.version = version;
  lock.packages[""].version = version;
  lock.packages["apps/desktop"].version = version;
  for (const [file, value] of [
    [rootPackagePath, rootPackage],
    [desktopPackagePath, desktopPackage],
    [tauriConfigPath, tauriConfig],
    [lockPath, lock],
  ]) {
    await writeFile(file, `${JSON.stringify(value, null, 2)}\n`, { encoding: "utf8" });
  }

  const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
  const unlockedMetadata = JSON.parse(run(cargo, ["metadata", "--offline", "--format-version", "1", "--no-deps"], { cwd: destination }));
  const unlockedWorkspacePackages = unlockedMetadata.packages.filter((item) => unlockedMetadata.workspace_members.includes(item.id));
  if (unlockedWorkspacePackages.length === 0 || unlockedWorkspacePackages.some((item) => item.version !== version)) {
    fail("Cargo metadata did not converge every workspace package on the target version");
  }
  const workspaceNames = new Set(unlockedWorkspacePackages.map((item) => item.name));
  if (workspaceNames.size !== unlockedWorkspacePackages.length) fail("workspace package names are not unique");
  const cargoLock = await boundedText(cargoLockPath, "Cargo.lock");
  const seenWorkspaceLocks = new Set();
  const rewrittenCargoLock = cargoLock.replace(
    /(^|\n)(\[\[package\]\]\r?\nname = "([^"\r\n]+)"\r?\nversion = ")([^"\r\n]+)(")/gu,
    (block, prefix, header, name, lockedVersion, suffix) => {
      if (!workspaceNames.has(name)) return block;
      if (seenWorkspaceLocks.has(name)) fail(`Cargo.lock repeats workspace package ${name}`);
      if (lockedVersion !== baseVersion && lockedVersion !== version) {
        fail(`Cargo.lock has an unexpected source version for workspace package ${name}`);
      }
      seenWorkspaceLocks.add(name);
      return `${prefix}${header}${version}${suffix}`;
    },
  );
  if (seenWorkspaceLocks.size !== workspaceNames.size) fail("Cargo.lock is missing a workspace package");
  await writeFile(cargoLockPath, rewrittenCargoLock, "utf8");
  const metadata = JSON.parse(run(cargo, ["metadata", "--offline", "--format-version", "1", "--no-deps", "--locked"], { cwd: destination }));
  const workspacePackages = metadata.packages.filter((item) => metadata.workspace_members.includes(item.id));
  if (workspacePackages.length === 0 || workspacePackages.some((item) => item.version !== version)) {
    fail("Cargo metadata did not converge every workspace package on the target version");
  }

  const git = process.platform === "win32" ? "git.exe" : "git";
  run(git, ["init", "--initial-branch=main", "--quiet"], { cwd: destination });
  run(git, ["config", "user.name", "MCG qualification"], { cwd: destination });
  run(git, ["config", "user.email", "qualification@invalid"], { cwd: destination });
  if (process.platform === "win32") run(git, ["config", "core.filemode", "false"], { cwd: destination });
  run(git, ["add", "-A"], { cwd: destination });
  run(git, ["commit", "--quiet", "-m", `Updater UAT ${version}`], { cwd: destination });
  const status = run(git, ["status", "--porcelain"], { cwd: destination });
  if (status.trim()) fail("prepared source is not clean after commit");
  const commit = run(git, ["rev-parse", "HEAD"], { cwd: destination }).trim();
  assert.match(commit, /^[0-9a-f]{40}$/u);
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    archive_sha256: await sha256(archive),
    archive_entries: entries.length,
    base_version: baseVersion,
    prepared_version: version,
    commit,
  })}\n`);
} catch (error) {
  if (created) await rm(destination, { recursive: true, force: true });
  throw error;
}
