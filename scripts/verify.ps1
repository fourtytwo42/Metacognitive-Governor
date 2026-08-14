$ErrorActionPreference = "Stop"

function Assert-NativeSuccess {
    param([Parameter(Mandatory = $true)][string]$Step)
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed with exit code $LASTEXITCODE"
    }
}

cargo fmt --all -- --check
Assert-NativeSuccess "cargo fmt"
cargo clippy --workspace --all-targets -- -D warnings
Assert-NativeSuccess "cargo clippy"
cargo test --workspace
Assert-NativeSuccess "cargo test"
npm ci
Assert-NativeSuccess "npm ci"
npm run check:docs
Assert-NativeSuccess "documentation check"
npm run check
Assert-NativeSuccess "frontend typecheck"
npm run test
Assert-NativeSuccess "frontend tests"
npm run build
Assert-NativeSuccess "frontend build"
npm run tauri --workspace @mcg/desktop -- build --debug
Assert-NativeSuccess "Tauri build"
npx playwright install chromium
Assert-NativeSuccess "Playwright browser install"
npm run test:e2e
Assert-NativeSuccess "Playwright end-to-end tests"
