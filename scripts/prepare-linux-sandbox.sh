#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "The bundled Linux sandbox currently supports x86_64 Linux only." >&2
  exit 1
fi

for command in git meson ninja sha256sum stat; do
  command -v "$command" >/dev/null || {
    echo "Missing Bubblewrap build dependency: $command" >&2
    exit 1
  }
done

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_directory="$repository_root/target/release/linux-sandbox"
mkdir -p "$repository_root/target/release" "$output_directory"
work_directory="$(mktemp -d "$repository_root/target/release/linux-sandbox-build.XXXXXX")"
cleanup() {
  rm -rf -- "$work_directory"
}
trap cleanup EXIT

upstream="https://github.com/containers/bubblewrap.git"
commit="1b80120ef26a28e065e67f89bfef873f13bdd317"
source_directory="$work_directory/source"
git init -q "$source_directory"
git -C "$source_directory" remote add origin "$upstream"
git -C "$source_directory" fetch -q --depth 1 origin "$commit"
[[ "$(git -C "$source_directory" rev-parse FETCH_HEAD)" == "$commit" ]] || {
  echo "Bubblewrap source did not resolve to the reviewed commit." >&2
  exit 1
}
git -C "$source_directory" checkout -q --detach FETCH_HEAD

meson setup "$work_directory/build" "$source_directory" \
  -Dman=disabled -Dbash_completion=disabled -Dzsh_completion=disabled \
  -Dselinux=enabled -Dsupport_setuid=false -Dtests=true
meson compile -C "$work_directory/build"
meson test -C "$work_directory/build" --print-errorlogs

binary="$work_directory/build/bwrap"
[[ -f "$binary" && -x "$binary" ]] || {
  echo "The pinned Bubblewrap build did not produce an executable." >&2
  exit 1
}
[[ "$($binary --version)" == "bubblewrap 0.11.2" ]] || {
  echo "The pinned Bubblewrap build reported an unexpected version." >&2
  exit 1
}
install -m 0755 "$binary" "$output_directory/mcg-bwrap"
[[ "$(stat -c '%a' "$output_directory/mcg-bwrap")" == "755" ]] || {
  echo "The staged Bubblewrap executable is not mode 0755." >&2
  exit 1
}
install -m 0644 "$source_directory/COPYING" "$output_directory/Bubblewrap-GPL-2.0-or-later.txt"
git -C "$source_directory" archive --format=tar.gz \
  --prefix=bubblewrap-v0.11.2/ -o "$output_directory/bubblewrap-v0.11.2.tar.gz" "$commit"
(
  cd "$output_directory"
  sha256sum mcg-bwrap Bubblewrap-GPL-2.0-or-later.txt bubblewrap-v0.11.2.tar.gz >SHA256SUMS
)

printf '%s\n' "$output_directory"
