$ErrorActionPreference = "Stop"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm ci
npm run check:docs
npm run check
npm run test
npm run build
npm run tauri --workspace @mcg/desktop -- build --debug
npx playwright install chromium
npm run test:e2e
