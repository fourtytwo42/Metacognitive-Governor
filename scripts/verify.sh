#!/usr/bin/env bash
set -euo pipefail

npm ci
npx playwright install chromium
export MCG_BROWSER_EXECUTABLE="$(node -e "console.log(require('playwright').chromium.executablePath())")"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check:docs
npm run check
npm run test
npm run build
npm run tauri --workspace @mcg/desktop -- build --debug
npm run test:e2e
