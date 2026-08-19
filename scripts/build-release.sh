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
bash "$repository_root/scripts/prepare-linux-sandbox.sh"
tauri_config=(--config src-tauri/tauri.release.conf.json)
updater_config_directory=""
loadedSigningKeyFromPath=0
cleanup_updater_config() {
  if [[ "$loadedSigningKeyFromPath" == 1 ]]; then
    unset TAURI_SIGNING_PRIVATE_KEY
  fi
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
  if [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(<"$TAURI_SIGNING_PRIVATE_KEY_PATH")"
    unset TAURI_SIGNING_PRIVATE_KEY_PATH
    loadedSigningKeyFromPath=1
  fi
  updater_config_directory="$(mktemp -d "${TMPDIR:-/tmp}/mcg-updater-build.XXXXXX")"
  updater_config="$updater_config_directory/updater.json"
  node scripts/write-updater-build-config.mjs "$updater_config"
  tauri_config+=(--config src-tauri/tauri.updater.conf.json --config "$updater_config")
fi
npm run tauri --workspace @mcg/desktop -- build --ci "${tauri_config[@]}" --bundles deb,appimage
bash "$repository_root/scripts/harden-linux-deb.sh" "$repository_root/target/release/bundle/deb"
node scripts/write-release-manifest.mjs target/release/bundle/deb target/release/bundle/appimage
