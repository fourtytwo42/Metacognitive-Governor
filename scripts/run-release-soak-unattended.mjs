import { createHash, randomUUID } from "node:crypto";
import { closeSync, openSync } from "node:fs";
import { lstat, mkdir, readFile, realpath, rename, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, join, resolve } from "node:path";
import process from "node:process";
import { performance } from "node:perf_hooks";

const isWindows = process.platform === "win32";

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined || values.has(name)) {
      throw new Error("unattended soak arguments must be unique --name value pairs");
    }
    values.set(name, value);
  }
  const allowed = new Set([
    "--harness",
    "--daemon",
    "--output",
    "--expected-harness-sha256",
    "--expected-daemon-sha256",
    "--duration-seconds",
    "--tasks",
    "--concurrency",
  ]);
  for (const name of values.keys()) {
    if (!allowed.has(name)) throw new Error(`unknown unattended soak argument: ${name}`);
  }
  const required = (name) => {
    const value = values.get(name);
    if (!value) throw new Error(`${name} is required`);
    return value;
  };
  const integer = (name, minimum, maximum) => {
    const raw = required(name);
    const value = Number(raw);
    if (!Number.isInteger(value) || value < minimum || value > maximum) {
      throw new Error(`${name} must be an integer from ${minimum} through ${maximum}`);
    }
    return value;
  };
  const digest = (name) => {
    const value = required(name);
    if (!/^[a-f0-9]{64}$/.test(value)) {
      throw new Error(`${name} must be canonical lowercase SHA-256`);
    }
    return value;
  };
  return {
    harness: required("--harness"),
    daemon: required("--daemon"),
    output: required("--output"),
    expectedHarnessSha256: digest("--expected-harness-sha256"),
    expectedDaemonSha256: digest("--expected-daemon-sha256"),
    durationSeconds: integer("--duration-seconds", 5, 86_400),
    tasks: integer("--tasks", 10, 100_000),
    concurrency: integer("--concurrency", 1, 128),
  };
}

function samePath(left, right) {
  return isWindows
    ? left.toLowerCase() === right.toLowerCase()
    : left === right;
}

async function canonicalFile(input, label, maximumBytes) {
  if (!isAbsolute(input) || !samePath(resolve(input), input)) {
    throw new Error(`${label} must be an absolute canonical path`);
  }
  const metadata = await lstat(input);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size <= 0 || metadata.size > maximumBytes) {
    throw new Error(`${label} must be a bounded regular non-symlink file`);
  }
  const physical = await realpath(input);
  if (!samePath(physical, input)) throw new Error(`${label} must not traverse a link or alias`);
  return { path: input, bytes: await readFile(input) };
}

async function prepareOutputDirectory(input) {
  if (!isAbsolute(input) || !samePath(resolve(input), input)) {
    throw new Error("--output must be an absolute canonical path");
  }
  const parent = dirname(input);
  const parentMetadata = await lstat(parent);
  if (!parentMetadata.isDirectory() || parentMetadata.isSymbolicLink() ||
      !samePath(await realpath(parent), parent)) {
    throw new Error("--output parent must be a canonical non-symlink directory");
  }
  try {
    await lstat(input);
    throw new Error("--output must not already exist");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  await mkdir(input, { mode: 0o700 });
  return input;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const options = parseArguments(process.argv.slice(2));
const harness = await canonicalFile(options.harness, "--harness", 2 * 1024 * 1024);
const daemon = await canonicalFile(options.daemon, "--daemon", 512 * 1024 * 1024);
const harnessSha256 = sha256(harness.bytes);
const daemonSha256 = sha256(daemon.bytes);
if (harnessSha256 !== options.expectedHarnessSha256) {
  throw new Error("unattended soak harness SHA-256 does not match the sealed expectation");
}
if (daemonSha256 !== options.expectedDaemonSha256) {
  throw new Error("unattended soak daemon SHA-256 does not match the sealed expectation");
}
if (!options.harness.endsWith("test-release-soak.mjs")) {
  throw new Error("--harness must name test-release-soak.mjs");
}

const outputDirectory = await prepareOutputDirectory(options.output);
const stdoutPath = join(outputDirectory, "soak.stdout.log");
const stderrPath = join(outputDirectory, "soak.stderr.log");
const completionPath = join(outputDirectory, "completion.json");
const startedAt = new Date();
const started = performance.now();
const stdoutFd = openSync(stdoutPath, "wx", 0o600);
const stderrFd = openSync(stderrPath, "wx", 0o600);
const childEnvironment = {
  ...process.env,
  MCG_SOAK_DAEMON: daemon.path,
};
if (isWindows) childEnvironment.MCG_SOAK_WINDOWS_COMMAND_BROKER = "1";
else delete childEnvironment.MCG_SOAK_WINDOWS_COMMAND_BROKER;

let child = null;
let spawnException = null;
try {
  try {
    child = spawnSync(process.execPath, [
      harness.path,
      "--duration-seconds",
      String(options.durationSeconds),
      "--tasks",
      String(options.tasks),
      "--concurrency",
      String(options.concurrency),
    ], {
      cwd: dirname(harness.path),
      env: childEnvironment,
      stdio: ["ignore", stdoutFd, stderrFd],
      windowsHide: true,
    });
  } catch (error) {
    spawnException = error;
  }
} finally {
  closeSync(stdoutFd);
  closeSync(stderrFd);
}

const completion = {
  schema_version: 1,
  started_at: startedAt.toISOString(),
  completed_at: new Date().toISOString(),
  elapsed_ms: Math.round(performance.now() - started),
  duration_seconds: options.durationSeconds,
  tasks: options.tasks,
  concurrency: options.concurrency,
  harness_sha256: harnessSha256,
  daemon_sha256: daemonSha256,
  stdout_sha256: sha256(await readFile(stdoutPath)),
  stderr_sha256: sha256(await readFile(stderrPath)),
  exit_code: Number.isInteger(child?.status) ? child.status : null,
  signal: child?.signal ?? null,
  spawn_error_code: typeof (child?.error ?? spawnException)?.code === "string"
    ? (child?.error ?? spawnException).code
    : (spawnException === null ? null : "SPAWN_EXCEPTION"),
  success: child?.status === 0 && child.signal === null && child.error === undefined && spawnException === null,
};
const temporaryCompletion = join(outputDirectory, `.completion-${process.pid}-${randomUUID()}.tmp`);
try {
  await writeFile(temporaryCompletion, `${JSON.stringify(completion, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  await rename(temporaryCompletion, completionPath);
} finally {
  await rm(temporaryCompletion, { force: true });
}

console.log(JSON.stringify({ completion: completionPath, success: completion.success }));
process.exitCode = completion.success ? 0 : (completion.exit_code ?? 1);
