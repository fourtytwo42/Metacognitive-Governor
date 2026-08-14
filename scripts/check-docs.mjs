import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const ignoredDirectories = new Set([".git", "node_modules", "target", "dist"]);

function markdownFiles(directory) {
  return readdirSync(directory).flatMap((entry) => {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      return ignoredDirectories.has(entry) ? [] : markdownFiles(path);
    }
    return extname(path) === ".md" ? [path] : [];
  });
}

const errors = [];
for (const file of markdownFiles(root)) {
  const content = readFileSync(file, "utf8");
  const localLinks = content.matchAll(/\[[^\]]+\]\((?!https?:\/\/|#)([^)#]+)(?:#[^)]+)?\)/g);
  for (const match of localLinks) {
    const target = resolve(dirname(file), decodeURIComponent(match[1]));
    if (!existsSync(target)) {
      errors.push(`${file}: missing local link target ${match[1]}`);
    }
  }
}

const sectionNumbers = markdownFiles(join(root, "docs", "spec"))
  .flatMap((file) => [...readFileSync(file, "utf8").matchAll(/^## (\d+)\./gm)])
  .map((match) => Number(match[1]))
  .sort((left, right) => left - right);

const expected = Array.from({ length: 60 }, (_, index) => index + 1);
if (JSON.stringify(sectionNumbers) !== JSON.stringify(expected)) {
  errors.push("docs/spec must contain PRD sections 1 through 60 exactly once");
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else {
  console.log("Documentation links and PRD section coverage are valid.");
}

