import { open, lstat, readFile, realpath } from "node:fs/promises";
import { createReadStream } from "node:fs";
import https from "node:https";
import path from "node:path";
import process from "node:process";

const MAX_CONFIG_BYTES = 65_536;
const MAX_CONTROL_BYTES = 4_096;
const MAX_MANIFEST_BYTES = 65_536;
const MAX_ARTIFACT_BYTES = 2 * 1024 * 1024 * 1024;
const SCENARIO_PATTERN = /^[a-z0-9][a-z0-9-]{0,63}$/u;
const ARTIFACT_ROUTE_PATTERN = /^\/artifacts\/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/u;
const FAILURE_STATUSES = new Set([408, 429, 500, 502, 503, 504]);

function fail(message) {
  throw new Error(`Updater UAT server rejected configuration: ${message}`);
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) fail(`${label} has unexpected or missing fields`);
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

function loopbackOrigin(value) {
  if (typeof value !== "string" || value.length > 2_048) fail("origin must be a bounded URL");
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("origin is invalid");
  }
  if (
    parsed.protocol !== "https:" ||
    !parsed.port ||
    parsed.username ||
    parsed.password ||
    parsed.pathname !== "/" ||
    parsed.search ||
    parsed.hash ||
    !["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname)
  ) {
    fail("origin must be canonical loopback HTTPS with an explicit port");
  }
  if (parsed.href !== value) fail("origin must be canonical");
  return parsed;
}

async function readBoundedJson(file, maximum, label) {
  const bytes = await readFile(file);
  if (bytes.length === 0 || bytes.length > maximum) throw new Error(`${label} is empty or oversized`);
  try {
    return JSON.parse(bytes.toString("utf8"));
  } catch {
    throw new Error(`${label} is not valid UTF-8 JSON`);
  }
}

const [configurationArgument] = process.argv.slice(2);
if (!configurationArgument || process.argv.length !== 3) {
  fail("usage: node scripts/updater-uat-server.mjs CONFIG.json");
}
const configurationPath = await regularFile(configurationArgument, "configuration", MAX_CONFIG_BYTES);
const configuration = await readBoundedJson(configurationPath, MAX_CONFIG_BYTES, "configuration");
exactKeys(
  configuration,
  ["origin", "certificate", "private_key", "control", "request_log", "scenarios", "artifacts"],
  "configuration",
);
const origin = loopbackOrigin(configuration.origin);
const port = Number(origin.port);
const bind = origin.hostname === "[::1]" ? "::1" : "127.0.0.1";
const certificate = await regularFile(configuration.certificate, "certificate", 65_536);
const privateKey = await regularFile(configuration.private_key, "private key", 65_536);
const control = await regularFile(configuration.control, "control", MAX_CONTROL_BYTES);
if (typeof configuration.request_log !== "string" || !path.isAbsolute(configuration.request_log)) {
  fail("request_log must be an absolute new path");
}

if (!configuration.scenarios || typeof configuration.scenarios !== "object" || Array.isArray(configuration.scenarios)) {
  fail("scenarios must be an object");
}
const scenarioEntries = Object.entries(configuration.scenarios);
if (scenarioEntries.length === 0 || scenarioEntries.length > 32) fail("scenario count is invalid");
const scenarios = new Map();
for (const [name, scenario] of scenarioEntries) {
  if (!SCENARIO_PATTERN.test(name)) fail(`scenario name ${name} is invalid`);
  if (!scenario || typeof scenario !== "object" || Array.isArray(scenario)) fail(`${name} must be an object`);
  if (scenario.kind === "manifest") {
    exactKeys(scenario, ["kind", "manifest"], `${name} scenario`);
    const manifest = await regularFile(scenario.manifest, `${name} manifest`, MAX_MANIFEST_BYTES);
    await readBoundedJson(manifest, MAX_MANIFEST_BYTES, `${name} manifest`);
    scenarios.set(name, { kind: "manifest", manifest });
  } else if (scenario.kind === "corrupt-json") {
    exactKeys(scenario, ["kind"], `${name} scenario`);
    scenarios.set(name, { kind: "corrupt-json" });
  } else if (scenario.kind === "status") {
    exactKeys(scenario, ["kind", "status"], `${name} scenario`);
    if (!Number.isInteger(scenario.status) || !FAILURE_STATUSES.has(scenario.status)) {
      fail(`${name} status is not an allowed updater failure status`);
    }
    scenarios.set(name, { kind: "status", status: scenario.status });
  } else {
    fail(`${name} has an unsupported scenario kind`);
  }
}

if (!configuration.artifacts || typeof configuration.artifacts !== "object" || Array.isArray(configuration.artifacts)) {
  fail("artifacts must be an object");
}
const artifactEntries = Object.entries(configuration.artifacts);
if (artifactEntries.length === 0 || artifactEntries.length > 16) fail("artifact count is invalid");
const artifacts = new Map();
for (const [route, file] of artifactEntries) {
  if (!ARTIFACT_ROUTE_PATTERN.test(route)) fail(`artifact route ${route} is invalid`);
  const artifact = await regularFile(file, `${route} artifact`, MAX_ARTIFACT_BYTES);
  const metadata = await lstat(artifact);
  artifacts.set(route, { file: artifact, size: metadata.size });
}

const requestLog = await open(configuration.request_log, "wx", 0o600);
let closing = false;

async function activeScenario() {
  const value = await readBoundedJson(control, MAX_CONTROL_BYTES, "control");
  exactKeys(value, ["scenario"], "control");
  if (typeof value.scenario !== "string" || !SCENARIO_PATTERN.test(value.scenario)) {
    throw new Error("control scenario is invalid");
  }
  const scenario = scenarios.get(value.scenario);
  if (!scenario) throw new Error("control selected an unknown scenario");
  return [value.scenario, scenario];
}

function headers(contentType, contentLength) {
  return {
    "cache-control": "no-store",
    "content-length": String(contentLength),
    "content-security-policy": "default-src 'none'; frame-ancestors 'none'",
    "content-type": contentType,
    "x-content-type-options": "nosniff",
  };
}

async function record(method, route, status, bytes, scenario = null) {
  const entry = JSON.stringify({
    at: new Date().toISOString(),
    method,
    route,
    status,
    bytes,
    scenario,
  });
  await requestLog.appendFile(`${entry}\n`, "utf8");
}

async function respond(request, response) {
  const method = request.method ?? "";
  let parsed;
  try {
    parsed = new URL(request.url ?? "", origin);
  } catch {
    response.writeHead(400, headers("application/json; charset=utf-8", 24));
    response.end('{"error":"bad request"}\n');
    await record(method, "invalid", 400, 24);
    return;
  }
  const route = parsed.pathname;
  if (parsed.origin !== origin.origin || parsed.search || parsed.hash) {
    const body = Buffer.from('{"error":"bad request"}\n');
    response.writeHead(400, headers("application/json; charset=utf-8", body.length));
    response.end(body);
    await record(method, route, 400, body.length);
    return;
  }
  if (method !== "GET") {
    const body = Buffer.from('{"error":"method not allowed"}\n');
    response.writeHead(405, { ...headers("application/json; charset=utf-8", body.length), allow: "GET" });
    response.end(body);
    await record(method, route, 405, body.length);
    return;
  }
  if (route === "/health") {
    const body = Buffer.from('{"status":"ok"}\n');
    response.writeHead(200, headers("application/json; charset=utf-8", body.length));
    response.end(body);
    await record(method, route, 200, body.length);
    return;
  }
  if (route === "/latest.json") {
    let name;
    let scenario;
    try {
      [name, scenario] = await activeScenario();
    } catch {
      const body = Buffer.from('{"error":"invalid fixture control"}\n');
      response.writeHead(503, headers("application/json; charset=utf-8", body.length));
      response.end(body);
      await record(method, route, 503, body.length, "invalid-control");
      return;
    }
    if (scenario.kind === "manifest") {
      const body = await readFile(scenario.manifest);
      response.writeHead(200, headers("application/json; charset=utf-8", body.length));
      response.end(body);
      await record(method, route, 200, body.length, name);
      return;
    }
    if (scenario.kind === "corrupt-json") {
      const body = Buffer.from("{\n");
      response.writeHead(200, headers("application/json; charset=utf-8", body.length));
      response.end(body);
      await record(method, route, 200, body.length, name);
      return;
    }
    const body = Buffer.from(`{"error":"fixture status ${scenario.status}"}\n`);
    response.writeHead(scenario.status, headers("application/json; charset=utf-8", body.length));
    response.end(body);
    await record(method, route, scenario.status, body.length, name);
    return;
  }
  const artifact = artifacts.get(route);
  if (artifact) {
    response.writeHead(200, headers("application/octet-stream", artifact.size));
    await new Promise((resolve, reject) => {
      const stream = createReadStream(artifact.file);
      stream.once("error", reject);
      response.once("error", reject);
      response.once("finish", resolve);
      stream.pipe(response);
    });
    await record(method, route, 200, artifact.size);
    return;
  }
  const body = Buffer.from('{"error":"not found"}\n');
  response.writeHead(404, headers("application/json; charset=utf-8", body.length));
  response.end(body);
  await record(method, route, 404, body.length);
}

const server = https.createServer(
  {
    cert: await readFile(certificate),
    key: await readFile(privateKey),
    minVersion: "TLSv1.2",
  },
  (request, response) => {
    respond(request, response).catch(async () => {
      if (!response.headersSent) {
        const body = Buffer.from('{"error":"fixture failure"}\n');
        response.writeHead(500, headers("application/json; charset=utf-8", body.length));
        response.end(body);
      } else {
        response.destroy();
      }
    });
  },
);

async function close(exitCode = 0) {
  if (closing) return;
  closing = true;
  server.close(async () => {
    await requestLog.close();
    process.exitCode = exitCode;
  });
}

process.once("SIGINT", () => void close());
process.once("SIGTERM", () => void close());
server.on("error", async (error) => {
  process.stderr.write(`Updater UAT server failed: ${error.message}\n`);
  await requestLog.close();
  process.exitCode = 1;
});
server.listen(port, bind, () => {
  process.stdout.write(`UPDATER_UAT_SERVER_READY ${origin.href}latest.json\n`);
});
