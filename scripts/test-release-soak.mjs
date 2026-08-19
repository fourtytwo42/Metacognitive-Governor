import { spawn } from "node:child_process";
import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

const isWindows = process.platform === "win32";
const useWindowsCommandBroker = isWindows && process.env.MCG_SOAK_WINDOWS_COMMAND_BROKER === "1";
const defaults = process.argv.includes("--quick")
  ? { durationSeconds: 10, taskCount: 300, concurrency: 8 }
  : { durationSeconds: 60, taskCount: 2_000, concurrency: 16 };

function integerArgument(name, fallback, minimum, maximum) {
  const index = process.argv.indexOf(name);
  const raw = index === -1 ? fallback : Number(process.argv[index + 1]);
  if (!Number.isInteger(raw) || raw < minimum || raw > maximum) {
    throw new Error(`${name} must be an integer from ${minimum} through ${maximum}`);
  }
  return raw;
}

const durationSeconds = integerArgument("--duration-seconds", defaults.durationSeconds, 5, 86_400);
const taskCount = integerArgument("--tasks", defaults.taskCount, 10, 100_000);
const concurrency = integerArgument("--concurrency", defaults.concurrency, 1, 128);
const startupLimitMs = integerArgument("--startup-limit-ms", 10_000, 1_000, 120_000);
const p95LimitMs = integerArgument("--p95-limit-ms", 500, 10, 30_000);
const minimumThroughput = integerArgument("--minimum-throughput", 20, 1, 100_000);
const memoryLimitMiB = integerArgument("--memory-limit-mib", 768, 64, 16_384);
const growthLimitMiB = integerArgument("--growth-limit-mib", 256, 16, 8_192);
const repositoryRoot = resolve(import.meta.dirname, "..");
const defaultDaemon = join(repositoryRoot, "target", "release", `mcg-daemon${isWindows ? ".exe" : ""}`);
const daemonPath = resolve(process.env.MCG_SOAK_DAEMON || defaultDaemon);

await access(daemonPath);

const dataRoot = await mkdtemp(join(tmpdir(), "mcg-release-soak-"));
const brokerPidPath = join(dataRoot, "broker-daemon.pid");
const brokerStopPath = join(dataRoot, "broker-daemon.stop");
const brokerRssPath = join(dataRoot, "broker-daemon.rss");
let daemon;
let daemonProcessPid;
let daemonOutput = "";

function appendOutput(chunk) {
  daemonOutput = `${daemonOutput}${chunk}`.slice(-65_536);
}

async function reservePort() {
  const server = createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolveClose, reject) => server.close((error) => error ? reject(error) : resolveClose()));
  if (!port) throw new Error("could not reserve a loopback port");
  return port;
}

const port = await reservePort();
const baseUrl = `http://127.0.0.1:${port}`;

function startDaemon() {
  daemonOutput = "";
  daemonProcessPid = undefined;
  const executable = useWindowsCommandBroker ? "powershell.exe" : daemonPath;
  const arguments_ = useWindowsCommandBroker
    ? [
      "-NoProfile",
      "-NonInteractive",
      "-File",
      join(repositoryRoot, "scripts", "windows-soak-process-broker.ps1"),
      "-FilePath",
      daemonPath,
      "-PidFile",
      brokerPidPath,
      "-StopFile",
      brokerStopPath,
      "-RssFile",
      brokerRssPath,
      "-ParentPid",
      String(process.pid),
    ]
    : [];
  const child = spawn(executable, arguments_, {
    env: {
      ...process.env,
      MCG_BIND_ADDRESS: `127.0.0.1:${port}`,
      MCG_DATA_DIR: dataRoot,
      MCG_SHADOW_EXPERIMENTS: "0",
      RUST_LOG: "mcg=warn",
    },
    stdio: useWindowsCommandBroker ? ["ignore", "inherit", "inherit"] : ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.on("data", appendOutput);
  child.stderr?.on("data", appendOutput);
  return child;
}

async function stopDaemon(signal = "SIGTERM") {
  const child = daemon;
  daemon = undefined;
  daemonProcessPid = undefined;
  if (!child) return;
  if (child.exitCode !== null) {
    if (useWindowsCommandBroker) {
      await rm(brokerPidPath, { force: true });
      await rm(brokerStopPath, { force: true });
      await rm(brokerRssPath, { force: true });
    }
    return;
  }
  const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
  if (useWindowsCommandBroker) {
    await writeFile(brokerStopPath, "stop\n", { flag: "wx" });
  } else {
    child.kill(signal);
  }
  await Promise.race([
    exited,
    new Promise((_, reject) => setTimeout(() => reject(new Error("daemon did not exit")), 10_000)),
  ]);
  if (useWindowsCommandBroker) {
    await rm(brokerPidPath, { force: true });
    await rm(brokerStopPath, { force: true });
    await rm(brokerRssPath, { force: true });
  }
}

async function currentDaemonProcessId() {
  if (!useWindowsCommandBroker) return daemon?.pid;
  if (daemonProcessPid) return daemonProcessPid;
  const startedAt = performance.now();
  while (performance.now() - startedAt <= startupLimitMs) {
    try {
      const pid = Number((await readFile(brokerPidPath, "utf8")).trim());
      if (Number.isInteger(pid) && pid > 0) {
        daemonProcessPid = pid;
        return pid;
      }
    } catch {
      // The broker publishes the PID immediately after a successful launch.
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 25));
  }
  throw new Error("broker did not publish a valid daemon process ID");
}

async function waitForHealth() {
  const startedAt = performance.now();
  while (performance.now() - startedAt <= startupLimitMs) {
    if (daemon?.exitCode !== null) {
      throw new Error(`daemon exited during startup\n${daemonOutput}`);
    }
    try {
      const response = await fetch(`${baseUrl}/api/v1/system/health`, { signal: AbortSignal.timeout(1_000) });
      const body = await response.json();
      if (response.ok && body.status === "ok" && body.service === "mcg-daemon") {
        return performance.now() - startedAt;
      }
    } catch {
      // Startup polling is intentionally tolerant until the declared deadline.
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 50));
  }
  throw new Error(`daemon did not become healthy within ${startupLimitMs} ms\n${daemonOutput}`);
}

async function authenticatedFetch(path, token, options = {}) {
  const response = await fetch(`${baseUrl}${path}`, {
    ...options,
    headers: {
      authorization: `Bearer ${token}`,
      ...(options.body ? { "content-type": "application/json" } : {}),
      ...options.headers,
    },
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) {
    throw new Error(`${options.method || "GET"} ${path} returned ${response.status}: ${await response.text()}`);
  }
  return response.json();
}

async function residentBytes(pid) {
  if (isWindows) {
    if (useWindowsCommandBroker) {
      try {
        const bytes = Number((await readFile(brokerRssPath, "utf8")).trim());
        return Number.isFinite(bytes) ? bytes : undefined;
      } catch {
        return undefined;
      }
    }
    const command = `(Get-Process -Id ${pid} -ErrorAction Stop).WorkingSet64`;
    const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command], {
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let stdout = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    const code = await new Promise((resolveExit) => child.once("exit", resolveExit));
    if (code !== 0) return undefined;
    const bytes = Number(stdout.trim());
    return Number.isFinite(bytes) ? bytes : undefined;
  }
  try {
    const status = await readFile(`/proc/${pid}/status`, "utf8");
    const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status);
    return match ? Number(match[1]) * 1_024 : undefined;
  } catch {
    return undefined;
  }
}

function percentile(values, fraction) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
}

let primaryError;
try {
  daemon = startDaemon();
  const coldStartupMs = await waitForHealth();
  const token = (await readFile(join(dataRoot, "local-auth-token"), "utf8")).trim();
  if (!token) throw new Error("daemon created an empty authentication token");

  const baselineRss = await residentBytes(await currentDaemonProcessId());
  const latencies = [];
  const taskIds = new Array(taskCount);
  const eventSequences = new Array(taskCount);
  let nextTask = 0;
  const loadStartedAt = performance.now();
  const workers = Array.from({ length: concurrency }, async () => {
    while (true) {
      const index = nextTask++;
      if (index >= taskCount) return;
      const requestStartedAt = performance.now();
      const created = await authenticatedFetch("/api/v1/tasks", token, {
        method: "POST",
        body: JSON.stringify({
          goal: `Release soak task ${index}`,
          constraints: ["Retain durable history across a forced restart"],
          success_conditions: ["Task and initial event remain queryable"],
        }),
      });
      latencies.push(performance.now() - requestStartedAt);
      if (!created.task?.id || !Number.isInteger(created.event?.sequence) || created.event.sequence < 1) {
        throw new Error(`task ${index} returned an invalid durable creation envelope`);
      }
      taskIds[index] = created.task.id;
      eventSequences[index] = created.event.sequence;
    }
  });
  await Promise.all(workers);
  const loadElapsedMs = performance.now() - loadStartedAt;
  const throughput = taskCount / (loadElapsedMs / 1_000);
  const p95Ms = percentile(latencies, 0.95);
  if (throughput < minimumThroughput) {
    throw new Error(`task throughput ${throughput.toFixed(1)}/s is below ${minimumThroughput}/s`);
  }
  if (p95Ms > p95LimitMs) {
    throw new Error(`task creation p95 ${p95Ms.toFixed(1)} ms exceeds ${p95LimitMs} ms`);
  }

  let peakRss = baselineRss;
  const soakDeadline = performance.now() + durationSeconds * 1_000;
  while (performance.now() < soakDeadline) {
    const health = await fetch(`${baseUrl}/api/v1/system/health`, { signal: AbortSignal.timeout(2_000) });
    if (!health.ok) throw new Error(`health polling returned ${health.status}`);
    const rss = await residentBytes(await currentDaemonProcessId());
    if (rss !== undefined) peakRss = Math.max(peakRss || 0, rss);
    await new Promise((resolveWait) => setTimeout(resolveWait, 500));
  }

  await stopDaemon("SIGKILL");
  daemon = startDaemon();
  const restartMs = await waitForHealth();
  await currentDaemonProcessId();
  const restartedToken = (await readFile(join(dataRoot, "local-auth-token"), "utf8")).trim();
  if (restartedToken !== token) throw new Error("daemon restart unexpectedly rotated loopback authentication");

  const listed = await authenticatedFetch("/api/v1/tasks", token);
  const recoveredIds = new Set(listed.tasks?.map((task) => task.id));
  if (recoveredIds.size !== taskCount || taskIds.some((id) => !recoveredIds.has(id))) {
    throw new Error(`restart recovered ${recoveredIds.size} of ${taskCount} tasks`);
  }
  const sampleIndexes = [...new Set([0, 1, 2, Math.floor(taskCount / 2), taskCount - 3, taskCount - 2, taskCount - 1])]
    .filter((index) => index >= 0 && index < taskCount);
  for (const index of sampleIndexes) {
    const history = await authenticatedFetch(`/api/v1/tasks/${taskIds[index]}/events`, token);
    if (history.events?.length !== 1 || history.events[0].sequence !== eventSequences[index]) {
      throw new Error(`task ${index} did not retain one consistent initial event`);
    }
  }
  const diagnostics = await authenticatedFetch("/api/v1/system/diagnostics", token);
  const integrity = diagnostics.checks?.find((check) => check.code === "database_integrity");
  const schema = diagnostics.checks?.find((check) => check.code === "schema_compatibility");
  if (integrity?.severity !== "pass" || schema?.severity !== "pass") {
    throw new Error("post-crash diagnostics did not prove database and schema integrity");
  }
  if (diagnostics.counts?.tasks !== taskCount || diagnostics.counts?.events !== taskCount) {
    throw new Error("post-crash diagnostic counts do not match the admitted workload");
  }

  const finalRss = await residentBytes(await currentDaemonProcessId());
  peakRss = Math.max(peakRss || 0, finalRss || 0);
  const mib = 1_024 * 1_024;
  const growthBytes = baselineRss === undefined ? undefined : Math.max(0, peakRss - baselineRss);
  if (peakRss && peakRss > memoryLimitMiB * mib) {
    throw new Error(`daemon peak RSS ${(peakRss / mib).toFixed(1)} MiB exceeds ${memoryLimitMiB} MiB`);
  }
  if (growthBytes !== undefined && growthBytes > growthLimitMiB * mib) {
    throw new Error(`daemon RSS growth ${(growthBytes / mib).toFixed(1)} MiB exceeds ${growthLimitMiB} MiB`);
  }

  console.log(JSON.stringify({
    schema_version: 1,
    daemon: basename(daemonPath),
    task_count: taskCount,
    duration_seconds: durationSeconds,
    concurrency,
    cold_startup_ms: Math.round(coldStartupMs),
    restart_ms: Math.round(restartMs),
    throughput_per_second: Number(throughput.toFixed(1)),
    task_creation_p95_ms: Number(p95Ms.toFixed(1)),
    baseline_rss_mib: baselineRss === undefined ? null : Number((baselineRss / mib).toFixed(1)),
    peak_rss_mib: peakRss ? Number((peakRss / mib).toFixed(1)) : null,
    rss_growth_mib: growthBytes === undefined ? null : Number((growthBytes / mib).toFixed(1)),
    recovered_tasks: recoveredIds.size,
    recovered_events: diagnostics.counts.events,
    database_integrity: integrity.severity,
    schema_compatibility: schema.severity,
  }, null, 2));
} catch (error) {
  primaryError = error;
  throw error;
} finally {
  let cleanupError;
  try {
    await stopDaemon();
  } catch (error) {
    cleanupError = error;
  }
  try {
    await rm(dataRoot, { recursive: true, force: true });
  } catch (error) {
    cleanupError ||= error;
  }
  if (cleanupError) {
    if (primaryError) {
      console.error(`[soak] cleanup also failed: ${cleanupError.message}`);
    } else {
      throw cleanupError;
    }
  }
}
