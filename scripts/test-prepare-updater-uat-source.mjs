import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { gunzipSync, gzipSync } from "node:zlib";

const root = path.resolve(import.meta.dirname, "..");
const [archiveArgument] = process.argv.slice(2);
if (process.argv.length > 3) {
  throw new Error("usage: node scripts/test-prepare-updater-uat-source.mjs [ARCHIVE.tar.gz]");
}
const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "mcg-updater-uat-source-"));
const archive = archiveArgument ? path.resolve(archiveArgument) : path.join(temporaryRoot, "source.tar.gz");

function run(arguments_, expectedSuccess = true) {
  const result = spawnSync(process.execPath, ["scripts/prepare-updater-uat-source.mjs", ...arguments_], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (expectedSuccess) assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
  else assert.notEqual(result.status, 0, "invalid preparation unexpectedly succeeded");
  return result;
}

function runPrepared(command, arguments_, cwd) {
  const result = spawnSync(command, arguments_, {
    cwd,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
  });
  assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
  return result.stdout;
}

async function archiveWithFirstFileChangedToSymlink(source, destination) {
  const tar = gunzipSync(await readFile(source));
  for (let offset = 0; offset + 512 <= tar.length;) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const sizeText = header.subarray(124, 136).toString("ascii").replaceAll("\0", "").trim();
    const size = Number.parseInt(sizeText || "0", 8);
    const type = String.fromCharCode(header[156]);
    if (type === "0" || type === "\0") {
      header[156] = "2".charCodeAt(0);
      header.fill(" ".charCodeAt(0), 148, 156);
      const checksum = header.reduce((sum, byte) => sum + byte, 0).toString(8).padStart(6, "0");
      header.write(checksum, 148, 6, "ascii");
      header[154] = 0;
      header[155] = " ".charCodeAt(0);
      await writeFile(destination, gzipSync(tar));
      return;
    }
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  throw new Error("fixture archive contains no regular file to mutate");
}

try {
  if (!archiveArgument) {
    runPrepared(process.platform === "win32" ? "git.exe" : "git", ["archive", "--format=tar.gz", `--output=${archive}`, "HEAD"], root);
  }
  const baseline = path.join(temporaryRoot, "baseline");
  const candidate = path.join(temporaryRoot, "candidate");
  const sourceVersion = JSON.parse(await readFile(path.join(root, "package.json"), "utf8")).version;
  const [major, minor, patchVersion] = sourceVersion.split(".").map(Number);
  assert.ok([major, minor, patchVersion].every(Number.isSafeInteger), "source version must be stable semantic version text");
  const candidateVersion = `${major}.${minor}.${patchVersion + 1}-uat.1`;
  const listed = runPrepared("tar", ["-tzf", archive], root).split(/\r?\n/u).filter(Boolean);
  assert.ok(listed.some((entry) => entry.endsWith("/")), "fixture must exercise explicit directory entries");
  const baselineResult = JSON.parse(run([archive, baseline, sourceVersion]).stdout);
  const candidateResult = JSON.parse(run([archive, candidate, candidateVersion]).stdout);
  assert.equal(baselineResult.schema_version, 1);
  assert.equal(baselineResult.archive_sha256, candidateResult.archive_sha256);
  assert.equal(baselineResult.archive_entries, candidateResult.archive_entries);
  assert.equal(baselineResult.base_version, sourceVersion);
  assert.equal(baselineResult.prepared_version, sourceVersion);
  assert.equal(candidateResult.prepared_version, candidateVersion);
  assert.match(baselineResult.commit, /^[0-9a-f]{40}$/u);
  assert.match(candidateResult.commit, /^[0-9a-f]{40}$/u);
  assert.notEqual(baselineResult.commit, candidateResult.commit);
  const candidateTauri = JSON.parse(await readFile(path.join(candidate, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"));
  const candidateNpm = JSON.parse(await readFile(path.join(candidate, "package-lock.json"), "utf8"));
  assert.equal(candidateTauri.version, candidateVersion);
  assert.equal(candidateNpm.version, candidateVersion);
  assert.equal(candidateNpm.packages["apps/desktop"].version, candidateVersion);
  const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
  const git = process.platform === "win32" ? "git.exe" : "git";
  const candidateMetadata = JSON.parse(runPrepared(cargo, ["metadata", "--offline", "--format-version", "1", "--no-deps", "--locked"], candidate));
  const candidateWorkspace = candidateMetadata.packages.filter((item) => candidateMetadata.workspace_members.includes(item.id));
  assert.ok(candidateWorkspace.length > 0);
  assert.ok(candidateWorkspace.every((item) => item.version === candidateVersion));
  const candidateCargoLock = await readFile(path.join(candidate, "Cargo.lock"), "utf8");
  for (const item of candidateWorkspace) {
    const escapedName = item.name.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
    const escapedVersion = candidateVersion.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
    assert.match(candidateCargoLock, new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${escapedName}"\\r?\\nversion = "${escapedVersion}"`, "u"));
  }
  assert.equal(runPrepared(git, ["status", "--porcelain"], candidate).trim(), "");
  run([archive, path.join(temporaryRoot, "invalid-version"), "01.1.0"], false);
  run([archive, baseline, `${major}.${minor}.${patchVersion + 2}`], false);
  const symlinkArchive = path.join(temporaryRoot, "symlink.tar.gz");
  await archiveWithFirstFileChangedToSymlink(archive, symlinkArchive);
  const symlinkResult = run([symlinkArchive, path.join(temporaryRoot, "symlink-source"), sourceVersion], false);
  assert.match(symlinkResult.stderr, /archive entry has an unsafe type/u);
  console.log("Updater UAT source preparation passed exact extraction, lockfile/version convergence, clean commits, rejection, and cleanup checks.");
} finally {
  const resolved = path.resolve(temporaryRoot);
  assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
  assert.match(path.basename(resolved), /^mcg-updater-uat-source-/u);
  await rm(resolved, { recursive: true, force: true });
}
