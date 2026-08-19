import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { lstat, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const MAX_FILE_BYTES = 65_536;
const MAX_SIGNATURE_BYTES = 8_192;
const TARGETS = new Set([
  "windows-x86_64-nsis",
  "windows-x86_64",
  "linux-x86_64-appimage",
  "linux-x86_64-deb",
  "linux-x86_64",
  "darwin-x86_64-app",
  "darwin-x86_64",
  "darwin-aarch64-app",
  "darwin-aarch64",
]);

function fail(message) {
  throw new Error(`Updater UAT scenarios rejected: ${message}`);
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) fail(`${label} has unexpected or missing fields`);
}

function parseSemver(value, label) {
  if (typeof value !== "string" || value.length > 128) fail(`${label} is not bounded text`);
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/u.exec(value);
  if (!match) fail(`${label} is not canonical semantic version text`);
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((part) => /^\d+$/u.test(part) && part.length > 1 && part.startsWith("0"))) {
    fail(`${label} has a noncanonical numeric prerelease identifier`);
  }
  return { text: value, core: match.slice(1, 4).map(BigInt), prerelease };
}

function compareSemver(left, right) {
  for (let index = 0; index < 3; index += 1) {
    if (left.core[index] !== right.core[index]) return left.core[index] < right.core[index] ? -1 : 1;
  }
  if (left.prerelease.length === 0 || right.prerelease.length === 0) {
    return left.prerelease.length === right.prerelease.length ? 0 : left.prerelease.length === 0 ? 1 : -1;
  }
  const length = Math.max(left.prerelease.length, right.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const a = left.prerelease[index];
    const b = right.prerelease[index];
    if (a === undefined || b === undefined) return a === b ? 0 : a === undefined ? -1 : 1;
    if (a === b) continue;
    const numericA = /^\d+$/u.test(a);
    const numericB = /^\d+$/u.test(b);
    if (numericA && numericB) return BigInt(a) < BigInt(b) ? -1 : 1;
    if (numericA !== numericB) return numericA ? -1 : 1;
    return a < b ? -1 : 1;
  }
  return 0;
}

async function regularFile(value, label, maximum = MAX_FILE_BYTES) {
  if (typeof value !== "string" || !path.isAbsolute(value) || value.includes("\0")) {
    fail(`${label} must be an absolute path`);
  }
  const metadata = await lstat(value);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size === 0 || metadata.size > maximum) {
    fail(`${label} must be a bounded regular non-symlink file`);
  }
  return realpath(value);
}

function canonicalSignature(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > MAX_SIGNATURE_BYTES) {
    fail(`${label} is empty or oversized`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) fail(`${label} is not canonical base64`);
  const envelope = decoded.toString("utf8");
  if (
    !envelope.startsWith("untrusted comment: signature from tauri secret key\n") ||
    !envelope.includes("\ntrusted comment: ") ||
    envelope.split(/\r?\n/u).filter(Boolean).length !== 4
  ) {
    fail(`${label} is not a Tauri minisign envelope`);
  }
  return value;
}

function canonicalLoopbackDownload(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("valid manifest download URL is invalid");
  }
  if (
    parsed.protocol !== "https:" ||
    !parsed.port ||
    !["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !/^\/artifacts\/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/u.test(parsed.pathname) ||
    parsed.href !== value
  ) {
    fail("valid manifest download URL must be canonical loopback HTTPS on an artifact route");
  }
}

function verify(commandArguments, expectedSuccess, cwd) {
  const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
  const result = spawnSync(cargo, commandArguments, { cwd, encoding: "utf8", windowsHide: true });
  if (expectedSuccess && result.status !== 0) fail("valid signature no longer verifies the candidate artifact");
  if (!expectedSuccess && result.status === 0) fail("bad signature unexpectedly verifies the candidate artifact");
}

const [manifestArgument, artifactArgument, publicKeyArgument, badSignatureArgument, outputArgument, baselineArgument, changedArgument] = process.argv.slice(2);
if (!changedArgument || process.argv.length !== 9) {
  fail("usage: write-updater-uat-scenarios.mjs VALID_MANIFEST CANDIDATE_ARTIFACT PUBLIC_KEY BAD_SIGNATURE ABSOLUTE_OUTPUT_DIR BASELINE_VERSION CHANGED_VERSION");
}
const manifestPath = await regularFile(path.resolve(manifestArgument), "valid manifest");
const artifactPath = await regularFile(path.resolve(artifactArgument), "candidate artifact", 2 * 1024 * 1024 * 1024);
const publicKeyPath = await regularFile(path.resolve(publicKeyArgument), "public key");
const badSignaturePath = await regularFile(path.resolve(badSignatureArgument), "bad signature");
const baselineVersion = parseSemver(baselineArgument, "baseline version");
const changedVersion = parseSemver(changedArgument, "changed version");
let valid;
try {
  valid = JSON.parse(await readFile(manifestPath, "utf8"));
} catch {
  fail("valid manifest is not UTF-8 JSON");
}
exactKeys(valid, ["version", "notes", "pub_date", "platforms"], "valid manifest");
const validVersion = parseSemver(valid.version, "valid manifest version");
if (compareSemver(baselineVersion, validVersion) >= 0) fail("baseline version must be older than the valid update");
if (compareSemver(changedVersion, validVersion) <= 0) fail("changed version must be newer than the valid update");
const platformEntries = Object.entries(valid.platforms ?? {});
if (platformEntries.length !== 1) fail("valid manifest must contain exactly one platform for a VM UAT lane");
const [[target, platform]] = platformEntries;
if (!TARGETS.has(target)) fail("valid manifest target is unsupported");
exactKeys(platform, ["signature", "url"], "valid manifest platform");
const validSignature = canonicalSignature(platform.signature, "valid signature");
const badSignature = canonicalSignature((await readFile(badSignaturePath, "utf8")).trim(), "bad signature");
if (badSignature === validSignature) fail("bad signature must differ from the valid signature");
canonicalLoopbackDownload(platform.url);

const output = path.resolve(outputArgument);
if (!path.isAbsolute(outputArgument) || outputArgument.includes("\0")) fail("output directory must be an absolute new path");
const outputParent = await realpath(path.dirname(output));
if (path.dirname(output) !== outputParent || output === outputParent) fail("output parent must be canonical and distinct");
try {
  await lstat(output);
  fail("output directory already exists");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}
let created = false;
try {
  await mkdir(output, { mode: 0o700 });
  created = true;
  const validSignaturePath = path.join(output, ".valid.sig");
  await writeFile(validSignaturePath, `${validSignature}\n`, { flag: "wx", mode: 0o600 });
  const verifier = ["run", "--quiet", "-p", "mcg-desktop", "--example", "verify-updater-signature", "--"];
  verify([...verifier, publicKeyPath, artifactPath, validSignaturePath], true, process.cwd());
  verify([...verifier, publicKeyPath, artifactPath, badSignaturePath], false, process.cwd());
  await rm(validSignaturePath);

  const documents = {
    "no-update.json": { ...valid, version: baselineVersion.text },
    "valid.json": valid,
    "changed-version.json": { ...valid, version: changedVersion.text },
    "bad-signature.json": {
      ...valid,
      platforms: { [target]: { ...platform, signature: badSignature } },
    },
  };
  const hashes = {};
  for (const [name, value] of Object.entries(documents)) {
    const payload = `${JSON.stringify(value, null, 2)}\n`;
    await writeFile(path.join(output, name), payload, { flag: "wx", mode: 0o600 });
    hashes[name] = createHash("sha256").update(payload).digest("hex");
  }
  const scenarios = {
    "no-update": { kind: "manifest", manifest: path.join(output, "no-update.json") },
    valid: { kind: "manifest", manifest: path.join(output, "valid.json") },
    "changed-version": { kind: "manifest", manifest: path.join(output, "changed-version.json") },
    "bad-signature": { kind: "manifest", manifest: path.join(output, "bad-signature.json") },
    "corrupt-json": { kind: "corrupt-json" },
    unavailable: { kind: "status", status: 503 },
  };
  await writeFile(path.join(output, "scenarios.json"), `${JSON.stringify(scenarios, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  await writeFile(path.join(output, "control.json"), '{"scenario":"no-update"}\n', { flag: "wx", mode: 0o600 });
  process.stdout.write(`${JSON.stringify({ schema_version: 1, target, baseline_version: baselineVersion.text, valid_version: validVersion.text, changed_version: changedVersion.text, manifests: hashes })}\n`);
} catch (error) {
  if (created) await rm(output, { recursive: true, force: true });
  throw error;
}
