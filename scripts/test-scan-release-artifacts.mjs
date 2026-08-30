import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  clamAvDailySignatureAgeSeconds,
  qualifyReleaseArtifacts,
  verifyManifestArtifacts,
} from "./scan-release-artifacts.mjs";

const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "mcg-release-scan-"));
const artifactDirectory = path.join(temporaryRoot, "target", "release", "bundle");
const evidenceDirectory = path.join(temporaryRoot, "target", "release");
const artifact = path.join(artifactDirectory, "MCG-qualification-package.bin");
const manifestPath = path.join(evidenceDirectory, "release-manifest.json");
const outputPath = path.join(evidenceDirectory, "release-security-scan.json");
const artifactBytes = Buffer.from("MCG release scanner qualification fixture\n", "utf8");
const artifactSha256 = createHash("sha256").update(artifactBytes).digest("hex");
const sourceCommit = "0123456789abcdef0123456789abcdef01234567";

const manifestFor = (overrides = {}) => ({
  schema_version: 1,
  application: "MetaCognitive Governor",
  version: "0.1.0",
  source_commit: sourceCommit,
  source_dirty: false,
  files: [{
    path: "target/release/bundle/MCG-qualification-package.bin",
    size_bytes: artifactBytes.length,
    sha256: artifactSha256,
  }],
  ...overrides,
});

async function writeManifest(destination, value) {
  await writeFile(destination, `${JSON.stringify(value, null, 2)}\n`, { flag: "w" });
}

async function expectFailure(action, pattern) {
  await assert.rejects(action, pattern);
}

try {
  const scannerNow = new Date("2026-08-21T00:00:00.000Z");
  assert.equal(clamAvDailySignatureAgeSeconds([
    { path: "/var/lib/clamav/main.cvd", mtimeMs: new Date("2026-01-01T00:00:00.000Z").getTime() },
    { path: "/var/lib/clamav/bytecode.cvd", mtimeMs: new Date("2026-01-01T00:00:00.000Z").getTime() },
    { path: "/var/lib/clamav/daily.cld", mtimeMs: new Date("2026-08-20T23:00:00.000Z").getTime() },
  ], scannerNow), 3600, "stable base databases must not make a current daily feed stale");
  await expectFailure(
    async () => clamAvDailySignatureAgeSeconds([
      { path: "/var/lib/clamav/daily.cvd", mtimeMs: new Date("2026-08-01T00:00:00.000Z").getTime() },
    ], scannerNow),
    /no more than 72 hours old/u,
  );

  await mkdir(artifactDirectory, { recursive: true });
  await writeFile(artifact, artifactBytes, { flag: "wx" });
  await writeManifest(manifestPath, manifestFor());

  const verified = await verifyManifestArtifacts(temporaryRoot, manifestPath);
  assert.equal(verified.artifacts.length, 1);
  assert.equal(verified.artifacts[0].sha256, artifactSha256);
  assert.equal(verified.artifacts[0].absolute, await realpath(artifact));

  const scanned = [];
  const report = await qualifyReleaseArtifacts({
    root: temporaryRoot,
    manifestPath,
    outputPath,
    platform: "qualification",
    scannedAt: new Date("2026-08-21T00:00:00.000Z"),
    scanner: {
      metadata: { name: "Qualification Scanner", signatures_version: "fixture-1", signatures_age_seconds: 0 },
      async scan(file) { scanned.push(file); },
    },
  });
  assert.equal(scanned.length, 1);
  assert.equal(report.clean, true);
  assert.deepEqual(report.artifacts, [{
    path: "target/release/bundle/MCG-qualification-package.bin",
    size_bytes: artifactBytes.length,
    sha256: artifactSha256,
    clean: true,
  }]);
  const reportText = await readFile(outputPath, "utf8");
  assert.equal(reportText.includes(temporaryRoot), false, "scan evidence must not disclose absolute paths");
  assert.equal(JSON.parse(reportText).scanner.name, "Qualification Scanner");
  await expectFailure(
    () => qualifyReleaseArtifacts({
      root: temporaryRoot,
      manifestPath,
      outputPath,
      platform: "qualification",
      scanner: { metadata: { name: "Qualification Scanner", signatures_age_seconds: 0 }, async scan() {} },
    }),
    /Refusing to replace existing release scan evidence/u,
  );

  const invalidManifest = path.join(evidenceDirectory, "invalid-manifest.json");
  await writeManifest(invalidManifest, manifestFor({
    files: [{ path: "../escape.bin", size_bytes: 1, sha256: "0".repeat(64) }],
  }));
  await expectFailure(() => verifyManifestArtifacts(temporaryRoot, invalidManifest), /unsafe path/u);

  const changedHashManifest = path.join(evidenceDirectory, "changed-hash-manifest.json");
  await writeManifest(changedHashManifest, manifestFor({
    files: [{
      path: "target/release/bundle/MCG-qualification-package.bin",
      size_bytes: artifactBytes.length,
      sha256: "f".repeat(64),
    }],
  }));
  await expectFailure(() => verifyManifestArtifacts(temporaryRoot, changedHashManifest), /SHA-256 mismatch/u);

  const infectedOutput = path.join(evidenceDirectory, "infected-security-scan.json");
  await expectFailure(
    () => qualifyReleaseArtifacts({
      root: temporaryRoot,
      manifestPath,
      outputPath: infectedOutput,
      platform: "qualification",
      scanner: {
        metadata: { name: "Qualification Scanner", signatures_age_seconds: 0 },
        async scan() { throw new Error("fixture detection"); },
      },
    }),
    /fixture detection/u,
  );
  await assert.rejects(readFile(infectedOutput), /ENOENT/u);

  console.log("Release artifact manifest binding, clean scanning, privacy, no-clobber, traversal, tamper, and detection rejection passed.");
} finally {
  const resolved = path.resolve(temporaryRoot);
  const systemTemporary = path.resolve(os.tmpdir());
  assert.equal(path.dirname(resolved), systemTemporary);
  assert.match(path.basename(resolved), /^mcg-release-scan-/u);
  await rm(resolved, { recursive: true, force: true });
}
