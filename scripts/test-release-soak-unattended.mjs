import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import process from "node:process";
import assert from "node:assert/strict";

const launcher = resolve(import.meta.dirname, "run-release-soak-unattended.mjs");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const temporaryRoot = await mkdtemp(join(tmpdir(), "mcg-soak-launcher-test-"));

function invoke({ harness, daemon, output, harnessSha256, daemonSha256 }) {
  return spawnSync(process.execPath, [
    launcher,
    "--harness", harness,
    "--daemon", daemon,
    "--output", output,
    "--expected-harness-sha256", harnessSha256,
    "--expected-daemon-sha256", daemonSha256,
    "--duration-seconds", "5",
    "--tasks", "10",
    "--concurrency", "2",
  ], { encoding: "utf8", windowsHide: true });
}

try {
  const daemon = join(temporaryRoot, process.platform === "win32" ? "mcg-daemon.exe" : "mcg-daemon");
  const daemonBytes = Buffer.from("sealed unattended soak daemon fixture\n", "utf8");
  await writeFile(daemon, daemonBytes, { flag: "wx", mode: 0o700 });
  const daemonSha256 = sha256(daemonBytes);

  const successDirectory = join(temporaryRoot, "success-fixture");
  await mkdir(successDirectory);
  const successHarness = join(successDirectory, "test-release-soak.mjs");
  const successSource = `
    import process from "node:process";
    const expected = ${JSON.stringify(daemon)};
    const expectedArguments = ["--duration-seconds", "5", "--tasks", "10", "--concurrency", "2"];
    if (process.env.MCG_SOAK_DAEMON !== expected) throw new Error("daemon environment mismatch");
    const broker = process.env.MCG_SOAK_WINDOWS_COMMAND_BROKER;
    if (process.platform === "win32" ? broker !== "1" : broker !== undefined) {
      throw new Error("Windows command-broker environment mismatch");
    }
    if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(expectedArguments)) {
      throw new Error("soak arguments mismatch");
    }
    console.log(JSON.stringify({ fixture: "pass" }));
    console.error("fixture-stderr");
  `.trimStart();
  await writeFile(successHarness, successSource, { flag: "wx", mode: 0o600 });
  const successOutput = join(temporaryRoot, "success-output");
  const success = invoke({
    harness: successHarness,
    daemon,
    output: successOutput,
    harnessSha256: sha256(Buffer.from(successSource)),
    daemonSha256,
  });
  assert.equal(success.status, 0, success.stderr || success.stdout);
  const successCompletionText = await readFile(join(successOutput, "completion.json"), "utf8");
  const successCompletion = JSON.parse(successCompletionText);
  assert.equal(successCompletion.schema_version, 1);
  assert.equal(successCompletion.success, true);
  assert.equal(successCompletion.exit_code, 0);
  assert.equal(successCompletion.signal, null);
  assert.equal(successCompletion.spawn_error_code, null);
  assert.equal(successCompletion.harness_sha256, sha256(Buffer.from(successSource)));
  assert.equal(successCompletion.daemon_sha256, daemonSha256);
  assert.equal(await readFile(join(successOutput, "soak.stdout.log"), "utf8"), '{"fixture":"pass"}\n');
  assert.equal(await readFile(join(successOutput, "soak.stderr.log"), "utf8"), "fixture-stderr\n");
  for (const forbidden of ["auth_token", "access_token", "refresh_token", "password", "private_key", temporaryRoot]) {
    assert.equal(successCompletionText.includes(forbidden), false, `completion evidence contains ${forbidden}`);
  }

  const failureDirectory = join(temporaryRoot, "failure-fixture");
  await mkdir(failureDirectory);
  const failureHarness = join(failureDirectory, "test-release-soak.mjs");
  const failureSource = 'console.error("intentional fixture failure"); process.exit(7);\n';
  await writeFile(failureHarness, failureSource, { flag: "wx", mode: 0o600 });
  const failureOutput = join(temporaryRoot, "failure-output");
  const failure = invoke({
    harness: failureHarness,
    daemon,
    output: failureOutput,
    harnessSha256: sha256(Buffer.from(failureSource)),
    daemonSha256,
  });
  assert.equal(failure.status, 7);
  const failureCompletion = JSON.parse(await readFile(join(failureOutput, "completion.json"), "utf8"));
  assert.equal(failureCompletion.success, false);
  assert.equal(failureCompletion.exit_code, 7);
  assert.match(await readFile(join(failureOutput, "soak.stderr.log"), "utf8"), /intentional fixture failure/);

  const rejectedOutput = join(temporaryRoot, "rejected-output");
  const rejected = invoke({
    harness: successHarness,
    daemon,
    output: rejectedOutput,
    harnessSha256: "0".repeat(64),
    daemonSha256,
  });
  assert.notEqual(rejected.status, 0);
  await assert.rejects(realpath(rejectedOutput), { code: "ENOENT" });

  const reused = invoke({
    harness: successHarness,
    daemon,
    output: successOutput,
    harnessSha256: sha256(Buffer.from(successSource)),
    daemonSha256,
  });
  assert.notEqual(reused.status, 0);
  assert.deepEqual(JSON.parse(await readFile(join(successOutput, "completion.json"), "utf8")), successCompletion);

  console.log("Unattended release-soak launcher passed hash binding, success/failure markers, privacy, and no-clobber tests.");
} finally {
  const physical = await realpath(temporaryRoot);
  const expectedParent = await realpath(tmpdir());
  if (dirname(physical) !== expectedParent || !/^mcg-soak-launcher-test-[A-Za-z0-9_-]+$/.test(basename(physical))) {
    throw new Error(`refusing to remove unexpected soak-launcher fixture root: ${physical}`);
  }
  await rm(physical, { recursive: true, force: true });
}
