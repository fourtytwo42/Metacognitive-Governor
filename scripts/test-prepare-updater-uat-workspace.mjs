import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { gzipSync } from "node:zlib";

const root = path.resolve(import.meta.dirname, "..");
const [archiveArgument] = process.argv.slice(2);
if (process.argv.length > 3) throw new Error("usage: test-prepare-updater-uat-workspace.mjs [ARCHIVE.tar.gz]");
const temporaryParent = await mkdtemp(path.join(os.tmpdir(), "mcg-updater-uat-workspace-test-"));
const archive = archiveArgument ? path.resolve(archiveArgument) : path.join(temporaryParent, "source.tar.gz");
const workspace = path.join(temporaryParent, "workspace");

function runScript(script, arguments_, success = true, cwd = root) {
  const result = spawnSync(process.execPath, [script, ...arguments_], {
    cwd,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (success) assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
  else assert.notEqual(result.status, 0, "invalid workspace preparation unexpectedly succeeded");
  return result;
}

function run(arguments_, success = true) {
  return runScript(path.join(root, "scripts/prepare-updater-uat-workspace.mjs"), arguments_, success);
}

function runIn(command, arguments_, cwd) {
  const result = spawnSync(command, arguments_, { cwd, encoding: "utf8", windowsHide: true });
  assert.equal(result.status, 0, `${result.stderr}\n${result.stdout}`);
  return result.stdout.trim();
}

async function createArchiveFixture() {
  const rawArchive = path.join(temporaryParent, "source.tar");
  const git = process.platform === "win32" ? "git.exe" : "git";
  runIn(git, ["archive", "--format=tar", `--output=${rawArchive}`, "HEAD"], root);
  const requiredHelpers = [
    "scripts/prepare-updater-uat-source.mjs",
    "scripts/generate-updater-uat-tls.py",
  ];
  const missing = requiredHelpers.filter((file) => runIn(git, ["ls-tree", "-r", "--name-only", "HEAD", "--", file], root) !== file);
  if (missing.length > 0) runIn("tar", ["-rf", rawArchive, ...missing], root);
  await writeFile(archive, gzipSync(await readFile(rawArchive)), { flag: "wx" });
}

try {
  if (!archiveArgument) await createArchiveFixture();
  const result = JSON.parse(run([archive, workspace, "0.1.0", "0.1.1-uat.1", "https://127.0.0.1:18443/"]).stdout);
  assert.equal(result.schema_version, 1);
  assert.equal(result.workspace, workspace);
  const publicDescriptor = JSON.parse(await readFile(result.public_descriptor, "utf8"));
  const privateDescriptor = JSON.parse(await readFile(result.private_descriptor, "utf8"));
  assert.equal(publicDescriptor.schema_version, 1);
  assert.equal(publicDescriptor.baseline.version, "0.1.0");
  assert.equal(publicDescriptor.candidate.version, "0.1.1-uat.1");
  assert.equal(publicDescriptor.updater_endpoint, "https://127.0.0.1:18443/latest.json");
  assert.match(publicDescriptor.archive_sha256, /^[0-9a-f]{64}$/u);
  assert.match(publicDescriptor.updater_public_key_sha256, /^[0-9a-f]{64}$/u);
  assert.match(publicDescriptor.source_preparer_sha256, /^[0-9a-f]{64}$/u);
  assert.match(publicDescriptor.tls_generator_sha256, /^[0-9a-f]{64}$/u);
  assert.equal(JSON.stringify(publicDescriptor).includes("private"), false);
  assert.equal(JSON.stringify(publicDescriptor).includes(workspace), false);
  await assert.rejects(access(path.join(workspace, "bootstrap")));
  const git = process.platform === "win32" ? "git.exe" : "git";
  for (const [prepared, descriptor, version] of [
    [privateDescriptor.baseline_source, publicDescriptor.baseline, "0.1.0"],
    [privateDescriptor.candidate_source, publicDescriptor.candidate, "0.1.1-uat.1"],
  ]) {
    assert.equal(runIn(git, ["rev-parse", "HEAD"], prepared), descriptor.commit);
    assert.equal(runIn(git, ["status", "--porcelain"], prepared), "");
    const locked = runIn(git, ["show", "HEAD:Cargo.lock"], prepared);
    const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
    assert.match(locked, new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "mcg-agents"\\r?\\nversion = "${escapedVersion}"`, "u"));
  }
  for (const file of [
    privateDescriptor.tls_ca_certificate,
    privateDescriptor.tls_server_certificate,
    privateDescriptor.tls_server_key,
    privateDescriptor.updater_private_key,
    privateDescriptor.updater_public_key,
  ]) {
    assert((await lstat(file)).isFile());
  }
  run([archive, workspace, "0.1.0", "0.1.2", "https://127.0.0.1:18443/"], false);
  run([archive, path.join(temporaryParent, "bad-version"), "0.1.1", "0.1.0", "https://127.0.0.1:18443/"], false);
  run([archive, path.join(temporaryParent, "bad-origin"), "0.1.0", "0.1.1", "https://updates.example.test/"], false);

  const legacyHarness = path.join(temporaryParent, "legacy-harness");
  const legacyScripts = path.join(legacyHarness, "scripts");
  await mkdir(legacyScripts, { recursive: true });
  const workspaceComposer = path.join(legacyScripts, "prepare-updater-uat-workspace.mjs");
  await writeFile(workspaceComposer, await readFile(path.join(root, "scripts/prepare-updater-uat-workspace.mjs")), { flag: "wx" });
  const realPreparer = path.join(root, "scripts/prepare-updater-uat-source.mjs");
  await writeFile(
    path.join(legacyScripts, "prepare-updater-uat-source.mjs"),
    `import { spawnSync } from "node:child_process";\nimport process from "node:process";\nconst args = process.argv.slice(2);\nif (args[2] !== "0.1.0") process.exit(91);\nconst result = spawnSync(process.execPath, [${JSON.stringify(realPreparer)}, ...args], { encoding: "utf8", windowsHide: true, maxBuffer: 32 * 1024 * 1024 });\nprocess.stdout.write(result.stdout ?? "");\nprocess.stderr.write(result.stderr ?? "");\nprocess.exit(result.status ?? 1);\n`,
    { flag: "wx" },
  );
  const legacyWorkspace = path.join(temporaryParent, "legacy-workspace");
  const legacyResult = JSON.parse(runScript(
    workspaceComposer,
    [archive, legacyWorkspace, "0.1.0", "0.1.1-uat.1", "https://127.0.0.1:18444/"],
    true,
    legacyHarness,
  ).stdout);
  const legacyPrivate = JSON.parse(await readFile(legacyResult.private_descriptor, "utf8"));
  assert.equal(runIn(git, ["status", "--porcelain"], legacyPrivate.candidate_source), "");
  assert.match(
    runIn(git, ["show", "HEAD:Cargo.lock"], legacyPrivate.candidate_source),
    /\[\[package\]\]\r?\nname = "mcg-agents"\r?\nversion = "0\.1\.1-uat\.1"/u,
  );
  console.log("Disposable updater UAT workspace passed archive-bound helper provenance, committed locks, TLS/signing isolation, privacy, rejection, and cleanup checks.");
} finally {
  const resolved = path.resolve(temporaryParent);
  assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
  assert.match(path.basename(resolved), /^mcg-updater-uat-workspace-test-/u);
  await rm(resolved, { recursive: true, force: true });
}
