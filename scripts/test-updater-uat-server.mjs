import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const root = path.resolve(import.meta.dirname, "..");
const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "mcg-updater-uat-server-"));
const caCertificate = path.join(temporaryRoot, "ca.crt");
const certificate = path.join(temporaryRoot, "server.crt");
const privateKey = path.join(temporaryRoot, "server.key");
const control = path.join(temporaryRoot, "control.json");
const requestLog = path.join(temporaryRoot, "requests.ndjson");
const artifact = path.join(temporaryRoot, "update.bin");
const manifest = path.join(temporaryRoot, "latest-valid.json");
const config = path.join(temporaryRoot, "server.json");
const port = 20_000 + Math.floor(Math.random() * 20_000);
const origin = `https://127.0.0.1:${port}/`;

function pythonCommand() {
  for (const candidate of ["python3", "python"]) {
    const result = spawnSync(candidate, ["-c", "import cryptography"], { windowsHide: true });
    if (result.status === 0) return candidate;
  }
  throw new Error("Python with cryptography is required for disposable updater TLS qualification");
}

async function setScenario(scenario) {
  const replacement = path.join(temporaryRoot, `control-${randomUUID()}.json`);
  await writeFile(replacement, `${JSON.stringify({ scenario })}\n`, { flag: "wx" });
  await rename(replacement, control);
}

function request(route, { method = "GET", authorization } = {}) {
  return new Promise((resolve, reject) => {
    const call = https.request(
      new URL(route, origin),
      {
        method,
        rejectUnauthorized: false,
        headers: authorization ? { authorization } : {},
      },
      (response) => {
        const chunks = [];
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () =>
          resolve({ status: response.statusCode, headers: response.headers, body: Buffer.concat(chunks) }),
        );
      },
    );
    call.on("error", reject);
    call.end();
  });
}

let server;
try {
  const python = pythonCommand();
  const generated = spawnSync(python, ["scripts/generate-updater-uat-tls.py", caCertificate, certificate, privateKey], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assert.equal(generated.status, 0, `${generated.stderr}\n${generated.stdout}`);
  const tlsEvidence = JSON.parse(generated.stdout);
  assert.equal(tlsEvidence.schema_version, 1);
  assert.equal(tlsEvidence.validity_hours, 24);
  assert.deepEqual(tlsEvidence.hosts, ["localhost", "127.0.0.1"]);
  assert.match(tlsEvidence.ca_sha256, /^[0-9a-f]{64}$/u);
  assert.match(tlsEvidence.server_sha256, /^[0-9a-f]{64}$/u);
  const remainingValidity = Date.parse(tlsEvidence.not_after) - Date.now();
  assert.ok(remainingValidity >= 23 * 60 * 60 * 1_000, "TLS fixture expires too soon for package UAT");
  assert.ok(remainingValidity <= 24 * 60 * 60 * 1_000, "TLS fixture exceeds its bounded lifetime");
  const overwrite = spawnSync(python, ["scripts/generate-updater-uat-tls.py", caCertificate, certificate, privateKey], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assert.notEqual(overwrite.status, 0, "TLS generator unexpectedly replaced existing material");

  await writeFile(artifact, Buffer.from("signed updater artifact fixture\n"), { flag: "wx" });
  await writeFile(
    manifest,
    `${JSON.stringify({
      version: "0.1.1",
      notes: "Updater UAT server contract.",
      pub_date: "2026-08-18T00:00:00.000Z",
      platforms: {
        "windows-x86_64-nsis": {
          signature: "qualification-signature",
          url: `${origin}artifacts/update.bin`,
        },
      },
    })}\n`,
    { flag: "wx" },
  );
  await writeFile(control, '{"scenario":"valid"}\n', { flag: "wx" });
  await writeFile(
    config,
    `${JSON.stringify({
      origin,
      certificate,
      private_key: privateKey,
      control,
      request_log: requestLog,
      scenarios: {
        valid: { kind: "manifest", manifest },
        corrupt: { kind: "corrupt-json" },
        unavailable: { kind: "status", status: 503 },
      },
      artifacts: { "/artifacts/update.bin": artifact },
    })}\n`,
    { flag: "wx" },
  );

  server = spawn(process.execPath, ["scripts/updater-uat-server.mjs", config], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("updater UAT server did not become ready")), 10_000);
    server.once("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`updater UAT server exited before ready with ${code}`));
    });
    server.stdout.on("data", (chunk) => {
      if (chunk.toString("utf8").includes("UPDATER_UAT_SERVER_READY")) {
        clearTimeout(timer);
        resolve();
      }
    });
  });

  const health = await request("health");
  assert.equal(health.status, 200);
  assert.deepEqual(JSON.parse(health.body), { status: "ok" });
  assert.equal(health.headers["cache-control"], "no-store");
  assert.equal(health.headers["x-content-type-options"], "nosniff");

  const valid = await request("latest.json", { authorization: "Bearer must-not-be-logged" });
  assert.equal(valid.status, 200);
  assert.equal(JSON.parse(valid.body).version, "0.1.1");
  const downloaded = await request("artifacts/update.bin");
  assert.equal(downloaded.status, 200);
  assert.equal(downloaded.body.toString("utf8"), "signed updater artifact fixture\n");

  await setScenario("corrupt");
  const corrupt = await request("latest.json");
  assert.equal(corrupt.status, 200);
  assert.throws(() => JSON.parse(corrupt.body));
  await setScenario("unavailable");
  assert.equal((await request("latest.json")).status, 503);
  assert.equal((await request("latest.json?credential=forbidden")).status, 400);
  assert.equal((await request("artifacts/%2e%2e%2fserver.key")).status, 404);
  assert.equal((await request("latest.json", { method: "POST" })).status, 405);

  server.kill();
  await new Promise((resolve) => server.once("exit", resolve));
  server = undefined;
  const log = await readFile(requestLog, "utf8");
  assert.doesNotMatch(log, /must-not-be-logged|authorization|credential=forbidden/u);
  const entries = log.trim().split("\n").map((line) => JSON.parse(line));
  assert(entries.some((entry) => entry.scenario === "valid" && entry.status === 200));
  assert(entries.some((entry) => entry.scenario === "corrupt" && entry.status === 200));
  assert(entries.some((entry) => entry.scenario === "unavailable" && entry.status === 503));
  console.log("Loopback HTTPS updater UAT scenario server passed confinement, switching, failure, artifact, and privacy tests.");
} finally {
  if (server && server.exitCode === null) server.kill();
  const resolved = path.resolve(temporaryRoot);
  assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
  assert.match(path.basename(resolved), /^mcg-updater-uat-server-/u);
  await rm(resolved, { recursive: true, force: true });
}
