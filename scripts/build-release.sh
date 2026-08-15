#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"
node scripts/check-release-source.mjs
"$repository_root/scripts/prepare-sidecars.sh" release
npm run tauri --workspace @mcg/desktop -- build --config src-tauri/tauri.release.conf.json --bundles deb,appimage
node scripts/write-release-manifest.mjs target/release/bundle/deb target/release/bundle/appimage
