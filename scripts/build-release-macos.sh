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
npm run tauri --workspace @mcg/desktop -- build --config src-tauri/tauri.release.conf.json --bundles app

case "$(uname -m)" in
  x86_64) bundle_arch=x64 ;;
  arm64|aarch64) bundle_arch=aarch64 ;;
  *) echo "Unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac
version="$(node -p "require('./apps/desktop/src-tauri/tauri.conf.json').version")"
dmg="$repository_root/target/release/bundle/dmg/MCG_${version}_${bundle_arch}.dmg"
"$repository_root/scripts/create-macos-dmg.sh" \
  "$repository_root/target/release/bundle/macos/MCG.app" \
  "$dmg"
node scripts/write-release-manifest.mjs target/release/bundle/dmg
