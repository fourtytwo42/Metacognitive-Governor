#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS release builds must run on macOS." >&2
  exit 1
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"
node scripts/check-release-source.mjs
"$repository_root/scripts/prepare-sidecars.sh" release
npm run tauri --workspace @mcg/desktop -- build --config src-tauri/tauri.release.conf.json --bundles app,dmg
node scripts/write-release-manifest.mjs target/release/bundle/dmg
