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

cargo_arguments=(build -p mcg-cli -p mcg-agent-mcp -p mcg-action-runner -p mcg-process-runner -p mcg-provider-runner -p mcg-runtime-worker -p mcg-plugin-runner -p mcg-shadow-runner -p mcg-updater)
if [[ "$profile" == "release" || ! -f "$repository_root/target/debug/mcg-protected-rootd" ]]; then
  cargo_arguments+=(-p mcg-protected-rootd)
fi
if [[ "$profile" == "release" ]]; then
  cargo_arguments+=(--release)
fi
cargo "${cargo_arguments[@]}"

source_directory="$repository_root/target/$profile"
sidecar_directory="$repository_root/apps/desktop/src-tauri/binaries"
mkdir -p "$sidecar_directory"
for mapping in mcg:mcg mcg-protected-rootd:mcg-service mcg-agent-mcp:mcg-agent-mcp mcg-action-runner:mcg-action-runner mcg-process-runner:mcg-process-runner mcg-provider-runner:mcg-provider-runner mcg-runtime-worker:mcg-runtime-worker mcg-plugin-runner:mcg-plugin-runner mcg-shadow-runner:mcg-shadow-runner mcg-updater:mcg-updater; do
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
