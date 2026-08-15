#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Linux release builds must run on Linux. Use scripts/build-release-macos.sh on macOS." >&2
  exit 1
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"
node scripts/check-release-source.mjs
"$repository_root/scripts/prepare-sidecars.sh" release
npm run tauri --workspace @mcg/desktop -- build --config src-tauri/tauri.release.conf.json --bundles deb,appimage
node scripts/write-release-manifest.mjs target/release/bundle/deb target/release/bundle/appimage
