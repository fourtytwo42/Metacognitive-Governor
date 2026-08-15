import { execFileSync } from "node:child_process";
import process from "node:process";

const status = execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], {
  cwd: process.cwd(),
  encoding: "utf8"
}).trim();
if (status) {
  throw new Error("Release packaging requires a clean, committed source tree so every binary has exact provenance.");
}
console.log("Release source tree is clean and committed.");
