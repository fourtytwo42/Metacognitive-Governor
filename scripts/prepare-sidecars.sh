#!/usr/bin/env bash
set -euo pipefail

profile="${1:-release}"
if [[ "$profile" != "debug" && "$profile" != "release" ]]; then
  echo "usage: $0 [debug|release]" >&2
  exit 2
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_triple="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$target_triple" ]]; then
  echo "Rust did not report a host target triple." >&2
  exit 1
fi

cargo_arguments=(build -p mcg-cli -p mcg-shadow-runner)
if [[ "$profile" == "release" || ! -f "$repository_root/target/debug/mcg-daemon" ]]; then
  cargo_arguments+=(-p mcg-daemon)
fi
if [[ "$profile" == "release" ]]; then
  cargo_arguments+=(--release)
fi
cargo "${cargo_arguments[@]}"

source_directory="$repository_root/target/$profile"
sidecar_directory="$repository_root/apps/desktop/src-tauri/binaries"
mkdir -p "$sidecar_directory"
for mapping in mcg:mcg mcg-daemon:mcg-service mcg-shadow-runner:mcg-shadow-runner; do
  binary="${mapping%%:*}"
  sidecar="${mapping##*:}"
  source="$source_directory/$binary"
  if [[ ! -f "$source" ]]; then
    echo "Required sidecar is missing: $source" >&2
    exit 1
  fi
  cp "$source" "$sidecar_directory/$sidecar-$target_triple"
  chmod 0755 "$sidecar_directory/$sidecar-$target_triple"
done

printf '%s\n' "$target_triple"
