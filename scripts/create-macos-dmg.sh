#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS disk images must be created on macOS." >&2
  exit 1
fi

source_app="${1:-}"
output_dmg="${2:-}"
[[ -d "$source_app" && "$source_app" == *.app ]] || { echo "Missing source app bundle: $source_app" >&2; exit 1; }
app_name="$(basename "$source_app")"
[[ "$app_name" == "MetaCog.app" || "$app_name" == "MCG.app" ]] || {
  echo "Source app bundle must use the current MetaCog.app or compatibility MCG.app name." >&2
  exit 1
}
[[ "$output_dmg" == *.dmg ]] || { echo "Output path must end in .dmg." >&2; exit 1; }
[[ ! -e "$output_dmg" ]] || { echo "Refusing to replace existing disk image: $output_dmg" >&2; exit 1; }
for command in ditto hdiutil; do
  command -v "$command" >/dev/null || { echo "Missing disk-image dependency: $command" >&2; exit 1; }
done

temporary_parent="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/mcg-macos-dmg-XXXXXX")"
staging="$temporary_root/staging"
cleanup() {
  local resolved_parent
  resolved_parent="$(cd "$(dirname "$temporary_root")" && pwd -P)"
  if [[ "$resolved_parent" != "$temporary_parent" || "$(basename "$temporary_root")" != mcg-macos-dmg-* || "$temporary_root" == "/" ]]; then
    echo "Refusing unsafe temporary disk-image cleanup: $temporary_root" >&2
    return 1
  fi
  rm -rf -- "$temporary_root"
}
trap cleanup EXIT

mkdir -p "$staging" "$(dirname "$output_dmg")"
ditto "$source_app" "$staging/$app_name"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -fs HFS+ -format UDZO -imagekey zlib-level=9 \
  -volname MetaCog -srcfolder "$staging" "$output_dmg"
hdiutil verify "$output_dmg"
[[ -s "$output_dmg" ]] || { echo "Disk image was not created: $output_dmg" >&2; exit 1; }
printf 'Created %s\n' "$output_dmg"
