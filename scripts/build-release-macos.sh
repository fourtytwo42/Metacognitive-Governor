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
tauri_config=(--config src-tauri/tauri.release.conf.json)
updater_config_directory=""
cleanup_updater_config() {
  if [[ -n "$updater_config_directory" ]]; then
    rm -rf -- "$updater_config_directory"
  fi
}
trap cleanup_updater_config EXIT
updater_requested=0
for name in MCG_UPDATER_ENDPOINT MCG_UPDATER_PUBKEY TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PATH; do
  [[ -n "${!name:-}" ]] && updater_requested=1
done
if [[ "$updater_requested" == 1 ]]; then
  for name in MCG_UPDATER_ENDPOINT MCG_UPDATER_PUBKEY; do
    [[ -n "${!name:-}" ]] || { echo "Signed updater builds require $name." >&2; exit 1; }
  done
  key_sources=0
  [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]] && ((key_sources += 1))
  [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]] && ((key_sources += 1))
  [[ "$key_sources" == 1 ]] || { echo "Signed updater builds require exactly one of TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH." >&2; exit 1; }
  node scripts/check-updater-config.mjs
  updater_config_directory="$(mktemp -d "${TMPDIR:-/tmp}/mcg-updater-build.XXXXXX")"
  updater_config="$updater_config_directory/updater.json"
  node scripts/write-updater-build-config.mjs "$updater_config"
  tauri_config+=(--config src-tauri/tauri.updater.conf.json --config "$updater_config")
fi
npm run tauri --workspace @mcg/desktop -- build --ci "${tauri_config[@]}" --bundles app

case "$(uname -m)" in
  x86_64) bundle_arch=x64 ;;
  arm64|aarch64) bundle_arch=aarch64 ;;
  *) echo "Unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac
version="$(node -p "require('./apps/desktop/src-tauri/tauri.conf.json').version")"
dmg_directory="$repository_root/target/release/bundle/dmg"
if [[ "$dmg_directory" != "$repository_root/target/release/bundle/dmg" || "$dmg_directory" == "/" ]]; then
  echo "Refusing unsafe macOS disk-image output cleanup: $dmg_directory" >&2
  exit 1
fi
rm -rf -- "$dmg_directory"
mkdir -p "$dmg_directory"
dmg="$dmg_directory/MCG_${version}_${bundle_arch}.dmg"
"$repository_root/scripts/create-macos-dmg.sh" \
  "$repository_root/target/release/bundle/macos/MCG.app" \
  "$dmg"
node scripts/write-release-manifest.mjs target/release/bundle/dmg
