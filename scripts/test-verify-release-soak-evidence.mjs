import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import process from "node:process";

const verifier = resolve(import.meta.dirname, "verify-release-soak-evidence.mjs");
const temporaryRoot = await mkdtemp(join(tmpdir(), "mcg-soak-evidence-test-"));
const harnessSha256 = "1".repeat(64);
const daemonSha256 = "2".repeat(64);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

function invoke(directory) {
  return spawnSync(process.execPath, [
    verifier,
    "--evidence-dir", directory,
    "--expected-harness-sha256", harnessSha256,
    "--expected-daemon-sha256", daemonSha256,
    "--duration-seconds", "5",
    "--tasks", "10",
    "--concurrency", "2",
  ], { encoding: "utf8", windowsHide: true });
}

async function writeEvidence(directory, mutate = () => {}) {
  await mkdir(directory);
  const stdout = Buffer.from(`${JSON.stringify({
    schema_version: 1,
    daemon: process.platform === "win32" ? "mcg-daemon.exe" : "mcg-daemon",
    task_count: 10,
    duration_seconds: 5,
    concurrency: 2,
    cold_startup_ms: 100,
    restart_ms: 120,
    throughput_per_second: 100,
    task_creation_p95_ms: 20,
    baseline_rss_mib: 10,
    peak_rss_mib: 12,
    rss_growth_mib: 2,
    recovered_tasks: 10,
    recovered_events: 10,
    database_integrity: "pass",
    schema_compatibility: "pass",
  }, null, 2)}\n`, "utf8");
  const stderr = Buffer.alloc(0);
  const startedAt = new Date(Date.now() - 5_100);
  const completion = {
    schema_version: 1,
    started_at: startedAt.toISOString(),
    completed_at: new Date(startedAt.getTime() + 5_100).toISOString(),
    elapsed_ms: 5_100,
    duration_seconds: 5,
    tasks: 10,
    concurrency: 2,
    harness_sha256: harnessSha256,
    daemon_sha256: daemonSha256,
    stdout_sha256: sha256(stdout),
    stderr_sha256: sha256(stderr),
    exit_code: 0,
    signal: null,
    spawn_error_code: null,
    success: true,
  };
  mutate({ completion, stdout, stderr });
  await writeFile(join(directory, "soak.stdout.log"), stdout, { flag: "wx" });
  await writeFile(join(directory, "soak.stderr.log"), stderr, { flag: "wx" });
  await writeFile(join(directory, "completion.json"), `${JSON.stringify(completion, null, 2)}\n`, { flag: "wx" });
}

try {
  const valid = join(temporaryRoot, "valid");
  await writeEvidence(valid);
  const accepted = invoke(valid);
  assert.equal(accepted.status, 0, accepted.stderr);
  const summary = JSON.parse(accepted.stdout);
  assert.equal(summary.verified, true);
  assert.equal(summary.daemon_sha256, daemonSha256);
  assert.equal(summary.recovered_tasks, 10);
  for (const forbidden of [temporaryRoot, "auth_token", "password", "private_key"]) {
    assert.equal(accepted.stdout.includes(forbidden), false);
  }

  const failed = join(temporaryRoot, "failed");
  await writeEvidence(failed, ({ completion }) => {
    completion.success = false;
    completion.exit_code = 1;
  });
  assert.notEqual(invoke(failed).status, 0);

  const tampered = join(temporaryRoot, "tampered");
  await writeEvidence(tampered);
  await writeFile(join(tampered, "soak.stdout.log"), "{}\n");
  assert.notEqual(invoke(tampered).status, 0);

  const stderrPresent = join(temporaryRoot, "stderr-present");
  await writeEvidence(stderrPresent);
  const stderrBytes = Buffer.from("unexpected warning\n");
  const stderrCompletion = JSON.parse(await readFile(join(stderrPresent, "completion.json"), "utf8"));
  stderrCompletion.stderr_sha256 = sha256(stderrBytes);
  await writeFile(join(stderrPresent, "soak.stderr.log"), stderrBytes);
  await writeFile(join(stderrPresent, "completion.json"), `${JSON.stringify(stderrCompletion, null, 2)}\n`);
  assert.notEqual(invoke(stderrPresent).status, 0);

  console.log("Release-soak evidence verifier passed exact identity, metrics, tamper, failure, privacy, and stderr tests.");
} finally {
  const physical = await realpath(temporaryRoot);
  const expectedParent = await realpath(tmpdir());
  if (dirname(physical) !== expectedParent || !/^mcg-soak-evidence-test-[A-Za-z0-9_-]+$/.test(basename(physical))) {
    throw new Error(`refusing to remove unexpected soak-evidence fixture root: ${physical}`);
  }
  await rm(physical, { recursive: true, force: true });
}
