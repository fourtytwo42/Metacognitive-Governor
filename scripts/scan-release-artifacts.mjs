import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import {
  lstat,
  readFile,
  readdir,
  realpath,
  rename,
  stat,
  unlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const MAX_MANIFEST_BYTES = 1024 * 1024;
const MAX_ARTIFACTS = 32;
const MAX_SCANNER_OUTPUT_BYTES = 64 * 1024;
const SCAN_TIMEOUT_MS = 15 * 60 * 1000;
const MAX_SIGNATURE_AGE_SECONDS = 72 * 60 * 60;

function fail(message) {
  throw new Error(message);
}

function isInside(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function requireObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object.`);
  return value;
}

function requireString(value, label, maximum = 512) {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\0")) {
    fail(`${label} must be a bounded non-empty string.`);
  }
  return value;
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: MAX_SCANNER_OUTPUT_BYTES,
    timeout: SCAN_TIMEOUT_MS,
    ...options,
  });
  if (result.error) fail(`${path.basename(command)} could not run: ${result.error.message}`);
  if (result.signal) fail(`${path.basename(command)} was interrupted by ${result.signal}.`);
  return result;
}

async function hashFile(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

async function requireProtectedExecutable(file, { windows = false } = {}) {
  const entry = await lstat(file).catch(() => null);
  if (!entry?.isFile() || entry.isSymbolicLink()) fail(`Required scanner is not a regular non-symlink file: ${file}`);
  const resolved = await realpath(file);
  if (!windows) {
    const metadata = await stat(resolved);
    if (metadata.uid !== 0 || (metadata.mode & 0o022) !== 0) {
      fail(`Release scanner must be root-owned and not group/other writable: ${resolved}`);
    }
  }
  return resolved;
}

async function createWindowsDefenderScanner(now = new Date()) {
  const programData = path.resolve(process.env.ProgramData || "C:\\ProgramData");
  const platformRoot = path.join(programData, "Microsoft", "Windows Defender", "Platform");
  const versions = (await readdir(platformRoot, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory() && /^\d+(?:\.\d+)+(?:-\d+)?$/u.test(entry.name))
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left, "en", { numeric: true }));
  if (versions.length === 0) fail("Microsoft Defender platform executable was not found.");
  const executable = await requireProtectedExecutable(path.join(platformRoot, versions[0], "MpCmdRun.exe"), { windows: true });

  const systemRoot = path.resolve(process.env.SystemRoot || "C:\\Windows");
  const powershell = await requireProtectedExecutable(
    path.join(systemRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe"),
    { windows: true },
  );
  const statusResult = run(powershell, [
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "Get-MpComputerStatus | Select-Object AMServiceEnabled,AntivirusEnabled,AMEngineVersion,AntivirusSignatureVersion,@{Name='AntivirusSignatureLastUpdated';Expression={$_.AntivirusSignatureLastUpdated.ToUniversalTime().ToString('o')}} | ConvertTo-Json -Compress",
  ]);
  if (statusResult.status !== 0) fail("Microsoft Defender status query failed.");
  const defender = requireObject(JSON.parse(statusResult.stdout), "Microsoft Defender status");
  if (defender.AMServiceEnabled !== true || defender.AntivirusEnabled !== true) {
    fail("Microsoft Defender antimalware and antivirus services must both be enabled.");
  }
  const signatureUpdated = new Date(requireString(defender.AntivirusSignatureLastUpdated, "Defender signature update time"));
  const signatureAgeSeconds = Math.floor((now.getTime() - signatureUpdated.getTime()) / 1000);
  if (!Number.isFinite(signatureAgeSeconds) || signatureAgeSeconds < 0 || signatureAgeSeconds > MAX_SIGNATURE_AGE_SECONDS) {
    fail("Microsoft Defender signatures must be no more than 72 hours old.");
  }

  return {
    metadata: {
      name: "Microsoft Defender Antivirus",
      engine_version: requireString(defender.AMEngineVersion, "Defender engine version", 128),
      signatures_version: requireString(defender.AntivirusSignatureVersion, "Defender signatures version", 128),
      signatures_age_seconds: signatureAgeSeconds,
    },
    async scan(file) {
      const result = run(executable, ["-Scan", "-ScanType", "3", "-File", file, "-DisableRemediation"]);
      if (result.status !== 0) {
        fail(`Microsoft Defender did not return a clean result for ${path.basename(file)} (exit ${result.status}).`);
      }
    },
  };
}

async function createLinuxClamAvScanner(now = new Date()) {
  let executable = null;
  for (const candidate of ["/usr/bin/clamscan", "/usr/local/bin/clamscan"]) {
    try {
      executable = await requireProtectedExecutable(candidate);
      break;
    } catch {
      // Try the next fixed protected location.
    }
  }
  if (!executable) fail("ClamAV clamscan is required at /usr/bin/clamscan or /usr/local/bin/clamscan.");

  const databaseRoot = "/var/lib/clamav";
  const databases = (await readdir(databaseRoot, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && /\.(?:cvd|cld)$/u.test(entry.name))
    .map((entry) => path.join(databaseRoot, entry.name));
  if (databases.length === 0) fail("ClamAV has no installed signature databases.");
  let newestDatabaseMs = 0;
  for (const database of databases) {
    const metadata = await stat(database);
    if (metadata.uid !== 0 || (metadata.mode & 0o022) !== 0) fail("ClamAV signature databases must be protected root-owned files.");
    newestDatabaseMs = Math.max(newestDatabaseMs, metadata.mtimeMs);
  }
  const signatureAgeSeconds = Math.floor((now.getTime() - newestDatabaseMs) / 1000);
  if (!Number.isFinite(signatureAgeSeconds) || signatureAgeSeconds < 0 || signatureAgeSeconds > MAX_SIGNATURE_AGE_SECONDS) {
    fail("ClamAV signatures must be no more than 72 hours old.");
  }
  const version = run(executable, ["--version"]);
  if (version.status !== 0) fail("ClamAV version query failed.");

  return {
    metadata: {
      name: "ClamAV",
      engine_and_signatures: requireString(version.stdout.trim(), "ClamAV version", 512),
      signatures_age_seconds: signatureAgeSeconds,
    },
    async scan(file) {
      const result = run(executable, ["--infected", "--no-summary", "--stdout", file]);
      if (result.status !== 0) fail(`ClamAV did not return a clean result for ${path.basename(file)} (exit ${result.status}).`);
    },
  };
}

export async function createNativeScanner(platform = process.platform, now = new Date()) {
  if (platform === "win32") return createWindowsDefenderScanner(now);
  if (platform === "linux") return createLinuxClamAvScanner(now);
  fail("Release artifact malware scanning supports Windows and Linux. macOS public-release reputation is enforced by the notarization, stapling, and Gatekeeper package lane.");
}

export async function verifyManifestArtifacts(rootArgument, manifestArgument) {
  const root = await realpath(path.resolve(rootArgument));
  const manifestPath = path.resolve(manifestArgument);
  if (!isInside(root, manifestPath)) fail("Release manifest must be inside the repository root.");
  const manifestEntry = await lstat(manifestPath);
  if (!manifestEntry.isFile() || manifestEntry.isSymbolicLink() || manifestEntry.size > MAX_MANIFEST_BYTES) {
    fail("Release manifest must be a bounded regular non-symlink file.");
  }
  const manifestReal = await realpath(manifestPath);
  if (!isInside(root, manifestReal)) fail("Release manifest resolves outside the repository root.");
  const manifestBytes = await readFile(manifestReal);
  const manifest = requireObject(JSON.parse(manifestBytes.toString("utf8")), "release manifest");
  if (manifest.schema_version !== 1 || manifest.application !== "MetaCognitive Governor" || manifest.source_dirty !== false) {
    fail("Release manifest identity/schema/clean-source contract is invalid.");
  }
  requireString(manifest.version, "release version", 128);
  if (!/^[0-9a-f]{40}$/u.test(manifest.source_commit)) fail("Release manifest source commit must be a lowercase Git SHA-1.");
  if (!Array.isArray(manifest.files) || manifest.files.length === 0 || manifest.files.length > MAX_ARTIFACTS) {
    fail(`Release manifest must contain between 1 and ${MAX_ARTIFACTS} artifacts.`);
  }

  const artifacts = [];
  const seen = new Set();
  for (const [index, raw] of manifest.files.entries()) {
    const entry = requireObject(raw, `release artifact ${index}`);
    const relative = requireString(entry.path, `release artifact ${index} path`, 512);
    if (relative.includes("\\") || path.posix.isAbsolute(relative) || path.posix.normalize(relative) !== relative || relative.startsWith("../")) {
      fail(`Release artifact ${index} has an unsafe path.`);
    }
    if (seen.has(relative)) fail(`Release artifact path is duplicated: ${relative}`);
    seen.add(relative);
    if (!Number.isSafeInteger(entry.size_bytes) || entry.size_bytes < 1) fail(`Release artifact ${relative} has an invalid size.`);
    if (typeof entry.sha256 !== "string" || !/^[0-9a-f]{64}$/u.test(entry.sha256)) fail(`Release artifact ${relative} has an invalid SHA-256.`);
    const absolute = path.resolve(root, ...relative.split("/"));
    if (!isInside(root, absolute)) fail(`Release artifact escapes the repository: ${relative}`);
    const metadata = await lstat(absolute);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== entry.size_bytes) {
      fail(`Release artifact size/type mismatch: ${relative}`);
    }
    const resolved = await realpath(absolute);
    if (!isInside(root, resolved)) fail(`Release artifact resolves outside the repository: ${relative}`);
    const sha256 = await hashFile(resolved);
    if (sha256 !== entry.sha256) fail(`Release artifact SHA-256 mismatch: ${relative}`);
    artifacts.push({ absolute: resolved, path: relative, size_bytes: entry.size_bytes, sha256 });
  }

  return {
    root,
    manifest,
    manifest_path: manifestReal,
    manifest_sha256: createHash("sha256").update(manifestBytes).digest("hex"),
    artifacts,
  };
}

export async function qualifyReleaseArtifacts({ root, manifestPath, outputPath, scanner, platform, scannedAt = new Date() }) {
  const verified = await verifyManifestArtifacts(root, manifestPath);
  const scannerMetadata = requireObject(scanner?.metadata, "scanner metadata");
  if (typeof scanner?.scan !== "function") fail("scanner must provide a scan function.");
  const output = path.resolve(outputPath);
  if (!isInside(verified.root, output)) fail("Release scan evidence must be written inside the repository root.");
  const outputParent = await realpath(path.dirname(output));
  if (!isInside(verified.root, outputParent)) fail("Release scan evidence parent resolves outside the repository root.");
  if (await lstat(output).then(() => true, () => false)) fail("Refusing to replace existing release scan evidence.");
  const cleanArtifacts = [];
  for (const artifact of verified.artifacts) {
    await scanner.scan(artifact.absolute);
    cleanArtifacts.push({ path: artifact.path, size_bytes: artifact.size_bytes, sha256: artifact.sha256, clean: true });
  }

  const report = {
    schema_version: 1,
    application: verified.manifest.application,
    version: verified.manifest.version,
    source_commit: verified.manifest.source_commit,
    manifest_sha256: verified.manifest_sha256,
    platform,
    scanned_at: scannedAt.toISOString(),
    scanner: scannerMetadata,
    artifacts: cleanArtifacts,
    clean: true,
  };
  const temporary = `${output}.tmp-${process.pid}`;
  await writeFile(temporary, `${JSON.stringify(report, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  try {
    await rename(temporary, output);
  } catch (error) {
    await unlink(temporary).catch(() => {});
    throw error;
  }
  return report;
}

function parseArguments(arguments_) {
  const values = new Map();
  for (let index = 0; index < arguments_.length; index += 2) {
    const name = arguments_[index];
    const value = arguments_[index + 1];
    if (!["--root", "--manifest", "--output"].includes(name) || value === undefined) {
      fail("usage: node scripts/scan-release-artifacts.mjs --root REPOSITORY --manifest MANIFEST --output REPORT");
    }
    if (values.has(name)) fail(`Duplicate argument: ${name}`);
    values.set(name, value);
  }
  for (const name of ["--root", "--manifest", "--output"]) if (!values.has(name)) fail(`Missing required argument: ${name}`);
  return values;
}

async function main() {
  const arguments_ = parseArguments(process.argv.slice(2));
  const root = await realpath(path.resolve(arguments_.get("--root")));
  const gitStatus = run("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: root });
  if (gitStatus.status !== 0 || gitStatus.stdout.trim()) fail("Release artifact scanning requires a clean source tree.");
  const gitCommit = run("git", ["rev-parse", "HEAD"], { cwd: root });
  if (gitCommit.status !== 0) fail("Release source commit could not be read.");
  const verified = await verifyManifestArtifacts(root, path.resolve(arguments_.get("--manifest")));
  if (gitCommit.stdout.trim() !== verified.manifest.source_commit) fail("Release manifest source commit does not match repository HEAD.");
  const scanner = await createNativeScanner();
  const report = await qualifyReleaseArtifacts({
    root,
    manifestPath: arguments_.get("--manifest"),
    outputPath: arguments_.get("--output"),
    scanner,
    platform: process.platform,
  });
  console.log(JSON.stringify({ clean: report.clean, platform: report.platform, artifacts: report.artifacts.length }));
}

const invoked = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invoked) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
