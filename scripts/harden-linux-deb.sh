#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Linux package hardening must run on Linux." >&2
  exit 1
fi
for command in dpkg-deb install sed sha256sum; do
  command -v "$command" >/dev/null || {
    echo "Missing Linux package hardening dependency: $command" >&2
    exit 1
  }
done

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
deb_directory="${1:-$repository_root/target/release/bundle/deb}"
sandbox_directory="$repository_root/target/release/linux-sandbox"
profile="$repository_root/packaging/linux/apparmor.d/metacognitive-governor-bwrap"
[[ -f "$sandbox_directory/SHA256SUMS" && -f "$profile" ]] || {
  echo "Prepared Bubblewrap payload or AppArmor profile is missing." >&2
  exit 1
}
(
  cd "$sandbox_directory"
  sha256sum -c SHA256SUMS
)

mapfile -t packages < <(find "$deb_directory" -maxdepth 1 -type f -name '*.deb' -print)
[[ "${#packages[@]}" -eq 1 ]] || {
  echo "Expected exactly one Debian package in $deb_directory." >&2
  exit 1
}
package="${packages[0]}"
work_directory="$(mktemp -d "$repository_root/target/release/mcg-deb-hardening.XXXXXX")"
cleanup() {
  rm -rf -- "$work_directory"
}
trap cleanup EXIT

root="$work_directory/root"
dpkg-deb -R "$package" "$root"
install -D -m 0755 "$sandbox_directory/mcg-bwrap" "$root/usr/lib/MCG/mcg-bwrap"
install -D -m 0644 "$profile" "$root/etc/apparmor.d/metacognitive-governor-bwrap"
install -D -m 0644 \
  "$sandbox_directory/Bubblewrap-GPL-2.0-or-later.txt" \
  "$root/usr/lib/MCG/licenses/Bubblewrap-GPL-2.0-or-later.txt"
install -D -m 0644 \
  "$sandbox_directory/bubblewrap-v0.11.2.tar.gz" \
  "$root/usr/lib/MCG/sources/bubblewrap-v0.11.2.tar.gz"

control="$root/DEBIAN/control"
if ! grep -Eq '^Depends:.*(^|, )apparmor([ (>,]|$)' "$control"; then
  sed -i '/^Depends:/ s/$/, apparmor (>= 4.0.1), libcap2, libselinux1, libpcre2-8-0/' "$control"
fi

postinst="$root/DEBIAN/postinst"
postinst_body="$work_directory/postinst.body"
if [[ -f "$postinst" ]]; then
  sed '1{/^#!/d;}' "$postinst" >"$postinst_body"
else
  : >"$postinst_body"
fi
cat >"$postinst" <<'SCRIPT'
#!/bin/sh
set -e
if [ -x /usr/sbin/apparmor_parser ] && [ -d /sys/kernel/security/apparmor ]; then
  /usr/sbin/apparmor_parser -r /etc/apparmor.d/metacognitive-governor-bwrap
fi
SCRIPT
cat "$postinst_body" >>"$postinst"
chmod 0755 "$postinst"

prerm="$root/DEBIAN/prerm"
prerm_body="$work_directory/prerm.body"
if [[ -f "$prerm" ]]; then
  sed '1{/^#!/d;}' "$prerm" >"$prerm_body"
else
  : >"$prerm_body"
fi
cat >"$prerm" <<'SCRIPT'
#!/bin/sh
set -e
if [ "${1:-}" = remove ] || [ "${1:-}" = deconfigure ]; then
  if [ -x /usr/sbin/apparmor_parser ] && [ -d /sys/kernel/security/apparmor ]; then
    /usr/sbin/apparmor_parser -R /etc/apparmor.d/metacognitive-governor-bwrap 2>/dev/null || true
  fi
fi
SCRIPT
cat "$prerm_body" >>"$prerm"
chmod 0755 "$prerm"

rebuilt="$work_directory/$(basename "$package")"
dpkg-deb --root-owner-group --build "$root" "$rebuilt" >/dev/null
mv -f -- "$rebuilt" "$package"

contents="$work_directory/package-contents.txt"
dpkg-deb --contents "$package" >"$contents"
grep -Eq 'usr/lib/MCG/mcg-bwrap$' "$contents"
grep -Eq 'etc/apparmor.d/metacognitive-governor-bwrap$' "$contents"
printf '%s\n' "$package"
