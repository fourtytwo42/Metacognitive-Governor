$ErrorActionPreference = "Stop"

function Assert-NativeSuccess {
    param([Parameter(Mandatory = $true)][string]$Step)
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed with exit code $LASTEXITCODE"
    }
}

npm ci
Assert-NativeSuccess "npm ci"
npx playwright install chromium
Assert-NativeSuccess "Playwright browser install"
$browserPath = node scripts/browser-executable.mjs
Assert-NativeSuccess "Playwright browser path"
$env:MCG_BROWSER_EXECUTABLE = $browserPath.Trim()
cargo fmt --all -- --check
Assert-NativeSuccess "cargo fmt"
cargo clippy --workspace --all-targets --all-features -- -D warnings
Assert-NativeSuccess "cargo clippy"
cargo test --workspace --all-features
Assert-NativeSuccess "cargo test"
npm run check:docs
Assert-NativeSuccess "documentation check"
npm run check:packaging
Assert-NativeSuccess "packaging contract check"
npm run test:updater-signing
Assert-NativeSuccess "updater signing tests"
npm run test:updater-uat-server
Assert-NativeSuccess "updater scenario server tests"
npm run test:prepare-updater-uat-source
Assert-NativeSuccess "updater source preparation tests"
npm run test:prepare-updater-uat-workspace
Assert-NativeSuccess "updater workspace preparation tests"
npm run test:release-soak-unattended
Assert-NativeSuccess "unattended release-soak launcher tests"
npm run test:verify-release-soak-evidence
Assert-NativeSuccess "release-soak evidence verifier tests"
& powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-windows-updater-uat.ps1
Assert-NativeSuccess "Windows updater UAT harness fixture"
npm run check
Assert-NativeSuccess "frontend typecheck"
npm run test
Assert-NativeSuccess "frontend tests"
npm run build
Assert-NativeSuccess "frontend build"
npm run tauri --workspace @mcg/desktop -- build --debug
Assert-NativeSuccess "Tauri build"
npm run test:e2e
Assert-NativeSuccess "Playwright end-to-end tests"
