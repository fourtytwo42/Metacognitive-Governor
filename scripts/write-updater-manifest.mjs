import assert from "node:assert/strict";
import { link, lstat, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";

const MAX_CONFIG_BYTES = 65_536;
const MAX_NOTES_CHARS = 8_192;
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
  throw new Error(`Updater manifest rejected: ${message}`);
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) fail(`${label} has unexpected or missing fields`);
}

function canonicalHttps(value, label) {
  if (typeof value !== "string" || value.length > 2_048) fail(`${label} is not a bounded URL`);
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} is invalid`);
  }
  if (
    parsed.protocol !== "https:" ||
    !parsed.hostname ||
    parsed.username ||
    parsed.password ||
    parsed.hash ||
    parsed.href !== value
  ) {
    fail(`${label} must be canonical credential-free HTTPS without a fragment`);
  }
  return parsed.href;
}

function canonicalSignature(value, target) {
  if (typeof value !== "string" || value.length === 0 || value.length > MAX_SIGNATURE_BYTES) {
    fail(`${target} signature is empty or oversized`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) fail(`${target} signature is not canonical base64`);
  const envelope = decoded.toString("utf8");
  if (
    !envelope.startsWith("untrusted comment: signature from tauri secret key\n") ||
    !envelope.includes("\ntrusted comment: ") ||
    envelope.split(/\r?\n/u).filter(Boolean).length !== 4
  ) {
    fail(`${target} signature is not a Tauri minisign envelope`);
  }
  return value;
}

function canonicalPublicKey(value) {
  if (typeof value !== "string" || value.length === 0 || value.length > 4_096) {
    fail("public key is empty or oversized");
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) fail("public key is not canonical base64");
  const envelope = decoded.toString("utf8");
  if (!envelope.startsWith("untrusted comment: minisign public key:") || envelope.split(/\r?\n/u).filter(Boolean).length !== 2) {
    fail("public key is not a Tauri minisign envelope");
  }
}

async function regularFile(value, label) {
  if (typeof value !== "string" || !value || value.includes("\0")) fail(`${label} path is invalid`);
  const resolved = path.resolve(value);
  const metadata = await lstat(resolved);
  if (!metadata.isFile() || metadata.isSymbolicLink()) fail(`${label} must be a regular non-symlink file`);
  return resolved;
}

const [configArgument, outputArgument] = process.argv.slice(2);
if (!configArgument || !outputArgument || process.argv.length !== 4) {
  fail("usage: node scripts/write-updater-manifest.mjs CONFIG.json OUTPUT.json");
}
const configPath = await regularFile(configArgument, "configuration");
const configBytes = await readFile(configPath);
if (configBytes.length === 0 || configBytes.length > MAX_CONFIG_BYTES) fail("configuration is empty or oversized");
let config;
try {
  config = JSON.parse(configBytes.toString("utf8"));
} catch {
  fail("configuration is not valid UTF-8 JSON");
}
exactKeys(config, ["version", "notes", "pub_date", "public_key", "platforms"], "configuration");
if (typeof config.version !== "string" || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(config.version)) {
  fail("version is not canonical semantic version text");
}
if (typeof config.notes !== "string" || config.notes.includes("\0") || [...config.notes].length > MAX_NOTES_CHARS) {
  fail("notes are not bounded text");
}
if (typeof config.pub_date !== "string" || new Date(config.pub_date).toISOString() !== config.pub_date) {
  fail("pub_date must be canonical UTC RFC3339 text");
}
if (!config.platforms || typeof config.platforms !== "object" || Array.isArray(config.platforms)) {
  fail("platforms must be an object");
}
const platformEntries = Object.entries(config.platforms);
if (platformEntries.length === 0 || platformEntries.length > TARGETS.size) fail("platform count is invalid");
const publicKeyPath = await regularFile(config.public_key, "public key");
canonicalPublicKey((await readFile(publicKeyPath, "utf8")).trim());

const platforms = {};
for (const [target, entry] of platformEntries.sort(([left], [right]) => left.localeCompare(right))) {
  if (!TARGETS.has(target)) fail(`unsupported platform target ${target}`);
  exactKeys(entry, ["artifact", "signature", "url"], `${target} configuration`);
  const artifactPath = await regularFile(entry.artifact, `${target} artifact`);
  const signaturePath = await regularFile(entry.signature, `${target} signature`);
  const signatureBytes = await readFile(signaturePath);
  if (signatureBytes.length === 0 || signatureBytes.length > MAX_SIGNATURE_BYTES) {
    fail(`${target} signature file is empty or oversized`);
  }
  const signatureValue = canonicalSignature(signatureBytes.toString("utf8").trim(), target);
  const downloadUrl = canonicalHttps(entry.url, `${target} download URL`);
  const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
  const verification = spawnSync(
    cargo,
    [
      "run",
      "--quiet",
      "-p",
      "mcg-desktop",
      "--example",
      "verify-updater-signature",
      "--",
      publicKeyPath,
      artifactPath,
      signaturePath,
    ],
    { cwd: process.cwd(), encoding: "utf8", windowsHide: true },
  );
  if (verification.status !== 0) fail(`${target} signature does not verify the selected artifact`);
  platforms[target] = {
    signature: signatureValue,
    url: downloadUrl,
  };
}

const manifest = `${JSON.stringify(
  {
    version: config.version,
    notes: config.notes,
    pub_date: config.pub_date,
    platforms,
  },
  null,
  2,
)}\n`;
const outputPath = path.resolve(outputArgument);
if (path.extname(outputPath).toLowerCase() !== ".json") fail("output must use a .json extension");
try {
  await lstat(outputPath);
  fail("output already exists");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}
const outputDirectory = path.dirname(outputPath);
const temporaryOutput = path.join(outputDirectory, `.${path.basename(outputPath)}.${randomUUID()}.tmp`);
try {
  await writeFile(temporaryOutput, manifest, { encoding: "utf8", flag: "wx", mode: 0o600 });
  await link(temporaryOutput, outputPath);
} finally {
  await rm(temporaryOutput, { force: true });
}
assert.deepEqual(JSON.parse(await readFile(outputPath, "utf8")), JSON.parse(manifest));
console.log(`Updater manifest created for ${Object.keys(platforms).join(", ")}.`);
