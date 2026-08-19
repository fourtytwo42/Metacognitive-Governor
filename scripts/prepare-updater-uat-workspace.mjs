import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { lstat, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

function fail(message) {
  throw new Error(`Updater UAT workspace rejected: ${message}`);
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
    ...options,
  });
  if (result.status !== 0) {
    const detail = `${result.error?.message ?? ""}\n${result.stderr ?? ""}\n${result.stdout ?? ""}`.trim().slice(0, 4_000);
    fail(`${path.basename(command)} failed with status ${result.status}: ${detail}`);
  }
  return result.stdout;
}

function pythonCommand() {
  for (const candidate of ["python3", "python"]) {
    const result = spawnSync(candidate, ["-c", "import cryptography"], { windowsHide: true });
    if (result.status === 0) return candidate;
  }
  fail("Python with cryptography is required for disposable updater TLS material");
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

function canonicalOrigin(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("origin is invalid");
  }
  if (
    parsed.protocol !== "https:" ||
    !parsed.port ||
    !["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname) ||
    parsed.username ||
    parsed.password ||
    parsed.pathname !== "/" ||
    parsed.search ||
    parsed.hash ||
    parsed.href !== value
  ) {
    fail("origin must be canonical loopback HTTPS with an explicit port");
  }
  return parsed.href;
}

async function regularArchive(value) {
  const resolved = path.resolve(value);
  const metadata = await lstat(resolved);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size === 0 || metadata.size > 128 * 1024 * 1024) {
    fail("archive must be a bounded regular non-symlink file");
  }
  return realpath(resolved);
}

async function regularHelper(value, root, label) {
  const metadata = await lstat(value);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size === 0 || metadata.size > 1024 * 1024) {
    fail(`${label} must be a bounded regular non-symlink file`);
  }
  const resolved = await realpath(value);
  const relative = path.relative(root, resolved);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`${label} escaped the prepared source`);
  }
  return resolved;
}

const [archiveArgument, rootArgument, baselineArgument, candidateArgument, originArgument] = process.argv.slice(2);
if (!originArgument || process.argv.length !== 7) {
  fail("usage: prepare-updater-uat-workspace.mjs ARCHIVE.tar.gz ABSOLUTE_NEW_ROOT BASELINE_VERSION CANDIDATE_VERSION LOOPBACK_HTTPS_ORIGIN");
}
const archive = await regularArchive(archiveArgument);
const baselineVersion = parseSemver(baselineArgument, "baseline version");
const candidateVersion = parseSemver(candidateArgument, "candidate version");
if (compareSemver(baselineVersion, candidateVersion) >= 0) fail("candidate version must be newer than baseline version");
const origin = canonicalOrigin(originArgument);
if (!path.isAbsolute(rootArgument) || rootArgument.includes("\0")) fail("root must be an absolute new path");
const workspace = path.resolve(rootArgument);
const parent = await realpath(path.dirname(workspace));
if (path.dirname(workspace) !== parent || workspace === parent) fail("root parent must be canonical and distinct");
try {
  await lstat(workspace);
  fail("workspace already exists");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}

let created = false;
try {
  await mkdir(workspace, { mode: 0o700 });
  created = true;
  const baseline = path.join(workspace, "baseline");
  const candidate = path.join(workspace, "candidate");
  const bootstrap = path.join(workspace, "bootstrap");
  const tls = path.join(workspace, "tls");
  const signing = path.join(workspace, "signing");
  const feed = path.join(workspace, "feed");
  const evidence = path.join(workspace, "evidence");
  for (const directory of [tls, signing, feed, evidence]) await mkdir(directory, { mode: 0o700 });

  const prepare = (preparer, destination, version, cwd) => JSON.parse(run(process.execPath, [
    preparer,
    archive,
    destination,
    version,
  ], { cwd }));
  const callerRoot = path.resolve(import.meta.dirname, "..");
  const callerPreparer = path.join(import.meta.dirname, "prepare-updater-uat-source.mjs");
  const bootstrapSource = prepare(callerPreparer, bootstrap, baselineVersion.text, callerRoot);
  const archivePreparer = await regularHelper(
    path.join(bootstrap, "scripts/prepare-updater-uat-source.mjs"),
    bootstrap,
    "archive source preparer",
  );
  const baselineSource = prepare(archivePreparer, baseline, baselineVersion.text, bootstrap);
  const candidateSource = prepare(archivePreparer, candidate, candidateVersion.text, bootstrap);
  if (
    bootstrapSource.archive_sha256 !== baselineSource.archive_sha256 ||
    bootstrapSource.archive_entries !== baselineSource.archive_entries ||
    bootstrapSource.base_version !== baselineSource.base_version ||
    baselineSource.archive_sha256 !== candidateSource.archive_sha256 ||
    baselineSource.archive_entries !== candidateSource.archive_entries ||
    baselineSource.base_version !== candidateSource.base_version
  ) {
    fail("prepared source provenance diverged");
  }
  const baselinePreparer = await regularHelper(
    path.join(baseline, "scripts/prepare-updater-uat-source.mjs"),
    baseline,
    "baseline source preparer",
  );
  const candidatePreparer = await regularHelper(
    path.join(candidate, "scripts/prepare-updater-uat-source.mjs"),
    candidate,
    "candidate source preparer",
  );
  const archivePreparerBytes = await readFile(archivePreparer);
  if (
    !archivePreparerBytes.equals(await readFile(baselinePreparer)) ||
    !archivePreparerBytes.equals(await readFile(candidatePreparer))
  ) {
    fail("archive source preparer changed across prepared sources");
  }
  await rm(bootstrap, { recursive: true });
  if (process.platform === "win32") {
    const commandInterpreter = process.env.ComSpec ?? "C:\\Windows\\System32\\cmd.exe";
    run(commandInterpreter, ["/d", "/s", "/c", "npm.cmd ci"], { cwd: baseline });
    run(commandInterpreter, ["/d", "/s", "/c", "npm.cmd ci"], { cwd: candidate });
  } else {
    run("npm", ["ci"], { cwd: baseline });
    run("npm", ["ci"], { cwd: candidate });
  }

  const python = pythonCommand();
  const caCertificate = path.join(tls, "ca.crt");
  const serverCertificate = path.join(tls, "server.crt");
  const serverKey = path.join(tls, "server.key");
  const tlsGenerator = await regularHelper(
    path.join(baseline, "scripts/generate-updater-uat-tls.py"),
    baseline,
    "archive TLS generator",
  );
  const tlsEvidence = JSON.parse(run(python, [
    tlsGenerator,
    caCertificate,
    serverCertificate,
    serverKey,
  ], { cwd: baseline }));

  const privateKey = path.join(signing, "qualification.key");
  const publicKey = `${privateKey}.pub`;
  const tauriCli = path.join(baseline, "node_modules", "@tauri-apps", "cli", "tauri.js");
  run(process.execPath, [tauriCli, "signer", "generate", "--ci", "--write-keys", privateKey], { cwd: baseline });
  const publicKeyValue = (await readFile(publicKey, "utf8")).trim();
  const publicKeyEnvelope = Buffer.from(publicKeyValue, "base64").toString("utf8");
  if (!publicKeyEnvelope.startsWith("untrusted comment: minisign public key:")) {
    fail("generated updater public key is not a Tauri minisign envelope");
  }

  const publicDescriptor = {
    schema_version: 1,
    archive_sha256: baselineSource.archive_sha256,
    archive_entries: baselineSource.archive_entries,
    source_base_version: baselineSource.base_version,
    source_preparer_sha256: createHash("sha256").update(archivePreparerBytes).digest("hex"),
    tls_generator_sha256: createHash("sha256").update(await readFile(tlsGenerator)).digest("hex"),
    baseline: { version: baselineVersion.text, commit: baselineSource.commit },
    candidate: { version: candidateVersion.text, commit: candidateSource.commit },
    updater_endpoint: `${origin}latest.json`,
    updater_public_key_sha256: createHash("sha256").update(publicKeyValue).digest("hex"),
    tls: tlsEvidence,
  };
  const privateDescriptor = {
    schema_version: 1,
    baseline_source: baseline,
    candidate_source: candidate,
    tls_ca_certificate: caCertificate,
    tls_server_certificate: serverCertificate,
    tls_server_key: serverKey,
    updater_private_key: privateKey,
    updater_public_key: publicKey,
    feed_directory: feed,
    evidence_directory: evidence,
    updater_origin: origin,
  };
  const publicDescriptorPath = path.join(workspace, "qualification-public.json");
  const privateDescriptorPath = path.join(workspace, "qualification-private.json");
  await writeFile(publicDescriptorPath, `${JSON.stringify(publicDescriptor, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  await writeFile(privateDescriptorPath, `${JSON.stringify(privateDescriptor, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  process.stdout.write(`${JSON.stringify({ schema_version: 1, workspace, public_descriptor: publicDescriptorPath, private_descriptor: privateDescriptorPath })}\n`);
} catch (error) {
  if (created) await rm(workspace, { recursive: true, force: true });
  throw error;
}
