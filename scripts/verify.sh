#!/usr/bin/env bash
set -euo pipefail

npm ci
npx playwright install chromium
export MCG_BROWSER_EXECUTABLE="$(node scripts/browser-executable.mjs)"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
npm run check:docs
npm run check:packaging
npm run test:updater-signing
npm run test:release-artifact-scan
npm run test:updater-uat-server
npm run test:prepare-updater-uat-source
npm run test:prepare-updater-uat-workspace
npm run test:release-soak-unattended
npm run test:verify-release-soak-evidence
npm run check
npm run test
npm run build
npm run tauri --workspace @mcg/desktop -- build --debug
npm run test:e2e
