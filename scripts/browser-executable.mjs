import { existsSync } from "node:fs";
import { chromium } from "playwright";

const candidates = [
  process.env.MCG_BROWSER_EXECUTABLE,
  process.platform === "linux"
    ? "/opt/metacognitive-governor/browser/chrome"
    : undefined,
  chromium.executablePath(),
].filter(Boolean);

const executable = candidates.find((candidate) => existsSync(candidate));
if (!executable) {
  console.error("No local Chrome/Chromium executable is available for browser qualification.");
  process.exit(1);
}
process.stdout.write(executable);
