import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const required = ["MCG_UPDATER_ENDPOINT", "MCG_UPDATER_PUBKEY"];
for (const name of required) {
  if (!process.env[name]?.trim()) throw new Error(`Signed updater builds require ${name}.`);
}

const privateKeyContent = process.env.TAURI_SIGNING_PRIVATE_KEY?.trim();
const privateKeyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH?.trim();
if (Boolean(privateKeyContent) === Boolean(privateKeyPath)) {
  throw new Error("Signed updater builds require exactly one of TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH.");
}
if (privateKeyContent) {
  if (Buffer.byteLength(privateKeyContent, "utf8") > 64 * 1024) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY exceeds the product bound.");
  }
  if (path.isAbsolute(privateKeyContent)) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY contains a file path; use TAURI_SIGNING_PRIVATE_KEY_PATH.");
  }
} else {
  if (
    !path.isAbsolute(privateKeyPath) ||
    privateKeyPath.includes("\0") ||
    path.normalize(privateKeyPath) !== privateKeyPath ||
    path.resolve(privateKeyPath) !== privateKeyPath
  ) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY_PATH must be a canonical absolute path.");
  }
  const metadata = lstatSync(privateKeyPath, { throwIfNoEntry: false });
  if (!metadata?.isFile() || metadata.isSymbolicLink() || metadata.size === 0 || metadata.size > 64 * 1024) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY_PATH must identify a bounded regular non-symlink file.");
  }
  if (!readFileSync(privateKeyPath, "utf8").trim()) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY_PATH contains an empty key.");
  }
}

const endpoint = new URL(process.env.MCG_UPDATER_ENDPOINT);
if (endpoint.protocol !== "https:" || !endpoint.hostname || endpoint.username || endpoint.password || endpoint.hash) {
  throw new Error("MCG_UPDATER_ENDPOINT must be HTTPS without credentials or a fragment.");
}
if (process.env.MCG_UPDATER_ENDPOINT.length > 2_048) {
  throw new Error("MCG_UPDATER_ENDPOINT exceeds the product bound.");
}

const encodedKey = process.env.MCG_UPDATER_PUBKEY.trim();
if (encodedKey.length > 4_096 || Buffer.from(encodedKey, "base64").toString("base64") !== encodedKey) {
  throw new Error("MCG_UPDATER_PUBKEY must be canonical bounded base64.");
}
const decodedKey = Buffer.from(encodedKey, "base64").toString("utf8");
const keyLines = decodedKey.split(/\r?\n/u).filter(Boolean);
const keyBytes = keyLines.length === 2 ? Buffer.from(keyLines[1], "base64") : Buffer.alloc(0);
if (
  keyLines.length !== 2 ||
  !keyLines[0].startsWith("untrusted comment: minisign public key:") ||
  keyBytes.length !== 42 ||
  keyBytes.toString("base64") !== keyLines[1]
) {
  throw new Error("MCG_UPDATER_PUBKEY must contain minisign public-key text.");
}

console.log(`Signed updater configuration is valid for ${endpoint.origin}.`);
