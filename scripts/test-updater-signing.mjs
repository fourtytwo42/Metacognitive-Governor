import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const tauriCli = path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "mcg-updater-signing-"));
const privateKey = path.join(temporaryRoot, "qualification.key");
const publicKey = `${privateKey}.pub`;
const artifact = path.join(temporaryRoot, "MCG-qualification-update.bin");
const signature = `${artifact}.sig`;
const decoyArtifact = path.join(temporaryRoot, "MCG-qualification-decoy.bin");
const decoySignature = `${decoyArtifact}.sig`;
const manifestConfig = path.join(temporaryRoot, "manifest-config.json");
const manifestOutput = path.join(temporaryRoot, "latest.json");
const scenariosOutput = path.join(temporaryRoot, "scenarios");
const bundlerConfig = path.join(temporaryRoot, "updater-build.json");

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
  if (result.status !== 0) {
    const detail = `${result.stderr ?? ""}\n${result.stdout ?? ""}`.trim().slice(0, 2_000);
    throw new Error(`${path.basename(command)} failed with status ${result.status}: ${detail}`);
  }
  return result;
}

function runFailure(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
  assert.notEqual(result.status, 0, `${path.basename(command)} unexpectedly succeeded`);
  return result;
}

try {
  run(process.execPath, [
    tauriCli,
    "signer",
    "generate",
    "--ci",
    "--write-keys",
    privateKey,
  ]);

  await writeFile(
    artifact,
    Buffer.from("MCG disposable updater signing qualification fixture\n", "utf8"),
    { flag: "wx" },
  );
  run(
    process.execPath,
    [tauriCli, "signer", "sign", "--password", "", artifact],
    {
      env: {
        ...process.env,
        TAURI_SIGNING_PRIVATE_KEY_PATH: privateKey,
      },
    },
  );
  await writeFile(decoyArtifact, Buffer.from("MCG deliberately different updater artifact\n", "utf8"), { flag: "wx" });
  run(
    process.execPath,
    [tauriCli, "signer", "sign", "--password", "", decoyArtifact],
    {
      env: {
        ...process.env,
        TAURI_SIGNING_PRIVATE_KEY_PATH: privateKey,
      },
    },
  );

  const encodedPublicKey = (await readFile(publicKey, "utf8")).trim();
  const publicKeyText = Buffer.from(encodedPublicKey, "base64").toString("utf8");
  assert.match(publicKeyText, /minisign public key/u);
  const cleanSigningEnvironment = { ...process.env };
  delete cleanSigningEnvironment.TAURI_SIGNING_PRIVATE_KEY;
  delete cleanSigningEnvironment.TAURI_SIGNING_PRIVATE_KEY_PATH;
  const updaterConfigEnvironment = {
    ...cleanSigningEnvironment,
    MCG_UPDATER_ENDPOINT: "https://updates.qualification.invalid/latest.json",
    MCG_UPDATER_PUBKEY: encodedPublicKey,
  };
  const privateKeyContent = await readFile(privateKey, "utf8");
  run(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY: privateKeyContent,
    },
  });
  run(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY_PATH: privateKey,
    },
  });
  assert.match(runFailure(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY: privateKeyContent,
      TAURI_SIGNING_PRIVATE_KEY_PATH: privateKey,
    },
  }).stderr, /exactly one of TAURI_SIGNING_PRIVATE_KEY/u);
  assert.match(runFailure(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: updaterConfigEnvironment,
  }).stderr, /exactly one of TAURI_SIGNING_PRIVATE_KEY/u);
  assert.match(runFailure(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY: privateKey,
    },
  }).stderr, /contains a file path/u);
  assert.match(runFailure(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY_PATH: "qualification.key",
    },
  }).stderr, /canonical absolute path/u);
  assert.match(runFailure(process.execPath, ["scripts/check-updater-config.mjs"], {
    env: {
      ...updaterConfigEnvironment,
      TAURI_SIGNING_PRIVATE_KEY_PATH: path.join(temporaryRoot, "missing.key"),
    },
  }).stderr, /bounded regular non-symlink file/u);
  const updaterEnvironment = {
    ...cleanSigningEnvironment,
    MCG_UPDATER_ENDPOINT: "https://updates.qualification.invalid/latest.json",
    MCG_UPDATER_PUBKEY: encodedPublicKey,
  };
  run(process.execPath, ["scripts/write-updater-build-config.mjs", bundlerConfig], { env: updaterEnvironment });
  const generatedBundlerConfig = JSON.parse(await readFile(bundlerConfig, "utf8"));
  assert.deepEqual(generatedBundlerConfig.plugins.updater, {
    endpoints: ["https://updates.qualification.invalid/latest.json"],
    pubkey: encodedPublicKey,
  });
  assert.equal(generatedBundlerConfig.bundle.createUpdaterArtifacts, true);
  runFailure(process.execPath, ["scripts/write-updater-build-config.mjs", bundlerConfig], { env: updaterEnvironment });

  const qualificationTarget = {
    win32: "windows-x86_64-nsis",
    linux: "linux-x86_64-appimage",
    darwin: process.arch === "arm64" ? "darwin-aarch64-app" : "darwin-x86_64-app",
  }[process.platform];
  assert.ok(qualificationTarget, `unsupported qualification platform ${process.platform}`);
  const manifestFixture = {
    version: "0.1.1",
    notes: "Disposable updater publication qualification.",
    pub_date: "2026-08-18T00:00:00.000Z",
    public_key: publicKey,
    platforms: {
      [qualificationTarget]: {
        artifact,
        signature,
        url: `https://127.0.0.1:18443/artifacts/${path.basename(artifact)}`,
      },
    },
  };
  await writeFile(manifestConfig, `${JSON.stringify(manifestFixture)}\n`, { flag: "wx" });
  run(process.execPath, ["scripts/write-updater-manifest.mjs", manifestConfig, manifestOutput]);
  const manifest = JSON.parse(await readFile(manifestOutput, "utf8"));
  assert.deepEqual(Object.keys(manifest.platforms), [qualificationTarget]);
  assert.equal(manifest.platforms[qualificationTarget].signature, (await readFile(signature, "utf8")).trim());
  assert.equal("artifact" in manifest.platforms[qualificationTarget], false);
  const scenarios = JSON.parse(run(process.execPath, [
    "scripts/write-updater-uat-scenarios.mjs",
    manifestOutput,
    artifact,
    publicKey,
    decoySignature,
    scenariosOutput,
    "0.1.0",
    "0.1.2",
  ]).stdout);
  assert.equal(scenarios.target, qualificationTarget);
  assert.equal(scenarios.baseline_version, "0.1.0");
  assert.equal(scenarios.valid_version, "0.1.1");
  assert.equal(scenarios.changed_version, "0.1.2");
  const badManifest = JSON.parse(await readFile(path.join(scenariosOutput, "bad-signature.json"), "utf8"));
  assert.equal(badManifest.platforms[qualificationTarget].signature, (await readFile(decoySignature, "utf8")).trim());
  assert.notEqual(badManifest.platforms[qualificationTarget].signature, manifest.platforms[qualificationTarget].signature);
  assert.equal(JSON.parse(await readFile(path.join(scenariosOutput, "control.json"), "utf8")).scenario, "no-update");
  runFailure(process.execPath, [
    "scripts/write-updater-uat-scenarios.mjs",
    manifestOutput,
    artifact,
    publicKey,
    decoySignature,
    path.join(temporaryRoot, "invalid-scenarios"),
    "0.1.1",
    "0.1.2",
  ]);

  const invalidConfig = path.join(temporaryRoot, "invalid-http-config.json");
  const invalidOutput = path.join(temporaryRoot, "invalid-latest.json");
  manifestFixture.platforms[qualificationTarget].url = "http://updates.qualification.invalid/update.bin";
  await writeFile(invalidConfig, `${JSON.stringify(manifestFixture)}\n`, { flag: "wx" });
  runFailure(process.execPath, ["scripts/write-updater-manifest.mjs", invalidConfig, invalidOutput]);

  console.log("Ephemeral updater signing, publication, cryptographic scenario feeds, tamper rejection, and preflight passed.");
} finally {
  const resolvedTemporaryRoot = path.resolve(temporaryRoot);
  const resolvedSystemTemporaryRoot = path.resolve(os.tmpdir());
  assert.notEqual(resolvedTemporaryRoot, resolvedSystemTemporaryRoot);
  assert.equal(path.dirname(resolvedTemporaryRoot), resolvedSystemTemporaryRoot);
  assert.match(path.basename(resolvedTemporaryRoot), /^mcg-updater-signing-/u);
  await rm(resolvedTemporaryRoot, { recursive: true, force: true });
}
