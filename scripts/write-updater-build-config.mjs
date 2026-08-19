import { lstat, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

function fail(message) {
  throw new Error(`Updater build config rejected: ${message}`);
}

const [outputArgument] = process.argv.slice(2);
if (!outputArgument || process.argv.length !== 3 || !path.isAbsolute(outputArgument) || outputArgument.includes("\0")) {
  fail("usage: write-updater-build-config.mjs ABSOLUTE_NEW_OUTPUT.json");
}

const output = path.resolve(outputArgument);
const parent = await realpath(path.dirname(output));
if (path.dirname(output) !== parent || output === parent) fail("output parent must be canonical and distinct");
try {
  await lstat(output);
  fail("output already exists");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}

const endpointValue = process.env.MCG_UPDATER_ENDPOINT?.trim();
const publicKey = process.env.MCG_UPDATER_PUBKEY?.trim();
if (!endpointValue || !publicKey) fail("validated endpoint and public key environment are required");
let endpoint;
try {
  endpoint = new URL(endpointValue);
} catch {
  fail("endpoint is invalid");
}
if (
  endpoint.protocol !== "https:" ||
  !endpoint.hostname ||
  endpoint.username ||
  endpoint.password ||
  endpoint.hash ||
  endpoint.href !== endpointValue ||
  endpointValue.length > 2_048
) {
  fail("endpoint must be canonical bounded credential-free HTTPS");
}
if (publicKey.length > 4_096 || Buffer.from(publicKey, "base64").toString("base64") !== publicKey) {
  fail("public key must be canonical bounded base64");
}
const decodedKey = Buffer.from(publicKey, "base64").toString("utf8");
const keyLines = decodedKey.split(/\r?\n/u).filter(Boolean);
const keyBytes = keyLines.length === 2 ? Buffer.from(keyLines[1], "base64") : Buffer.alloc(0);
if (
  keyLines.length !== 2 ||
  !keyLines[0].startsWith("untrusted comment: minisign public key:") ||
  keyBytes.length !== 42 ||
  keyBytes.toString("base64") !== keyLines[1]
) {
  fail("public key is not a Tauri minisign envelope");
}

const config = {
  $schema: "https://schema.tauri.app/config/2",
  bundle: { createUpdaterArtifacts: true },
  plugins: { updater: { endpoints: [endpoint.href], pubkey: publicKey } },
};
await writeFile(output, `${JSON.stringify(config, null, 2)}\n`, { flag: "wx", mode: 0o600 });
process.stdout.write("Updater bundler public configuration created.\n");
