import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { execFileSync } from "node:child_process";

if (process.argv.length < 3) {
  throw new Error("usage: node scripts/write-release-manifest.mjs DIRECTORY [DIRECTORY ...]");
}

const root = process.cwd();
const sourceStatus = execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: root, encoding: "utf8" }).trim();
if (sourceStatus) throw new Error("Refusing to describe release packages from a dirty source tree.");
const files = [];
for (const argument of process.argv.slice(2)) {
  const directory = path.resolve(root, argument);
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const absolute = path.join(directory, entry.name);
    const bytes = await readFile(absolute);
    files.push({
      path: path.relative(root, absolute).replaceAll(path.sep, "/"),
      size_bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex")
    });
  }
}
files.sort((left, right) => left.path.localeCompare(right.path));
if (files.length === 0) throw new Error("No release packages were found for the manifest.");

const cargo = await readFile(path.join(root, "Cargo.toml"), "utf8");
const version = cargo.match(/\[workspace\.package\][\s\S]*?\nversion = "([^"]+)"/)?.[1];
if (!version) throw new Error("Workspace version could not be read.");
const commit = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
const manifest = {
  schema_version: 1,
  application: "MetaCognitive Governor",
  version,
  source_commit: commit,
  source_dirty: false,
  files
};
const destination = path.join(root, "target", "release", "release-manifest.json");
await writeFile(destination, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "w" });
console.log(destination);
