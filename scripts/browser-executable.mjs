import { existsSync } from "node:fs";
import { chromium } from "playwright";

const playwrightChromium = chromium.executablePath();

function matchingHeadlessShell(chromiumPath) {
  const match = chromiumPath.match(/^(.*[\\/])chromium-(\d+)[\\/]/);
  if (!match) return undefined;
  if (process.platform === "win32") {
    return `${match[1]}chromium_headless_shell-${match[2]}\\chrome-headless-shell-win64\\chrome-headless-shell.exe`;
  }
  if (process.platform === "linux") {
    return `${match[1]}chromium_headless_shell-${match[2]}/chrome-headless-shell-linux64/chrome-headless-shell`;
  }
  return undefined;
}

const candidates = [
  process.env.MCG_BROWSER_EXECUTABLE,
  process.platform === "linux"
    ? "/opt/metacognitive-governor/browser/chrome"
    : undefined,
  matchingHeadlessShell(playwrightChromium),
  playwrightChromium,
].filter(Boolean);

const executable = candidates.find((candidate) => existsSync(candidate));
if (!executable) {
  console.error("No local Chrome/Chromium executable is available for browser qualification.");
  process.exit(1);
}
process.stdout.write(executable);
