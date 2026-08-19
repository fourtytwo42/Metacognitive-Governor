import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { lstat, readFile, realpath } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import process from "node:process";

const isWindows = process.platform === "win32";

function samePath(left, right) {
  return isWindows ? left.toLowerCase() === right.toLowerCase() : left === right;
}

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined || values.has(name)) {
      throw new Error("soak evidence arguments must be unique --name value pairs");
    }
    values.set(name, value);
  }
  const allowed = new Set([
    "--evidence-dir",
    "--expected-harness-sha256",
    "--expected-daemon-sha256",
    "--duration-seconds",
    "--tasks",
    "--concurrency",
  ]);
  for (const name of values.keys()) {
    if (!allowed.has(name)) throw new Error(`unknown soak evidence argument: ${name}`);
  }
  const required = (name) => {
    const value = values.get(name);
    if (!value) throw new Error(`${name} is required`);
    return value;
  };
  const integer = (name, minimum, maximum) => {
    const value = Number(required(name));
    if (!Number.isInteger(value) || value < minimum || value > maximum) {
      throw new Error(`${name} must be an integer from ${minimum} through ${maximum}`);
    }
    return value;
  };
  const digest = (name) => {
    const value = required(name);
    if (!/^[a-f0-9]{64}$/.test(value)) throw new Error(`${name} must be canonical lowercase SHA-256`);
    return value;
  };
  return {
    evidenceDirectory: required("--evidence-dir"),
    expectedHarnessSha256: digest("--expected-harness-sha256"),
    expectedDaemonSha256: digest("--expected-daemon-sha256"),
    durationSeconds: integer("--duration-seconds", 5, 86_400),
    tasks: integer("--tasks", 10, 100_000),
    concurrency: integer("--concurrency", 1, 128),
  };
}

async function canonicalDirectory(input) {
  if (!isAbsolute(input) || !samePath(resolve(input), input)) {
    throw new Error("--evidence-dir must be an absolute canonical path");
  }
  const metadata = await lstat(input);
  if (!metadata.isDirectory() || metadata.isSymbolicLink() || !samePath(await realpath(input), input)) {
    throw new Error("--evidence-dir must be a canonical non-symlink directory");
  }
  return input;
}

async function boundedEvidenceFile(directory, name, maximumBytes) {
  const path = resolve(directory, name);
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > maximumBytes ||
      !samePath(await realpath(path), path)) {
    throw new Error(`${name} must be a bounded regular non-symlink file`);
  }
  return readFile(path);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function exactKeys(value, expected, label) {
  assert.deepEqual(Object.keys(value).sort(), [...expected].sort(), `${label} fields changed`);
}

function finiteNumber(value, label, minimum = 0, maximum = Number.MAX_SAFE_INTEGER) {
  assert.equal(typeof value, "number", `${label} must be numeric`);
  assert.ok(Number.isFinite(value) && value >= minimum && value <= maximum, `${label} is outside its release bound`);
}

const options = parseArguments(process.argv.slice(2));
const evidenceDirectory = await canonicalDirectory(options.evidenceDirectory);
const completionBytes = await boundedEvidenceFile(evidenceDirectory, "completion.json", 64 * 1024);
const stdoutBytes = await boundedEvidenceFile(evidenceDirectory, "soak.stdout.log", 1024 * 1024);
const stderrBytes = await boundedEvidenceFile(evidenceDirectory, "soak.stderr.log", 1024 * 1024);
const completion = JSON.parse(completionBytes.toString("utf8"));

exactKeys(completion, [
  "schema_version", "started_at", "completed_at", "elapsed_ms", "duration_seconds", "tasks",
  "concurrency", "harness_sha256", "daemon_sha256", "stdout_sha256", "stderr_sha256",
  "exit_code", "signal", "spawn_error_code", "success",
], "completion evidence");
assert.equal(completion.schema_version, 1);
assert.equal(completion.success, true);
assert.equal(completion.exit_code, 0);
assert.equal(completion.signal, null);
assert.equal(completion.spawn_error_code, null);
assert.equal(completion.duration_seconds, options.durationSeconds);
assert.equal(completion.tasks, options.tasks);
assert.equal(completion.concurrency, options.concurrency);
assert.equal(completion.harness_sha256, options.expectedHarnessSha256);
assert.equal(completion.daemon_sha256, options.expectedDaemonSha256);
assert.equal(completion.stdout_sha256, sha256(stdoutBytes));
assert.equal(completion.stderr_sha256, sha256(stderrBytes));
assert.equal(stderrBytes.length, 0, "successful soak evidence must have empty stderr");

const startedAt = Date.parse(completion.started_at);
const completedAt = Date.parse(completion.completed_at);
finiteNumber(startedAt, "started_at", 0);
finiteNumber(completedAt, "completed_at", startedAt);
assert.ok(Math.abs((completedAt - startedAt) - completion.elapsed_ms) <= 5_000, "completion timestamps and elapsed_ms diverge");
finiteNumber(completion.elapsed_ms, "elapsed_ms", options.durationSeconds * 1_000,
  (options.durationSeconds + Math.ceil(options.tasks / 20) + 600) * 1_000);

const result = JSON.parse(stdoutBytes.toString("utf8"));
exactKeys(result, [
  "schema_version", "daemon", "task_count", "duration_seconds", "concurrency", "cold_startup_ms",
  "restart_ms", "throughput_per_second", "task_creation_p95_ms", "baseline_rss_mib", "peak_rss_mib",
  "rss_growth_mib", "recovered_tasks", "recovered_events", "database_integrity", "schema_compatibility",
], "soak result");
assert.equal(result.schema_version, 1);
assert.ok(result.daemon === "mcg-daemon" || result.daemon === "mcg-daemon.exe", "unexpected daemon basename");
assert.equal(result.task_count, options.tasks);
assert.equal(result.duration_seconds, options.durationSeconds);
assert.equal(result.concurrency, options.concurrency);
assert.equal(result.recovered_tasks, options.tasks);
assert.equal(result.recovered_events, options.tasks);
assert.equal(result.database_integrity, "pass");
assert.equal(result.schema_compatibility, "pass");
finiteNumber(result.cold_startup_ms, "cold_startup_ms", 0, 10_000);
finiteNumber(result.restart_ms, "restart_ms", 0, 10_000);
finiteNumber(result.throughput_per_second, "throughput_per_second", 20);
finiteNumber(result.task_creation_p95_ms, "task_creation_p95_ms", 0, 500);
finiteNumber(result.baseline_rss_mib, "baseline_rss_mib", 0, 768);
finiteNumber(result.peak_rss_mib, "peak_rss_mib", result.baseline_rss_mib, 768);
finiteNumber(result.rss_growth_mib, "rss_growth_mib", 0, 256);
assert.ok(Math.abs((result.peak_rss_mib - result.baseline_rss_mib) - result.rss_growth_mib) <= 0.2,
  "reported RSS growth is inconsistent with baseline and peak");

console.log(JSON.stringify({
  schema_version: 1,
  verified: true,
  harness_sha256: completion.harness_sha256,
  daemon_sha256: completion.daemon_sha256,
  completion_sha256: sha256(completionBytes),
  stdout_sha256: completion.stdout_sha256,
  duration_seconds: result.duration_seconds,
  task_count: result.task_count,
  concurrency: result.concurrency,
  throughput_per_second: result.throughput_per_second,
  task_creation_p95_ms: result.task_creation_p95_ms,
  peak_rss_mib: result.peak_rss_mib,
  restart_ms: result.restart_ms,
  recovered_tasks: result.recovered_tasks,
  recovered_events: result.recovered_events,
}, null, 2));
